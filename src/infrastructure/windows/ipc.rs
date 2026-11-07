use std::ffi::OsString;
use std::marker::PhantomData;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use windows_sys::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_IO_PENDING, ERROR_MORE_DATA, ERROR_PIPE_CONNECTED, GetLastError,
    HANDLE, HLOCAL, INVALID_HANDLE_VALUE, LocalFree, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::{
    GetTokenInformation, RevertToSelf, SECURITY_ATTRIBUTES, SID_AND_ATTRIBUTES, TOKEN_USER,
    TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
    ImpersonateNamedPipeClient, PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_MESSAGE, PIPE_WAIT, PeekNamedPipe, SetNamedPipeHandleState, WaitNamedPipeW,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, GetCurrentThread, OpenThreadToken, SetEvent,
    WaitForMultipleObjects, WaitForSingleObject,
};

use super::single_instance::InstanceIdentity;

const SDDL_STRING_DESCRIPTOR: u32 = 1;
const TOKEN_QUERY: u32 = 0x0008;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const OPEN_EXISTING: u32 = 3;
const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
const MAX_PAYLOAD: usize = 4096;
const FRAME_CAPACITY: usize = 4 + MAX_PAYLOAD;
const ACK_CAPACITY: usize = 128;
const CONNECT_BUDGET: Duration = Duration::from_millis(1500);
const RETRY_BACKOFF: Duration = Duration::from_millis(25);
const ERROR_PIPE_BUSY: u32 = 231;

/// The only commands the main instance accepts over IPC; everything else,
/// including shell strings, paths or arguments, is rejected before dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IpcCommand {
    Show,
    Hide,
    Toggle,
    ShowAndFocus,
    OpenSearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IpcSource {
    SecondaryInstance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcMessage {
    version: u32,
    command: IpcCommand,
    source: IpcSource,
}

impl IpcMessage {
    pub fn new(command: IpcCommand) -> Self {
        Self {
            version: 1,
            command,
            source: IpcSource::SecondaryInstance,
        }
    }

    pub fn command(&self) -> IpcCommand {
        self.command
    }

    pub fn parse(payload: &[u8]) -> Result<Self, IpcError> {
        if payload.is_empty() || payload.len() > MAX_PAYLOAD {
            return Err(IpcError::BadFrame("payload length out of range"));
        }
        let text =
            std::str::from_utf8(payload).map_err(|_| IpcError::BadFrame("payload is not UTF-8"))?;
        serde_json::from_str(text)
            .map_err(|_| IpcError::BadFrame("payload is not a valid IPC message"))
    }

    pub fn encode(&self) -> Result<Vec<u8>, IpcError> {
        let payload =
            serde_json::to_vec(self).map_err(|_| IpcError::BadFrame("serialization failed"))?;
        if payload.is_empty() || payload.len() > MAX_PAYLOAD {
            return Err(IpcError::BadFrame("serialized payload out of range"));
        }
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(&payload);
        Ok(frame)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    /// The peer violated the framing/protocol rules; the connection is unusable.
    BadFrame(&'static str),
    /// The pipe name did not resolve or stayed busy within the budget.
    Unavailable,
    /// The shared 1500 ms transaction deadline expired.
    TimedOut,
    /// A Win32 call failed; the payload is GetLastError() at the failure point.
    Failed(u32),
    /// The client identity did not match the server user (server side).
    RejectedForeignClient,
}

pub const FRAME_CAPACITY_LIMIT: usize = FRAME_CAPACITY;
pub const CONNECT_BUDGET_MS: u128 = CONNECT_BUDGET.as_millis();

fn wide(value: &str) -> Vec<u16> {
    OsString::from(value).encode_wide().chain(Some(0)).collect()
}

fn sid_from_token(token: HANDLE) -> Result<Vec<u8>, u32> {
    let mut needed = 0_u32;
    // SAFETY: NULL buffer requests the required size; GetLastError read at once.
    unsafe {
        let _ = GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
        if needed == 0 {
            return Err(GetLastError());
        }
    }
    let mut buffer = vec![0_u8; needed as usize];
    // SAFETY: buffer holds `needed` writable bytes for the token query.
    unsafe {
        if GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
            needed,
            &mut needed,
        ) == 0
        {
            return Err(GetLastError());
        }
    }
    let token_user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
    let attributes: &SID_AND_ATTRIBUTES = unsafe { &*(&token_user.User as *const _) };
    let sid = unsafe { &*attributes.Sid.cast::<windows_sys::Win32::Security::SID>() };
    // SAFETY: SubAuthorities follow the header inside the token allocation.
    let sub_authorities = unsafe {
        core::slice::from_raw_parts(sid.SubAuthority.as_ptr(), sid.SubAuthorityCount as usize)
    };
    let mut bytes = Vec::with_capacity(4 + 6 + sub_authorities.len() * 4);
    bytes.extend_from_slice(&sid.Revision.to_le_bytes());
    bytes.extend_from_slice(&sid.IdentifierAuthority.Value);
    for authority in sub_authorities {
        bytes.extend_from_slice(&authority.to_le_bytes());
    }
    Ok(bytes)
}

fn current_user_sid_bytes() -> Result<Vec<u8>, u32> {
    let mut token = HANDLE::default();
    // SAFETY: Process pseudo-handle; the token handle is closed on every path.
    unsafe {
        if windows_sys::Win32::System::Threading::OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return Err(GetLastError());
        }
    }
    let result = sid_from_token(token);
    // SAFETY: Token handle from OpenProcessToken is closed exactly once.
    unsafe {
        windows_sys::Win32::Foundation::CloseHandle(token);
    }
    result
}

/// Converts raw SID bytes (Revision | IdentifierAuthority | SubAuthorities) into
/// the S-1-a-b-c string form used by SDDL. No Win32 string API is required.
fn sid_bytes_to_string(bytes: &[u8]) -> String {
    let authority = {
        let mut value = 0_u64;
        for byte in &bytes[1..7] {
            value = (value << 8) | u64::from(*byte);
        }
        value
    };
    let mut sddl = format!("S-1-{authority}");
    let mut offset = 7;
    while offset + 4 <= bytes.len() {
        let rid = u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("in-bounds"));
        sddl.push('-');
        sddl.push_str(&rid.to_string());
        offset += 4;
    }
    sddl
}

/// D:P(A;;GA;;;<current user>)(A;;GA;;;SY) - only this user (plus SYSTEM) may
/// touch the pipe; the baseline SYSTEM allowance is a second ACE, while every
/// other principal, including remote ones, stays denied by the protected DACL.
fn security_descriptor_sddl() -> Result<String, u32> {
    let sid = current_user_sid_bytes()?;
    Ok(format!(
        "D:P(A;;GA;;;{})(A;;GA;;;SY)",
        sid_bytes_to_string(&sid)
    ))
}

/// Frees the security descriptor produced by ConvertStringSecurityDescriptor...
struct SecurityDescriptorGuard {
    descriptor: HLOCAL,
}

impl Drop for SecurityDescriptorGuard {
    fn drop(&mut self) {
        // SAFETY: The descriptor came from ConvertStringSecurityDescriptor... and
        // must be freed with LocalFree exactly once.
        unsafe {
            LocalFree(self.descriptor);
        }
    }
}

fn build_security_attributes(sddl: &str) -> Result<SECURITY_ATTRIBUTES, u32> {
    let units = wide(sddl);
    let mut descriptor: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: Valid SDDL string and out pointer; SDDL_REVISION_1 == 1. The returned
    // descriptor is freed by SecurityDescriptorGuard on every path.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            units.as_ptr(),
            SDDL_STRING_DESCRIPTOR,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(GetLastError());
        }
    }
    Ok(SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    })
}

/// Every transaction (connect, request and ACK) shares this budget.
const CANCEL_GRACE: Duration = Duration::from_millis(100);
const SHUTDOWN_BUDGET: Duration = Duration::from_millis(500);

fn own_pipe(handle: HANDLE) -> Result<OwnedHandle, IpcError> {
    // CreateFileW/CreateNamedPipeW fail with -1, not NULL.
    if handle == INVALID_HANDLE_VALUE {
        Err(IpcError::Failed(unsafe { GetLastError() }))
    } else {
        // SAFETY: Successful file/pipe creation transfers ownership to us.
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
}

struct StopEvent(OwnedHandle);

impl StopEvent {
    fn new() -> Result<Self, IpcError> {
        // SAFETY: Unnamed, manual-reset event, initially unsignaled.
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if handle.is_null() {
            return Err(IpcError::Failed(unsafe { GetLastError() }));
        }
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(handle) }))
    }

    fn handle(&self) -> HANDLE {
        self.0.as_raw_handle()
    }

    fn signal(&self) {
        // SAFETY: The event remains owned for the duration of this call.
        unsafe { SetEvent(self.handle()) };
    }

    fn is_set(&self) -> bool {
        unsafe { WaitForSingleObject(self.handle(), 0) == WAIT_OBJECT_0 }
    }
}

fn remaining_ms(deadline: Instant) -> u32 {
    let remaining = deadline.saturating_duration_since(Instant::now());
    // Round up, but never use INFINITE or a zero timeout (special for pipe waits).
    remaining
        .as_millis()
        .saturating_add(1)
        .min(u128::from(u32::MAX - 1)) as u32
}

/// Own every kernel-referenced allocation until completion, even after cancel.
struct PendingIo {
    pipe: Arc<OwnedHandle>,
    event: StopEvent,
    overlapped: Box<OVERLAPPED>,
    buffer: Vec<u8>,
}

impl PendingIo {
    fn new(pipe: &Arc<OwnedHandle>, buffer: Vec<u8>) -> Result<Self, IpcError> {
        let event = StopEvent::new()?;
        let mut overlapped = Box::<OVERLAPPED>::default();
        overlapped.hEvent = event.handle();
        Ok(Self {
            pipe: Arc::clone(pipe),
            event,
            overlapped,
            buffer,
        })
    }

    fn finish(
        self,
        started: i32,
        deadline: Instant,
        stop: Option<&StopEvent>,
    ) -> Result<Vec<u8>, IpcError> {
        let mut this = self;
        if started == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_IO_PENDING {
                return Err(io_error(error));
            }
        }
        if started == 0 {
            let wait = if let Some(stop) = stop {
                let handles = [stop.handle(), this.event.handle()];
                unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, remaining_ms(deadline)) }
            } else {
                let wait =
                    unsafe { WaitForSingleObject(this.event.handle(), remaining_ms(deadline)) };
                if wait == WAIT_OBJECT_0 {
                    WAIT_OBJECT_0 + 1
                } else {
                    wait
                }
            };
            if wait != WAIT_OBJECT_0 + 1 {
                let error = if wait == WAIT_TIMEOUT || wait == WAIT_OBJECT_0 {
                    IpcError::TimedOut
                } else {
                    IpcError::Failed(unsafe { GetLastError() })
                };
                // SAFETY: Cancel only this operation, retaining its event, buffer,
                // OVERLAPPED and pipe until Windows confirms completion.
                unsafe { CancelIoEx(this.pipe.as_raw_handle(), &*this.overlapped) };
                let cancelled = unsafe {
                    WaitForSingleObject(this.event.handle(), CANCEL_GRACE.as_millis() as u32)
                };
                if cancelled != WAIT_OBJECT_0 {
                    // A broken driver must not turn a deadline into an unbounded
                    // wait or a use-after-free. Quarantine this one allocation;
                    // a normal Windows named-pipe cancellation completes at once.
                    std::mem::forget(this);
                }
                return Err(error);
            }
        }
        let mut transferred = 0;
        // SAFETY: Completion is signaled (or the operation completed inline).
        if unsafe {
            GetOverlappedResult(
                this.pipe.as_raw_handle(),
                &*this.overlapped,
                &mut transferred,
                0,
            )
        } == 0
        {
            return Err(io_error(unsafe { GetLastError() }));
        }
        this.buffer.truncate(transferred as usize);
        Ok(this.buffer)
    }
}

fn io_error(error: u32) -> IpcError {
    if error == ERROR_MORE_DATA {
        IpcError::BadFrame("message exceeds frame capacity")
    } else {
        IpcError::Failed(error)
    }
}

fn check_deadline(deadline: Instant, stop: Option<&StopEvent>) -> Result<(), IpcError> {
    if Instant::now() >= deadline || stop.is_some_and(StopEvent::is_set) {
        Err(IpcError::TimedOut)
    } else {
        Ok(())
    }
}

fn write_exact(
    pipe: &Arc<OwnedHandle>,
    buffer: &[u8],
    deadline: Instant,
    stop: Option<&StopEvent>,
) -> Result<(), IpcError> {
    check_deadline(deadline, stop)?;
    let mut operation = PendingIo::new(pipe, buffer.to_vec())?;
    // SAFETY: Heap allocations remain stable while the operation is pending.
    let started = unsafe {
        WriteFile(
            pipe.as_raw_handle(),
            operation.buffer.as_ptr(),
            buffer.len() as u32,
            std::ptr::null_mut(),
            &mut *operation.overlapped,
        )
    };
    let written = operation.finish(started, deadline, stop)?;
    if written.len() != buffer.len() {
        return Err(IpcError::BadFrame("incomplete pipe write"));
    }
    Ok(())
}

fn read_message(
    pipe: &Arc<OwnedHandle>,
    capacity: usize,
    deadline: Instant,
    stop: Option<&StopEvent>,
) -> Result<Vec<u8>, IpcError> {
    check_deadline(deadline, stop)?;
    let mut operation = PendingIo::new(pipe, vec![0; capacity])?;
    // SAFETY: Heap allocations remain stable while the operation is pending.
    let started = unsafe {
        ReadFile(
            pipe.as_raw_handle(),
            operation.buffer.as_mut_ptr(),
            capacity as u32,
            std::ptr::null_mut(),
            &mut *operation.overlapped,
        )
    };
    let buffer = operation.finish(started, deadline, stop)?;
    if buffer.len() < 4 {
        return Err(IpcError::BadFrame("frame shorter than its length prefix"));
    }
    Ok(buffer)
}

pub struct PipeServer {
    pub name: String,
    events: Option<Receiver<IpcCommand>>,
    thread: Option<JoinHandle<()>>,
    stop: Arc<StopEvent>,
}

impl PipeServer {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn try_recv(&self) -> Result<IpcCommand, TryRecvError> {
        match self.events.as_ref() {
            Some(receiver) => receiver.try_recv(),
            None => Err(TryRecvError::Disconnected),
        }
    }

    pub fn drain(&self) -> Vec<IpcCommand> {
        let mut commands = Vec::new();
        while let Ok(command) = self.try_recv() {
            commands.push(command);
        }
        commands
    }
}

impl Drop for PipeServer {
    fn drop(&mut self) {
        self.stop.signal();
        if let Some(thread) = self.thread.take() {
            // The stop event wakes accept/read/write immediately; that thread
            // cancels its current operation. Join only after a bounded OS wait.
            let exited = unsafe {
                WaitForSingleObject(thread.as_raw_handle(), SHUTDOWN_BUDGET.as_millis() as u32)
            };
            if exited == WAIT_OBJECT_0 {
                let _ = thread.join();
            }
            // A failed kernel/driver cannot make Drop wait forever. The worker
            // retains all its owned state if detached by this fallback.
        }
    }
}

/// Ends this thread's impersonation on every path, including early returns.
struct ImpersonationGuard;

impl Drop for ImpersonationGuard {
    fn drop(&mut self) {
        // SAFETY: RevertToSelf ends this thread's impersonation.
        unsafe {
            let _ = RevertToSelf();
        }
    }
}

/// The client must already have written its frame, making it the pipe's last
/// writer for impersonation. Returns the client PID as audit evidence only.
fn client_matches_server_user(handle: HANDLE, server_sid: &[u8]) -> Result<(bool, u32), IpcError> {
    let mut client_pid = 0_u32;
    // SAFETY: Valid pipe handle and out PID; the PID is diagnostic evidence only.
    unsafe {
        if GetNamedPipeClientProcessId(handle, &mut client_pid) == 0 {
            return Err(IpcError::Failed(GetLastError()));
        }
    }
    // SAFETY: Impersonating a connected named-pipe client on this thread; the
    // guard reverts unconditionally, including on the error paths below.
    unsafe {
        if ImpersonateNamedPipeClient(handle) == 0 {
            return Err(IpcError::Failed(GetLastError()));
        }
    }
    let _guard = ImpersonationGuard;
    let mut thread_token = HANDLE::default();
    // SAFETY: Current thread with query rights; flag TRUE keeps the token usable
    // only for queries while impersonated.
    unsafe {
        if OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut thread_token) == 0 {
            return Err(IpcError::Failed(GetLastError()));
        }
    }
    // SAFETY: The thread token handle is closed on every path below.
    let comparison = (|| {
        let client_sid = sid_from_token(thread_token).map_err(IpcError::Failed)?;
        Ok((client_sid == server_sid, client_pid))
    })();
    unsafe {
        windows_sys::Win32::Foundation::CloseHandle(thread_token);
    }
    drop(_guard);
    comparison
}

fn respond(
    pipe: &Arc<OwnedHandle>,
    accepted: bool,
    deadline: Instant,
    stop: &StopEvent,
) -> Result<(), IpcError> {
    // A minimal versioned ack keeps duplex semantics meaningful; the client can
    // distinguish "written" from "validated and accepted".
    let ack = if accepted {
        serde_json::json!({"version":1,"status":"accepted"})
    } else {
        serde_json::json!({"version":1,"status":"rejected"})
    };
    let payload =
        serde_json::to_vec(&ack).map_err(|_| IpcError::BadFrame("ack serialization failed"))?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
    write_exact(pipe, &frame, deadline, Some(stop))?;
    // Write completion only means buffered, not delivered. Never disconnect
    // immediately or FlushFileBuffers indefinitely. Preserve the ACK until the
    // peer closes (our clients close after reading it) or this session expires.
    // PeekNamedPipe is safe here: this overlapped handle has no outstanding I/O.
    loop {
        check_deadline(deadline, Some(stop))?;
        if unsafe {
            PeekNamedPipe(
                pipe.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Ok(());
        }
        unsafe { WaitForSingleObject(stop.handle(), remaining_ms(deadline).min(10)) };
    }
}

fn parse_ack(frame: &[u8]) -> Result<bool, IpcError> {
    if frame.len() < 4 {
        return Err(IpcError::BadFrame("ack shorter than its length prefix"));
    }
    let payload_length = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    if payload_length == 0 || payload_length > frame.len() - 4 {
        return Err(IpcError::BadFrame("ack payload out of range"));
    }
    let text = std::str::from_utf8(&frame[4..4 + payload_length])
        .map_err(|_| IpcError::BadFrame("ack is not UTF-8"))?;
    let status = serde_json::from_str::<serde_json::Value>(text)
        .map_err(|_| IpcError::BadFrame("ack is not valid JSON"))?;
    Ok(status.get("status").and_then(|value| value.as_str()) == Some("accepted"))
}

fn serve_client(
    pipe: &Arc<OwnedHandle>,
    server_sid: &[u8],
    events: &std::sync::mpsc::Sender<IpcCommand>,
    stop: &StopEvent,
) {
    let deadline = Instant::now() + CONNECT_BUDGET;
    let frame = match read_message(pipe, FRAME_CAPACITY, deadline, Some(stop)) {
        Ok(frame) => frame,
        Err(error) => {
            let _ = respond(pipe, false, deadline, stop);
            eprintln!("IPC frame rejected: {error:?}");
            return;
        }
    };
    let payload_length = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    if payload_length == 0 || payload_length > MAX_PAYLOAD || 4 + payload_length > frame.len() {
        let _ = respond(pipe, false, deadline, stop);
        eprintln!("IPC length prefix rejected: {payload_length}");
        return;
    }
    let identity = match client_matches_server_user(pipe.as_raw_handle(), server_sid) {
        Ok((matches, pid)) => {
            if !matches {
                eprintln!("IPC client {pid} rejected: foreign user");
                let _ = respond(pipe, false, deadline, stop);
                return;
            }
            pid
        }
        Err(error) => {
            eprintln!("IPC client identity check failed: {error:?}");
            let _ = respond(pipe, false, deadline, stop);
            return;
        }
    };
    let message = match IpcMessage::parse(&frame[4..4 + payload_length]) {
        Ok(message) => message,
        Err(error) => {
            eprintln!("IPC message from {identity} rejected: {error:?}");
            let _ = respond(pipe, false, deadline, stop);
            return;
        }
    };
    if events.send(message.command()).is_err() {
        return;
    }
    if let Err(error) = respond(pipe, true, deadline, stop) {
        eprintln!("IPC ack failed: {error:?}");
    }
}

fn create_server_pipe(name: &str) -> Result<Arc<OwnedHandle>, IpcError> {
    let sddl = security_descriptor_sddl().map_err(IpcError::Failed)?;
    let attributes = build_security_attributes(&sddl).map_err(IpcError::Failed)?;
    let _descriptor_guard = SecurityDescriptorGuard {
        descriptor: attributes.lpSecurityDescriptor,
    };
    let name_units = wide(name);
    // SAFETY: Name and descriptor outlive the call. One per-user, local-only
    // message pipe, opened overlapped so every blocking operation is cancelable.
    let handle = unsafe {
        CreateNamedPipeW(
            name_units.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            FRAME_CAPACITY as u32,
            FRAME_CAPACITY as u32,
            0,
            &attributes,
        )
    };
    own_pipe(handle).map(Arc::new)
}

fn accept(pipe: &Arc<OwnedHandle>, deadline: Instant, stop: &StopEvent) -> Result<(), IpcError> {
    check_deadline(deadline, Some(stop))?;
    let mut operation = PendingIo::new(pipe, Vec::new())?;
    let started = unsafe { ConnectNamedPipe(pipe.as_raw_handle(), &mut *operation.overlapped) };
    if started == 0 && unsafe { GetLastError() } == ERROR_PIPE_CONNECTED {
        return Ok(());
    }
    operation.finish(started, deadline, Some(stop)).map(|_| ())
}

fn pump(
    pipe: Arc<OwnedHandle>,
    stop: Arc<StopEvent>,
    events: std::sync::mpsc::Sender<IpcCommand>,
    server_sid: Vec<u8>,
) {
    while !stop.is_set() {
        match accept(&pipe, Instant::now() + CONNECT_BUDGET, &stop) {
            Ok(()) => serve_client(&pipe, &server_sid, &events, &stop),
            Err(IpcError::TimedOut) => {}
            Err(error) => {
                eprintln!("IPC accept failed: {error:?}");
                break;
            }
        }
        if stop.is_set() {
            break;
        }
        // ACK delivery has finished or the session deadline has expired.
        unsafe { DisconnectNamedPipe(pipe.as_raw_handle()) };
    }
}

fn start_server_named(name: String) -> Result<PipeServer, IpcError> {
    // Initialize before returning: SID, descriptor, event, pipe-name collision
    // and thread creation failures are visible to the existing caller.
    let server_sid = current_user_sid_bytes().map_err(IpcError::Failed)?;
    let stop = Arc::new(StopEvent::new()?);
    let pipe = create_server_pipe(&name)?;
    let (events_tx, events_rx) = channel();
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("quicknest-ipc".into())
        .spawn(move || pump(pipe, thread_stop, events_tx, server_sid))
        .map_err(|error| IpcError::Failed(error.raw_os_error().unwrap_or(0) as u32))?;
    Ok(PipeServer {
        name,
        events: Some(events_rx),
        thread: Some(thread),
        stop,
    })
}

/// Starts the IPC server; returns only once the secured pipe exists.
pub fn start_server() -> Result<PipeServer, IpcError> {
    let identity = InstanceIdentity::current().map_err(IpcError::Failed)?;
    start_server_named(identity.pipe_name())
}

fn connect_until(name: &str, deadline: Instant, flags: u32) -> Result<OwnedHandle, IpcError> {
    let units = wide(name);
    loop {
        check_deadline(deadline, None)?;
        // SAFETY: Local pipe name and read/write access, no inherited handle.
        let handle = unsafe {
            CreateFileW(
                units.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                flags,
                std::ptr::null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            return own_pipe(handle);
        }
        let error = unsafe { GetLastError() };
        check_deadline(deadline, None)?;
        match error {
            ERROR_FILE_NOT_FOUND => std::thread::sleep(
                RETRY_BACKOFF.min(deadline.saturating_duration_since(Instant::now())),
            ),
            ERROR_PIPE_BUSY => {
                // Only the remaining budget: never restart the full timeout.
                unsafe {
                    WaitNamedPipeW(
                        units.as_ptr(),
                        remaining_ms(deadline).min(RETRY_BACKOFF.as_millis() as u32),
                    )
                };
            }
            _ => return Err(IpcError::Failed(error)),
        }
    }
}

/// One-shot connection with a 1500 ms budget. Retains the historical synchronous
/// raw handle contract; the caller owns and must CloseHandle the result.
pub fn connect_client(name: &str) -> Result<HANDLE, IpcError> {
    use std::os::windows::io::IntoRawHandle;
    connect_until(name, Instant::now() + CONNECT_BUDGET, 0).map(IntoRawHandle::into_raw_handle)
}

fn connect_overlapped(name: &str, deadline: Instant) -> Result<Arc<OwnedHandle>, IpcError> {
    let pipe = connect_until(name, deadline, FILE_FLAG_OVERLAPPED)?;
    let mode = PIPE_READMODE_MESSAGE;
    if unsafe {
        SetNamedPipeHandleState(
            pipe.as_raw_handle(),
            &mode,
            std::ptr::null(),
            std::ptr::null(),
        )
    } == 0
    {
        return Err(IpcError::Failed(unsafe { GetLastError() }));
    }
    check_deadline(deadline, None)?;
    Ok(Arc::new(pipe))
}

fn send_frame_until(name: &str, frame: &[u8], deadline: Instant) -> Result<bool, IpcError> {
    let pipe = connect_overlapped(name, deadline)?;
    write_exact(&pipe, frame, deadline, None)?;
    parse_ack(&read_message(&pipe, ACK_CAPACITY, deadline, None)?)
}

/// Connects, writes one framed message and receives its ACK within one 1500 ms
/// deadline. ACK acceptance means validated and queued, not UI execution.
pub fn send_message(name: &str, message: &IpcMessage) -> Result<bool, IpcError> {
    let deadline = Instant::now() + CONNECT_BUDGET;
    send_frame_until(name, &message.encode()?, deadline)
}

/// Raw low-level send used by rejection tests; not part of the public protocol.
pub fn send_raw(name: &str, frame: &[u8]) -> Result<Option<bool>, IpcError> {
    let deadline = Instant::now() + CONNECT_BUDGET;
    if frame.len() > FRAME_CAPACITY {
        return Err(IpcError::BadFrame("raw frame exceeds capacity"));
    }
    send_frame_until(name, frame, deadline).map(Some)
}

// Kept for source compatibility with the original public type.
pub struct OverlappedPlaceholder {
    _private: PhantomData<()>,
}

#[cfg(test)]
#[path = "ipc_tests.rs"]
mod ipc_tests;

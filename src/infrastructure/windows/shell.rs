use std::error::Error;
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, GetLastError, HWND, LPARAM, PROPERTYKEY, WPARAM};
use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile, STGM_READ,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Shell::{
    IShellItem, IShellItem2, IShellLinkDataList, IShellLinkW, SEE_MASK_NOCLOSEPROCESS,
    SHCreateItemFromIDList, SHCreateItemFromParsingName, SHELLEXECUTEINFOW, SHGetPathFromIDListW,
    SIGDN_NORMALDISPLAY, SLDF_HAS_DARWINID, SLGP_FLAGS, SLGP_RAWPATH, ShellExecuteExW, ShellLink,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx,
    PostThreadMessageW, QS_ALLINPUT, TranslateMessage, WM_QUIT,
};
use windows::core::{Interface, PCWSTR};

const WAKEUP: u32 = 0x0400 + 0x7E57;
const RESOLVE_FLAGS: SLGP_FLAGS = SLGP_RAWPATH;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellLinkData {
    pub path: Option<PathBuf>,
    pub id_list_path: Option<PathBuf>,
    pub arguments: String,
    pub working_directory: String,
    pub icon_location: String,
    pub icon_index: i32,
}

#[derive(Debug)]
pub enum ShellOutcome {
    Link(ShellLinkData),
    Executed,
    Failed { hresult: i32 },
    TimedOut,
    Unavailable,
}

type Task = Box<dyn FnOnce() -> ShellOutcome + Send>;

pub struct ShellRequest {
    task: Task,
    reply: Sender<ShellOutcome>,
}

struct ComStaGuard {
    // Only S_OK constructs this guard; the value documents the acquired reference.
    #[expect(dead_code)]
    hresult: i32,
}

impl Drop for ComStaGuard {
    fn drop(&mut self) {
        // SAFETY: Built only after a successful CoInitializeEx on this STA thread.
        // The guard is !Send, so the paired release stays on the initializing thread.
        unsafe { CoUninitialize() };
    }
}

fn init_sta() -> Result<ComStaGuard, i32> {
    // SAFETY: Reserved pointer must be None. Flags match baseline 6.2 exactly.
    let hresult =
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
    if !hresult.is_ok() {
        return Err(hresult.0);
    }
    Ok(ComStaGuard { hresult: hresult.0 })
}

fn wide(value: &str) -> Vec<u16> {
    OsString::from(value).encode_wide().chain(Some(0)).collect()
}

fn from_wide(units: &[u16]) -> String {
    let len = units
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.len());
    OsString::from_wide(&units[..len])
        .to_string_lossy()
        .into_owned()
}

/// Runs one COM workload on the calling STA thread. Interfaces never leave it.
fn resolve_on_sta(path: &Path) -> ShellOutcome {
    let file = wide(&path.to_string_lossy());
    let link =
        match unsafe { CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        {
            Ok(value) => value,
            Err(error) => {
                return ShellOutcome::Failed {
                    hresult: error.code().0,
                };
            }
        };
    let persist = match link.cast::<IPersistFile>() {
        Ok(value) => value,
        Err(error) => {
            return ShellOutcome::Failed {
                hresult: error.code().0,
            };
        }
    };
    // SAFETY: The wide path buffer outlives this same-thread Load call.
    if let Err(error) = unsafe { persist.Load(PCWSTR(file.as_ptr()), STGM_READ) } {
        return ShellOutcome::Failed {
            hresult: error.code().0,
        };
    }
    // SAFETY: Each getter fills its fixed wide buffer on this STA thread only.
    unsafe fn get_path(link: &IShellLinkW) -> windows::core::Result<String> {
        unsafe {
            let mut units = [0_u16; 1024];
            let mut find_data = WIN32_FIND_DATAW::default();
            link.GetPath(&mut units, &mut find_data, RESOLVE_FLAGS.0 as u32)?;
            Ok(from_wide(&units))
        }
    }
    unsafe fn get_arguments(link: &IShellLinkW) -> windows::core::Result<String> {
        unsafe {
            let mut units = [0_u16; 1024];
            link.GetArguments(&mut units)?;
            Ok(from_wide(&units))
        }
    }
    unsafe fn get_working_directory(link: &IShellLinkW) -> windows::core::Result<String> {
        unsafe {
            let mut units = [0_u16; 1024];
            link.GetWorkingDirectory(&mut units)?;
            Ok(from_wide(&units))
        }
    }
    unsafe fn get_icon_location(link: &IShellLinkW) -> windows::core::Result<(String, i32)> {
        unsafe {
            let mut units = [0_u16; 1024];
            let mut index = 0_i32;
            link.GetIconLocation(&mut units, &mut index)?;
            Ok((from_wide(&units), index))
        }
    }
    let path = unsafe { get_path(&link) };
    let arguments = unsafe { get_arguments(&link) };
    let working_directory = unsafe { get_working_directory(&link) };
    let icon_location = unsafe { get_icon_location(&link) };
    let id_list_path = unsafe {
        link.GetIDList().ok().and_then(|pidl| {
            // SAFETY: Converts the shell-owned PIDL into a fixed buffer, then frees
            // the allocator-owned PIDL. Neither leaves this STA thread.
            let mut path_units = [0_u16; 260];
            let converted = SHGetPathFromIDListW(pidl, &mut path_units);
            CoTaskMemFree(Some(pidl.cast::<core::ffi::c_void>()));
            converted
                .as_bool()
                .then(|| PathBuf::from(from_wide(&path_units)))
        })
    };
    let failure = [
        path.as_ref().err(),
        arguments.as_ref().err(),
        working_directory.as_ref().err(),
        icon_location.as_ref().err(),
    ]
    .into_iter()
    .flatten()
    .map(|error| error.code().0)
    .next();
    if let Some(code) = failure {
        return ShellOutcome::Failed { hresult: code };
    }
    let path = path.unwrap_or_default();
    let (icon_location, icon_index) = icon_location.unwrap_or_default();
    ShellOutcome::Link(ShellLinkData {
        path: if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        },
        id_list_path,
        arguments: arguments.unwrap_or_default(),
        working_directory: working_directory.unwrap_or_default(),
        icon_location,
        icon_index,
    })
}

/// Synchronous resolve for the UI thread (OLE is already initialized there).
/// Used when the add/edit dialog picks an LNK (baseline 4.1); the item keeps
/// the original LNK path and uses these fields only as metadata.
pub fn resolve_link_now(path: &Path) -> Option<ShellLinkData> {
    match resolve_on_sta(path) {
        ShellOutcome::Link(data) => Some(data),
        _ => None,
    }
}

/// P2-09 (baseline 4.3): whether the shortcut is an MSI advertised shortcut.
/// Windows Installer marks these with the `SLDF_HAS_DARWINID` flag and a
/// Darwin data block; the Shell uses that block to perform install-on-demand
/// and repair. Detection never resolves the link's target, so it cannot
/// trigger Windows Installer.
///
/// Must run on an STA thread with COM initialized (same contract as
/// [`resolve_link_now`]).
pub fn detect_advertised_shortcut(path: &Path) -> bool {
    let file = wide(&path.to_string_lossy());
    let link =
        match unsafe { CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        {
            Ok(value) => value,
            Err(_) => return false,
        };
    let Ok(persist) = link.cast::<IPersistFile>() else {
        return false;
    };
    // SAFETY: The wide path buffer outlives this same-thread Load call.
    if unsafe { persist.Load(PCWSTR(file.as_ptr()), STGM_READ) }.is_err() {
        return false;
    }
    let Ok(data_list) = link.cast::<IShellLinkDataList>() else {
        return false;
    };
    // SAFETY: GetFlags only writes the link's data flags.
    let Ok(flags) = (unsafe { data_list.GetFlags() }) else {
        return false;
    };
    flags & SLDF_HAS_DARWINID.0 as u32 != 0
}

/// P2-10 (baseline 4.4): the Shell display name of a namespace target
/// (`::{CLSID}`, `shell:...`), used to fill the item name when the user adds
/// a special Shell item without typing one. `ms-settings:` URIs have no
/// namespace item, so they return `None`.
///
/// Must run on an STA thread with COM initialized (same contract as
/// [`resolve_link_now`]).
pub fn shell_display_name(path: &Path) -> Option<String> {
    let parsing = wide(&path.to_string_lossy());
    // SAFETY: the parsing-name buffer outlives this same-thread call.
    let item: IShellItem =
        unsafe { SHCreateItemFromParsingName(PCWSTR(parsing.as_ptr()), None) }.ok()?;
    // SAFETY: the returned string is Shell-allocated and freed exactly once
    // below; it never leaves this function.
    let value = unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }.ok()?;
    let text = unsafe {
        let mut len = 0_usize;
        while *value.as_ptr().add(len) != 0 {
            len += 1;
        }
        from_wide(core::slice::from_raw_parts(value.as_ptr(), len))
    };
    unsafe { CoTaskMemFree(Some(value.as_ptr().cast::<core::ffi::c_void>())) };
    let trimmed = text.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// P2-07 (baseline 4.2): the AppUserModel ID read from a UWP shortcut's
/// target AppsFolder item. Empty when the shortcut is not a UWP link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UwpLinkData {
    pub aumid: String,
}

/// `System.AppUserModel.ID` — read from the target item, never guessed from
/// a display name (baseline 4.2: 不得仅凭显示名称猜测 AUMID).
const PKEY_APPUSERMODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows_core::GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
    pid: 5,
};

/// Detects a UWP shortcut: the link resolves with **no file-system target**
/// while its IDList points at a Shell item that carries an AppUserModel ID
/// (an AppsFolder entry). Ordinary LNKs, folder links and other special
/// folders return `None`.
///
/// Must run on an STA thread with COM initialized (same contract as
/// [`resolve_link_now`]).
pub fn detect_uwp_link(path: &Path) -> Option<UwpLinkData> {
    let file = wide(&path.to_string_lossy());
    let link =
        match unsafe { CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        {
            Ok(value) => value,
            Err(_) => return None,
        };
    let persist = link.cast::<IPersistFile>().ok()?;
    // SAFETY: The wide path buffer outlives this same-thread Load call.
    unsafe { persist.Load(PCWSTR(file.as_ptr()), STGM_READ) }.ok()?;
    // SAFETY: getter fills a fixed buffer on this STA thread only.
    let target = unsafe {
        let mut units = [0_u16; 1024];
        let mut find_data = WIN32_FIND_DATAW::default();
        link.GetPath(&mut units, &mut find_data, RESOLVE_FLAGS.0 as u32)
            .ok()
            .map(|_| from_wide(&units))
    };
    // A non-empty target means an ordinary file/EXE link, not a UWP entry.
    if !target.unwrap_or_default().trim().is_empty() {
        return None;
    }
    // SAFETY: The allocator-owned PIDL is produced here and freed exactly
    // once at the end of this function; no early return happens after this
    // point (all failures below are folded into Option values).
    let pidl = unsafe { link.GetIDList().ok()? };
    // A resolvable file-system path means a folder/special location, not an
    // AppsFolder entry.
    let has_file_path = unsafe {
        let mut units = [0_u16; 260];
        let converted = SHGetPathFromIDListW(pidl, &mut units);
        converted.as_bool() && !from_wide(&units).trim().is_empty()
    };
    let aumid = if has_file_path {
        None
    } else {
        let item = unsafe { SHCreateItemFromIDList(pidl) }.ok().and_then(
            |shell_item: windows::Win32::UI::Shell::IShellItem| {
                shell_item.cast::<IShellItem2>().ok()
            },
        );
        item.and_then(|shell_item| unsafe { shell_item.GetString(&PKEY_APPUSERMODEL_ID).ok() })
            .and_then(|value| {
                // SAFETY: The string lives until CoTaskMemFree below.
                let text = unsafe {
                    let mut len = 0_usize;
                    while *value.as_ptr().add(len) != 0 {
                        len += 1;
                    }
                    from_wide(core::slice::from_raw_parts(value.as_ptr(), len))
                };
                unsafe { CoTaskMemFree(Some(value.as_ptr().cast::<core::ffi::c_void>())) };
                let trimmed = text.trim().to_string();
                (!trimmed.is_empty()).then_some(trimmed)
            })
    };
    unsafe { CoTaskMemFree(Some(pidl.cast::<core::ffi::c_void>())) };
    aumid.map(|aumid| UwpLinkData { aumid })
}
fn create_link_on_sta(
    target: &Path,
    working_directory: &Path,
    icon_location: &str,
    icon_index: i32,
    link_path: &Path,
) -> ShellOutcome {
    let target_units = wide(&target.to_string_lossy());
    let work_units = wide(&working_directory.to_string_lossy());
    let icon_units = wide(icon_location);
    let link_units = wide(&link_path.to_string_lossy());
    let link =
        match unsafe { CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        {
            Ok(value) => value,
            Err(error) => {
                return ShellOutcome::Failed {
                    hresult: error.code().0,
                };
            }
        };
    // SAFETY: Wide buffers outlive these synchronous same-thread calls.
    unsafe {
        if let Err(error) = link.SetPath(PCWSTR(target_units.as_ptr())) {
            return ShellOutcome::Failed {
                hresult: error.code().0,
            };
        }
        if let Err(error) = link.SetWorkingDirectory(PCWSTR(work_units.as_ptr())) {
            return ShellOutcome::Failed {
                hresult: error.code().0,
            };
        }
        if let Err(error) = link.SetIconLocation(PCWSTR(icon_units.as_ptr()), icon_index) {
            return ShellOutcome::Failed {
                hresult: error.code().0,
            };
        }
    }
    let persist = match link.cast::<IPersistFile>() {
        Ok(value) => value,
        Err(error) => {
            return ShellOutcome::Failed {
                hresult: error.code().0,
            };
        }
    };
    // SAFETY: The wide path buffer outlives this same-thread Save call.
    if let Err(error) = unsafe { persist.Save(PCWSTR(link_units.as_ptr()), true) } {
        return ShellOutcome::Failed {
            hresult: error.code().0,
        };
    }
    ShellOutcome::Executed
}

fn execute_folder_on_sta(path: &Path) -> ShellOutcome {
    let file = wide(&path.to_string_lossy());
    let verb = wide("open");
    // SAFETY: Zeroed SHELLEXECUTEINFOW is valid before field assignment.
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.hwnd = HWND::default();
    info.lpVerb = PCWSTR(verb.as_ptr());
    info.lpFile = PCWSTR(file.as_ptr());
    info.nShow = 1;
    // SAFETY: Wide pointers outlive the synchronous call; NOCLOSEPROCESS hands a
    // process handle to this thread, which closes it below.
    unsafe {
        if ShellExecuteExW(&mut info).is_err() {
            return ShellOutcome::Failed {
                hresult: GetLastError().0 as i32,
            };
        }
        if !info.hProcess.0.is_null() {
            let _ = CloseHandle(info.hProcess);
        }
    }
    ShellOutcome::Executed
}

/// The thread-bound STA handle. Drop posts WM_QUIT; the pump exits before COM release.
pub struct ShellService {
    requests: Option<Sender<ShellRequest>>,
    thread: Option<JoinHandle<()>>,
    thread_id: u32,
}

impl ShellService {
    pub fn start() -> Result<Self, Box<dyn Error>> {
        let (ready_tx, ready_rx) = channel();
        let (requests, worker) = channel::<ShellRequest>();
        let thread = std::thread::spawn(move || pump(ready_tx, worker));
        let outcome = ready_rx.recv()?;
        let thread_id = match outcome {
            Ok(thread_id) => thread_id,
            Err(failure) => {
                let _ = thread.join();
                return Err(format!("Shell STA startup failed (0x{:08X})", failure as u32).into());
            }
        };
        Ok(Self {
            requests: Some(requests),
            thread: Some(thread),
            thread_id,
        })
    }

    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }

    pub fn resolve(&self, path: &Path, timeout: Duration) -> ShellOutcome {
        let path = path.to_path_buf();
        self.call(timeout, move |reply| ShellRequest {
            task: Box::new(move || resolve_on_sta(&path)),
            reply,
        })
    }

    pub fn create_link(
        &self,
        target: &Path,
        working_directory: &Path,
        icon_location: &str,
        icon_index: i32,
        link_path: &Path,
        timeout: Duration,
    ) -> ShellOutcome {
        let target = target.to_path_buf();
        let working_directory = working_directory.to_path_buf();
        let icon_location = icon_location.to_string();
        let link_path = link_path.to_path_buf();
        self.call(timeout, move |reply| ShellRequest {
            task: Box::new(move || {
                create_link_on_sta(
                    &target,
                    &working_directory,
                    &icon_location,
                    icon_index,
                    &link_path,
                )
            }),
            reply,
        })
    }

    pub fn execute_folder(&self, path: &Path, timeout: Duration) -> ShellOutcome {
        let path = path.to_path_buf();
        self.call(timeout, move |reply| ShellRequest {
            task: Box::new(move || execute_folder_on_sta(&path)),
            reply,
        })
    }

    fn call(
        &self,
        timeout: Duration,
        build: impl FnOnce(Sender<ShellOutcome>) -> ShellRequest,
    ) -> ShellOutcome {
        let Some(requests) = self.requests.as_ref() else {
            return ShellOutcome::Unavailable;
        };
        let (reply_tx, reply_rx) = channel();
        let request = build(reply_tx);
        if requests.send(request).is_err() {
            return ShellOutcome::Unavailable;
        }
        // SAFETY: The STA queue exists before this id is published; the wakeup makes
        // the blocked GetMessageW observe the queued request. Posting failure would
        // leave the caller to time out, never a crash.
        unsafe {
            // Posting failure only loses the wakeup; the caller then times out
            // and observes Unavailable via the closed reply channel.
            let _ = PostThreadMessageW(self.thread_id, WAKEUP, WPARAM(0), LPARAM(0));
        }
        match reply_rx.recv_timeout(timeout) {
            Ok(outcome) => outcome,
            Err(RecvTimeoutError::Timeout) => ShellOutcome::TimedOut,
            Err(RecvTimeoutError::Disconnected) => ShellOutcome::Unavailable,
        }
    }
}

impl Drop for ShellService {
    fn drop(&mut self) {
        self.requests.take();
        if self.thread_id != 0 {
            // SAFETY: Queue exists before this id is published; the pump exits on
            // WM_QUIT and posting failure only loses a graceful shutdown.
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn pump(ready: Sender<Result<u32, i32>>, requests: Receiver<ShellRequest>) {
    let sta = match init_sta() {
        Ok(guard) => guard,
        Err(hresult) => {
            let _ = ready.send(Err(hresult));
            return;
        }
    };
    // SAFETY: Zero timeout forces queue creation before the id is published.
    unsafe {
        MsgWaitForMultipleObjectsEx(None, 0, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
    // SAFETY: Identity query without pointer arguments.
    let thread_id = unsafe { GetCurrentThreadId() };
    if ready.send(Ok(thread_id)).is_err() {
        return;
    }
    let mut message = MSG::default();
    loop {
        // SAFETY: Valid MSG storage; no window selects thread messages. The return
        // value distinguishes message/quit/error with three branches.
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 == 0 || result.0 == -1 {
            break;
        }
        // SAFETY: Both calls operate on the message just read on this thread.
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        loop {
            match requests.try_recv() {
                Ok(request) => {
                    let outcome = (request.task)();
                    let _ = request.reply.send(outcome);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    drop(sta);
                    return;
                }
            }
        }
    }
    drop(sta);
}

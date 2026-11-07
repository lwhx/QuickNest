use std::ffi::OsString;
use std::marker::PhantomData;
use std::os::windows::ffi::OsStrExt;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, WAIT_ABANDONED, WAIT_EVENT, WAIT_FAILED, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, SID, SID_AND_ATTRIBUTES, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, GetCurrentProcessId, OpenProcessToken, ReleaseMutex,
    WaitForSingleObject,
};
use windows_sys::core::PWSTR;

const ERROR_ALREADY_EXISTS: u32 = 183;
const TOKEN_QUERY: u32 = 0x0008;
const MUTEX_NAME_PREFIX: &str = "Local\\QuickNest-";
const PIPE_NAME_PREFIX: &str = "\\\\.\\pipe\\QuickNest-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquireOutcome {
    /// CreateMutexW created a fresh mutex: no other instance held it.
    FirstInstance,
    /// The mutex existed but its owner died; this instance took ownership.
    TookOverAbandoned,
    /// A live instance still owns the mutex.
    AlreadyRunning,
    /// A Win32 call failed; the payload is GetLastError() at the failure point.
    Failed(u32),
}

/// Thread-bound owner of the single-instance mutex handle.
/// Drop releases ownership (if held) and closes the handle.
pub struct MutexGuard {
    handle: Option<HANDLE>,
    owner: bool,
    name: String,
    _thread_bound: PhantomData<Rc<()>>,
}

impl Drop for MutexGuard {
    fn drop(&mut self) {
        // SAFETY: The handle, when present, is valid for this guard's lifetime; it
        // never crosses threads (!Send via Rc), so release/close run on the
        // acquiring thread. A probe-only guard carries no handle at all.
        unsafe {
            if let Some(handle) = self.handle {
                if self.owner {
                    ReleaseMutex(handle);
                }
                CloseHandle(handle);
            }
        }
    }
}

impl MutexGuard {
    pub fn name(&self) -> &str {
        &self.name
    }
}

fn wide(value: &str) -> Vec<u16> {
    OsString::from(value).encode_wide().chain(Some(0)).collect()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Hashes the current user's SID so different users never share one namespace key.
fn sid_hash() -> Result<u64, u32> {
    let mut token = HANDLE::default();
    // SAFETY: GetCurrentProcess returns a pseudo-handle; the token handle is written
    // to valid stack storage and closed by the caller on every path below.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(GetLastError());
        }
    }
    let result = (|| {
        let mut needed = 0_u32;
        // SAFETY: NULL buffer with length 0 requests the required size; GetLastError
        // is read immediately after the documented failure.
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
        let sid_and_attributes: &SID_AND_ATTRIBUTES = unsafe { &*(&token_user.User as *const _) };
        let sid: &SID = unsafe { &*sid_and_attributes.Sid.cast::<SID>() };
        // SAFETY: The SID layout is fixed; SubAuthorities follow the header in one
        // allocation returned by the token query, so a bounded byte read is valid.
        let sub_authorities = unsafe {
            core::slice::from_raw_parts(sid.SubAuthority.as_ptr(), sid.SubAuthorityCount as usize)
        };
        let mut bytes = Vec::with_capacity(4 + 6 + sub_authorities.len() * 4);
        bytes.extend_from_slice(&sid.Revision.to_le_bytes());
        bytes.extend_from_slice(&sid.IdentifierAuthority.Value);
        for authority in sub_authorities {
            bytes.extend_from_slice(&authority.to_le_bytes());
        }
        Ok(fnv1a64(&bytes))
    })();
    // SAFETY: The token handle from OpenProcessToken is closed exactly once.
    unsafe {
        CloseHandle(token);
    }
    result
}

fn session_id() -> Result<u32, u32> {
    let mut session = 0_u32;
    // SAFETY: Valid writable stack storage for the session id.
    let process_id = unsafe { GetCurrentProcessId() };
    unsafe {
        if ProcessIdToSessionId(process_id, &mut session) == 0 {
            return Err(GetLastError());
        }
    }
    Ok(session)
}

fn instance_namespace_hash() -> Option<u64> {
    std::env::var_os("QUICKNEST_INSTANCE_NAMESPACE")
        .filter(|value| !value.is_empty())
        .map(|value| fnv1a64(value.to_string_lossy().as_bytes()))
}

/// Stable per-user/per-session identity shared by the Mutex and Pipe namespaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceIdentity {
    pub sid_hash: u64,
    pub session_id: u32,
    pub namespace_hash: Option<u64>,
}

impl InstanceIdentity {
    pub fn current() -> Result<Self, u32> {
        Ok(Self {
            sid_hash: sid_hash()?,
            session_id: session_id()?,
            namespace_hash: instance_namespace_hash(),
        })
    }

    fn namespace_suffix(&self) -> String {
        self.namespace_hash
            .map(|hash| format!("-test-{hash:016x}"))
            .unwrap_or_default()
    }

    pub fn mutex_name(&self) -> String {
        format!(
            "{MUTEX_NAME_PREFIX}{:016x}-{}{}",
            self.sid_hash,
            self.session_id,
            self.namespace_suffix()
        )
    }

    pub fn pipe_name(&self) -> String {
        format!(
            "{PIPE_NAME_PREFIX}{:016x}-{}{}",
            self.sid_hash,
            self.session_id,
            self.namespace_suffix()
        )
    }
}

pub fn mutex_name() -> Result<String, u32> {
    Ok(InstanceIdentity::current()?.mutex_name())
}

/// Acquires the per-user/session single-instance mutex.
/// The baseline's WAIT_* return list applies to the wait probe, not CreateMutexW;
/// this function maps both to their real Win32 sources and records each stage.
pub fn acquire() -> Result<(AcquireOutcome, MutexGuard), u32> {
    let name = mutex_name()?;
    let name_units = wide(&name);
    // SAFETY: NULL security attributes, we request initial ownership, and the name
    // buffer outlives the call. GetLastError is read immediately afterwards.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name_units.as_ptr() as PWSTR) };
    if handle.is_null() {
        // SAFETY: GetLastError immediately after the failed CreateMutexW.
        let error = unsafe { GetLastError() };
        return Err(error);
    }
    // SAFETY: GetLastError must be the very next call after CreateMutexW.
    let create_error = unsafe { GetLastError() };
    if create_error != ERROR_ALREADY_EXISTS {
        return Ok((
            AcquireOutcome::FirstInstance,
            MutexGuard {
                handle: Some(handle),
                owner: true,
                name,
                _thread_bound: PhantomData,
            },
        ));
    }
    // SAFETY: Zero timeout probes ownership without blocking.
    let wait = unsafe { WaitForSingleObject(handle, 0) };
    if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
        return Ok((
            if wait == WAIT_ABANDONED {
                AcquireOutcome::TookOverAbandoned
            } else {
                AcquireOutcome::FirstInstance
            },
            MutexGuard {
                handle: Some(handle),
                owner: true,
                name,
                _thread_bound: PhantomData,
            },
        ));
    }
    if wait == WAIT_TIMEOUT {
        // SAFETY: Closing the handle we opened for the probe.
        unsafe {
            CloseHandle(handle);
        }
        return Ok((
            AcquireOutcome::AlreadyRunning,
            MutexGuard {
                handle: None,
                owner: false,
                name,
                _thread_bound: PhantomData,
            },
        ));
    }
    // SAFETY: GetLastError after the failed wait.
    let error = unsafe { GetLastError() };
    unsafe {
        CloseHandle(handle);
    }
    Err(if error == 0 { WAIT_FAILED } else { error })
}

/// Opens the named mutex without taking ownership (used by crash-recovery tests).
pub fn open_existing(name: &str) -> Result<HANDLE, u32> {
    let units = wide(name);
    // SAFETY: SYNCHRONIZE (0x00100000) requests wait rights; the name buffer
    // outlives the call and the returned handle is closed by the caller.
    let handle = unsafe {
        windows_sys::Win32::System::Threading::OpenMutexW(0x0010_0000, 0, units.as_ptr() as PWSTR)
    };
    if handle.is_null() {
        Err(unsafe { GetLastError() })
    } else {
        Ok(handle)
    }
}

/// Waits for the mutex and records the raw WAIT_EVENT verdict.
///
/// # Safety
///
/// `handle` must be a live mutex handle opened by this thread; it is
/// released after the wait so a crashed owner's abandoned object can be destroyed.
pub unsafe fn wait_and_release(handle: HANDLE) -> WAIT_EVENT {
    // SAFETY: Valid handle owned by this thread.
    let wait = unsafe { WaitForSingleObject(handle, 15_000) };
    if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
        // SAFETY: We own the mutex after the wait; releasing lets the OS destroy
        // the abandoned object once the last handle closes.
        unsafe {
            ReleaseMutex(handle);
        }
    }
    wait
}

pub fn wait_event_name(wait: WAIT_EVENT) -> &'static str {
    match wait {
        WAIT_OBJECT_0 => "WAIT_OBJECT_0",
        WAIT_ABANDONED => "WAIT_ABANDONED",
        WAIT_TIMEOUT => "WAIT_TIMEOUT",
        _ => "WAIT_FAILED",
    }
}

#[cfg(test)]
mod namespace_tests {
    use super::*;

    #[test]
    fn explicit_test_namespace_changes_both_names_without_touching_production() {
        let production = InstanceIdentity {
            sid_hash: 7,
            session_id: 3,
            namespace_hash: None,
        };
        let test = InstanceIdentity {
            namespace_hash: Some(fnv1a64(b"isolated-run")),
            ..production.clone()
        };
        assert_eq!(
            production.mutex_name(),
            r"Local\QuickNest-0000000000000007-3"
        );
        assert_ne!(production.mutex_name(), test.mutex_name());
        assert_ne!(production.pipe_name(), test.pipe_name());
        assert!(test.mutex_name().ends_with(&test.namespace_suffix()));
        assert!(test.pipe_name().ends_with(&test.namespace_suffix()));
    }
}

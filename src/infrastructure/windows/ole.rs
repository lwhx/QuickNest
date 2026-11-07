use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

use windows_sys::Win32::System::Com::CoGetApartmentType;
use windows_sys::Win32::System::Ole::{OleInitialize, OleUninitialize};

pub const S_OK: i32 = 0;
pub const S_FALSE: i32 = 1;
pub const CO_E_NOTINITIALIZED: i32 = 0x800401F0_u32 as i32;
pub const RPC_E_CHANGED_MODE: i32 = 0x80010106_u32 as i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApartmentObservation {
    pub hresult: i32,
    pub kind: i32,
    pub qualifier: i32,
}

impl ApartmentObservation {
    pub fn is_sta(self) -> bool {
        self.hresult == S_OK && matches!(self.kind, 0 | 3)
    }

    pub fn is_mta(self) -> bool {
        self.hresult == S_OK && self.kind == 1
    }
}

pub fn apartment() -> ApartmentObservation {
    let mut kind = -1;
    let mut qualifier = -1;
    // SAFETY: Both output pointers refer to writable stack values. No COM interfaces
    // are acquired or moved; this only queries the calling thread's apartment.
    let hresult = unsafe { CoGetApartmentType(&mut kind, &mut qualifier) };
    if hresult != S_OK {
        kind = -1;
        qualifier = -1;
    }
    ApartmentObservation {
        hresult,
        kind,
        qualifier,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OleError(pub i32);

impl fmt::Display for OleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == RPC_E_CHANGED_MODE {
            write!(
                f,
                "OLE initialization failed: incompatible COM apartment (0x{:08X})",
                self.0 as u32
            )
        } else {
            write!(f, "OLE initialization failed (0x{:08X})", self.0 as u32)
        }
    }
}

impl Error for OleError {}

/// Owns exactly one successful OleInitialize call on this thread.
///
/// ```compile_fail
/// use quicknest::infrastructure::windows::ole::OleGuard;
/// fn requires_send<T: Send>() {}
/// requires_send::<OleGuard>();
/// ```
///
/// ```compile_fail
/// use quicknest::infrastructure::windows::ole::OleGuard;
/// fn requires_sync<T: Sync>() {}
/// requires_sync::<OleGuard>();
/// ```
#[derive(Debug)]
pub struct OleGuard {
    hresult: i32,
    // Rc is !Send + !Sync, preventing cleanup or shared access from another thread.
    _thread_bound: PhantomData<Rc<()>>,
}

impl OleGuard {
    pub fn initialize() -> Result<Self, OleError> {
        // SAFETY: The reserved pointer must be NULL. S_OK and S_FALSE both acquire
        // one initialization reference. Failure acquires none and creates no guard.
        let hresult = unsafe { OleInitialize(std::ptr::null()) };
        match hresult {
            S_OK | S_FALSE => Ok(Self {
                hresult,
                _thread_bound: PhantomData,
            }),
            error => Err(OleError(error)),
        }
    }

    pub fn hresult(&self) -> i32 {
        self.hresult
    }
}

impl Drop for OleGuard {
    fn drop(&mut self) {
        // SAFETY: Only a successful initialize constructs this non-Clone guard.
        // !Send/!Sync keeps its exactly-once release on the initializing thread.
        unsafe { OleUninitialize() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_ole_successes_release_exactly_their_own_thread_references() {
        std::thread::spawn(|| {
            assert_eq!(apartment().hresult, CO_E_NOTINITIALIZED);
            let outer = OleGuard::initialize().unwrap();
            assert_eq!(outer.hresult(), S_OK);
            assert!(apartment().is_sta());
            let inner = OleGuard::initialize().unwrap();
            assert_eq!(inner.hresult(), S_FALSE);
            drop(inner);
            assert!(apartment().is_sta());
            drop(outer);
            assert_eq!(apartment().hresult, CO_E_NOTINITIALIZED);
        })
        .join()
        .unwrap();
    }

    // The MTA-conflict scenario lives in tests/ole_mta.rs: an MTA is
    // process-wide and would turn later fresh threads into implicit-MTA
    // threads, breaking the strict apartment assertions above.
}

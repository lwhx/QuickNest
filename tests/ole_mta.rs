//! The MTA-conflict scenario runs in its own test process: an MTA is
//! process-wide, and creating one inside the unit-test process would turn
//! later fresh threads into implicit-MTA threads (breaking the strict
//! apartment assertions in `ole::tests`).

use quicknest::infrastructure::windows::ole::{
    CO_E_NOTINITIALIZED, OleError, OleGuard, RPC_E_CHANGED_MODE, S_OK, apartment,
};
use windows_sys::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

#[test]
fn mta_conflict_does_not_uninitialize_the_existing_apartment() {
    // SAFETY: this test binary owns its process; the spawned thread performs
    // exactly one successful CoInitializeEx balanced below, and no interface
    // crosses threads.
    std::thread::spawn(|| {
        assert_eq!(
            unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) },
            S_OK
        );
        assert_eq!(
            OleGuard::initialize().unwrap_err(),
            OleError(RPC_E_CHANGED_MODE)
        );
        assert!(apartment().is_mta());
        // SAFETY: balances only the successful CoInitializeEx above, never the
        // failed OLE call.
        unsafe { CoUninitialize() };
        let after = apartment();
        // The thread is no longer explicitly initialized. Depending on whether
        // the process MTA is still alive it reports either "not initialized"
        // or the implicit MTA; both mean the reference was balanced.
        assert!(
            after.hresult == CO_E_NOTINITIALIZED || after.kind == 3,
            "unexpected apartment after CoUninitialize: {after:?}"
        );
    })
    .join()
    .unwrap();
}

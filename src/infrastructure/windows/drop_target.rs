use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{
    DRAGDROP_E_INVALIDHWND, E_INVALIDARG, E_POINTER, HWND, POINTL, RPC_E_WRONG_THREAD,
};
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, RegisterDragDrop, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::core::{Error, Ref, Result, implement};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;

use super::ole::OleGuard;
use super::window_handle::{inspect_window, observe_value};

#[derive(Default)]
pub struct TargetStats {
    pub destroyed: Cell<u32>,
    pub enters: Cell<u32>,
    pub overs: Cell<u32>,
    pub leaves: Cell<u32>,
    pub drops: Cell<u32>,
}

#[implement(IDropTarget, Agile = false)]
struct RejectingDropTarget {
    stats: Rc<TargetStats>,
    tid: u32,
}

impl RejectingDropTarget {
    fn reject(&self, effect: *mut DROPEFFECT) -> Result<()> {
        // SAFETY: Identity query has no pointer parameters and changes no thread state.
        if unsafe { GetCurrentThreadId() } != self.tid {
            return Err(Error::from_hresult(RPC_E_WRONG_THREAD));
        }
        if effect.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }
        // SAFETY: COM callers must supply a writable DROPEFFECT output pointer.
        // Null is rejected above; this target never dereferences IDataObject.
        unsafe { *effect = DROPEFFECT_NONE };
        Ok(())
    }
}

impl Drop for RejectingDropTarget {
    fn drop(&mut self) {
        self.stats.destroyed.set(self.stats.destroyed.get() + 1);
    }
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for RejectingDropTarget_Impl {
    fn DragEnter(
        &self,
        _data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        self.reject(effect)?;
        self.stats.enters.set(self.stats.enters.get() + 1);
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        _point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        self.reject(effect)?;
        self.stats.overs.set(self.stats.overs.get() + 1);
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        // SAFETY: Current thread ID only; no borrowed interface is moved across threads.
        if unsafe { GetCurrentThreadId() } != self.tid {
            return Err(Error::from_hresult(RPC_E_WRONG_THREAD));
        }
        self.stats.leaves.set(self.stats.leaves.get() + 1);
        Ok(())
    }

    fn Drop(
        &self,
        _data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        self.reject(effect)?;
        self.stats.drops.set(self.stats.drops.get() + 1);
        Ok(())
    }
}

/// UI-thread-only COM owner. No raw interface is exposed for cross-thread sharing.
pub struct DropTarget {
    interface: IDropTarget,
    _thread_bound: PhantomData<Rc<()>>,
}

impl DropTarget {
    pub(crate) fn from_interface(interface: IDropTarget) -> Self {
        Self {
            interface,
            _thread_bound: PhantomData,
        }
    }

    pub fn rejecting(stats: Rc<TargetStats>) -> Self {
        // SAFETY: Current-thread identity query has no pointer parameters.
        let tid = unsafe { GetCurrentThreadId() };
        Self {
            interface: RejectingDropTarget { stats, tid }.into(),
            _thread_bound: PhantomData,
        }
    }

    /// The borrowed COM interface must stay on its owning UI thread.
    pub fn interface(&self) -> &IDropTarget {
        &self.interface
    }
}

/// Owns only its successful registration. Drop revokes before releasing provider/OLE.
///
/// ```compile_fail
/// use quicknest::infrastructure::windows::drop_target::DropRegistration;
/// fn requires_send<T: Send>() {}
/// requires_send::<DropRegistration>();
/// ```
pub struct DropRegistration {
    hwnd: HWND,
    registered: bool,
    _target: DropTarget,
    _provider: slint::WindowHandle,
    _ole: OleGuard,
}

impl DropRegistration {
    pub fn register(window: &slint::Window, target: DropTarget) -> Result<Self> {
        let ole = OleGuard::initialize()
            .map_err(|error| Error::from_hresult(windows::core::HRESULT(error.0)))?;
        let observed =
            inspect_window(window).map_err(|_| Error::from_hresult(DRAGDROP_E_INVALIDHWND))?;
        let provider = window.window_handle();
        let borrowed = HasWindowHandle::window_handle(&provider)
            .map_err(|_| Error::from_hresult(E_INVALIDARG))?;
        let RawWindowHandle::Win32(raw) = borrowed.as_raw() else {
            return Err(Error::from_hresult(E_INVALIDARG));
        };
        if raw.hwnd.get() != observed.hwnd {
            return Err(Error::from_hresult(DRAGDROP_E_INVALIDHWND));
        }
        let hwnd = HWND(raw.hwnd.get() as *mut _);
        // SAFETY: HWND is alive and owned by this STA UI thread. The provider and
        // OLE guard stay owned until revocation; COM receives a valid IDropTarget.
        // On failure no registration owner exists, so no unrelated target is revoked.
        unsafe { RegisterDragDrop(hwnd, target.interface()) }?;
        Ok(Self {
            hwnd,
            registered: true,
            _target: target,
            _provider: provider,
            _ole: ole,
        })
    }

    pub fn revoke(&mut self) -> Result<()> {
        if self.registered {
            // SAFETY: This !Send guard owns a successful registration and retains
            // its native-window provider/OLE initialization on the registering thread.
            unsafe { RevokeDragDrop(self.hwnd) }?;
            self.registered = false;
        }
        Ok(())
    }

    pub fn is_registered(&self) -> bool {
        self.registered
    }
}

impl Drop for DropRegistration {
    fn drop(&mut self) {
        if let Err(error) = self.revoke() {
            eprintln!("Failed to revoke owned IDropTarget: {error}");
        }
    }
}

/// Diagnostic rejection of null/stale values; never registers an arbitrary HWND.
pub fn validate_observation(value: isize) -> Result<()> {
    if !observe_value(value).is_window {
        return Err(Error::from_hresult(DRAGDROP_E_INVALIDHWND));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::E_NOINTERFACE;
    use windows::Win32::System::Com::IStream;
    use windows::core::{IUnknown, Interface};

    #[test]
    fn callbacks_reject_all_effects_and_null_output_without_data_access() {
        let stats = Rc::new(TargetStats::default());
        let target = DropTarget::rejecting(stats.clone());
        let mut effect = DROPEFFECT(1);
        let point = POINTL { x: 0, y: 0 };
        // SAFETY: Valid same-thread COM owner, valid effect/point storage. None is
        // intentional: this target ignores IDataObject and accepts no data formats.
        unsafe {
            target
                .interface()
                .DragEnter(None, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                .unwrap();
            assert_eq!(effect, DROPEFFECT_NONE);
            effect = DROPEFFECT(1);
            target
                .interface()
                .DragOver(MODIFIERKEYS_FLAGS(0), point, &mut effect)
                .unwrap();
            assert_eq!(effect, DROPEFFECT_NONE);
            target.interface().DragLeave().unwrap();
            target
                .interface()
                .Drop(None, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                .unwrap();
            assert_eq!(effect, DROPEFFECT_NONE);
            assert_eq!(
                target
                    .interface()
                    .DragOver(MODIFIERKEYS_FLAGS(0), point, std::ptr::null_mut())
                    .unwrap_err()
                    .code(),
                E_POINTER
            );
        }
        assert_eq!(
            (
                stats.enters.get(),
                stats.overs.get(),
                stats.leaves.get(),
                stats.drops.get()
            ),
            (1, 1, 1, 1)
        );
        drop(target);
        assert_eq!(stats.destroyed.get(), 1);
    }

    #[test]
    fn query_interface_preserves_identity_and_rejects_unsupported_interface() {
        let stats = Rc::new(TargetStats::default());
        let target = DropTarget::rejecting(stats.clone());
        let unknown: IUnknown = target.interface().cast().unwrap();
        let again: IDropTarget = unknown.cast().unwrap();
        let identity: IUnknown = again.cast().unwrap();
        assert_eq!(Interface::as_raw(&unknown), Interface::as_raw(&identity));
        assert_eq!(
            target.interface().cast::<IStream>().unwrap_err().code(),
            E_NOINTERFACE
        );
        drop(identity);
        drop(again);
        drop(unknown);
        drop(target);
        assert_eq!(stats.destroyed.get(), 1);
    }

    #[test]
    fn null_window_is_not_an_eligible_registration_observation() {
        assert_eq!(
            validate_observation(0).unwrap_err().code(),
            DRAGDROP_E_INVALIDHWND
        );
    }
}

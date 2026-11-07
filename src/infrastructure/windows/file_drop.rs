use std::cell::Cell;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{SyncSender, TrySendError};

use windows::Win32::Foundation::{
    DV_E_FORMATETC, E_FAIL, E_POINTER, POINTL, RPC_E_WRONG_THREAD, S_OK,
};
use windows::Win32::System::Com::{
    DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl,
    ReleaseStgMedium,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::core::{Error, Ref, Result, implement};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;

use super::drop_target::DropTarget;

const MAX_FILES: u32 = 4096;
const MAX_PATH_UNITS: u32 = 32767;
const MAX_TOTAL_UNITS: usize = 1_048_576;

#[derive(Debug)]
pub enum FileDropEvent {
    Enter { supported: bool, x: i32, y: i32 },
    Over { x: i32, y: i32 },
    Leave,
    Dropped { paths: Vec<PathBuf>, x: i32, y: i32 },
    Failed { hresult: i32 },
}

#[derive(Default)]
pub struct FileDropStats {
    pub acquired: Cell<u32>,
    pub released: Cell<u32>,
    pub destroyed: Cell<u32>,
    pub enters: Cell<u32>,
    pub overs: Cell<u32>,
    pub leaves: Cell<u32>,
    pub drops: Cell<u32>,
    pub errors: Cell<u32>,
}

fn format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

pub fn supports_files(data: &IDataObject) -> bool {
    // SAFETY: Same-thread borrowed COM data object; FORMATETC is valid for this call.
    unsafe { data.QueryGetData(&format()) == S_OK }
}

struct OwnedMedium<'a> {
    value: STGMEDIUM,
    stats: &'a FileDropStats,
}

impl Drop for OwnedMedium<'_> {
    fn drop(&mut self) {
        // SAFETY: This unique medium came from successful GetData; it has not been
        // cloned or manually freed. ReleaseStgMedium also honors pUnkForRelease.
        unsafe { ReleaseStgMedium(&mut self.value) };
        self.stats.released.set(self.stats.released.get() + 1);
    }
}

pub fn read_files(data: &IDataObject, stats: &FileDropStats) -> Result<Vec<PathBuf>> {
    if !supports_files(data) {
        return Err(Error::from_hresult(DV_E_FORMATETC));
    }
    // SAFETY: Valid FORMATETC on the object's receiving STA. Successful result is
    // immediately placed in its unique RAII owner, including on every error path.
    let value = unsafe { data.GetData(&format()) }?;
    stats.acquired.set(stats.acquired.get() + 1);
    let medium = OwnedMedium { value, stats };
    if medium.value.tymed != TYMED_HGLOBAL.0 as u32 {
        return Err(Error::from_hresult(DV_E_FORMATETC));
    }
    // SAFETY: The union's hGlobal member is selected by checked TYMED_HGLOBAL.
    let global = unsafe { medium.value.u.hGlobal };
    if global.0.is_null() {
        return Err(Error::from_hresult(E_POINTER));
    }
    let hdrop = HDROP(global.0);
    // SAFETY: CF_HDROP GetData returned this HGLOBAL, owned by medium throughout
    // all queries. DragQueryFileW borrows it and does not transfer ownership.
    let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
    if count == 0 || count > MAX_FILES {
        return Err(Error::from_hresult(E_FAIL));
    }
    let mut paths = Vec::with_capacity(count as usize);
    let mut total = 0_usize;
    for index in 0..count {
        // SAFETY: Same live CF_HDROP; None requests just this entry's length.
        let len = unsafe { DragQueryFileW(hdrop, index, None) };
        total += len as usize;
        if len == 0 || len > MAX_PATH_UNITS || total > MAX_TOTAL_UNITS {
            return Err(Error::from_hresult(E_FAIL));
        }
        let mut units = vec![0_u16; len as usize + 1];
        // SAFETY: Buffer includes the terminator and the medium still owns hdrop.
        let copied = unsafe { DragQueryFileW(hdrop, index, Some(&mut units)) };
        if copied != len || units[len as usize] != 0 {
            return Err(Error::from_hresult(E_FAIL));
        }
        paths.push(PathBuf::from(OsString::from_wide(&units[..len as usize])));
    }
    Ok(paths)
}

pub fn copy_effect(supported: bool, allowed: DROPEFFECT) -> DROPEFFECT {
    if supported && (allowed.0 & DROPEFFECT_COPY.0) != 0 {
        DROPEFFECT_COPY
    } else {
        DROPEFFECT_NONE
    }
}

#[implement(IDropTarget, Agile = false)]
struct FileDropTarget {
    sender: SyncSender<FileDropEvent>,
    stats: Rc<FileDropStats>,
    supported: Cell<bool>,
    tid: u32,
}

impl FileDropTarget {
    fn effect_input(&self, effect: *mut DROPEFFECT) -> Result<DROPEFFECT> {
        // SAFETY: Current thread identity only; no pointers.
        if unsafe { GetCurrentThreadId() } != self.tid {
            return Err(Error::from_hresult(RPC_E_WRONG_THREAD));
        }
        if effect.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }
        // SAFETY: COM contract requires a valid in/out effect; NULL is rejected.
        let allowed = unsafe { *effect };
        // SAFETY: Same valid output pointer. Failure defaults to no transfer.
        unsafe { *effect = DROPEFFECT_NONE };
        Ok(allowed)
    }

    fn send(&self, event: FileDropEvent) -> Result<()> {
        self.sender.try_send(event).map_err(|error| match error {
            TrySendError::Full(_) | TrySendError::Disconnected(_) => Error::from_hresult(E_FAIL),
        })
    }
}

impl Drop for FileDropTarget {
    fn drop(&mut self) {
        self.stats.destroyed.set(self.stats.destroyed.get() + 1);
    }
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for FileDropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        self.supported.set(false);
        let allowed = self.effect_input(effect)?;
        let supported = data.as_ref().is_some_and(supports_files);
        self.stats.enters.set(self.stats.enters.get() + 1);
        self.send(FileDropEvent::Enter {
            supported,
            x: point.x,
            y: point.y,
        })?;
        self.supported.set(supported);
        // SAFETY: effect_input checked the writable pointer on this thread.
        unsafe { *effect = copy_effect(supported, allowed) };
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let allowed = self.effect_input(effect)?;
        self.stats.overs.set(self.stats.overs.get() + 1);
        // Hover telemetry is lossy under backpressure; terminal data events are not.
        let _ = self.sender.try_send(FileDropEvent::Over {
            x: point.x,
            y: point.y,
        });
        // SAFETY: effect_input checked the writable pointer.
        unsafe { *effect = copy_effect(self.supported.get(), allowed) };
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        // SAFETY: Identity query only; callback state remains on the owner thread.
        if unsafe { GetCurrentThreadId() } != self.tid {
            return Err(Error::from_hresult(RPC_E_WRONG_THREAD));
        }
        self.supported.set(false);
        self.stats.leaves.set(self.stats.leaves.get() + 1);
        self.send(FileDropEvent::Leave)
    }

    fn Drop(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        self.supported.set(false);
        let allowed = self.effect_input(effect)?;
        self.stats.drops.set(self.stats.drops.get() + 1);
        if copy_effect(true, allowed) == DROPEFFECT_NONE {
            return Ok(());
        }
        let result = (|| {
            let paths = read_files(data.ok()?, &self.stats)?;
            self.send(FileDropEvent::Dropped {
                paths,
                x: point.x,
                y: point.y,
            })
        })();
        match result {
            Ok(()) => {
                // SAFETY: Successful copy into the owned event queue; no file was moved.
                unsafe { *effect = DROPEFFECT_COPY };
                Ok(())
            }
            Err(error) => {
                self.stats.errors.set(self.stats.errors.get() + 1);
                let _ = self.sender.try_send(FileDropEvent::Failed {
                    hresult: error.code().0,
                });
                Err(error)
            }
        }
    }
}

pub fn receiving_target(sender: SyncSender<FileDropEvent>, stats: Rc<FileDropStats>) -> DropTarget {
    // SAFETY: Query only; target and callbacks are bound to the current STA thread.
    let tid = unsafe { GetCurrentThreadId() };
    let interface: IDropTarget = FileDropTarget {
        sender,
        stats,
        supported: Cell::new(false),
        tid,
    }
    .into();
    DropTarget::from_interface(interface)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_requires_both_supported_format_and_source_permission() {
        assert_eq!(copy_effect(true, DROPEFFECT(3)), DROPEFFECT_COPY);
        assert_eq!(copy_effect(false, DROPEFFECT_COPY), DROPEFFECT_NONE);
        assert_eq!(copy_effect(true, DROPEFFECT(2)), DROPEFFECT_NONE);
    }

    #[test]
    fn missing_data_and_closed_queue_never_report_copy() {
        use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
        let (sender, receiver) = std::sync::mpsc::sync_channel(4);
        let stats = Rc::new(FileDropStats::default());
        let target = receiving_target(sender, stats.clone());
        let mut effect = DROPEFFECT_COPY;
        let point = POINTL { x: 10, y: 20 };
        // SAFETY: Same-thread interface and valid output storage; None is deliberately unsupported.
        unsafe {
            target
                .interface()
                .DragEnter(None, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                .unwrap();
        }
        assert_eq!(effect, DROPEFFECT_NONE);
        drop(receiver);
        effect = DROPEFFECT_COPY;
        // SAFETY: Same valid owner and effect; Drop must fail without a data object.
        assert!(
            unsafe {
                target
                    .interface()
                    .Drop(None, MODIFIERKEYS_FLAGS(0), point, &mut effect)
            }
            .is_err()
        );
        assert_eq!(effect, DROPEFFECT_NONE);
        assert_eq!((stats.acquired.get(), stats.released.get()), (0, 0));
        drop(target);
        assert_eq!(stats.destroyed.get(), 1);
    }
}

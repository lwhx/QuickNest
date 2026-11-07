use std::cell::RefCell;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use quicknest::infrastructure::windows::drop_target::{
    DropRegistration, DropTarget, TargetStats, validate_observation,
};
use quicknest::infrastructure::windows::window_handle::{WindowObservation, observe_value};
use windows::Win32::Foundation::{
    DRAGDROP_E_ALREADYREGISTERED, DRAGDROP_E_INVALIDHWND, DRAGDROP_E_NOTREGISTERED, HWND,
};
use windows::Win32::System::Ole::RevokeDragDrop;
use windows::core::HRESULT;

use crate::hwnd_validation::LifecycleHooks;

type SharedReport = Rc<RefCell<File>>;

struct RegistrationValidation {
    // Must release registration before releasing its retained provider/OLE state.
    registration: Option<DropRegistration>,
    stats: Option<Rc<TargetStats>>,
    report: SharedReport,
    started: Instant,
}

impl RegistrationValidation {
    fn record(
        &self,
        generation: u8,
        phase: &str,
        observed: WindowObservation,
        hr: HRESULT,
        destroyed: u32,
    ) -> Result<(), Box<dyn Error>> {
        let now = observe_value(observed.hwnd);
        writeln!(
            self.report.borrow_mut(),
            "{}\t{generation}\t{phase}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{destroyed}",
            self.started.elapsed().as_millis(),
            observed.pid,
            observed.tid,
            observed.hwnd,
            hr.0 as u32,
            now.is_window,
            now.visible,
            self.registration
                .as_ref()
                .is_some_and(|guard| guard.is_registered()),
        )?;
        self.report.borrow_mut().flush()?;
        Ok(())
    }

    fn duplicate(&self, window: &slint::Window) -> Result<HRESULT, Box<dyn Error>> {
        let stats = Rc::new(TargetStats::default());
        let target = DropTarget::rejecting(stats.clone());
        let error = match DropRegistration::register(window, target) {
            Ok(_) => return Err("Duplicate IDropTarget registration unexpectedly succeeded".into()),
            Err(error) => error,
        };
        if error.code() != DRAGDROP_E_ALREADYREGISTERED || stats.destroyed.get() != 1 {
            return Err(
                format!("Duplicate registration/reference cleanup mismatch: {error}").into(),
            );
        }
        Ok(error.code())
    }
}

impl LifecycleHooks for RegistrationValidation {
    fn phase(
        &mut self,
        phase: &str,
        generation: u8,
        window: Option<&slint::Window>,
        observed: WindowObservation,
    ) -> Result<(), Box<dyn Error>> {
        match phase {
            "created" => {
                let window = window.ok_or("Missing created Slint window")?;
                let stats = Rc::new(TargetStats::default());
                let target = DropTarget::rejecting(stats.clone());
                self.registration = Some(DropRegistration::register(window, target)?);
                self.stats = Some(stats);
                self.record(generation, "registered", observed, HRESULT(0), 0)?;
                let hr = self.duplicate(window)?;
                self.record(generation, "duplicate_rejected", observed, hr, 0)?;
                // A second conflict proves dropping a failed owner did not revoke A.
                let hr = self.duplicate(window)?;
                self.record(
                    generation,
                    "failed_owner_preserved_registration",
                    observed,
                    hr,
                    0,
                )?;
            }
            "hidden" | "restored" => {
                let window = window.ok_or("Missing live Slint window")?;
                let hr = self.duplicate(window)?;
                self.record(generation, phase, observed, hr, 0)?;
            }
            "before_release" => {
                let window = window.ok_or("Missing window before revocation")?;
                let mut guard = self.registration.take().ok_or("No registration owner")?;
                guard.revoke()?;
                if guard.is_registered() {
                    return Err("Explicit revocation retained registration ownership".into());
                }
                self.record(generation, "revoked", observed, HRESULT(0), 0)?;
                // SAFETY: Known live HWND, our successful revoke just removed our own
                // registration. Diagnostic repeat must fail; no other target is present.
                let error = unsafe { RevokeDragDrop(HWND(observed.hwnd as *mut _)) }
                    .expect_err("Repeated revoke must report NOTREGISTERED");
                if error.code() != DRAGDROP_E_NOTREGISTERED {
                    return Err(error.into());
                }
                self.record(
                    generation,
                    "not_registered_confirmed",
                    observed,
                    error.code(),
                    0,
                )?;
                drop(guard);
                let stats = self.stats.take().ok_or("Missing COM destruction counter")?;
                if stats.destroyed.get() != 1 {
                    return Err("Explicitly revoked COM target was not finally released".into());
                }
                self.record(
                    generation,
                    "target_released",
                    observed,
                    HRESULT(0),
                    stats.destroyed.get(),
                )?;

                let stats = Rc::new(TargetStats::default());
                let target = DropTarget::rejecting(stats.clone());
                self.registration = Some(DropRegistration::register(window, target)?);
                self.record(generation, "reregistered", observed, HRESULT(0), 0)?;
                drop(self.registration.take());
                // SAFETY: Guard Drop just revoked its successful same-thread registration;
                // this diagnostic observes absence while retaining the Slint window owner.
                let error = unsafe { RevokeDragDrop(HWND(observed.hwnd as *mut _)) }
                    .expect_err("RAII revoke must leave no registration");
                if error.code() != DRAGDROP_E_NOTREGISTERED || stats.destroyed.get() != 1 {
                    return Err("RAII registration or COM target release failed".into());
                }
                self.record(
                    generation,
                    "raii_revoked_and_released",
                    observed,
                    error.code(),
                    stats.destroyed.get(),
                )?;
            }
            "destroyed" => {
                let error = validate_observation(observed.hwnd)
                    .expect_err("Destroyed HWND must be rejected");
                if error.code() != DRAGDROP_E_INVALIDHWND {
                    return Err(error.into());
                }
                self.record(generation, "stale_rejected", observed, error.code(), 0)?;
            }
            _ => return Err("Unexpected lifecycle hook".into()),
        }
        Ok(())
    }
}

pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    writeln!(
        file,
        "elapsed_ms\tgeneration\tphase\tpid\ttid\thwnd\thresult\tis_window\tvisible\tregistered\tdestroyed"
    )?;
    file.flush()?;
    let report = Rc::new(RefCell::new(file));
    let started = Instant::now();
    let hooks = RegistrationValidation {
        registration: None,
        stats: None,
        report: report.clone(),
        started,
    };
    let lifecycle = path.with_extension("hwnd.tsv");
    crate::hwnd_validation::run_with_hooks(&lifecycle, Box::new(hooks))?;
    // SAFETY: Identity queries accept no pointers and do not alter thread state.
    let (pid, tid) = unsafe {
        (
            windows_sys::Win32::System::Threading::GetCurrentProcessId(),
            windows_sys::Win32::System::Threading::GetCurrentThreadId(),
        )
    };
    writeln!(
        report.borrow_mut(),
        "{}\t0\tcompleted\t{pid}\t{tid}\t0\t0x00000000\tfalse\tfalse\tfalse\t0",
        started.elapsed().as_millis()
    )?;
    report.borrow_mut().flush()?;
    Ok(())
}

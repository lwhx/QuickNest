use std::cell::{Cell, RefCell};
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use quicknest::infrastructure::windows::ole::{
    ApartmentObservation, CO_E_NOTINITIALIZED, OleGuard, RPC_E_CHANGED_MODE, S_FALSE, S_OK,
    apartment,
};
use slint::{Timer, TimerMode};
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};

struct Report {
    file: File,
    started: Instant,
}

impl Report {
    fn record(&mut self, case: &str, phase: &str, hr: i32) -> Result<(), Box<dyn Error>> {
        self.observation(case, phase, hr, apartment())
    }

    fn observation(
        &mut self,
        case: &str,
        phase: &str,
        hr: i32,
        observed: ApartmentObservation,
    ) -> Result<(), Box<dyn Error>> {
        // SAFETY: Identity queries accept no pointers and do not change thread state.
        let (pid, tid) = unsafe { (GetCurrentProcessId(), GetCurrentThreadId()) };
        writeln!(
            self.file,
            "{}\t{case}\t{phase}\t{pid}\t{tid}\t0x{:08X}\t0x{:08X}\t{}\t{}",
            self.started.elapsed().as_millis(),
            hr as u32,
            observed.hresult as u32,
            observed.kind,
            observed.qualifier
        )?;
        self.file.flush()?;
        Ok(())
    }
}

fn require(ok: bool, error: &str) -> Result<(), Box<dyn Error>> {
    if ok { Ok(()) } else { Err(error.into()) }
}

// This temporary baseline owns exactly one successful CoInitializeEx on its fresh
// worker thread. It is separate from the OLE guards whose reference counts we test.
struct ComBaseline;

impl Drop for ComBaseline {
    fn drop(&mut self) {
        // SAFETY: Only worker_case constructs this local value after successful COM init.
        // It is dropped on that same thread, after all nested OLE guards have gone away.
        unsafe { CoUninitialize() };
    }
}

fn worker_case(mut report: Report, case: &'static str) -> Result<Report, Box<dyn Error>> {
    require(
        apartment().hresult == CO_E_NOTINITIALIZED,
        "Fresh worker was already initialized",
    )?;
    report.record(case, "before", S_OK)?;
    let baseline = if case == "fresh" {
        None
    } else {
        let flags = if case == "mta" {
            COINIT_MULTITHREADED
        } else {
            COINIT_APARTMENTTHREADED
        };
        // SAFETY: Fresh worker, NULL reserved pointer; successful call immediately gets
        // one local RAII owner, released only after nested OLE calls on this thread.
        let hr = unsafe { CoInitializeEx(std::ptr::null(), flags as u32) };
        require(hr == S_OK, "Baseline COM initialization was not S_OK")?;
        let baseline = ComBaseline;
        report.record(case, "com_initialized", hr)?;
        Some(baseline)
    };
    if case == "mta" {
        let error = OleGuard::initialize().expect_err("MTA must reject OLE initialization");
        require(
            error.0 == RPC_E_CHANGED_MODE,
            "MTA conflict returned an unexpected HRESULT",
        )?;
        require(
            apartment().is_mta(),
            "Failed OLE call changed the existing MTA",
        )?;
        report.record(case, "conflict", error.0)?;
        // No OleUninitialize: the failed call acquired no reference.
    } else {
        let outer = OleGuard::initialize()?;
        let outer_hr = outer.hresult();
        report.record(case, "ole_outer", outer_hr)?;
        // CoInitializeEx(STA) alone does not initialize OLE. In both workers this
        // is their first OLE call; the nested call below must return S_FALSE.
        require(
            outer_hr == S_OK && apartment().is_sta(),
            "First OLE initialization was not S_OK in a compatible thread",
        )?;
        let inner = OleGuard::initialize()?;
        require(
            inner.hresult() == S_FALSE && apartment().is_sta(),
            "Nested OLE initialization was not S_FALSE",
        )?;
        report.record(case, "ole_inner", inner.hresult())?;
        drop(inner);
        require(
            apartment().is_sta(),
            "Nested release destroyed the outer apartment",
        )?;
        report.record(case, "inner_released", S_OK)?;
        drop(outer);
        let expected_after = if case == "fresh" {
            CO_E_NOTINITIALIZED
        } else {
            S_OK
        };
        require(
            apartment().hresult == expected_after,
            "Outer OLE release did not preserve the expected baseline",
        )?;
        report.record(case, "outer_released", S_OK)?;
    }
    drop(baseline);
    require(
        apartment().hresult == CO_E_NOTINITIALIZED,
        "Worker COM references were not fully released",
    )?;
    report.record(case, "cleaned", S_OK)?;
    Ok(report)
}

fn isolated_case(report: Report, case: &'static str) -> Result<Report, Box<dyn Error>> {
    // Only File/Instant/string observations cross threads, never OLE guards or interfaces.
    std::thread::spawn(move || worker_case(report, case).map_err(|error| error.to_string()))
        .join()
        .map_err(|_| "OLE worker panicked")?
        .map_err(Into::into)
}

pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    writeln!(
        file,
        "elapsed_ms\tcase\tphase\tpid\ttid\thresult\tapartment_hr\tapartment\tqualifier"
    )?;
    file.flush()?;
    let mut report = Report {
        file,
        started: Instant::now(),
    };
    for case in ["fresh", "sta", "mta"] {
        report = isolated_case(report, case)?;
    }
    let ui_before = apartment();
    report.record("ui", "before", S_OK)?;
    let ole = OleGuard::initialize()?;
    let init_hr = ole.hresult();
    let expected = if ui_before.hresult == CO_E_NOTINITIALIZED {
        S_OK
    } else {
        S_FALSE
    };
    require(
        init_hr == expected && apartment().is_sta(),
        "UI OLE initialization did not establish STA",
    )?;
    report.record("ui", "ole_initialized", init_hr)?;
    crate::select_backend()?;
    require(
        apartment().is_sta(),
        "Backend selection changed the UI apartment",
    )?;
    report.record("ui", "backend_selected", S_OK)?;

    let report = Rc::new(RefCell::new(report));
    let failure = Rc::new(RefCell::new(None::<String>));
    let ticks = Rc::new(Cell::new(0_u32));
    let timer = Timer::default();
    let report_ref = report.clone();
    let failure_ref = failure.clone();
    let tick_ref = ticks.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(25), move || {
        let result = (|| -> Result<(), Box<dyn Error>> {
            require(
                apartment().is_sta(),
                "UI timer observed a non-STA apartment",
            )?;
            let count = tick_ref.get() + 1;
            tick_ref.set(count);
            if count == 1 {
                let inner = OleGuard::initialize()?;
                require(
                    inner.hresult() == S_FALSE,
                    "UI nested OleInitialize was not S_FALSE",
                )?;
                report_ref
                    .borrow_mut()
                    .record("ui", "loop_nested", inner.hresult())?;
                drop(inner);
                require(
                    apartment().is_sta(),
                    "UI nested release removed the outer STA",
                )?;
                report_ref
                    .borrow_mut()
                    .record("ui", "loop_nested_released", S_OK)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            *failure_ref.borrow_mut() = Some(error.to_string());
            if let Err(error) = slint::quit_event_loop() {
                eprintln!("Could not quit failed OLE validation: {error}");
                std::process::exit(1);
            }
        }
    });
    let (sender, receiver) = mpsc::channel();
    // SAFETY: Current-thread identity only; captured as data for the callback check.
    let ui_tid = unsafe { GetCurrentThreadId() };
    let worker = std::thread::spawn(move || -> Result<(), String> {
        for _ in 0..3 {
            let sender = sender.clone();
            slint::invoke_from_event_loop(move || {
                // SAFETY: The callback reads its own thread identity without pointers.
                let tid = unsafe { GetCurrentThreadId() };
                let _ = sender.send((tid, apartment()));
            })
            .map_err(|error| error.to_string())?;
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    });

    // Reuse the validated HWND lifecycle runner: actual HWND creation, visibility,
    // component release and native destruction all occur while our OLE guard is alive.
    let lifecycle_path = path.with_extension("hwnd.tsv");
    let lifecycle_result = crate::hwnd_validation::run(&lifecycle_path);
    drop(timer);
    let worker_result = worker.join().map_err(|_| "UI dispatch worker panicked")?;
    worker_result?;
    lifecycle_result?;
    if let Some(error) = failure.borrow().as_ref() {
        return Err(error.clone().into());
    }
    require(
        ticks.get() >= 20,
        "Insufficient UI timer activity during OLE validation",
    )?;
    let callbacks: Vec<_> = receiver.try_iter().collect();
    require(
        callbacks.len() == 3,
        "Cross-thread UI dispatch did not deliver all three callbacks",
    )?;
    for (tid, observed) in callbacks {
        require(
            tid == ui_tid && observed.is_sta(),
            "UI callback used a different thread or non-STA apartment",
        )?;
        report
            .borrow_mut()
            .observation("ui", "dispatch_received", S_OK, observed)?;
    }
    report
        .borrow_mut()
        .record("ui", "hwnd_lifecycle_passed", S_OK)?;
    report.borrow_mut().record("ui", "before_release", S_OK)?;
    drop(ole);
    let after = apartment();
    require(
        after.hresult == CO_E_NOTINITIALIZED || after.is_sta(),
        "UI apartment incompatible after releasing our own OLE reference",
    )?;
    report
        .borrow_mut()
        .record("ui", "own_reference_released", S_OK)?;
    report.borrow_mut().record("all", "completed", S_OK)?;
    Ok(())
}

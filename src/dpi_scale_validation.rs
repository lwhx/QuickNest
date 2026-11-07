//! P2-15 (baseline 16.4): live evidence for the DPI scale factors.
//!
//! The validation window reports the scale factor it currently renders with,
//! the DPI awareness context of its window and the placement data of the
//! connected displays. The acceptance script drives the scale factor by
//! posting `WM_DPICHANGED` with the DPI of the scale under test and reads
//! the reported state back, so the four required scale factors are measured
//! on the running application instead of being asserted in the abstract.

use std::cell::RefCell;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use std::time::{Duration, Instant};

use quicknest::infrastructure::windows::placement;
use quicknest::placement::{REQUIRED_DPIS, scale_for_dpi};
use slint::ComponentHandle;

use crate::MainWindow;

fn write_line(
    report: &Rc<RefCell<File>>,
    started: Instant,
    phase: &str,
    detail: &str,
) -> Result<(), Box<dyn Error>> {
    writeln!(
        report.borrow_mut(),
        "{}\t{}\t{}",
        started.elapsed().as_millis(),
        phase,
        detail
    )?;
    Ok(())
}

/// The scale factor the window currently renders with and the state of the
/// connected displays.
fn report_state(ui: &MainWindow) -> Result<String, Box<dyn Error>> {
    let window = ui.window();
    let scale = window.scale_factor();
    let size = window.size();
    let position = window.position();
    let mut detail = format!(
        "scale={scale:.4} dpi={} size={}x{} pos={},{}",
        (scale * 96.0).round() as u32,
        size.width,
        size.height,
        position.x,
        position.y
    );
    if let Ok(observation) =
        quicknest::infrastructure::windows::window_handle::inspect_window(window)
    {
        let dpi = placement::dpi_for_window(observation.hwnd);
        detail.push_str(&format!(" window_dpi={dpi}"));
        if let Some(rect) = placement::window_rect(observation.hwnd) {
            detail.push_str(&format!(
                " rect={},{},{},{}",
                rect.left, rect.top, rect.right, rect.bottom
            ));
        }
    }
    for (index, area) in placement::monitor_areas().iter().enumerate() {
        detail.push_str(&format!(
            " monitor{index}={device}:{left},{top},{right},{bottom}@{dpi}dpi",
            device = area.device,
            left = area.work.left,
            top = area.work.top,
            right = area.work.right,
            bottom = area.work.bottom,
            dpi = area.dpi
        ));
    }
    Ok(detail)
}

/// Runs the DPI scale validation window until it is closed.
pub fn run(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let mut report = File::create(path)?;
    writeln!(report, "elapsed_ms\tphase\tdetail")?;
    report.flush()?;
    let report = Rc::new(RefCell::new(report));
    let started = Instant::now();

    let ui = crate::create_window()?;
    ui.set_validation_status("P2-15：DPI 缩放验证".into());
    ui.on_exit_requested(|| {
        let _ = slint::quit_event_loop();
    });
    ui.window().on_close_requested(|| {
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });
    ui.show()?;

    write_line(&report, started, "monitors", &report_state(&ui)?)?;
    for dpi in REQUIRED_DPIS {
        write_line(
            &report,
            started,
            "required_scale",
            &format!("dpi={dpi} scale={:.4}", scale_for_dpi(dpi)),
        )?;
    }

    // The script posts WM_DPICHANGED; the window reports the resulting state
    // on every tick so the change is captured with its timestamp.
    let weak = ui.as_weak();
    let report_ref = report.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(250),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let detail = match report_state(&ui) {
                Ok(detail) => detail,
                Err(error) => format!("error={error}"),
            };
            let _ = write_line(&report_ref, started, "state", &detail);
        },
    );
    // Keep the timer alive until the window is closed.
    slint::run_event_loop_until_quit()?;
    write_line(
        &report,
        started,
        "completed",
        "dpi scale validation finished",
    )?;
    Ok(())
}

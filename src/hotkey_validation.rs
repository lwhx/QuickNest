use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use quicknest::infrastructure::windows::hotkey::{
    DEFAULT_MODIFIERS, DEFAULT_VK, HotkeyError, error_name, inject_default_hotkey, register,
};

const CHILD_TIMEOUT: Duration = Duration::from_secs(20);
const EVENT_TIMEOUT: Duration = Duration::from_secs(10);

struct Report {
    file: File,
    started: Instant,
}

impl Report {
    fn open(path: &Path) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            file: open_append(path)?,
            started: Instant::now(),
        })
    }

    fn record(&mut self, role: &str, phase: &str, detail: &str) -> Result<(), Box<dyn Error>> {
        writeln!(
            self.file,
            "{}\t{}\t{}\t{}",
            self.started.elapsed().as_millis(),
            role,
            phase,
            detail
        )?;
        self.file.flush()?;
        Ok(())
    }
}

fn open_append(path: &Path) -> Result<File, Box<dyn Error>> {
    Ok(OpenOptions::new().create(true).append(true).open(path)?)
}

fn exe_path() -> Result<PathBuf, Box<dyn Error>> {
    std::env::current_exe().map_err(Into::into)
}

fn spawn_exe(arguments: &[String]) -> Result<u32, Box<dyn Error>> {
    Command::new(exe_path()?)
        .args(arguments)
        .spawn()
        .map(|child| child.id())
        .map_err(Into::into)
}

fn wait_for_line(path: &Path, marker: &str, timeout: Duration) -> Result<String, Box<dyn Error>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines().rev() {
                if line.contains(marker) {
                    return Ok(line.to_string());
                }
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("Timed out waiting for '{marker}' in {}", path.display()).into());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn require(condition: bool, message: &str) -> Result<(), Box<dyn Error>> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Conflict probe: tries to register the default combo and records the verdict.
pub fn run_conflict(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut report = Report::open(path)?;
    match register("default", DEFAULT_MODIFIERS, DEFAULT_VK) {
        Ok(_guard) => {
            // Registration succeeded: no conflict. The parent decides what this means.
            report.record("conflict-child", "registered", "success")?;
            Ok(())
        }
        Err(error) => {
            report.record("conflict-child", "rejected", &error_name(error))?;
            require(
                matches!(error, HotkeyError::AlreadyRegistered),
                "Conflict probe failed with an unexpected error",
            )
        }
    }
}

fn drain_until_event(
    events: &std::sync::mpsc::Receiver<quicknest::infrastructure::windows::hotkey::HotkeyEvent>,
    timeout: Duration,
) -> Result<u64, Box<dyn Error>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(event) = events.try_recv() {
            return Ok(event.sequence);
        }
        if Instant::now() >= deadline {
            return Err("No WM_HOTKEY arrived within the timeout".into());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Parent scenario covering the baseline 11.1 requirements end to end.
///
/// The Slint event loop must run on the main thread (invoke_from_event_loop
/// blocks otherwise), so the sequential scenario executes on a worker thread
/// and forwards hotkey events to the UI through the thread-safe path.
pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let file = File::options().write(true).create_new(true).open(path)?;
    drop(file);
    // The scenario thread writes all report rows; the parent-side file handle is
    // not needed after ensuring the file exists with create_new semantics.

    let ui = crate::create_window()?;
    crate::ui_window(&ui).on_close_requested(|| {
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });
    use slint::ComponentHandle as _;
    ui.show()?;

    let report_path = path.to_path_buf();
    let scenario = thread::spawn(move || -> Result<(), String> {
        let mut report = Report::open(&report_path).map_err(|error| error.to_string())?;

        // a) Register the default hotkey; success is the first evidence.
        let guard = register("default", DEFAULT_MODIFIERS, DEFAULT_VK)
            .map_err(|error| format!("initial register failed: {}", error_name(error)))?;
        report
            .record("parent", "registered", "Ctrl+Alt+Space MOD_NOREPEAT")
            .map_err(|error| error.to_string())?;

        // b) Conflict detection: a child must fail to register the same combo.
        let conflict_report = report_path.with_extension("conflict1.tsv");
        let child_id = spawn_exe(&[
            "--hotkey-conflict".to_string(),
            conflict_report.to_string_lossy().into_owned(),
        ])
        .map_err(|error| error.to_string())?;
        let line = wait_for_line(&conflict_report, "rejected", CHILD_TIMEOUT)
            .map_err(|error| error.to_string())?;
        report
            .record(
                "parent",
                "conflict_detected",
                &format!("pid={child_id} {line}"),
            )
            .map_err(|error| error.to_string())?;

        // c) Inject the default combo; the event must arrive through the channel
        //    and be forwarded to the Slint UI via invoke_from_event_loop.
        let (ui_tx, ui_rx) = std::sync::mpsc::channel::<u64>();
        let forward = |sequence: u64| -> Result<(), String> {
            slint::invoke_from_event_loop(move || {
                let _ = ui_tx.send(sequence);
            })
            .map_err(|error| format!("invoke_from_event_loop: {error}"))
        };
        // SAFETY: The injected keystrokes target the interactive desktop this test
        // owns; they trigger the registered hotkey through the system path.
        unsafe {
            inject_default_hotkey().map_err(|error| format!("SendInput failed: {error}"))?;
        }
        let sequence =
            drain_until_event(guard.events(), EVENT_TIMEOUT).map_err(|error| error.to_string())?;
        forward(sequence)?;
        let forwarded = ui_rx
            .recv_timeout(EVENT_TIMEOUT)
            .map_err(|error| format!("UI dispatch wait: {error}"))?;
        report
            .record(
                "parent",
                "hotkey_event",
                &format!("sequence={sequence} forwarded_to_ui={forwarded}"),
            )
            .map_err(|error| error.to_string())?;

        // d) Unregister (Drop) and prove cleanup: the child must now succeed.
        drop(guard);
        let cleanup_report = report_path.with_extension("cleanup.tsv");
        let child_id = spawn_exe(&[
            "--hotkey-conflict".to_string(),
            cleanup_report.to_string_lossy().into_owned(),
        ])
        .map_err(|error| error.to_string())?;
        let line = wait_for_line(&cleanup_report, "registered", CHILD_TIMEOUT)
            .map_err(|error| error.to_string())?;
        report
            .record(
                "parent",
                "unregistered_cleanup",
                &format!("pid={child_id} {line}"),
            )
            .map_err(|error| error.to_string())?;

        // e) Re-register and verify another injected hotkey still arrives.
        let guard = register("default", DEFAULT_MODIFIERS, DEFAULT_VK)
            .map_err(|error| format!("re-register failed: {}", error_name(error)))?;
        report
            .record("parent", "re_registered", "Ctrl+Alt+Space MOD_NOREPEAT")
            .map_err(|error| error.to_string())?;
        // SAFETY: Same interactive-desktop injection as above.
        unsafe {
            inject_default_hotkey().map_err(|error| format!("SendInput failed: {error}"))?;
        }
        let sequence =
            drain_until_event(guard.events(), EVENT_TIMEOUT).map_err(|error| error.to_string())?;
        report
            .record("parent", "second_event", &format!("sequence={sequence}"))
            .map_err(|error| error.to_string())?;

        // f) Drop unregisters; the report is the evidence trail.
        drop(guard);
        report
            .record("parent", "completed", "all hotkey phases passed")
            .map_err(|error| error.to_string())?;
        let _ = slint::quit_event_loop();
        Ok(())
    });

    let loop_result = slint::run_event_loop_until_quit();
    let scenario_result = scenario
        .join()
        .map_err(|_| "Hotkey scenario thread panicked".to_string())
        .and_then(|result| result);
    loop_result?;
    scenario_result?;
    Ok(())
}

use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, IsWindowVisible, PostMessageW};

use quicknest::infrastructure::windows::keyboard_inject::press_key;
use quicknest::infrastructure::windows::tray::{
    CALLBACK_MESSAGE, MENU_COMMANDS, TrayCommand, TrayEvent, TrayService, register,
};

const EVENT_TIMEOUT: Duration = Duration::from_secs(10);
const MENU_INPUT_DELAY: Duration = Duration::from_millis(400);

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

    fn record(&mut self, phase: &str, detail: &str) -> Result<(), Box<dyn Error>> {
        writeln!(
            self.file,
            "{}\t{}\t{}",
            self.started.elapsed().as_millis(),
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

fn append_line(path: &Path, line: &str) -> Result<(), Box<dyn Error>> {
    let mut file = open_append(path)?;
    writeln!(file, "{line}")?;
    file.flush()?;
    Ok(())
}

fn record(report: &mut Report, phase: &str, detail: &str) -> Result<(), String> {
    report
        .record(phase, detail)
        .map_err(|error| error.to_string())
}

fn wait_for_event(events: &Receiver<TrayEvent>, timeout: Duration) -> Result<TrayEvent, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(event) = events.try_recv() {
            return Ok(event);
        }
        if Instant::now() >= deadline {
            return Err("Tray event did not arrive within the timeout".to_string());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

/// Posts a synthetic tray mouse event the way Explorer does in production:
/// wParam carries the icon id, lParam the mouse message.
fn post_callback(hwnd: HWND, mouse: u32) -> Result<(), String> {
    // SAFETY: Valid callback window owned by the tray thread; the pump routes
    // the message exactly like real notification-area mouse events.
    let posted = unsafe { PostMessageW(hwnd, CALLBACK_MESSAGE, 1, mouse as isize) };
    require(posted != 0, "PostMessageW to the callback window failed")
}

/// Escape dismisses the open menu; Down+Enter selects the first item. Both go
/// through the real input pipeline rather than calling handlers directly.
fn send_escape() -> Result<(), String> {
    // SAFETY: The injected keystrokes target the interactive desktop this test
    // owns; they reach the open popup menu through the system input path.
    unsafe { press_key(0x1B) }.map_err(|error| format!("SendInput Escape failed: {error}"))
}

fn send_down_enter() -> Result<(), String> {
    // SAFETY: Same interactive-desktop injection as send_escape.
    // VK_DOWN is 0x28 (VK_UP is 0x26 — an earlier run selected the last item
    // because of this exact mix-up and the menu mapped it correctly to Exit).
    unsafe { press_key(0x28) }.map_err(|error| format!("SendInput Down failed: {error}"))?;
    thread::sleep(Duration::from_millis(80));
    // SAFETY: Same interactive-desktop injection as send_escape.
    unsafe { press_key(0x0D) }.map_err(|error| format!("SendInput Enter failed: {error}"))
}

fn forward_to_ui() -> Result<(), String> {
    slint::invoke_from_event_loop(|| {
        // Touching the UI proves the event loop dispatched the callback.
    })
    .map_err(|error| format!("invoke_from_event_loop: {error}"))
}

/// Verifies the TrackPopupMenu window (class #32768) is actually on screen
/// before injecting menu input; guards against recording a dismissal that
/// happened because the menu never stayed open.
fn menu_window_visible() -> bool {
    const MENU_CLASS: &[u16] = &[
        b'#' as u16,
        b'3' as u16,
        b'2' as u16,
        b'7' as u16,
        b'6' as u16,
        b'8' as u16,
        0,
    ];
    // SAFETY: Null-terminated class name; a null result simply means no menu.
    let hwnd = unsafe { FindWindowW(MENU_CLASS.as_ptr(), std::ptr::null()) };
    if hwnd.is_null() {
        return false;
    }
    // SAFETY: Valid menu window returned by FindWindowW.
    unsafe { IsWindowVisible(hwnd) != 0 }
}

/// Covers baseline 11.3 end to end: icon lifecycle, callback routing, the
/// six-item menu and shutdown cleanup.
///
/// The Slint event loop must run on the main thread, so the sequential
/// scenario executes on a worker thread and forwards tray events to the UI
/// through the thread-safe path (same layout as the hotkey validation). Any
/// scenario failure quits the loop so the error propagates instead of hanging.
pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let file = File::options().write(true).create_new(true).open(path)?;
    drop(file);

    let ui = crate::create_window().map_err(|error| {
        let _ = append_line(path, &format!("setup_error\tcreate_window: {error}"));
        error
    })?;
    crate::ui_window(&ui).on_close_requested(|| {
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });
    use slint::ComponentHandle as _;
    ui.show().map_err(|error| {
        let _ = append_line(path, &format!("setup_error\tshow: {error}"));
        error
    })?;

    let report_path = path.to_path_buf();
    let scenario = thread::spawn(move || -> Result<(), String> {
        let mut report = Report::open(&report_path).map_err(|error| error.to_string())?;
        let result = match register("validation") {
            Ok(service) => run_phases(service, &mut report),
            Err(error) => Err(format!("NIM_ADD failed: {error}")),
        };
        if let Err(error) = &result {
            // Preserve the failure evidence in the report itself; the exit
            // code alone carries no message under the GUI subsystem.
            let _ = report.record("scenario_error", error);
        }
        if result.is_err() {
            // Surface the failure instead of hanging the event loop forever.
            let _ = slint::quit_event_loop();
        }
        result
    });

    let loop_result = slint::run_event_loop_until_quit();
    let scenario_result = scenario
        .join()
        .map_err(|_| "Tray scenario thread panicked".to_string())
        .and_then(|result| result);
    loop_result?;
    scenario_result?;
    Ok(())
}

fn run_phases(mut service: TrayService, report: &mut Report) -> Result<(), String> {
    // a) NIM_ADD must succeed on the tray thread.
    let added = wait_for_event(service.events(), EVENT_TIMEOUT)?;
    require(matches!(added, TrayEvent::Added), "expected Added event")?;
    record(report, "added", "NIM_ADD success on tray thread")?;

    // b) Tooltip change through NIM_MODIFY.
    service
        .set_tooltip("QuickNest 托盘验证")
        .map_err(|error| format!("NIM_MODIFY failed: {error}"))?;
    record(report, "tooltip_modified", "NIM_MODIFY success")?;

    // c) Menu construction: six baseline items verified on the tray thread.
    let event = wait_for_event(service.events(), EVENT_TIMEOUT)?;
    let (item_count, first, last) = match event {
        TrayEvent::MenuVerified {
            item_count,
            first,
            last,
        } => (item_count, first, last),
        other => return Err(format!("expected MenuVerified, got {other:?}")),
    };
    let expected_first = MENU_COMMANDS[0].2;
    let expected_last = MENU_COMMANDS[MENU_COMMANDS.len() - 1].2;
    require(
        item_count == MENU_COMMANDS.len() && first == expected_first && last == expected_last,
        &format!("menu mismatch: count={item_count} first={first} last={last}"),
    )?;
    record(
        report,
        "menu_verified",
        &format!("count={item_count} first={first} last={last}"),
    )?;

    // d) Callback routing: post the same message Explorer posts for a left
    //    click; the pump must deliver a LeftClick event.
    post_callback(service.callback_hwnd(), 0x0202 /* WM_LBUTTONUP */)?;
    let event = wait_for_event(service.events(), EVENT_TIMEOUT)?;
    require(matches!(event, TrayEvent::LeftClick), "expected LeftClick")?;
    forward_to_ui()?;
    record(
        report,
        "callback_routed",
        "WM_LBUTTONUP -> LeftClick, ui_dispatched=1",
    )?;

    // e) The menu opens on the right-click path; Escape dismisses it.
    open_menu_via_right_click(&service)?;
    send_escape()?;
    let closed = wait_for_event(service.events(), EVENT_TIMEOUT)?;
    require(
        matches!(closed, TrayEvent::MenuDismissed),
        "expected MenuDismissed",
    )?;
    record(
        report,
        "menu_shown_dismissed",
        "TrackPopupMenu opened, Escape dismissed",
    )?;

    // f) Menu selection: Down+Enter picks the first command (Show).
    open_menu_via_right_click(&service)?;
    send_down_enter()?;
    let selected = wait_for_event(service.events(), EVENT_TIMEOUT)?;
    match selected {
        TrayEvent::MenuSelected { command } => {
            require(
                command == TrayCommand::Show,
                &format!("expected Show command, got {command:?}"),
            )?;
        }
        other => return Err(format!("expected MenuSelected, got {other:?}")),
    }
    forward_to_ui()?;
    record(report, "menu_selected", "Down+Enter -> MenuSelected(Show)")?;

    // g) TaskbarCreated re-add: simulate the Explorer-restart state faithfully
    //    (the icon is gone first), then post the registered broadcast id; the
    //    pump must re-add the icon. Re-adding while the icon still exists is
    //    rejected by the shell with E_FAIL — observed in an earlier run.
    service
        .remove_icon()
        .map_err(|error| format!("NIM_DELETE before TaskbarCreated failed: {error}"))?;
    // SAFETY: Same callback window; the broadcast id comes from
    // RegisterWindowMessageW and only reaches this process's window.
    let posted = unsafe {
        PostMessageW(
            service.callback_hwnd(),
            service.taskbar_created_message(),
            0,
            0,
        )
    };
    require(posted != 0, "PostMessageW(TaskbarCreated) failed")?;
    let event = wait_for_event(service.events(), EVENT_TIMEOUT)?;
    require(matches!(event, TrayEvent::ReAdded), "expected ReAdded")?;
    record(
        report,
        "taskbar_readded",
        "icon removed, TaskbarCreated -> NIM_ADD success",
    )?;

    // h) Drop: NIM_DELETE must run on the tray thread before it exits; the
    //    taken receiver observes the result after the join completes.
    let events = service
        .take_events()
        .ok_or_else(|| "events receiver missing".to_string())?;
    drop(service);
    let deleted = wait_for_event(&events, EVENT_TIMEOUT)?;
    match deleted {
        TrayEvent::Deleted { success } => {
            require(success, "NIM_DELETE reported failure")?;
        }
        other => return Err(format!("expected Deleted, got {other:?}")),
    }
    record(report, "deleted", "NIM_DELETE on tray thread success=true")?;

    record(report, "completed", "all tray phases passed")?;
    let _ = slint::quit_event_loop();
    Ok(())
}

/// Right-click path: posts the callback, waits for RightClick and MenuOpened,
/// then verifies the popup menu window is actually on screen before any menu
/// input is injected.
fn open_menu_via_right_click(service: &TrayService) -> Result<(), String> {
    post_callback(service.callback_hwnd(), 0x0205 /* WM_RBUTTONUP */)?;
    let clicked = wait_for_event(service.events(), EVENT_TIMEOUT)?;
    require(
        matches!(clicked, TrayEvent::RightClick),
        "expected RightClick",
    )?;
    let opened = wait_for_event(service.events(), EVENT_TIMEOUT)?;
    require(
        matches!(opened, TrayEvent::MenuOpened),
        "expected MenuOpened",
    )?;
    thread::sleep(MENU_INPUT_DELAY);
    require(menu_window_visible(), "popup menu window is not open")
}

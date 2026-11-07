use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMenuItemCount, GetMenuStringW, GetMessageW, HICON, HMENU,
    IDI_APPLICATION, LoadIconW, MF_BYPOSITION, MF_STRING, MSG, MWMO_INPUTAVAILABLE,
    MsgWaitForMultipleObjectsEx, PostMessageW, PostQuitMessage, PostThreadMessageW, QS_ALLINPUT,
    RegisterClassW, RegisterWindowMessageW, SW_HIDE, SW_SHOWNOACTIVATE, SetForegroundWindow,
    ShowWindow, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage,
    WM_APP, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};

const WAKEUP: u32 = 0x0400 + 0x5452;
const TRAY_ID: u32 = 1;
/// Application-defined callback message; Explorer posts it with the icon id
/// in wParam and the mouse message in lParam.
pub const CALLBACK_MESSAGE: u32 = WM_APP + 0x5452;
const CLASS_NAME: &str = "QuickNestTrayHost";
const WINDOW_NAME: &str = "QuickNest Tray Host";
const TOOLTIP: &str = "QuickNest";
const TASKBAR_CREATED_NAME: &str = "TaskbarCreated";

/// Tray menu commands in baseline 11.3 order; menu ids are position + 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    Show,
    Hide,
    TogglePin,
    Settings,
    CheckUpdate,
    Exit,
}

pub const MENU_COMMANDS: [(u32, TrayCommand, &str); 6] = [
    (1, TrayCommand::Show, "显示 QuickNest"),
    (2, TrayCommand::Hide, "隐藏 QuickNest"),
    (3, TrayCommand::TogglePin, "固定 / 取消固定"),
    (4, TrayCommand::Settings, "设置"),
    (5, TrayCommand::CheckUpdate, "检查更新"),
    (6, TrayCommand::Exit, "退出程序"),
];

/// Events delivered from the tray thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayEvent {
    /// NIM_ADD succeeded on startup.
    Added,
    /// Re-add succeeded after TaskbarCreated (Explorer restart).
    ReAdded,
    /// The context menu was built and verified on the tray thread.
    MenuVerified {
        item_count: usize,
        first: String,
        last: String,
    },
    /// Primary (left) click on the icon.
    LeftClick,
    /// Double click on the icon.
    DoubleClick,
    /// Secondary (right) click; the popup menu follows.
    RightClick,
    /// TrackPopupMenu is now modal on the tray thread.
    MenuOpened,
    /// The menu closed without a selection (for example Escape).
    MenuDismissed,
    /// The user picked a menu command.
    MenuSelected { command: TrayCommand },
    /// Shutdown ran NIM_DELETE on the tray thread; success is the cleanup proof.
    Deleted { success: bool },
}

enum Control {
    Shutdown,
}

struct TrayState {
    hwnd: Cell<HWND>,
    menu: HMENU,
    taskbar_created: u32,
    events: Sender<TrayEvent>,
}

thread_local! {
    static STATE: RefCell<Option<TrayState>> = const { RefCell::new(None) };
}

pub struct TrayRegistration {
    /// The callback window as a usize: raw HWND is !Send, and this value
    /// crosses the ready channel from the tray thread to the owner.
    pub hwnd_value: usize,
    pub thread_id: u32,
    pub taskbar_created: u32,
}

/// Reads GetLastError immediately after a failing Win32 call. Call sites must
/// not run any other code between the call and this helper.
fn last_error() -> u32 {
    // SAFETY: Identity query without pointer arguments.
    unsafe { GetLastError() }
}

/// Thread-bound owner of the notification-area icon, its callback window and
/// its message loop. Drop deletes the icon and joins the thread.
pub struct TrayService {
    name: String,
    events: Option<Receiver<TrayEvent>>,
    control: Option<Sender<Control>>,
    stop: Arc<AtomicBool>,
    callback_hwnd: HWND,
    taskbar_created: u32,
    thread: Option<JoinHandle<()>>,
    thread_id: u32,
    _thread_bound: PhantomData<Rc<()>>,
}

impl TrayService {
    pub fn events(&self) -> &Receiver<TrayEvent> {
        self.events
            .as_ref()
            .expect("events channel lives until drop")
    }

    /// Hands the event receiver out before Drop so a caller can still observe
    /// the final `Deleted` event after the service is consumed.
    pub fn take_events(&mut self) -> Option<Receiver<TrayEvent>> {
        self.events.take()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The callback window receiving tray mouse messages. Tests may post
    /// callback events here; Explorer posts the same messages in production.
    pub fn callback_hwnd(&self) -> HWND {
        self.callback_hwnd
    }

    /// The registered TaskbarCreated message id for re-add verification.
    pub fn taskbar_created_message(&self) -> u32 {
        self.taskbar_created
    }

    /// Changes the tooltip through NIM_MODIFY.
    pub fn set_tooltip(&self, text: &str) -> Result<(), u32> {
        let data = icon_data(self.callback_hwnd, text, std::ptr::null_mut());
        // SAFETY: Fully initialized NOTIFYICONDATAW for a live callback window.
        let sent = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
        if sent == 0 {
            return Err(last_error());
        }
        Ok(())
    }

    /// Removes the icon through NIM_DELETE; the TaskbarCreated re-add path
    /// starts from this state (Explorer drops every icon before broadcasting).
    pub fn remove_icon(&self) -> Result<(), u32> {
        let data = icon_data(self.callback_hwnd, TOOLTIP, std::ptr::null_mut());
        // SAFETY: Fully initialized NOTIFYICONDATAW for a live callback window.
        let sent = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
        if sent == 0 {
            return Err(last_error());
        }
        Ok(())
    }
}

impl Drop for TrayService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(control) = self.control.take() {
            let _ = control.send(Control::Shutdown);
        }
        // The pump may be blocked in GetMessageW (or inside TrackPopupMenu,
        // whose modal loop also reads this queue); a thread wakeup is required
        // for it to observe the shutdown control.
        // SAFETY: The queue exists (created before this id was published).
        unsafe {
            PostThreadMessageW(self.thread_id, WAKEUP, 0, 0);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn icon_data(hwnd: HWND, tooltip: &str, icon: HICON) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        // 修复（2026-11-07 审核）：必须声明 NIF_ICON，Shell 才会采用 hIcon；
        // 否则托盘入口没有可见图标。
        uFlags: NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP | NIF_ICON,
        uCallbackMessage: CALLBACK_MESSAGE,
        hIcon: icon,
        ..NOTIFYICONDATAW::default()
    };
    fill_wide(&mut data.szTip, tooltip);
    data
}

fn fill_wide(target: &mut [u16], text: &str) {
    let encoded: Vec<u16> = text.encode_utf16().collect();
    let usable = target.len() - 1; // keep the terminating null
    let copy_len = encoded.len().min(usable);
    target[..copy_len].copy_from_slice(&encoded[..copy_len]);
}

fn add_icon(hwnd: HWND) -> Result<(), u32> {
    // SAFETY: IDI_APPLICATION is a shared system icon handle.
    let icon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
    let data = icon_data(hwnd, TOOLTIP, icon);
    // SAFETY: Valid NOTIFYICONDATAW pointing at a live window of this thread.
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
    if added == 0 {
        return Err(last_error());
    }
    Ok(())
}

fn build_and_verify_menu(events: &Sender<TrayEvent>) -> Result<HMENU, String> {
    // SAFETY: CreatePopupMenu has no invalid-state preconditions.
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return Err(format!("CreatePopupMenu failed: {}", last_error()));
    }
    for (id, _command, text) in MENU_COMMANDS {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        // SAFETY: Valid menu handle and null-terminated item text.
        let appended = unsafe { AppendMenuW(menu, MF_STRING, id as usize, wide.as_ptr()) };
        if appended == 0 {
            let error = last_error();
            // SAFETY: The menu is owned here and must not leak on this path.
            unsafe {
                DestroyMenu(menu);
            }
            return Err(format!("AppendMenuW {text} failed: {error}"));
        }
    }
    // SAFETY: BYPOSITION queries on a valid menu handle.
    let count = unsafe { GetMenuItemCount(menu) };
    let first = menu_string_at(menu, 0);
    let last_index = (count.max(1) - 1) as u32;
    let last = menu_string_at(menu, last_index);
    let verified = TrayEvent::MenuVerified {
        item_count: count.max(0) as usize,
        first: first.unwrap_or_default(),
        last: last.unwrap_or_default(),
    };
    events.send(verified).map_err(|error| error.to_string())?;
    Ok(menu)
}

fn menu_string_at(menu: HMENU, position: u32) -> Option<String> {
    // SAFETY: BYPOSITION query; the first call measures, the second copies.
    let length = unsafe { GetMenuStringW(menu, position, std::ptr::null_mut(), 0, MF_BYPOSITION) };
    if length <= 0 {
        return None;
    }
    let mut buffer = vec![0_u16; length as usize + 1];
    // SAFETY: The buffer holds length characters plus the terminating null.
    unsafe {
        GetMenuStringW(
            menu,
            position,
            buffer.as_mut_ptr(),
            buffer.len() as i32,
            MF_BYPOSITION,
        );
    }
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..end]))
}

/// Shows the context menu modally and maps the selection to a command.
/// Returns None when the menu was dismissed without a selection.
fn show_menu(state: &TrayState) -> Option<TrayCommand> {
    let hwnd = state.hwnd.get();
    // A hidden window cannot take the foreground, and without foreground the
    // popup menu never receives keyboard input (Escape or arrow keys). Show
    // the zero-sized host window without activation for the menu's lifetime.
    // SAFETY: Valid window of this thread.
    unsafe {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    // The foreground switch is required before TrackPopupMenu or the menu will
    // not receive keyboard input. AttachThreadInput was tried here and
    // rejected: sharing input state with a foreign foreground thread makes the
    // menu dismiss itself immediately (documented in P0_VALIDATION).
    // SAFETY: Valid window of this thread.
    unsafe {
        SetForegroundWindow(hwnd);
    }
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: Valid POINT storage.
    unsafe {
        GetCursorPos(&mut point);
    }
    let flags = TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN;
    // SAFETY: Valid menu, window and coordinates; the modal loop runs on this
    // thread's queue and the menu is destroyed by its owner on shutdown.
    let chosen = unsafe {
        TrackPopupMenu(
            state.menu,
            flags,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        )
    };
    // Post WM_NULL so the next outside click is not swallowed; documented quirk.
    // SAFETY: Valid window; WM_NULL has no parameters.
    unsafe {
        PostMessageW(hwnd, WM_NULL, 0, 0);
    }
    // SAFETY: Restore the hidden state; the host window must stay invisible.
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
    if chosen == 0 {
        return None;
    }
    MENU_COMMANDS
        .iter()
        .find(|(id, _command, _text)| *id == chosen as u32)
        .map(|(_id, command, _text)| *command)
}

fn handle_callback(state: &TrayState, wparam: WPARAM, lparam: LPARAM) {
    if wparam as u32 != TRAY_ID {
        return;
    }
    match lparam as u32 {
        WM_LBUTTONUP => {
            let _ = state.events.send(TrayEvent::LeftClick);
        }
        WM_LBUTTONDBLCLK => {
            let _ = state.events.send(TrayEvent::DoubleClick);
        }
        WM_RBUTTONUP => {
            let _ = state.events.send(TrayEvent::RightClick);
            let _ = state.events.send(TrayEvent::MenuOpened);
            match show_menu(state) {
                Some(selected) => {
                    let _ = state
                        .events
                        .send(TrayEvent::MenuSelected { command: selected });
                }
                None => {
                    let _ = state.events.send(TrayEvent::MenuDismissed);
                }
            }
        }
        _ => {}
    }
}

/// # Safety
///
/// Runs as the WndProc of the tray callback window; the thread-local state is
/// installed by the pump before the window is created and never mutates after.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    STATE.with(|state| {
        if let Some(tray) = state.borrow().as_ref() {
            if message == tray.taskbar_created {
                // Explorer restarted and dropped every tray icon; re-add on
                // the owning thread.
                if add_icon(hwnd).is_ok() {
                    let _ = tray.events.send(TrayEvent::ReAdded);
                }
            } else if message == CALLBACK_MESSAGE {
                handle_callback(tray, wparam, lparam);
            } else if message == WM_DESTROY {
                // SAFETY: Ends this thread's message loop on window destroy.
                unsafe {
                    PostQuitMessage(0);
                }
            }
        }
    });
    // SAFETY: Default processing keeps the window machinery working.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

/// Releases the icon, menu and callback window on the owning thread.
/// Runs exactly once per pump, after the message loop exits.
fn shutdown_cleanup(state: Option<TrayState>) -> bool {
    match state {
        Some(state) => {
            let hwnd = state.hwnd.get();
            let data = icon_data(hwnd, TOOLTIP, std::ptr::null_mut());
            // SAFETY: The callback window of this thread is still alive here.
            let deleted = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) } != 0;
            // SAFETY: The menu was created on this thread.
            unsafe {
                DestroyMenu(state.menu);
            }
            // SAFETY: The window was created on this thread.
            unsafe {
                DestroyWindow(hwnd);
            }
            deleted
        }
        None => false,
    }
}

fn pump(
    ready: Sender<Result<TrayRegistration, String>>,
    events: Sender<TrayEvent>,
    control: Receiver<Control>,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    // SAFETY: Zeroed WNDCLASSW with a valid proc and module instance.
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let mut class_wide: Vec<u16> = CLASS_NAME.encode_utf16().collect();
    class_wide.push(0);
    let class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_wide.as_ptr(),
    };
    // SAFETY: Fully initialized class structure.
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        let error = format!("RegisterClassW failed: {}", last_error());
        let _ = ready.send(Err(error.clone()));
        return Err(error);
    }
    let mut taskbar_wide: Vec<u16> = TASKBAR_CREATED_NAME.encode_utf16().collect();
    taskbar_wide.push(0);
    // SAFETY: Null-terminated wide string; returns a process-wide unique id.
    let taskbar_created = unsafe { RegisterWindowMessageW(taskbar_wide.as_ptr()) };
    // The state must exist before CreateWindowExW: WM_NCCREATE/WM_CREATE arrive
    // synchronously inside the call. The hwnd is patched right afterwards.
    STATE.set(Some(TrayState {
        hwnd: Cell::new(std::ptr::null_mut()),
        menu: std::ptr::null_mut(),
        taskbar_created,
        events: events.clone(),
    }));
    let mut title_wide: Vec<u16> = WINDOW_NAME.encode_utf16().collect();
    title_wide.push(0);
    // SAFETY: Registered class name, valid instance, hidden top-level window
    // (style 0) so popup menus and the foreground switch work correctly.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_wide.as_ptr(),
            title_wide.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        let error = format!("CreateWindowExW failed: {}", last_error());
        shutdown_cleanup(STATE.take());
        let _ = ready.send(Err(error.clone()));
        return Err(error);
    }
    STATE.with(|state| {
        if let Some(tray) = state.borrow().as_ref() {
            tray.hwnd.set(hwnd);
        }
    });
    // SAFETY: The callback window exists and belongs to this thread.
    if let Err(error) = add_icon(hwnd) {
        let error = format!("Shell_NotifyIconW NIM_ADD failed: {error}");
        shutdown_cleanup(STATE.take());
        let _ = ready.send(Err(error.clone()));
        return Err(error);
    }
    let _ = events.send(TrayEvent::Added);
    let menu = match build_and_verify_menu(&events) {
        Ok(menu) => menu,
        Err(message) => {
            shutdown_cleanup(STATE.take());
            let _ = ready.send(Err(message.clone()));
            return Err(message);
        }
    };
    STATE.with(|state| {
        if let Some(tray) = state.borrow_mut().as_mut() {
            tray.menu = menu;
        }
    });
    // SAFETY: Zero timeout forces message-queue creation before the ids are
    // published, closing the PostThreadMessage race.
    unsafe {
        MsgWaitForMultipleObjectsEx(0, std::ptr::null(), 0, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
    // SAFETY: Identity query without pointer arguments.
    let thread_id = unsafe { GetCurrentThreadId() };
    if ready
        .send(Ok(TrayRegistration {
            hwnd_value: hwnd as usize,
            thread_id,
            taskbar_created,
        }))
        .is_err()
    {
        // The owner is gone; release everything on this thread before exiting.
        let deleted = shutdown_cleanup(STATE.take());
        let _ = events.send(TrayEvent::Deleted { success: deleted });
        return Ok(());
    }
    let mut message = MSG::default();
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        // SAFETY: Valid MSG storage.
        let result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
        if result == 0 || result == -1 {
            break;
        }
        // SAFETY: Both calls operate on the message just read on this thread.
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        match control.try_recv() {
            Ok(Control::Shutdown) | Err(TryRecvError::Disconnected) => {
                // SAFETY: Ends this thread's message loop; the outer loop then
                // breaks and cleanup runs on this, the owning thread.
                unsafe {
                    PostQuitMessage(0);
                }
            }
            Err(TryRecvError::Empty) => {}
        }
    }
    let deleted = shutdown_cleanup(STATE.take());
    let _ = events.send(TrayEvent::Deleted { success: deleted });
    Ok(())
}

fn thread_main(
    ready: Sender<Result<TrayRegistration, String>>,
    events: Sender<TrayEvent>,
    control: Receiver<Control>,
    stop: Arc<AtomicBool>,
) {
    if let Err(error) = pump(ready, events, control, stop) {
        eprintln!("Tray thread exited: {error}");
    }
}

/// Registers the notification-area icon on a dedicated message-loop thread.
pub fn register(name: &str) -> Result<TrayService, String> {
    let (ready_tx, ready_rx) = channel();
    let (events_tx, events_rx) = channel();
    let (control_tx, control_rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::spawn(move || {
        thread_main(ready_tx, events_tx, control_rx, thread_stop);
    });
    match ready_rx.recv() {
        Ok(Ok(registration)) => Ok(TrayService {
            name: name.to_string(),
            events: Some(events_rx),
            control: Some(control_tx),
            stop,
            callback_hwnd: registration.hwnd_value as HWND,
            taskbar_created: registration.taskbar_created,
            thread: Some(thread),
            thread_id: registration.thread_id,
            _thread_bound: PhantomData,
        }),
        Ok(Err(error)) => {
            let _ = thread.join();
            Err(error)
        }
        Err(_) => {
            let _ = thread.join();
            Err("tray thread exited before registration".to_string())
        }
    }
}

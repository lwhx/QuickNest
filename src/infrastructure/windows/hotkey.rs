use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx,
    PostQuitMessage, PostThreadMessageW, QS_ALLINPUT, TranslateMessage, WM_HOTKEY,
};

const WAKEUP: u32 = 0x0400 + 0x484B;
const HOTKEY_ID: i32 = 1;

/// Default baseline hotkey: Ctrl + Alt + Space with MOD_NOREPEAT.
pub const DEFAULT_MODIFIERS: u32 = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
pub const DEFAULT_VK: u32 = 0x20; // VK_SPACE

/// Error codes with specific meanings for the hotkey flow.
pub const ERROR_HOTKEY_ALREADY_REGISTERED: u32 = 1409;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyError {
    /// The combination is registered by another process (conflict detection).
    AlreadyRegistered,
    /// A Win32 call failed; the payload is GetLastError() at the failure point.
    Failed(u32),
}

/// Events delivered from the hotkey thread; one per accepted WM_HOTKEY.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyEvent {
    pub sequence: u64,
}

enum Control {
    Shutdown,
}

/// Thread-bound owner of a registered global hotkey with its own message loop.
///
/// Drop signals shutdown and joins the thread; `UnregisterHotKey` runs on the
/// registering thread as Win32 requires.
pub struct HotkeyGuard {
    name: String,
    events: Option<Receiver<HotkeyEvent>>,
    control: Option<Sender<Control>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    thread_id: u32,
    _thread_bound: PhantomData<Rc<()>>,
}

pub struct HotkeyRegistration {
    pub thread_id: u32,
}

impl HotkeyGuard {
    pub fn events(&self) -> &Receiver<HotkeyEvent> {
        self.events
            .as_ref()
            .expect("events channel lives until drop")
    }

    /// Hands the event receiver out so another thread (for example a state
    /// machine forwarder) can consume events while the guard stays here.
    pub fn take_events(&mut self) -> Option<Receiver<HotkeyEvent>> {
        self.events.take()
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for HotkeyGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(control) = self.control.take() {
            let _ = control.send(Control::Shutdown);
        }
        // The pump is blocked in GetMessageW; a thread wakeup is required for it
        // to observe the shutdown control. Without this, Drop would hang forever.
        // SAFETY: The queue exists (created before this id was published).
        unsafe {
            PostThreadMessageW(self.thread_id, WAKEUP, 0, 0);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn pump(
    ready: Sender<Result<HotkeyRegistration, HotkeyError>>,
    events: Sender<HotkeyEvent>,
    control: Receiver<Control>,
    stop: Arc<AtomicBool>,
    modifiers: u32,
    vk: u32,
) -> Result<(), u32> {
    // SAFETY: Registering a thread-local hotkey (NULL hwnd) so WM_HOTKEY is posted
    // to this thread's message queue; MOD_NOREPEAT is baseline-preferred.
    let registered = unsafe { RegisterHotKey(std::ptr::null_mut(), HOTKEY_ID, modifiers, vk) };
    if registered == 0 {
        let error = unsafe { GetLastError() };
        let mapped = if error == ERROR_HOTKEY_ALREADY_REGISTERED {
            HotkeyError::AlreadyRegistered
        } else {
            HotkeyError::Failed(error)
        };
        let _ = ready.send(Err(mapped));
        return Ok(());
    }
    // SAFETY: Zero timeout forces message-queue creation before the id is published,
    // closing the PostThreadMessage race.
    unsafe {
        MsgWaitForMultipleObjectsEx(0, std::ptr::null(), 0, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
    // SAFETY: Identity query without pointer arguments.
    let thread_id = unsafe { GetCurrentThreadId() };
    if ready.send(Ok(HotkeyRegistration { thread_id })).is_err() {
        // SAFETY: Owner thread must unregister before exiting on this path.
        unsafe {
            UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
        }
        return Ok(());
    }
    let mut message = MSG::default();
    let mut sequence: u64 = 0;
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        // SAFETY: Valid MSG storage; NULL hwnd selects thread messages.
        let result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
        if result == 0 || result == -1 {
            break;
        }
        // SAFETY: Both calls operate on the message just read on this thread.
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if message.message == WM_HOTKEY {
            sequence += 1;
            if events.send(HotkeyEvent { sequence }).is_err() {
                break;
            }
        }
        match control.try_recv() {
            Ok(Control::Shutdown) | Err(TryRecvError::Disconnected) => {
                // SAFETY: Ends this thread's message loop; the outer loop then
                // breaks and the final UnregisterHotKey after the loop releases
                // the hotkey on this, the registering thread.
                unsafe {
                    PostQuitMessage(0);
                }
            }
            Err(TryRecvError::Empty) => {}
        }
    }
    // SAFETY: Every exit path (stop flag, WM_QUIT, GetMessageW error, closed
    // events channel) lands here, releasing the hotkey on the registering thread.
    unsafe {
        UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
    }
    Ok(())
}

fn thread_main(
    ready: Sender<Result<HotkeyRegistration, HotkeyError>>,
    events: Sender<HotkeyEvent>,
    control: Receiver<Control>,
    stop: Arc<AtomicBool>,
    modifiers: u32,
    vk: u32,
) {
    if let Err(error) = pump(ready, events, control, stop, modifiers, vk) {
        eprintln!("Hotkey thread exited: {error}");
    }
}

/// Registers the given global hotkey on a dedicated message-loop thread.
pub fn register(name: &str, modifiers: u32, vk: u32) -> Result<HotkeyGuard, HotkeyError> {
    let (ready_tx, ready_rx) = channel();
    let (events_tx, events_rx) = channel();
    let (control_tx, control_rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::spawn(move || {
        thread_main(ready_tx, events_tx, control_rx, thread_stop, modifiers, vk);
    });
    match ready_rx.recv() {
        Ok(Ok(registration)) => Ok(HotkeyGuard {
            name: name.to_string(),
            events: Some(events_rx),
            control: Some(control_tx),
            stop,
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
            Err(HotkeyError::Failed(0))
        }
    }
}

/// Sends a control wakeup; used by tests that need the loop to observe stop.
pub fn wakeup(thread_id: u32) -> Result<(), u32> {
    // SAFETY: The target thread owns a message queue (created before publication);
    // a failed wakeup only affects timing, never correctness.
    unsafe {
        if PostThreadMessageW(thread_id, WAKEUP, 0, 0) == 0 {
            return Err(GetLastError());
        }
    }
    Ok(())
}

pub fn error_name(error: HotkeyError) -> String {
    match error {
        HotkeyError::AlreadyRegistered => "ERROR_HOTKEY_ALREADY_REGISTERED".to_string(),
        HotkeyError::Failed(code) => format!("0x{:08X}", code),
    }
}

/// Posts a synthetic Ctrl+Alt+Space through SendInput so the registered hotkey
/// fires. Callers must run on an interactive desktop.
///
/// # Safety
///
/// The injected keystrokes target the interactive desktop; only call this from a
/// process with access to that desktop.
pub unsafe fn inject_default_hotkey() -> Result<(), u32> {
    // SendInput is declared in windows-sys Input module; reuse the keybd_event
    // pathway through SendInput with KEYBDINPUT structures.
    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(non_snake_case)]
    #[allow(clippy::upper_case_acronyms)]
    struct KEYBDINPUT {
        wVk: u16,
        wScan: u16,
        dwFlags: u32,
        time: u32,
        dwExtraInfo: usize,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(non_snake_case)]
    #[allow(clippy::upper_case_acronyms)]
    struct MOUSEINPUT {
        dx: i32,
        dy: i32,
        mouseData: u32,
        dwFlags: u32,
        time: u32,
        dwExtraInfo: usize,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(non_snake_case)]
    #[allow(clippy::upper_case_acronyms)]
    struct HARDWAREINPUT {
        uMsg: u32,
        wParamL: u16,
        wParamH: u16,
    }
    #[repr(C)]
    union INPUT_UNION {
        ki: KEYBDINPUT,
        mi: MOUSEINPUT,
        hi: HARDWAREINPUT,
    }
    #[repr(C)]
    #[allow(non_snake_case)]
    #[allow(clippy::upper_case_acronyms)]
    struct INPUT {
        r#type: u32,
        union: INPUT_UNION,
    }
    const INPUT_KEYBOARD: u32 = 1;
    const KEYEVENTF_KEYUP: u32 = 2;
    const VK_CONTROL: u16 = 0x11;
    const VK_MENU: u16 = 0x12;
    const VK_SPACE: u16 = 0x20;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SendInput(cinputs: u32, pinputs: *const INPUT, cbsize: i32) -> u32;
    }

    let make = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        union: INPUT_UNION {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    // Order matters: modifiers down, VK down, VK up, modifiers up.
    let inputs = [
        make(VK_CONTROL, false),
        make(VK_MENU, false),
        make(VK_SPACE, false),
        make(VK_SPACE, true),
        make(VK_MENU, true),
        make(VK_CONTROL, true),
    ];
    // SAFETY: Valid input array with the correct structure size; user32 processes
    // the synthesized keystrokes, which the registered hotkey will observe.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent != inputs.len() as u32 {
        return Err(unsafe { GetLastError() });
    }
    Ok(())
}

/// Parses a settings hotkey string such as `Ctrl+Alt+Space` into
/// `(modifiers, virtual-key)`, always adding `MOD_NOREPEAT` (baseline 11.1).
/// Returns `None` for unparsable input so the caller can surface a clear
/// error instead of silently falling back.
pub fn parse_hotkey(spec: &str) -> Option<(u32, u32)> {
    let mut modifiers = 0_u32;
    let mut virtual_key = None;
    for part in spec.split('+') {
        let token = part.trim().to_ascii_lowercase();
        match token.as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "alt" => modifiers |= MOD_ALT,
            "shift" => modifiers |= MOD_SHIFT,
            "win" | "super" | "meta" => modifiers |= MOD_WIN,
            "" => return None,
            key => {
                if virtual_key.is_some() {
                    return None;
                }
                virtual_key = Some(key_to_virtual_key(key)?);
            }
        }
    }
    Some((modifiers | MOD_NOREPEAT, virtual_key?))
}

fn key_to_virtual_key(key: &str) -> Option<u32> {
    if key.len() == 1 {
        let character = key.chars().next()?;
        if character.is_ascii_alphabetic() {
            return Some(character.to_ascii_uppercase() as u32);
        }
        if character.is_ascii_digit() {
            return Some(character as u32);
        }
        return None;
    }
    match key {
        "space" => Some(0x20),
        "tab" => Some(0x09),
        "enter" | "return" => Some(0x0D),
        "esc" | "escape" => Some(0x1B),
        "home" => Some(0x24),
        "end" => Some(0x23),
        "insert" => Some(0x2D),
        "delete" => Some(0x2E),
        "left" => Some(0x25),
        "right" => Some(0x27),
        "up" => Some(0x26),
        "down" => Some(0x28),
        function => {
            let digits = function.strip_prefix('f')?;
            let number: u32 = digits.parse().ok()?;
            (1..=24).contains(&number).then_some(0x6F + number)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal lifecycle proof: register, drop, register again, drop.
    /// Exercises Drop's shutdown path without Slint or SendInput.
    #[test]
    fn drop_unregisters_and_allows_re_registration() {
        let guard = register("unit", DEFAULT_MODIFIERS, DEFAULT_VK)
            .expect("initial registration must succeed");
        drop(guard);
        let guard = register("unit", DEFAULT_MODIFIERS, DEFAULT_VK)
            .expect("re-registration after drop must succeed");
        drop(guard);
    }

    #[test]
    fn parses_settings_hotkey_strings() {
        assert_eq!(
            parse_hotkey("Ctrl+Alt+Space"),
            Some((MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x20))
        );
        assert_eq!(
            parse_hotkey(" control + alt + F9 "),
            Some((MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x78))
        );
        assert_eq!(
            parse_hotkey("Shift+Ctrl+A"),
            Some((MOD_SHIFT | MOD_CONTROL | MOD_NOREPEAT, 0x41))
        );
        assert_eq!(parse_hotkey("Win+1"), Some((MOD_WIN | MOD_NOREPEAT, 0x31)));
        // Invalid specifications are rejected, never silently defaulted.
        assert_eq!(parse_hotkey(""), None);
        assert_eq!(parse_hotkey("Ctrl+Alt"), None);
        assert_eq!(parse_hotkey("Ctrl+Alt+Space+F1"), None);
        assert_eq!(parse_hotkey("Ctrl+Alt+Unknown"), None);
        assert_eq!(parse_hotkey("Ctrl++Space"), None);
    }
}

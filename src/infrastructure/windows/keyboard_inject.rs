//! SendInput-based key injection for validation scenarios. Production code
//! never synthesizes input; this module exists so automated tests can drive
//! real keyboard paths (menus, hotkeys) instead of calling handlers directly.

use windows_sys::Win32::Foundation::GetLastError;

const INPUT_KEYBOARD: u32 = 1;
const KEYEVENTF_KEYUP: u32 = 2;

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
#[derive(Clone, Copy)]
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

#[link(name = "user32")]
unsafe extern "system" {
    fn SendInput(cinputs: u32, pinputs: *const INPUT, cbsize: i32) -> u32;
}

/// Presses and releases one virtual key through SendInput.
///
/// # Safety
///
/// The injected keystrokes target the interactive desktop; only call this from
/// a process with access to that desktop.
pub unsafe fn press_key(vk: u16) -> Result<(), u32> {
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
    let inputs = [make(vk, false), make(vk, true)];
    // SAFETY: Valid input array with the correct structure size.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent != inputs.len() as u32 {
        // SAFETY: Read immediately after the failing call.
        return Err(unsafe { GetLastError() });
    }
    Ok(())
}

const INPUT_MOUSE: u32 = 0;
const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
const MOUSEEVENTF_MOVE: u32 = 0x0001;
const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;

/// Moves the cursor to the point through a SendInput absolute move (normalized
/// over the virtual desktop) and injects one real left click. SendInput is the
/// only injection pathway verified under restricted tokens (P0-05); plain
/// SetCursorPos is refused there with ERROR_ALREADY_EXISTS.
///
/// # Safety
///
/// The injected click targets the interactive desktop; only call this from a
/// process with access to that desktop.
pub unsafe fn click_at(x: i32, y: i32) -> Result<(), u32> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };
    // SAFETY: Screen-metric identity queries without pointer arguments.
    let (virtual_x, virtual_y, virtual_width, virtual_height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    if virtual_width <= 0 || virtual_height <= 0 {
        return Err(0);
    }
    let normalized_x = ((x - virtual_x) * 65535) / (virtual_width - 1);
    let normalized_y = ((y - virtual_y) * 65535) / (virtual_height - 1);
    let move_input = INPUT {
        r#type: INPUT_MOUSE,
        union: INPUT_UNION {
            mi: MOUSEINPUT {
                dx: normalized_x,
                dy: normalized_y,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let make = |down: bool| INPUT {
        r#type: INPUT_MOUSE,
        union: INPUT_UNION {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: if down {
                    MOUSEEVENTF_LEFTDOWN
                } else {
                    MOUSEEVENTF_LEFTUP
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [move_input, make(true), make(false)];
    // SAFETY: Valid input array with the correct structure size.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent != inputs.len() as u32 {
        // SAFETY: Read immediately after the failing call.
        return Err(unsafe { GetLastError() });
    }
    Ok(())
}

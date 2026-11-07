use std::error::Error;
use std::fmt;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowThreadProcessId, IsWindow, IsWindowVisible,
};

#[derive(Debug, Clone, Copy)]
pub struct WindowObservation {
    // Observations are diagnostic values, not a handle for later window operations.
    pub hwnd: isize,
    pub is_window: bool,
    pub visible: bool,
    pub pid: u32,
    pub tid: u32,
}

#[derive(Debug)]
pub enum WindowHandleError {
    Unavailable(raw_window_handle::HandleError),
    NotWin32,
    InvalidWindow,
    WrongOwnerThread,
}

impl fmt::Display for WindowHandleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(error) => write!(f, "Native window not ready: {error}"),
            Self::NotWin32 => f.write_str("Slint did not provide a Win32 window"),
            Self::InvalidWindow => f.write_str("Slint HWND is no longer valid"),
            Self::WrongOwnerThread => f.write_str("HWND is not owned by the current UI thread"),
        }
    }
}

impl Error for WindowHandleError {}

/// Acquire a fresh provider and inspect it while its native-window owner is alive.
/// Call on the Slint UI thread after the window has actually been created.
pub fn inspect_window(window: &slint::Window) -> Result<WindowObservation, WindowHandleError> {
    let provider = window.window_handle();
    let borrowed =
        HasWindowHandle::window_handle(&provider).map_err(WindowHandleError::Unavailable)?;
    let RawWindowHandle::Win32(raw) = borrowed.as_raw() else {
        return Err(WindowHandleError::NotWin32);
    };
    let observation = observe_value(raw.hwnd.get());
    if !observation.is_window {
        return Err(WindowHandleError::InvalidWindow);
    }
    // SAFETY: These APIs read the identity of the current process/thread and take no pointers.
    let (pid, tid) = unsafe { (GetCurrentProcessId(), GetCurrentThreadId()) };
    if observation.pid != pid || observation.tid != tid {
        return Err(WindowHandleError::WrongOwnerThread);
    }
    Ok(observation)
}

/// A point-in-time probe for tests only. The numeric value may be stale or reused;
/// this function does not dereference it or treat it as authority to operate a window.
pub fn observe_value(value: isize) -> WindowObservation {
    let hwnd = value as HWND;
    let mut observation = WindowObservation {
        hwnd: value,
        is_window: false,
        visible: false,
        pid: 0,
        tid: 0,
    };
    if value == 0 {
        return observation;
    }
    // SAFETY: IsWindow/IsWindowVisible tolerate stale HWND values. The PID pointer
    // is valid writable stack memory. These are observations only; no window is mutated.
    unsafe {
        observation.is_window = IsWindow(hwnd) != 0;
        if observation.is_window {
            observation.tid = GetWindowThreadProcessId(hwnd, &mut observation.pid);
            observation.visible = IsWindowVisible(hwnd) != 0;
        }
    }
    observation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_handle_is_rejected_as_an_observation() {
        let observed = observe_value(0);
        assert!(!observed.is_window);
        assert!(!observed.visible);
        assert_eq!((observed.pid, observed.tid), (0, 0));
    }
}

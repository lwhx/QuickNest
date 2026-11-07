//! Foreground-window helpers for the focus state machine (baseline 10.3/10.4).

use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, FindWindowW, GA_ROOTOWNER, GetAncestor, GetForegroundWindow,
    GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, SW_SHOW, SetForegroundWindow,
    ShowWindow,
};
use windows_sys::core::BOOL;

/// The foreground window, or None when no window has the foreground.
pub fn foreground_window() -> Option<HWND> {
    // SAFETY: Identity query without pointer arguments.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() { None } else { Some(hwnd) }
}

/// Whether the given window belongs to the given process.
///
/// # Safety
///
/// The caller must pass a window handle that is either valid or null.
pub unsafe fn window_process_id(hwnd: HWND) -> u32 {
    let mut pid = 0_u32;
    // SAFETY: Valid out pointer; the return value (thread id) is ignored.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    pid
}

/// Walks the owner chain to the top-level owner window. Owned popups (dialogs,
/// menus) map to their owner so blur checks treat them as "ours".
///
/// # Safety
///
/// The caller must pass a window handle that is either valid or null.
pub unsafe fn root_owner(hwnd: HWND) -> HWND {
    // SAFETY: GA_ROOTOWNER on a valid window; returns the window itself when
    // it has no owner.
    unsafe { GetAncestor(hwnd, GA_ROOTOWNER) }
}

/// Whether the current foreground belongs to us: it is our window, one of
/// its owned windows, or any other window of our process (dialogs, the tray
/// menu host). Baseline 10.3 step 7 "QuickNest 或其拥有的弹窗", with
/// GetWindowThreadProcessId from baseline 10.4 as the process-level source.
///
/// # Safety
///
/// The caller must pass a window handle that is either valid or null.
pub unsafe fn foreground_belongs_to(our_hwnd: HWND, our_pid: u32) -> bool {
    let Some(foreground) = foreground_window() else {
        return false;
    };
    // SAFETY: Handles are valid or null per the safety contract.
    unsafe {
        root_owner(foreground) == root_owner(our_hwnd) || window_process_id(foreground) == our_pid
    }
}

/// The center point of the window's screen rect.
///
/// # Safety
///
/// The caller must pass a window handle that is either valid or null.
pub unsafe fn window_center(hwnd: HWND) -> Option<(i32, i32)> {
    // SAFETY: Delegates to the rect query with the same handle contract.
    let (left, top, right, bottom) = unsafe { window_rect(hwnd) }?;
    Some(((left + right) / 2, (top + bottom) / 2))
}

/// The window's screen rect as (left, top, right, bottom).
///
/// # Safety
///
/// The caller must pass a window handle that is either valid or null.
pub unsafe fn window_rect(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: Valid out RECT for a live window handle.
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return None;
    }
    Some((rect.left, rect.top, rect.right, rect.bottom))
}

/// The visible popup-menu window (class #32768) if one is on screen.
///
/// # Safety
///
/// Reads window identity only; safe to call from any thread.
pub unsafe fn popup_menu_window() -> Option<HWND> {
    const MENU_CLASS: &[u16] = &[
        b'#' as u16,
        b'3' as u16,
        b'2' as u16,
        b'7' as u16,
        b'6' as u16,
        b'8' as u16,
        0,
    ];
    // SAFETY: Null-terminated class name; a null result means no menu.
    let hwnd = unsafe { FindWindowW(MENU_CLASS.as_ptr(), std::ptr::null()) };
    if hwnd.is_null() { None } else { Some(hwnd) }
}

/// Brings a window to the foreground through the standard path. Returns
/// whether the switch was accepted; callers verify with foreground checks.
///
/// # Safety
///
/// The caller must pass a window handle that is either valid or null.
pub unsafe fn activate_window(hwnd: HWND) -> bool {
    // SAFETY: Standard window operations on a live window of this session.
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd) != 0
    }
}

/// All visible top-level windows of the given process, newest last.
pub fn visible_windows_of_process(pid: u32) -> Vec<HWND> {
    let mut found: Vec<HWND> = Vec::new();
    let mut context = EnumContext {
        pid,
        out: &mut found,
    };
    // SAFETY: The callback only reads window identity and appends to the
    // context slice; EnumWindows invokes it on this thread only.
    unsafe {
        EnumWindows(Some(enum_proc), &mut context as *mut EnumContext as LPARAM);
    }
    found
}

struct EnumContext<'a> {
    pid: u32,
    out: &'a mut Vec<HWND>,
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: The context pointer was created from a live &mut on this thread.
    let context = unsafe { &mut *(lparam as *mut EnumContext<'_>) };
    // SAFETY: EnumWindows hands out live window handles.
    if unsafe { window_process_id(hwnd) } == context.pid {
        // SAFETY: Observation only; a stale handle simply reports false.
        let visible = unsafe { IsWindowVisible(hwnd) != 0 };
        if visible {
            context.out.push(hwnd);
        }
    }
    1
}

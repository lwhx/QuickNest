//! P2-15 (baseline 16.4): the Windows side of the placement rules.
//!
//! Enumerates the connected displays with their work areas and effective
//! DPI, reads and writes the window rectangle and applies the correction
//! from [`crate::placement`]. All rectangles are physical pixels.

use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT, TRUE};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFOEXW,
    MonitorFromWindow,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, HWND_TOP, MONITORINFOF_PRIMARY, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowPos,
};
use windows_sys::core::BOOL;

use crate::placement::{MonitorArea, Rect, ensure_visible, nearest_monitor};

fn to_rect(rect: &RECT) -> Rect {
    Rect::new(rect.left, rect.top, rect.right, rect.bottom)
}

unsafe extern "system" fn collect(
    monitor: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // SAFETY: the callback only appends to the list the LPARAM points to.
    unsafe {
        let list = &mut *(data as *mut Vec<MonitorArea>);
        // MONITORINFOEXW starts with MONITORINFO, so the same pointer serves
        // both views; the larger cbSize makes Windows fill the device name.
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info.monitorInfo) == 0 {
            return TRUE;
        }
        let mut dpi_x: u32 = 0;
        let mut dpi_y: u32 = 0;
        // The effective DPI is the one the compositor uses for this display.
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        let device: String = info
            .szDevice
            .iter()
            .take_while(|unit| **unit != 0)
            .map(|unit| char::from_u32(u32::from(*unit)).unwrap_or('?'))
            .collect();
        list.push(MonitorArea {
            bounds: to_rect(&info.monitorInfo.rcMonitor),
            work: to_rect(&info.monitorInfo.rcWork),
            dpi: if dpi_x == 0 { 96 } else { dpi_x },
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            device,
        });
    }
    TRUE
}

/// Every connected display with its work area and effective DPI.
pub fn monitor_areas() -> Vec<MonitorArea> {
    let mut list: Vec<MonitorArea> = Vec::new();
    // SAFETY: the callback only appends to the list the LPARAM points to, and
    // the list outlives the enumeration call.
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect),
            &mut list as *mut Vec<MonitorArea> as LPARAM,
        );
    }
    list
}

/// The rectangle of the window in physical pixels.
pub fn window_rect(hwnd: isize) -> Option<Rect> {
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    // SAFETY: the handle belongs to this process; the out pointer is valid.
    if unsafe { GetWindowRect(hwnd as HWND, &mut rect) } == 0 {
        return None;
    }
    Some(to_rect(&rect))
}

/// The effective DPI of the display that owns the window.
pub fn dpi_for_window(hwnd: isize) -> u32 {
    // SAFETY: the handle belongs to this process.
    let dpi = unsafe { GetDpiForWindow(hwnd as HWND) };
    if dpi == 0 { 96 } else { dpi }
}

/// Moves the window to a physical position.
pub fn set_window_position(hwnd: isize, x: i32, y: i32) -> bool {
    // SAFETY: the handle belongs to this process; only the position changes.
    unsafe {
        SetWindowPos(
            hwnd as HWND,
            HWND_TOP,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        ) != 0
    }
}

/// Corrects a window that would sit outside every connected display.
///
/// Returns the corrected rectangle when the window had to be moved.
pub fn correct_window_position(hwnd: isize) -> Option<Rect> {
    let window = window_rect(hwnd)?;
    let monitors = monitor_areas();
    if monitors.is_empty() {
        return None;
    }
    let corrected = ensure_visible(window, &monitors)?;
    let _ = set_window_position(hwnd, corrected.left, corrected.top);
    Some(corrected)
}

/// The work area of the display the window currently sits on, falling back to
/// the primary display when the window has no display yet.
pub fn current_work_area(hwnd: isize) -> Option<Rect> {
    let monitors = monitor_areas();
    if monitors.is_empty() {
        return None;
    }
    if let Some(window) = window_rect(hwnd)
        && let Some(index) = nearest_monitor(window, &monitors)
    {
        return Some(monitors[index].work);
    }
    monitors
        .iter()
        .find(|area| area.primary)
        .or_else(|| monitors.first())
        .map(|area| area.work)
}

/// The DPI of every connected display, used by the acceptance script to
/// document the scale factors of the machine under test.
pub fn monitor_dpis() -> Vec<(u32, bool)> {
    monitor_areas()
        .iter()
        .map(|area| (area.dpi, area.primary))
        .collect()
}

/// The display the window belongs to, as Windows reports it.
pub fn monitor_for_window(hwnd: isize) -> Option<HMONITOR> {
    // SAFETY: the handle belongs to this process.
    let handle = unsafe { MonitorFromWindow(hwnd as HWND, MONITOR_DEFAULTTONEAREST) };
    if handle.is_null() { None } else { Some(handle) }
}

/// P3-04 (baseline 22 鼠标热区): the bounds of the primary display.
pub fn primary_bounds() -> Option<crate::placement::Rect> {
    let areas = monitor_areas();
    areas
        .iter()
        .find(|area| area.primary)
        .or_else(|| areas.first())
        .map(|area| area.bounds)
}

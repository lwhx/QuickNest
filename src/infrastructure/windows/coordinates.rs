use std::error::Error;
use std::io;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows_sys::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
    GetWindowDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetClientRect, GetCursorPos};

use super::window_handle::inspect_window;

#[derive(Debug, Clone, Copy)]
pub struct CoordinateSnapshot {
    pub screen: (i32, i32),
    pub client: (i32, i32),
    pub logical: (f32, f32),
    pub client_size: (i32, i32),
    pub slint_size: (u32, u32),
    pub client_origin: (i32, i32),
    pub dpi: u32,
    pub scale: f32,
    pub per_monitor_v2: bool,
    pub inside: bool,
    pub sizes_match: bool,
    pub scale_matches_dpi: bool,
}

pub fn logical_point(point: (i32, i32), scale: f32) -> Result<(f32, f32), &'static str> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err("Scale factor must be finite and positive");
    }
    let logical = slint::PhysicalPosition::new(point.0, point.1).to_logical(scale);
    Ok((logical.x, logical.y))
}

pub fn client_contains(point: (i32, i32), size: (i32, i32)) -> bool {
    size.0 > 0 && size.1 > 0 && point.0 >= 0 && point.1 >= 0 && point.0 < size.0 && point.1 < size.1
}

pub fn cursor_screen() -> Result<(i32, i32), io::Error> {
    let mut point = POINT::default();
    // SAFETY: Output is valid writable stack memory. The process is PMv2 aware.
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((point.x, point.y))
}

/// Snapshot taken on the owning UI thread when a queued screen point is consumed.
/// It is deliberately not a durable HWND or an atomic cross-monitor observation.
pub fn from_screen(
    window: &slint::Window,
    screen: (i32, i32),
) -> Result<CoordinateSnapshot, Box<dyn Error>> {
    let observed = inspect_window(window)?;
    let provider = window.window_handle();
    let borrowed = HasWindowHandle::window_handle(&provider)?;
    let RawWindowHandle::Win32(raw) = borrowed.as_raw() else {
        return Err("Not a Win32 window".into());
    };
    if raw.hwnd.get() != observed.hwnd {
        return Err("Window identity changed while acquiring coordinates".into());
    }
    let hwnd = raw.hwnd.get() as HWND;
    let mut point = POINT {
        x: screen.0,
        y: screen.1,
    };
    let mut rect = RECT::default();
    let mut origin = POINT::default();
    // SAFETY: Fresh provider keeps the same-thread native window alive; all output
    // pointers are valid. ScreenToClient handles nonclient offsets and signed origins.
    let (dpi, context, pmv2) = unsafe {
        if ScreenToClient(hwnd, &mut point) == 0
            || GetClientRect(hwnd, &mut rect) == 0
            || ClientToScreen(hwnd, &mut origin) == 0
        {
            return Err(io::Error::last_os_error().into());
        }
        let context = GetWindowDpiAwarenessContext(hwnd);
        let v2 =
            AreDpiAwarenessContextsEqual(context, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) != 0;
        (GetDpiForWindow(hwnd), context, v2)
    };
    if dpi == 0 || context.is_null() {
        return Err("Window DPI/context unavailable".into());
    }
    let scale = window.scale_factor();
    let logical = logical_point((point.x, point.y), scale)?;
    let size = (rect.right - rect.left, rect.bottom - rect.top);
    let slint_size = window.size();
    Ok(CoordinateSnapshot {
        screen,
        client: (point.x, point.y),
        logical,
        client_size: size,
        slint_size: (slint_size.width, slint_size.height),
        client_origin: (origin.x, origin.y),
        dpi,
        scale,
        per_monitor_v2: pmv2,
        inside: client_contains((point.x, point.y), size),
        sizes_match: size.0 >= 0
            && size.1 >= 0
            && size.0 as u32 == slint_size.width
            && size.1 as u32 == slint_size.height,
        scale_matches_dpi: (scale - dpi as f32 / 96.0).abs() <= 0.001,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_scales_preserve_fractional_and_negative_coordinates() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let logical = logical_point((-101, 203), scale).unwrap();
            assert!((logical.0 + 101.0 / scale).abs() < 0.0001);
            assert!((logical.1 - 203.0 / scale).abs() < 0.0001);
        }
        assert!((logical_point((101, 0), 1.25).unwrap().0 - 80.8).abs() < 0.0001);
    }

    #[test]
    fn client_bounds_are_half_open_and_never_clamp() {
        assert!(client_contains((0, 0), (100, 80)));
        assert!(client_contains((99, 79), (100, 80)));
        for point in [(-1, 0), (0, -1), (100, 20), (20, 80)] {
            assert!(!client_contains(point, (100, 80)));
        }
        assert!(!client_contains((0, 0), (0, 80)));
        assert!(!client_contains((0, 0), (100, -1)));
    }

    #[test]
    fn invalid_factors_are_not_used_for_conversion() {
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(logical_point((1, 1), scale).is_err());
        }
    }
}

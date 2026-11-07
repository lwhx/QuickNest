//! P2-13 (baseline 16.2): window opacity and rounded corners.
//! P2-14 (baseline 16.3): DWM system backdrop material.
//!
//! Opacity uses a layered window (`WS_EX_LAYERED` + `SetLayeredWindowAttributes`)
//! with per-pixel-independent uniform alpha, so the Slint software renderer is
//! untouched. Corners use the DWM window-corner preference: Windows 11 rounds
//! the frame, older systems fail the call and degrade to square corners
//! without affecting functionality. The backdrop uses `DWMWA_SYSTEMBACKDROP_TYPE`,
//! whose success doubles as the Windows 11 22H2 capability probe.

use std::ffi::c_void;

use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_MAINWINDOW, DWMSBT_NONE,
    DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_DONOTROUND, DWMWCP_ROUND, DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, LWA_ALPHA, SetLayeredWindowAttributes, SetWindowLongPtrW,
    WS_EX_LAYERED,
};
use windows::core::w;

/// P2-14 (baseline 16.3): DWM system backdrop material (baseline 16.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backdrop {
    None,
    Mica,
    Acrylic,
}

/// Applies per-window opacity (50%–100%) and the rounded-corner preference to
/// the main window. Idempotent: every call refreshes both values. Failures
/// (for example Windows 10 without the corner attribute) degrade silently to
/// the previous look; functionality is never affected.
pub fn apply_window_effects(hwnd: isize, opacity: f64, rounded: bool) {
    let opacity = opacity.clamp(0.5, 1.0);
    let hwnd = HWND(hwnd as _);
    // SAFETY: the handle belongs to this process's main window; the style and
    // layered attributes only change this window's appearance.
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // P2-14: layered alpha must be off while a DWM system backdrop is
        // composed (layered windows never show system materials); 100%
        // opacity therefore drops the style instead of keeping alpha 255.
        if opacity < 1.0 {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as isize);
            let alpha = ((opacity * 255.0).round() as i32).clamp(0, 255) as u8;
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
        } else {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style & !WS_EX_LAYERED.0 as isize);
        }
        let preference = if rounded {
            DWMWCP_ROUND
        } else {
            DWMWCP_DONOTROUND
        };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const DWM_WINDOW_CORNER_PREFERENCE as *const c_void,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
    }
}

/// P2-14 (baseline 16.3): applies the DWM system backdrop (Mica / Acrylic /
/// none). The `DwmSetWindowAttribute` success doubles as the Windows 11 22H2
/// capability probe; older systems fail it and the caller falls back to the
/// plain solid color. Returns the final HRESULT of the backdrop set
/// (`Ok(())` = applied).
///
/// The frame is kept out of the client area (margins zero): extending it with
/// `-1` under a layered window (`WS_EX_LAYERED`, i.e. opacity < 100%, the
/// default) makes DWM compose the backdrop sheet behind the client and the
/// layered alpha then blends the whole window against the desktop — the panel
/// turns semi-transparent and typed input becomes unreliable (observed
/// 2026-11-05, fixed the same day). The opaque software-rendered client hides
/// the material behind it either way, so `-1` has no benefit here; the
/// material stays visible in DWM-managed regions (rounded corners).
/// P2-14 (baseline 16.3): applies the DWM system backdrop (Mica / Acrylic /
/// none). The `DwmSetWindowAttribute` success doubles as the Windows 11 22H2
/// capability probe; older systems fail it and the caller falls back to the
/// plain solid color. Returns the final HRESULT of the backdrop set
/// (`Ok(())` = applied).
///
/// The frame is deliberately *not* extended into the client area. With
/// `DwmExtendFrameIntoClientArea(-1)` DWM treats the client area as part of
/// the frame, and the layered alpha that carries the P2-13 opacity stops
/// applying to it: the panel turns fully opaque (measured 2026-11-05: the
/// 95% opacity pixel read 246 instead of the expected 234) and the opacity
/// slider silently stops working. The software renderer also paints opaque
/// client pixels, so the material could not show through them anyway; it
/// stays visible in the DWM-managed frame areas (rounded corners).
pub fn apply_backdrop(hwnd: isize, backdrop: Backdrop) -> Result<(), i32> {
    let backdrop_type = match backdrop {
        Backdrop::None => DWMSBT_NONE,
        Backdrop::Mica => DWMSBT_MAINWINDOW,
        Backdrop::Acrylic => DWMSBT_TRANSIENTWINDOW,
    };
    let hwnd = HWND(hwnd as _);
    // SAFETY: the handle belongs to this process's main window; the attribute
    // only changes this window's DWM backdrop appearance.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop_type as *const DWM_SYSTEMBACKDROP_TYPE as *const c_void,
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
        .map_err(|error| error.code().0)?;
    }
    Ok(())
}

/// Reads back the DWM-applied system backdrop type for verification:
/// `Ok(value)` carries the raw `DWMSBT` (1=none, 2=Mica, 3=Acrylic); `Err`
/// means the system did not accept or report the attribute.
pub fn read_backdrop(hwnd: isize) -> Result<i32, i32> {
    let hwnd = HWND(hwnd as _);
    let mut value = [0_i32; 1];
    // SAFETY: the out buffer describes one i32 and stays alive for the read.
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            value.as_mut_ptr() as *mut c_void,
            std::mem::size_of::<i32>() as u32,
        )
        .map_err(|error| error.code().0)?;
        Ok(value[0])
    }
}

/// P2-14: undocumented-but-verified `SetWindowCompositionAttribute` accent
/// policy (baseline 16.3 "旧式模糊作为经过验证的兼容增强"). `state` 3 =
/// blur behind (Windows 10/11), 4 = acrylic blur behind (Windows 11),
/// 0 = off. The material composes with translucent windows' uniform alpha.
pub fn set_window_accent(hwnd: isize, state: i32, tint: u32) -> Result<(), i32> {
    #[repr(C)]
    struct AccentPolicy {
        accent_state: i32,
        accent_flags: i32,
        gradient_color: u32,
        animation_id: i32,
    }
    #[repr(C)]
    struct WindowCompositionAttributeData {
        attribute: i32,
        data: *mut c_void,
        size_of_data: isize,
    }
    type SetWindowCompositionAttributeFn =
        unsafe extern "system" fn(HWND, *const WindowCompositionAttributeData) -> i32;
    // The function is undocumented and not statically exported from
    // user32.lib; resolve it dynamically like every other consumer of this
    // compatibility API.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn GetProcAddress(module: isize, name: *const u8) -> isize;
    }
    let module = unsafe { GetModuleHandleW(w!("user32.dll").as_ptr()) };
    if module == 0 {
        return Err(0);
    }
    let address =
        unsafe { GetProcAddress(module, c"SetWindowCompositionAttribute".as_ptr().cast()) };
    if address == 0 {
        return Err(0);
    }
    let policy = AccentPolicy {
        accent_state: state,
        accent_flags: 2,
        gradient_color: tint,
        animation_id: 0,
    };
    let data = WindowCompositionAttributeData {
        attribute: 19, // WCA_ACCENT_POLICY
        data: &policy as *const AccentPolicy as *mut AccentPolicy as *mut c_void,
        size_of_data: std::mem::size_of::<AccentPolicy>() as isize,
    };
    let hwnd = HWND(hwnd as _);
    // SAFETY: the resolved address is the documented user32 entry point; the
    // attribute data is fully initialized and outlives the call.
    let set_attribute: SetWindowCompositionAttributeFn = unsafe { std::mem::transmute(address) };
    let status = unsafe { set_attribute(hwnd, &data) };
    if status != 0 { Ok(()) } else { Err(0) }
}

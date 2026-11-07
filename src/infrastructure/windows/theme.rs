//! P2-12 (baseline 16.1): the Windows app theme preference. `AppsUseLightTheme`
//! is the per-user DWM/immersive-color value that Windows 10/11 set from
//! Settings > Personalization > Colors. Reading it never writes or changes the
//! user's preference.

use std::ffi::c_void;

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::w;

/// Whether the current user prefers dark app windows. Missing/unreadable
/// values fall back to the light default.
pub fn system_uses_dark_theme() -> bool {
    let mut value: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: the buffer/size pair describes a u32 and stays alive for the
    // synchronous registry read; the key is a well-known HKCU subkey.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    status == ERROR_SUCCESS && value == 0
}

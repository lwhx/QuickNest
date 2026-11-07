//! Real icon extraction runs in its own process: the Shell COM calls leave
//! process-wide OLE state that must not leak into the unit-test process.

use quicknest::infrastructure::windows::icons::{ICON_SIZE, extract_icon};
use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

#[test]
fn extracts_a_real_exe_icon_with_alpha() {
    // SAFETY: this test binary owns its process; the apartment is balanced at
    // the end of the test.
    let initialized =
        unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) } >= 0;
    let pixels = extract_icon("C:\\Windows\\System32\\cmd.exe", ICON_SIZE).expect("cmd icon");
    if initialized {
        unsafe { CoUninitialize() };
    }
    assert!(pixels.width > 0 && pixels.height > 0);
    assert_eq!(
        pixels.rgba.len(),
        (pixels.width * pixels.height * 4) as usize,
        "RGBA buffer size"
    );
    assert!(
        pixels.rgba.iter().any(|&byte| byte != 0),
        "icon must not be blank"
    );
    // The alpha channel must carry transparency information (baseline 12.1).
    let alphas: Vec<u8> = pixels.rgba.chunks_exact(4).map(|pixel| pixel[3]).collect();
    assert!(
        alphas.iter().any(|&alpha| alpha > 0),
        "icon has opaque pixels"
    );
    assert!(
        alphas.iter().any(|&alpha| alpha < 255),
        "icon has transparent pixels"
    );
}

#[test]
fn missing_sources_do_not_produce_icons() {
    assert!(extract_icon("", ICON_SIZE).is_none());
}

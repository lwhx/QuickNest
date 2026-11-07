//! P2-09 MSI advertised shortcut detection in its own process: the fixture is
//! a normal Shell link extended with a Darwin data block and the
//! `SLDF_HAS_DARWINID` flag — the on-disk shape Windows Installer uses. OLE is
//! initialized and balanced on the test thread.

use quicknest::infrastructure::windows::shell::detect_advertised_shortcut;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IPersistFile, STGM_READWRITE,
};
use windows::Win32::UI::Shell::{
    EXP_DARWIN_ID_SIG, EXP_DARWIN_LINK, IShellLinkDataList, IShellLinkW, SLDF_HAS_DARWINID,
    ShellLink,
};
use windows::core::{Interface, PCWSTR};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[test]
fn advertised_shortcut_detection() {
    // SAFETY: the test thread owns this STA for the duration; balanced below.
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        // Run the body in a closure so COM teardown still happens when an
        // assertion fails (a failed test must not leak the STA).
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
        if initialized {
            CoUninitialize();
        }
        if let Err(payload) = outcome {
            std::panic::resume_unwind(payload);
        }
    }
}

fn run() {
    let directory = std::env::temp_dir()
        .join(format!("quicknest-msi-{}", std::process::id()))
        .join("fixtures");
    std::fs::create_dir_all(&directory).expect("fixture directory");

    // An ordinary LNK is not an MSI advertised shortcut.
    let normal = directory.join("normal.lnk");
    create_cmd_link(&normal);
    assert!(
        !detect_advertised_shortcut(&normal),
        "a normal LNK must not be detected as MSI advertised"
    );

    // A Darwin block plus SLDF_HAS_DARWINID is the advertised shape: it must
    // be detected without resolving the link target.
    let advertised = directory.join("advertised.lnk");
    create_cmd_link(&advertised);
    mark_advertised(&advertised);
    assert!(
        detect_advertised_shortcut(&advertised),
        "SLDF_HAS_DARWINID must be detected"
    );

    // A missing or unreadable link is simply "not advertised"; callers then
    // keep the generic missing-target path.
    assert!(!detect_advertised_shortcut(&directory.join("missing.lnk")));

    let _ = std::fs::remove_dir_all(&directory);
}

fn create_cmd_link(link_path: &std::path::Path) {
    // SAFETY: the test thread owns this STA.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).expect("shell link");
        let target = wide(r"C:\Windows\System32\cmd.exe");
        link.SetPath(PCWSTR(target.as_ptr())).expect("set path");
        let arguments = wide("/c exit");
        link.SetArguments(PCWSTR(arguments.as_ptr()))
            .expect("set arguments");
        let persist: IPersistFile = link.cast().expect("persist file");
        let units = wide(&link_path.to_string_lossy());
        persist.Save(PCWSTR(units.as_ptr()), true).expect("save");
    }
}

/// Adds the Darwin data block and flag exactly as an MSI advertised shortcut
/// carries them.
fn mark_advertised(link_path: &std::path::Path) {
    // SAFETY: all interfaces stay on the test's STA and the wide buffers
    // outlive each call.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).expect("shell link");
        let persist: IPersistFile = link.cast().expect("persist file");
        let units = wide(&link_path.to_string_lossy());
        persist
            .Load(PCWSTR(units.as_ptr()), STGM_READWRITE)
            .expect("load for update");
        let data_list: IShellLinkDataList = link.cast().expect("data list");

        let mut block = EXP_DARWIN_LINK::default();
        block.dbh.cbSize = std::mem::size_of::<EXP_DARWIN_LINK>() as u32;
        block.dbh.dwSignature = EXP_DARWIN_ID_SIG;
        let mut ansi_id = [0_i8; 260];
        let darwin_id = b"QuickNest.P2-09.Test";
        for (slot, byte) in ansi_id.iter_mut().zip(darwin_id.iter()) {
            *slot = *byte as i8;
        }
        std::ptr::addr_of_mut!(block.szDarwinID).write_unaligned(ansi_id);
        let mut wide_id = [0_u16; 260];
        for (slot, unit) in wide_id
            .iter_mut()
            .zip("QuickNest.P2-09.Test".encode_utf16())
        {
            *slot = unit;
        }
        std::ptr::addr_of_mut!(block.szwDarwinID).write_unaligned(wide_id);
        data_list
            .AddDataBlock(&block as *const _ as *const core::ffi::c_void)
            .expect("add darwin block");
        let flags = data_list.GetFlags().expect("get flags");
        data_list
            .SetFlags(flags | SLDF_HAS_DARWINID.0 as u32)
            .expect("set darwin flag");
        persist.Save(PCWSTR(units.as_ptr()), true).expect("save");
    }
}

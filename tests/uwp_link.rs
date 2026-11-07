//! UWP link detection test in its own process: the fixture shortcut is built
//! through IShellLinkW (SetIDList to an AppsFolder item) and read back with
//! the P2-07 detector; OLE is initialized and balanced on the test thread.

use quicknest::infrastructure::windows::launch::{
    LaunchOutcome, activate_application, launch_item,
};
use quicknest::infrastructure::windows::shell::{detect_uwp_link, resolve_link_now};
use quicknest::library_model::{Item, ItemKind, Launch, WindowState};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IPersistFile,
};
use windows::Win32::UI::Shell::{
    IEnumShellItems, IShellItem, IShellItem2, IShellLinkW, SHCreateItemFromParsingName,
    SHParseDisplayName, ShellLink,
};
use windows::core::{Interface, PCWSTR, PWSTR};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[test]
fn uwp_link_detection() {
    // SAFETY: the test thread owns this STA for the duration; balanced below.
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        // The body runs in a closure so the COM teardown below still happens
        // when an assertion fails (test failure must not leak the STA).
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
        .join(format!("quicknest-uwp-{}", std::process::id()))
        .join("fixtures");
    std::fs::create_dir_all(&directory).expect("fixture directory");
    let normal_link = directory.join("normal.lnk");
    let uwp_link = directory.join("uwp.lnk");

    // 1. An ordinary file shortcut is never detected as UWP.
    create_file_link(&normal_link);
    eprintln!("checkpoint: normal link created");
    assert!(
        detect_uwp_link(&normal_link).is_none(),
        "a normal LNK must not be detected as UWP"
    );
    eprintln!("checkpoint: normal link not UWP");
    assert!(
        resolve_link_now(&normal_link).is_some(),
        "the normal LNK still resolves"
    );
    eprintln!("checkpoint: normal link resolved");

    // 2. Pick a real AppsFolder entry that carries an AppUserModel ID.
    let aumid = first_apps_folder_aumid().expect("an AppsFolder item with an AUMID");
    eprintln!("checkpoint: aumid picked = {aumid}");

    // 3. A UWP shortcut (IDList to the AppsFolder item) is detected and the
    //    AUMID is read from the target item — never guessed from a name.
    create_apps_folder_link(&aumid, &uwp_link);
    eprintln!("checkpoint: uwp link created");
    let detected = detect_uwp_link(&uwp_link).expect("the UWP LNK must be detected");
    eprintln!("checkpoint: uwp detected = {}", detected.aumid);
    assert_eq!(
        detected.aumid, aumid,
        "the AUMID must come from the target item"
    );
    // The original resolve stays usable; its target is empty for UWP links.
    let resolved = resolve_link_now(&uwp_link).expect("resolve");
    assert!(
        resolved
            .path
            .as_ref()
            .is_none_or(|path| path.as_os_str().is_empty()),
        "a UWP LNK must not resolve to a file-system target: {:?}",
        resolved.path
    );

    // P2-08: AUMID activation launches the real packaged app (its window may
    // take a moment, so only the PID is asserted here).
    let calc_aumid = "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App";
    match activate_application(calc_aumid, "") {
        Ok(pid) => assert!(pid > 0, "activation pid must be positive"),
        Err(code) => panic!("ActivateApplication failed: {code:?}"),
    }
    eprintln!("checkpoint: aumid activation started");
    // Give the activated app a moment to register its window, then close it
    // so the machine is left clean.
    std::thread::sleep(std::time::Duration::from_millis(1500));

    // P2-08 fallback: an invalid AUMID fails activation, and the original LNK
    // (a cmd that exits immediately) still runs through the Shell.
    let fallback_link = directory.join("fallback.lnk");
    create_cmd_link(&fallback_link);
    let item = Item {
        id: "fallback".into(),
        name: "Fallback".into(),
        item_type: ItemKind::Lnk,
        launch: Launch {
            path: fallback_link.to_string_lossy().into_owned(),
            args: vec![],
            workdir: String::new(),
            run_as_admin: false,
            aumid: "QuickNest.NoSuch.App.2026".into(),
            msi_advertised: false,
            delay_seconds: 0,
            execution_mode: String::new(),
            confirm_on_first_run: false,
        },
        icon: String::new(),
        window_state: WindowState::Normal,
        note: String::new(),
        last_used: 0,
        launch_count: 0,
        favorite: false,
    };
    match launch_item(&item, false) {
        LaunchOutcome::Started { pid } => assert!(pid > 0, "fallback pid"),
        LaunchOutcome::Failed(error) => {
            panic!(
                "AUMID failure must fall back to the original LNK: {}",
                error.message()
            )
        }
    }

    // P2-08: a valid AUMID launches even when the original LNK is gone.
    let gone_link = directory.join("gone.lnk");
    create_apps_folder_link(calc_aumid, &gone_link);
    std::fs::remove_file(&gone_link).expect("remove gone link");
    let gone_item = Item {
        id: "gone".into(),
        name: "Gone".into(),
        item_type: ItemKind::Lnk,
        launch: Launch {
            path: gone_link.to_string_lossy().into_owned(),
            args: vec![],
            workdir: String::new(),
            run_as_admin: false,
            aumid: calc_aumid.into(),
            msi_advertised: false,
            delay_seconds: 0,
            execution_mode: String::new(),
            confirm_on_first_run: false,
        },
        icon: String::new(),
        window_state: WindowState::Normal,
        note: String::new(),
        last_used: 0,
        launch_count: 0,
        favorite: false,
    };
    match launch_item(&gone_item, false) {
        LaunchOutcome::Started { pid } => assert!(pid > 0, "gone-link pid"),
        LaunchOutcome::Failed(error) => {
            panic!(
                "a valid AUMID must launch without the LNK: {}",
                error.message()
            )
        }
    }
    eprintln!("checkpoint: aumid activation + fallback ok");

    let _ = std::fs::remove_dir_all(directory.parent().expect("run dir"));
    println!("uwp link detection ok: {}", detected.aumid);
}

/// Creates a shortcut whose target is a file (the non-UWP control).
fn create_file_link(link_path: &std::path::Path) {
    // SAFETY: the test thread owns this STA.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).expect("shell link");
        let target = wide(r"C:\Windows\System32\notepad.exe");
        link.SetPath(PCWSTR(target.as_ptr())).expect("set path");
        let persist: IPersistFile = link.cast().expect("persist file");
        let units = wide(&link_path.to_string_lossy());
        persist.Save(PCWSTR(units.as_ptr()), true).expect("save");
    }
}

/// Creates a shortcut to `cmd /c exit` — the Shell fallback target for the
/// invalid-AUMID test; the spawned cmd exits immediately.
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

/// Creates a shortcut whose IDList points at an AppsFolder item (the UWP
/// shape used by Windows when users pin packaged apps).
fn create_apps_folder_link(aumid: &str, link_path: &std::path::Path) {
    // SAFETY: the parsing call allocates a PIDL freed below; the test thread
    // owns the STA throughout.
    unsafe {
        let parsing = wide(&format!("shell:AppsFolder\\{aumid}"));
        let mut pidl = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(parsing.as_ptr()), None, &mut pidl, 0, None)
            .expect("parse apps folder item");
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).expect("shell link");
        link.SetIDList(pidl).expect("set id list");
        let persist: IPersistFile = link.cast().expect("persist file");
        let units = wide(&link_path.to_string_lossy());
        persist.Save(PCWSTR(units.as_ptr()), true).expect("save");
        CoTaskMemFree(Some(pidl.cast::<core::ffi::c_void>()));
    }
}

/// Finds the first AppsFolder item that carries an AppUserModel ID.
fn first_apps_folder_aumid() -> Option<String> {
    // SAFETY: all COM calls run on the test's STA; allocated strings are
    // freed as soon as they are converted.
    unsafe {
        let apps = wide("shell:AppsFolder");
        let folder: IShellItem =
            SHCreateItemFromParsingName(PCWSTR(apps.as_ptr()), None).expect("apps folder");
        let items: IEnumShellItems = folder
            .BindToHandler(None, &windows::Win32::UI::Shell::BHID_EnumItems)
            .expect("enum");
        let mut fetched = [None; 1];
        while items.Next(&mut fetched, None).is_ok() {
            if let Some(item) = fetched[0].take() {
                let item2: IShellItem2 = item.cast().expect("item2");
                if let Ok(value) = item2.GetString(&APPUSERMODEL_ID) {
                    let text = read_pwstr(&value);
                    CoTaskMemFree(Some(value.as_ptr().cast::<core::ffi::c_void>()));
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
        }
    }
    None
}

static APPUSERMODEL_ID: windows::Win32::Foundation::PROPERTYKEY =
    windows::Win32::Foundation::PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
        pid: 5,
    };

/// Reads a NUL-terminated PWSTR into a String without API assumptions.
///
/// # Safety
///
/// `value` must point at a NUL-terminated wide string owned by the caller.
unsafe fn read_pwstr(value: &PWSTR) -> String {
    // SAFETY: the caller guarantees a live NUL-terminated wide string.
    unsafe {
        let mut len = 0_usize;
        while *value.as_ptr().add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(value.as_ptr(), len))
    }
}

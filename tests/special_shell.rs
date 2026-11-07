//! P2-10 special Shell targets in their own process: a Control Panel Shell
//! Link (IDList, no file-system target), `shell:` monikers, `::{CLSID}`
//! namespace folders and the namespace display name. OLE is initialized and
//! balanced on the test thread; every window opened by the test is closed.

use std::path::Path;
use std::time::{Duration, Instant};

use quicknest::infrastructure::windows::launch::{
    LaunchError, LaunchOutcome, launch_item, open_item_location,
};
use quicknest::infrastructure::windows::shell::{resolve_link_now, shell_display_name};
use quicknest::library_model::{Item, ItemKind, Launch, WindowState};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IPersistFile,
};
use windows::Win32::UI::Shell::{IShellLinkW, SHParseDisplayName, ShellLink};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, IsWindowVisible, PostMessageW, WM_CLOSE,
};
use windows::core::{BOOL, Interface, PCWSTR};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[test]
fn special_shell_targets() {
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
        .join(format!("quicknest-shell-{}", std::process::id()))
        .join("fixtures");
    std::fs::create_dir_all(&directory).expect("fixture directory");

    // 1. A Control Panel Shell Link: IDList target, no file-system path. The
    //    link must resolve as "no file target" and keep working through the
    //    original LNK (baseline 4.1/4.4).
    let control_panel_lnk = directory.join("control-panel.lnk");
    create_namespace_link("shell:ControlPanelFolder", &control_panel_lnk);
    let resolved = resolve_link_now(&control_panel_lnk).expect("a special Shell Link resolves");
    assert!(
        resolved
            .path
            .as_ref()
            .is_none_or(|path| path.as_os_str().is_empty()),
        "a special Shell Link must not resolve to a file-system path: {:?}",
        resolved.path
    );

    let lnk_item = Item {
        id: "control-panel".into(),
        name: "Control Panel".into(),
        item_type: ItemKind::Lnk,
        launch: Launch {
            path: control_panel_lnk.to_string_lossy().into_owned(),
            ..Launch::default()
        },
        icon: String::new(),
        last_used: 0,
        launch_count: 0,
        favorite: false,
        window_state: WindowState::Normal,
        note: String::new(),
    };
    let before = visible_windows();
    match launch_item(&lnk_item, false) {
        LaunchOutcome::Started { .. } => {}
        LaunchOutcome::Failed(error) => panic!("Control Panel LNK failed: {}", error.message()),
    }
    let window = wait_for_new_window(&before, &["控制面板", "Control Panel"], 10_000)
        .expect("the Control Panel window did not appear");
    close_window(window.0);
    eprintln!("checkpoint: control panel link opened {}", window.1);

    // 2. `shell:` moniker as a Folder target (no local exists() probe).
    let this_pc = namespace_item(
        ItemKind::Folder,
        "shell:MyComputerFolder",
        "This PC (shell)",
    );
    let before = visible_windows();
    match launch_item(&this_pc, false) {
        LaunchOutcome::Started { .. } => {}
        LaunchOutcome::Failed(error) => {
            panic!("shell:MyComputerFolder failed: {}", error.message())
        }
    }
    let window = wait_for_new_window(&before, &["此电脑", "This PC"], 10_000)
        .expect("the This PC window (shell:) did not appear");
    close_window(window.0);
    eprintln!("checkpoint: shell:MyComputerFolder opened {}", window.1);

    // 3. `::{CLSID}` namespace folder.
    let this_pc_clsid = namespace_item(
        ItemKind::Folder,
        "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}",
        "This PC (CLSID)",
    );
    let before = visible_windows();
    match launch_item(&this_pc_clsid, false) {
        LaunchOutcome::Started { .. } => {}
        LaunchOutcome::Failed(error) => panic!("::{{CLSID}} failed: {}", error.message()),
    }
    let window = wait_for_new_window(&before, &["此电脑", "This PC"], 10_000)
        .expect("the This PC window (::{CLSID}) did not appear");
    close_window(window.0);
    eprintln!("checkpoint: ::{{CLSID}} opened {}", window.1);

    // 4. The Shell display name is read from the namespace, never guessed.
    let display = shell_display_name(Path::new("::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"))
        .expect("the This PC display name");
    assert!(!display.trim().is_empty());
    eprintln!("checkpoint: display name = {display}");

    // 5. "Open file location" has no meaning for namespace/URI targets.
    for item in [&lnk_item, &this_pc_clsid] {
        if item.item_type == ItemKind::Lnk {
            continue;
        }
        match open_item_location(item) {
            LaunchOutcome::Failed(LaunchError::NoFileLocation) => {}
            other => panic!("expected NoFileLocation, got {other:?}"),
        }
    }

    let _ = std::fs::remove_dir_all(&directory);
}

fn namespace_item(kind: ItemKind, path: &str, name: &str) -> Item {
    Item {
        id: name.to_string(),
        name: name.to_string(),
        item_type: kind,
        launch: Launch {
            path: path.to_string(),
            ..Launch::default()
        },
        icon: String::new(),
        last_used: 0,
        launch_count: 0,
        favorite: false,
        window_state: WindowState::Normal,
        note: String::new(),
    }
}

/// Creates a Shell Link whose target is a namespace parsing name (`IDList`).
fn create_namespace_link(parsing_name: &str, link_path: &Path) {
    // SAFETY: all interfaces stay on the test's STA; the PIDL is freed after
    // SetIDList has copied it.
    unsafe {
        let parsing = wide(parsing_name);
        let mut pidl = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(parsing.as_ptr()), None, &mut pidl, 0, None)
            .expect("parse namespace name");
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).expect("shell link");
        link.SetIDList(pidl).expect("set id list");
        let persist: IPersistFile = link.cast().expect("persist file");
        let units = wide(&link_path.to_string_lossy());
        persist.Save(PCWSTR(units.as_ptr()), true).expect("save");
        CoTaskMemFree(Some(pidl.cast::<core::ffi::c_void>()));
    }
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: the pointer comes from `visible_windows` and lives for the
    // duration of EnumWindows.
    unsafe {
        let windows = &mut *(lparam.0 as *mut Vec<(isize, String)>);
        if IsWindowVisible(hwnd).as_bool() {
            let mut buffer = [0_u16; 512];
            let len = GetWindowTextW(hwnd, &mut buffer);
            if len > 0 {
                windows.push((
                    hwnd.0 as isize,
                    String::from_utf16_lossy(&buffer[..len as usize]),
                ));
            }
        }
    }
    BOOL(1)
}

fn visible_windows() -> Vec<(isize, String)> {
    let mut windows = Vec::new();
    // SAFETY: the callback writes into `windows`, which outlives the call.
    unsafe {
        let _ = EnumWindows(
            Some(collect_window),
            LPARAM(&mut windows as *mut Vec<(isize, String)> as isize),
        );
    }
    windows
}

fn wait_for_new_window(
    before: &[(isize, String)],
    needles: &[&str],
    timeout_ms: u64,
) -> Option<(isize, String)> {
    let previous: Vec<isize> = before.iter().map(|(hwnd, _)| *hwnd).collect();
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        for (hwnd, title) in visible_windows() {
            if !previous.contains(&hwnd) && needles.iter().any(|needle| title.contains(needle)) {
                return Some((hwnd, title));
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

fn close_window(hwnd: isize) {
    // SAFETY: the HWND was returned by EnumWindows for a visible window; a
    // failed PostMessage only leaves the window open.
    unsafe {
        let _ = PostMessageW(
            Some(HWND(hwnd as *mut core::ffi::c_void)),
            WM_CLOSE,
            WPARAM(0),
            LPARAM(0),
        );
    }
    std::thread::sleep(Duration::from_millis(300));
}

//! LNK launch test in its own process: creating a Shell Link initializes OLE
//! and leaves an implicit MTA, which must not leak into the unit-test process
//! (whose apartment assertions require fresh threads to be uninitialized).

use quicknest::infrastructure::windows::launch::{LaunchOutcome, launch_item};
use quicknest::library_model::{Item, ItemKind, Launch, WindowState};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IPersistFile,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{Interface, PCWSTR};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Creates a real .lnk through IShellLinkW/IPersistFile (baseline 4.1).
fn create_test_link(target: &str, arguments: &str, workdir: &str, link_path: &std::path::Path) {
    // SAFETY: the test thread owns this STA for the duration of the calls; the
    // apartment is balanced below.
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).expect("shell link");
        let target_units = wide(target);
        let argument_units = wide(arguments);
        let workdir_units = wide(workdir);
        link.SetPath(PCWSTR(target_units.as_ptr())).expect("path");
        link.SetArguments(PCWSTR(argument_units.as_ptr()))
            .expect("arguments");
        link.SetWorkingDirectory(PCWSTR(workdir_units.as_ptr()))
            .expect("workdir");
        let persist: IPersistFile = link.cast().expect("persist file");
        let link_units = wide(&link_path.to_string_lossy());
        persist
            .Save(PCWSTR(link_units.as_ptr()), true)
            .expect("save link");
        if initialized {
            CoUninitialize();
        }
    }
}

#[test]
fn launching_a_shortcut_keeps_its_own_arguments_and_workdir() {
    let directory = std::env::temp_dir().join(format!(
        "quicknest-lnk-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&directory).expect("temp dir");
    let link_path = directory.join("probe.lnk");
    create_test_link(
        "C:\\Windows\\System32\\cmd.exe",
        "/c echo via-lnk>probe.txt",
        &directory.to_string_lossy(),
        &link_path,
    );
    let probe = directory.join("probe.txt");
    let item = Item {
        id: "test-item".to_string(),
        name: "测试快捷方式".to_string(),
        item_type: ItemKind::Lnk,
        launch: Launch {
            path: link_path.to_string_lossy().into_owned(),
            args: vec!["/c".to_string(), "echo wrong>wrong.txt".to_string()],
            workdir: directory.join("ignored").to_string_lossy().into_owned(),
            run_as_admin: false,
            aumid: String::new(),
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
        LaunchOutcome::Started { pid } => assert!(pid > 0, "pid"),
        LaunchOutcome::Failed(error) => panic!("launch failed: {}", error.message()),
    }
    for _ in 0..50 {
        if probe.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let content = std::fs::read_to_string(&probe).expect("lnk probe written");
    assert!(content.contains("via-lnk"), "content: {content}");
    assert!(
        !directory.join("wrong.txt").exists(),
        "the model metadata must not be passed to the LNK"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

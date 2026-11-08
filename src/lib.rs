use std::path::PathBuf;

pub mod build_info;
pub mod card_ui;
pub mod config_transfer;
pub mod crash_log;
pub mod drop_layout;
pub mod infrastructure;
pub mod launch_tasks;
pub mod library_controller;
pub mod library_model;
pub mod pinyin_index;
pub mod placement;
pub mod portable;
pub mod search;
pub mod theme;
pub mod update_net;
pub mod updater;
pub mod updater_validation;
pub mod window_state;

pub const USAGE: &str = "Usage: quicknest [--smoke-test | --hwnd-test <report.tsv> | --ole-test <report.tsv> | --drop-target-test <report.tsv> | --file-drop-test <report.tsv> | --dpi-test <report.tsv> | --dpi-scale-test <report.tsv> | --transfer-test <report.tsv> | --shell-test <report.tsv> | --mutex-test <report.tsv> | --mutex-child <report.tsv> | --mutex-keeper <name> <report.tsv> | --ipc-test <report.tsv> | --ipc-client <report.tsv> <command> | --ipc-raw <report.tsv> <kind> | --hotkey-test <report.tsv> | --hotkey-conflict <report.tsv> | --tray-test <report.tsv> | --focus-test <report.tsv> | --save-test <report.tsv> | --update-test <report.tsv> <url> | --crash-test | --help]";

pub const DEMO_ITEMS: [(&str, &str); 6] = [
    ("应用示例", "演示图标 · 不启动"),
    ("工具示例", "演示图标 · 不启动"),
    ("文件示例", "演示图标 · 不启动"),
    ("文件夹示例", "演示图标 · 不启动"),
    ("链接示例", "演示图标 · 不启动"),
    ("命令示例", "演示图标 · 不启动"),
];

#[derive(Debug, PartialEq, Eq)]
pub enum RunMode {
    Interactive,
    SmokeTest,
    HwndTest(PathBuf),
    OleTest(PathBuf),
    DropTargetTest(PathBuf),
    FileDropTest(PathBuf),
    DpiTest(PathBuf),
    DpiScaleTest(PathBuf),
    TransferTest(PathBuf),
    ShellTest(PathBuf),
    MutexTest(PathBuf),
    MutexChild(PathBuf),
    MutexKeeper { name: String, report: PathBuf },
    IpcTest(PathBuf),
    IpcClient { report: PathBuf, command: String },
    IpcRaw { report: PathBuf, kind: String },
    HotkeyTest(PathBuf),
    HotkeyConflict(PathBuf),
    TrayTest(PathBuf),
    FocusTest(PathBuf),
    SaveTest(PathBuf),
    UpdateTest { report: PathBuf, url: String },
    CrashTest,
    Help,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<RunMode, String> {
    let args: Vec<String> = args.into_iter().collect();
    match args.as_slice() {
        [] => Ok(RunMode::Interactive),
        [arg] if arg == "--smoke-test" => Ok(RunMode::SmokeTest),
        [arg] if arg == "--crash-test" => Ok(RunMode::CrashTest),
        [arg, path, url] if arg == "--update-test" && !path.is_empty() && !url.is_empty() => {
            Ok(RunMode::UpdateTest {
                report: PathBuf::from(path),
                url: url.clone(),
            })
        }
        [arg, path] if arg == "--hwnd-test" && !path.is_empty() => {
            Ok(RunMode::HwndTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--ole-test" && !path.is_empty() => {
            Ok(RunMode::OleTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--drop-target-test" && !path.is_empty() => {
            Ok(RunMode::DropTargetTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--file-drop-test" && !path.is_empty() => {
            Ok(RunMode::FileDropTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--dpi-test" && !path.is_empty() => {
            Ok(RunMode::DpiTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--dpi-scale-test" && !path.is_empty() => {
            Ok(RunMode::DpiScaleTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--transfer-test" && !path.is_empty() => {
            Ok(RunMode::TransferTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--shell-test" && !path.is_empty() => {
            Ok(RunMode::ShellTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--mutex-test" && !path.is_empty() => {
            Ok(RunMode::MutexTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--mutex-child" && !path.is_empty() => {
            Ok(RunMode::MutexChild(PathBuf::from(path)))
        }
        [arg, name, path] if arg == "--mutex-keeper" && !name.is_empty() && !path.is_empty() => {
            Ok(RunMode::MutexKeeper {
                name: name.into(),
                report: PathBuf::from(path),
            })
        }
        [arg, path] if arg == "--ipc-test" && !path.is_empty() => {
            Ok(RunMode::IpcTest(PathBuf::from(path)))
        }
        [arg, path, command]
            if arg == "--ipc-client" && !path.is_empty() && !command.is_empty() =>
        {
            Ok(RunMode::IpcClient {
                report: PathBuf::from(path),
                command: command.into(),
            })
        }
        [arg, path, kind] if arg == "--ipc-raw" && !path.is_empty() && !kind.is_empty() => {
            Ok(RunMode::IpcRaw {
                report: PathBuf::from(path),
                kind: kind.into(),
            })
        }
        [arg, path] if arg == "--hotkey-test" && !path.is_empty() => {
            Ok(RunMode::HotkeyTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--hotkey-conflict" && !path.is_empty() => {
            Ok(RunMode::HotkeyConflict(PathBuf::from(path)))
        }
        [arg, path] if arg == "--tray-test" && !path.is_empty() => {
            Ok(RunMode::TrayTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--focus-test" && !path.is_empty() => {
            Ok(RunMode::FocusTest(PathBuf::from(path)))
        }
        [arg, path] if arg == "--save-test" && !path.is_empty() => {
            Ok(RunMode::SaveTest(PathBuf::from(path)))
        }
        [arg] if arg == "--help" || arg == "-h" => Ok(RunMode::Help),
        _ => Err(USAGE.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_selects_interactive_mode() {
        assert_eq!(parse_args([]), Ok(RunMode::Interactive));
    }

    #[test]
    fn smoke_test_requires_explicit_argument() {
        assert_eq!(parse_args(["--smoke-test".into()]), Ok(RunMode::SmokeTest));
    }

    #[test]
    fn hwnd_test_requires_one_nonempty_report_path() {
        assert_eq!(
            parse_args(["--hwnd-test".into(), "report.tsv".into()]),
            Ok(RunMode::HwndTest(PathBuf::from("report.tsv")))
        );
        assert!(parse_args(["--hwnd-test".into()]).is_err());
        assert!(parse_args(["--hwnd-test".into(), String::new()]).is_err());
        assert!(parse_args(["--hwnd-test".into(), "report.tsv".into(), "extra".into()]).is_err());
    }

    #[test]
    fn ole_test_requires_one_nonempty_report_path() {
        assert_eq!(
            parse_args(["--ole-test".into(), "report.tsv".into()]),
            Ok(RunMode::OleTest(PathBuf::from("report.tsv")))
        );
        assert!(parse_args(["--ole-test".into()]).is_err());
        assert!(parse_args(["--ole-test".into(), String::new()]).is_err());
        assert!(parse_args(["--ole-test".into(), "report.tsv".into(), "extra".into()]).is_err());
    }

    #[test]
    fn drop_target_test_requires_one_nonempty_report_path() {
        assert_eq!(
            parse_args(["--drop-target-test".into(), "report.tsv".into()]),
            Ok(RunMode::DropTargetTest(PathBuf::from("report.tsv")))
        );
        assert!(parse_args(["--drop-target-test".into()]).is_err());
        assert!(parse_args(["--drop-target-test".into(), String::new()]).is_err());
        assert!(
            parse_args([
                "--drop-target-test".into(),
                "report.tsv".into(),
                "extra".into()
            ])
            .is_err()
        );
    }

    #[test]
    fn file_drop_test_requires_one_nonempty_report_path() {
        assert_eq!(
            parse_args(["--file-drop-test".into(), "report.tsv".into()]),
            Ok(RunMode::FileDropTest(PathBuf::from("report.tsv")))
        );
        assert!(parse_args(["--file-drop-test".into()]).is_err());
        assert!(parse_args(["--file-drop-test".into(), String::new()]).is_err());
        assert!(
            parse_args([
                "--file-drop-test".into(),
                "report.tsv".into(),
                "extra".into()
            ])
            .is_err()
        );
    }

    #[test]
    fn dpi_test_requires_one_nonempty_report_path() {
        assert_eq!(
            parse_args(["--dpi-test".into(), "report.tsv".into()]),
            Ok(RunMode::DpiTest(PathBuf::from("report.tsv")))
        );
        assert!(parse_args(["--dpi-test".into()]).is_err());
        assert!(parse_args(["--dpi-test".into(), String::new()]).is_err());
        assert!(parse_args(["--dpi-test".into(), "report.tsv".into(), "extra".into()]).is_err());
    }

    #[test]
    fn shell_test_requires_one_nonempty_report_path() {
        assert_eq!(
            parse_args(["--shell-test".into(), "report.tsv".into()]),
            Ok(RunMode::ShellTest(PathBuf::from("report.tsv")))
        );
        assert!(parse_args(["--shell-test".into()]).is_err());
        assert!(parse_args(["--shell-test".into(), String::new()]).is_err());
        assert!(parse_args(["--shell-test".into(), "report.tsv".into(), "extra".into()]).is_err());
    }

    #[test]
    fn mutex_modes_parse_strictly() {
        assert_eq!(
            parse_args(["--mutex-test".into(), "r.tsv".into()]),
            Ok(RunMode::MutexTest(PathBuf::from("r.tsv")))
        );
        assert_eq!(
            parse_args(["--mutex-child".into(), "r.tsv".into()]),
            Ok(RunMode::MutexChild(PathBuf::from("r.tsv")))
        );
        assert_eq!(
            parse_args(["--mutex-keeper".into(), "Local\\Q-1".into(), "r.tsv".into()]),
            Ok(RunMode::MutexKeeper {
                name: "Local\\Q-1".into(),
                report: PathBuf::from("r.tsv")
            })
        );
        for bad in [
            &["--mutex-test".to_string()][..],
            &["--mutex-test".to_string(), String::new()][..],
            &[
                "--mutex-test".to_string(),
                "r.tsv".to_string(),
                "x".to_string(),
            ][..],
            &["--mutex-child".to_string()][..],
            &["--mutex-keeper".to_string(), "n".to_string()][..],
            &["--mutex-keeper".to_string(), String::new(), "r".to_string()][..],
        ] {
            assert!(
                parse_args(bad.iter().cloned()).is_err(),
                "expected rejection of {bad:?}"
            );
        }
    }

    #[test]
    fn ipc_modes_parse_strictly() {
        assert_eq!(
            parse_args(["--ipc-test".into(), "r.tsv".into()]),
            Ok(RunMode::IpcTest(PathBuf::from("r.tsv")))
        );
        assert_eq!(
            parse_args(["--ipc-client".into(), "r.tsv".into(), "show".into()]),
            Ok(RunMode::IpcClient {
                report: PathBuf::from("r.tsv"),
                command: "show".into()
            })
        );
        assert_eq!(
            parse_args(["--ipc-raw".into(), "r.tsv".into(), "bad-json".into()]),
            Ok(RunMode::IpcRaw {
                report: PathBuf::from("r.tsv"),
                kind: "bad-json".into()
            })
        );
        for bad in [
            &["--ipc-test".to_string()][..],
            &["--ipc-test".to_string(), String::new()][..],
            &["--ipc-client".to_string(), "r.tsv".to_string()][..],
            &[
                "--ipc-raw".to_string(),
                "r.tsv".to_string(),
                "kind".to_string(),
                "extra".to_string(),
            ][..],
        ] {
            assert!(
                parse_args(bad.iter().cloned()).is_err(),
                "expected rejection of {bad:?}"
            );
        }
    }

    #[test]
    fn hotkey_modes_parse_strictly() {
        assert_eq!(
            parse_args(["--hotkey-test".into(), "r.tsv".into()]),
            Ok(RunMode::HotkeyTest(PathBuf::from("r.tsv")))
        );
        assert_eq!(
            parse_args(["--hotkey-conflict".into(), "r.tsv".into()]),
            Ok(RunMode::HotkeyConflict(PathBuf::from("r.tsv")))
        );
        for bad in [
            &["--hotkey-test".to_string()][..],
            &["--hotkey-test".to_string(), String::new()][..],
            &[
                "--hotkey-test".to_string(),
                "r.tsv".to_string(),
                "x".to_string(),
            ][..],
            &["--hotkey-conflict".to_string()][..],
        ] {
            assert!(
                parse_args(bad.iter().cloned()).is_err(),
                "expected rejection of {bad:?}"
            );
        }
    }

    #[test]
    fn tray_test_requires_one_nonempty_report_path() {
        assert_eq!(
            parse_args(["--tray-test".into(), "r.tsv".into()]),
            Ok(RunMode::TrayTest(PathBuf::from("r.tsv")))
        );
        assert!(parse_args(["--tray-test".into()]).is_err());
        assert!(parse_args(["--tray-test".into(), String::new()]).is_err());
        assert!(parse_args(["--tray-test".into(), "r.tsv".into(), "extra".into()]).is_err());
    }

    #[test]
    fn focus_test_requires_one_nonempty_report_path() {
        assert_eq!(
            parse_args(["--focus-test".into(), "r.tsv".into()]),
            Ok(RunMode::FocusTest(PathBuf::from("r.tsv")))
        );
        assert!(parse_args(["--focus-test".into()]).is_err());
        assert!(parse_args(["--focus-test".into(), String::new()]).is_err());
        assert!(parse_args(["--focus-test".into(), "r.tsv".into(), "extra".into()]).is_err());
    }

    #[test]
    fn save_test_requires_one_nonempty_report_path() {
        assert_eq!(
            parse_args(["--save-test".into(), "r.tsv".into()]),
            Ok(RunMode::SaveTest(PathBuf::from("r.tsv")))
        );
        assert!(parse_args(["--save-test".into()]).is_err());
        assert!(parse_args(["--save-test".into(), String::new()]).is_err());
        assert!(parse_args(["--save-test".into(), "r.tsv".into(), "extra".into()]).is_err());
    }

    #[test]
    fn help_does_not_open_a_window() {
        for arg in ["--help", "-h"] {
            assert_eq!(parse_args([arg.into()]), Ok(RunMode::Help));
        }
    }

    #[test]
    fn unexpected_arguments_are_rejected() {
        assert!(parse_args(["--unknown".into()]).is_err());
        assert!(parse_args(["--smoke-test".into(), "extra".into()]).is_err());
    }

    #[test]
    fn demo_grid_has_six_distinct_non_launching_items() {
        assert_eq!(DEMO_ITEMS.len(), 6);
        for (index, (name, detail)) in DEMO_ITEMS.iter().enumerate() {
            assert!(!name.is_empty());
            assert!(detail.contains("不启动"));
            assert!(!DEMO_ITEMS[..index].iter().any(|(other, _)| other == name));
        }
    }
}

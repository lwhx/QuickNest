//! P1-06 EXE launching and P2-08 AUMID activation (baseline 4.6).
//!
//! EXE items start through `CreateProcessW` (arguments, working directory and
//! window state are all honoured). When that fails, the Shell (`open`) is the
//! fallback. Elevation (`run-as-admin` or the menu entry) always goes through
//! `ShellExecuteExW` with the `runas` verb. UWP items with a saved AUMID are
//! activated directly (P2-08) with the original LNK as the Shell fallback.
//! Every failure is structured so the UI can show it (baseline 4: never fail
//! silently).

use windows::Win32::Foundation::{CloseHandle, GetLastError};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Threading::{
    CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTF_USESHOWWINDOW, STARTUPINFOW,
};
use windows::Win32::UI::Shell::{
    ACTIVATEOPTIONS, ApplicationActivationManager, IApplicationActivationManager, SEE_MASK_NOASYNC,
    SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, SW_SHOWNORMAL};
use windows::core::{HRESULT, PCWSTR, PWSTR};

use crate::library_model::{
    Item, ItemKind, WindowState, is_shell_namespace_target, is_unc_path, is_uri_target,
};

/// Why a launch did not start (baseline 4: structured, user-facing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// The item kind is implemented by a later task (P1-07..P1-09).
    UnsupportedType(ItemKind),
    EmptyTarget,
    MissingTarget(String),
    /// `CreateProcessW` failed (the Shell fallback failed too).
    Spawn {
        win32: u32,
    },
    /// Elevated launch through `runas` failed.
    PrivilegedRun {
        win32: u32,
    },
    /// P2-09 (baseline 4.3): an MSI advertised shortcut's LNK is gone.
    MsiShortcutMissing {
        path: String,
    },
    /// P2-09: the Shell could not start an MSI advertised shortcut; the
    /// product likely needs a repair or the entry must be re-added.
    MsiLaunchFailed {
        win32: u32,
    },
    /// P2-10 (baseline 4.4): the target is a URI, Shell namespace entry or
    /// UNC share; there is no local file to reveal in Explorer.
    NoFileLocation,
    /// The URI has no valid scheme (baseline 3.1: HTTP/HTTPS/steam/custom).
    InvalidTarget(String),
    /// The structured arguments cannot be represented safely by this launcher.
    UnsafeCommand(String),
    /// B8 (audit 2026-11-08): the declared item type contradicts the script
    /// suffix of the target (.bat/.cmd/.ps1). Native argv encoding and the
    /// Shell association fallback are wrong for scripts; the item must be
    /// switched to an explicit command mode.
    ScriptTypeMismatch {
        target: String,
    },
}

impl LaunchError {
    pub fn message(&self) -> String {
        match self {
            Self::UnsupportedType(kind) => {
                format!("{} 启动将在后续任务实现", kind.label())
            }
            Self::EmptyTarget => "项目目标为空".to_string(),
            Self::MissingTarget(path) => format!("目标不存在：{path}"),
            Self::Spawn { win32 } => format!("启动失败（Win32 错误 {win32}）"),
            Self::PrivilegedRun { win32 } => format!("管理员启动失败（Win32 错误 {win32}）"),
            Self::MsiShortcutMissing { path } => {
                format!("MSI 快捷方式已失效：{path}。请修复安装或重新添加入口")
            }
            Self::MsiLaunchFailed { win32 } => {
                format!("MSI 快捷方式启动失败（Win32 错误 {win32}）。请修复安装或重新添加入口")
            }
            Self::NoFileLocation => "该项目没有本地文件位置（Shell 项目）".to_string(),
            Self::InvalidTarget(value) => format!("不是有效的 URI：{value}"),
            Self::UnsafeCommand(reason) => format!("拒绝启动：{reason}"),
            Self::ScriptTypeMismatch { target } => format!(
                "目标“{target}”是脚本文件（.bat/.cmd/.ps1），但项目类型与执行模式不匹配；请把该项目改为“自定义命令”并选择 CMD 或 PowerShell 执行模式"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchOutcome {
    Started { pid: u32 },
    Failed(LaunchError),
}

/// B8 (audit 2026-11-08): the dispatch-time guard. A declared type that
/// contradicts the script suffix of its target must never reach the native
/// launcher: `CreateProcessW` would encode arguments with native CRT rules
/// and the failure path would hand the script to the Shell association —
/// both bypass the dedicated CMD encoding and the command confirmation.
/// Runs before any process creation, so neither the direct spawn nor the
/// ShellExecute fallback can be reached for a mismatched item. This is a
/// hard stop: confirming the item once (P2-17) explains the problem, but the
/// launch stays refused until the type/mode is actually fixed.
pub fn validate_launch_guard(item: &Item) -> Result<(), LaunchError> {
    if item.has_script_mismatch() {
        return Err(LaunchError::ScriptTypeMismatch {
            target: item.launch.path.trim().to_string(),
        });
    }
    Ok(())
}

/// Launches an item. EXE items use `CreateProcessW` (Shell fallback); LNK
/// items always go through the Shell with the original shortcut so the link's
/// own target/arguments/working-directory semantics stay intact (baseline
/// 4.1/4.6). `as_admin` forces elevation regardless of the item's
/// `run_as_admin` flag (baseline 3.5 "管理员身份运行").
pub fn launch_item(item: &Item, as_admin: bool) -> LaunchOutcome {
    if item.launch.path.contains('\0')
        || item.launch.workdir.contains('\0')
        || item.launch.args.iter().any(|arg| arg.contains('\0'))
    {
        return LaunchOutcome::Failed(LaunchError::UnsafeCommand(
            "目标、参数和工作目录不能包含 NUL".into(),
        ));
    }
    // 便携版（基线 23 章）：相对路径项目按便携根解析后再分发。
    let item = item.with_resolved_target();
    let item = &item;
    // B8：分发最前的类型一致性闸门——mismatch 项在任何原生执行
    // （CreateProcessW 与 ShellExecute 回退）之前被结构化拒绝。
    if let Err(error) = validate_launch_guard(item) {
        return LaunchOutcome::Failed(error);
    }
    match item.item_type {
        ItemKind::Exe => launch_exe(item, as_admin),
        ItemKind::Lnk => launch_lnk(item, as_admin),
        ItemKind::File | ItemKind::Folder => launch_shell_target(item, as_admin),
        ItemKind::Uri => launch_uri(item, as_admin),
        ItemKind::Command => launch_command(item, as_admin),
        other => LaunchOutcome::Failed(LaunchError::UnsupportedType(other)),
    }
}

/// P3-08 (baseline 8.2 CMD): cmd.exe honors `/d` (no AutoRun), `/s /c`
/// (strip only the outer quotes of the whole tail) so a quoted program path
/// plus arguments reach cmd.exe verbatim.
fn cmd_exe_path() -> String {
    std::env::var("COMSPEC").unwrap_or_else(|_| "C:\\Windows\\System32\\cmd.exe".to_string())
}

/// P3-08 (baseline 8.2 PowerShell): Windows PowerShell 5.1 ships in
/// System32; `-NoProfile -File` runs a script file, never inline scripts.
fn powershell_exe_path() -> String {
    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe".to_string()
}

/// Builds the complete lpParameters for CMD, including its /s outer wrapper.
/// CMD does not understand CRT backslash escapes for quotes. Reject quotes,
/// expansion (%/!), and control characters instead of turning literal input
/// into executable shell source. Operators (&|<>^()) stay inside quotes.
/// Only file commands are accepted: builtins have incompatible argument rules.
pub fn build_cmd_parameters(program: &str, args: &[String]) -> Result<String, LaunchError> {
    let invalid = |value: &str| {
        value
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | '%' | '!'))
    };
    if program.is_empty() || invalid(program) || args.iter().any(|arg| invalid(arg)) {
        return Err(LaunchError::UnsafeCommand(
            "CMD 字面目标/参数不能包含双引号、%、! 或控制字符；请使用 Direct 或专用脚本".into(),
        ));
    }
    let extension = std::path::Path::new(program)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let batch = matches!(extension.as_str(), "bat" | "cmd");
    if !batch && !matches!(extension.as_str(), "exe" | "com") {
        return Err(LaunchError::UnsafeCommand(
            "CMD 目标必须是 .exe、.com、.bat 或 .cmd 文件；不支持内建命令或命令串".into(),
        ));
    }
    let mut tail = format!("\"{program}\"");
    for arg in args {
        tail.push(' ');
        // Batch parameters retain their enclosing quotes and literal trailing
        // backslashes; native programs decode backslashes using CRT rules.
        if batch {
            tail.push('"');
            tail.push_str(arg);
            tail.push('"');
        } else {
            tail.push_str(&crate::library_model::quote_arg_always(arg));
        }
    }
    let parameters = format!("/d /v:off /s /c \"{tail}\"");
    if parameters.encode_utf16().count() + cmd_exe_path().encode_utf16().count() + 4 > 8191 {
        return Err(LaunchError::UnsafeCommand(
            "CMD 命令行超过 8191 字符限制".into(),
        ));
    }
    Ok(parameters)
}

/// P3-08 (baseline 8.1/8.2): structured command launch. direct runs the
/// program itself through CreateProcessW (no cmd.exe, no shell string), cmd
/// runs `cmd.exe /d /s /c <program> <args...>`, powershell runs
/// `powershell.exe -NoProfile -File <script> <args...>`.
fn launch_command(item: &Item, as_admin: bool) -> LaunchOutcome {
    let program = item.launch.path.trim();
    if program.is_empty() {
        return LaunchOutcome::Failed(LaunchError::EmptyTarget);
    }
    match crate::library_model::normalize_execution_mode(&item.launch.execution_mode) {
        "cmd" => {
            let parameters = match build_cmd_parameters(program, &item.launch.args) {
                Ok(parameters) => parameters,
                Err(error) => return LaunchOutcome::Failed(error),
            };
            let interpreter = cmd_exe_path();
            // CMD's entire command tail is shell syntax, never a CRT argv item.
            // Both elevation paths receive exactly the same validated tail.
            if as_admin || item.launch.run_as_admin {
                return shell_execute_parameters(
                    &interpreter,
                    &parameters,
                    &item.launch.workdir,
                    item.window_state,
                    true,
                );
            }
            match spawn_exe_parameters(
                &interpreter,
                &parameters,
                &item.launch.workdir,
                item.window_state,
            ) {
                Ok(pid) => LaunchOutcome::Started { pid },
                Err(spawn_error) => {
                    LaunchOutcome::Failed(LaunchError::Spawn { win32: spawn_error })
                }
            }
        }
        "powershell" => {
            if !std::path::Path::new(program).exists() {
                return LaunchOutcome::Failed(LaunchError::MissingTarget(program.to_string()));
            }
            let mut parts: Vec<String> = vec![
                "-NoProfile".to_string(),
                "-File".to_string(),
                program.to_string(),
            ];
            parts.extend(item.launch.args.iter().cloned());
            if as_admin || item.launch.run_as_admin {
                return shell_execute(
                    &powershell_exe_path(),
                    &parts,
                    &item.launch.workdir,
                    item.window_state,
                    true,
                );
            }
            match spawn_exe(
                &powershell_exe_path(),
                &parts,
                &item.launch.workdir,
                item.window_state,
            ) {
                Ok(pid) => LaunchOutcome::Started { pid },
                Err(spawn_error) => {
                    LaunchOutcome::Failed(LaunchError::Spawn { win32: spawn_error })
                }
            }
        }
        // Baseline 8.2 Direct (default): the program path stays independent,
        // the argument array stays independent, nothing goes through
        // cmd.exe.
        _ => {
            // Association fallback for batch files would silently enter CMD
            // with native argv escaping. Require the explicit safe CMD mode.
            if std::path::Path::new(program)
                .extension()
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("cmd")
                        || ext.eq_ignore_ascii_case("bat")
                        || ext.eq_ignore_ascii_case("ps1")
                })
            {
                return LaunchOutcome::Failed(LaunchError::UnsafeCommand(
                    "Direct 不支持脚本文件；请选择 CMD 或 PowerShell 执行模式".into(),
                ));
            }
            if !std::path::Path::new(program).exists() {
                return LaunchOutcome::Failed(LaunchError::MissingTarget(program.to_string()));
            }
            if as_admin || item.launch.run_as_admin {
                return shell_execute(
                    program,
                    &item.launch.args,
                    &item.launch.workdir,
                    item.window_state,
                    true,
                );
            }
            match spawn_exe(
                program,
                &item.launch.args,
                &item.launch.workdir,
                item.window_state,
            ) {
                Ok(pid) => LaunchOutcome::Started { pid },
                Err(spawn_error) => {
                    // Same Shell fallback as EXE items (baseline 4.6); if the
                    // Shell also fails, the CreateProcessW error is reported.
                    match shell_execute(
                        program,
                        &item.launch.args,
                        &item.launch.workdir,
                        item.window_state,
                        false,
                    ) {
                        LaunchOutcome::Started { pid } => LaunchOutcome::Started { pid },
                        LaunchOutcome::Failed(_) => {
                            LaunchOutcome::Failed(LaunchError::Spawn { win32: spawn_error })
                        }
                    }
                }
            }
        }
    }
}

/// P3-09 (baseline 22 Everything 可选集成): resolves the Everything
/// executable — the configured path when it exists, else the first fallback
/// that exists. `None` means Everything is not installed; the optional
/// integration never breaks the main program.
pub fn resolve_everything_path(configured: &str, fallbacks: &[String]) -> Option<String> {
    if !configured.trim().is_empty() && std::path::Path::new(configured).exists() {
        return Some(configured.to_string());
    }
    fallbacks
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .cloned()
}

/// P3-09: the standard fallback install locations searched after the
/// configured path.
pub fn everything_fallback_paths() -> Vec<String> {
    let mut paths = Vec::new();
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        paths.push(format!("{}\\Everything\\Everything.exe", program_files));
    }
    if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
        paths.push(format!("{}\\Everything\\Everything.exe", program_files_x86));
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        paths.push(format!("{}\\Everything\\Everything.exe", local_app_data));
    }
    paths
}

/// P3-09: builds the synthetic EXE item that hands a search query to
/// Everything (`Everything.exe -search <query>`).
pub fn everything_handoff_item(path: &str, query: &str) -> Item {
    let mut item = Item::new(
        "everything-handoff",
        format!("Everything：{query}"),
        ItemKind::Exe,
        path,
    );
    item.launch.args = vec!["-search".to_string(), query.to_string()];
    item.launch.confirm_on_first_run = false;
    item
}

/// P3-08 (baseline 8.2): the actual execution content of a command item, for
/// the UI (CMD/PowerShell must display what actually runs).
pub fn command_display(item: &Item) -> String {
    let program = item.launch.path.trim();
    let mut parts: Vec<String> = Vec::new();
    let prefix = match crate::library_model::normalize_execution_mode(&item.launch.execution_mode) {
        "cmd" => {
            return match build_cmd_parameters(program, &item.launch.args) {
                Ok(parameters) => format!("cmd {parameters}"),
                Err(error) => error.message(),
            };
        }
        "powershell" => {
            parts.push("-NoProfile".to_string());
            parts.push("-File".to_string());
            parts.push(program.to_string());
            "powershell"
        }
        _ => {
            parts.push(program.to_string());
            "direct"
        }
    };
    parts.extend(item.launch.args.iter().cloned());
    format!("{prefix} {}", join_args(&parts))
}

/// Launches a URI through the system protocol handler (baseline 3.1/4.6):
/// http/https, steam, vscode and custom schemes all go through the Shell.
fn launch_uri(item: &Item, as_admin: bool) -> LaunchOutcome {
    let uri = item.launch.path.trim();
    if uri.is_empty() {
        return LaunchOutcome::Failed(LaunchError::EmptyTarget);
    }
    if !is_uri(uri) {
        return LaunchOutcome::Failed(LaunchError::InvalidTarget(uri.to_string()));
    }
    shell_execute(
        uri,
        &[],
        "",
        item.window_state,
        as_admin || item.launch.run_as_admin,
    )
}

/// Whether the value looks like a URI with a valid scheme (RFC 3986 scheme
/// grammar: ALPHA *( ALPHA / DIGIT / "+" / "-" / "." ) followed by ":").
/// Single-letter schemes are rejected so Windows drive paths like `C:\x` are
/// not mistaken for URIs. P2-10: the rule is shared with the model's kind
/// inference so `shell:` and `ms-settings:` stay URIs everywhere.
pub(crate) fn is_uri(value: &str) -> bool {
    is_uri_target(value)
}

/// Launches a plain file or folder through the Shell (baseline 4.6): files use
/// the system file association, folders open in Explorer. The model's args and
/// workdir are metadata only (baseline 3.3) and are not passed.
fn launch_shell_target(item: &Item, as_admin: bool) -> LaunchOutcome {
    let path = item.launch.path.trim();
    if path.is_empty() {
        return LaunchOutcome::Failed(LaunchError::EmptyTarget);
    }
    // P2-10 (baseline 4.4): Shell namespace entries (`::{CLSID}`,
    // `shell:...`, `ms-settings:...`) and UNC shares have no local file to
    // test. Hand them straight to the Shell; an offline share then gets the
    // Shell's own error instead of a blocking exists() probe.
    if !is_shell_namespace_target(path)
        && !is_unc_path(path)
        && !std::path::Path::new(path).exists()
    {
        return LaunchOutcome::Failed(LaunchError::MissingTarget(path.to_string()));
    }
    shell_execute(
        path,
        &[],
        "",
        item.window_state,
        as_admin || item.launch.run_as_admin,
    )
}

/// "打开文件所在位置并选中目标" (baseline 3.5): `SHOpenFolderAndSelectItems`
/// opens the containing folder with the item selected. Folders select
/// themselves in their parent.
pub fn open_item_location(item: &Item) -> LaunchOutcome {
    use windows::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems};
    let item = item.with_resolved_target();
    let path = item.launch.path.trim();
    if path.is_empty() {
        return LaunchOutcome::Failed(LaunchError::EmptyTarget);
    }
    // P2-10 (baseline 4.4): URIs, Shell namespace entries and UNC shares do
    // not map to a local file that could be selected in Explorer.
    if item.item_type == ItemKind::Uri || is_shell_namespace_target(path) || is_unc_path(path) {
        return LaunchOutcome::Failed(LaunchError::NoFileLocation);
    }
    if !std::path::Path::new(path).exists() {
        return LaunchOutcome::Failed(LaunchError::MissingTarget(path.to_string()));
    }
    let path_wide = wide(path);
    // SAFETY: the PIDL is created, used and freed within this scope.
    unsafe {
        let pidl = ILCreateFromPathW(PCWSTR(path_wide.as_ptr()));
        if pidl.is_null() {
            return LaunchOutcome::Failed(LaunchError::Spawn {
                win32: GetLastError().0,
            });
        }
        let result = SHOpenFolderAndSelectItems(pidl, None, 0);
        ILFree(Some(pidl));
        match result {
            Ok(()) => LaunchOutcome::Started { pid: 0 },
            Err(error) => LaunchOutcome::Failed(LaunchError::Spawn {
                win32: error.code().0 as u32,
            }),
        }
    }
}

/// D3 maintenance entries: opens a directory's CONTENTS in Explorer via the
/// `explore` verb. Fails with the Win32 error when the verb is refused.
pub fn open_directory(path: &std::path::Path) -> Result<(), u32> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;
    if !path.is_dir() {
        return Err(2);
    }
    let wide = wide(&path.to_string_lossy());
    // SAFETY: NUL-terminated wide buffers outlive the call; no elevation is
    // requested and the verb only opens a folder window.
    let result = unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("explore"),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        // SAFETY: identity query for the failed ShellExecuteW above.
        Err(unsafe { GetLastError().0 })
    }
}

/// D3 maintenance entries: reveals one existing file (for example the crash
/// log) in its containing folder, mirroring `open_item_location` without the
/// item rules.
pub fn reveal_path(path: &std::path::Path) -> Result<(), u32> {
    use windows::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems};
    use windows::core::PCWSTR;
    if !path.exists() {
        return Err(2);
    }
    let path_wide = wide(&path.to_string_lossy());
    // SAFETY: the PIDL is created, used and freed within this scope.
    unsafe {
        let pidl = ILCreateFromPathW(PCWSTR(path_wide.as_ptr()));
        if pidl.is_null() {
            return Err(GetLastError().0);
        }
        let result = SHOpenFolderAndSelectItems(pidl, None, 0);
        ILFree(Some(pidl));
        match result {
            Ok(()) => Ok(()),
            Err(error) => Err(error.code().0 as u32),
        }
    }
}

fn launch_exe(item: &Item, as_admin: bool) -> LaunchOutcome {
    let path = item.launch.path.trim();
    if path.is_empty() {
        return LaunchOutcome::Failed(LaunchError::EmptyTarget);
    }
    if !std::path::Path::new(path).exists() {
        return LaunchOutcome::Failed(LaunchError::MissingTarget(path.to_string()));
    }
    if as_admin || item.launch.run_as_admin {
        return shell_execute(
            path,
            &item.launch.args,
            &item.launch.workdir,
            item.window_state,
            true,
        );
    }
    match spawn_exe(
        path,
        &item.launch.args,
        &item.launch.workdir,
        item.window_state,
    ) {
        Ok(pid) => LaunchOutcome::Started { pid },
        Err(spawn_error) => {
            // Baseline 4.6: fall back to the Shell. If that also fails report
            // the original CreateProcessW error.
            match shell_execute(
                path,
                &item.launch.args,
                &item.launch.workdir,
                item.window_state,
                false,
            ) {
                LaunchOutcome::Started { pid } => LaunchOutcome::Started { pid },
                LaunchOutcome::Failed(_) => {
                    LaunchOutcome::Failed(LaunchError::Spawn { win32: spawn_error })
                }
            }
        }
    }
}

fn launch_lnk(item: &Item, as_admin: bool) -> LaunchOutcome {
    let path = item.launch.path.trim();
    if path.is_empty() {
        return LaunchOutcome::Failed(LaunchError::EmptyTarget);
    }
    // P2-08 (baseline 4.6 "UWP AUMID: 首选 IApplicationActivationManager"):
    // a saved AUMID is activated directly and does not need the LNK file at
    // all — the original shortcut may have been deleted. Elevated runs keep
    // the LNK's Shell semantics. MSI advertised shortcuts never use AUMID.
    if !as_admin
        && !item.launch.run_as_admin
        && !item.launch.msi_advertised
        && !item.launch.aumid.trim().is_empty()
        && let Ok(pid) = activate_application(&item.launch.aumid, &join_args(&item.launch.args))
    {
        return LaunchOutcome::Started { pid };
    }
    if !std::path::Path::new(path).exists() {
        // P2-09 (baseline 4.3): an advertised shortcut is only a Darwin
        // reference into the MSI; a missing LNK means the entry can no longer
        // be started and the user must repair the install or re-add it.
        return LaunchOutcome::Failed(if item.launch.msi_advertised {
            LaunchError::MsiShortcutMissing {
                path: path.to_string(),
            }
        } else {
            LaunchError::MissingTarget(path.to_string())
        });
    }
    // Baseline 4.1/4.2.5/4.3: the original LNK through the Shell — both the
    // normal path for UWP items without an AUMID and the fallback when AUMID
    // activation failed (baseline 4.2.5). MSI advertised shortcuts are never
    // resolved to an EXE, copied or rewritten: Windows Installer must stay in
    // control of installation, repair and component activation.
    let outcome = shell_execute(
        path,
        &[],
        "",
        item.window_state,
        as_admin || item.launch.run_as_admin,
    );
    map_msi_failure(outcome, item.launch.msi_advertised)
}

/// P2-09: Shell-level failures on an MSI advertised shortcut are reported
/// with the repair/re-add guidance instead of a bare Win32 code.
fn map_msi_failure(outcome: LaunchOutcome, msi_advertised: bool) -> LaunchOutcome {
    if !msi_advertised {
        return outcome;
    }
    match outcome {
        LaunchOutcome::Failed(LaunchError::Spawn { win32 })
        | LaunchOutcome::Failed(LaunchError::PrivilegedRun { win32 }) => {
            LaunchOutcome::Failed(LaunchError::MsiLaunchFailed { win32 })
        }
        other => other,
    }
}

/// P2-08 (baseline 4.2.4): direct UWP activation through
/// `IApplicationActivationManager::ActivateApplication`. Returns the PID of
/// the activated application. Must run on a COM-initialized thread (the UI
/// thread holds OLE for the process lifetime).
pub fn activate_application(aumid: &str, arguments: &str) -> Result<u32, HRESULT> {
    let manager: IApplicationActivationManager =
        unsafe { CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| error.code())?;
    let aumid_wide = wide(aumid);
    let arguments_wide = wide(arguments);
    // SAFETY: both wide strings outlive the call on this thread; the returned
    // PID is a plain u32.
    let pid = unsafe {
        manager.ActivateApplication(
            PCWSTR(aumid_wide.as_ptr()),
            PCWSTR(arguments_wide.as_ptr()),
            ACTIVATEOPTIONS(0),
        )
    }
    .map_err(|error| error.code())?;
    Ok(pid)
}

fn spawn_exe(path: &str, args: &[String], workdir: &str, state: WindowState) -> Result<u32, u32> {
    spawn_exe_parameters(path, &join_args(args), workdir, state)
}

/// `parameters` is already encoded for the selected interpreter. Do not quote
/// it again: CMD's /c tail is not an argv entry.
fn spawn_exe_parameters(
    path: &str,
    parameters: &str,
    workdir: &str,
    state: WindowState,
) -> Result<u32, u32> {
    // argv[0] uses its own Windows grammar; quotes/NUL cannot name an exe.
    if path.contains(['"', '\0']) || parameters.contains('\0') || workdir.contains('\0') {
        return Err(87); // ERROR_INVALID_PARAMETER
    }
    let path_wide = wide(path);
    let mut command_line = wide(&format!("\"{path}\" {parameters}"));
    let workdir_wide = (!workdir.trim().is_empty()).then(|| wide(workdir.trim()));
    let current_directory = workdir_wide
        .as_ref()
        .map(|directory| PCWSTR(directory.as_ptr()))
        .unwrap_or(PCWSTR::null());
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        dwFlags: STARTF_USESHOWWINDOW,
        wShowWindow: show_state(state) as u16,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: the wide buffers live for the duration of the call; the process
    // and thread handles returned are closed below.
    let created = unsafe {
        CreateProcessW(
            PCWSTR(path_wide.as_ptr()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            false,
            PROCESS_CREATION_FLAGS(0),
            None,
            current_directory,
            &startup,
            &mut process,
        )
    };
    match created {
        Ok(()) => {
            let pid = process.dwProcessId;
            // SAFETY: both handles are owned by this call and never reused.
            unsafe {
                let _ = CloseHandle(process.hProcess);
                let _ = CloseHandle(process.hThread);
            }
            Ok(pid)
        }
        Err(_) => {
            // GetLastError must be read immediately after the failure.
            // SAFETY: no intervening Win32 call.
            Err(unsafe { GetLastError().0 })
        }
    }
}

fn shell_execute(
    path: &str,
    args: &[String],
    workdir: &str,
    state: WindowState,
    admin: bool,
) -> LaunchOutcome {
    shell_execute_parameters(path, &join_args(args), workdir, state, admin)
}

fn shell_execute_parameters(
    path: &str,
    parameters: &str,
    workdir: &str,
    state: WindowState,
    admin: bool,
) -> LaunchOutcome {
    if path.contains('\0') || parameters.contains('\0') || workdir.contains('\0') {
        return LaunchOutcome::Failed(LaunchError::UnsafeCommand(
            "目标、参数和工作目录不能包含 NUL".into(),
        ));
    }
    let file = wide(path);
    let verb = wide(if admin { "runas" } else { "open" });
    let parameters = (!parameters.is_empty()).then(|| wide(parameters));
    let directory = (!workdir.trim().is_empty()).then(|| wide(workdir.trim()));
    // SAFETY: a zeroed SHELLEXECUTEINFOW is valid before the fields are set.
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    // Worker threads can uninitialize COM immediately after this returns.
    // Wait for Shell/DDE handoff, not for the launched process to exit.
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = PCWSTR(verb.as_ptr());
    info.lpFile = PCWSTR(file.as_ptr());
    if let Some(parameters) = &parameters {
        info.lpParameters = PCWSTR(parameters.as_ptr());
    }
    if let Some(directory) = &directory {
        info.lpDirectory = PCWSTR(directory.as_ptr());
    }
    info.nShow = show_state(state);
    // SAFETY: the wide buffers outlive the synchronous call; the returned
    // process handle (NOCLOSEPROCESS) is closed here.
    unsafe {
        if ShellExecuteExW(&mut info).is_err() {
            let win32 = GetLastError().0;
            return LaunchOutcome::Failed(if admin {
                LaunchError::PrivilegedRun { win32 }
            } else {
                LaunchError::Spawn { win32 }
            });
        }
        let pid = if info.hProcess.0.is_null() {
            0
        } else {
            let pid = windows::Win32::System::Threading::GetProcessId(info.hProcess);
            let _ = CloseHandle(info.hProcess);
            pid
        };
        LaunchOutcome::Started { pid }
    }
}

fn show_state(state: WindowState) -> i32 {
    match state {
        WindowState::Normal => SW_SHOWNORMAL.0,
        WindowState::Minimized => SW_SHOWMINIMIZED.0,
        WindowState::Maximized => SW_SHOWMAXIMIZED.0,
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The launcher and editor use the same Windows argument grammar.
pub(crate) use crate::library_model::join_args;
#[cfg(test)]
use crate::library_model::quote_arg;

/// Builds the NUL-terminated native line. argv[0] follows the distinct
/// executable-name grammar; the remaining entries follow CRT escaping.
#[cfg(test)]
pub(crate) fn build_command_line(path: &str, args: &[String]) -> Vec<u16> {
    let mut line = format!("\"{path}\"");
    if !args.is_empty() {
        line.push(' ');
        line.push_str(&join_args(args));
    }
    wide(&line)
}

/// P3-04 (baseline 22 鼠标热区): the cursor position in physical pixels.
pub fn cursor_position() -> Option<(i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: the out pointer is valid stack memory.
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok == 0 {
        return None;
    }
    Some((point.x, point.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_model::Launch;

    fn exe_item(path: &str, args: &[&str], workdir: &str) -> Item {
        Item {
            id: "test-item".to_string(),
            name: "测试程序".to_string(),
            item_type: ItemKind::Exe,
            launch: Launch {
                path: path.to_string(),
                args: args.iter().map(|arg| (*arg).to_string()).collect(),
                workdir: workdir.to_string(),
                run_as_admin: false,
                aumid: String::new(),
                msi_advertised: false,
                confirm_on_first_run: false,
                delay_seconds: 0,
                execution_mode: String::new(),
            },
            icon: String::new(),
            window_state: WindowState::Normal,
            last_used: 0,
            launch_count: 0,
            favorite: false,
            note: String::new(),
        }
    }

    #[test]
    fn cmd_arguments_reject_shell_expansion_and_quote_injection_before_launch() {
        for bad in [
            "a\" & echo injected",
            "%COMSPEC%",
            "!PATH!",
            "line\r\nbreak",
            "nul\0tail",
            "tab\tvalue",
        ] {
            assert!(
                matches!(
                    build_cmd_parameters("probe.exe", &[bad.into()]),
                    Err(LaunchError::UnsafeCommand(_))
                ),
                "{bad:?}"
            );
            assert!(build_cmd_parameters(bad, &[]).is_err());
            let mut item = exe_item("probe.cmd", &[bad], "");
            item.item_type = ItemKind::Command;
            item.launch.execution_mode = "cmd".into();
            item.launch.confirm_on_first_run = false;
            // No process or UAC is reached, including both elevation requests.
            assert!(matches!(
                launch_item(&item, false),
                LaunchOutcome::Failed(LaunchError::UnsafeCommand(_))
            ));
            item.launch.run_as_admin = true;
            assert!(matches!(
                launch_item(&item, true),
                LaunchOutcome::Failed(LaunchError::UnsafeCommand(_))
            ));
        }
        for target in [
            "echo",
            "probe.cmd & echo injected",
            "powershell",
            "probe.ps1",
        ] {
            assert!(build_cmd_parameters(target, &[]).is_err(), "{target}");
        }
        assert!(build_cmd_parameters("probe.exe", &["x".repeat(8192)]).is_err());
    }

    // A subprocess oracle for Rust's actual Windows CRT argument decoding.
    // Only explicitly invoked by the next test, with an isolated output file.
    #[test]
    #[ignore]
    fn cmd_arguments_native_probe_child() {
        if let Some(path) = std::env::var_os("QUICKNEST_ARGV_PROBE") {
            let args: Vec<String> = std::env::args()
                .skip_while(|arg| arg != "--")
                .skip(1)
                .collect();
            std::fs::write(path, serde_json::to_vec(&args).unwrap()).unwrap();
        }
    }

    #[test]
    fn cmd_arguments_native_literals_match_actual_windows_argv() {
        use std::os::windows::process::CommandExt;
        let dir =
            std::env::temp_dir().join(format!("qn-argv-{}", crate::library_model::new_uuid()));
        std::fs::create_dir_all(&dir).unwrap();
        let output = dir.join("argv.json");
        let expected: Vec<String> = [
            "",
            "a b",
            "&echo injected>injected.txt",
            "a|b<c>d",
            "^caret()",
            "C:\\space dir\\",
            "中文🙂",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        let mut args: Vec<String> = [
            "--exact",
            "infrastructure::windows::launch::tests::cmd_arguments_native_probe_child",
            "--ignored",
            "--nocapture",
            "--",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        args.extend(expected.clone());
        let exe = std::env::current_exe().unwrap();
        let parameters = build_cmd_parameters(&exe.to_string_lossy(), &args).unwrap();
        let status = std::process::Command::new(cmd_exe_path())
            .raw_arg(parameters)
            .env("QUICKNEST_ARGV_PROBE", &output)
            .current_dir(&dir)
            .status()
            .unwrap();
        assert!(status.success());
        let actual: Vec<String> = serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!(actual, expected);
        assert!(!dir.join("injected.txt").exists());
        // The same subprocess independently checks native quoting, including
        // values that CMD deliberately refuses but Direct must preserve.
        let mut native_args = args.clone();
        native_args.extend(["a\"b".into(), "slash\\\\\"quote".into(), "tab\t中文".into()]);
        let status = std::process::Command::new(&exe)
            .raw_arg(join_args(&native_args))
            .env("QUICKNEST_ARGV_PROBE", &output)
            .current_dir(&dir)
            .status()
            .unwrap();
        assert!(status.success());
        let actual: Vec<String> = serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!(actual, native_args[5..]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cmd_arguments_batch_path_and_metacharacters_are_literal() {
        use std::os::windows::process::CommandExt;
        let dir =
            std::env::temp_dir().join(format!("qn batch & {}", crate::library_model::new_uuid()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("probe & script.cmd");
        // The script itself uses safe quoted SET syntax, then writes via SET
        // without reparsing expanded argument text as commands.
        std::fs::write(&script, "@echo off\r\nset \"arg1=%~1\"\r\nset \"arg2=%~2\"\r\nset \"arg3=%~3\"\r\nset arg > probe.txt\r\n").unwrap();
        let args = vec![
            "a&echo injected>injected.txt".into(),
            "^()|<>".into(),
            "C:\\space dir\\".into(),
        ];
        let parameters = build_cmd_parameters(&script.to_string_lossy(), &args).unwrap();
        assert!(
            std::process::Command::new(cmd_exe_path())
                .raw_arg(parameters)
                .current_dir(&dir)
                .status()
                .unwrap()
                .success()
        );
        let content = std::fs::read_to_string(dir.join("probe.txt")).unwrap();
        for (index, arg) in args.iter().enumerate() {
            assert!(
                content.contains(&format!("arg{}={arg}\r\n", index + 1)),
                "{content:?}"
            );
        }
        assert!(!dir.join("injected.txt").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn argument_quoting_rules() {
        assert_eq!(quote_arg("plain"), "plain");
        assert_eq!(quote_arg("has space"), "\"has space\"");
        assert_eq!(quote_arg("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote_arg(""), "\"\"");
        assert_eq!(join_args(&["-a".into(), "b c".into()]), "-a \"b c\"");
    }

    #[test]
    fn command_line_starts_with_the_quoted_path_and_ends_with_nul() {
        let line = build_command_line("C:\\Apps\\demo.exe", &["--x".into(), "a b".into()]);
        let text = String::from_utf16(&line[..line.len() - 1]).expect("utf16");
        assert_eq!(text, "\"C:\\Apps\\demo.exe\" --x \"a b\"");
        assert_eq!(*line.last().expect("nul"), 0);
    }

    #[test]
    fn unsupported_kinds_report_a_structured_error() {
        let mut item = exe_item("C:\\Windows\\System32\\notepad.exe", &[], "");
        item.item_type = ItemKind::Automation;
        assert_eq!(
            launch_item(&item, false),
            LaunchOutcome::Failed(LaunchError::UnsupportedType(ItemKind::Automation))
        );
        // P3-08: commands run through the command engine now; an empty
        // target is still rejected before spawning.
        item.item_type = ItemKind::Command;
        item.launch.path = String::new();
        assert_eq!(
            launch_item(&item, false),
            LaunchOutcome::Failed(LaunchError::EmptyTarget)
        );
    }

    #[test]
    fn uri_scheme_validation() {
        for valid in [
            "https://example.com",
            "http://example.com/a?b=1",
            "steam://run/123",
            "vscode://file/C:/tmp/a.txt",
            "mailto:user@example.com",
            "ms-settings:display",
            "quicknest-p109:probe",
            "shell:AppsFolder",
        ] {
            assert!(is_uri(valid), "expected valid URI: {valid}");
        }
        for invalid in [
            "",
            "not a uri",
            "://example.com",
            "1abc:x",
            "http//example.com",
            "C:\\Windows\\notepad.exe",
        ] {
            assert!(!is_uri(invalid), "expected invalid URI: {invalid}");
        }
    }

    #[test]
    fn uri_targets_are_validated() {
        let mut item = exe_item("   ", &[], "");
        item.item_type = ItemKind::Uri;
        assert_eq!(
            launch_item(&item, false),
            LaunchOutcome::Failed(LaunchError::EmptyTarget)
        );
        item.launch.path = "not a uri".to_string();
        assert_eq!(
            launch_item(&item, false),
            LaunchOutcome::Failed(LaunchError::InvalidTarget("not a uri".to_string()))
        );
    }

    #[test]
    fn file_and_folder_targets_are_validated() {
        for kind in [ItemKind::File, ItemKind::Folder] {
            let mut item = exe_item("   ", &[], "");
            item.item_type = kind;
            assert_eq!(
                launch_item(&item, false),
                LaunchOutcome::Failed(LaunchError::EmptyTarget)
            );
            item.launch.path = "C:\\QuickNest Demo\\missing-target".to_string();
            assert!(matches!(
                launch_item(&item, false),
                LaunchOutcome::Failed(LaunchError::MissingTarget(_))
            ));
        }
        let mut item = exe_item("   ", &[], "");
        item.item_type = ItemKind::File;
        assert_eq!(
            open_item_location(&item),
            LaunchOutcome::Failed(LaunchError::EmptyTarget)
        );
        item.launch.path = "C:\\QuickNest Demo\\missing.txt".to_string();
        assert!(matches!(
            open_item_location(&item),
            LaunchOutcome::Failed(LaunchError::MissingTarget(_))
        ));
    }

    #[test]
    fn empty_and_missing_targets_are_rejected_before_spawning() {
        let empty = exe_item("   ", &[], "");
        assert_eq!(
            launch_item(&empty, false),
            LaunchOutcome::Failed(LaunchError::EmptyTarget)
        );
        let missing = exe_item("C:\\QuickNest Demo\\missing.exe", &[], "");
        assert!(matches!(
            launch_item(&missing, false),
            LaunchOutcome::Failed(LaunchError::MissingTarget(_))
        ));
    }

    #[test]
    fn spawning_cmd_honours_arguments_and_workdir() {
        let directory = std::env::temp_dir().join(format!(
            "quicknest-launch-{}",
            crate::library_model::new_uuid()
        ));
        std::fs::create_dir_all(&directory).expect("temp dir");
        let probe = directory.join("probe.txt");
        let item = exe_item(
            "C:\\Windows\\System32\\cmd.exe",
            &["/c", "echo launched>probe.txt"],
            &directory.to_string_lossy(),
        );
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
        let content = std::fs::read_to_string(&probe).expect("probe file written");
        assert!(content.contains("launched"), "content: {content}");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn msi_advertised_missing_link_requests_install_repair() {
        let mut item = exe_item("C:\\QuickNest Demo\\missing-msi.lnk", &[], "");
        item.item_type = ItemKind::Lnk;
        item.launch.msi_advertised = true;
        match launch_item(&item, false) {
            LaunchOutcome::Failed(error) => {
                assert!(
                    matches!(error, LaunchError::MsiShortcutMissing { .. }),
                    "expected an MSI-specific error, got {error:?}"
                );
                assert!(
                    error.message().contains("修复安装或重新添加入口"),
                    "message must point at repair/re-add: {}",
                    error.message()
                );
            }
            other => panic!("expected a structured failure, got {other:?}"),
        }
    }

    #[test]
    fn msi_shell_failures_are_mapped_to_repair_guidance() {
        let failure = map_msi_failure(LaunchOutcome::Failed(LaunchError::Spawn { win32: 2 }), true);
        match failure {
            LaunchOutcome::Failed(error) => {
                assert!(matches!(error, LaunchError::MsiLaunchFailed { win32: 2 }));
                assert!(error.message().contains("修复安装或重新添加入口"));
            }
            other => panic!("expected a mapped failure, got {other:?}"),
        }
        assert_eq!(
            map_msi_failure(
                LaunchOutcome::Failed(LaunchError::Spawn { win32: 2 }),
                false
            ),
            LaunchOutcome::Failed(LaunchError::Spawn { win32: 2 }),
            "ordinary LNK failures keep their original error"
        );
        assert_eq!(
            map_msi_failure(LaunchOutcome::Started { pid: 7 }, true),
            LaunchOutcome::Started { pid: 7 }
        );
    }

    #[test]
    fn shell_and_unc_targets_have_no_file_location() {
        for (kind, path) in [
            (ItemKind::Uri, "shell:MyComputerFolder"),
            (ItemKind::Folder, "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"),
            (ItemKind::Folder, "\\\\localhost\\C$"),
        ] {
            let mut item = exe_item(path, &[], "");
            item.item_type = kind;
            match open_item_location(&item) {
                LaunchOutcome::Failed(error) => {
                    assert_eq!(error, LaunchError::NoFileLocation, "{path}");
                    assert!(
                        error.message().contains("没有本地文件位置"),
                        "{}",
                        error.message()
                    );
                }
                other => panic!("expected NoFileLocation for {path}, got {other:?}"),
            }
        }
    }

    // P3-08 (baseline 8.2 CMD): a .cmd script runs through
    // cmd.exe /d /s /c and its probe file appears.
    #[test]
    fn command_cmd_mode_runs_the_script() {
        let dir = std::env::temp_dir().join(format!("qn-p308-cmd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let script = dir.join("run.cmd");
        std::fs::write(&script, "@echo off\r\necho cmd-ok >> probe-cmd.txt\r\n").expect("script");
        let mut item = exe_item(&script.to_string_lossy(), &[], &dir.to_string_lossy());
        item.item_type = ItemKind::Command;
        item.launch.execution_mode = "cmd".to_string();
        let outcome = launch_item(&item, false);
        assert!(
            matches!(outcome, LaunchOutcome::Started { .. }),
            "cmd launch failed: {outcome:?}"
        );
        let probe = dir.join("probe-cmd.txt");
        for _ in 0..50 {
            if probe.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let content = std::fs::read_to_string(&probe).expect("probe");
        assert!(content.contains("cmd-ok"), "probe: {content}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // P3-08 (baseline 8.2 PowerShell): a .ps1 script runs through
    // powershell.exe -NoProfile -File.
    #[test]
    fn command_powershell_mode_runs_the_script() {
        let dir = std::env::temp_dir().join(format!("qn-p308-ps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let script = dir.join("run.ps1");
        std::fs::write(&script, "Add-Content -Path probe-ps.txt -Value 'ps-ok'\r\n")
            .expect("script");
        let mut item = exe_item(&script.to_string_lossy(), &[], &dir.to_string_lossy());
        item.item_type = ItemKind::Command;
        item.launch.execution_mode = "powershell".to_string();
        let outcome = launch_item(&item, false);
        assert!(
            matches!(outcome, LaunchOutcome::Started { .. }),
            "powershell launch failed: {outcome:?}"
        );
        let probe = dir.join("probe-ps.txt");
        for _ in 0..80 {
            if probe.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let content = std::fs::read_to_string(&probe).expect("probe");
        assert!(content.contains("ps-ok"), "probe: {content}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // P3-08: a missing script/program reports MissingTarget instead of a
    // silent Shell error; the display helper shows the actual line.
    #[test]
    fn command_missing_target_and_display() {
        let mut item = exe_item("C:\\definitely\\missing\\run.ps1", &[], "");
        item.item_type = ItemKind::Command;
        item.launch.execution_mode = "powershell".to_string();
        assert_eq!(
            launch_item(&item, false),
            LaunchOutcome::Failed(LaunchError::MissingTarget(
                "C:\\definitely\\missing\\run.ps1".to_string()
            ))
        );
        item.launch.path = "C:\\Apps\\demo.cmd".to_string();
        item.launch.execution_mode = "cmd".to_string();
        item.launch.args = vec!["--x".to_string(), "a b".to_string()];
        assert_eq!(
            command_display(&item),
            "cmd /d /v:off /s /c \"\"C:\\Apps\\demo.cmd\" \"--x\" \"a b\"\""
        );
        item.launch.execution_mode = String::new();
        item.launch.path = "C:\\Apps\\demo.exe".to_string();
        assert_eq!(
            command_display(&item),
            "direct C:\\Apps\\demo.exe --x \"a b\""
        );
    }

    // P3-09: the handoff item carries -search and the query; path
    // resolution prefers the configured location and then the fallbacks.
    #[test]
    fn everything_handoff_and_path_resolution() {
        let item = everything_handoff_item("C:\\Tools\\Everything.exe", "qn probe");
        assert_eq!(item.launch.path, "C:\\Tools\\Everything.exe");
        assert_eq!(
            item.launch.args,
            vec!["-search".to_string(), "qn probe".to_string()]
        );
        assert!(!item.launch.confirm_on_first_run);
        // A configured path that exists wins.
        let dir = std::env::temp_dir().join(format!("qn-p309-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let configured = dir.join("Everything.exe");
        std::fs::write(&configured, b"x").expect("file");
        let missing = dir.join("missing.exe");
        let fallback = dir.join("fallback.exe");
        std::fs::write(&fallback, b"x").expect("file");
        let fallbacks = vec![
            missing.to_string_lossy().into_owned(),
            fallback.to_string_lossy().into_owned(),
        ];
        assert_eq!(
            resolve_everything_path(&configured.to_string_lossy(), &fallbacks),
            Some(configured.to_string_lossy().into_owned())
        );
        // A missing configured path falls back; nothing found means None.
        assert_eq!(
            resolve_everything_path(&missing.to_string_lossy(), &fallbacks),
            Some(fallback.to_string_lossy().into_owned())
        );
        assert_eq!(
            resolve_everything_path(&missing.to_string_lossy(), &[]),
            None
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // B8 (audit 2026-11-08): a script target declared as EXE/File, or a
    // command mode that contradicts the script suffix, is refused by the
    // dispatch guard before any process creation — the native spawn, the
    // ShellExecute association fallback and elevation requests are all
    // unreachable. The probe file proves nothing executed.
    #[test]
    fn script_type_mismatch_is_refused_before_native_or_shell_execution() {
        let dir =
            std::env::temp_dir().join(format!("qn-b8-guard-{}", crate::library_model::new_uuid()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let script = dir.join("guard-probe.cmd");
        // If any launcher path ran this script it would leave a probe file.
        std::fs::write(&script, "@echo off\r\necho mismatch-ran> probe.txt\r\n").expect("script");
        assert!(
            script.exists(),
            "the target exists, so the old native path would have been reached"
        );

        let mut item = exe_item(&script.to_string_lossy(), &[], "");
        let target = script.to_string_lossy().into_owned();
        assert_eq!(
            validate_launch_guard(&item),
            Err(LaunchError::ScriptTypeMismatch {
                target: target.clone()
            })
        );
        assert_eq!(
            launch_item(&item, false),
            LaunchOutcome::Failed(LaunchError::ScriptTypeMismatch {
                target: target.clone()
            })
        );
        // Elevation requests are guarded too (before any Shell runas).
        item.launch.run_as_admin = true;
        assert!(matches!(
            launch_item(&item, true),
            LaunchOutcome::Failed(LaunchError::ScriptTypeMismatch { .. })
        ));
        item.launch.run_as_admin = false;
        assert!(matches!(
            launch_item(&item, true),
            LaunchOutcome::Failed(LaunchError::ScriptTypeMismatch { .. })
        ));
        // The structured message explains the command-mode fix.
        match launch_item(&item, false) {
            LaunchOutcome::Failed(error) => {
                let message = error.message();
                assert!(message.contains("自定义命令"), "{message}");
                assert!(message.contains(&target), "{message}");
            }
            other => panic!("expected a structured refusal, got {other:?}"),
        }
        // No side effect: neither CreateProcessW nor the Shell fallback ran.
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(
            !dir.join("probe.txt").exists(),
            "the mismatched script must never execute, not even through the fallback"
        );

        // A plain File item pointing at a batch file is refused the same way.
        let mut file_item = exe_item(&target, &[], "");
        file_item.item_type = ItemKind::File;
        assert!(matches!(
            validate_launch_guard(&file_item),
            Err(LaunchError::ScriptTypeMismatch { .. })
        ));
        assert!(matches!(
            launch_item(&file_item, false),
            LaunchOutcome::Failed(LaunchError::ScriptTypeMismatch { .. })
        ));

        // A command mode contradicting the script suffix is refused before
        // the command engine's own checks (pure logic, nothing is spawned).
        let mut command_item = exe_item("C:\\Apps\\run.ps1", &[], "");
        command_item.item_type = ItemKind::Command;
        command_item.launch.execution_mode = "cmd".into();
        assert!(matches!(
            launch_item(&command_item, false),
            LaunchOutcome::Failed(LaunchError::ScriptTypeMismatch { .. })
        ));

        // Consistent items still pass the guard.
        assert_eq!(
            validate_launch_guard(&exe_item("C:\\Windows\\System32\\notepad.exe", &[], "")),
            Ok(())
        );
        let mut cmd_mode = exe_item("C:\\Apps\\run.cmd", &[], "");
        cmd_mode.item_type = ItemKind::Command;
        cmd_mode.launch.execution_mode = "cmd".into();
        assert_eq!(validate_launch_guard(&cmd_mode), Ok(()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Internal worker dispatched by updater::worker_entry before app initialization.
use super::*;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{REPLACE_FILE_FLAGS, ReplaceFileW};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
use windows::core::PCWSTR;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    signed_text: String,
    target: PathBuf,
    package: PathBuf,
    old_version: String,
    old_sha256: String,
    parent: u32,
    nonce: String,
    /// 父进程已登记的交接目录（handoff-{id}）；worker 仅在其内尽力写 failed 标记。
    #[serde(default)]
    handoff: Option<PathBuf>,
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::update_net::reject_reparse_path(path)?;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())
}
fn copy_new(from: &Path, to: &Path) -> Result<(), String> {
    crate::update_net::reject_reparse_path(from)?;
    crate::update_net::reject_reparse_path(to)?;
    let mut src = std::fs::File::open(from).map_err(|e| e.to_string())?;
    let mut dst = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(|e| e.to_string())?;
    let copied = std::io::copy(
        &mut std::io::Read::by_ref(&mut src).take(MAX_PACKAGE_BYTES + 1),
        &mut dst,
    )
    .map_err(|e| e.to_string())?;
    if copied > MAX_PACKAGE_BYTES {
        return Err("文件超过大小上限".into());
    }
    dst.sync_all().map_err(|e| e.to_string())
}
pub(super) fn spawn(update: VerifiedUpdate, target: &Path) -> Result<Child, String> {
    spawn_with(update, target, None, None, None, |text| {
        authenticate(text, release_key()?)
    })
}

/// 测试专用通道：签名验证替换为语法解析，最终执行的 worker 可替换为受控替身。
/// 生产路径从不调用本函数；发布构建不编译测试代码。
#[cfg(test)]
pub(super) fn spawn_for_test(
    update: VerifiedUpdate,
    target: &Path,
    worker_double: Option<&Path>,
    handoff: Option<&Path>,
    nonce: Option<&str>,
) -> Result<Child, String> {
    spawn_with(update, target, worker_double, handoff, nonce, |text| {
        UpdateManifest::parse(text)
    })
}

pub(super) fn spawn_with(
    mut update: VerifiedUpdate,
    target: &Path,
    worker_override: Option<&Path>,
    handoff_dir: Option<&Path>,
    nonce_override: Option<&str>,
    auth: impl FnOnce(&str) -> Result<UpdateManifest, String>,
) -> Result<Child, String> {
    // Installer and ZIP need transactional package-specific implementations.
    // Never silently treat their bytes as an executable replacement.
    if update.package_kind() != PackageKind::Raw {
        return Err(format!(
            "此 worker 尚不支持 {} 包；拒绝应用，请使用 raw 签名更新",
            update.package_kind().as_str()
        ));
    }
    let own = std::fs::canonicalize(std::env::current_exe().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let target = std::fs::canonicalize(target).map_err(|e| e.to_string())?;
    if own != target {
        return Err("生产更新目标必须是当前进程的可执行文件".into());
    }
    crate::update_net::reject_reparse_path(&target)?;
    auth(&update.signed_text)?;
    verify_package(&update.package_path, &update.manifest.package_sha256)?;
    let nonce = match nonce_override {
        Some(value) if value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit()) => {
            value.to_string()
        }
        Some(_) => return Err("交接 nonce 无效".into()),
        None => crate::update_net::random_token()?,
    };
    let worker = update.scratch.join("worker.exe");
    let worker_exe = match worker_override {
        // 测试替身仅替换最终执行的程序；生产路径始终复制自身并执行已验证副本。
        Some(double) => double.to_path_buf(),
        None => {
            copy_new(&own, &worker)?;
            worker
        }
    };
    let request = Request {
        signed_text: update.signed_text.clone(),
        target: target.clone(),
        package: update.package_path.clone(),
        old_version: update.old_version.clone(),
        old_sha256: crate::update_net::sha256_file(&own, MAX_PACKAGE_BYTES)?,
        parent: std::process::id(),
        nonce: nonce.clone(),
        handoff: handoff_dir.map(|dir| dir.to_path_buf()),
    };
    let request_path = update.scratch.join("request.json");
    write_new(
        &request_path,
        &serde_json::to_vec(&request).map_err(|e| e.to_string())?,
    )?;
    let mut worker_command = normal_command(&worker_exe, &target)?;
    let mut child = worker_command
        .arg("--quicknest-update-worker")
        .arg(&request_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let ready = update.scratch.join("ready");
    let start = Instant::now();
    loop {
        if std::fs::read_to_string(&ready).ok().as_deref() == Some(&request.nonce) {
            update.transferred = true;
            return Ok(child);
        }
        if let Some(code) = child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!(
                "worker 未就绪即退出 {code}；{}",
                std::fs::read_to_string(update.scratch.join("outcome.json")).unwrap_or_default()
            ));
        }
        if start.elapsed() > Duration::from_secs(10) {
            child
                .kill()
                .map_err(|e| format!("worker 就绪超时且终止失败：{e}"))?;
            let _ = child.wait();
            return Err("worker 就绪超时；主进程不得退出".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub(super) fn entry(args: &[String]) -> Option<Result<(), String>> {
    match args.get(1).map(String::as_str) {
        Some("--quicknest-update-health") => Some((|| {
            if args.len() != 4
                || args[2] != env!("CARGO_PKG_VERSION")
                || args[3].len() != 32
                || !args[3].bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("healthcheck 版本或 nonce 无效".into());
            }
            println!(
                "QuickNestHealth\t{}\t{}",
                env!("CARGO_PKG_VERSION"),
                args[3]
            );
            Ok(())
        })()),
        Some("--quicknest-update-worker") => Some((|| {
            if args.len() != 3 {
                return Err("worker 参数无效".into());
            }
            let request_path = PathBuf::from(&args[2]);
            let result = run(&request_path);
            // Durable true outcome. Rollback failure includes the retained backup.
            let outcome_written = (|| {
                let dir = request_path.parent().ok_or("缺少 worker 目录")?;
                let record = match &result {
                    Ok(detail) => serde_json::json!({"status":"updated", "detail":detail}),
                    Err(error) => serde_json::json!({"status":"failed", "detail":error}),
                };
                write_new(
                    &dir.join("outcome.json"),
                    &serde_json::to_vec(&record).map_err(|e| e.to_string())?,
                )
            })();
            // C2：不可恢复失败且父进程已退出时，尽力把 failed 标记写入交接目录。
            // 不做任何 UI 或消息框；失败与回滚位置的交付展示由主程序读取交接记录负责。
            if let Err(error) = &result {
                deliver_failure_marker(&request_path, error);
            }
            outcome_written?;
            result.map(|_| ())
        })()),
        _ => None,
    }
}
fn run(path: &Path) -> Result<String, String> {
    validate_data_root(std::env::var_os("QUICKNEST_DATA_DIR").as_deref())?;
    crate::update_net::reject_reparse_path(path)?;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() > 100_000 {
        return Err("worker 请求过大".into());
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let manifest = authenticate(&request.signed_text, release_key()?)?;
    if manifest.package_kind != "raw" {
        return Err("worker 仅支持 raw；setup/portablezip 被明确拒绝".into());
    }
    if request.old_version != env!("CARGO_PKG_VERSION")
        || !is_newer_version(&request.old_version, &manifest.version)
    {
        return Err("worker 版本或更新方向无效".into());
    }
    if request.nonce.len() != 32 || !request.nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("worker nonce 无效".into());
    }
    let dir = path.parent().ok_or("缺少 worker 目录")?;
    if request.package != dir.join("package.bin") {
        return Err("包路径必须属于 worker 独占目录".into());
    }
    crate::update_net::reject_reparse_path(&request.target)?;
    verify_package(&request.target, &request.old_sha256)?;
    verify_package(&request.package, &manifest.package_sha256)?;
    if request.old_sha256 == manifest.package_sha256 {
        return Err("新旧程序相同，拒绝虚假版本升级".into());
    }
    // Open a kernel handle BEFORE readiness; PID reuse cannot affect this wait.
    await_parent(request.parent, 120_000, || {
        write_new(&dir.join("ready"), request.nonce.as_bytes())
    })?;
    let outcome = apply_raw(
        &request.target,
        &request.package,
        &manifest.package_sha256,
        &request.old_sha256,
        |p| health(p, &request.old_version, &request.nonce),
        |p| health(p, &manifest.version, &request.nonce),
    );
    finish_apply(
        outcome,
        &manifest.version,
        || {
            relaunch(
                &request.target,
                &manifest.package_sha256,
                &manifest.version,
                &request.nonce,
            )
        },
        || {
            relaunch(
                &request.target,
                &request.old_sha256,
                &request.old_version,
                &request.nonce,
            )
        },
    )
}
// Call only the branch whose bytes/version have been established. A failed
// new-app launch does not silently start an old backup or change disk state.
fn finish_apply(
    outcome: ApplyOutcome,
    new_version: &str,
    start_new: impl FnOnce() -> Result<u32, String>,
    start_old: impl FnOnce() -> Result<u32, String>,
) -> Result<String, String> {
    match outcome {
        ApplyOutcome::Updated { placed_sha256 } => {
            let pid = start_new().map_err(|e| format!("更新已放置，但新版本重新启动失败：{e}"))?;
            Ok(format!(
                "version={new_version} sha256={placed_sha256} relaunched_pid={pid}; startup observed, UI readiness not confirmed"
            ))
        }
        ApplyOutcome::RolledBack { reason } => match start_old() {
            Ok(pid) => Err(format!(
                "更新失败；旧程序已恢复并重新启动 pid={pid}：{reason}"
            )),
            Err(error) => Err(format!(
                "更新失败；旧程序已恢复，但重新启动失败：{error}；原始原因：{reason}"
            )),
        },
        ApplyOutcome::RollbackFailed { reason, backup } => Err(format!(
            "ROLLBACK_FAILED：{reason}；未启动程序；请从 {} 手动恢复",
            backup.display()
        )),
    }
}
/// Best-effort failure delivery: every error here is silently skipped, since
/// outcome.json stays authoritative and the parent handoff record drives the
/// user-visible delivery. Only when the parent process is already gone and the
/// request carries a validated handoff directory is a failed marker written
/// (create_new; never overwrites an existing marker).
fn deliver_failure_marker(request_path: &Path, error: &str) {
    let Ok(bytes) = std::fs::read(request_path) else {
        return;
    };
    if bytes.len() > 100_000 {
        return;
    }
    let Ok(request) = serde_json::from_slice::<Request>(&bytes) else {
        return;
    };
    if request.nonce.len() != 32 || !request.nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
        return;
    }
    let Some(handoff) = &request.handoff else {
        return;
    };
    if parent_alive(request.parent) {
        // 父进程仍在：由其就绪等待与退出路径读取 outcome，不重复标记。
        return;
    }
    if !handoff.is_absolute() || !handoff.is_dir() {
        return;
    }
    if crate::update_net::reject_reparse_path(handoff).is_err() {
        return;
    }
    let record = serde_json::json!({"status":"failed","detail":error,"nonce":request.nonce});
    let Ok(bytes) = serde_json::to_vec(&record) else {
        return;
    };
    let _ = write_new(&handoff.join("failed.json"), &bytes);
}
fn parent_alive(pid: u32) -> bool {
    unsafe {
        OpenProcess(PROCESS_SYNCHRONIZE, false, pid)
            .map(|handle| {
                let _ = CloseHandle(handle);
                true
            })
            .unwrap_or(false)
    }
}
fn validate_data_root(value: Option<&std::ffi::OsStr>) -> Result<(), String> {
    if let Some(value) = value
        && (value.is_empty() || !Path::new(value).is_absolute())
    {
        return Err("QUICKNEST_DATA_DIR 必须为非空绝对路径；worker 拒绝相对数据根".into());
    }
    Ok(())
}
fn normal_command(executable: &Path, target: &Path) -> Result<Command, String> {
    validate_data_root(std::env::var_os("QUICKNEST_DATA_DIR").as_deref())?;
    let cwd = target.parent().ok_or("重新启动目标缺少工作目录")?;
    let mut command = Command::new(executable);
    // Inherit the complete worker environment, including data root and instance
    // namespace. No health/worker args are carried into normal application mode.
    command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(command)
}
fn relaunch(target: &Path, expected_hash: &str, version: &str, nonce: &str) -> Result<u32, String> {
    verify_package(target, expected_hash)?;
    health(target, version, nonce)?;
    start_observed(
        &mut normal_command(target, target)?,
        Duration::from_millis(750),
    )
    .map(|child| child.id())
}
fn start_observed(command: &mut Command, observation: Duration) -> Result<Child, String> {
    let mut child = command
        .spawn()
        .map_err(|e| format!("创建子进程失败：{e}"))?;
    let pid = child.id();
    let started = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("读取子进程启动状态失败 pid={pid}：{e}"))?
        {
            return Err(format!("子进程启动后提前退出 pid={pid} status={status}"));
        }
        if started.elapsed() >= observation {
            return Ok(child);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn await_parent(
    pid: u32,
    timeout_ms: u32,
    ready: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let parent = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }
        .map_err(|e| format!("无法打开父进程：{e}"))?;
    if let Err(error) = ready() {
        unsafe {
            let _ = CloseHandle(parent);
        }
        return Err(error);
    }
    let wait = unsafe { WaitForSingleObject(parent, timeout_ms) };
    unsafe {
        let _ = CloseHandle(parent);
    }
    if wait != WAIT_OBJECT_0 {
        return Err("父进程未在时限内退出；未修改程序".into());
    }
    Ok(())
}
fn health(path: &Path, version: &str, nonce: &str) -> Result<(), String> {
    let mut child = Command::new(path)
        .arg("--quicknest-update-health")
        .arg(version)
        .arg(nonce)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("healthcheck 无法启动：{e}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(10) {
            child
                .kill()
                .map_err(|e| format!("healthcheck 超时且无法终止：{e}"))?;
            child.wait().map_err(|e| e.to_string())?;
            return Err("healthcheck 超时".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let mut output = String::new();
    child
        .stdout
        .take()
        .ok_or("缺少 healthcheck 输出")?
        .take(1024)
        .read_to_string(&mut output)
        .map_err(|e| e.to_string())?;
    if !status.success() || output.trim_end() != format!("QuickNestHealth\t{version}\t{nonce}") {
        return Err(format!("healthcheck 未确认版本 {version}"));
    }
    Ok(())
}
fn wide_path(path: &Path) -> Result<Vec<u16>, String> {
    use std::os::windows::ffi::OsStrExt;
    // ReplaceFileW does not opt into Win32 long paths automatically. Canonicalize
    // the parent (backup need not exist) to retain the extended-length prefix.
    let absolute = std::fs::canonicalize(path.parent().ok_or("路径缺少目录")?)
        .map_err(|e| e.to_string())?
        .join(path.file_name().ok_or("路径缺少文件名")?);
    let mut w: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    if w.contains(&0) {
        return Err("路径含 NUL".into());
    }
    w.push(0);
    Ok(w)
}
fn replace(target: &Path, new: &Path, backup: Option<&Path>) -> Result<(), String> {
    crate::update_net::reject_reparse_path(target)?;
    crate::update_net::reject_reparse_path(new)?;
    let t = wide_path(target)?;
    let n = wide_path(new)?;
    let b = backup.map(wide_path).transpose()?;
    unsafe {
        ReplaceFileW(
            PCWSTR(t.as_ptr()),
            PCWSTR(n.as_ptr()),
            PCWSTR(b.as_ref().map_or(std::ptr::null(), |b| b.as_ptr())),
            REPLACE_FILE_FLAGS(0),
            None,
            None,
        )
        .map_err(|e| e.to_string())
    }
}
/// Only target executable and random siblings are written. No directory tree,
/// data migration, installer invocation, ZIP extraction, or recursive cleanup.
pub(super) fn apply_raw(
    target: &Path,
    downloaded: &Path,
    expected: &str,
    old_hash: &str,
    old_health: impl Fn(&Path) -> Result<(), String>,
    new_health: impl Fn(&Path) -> Result<(), String>,
) -> ApplyOutcome {
    let parent = match target.parent() {
        Some(p) => p,
        None => {
            return ApplyOutcome::RolledBack {
                reason: "目标缺少目录".into(),
            };
        }
    };
    let token = match crate::update_net::random_token() {
        Ok(t) => t,
        Err(reason) => return ApplyOutcome::RolledBack { reason },
    };
    let staged = parent.join(format!(".quicknest-{token}.new.exe"));
    let backup = parent.join(format!(".quicknest-{token}.old.exe"));
    let prepare = (|| {
        verify_package(target, old_hash)?;
        verify_package(downloaded, expected)?;
        if old_hash == expected {
            return Err("新旧程序相同".into());
        }
        old_health(target)?;
        copy_new(downloaded, &staged)?;
        verify_package(&staged, expected)?;
        if backup.exists() {
            return Err("备份路径已存在".into());
        }
        replace(target, &staged, Some(&backup))
    })();
    if let Err(reason) = prepare {
        let _ = std::fs::remove_file(&staged);
        // ReplaceFile can fail after creating/moving the backup. Inspect state,
        // never falsely claim that the original is still installed.
        if verify_package(target, old_hash).is_ok() {
            return ApplyOutcome::RolledBack { reason };
        }
        return restore(target, &backup, old_hash, reason);
    }
    match verify_package(target, expected).and_then(|hash| {
        new_health(target)?;
        Ok(hash)
    }) {
        Ok(placed_sha256) => ApplyOutcome::Updated { placed_sha256 },
        Err(reason) => restore(target, &backup, old_hash, reason),
    }
}
fn restore(target: &Path, backup: &Path, old_hash: &str, reason: String) -> ApplyOutcome {
    let result = (|| {
        verify_package(backup, old_hash)?;
        if target.exists() {
            replace(target, backup, None)?;
        } else {
            std::fs::rename(backup, target).map_err(|e| e.to_string())?;
        }
        verify_package(target, old_hash)?;
        Ok::<_, String>(())
    })();
    match result {
        Ok(()) => ApplyOutcome::RolledBack { reason },
        Err(e) => ApplyOutcome::RollbackFailed {
            reason: format!("{reason}；恢复失败：{e}"),
            backup: backup.into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf, PathBuf, String, String) {
        let dir = crate::update_net::private_temp_dir().unwrap();
        let old = dir.join("app.exe");
        let new = dir.join("download.bin");
        std::fs::write(&old, b"old-v1").unwrap();
        std::fs::write(&new, b"new-v2").unwrap();
        let old_hash = crate::update_net::sha256_file(&old, MAX_PACKAGE_BYTES).unwrap();
        let new_hash = crate::update_net::sha256_file(&new, MAX_PACKAGE_BYTES).unwrap();
        (dir, old, new, old_hash, new_hash)
    }
    fn cleanup(dir: &Path) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_file() {
                std::fs::remove_file(e.path()).unwrap();
            }
        }
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn relaunch_selects_only_verified_outcome_branch() {
        let starts = std::cell::RefCell::new(Vec::new());
        let updated = finish_apply(
            ApplyOutcome::Updated {
                placed_sha256: "newhash".into(),
            },
            "1.2.1",
            || {
                starts.borrow_mut().push("new");
                Ok(41)
            },
            || panic!("must not launch old after success"),
        )
        .unwrap();
        assert!(updated.contains("relaunched_pid=41"));
        let rolled_back = finish_apply(
            ApplyOutcome::RolledBack {
                reason: "health failed".into(),
            },
            "1.2.1",
            || panic!("must not launch new after rollback"),
            || {
                starts.borrow_mut().push("old");
                Ok(42)
            },
        )
        .unwrap_err();
        assert!(rolled_back.contains("pid=42"));
        let failed = finish_apply(
            ApplyOutcome::RollbackFailed {
                reason: "restore failed".into(),
                backup: "backup.exe".into(),
            },
            "1.2.1",
            || panic!("must not launch after failed rollback"),
            || panic!("must not launch after failed rollback"),
        )
        .unwrap_err();
        assert!(failed.contains("ROLLBACK_FAILED"));
        assert_eq!(*starts.borrow(), vec!["new", "old"]);
        let launch_error = finish_apply(
            ApplyOutcome::Updated {
                placed_sha256: "newhash".into(),
            },
            "1.2.1",
            || Err("spawn denied".into()),
            || panic!("no fallback launch"),
        )
        .unwrap_err();
        assert!(launch_error.contains("spawn denied"));
        let rollback_launch_error = finish_apply(
            ApplyOutcome::RolledBack {
                reason: "health failed".into(),
            },
            "1.2.1",
            || panic!("no new launch"),
            || Err("old spawn denied".into()),
        )
        .unwrap_err();
        assert!(rollback_launch_error.contains("old spawn denied"));
    }
    #[test]
    fn normal_relaunch_preserves_env_target_cwd_and_has_no_worker_args() {
        let dir = crate::update_net::private_temp_dir().unwrap();
        let marker = dir.join("launch.json");
        let target = dir.join("quicknest.exe");
        let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut command = normal_command(&powershell, &target).unwrap();
        assert_eq!(command.get_current_dir(), Some(dir.as_path()));
        assert_eq!(command.get_args().count(), 0);
        let data_root = dir.join("preserved-data");
        command.env("QUICKNEST_DATA_DIR", &data_root)
            .env("QUICKNEST_INSTANCE_NAMESPACE", "isolated-relaunch-fixture")
            .env("QUICKNEST_RELAUNCH_MARKER", &marker)
            .args(["-NoProfile", "-NonInteractive", "-Command", "$result = @{ cwd = [Environment]::CurrentDirectory; data = $env:QUICKNEST_DATA_DIR; namespace = $env:QUICKNEST_INSTANCE_NAMESPACE }; [IO.File]::WriteAllText($env:QUICKNEST_RELAUNCH_MARKER, ($result | ConvertTo-Json -Compress)); Start-Sleep -Milliseconds 1200"]);
        let mut child = start_observed(&mut command, Duration::from_millis(750)).unwrap();
        assert_ne!(child.id(), 0);
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("fixture did not exit in time");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
        assert_eq!(value["data"], data_root.to_string_lossy().as_ref());
        assert_eq!(value["namespace"], "isolated-relaunch-fixture");
        assert_eq!(
            std::fs::canonicalize(value["cwd"].as_str().unwrap()).unwrap(),
            std::fs::canonicalize(&dir).unwrap()
        );
        assert!(
            !data_root.exists(),
            "probe must not initialize any data root"
        );
        for bad in ["", "relative-data", r"C:drive-relative", r"\root-relative"] {
            assert!(validate_data_root(Some(std::ffi::OsStr::new(bad))).is_err());
        }
        assert!(validate_data_root(None).is_ok());
        assert!(validate_data_root(Some(data_root.as_os_str())).is_ok());
        cleanup(&dir);
    }
    #[test]
    fn relaunch_reports_spawn_failure_and_immediate_exit() {
        let dir = crate::update_net::private_temp_dir().unwrap();
        let target = dir.join("missing.exe");
        assert!(
            start_observed(
                &mut normal_command(&target, &target).unwrap(),
                Duration::from_millis(750)
            )
            .unwrap_err()
            .contains("创建子进程失败")
        );
        let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut command = normal_command(&powershell, &target).unwrap();
        command.args(["-NoProfile", "-NonInteractive", "-Command", "exit 7"]);
        assert!(
            start_observed(&mut command, Duration::from_secs(2))
                .unwrap_err()
                .contains("提前退出")
        );
        cleanup(&dir);
    }
    #[test]
    fn ack_requires_open_parent_and_timeout_does_not_apply() {
        let ack = std::cell::Cell::new(false);
        assert!(
            await_parent(0, 0, || {
                ack.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(
            !ack.get(),
            "ACK must follow successful parent handle acquisition"
        );
        assert!(
            await_parent(std::process::id(), 1, || {
                ack.set(true);
                Ok(())
            })
            .unwrap_err()
            .contains("未修改")
        );
        assert!(ack.get());
    }
    #[test]
    fn health_dispatch_requires_exact_compiled_version() {
        let args = vec![
            "app".into(),
            "--quicknest-update-health".into(),
            "99.0.0".into(),
            "a".repeat(32),
        ];
        assert!(entry(&args).unwrap().is_err());
        let args = vec![
            "app".into(),
            "--quicknest-update-health".into(),
            env!("CARGO_PKG_VERSION").into(),
            "a".repeat(32),
        ];
        assert!(entry(&args).unwrap().is_ok());
    }
    #[test]
    fn atomic_apply_preserves_old_backup_and_data() {
        let (dir, old, new, old_hash, new_hash) = fixture();
        let data = dir.join("data");
        std::fs::create_dir(&data).unwrap();
        std::fs::write(data.join("sentinel"), b"unchanged").unwrap();
        let outcome = apply_raw(&old, &new, &new_hash, &old_hash, |_| Ok(()), |_| Ok(()));
        assert!(
            matches!(outcome, ApplyOutcome::Updated { .. }),
            "{outcome:?}"
        );
        assert_eq!(std::fs::read(&old).unwrap(), b"new-v2");
        let backups: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".old.exe"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read(backups[0].path()).unwrap(), b"old-v1");
        assert_eq!(std::fs::read(data.join("sentinel")).unwrap(), b"unchanged");
        std::fs::remove_file(data.join("sentinel")).unwrap();
        std::fs::remove_dir(data).unwrap();
        cleanup(&dir);
    }
    #[test]
    fn health_failure_actually_restores_old_version() {
        let (dir, old, new, old_hash, new_hash) = fixture();
        assert!(matches!(
            apply_raw(
                &old,
                &new,
                &new_hash,
                &old_hash,
                |_| Ok(()),
                |_| Err("wrong version".into())
            ),
            ApplyOutcome::RolledBack { .. }
        ));
        assert_eq!(std::fs::read(&old).unwrap(), b"old-v1");
        cleanup(&dir);
    }
    #[test]
    fn corrupt_input_never_reaches_health_or_placement() {
        let (dir, old, new, old_hash, new_hash) = fixture();
        std::fs::write(&new, b"corrupt").unwrap();
        assert!(matches!(
            apply_raw(
                &old,
                &new,
                &new_hash,
                &old_hash,
                |_| panic!("must reject first"),
                |_| panic!("must reject first")
            ),
            ApplyOutcome::RolledBack { .. }
        ));
        assert_eq!(std::fs::read(&old).unwrap(), b"old-v1");
        cleanup(&dir);
    }
    #[test]
    fn rollback_failure_is_explicit() {
        let (dir, old, new, old_hash, new_hash) = fixture();
        let result = apply_raw(
            &old,
            &new,
            &new_hash,
            &old_hash,
            |_| Ok(()),
            |_| {
                // Destroy only this fixture's backup to inject a restoration failure.
                for e in std::fs::read_dir(&dir).unwrap().flatten() {
                    if e.file_name().to_string_lossy().ends_with(".old.exe") {
                        std::fs::remove_file(e.path()).unwrap();
                    }
                }
                Err("injected health failure".into())
            },
        );
        assert!(matches!(result, ApplyOutcome::RollbackFailed { .. }));
        cleanup(&dir);
    }
    fn marker_request(parent: u32, handoff: Option<PathBuf>, dir: &Path) -> Request {
        Request {
            signed_text: String::new(),
            target: dir.to_path_buf(),
            package: dir.join("package.bin"),
            old_version: "1.2.1".into(),
            old_sha256: "0".repeat(64),
            parent,
            nonce: "ab".repeat(16),
            handoff,
        }
    }
    #[test]
    fn failure_marker_only_when_parent_gone_and_handoff_valid() {
        let dir = crate::update_net::private_temp_dir().unwrap();
        let handoff = dir.join("handoff-marker");
        std::fs::create_dir(&handoff).unwrap();
        let request_path = dir.join("request.json");
        std::fs::write(
            &request_path,
            serde_json::to_vec(&marker_request(0, Some(handoff.clone()), &dir)).unwrap(),
        )
        .unwrap();
        // 父进程已不存在（pid 0 无法打开）→ 尽力写入 failed 标记
        deliver_failure_marker(&request_path, "注入的不可恢复失败");
        let marker = handoff.join("failed.json");
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
        assert_eq!(value["status"], "failed");
        assert_eq!(value["nonce"], "ab".repeat(16));
        assert_eq!(value["detail"], "注入的不可恢复失败");
        // create_new：重复投递不覆盖既有标记
        deliver_failure_marker(&request_path, "第二次");
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
        assert_eq!(value["detail"], "注入的不可恢复失败");
        // 父进程存活：不写标记（由父进程就绪等待/退出路径交付）
        let alive = dir.join("handoff-alive");
        std::fs::create_dir(&alive).unwrap();
        let request_path = dir.join("request-alive.json");
        std::fs::write(
            &request_path,
            serde_json::to_vec(&marker_request(
                std::process::id(),
                Some(alive.clone()),
                &dir,
            ))
            .unwrap(),
        )
        .unwrap();
        deliver_failure_marker(&request_path, "x");
        assert!(!alive.join("failed.json").exists());
        // 请求未携带交接目录：不写标记
        let none_dir = dir.join("handoff-none");
        std::fs::create_dir(&none_dir).unwrap();
        let request_path = dir.join("request-none.json");
        std::fs::write(
            &request_path,
            serde_json::to_vec(&marker_request(0, None, &dir)).unwrap(),
        )
        .unwrap();
        deliver_failure_marker(&request_path, "x");
        assert!(std::fs::read_dir(&none_dir).unwrap().next().is_none());
        // 交接目录不存在：尽力而为，不 panic、不创建
        let request_path = dir.join("request-missing.json");
        std::fs::write(
            &request_path,
            serde_json::to_vec(&marker_request(0, Some(dir.join("missing-dir")), &dir)).unwrap(),
        )
        .unwrap();
        deliver_failure_marker(&request_path, "x");
        assert!(!dir.join("missing-dir").exists());
        std::fs::remove_file(&marker).unwrap();
        std::fs::remove_dir(&handoff).unwrap();
        std::fs::remove_dir(&alive).unwrap();
        std::fs::remove_dir(&none_dir).unwrap();
        cleanup(&dir);
    }
    #[test]
    fn worker_request_handoff_field_is_optional_and_strict() {
        let base = serde_json::json!({
            "signed_text": "t",
            "target": "C:\\x\\app.exe",
            "package": "C:\\x\\package.bin",
            "old_version": "1.2.1",
            "old_sha256": "0".repeat(64),
            "parent": 1,
            "nonce": "ab".repeat(16)
        });
        let parsed: Request = serde_json::from_value(base.clone()).unwrap();
        assert!(parsed.handoff.is_none());
        let mut with = base.clone();
        with["handoff"] = serde_json::json!("C:\\handoff");
        let parsed: Request = serde_json::from_value(with).unwrap();
        assert_eq!(parsed.handoff.as_deref(), Some(Path::new("C:\\handoff")));
        let mut extra = base;
        extra["unknown"] = serde_json::json!(1);
        assert!(serde_json::from_value::<Request>(extra).is_err());
    }
}

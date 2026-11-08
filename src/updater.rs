//! Signed update checking. Only `VerifiedUpdate` can authorize a production worker.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

#[path = "updater_worker.rs"]
mod worker;

/// Release builds must inject their public BCrypt P-256 blob at BUILD TIME.
/// No checked-in development identity or runtime environment override is trusted.
pub const UPDATE_PUBLIC_KEY_HEX: &str = match option_env!("QUICKNEST_UPDATE_PUBLIC_KEY_HEX") {
    Some(key) => key,
    None => "",
};
const DISCLOSED_FIXTURE_KEY: &str = include_str!("update_key.pub");
pub const MAX_PACKAGE_BYTES: u64 = 200_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    Raw,
    Setup,
    Portablezip,
}
impl PackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Setup => "setup",
            Self::Portablezip => "portablezip",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateManifest {
    pub version: String,
    pub package_url: String,
    pub package_sha256: String,
    pub notes: String,
    pub package_kind: String,
    pub signature: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: String,
    signature: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    version: String,
    package_url: String,
    package_sha256: String,
    #[serde(default)]
    notes: String,
    package_kind: PackageKind,
}
fn envelope(text: &str) -> Result<Envelope, String> {
    if text.len() > 65_536 {
        return Err("清单超过 64 KiB 上限".into());
    }
    let envelope: Envelope =
        serde_json::from_str(text).map_err(|e| format!("清单格式无效：{e}"))?;
    if envelope.signature.len() != 128 || !envelope.signature.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("签名必须为 64 字节 ECDSA P-256 r||s hex".into());
    }
    Ok(envelope)
}
impl UpdateManifest {
    /// Syntax validation only. Trust is established exclusively by check_download.
    pub fn parse(text: &str) -> Result<Self, String> {
        let envelope = envelope(text)?;
        let p: Payload = serde_json::from_str(&envelope.payload)
            .map_err(|e| format!("payload 格式无效：{e}"))?;
        if parse_version(&p.version).is_none() {
            return Err("版本必须为规范 major.minor.patch".into());
        }
        crate::update_net::validate_https_url(&p.package_url)?;
        if p.package_sha256.len() != 64
            || !p
                .package_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("SHA-256 必须为 64 位小写 hex".into());
        }
        if p.notes.len() > 16_384 || p.notes.contains('\0') {
            return Err("发行说明无效".into());
        }
        Ok(Self {
            version: p.version,
            package_url: p.package_url,
            package_sha256: p.package_sha256,
            notes: p.notes,
            package_kind: p.package_kind.as_str().into(),
            signature: envelope.signature,
        })
    }
    pub fn payload_of(text: &str) -> Result<String, String> {
        Ok(envelope(text)?.payload)
    }
}

pub fn parse_version(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.split('.');
    let mut next = || {
        let part = parts.next()?;
        if part.is_empty()
            || !part.bytes().all(|b| b.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        part.parse::<u32>().ok()
    };
    let result = (next()?, next()?, next()?);
    if parts.next().is_some() {
        None
    } else {
        Some(result)
    }
}
pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    matches!((parse_version(current), parse_version(candidate)), (Some(a), Some(b)) if b > a)
}
fn release_key() -> Result<&'static str, String> {
    if UPDATE_PUBLIC_KEY_HEX.is_empty()
        || UPDATE_PUBLIC_KEY_HEX
            .trim()
            .eq_ignore_ascii_case(DISCLOSED_FIXTURE_KEY.trim())
    {
        return Err("此构建未配置独立发布公钥；已拒绝更新（仓库测试密钥不可信）".into());
    }
    Ok(UPDATE_PUBLIC_KEY_HEX)
}
pub(crate) fn authenticate(text: &str, key: &str) -> Result<UpdateManifest, String> {
    let manifest = UpdateManifest::parse(text)?;
    if !crate::update_net::ecdsa_p256_verify(
        key,
        UpdateManifest::payload_of(text)?.as_bytes(),
        &manifest.signature,
    ) {
        return Err("更新清单签名验证失败".into());
    }
    Ok(manifest)
}

/// Private fields prevent callers/UI from assembling an authorization from strings.
/// Dropping an unconsumed update cleans only its exclusively-created scratch files.
#[derive(Debug)]
pub struct VerifiedUpdate {
    manifest: UpdateManifest,
    signed_text: String,
    package_path: PathBuf,
    scratch: PathBuf,
    old_version: String,
    transferred: bool,
}
impl VerifiedUpdate {
    pub fn version(&self) -> &str {
        &self.manifest.version
    }
    pub fn notes(&self) -> &str {
        &self.manifest.notes
    }
    pub fn package_kind(&self) -> PackageKind {
        match self.manifest.package_kind.as_str() {
            "raw" => PackageKind::Raw,
            "setup" => PackageKind::Setup,
            _ => PackageKind::Portablezip,
        }
    }
    pub fn package_path(&self) -> &Path {
        &self.package_path
    }
    pub fn spawn_worker(self, target: &Path) -> Result<std::process::Child, String> {
        worker::spawn(self, target)
    }
}

/// C1：把 worker 复制、请求写入、进程启动与最长 10 秒就绪等待全部移入一次性
/// 后台线程，调用线程（UI）只阻塞在通道接收上。
///
/// 取消语义：丢弃返回的 Receiver 不会终止线程——准备工作继续运行至自然结束
/// （成功则交接完成、失败则按 VerifiedUpdate::drop 清理独占 scratch），只是结果
/// 无人接收；不做部分中断，也不改变任何落盘状态。
pub fn prepare_in_background(
    update: VerifiedUpdate,
    target: &Path,
) -> std::sync::mpsc::Receiver<Result<std::process::Child, String>> {
    background_prepare(update, target, |update, target| {
        worker::spawn(update, target)
    })
}

/// C2：后台准备并在成功 spawn 之前把交接记录写入 `handoff_dir` 下的专用子目录
/// （handoff-{handoff_id}/handoff.json）。失败（含登记失败）同样经通道回传；
/// 记录先行，即使准备失败，下次启动仍能加载记录、核对无剩余自有文件并确认。
/// `handoff_id` 必须来自 `new_handoff_id`（32 位 hex）。
pub fn prepare_in_background_with_handoff(
    update: VerifiedUpdate,
    target: &Path,
    handoff_dir: &Path,
    handoff_id: &str,
) -> std::sync::mpsc::Receiver<Result<std::process::Child, String>> {
    let handoff_dir = handoff_dir.to_path_buf();
    let handoff_id = handoff_id.to_string();
    background_prepare(update, target, move |update, target| {
        let nonce = crate::update_net::random_token()?;
        let receipt = HandoffReceipt {
            handoff_id: handoff_id.clone(),
            nonce: nonce.clone(),
            scratch: update.scratch.clone(),
            target: target.to_path_buf(),
            worker_path: update.scratch.join("worker.exe"),
            files: [
                "package.bin",
                "worker.exe",
                "request.json",
                "ready",
                "outcome.json",
            ]
            .into_iter()
            .map(|name| update.scratch.join(name))
            .collect(),
        };
        record_handoff(&handoff_dir, receipt).map_err(|e| e.to_string())?;
        let subdir = handoff_dir.join(format!("handoff-{}", handoff_id));
        worker::spawn_with(update, target, None, Some(&subdir), Some(&nonce), |text| {
            authenticate(text, release_key()?)
        })
    })
}

fn background_prepare(
    update: VerifiedUpdate,
    target: &Path,
    prepare: impl FnOnce(VerifiedUpdate, &Path) -> Result<std::process::Child, String> + Send + 'static,
) -> std::sync::mpsc::Receiver<Result<std::process::Child, String>> {
    let target = target.to_path_buf();
    let (sender, receiver) = std::sync::mpsc::channel();
    let thread_sender = sender.clone();
    let spawned = std::thread::Builder::new()
        .name("quicknest-update-prepare".into())
        .spawn(move || {
            let _ = thread_sender.send(prepare(update, &target));
        });
    if spawned.is_err() {
        // 线程闭包已被丢弃：VerifiedUpdate::drop 清理了独占 scratch，随后回传失败。
        let _ = sender.send(Err("更新准备线程无法启动；未开始更新".into()));
    }
    receiver
}

#[cfg(test)]
pub(crate) fn prepare_in_background_with_double(
    update: VerifiedUpdate,
    target: &Path,
    worker_double: &Path,
) -> std::sync::mpsc::Receiver<Result<std::process::Child, String>> {
    let worker_double = worker_double.to_path_buf();
    background_prepare(update, target, move |update, target| {
        worker::spawn_for_test(update, target, Some(&worker_double), None, None)
    })
}
impl Drop for VerifiedUpdate {
    fn drop(&mut self) {
        if !self.transferred {
            let _ = std::fs::remove_file(&self.package_path);
            let _ = std::fs::remove_dir(&self.scratch);
        }
    }
}

/// `manifest_url` is the full HTTPS manifest URL. None means current/newer.
/// Network operations are cancellable between bounded (5 second) I/O operations.
/// The package is streamed to an exclusive random OS-temp file, never app data.
pub fn check_download(
    manifest_url: &str,
    current_version: &str,
    cancel: &AtomicBool,
) -> Result<Option<VerifiedUpdate>, String> {
    let key = release_key()?;
    let text = crate::update_net::https_get_text_cancel(manifest_url, cancel)?;
    checked_download(&text, current_version, key, cancel)
}
#[cfg(test)]
pub(crate) fn test_checked_download(
    text: &str,
    current: &str,
    key: &str,
) -> Result<Option<VerifiedUpdate>, String> {
    checked_download(text, current, key, &AtomicBool::new(false))
}
fn checked_download(
    text: &str,
    current: &str,
    key: &str,
    cancel: &AtomicBool,
) -> Result<Option<VerifiedUpdate>, String> {
    if parse_version(current).is_none() {
        return Err("当前版本无效".into());
    }
    let manifest = authenticate(text, key)?;
    if !is_newer_version(current, &manifest.version) {
        return Ok(None);
    }
    if manifest.package_kind != "raw" {
        return Err(format!(
            "此版本尚不支持 {} 更新包，已在下载前拒绝",
            manifest.package_kind
        ));
    }
    let scratch = crate::update_net::private_temp_dir()?;
    let mut update = VerifiedUpdate {
        manifest,
        signed_text: text.into(),
        package_path: scratch.join("package.bin"),
        scratch,
        old_version: current.into(),
        transferred: false,
    };
    crate::update_net::https_get_file_cancel(
        &update.manifest.package_url,
        &update.package_path,
        MAX_PACKAGE_BYTES,
        cancel,
    )?;
    verify_package(&update.package_path, &update.manifest.package_sha256)?;
    // Keep the original signed bytes for worker re-authentication across processes.
    update.transferred = false;
    Ok(Some(update))
}
pub(crate) fn verify_package(path: &Path, expected: &str) -> Result<String, String> {
    crate::update_net::reject_reparse_path(path)?;
    let actual = crate::update_net::sha256_file(path, MAX_PACKAGE_BYTES)?;
    if actual != expected {
        return Err("下载包 SHA-256 哈希不符，已拒绝应用".into());
    }
    Ok(actual)
}

/// Register BEFORE configuration/data/UI initialization. The worker and health
/// probe must never call portable::init or open the user's data directory.
/// Args include argv[0]. Some(result) means the caller must exit with this result.
pub fn worker_entry(args: &[String]) -> Option<Result<(), String>> {
    worker::entry(args)
}

/// C2/C3：更新交接记录。父进程在 spawn 前登记、下次启动加载消费、确认后清除；
/// 清理只触碰本记录登记、且经 canonicalize + reparse 校验确认位于独占 scratch
/// 内的自有文件。不按名称或前缀批量清理，不触碰未知文件与回滚备份
/// （备份位于 target 目录，天然不在 scratch 内）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffReceipt {
    pub handoff_id: String,
    pub nonce: String,
    pub scratch: PathBuf,
    pub target: PathBuf,
    pub worker_path: PathBuf,
    pub files: Vec<PathBuf>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeReport {
    pub status: String,
    pub detail: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedOutcome {
    status: String,
    detail: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureMarker {
    status: String,
    detail: String,
    nonce: String,
}
fn valid_handoff_token(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn invalid_data(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}
fn validate_receipt(receipt: &HandoffReceipt) -> std::io::Result<()> {
    if !valid_handoff_token(&receipt.handoff_id) {
        return Err(invalid_data("handoff_id 必须为 32 位 hex"));
    }
    if !valid_handoff_token(&receipt.nonce) {
        return Err(invalid_data("nonce 必须为 32 位 hex"));
    }
    if !receipt.scratch.is_absolute()
        || !receipt.target.is_absolute()
        || !receipt.worker_path.is_absolute()
    {
        return Err(invalid_data("交接路径必须为绝对路径"));
    }
    for file in &receipt.files {
        if !file.is_absolute() {
            return Err(invalid_data("登记文件必须为绝对路径"));
        }
        if file.parent() != Some(receipt.scratch.as_path()) {
            return Err(invalid_data("登记文件必须是 scratch 的直接子路径"));
        }
    }
    Ok(())
}
/// 生成可安全用作交接子目录名的随机 id（32 位 hex）。
pub fn new_handoff_id() -> Result<String, String> {
    crate::update_net::random_token()
}
/// 在 `dir/handoff-{id}` 专用子目录内以 create_new 写 handoff.json，返回其路径。
/// 重复登记同一 id 返回 AlreadyExists 错误，不覆盖任何现有记录。
pub fn record_handoff(dir: &Path, receipt: HandoffReceipt) -> std::io::Result<PathBuf> {
    validate_receipt(&receipt)?;
    let subdir = dir.join(format!("handoff-{}", receipt.handoff_id));
    std::fs::create_dir(&subdir)?;
    let json = subdir.join("handoff.json");
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|e| invalid_data(format!("交接记录序列化失败：{e}")))?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&json)?
        .write_all(&bytes)?;
    Ok(json)
}
/// 加载 `dir` 下仍待消费的交接记录。缺目录、无关条目或损坏/越界记录被逐条
/// 跳过（不阻断其余记录），结果按 handoff_id 排序保证确定性。
pub fn load_pending_handoffs(dir: &Path) -> Vec<HandoffReceipt> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut receipts = Vec::new();
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        let json = entry.path().join("handoff.json");
        let Ok(bytes) = std::fs::read(&json) else {
            continue;
        };
        if bytes.len() > 65_536 {
            continue;
        }
        let Ok(receipt) = serde_json::from_slice::<HandoffReceipt>(&bytes) else {
            continue;
        };
        if validate_receipt(&receipt).is_err() {
            continue;
        }
        receipts.push(receipt);
    }
    receipts.sort_by(|a, b| a.handoff_id.cmp(&b.handoff_id));
    receipts
}
/// 确认交接已消费：移除 handoff.json 与已知 failed.json 标记后移除子目录。
/// 目录仍含未知文件时拒绝移除并报错；记录不存在时幂等返回成功。
pub fn acknowledge(dir: &Path, handoff_id: &str) -> std::io::Result<()> {
    if !valid_handoff_token(handoff_id) {
        return Err(invalid_data("handoff_id 必须为 32 位 hex"));
    }
    let subdir = dir.join(format!("handoff-{}", handoff_id));
    if !subdir.exists() {
        return Ok(());
    }
    for name in ["handoff.json", "failed.json"] {
        match std::fs::remove_file(subdir.join(name)) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
    }
    std::fs::remove_dir(&subdir)
}
/// C3：受限清理。`worker_exited == false` 时不删除任何文件并直接报错（worker.exe
/// 与全部登记文件保留）。已退出时逐条校验后仅删除登记文件：必须位于 receipt.scratch
/// 内（canonicalize 后为其直接子路径）、拒绝 reparse、必须是常规文件；未通过校验的
/// 登记条目一律跳过（整单合法性由 record/load 阶段负责，清理以逐条 canonical 校验
/// 为权威安全网）。未知文件与回滚备份永不清除；scratch 仅在因此变空时移除；
/// scratch 已不存在视为无剩余自有文件并成功返回。
pub fn cleanup_finished_owned(
    receipt: &HandoffReceipt,
    worker_exited: bool,
) -> std::io::Result<()> {
    if !worker_exited {
        return Err(std::io::Error::other(
            "worker 未确认退出；保留全部登记自有文件，不执行清理",
        ));
    }
    crate::update_net::reject_reparse_path(&receipt.scratch)
        .map_err(|e| invalid_data(format!("scratch 路径可疑：{e}")))?;
    let scratch = match std::fs::canonicalize(&receipt.scratch) {
        Ok(path) => path,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let mut first_error = None;
    for file in &receipt.files {
        if crate::update_net::reject_reparse_path(file).is_err() {
            continue;
        }
        let Ok(canonical) = std::fs::canonicalize(file) else {
            continue;
        };
        if canonical.parent() != Some(scratch.as_path()) {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&canonical) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if let Err(e) = std::fs::remove_file(&canonical)
            && first_error.is_none()
        {
            first_error = Some(e);
        }
    }
    if let Some(e) = first_error {
        return Err(e);
    }
    // 仅当登记文件清理后目录已空才会成功；未知文件保留在原地。
    let _ = std::fs::remove_dir(&scratch);
    Ok(())
}
/// C2：正常启动消费 worker 结果。outcome.json 由 worker 在独立进程中延迟写出；
/// 尚不存在、不完整或未通过严格校验（status 限定 updated/failed、拒绝未知字段、
/// 上限 64 KiB/16 KiB、必须为 scratch 直接子路径且非 reparse）时返回 None。
pub fn poll_outcome(receipt: &HandoffReceipt) -> Option<OutcomeReport> {
    validate_receipt(receipt).ok()?;
    let outcome = receipt.scratch.join("outcome.json");
    crate::update_net::reject_reparse_path(&outcome).ok()?;
    let scratch = std::fs::canonicalize(&receipt.scratch).ok()?;
    let canonical = std::fs::canonicalize(&outcome).ok()?;
    if canonical.parent() != Some(scratch.as_path()) {
        return None;
    }
    let meta = std::fs::symlink_metadata(&canonical).ok()?;
    if !meta.is_file() || meta.len() > 65_536 {
        return None;
    }
    let bytes = std::fs::read(&canonical).ok()?;
    if bytes.len() > 65_536 {
        return None;
    }
    let record: RecordedOutcome = serde_json::from_slice(&bytes).ok()?;
    if !matches!(record.status.as_str(), "updated" | "failed") || record.detail.len() > 16_384 {
        return None;
    }
    Some(OutcomeReport {
        status: record.status,
        detail: record.detail,
    })
}
/// 读取 worker 在交接目录中尽力写出的 failed 标记（failed.json），返回失败详情。
/// 标记缺失、损坏、越界或状态不符时返回 None；仅作为 poll_outcome 的补充交付。
pub fn pending_failure_detail(dir: &Path, handoff_id: &str) -> Option<String> {
    if !valid_handoff_token(handoff_id) {
        return None;
    }
    let subdir = dir.join(format!("handoff-{}", handoff_id));
    let marker = subdir.join("failed.json");
    crate::update_net::reject_reparse_path(&marker).ok()?;
    let subdir_canonical = std::fs::canonicalize(&subdir).ok()?;
    let canonical = std::fs::canonicalize(&marker).ok()?;
    if canonical.parent() != Some(subdir_canonical.as_path()) {
        return None;
    }
    let meta = std::fs::symlink_metadata(&canonical).ok()?;
    if !meta.is_file() || meta.len() > 65_536 {
        return None;
    }
    let bytes = std::fs::read(&canonical).ok()?;
    if bytes.len() > 65_536 {
        return None;
    }
    let marker: FailureMarker = serde_json::from_slice(&bytes).ok()?;
    if marker.status != "failed"
        || !valid_handoff_token(&marker.nonce)
        || marker.detail.len() > 16_384
    {
        return None;
    }
    Some(marker.detail)
}

#[derive(Debug)]
pub enum ApplyOutcome {
    Updated { placed_sha256: String },
    RolledBack { reason: String },
    RollbackFailed { reason: String, backup: PathBuf },
}
/// Retained only so old callers compile. A string hash is not authorization.
#[deprecated(note = "retain VerifiedUpdate and call spawn_worker; string-based apply is disabled")]
pub fn apply_update(_target: &Path, _downloaded: &Path, _expected_sha256: &str) -> ApplyOutcome {
    ApplyOutcome::RolledBack {
        reason: "旧版字符串更新接口已禁用；需要 VerifiedUpdate 独立 worker".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(payload: serde_json::Value) -> String {
        serde_json::json!({"payload": payload.to_string(), "signature": "00".repeat(64)})
            .to_string()
    }
    fn payload() -> serde_json::Value {
        serde_json::json!({"version":"1.2.1","package_url":"https://updates.example/package","package_sha256":"ab".repeat(32),"package_kind":"raw"})
    }
    #[test]
    fn canonical_versions_only() {
        for bad in [
            " 1.2.3",
            "1.2.3 ",
            "+1.2.3",
            "01.2.3",
            "1.02.3",
            "1.2",
            "1.2.3.4",
            "4294967296.0.0",
            "../app.exe",
            "１.2.3",
        ] {
            assert!(parse_version(bad).is_none(), "{bad}");
        }
        assert!(is_newer_version("1.2.9", "1.3.0"));
        assert!(!is_newer_version("1.2.0", "1.2.0"));
    }
    #[test]
    fn strict_manifest_rejects_ambiguous_fields() {
        assert!(UpdateManifest::parse(&text(payload())).is_ok());
        for (field, value) in [
            ("version", "../1"),
            ("package_url", "http://example/p"),
            ("package_sha256", "abc"),
            ("package_kind", "zip"),
        ] {
            let mut p = payload();
            p[field] = value.into();
            assert!(UpdateManifest::parse(&text(p)).is_err());
        }
        let mut p = payload();
        p.as_object_mut().unwrap().remove("package_kind");
        assert!(UpdateManifest::parse(&text(p)).is_err());
        let duplicate = format!(
            r#"{{"payload":{},"payload":{},"signature":"{}"}}"#,
            serde_json::to_string(&payload().to_string()).unwrap(),
            serde_json::to_string(&payload().to_string()).unwrap(),
            "00".repeat(64)
        );
        assert!(UpdateManifest::parse(&duplicate).is_err());
        let dupe_payload = payload()
            .to_string()
            .replacen('{', "{\"version\":\"8.0.0\",", 1);
        assert!(
            UpdateManifest::parse(
                &serde_json::json!({"payload":dupe_payload,"signature":"00".repeat(64)})
                    .to_string()
            )
            .is_err()
        );
    }
    #[test]
    fn production_never_trusts_disclosed_fixture() {
        if UPDATE_PUBLIC_KEY_HEX.is_empty()
            || UPDATE_PUBLIC_KEY_HEX
                .trim()
                .eq_ignore_ascii_case(DISCLOSED_FIXTURE_KEY.trim())
        {
            assert!(release_key().is_err());
        }
        assert!(authenticate(&text(payload()), DISCLOSED_FIXTURE_KEY).is_err());
    }
    #[test]
    fn corrupt_package_is_rejected_by_apply_gate() {
        let dir = crate::update_net::private_temp_dir().unwrap();
        let path = dir.join("package.bin");
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(verify_package(&path, &crate::update_net::sha256_hex(b"valid").unwrap()).is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    // ---- 阶段 6a：交接记录、受限清理与后台准备（C1/C2/C3） ----
    fn receipt_id() -> String {
        "ab".repeat(16)
    }
    fn seed_scratch(dir: &Path) -> PathBuf {
        let scratch = dir.join("scratch");
        std::fs::create_dir(&scratch).unwrap();
        std::fs::write(scratch.join("package.bin"), b"p").unwrap();
        std::fs::write(scratch.join("worker.exe"), b"w").unwrap();
        std::fs::write(scratch.join("request.json"), b"{}").unwrap();
        scratch
    }
    fn seeded_receipt(scratch: &Path, files: Vec<PathBuf>) -> HandoffReceipt {
        HandoffReceipt {
            handoff_id: receipt_id(),
            nonce: "cd".repeat(16),
            scratch: scratch.to_path_buf(),
            target: scratch.parent().unwrap_or(scratch).join("app.exe"),
            worker_path: scratch.join("worker.exe"),
            files,
        }
    }
    fn manifest_envelope(version: &str, sha: &str) -> String {
        serde_json::json!({
            "payload": serde_json::json!({
                "version": version,
                "package_url": "https://updates.example/package",
                "package_sha256": sha,
                "package_kind": "raw"
            })
            .to_string(),
            "signature": "00".repeat(64)
        })
        .to_string()
    }
    fn test_update(scratch: &Path, body: &[u8], declared_sha: &str) -> VerifiedUpdate {
        let package = scratch.join("package.bin");
        std::fs::write(&package, body).unwrap();
        VerifiedUpdate {
            manifest: UpdateManifest {
                version: "1.2.2".into(),
                package_url: "https://updates.example/package".into(),
                package_sha256: declared_sha.into(),
                notes: String::new(),
                package_kind: "raw".into(),
                signature: "00".repeat(64),
            },
            signed_text: manifest_envelope("1.2.2", declared_sha),
            package_path: package,
            scratch: scratch.to_path_buf(),
            old_version: "1.2.1".into(),
            transferred: false,
        }
    }
    fn write_worker_double(dir: &Path) -> PathBuf {
        // 受控 worker 替身：按生产协议接收 --quicknest-update-worker 与请求路径。
        // 请求路径固定取第二个参数（%~2），避免 -File 模式把连字符开头的首参当作
        // 命名参数绑定；脚本与输出均为纯 ASCII，不依赖编码。helper 以纯文本 trace
        // 自证执行结果，便于失败定位。
        let cmd = dir.join("double.cmd");
        std::fs::write(
            &cmd,
            "@echo off\r\nset \"REQUEST=%~2\"\r\nif \"%REQUEST%\"==\"\" set \"REQUEST=%~1\"\r\npowershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"%~dp0double_helper.ps1\" \"%REQUEST%\"\r\n",
        )
        .unwrap();
        let script = dir.join("double_helper.ps1");
        std::fs::write(
            &script,
            "param([string]$RequestPath)\r\n$log = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'double-trace.txt'\r\n\"start request=[$RequestPath]\" | Out-File -FilePath $log -Encoding ascii -Append\r\ntry {\r\n  $request = Get-Content -Raw -LiteralPath $RequestPath | ConvertFrom-Json\r\n  $ready = Join-Path (Split-Path -Parent $RequestPath) 'ready'\r\n  [IO.File]::WriteAllText($ready, $request.nonce)\r\n  \"ok ready=$ready\" | Out-File -FilePath $log -Encoding ascii -Append\r\n} catch {\r\n  \"error=$_\" | Out-File -FilePath $log -Encoding ascii -Append\r\n}\r\nStart-Sleep -Milliseconds 800\r\nexit 0\r\n",
        )
        .unwrap();
        cmd
    }
    fn teardown_scratch(dir: &Path) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            if entry.file_type().unwrap().is_file() {
                std::fs::remove_file(entry.path()).unwrap();
            } else {
                std::fs::remove_dir_all(entry.path()).unwrap();
            }
        }
        std::fs::remove_dir(dir).unwrap();
    }
    fn teardown_double(dir: &Path) {
        for name in ["double.cmd", "double_helper.ps1", "double-trace.txt"] {
            let _ = std::fs::remove_file(dir.join(name));
        }
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn handoff_record_load_acknowledge_roundtrip() {
        let root = crate::update_net::private_temp_dir().unwrap();
        let scratch = seed_scratch(&root);
        std::fs::write(root.join("app.exe"), b"target").unwrap();
        let receipt = HandoffReceipt {
            scratch: scratch.clone(),
            target: root.join("app.exe"),
            worker_path: scratch.join("worker.exe"),
            ..seeded_receipt(
                &scratch,
                vec![
                    scratch.join("package.bin"),
                    scratch.join("worker.exe"),
                    scratch.join("request.json"),
                ],
            )
        };
        let json = record_handoff(&root, receipt.clone()).unwrap();
        assert_eq!(
            json,
            root.join(format!("handoff-{}", receipt_id()))
                .join("handoff.json")
        );
        assert_eq!(load_pending_handoffs(&root), vec![receipt.clone()]);
        // 重复登记同一 id 被拒绝，且不覆盖现有记录
        assert!(record_handoff(&root, receipt.clone()).is_err());
        // 非法 id / nonce / 相对路径 / 越界登记在写入前被拒绝
        for bad in [
            HandoffReceipt {
                handoff_id: "../evil".into(),
                ..receipt.clone()
            },
            HandoffReceipt {
                handoff_id: "short".into(),
                ..receipt.clone()
            },
            HandoffReceipt {
                nonce: "zz".into(),
                ..receipt.clone()
            },
            HandoffReceipt {
                files: vec![root.join("app.exe")],
                ..receipt.clone()
            },
            HandoffReceipt {
                scratch: PathBuf::from("relative"),
                ..receipt.clone()
            },
        ] {
            assert!(record_handoff(&root, bad).is_err());
        }
        // 无关条目、损坏记录与未通过校验的记录被逐条跳过，不影响其余
        let junk = root.join("not-a-handoff");
        std::fs::create_dir(&junk).unwrap();
        std::fs::write(junk.join("handoff.json"), b"{ broken").unwrap();
        let hostile_dir = root.join("handoff-hostile");
        std::fs::create_dir(&hostile_dir).unwrap();
        let mut hostile = receipt.clone();
        hostile.handoff_id = "short".into();
        std::fs::write(
            hostile_dir.join("handoff.json"),
            serde_json::to_vec(&hostile).unwrap(),
        )
        .unwrap();
        assert_eq!(load_pending_handoffs(&root), vec![receipt.clone()]);
        // 确认后清除并幂等；登记的 scratch 文件不受 acknowledge 影响
        acknowledge(&root, &receipt.handoff_id).unwrap();
        assert!(load_pending_handoffs(&root).is_empty());
        acknowledge(&root, &receipt.handoff_id).unwrap();
        assert!(scratch.join("package.bin").exists());
        assert!(!root.join(format!("handoff-{}", receipt_id())).exists());
        for name in ["package.bin", "worker.exe", "request.json"] {
            std::fs::remove_file(scratch.join(name)).unwrap();
        }
        std::fs::remove_dir(&scratch).unwrap();
        std::fs::remove_file(root.join("app.exe")).unwrap();
        std::fs::remove_file(junk.join("handoff.json")).unwrap();
        std::fs::remove_dir(junk).unwrap();
        std::fs::remove_file(hostile_dir.join("handoff.json")).unwrap();
        std::fs::remove_dir(hostile_dir).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
    #[test]
    fn cleanup_gates_on_worker_exit_and_keeps_unknown_files() {
        let root = crate::update_net::private_temp_dir().unwrap();
        let scratch = seed_scratch(&root);
        std::fs::write(scratch.join("unknown.txt"), b"keep").unwrap();
        let receipt = HandoffReceipt {
            scratch: scratch.clone(),
            ..seeded_receipt(
                &scratch,
                vec![
                    scratch.join("package.bin"),
                    scratch.join("worker.exe"),
                    scratch.join("request.json"),
                ],
            )
        };
        // worker 未确认退出：删除前整体拒绝，worker.exe 与全部登记文件保留
        let error = cleanup_finished_owned(&receipt, false).unwrap_err();
        assert!(error.to_string().contains("未确认退出"), "{error}");
        for file in &receipt.files {
            assert!(file.exists(), "{}", file.display());
        }
        // 已退出：仅删除登记文件；未登记文件与其所在目录保留
        cleanup_finished_owned(&receipt, true).unwrap();
        assert!(!scratch.join("package.bin").exists());
        assert!(!scratch.join("worker.exe").exists());
        assert!(!scratch.join("request.json").exists());
        assert!(scratch.join("unknown.txt").exists(), "未登记文件必须保留");
        // 再次清理：已不存在的登记条目被跳过，未知文件仍保留
        cleanup_finished_owned(&receipt, true).unwrap();
        assert!(scratch.join("unknown.txt").exists());
        std::fs::remove_file(scratch.join("unknown.txt")).unwrap();
        // 目录已空才被移除；之后 scratch 缺失视为无剩余自有文件并成功
        cleanup_finished_owned(&receipt, true).unwrap();
        assert!(!scratch.exists());
        cleanup_finished_owned(&receipt, true).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
    #[test]
    fn cleanup_refuses_entries_outside_scratch_and_reparse_points() {
        let root = crate::update_net::private_temp_dir().unwrap();
        let scratch = seed_scratch(&root);
        let outside = root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        let victim = outside.join("victim.txt");
        std::fs::write(&victim, b"keep me").unwrap();
        let junction = scratch.join("link");
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction fixture 创建失败：{:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        let receipt = HandoffReceipt {
            scratch: scratch.clone(),
            files: vec![
                scratch.join("package.bin"),
                victim.clone(),
                junction.clone(),
                scratch.join("ghost.bin"),
            ],
            ..seeded_receipt(&scratch, Vec::new())
        };
        cleanup_finished_owned(&receipt, true).unwrap();
        assert!(
            !scratch.join("package.bin").exists(),
            "登记且合法的文件应被删除"
        );
        assert!(victim.exists(), "scratch 之外的登记条目必须拒绝删除");
        assert!(junction.exists(), "reparse 登记条目必须拒绝删除");
        assert!(scratch.exists(), "含未清理对象的 scratch 目录必须保留");
        // teardown：junction 先删（只删链接本身），目标不受影响
        std::fs::remove_dir(&junction).unwrap();
        assert!(victim.exists());
        for name in ["worker.exe", "request.json"] {
            std::fs::remove_file(scratch.join(name)).unwrap();
        }
        std::fs::remove_dir(&scratch).unwrap();
        std::fs::remove_file(&victim).unwrap();
        std::fs::remove_dir(&outside).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
    #[test]
    fn poll_outcome_handles_delayed_and_tampered_records() {
        let root = crate::update_net::private_temp_dir().unwrap();
        let scratch = root.join("scratch");
        std::fs::create_dir(&scratch).unwrap();
        let receipt = HandoffReceipt {
            scratch: scratch.clone(),
            ..seeded_receipt(&scratch, Vec::new())
        };
        // outcome.json 由 worker 延迟写出：先轮询为 None
        assert!(poll_outcome(&receipt).is_none());
        let outcome = scratch.join("outcome.json");
        std::fs::write(&outcome, br#"{"status":"updated","detail":"ok detail"}"#).unwrap();
        assert_eq!(
            poll_outcome(&receipt).unwrap(),
            OutcomeReport {
                status: "updated".into(),
                detail: "ok detail".into(),
            }
        );
        std::fs::write(
            &outcome,
            br#"{"status":"failed","detail":"rollback reason"}"#,
        )
        .unwrap();
        let failed = poll_outcome(&receipt).unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.detail, "rollback reason");
        for bad in [
            r#"{"status":"weird","detail":"d"}"#,
            r#"{"status":"updated","detail":"d","extra":1}"#,
            "not-json",
            r#"{"status":"failed"}"#,
        ] {
            std::fs::write(&outcome, bad).unwrap();
            assert!(poll_outcome(&receipt).is_none(), "{bad}");
        }
        let oversized = format!(
            r#"{{"status":"updated","detail":"{}"}}"#,
            "x".repeat(20_000)
        );
        std::fs::write(&outcome, oversized).unwrap();
        assert!(poll_outcome(&receipt).is_none());
        std::fs::remove_file(&outcome).unwrap();
        std::fs::create_dir(&outcome).unwrap();
        assert!(poll_outcome(&receipt).is_none(), "目录型 outcome 必须拒绝");
        std::fs::remove_dir(&outcome).unwrap();
        std::fs::remove_dir(&scratch).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
    #[test]
    fn prepare_in_background_reports_failure_and_cleans_scratch() {
        let scratch = crate::update_net::private_temp_dir().unwrap();
        let body = b"package-bytes";
        let update = test_update(&scratch, body, &"ab".repeat(32));
        // 目标 ≠ 当前进程：确定性失败，且先于任何密钥或签名校验
        let receiver = prepare_in_background(update, &scratch);
        let error = receiver
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("通道回传")
            .unwrap_err();
        assert!(error.contains("当前进程的可执行文件"), "{error}");
        // 失败即自然结束：独占 scratch 已被线程内的 VerifiedUpdate::drop 清理
        assert!(!scratch.exists(), "失败后独占 scratch 必须被清理");
    }
    #[test]
    fn prepare_in_background_delivers_spawned_worker_via_channel() {
        let scratch = crate::update_net::private_temp_dir().unwrap();
        let double_dir = crate::update_net::private_temp_dir().unwrap();
        let body = b"package-bytes";
        let sha = crate::update_net::sha256_hex(body).unwrap();
        let update = test_update(&scratch, body, &sha);
        let double = write_worker_double(&double_dir);
        let target = std::env::current_exe().unwrap();
        let receiver = prepare_in_background_with_double(update, &target, &double);
        let mut child = receiver
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("通道回传")
            .expect("替身 worker 应就绪");
        assert!(scratch.join("ready").exists(), "就绪标记必须已由替身写出");
        // 替身短暂存活后自退出；等待其退出避免遗留进程
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "替身 worker 未按时退出"
            );
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        child.wait().unwrap();
        // 替身 trace 自证：确实读到请求并写出就绪标记
        let trace =
            std::fs::read_to_string(double_dir.join("double-trace.txt")).expect("替身执行 trace");
        assert!(trace.contains("ok ready="), "替身 trace：{trace}");
        // transferred = true：Drop 不再清理，测试自行清理
        teardown_scratch(&scratch);
        teardown_double(&double_dir);
    }
    #[test]
    fn prepare_thread_continues_after_receiver_drop() {
        let scratch = crate::update_net::private_temp_dir().unwrap();
        let double_dir = crate::update_net::private_temp_dir().unwrap();
        let body = b"package-bytes";
        let sha = crate::update_net::sha256_hex(body).unwrap();
        let update = test_update(&scratch, body, &sha);
        let double = write_worker_double(&double_dir);
        let target = std::env::current_exe().unwrap();
        let receiver = prepare_in_background_with_double(update, &target, &double);
        drop(receiver);
        // 取消语义：丢弃 Receiver 不终止线程——替身继续运行并写出就绪标记
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !scratch.join("ready").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "线程未在 Receiver 丢弃后继续运行"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // 留出线程标记 transferred 并自然结束的窗口
        std::thread::sleep(std::time::Duration::from_millis(400));
        teardown_scratch(&scratch);
        teardown_double(&double_dir);
    }
    #[test]
    fn handoff_prepare_records_receipt_before_failure_and_completes_lifecycle() {
        let handoff_root = crate::update_net::private_temp_dir().unwrap();
        let scratch = crate::update_net::private_temp_dir().unwrap();
        let body = b"package-bytes";
        let sha = crate::update_net::sha256_hex(body).unwrap();
        let update = test_update(&scratch, body, &sha);
        let handoff_id = new_handoff_id().unwrap();
        let receiver =
            prepare_in_background_with_handoff(update, &scratch, &handoff_root, &handoff_id);
        let error = receiver
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("通道回传")
            .unwrap_err();
        assert!(error.contains("当前进程的可执行文件"), "{error}");
        // 记录在准备失败之前已经落盘，字段与登记清单完整
        let pending = load_pending_handoffs(&handoff_root);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].handoff_id, handoff_id);
        assert!(valid_handoff_token(&pending[0].nonce));
        assert_eq!(pending[0].files.len(), 5);
        assert!(
            pending[0]
                .files
                .iter()
                .all(|f| f.parent() == Some(scratch.as_path()))
        );
        assert!(poll_outcome(&pending[0]).is_none());
        // 失败线程已清理独占 scratch → 受限清理无事可做并成功
        assert!(!scratch.exists());
        cleanup_finished_owned(&pending[0], true).unwrap();
        acknowledge(&handoff_root, &handoff_id).unwrap();
        assert!(load_pending_handoffs(&handoff_root).is_empty());
        std::fs::remove_dir(&handoff_root).unwrap();
    }
}

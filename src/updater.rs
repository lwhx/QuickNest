//! Signed update checking. Only `VerifiedUpdate` can authorize a production worker.
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
}

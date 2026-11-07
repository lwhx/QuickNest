//! Isolated acceptance fixtures are available only in `cargo test` builds.
//! A release executable never loads test signing keys from runtime input.
use std::path::Path;

/// Legacy CLI hook fails closed. Use scripts/verify-p4-040506-update.ps1,
/// which creates temporary signing/TLS identities and runs the test-only path.
pub fn run(_report: &Path, _base_url: &str, _current_version: &str) -> Result<(), String> {
    Err("旧版 --update-test 已禁用；请运行隔离 verify-p4-040506-update.ps1（测试公钥不进入生产信任）".into())
}

#[cfg(test)]
mod tests {
    use crate::{update_net, updater};
    #[test]
    #[ignore = "requires ephemeral externally generated signing fixture"]
    fn signed_fixture_acceptance() {
        let dir = std::env::var_os("QUICKNEST_UPDATE_TEST_FIXTURE")
            .expect("explicit fixture directory required");
        let dir = std::path::PathBuf::from(dir);
        let key = std::fs::read_to_string(dir.join("public.hex")).unwrap();
        let text = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
        let manifest = updater::UpdateManifest::parse(&text).unwrap();
        let payload = updater::UpdateManifest::payload_of(&text).unwrap();
        assert!(update_net::ecdsa_p256_verify(
            &key,
            payload.as_bytes(),
            &manifest.signature
        ));
        assert!(!update_net::ecdsa_p256_verify(
            &key,
            format!("{payload} ").as_bytes(),
            &manifest.signature
        ));
        assert!(!update_net::ecdsa_p256_verify(
            updater::UPDATE_PUBLIC_KEY_HEX,
            payload.as_bytes(),
            &manifest.signature
        ));
        updater::verify_package(&dir.join("package.exe"), &manifest.package_sha256).unwrap();
        let corrupt = dir.join("corrupt.exe");
        let mut bytes = std::fs::read(dir.join("package.exe")).unwrap();
        assert!(!bytes.is_empty());
        bytes[0] ^= 1;
        std::fs::write(&corrupt, bytes).unwrap();
        assert!(updater::verify_package(&corrupt, &manifest.package_sha256).is_err());
        // Fixture key is injected only inside the test module. Production API
        // has no key argument, runtime environment switch or certificate bypass.
        let mut changed: serde_json::Value = serde_json::from_str(&text).unwrap();
        changed["payload"] = format!("{payload} ").into();
        assert!(updater::authenticate(&changed.to_string(), &key).is_err());
        assert!(
            updater::test_checked_download(&text, &manifest.version, &key)
                .unwrap()
                .is_none()
        );
        assert!(
            updater::test_checked_download(&text, "99.0.0", &key)
                .unwrap()
                .is_none()
        );
        std::fs::remove_file(corrupt).unwrap();
    }
    #[test]
    fn unicode_signature_never_panics() {
        for signature in ["é".repeat(64), "中".repeat(64), "00 ".repeat(64)] {
            assert!(!update_net::ecdsa_p256_verify("é", b"payload", &signature));
            let text = serde_json::json!({"payload":"{}", "signature":signature}).to_string();
            assert!(updater::UpdateManifest::parse(&text).is_err());
        }
    }
}

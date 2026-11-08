//! Read-only build provenance dispatch, before configuration or desktop startup.
use std::io::Write;

include!(concat!(env!("OUT_DIR"), "/quicknest_build_info.rs"));

#[derive(Debug, serde::Serialize)]
pub struct UpdateKeyInfo {
    pub status: &'static str,
    pub blob_sha256: Option<&'static str>,
}

#[derive(Debug, serde::Serialize)]
pub struct BuildInfo {
    pub schema_version: u32,
    pub version: &'static str,
    pub provenance: &'static str,
    pub source_fingerprint: Option<&'static str>,
    pub build_fingerprint: Option<&'static str>,
    pub base_head: Option<&'static str>,
    pub dirty: Option<bool>,
    pub target: &'static str,
    pub profile: &'static str,
    pub rustc: &'static str,
    pub cargo: &'static str,
    pub update_key: UpdateKeyInfo,
}

pub fn current() -> BuildInfo {
    BuildInfo {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION"),
        provenance: BUILD_PROVENANCE,
        source_fingerprint: BUILD_SOURCE_FINGERPRINT,
        build_fingerprint: BUILD_FINGERPRINT,
        base_head: BUILD_BASE_HEAD,
        dirty: BUILD_DIRTY,
        target: BUILD_TARGET,
        profile: BUILD_PROFILE,
        rustc: BUILD_RUSTC,
        cargo: BUILD_CARGO,
        update_key: UpdateKeyInfo {
            status: BUILD_UPDATE_KEY_STATUS,
            blob_sha256: BUILD_UPDATE_KEY_SHA256,
        },
    }
}

/// Receives argv including argv[0], just like updater::worker_entry.
/// Every invocation containing this switch is consumed, including malformed ones.
/// This path reads compiled constants and writes stdout; it never opens user data.
pub fn entry(args: &[String]) -> Option<Result<(), String>> {
    if !args
        .iter()
        .skip(1)
        .any(|arg| arg == "--quicknest-build-info")
    {
        return None;
    }
    Some((|| {
        if args.len() != 2 {
            return Err("Usage: quicknest --quicknest-build-info".into());
        }
        let mut output = std::io::stdout().lock();
        serde_json::to_writer(&mut output, &current()).map_err(|err| err.to_string())?;
        output.write_all(b"\n").map_err(|err| err.to_string())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrelated_protocols_are_left_to_their_existing_dispatcher() {
        for args in [
            vec!["quicknest".into()],
            vec![
                "quicknest".into(),
                "--quicknest-update-health".into(),
                "1.2.1".into(),
                "nonce".into(),
            ],
        ] {
            assert!(entry(&args).is_none());
        }
    }

    #[test]
    fn malformed_build_info_is_consumed_before_ui_startup() {
        let args = [
            "quicknest".into(),
            "--quicknest-build-info".into(),
            "extra".into(),
        ];
        assert!(entry(&args).unwrap().is_err());
    }

    #[test]
    fn public_report_has_fingerprints_and_no_key_material() {
        let report = serde_json::to_value(current()).unwrap();
        assert_eq!(report["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(report["schema_version"], 1);
        assert!(report["update_key"].get("blob_sha256").is_some());
        assert_eq!(report["update_key"].as_object().unwrap().len(), 2);
        assert!(matches!(BUILD_PROVENANCE, "recorded" | "unrecorded"));
    }
}

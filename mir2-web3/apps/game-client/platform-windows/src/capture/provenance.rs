//! Trusted build provenance for native visual captures.
//!
//! A capture may only claim build hashes when the running executable is inside
//! a packaged Candidate whose `VERSION.json` and `PACKAGE-MANIFEST.json` both
//! bind the exact executable bytes. Development builds deliberately return an
//! empty provenance record and therefore stay on the draft sidecar schema.

use super::{NativeCaptureBuild, is_sha256_hex, sha256_hex};
use serde_json::Value;
use std::{env, fs, sync::OnceLock};

static TRUSTED_BUILD: OnceLock<NativeCaptureBuild> = OnceLock::new();

pub(super) fn trusted_build_provenance() -> NativeCaptureBuild {
    TRUSTED_BUILD
        .get_or_init(load_trusted_build_provenance)
        .clone()
}

fn load_trusted_build_provenance() -> NativeCaptureBuild {
    let Some(executable_path) = env::current_exe().ok() else {
        return NativeCaptureBuild::default();
    };
    let Some(package_root) = executable_path.parent() else {
        return NativeCaptureBuild::default();
    };
    let Ok(executable_bytes) = fs::read(&executable_path) else {
        return NativeCaptureBuild::default();
    };
    let Ok(version_bytes) = fs::read(package_root.join("VERSION.json")) else {
        return NativeCaptureBuild::default();
    };
    let Ok(package_manifest_bytes) = fs::read(package_root.join("PACKAGE-MANIFEST.json")) else {
        return NativeCaptureBuild::default();
    };
    let executable_name = executable_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    parse_trusted_build_provenance(
        &version_bytes,
        &package_manifest_bytes,
        executable_name,
        executable_bytes.len() as u64,
        &sha256_hex(&executable_bytes),
    )
    .unwrap_or_default()
}

fn parse_trusted_build_provenance(
    version_bytes: &[u8],
    package_manifest_bytes: &[u8],
    executable_name: &str,
    executable_size: u64,
    executable_sha256: &str,
) -> Option<NativeCaptureBuild> {
    if executable_name.is_empty() || !is_sha256_hex(executable_sha256) {
        return None;
    }
    let version: Value = serde_json::from_slice(version_bytes).ok()?;
    let revision = version.get("gitRevision")?.as_str()?.trim();
    let version_executable_name = version.get("exeName")?.as_str()?.trim();
    let version_executable_sha256 = version.get("exeSha256")?.as_str()?.trim();
    let version_executable_size = version.get("exeSizeBytes")?.as_u64()?;
    if !valid_revision(revision)
        || !version_executable_name.eq_ignore_ascii_case(executable_name)
        || !version_executable_sha256.eq_ignore_ascii_case(executable_sha256)
        || version_executable_size != executable_size
    {
        return None;
    }

    let manifest: Value = serde_json::from_slice(package_manifest_bytes).ok()?;
    if manifest.get("schema")?.as_str()? != "mir2.windows.package-manifest.v4" {
        return None;
    }
    let executable_entry = manifest.get("files")?.as_array()?.iter().find(|entry| {
        entry
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| path.eq_ignore_ascii_case(executable_name))
    })?;
    if executable_entry.get("size")?.as_u64()? != executable_size
        || !executable_entry
            .get("sha256")?
            .as_str()?
            .eq_ignore_ascii_case(executable_sha256)
    {
        return None;
    }

    Some(NativeCaptureBuild {
        source_revision: Some(revision.to_owned()),
        executable_sha256: Some(executable_sha256.to_ascii_lowercase()),
        // The Candidate package manifest covers every payload file, including
        // `mir2-assets`. Its own digest is the reproducible asset-pack binding
        // used by the visual-capture v1 contract.
        asset_manifest_sha256: Some(sha256_hex(package_manifest_bytes)),
    })
}

fn valid_revision(value: &str) -> bool {
    (7..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(executable_hash: &str) -> (Vec<u8>, Vec<u8>) {
        let version = serde_json::json!({
            "gitRevision": "78c1e62aed8f7eea82d2a84110a3a3e48c229161",
            "exeName": "mir2-platform-windows.exe",
            "exeSha256": executable_hash.to_ascii_uppercase(),
            "exeSizeBytes": 1234,
        });
        let manifest = serde_json::json!({
            "schema": "mir2.windows.package-manifest.v4",
            "files": [{
                "path": "mir2-platform-windows.exe",
                "size": 1234,
                "sha256": executable_hash.to_ascii_uppercase(),
            }],
        });
        (
            serde_json::to_vec(&version).expect("version fixture"),
            serde_json::to_vec(&manifest).expect("manifest fixture"),
        )
    }

    #[test]
    fn package_contract_binds_revision_executable_and_manifest_bytes() {
        let executable_hash = "a".repeat(64);
        let (version, manifest) = fixture(&executable_hash);
        let build = parse_trusted_build_provenance(
            &version,
            &manifest,
            "mir2-platform-windows.exe",
            1234,
            &executable_hash,
        )
        .expect("trusted package");
        assert_eq!(
            build.source_revision.as_deref(),
            Some("78c1e62aed8f7eea82d2a84110a3a3e48c229161")
        );
        assert_eq!(
            build.executable_sha256.as_deref(),
            Some(executable_hash.as_str())
        );
        let manifest_hash = sha256_hex(&manifest);
        assert_eq!(
            build.asset_manifest_sha256.as_deref(),
            Some(manifest_hash.as_str())
        );
    }

    #[test]
    fn package_contract_rejects_any_executable_mismatch() {
        let executable_hash = "a".repeat(64);
        let (version, manifest) = fixture(&executable_hash);
        assert!(
            parse_trusted_build_provenance(
                &version,
                &manifest,
                "mir2-platform-windows.exe",
                1234,
                &"b".repeat(64),
            )
            .is_none()
        );
        assert!(
            parse_trusted_build_provenance(
                &version,
                &manifest,
                "renamed.exe",
                1234,
                &executable_hash,
            )
            .is_none()
        );
    }
}

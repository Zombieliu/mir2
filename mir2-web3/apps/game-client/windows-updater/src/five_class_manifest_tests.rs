//! Large five-class inventories must remain signed, bounded and path-safe.
use crate::model::{hash, FileEntry, Manifest, MAX_MANIFEST_FILES, MAX_META, META};

fn manifest_bytes(mut entries: Vec<FileEntry>) -> Vec<u8> {
    for path in ["mir2-platform-windows.exe", "BUILD-ATTESTATION.json"] {
        entries.push(FileEntry {
            path: path.to_owned(),
            size: 5,
            sha256: hash(b"frame"),
        });
    }
    let mut sorted = entries;
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let canonical = sorted
        .iter()
        .map(|entry| format!("{}\t{}\t{}\n", entry.path, entry.size, entry.sha256))
        .collect::<String>();
    let total_bytes: u64 = sorted.iter().map(|entry| entry.size).sum();
    serde_json::to_vec(&serde_json::json!({
        "schema": "mir2.windows.package-manifest.v4",
        "coverage": {"excludes": META, "rule": "exact signed payload"},
        "fileCount": sorted.len(),
        "totalBytes": total_bytes,
        "aggregateSha256": hash(canonical.as_bytes()),
        "files": sorted,
    }))
    .unwrap()
}

#[test]
fn five_class_manifest_accepts_inventory_above_both_old_limits() {
    let frame_hash = hash(b"frame");
    let entries = (0..199_999)
        .map(|index| FileEntry {
            path: format!(
                "mir2-assets/original-ui/AWeapon/00 L/frames/source-generation/{index:06}.png"
            ),
            size: 5,
            sha256: frame_hash.clone(),
        })
        .collect();
    let bytes = manifest_bytes(entries);
    assert!(bytes.len() > 32 * 1024 * 1024);
    assert!(bytes.len() as u64 <= MAX_META);
    let manifest = Manifest::parse(&bytes).unwrap();
    assert_eq!(manifest.file_count, 200_001);
    assert_eq!(manifest.total_bytes, 1_000_005);
}

#[test]
fn five_class_manifest_rejects_oversized_metadata_before_json_parsing() {
    let bytes = vec![b' '; MAX_META as usize + 1];
    let error = Manifest::parse(&bytes).unwrap_err();
    assert!(error.to_string().contains("manifest too large"));
}

#[test]
fn five_class_manifest_rejects_count_above_new_finite_bound() {
    let frame_hash = hash(b"frame");
    let entries = (0..MAX_MANIFEST_FILES - 1)
        .map(|index| FileEntry {
            path: format!("mir2-assets/{index:06}.png"),
            size: 5,
            sha256: frame_hash.clone(),
        })
        .collect();
    let bytes = manifest_bytes(entries);
    assert!(bytes.len() as u64 <= MAX_META);
    assert!(Manifest::parse(&bytes)
        .unwrap_err()
        .to_string()
        .contains("manifest bounds"));
}

#[test]
fn five_class_manifest_still_rejects_case_collisions_and_executable_assets() {
    let entry = |path: &str| FileEntry {
        path: path.to_owned(),
        size: 5,
        sha256: hash(b"frame"),
    };
    let collision = manifest_bytes(vec![
        entry("mir2-assets/Frame.png"),
        entry("mir2-assets/frame.png"),
    ]);
    assert!(Manifest::parse(&collision).is_err());
    let executable = manifest_bytes(vec![entry("mir2-assets/frame.cmd.png")]);
    assert!(Manifest::parse(&executable).is_err());
}

#[test]
fn five_class_engine_uses_separate_pinned_channel_for_both_origins() {
    let cdn = url::Url::parse(crate::CDN_FEED_URL).unwrap();
    let origin = url::Url::parse(crate::FEED_URL).unwrap();
    assert_eq!(cdn.scheme(), "https");
    assert_eq!(origin.scheme(), "https");
    assert_eq!(cdn.path(), "/client-updates/five-class/latest.json");
    assert_eq!(origin.path(), cdn.path());
    assert_eq!(
        cdn.join("./").unwrap().path(),
        "/client-updates/five-class/"
    );
    assert_eq!(
        origin.join("./").unwrap().path(),
        "/client-updates/five-class/"
    );
    assert!(cdn.query().is_none() && origin.query().is_none());
    assert!(cdn.fragment().is_none() && origin.fragment().is_none());
}

#[test]
fn five_class_update_progress_keeps_large_inventory_counts() {
    for phase in [
        "extracting",
        "installing",
        "recovering",
        "checking-installed",
    ] {
        let text = crate::ui::stage("en", &format!("{phase}:200011/253554"));
        assert!(text.contains("(200011 / 253554)"));
        let over_limit = crate::ui::stage(
            "en",
            &format!("{phase}:1/{}", crate::model::MAX_TRANSACTION_FILES + 1),
        );
        assert!(!over_limit.contains("(1 /"));
    }
}

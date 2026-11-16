//! CPU/filesystem-only diagnostics tests. No window, GPU, network or auth.
use super::*;
use mir2_resource_pack::{PackStore, ResourceInput};

fn fixture(profiled: bool) -> (tempfile::TempDir, PackStore) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let mut inputs = Vec::new();
    for (path, bytes) in [
        ("one.png", b"first".as_slice()),
        ("two.png", b"second".as_slice()),
    ] {
        fs::write(source.join(path), bytes).unwrap();
        inputs.push(ResourceInput {
            path: path.into(),
            size: bytes.len() as u64,
            sha256: mir2_resource_pack::sha256(bytes),
        });
    }
    let objects = temp.path().join("objects");
    let output =
        mir2_resource_pack::build(&source, &objects, &inputs, None, Default::default()).unwrap();
    let store = if profiled {
        PackStore::open_profiled(&objects, &output.catalog_bytes, &output.catalog_sha256)
    } else {
        PackStore::open(&objects, &output.catalog_bytes, &output.catalog_sha256)
    }
    .unwrap();
    (temp, store)
}

fn provenance(store: &PackStore) -> VerifiedPackedProvenance {
    VerifiedPackedProvenance {
        generation_manifest_sha256: "A".repeat(64),
        catalog_sha256: store.catalog_sha256().into(),
        game_source_revision: "b".repeat(40),
        content_profile: "native-full-crystal".into(),
        executable_sha256: "C".repeat(64),
        pack_sha256: store
            .catalog()
            .packs
            .iter()
            .map(|p| p.sha256.clone())
            .collect(),
    }
}

fn request(path: &Path, store: &PackStore) -> NativeCaptureRequest {
    let mut request = super::super::capture_request(
        path.into(),
        "login",
        None,
        None,
        None,
        None,
        None,
        Some(1.0),
        None,
        Some(42),
    );
    let provenance = provenance(store);
    request.build = super::super::NativeCaptureBuild {
        packed_generation: Some(provenance.clone()),
        ..Default::default()
    };
    request.resource_reads = freeze(store.read_profile_snapshot(), Some(&provenance)).unwrap();
    request
}

fn cpu_image() -> bevy::prelude::Image {
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };
    bevy::prelude::Image::new_fill(
        Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[1, 2, 3, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

#[test]
fn resource_profile_uses_the_existing_valid_capture_gate_only() {
    use super::super::NativeCaptureConfig;
    for invalid in [
        None,
        Some(""),
        Some("  "),
        Some("../escape"),
        Some("a/../b"),
        Some("bad\0dir"),
    ] {
        assert!(NativeCaptureConfig::from_values(invalid, None, None, None).is_none());
    }
    assert!(NativeCaptureConfig::from_values(Some("captures"), None, None, None).is_some());
    let (_temp, store) = fixture(false);
    assert!(freeze(store.read_profile_snapshot(), None)
        .unwrap()
        .is_none());
}

#[test]
fn resource_snapshot_is_frozen_before_later_success_and_failure() {
    let (temp, store) = fixture(true);
    store.read("one.png").unwrap();
    let request = request(&temp.path().join("capture.png"), &store);
    store.read("two.png").unwrap();
    assert!(store.read("not-in-the-catalog.png").is_err());
    let frozen = &request.resource_reads.as_ref().unwrap().profile;
    assert_eq!(frozen.successful_read_operations, 1);
    assert_eq!(frozen.failed_read_operations, 0);
    assert_eq!(frozen.resources.keys().collect::<Vec<_>>(), vec!["one.png"]);
    let live = store.read_profile_snapshot().unwrap();
    assert_eq!(live.successful_read_operations, 2);
    assert_eq!(live.failed_read_operations, 1);
    assert!(!live.resources.contains_key("not-in-the-catalog.png"));
}

#[test]
fn resource_diagnostic_requires_authenticated_matching_catalog_and_request_generation() {
    let (temp, store) = fixture(true);
    let mut provenance = provenance(&store);
    assert!(freeze(store.read_profile_snapshot(), None).is_err());
    provenance.catalog_sha256 = "D".repeat(64);
    assert!(freeze(store.read_profile_snapshot(), Some(&provenance)).is_err());
    let mut request = request(&temp.path().join("capture.png"), &store);
    request
        .build
        .packed_generation
        .as_mut()
        .unwrap()
        .generation_manifest_sha256 = "E".repeat(64);
    assert!(serialize(&request, &"F".repeat(64)).is_err());
}

#[test]
fn resource_diagnostic_keeps_overflow_flags_and_never_claims_complete_boot_or_capture_eligibility()
{
    let (temp, store) = fixture(true);
    store.read("one.png").unwrap();
    let mut request = request(&temp.path().join("capture.png"), &store);
    let before = super::super::serialize_capture_sidecar(&request, 2, 2, "1".repeat(64)).unwrap();
    request
        .resource_reads
        .as_mut()
        .unwrap()
        .profile
        .counters_overflowed = true;
    request
        .resource_reads
        .as_mut()
        .unwrap()
        .profile
        .poison_recovered = true;
    let value: serde_json::Value =
        serde_json::from_slice(&serialize(&request, &"1".repeat(64)).unwrap().unwrap()).unwrap();
    assert_eq!(value["diagnosticOnly"], true);
    assert_eq!(value["completeBootClosureClaimed"], false);
    assert_eq!(value["reads"]["countersOverflowed"], true);
    assert_eq!(value["reads"]["poisonRecovered"], true);
    assert_eq!(value["requestFrame"], 42);
    assert_eq!(value["scene"], "login");
    let after = super::super::serialize_capture_sidecar(&request, 2, 2, "1".repeat(64)).unwrap();
    assert_eq!(
        before, after,
        "resource diagnostics cannot modify capture-v1/draft truth"
    );
    assert!(!String::from_utf8(after).unwrap().contains("resourceReads"));
}

#[test]
fn resource_diagnostic_pairs_the_final_cpu_png_hash_and_refuses_overwrite() {
    let (temp, store) = fixture(true);
    store.read("one.png").unwrap();
    let png = temp.path().join("capture.png");
    let request = request(&png, &store);
    super::super::write_capture_and_sidecar(&cpu_image(), &request).unwrap();
    let diagnostic = png.with_extension("resource-reads.json");
    let original = fs::read(&diagnostic).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    let image_hash = super::super::sha256_hex(&fs::read(&png).unwrap());
    assert_eq!(value["imageSha256"], image_hash);
    assert_eq!(value["imagePath"], "capture.png");
    assert_eq!(value["generation"]["catalogSha256"], store.catalog_sha256());
    assert_eq!(value["reads"]["resources"]["one.png"], 1);
    assert_eq!(
        write(&request, &image_hash).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&diagnostic).unwrap(), original);
    assert!(
        png.with_extension("json").is_file(),
        "original capture completion marker remains"
    );
}

#[test]
fn resource_diagnostic_failure_does_not_prevent_the_original_capture_pair() {
    let (temp, store) = fixture(true);
    let png = temp.path().join("capture.png");
    let request = request(&png, &store);
    let collision = png.with_extension("resource-reads.json");
    fs::create_dir(&collision).unwrap();
    super::super::write_capture_and_sidecar(&cpu_image(), &request).unwrap();
    assert!(png.is_file());
    let pair: serde_json::Value =
        serde_json::from_slice(&fs::read(png.with_extension("json")).unwrap()).unwrap();
    assert_eq!(
        pair["imageSha256"],
        super::super::sha256_hex(&fs::read(&png).unwrap())
    );
    assert_eq!(
        pair["schemaVersion"],
        super::super::CAPTURE_DRAFT_SCHEMA_VERSION
    );
    assert_eq!(pair["acceptance"]["eligible"], false);
    assert!(collision.is_dir(), "earlier evidence is never replaced");
}

#[test]
fn resource_diagnostic_publication_has_one_create_only_winner() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("race.resource-reads.json");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|index| {
            let target = target.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                write_new_synced(&target, format!("{{\"writer\":{index}}}").as_bytes())
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| r
                .as_ref()
                .is_err_and(|e| e.kind() == io::ErrorKind::AlreadyExists))
            .count(),
        1
    );
    let _: serde_json::Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    assert_eq!(
        fs::read_dir(temp.path()).unwrap().count(),
        1,
        "only owned temporary links were cleaned up"
    );
}

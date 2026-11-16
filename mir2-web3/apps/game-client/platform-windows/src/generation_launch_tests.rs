//! Real Windows exclusion handles and actual PackStore regression fixtures.
//! Selector fixtures test the post-authentication local policy; they do not
//! replace the CMS checks that precede this gate in load_inner.
use super::*;
use mir2_windows_updater::generation::{ActiveRecord, Health, Selection};

const DIGEST: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const ATTEMPT: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

fn selector(root: &Path, previous: Option<Selection>, health: Health) {
    std::fs::create_dir_all(root.join("state")).unwrap();
    let launch_started = health != Health::Healthy;
    let record = ActiveRecord {
        schema: "mir2.windows.active-generation.v1".into(),
        active: Selection::Generation {
            manifest_sha256: DIGEST.into(),
            sequence: 7,
        },
        previous,
        health,
        attempt: ATTEMPT.into(),
        launch_started,
    };
    std::fs::write(
        root.join("state/active-generation.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
}

fn guard(root: &Path) -> anyhow::Result<Option<GameGuard>> {
    legacy_exclusion_for_selected_generation(root, DIGEST, 7, ATTEMPT)
}

#[test]
fn packed_only_first_launch_and_healthy_relaunch_need_no_phantom_legacy_image() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("game")).unwrap();
    selector(temp.path(), None, Health::FirstLaunchPending);
    // This is the actual old precondition that made the r17 Windows game fatal.
    let mut old = GameGuard::new(&temp.path().join("game/mir2-platform-windows.exe")).unwrap();
    assert!(!old.is_running().unwrap());
    assert!(!old.excluded());
    assert!(guard(temp.path()).unwrap().is_none());
    selector(temp.path(), None, Health::Healthy);
    assert!(guard(temp.path()).unwrap().is_none());
    selector(
        temp.path(),
        Some(Selection::Generation {
            manifest_sha256: "C".repeat(64),
            sequence: 6,
        }),
        Health::Pending,
    );
    assert!(guard(temp.path()).unwrap().is_none());
}

#[test]
fn missing_retained_legacy_rollback_is_still_rejected() {
    let temp = tempfile::tempdir().unwrap();
    selector(
        temp.path(),
        Some(Selection::LegacyV4 {
            candidate: "fixture".into(),
            version_sha256: "C".repeat(64),
            exe_sha256: "D".repeat(64),
        }),
        Health::Pending,
    );
    assert!(guard(temp.path()).is_err());
}

#[test]
fn actual_legacy_executable_keeps_a_real_exclusion_handle() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("game")).unwrap();
    let exe = temp.path().join("game/mir2-platform-windows.exe");
    std::fs::write(&exe, b"non-executable fixture; never launched").unwrap();
    selector(temp.path(), None, Health::FirstLaunchPending);
    let retained = guard(temp.path())
        .unwrap()
        .expect("existing legacy image must be excluded");
    assert!(retained.excluded());
    retained.verify_excluded().unwrap();
    assert!(std::fs::OpenOptions::new().read(true).open(&exe).is_err());
    drop(retained);
    let busy = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&exe)
        .unwrap();
    assert!(guard(temp.path()).is_err());
    drop(busy);
    assert_eq!(
        std::fs::read(&exe).unwrap(),
        b"non-executable fixture; never launched"
    );
}

#[test]
fn quarantine_unknown_selector_and_attempt_mismatch_never_gain_the_missing_legacy_exception() {
    let temp = tempfile::tempdir().unwrap();
    selector(temp.path(), None, Health::FirstLaunchPending);
    assert!(
        legacy_exclusion_for_selected_generation(temp.path(), DIGEST, 7, &"C".repeat(64)).is_err()
    );
    let quarantine = temp.path().join(format!("state/quarantine/{DIGEST}.json"));
    std::fs::create_dir(quarantine.parent().unwrap()).unwrap();
    std::fs::write(&quarantine, b"retained quarantine evidence").unwrap();
    assert!(guard(temp.path()).is_err());
    assert_eq!(
        std::fs::read(&quarantine).unwrap(),
        b"retained quarantine evidence"
    );
    std::fs::remove_file(&quarantine).unwrap();
    let file = temp.path().join("state/active-generation.json");
    let mut record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    record["health"] = serde_json::json!("unrecognized");
    std::fs::write(&file, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(guard(temp.path()).is_err());
}

#[test]
fn directory_or_reparse_legacy_path_is_not_genuine_absence() {
    let temp = tempfile::tempdir().unwrap();
    selector(temp.path(), None, Health::FirstLaunchPending);
    let path = temp.path().join("game/mir2-platform-windows.exe");
    std::fs::create_dir_all(&path).unwrap();
    assert!(guard(temp.path()).is_err());
    let linked = tempfile::tempdir().unwrap();
    selector(linked.path(), None, Health::FirstLaunchPending);
    let physical = linked.path().join("physical-game");
    std::fs::create_dir(&physical).unwrap();
    let result = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(linked.path().join("game"))
        .arg(&physical)
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture junction creation failed");
    assert!(
        guard(linked.path()).is_err(),
        "reparse parent is not genuine NotFound"
    );
}

const REQUIRED: &[&str] = &[
    "crystal-map-pack/0.map.gz",
    "bevy-entity-atlases/manifest.json",
    "generated/map-atlas/manifest.json",
    "generated/native-map-keyed/manifest.json",
    "original-effects/effects.generated.json",
    "original-ui/Items/meta.json",
    "original-ui/Items/0.png",
    "original-ui/Items/3792.png",
    "original-ui/StateItem/meta.json",
    "original-ui/StateItem/30.png",
    "original-ui/StateItem/5152.png",
    "original-ui/Prguse2/meta.json",
    "original-ui/Prguse2/1202.png",
    "original-ui/Prguse2/1203.png",
    "original-ui/Prguse2/1204.png",
    "original-ui/Prguse2/1205.png",
    "original-ui/Cursors/Cursor_Default.png",
    "original-ui/Cursors/Cursor_Normal_Atk.png",
    "original-ui/Cursors/Cursor_Compulsion_Atk.png",
    "original-ui/Cursors/Cursor_Npc.png",
];

fn pack_fixture(temp: &Path, missing: Option<&str>) -> (PathBuf, mir2_resource_pack::BuildOutput) {
    let source = temp.join("source");
    std::fs::create_dir(&source).unwrap();
    let mut inputs = Vec::new();
    for path in REQUIRED.iter().filter(|path| Some(**path) != missing) {
        let file = source.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let bytes = format!("authenticated test resource bytes: {path}").into_bytes();
        std::fs::write(&file, &bytes).unwrap();
        inputs.push(mir2_resource_pack::ResourceInput {
            path: (*path).into(),
            size: bytes.len() as u64,
            sha256: mir2_resource_pack::sha256(&bytes),
        });
    }
    let objects = temp.join("objects");
    let out =
        mir2_resource_pack::build(&source, &objects, &inputs, None, Default::default()).unwrap();
    (objects, out)
}

#[test]
fn required_resources_are_read_from_verified_packs_without_any_loose_root() {
    let temp = tempfile::tempdir().unwrap();
    let (objects, out) = pack_fixture(temp.path(), None);
    let store = PackStore::open(&objects, &out.catalog_bytes, &out.catalog_sha256).unwrap();
    assert!(!temp.path().join("mir2-assets").exists());
    crate::assets::verify_packed_startup_assets(&store).unwrap();
}

#[test]
fn absent_required_resource_or_corrupt_pack_is_rejected_even_with_loose_bytes_present() {
    let temp = tempfile::tempdir().unwrap();
    let absent = "original-ui/Cursors/Cursor_Npc.png";
    let (objects, out) = pack_fixture(temp.path(), Some(absent));
    let loose = temp.path().join("mir2-assets").join(absent);
    std::fs::create_dir_all(loose.parent().unwrap()).unwrap();
    std::fs::write(
        &loose,
        b"loose replacement must not satisfy packed validation",
    )
    .unwrap();
    let store = PackStore::open(&objects, &out.catalog_bytes, &out.catalog_sha256).unwrap();
    assert!(crate::assets::verify_packed_startup_assets(&store)
        .unwrap_err()
        .contains(absent));
    drop(store);
    let object = objects.join(out.catalog.packs[0].file_name());
    let mut bytes = std::fs::read(&object).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(&object, bytes).unwrap();
    assert!(PackStore::open(&objects, &out.catalog_bytes, &out.catalog_sha256).is_err());
}

#[test]
fn resource_profile_is_opt_in_and_includes_all_startup_reads_before_mount() {
    let temp = tempfile::tempdir().unwrap();
    let (objects, out) = pack_fixture(temp.path(), None);
    let ordinary =
        open_resource_store(&objects, &out.catalog_bytes, &out.catalog_sha256, false).unwrap();
    assert!(ordinary.read_profile_snapshot().is_none());
    drop(ordinary);
    let profiled =
        open_resource_store(&objects, &out.catalog_bytes, &out.catalog_sha256, true).unwrap();
    let snapshot = profiled.read_profile_snapshot().unwrap();
    assert_eq!(snapshot.catalog_sha256, out.catalog_sha256);
    assert_eq!(snapshot.successful_read_operations, REQUIRED.len() as u64);
    assert_eq!(snapshot.failed_read_operations, 0);
    assert_eq!(snapshot.resources.len(), REQUIRED.len());
    assert!(REQUIRED
        .iter()
        .all(|path| snapshot.resources.get(*path) == Some(&1)));
}

#[test]
fn resource_profile_never_relaxes_missing_startup_or_full_pack_validation() {
    let temp = tempfile::tempdir().unwrap();
    let (objects, out) = pack_fixture(temp.path(), Some(REQUIRED[1]));
    assert!(open_resource_store(&objects, &out.catalog_bytes, &out.catalog_sha256, true).is_err());
    let complete = tempfile::tempdir().unwrap();
    let (objects, out) = pack_fixture(complete.path(), None);
    std::fs::remove_file(objects.join(out.catalog.packs[0].file_name())).unwrap();
    assert!(open_resource_store(&objects, &out.catalog_bytes, &out.catalog_sha256, true).is_err());
}

#[test]
fn progressive_mount_refuses_deferred_one_shot_inputs_but_allows_deferred_world_png() {
    use mir2_resource_pack::{BuildOptions, ResourceInput, ScopedPackStore, ScopedRead};
    let temp = tempfile::tempdir().unwrap();
    let (_, original) = pack_fixture(temp.path(), None);
    let source = temp.path().join("source");
    let extra = [
        "original-ui/Monster9/meta.json",
        "crystal-map-pack/other.map.gz",
        "original-audio/Main.wav",
        "original-ui/info.txt",
        "bevy-entity-atlases/starter.png",
        "original-ui/Monster9/20.png",
    ];
    let mut inputs = original
        .catalog
        .resources
        .iter()
        .map(|(path, entry)| ResourceInput {
            path: path.clone(),
            size: entry.size,
            sha256: entry.sha256.clone(),
        })
        .collect::<Vec<_>>();
    for path in extra {
        let bytes = format!("independent authenticated content: {path}").into_bytes();
        let file = source.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, &bytes).unwrap();
        inputs.push(ResourceInput {
            path: path.into(),
            size: bytes.len() as u64,
            sha256: mir2_resource_pack::sha256(&bytes),
        });
    }
    let objects = temp.path().join("separated");
    let out = mir2_resource_pack::build(
        &source,
        &objects,
        &inputs,
        None,
        BuildOptions {
            target_pack_bytes: 96,
        },
    )
    .unwrap();
    let cache = temp.path().join("cache");
    std::fs::create_dir(&cache).unwrap();
    let mut required = REQUIRED
        .iter()
        .map(|path| path.to_string())
        .collect::<Vec<_>>();
    required.sort();
    let scope = serde_json::to_vec(&serde_json::json!({
        "schema": "mir2.native.required-resource-scope.v1", "id":"native-startup-test",
        "catalogSha256":out.catalog_sha256, "requiredPaths":required
    }))
    .unwrap();
    for path in &required {
        let sha = &out.catalog.resources[path].pack_sha256;
        let pack = out
            .catalog
            .packs
            .iter()
            .find(|pack| &pack.sha256 == sha)
            .unwrap();
        let target = cache.join(pack.file_name());
        if !target.exists() {
            std::fs::copy(objects.join(pack.file_name()), target).unwrap();
        }
    }
    let store = ScopedPackStore::open(
        &cache,
        &out.catalog_bytes,
        &out.catalog_sha256,
        &scope,
        &mir2_resource_pack::sha256(&scope),
    )
    .unwrap();
    let error = crate::assets::verify_progressive_startup_assets(&store).unwrap_err();
    assert!(error.contains("pending"));
    for path in extra
        .iter()
        .filter(|path| **path != "original-ui/Monster9/20.png")
    {
        let sha = &out.catalog.resources[*path].pack_sha256;
        let pack = out
            .catalog
            .packs
            .iter()
            .find(|pack| &pack.sha256 == sha)
            .unwrap();
        std::fs::copy(objects.join(pack.file_name()), cache.join(pack.file_name())).unwrap();
        store.admit_cached_pack(sha).unwrap();
    }
    crate::assets::verify_progressive_startup_assets(&store).unwrap();
    assert!(matches!(
        store.read("original-ui/Monster9/20.png").unwrap(),
        ScopedRead::Pending(_)
    ));
    assert!(!store.is_fully_available().unwrap());
}

//! Real pinned-CMS engine-only integration, entirely local. Fixture executables
//! are deliberately non-PE bytes; only the OS rejection path is attempted.
#![cfg(windows)]
use anyhow::{ensure, Context, Result};
use mir2_windows_updater::{
    fs_safe as safe,
    generation::{self, Health, PackSource},
    generation_runtime::GameLease,
    model::{self, FileEntry},
    platform,
    update::{self, engine_maintenance as maintenance, Source, Status},
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Fixture {
    baseline_root: PathBuf,
    packed_root: PathBuf,
    output_root: PathBuf,
    engine_maintenance_root: PathBuf,
    engine_maintenance_same_sequence_root: PathBuf,
    engine_wrong_scope_root: PathBuf,
    engine_old_root: PathBuf,
    #[serde(rename = "lowerPackedRoot")]
    _lower: Option<PathBuf>,
    #[serde(rename = "sameSequencePackedRoot")]
    _same: Option<PathBuf>,
}
struct Quiet;
impl Status for Quiet {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        false
    }
}
struct Cancel(Arc<AtomicBool>);
impl Status for Cancel {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
struct Files {
    root: PathBuf,
    gets: Mutex<Vec<String>>,
    downloads: Mutex<Vec<String>>,
    bad_sig: bool,
    bad_payload: bool,
    cancel: Option<Arc<AtomicBool>>,
}
impl Files {
    fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            root: fs::canonicalize(root)?,
            gets: Mutex::new(Vec::new()),
            downloads: Mutex::new(Vec::new()),
            bad_sig: false,
            bad_payload: false,
            cancel: None,
        })
    }
    fn actual_feed(&self) -> Result<model::Feed> {
        let bytes = safe::read_bounded(&self.root.join("latest.json"), 32768)?;
        let sig = safe::read_bounded(&self.root.join("latest.p7s"), 32768)?;
        let feed = model::Feed::parse(&bytes, update::now(), true)?;
        platform::verify_cms_at(
            &bytes,
            &sig,
            mir2_windows_updater::SIGNING_KEY,
            feed.created_unix,
        )
        .map_err(anyhow::Error::msg)?;
        Ok(feed)
    }
}
impl Source for Files {
    fn get(
        &self,
        name: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.gets.lock().unwrap().push(name.into());
        let mut bytes = safe::read_bounded(&safe::target(&self.root, name)?, limit)?;
        if self.bad_sig && name == "latest.p7s" {
            bytes = vec![0, 1, 2];
        }
        progress(bytes.len() as u64)?;
        Ok(bytes)
    }
    fn download(
        &self,
        name: &str,
        entry: &FileEntry,
        dest: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        self.downloads.lock().unwrap().push(name.into());
        let mut bytes = safe::read_bounded(&safe::target(&self.root, name)?, entry.size)?;
        if self.bad_payload {
            bytes[0] ^= 1;
        }
        progress(bytes.len() as u64)?;
        safe::write_new(dest, &bytes)?;
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}
impl PackSource for Files {
    fn packed_pair(
        &self,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        Ok(Some((
            self.get("latest.json", 32768, progress)?,
            self.get("latest.p7s", 32768, progress)?,
        )))
    }
}
fn fresh(f: &Fixture, name: &str) -> Result<PathBuf> {
    let root = f.output_root.join(name);
    ensure!(!root.exists(), "fresh engine integration root required");
    let (engine, _) = update::active_engine(&f.baseline_root)?;
    update::verify_launch(&f.baseline_root)?;
    let mut files = vec![
        "Mir2Launcher.exe".to_owned(),
        "updater/active.txt".into(),
        "updater/seed-feed.json".into(),
        "updater/seed-feed.p7s".into(),
    ];
    files.extend(
        model::META
            .into_iter()
            .chain(["mir2-platform-windows.exe", "BUILD-ATTESTATION.json"])
            .map(|name| format!("game/{name}")),
    );
    files.extend(
        ["ENGINE.json", "ENGINE.p7s", "Mir2Updater.exe"]
            .map(|name| format!("updater/engines/{}/{name}", engine.exe_sha256)),
    );
    for name in files {
        safe::write_new(
            &safe::target(&root, &name)?,
            &safe::read_bounded(&safe::target(&f.baseline_root, &name)?, model::MAX_FILE)?,
        )?;
    }
    safe::write_new(&root.join("game/logs/player.log"), b"legacy personal")?;
    safe::write_new(&root.join("userdata/player.log"), b"packed personal")?;
    Ok(root)
}
fn witness(root: &Path) -> Result<BTreeMap<String, String>> {
    model::META
        .into_iter()
        .chain([
            "mir2-platform-windows.exe",
            "BUILD-ATTESTATION.json",
            "logs/player.log",
        ])
        .map(|name| {
            Ok((
                format!("game/{name}"),
                safe::digest_file(&root.join("game").join(name))?.1,
            ))
        })
        .chain(std::iter::once(Ok((
            "userdata/player.log".into(),
            safe::digest_file(&root.join("userdata/player.log"))?.1,
        ))))
        .collect()
}
fn engine_pointer(root: &Path) -> Result<Vec<u8>> {
    safe::read_bounded(&root.join("updater/active.txt"), 64)
}
fn assert_dummy_os_rejection(root: &Path, handoff: &maintenance::Handoff) -> Result<i32> {
    let (_, exe) = update::active_engine(root)?;
    let bytes = safe::read_bounded(&exe, model::MAX_FILE)?;
    let valid_pe_header = bytes.starts_with(b"MZ")
        && bytes.get(0x3c..0x40).is_some_and(|offset| {
            let offset = u32::from_le_bytes(offset.try_into().unwrap()) as usize;
            offset.checked_add(4).and_then(|end| bytes.get(offset..end))
                == Some(b"PE\0\0".as_slice())
        });
    ensure!(
        !valid_pe_header,
        "fixture must be deliberately non-executable; no game or engine code is run"
    );
    let error = handoff
        .spawn(true)
        .expect_err("non-PE fixture must fail in the OS loader");
    ensure!(
        matches!(error.raw_os_error(), Some(193 | 216)),
        "expected OS bad-executable rejection, got {error}"
    );
    Ok(error.raw_os_error().unwrap())
}
#[test]
#[ignore = "requires MIR2_UPDATER_ENGINE_FIXTURES with real pinned-key signed engine-only fixtures"]
fn signed_engine_only_patch_keeps_game_and_selector_and_os_failure_restores_only_engine(
) -> Result<()> {
    let path = std::env::var_os("MIR2_UPDATER_ENGINE_FIXTURES").context("engine fixture config")?;
    let f: Fixture = serde_json::from_slice(&safe::read_bounded(Path::new(&path), 32768)?)?;
    ensure!(
        !f.output_root.exists(),
        "fresh explicit integration output required"
    );
    fs::create_dir_all(&f.output_root)?;
    let root = fresh(&f, "healthy-pack")?;
    let _lock = platform::InstallLock::acquire(&root).map_err(anyhow::Error::msg)?;
    let _idle = GameLease::acquire(&root)?;
    assert!(
        maintenance::network_source(&root)?.is_none(),
        "synthetic signed baseline must not enable production network"
    );
    let pack = Files::new(&f.packed_root)?;
    generation::check_update(&root, &pack, &Quiet)?.context("pack target")?;
    let selected = generation::active_record(&root)?.unwrap();
    generation::begin_launch(&root, &selected.attempt)?;
    generation::accept(&root, &selected.attempt)?;
    let selector = fs::read(root.join("state/active-generation.json"))?;
    let original = witness(&root)?;
    let old_pointer = engine_pointer(&root)?;

    let old = Files::new(&f.engine_old_root)?;
    old.actual_feed()?;
    assert!(maintenance::check(&root, &old, &Quiet)?.is_none());
    assert_eq!(*old.gets.lock().unwrap(), ["latest.json", "latest.p7s"]);
    let wrong = Files::new(&f.engine_wrong_scope_root)?;
    wrong.actual_feed()?;
    assert!(maintenance::check(&root, &wrong, &Quiet).is_err());
    assert_eq!(*wrong.gets.lock().unwrap(), ["latest.json", "latest.p7s"]);
    let mut bad = Files::new(&f.engine_maintenance_root)?;
    bad.bad_sig = true;
    assert!(maintenance::check(&root, &bad, &Quiet).is_err());
    assert_eq!(
        bad.gets.lock().unwrap().len(),
        6,
        "three complete discovery pairs, no engine fetch"
    );
    assert_eq!(engine_pointer(&root)?, old_pointer);
    assert_eq!(
        fs::read(root.join("state/active-generation.json"))?,
        selector
    );

    let source = Files::new(&f.engine_maintenance_root)?;
    let signed_feed = source.actual_feed()?;
    let (old_engine, _) = update::active_engine(&root)?;
    let handoff = maintenance::check(&root, &source, &Quiet)?.context("signed engine patch")?;
    assert_eq!(handoff.sequence, signed_feed.sequence);
    assert_ne!(engine_pointer(&root)?, old_pointer);
    let (patch_engine, _) = update::active_engine(&root)?;
    assert_eq!(patch_engine.version, old_engine.version);
    assert_ne!(patch_engine.source_revision, old_engine.source_revision);
    assert_eq!(
        source.downloads.lock().unwrap().as_slice(),
        [format!("{}/Mir2Updater.exe", signed_feed.engine.directory)]
    );
    assert_eq!(witness(&root)?, original);
    assert_eq!(
        fs::read(root.join("state/active-generation.json"))?,
        selector
    );
    let unchanged = generation::check_update(&root, &pack, &Quiet)?
        .context("current pack under compatible newer engine")?;
    assert!(!unchanged.activated && unchanged.downloaded_packs == 0);
    assert_eq!(
        generation::active_record(&root)?.unwrap().health,
        Health::Healthy
    );
    let floor = fs::read(root.join("state/engine-maintenance-highest.txt"))?;

    // A changed pointer cannot be overwritten by a stale handoff rollback.
    safe::atomic_write(&root.join("updater/active.txt"), &old_pointer)?;
    assert!(maintenance::failed_spawn(&root, &handoff).is_err());
    assert_eq!(engine_pointer(&root)?, old_pointer);
    safe::atomic_write(
        &root.join("updater/active.txt"),
        handoff.engine_sha256.as_bytes(),
    )?;
    let first_os_error = assert_dummy_os_rejection(&root, &handoff)?;
    maintenance::failed_spawn(&root, &handoff)?;
    assert_eq!(engine_pointer(&root)?, old_pointer);
    assert_eq!(
        fs::read(root.join("state/engine-maintenance-highest.txt"))?,
        floor
    );
    assert!(root
        .join(format!(
            "state/engine-quarantine/{}.json",
            handoff.engine_sha256
        ))
        .exists());
    let repeat = Files::new(&f.engine_maintenance_root)?;
    assert!(maintenance::check(&root, &repeat, &Quiet).is_err());
    assert!(repeat.downloads.lock().unwrap().is_empty());
    let collision = Files::new(&f.engine_maintenance_same_sequence_root)?;
    assert_eq!(collision.actual_feed()?.sequence, signed_feed.sequence);
    assert!(maintenance::check(&root, &collision, &Quiet).is_err());
    assert_eq!(
        *collision.gets.lock().unwrap(),
        ["latest.json", "latest.p7s"]
    );
    assert_eq!(
        fs::read(root.join("state/active-generation.json"))?,
        selector
    );
    assert_eq!(witness(&root)?, original);

    // Corrupt body and cancellation keep the old engine and no selector. A
    // valid retry reuses the wholly verified engine object without downloading.
    let fresh_root = fresh(&f, "fresh-floor")?;
    let _fresh_lock = platform::InstallLock::acquire(&fresh_root).map_err(anyhow::Error::msg)?;
    let _fresh_idle = GameLease::acquire(&fresh_root)?;
    let fresh_witness = witness(&fresh_root)?;
    let mut corrupt = Files::new(&f.engine_maintenance_root)?;
    corrupt.bad_payload = true;
    assert!(maintenance::check(&fresh_root, &corrupt, &Quiet).is_err());
    assert_eq!(engine_pointer(&fresh_root)?, old_pointer);
    let flag = Arc::new(AtomicBool::new(false));
    let mut cancelled = Files::new(&f.engine_maintenance_root)?;
    cancelled.cancel = Some(flag.clone());
    assert!(maintenance::check(&fresh_root, &cancelled, &Cancel(flag)).is_err());
    assert_eq!(engine_pointer(&fresh_root)?, old_pointer);
    assert!(!fresh_root
        .join("state/engine-maintenance-highest.txt")
        .exists());
    let retry = Files::new(&f.engine_maintenance_root)?;
    let fresh_handoff =
        maintenance::check(&fresh_root, &retry, &Quiet)?.context("zero-payload retry")?;
    assert!(retry.downloads.lock().unwrap().is_empty());
    let below = Files::new(&f.packed_root)?;
    assert!(generation::check_update(&fresh_root, &below, &Quiet).is_err());
    assert_eq!(
        *below.gets.lock().unwrap(),
        ["latest.json", "latest.p7s"],
        "fresh pack below maintenance floor rejected before manifest/pack requests"
    );
    assert!(generation::active_record(&fresh_root)?.is_none());
    let second_os_error = assert_dummy_os_rejection(&fresh_root, &fresh_handoff)?;
    maintenance::failed_spawn(&fresh_root, &fresh_handoff)?;
    assert_eq!(witness(&fresh_root)?, fresh_witness);
    safe::write_new(
        &f.output_root.join("engine-maintenance-integration.json"),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"mir2.windows.engine-maintenance-integration.v1","realPinnedCms":true,"sequence":handoff.sequence,
            "patchEngineSha256":handoff.engine_sha256,"previousEngineSha256":handoff.previous_engine_sha256,
            "sameVersionSignedPatch":true,"syntheticBaselineProductionNetworkDisabled":true,"wrongScopeRejectedBeforeObjects":true,
            "badCmsRejected":true,"gameAndSelectorUnchanged":true,"compatibleCurrentPackRetained":true,
        "sameSequenceCollisionRejected":true,"staleFeedIgnored":true,"osSpawnFailureRestoredPointer":true,"osErrorCodes":[first_os_error,second_os_error],
            "changedPointerCasRejected":true,"floorNotLowered":true,"corruptBodyRejected":true,"cancelBeforePointerPreserved":true,
            "retryPayloadDownloads":0,"freshPackBelowEngineFloorRejected":true,"personalMarkersPreserved":true,
            "engineProcessHealth":"not exercised; intentionally non-PE OS rejection only","gameProcess":"not launched","network":"local signed files only"
        }))?,
    )?;
    Ok(())
}

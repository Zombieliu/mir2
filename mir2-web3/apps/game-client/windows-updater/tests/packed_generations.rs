//! Offline integration using real pinned-key signatures. This exercises the
//! updater's durable selector, not interactive readiness or public networking.
#![cfg(windows)]
use anyhow::{ensure, Context, Result};
use mir2_windows_updater::{
    fs_safe as safe,
    generation::{self, Health, PackSource, Selection},
    generation_runtime::GameLease,
    model::{self, FileEntry},
    update::{self, Source, Status},
};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Fixture {
    baseline_root: PathBuf,
    packed_root: PathBuf,
    output_root: PathBuf,
    lower_packed_root: Option<PathBuf>,
    same_sequence_packed_root: Option<PathBuf>,
}
struct Quiet;
impl Status for Quiet {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        false
    }
}
struct Files {
    root: PathBuf,
    downloads: Mutex<Vec<String>>,
    gets: Mutex<Vec<String>>,
}
impl Files {
    fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            root: fs::canonicalize(root)?,
            downloads: Mutex::new(Vec::new()),
            gets: Mutex::new(Vec::new()),
        })
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
        let bytes = safe::read_bounded(&safe::target(&self.root, name)?, limit)?;
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
        let source = safe::target(&self.root, name)?;
        ensure!(
            safe::matches(&source, entry)?,
            "signed fixture object hash differs"
        );
        let bytes = safe::read_bounded(&source, entry.size)?;
        progress(bytes.len() as u64)?;
        safe::write_new(dest, &bytes)?;
        self.downloads.lock().unwrap().push(name.into());
        Ok(())
    }
}
impl PackSource for Files {
    fn packed_pair(
        &self,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        let json = self.get("latest.json", 32768, &mut |n| progress(n))?;
        let sig = self.get("latest.p7s", 32768, &mut |n| progress(n))?;
        Ok(Some((json, sig)))
    }
}
fn copy(root: &Path, dest: &Path, name: &str, limit: u64) -> Result<()> {
    safe::write_new(
        &safe::target(dest, name)?,
        &safe::read_bounded(&safe::target(root, name)?, limit)?,
    )
}
fn installation(fixture: &Fixture, name: &str) -> Result<PathBuf> {
    let root = fixture.output_root.join(name);
    ensure!(!root.exists(), "integration installation must be fresh");
    update::verify_launch(&fixture.baseline_root)?;
    let (engine, _) = update::active_engine(&fixture.baseline_root)?;
    ensure!(
        engine.version >= 2,
        "real signed version2 engine fixture required"
    );
    copy(
        &fixture.baseline_root,
        &root,
        "Mir2Launcher.exe",
        model::MAX_FILE,
    )?;
    for name in model::META
        .into_iter()
        .chain(["mir2-platform-windows.exe", "BUILD-ATTESTATION.json"])
    {
        copy(
            &fixture.baseline_root,
            &root,
            &format!("game/{name}"),
            model::MAX_FILE,
        )?;
    }
    for name in [
        "updater/active.txt",
        "updater/seed-feed.json",
        "updater/seed-feed.p7s",
    ] {
        copy(&fixture.baseline_root, &root, name, 32768)?;
    }
    for name in ["Mir2Updater.exe", "ENGINE.json", "ENGINE.p7s"] {
        copy(
            &fixture.baseline_root,
            &root,
            &format!("updater/engines/{}/{name}", engine.exe_sha256),
            model::MAX_FILE,
        )?;
    }
    safe::write_new(
        &root.join("game/logs/player.log"),
        b"legacy personal witness",
    )?;
    safe::write_new(
        &root.join("userdata/player.log"),
        b"packed personal witness",
    )?;
    let _signed_exe_lease = safe::FileLease::verified(
        &root.join("game/mir2-platform-windows.exe"),
        &update::launch_executable_entry(&root)?,
    )?;
    Ok(root)
}
fn witnesses(root: &Path, original_version: &[u8]) -> Result<()> {
    ensure!(
        fs::read(root.join("game/VERSION.json"))? == original_version,
        "legacy VERSION mutated"
    );
    ensure!(
        fs::read(root.join("game/logs/player.log"))? == b"legacy personal witness",
        "legacy personal data changed"
    );
    ensure!(
        fs::read(root.join("userdata/player.log"))? == b"packed personal witness",
        "packed personal data changed"
    );
    Ok(())
}

#[test]
#[ignore = "requires MIR2_UPDATER_PACK_FIXTURES with real pinned-key signed generation and version2 engine"]
fn signed_generation_write_failure_retry_pending_and_interrupted_launch_rollback() -> Result<()> {
    let path =
        std::env::var_os("MIR2_UPDATER_PACK_FIXTURES").context("signed pack fixture config")?;
    let fixture: Fixture = serde_json::from_slice(&safe::read_bounded(Path::new(&path), 32768)?)?;
    safe::ancestors(&fixture.output_root)?;
    ensure!(
        !fixture.output_root.exists(),
        "integration output must be fresh; never edits installed game"
    );
    fs::create_dir_all(&fixture.output_root)?;
    let root = installation(&fixture, "selector-write-failure")?;
    let original_version = fs::read(root.join("game/VERSION.json"))?;
    let source = Files::new(&fixture.packed_root)?;
    let _lease = GameLease::acquire(&root)?;

    // A real failed atomic-write precondition after authenticated pack download
    // leaves the complete old selection. This is not a cryptographic bypass.
    fs::create_dir_all(root.join("state/active-generation.tmp"))?;
    assert!(generation::check_update(&root, &source, &Quiet).is_err());
    assert!(generation::active_record(&root)?.is_none());
    witnesses(&root, &original_version)?;
    let floor = fs::read(root.join("state/pack-highest.txt"))?;
    let initial_downloads = source.downloads.lock().unwrap().len();
    assert!(
        initial_downloads > 3,
        "fixture must download real pack objects and controls"
    );
    fs::remove_dir(root.join("state/active-generation.tmp"))?;

    let outcome = generation::check_update(&root, &source, &Quiet)?.context("packed outcome")?;
    assert!(outcome.activated && outcome.downloaded_packs == 0);
    assert_eq!(
        source.downloads.lock().unwrap().len(),
        initial_downloads,
        "verified immutable payloads must be reused"
    );
    let pending = generation::active_record(&root)?.unwrap();
    assert_eq!(pending.health, Health::Pending);
    assert!(!pending.launch_started);
    assert!(matches!(pending.previous, Some(Selection::LegacyV4 { .. })));
    let verified = generation::verify_generation_for_launch(&root, &outcome.manifest_sha256)?;
    let _packs = mir2_resource_pack::PackStore::open(
        root.join("objects/packs"),
        &verified.catalog_bytes,
        &verified.manifest.catalog.sha256,
    )?;
    assert!(
        !generation::recover(&root)?,
        "check-only pending cannot be accepted or rolled back as a started game"
    );
    witnesses(&root, &original_version)?;

    // Exact catalog tamper is rejected without reselecting the retained game.
    let catalog_path = verified.root.join("catalog.json");
    let catalog_bytes = fs::read(&catalog_path)?;
    fs::write(&catalog_path, b"tamper")?;
    assert!(generation::verify_generation_for_launch(&root, &outcome.manifest_sha256).is_err());
    fs::write(catalog_path, &catalog_bytes)?;
    assert_eq!(
        generation::active_record(&root)?.unwrap().health,
        Health::Pending
    );
    drop(_packs);

    // Simulate interruption AFTER the durable launch obligation, without
    // claiming that an interactive game/ready handshake has occurred.
    generation::begin_launch(&root, &pending.attempt)?;
    assert!(generation::recover(&root)?);
    let restored = generation::verify_current_selection(&root)?;
    assert_eq!(restored.health, Health::Healthy);
    assert!(matches!(restored.active, Selection::LegacyV4 { .. }));
    assert!(!restored.launch_started && restored.previous.is_none());
    assert_eq!(
        fs::read(root.join("state/pack-highest.txt"))?,
        floor,
        "health rollback cannot lower the signed floor"
    );
    assert!(root
        .join(format!("state/quarantine/{}.json", outcome.manifest_sha256))
        .is_file());
    assert!(
        generation::check_update(&root, &source, &Quiet).is_err(),
        "quarantine must reject repeated failed target"
    );
    let continued = generation::verified_healthy_selection(&root)?
        .context("explicit rollback must remain usable offline")?;
    assert!(matches!(continued.active, Selection::LegacyV4 { .. }));
    assert_eq!(
        source.downloads.lock().unwrap().len(),
        initial_downloads,
        "continuation cannot request a v4 payload"
    );
    witnesses(&root, &original_version)?;

    // This second installation directly exercises the authenticated installed
    // baseline when its separate highest pointer is lost. accept is invoked by
    // the test to construct a healthy state; no game health is being claimed.
    let healthy_root = installation(&fixture, "signed-installed-floor")?;
    let _healthy_lease = GameLease::acquire(&healthy_root)?;
    let healthy_outcome = generation::check_update(&healthy_root, &source, &Quiet)?
        .context("signed floor outcome")?;
    let selected = generation::active_record(&healthy_root)?.unwrap();
    generation::begin_launch(&healthy_root, &selected.attempt)?;
    generation::accept(&healthy_root, &selected.attempt)?;
    let highest_path = healthy_root.join("state/pack-highest.txt");
    let highest_bytes = fs::read(&highest_path)?;
    fs::remove_file(&highest_path)?;
    let mut signed_replay_variants = 0;
    let main_feed = fs::read(fixture.packed_root.join("latest.json"))?;
    for (same_sequence, variant) in [
        (false, &fixture.lower_packed_root),
        (true, &fixture.same_sequence_packed_root),
    ] {
        let Some(variant) = variant else { continue };
        let replay = Files::new(variant)?;
        let replay_bytes = safe::read_bounded(&replay.root.join("latest.json"), 32768)?;
        let replay_sig = safe::read_bounded(&replay.root.join("latest.p7s"), 32768)?;
        let replay_feed: generation::PackFeed = serde_json::from_slice(&replay_bytes)?;
        mir2_windows_updater::platform::verify_cms_at(
            &replay_bytes,
            &replay_sig,
            mir2_windows_updater::SIGNING_KEY,
            replay_feed.created_unix,
        )
        .map_err(anyhow::Error::msg)?;
        if same_sequence {
            assert_eq!(replay_feed.sequence, healthy_outcome.sequence);
            assert_eq!(replay_feed.manifest.sha256, healthy_outcome.manifest_sha256);
            assert_ne!(model::hash(&replay_bytes), model::hash(&main_feed));
        } else {
            assert!(replay_feed.sequence < healthy_outcome.sequence);
        }
        assert!(generation::check_update(&healthy_root, &replay, &Quiet).is_err());
        assert_eq!(
            *replay.gets.lock().unwrap(),
            ["latest.json", "latest.p7s"],
            "installed-floor rejection precedes target-object requests"
        );
        assert!(replay.downloads.lock().unwrap().is_empty() && !highest_path.exists());
        assert_eq!(
            generation::active_record(&healthy_root)?.unwrap().active,
            selected.active
        );
        signed_replay_variants += 1;
    }
    let unchanged = generation::check_update(&healthy_root, &source, &Quiet)?
        .context("same signed active target")?;
    assert!(!unchanged.activated && unchanged.downloaded_packs == 0);
    assert_eq!(fs::read(highest_path)?, highest_bytes);
    assert_eq!(
        generation::active_record(&healthy_root)?.unwrap().health,
        Health::Healthy
    );
    witnesses(&healthy_root, &original_version)?;

    let damaged_root = installation(&fixture, "damaged-pending")?;
    let _damaged_lease = GameLease::acquire(&damaged_root)?;
    let damaged_outcome = generation::check_update(&damaged_root, &source, &Quiet)?
        .context("damaged pending outcome")?;
    let damaged =
        generation::verify_generation_for_launch(&damaged_root, &damaged_outcome.manifest_sha256)?;
    let damaged_floor = fs::read(damaged_root.join("state/pack-highest.txt"))?;
    // No launch was started: an incomplete/corrupted first-use generation must
    // still be quarantined and restore only its authenticated predecessor.
    fs::remove_file(damaged.root.join("catalog.json"))?;
    assert!(generation::recover(&damaged_root)?);
    let recovered = generation::verified_healthy_selection(&damaged_root)?
        .context("damaged pending recovery")?;
    assert!(matches!(recovered.active, Selection::LegacyV4 { .. }));
    assert_eq!(
        fs::read(damaged_root.join("state/pack-highest.txt"))?,
        damaged_floor
    );
    assert!(damaged_root
        .join(format!(
            "state/quarantine/{}.json",
            damaged_outcome.manifest_sha256
        ))
        .is_file());
    witnesses(&damaged_root, &original_version)?;
    safe::write_new(
        &fixture
            .output_root
            .join("signed-generation-integration.json"),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"mir2.windows.signed-generation-integration.v1","sequence":outcome.sequence,"manifestSha256":outcome.manifest_sha256,
            "gameSourceRevision":verified.manifest.game_source_revision,"engineSourceRevision":verified.manifest.engine_source_revision,
            "realPinnedCms":true,"selectorWriteFailurePreservedLegacy":true,"retryPayloadDownloads":0,
            "checkOnlyKeptPending":true,"catalogTamperRejected":true,"interruptedLaunchRolledBack":true,
        "highestFloorPreserved":true,"personalMarkersPreserved":true,"explicitLegacyRollbackContinuesWithoutV4Network":true,
        "activeManifestAfterFloorPointerLoss":healthy_outcome.manifest_sha256,"signedReplayVariantsRejected":signed_replay_variants,
        "corruptUnstartedPendingQuarantinedAndRolledBack":true,
        "healthStateSetup":"accept API invoked by test; no process readiness claimed","interactiveGameHealth":"not exercised","network":"local signed files only"
        }))?,
    )?;
    Ok(())
}

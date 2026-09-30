//! Real signed-package delta integration. Requires preserved r5/r6 and the freshly
//! attested updater bundle; never launches or edits the user's installed game.
#![cfg(windows)]
use anyhow::{ensure, Result};
use mir2_windows_updater::{
    fs_safe as safe,
    model::{self, FileEntry, Manifest},
    transaction,
    update::{self, Source, Status},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

struct Quiet;
impl Status for Quiet {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        false
    }
}
struct Files {
    root: PathBuf,
    game_dir: String,
    game_root: PathBuf,
    engine_dir: String,
    engine_root: PathBuf,
    requests: Mutex<Vec<String>>,
}
impl Files {
    fn file(&self, name: &str) -> PathBuf {
        if let Some(rest) = name.strip_prefix(&format!("{}/", self.game_dir)) {
            self.game_root.join(rest)
        } else if let Some(rest) = name.strip_prefix(&format!("{}/", self.engine_dir)) {
            self.engine_root.join(rest)
        } else {
            self.root.join(name)
        }
    }
}
impl Source for Files {
    fn get(
        &self,
        path: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>> {
        let bytes = safe::read_bounded(&self.file(path), limit)?;
        progress(bytes.len() as u64)?;
        Ok(bytes)
    }
    fn download(
        &self,
        path: &str,
        entry: &FileEntry,
        dest: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        self.requests.lock().unwrap().push(path.into());
        ensure!(safe::matches(&self.file(path), entry)?, "fixture hash");
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::copy(self.file(path), dest)?;
        progress(entry.size)?;
        Ok(())
    }
}
fn copy_tree(src: &Path, dest: &Path) -> Result<()> {
    safe::ancestors(src)?;
    fs::create_dir_all(dest)?;
    for item in fs::read_dir(src)? {
        let item = item?;
        let path = item.path();
        let next = dest.join(item.file_name());
        if item.file_type()?.is_dir() {
            copy_tree(&path, &next)?
        } else {
            safe::regular(&path)?;
            fs::copy(&path, &next)?;
        }
    }
    Ok(())
}
#[test]
#[ignore = "requires MIR2_UPDATER_REAL_FIXTURES JSON generated after clean signed build"]
fn preserved_r5_to_r6_downloads_only_three_files_and_preserves_personal_files() {
    let config: serde_json::Value = serde_json::from_slice(
        &fs::read(std::env::var_os("MIR2_UPDATER_REAL_FIXTURES").unwrap()).unwrap(),
    )
    .unwrap();
    let path = |key: &str| PathBuf::from(config[key].as_str().unwrap());
    let old = path("r5");
    let new = path("r6");
    let bundle = path("bundle");
    let feed_root = path("feed");
    let engine_root = path("engine");
    let output = path("output");
    assert!(!output.exists(), "fresh integration output");
    fs::create_dir_all(&output).unwrap();
    let root = output.join("installation");
    copy_tree(&old, &root.join("game")).unwrap();
    copy_tree(&bundle, &root).unwrap();
    // Authenticated r5 floor is a QA-only signed fixture, never the public pointer.
    fs::copy(path("r5Seed"), root.join("updater/seed-feed.json")).unwrap();
    fs::copy(path("r5SeedSignature"), root.join("updater/seed-feed.p7s")).unwrap();
    safe::write_new(&root.join("game/logs/player.log"), b"saved diagnostic").unwrap();
    safe::write_new(&root.join("game/personal-file.txt"), b"personal file").unwrap();
    let feed: serde_json::Value =
        serde_json::from_slice(&fs::read(feed_root.join("latest.json")).unwrap()).unwrap();
    let files = Files {
        root: feed_root,
        game_dir: feed["game"]["directory"].as_str().unwrap().into(),
        game_root: new.clone(),
        engine_dir: feed["engine"]["directory"].as_str().unwrap().into(),
        engine_root,
        requests: Mutex::new(vec![]),
    };
    let result = update::check_update(&root, &files, &Quiet).unwrap();
    assert_eq!(result.changed_game_files, 3);
    assert_eq!(result.downloaded_files, 3);
    assert!(result.activated);
    assert!(transaction::pending_activation(&root).unwrap());
    // Simulate relaunch after --check-only/power loss: health obligation retained.
    let pending = update::check_update(&root, &files, &Quiet).unwrap();
    assert!(pending.activated);
    assert_eq!(pending.downloaded_files, 0);
    assert_eq!(files.requests.lock().unwrap().len(), 3);
    let requested = files.requests.lock().unwrap().clone();
    for name in [
        "BUILD-ATTESTATION.json",
        "README-START.txt",
        "mir2-platform-windows.exe",
    ] {
        assert!(requested.iter().any(|p| p.ends_with(name)), "{name}");
    }
    assert!(!requested.iter().any(|p| p.contains("mir2-assets/")));
    let old_manifest =
        Manifest::parse(&fs::read(old.join("PACKAGE-MANIFEST.json")).unwrap()).unwrap();
    let new_manifest =
        Manifest::parse(&fs::read(new.join("PACKAGE-MANIFEST.json")).unwrap()).unwrap();
    let old_map = old_manifest.map();
    let mut unchanged = 0;
    for entry in &new_manifest.files {
        assert!(safe::matches(&root.join("game").join(&entry.path), entry).unwrap());
        if old_map
            .get(entry.path.as_str())
            .is_some_and(|f| f.sha256 == entry.sha256)
        {
            unchanged += 1;
        }
    }
    assert_eq!(unchanged, 123028);
    assert_eq!(
        fs::read(root.join("game/logs/player.log")).unwrap(),
        b"saved diagnostic"
    );
    assert_eq!(
        fs::read(root.join("game/personal-file.txt")).unwrap(),
        b"personal file"
    );
    // Roll back before acceptance, keeping the signed highest sequence.
    update::quarantine_and_rollback(&root).unwrap();
    assert_eq!(
        update::verify_launch(&root).unwrap(),
        "WN-CANDIDATE-20260930-invited-05"
    );
    assert!(update::check_update(&root, &files, &Quiet).is_err());
    assert_eq!(
        fs::read(root.join("game/logs/player.log")).unwrap(),
        b"saved diagnostic"
    );
    let report = serde_json::json!({"passed":true,"result":result,"unchangedPayloadFiles":unchanged,
        "requests":requested,"allR6PayloadHashesVerified":true,"rollback":"signed r5 restored",
        "pendingFirstLaunchRetained":true,"failedReleaseQuarantined":true,"gameLaunched":false});
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
#[test]
#[ignore = "requires MIR2_UPDATER_REAL_FIXTURES with preserved signed r6"]
fn real_r6_metadata_matches_native_cms_key_pin_and_rejects_tampering() {
    let config: serde_json::Value = serde_json::from_slice(
        &fs::read(std::env::var_os("MIR2_UPDATER_REAL_FIXTURES").unwrap()).unwrap(),
    )
    .unwrap();
    let root = PathBuf::from(config["r6"].as_str().unwrap());
    let mut meta = BTreeMap::new();
    for name in model::META {
        meta.insert(name.into(), fs::read(root.join(name)).unwrap());
    }
    mir2_windows_updater::platform::verify_cms(
        &meta["RELEASE-STATEMENT.json"],
        &meta["RELEASE-STATEMENT.p7s"],
        mir2_windows_updater::SIGNING_KEY,
    )
    .unwrap();
    let manifest = model::bind_game(&meta).unwrap();
    assert_eq!(manifest.file_count, 123031);
    let mut signature = meta["RELEASE-STATEMENT.p7s"].clone();
    *signature.last_mut().unwrap() ^= 1;
    assert!(mir2_windows_updater::platform::verify_cms(
        &meta["RELEASE-STATEMENT.json"],
        &signature,
        mir2_windows_updater::SIGNING_KEY
    )
    .is_err());
    meta.get_mut("VERSION.json").unwrap()[30] ^= 1;
    assert!(model::bind_game(&meta).is_err());
}

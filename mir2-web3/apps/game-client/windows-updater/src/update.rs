use crate::{
    fs_safe as safe,
    model::*,
    transaction::{self, Change},
};
use anyhow::{ensure, Context, Result};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub trait Source {
    fn get(
        &self,
        path: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>>;
    fn download(
        &self,
        path: &str,
        file: &FileEntry,
        destination: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()>;
}
pub struct HttpsSource {
    base: url::Url,
    agent: ureq::Agent,
}
impl HttpsSource {
    pub fn new() -> Result<Self> {
        let base = url::Url::parse(crate::FEED_URL)?.join("./")?;
        ensure!(
            base.scheme() == "https" && base.username().is_empty() && base.password().is_none(),
            "invalid update origin"
        );
        Ok(Self {
            base,
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout_connect(Duration::from_secs(8))
                .timeout_read(Duration::from_secs(30))
                .timeout_write(Duration::from_secs(30))
                .build(),
        })
    }
    fn open(&self, path: &str) -> Result<ureq::Response> {
        relative(path)?;
        let url = self.base.join(path)?;
        ensure!(
            url.origin() == self.base.origin() && url.path().starts_with(self.base.path()),
            "update origin escape"
        );
        let response = self
            .agent
            .get(url.as_str())
            .set("Accept-Encoding", "identity")
            .call()?;
        ensure!(
            response.status() == 200
                && response.header("Content-Encoding").unwrap_or("identity") == "identity",
            "invalid update response"
        );
        Ok(response)
    }
}
impl Source for HttpsSource {
    fn get(
        &self,
        path: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>> {
        let r = self.open(path)?;
        if let Some(len) = r.header("Content-Length") {
            ensure!(len.parse::<u64>()? <= limit, "response too large");
        }
        let mut result = Vec::new();
        let mut reader = r.into_reader().take(limit + 1);
        let mut buffer = [0u8; 65536];
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            result.extend_from_slice(&buffer[..n]);
            ensure!(result.len() as u64 <= limit, "response too large");
            progress(result.len() as u64)?;
        }
        Ok(result)
    }
    fn download(
        &self,
        path: &str,
        entry: &FileEntry,
        destination: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        let r = self.open(path)?;
        if let Some(len) = r.header("Content-Length") {
            ensure!(len.parse::<u64>()? == entry.size, "payload length mismatch");
        }
        safe::ancestors(destination)?;
        fs::create_dir_all(destination.parent().unwrap())?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        let mut reader = r.into_reader().take(entry.size + 1);
        let mut total = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            ensure!(total <= entry.size, "oversized payload");
            file.write_all(&buffer[..n])?;
            progress(total)?;
        }
        file.sync_all()?;
        ensure!(
            total == entry.size && safe::matches(destination, entry)?,
            "truncated/corrupt download"
        );
        Ok(())
    }
}
pub trait Status {
    fn set(&self, key: &str, percent: u32);
    fn cancelled(&self) -> bool;
}
fn progress(status: &impl Status, key: &str, percent: u32) -> Result<()> {
    ensure!(!status.cancelled(), "update cancelled");
    status.set(key, percent);
    Ok(())
}
fn cms(bytes: &[u8], sig: &[u8]) -> Result<()> {
    crate::platform::verify_cms(bytes, sig, crate::SIGNING_KEY).map_err(anyhow::Error::msg)
}
fn feed_auth(bytes: &[u8], sig: &[u8], fresh: bool) -> Result<Feed> {
    let feed = Feed::parse(bytes, now(), fresh)?;
    crate::platform::verify_cms_at(bytes, sig, crate::SIGNING_KEY, feed.created_unix)
        .map_err(anyhow::Error::msg)?;
    Ok(feed)
}
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn engine_at(root: &Path, digest: &str) -> Result<(Engine, PathBuf)> {
    ensure!(valid_hash(digest), "invalid engine pointer");
    let dir = safe::target(root, &format!("updater/engines/{digest}"))?;
    let bytes = safe::read_bounded(&dir.join("ENGINE.json"), 32768)?;
    let signature = safe::read_bounded(&dir.join("ENGINE.p7s"), 32768)?;
    let engine = Engine::parse(&bytes)?;
    crate::platform::verify_cms_at(&bytes, &signature, crate::SIGNING_KEY, engine.built_unix)
        .map_err(anyhow::Error::msg)?;
    ensure!(engine.exe_sha256 == digest, "engine generation mismatch");
    let exe = dir.join("Mir2Updater.exe");
    ensure!(
        safe::matches(
            &exe,
            &FileEntry {
                path: "Mir2Updater.exe".into(),
                size: engine.exe_size,
                sha256: engine.exe_sha256.clone()
            }
        )?,
        "engine executable hash mismatch"
    );
    Ok((engine, exe))
}
pub fn active_engine(root: &Path) -> Result<(Engine, PathBuf)> {
    let pointer = safe::read_bounded(&root.join("updater/active.txt"), 64)?;
    engine_at(root, std::str::from_utf8(&pointer)?)
}
fn local_game(root: &Path) -> Result<(Manifest, BTreeMap<String, Vec<u8>>)> {
    let mut meta = BTreeMap::new();
    for name in META {
        meta.insert(
            name.into(),
            safe::read_bounded(&root.join("game").join(name), MAX_META)?,
        );
    }
    cms(
        &meta["RELEASE-STATEMENT.json"],
        &meta["RELEASE-STATEMENT.p7s"],
    )?;
    let manifest = bind_game(&meta)?;
    Ok((manifest, meta))
}
pub fn verify_launch(root: &Path) -> Result<String> {
    let (manifest, meta) = local_game(root)?;
    ensure!(
        safe::matches(
            &root.join("game/mir2-platform-windows.exe"),
            manifest.map()["mir2-platform-windows.exe"]
        )?,
        "installed game executable failed verification"
    );
    let attest = manifest.map()["BUILD-ATTESTATION.json"];
    ensure!(
        safe::matches(&root.join("game/BUILD-ATTESTATION.json"), attest)?,
        "installed build attestation failed verification"
    );
    let version: serde_json::Value = serde_json::from_slice(&meta["VERSION.json"])?;
    Ok(version["candidate"]
        .as_str()
        .context("missing candidate")?
        .to_owned())
}
fn read_previous(root: &Path) -> Result<Option<(Feed, Vec<u8>)>> {
    let highest = root.join(".update/highest.txt");
    if highest.exists() {
        let pointer = safe::read_bounded(&highest, 64)?;
        let digest = std::str::from_utf8(&pointer)?;
        ensure!(valid_hash(digest), "invalid highest-release pointer");
        let dir = safe::target(root, &format!(".update/receipts/{digest}"))?;
        let bytes = safe::read_bounded(&dir.join("FEED.json"), 32768)?;
        let sig = safe::read_bounded(&dir.join("FEED.p7s"), 32768)?;
        ensure!(hash(&bytes) == digest, "highest-release hash mismatch");
        return Ok(Some((feed_auth(&bytes, &sig, false)?, bytes)));
    }
    for (json, sig) in [
        (".update/accepted-feed.json", ".update/accepted-feed.p7s"),
        ("updater/seed-feed.json", "updater/seed-feed.p7s"),
    ] {
        let p = root.join(json);
        if p.exists() {
            let bytes = safe::read_bounded(&p, 32768)?;
            let signature = safe::read_bounded(&root.join(sig), 32768)?;
            let feed = feed_auth(&bytes, &signature, false)?;
            if json == "updater/seed-feed.json" {
                let version = safe::read_bounded(&root.join("game/VERSION.json"), MAX_META)?;
                ensure!(
                    feed.game
                        .metadata
                        .iter()
                        .any(|e| e.path == "VERSION.json" && e.sha256 == hash(&version)),
                    "updated installation lost its authenticated release history"
                );
            }
            return Ok(Some((feed, bytes)));
        }
    }
    anyhow::bail!("signed installer seed/release history is missing")
}
fn remember_highest(root: &Path, bytes: &[u8], sig: &[u8]) -> Result<()> {
    let digest = hash(bytes);
    let dir = safe::target(root, &format!(".update/receipts/{digest}"))?;
    safe::atomic_write(&dir.join("FEED.json"), bytes)?;
    safe::atomic_write(&dir.join("FEED.p7s"), sig)?;
    safe::atomic_write(&root.join(".update/highest.txt"), digest.as_bytes())
}
fn remote_meta(
    source: &impl Source,
    component: &Component,
    status: &impl Status,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    for entry in &component.metadata {
        let bytes = source.get(
            &format!("{}/{}", component.directory, entry.path),
            entry.size,
            &mut |_| progress(status, "checking", 10),
        )?;
        ensure!(
            bytes.len() as u64 == entry.size && hash(&bytes) == entry.sha256,
            "metadata hash mismatch"
        );
        files.insert(entry.path.clone(), bytes);
    }
    Ok(files)
}
fn cached_payload(
    root: &Path,
    source: &impl Source,
    path: &str,
    entry: &FileEntry,
    status: &impl Status,
    done: u64,
    total: u64,
) -> Result<(PathBuf, bool)> {
    let cache = safe::target(root, &format!(".update/downloads/{}", entry.sha256))?;
    if cache.exists() && safe::matches(&cache, entry)? {
        return Ok((cache, false));
    }
    if cache.exists() {
        safe::regular(&cache)?;
        fs::remove_file(&cache)?;
    }
    let part = cache.with_extension("part");
    if part.exists() {
        safe::regular(&part)?;
        fs::remove_file(&part)?;
    }
    let result = source.download(path, entry, &part, &mut |bytes| {
        progress(
            status,
            "downloading",
            30 + ((done + bytes) * 45 / total.max(1)) as u32,
        )
    });
    if let Err(e) = result {
        if part.exists() {
            safe::regular(&part)?;
            fs::remove_file(&part)?;
        }
        return Err(e);
    }
    ensure!(safe::matches(&part, entry)?, "download hash mismatch");
    crate::platform::replace_file(&part, &cache)?;
    Ok((cache, true))
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub candidate: String,
    pub sequence: u64,
    pub downloaded_files: usize,
    pub downloaded_bytes: u64,
    pub changed_game_files: usize,
    pub activated: bool,
}
use serde::Serialize;
pub fn check_update(root: &Path, source: &impl Source, status: &impl Status) -> Result<Outcome> {
    progress(status, "checking", 0)?;
    ensure!(
        !crate::platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
        "game is already running"
    );
    if transaction::recover(root)? {
        progress(status, "recovering", 5)?;
    }
    let (old, old_meta) = local_game(root)?;
    let old_metadata_bytes = old_meta.values().map(|b| b.len() as u64).sum::<u64>();
    let (old_engine, _) = active_engine(root)?;
    let previous = read_previous(root)?;
    // A check-only run or power loss after commit must still receive first-launch
    // health/rollback. Never overwrite its only backup with a subsequent update.
    if transaction::pending_activation(root)? {
        let candidate = verify_launch(root)?;
        return Ok(Outcome {
            candidate,
            sequence: previous.as_ref().map_or(0, |(f, _)| f.sequence),
            downloaded_files: 0,
            downloaded_bytes: 0,
            changed_game_files: 0,
            activated: true,
        });
    }
    let bytes = source.get("latest.json", 32768, &mut |_| {
        progress(status, "checking", 5)
    })?;
    let signature = source.get("latest.p7s", 32768, &mut |_| {
        progress(status, "checking", 5)
    })?;
    let feed = feed_auth(&bytes, &signature, true)?;
    feed.check_advance(&bytes, previous.as_ref().map(|(f, b)| (f, b.as_slice())))?;
    let failed = root.join(".update/failed-release.txt");
    if failed.exists() {
        ensure!(
            safe::read_bounded(&failed, 64)? != hash(&bytes).as_bytes(),
            "release quarantined after unsuccessful first launch"
        );
    }
    remember_highest(root, &bytes, &signature)?;
    let game_same = feed.game.metadata.iter().all(|e| {
        old_meta
            .get(&e.path)
            .is_some_and(|b| hash(b) == e.sha256 && b.len() as u64 == e.size)
    });
    let old_version: serde_json::Value = serde_json::from_slice(&old_meta["VERSION.json"])?;
    let mut outcome = Outcome {
        candidate: old_version["candidate"]
            .as_str()
            .context("candidate")?
            .into(),
        sequence: feed.sequence,
        downloaded_files: 0,
        downloaded_bytes: 0,
        changed_game_files: 0,
        activated: false,
    };
    let engine_meta = remote_meta(source, &feed.engine, status)?;
    let engine = Engine::parse(&engine_meta["ENGINE.json"])?;
    crate::platform::verify_cms_at(
        &engine_meta["ENGINE.json"],
        &engine_meta["ENGINE.p7s"],
        crate::SIGNING_KEY,
        engine.built_unix,
    )
    .map_err(anyhow::Error::msg)?;
    ensure!(
        engine.version >= old_engine.version && engine.version.to_string() == feed.engine.identity,
        "engine downgrade/identity mismatch"
    );
    ensure!(
        engine.version <= crate::ENGINE_VERSION || engine.exe_sha256 != old_engine.exe_sha256,
        "engine version mismatch"
    );
    if game_same
        && engine.exe_sha256 == old_engine.exe_sha256
        && previous
            .as_ref()
            .is_some_and(|(_, old_bytes)| old_bytes == &bytes)
    {
        if verify_launch(root).is_ok() {
            return Ok(outcome);
        }
    }
    let metadata = if game_same {
        old_meta
    } else {
        remote_meta(source, &feed.game, status)?
    };
    cms(
        &metadata["RELEASE-STATEMENT.json"],
        &metadata["RELEASE-STATEMENT.p7s"],
    )?;
    let new = bind_game(&metadata)?;
    let version: serde_json::Value = serde_json::from_slice(&metadata["VERSION.json"])?;
    ensure!(
        version["candidate"].as_str() == Some(&feed.game.identity),
        "candidate/feed mismatch"
    );
    outcome.candidate = feed.game.identity.clone();
    progress(status, "verifying", 20)?;
    let old_files = old.map();
    let new_files = new.map();
    let mut pending = vec![];
    let mut removed = vec![];
    // Hash on update, not on each launch. Missing/corrupted assets are repaired.
    for (index, entry) in new.files.iter().enumerate() {
        if index % 256 == 0 {
            progress(
                status,
                "verifying",
                20 + (index * 10 / new.file_count.max(1)) as u32,
            )?;
        }
        if !safe::matches(&safe::target(&root.join("game"), &entry.path)?, entry)? {
            pending.push(entry.clone());
        }
    }
    for (path, old_entry) in &old_files {
        if !new_files.contains_key(path) && root.join("game").join(path).exists() {
            ensure!(
                safe::matches(&safe::target(&root.join("game"), path)?, old_entry)?,
                "removed managed file was modified"
            );
            removed.push((*path).to_owned());
        }
    }
    let engine_needed = engine.exe_sha256 != old_engine.exe_sha256;
    let total = pending.iter().map(|f| f.size).sum::<u64>()
        + if engine_needed { engine.exe_size } else { 0 };
    let mut backups = old_metadata_bytes;
    let mut largest = 0;
    for entry in &pending {
        let path = safe::target(&root.join("game"), &entry.path)?;
        let old_size = if path.exists() {
            safe::regular(&path)?.len()
        } else {
            0
        };
        backups += old_size;
        largest = largest.max(old_size).max(entry.size);
    }
    for path in &removed {
        backups += safe::regular(&root.join("game").join(path))?.len();
    }
    let metadata_bytes = metadata.values().map(|b| b.len() as u64).sum::<u64>();
    largest = largest.max(metadata_bytes.min(MAX_META));
    ensure!(
        total <= MAX_TOTAL
            && crate::platform::free_bytes(root)?
                > total.saturating_mul(2) + metadata_bytes + backups + largest + 64 * 1024 * 1024,
        "insufficient update and rollback space"
    );
    safe::clear_scratch(root, "staging")?;
    let mut changes = vec![];
    let mut done = 0;
    for entry in &pending {
        let (cache, downloaded) = cached_payload(
            root,
            source,
            &format!("{}/{}", feed.game.directory, entry.path),
            entry,
            status,
            done,
            total,
        )?;
        let target = safe::target(
            &root.join(".update/staging"),
            &format!("game/{}", entry.path),
        )?;
        fs::create_dir_all(target.parent().unwrap())?;
        fs::copy(cache, &target)?;
        fs::OpenOptions::new()
            .write(true)
            .open(&target)?
            .sync_all()?;
        let mut new = entry.clone();
        new.path = format!("game/{}", entry.path);
        changes.push(Change {
            path: new.path.clone(),
            new: Some(new),
        });
        done += entry.size;
        if downloaded {
            outcome.downloaded_files += 1;
            outcome.downloaded_bytes += entry.size;
        }
    }
    outcome.changed_game_files = pending.len() + removed.len();
    for path in removed {
        changes.push(Change {
            path: format!("game/{path}"),
            new: None,
        });
    }
    // Candidate metadata comes last, then the immutable engine pointer and receipt.
    for name in META {
        changes.push(transaction::stage_bytes(
            root,
            &format!("game/{name}"),
            &metadata[name],
        )?);
    }
    if engine_needed {
        let entry = FileEntry {
            path: "Mir2Updater.exe".into(),
            size: engine.exe_size,
            sha256: engine.exe_sha256.clone(),
        };
        let (cache, downloaded) = cached_payload(
            root,
            source,
            &format!("{}/Mir2Updater.exe", feed.engine.directory),
            &entry,
            status,
            done,
            total,
        )?;
        let dir = safe::target(root, &format!("updater/engines/{}", engine.exe_sha256))?;
        fs::create_dir_all(&dir)?;
        for name in ["ENGINE.json", "ENGINE.p7s"] {
            let dest = dir.join(name);
            // This generation is inactive. Atomic writes also repair an incomplete
            // generation left by power loss before its pointer was published.
            safe::atomic_write(&dest, &engine_meta[name])?;
        }
        let dest = dir.join("Mir2Updater.exe");
        if !safe::matches(&dest, &entry)? {
            let temp = dir.join("Mir2Updater.download");
            safe::ancestors(&temp)?;
            if temp.exists() {
                safe::regular(&temp)?;
                fs::remove_file(&temp)?;
            }
            fs::copy(cache, &temp)?;
            fs::OpenOptions::new().write(true).open(&temp)?.sync_all()?;
            ensure!(
                safe::matches(&temp, &entry)?,
                "engine copy failed validation"
            );
            crate::platform::replace_file(&temp, &dest)?;
        }
        engine_at(root, &engine.exe_sha256)?;
        if downloaded {
            outcome.downloaded_files += 1;
            outcome.downloaded_bytes += entry.size;
        }
    }
    changes.push(transaction::stage_bytes(
        root,
        "updater/active.txt",
        engine.exe_sha256.as_bytes(),
    )?);
    changes.push(transaction::stage_bytes(
        root,
        ".update/accepted-feed.json",
        &bytes,
    )?);
    changes.push(transaction::stage_bytes(
        root,
        ".update/accepted-feed.p7s",
        &signature,
    )?);
    progress(status, "applying", 85)?;
    if let Err(error) = transaction::prepare_and_apply(root, &changes) {
        transaction::recover(root)
            .context("activation failed AND rollback failed; do not launch")?;
        return Err(error);
    }
    if let Err(error) = verify_launch(root) {
        transaction::rollback(root)?;
        return Err(error);
    }
    progress(status, "launching", 100)?;
    outcome.activated = true;
    Ok(outcome)
}
pub fn quarantine_and_rollback(root: &Path) -> Result<()> {
    let bytes = safe::read_bounded(&root.join(".update/accepted-feed.json"), 32768)?;
    let sig = safe::read_bounded(&root.join(".update/accepted-feed.p7s"), 32768)?;
    feed_auth(&bytes, &sig, false)?;
    safe::atomic_write(
        &root.join(".update/failed-release.txt"),
        hash(&bytes).as_bytes(),
    )?;
    transaction::rollback(root)
}
pub fn log(root: &Path, event: &str, detail: impl serde::Serialize) {
    let result = (|| -> Result<()> {
        let path = root.join(".update/last-run.jsonl");
        safe::ancestors(&path)?;
        fs::create_dir_all(path.parent().unwrap())?;
        if path.exists() && safe::regular(&path)?.len() > 1024 * 1024 {
            crate::platform::replace_file(&path, &path.with_extension("previous.jsonl"))?;
        }
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        serde_json::to_writer(
            &mut file,
            &serde_json::json!({"time":now(),"event":event,"detail":detail}),
        )?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(())
    })();
    if result.is_err() {} // Logging cannot permit or block an unverified launch.
}

//! Signed engine-only maintenance. Legacy game bytes and generation selection
//! are never part of this transaction. The caller holds InstallLock/GameLease.
use super::*;
use crate::generation;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const HIGHEST: &str = "state/engine-maintenance-highest.txt";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    Root,
    FiveClass,
    Unified,
}
impl Scope {
    fn feed_urls(self) -> (&'static str, &'static str) {
        match self {
            Self::Root => (
                "https://assets.mir2.obelisk.build/client-updates/latest.json",
                "https://165.154.65.136.sslip.io/client-updates/latest.json",
            ),
            Self::FiveClass => (
                "https://assets.mir2.obelisk.build/client-updates/five-class/latest.json",
                "https://165.154.65.136.sslip.io/client-updates/five-class/latest.json",
            ),
            Self::Unified => (
                "https://assets.mir2.obelisk.build/client-updates/unified/latest.json",
                "https://165.154.65.136.sslip.io/client-updates/unified/latest.json",
            ),
        }
    }
}
fn scope_for_version(digest: &str) -> Option<Scope> {
    match digest {
        "481DA3F3EE5AFAB88A9B20F2DEB5A70E359B859C53671D9099B6213E5748FBAB" => Some(Scope::Root),
        "C54DC2C9D8301E1793B8316AFE6CAB554DF19FC62468D6005F41C7897DBC802F" => {
            Some(Scope::FiveClass)
        }
        "51AFE182412B0CD7DABEB23228F3CCE354D82DB9694062FA40FAB1166397308A" => Some(Scope::Unified),
        _ => None,
    }
}
// Construct a source only after an exact authenticated baseline match. This
// boundary performs no requests and cannot create a source for an unknown hash.
fn source_for_version(digest: &str) -> Result<Option<HttpsSource>> {
    let Some(scope) = scope_for_version(digest) else {
        return Ok(None);
    };
    let (primary, fallback) = scope.feed_urls();
    Ok(Some(HttpsSource {
        base: HttpsSource::pinned_base(primary)?,
        legacy_base: HttpsSource::pinned_base(fallback)?,
        agent: ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(8))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build(),
    }))
}
/// Only exact authenticated baseline byte identities select a compiled
/// production channel. An unknown signed baseline never enables network I/O.
pub fn network_source(root: &Path) -> Result<Option<HttpsSource>> {
    let (_, meta) = local_game(root)?;
    let digest = hash(&meta["VERSION.json"]);
    let source = source_for_version(&digest)?;
    if source.is_none() {
        log(
            root,
            "engine-maintenance-unsupported-scope",
            serde_json::json!({"baselineVersionSha256":digest}),
        );
    }
    Ok(source)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Receipt {
    schema: String,
    feed_sha256: String,
    sequence: u64,
    baseline_version_sha256: String,
    scope: Option<Scope>,
    previous_engine_sha256: String,
    engine_sha256: String,
    engine_version: u32,
    engine_source_revision: String,
}
fn bind_baseline(feed: &Feed, meta: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let version: serde_json::Value = serde_json::from_slice(&meta["VERSION.json"])?;
    ensure!(
        version["candidate"].as_str() == Some(feed.game.identity.as_str())
            && feed
                .game
                .metadata
                .iter()
                .all(
                    |entry| meta.get(&entry.path).is_some_and(|bytes| bytes.len() as u64
                        == entry.size
                        && hash(bytes) == entry.sha256)
                ),
        "engine feed game scope differs from signed baseline"
    );
    Ok(())
}
fn bind_engine(feed: &Feed, engine: &Engine, metadata: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    ensure!(
        engine.version.to_string() == feed.engine.identity,
        "engine maintenance identity mismatch"
    );
    ensure!(
        engine.built_unix <= feed.created_unix.saturating_add(300),
        "engine built after authenticated maintenance feed"
    );
    ensure!(
        engine
            .source_revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "engine maintenance source must be canonical lowercase revision"
    );
    for entry in &feed.engine.metadata {
        ensure!(
            entry.size > 0
                && entry.size <= 32768
                && metadata.get(&entry.path).is_some_and(
                    |bytes| bytes.len() as u64 == entry.size && hash(bytes) == entry.sha256
                ),
            "engine maintenance metadata binding"
        );
    }
    Ok(())
}
fn receipt(root: &Path) -> Result<Option<(Receipt, Feed, String)>> {
    let path = safe::target(root, HIGHEST)?;
    if !path.exists() {
        return Ok(None);
    }
    let digest = String::from_utf8(safe::read_bounded(&path, 64)?)?;
    ensure!(valid_hash(&digest), "engine receipt pointer hash");
    let dir = safe::target(root, &format!("state/engine-maintenance/{digest}"))?;
    let bytes = safe::read_bounded(&dir.join("FEED.json"), 32768)?;
    let sig = safe::read_bounded(&dir.join("FEED.p7s"), 32768)?;
    ensure!(hash(&bytes) == digest, "engine receipt feed identity");
    let feed = feed_auth(&bytes, &sig, false)?;
    let record: Receipt =
        serde_json::from_slice(&safe::read_bounded(&dir.join("RECEIPT.json"), 32768)?)?;
    let (_, meta) = local_game(root)?;
    bind_baseline(&feed, &meta)?;
    ensure!(
        record.schema == "mir2.windows.engine-maintenance.v1"
            && record.feed_sha256 == digest
            && record.sequence == feed.sequence
            && record.baseline_version_sha256 == hash(&meta["VERSION.json"])
            && record.scope == scope_for_version(&record.baseline_version_sha256)
            && valid_hash(&record.previous_engine_sha256)
            && valid_hash(&record.engine_sha256),
        "engine maintenance receipt binding"
    );
    let (engine, exe) = engine_at(root, &record.engine_sha256)?;
    ensure!(
        engine.version == record.engine_version
            && engine.source_revision == record.engine_source_revision,
        "engine receipt descriptor identity"
    );
    let dir = exe.parent().context("stored engine parent")?;
    let mut metadata = BTreeMap::new();
    for name in ["ENGINE.json", "ENGINE.p7s"] {
        metadata.insert(name.into(), safe::read_bounded(&dir.join(name), 32768)?);
    }
    bind_engine(&feed, &engine, &metadata)?;
    Ok(Some((record, feed, digest)))
}
pub(crate) fn receipt_sequence_floor(root: &Path) -> Result<u64> {
    Ok(receipt(root)?.map_or(0, |(record, _, _)| record.sequence))
}
fn remember(root: &Path, record: &Receipt, bytes: &[u8], sig: &[u8]) -> Result<()> {
    let dir = safe::target(
        root,
        &format!("state/engine-maintenance/{}", record.feed_sha256),
    )?;
    generation::immutable_bytes(&dir.join("FEED.json"), bytes)?;
    generation::immutable_bytes(&dir.join("FEED.p7s"), sig)?;
    generation::immutable_bytes(&dir.join("RECEIPT.json"), &serde_json::to_vec(record)?)?;
    safe::atomic_write(&safe::target(root, HIGHEST)?, record.feed_sha256.as_bytes())
}
fn quarantine(root: &Path, digest: &str) -> Result<PathBuf> {
    ensure!(valid_hash(digest), "engine quarantine hash");
    safe::target(root, &format!("state/engine-quarantine/{digest}.json"))
}

pub struct Handoff {
    pub engine_sha256: String,
    pub previous_engine_sha256: String,
    pub sequence: u64,
    scratch: PathBuf,
    _scratch_lease: safe::FileLease,
    _source_lease: safe::FileLease,
}
impl Handoff {
    /// Caller releases InstallLock and GameLease immediately before spawn.
    /// Retain this Handoff until the child exits, protecting its verified image.
    pub fn spawn(&self, check_only: bool) -> std::io::Result<std::process::Child> {
        let mut command = std::process::Command::new(&self.scratch);
        command.current_dir(self.scratch.parent().unwrap().parent().unwrap());
        if check_only {
            command.arg("--check-only");
        }
        command.spawn()
    }
}
/// Source is already the one compiled baseline channel (or explicit offline
/// fixture). No game object is fetched and no generation selector is written.
pub fn check(root: &Path, source: &impl Source, status: &impl Status) -> Result<Option<Handoff>> {
    ensure!(
        !transaction::pending_activation(root)?,
        "legacy pending activation precedes engine maintenance"
    );
    let (_, meta) = local_game(root)?;
    let (old_engine, _) = active_engine(root)?;
    let mut transfers = Transfers::default();
    let (feed, bytes, sig) = fresh_discovery(root, source, status, &mut transfers)?;
    bind_baseline(&feed, &meta)?;
    let previous = receipt(root)?;
    if let Some((_, old, identity)) = &previous {
        ensure!(
            feed.sequence != old.sequence || hash(&bytes) == *identity,
            "engine maintenance reused signed sequence"
        );
    }
    let global_floor = legacy_sequence_floor(root)?
        .max(generation::signed_sequence_floor(root)?)
        .max(
            previous
                .as_ref()
                .map_or(0, |(record, _, _)| record.sequence),
        );
    let retry = previous
        .as_ref()
        .is_some_and(|(_, old, id)| old.sequence == feed.sequence && *id == hash(&bytes));
    if feed.sequence < global_floor || (feed.sequence == global_floor && !retry) {
        log(
            root,
            "engine-maintenance-stale",
            serde_json::json!({"sequence":feed.sequence,"globalFloor":global_floor,"responseBodyBytes":transfers.wire_bytes}),
        );
        return Ok(None);
    }
    for entry in &feed.engine.metadata {
        ensure!(
            entry.size > 0 && entry.size <= 32768,
            "engine metadata bound"
        );
    }
    let metadata = remote_meta(source, &feed.engine, status, &mut transfers)?;
    let engine = Engine::parse(&metadata["ENGINE.json"])?;
    crate::platform::verify_cms_at(
        &metadata["ENGINE.json"],
        &metadata["ENGINE.p7s"],
        crate::SIGNING_KEY,
        engine.built_unix,
    )
    .map_err(anyhow::Error::msg)?;
    bind_engine(&feed, &engine, &metadata)?;
    ensure!(
        engine.version >= old_engine.version
            && (engine.version > old_engine.version || engine.built_unix >= old_engine.built_unix),
        "engine downgrade/old same-version patch"
    );
    ensure!(
        !quarantine(root, &engine.exe_sha256)?.exists(),
        "engine patch quarantined after OS spawn failure"
    );
    let dir = safe::target(root, &format!("updater/engines/{}", engine.exe_sha256))?;
    for name in ["ENGINE.json", "ENGINE.p7s"] {
        generation::immutable_bytes(&dir.join(name), &metadata[name])?;
    }
    let entry = FileEntry {
        path: "Mir2Updater.exe".into(),
        size: engine.exe_size,
        sha256: engine.exe_sha256.clone(),
    };
    let wire = AtomicU64::new(transfers.wire_bytes);
    let cancel = AtomicBool::new(false);
    ensure!(!status.cancelled(), "engine maintenance cancelled");
    generation::ensure_object_observed(
        source,
        &format!("{}/Mir2Updater.exe", feed.engine.directory),
        &entry,
        &dir.join("Mir2Updater.exe"),
        &safe::target(root, ".update/engine-downloads")?,
        &cancel,
        &wire,
        status,
    )?;
    let (verified, source_exe) = engine_at(root, &engine.exe_sha256)?;
    ensure!(
        verified.source_revision == engine.source_revision && verified.version == engine.version,
        "stored engine differs"
    );
    let source_lease = safe::FileLease::verified(&source_exe, &entry)?;
    let changing = engine.exe_sha256 != old_engine.exe_sha256;
    // For an identical receipt retry after a crash, keep its original old-engine
    // binding; otherwise an already switched pointer would mutate immutable data.
    let previous_sha = if retry {
        previous.as_ref().unwrap().0.previous_engine_sha256.clone()
    } else {
        old_engine.exe_sha256.clone()
    };
    let record = Receipt {
        schema: "mir2.windows.engine-maintenance.v1".into(),
        feed_sha256: hash(&bytes),
        sequence: feed.sequence,
        baseline_version_sha256: hash(&meta["VERSION.json"]),
        scope: scope_for_version(&hash(&meta["VERSION.json"])),
        previous_engine_sha256: previous_sha.clone(),
        engine_sha256: engine.exe_sha256.clone(),
        engine_version: engine.version,
        engine_source_revision: engine.source_revision.clone(),
    };
    let scratch = safe::target(root, &format!(".update/engine-{}.exe", engine.exe_sha256))?;
    if changing {
        generation::immutable_bytes(&scratch, &safe::read_bounded(&source_exe, entry.size)?)?;
    }
    let scratch_lease = if changing {
        Some(safe::FileLease::verified(&scratch, &entry)?)
    } else {
        None
    };
    ensure!(
        !status.cancelled(),
        "engine maintenance cancelled before pointer commit"
    );
    remember(root, &record, &bytes, &sig)?;
    if !changing {
        return Ok(None);
    }
    safe::atomic_write(
        &safe::target(root, "updater/active.txt")?,
        engine.exe_sha256.as_bytes(),
    )?;
    log(
        root,
        "engine-maintenance-activated",
        serde_json::json!({"sequence":feed.sequence,"baselineVersionSha256":record.baseline_version_sha256,"scope":record.scope,"engineSha256":engine.exe_sha256,"previousEngineSha256":previous_sha,"responseBodyBytes":wire.load(Ordering::Relaxed),"gameOrSelectorWrites":false}),
    );
    Ok(Some(Handoff {
        engine_sha256: engine.exe_sha256,
        previous_engine_sha256: previous_sha,
        sequence: feed.sequence,
        scratch,
        _scratch_lease: scratch_lease.unwrap(),
        _source_lease: source_lease,
    }))
}
/// Only OS spawn failure may restore the known engine. Caller has reacquired
/// both locks; a concurrent pointer change fails instead of being overwritten.
pub fn failed_spawn(root: &Path, handoff: &Handoff) -> Result<()> {
    ensure!(
        safe::read_bounded(&safe::target(root, "updater/active.txt")?, 64)?
            == handoff.engine_sha256.as_bytes(),
        "engine pointer changed during failed handoff"
    );
    engine_at(root, &handoff.previous_engine_sha256)?;
    generation::immutable_bytes(
        &quarantine(root, &handoff.engine_sha256)?,
        &serde_json::to_vec(
            &serde_json::json!({"schema":"mir2.windows.failed-engine.v1","engineSha256":handoff.engine_sha256}),
        )?,
    )?;
    safe::atomic_write(
        &safe::target(root, "updater/active.txt")?,
        handoff.previous_engine_sha256.as_bytes(),
    )?;
    log(
        root,
        "engine-maintenance-os-spawn-rollback",
        serde_json::json!({"engineSha256":handoff.engine_sha256,"restoredEngineSha256":handoff.previous_engine_sha256,"sequence":handoff.sequence,"floorLowered":false}),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_scope_only_uses_three_authenticated_baseline_hashes() {
        assert_eq!(
            scope_for_version("481DA3F3EE5AFAB88A9B20F2DEB5A70E359B859C53671D9099B6213E5748FBAB"),
            Some(Scope::Root)
        );
        assert_eq!(
            scope_for_version("C54DC2C9D8301E1793B8316AFE6CAB554DF19FC62468D6005F41C7897DBC802F"),
            Some(Scope::FiveClass)
        );
        assert_eq!(
            scope_for_version("51AFE182412B0CD7DABEB23228F3CCE354D82DB9694062FA40FAB1166397308A"),
            Some(Scope::Unified)
        );
        assert_eq!(scope_for_version(&"A".repeat(64)), None);
    }

    #[test]
    fn unknown_version_cannot_construct_a_network_source() {
        for digest in [
            String::new(),
            "A".repeat(64),
            "481da3f3ee5afab88a9b20f2deb5a70e359b859c53671d9099b6213e5748fbab".into(),
            "C54DC2C9D8301E1793B8316AFE6CAB554DF19FC62468D6005F41C7897DBC802F/".into(),
            "51afe182412b0cd7dabeb23228f3cce354d82db9694062fa40fab1166397308a".into(),
            "51AFE182412B0CD7DABEB23228F3CCE354D82DB9694062FA40FAB1166397308B".into(),
        ] {
            // No Source exists on this branch, so no discovery/request method
            // can be called, including for a near match of an admitted hash.
            assert!(source_for_version(&digest).unwrap().is_none());
        }
    }

    #[test]
    fn authenticated_baselines_construct_only_their_matching_engine_source() {
        for (digest, expected_scope) in [
            (
                "481DA3F3EE5AFAB88A9B20F2DEB5A70E359B859C53671D9099B6213E5748FBAB",
                Scope::Root,
            ),
            (
                "C54DC2C9D8301E1793B8316AFE6CAB554DF19FC62468D6005F41C7897DBC802F",
                Scope::FiveClass,
            ),
            (
                "51AFE182412B0CD7DABEB23228F3CCE354D82DB9694062FA40FAB1166397308A",
                Scope::Unified,
            ),
        ] {
            let source = source_for_version(digest).unwrap().unwrap();
            let (primary, fallback) = expected_scope.feed_urls();
            assert_eq!(source.base, HttpsSource::pinned_base(primary).unwrap());
            assert_eq!(
                source.legacy_base,
                HttpsSource::pinned_base(fallback).unwrap()
            );
        }
    }

    #[test]
    fn compiled_engine_channels_are_disjoint_and_keep_both_pinned_hosts() {
        let mut primary_paths = std::collections::BTreeSet::new();
        let mut fallback_paths = std::collections::BTreeSet::new();
        for (scope, expected_path) in [
            (Scope::Root, "/client-updates/latest.json"),
            (Scope::FiveClass, "/client-updates/five-class/latest.json"),
            (Scope::Unified, "/client-updates/unified/latest.json"),
        ] {
            let (primary, fallback) = scope.feed_urls();
            for (raw, host) in [
                (primary, "assets.mir2.obelisk.build"),
                (fallback, "165.154.65.136.sslip.io"),
            ] {
                let parsed = url::Url::parse(raw).unwrap();
                assert_eq!(parsed.scheme(), "https");
                assert_eq!(parsed.host_str(), Some(host));
                assert_eq!(parsed.path(), expected_path);
                assert!(parsed.username().is_empty());
                assert!(parsed.password().is_none());
                assert!(parsed.query().is_none());
                assert!(parsed.fragment().is_none());
            }
            assert!(primary_paths.insert(primary));
            assert!(fallback_paths.insert(fallback));
        }
    }

    #[test]
    fn legacy_scope_receipt_encodings_remain_stable_with_unified_scope() {
        for (scope, encoded) in [
            (Scope::Root, "\"root\""),
            (Scope::FiveClass, "\"fiveClass\""),
            (Scope::Unified, "\"unified\""),
        ] {
            assert_eq!(serde_json::to_string(&scope).unwrap(), encoded);
            assert_eq!(serde_json::from_str::<Scope>(encoded).unwrap(), scope);
        }
        assert!(serde_json::from_str::<Scope>("\"UNIFIED\"").is_err());
    }
}

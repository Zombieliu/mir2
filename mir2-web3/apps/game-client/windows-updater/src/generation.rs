//! Authenticated immutable generations. Only one small selector is activated;
//! legacy game files and all private user data are never rewritten here.
use crate::{
    fs_safe as safe,
    model::{self, FileEntry},
    update::{Source, Status},
};
use anyhow::{ensure, Context, Result};
use mir2_resource_pack::{Catalog, PackStore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Mutex,
    },
    time::{Duration, Instant},
};

pub const MAX_GENERATION_METADATA: u64 = 4 * 1024 * 1024;
pub const MAX_CATALOG: u64 = mir2_resource_pack::MAX_CATALOG_BYTES as u64;
pub const MAX_PACK: u64 = mir2_resource_pack::MAX_PACK_BYTES;
pub const MAX_PACKS: usize = mir2_resource_pack::MAX_PACKS;
pub const MAX_GENERATION_TOTAL: u64 = mir2_resource_pack::MAX_TOTAL_BYTES;
const SELECTOR: &str = "state/active-generation.json";
const HIGHEST: &str = "state/pack-highest.txt";

/// A fresh, authenticated pack feed from an older release line is not a
/// migration for an installation with a newer signed v4 seed. Only this exact
/// pre-download rejection may retain an untouched, verified legacy install.
/// Signature, schema, expiry, equal-sequence and payload failures stay errors.
#[derive(Debug)]
struct BelowLegacyFloor {
    offered: u64,
    floor: u64,
}
impl std::fmt::Display for BelowLegacyFloor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "authenticated packed sequence {} is below legacy floor {}",
            self.offered, self.floor
        )
    }
}
impl std::error::Error for BelowLegacyFloor {}

fn require_legacy_advance(offered: u64, floor: u64) -> Result<()> {
    if offered < floor {
        return Err(BelowLegacyFloor { offered, floor }.into());
    }
    ensure!(
        offered > floor,
        "packed sequence must exceed every legacy bridge"
    );
    Ok(())
}

/// No discovery retry, state mutation, or alternate release is allowed here.
/// A migrated/pending install must keep its existing signed selector policy.
pub fn retain_verified_legacy_after_rejected_feed(
    root: &Path,
    error: &anyhow::Error,
) -> Result<bool> {
    let Some(rejected) = error.downcast_ref::<BelowLegacyFloor>() else {
        return Ok(false);
    };
    if active_record(root)?.is_some() || highest(root)?.is_some() {
        return Ok(false);
    }
    ensure!(
        !crate::transaction::pending_activation(root)?,
        "legacy activation must recover before retaining an installation"
    );
    let floor = crate::update::legacy_sequence_floor(root)?;
    ensure!(
        floor == rejected.floor && rejected.offered < floor,
        "legacy floor changed during rejected discovery"
    );
    crate::update::verify_launch(root)?;
    Ok(true)
}
struct Phase<'a> {
    root: &'a Path,
    name: &'static str,
    started: Instant,
    wire: &'a AtomicU64,
    before: u64,
    complete: bool,
}
impl<'a> Phase<'a> {
    fn new(root: &'a Path, name: &'static str, wire: &'a AtomicU64) -> Self {
        Self {
            root,
            name,
            started: Instant::now(),
            wire,
            before: wire.load(Ordering::Relaxed),
            complete: false,
        }
    }
    fn finish(&mut self) {
        self.complete = true;
    }
}
impl Drop for Phase<'_> {
    fn drop(&mut self) {
        crate::update::log(
            self.root,
            "packed-phase-end",
            serde_json::json!({"phase":self.name,"elapsedMs":self.started.elapsed().as_millis(),"responseBodyBytes":self.wire.load(Ordering::Relaxed).saturating_sub(self.before),"outcome":if self.complete{"complete"}else{"unfinished"}}),
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GenerationManifest {
    pub schema: String,
    pub sequence: u64,
    pub min_engine: u32,
    pub game_source_revision: String,
    pub engine_source_revision: String,
    pub protocol: String,
    pub content_profile: String,
    pub exe: FileEntry,
    pub signed_config: FileEntry,
    pub catalog: FileEntry,
    pub packs: Vec<FileEntry>,
}
impl GenerationManifest {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() as u64 <= MAX_GENERATION_METADATA,
            "generation metadata bound"
        );
        let value: Self = serde_json::from_slice(bytes)?;
        ensure!(
            value.schema == "mir2.windows.generation.v1"
                && value.sequence > 0
                && value.min_engine >= 2
                && value.min_engine <= crate::ENGINE_VERSION
                && value.protocol == "crystal-mir2-v1",
            "incompatible generation"
        );
        for revision in [&value.game_source_revision, &value.engine_source_revision] {
            ensure!(
                revision.len() == 40
                    && revision
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "invalid generation source"
            );
        }
        ensure!(
            !value.content_profile.is_empty()
                && value.content_profile.len() <= 64
                && value
                    .content_profile
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "invalid generation content profile"
        );
        for (file, name, limit) in [
            (&value.exe, "mir2-platform-windows.exe", model::MAX_FILE),
            (&value.signed_config, "mir2-client.toml", 32768),
            (&value.catalog, "catalog.json", MAX_CATALOG),
        ] {
            file.validate()?;
            ensure!(
                file.path == name && file.size > 0 && file.size <= limit,
                "invalid generation control object"
            );
        }
        ensure!(
            !value.packs.is_empty() && value.packs.len() <= MAX_PACKS,
            "pack object count bound"
        );
        let mut seen = BTreeSet::new();
        let mut total = 0u64;
        for pack in &value.packs {
            pack.validate()?;
            ensure!(
                pack.path == format!("objects/packs/{}.m2p", pack.sha256.to_ascii_lowercase())
                    && pack.size > 0
                    && pack.size <= MAX_PACK
                    && seen.insert(pack.path.clone()),
                "invalid/duplicate generation pack"
            );
            total = total
                .checked_add(pack.size)
                .context("generation total overflow")?;
        }
        ensure!(total <= MAX_GENERATION_TOTAL, "generation total bound");
        Ok(value)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PackFeed {
    pub schema: String,
    pub channel: String,
    pub platform: String,
    pub sequence: u64,
    pub created_unix: i64,
    pub expires_unix: i64,
    pub min_engine: u32,
    pub manifest: FileEntry,
    pub signature: FileEntry,
}
impl PackFeed {
    fn parse(bytes: &[u8], now: i64, fresh: bool) -> Result<Self> {
        ensure!(bytes.len() <= 32768, "pack feed bound");
        let feed: Self = serde_json::from_slice(bytes)?;
        ensure!(
            feed.schema == "mir2.windows.pack-feed.v1"
                && feed.channel == "invited"
                && feed.platform == "windows-x64"
                && feed.sequence > 0
                && feed.min_engine >= 2
                && feed.min_engine <= crate::ENGINE_VERSION,
            "incompatible pack feed"
        );
        ensure!(
            feed.created_unix > 0
                && feed.expires_unix > feed.created_unix
                && feed
                    .expires_unix
                    .checked_sub(feed.created_unix)
                    .is_some_and(|n| n <= 31 * 86400),
            "pack feed validity"
        );
        ensure!(
            !fresh || (feed.created_unix <= now + 300 && feed.expires_unix >= now),
            "expired/future pack feed"
        );
        feed.manifest.validate()?;
        feed.signature.validate()?;
        ensure!(
            feed.manifest.size > 0
                && feed.manifest.size <= MAX_GENERATION_METADATA
                && feed.signature.size > 0
                && feed.signature.size <= 32768,
            "pack feed metadata bounds"
        );
        let directory = directory(&feed.manifest.sha256)?;
        ensure!(
            feed.manifest.path == format!("{directory}/GENERATION.json")
                && feed.signature.path == format!("{directory}/GENERATION.p7s"),
            "pack feed path binding"
        );
        Ok(feed)
    }
}
fn directory(digest: &str) -> Result<String> {
    ensure!(model::valid_hash(digest), "invalid generation digest");
    Ok(format!("generations/{}", digest.to_ascii_lowercase()))
}
fn authenticate(bytes: &[u8], signature: &[u8], created: i64) -> Result<()> {
    ensure!(
        !signature.is_empty() && signature.len() <= 32768,
        "generation signature bound"
    );
    crate::platform::verify_cms_at(bytes, signature, crate::SIGNING_KEY, created)
        .map_err(anyhow::Error::msg)
}
fn authenticated_feed(bytes: &[u8], sig: &[u8], fresh: bool) -> Result<PackFeed> {
    let feed = PackFeed::parse(bytes, crate::update::now(), fresh)?;
    authenticate(bytes, sig, feed.created_unix)?;
    Ok(feed)
}
pub trait PackSource: Source + Sync {
    fn packed_pair(
        &self,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>>;
}
impl PackSource for crate::update::HttpsSource {
    fn packed_pair(
        &self,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        self.packed_discovery_pair(progress)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Selection {
    #[serde(rename = "generation", rename_all = "camelCase")]
    Generation {
        manifest_sha256: String,
        sequence: u64,
    },
    #[serde(rename = "legacyV4", rename_all = "camelCase")]
    LegacyV4 {
        candidate: String,
        version_sha256: String,
        exe_sha256: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Health {
    Pending,
    Healthy,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ActiveRecord {
    pub schema: String,
    pub active: Selection,
    pub previous: Option<Selection>,
    pub health: Health,
    pub attempt: String,
    #[serde(default)]
    pub launch_started: bool,
}
fn validate_selection(selection: &Selection) -> Result<()> {
    match selection {
        Selection::Generation {
            manifest_sha256,
            sequence,
        } => ensure!(
            model::valid_hash(manifest_sha256) && *sequence > 0,
            "invalid generation selector"
        ),
        Selection::LegacyV4 {
            candidate,
            version_sha256,
            exe_sha256,
        } => ensure!(
            !candidate.is_empty()
                && candidate.len() < 100
                && model::valid_hash(version_sha256)
                && model::valid_hash(exe_sha256),
            "invalid legacy selector"
        ),
    }
    Ok(())
}
pub fn active_record(root: &Path) -> Result<Option<ActiveRecord>> {
    let path = safe::target(root, SELECTOR)?;
    if !path.exists() {
        return Ok(None);
    }
    let record: ActiveRecord = serde_json::from_slice(&safe::read_bounded(&path, 32768)?)?;
    ensure!(
        record.schema == "mir2.windows.active-generation.v1" && model::valid_hash(&record.attempt),
        "invalid active generation record"
    );
    validate_selection(&record.active)?;
    if let Some(previous) = &record.previous {
        validate_selection(previous)?;
        ensure!(
            previous != &record.active,
            "generation rollback selector aliases active"
        );
    }
    ensure!(
        record.health != Health::Pending
            || (record.previous.is_some() && matches!(record.active, Selection::Generation { .. })),
        "pending generation lost rollback selector"
    );
    ensure!(
        record.health != Health::Healthy || !record.launch_started,
        "healthy generation has unfinished launch"
    );
    Ok(Some(record))
}
fn save_record(root: &Path, record: &ActiveRecord) -> Result<()> {
    safe::atomic_write(&safe::target(root, SELECTOR)?, &serde_json::to_vec(record)?)
}
fn nonce() -> Result<String> {
    let mut bytes = [0u8; 32];
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Cryptography::{
            BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        };
        ensure!(
            unsafe {
                BCryptGenRandom(
                    std::ptr::null_mut(),
                    bytes.as_mut_ptr(),
                    bytes.len() as u32,
                    BCRYPT_USE_SYSTEM_PREFERRED_RNG,
                )
            } >= 0,
            "generation nonce failed"
        );
    }
    #[cfg(not(windows))]
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02X}")).collect())
}
pub(crate) fn immutable_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        ensure!(
            safe::read_bounded(path, bytes.len() as u64)? == bytes,
            "existing immutable generation differs"
        );
    } else {
        let temporary = path.with_file_name(format!(
            "{}.part",
            path.file_name()
                .context("immutable file name")?
                .to_string_lossy()
        ));
        safe::ancestors(&temporary)?;
        if temporary.exists() {
            safe::regular(&temporary)?;
            fs::remove_file(&temporary)?;
        }
        safe::write_new(&temporary, bytes)?;
        safe::ancestors(path)?;
        crate::platform::promote_new(&temporary, path)?;
    }
    Ok(())
}
fn read_generation(
    root: &Path,
    digest: &str,
) -> Result<(GenerationManifest, PathBuf, Vec<u8>, String)> {
    let generation = safe::target(root, &directory(digest)?)?;
    let bytes = safe::read_bounded(&generation.join("GENERATION.json"), MAX_GENERATION_METADATA)?;
    ensure!(
        model::hash(&bytes) == digest,
        "generation manifest digest differs"
    );
    let feed_bytes = safe::read_bounded(&generation.join("PACK-FEED.json"), 32768)?;
    let feed_sig = safe::read_bounded(&generation.join("PACK-FEED.p7s"), 32768)?;
    let feed = authenticated_feed(&feed_bytes, &feed_sig, false)?;
    ensure!(
        feed.manifest.sha256 == digest && bytes.len() as u64 == feed.manifest.size,
        "generation feed binding"
    );
    let sig = safe::read_bounded(&generation.join("GENERATION.p7s"), 32768)?;
    ensure!(
        model::hash(&sig) == feed.signature.sha256 && sig.len() as u64 == feed.signature.size,
        "generation CMS bytes differ"
    );
    authenticate(&bytes, &sig, feed.created_unix)?;
    let manifest = GenerationManifest::parse(&bytes)?;
    ensure!(
        manifest.sequence == feed.sequence && manifest.min_engine == feed.min_engine,
        "generation/feed sequence or engine mismatch"
    );
    let (engine, _) = crate::update::active_engine(root)?;
    // A newer authenticated compatible engine may launch the retained healthy
    // generation after rollback. Fresh activation below binds its exact source.
    ensure!(
        engine.version >= manifest.min_engine,
        "generation engine capability mismatch"
    );
    for entry in [&manifest.exe, &manifest.signed_config, &manifest.catalog] {
        ensure!(
            safe::matches(&safe::target(&generation, &entry.path)?, entry)?,
            "generation control object hash mismatch"
        );
    }
    let catalog = safe::read_bounded(&generation.join(&manifest.catalog.path), MAX_CATALOG)?;
    let parsed = Catalog::parse(&catalog, &manifest.catalog.sha256)?;
    let expected: BTreeSet<_> = manifest
        .packs
        .iter()
        .map(|p| (p.sha256.clone(), p.size))
        .collect();
    let found: BTreeSet<_> = parsed
        .objects()
        .iter()
        .map(|p| (p.sha256.clone(), p.size))
        .collect();
    ensure!(expected == found, "generation/catalog pack closure differs");
    Ok((manifest, generation, catalog, model::hash(&feed_bytes)))
}
pub struct VerifiedGeneration {
    pub manifest: GenerationManifest,
    pub root: PathBuf,
    pub catalog_bytes: Vec<u8>,
    pub manifest_sha256: String,
    pub attempt: String,
}
pub fn verify_generation_for_launch(
    install_root: &Path,
    manifest_sha: &str,
) -> Result<VerifiedGeneration> {
    let record = active_record(install_root)?.context("no active generation")?;
    ensure!(
        matches!(&record.active,Selection::Generation{manifest_sha256,..} if manifest_sha256==manifest_sha),
        "launch does not match active generation"
    );
    let (manifest, root, catalog_bytes, _) = read_generation(install_root, manifest_sha)?;
    ensure!(
        matches!(record.active,Selection::Generation{sequence,..} if sequence==manifest.sequence),
        "active generation sequence differs"
    );
    Ok(VerifiedGeneration {
        manifest,
        root,
        catalog_bytes,
        manifest_sha256: manifest_sha.into(),
        attempt: record.attempt,
    })
}
/// Validate the selected generation or the explicitly retained legacy rollback
/// baseline. This never chooses an older network release or changes selection.
pub fn verify_current_selection(root: &Path) -> Result<ActiveRecord> {
    let record = active_record(root)?.context("no selected generation")?;
    verify_selection(root, &record.active)?;
    Ok(record)
}
/// Continue only the already selected authenticated healthy release. In
/// particular, LegacyV4 is permitted only after an explicit selector rollback;
/// this function never discovers a v4 feed or infers a fallback from an error.
pub fn verified_healthy_selection(root: &Path) -> Result<Option<ActiveRecord>> {
    let Some(record) = active_record(root)? else {
        return Ok(None);
    };
    if record.health != Health::Healthy {
        return Ok(None);
    }
    let verified = verify_current_selection(root)?;
    ensure!(
        verified.health == Health::Healthy,
        "healthy selection changed during verification"
    );
    Ok(Some(verified))
}
fn check_installed_advance(
    feed: &PackFeed,
    bytes: &[u8],
    sequence: u64,
    manifest_sha: &str,
    feed_identity: &str,
) -> Result<()> {
    ensure!(
        feed.sequence > sequence
            || (feed.sequence == sequence
                && feed.manifest.sha256 == manifest_sha
                && model::hash(bytes) == feed_identity),
        "pack feed below installed generation or active sequence identity differs"
    );
    Ok(())
}
fn verify_selection(root: &Path, selection: &Selection) -> Result<()> {
    match selection {
        Selection::Generation {
            manifest_sha256,
            sequence,
        } => {
            let (manifest, _, catalog, _) = read_generation(root, manifest_sha256)?;
            ensure!(
                manifest.sequence == *sequence,
                "previous generation sequence differs"
            );
            let _lease = PackStore::open(
                safe::target(root, "objects/packs")?,
                &catalog,
                &manifest.catalog.sha256,
            )?;
        }
        Selection::LegacyV4 {
            candidate,
            version_sha256,
            exe_sha256,
        } => {
            ensure!(
                crate::update::verify_launch(root)? == *candidate
                    && model::hash(&safe::read_bounded(
                        &safe::target(root, "game/VERSION.json")?,
                        model::MAX_META
                    )?) == *version_sha256
                    && safe::digest_file(&safe::target(root, "game/mir2-platform-windows.exe")?)?.1
                        == *exe_sha256,
                "legacy rollback baseline changed"
            );
        }
    }
    Ok(())
}
fn legacy_selection(root: &Path) -> Result<Selection> {
    Ok(Selection::LegacyV4 {
        candidate: crate::update::verify_launch(root)?,
        version_sha256: model::hash(&safe::read_bounded(
            &safe::target(root, "game/VERSION.json")?,
            model::MAX_META,
        )?),
        exe_sha256: safe::digest_file(&safe::target(root, "game/mir2-platform-windows.exe")?)?.1,
    })
}
pub fn accept(root: &Path, attempt: &str) -> Result<()> {
    let mut record = active_record(root)?.context("no pending generation")?;
    ensure!(
        record.attempt == attempt && record.health == Health::Pending && record.launch_started,
        "generation health attempt mismatch"
    );
    verify_selection(root, &record.active)?;
    record.health = Health::Healthy;
    record.launch_started = false;
    save_record(root, &record)
}
/// Persist the first-launch obligation before releasing the activation lease.
pub fn begin_launch(root: &Path, attempt: &str) -> Result<()> {
    let mut record = active_record(root)?.context("no selected generation")?;
    ensure!(record.attempt == attempt, "launch attempt changed");
    if record.health == Health::Pending {
        record.launch_started = true;
        save_record(root, &record)?;
    }
    Ok(())
}
/// Called only while holding GameLease, so an orphaned live child prevents this
/// recovery. An interrupted first launch never becomes a healthy release.
pub fn recover(root: &Path) -> Result<bool> {
    let Some(record) = active_record(root)? else {
        return Ok(false);
    };
    if record.health == Health::Pending {
        let quarantined = match &record.active {
            Selection::Generation {
                manifest_sha256, ..
            } => safe::target(root, &format!("state/quarantine/{manifest_sha256}.json"))?.exists(),
            _ => false,
        };
        let invalid = if !record.launch_started && !quarantined {
            match verify_selection(root, &record.active) {
                Ok(()) => false,
                Err(error) => {
                    crate::update::log(
                        root,
                        "packed-pending-integrity-failure",
                        format!("{error:#}"),
                    );
                    true
                }
            }
        } else {
            false
        };
        if record.launch_started || quarantined || invalid {
            rollback(root, &record.attempt)?;
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn rollback(root: &Path, attempt: &str) -> Result<()> {
    let record = active_record(root)?.context("no pending generation")?;
    ensure!(
        record.attempt == attempt && record.health == Health::Pending,
        "generation rollback attempt mismatch"
    );
    let previous = record.previous.context("no previous generation")?;
    verify_selection(root, &previous)?;
    let mut legacy_guard =
        crate::platform::GameGuard::new(&root.join("game/mir2-platform-windows.exe"))?;
    ensure!(
        !legacy_guard.is_running()? && legacy_guard.excluded(),
        "cannot exclude legacy game during generation rollback"
    );
    if let Selection::Generation {
        manifest_sha256, ..
    } = &record.active
    {
        immutable_bytes(
            &safe::target(root, &format!("state/quarantine/{manifest_sha256}.json"))?,
            &serde_json::to_vec(&record.active)?,
        )?;
    }
    legacy_guard.verify_excluded()?;
    save_record(
        root,
        &ActiveRecord {
            schema: record.schema,
            active: previous,
            previous: None,
            health: Health::Healthy,
            attempt: nonce()?,
            launch_started: false,
        },
    )
}
fn highest(root: &Path) -> Result<Option<(PackFeed, String)>> {
    let path = safe::target(root, HIGHEST)?;
    if !path.exists() {
        return Ok(None);
    }
    let digest = String::from_utf8(safe::read_bounded(&path, 64)?)?;
    ensure!(model::valid_hash(&digest), "invalid pack highest pointer");
    let base = safe::target(root, &format!("state/pack-receipts/{digest}"))?;
    let bytes = safe::read_bounded(&base.join("FEED.json"), 32768)?;
    ensure!(
        model::hash(&bytes) == digest,
        "highest pack feed hash mismatch"
    );
    let sig = safe::read_bounded(&base.join("FEED.p7s"), 32768)?;
    Ok(Some((authenticated_feed(&bytes, &sig, false)?, digest)))
}
pub(crate) fn signed_sequence_floor(root: &Path) -> Result<u64> {
    let mut floor = highest(root)?.map_or(0, |(feed, _)| feed.sequence);
    if let Some(ActiveRecord {
        active: Selection::Generation {
            manifest_sha256,
            sequence,
        },
        ..
    }) = active_record(root)?
    {
        let (manifest, _, _, _) = read_generation(root, &manifest_sha256)?;
        ensure!(
            manifest.sequence == sequence,
            "selected signed sequence differs"
        );
        floor = floor.max(sequence);
    }
    Ok(floor)
}
fn remember_highest(root: &Path, bytes: &[u8], sig: &[u8]) -> Result<()> {
    let digest = model::hash(bytes);
    let base = safe::target(root, &format!("state/pack-receipts/{digest}"))?;
    immutable_bytes(&base.join("FEED.json"), bytes)?;
    immutable_bytes(&base.join("FEED.p7s"), sig)?;
    safe::atomic_write(&safe::target(root, HIGHEST)?, digest.as_bytes())
}
fn fetch_checked(
    source: &impl Source,
    entry: &FileEntry,
    status: &impl Status,
    wire: &AtomicU64,
) -> Result<Vec<u8>> {
    let mut counted = 0;
    let bytes = source.get(&entry.path, entry.size, &mut |count| {
        ensure!(
            count <= entry.size.saturating_add(1),
            "packed response progress bound"
        );
        wire.fetch_add(count.saturating_sub(counted), Ordering::Relaxed);
        counted = counted.max(count);
        ensure!(!status.cancelled(), "packed update cancelled");
        Ok(())
    })?;
    wire.fetch_add(
        (bytes.len() as u64).saturating_sub(counted),
        Ordering::Relaxed,
    );
    ensure!(
        bytes.len() as u64 == entry.size && model::hash(&bytes) == entry.sha256,
        "packed metadata hash differs"
    );
    Ok(bytes)
}
pub(crate) fn ensure_object(
    source: &impl Source,
    remote: &str,
    entry: &FileEntry,
    target: &Path,
    parts: &Path,
    cancel: &AtomicBool,
    wire: &AtomicU64,
) -> Result<bool> {
    ensure_object_inner(source, remote, entry, target, parts, cancel, wire, None)
}
pub(crate) fn ensure_object_observed(
    source: &impl Source,
    remote: &str,
    entry: &FileEntry,
    target: &Path,
    parts: &Path,
    cancel: &AtomicBool,
    wire: &AtomicU64,
    status: &dyn Status,
) -> Result<bool> {
    ensure_object_inner(
        source,
        remote,
        entry,
        target,
        parts,
        cancel,
        wire,
        Some(status),
    )
}
fn ensure_object_inner(
    source: &impl Source,
    remote: &str,
    entry: &FileEntry,
    target: &Path,
    parts: &Path,
    cancel: &AtomicBool,
    wire: &AtomicU64,
    status: Option<&dyn Status>,
) -> Result<bool> {
    safe::ancestors(target)?;
    if target.exists() {
        ensure!(
            safe::matches(target, entry)?,
            "existing immutable object differs"
        );
        return Ok(false);
    }
    let part = safe::target(parts, &format!("{}.part", entry.sha256))?;
    if part.exists() && safe::regular(&part)?.len() >= entry.size && !safe::matches(&part, entry)? {
        fs::remove_file(&part)?;
    }
    if part.exists() && !source.permits_partial_resume() && !safe::matches(&part, entry)? {
        safe::regular(&part)?;
        fs::remove_file(&part)?;
    }
    if !part.exists() || !safe::matches(&part, entry)? {
        let mut counted = 0;
        source.download_resuming(remote, entry, &part, &mut |p| {
            ensure!(
                !cancel.load(Ordering::Relaxed) && status.is_none_or(|status| !status.cancelled()),
                "packed update cancelled"
            );
            wire.fetch_add(p.received.saturating_sub(counted), Ordering::Relaxed);
            counted = p.received;
            Ok(())
        })?;
    }
    // Hash and flush the same actual no-reparse, single-link writable handle.
    // The Windows share-zero lease excludes tampering until it is dropped for
    // the create-only, same-volume WRITE_THROUGH rename below.
    let mut file = safe::open_download(&part)?;
    ensure!(
        file.metadata()?.len() == entry.size,
        "packed object length differs"
    );
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .context("pack hash length overflow")?;
        ensure!(
            count <= entry.size,
            "packed object grew during verification"
        );
        digest.update(&buffer[..read]);
    }
    ensure!(
        count == entry.size
            && file.metadata()?.len() == entry.size
            && format!("{:X}", digest.finalize()) == entry.sha256,
        "downloaded packed object hash differs"
    );
    file.sync_all()?;
    drop(file);
    fs::create_dir_all(target.parent().unwrap())?;
    safe::ancestors(target)?;
    crate::platform::promote_new(&part, target)?;
    Ok(true)
}
fn download_packs(
    source: &impl PackSource,
    manifest: &GenerationManifest,
    root: &Path,
    status: &impl Status,
    wire: &AtomicU64,
) -> Result<usize> {
    let queue = Mutex::new(
        manifest
            .packs
            .iter()
            .collect::<std::collections::VecDeque<_>>(),
    );
    let cancelled = AtomicBool::new(false);
    let downloaded = AtomicU64::new(0);
    let parts = safe::target(root, ".update/pack-downloads")?;
    std::thread::scope(|scope| -> Result<usize> {
        let (sender, receiver) = mpsc::channel();
        let mut handles = vec![];
        for _ in 0..manifest.packs.len().min(4) {
            let sender = sender.clone();
            let queue = &queue;
            let cancelled = &cancelled;
            let parts = &parts;
            let downloaded = &downloaded;
            handles.push(scope.spawn(move || {
                while !cancelled.load(Ordering::Relaxed) {
                    let Some(entry) = queue.lock().unwrap_or_else(|p| p.into_inner()).pop_front()
                    else {
                        break;
                    };
                    let result = (|| {
                        let target = safe::target(root, &entry.path)?;
                        ensure_object(source, &entry.path, entry, &target, parts, cancelled, wire)
                    })();
                    if matches!(result, Ok(true)) {
                        downloaded.fetch_add(1, Ordering::Relaxed);
                    }
                    if result.is_err() {
                        cancelled.store(true, Ordering::Relaxed);
                    }
                    if sender.send(result).is_err() {
                        break;
                    }
                }
            }));
        }
        drop(sender);
        let mut first_error = None;
        let mut completed = 0;
        loop {
            if status.cancelled() {
                cancelled.store(true, Ordering::Relaxed);
                if first_error.is_none() {
                    first_error = Some(anyhow::anyhow!("packed update cancelled"));
                }
            }
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(_)) => {
                    completed += 1;
                    status.set(
                        "downloading",
                        10 + (completed as u32 * 65 / manifest.packs.len() as u32),
                    );
                }
                Ok(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        for handle in handles {
            if handle.join().is_err() {
                first_error = Some(anyhow::anyhow!("packed download worker panicked"));
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        ensure!(
            completed == manifest.packs.len(),
            "packed object closure incomplete"
        );
        Ok(downloaded.load(Ordering::Relaxed) as usize)
    })
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub sequence: u64,
    pub manifest_sha256: String,
    pub downloaded_packs: usize,
    pub wire_bytes: u64,
    pub activated: bool,
    pub elapsed_ms: u128,
}
/// Caller holds the installation mutex and an exclusive game lease. Only an
/// authenticated, wholly durable generation may cross the selector boundary.
pub fn check_update(
    root: &Path,
    source: &impl PackSource,
    status: &impl Status,
) -> Result<Option<Outcome>> {
    let started = Instant::now();
    let record = active_record(root)?;
    if let Some(record) = &record {
        if record.health == Health::Pending {
            verify_selection(root, &record.active)?;
            let Selection::Generation {
                manifest_sha256,
                sequence,
            } = &record.active
            else {
                anyhow::bail!("pending legacy selector")
            };
            return Ok(Some(Outcome {
                sequence: *sequence,
                manifest_sha256: manifest_sha256.clone(),
                downloaded_packs: 0,
                wire_bytes: 0,
                activated: false,
                elapsed_ms: started.elapsed().as_millis(),
            }));
        }
    }
    let wire = AtomicU64::new(0);
    let mut metadata_phase = Phase::new(root, "discovery-and-metadata", &wire);
    let mut pair_bytes = 0u64;
    let pair = source.packed_pair(&mut |count| {
        pair_bytes = pair_bytes
            .checked_add(count)
            .context("packed discovery transfer overflow")?;
        ensure!(pair_bytes <= 4 * 32769, "packed discovery transfer bound");
        wire.fetch_add(count, Ordering::Relaxed);
        ensure!(!status.cancelled(), "packed update cancelled");
        Ok(())
    })?;
    let Some((feed_bytes, feed_sig)) = pair else {
        ensure!(
            record.is_none() && highest(root)?.is_none(),
            "packed channel disappeared after migration"
        );
        return Ok(None);
    };
    let feed = authenticated_feed(&feed_bytes, &feed_sig, true)?;
    let floor = crate::update::legacy_sequence_floor(root)?;
    require_legacy_advance(feed.sequence, floor)?;
    if let Some((old, digest)) = highest(root)? {
        ensure!(
            feed.sequence > old.sequence
                || (feed.sequence == old.sequence && model::hash(&feed_bytes) == digest),
            "pack feed downgrade or sequence reuse"
        );
    }
    // The installed signed baseline remains a floor independently of the
    // mutable highest pointer. Losing that pointer must not make a replayed
    // older feed, or different bytes at the active sequence, acceptable.
    if let Some(ActiveRecord {
        active: Selection::Generation {
            manifest_sha256,
            sequence,
        },
        ..
    }) = &record
    {
        let (installed, _, _, feed_identity) = read_generation(root, manifest_sha256)?;
        ensure!(
            installed.sequence == *sequence,
            "installed generation sequence differs"
        );
        check_installed_advance(
            &feed,
            &feed_bytes,
            installed.sequence,
            manifest_sha256,
            &feed_identity,
        )?;
    }
    let already_selected = record.as_ref().is_some_and(|old| matches!(&old.active,Selection::Generation{manifest_sha256,sequence} if manifest_sha256==&feed.manifest.sha256 && *sequence==feed.sequence));
    ensure!(
        already_selected
            || feed.sequence > crate::update::engine_maintenance::receipt_sequence_floor(root)?,
        "new packed activation must exceed engine maintenance floor"
    );
    let digest = feed.manifest.sha256.clone();
    ensure!(
        !safe::target(root, &format!("state/quarantine/{digest}.json"))?.exists(),
        "packed generation quarantined after failed launch"
    );
    let manifest_bytes = fetch_checked(source, &feed.manifest, status, &wire)?;
    let signature = fetch_checked(source, &feed.signature, status, &wire)?;
    authenticate(&manifest_bytes, &signature, feed.created_unix)?;
    let manifest = GenerationManifest::parse(&manifest_bytes)?;
    ensure!(
        manifest.sequence == feed.sequence && manifest.min_engine == feed.min_engine,
        "pack feed/manifest binding"
    );
    let (engine, _) = crate::update::active_engine(root)?;
    ensure!(
        engine.version >= manifest.min_engine
            && (already_selected || engine.source_revision == manifest.engine_source_revision),
        "pack target requires a different engine"
    );
    remember_highest(root, &feed_bytes, &feed_sig)?;
    let generation = safe::target(root, &directory(&digest)?)?;
    immutable_bytes(&generation.join("GENERATION.json"), &manifest_bytes)?;
    immutable_bytes(&generation.join("GENERATION.p7s"), &signature)?;
    immutable_bytes(&generation.join("PACK-FEED.json"), &feed_bytes)?;
    immutable_bytes(&generation.join("PACK-FEED.p7s"), &feed_sig)?;
    metadata_phase.finish();
    drop(metadata_phase);
    let parts = safe::target(root, ".update/pack-downloads")?;
    let cancelled = AtomicBool::new(false);
    let mut controls_phase = Phase::new(root, "control-objects", &wire);
    for entry in [&manifest.exe, &manifest.signed_config, &manifest.catalog] {
        ensure!(!status.cancelled(), "packed update cancelled");
        ensure_object_observed(
            source,
            &format!("{}/{}", directory(&digest)?, entry.path),
            entry,
            &safe::target(&generation, &entry.path)?,
            &parts,
            &cancelled,
            &wire,
            status,
        )?;
    }
    controls_phase.finish();
    drop(controls_phase);
    let mut packs_phase = Phase::new(root, "pack-download-and-hash", &wire);
    let packs = download_packs(source, &manifest, root, status, &wire)?;
    packs_phase.finish();
    drop(packs_phase);
    let mut verification_phase = Phase::new(root, "catalog-and-pack-validation", &wire);
    status.set_final("verifying", 80);
    let (_, _, catalog, _) = read_generation(root, &digest)?;
    let _pack_lease = PackStore::open(
        safe::target(root, "objects/packs")?,
        &catalog,
        &manifest.catalog.sha256,
    )?;
    verification_phase.finish();
    drop(verification_phase);
    ensure!(
        !status.cancelled(),
        "packed update cancelled before activation"
    );
    let active = Selection::Generation {
        manifest_sha256: digest.clone(),
        sequence: feed.sequence,
    };
    let unchanged = record.as_ref().is_some_and(|old| old.active == active);
    let mut activation_phase = Phase::new(root, "atomic-selector", &wire);
    status.set_final("applying", 95);
    if !unchanged {
        let previous = match record {
            Some(old) => {
                verify_selection(root, &old.active)?;
                old.active
            }
            None => legacy_selection(root)?,
        };
        let mut legacy_guard =
            crate::platform::GameGuard::new(&root.join("game/mir2-platform-windows.exe"))?;
        ensure!(
            !legacy_guard.is_running()? && legacy_guard.excluded(),
            "cannot exclude legacy game during generation activation"
        );
        legacy_guard.verify_excluded()?;
        ensure!(
            !status.cancelled(),
            "packed update cancelled before selector commit"
        );
        save_record(
            root,
            &ActiveRecord {
                schema: "mir2.windows.active-generation.v1".into(),
                active,
                previous: Some(previous),
                health: Health::Pending,
                attempt: nonce()?,
                launch_started: false,
            },
        )?;
    }
    activation_phase.finish();
    drop(activation_phase);
    let outcome = Outcome {
        sequence: feed.sequence,
        manifest_sha256: digest,
        downloaded_packs: packs,
        wire_bytes: wire.load(Ordering::Relaxed),
        activated: !unchanged,
        elapsed_ms: started.elapsed().as_millis(),
    };
    crate::update::log(root, "packed-update", &outcome);
    status.set_final("launching", 100);
    Ok(Some(outcome))
}

#[cfg(test)]
#[path = "generation_tests.rs"]
mod tests;

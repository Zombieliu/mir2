//! Optional delivery accelerators only populate the verified content cache.
//! The signed package manifest remains the authority for every target byte.
//!
//! Bundle v1: one gzip member containing `MIR2PK01`, LE u32 entry count, then
//! entries of LE u16 ASCII-path length, path, LE u64 size, and exact raw bytes.
//! Delta v1: one gzip member containing `MIR2DP01`, LE u32 operation count,
//! LE u64 target size, then tag 0 + LE u64 base offset + LE u64 copy length or
//! tag 1 + LE u64 literal length + literal bytes. Both streams end exactly;
//! gzip concatenation, archive entry types, links, and executable tools do not
//! exist in these formats.
use crate::{fs_safe as safe, model::*};
use anyhow::{ensure, Context, Result};
use flate2::bufread::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::Instant,
};

pub const MAX_COMPRESSED: u64 = 128 * 1024 * 1024;
pub const MAX_BUNDLE_OUTPUT: u64 = 512 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 200_000;
pub const MAX_OPERATIONS: u32 = 65_536;
pub const BUNDLE_MAGIC: &[u8; 8] = b"MIR2PK01";
pub const DELTA_MAGIC: &[u8; 8] = b"MIR2DP01";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Bundle {
    pub archive: FileEntry,
    pub files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Delta {
    pub path: String,
    pub base_size: u64,
    pub base_sha256: String,
    pub patch: FileEntry,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Delivery {
    pub schema: String,
    pub candidate: String,
    pub package_manifest_sha256: String,
    pub version_sha256: String,
    pub built_unix: i64,
    pub bundles: Vec<Bundle>,
    pub patches: Vec<Delta>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub bundles: usize,
    pub patches: usize,
    pub fallbacks: usize,
}

fn artifact(entry: &FileEntry, suffix: &str, names: &mut BTreeSet<String>) -> Result<()> {
    entry.validate()?;
    ensure!(
        entry.path.starts_with("delivery/")
            && entry.path.ends_with(suffix)
            && entry.size > 0
            && entry.size <= MAX_COMPRESSED
            && names.insert(entry.path.to_ascii_lowercase()),
        "invalid/duplicate delivery artifact"
    );
    Ok(())
}

impl Delivery {
    /// Callers must authenticate these exact bytes with the pinned CMS signer
    /// before using the descriptor. Binding additionally prevents a signed
    /// accelerator for another release from influencing this target package.
    pub fn parse(
        bytes: &[u8],
        candidate: &str,
        manifest_bytes: &[u8],
        version_bytes: &[u8],
        manifest: &Manifest,
        now: i64,
    ) -> Result<Self> {
        ensure!(
            bytes.len() as u64 <= MAX_META,
            "delivery metadata too large"
        );
        let delivery: Self = serde_json::from_slice(bytes)?;
        ensure!(
            delivery.schema == "mir2.windows.delivery.v1"
                && delivery.candidate == candidate
                && delivery.package_manifest_sha256 == hash(manifest_bytes)
                && delivery.version_sha256 == hash(version_bytes)
                && delivery.built_unix > 0
                && delivery.built_unix <= now.saturating_add(300),
            "delivery release binding mismatch"
        );
        ensure!(
            delivery.bundles.len() <= 64 && delivery.patches.len() <= 64,
            "delivery object count bound"
        );
        let entries = manifest.map();
        let mut artifacts = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for bundle in &delivery.bundles {
            artifact(&bundle.archive, ".m2b.gz", &mut artifacts)?;
            ensure!(
                !bundle.files.is_empty() && bundle.files.len() <= MAX_ENTRIES,
                "delivery bundle file count bound"
            );
            let mut size = 12u64;
            for path in &bundle.files {
                game_path(path)?;
                let entry = entries
                    .get(path.as_str())
                    .context("delivery file absent from signed manifest")?;
                ensure!(
                    paths.insert(path.as_str()) && paths.len() <= MAX_ENTRIES,
                    "duplicate/oversized delivery file set"
                );
                size = size
                    .checked_add(10 + path.len() as u64)
                    .and_then(|size| size.checked_add(entry.size))
                    .context("delivery bundle size overflow")?;
            }
            ensure!(
                size <= MAX_BUNDLE_OUTPUT,
                "delivery expanded bundle size bound"
            );
        }
        let mut bases = BTreeSet::new();
        for delta in &delivery.patches {
            artifact(&delta.patch, ".m2d.gz", &mut artifacts)?;
            ensure!(
                delta.path == "mir2-platform-windows.exe"
                    && entries.contains_key(delta.path.as_str())
                    && delta.base_size > 0
                    && delta.base_size <= MAX_FILE
                    && valid_hash(&delta.base_sha256)
                    && bases.insert((delta.path.as_str(), delta.base_sha256.as_str())),
                "invalid/duplicate delta base/target"
            );
        }
        ensure!(
            delivery.extra_cache_bytes() <= MAX_TOTAL,
            "delivery compressed total bound"
        );
        Ok(delivery)
    }

    pub fn extra_cache_bytes(&self) -> u64 {
        self.bundles
            .iter()
            .map(|b| b.archive.size)
            .chain(self.patches.iter().map(|p| p.patch.size))
            .fold(0, u64::saturating_add)
    }
}

fn fallback(
    root: &Path,
    status: &impl super::Status,
    stats: &mut Stats,
    kind: &str,
    path: &str,
    error: &anyhow::Error,
    wire_bytes: u64,
) -> Result<()> {
    ensure!(!status.cancelled(), "update cancelled");
    stats.fallbacks += 1;
    super::log(
        root,
        kind,
        serde_json::json!({"path":path,"reason":format!("{error:#}"),"wireBytes":wire_bytes}),
    );
    Ok(())
}

/// Try signed accelerators, then let the caller's original per-file repair
/// consume the resulting cache. No accelerator writes an activation target.
pub(super) struct Acceleration<'a, S, T> {
    pub(super) root: &'a Path,
    pub(super) source: &'a S,
    pub(super) component: &'a Component,
    pub(super) status: &'a T,
    pub(super) total: u64,
    pub(super) transfers: &'a mut super::Transfers,
    pub(super) stats: &'a mut Stats,
}
impl<S: super::Source, T: super::Status> Acceleration<'_, S, T> {
    pub(super) fn run(
        &mut self,
        delivery: &Delivery,
        manifest: &Manifest,
        pending: &[FileEntry],
    ) -> Result<()> {
        let root = self.root;
        let source = self.source;
        let component = self.component;
        let status = self.status;
        let total = self.total;
        let transfers = &mut *self.transfers;
        let stats = &mut *self.stats;
        let mut needed = BTreeMap::new();
        for entry in pending {
            if !safe::matches(&payload_cache(root, entry)?, entry)? {
                needed.insert(entry.path.as_str(), entry);
            }
        }
        let mut check = || -> Result<()> {
            ensure!(!status.cancelled(), "update cancelled");
            Ok(())
        };
        if let Some(target) = needed.get("mir2-platform-windows.exe").copied() {
            let base_path = safe::target(&root.join("game"), &target.path)?;
            if !delivery.patches.is_empty() && base_path.exists() {
                let (size, sha256) = safe::digest_file(&base_path)?;
                if let Some(delta) = delivery.patches.iter().find(|delta| {
                    delta.base_size == size
                        && delta.base_sha256 == sha256
                        && delta.patch.size < target.size
                }) {
                    let base = FileEntry {
                        path: delta.path.clone(),
                        size,
                        sha256,
                    };
                    let started = Instant::now();
                    let before = transfers.wire_bytes;
                    let result = (|| -> Result<()> {
                        let (cache, _) = super::cached_payload(
                            root,
                            source,
                            &format!("{}/{}", component.directory, delta.patch.path),
                            &delta.patch,
                            status,
                            (0, total),
                            transfers,
                        )?;
                        let mut phase =
                            super::LocalPhase::new(root, status, "extracting", 1, (30, 65));
                        let result = apply_delta(
                            root,
                            &cache,
                            &delta.patch,
                            &base_path,
                            &base,
                            target,
                            &mut check,
                        );
                        if result.is_ok() {
                            phase.advance(1);
                            phase.finish();
                        }
                        result
                    })();
                    match result {
                        Ok(()) => {
                            stats.patches += 1;
                            needed.remove("mir2-platform-windows.exe");
                            super::log(
                                root,
                                "delivery-delta",
                                serde_json::json!({
                                    "path":delta.path,"patch":delta.patch.path,"wireBytes":transfers.wire_bytes-before,
                                    "targetBytes":target.size,"elapsedMs":started.elapsed().as_millis()
                                }),
                            );
                        }
                        Err(error) => fallback(
                            root,
                            status,
                            stats,
                            "delivery-delta-fallback",
                            &delta.patch.path,
                            &error,
                            transfers.wire_bytes - before,
                        )?,
                    }
                } else {
                    super::log(
                        root,
                        "delivery-delta-base-unavailable",
                        serde_json::json!({"path":target.path,"size":size,"sha256":sha256}),
                    );
                }
            }
        }
        let entries = manifest.map();
        for bundle in &delivery.bundles {
            let count = bundle
                .files
                .iter()
                .filter(|path| needed.contains_key(path.as_str()))
                .count();
            let bytes = bundle
                .files
                .iter()
                .filter_map(|path| needed.get(path.as_str()))
                .map(|entry| entry.size)
                .sum::<u64>();
            // A full initial bundle amortizes many requests. A small repair should
            // not fetch a large bundle just because one contained asset is missing.
            if count == 0
                || !(count == bundle.files.len() || (count >= 256 && bundle.archive.size <= bytes))
            {
                continue;
            }
            let expected = bundle
                .files
                .iter()
                .map(|path| entries[path.as_str()])
                .collect::<Vec<_>>();
            let started = Instant::now();
            let before = transfers.wire_bytes;
            let result = (|| -> Result<()> {
                let (cache, _) = super::cached_payload(
                    root,
                    source,
                    &format!("{}/{}", component.directory, bundle.archive.path),
                    &bundle.archive,
                    status,
                    (0, total),
                    transfers,
                )?;
                let mut phase =
                    super::LocalPhase::new(root, status, "extracting", expected.len(), (30, 65));
                let result = extract_bundle_observed(
                    root,
                    &cache,
                    &bundle.archive,
                    &expected,
                    &mut check,
                    &mut |completed| phase.advance(completed),
                );
                if result.is_ok() {
                    phase.finish();
                }
                result
            })();
            match result {
                Ok(()) => {
                    stats.bundles += 1;
                    for path in &bundle.files {
                        needed.remove(path.as_str());
                    }
                    super::log(
                        root,
                        "delivery-bundle",
                        serde_json::json!({
                            "path":bundle.archive.path,"fileCount":expected.len(),"wireBytes":transfers.wire_bytes-before,
                            "elapsedMs":started.elapsed().as_millis()
                        }),
                    );
                }
                Err(error) => fallback(
                    root,
                    status,
                    stats,
                    "delivery-bundle-fallback",
                    &bundle.archive.path,
                    &error,
                    transfers.wire_bytes - before,
                )?,
            }
        }
        check()
    }
}

pub fn payload_cache(root: &Path, entry: &FileEntry) -> Result<PathBuf> {
    entry.validate()?;
    safe::target(root, &format!(".update/downloads/{}", entry.sha256))
}

fn compressed_reader(path: &Path, entry: &FileEntry) -> Result<GzDecoder<BufReader<File>>> {
    entry.validate()?;
    ensure!(
        entry.size > 0 && entry.size <= MAX_COMPRESSED,
        "delivery size bound"
    );
    ensure!(
        safe::matches(path, entry)?,
        "delivery artifact hash mismatch"
    );
    Ok(GzDecoder::new(BufReader::new(File::open(path)?)))
}

fn number<const N: usize>(reader: &mut impl Read) -> Result<[u8; N]> {
    let mut bytes = [0; N];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn finish_gzip(mut reader: GzDecoder<BufReader<File>>) -> Result<()> {
    ensure!(reader.read(&mut [0])? == 0, "trailing delivery data");
    ensure!(
        reader.into_inner().fill_buf()?.is_empty(),
        "trailing gzip member/data"
    );
    Ok(())
}

fn remove_part(path: &Path) -> Result<()> {
    safe::ancestors(path)?;
    if path.exists() {
        safe::regular(path)?;
        fs::remove_file(path)?;
    }
    Ok(())
}

fn new_cache_part(root: &Path, entry: &FileEntry) -> Result<(PathBuf, PathBuf, File)> {
    let cache = payload_cache(root, entry)?;
    let part = cache.with_extension("delivery-part");
    remove_part(&part)?;
    safe::ancestors(&cache)?;
    fs::create_dir_all(cache.parent().context("cache directory")?)?;
    safe::ancestors(&cache)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)?;
    Ok((cache, part, file))
}

fn publish_part(cache: &Path, part: &Path, entry: &FileEntry) -> Result<()> {
    ensure!(
        safe::matches(part, entry)?,
        "reconstructed target hash mismatch"
    );
    safe::ancestors(cache)?;
    if cache.exists() {
        safe::regular(cache)?;
    }
    crate::platform::replace_file(part, cache)?;
    ensure!(
        safe::matches(cache, entry)?,
        "published target hash mismatch"
    );
    Ok(())
}

fn copy_exact(
    reader: &mut impl Read,
    mut remaining: u64,
    mut file: Option<&mut File>,
    digest: &mut Sha256,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let mut buffer = [0; 65_536];
    while remaining != 0 {
        check()?;
        let count = remaining.min(buffer.len() as u64) as usize;
        reader.read_exact(&mut buffer[..count])?;
        digest.update(&buffer[..count]);
        if let Some(file) = file.as_deref_mut() {
            file.write_all(&buffer[..count])?;
        }
        remaining -= count as u64;
    }
    Ok(())
}

fn cache_entry(
    root: &Path,
    entry: &FileEntry,
    reader: &mut impl Read,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let cache = payload_cache(root, entry)?;
    if safe::matches(&cache, entry)? {
        let mut digest = Sha256::new();
        copy_exact(reader, entry.size, None, &mut digest, check)?;
        ensure!(
            format!("{:X}", digest.finalize()) == entry.sha256,
            "bundle target hash mismatch"
        );
        return Ok(());
    }
    let (cache, part, mut file) = new_cache_part(root, entry)?;
    let result = (|| -> Result<()> {
        let mut digest = Sha256::new();
        copy_exact(reader, entry.size, Some(&mut file), &mut digest, check)?;
        ensure!(
            format!("{:X}", digest.finalize()) == entry.sha256,
            "bundle target hash mismatch"
        );
        // The compressed artifact is durable. Cache content may be rebuilt
        // after interruption and is rehashed before staging/on retry; keep
        // activation and staging durability in the original transaction path.
        drop(file);
        publish_part(&cache, &part, entry)
    })();
    if result.is_err() {
        remove_part(&part)?;
    }
    result
}

/// Decode exactly the manifest entries declared by an authenticated bundle.
/// A failed bundle may retain individually verified cache entries, never live
/// installation files; subsequent per-file repair safely reuses those entries.
pub fn extract_bundle(
    root: &Path,
    archive_path: &Path,
    archive: &FileEntry,
    expected: &[&FileEntry],
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    extract_bundle_observed(root, archive_path, archive, expected, check, &mut |_| {})
}
fn extract_bundle_observed(
    root: &Path,
    archive_path: &Path,
    archive: &FileEntry,
    expected: &[&FileEntry],
    check: &mut dyn FnMut() -> Result<()>,
    completed: &mut dyn FnMut(usize),
) -> Result<()> {
    ensure!(
        !expected.is_empty() && expected.len() <= MAX_ENTRIES,
        "bundle entry bound"
    );
    let mut entries = BTreeMap::new();
    let mut aliases = BTreeSet::new();
    let mut total = 12u64;
    for entry in expected {
        entry.validate()?;
        game_path(&entry.path)?;
        ensure!(
            !META.contains(&entry.path.as_str()),
            "bundle metadata entry"
        );
        ensure!(
            aliases.insert(entry.path.to_ascii_lowercase()),
            "duplicate/case-colliding bundle path"
        );
        ensure!(
            entries.insert(entry.path.as_str(), *entry).is_none(),
            "duplicate bundle path"
        );
        total = total
            .checked_add(10 + entry.path.len() as u64)
            .and_then(|total| total.checked_add(entry.size))
            .context("bundle size overflow")?;
    }
    ensure!(total <= MAX_BUNDLE_OUTPUT, "expanded bundle size bound");
    check()?;
    let mut reader = compressed_reader(archive_path, archive)?;
    ensure!(
        &number::<8>(&mut reader)? == BUNDLE_MAGIC,
        "bundle format mismatch"
    );
    ensure!(
        u32::from_le_bytes(number(&mut reader)?) as usize == entries.len(),
        "bundle entry count mismatch"
    );
    let mut seen = BTreeSet::new();
    for index in 0..entries.len() {
        check()?;
        let length = u16::from_le_bytes(number(&mut reader)?) as usize;
        ensure!(length > 0 && length <= 230, "bundle path length bound");
        let mut path = vec![0; length];
        reader.read_exact(&mut path)?;
        let path = std::str::from_utf8(&path)?;
        game_path(path)?;
        let entry = entries.get(path).context("unknown bundle entry")?;
        ensure!(seen.insert(path.to_owned()), "duplicate bundle entry");
        ensure!(
            u64::from_le_bytes(number(&mut reader)?) == entry.size,
            "bundle target size mismatch"
        );
        cache_entry(root, entry, &mut reader, check)?;
        completed(index + 1);
    }
    finish_gzip(reader)?;
    check()
}

/// Reconstruct the sole permitted executable from bounded copy/literal ops.
/// The verified base is read in place; output only becomes a cache payload once
/// its final size/hash agrees with the separately signed target manifest.
pub fn apply_delta(
    root: &Path,
    patch_path: &Path,
    patch: &FileEntry,
    base_path: &Path,
    base: &FileEntry,
    target: &FileEntry,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    base.validate()?;
    target.validate()?;
    ensure!(
        base.path == "mir2-platform-windows.exe" && target.path == base.path,
        "unsupported delta target"
    );
    ensure!(
        target.size > 0 && safe::matches(base_path, base)?,
        "delta base mismatch"
    );
    check()?;
    let mut reader = compressed_reader(patch_path, patch)?;
    ensure!(
        &number::<8>(&mut reader)? == DELTA_MAGIC,
        "delta format mismatch"
    );
    let operations = u32::from_le_bytes(number(&mut reader)?);
    ensure!(
        operations > 0 && operations <= MAX_OPERATIONS,
        "delta operation bound"
    );
    ensure!(
        u64::from_le_bytes(number(&mut reader)?) == target.size,
        "delta target size mismatch"
    );
    let mut source = File::open(base_path)?;
    let (cache, part, mut file) = new_cache_part(root, target)?;
    let result = (|| -> Result<()> {
        let mut size = 0u64;
        let mut digest = Sha256::new();
        for _ in 0..operations {
            check()?;
            let tag = number::<1>(&mut reader)?[0];
            let offset = if tag == 0 {
                Some(u64::from_le_bytes(number(&mut reader)?))
            } else {
                None
            };
            ensure!(tag <= 1, "unknown delta operation");
            let length = u64::from_le_bytes(number(&mut reader)?);
            ensure!(length > 0, "empty delta operation");
            size = size.checked_add(length).context("delta output overflow")?;
            ensure!(size <= target.size, "oversized delta output");
            if let Some(offset) = offset {
                ensure!(
                    offset
                        .checked_add(length)
                        .is_some_and(|end| end <= base.size),
                    "delta copy outside base"
                );
                source.seek(SeekFrom::Start(offset))?;
                copy_exact(&mut source, length, Some(&mut file), &mut digest, check)?;
            } else {
                copy_exact(&mut reader, length, Some(&mut file), &mut digest, check)?;
            }
        }
        ensure!(
            size == target.size && format!("{:X}", digest.finalize()) == target.sha256,
            "delta target hash/size mismatch"
        );
        finish_gzip(reader)?;
        ensure!(
            safe::matches(base_path, base)?,
            "delta base changed during reconstruction"
        );
        // A durable patch can reconstruct an interrupted cache write. The
        // original staging/activation path still flushes and rechecks bytes.
        drop(file);
        check()?;
        publish_part(&cache, &part, target)
    })();
    if result.is_err() {
        remove_part(&part)?;
    }
    result
}

#[cfg(test)]
#[path = "delivery_tests.rs"]
mod tests;

//! Generation-v2 availability is granted only by a signed, catalog-bound scope.
//! This is independent of the v1 full-store contract and never authorizes an
//! alternate catalog, unsigned URL, loose resource or placeholder asset.
use super::*;
use mir2_resource_pack::{scoped::RequiredResourceScope, ScopedPackStore};

pub(super) fn required_pack_entries(
    manifest: &GenerationManifest,
    catalog_bytes: &[u8],
    scope_bytes: Option<&[u8]>,
) -> Result<Vec<FileEntry>> {
    let catalog = Catalog::parse(catalog_bytes, &manifest.catalog.sha256)?;
    let expected: BTreeSet<_> = manifest
        .packs
        .iter()
        .map(|pack| (pack.sha256.clone(), pack.size))
        .collect();
    let found: BTreeSet<_> = catalog
        .objects()
        .iter()
        .map(|pack| (pack.sha256.clone(), pack.size))
        .collect();
    ensure!(
        expected.len() == manifest.packs.len() && expected == found,
        "generation/catalog pack closure differs"
    );
    match (
        manifest.schema.as_str(),
        manifest.required_scope.as_ref(),
        scope_bytes,
    ) {
        ("mir2.windows.generation.v1", None, None) => Ok(manifest.packs.clone()),
        ("mir2.windows.generation.v2", Some(entry), Some(bytes)) => {
            ensure!(
                entry.path == "required-scope.json"
                    && !bytes.is_empty()
                    && bytes.len() as u64 == entry.size
                    && entry.size <= MAX_REQUIRED_SCOPE,
                "required scope length/path differs"
            );
            let scope = RequiredResourceScope::parse(
                bytes,
                &entry.sha256,
                &manifest.catalog.sha256,
                &catalog,
            )?;
            let required: BTreeSet<_> = scope
                .required_objects(&catalog)?
                .into_iter()
                .map(|object| object.sha256)
                .collect();
            let entries: Vec<_> = manifest
                .packs
                .iter()
                .filter(|entry| required.contains(&entry.sha256))
                .cloned()
                .collect();
            ensure!(
                !entries.is_empty() && entries.len() == required.len(),
                "required pack closure differs from signed manifest"
            );
            Ok(entries)
        }
        _ => anyhow::bail!("generation schema/required-scope authority differs"),
    }
}

/// Keep the real immutable pack handles alive through selector commit. An
/// optional object remains pending even when an unadmitted file is on disk.
pub(super) enum PackLease {
    Full { _store: PackStore },
    Scoped { _store: ScopedPackStore },
}

pub(super) fn open_packs(
    root: &Path,
    manifest: &GenerationManifest,
    catalog: &[u8],
    scope: Option<&[u8]>,
) -> Result<PackLease> {
    required_pack_entries(manifest, catalog, scope)?;
    let objects = safe::target(root, "objects/packs")?;
    match scope {
        None => Ok(PackLease::Full {
            _store: PackStore::open(objects, catalog, &manifest.catalog.sha256)?,
        }),
        Some(bytes) => Ok(PackLease::Scoped {
            _store: ScopedPackStore::open(
                objects,
                catalog,
                &manifest.catalog.sha256,
                bytes,
                &manifest
                    .required_scope
                    .as_ref()
                    .context("missing signed required scope")?
                    .sha256,
            )?,
        }),
    }
}

pub(super) struct Metadata {
    catalog: Vec<u8>,
    scope: Vec<u8>,
    pub packs: Vec<FileEntry>,
}
impl Metadata {
    pub fn control_bytes(&self, entry: &FileEntry) -> Option<&[u8]> {
        match entry.path.as_str() {
            "catalog.json" => Some(&self.catalog),
            "required-scope.json" => Some(&self.scope),
            _ => None,
        }
    }
}

/// Read-only bounded planning. Network metadata can precede consent, is shown
/// separately, and is never described as a pre-consent payload-free network
/// operation. Cancellation leaves selectors and payload files untouched.
pub(super) fn fetch_metadata(
    root: &Path,
    generation: &Path,
    manifest_digest: &str,
    source: &impl Source,
    manifest: &GenerationManifest,
    status: &impl Status,
    wire: &AtomicU64,
) -> Result<Metadata> {
    let scope = manifest
        .required_scope
        .as_ref()
        .context("progressive metadata requires signed scope")?;
    let entries = [&manifest.catalog, scope];
    let total = entries.iter().try_fold(0u64, |sum, entry| {
        let present = safe::target(generation, &entry.path)?.exists();
        Ok::<_, anyhow::Error>(sum + if present { 0 } else { entry.size })
    })?;
    let mut fetched = 0;
    let mut values = Vec::with_capacity(2);
    status.set_final("checking", 5);
    status.transfer(0, total);
    for entry in entries {
        ensure!(!status.cancelled(), "packed metadata planning cancelled");
        let path = safe::target(generation, &entry.path)?;
        let bytes = if path.exists() {
            let bytes = safe::read_bounded(&path, entry.size)?;
            ensure!(
                bytes.len() as u64 == entry.size && model::hash(&bytes) == entry.sha256,
                "cached immutable control object differs"
            );
            bytes
        } else {
            let remote = FileEntry {
                path: format!("{}/{}", directory(manifest_digest)?, entry.path),
                ..entry.clone()
            };
            let bytes = fetch_checked_progress(source, &remote, status, wire, |count| {
                status.transfer(fetched + count, total);
            })?;
            fetched += entry.size;
            bytes
        };
        values.push(bytes);
    }
    let scope = values.pop().context("scope metadata unavailable")?;
    let catalog = values.pop().context("catalog metadata unavailable")?;
    let packs = required_pack_entries(manifest, &catalog, Some(&scope))?;
    crate::update::log(
        root,
        "packed-progressive-metadata",
        serde_json::json!({
            "preConsentMetadataBytes": fetched,
            "payloadWritten": false,
            "requiredPackCount": packs.len(),
            "fullAssetCompletionClaimed": false,
        }),
    );
    Ok(Metadata {
        catalog,
        scope,
        packs,
    })
}

#[cfg(test)]
#[path = "generation_progressive_tests.rs"]
mod tests;

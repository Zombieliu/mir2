//! Controlled availability under an authenticated full catalog and an
//! independently authenticated required-scope digest. This module does not
//! authenticate CMS, prove dependency closure, download, or activate releases.
use crate::{
    digest, directories, read_pack, sha256, validate_logical_path, Catalog, PackFile, PackObject,
    MAX_RESOURCES,
};
use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

pub const REQUIRED_SCOPE_SCHEMA: &str = "mir2.native.required-resource-scope.v1";
pub const MAX_SCOPE_BYTES: usize = 4 * 1024 * 1024;

/// A caller-authenticated list of required logical resources, not a claim that
/// these resources cover every possible scene, appearance or gameplay state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RequiredResourceScope {
    pub schema: String,
    pub catalog_sha256: String,
    /// Opaque printable ASCII, 1..64 bytes. Never interpreted as a path.
    pub id: String,
    #[serde(deserialize_with = "bounded_required_paths")]
    pub required_paths: Vec<String>,
}

fn bounded_required_paths<'de, D: Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<String>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "at most {MAX_RESOURCES} required paths")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut paths = Vec::new();
            while let Some(path) = seq.next_element::<String>()? {
                if paths.len() == MAX_RESOURCES {
                    return Err(serde::de::Error::custom("required path count bound"));
                }
                paths.push(path);
            }
            Ok(paths)
        }
    }
    d.deserialize_seq(Bounded)
}

impl RequiredResourceScope {
    /// Metadata-only validation. `catalog` must be the result of
    /// `Catalog::parse(full_bytes, expected_catalog_sha)` from the caller's
    /// authenticated release flow. Both expected digests come from that flow;
    /// this method does not authenticate CMS or certify dependency closure.
    pub fn parse(
        bytes: &[u8],
        expected_scope_sha: &str,
        expected_catalog_sha: &str,
        catalog: &Catalog,
    ) -> Result<Self> {
        ensure!(bytes.len() <= MAX_SCOPE_BYTES, "required scope byte bound");
        ensure!(
            sha256(bytes) == digest(expected_scope_sha)?,
            "required scope SHA mismatch"
        );
        let mut scope: Self = serde_json::from_slice(bytes)?;
        scope.catalog_sha256 = digest(&scope.catalog_sha256)?;
        scope.validate_for(&digest(expected_catalog_sha)?, catalog)?;
        Ok(scope)
    }

    /// Select declared pack descriptors before any object files exist. Pass
    /// the same authenticated Catalog used by `parse`. This helper rechecks
    /// scope structure/membership, but cannot re-authenticate a Catalog model
    /// without its original bytes and caller-owned expected digest.
    pub fn required_objects(&self, catalog: &Catalog) -> Result<Vec<PackObject>> {
        self.validate_for(&digest(&self.catalog_sha256)?, catalog)?;
        let ids: BTreeSet<_> = self
            .required_paths
            .iter()
            .map(|path| catalog.resources[path].pack_sha256.as_str())
            .collect();
        let objects: Vec<_> = catalog
            .packs
            .iter()
            .filter(|object| ids.contains(object.sha256.as_str()))
            .cloned()
            .collect();
        ensure!(
            objects.len() == ids.len(),
            "required pack absent/duplicated in full catalog"
        );
        Ok(objects)
    }

    fn validate_for(&self, catalog_sha: &str, catalog: &Catalog) -> Result<()> {
        ensure!(
            self.schema == REQUIRED_SCOPE_SCHEMA,
            "unsupported required scope schema"
        );
        ensure!(
            self.catalog_sha256 == catalog_sha,
            "scope/full catalog SHA mismatch"
        );
        ensure!(
            (1..=64).contains(&self.id.len()) && self.id.bytes().all(|b| b.is_ascii_graphic()),
            "scope id must be 1..64 printable ASCII bytes"
        );
        ensure!(
            !self.required_paths.is_empty() && self.required_paths.len() <= MAX_RESOURCES,
            "required path count bound"
        );
        let mut previous: Option<&str> = None;
        let mut folded = BTreeSet::new();
        for path in &self.required_paths {
            validate_logical_path(path)?;
            ensure!(
                previous.is_none_or(|old| old < path.as_str()),
                "required paths must be sorted and unique"
            );
            ensure!(
                folded.insert(path.to_ascii_lowercase()),
                "case-colliding required paths"
            );
            ensure!(
                catalog.resources.contains_key(path),
                "required path absent from full catalog: {path}"
            );
            previous = Some(path);
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScopedRead {
    Ready(Vec<u8>),
    /// The logical resource exists. Its declared pack has not been admitted,
    /// regardless of whether an unverified file happens to exist on disk.
    Pending(PackObject),
}

/// Required objects are fully checked on open. Other declared objects can be
/// admitted by a caller-owned background thread. The map lock protects only
/// Arc publication: no whole-pack validation or resource I/O holds it.
///
/// Windows handles retain the full store's read/delete-deny sharing policy.
/// Unix callers retain the same immutable-directory requirement as PackStore.
#[derive(Debug)]
pub struct ScopedPackStore {
    root: PathBuf,
    catalog: Catalog,
    catalog_sha: String,
    scope: RequiredResourceScope,
    scope_sha: String,
    required: Vec<PackObject>,
    files: RwLock<BTreeMap<String, Arc<PackFile>>>,
    _directory_leases: Vec<File>,
}

impl ScopedPackStore {
    pub fn open(
        objects_root: impl AsRef<Path>,
        catalog_bytes: &[u8],
        expected_catalog_sha: &str,
        scope_bytes: &[u8],
        expected_scope_sha: &str,
    ) -> Result<Self> {
        let catalog = Catalog::parse(catalog_bytes, expected_catalog_sha)?;
        let catalog_sha = digest(expected_catalog_sha)?;
        let scope =
            RequiredResourceScope::parse(scope_bytes, expected_scope_sha, &catalog_sha, &catalog)?;
        let root = objects_root.as_ref();
        let leases = directories(root)?;
        let required = scope.required_objects(&catalog)?;
        let mut files = BTreeMap::new();
        for object in &required {
            let pack = read_pack(root, object)?;
            validate_all_locators(&catalog, &object.sha256, &pack)?;
            files.insert(object.sha256.clone(), Arc::new(pack));
        }
        Ok(Self {
            root: root.to_owned(),
            catalog,
            catalog_sha,
            scope,
            scope_sha: digest(expected_scope_sha)?,
            required,
            files: RwLock::new(files),
            _directory_leases: leases,
        })
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub fn catalog_sha256(&self) -> &str {
        &self.catalog_sha
    }
    pub fn scope(&self) -> &RequiredResourceScope {
        &self.scope
    }
    pub fn scope_sha256(&self) -> &str {
        &self.scope_sha
    }
    /// Logical existence is independent of cache/admission state.
    pub fn exists(&self, path: &str) -> bool {
        self.catalog.resources.contains_key(path)
    }
    pub fn required_objects(&self) -> &[PackObject] {
        &self.required
    }

    pub fn pending_objects(&self) -> Result<Vec<PackObject>> {
        let files = self
            .files
            .read()
            .map_err(|_| anyhow!("scoped pack map poisoned"))?;
        Ok(self
            .catalog
            .packs
            .iter()
            .filter(|object| !files.contains_key(&object.sha256))
            .cloned()
            .collect())
    }

    pub fn is_fully_available(&self) -> Result<bool> {
        let files = self
            .files
            .read()
            .map_err(|_| anyhow!("scoped pack map poisoned"))?;
        Ok(files.len() == self.catalog.packs.len())
    }

    fn admitted(&self, sha: &str) -> Result<Option<Arc<PackFile>>> {
        let files = self
            .files
            .read()
            .map_err(|_| anyhow!("scoped pack map poisoned"))?;
        Ok(files.get(sha).cloned())
    }

    /// Only performs per-resource hash/positional I/O on an admitted handle.
    /// It never discovers, opens or whole-hashes a new cached pack.
    pub fn read(&self, path: &str) -> Result<ScopedRead> {
        validate_logical_path(path)?;
        let entry = self
            .catalog
            .resources
            .get(path)
            .context("resource absent from full catalog")?;
        if let Some(pack) = self.admitted(&entry.pack_sha256)? {
            return Ok(ScopedRead::Ready(pack.read_resource(entry)?));
        }
        let object = self
            .catalog
            .packs
            .iter()
            .find(|object| object.sha256 == entry.pack_sha256)
            .context("catalog pack missing")?;
        Ok(ScopedRead::Pending(object.clone()))
    }

    /// Caller-owned background work. Only a declared SHA is accepted. Returns
    /// true for a newly published pack, false for an already admitted pack.
    /// Validation failure publishes nothing; concurrent valid callers are
    /// idempotent. No network or thread scheduling occurs inside this method.
    pub fn admit_cached_pack(&self, sha: &str) -> Result<bool> {
        let sha = digest(sha)?;
        let object = self
            .catalog
            .packs
            .iter()
            .find(|object| object.sha256 == sha)
            .context("pack not declared in full catalog")?;
        if self.admitted(&sha)?.is_some() {
            return Ok(false);
        }
        // No file-map write lock (or read lock) during any validation.
        let pack = read_pack(&self.root, object)?;
        validate_all_locators(&self.catalog, &sha, &pack)?;
        let pack = Arc::new(pack);
        let mut files = self
            .files
            .write()
            .map_err(|_| anyhow!("scoped pack map poisoned"))?;
        match files.entry(sha) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(pack);
                Ok(true)
            }
            std::collections::btree_map::Entry::Occupied(_) => Ok(false),
        }
    }
}

fn validate_all_locators(catalog: &Catalog, sha: &str, pack: &PackFile) -> Result<()> {
    // Include every catalog entry in this pack, not just requiredPaths. A
    // required pack can also contain deferred resources and content aliases.
    for entry in catalog
        .resources
        .values()
        .filter(|entry| entry.pack_sha256 == sha)
    {
        pack.validate_locator(entry)?;
    }
    Ok(())
}

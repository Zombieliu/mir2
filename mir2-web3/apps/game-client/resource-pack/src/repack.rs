//! Offline full-inventory repacking. A plan is bound to an already authenticated
//! source catalog. It changes physical placement only: it cannot authorize a
//! partial installation, a new generation, signing, or resource discovery.
use crate::{
    absolute_plain_path, create_object, digest, directories, handle_check, open_file,
    pack_bytes_with, raw_digest, read_at, sha256, write_bytes_new, write_catalog_new, Catalog,
    PackObject, PackStore, ResourceInput, ResourceRef, CATALOG_SCHEMA, HEADER, MAX_CATALOG_BYTES,
    MAX_PACKS, MAX_PACK_BYTES, MAX_RESOURCES, MAX_TOTAL_BYTES, RECORD,
};
use anyhow::{ensure, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, collections::BTreeSet, fs, fs::File, path::Path};

pub const PLAN_SCHEMA: &str = "mir2.native.resource-repack-plan.v1";
pub const RECEIPT_SCHEMA: &str = "mir2.native.resource-repack-receipt.v1";
pub const DEFAULT_GROUP: &str = "default";

/// The last, implicit default group counts toward the existing 256-group bound.
/// `targetPackBytes` is a soft target, just as in `build`: an indivisible larger
/// resource may occupy one pack, but never exceed MAX_PACK_BYTES including index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepackPlan {
    pub schema: String,
    pub source_catalog_sha256: String,
    pub target_pack_bytes: u64,
    #[serde(deserialize_with = "bounded_groups")]
    pub groups: Vec<RepackGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepackGroup {
    pub name: String,
    #[serde(deserialize_with = "bounded_paths")]
    pub paths: Vec<String>,
}

fn bounded_vec<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    deserializer: D,
    limit: usize,
) -> std::result::Result<Vec<T>, D::Error> {
    struct Bounded<T>(usize, std::marker::PhantomData<T>);
    impl<'de, T: DeserializeOwned> serde::de::Visitor<'de> for Bounded<T> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "at most {} entries", self.0)
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element()? {
                if values.len() == self.0 {
                    return Err(serde::de::Error::custom("repack plan sequence bound"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Bounded(limit, std::marker::PhantomData))
}
fn bounded_groups<'de, D: Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<RepackGroup>, D::Error> {
    bounded_vec(d, MAX_PACKS - 1)
}
fn bounded_paths<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Vec<String>, D::Error> {
    bounded_vec(d, MAX_RESOURCES)
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupPreflight {
    pub name: String,
    /// All logical aliases whose payload is owned by this group, including
    /// aliases explicitly requested in later groups or left in default.
    pub logical_resource_count: usize,
    pub unique_payload_count: usize,
    pub payload_bytes: u64,
    pub pack_count: usize,
    pub pack_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepackPreflight {
    pub schema: &'static str,
    pub source_catalog_sha256: String,
    pub plan_sha256: String,
    pub target_pack_bytes: u64,
    pub resource_count: usize,
    pub logical_bytes: u64,
    pub unique_payload_count: usize,
    pub unique_payload_bytes: u64,
    pub projected_pack_count: usize,
    pub projected_pack_bytes: u64,
    pub projected_catalog_bytes: usize,
    /// SHA256 of domain then each lexically sorted (path length u64 LE, UTF8
    /// path, resource size u64 LE, raw32 resource SHA). Physical locators excluded.
    pub logical_inventory_sha256: String,
    pub groups: Vec<GroupPreflight>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupObjects {
    pub name: String,
    pub packs: Vec<PackObject>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepackReceipt {
    pub schema: &'static str,
    pub preflight: RepackPreflight,
    pub catalog_sha256: String,
    pub catalog_bytes: usize,
    pub groups: Vec<GroupObjects>,
    pub verified_unique_payloads: usize,
    pub full_logical_equivalence_verified: bool,
    pub output_pack_store_verified: bool,
    pub bootstrap_authorized: bool,
}

struct Chunk {
    group: usize,
    inputs: Vec<ResourceInput>,
}
struct Prepared {
    summary: RepackPreflight,
    chunks: Vec<Chunk>,
}

fn inventory_sha(catalog: &Catalog) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(b"mir2.native.logical-resource-inventory.v1\0");
    for (path, entry) in &catalog.resources {
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
        hash.update(entry.size.to_le_bytes());
        hash.update(raw_digest(&entry.sha256)?);
    }
    Ok(format!("{:X}", hash.finalize()))
}

fn relocated_catalog(
    source: &PackStore,
    locators: &BTreeMap<String, ResourceRef>,
    packs: Vec<PackObject>,
) -> Result<Catalog> {
    let mut catalog = Catalog {
        schema: CATALOG_SCHEMA.into(),
        resources: BTreeMap::new(),
        packs,
    };
    for (path, original) in &source.catalog.resources {
        let entry = locators
            .get(&original.sha256)
            .context("repack content locator missing")?;
        ensure!(
            entry.size == original.size && entry.sha256 == original.sha256,
            "repack changed logical content"
        );
        catalog.resources.insert(path.clone(), entry.clone());
    }
    catalog.validate()?;
    Ok(catalog)
}

fn prepare(source: &PackStore, bytes: &[u8]) -> Result<Prepared> {
    ensure!(bytes.len() <= MAX_CATALOG_BYTES, "repack plan byte bound");
    let plan: RepackPlan = serde_json::from_slice(bytes)?;
    ensure!(plan.schema == PLAN_SCHEMA, "unsupported repack plan schema");
    ensure!(
        digest(&plan.source_catalog_sha256)? == source.catalog_sha,
        "repack plan/source catalog SHA mismatch"
    );
    ensure!(
        (HEADER + RECORD..=MAX_PACK_BYTES).contains(&plan.target_pack_bytes),
        "repack target byte bound"
    );
    ensure!(
        !source.catalog.resources.is_empty(),
        "repack requires a nonempty full source catalog"
    );
    let mut names = BTreeSet::from([DEFAULT_GROUP.to_owned()]);
    let mut requested = BTreeMap::new();
    let mut path_names = BTreeSet::new();
    let mut groups = Vec::new();
    for (index, group) in plan.groups.iter().enumerate() {
        crate::validate_logical_path(&group.name)?;
        ensure!(
            names.insert(group.name.to_ascii_lowercase()),
            "duplicate/reserved repack group name"
        );
        ensure!(!group.paths.is_empty(), "empty explicit repack group");
        for path in &group.paths {
            crate::validate_logical_path(path)?;
            ensure!(
                path_names.insert(path.to_ascii_lowercase()),
                "duplicate/case-colliding repack plan path"
            );
            ensure!(
                source.catalog.resources.contains_key(path),
                "repack plan path absent from source catalog: {path}"
            );
            requested.insert(path.clone(), index);
            ensure!(requested.len() <= MAX_RESOURCES, "repack path count bound");
        }
        groups.push(GroupPreflight {
            name: group.name.clone(),
            ..Default::default()
        });
    }
    groups.push(GroupPreflight {
        name: DEFAULT_GROUP.into(),
        ..Default::default()
    });
    let default = plan.groups.len();
    // Iterate the entire source, not just the plan. Lowest group index wins
    // across aliases; equal-priority aliases use their first lexical path.
    let mut owners = BTreeMap::<String, (usize, ResourceInput)>::new();
    let mut logical_bytes = 0u64;
    for (path, entry) in &source.catalog.resources {
        logical_bytes = logical_bytes
            .checked_add(entry.size)
            .context("repack logical total overflow")?;
        let group = requested.get(path).copied().unwrap_or(default);
        let replace = owners
            .get(&entry.sha256)
            .is_none_or(|(old, _)| group < *old);
        if replace {
            owners.insert(
                entry.sha256.clone(),
                (
                    group,
                    ResourceInput {
                        path: path.clone(),
                        size: entry.size,
                        sha256: entry.sha256.clone(),
                    },
                ),
            );
        }
    }
    ensure!(
        logical_bytes <= MAX_TOTAL_BYTES,
        "repack logical total bound"
    );
    for entry in source.catalog.resources.values() {
        groups[owners[&entry.sha256].0].logical_resource_count += 1;
    }
    let mut by_group = vec![Vec::new(); groups.len()];
    for (group, input) in owners.values() {
        groups[*group].unique_payload_count += 1;
        groups[*group].payload_bytes = groups[*group]
            .payload_bytes
            .checked_add(input.size)
            .context("repack group payload overflow")?;
        by_group[*group].push(input.clone());
    }
    let mut chunks = Vec::new();
    let mut projected_packs = Vec::new();
    let mut projected_locators = BTreeMap::new();
    for (index, inputs) in by_group.into_iter().enumerate() {
        let mut pending = Vec::new();
        let mut size = HEADER;
        for input in inputs {
            let cost = RECORD
                .checked_add(input.size)
                .context("repack cost overflow")?;
            ensure!(
                cost.checked_add(HEADER)
                    .is_some_and(|n| n <= MAX_PACK_BYTES),
                "resource cannot fit bounded repack object"
            );
            if !pending.is_empty() && size + cost > plan.target_pack_bytes {
                chunks.push(Chunk {
                    group: index,
                    inputs: std::mem::take(&mut pending),
                });
                size = HEADER;
            }
            size += cost;
            pending.push(input);
        }
        if !pending.is_empty() {
            chunks.push(Chunk {
                group: index,
                inputs: pending,
            });
        }
    }
    ensure!(chunks.len() <= MAX_PACKS, "repack object count bound");
    for (index, chunk) in chunks.iter().enumerate() {
        let payload = chunk.inputs.iter().try_fold(0u64, |sum, input| {
            sum.checked_add(input.size)
                .context("repack payload overflow")
        })?;
        let mut cursor = HEADER + chunk.inputs.len() as u64 * RECORD;
        let size = cursor
            .checked_add(payload)
            .context("repack size overflow")?;
        ensure!(size <= MAX_PACK_BYTES, "repack object byte bound");
        // A fixed-width stand-in permits exact catalog size/closure validation
        // before payload reads or writes. These IDs are never output as objects.
        let placeholder = format!("{:064X}", index + 1);
        projected_packs.push(PackObject {
            sha256: placeholder.clone(),
            size,
        });
        groups[chunk.group].pack_count += 1;
        groups[chunk.group].pack_bytes += size;
        for input in &chunk.inputs {
            projected_locators.insert(
                input.sha256.clone(),
                ResourceRef {
                    sha256: input.sha256.clone(),
                    size: input.size,
                    pack_sha256: placeholder.clone(),
                    offset: cursor,
                    stored_size: input.size,
                    codec: "stored".into(),
                },
            );
            cursor += input.size;
        }
    }
    let total: u64 = groups.iter().map(|group| group.pack_bytes).sum();
    ensure!(total <= MAX_TOTAL_BYTES, "repack physical total bound");
    let projected = relocated_catalog(source, &projected_locators, projected_packs)?;
    let projected_bytes = serde_json::to_vec(&projected)?.len();
    ensure!(
        projected_bytes <= MAX_CATALOG_BYTES,
        "repack catalog byte bound"
    );
    let summary = RepackPreflight {
        schema: "mir2.native.resource-repack-preflight.v1",
        source_catalog_sha256: source.catalog_sha.clone(),
        plan_sha256: sha256(bytes),
        target_pack_bytes: plan.target_pack_bytes,
        resource_count: source.catalog.resources.len(),
        logical_bytes,
        unique_payload_count: owners.len(),
        unique_payload_bytes: groups.iter().map(|group| group.payload_bytes).sum(),
        projected_pack_count: chunks.len(),
        projected_pack_bytes: total,
        projected_catalog_bytes: projected_bytes,
        logical_inventory_sha256: inventory_sha(&source.catalog)?,
        groups,
    };
    Ok(Prepared { summary, chunks })
}

/// No output filesystem operations. The source still has the original full
/// PackStore validation and lifetime leases; this is not a metadata-only store.
pub fn preflight(source: &PackStore, plan_bytes: &[u8]) -> Result<RepackPreflight> {
    Ok(prepare(source, plan_bytes)?.summary)
}

fn directory_identity(file: &File) -> Result<(u64, u64)> {
    handle_check(file, true)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = file.metadata()?;
        Ok((meta.dev(), meta.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut info) } != 0,
            "directory identity failed"
        );
        Ok((
            u64::from(info.dwVolumeSerialNumber),
            (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        ))
    }
}

/// Output must not exist and its parent must already exist. No cleanup or retry
/// removes partial objects: any failure retains the entire fresh output for
/// inspection, and another invocation must choose a different fresh directory.
pub fn repack(source: &PackStore, plan_bytes: &[u8], fresh_output: &Path) -> Result<RepackReceipt> {
    let Prepared { summary, chunks } = prepare(source, plan_bytes)?;
    absolute_plain_path(fresh_output)?;
    let parent_leases = directories(fresh_output.parent().context("repack output parent")?)?;
    let source_id = directory_identity(
        source
            ._directory_leases
            .last()
            .context("source directory lease")?,
    )?;
    for parent in &parent_leases {
        ensure!(
            directory_identity(parent)? != source_id,
            "repack output inside source object directory"
        );
    }
    // create_dir, rather than create_dir_all, refuses even an existing empty
    // directory. Every child and control file is subsequently create-only too.
    fs::create_dir(fresh_output)?;
    let _output_leases = directories(fresh_output)?;
    let objects = fresh_output.join("objects");
    fs::create_dir(&objects)?;
    let _objects_leases = directories(&objects)?;
    let objects = objects.join("packs");
    fs::create_dir(&objects)?;
    let _pack_leases = directories(&objects)?;
    let mut locators = BTreeMap::new();
    let mut packs = Vec::new();
    let mut group_objects: Vec<_> = summary
        .groups
        .iter()
        .map(|group| GroupObjects {
            name: group.name.clone(),
            packs: Vec::new(),
        })
        .collect();
    let mut verified_unique_payloads = 0;
    for chunk in chunks {
        let (bytes, entries) = pack_bytes_with(&chunk.inputs, |input, target| {
            let bytes = source.read(&input.path)?;
            ensure!(
                bytes.len() == target.len(),
                "repack source byte count changed"
            );
            target.copy_from_slice(&bytes);
            Ok(())
        })?;
        let (object, created) = create_object(&objects, &bytes)?;
        ensure!(created, "repack refuses any existing output object");
        for (sha, extent) in entries {
            locators.insert(
                sha.clone(),
                ResourceRef {
                    sha256: sha,
                    size: extent.size,
                    pack_sha256: object.sha256.clone(),
                    offset: extent.offset,
                    stored_size: extent.size,
                    codec: "stored".into(),
                },
            );
            verified_unique_payloads += 1;
        }
        group_objects[chunk.group].packs.push(object.clone());
        packs.push(object);
    }
    ensure!(
        verified_unique_payloads == summary.unique_payload_count,
        "repack verified payload count differs"
    );
    let catalog = relocated_catalog(source, &locators, packs)?;
    ensure!(
        inventory_sha(&catalog)? == summary.logical_inventory_sha256,
        "repack logical inventory differs"
    );
    let catalog_bytes = serde_json::to_vec(&catalog)?;
    ensure!(
        catalog_bytes.len() == summary.projected_catalog_bytes,
        "repack catalog preflight differs"
    );
    let catalog_sha = sha256(&catalog_bytes);
    let _verified = PackStore::open(&objects, &catalog_bytes, &catalog_sha)?;
    write_catalog_new(
        &fresh_output.join("catalog.json"),
        &catalog_bytes,
        &catalog_sha,
    )?;
    let receipt = RepackReceipt {
        schema: RECEIPT_SCHEMA,
        preflight: summary,
        catalog_sha256: catalog_sha,
        catalog_bytes: catalog_bytes.len(),
        groups: group_objects,
        verified_unique_payloads,
        full_logical_equivalence_verified: true,
        output_pack_store_verified: true,
        bootstrap_authorized: false,
    };
    let receipt_bytes = serde_json::to_vec(&receipt)?;
    write_bytes_new(
        &fresh_output.join("REPACK-RECEIPT.json"),
        &receipt_bytes,
        &sha256(&receipt_bytes),
    )?;
    Ok(receipt)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    let file = open_file(path)?;
    let size = file.metadata()?.len();
    ensure!(
        size <= MAX_CATALOG_BYTES as u64,
        "repack input file byte bound"
    );
    let mut bytes = vec![0; usize::try_from(size)?];
    read_at(&file, 0, &mut bytes)?;
    handle_check(&file, false)?;
    ensure!(file.metadata()?.len() == size, "repack input size changed");
    Ok(bytes)
}

/// Safe CLI/file adapter. Both JSON inputs reject symlinks/reparse points and
/// hardlinks through the same handle policy as pack sources. The caller, not
/// this unsigned tool, must authenticate trusted_catalog_sha in its release flow.
pub fn repack_files(
    old_catalog: &Path,
    trusted_catalog_sha: &str,
    old_objects: &Path,
    plan: &Path,
    fresh_output: &Path,
) -> Result<RepackReceipt> {
    let catalog_bytes = read_bounded(old_catalog)?;
    let store = PackStore::open(old_objects, &catalog_bytes, trusted_catalog_sha)?;
    repack(&store, &read_bounded(plan)?, fresh_output)
}

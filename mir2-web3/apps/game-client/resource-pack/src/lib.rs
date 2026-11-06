//! Immutable, randomly readable native resource objects. CMS authentication and
//! release selection belong to the caller; `expected_catalog_sha` must originate
//! in an authenticated generation manifest. No network or game directory access.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path},
};

pub const CATALOG_SCHEMA: &str = "mir2.native.resource-catalog.v1";
pub const MAX_RESOURCES: usize = 300_000;
pub const MAX_PACKS: usize = 256;
pub const MAX_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_CATALOG_BYTES: usize = 128 * 1024 * 1024;
pub const TARGET_PACK_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_PACK_BYTES: u64 = 64 * 1024 * 1024;
const HEADER: u64 = 32;
const RECORD: u64 = 64;
const MAGIC: &[u8; 8] = b"M2PACK01";

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:X}", Sha256::digest(bytes))
}
fn digest(value: &str) -> Result<String> {
    ensure!(
        value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid SHA256"
    );
    Ok(value.to_ascii_uppercase())
}
fn raw_digest(value: &str) -> Result<[u8; 32]> {
    let value = digest(value)?;
    let mut bytes = [0; 32];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
    }
    Ok(bytes)
}
fn digest_text(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// Logical names are portable Windows relative paths. They never become output
/// extraction paths. Code and script extensions are forbidden in resource packs.
pub fn validate_logical_path(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 230 && value.is_ascii(),
        "invalid resource path length/encoding"
    );
    ensure!(
        !value.bytes().any(|b| b < 32 || b"\\:\"<>|?*".contains(&b)),
        "unsafe resource path"
    );
    for segment in value.split('/') {
        ensure!(
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && !segment.ends_with(['.', ' ']),
            "unsafe resource segment"
        );
        let stem = segment.split('.').next().unwrap().to_ascii_uppercase();
        ensure!(
            !["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && (b'1'..=b'9').contains(&stem.as_bytes()[3])),
            "reserved resource path"
        );
        for extension in segment.to_ascii_lowercase().split('.').skip(1) {
            ensure!(
                ![
                    "exe", "dll", "com", "scr", "cpl", "msi", "msp", "bat", "cmd", "ps1", "psm1",
                    "psd1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "reg", "lnk", "chm",
                    "jar", "py", "pyw", "sh", "bash", "zsh", "fish"
                ]
                .contains(&extension),
                "executable/script resource rejected"
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResourceRef {
    pub sha256: String,
    pub size: u64,
    pub pack_sha256: String,
    pub offset: u64,
    pub stored_size: u64,
    pub codec: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PackObject {
    pub sha256: String,
    pub size: u64,
}
impl PackObject {
    pub fn file_name(&self) -> String {
        format!("{}.m2p", self.sha256.to_ascii_lowercase())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Catalog {
    pub schema: String,
    #[serde(deserialize_with = "unique_resources")]
    pub resources: BTreeMap<String, ResourceRef>,
    #[serde(deserialize_with = "bounded_packs")]
    pub packs: Vec<PackObject>,
}
fn bounded_packs<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<PackObject>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<PackObject>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("bounded pack sequence")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = vec![];
            while let Some(pack) = seq.next_element()? {
                if result.len() >= MAX_PACKS {
                    return Err(serde::de::Error::custom("pack count bound"));
                }
                result.push(pack);
            }
            Ok(result)
        }
    }
    deserializer.deserialize_seq(Bounded)
}
fn unique_resources<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<BTreeMap<String, ResourceRef>, D::Error> {
    struct Unique;
    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = BTreeMap<String, ResourceRef>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("unique resource map")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, ResourceRef>()? {
                if result.len() >= MAX_RESOURCES || result.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate/oversized resource map"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Unique)
}
impl Catalog {
    /// This validates catalog structure and its exact object-reference closure.
    /// Use `PackStore::open` to validate actual object bytes and pack indexes.
    pub fn parse(bytes: &[u8], expected_sha: &str) -> Result<Self> {
        ensure!(bytes.len() <= MAX_CATALOG_BYTES, "catalog byte bound");
        ensure!(
            sha256(bytes) == digest(expected_sha)?,
            "catalog SHA mismatch"
        );
        let mut catalog: Self = serde_json::from_slice(bytes)?;
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn objects(&self) -> &[PackObject] {
        &self.packs
    }
    fn validate(&mut self) -> Result<()> {
        ensure!(self.schema == CATALOG_SCHEMA, "unsupported catalog schema");
        ensure!(
            self.resources.len() <= MAX_RESOURCES && self.packs.len() <= MAX_PACKS,
            "catalog count bound"
        );
        let mut objects = BTreeMap::new();
        let mut physical = 0u64;
        for pack in &mut self.packs {
            pack.sha256 = digest(&pack.sha256)?;
            ensure!(
                (HEADER..=MAX_PACK_BYTES).contains(&pack.size),
                "pack size bound"
            );
            ensure!(
                objects.insert(pack.sha256.clone(), pack.size).is_none(),
                "duplicate pack digest"
            );
            physical = physical
                .checked_add(pack.size)
                .context("pack total overflow")?;
        }
        ensure!(physical <= MAX_TOTAL_BYTES, "physical pack total bound");
        let mut used = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut contents = BTreeMap::new();
        let mut logical = 0u64;
        for (path, entry) in &mut self.resources {
            validate_logical_path(path)?;
            ensure!(
                names.insert(path.to_ascii_lowercase()),
                "case-colliding resource path"
            );
            entry.sha256 = digest(&entry.sha256)?;
            entry.pack_sha256 = digest(&entry.pack_sha256)?;
            let size = *objects
                .get(&entry.pack_sha256)
                .context("resource references undeclared pack")?;
            ensure!(
                entry.codec == "stored" && entry.stored_size == entry.size,
                "unsupported resource codec/length"
            );
            ensure!(
                entry.offset >= HEADER
                    && entry
                        .offset
                        .checked_add(entry.stored_size)
                        .is_some_and(|end| end <= size),
                "resource extent outside pack"
            );
            if let Some(previous) = contents.insert(entry.sha256.clone(), entry.clone()) {
                ensure!(
                    previous == *entry,
                    "same content digest has inconsistent locator"
                );
            }
            used.insert(entry.pack_sha256.clone());
            logical = logical
                .checked_add(entry.size)
                .context("logical total overflow")?;
        }
        ensure!(logical <= MAX_TOTAL_BYTES, "logical resource total bound");
        ensure!(
            used == objects.keys().cloned().collect(),
            "pack set differs from exact resource reference closure"
        );
        // A file cannot also be a directory, including Windows case aliases.
        for path in self.resources.keys() {
            for (index, _) in path.match_indices('/') {
                ensure!(
                    !names.contains(&path[..index].to_ascii_lowercase()),
                    "resource file/directory collision"
                );
            }
        }
        self.packs.sort_by(|a, b| a.sha256.cmp(&b.sha256));
        Ok(())
    }
}

fn absolute_plain_path(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute()
            && !path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir)),
        "absolute normalized filesystem path required"
    );
    #[cfg(windows)]
    {
        use std::path::Prefix;
        ensure!(
            matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))),
            "only local drive paths supported"
        );
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) => {
                ensure!(
                    !meta.file_type().is_symlink(),
                    "filesystem symlink rejected"
                );
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(
                        meta.file_attributes() & 0x400 == 0,
                        "filesystem reparse point rejected"
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
fn handle_check(file: &File, directory: bool) -> Result<()> {
    let meta = file.metadata()?;
    ensure!(
        if directory {
            meta.is_dir()
        } else {
            meta.is_file()
        },
        "unexpected handle type"
    );
    #[cfg(unix)]
    if !directory {
        use std::os::unix::fs::MetadataExt;
        ensure!(meta.nlink() == 1, "hardlinked file handle rejected");
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
            "file handle information failed"
        );
        ensure!(
            info.dwFileAttributes & 0x400 == 0,
            "reparse file handle rejected"
        );
        ensure!(
            directory || info.nNumberOfLinks == 1,
            "hardlinked file handle rejected"
        );
    }
    Ok(())
}
fn directories(path: &Path) -> Result<Vec<File>> {
    absolute_plain_path(path)?;
    let mut ancestors: Vec<_> = path.ancestors().collect();
    ancestors.reverse();
    let mut result = vec![];
    for ancestor in ancestors {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
                FILE_SHARE_WRITE,
            };
            // Allow directory contents to change, but deny directory rename/delete.
            options
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
        }
        let file = options.open(ancestor)?;
        handle_check(&file, true)?;
        absolute_plain_path(ancestor)?;
        result.push(file);
    }
    Ok(result)
}
fn open_file(path: &Path) -> Result<File> {
    absolute_plain_path(path)?;
    let _parent_leases = directories(path.parent().context("file parent")?)?;
    let before = fs::symlink_metadata(path)?;
    ensure!(before.is_file(), "plain source file required");
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        };
        options
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    handle_check(&file, false)?;
    absolute_plain_path(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = file.metadata()?;
        let after = fs::symlink_metadata(path)?;
        ensure!(
            actual.dev() == before.dev()
                && actual.ino() == before.ino()
                && actual.dev() == after.dev()
                && actual.ino() == after.ino(),
            "file changed during open"
        );
    }
    Ok(file)
}
fn read_at(file: &File, mut offset: u64, mut buffer: &mut [u8]) -> Result<()> {
    while !buffer.is_empty() {
        #[cfg(windows)]
        let count = {
            use std::os::windows::fs::FileExt;
            file.seek_read(buffer, offset)?
        };
        #[cfg(unix)]
        let count = {
            use std::os::unix::fs::FileExt;
            file.read_at(buffer, offset)?
        };
        ensure!(count != 0, "truncated pack/source");
        offset = offset
            .checked_add(count as u64)
            .context("read offset overflow")?;
        buffer = &mut buffer[count..];
    }
    Ok(())
}
fn hash_file(file: &File, size: u64) -> Result<String> {
    ensure!(file.metadata()?.len() == size, "file size mismatch");
    let mut state = Sha256::new();
    let mut buffer = [0; 65_536];
    let mut offset = 0;
    while offset < size {
        let count = (size - offset).min(buffer.len() as u64) as usize;
        read_at(file, offset, &mut buffer[..count])?;
        state.update(&buffer[..count]);
        offset += count as u64;
    }
    handle_check(file, false)?;
    ensure!(
        file.metadata()?.len() == size,
        "file changed during hashing"
    );
    Ok(format!("{:X}", state.finalize()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Extent {
    offset: u64,
    size: u64,
}
#[derive(Debug)]
struct PackFile {
    file: File,
    entries: BTreeMap<String, Extent>,
}
fn read_pack(root: &Path, object: &PackObject) -> Result<PackFile> {
    let file = open_file(&root.join(object.file_name()))?;
    ensure!(
        hash_file(&file, object.size)? == object.sha256,
        "pack SHA mismatch"
    );
    let mut header = [0; HEADER as usize];
    read_at(&file, 0, &mut header)?;
    ensure!(
        &header[..8] == MAGIC && u32::from_le_bytes(header[8..12].try_into()?) == 1,
        "pack magic/version mismatch"
    );
    let count = u32::from_le_bytes(header[12..16].try_into()?) as usize;
    let index_bytes = u64::from_le_bytes(header[16..24].try_into()?);
    let payload_bytes = u64::from_le_bytes(header[24..32].try_into()?);
    ensure!(
        count > 0 && count <= MAX_RESOURCES && index_bytes == count as u64 * RECORD,
        "pack index count/size bound"
    );
    let data_start = HEADER
        .checked_add(index_bytes)
        .context("pack header overflow")?;
    ensure!(
        data_start.checked_add(payload_bytes) == Some(object.size),
        "pack declared size mismatch"
    );
    let mut entries = BTreeMap::new();
    let mut previous = String::new();
    let mut cursor = data_start;
    // One bounded index read, rather than one filesystem operation per resource.
    let mut index_data = vec![0; usize::try_from(index_bytes)?];
    read_at(&file, HEADER, &mut index_data)?;
    for record in index_data.chunks_exact(RECORD as usize) {
        let sha = digest_text(&record[..32]);
        let offset = u64::from_le_bytes(record[32..40].try_into()?);
        let size = u64::from_le_bytes(record[40..48].try_into()?);
        let stored = u64::from_le_bytes(record[48..56].try_into()?);
        ensure!(
            sha > previous && record[56..].iter().all(|byte| *byte == 0),
            "duplicate/unsorted index or unsupported codec/reserved bits"
        );
        ensure!(
            offset == cursor && size == stored,
            "pack extent overlap/gap/codec length"
        );
        cursor = cursor.checked_add(size).context("pack extent overflow")?;
        ensure!(cursor <= object.size, "pack extent outside object");
        entries.insert(sha.clone(), Extent { offset, size });
        previous = sha;
    }
    ensure!(cursor == object.size, "trailing/unindexed pack bytes");
    Ok(PackFile { file, entries })
}

/// On Windows every object handle denies write/delete sharing until this store
/// is dropped. Directory handles also prevent ancestor replacement. Random reads
/// use positional I/O and share no mutable cursor or global lock. Unix callers
/// additionally need an immutable object directory policy; each read hashes bytes.
#[derive(Debug)]
pub struct PackStore {
    catalog: Catalog,
    catalog_sha: String,
    files: BTreeMap<String, PackFile>,
    _directory_leases: Vec<File>,
}
impl PackStore {
    pub fn open(
        objects_root: impl AsRef<Path>,
        catalog_bytes: &[u8],
        expected_catalog_sha: &str,
    ) -> Result<Self> {
        let root = objects_root.as_ref();
        let leases = directories(root)?;
        let catalog = Catalog::parse(catalog_bytes, expected_catalog_sha)?;
        let mut files = BTreeMap::new();
        for object in &catalog.packs {
            files.insert(object.sha256.clone(), read_pack(root, object)?);
        }
        for entry in catalog.resources.values() {
            let extent = files[&entry.pack_sha256]
                .entries
                .get(&entry.sha256)
                .context("resource absent from pack index")?;
            ensure!(
                extent.offset == entry.offset && extent.size == entry.size,
                "catalog/index locator mismatch"
            );
        }
        Ok(Self {
            catalog,
            catalog_sha: sha256(catalog_bytes),
            files,
            _directory_leases: leases,
        })
    }
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub fn catalog_sha256(&self) -> &str {
        &self.catalog_sha
    }
    pub fn exists(&self, path: &str) -> bool {
        self.catalog.resources.contains_key(path)
    }
    pub fn is_directory(&self, path: &str) -> bool {
        let Ok(prefix) = directory_prefix(path) else {
            return false;
        };
        prefix.is_empty()
            || self
                .catalog
                .resources
                .range(prefix.clone()..)
                .next()
                .is_some_and(|(key, _)| key.starts_with(&prefix))
    }
    /// Recursively returns sorted full logical paths beneath a directory prefix.
    /// Empty prefix denotes root; a single trailing slash is optional.
    pub fn list_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        let prefix = directory_prefix(prefix)?;
        Ok(self
            .catalog
            .resources
            .range(prefix.clone()..)
            .take_while(|(path, _)| path.starts_with(&prefix))
            .map(|(path, _)| path.clone())
            .collect())
    }
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        validate_logical_path(path)?;
        let entry = self
            .catalog
            .resources
            .get(path)
            .context("resource absent from generation catalog")?;
        let object = &self.files[&entry.pack_sha256];
        handle_check(&object.file, false)?;
        let mut bytes = vec![0; usize::try_from(entry.size)?];
        read_at(&object.file, entry.offset, &mut bytes)?;
        ensure!(
            sha256(&bytes) == entry.sha256,
            "resource content SHA mismatch"
        );
        handle_check(&object.file, false)?;
        Ok(bytes)
    }
}
fn directory_prefix(path: &str) -> Result<String> {
    if path.is_empty() {
        return Ok(String::new());
    }
    let path = path.strip_suffix('/').unwrap_or(path);
    validate_logical_path(path)?;
    Ok(format!("{path}/"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceInput {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Debug, Clone, Copy)]
pub struct BuildOptions {
    pub target_pack_bytes: u64,
}
impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            target_pack_bytes: TARGET_PACK_BYTES,
        }
    }
}
#[derive(Debug)]
pub struct BuildOutput {
    pub catalog: Catalog,
    pub catalog_bytes: Vec<u8>,
    pub catalog_sha256: String,
    pub new_packs: Vec<PackObject>,
    pub reused_resources: usize,
}

/// Persist an already constructed catalog to a fresh local file. This does not
/// sign it or activate a generation. Parent directory leases and the actual
/// create-new handle use the same no-reparse/no-hardlink policy as pack objects.
pub fn write_catalog_new(path: &Path, bytes: &[u8], expected_sha: &str) -> Result<()> {
    Catalog::parse(bytes, expected_sha)?;
    absolute_plain_path(path)?;
    let _leases = directories(path.parent().context("catalog parent")?)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    handle_check(&file, false)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    ensure!(
        hash_file(&file, bytes.len() as u64)? == digest(expected_sha)?,
        "catalog write readback mismatch"
    );
    Ok(())
}
fn create_object(root: &Path, bytes: &[u8]) -> Result<(PackObject, bool)> {
    ensure!(bytes.len() as u64 <= MAX_PACK_BYTES, "new pack byte bound");
    let object = PackObject {
        sha256: sha256(bytes),
        size: bytes.len() as u64,
    };
    let path = root.join(object.file_name());
    absolute_plain_path(&path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    match options.open(&path) {
        Ok(mut file) => {
            handle_check(&file, false)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            handle_check(&file, false)?;
            drop(file);
            read_pack(root, &object)?;
            Ok((object, true))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            read_pack(root, &object)?;
            Ok((object, false))
        }
        Err(error) => Err(error.into()),
    }
}
fn pack_bytes(
    source_root: &Path,
    group: &[ResourceInput],
) -> Result<(Vec<u8>, BTreeMap<String, Extent>)> {
    let data_start = HEADER + group.len() as u64 * RECORD;
    let payload = group.iter().try_fold(0u64, |sum, input| {
        sum.checked_add(input.size).context("pack payload overflow")
    })?;
    let total = data_start
        .checked_add(payload)
        .context("pack size overflow")?;
    ensure!(total <= MAX_PACK_BYTES, "resource cannot fit bounded pack");
    let mut bytes = Vec::with_capacity(total as usize);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&(group.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(group.len() as u64 * RECORD).to_le_bytes());
    bytes.extend_from_slice(&payload.to_le_bytes());
    let mut cursor = data_start;
    let mut entries = BTreeMap::new();
    for input in group {
        bytes.extend_from_slice(&raw_digest(&input.sha256)?);
        bytes.extend_from_slice(&cursor.to_le_bytes());
        bytes.extend_from_slice(&input.size.to_le_bytes());
        bytes.extend_from_slice(&input.size.to_le_bytes());
        bytes.extend_from_slice(&[0; 8]); // codec 0=stored, seven reserved zero bytes
        entries.insert(
            input.sha256.clone(),
            Extent {
                offset: cursor,
                size: input.size,
            },
        );
        cursor += input.size;
    }
    for input in group {
        let file = open_file(&source_root.join(&input.path))?;
        ensure!(
            hash_file(&file, input.size)? == input.sha256,
            "source resource SHA mismatch"
        );
        let start = bytes.len();
        bytes.resize(start + input.size as usize, 0);
        read_at(&file, 0, &mut bytes[start..])?;
        ensure!(
            sha256(&bytes[start..]) == input.sha256,
            "source changed during pack construction"
        );
        handle_check(&file, false)?;
    }
    Ok((bytes, entries))
}

/// Builds only new content; previous references retain their original pack and
/// extent. The caller supplies the authoritative full logical resource inventory.
/// Reused objects must already exist in the same destination object store.
/// Failed create-only output is retained and is never overwritten on retry.
pub fn build(
    source_root: &Path,
    objects_root: &Path,
    resources: &[ResourceInput],
    previous: Option<&PackStore>,
    options: BuildOptions,
) -> Result<BuildOutput> {
    ensure!(
        (HEADER + RECORD..=MAX_PACK_BYTES).contains(&options.target_pack_bytes),
        "pack target byte bound"
    );
    ensure!(resources.len() <= MAX_RESOURCES, "resource count bound");
    let _source_leases = directories(source_root)?;
    absolute_plain_path(objects_root)?;
    // Hold the existing parent before creating the flat object store.
    let existing = objects_root
        .ancestors()
        .find(|path| path.exists())
        .context("objects parent")?;
    let _parent_leases = directories(existing)?;
    fs::create_dir_all(objects_root)?;
    let _object_leases = directories(objects_root)?;
    let mut inputs = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for input in resources {
        validate_logical_path(&input.path)?;
        ensure!(
            names.insert(input.path.to_ascii_lowercase()),
            "duplicate/case-colliding resource input"
        );
        ensure!(
            input
                .size
                .checked_add(HEADER + RECORD)
                .is_some_and(|size| size <= MAX_PACK_BYTES),
            "resource exceeds single pack hard bound"
        );
        total = total
            .checked_add(input.size)
            .context("resource total overflow")?;
        let mut input = input.clone();
        input.sha256 = digest(&input.sha256)?;
        inputs.insert(input.path.clone(), input);
    }
    ensure!(total <= MAX_TOTAL_BYTES, "resource total bound");
    for path in inputs.keys() {
        for (index, _) in path.match_indices('/') {
            ensure!(
                !names.contains(&path[..index].to_ascii_lowercase()),
                "resource file/directory collision"
            );
        }
    }
    let mut locators = BTreeMap::new();
    let mut objects = BTreeMap::new();
    if let Some(previous) = previous {
        for entry in previous.catalog.resources.values() {
            locators.insert(entry.sha256.clone(), entry.clone());
        }
        for object in &previous.catalog.packs {
            objects.insert(object.sha256.clone(), object.clone());
        }
    }
    let mut fresh = BTreeMap::new();
    let mut reused_resources = 0;
    for input in inputs.values() {
        if let Some(entry) = locators.get(&input.sha256) {
            ensure!(entry.size == input.size, "content digest size conflict");
            reused_resources += 1;
        } else if let Some(old) = fresh.insert(input.sha256.clone(), input.clone()) {
            ensure!(old.size == input.size, "new content digest size conflict");
        }
    }
    let mut groups = Vec::<Vec<ResourceInput>>::new();
    let mut group = vec![];
    let mut size = HEADER;
    for input in fresh.into_values() {
        let cost = RECORD + input.size;
        if !group.is_empty() && size + cost > options.target_pack_bytes {
            groups.push(std::mem::take(&mut group));
            size = HEADER;
        }
        group.push(input);
        size += cost;
    }
    if !group.is_empty() {
        groups.push(group);
    }
    let reused_pack_ids = inputs
        .values()
        .filter_map(|input| {
            locators
                .get(&input.sha256)
                .map(|entry| entry.pack_sha256.clone())
        })
        .collect::<BTreeSet<_>>();
    ensure!(
        groups.len() + reused_pack_ids.len() <= MAX_PACKS,
        "required pack count bound"
    );
    let reused_bytes = reused_pack_ids.iter().try_fold(0u64, |sum, sha| {
        sum.checked_add(objects[sha].size)
            .context("reused pack total overflow")
    })?;
    let new_bytes = groups.iter().try_fold(0u64, |sum, group| {
        let size = group.iter().try_fold(HEADER, |size, input| {
            size.checked_add(RECORD + input.size)
                .context("new pack size overflow")
        })?;
        sum.checked_add(size).context("new pack total overflow")
    })?;
    ensure!(
        reused_bytes
            .checked_add(new_bytes)
            .is_some_and(|size| size <= MAX_TOTAL_BYTES),
        "required physical pack total bound"
    );
    let mut new_packs = vec![];
    for group in groups {
        let (bytes, entries) = pack_bytes(source_root, &group)?;
        let (object, created) = create_object(objects_root, &bytes)?;
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
        }
        if created {
            new_packs.push(object.clone());
        }
        objects.insert(object.sha256.clone(), object);
    }
    let mut catalog = Catalog {
        schema: CATALOG_SCHEMA.into(),
        resources: BTreeMap::new(),
        packs: vec![],
    };
    let mut required = BTreeSet::new();
    for (path, input) in inputs {
        let entry = locators
            .get(&input.sha256)
            .context("builder content locator absent")?
            .clone();
        required.insert(entry.pack_sha256.clone());
        catalog.resources.insert(path, entry);
    }
    catalog.packs = required
        .into_iter()
        .map(|sha| {
            objects
                .remove(&sha)
                .context("builder object descriptor absent")
        })
        .collect::<Result<_>>()?;
    catalog.validate()?;
    let catalog_bytes = serde_json::to_vec(&catalog)?;
    ensure!(
        catalog_bytes.len() <= MAX_CATALOG_BYTES,
        "built catalog byte bound"
    );
    let catalog_sha256 = sha256(&catalog_bytes);
    // This also verifies reused destination objects and the complete index closure.
    PackStore::open(objects_root, &catalog_bytes, &catalog_sha256)?;
    Ok(BuildOutput {
        catalog,
        catalog_bytes,
        catalog_sha256,
        new_packs,
        reused_resources,
    })
}

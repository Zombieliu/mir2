//! Durable, server-global identity allocation for Crystal `UserItem` values.
//!
//! The file authority is intentionally independent from account and world
//! snapshots. Each successful allocation advances a dedicated sidecar before
//! the UID is returned, so a crash can create a gap but cannot return an
//! uncommitted value. Callers must use a local filesystem whose locking,
//! atomic-replace, and durability guarantees match the operating system APIs
//! used here; shared/network filesystems are outside this authority's contract.
//!
//! No allocator can detect an administrator rolling back *all* historical
//! account data, world checkpoints, and UID authority state to the same older
//! generation. Operations must therefore preserve and restore the UID sidecar
//! together with the rest of the authoritative deployment state, and disaster
//! recovery must never lower its recorded high-water mark.
//!
//! UserItemUid spans the full u64 range. Every JavaScript or JSON boundary must
//! encode it as a canonical decimal string, never as a JSON number, because
//! JavaScript numbers cannot exactly represent all values above 2^53.
//!
//! The authority directory is a trusted, dedicated local directory. This module
//! binds its directory and lock-file identities and checks them around every
//! critical phase. Replacing either path while the process is online is an
//! unsupported same-user/administrator operation and is detected fail-closed
//! whenever observed. A hostile actor able to replace path components in the
//! tiny interval between an identity check and a path-based OS operation remains
//! outside this file authority's threat model; production ACLs must prevent it.
//! Likewise, replacing the state sidecar with an older but structurally valid
//! copy while leaving the bound directory and lock object unchanged is not
//! authenticated by Stage A. Such online replacement is unsupported; callers
//! must use a dedicated ACL-protected directory and raise the floor from scanned
//! authoritative history during migration or disaster recovery.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::num::NonZeroU64;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const FILE_STATE_SCHEMA_VERSION: u16 = 1;
const MAX_STATE_BYTES: u64 = 4 * 1024;
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub const USER_ITEM_UID_MIN: u64 = 1;
pub const USER_ITEM_UID_MAX: u64 = u64::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserItemUid(NonZeroU64);

impl UserItemUid {
    pub fn new(value: u64) -> Result<Self, UserItemUidError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(UserItemUidError::InvalidUid { value })
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl fmt::Display for UserItemUid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UserItemUidReason {
    CharacterStartItem,
    PlayerPartialDrop,
    MonsterDrop,
    QuestReward,
    QuestCarryItem,
    NpcScriptGiveItem,
    NpcTradePurchase,
    GameShopGrant,
    SystemMailGrant,
    Mining,
    Fishing,
    Crafting,
    OnchainMining,
    GmCreate,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum UserItemUidError {
    InvalidUid {
        value: u64,
    },
    InvalidPath {
        path: PathBuf,
        reason: String,
    },
    UnsafePath {
        path: PathBuf,
        reason: String,
    },
    MissingState {
        path: PathBuf,
    },
    AlreadyInitialized {
        path: PathBuf,
    },
    LockFailed {
        path: PathBuf,
        source: io::Error,
    },
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    CorruptState {
        path: PathBuf,
        reason: String,
    },
    PersistenceFailed {
        path: PathBuf,
        source: io::Error,
    },
    CommitOutcomeUnknown {
        path: PathBuf,
        reason: String,
    },
    AuthorityIdentityChanged {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    Exhausted {
        issued_through: u64,
        generation: u64,
    },
}

impl fmt::Display for UserItemUidError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUid { value } => write!(formatter, "invalid user item UID {value}"),
            Self::InvalidPath { path, reason } => {
                write!(
                    formatter,
                    "invalid UID authority path {}: {reason}",
                    path.display()
                )
            }
            Self::UnsafePath { path, reason } => {
                write!(
                    formatter,
                    "unsafe UID authority path {}: {reason}",
                    path.display()
                )
            }
            Self::MissingState { path } => {
                write!(
                    formatter,
                    "UID authority state is missing: {}",
                    path.display()
                )
            }
            Self::AlreadyInitialized { path } => write!(
                formatter,
                "UID authority state already exists: {}",
                path.display()
            ),
            Self::LockFailed { path, source } => write!(
                formatter,
                "failed to lock UID authority {}: {source}",
                path.display()
            ),
            Self::ReadFailed { path, source } => write!(
                formatter,
                "failed to read UID authority {}: {source}",
                path.display()
            ),
            Self::CorruptState { path, reason } => write!(
                formatter,
                "corrupt UID authority state {}: {reason}",
                path.display()
            ),
            Self::PersistenceFailed { path, source } => write!(
                formatter,
                "failed to persist UID authority {}: {source}",
                path.display()
            ),
            Self::CommitOutcomeUnknown { path, reason } => write!(
                formatter,
                "UID authority commit outcome is unknown for {}: {reason}",
                path.display()
            ),
            Self::AuthorityIdentityChanged {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "UID authority identity changed at {}: expected {expected}, actual {actual}",
                path.display()
            ),
            Self::Exhausted {
                issued_through,
                generation,
            } => write!(
                formatter,
                "UID authority exhausted at issuedThrough={issued_through}, generation={generation}"
            ),
        }
    }
}

impl Error for UserItemUidError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LockFailed { source, .. }
            | Self::ReadFailed { source, .. }
            | Self::PersistenceFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub trait UserItemUidStore: Send + Sync {
    fn issue_one(&self, reason: UserItemUidReason) -> Result<UserItemUid, UserItemUidError>;

    fn issued_through(&self) -> Result<u64, UserItemUidError>;

    /// Raises the durable high-water mark for migration/recovery and never
    /// lowers it. A lower or equal floor is a locked, persistence-free no-op.
    fn ensure_issued_through_at_least(&self, floor: u64) -> Result<u64, UserItemUidError>;
}

#[derive(Clone)]
pub struct UserItemUidAllocator {
    inner: Arc<dyn UserItemUidStore>,
}

impl fmt::Debug for UserItemUidAllocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UserItemUidAllocator")
            .finish_non_exhaustive()
    }
}

impl UserItemUidAllocator {
    pub fn initialize_file(
        state_path: impl AsRef<Path>,
        initial_issued_through: u64,
    ) -> Result<Self, UserItemUidError> {
        Ok(Self {
            inner: Arc::new(FileUserItemUidAuthority::initialize_new(
                state_path,
                initial_issued_through,
            )?),
        })
    }

    pub fn open_file(state_path: impl AsRef<Path>) -> Result<Self, UserItemUidError> {
        Ok(Self {
            inner: Arc::new(FileUserItemUidAuthority::open_existing(state_path)?),
        })
    }

    pub fn issue(&self, reason: UserItemUidReason) -> Result<UserItemUid, UserItemUidError> {
        self.inner.issue_one(reason)
    }

    pub fn issued_through(&self) -> Result<u64, UserItemUidError> {
        self.inner.issued_through()
    }

    /// Raises the durable floor after scanning historical data or restoring
    /// checkpoints. This method never lowers the current high-water mark.
    pub fn ensure_issued_through_at_least(&self, floor: u64) -> Result<u64, UserItemUidError> {
        self.inner.ensure_issued_through_at_least(floor)
    }

    pub const fn is_durable(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StableFileIdentity {
    storage_domain: u64,
    object_id: [u8; 16],
}

impl fmt::Display for StableFileIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}:", self.storage_domain)?;
        for byte in self.object_id {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct WindowsFileId128 {
    identifier: [u8; 16],
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct WindowsFileIdInfo {
    volume_serial_number: u64,
    file_id: WindowsFileId128,
}

// WinBase.h FILE_INFO_BY_HANDLE_CLASS::FileIdInfo has the official value 18 (0x12).
#[cfg(windows)]
const WINDOWS_FILE_ID_INFO_CLASS: i32 = 18;

// FILE_ID_128 is 16 bytes. FILE_ID_INFO is an 8-byte volume serial followed by
// that 128-bit identifier, with the Windows ABI's 8-byte aggregate alignment.
#[cfg(windows)]
const _: [(); 16] = [(); std::mem::size_of::<WindowsFileId128>()];
#[cfg(windows)]
const _: [(); 1] = [(); std::mem::align_of::<WindowsFileId128>()];
#[cfg(windows)]
const _: [(); 24] = [(); std::mem::size_of::<WindowsFileIdInfo>()];
#[cfg(windows)]
const _: [(); 8] = [(); std::mem::align_of::<WindowsFileIdInfo>()];
#[cfg(windows)]
const _: [(); 8] = [(); std::mem::offset_of!(WindowsFileIdInfo, file_id)];

/// A durable file-backed UID authority.
///
/// The strict sidecar JSON contains schemaVersion, issuedThrough (a canonical
/// decimal string), and generation. Use initialize_new exactly once for a new
/// deployment, explicitly supplying the already-issued high-water floor (zero
/// must also be explicit), and open_existing thereafter.
#[derive(Debug, Clone)]
pub struct FileUserItemUidAuthority {
    state_path: PathBuf,
    lock_path: PathBuf,
    directory_identity: StableFileIdentity,
    lock_identity: StableFileIdentity,
}

impl FileUserItemUidAuthority {
    pub fn initialize_new(
        state_path: impl AsRef<Path>,
        initial_issued_through: u64,
    ) -> Result<Self, UserItemUidError> {
        let (state_path, lock_path) = resolve_authority_paths(state_path.as_ref())?;
        let parent = state_path
            .parent()
            .expect("normalized state path has a parent");
        let directory_file = open_authority_directory(parent)?;
        let directory_identity = stable_file_identity(&directory_file, parent)?;
        let lock_file = open_or_create_initialization_lock(&lock_path)?;
        let lock_identity = stable_file_identity(&lock_file, &lock_path)?;
        acquire_exclusive_lock(&lock_file, &lock_path)?;
        let authority = Self {
            state_path,
            lock_path,
            directory_identity,
            lock_identity,
        };

        authority.verify_bindings(&lock_file)?;
        match secure_file_kind(&authority.state_path) {
            Ok(Some(_)) => {
                return Err(UserItemUidError::AlreadyInitialized {
                    path: authority.state_path.clone(),
                });
            }
            Ok(None) => {}
            Err(error) => return Err(error),
        }

        let initial = FileUidState {
            schema_version: FILE_STATE_SCHEMA_VERSION,
            issued_through: initial_issued_through.to_string(),
            generation: 1,
        };
        authority.persist_state(&initial, &lock_file)?;
        authority.verify_bindings(&lock_file)?;
        Ok(authority)
    }

    pub fn open_existing(state_path: impl AsRef<Path>) -> Result<Self, UserItemUidError> {
        let (state_path, lock_path) = resolve_authority_paths(state_path.as_ref())?;
        let parent = state_path
            .parent()
            .expect("normalized state path has a parent");
        let directory_file = open_authority_directory(parent)?;
        let directory_identity = stable_file_identity(&directory_file, parent)?;
        let lock_file = open_existing_regular_file(&lock_path, FilePurpose::Lock)?;
        let lock_identity = stable_file_identity(&lock_file, &lock_path)?;
        acquire_exclusive_lock(&lock_file, &lock_path)?;
        let authority = Self {
            state_path,
            lock_path,
            directory_identity,
            lock_identity,
        };
        authority.verify_bindings(&lock_file)?;
        let _ = authority.read_state()?;
        authority.verify_bindings(&lock_file)?;
        Ok(authority)
    }

    pub fn state_path(&self) -> &Path {
        &self.state_path
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    fn validate_parent_chain(&self) -> Result<(), UserItemUidError> {
        let parent = self
            .state_path
            .parent()
            .ok_or_else(|| UserItemUidError::InvalidPath {
                path: self.state_path.clone(),
                reason: "state path has no parent directory".to_owned(),
            })?;
        reject_link_or_reparse_chain(parent)
    }

    fn verify_directory_identity(&self) -> Result<(), UserItemUidError> {
        self.validate_parent_chain()?;
        let parent = self
            .state_path
            .parent()
            .expect("normalized state path has a parent");
        let directory_file = open_authority_directory(parent)?;
        let actual = stable_file_identity(&directory_file, parent)?;
        verify_identity(parent, self.directory_identity, actual)
    }

    fn verify_lock_handle_identity(&self, lock_file: &File) -> Result<(), UserItemUidError> {
        let actual = stable_file_identity(lock_file, &self.lock_path)?;
        verify_identity(&self.lock_path, self.lock_identity, actual)
    }

    fn verify_bindings(&self, lock_file: &File) -> Result<(), UserItemUidError> {
        self.verify_directory_identity()?;
        self.verify_lock_handle_identity(lock_file)?;
        let path_lock = open_existing_regular_file(&self.lock_path, FilePurpose::Lock)?;
        let actual = stable_file_identity(&path_lock, &self.lock_path)?;
        verify_identity(&self.lock_path, self.lock_identity, actual)
    }

    fn acquire_bound_lock(&self) -> Result<File, UserItemUidError> {
        self.verify_directory_identity()?;
        let lock_file = open_existing_regular_file(&self.lock_path, FilePurpose::Lock)?;
        self.verify_lock_handle_identity(&lock_file)?;
        acquire_exclusive_lock(&lock_file, &self.lock_path)?;
        self.verify_bindings(&lock_file)?;
        Ok(lock_file)
    }

    fn with_locked_state<T>(
        &self,
        operation: impl FnOnce(&FileUidState) -> Result<T, UserItemUidError>,
    ) -> Result<T, UserItemUidError> {
        let lock_file = self.acquire_bound_lock()?;
        let state = self.read_state()?;
        self.verify_bindings(&lock_file)?;
        let outcome = operation(&state);
        self.verify_bindings(&lock_file)?;
        outcome
    }

    fn read_state(&self) -> Result<FileUidState, UserItemUidError> {
        let mut file = open_existing_regular_file(&self.state_path, FilePurpose::State)?;
        let metadata = file
            .metadata()
            .map_err(|source| UserItemUidError::ReadFailed {
                path: self.state_path.clone(),
                source,
            })?;
        if metadata.len() == 0 {
            return Err(self.corrupt("state file is empty"));
        }
        if metadata.len() > MAX_STATE_BYTES {
            return Err(self.corrupt(format!("state file exceeds {MAX_STATE_BYTES} bytes")));
        }

        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        Read::by_ref(&mut file)
            .take(MAX_STATE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| UserItemUidError::ReadFailed {
                path: self.state_path.clone(),
                source,
            })?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err(self.corrupt(format!("state file exceeds {MAX_STATE_BYTES} bytes")));
        }
        let state: FileUidState = serde_json::from_slice(&bytes)
            .map_err(|error| self.corrupt(format!("invalid strict JSON: {error}")))?;
        state.validate(&self.state_path)?;
        Ok(state)
    }

    fn persist_state(
        &self,
        state: &FileUidState,
        lock_file: &File,
    ) -> Result<(), UserItemUidError> {
        state.validate(&self.state_path)?;
        self.verify_bindings(lock_file)?;
        if let Some(kind) = secure_file_kind(&self.state_path)? {
            if kind != ExistingFileKind::Regular {
                return Err(UserItemUidError::UnsafePath {
                    path: self.state_path.clone(),
                    reason: "state path is not a regular file".to_owned(),
                });
            }
        }

        let mut bytes = serde_json::to_vec_pretty(state).map_err(|error| {
            UserItemUidError::PersistenceFailed {
                path: self.state_path.clone(),
                source: io::Error::new(io::ErrorKind::InvalidData, error),
            }
        })?;
        bytes.push(b'\n');

        let parent = self
            .state_path
            .parent()
            .expect("normalized path has a parent");
        let file_name = self
            .state_path
            .file_name()
            .expect("normalized path has a file name");
        let mut last_collision = None;

        for _ in 0..64 {
            let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let mut temp_name = file_name.to_os_string();
            temp_name.push(format!(".tmp.{}.{}", std::process::id(), sequence));
            let temp_path = parent.join(temp_name);
            match create_new_regular_file(&temp_path) {
                Ok(mut temp_file) => {
                    let precommit_write = (|| -> io::Result<()> {
                        temp_file.write_all(&bytes)?;
                        temp_file.sync_all()?;
                        drop(temp_file);
                        Ok(())
                    })();
                    if let Err(source) = precommit_write {
                        let _ = fs::remove_file(&temp_path);
                        return Err(UserItemUidError::PersistenceFailed {
                            path: self.state_path.clone(),
                            source,
                        });
                    }

                    if let Err(error) = self.verify_bindings(lock_file) {
                        let _ = fs::remove_file(&temp_path);
                        return Err(error);
                    }
                    if let Err(error) = secure_file_kind(&self.state_path) {
                        let _ = fs::remove_file(&temp_path);
                        return Err(error);
                    }
                    if let Err(source) = atomic_replace(&temp_path, &self.state_path) {
                        let _ = fs::remove_file(&temp_path);
                        return Err(UserItemUidError::PersistenceFailed {
                            path: self.state_path.clone(),
                            source,
                        });
                    }

                    let post_commit = (|| -> Result<(), String> {
                        sync_parent_directory(parent).map_err(|error| {
                            format!("parent directory sync failed after atomic replace: {error}")
                        })?;
                        self.verify_bindings(lock_file).map_err(|error| {
                            format!("authority identity changed after atomic replace: {error}")
                        })?;
                        let committed = self.read_state().map_err(|error| {
                            format!("committed state could not be read back: {error}")
                        })?;
                        self.verify_bindings(lock_file).map_err(|error| {
                            format!("authority identity changed after read-back: {error}")
                        })?;
                        if committed != *state {
                            return Err(
                                "committed state did not match the requested state".to_owned()
                            );
                        }
                        Ok(())
                    })();
                    if let Err(reason) = post_commit {
                        return Err(UserItemUidError::CommitOutcomeUnknown {
                            path: self.state_path.clone(),
                            reason,
                        });
                    }
                    return Ok(());
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    last_collision = Some(error);
                }
                Err(source) => {
                    return Err(UserItemUidError::PersistenceFailed {
                        path: temp_path,
                        source,
                    });
                }
            }
        }

        Err(UserItemUidError::PersistenceFailed {
            path: self.state_path.clone(),
            source: last_collision.unwrap_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "could not reserve a unique sidecar temporary file",
                )
            }),
        })
    }

    fn corrupt(&self, reason: impl Into<String>) -> UserItemUidError {
        UserItemUidError::CorruptState {
            path: self.state_path.clone(),
            reason: reason.into(),
        }
    }
}

impl UserItemUidStore for FileUserItemUidAuthority {
    fn issue_one(&self, _reason: UserItemUidReason) -> Result<UserItemUid, UserItemUidError> {
        let lock_file = self.acquire_bound_lock()?;
        let current = self.read_state()?;
        self.verify_bindings(&lock_file)?;
        let issued_through = current.issued_value(&self.state_path)?;
        let next = issued_through
            .checked_add(1)
            .ok_or(UserItemUidError::Exhausted {
                issued_through,
                generation: current.generation,
            })?;
        let next_generation =
            current
                .generation
                .checked_add(1)
                .ok_or(UserItemUidError::Exhausted {
                    issued_through,
                    generation: current.generation,
                })?;
        let next_state = FileUidState {
            schema_version: FILE_STATE_SCHEMA_VERSION,
            issued_through: next.to_string(),
            generation: next_generation,
        };
        self.persist_state(&next_state, &lock_file)?;
        self.verify_bindings(&lock_file)?;
        UserItemUid::new(next)
    }

    fn issued_through(&self) -> Result<u64, UserItemUidError> {
        self.with_locked_state(|state| state.issued_value(&self.state_path))
    }

    fn ensure_issued_through_at_least(&self, floor: u64) -> Result<u64, UserItemUidError> {
        let lock_file = self.acquire_bound_lock()?;
        let current = self.read_state()?;
        self.verify_bindings(&lock_file)?;
        let issued_through = current.issued_value(&self.state_path)?;
        if floor <= issued_through {
            self.verify_bindings(&lock_file)?;
            return Ok(issued_through);
        }

        let next_generation =
            current
                .generation
                .checked_add(1)
                .ok_or(UserItemUidError::Exhausted {
                    issued_through,
                    generation: current.generation,
                })?;
        let raised = FileUidState {
            schema_version: FILE_STATE_SCHEMA_VERSION,
            issued_through: floor.to_string(),
            generation: next_generation,
        };
        self.persist_state(&raised, &lock_file)?;
        self.verify_bindings(&lock_file)?;
        Ok(floor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileUidState {
    schema_version: u16,
    issued_through: String,
    generation: u64,
}

impl FileUidState {
    fn validate(&self, path: &Path) -> Result<(), UserItemUidError> {
        if self.schema_version != FILE_STATE_SCHEMA_VERSION {
            return Err(UserItemUidError::CorruptState {
                path: path.to_path_buf(),
                reason: format!(
                    "unsupported schemaVersion {}; expected {FILE_STATE_SCHEMA_VERSION}",
                    self.schema_version
                ),
            });
        }
        if self.generation == 0 {
            return Err(UserItemUidError::CorruptState {
                path: path.to_path_buf(),
                reason: "generation must be non-zero".to_owned(),
            });
        }
        let _ = self.issued_value(path)?;
        Ok(())
    }

    fn issued_value(&self, path: &Path) -> Result<u64, UserItemUidError> {
        if self.issued_through.is_empty()
            || !self
                .issued_through
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        {
            return Err(UserItemUidError::CorruptState {
                path: path.to_path_buf(),
                reason: "issuedThrough must be a non-empty decimal string".to_owned(),
            });
        }
        let value =
            self.issued_through
                .parse::<u64>()
                .map_err(|error| UserItemUidError::CorruptState {
                    path: path.to_path_buf(),
                    reason: format!("issuedThrough is outside the u64 range: {error}"),
                })?;
        if value.to_string() != self.issued_through {
            return Err(UserItemUidError::CorruptState {
                path: path.to_path_buf(),
                reason: "issuedThrough is not canonical decimal".to_owned(),
            });
        }
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExistingFileKind {
    Regular,
}

#[derive(Debug, Clone, Copy)]
enum FilePurpose {
    State,
    Lock,
}

fn resolve_authority_paths(path: &Path) -> Result<(PathBuf, PathBuf), UserItemUidError> {
    let state_path = normalize_state_path(path)?;
    let lock_path = lock_path_for(&state_path)?;
    Ok((state_path, lock_path))
}

fn verify_identity(
    path: &Path,
    expected: StableFileIdentity,
    actual: StableFileIdentity,
) -> Result<(), UserItemUidError> {
    if expected == actual {
        Ok(())
    } else {
        Err(UserItemUidError::AuthorityIdentityChanged {
            path: path.to_path_buf(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn stable_file_identity_from_parts(
    storage_domain: u64,
    object_id: [u8; 16],
    path: &Path,
) -> Result<StableFileIdentity, UserItemUidError> {
    if object_id.iter().all(|byte| *byte == 0) {
        return Err(UserItemUidError::ReadFailed {
            path: path.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::InvalidData,
                "filesystem returned an all-zero stable file object identifier",
            ),
        });
    }
    Ok(StableFileIdentity {
        storage_domain,
        object_id,
    })
}

#[cfg(unix)]
fn stable_file_identity(file: &File, path: &Path) -> Result<StableFileIdentity, UserItemUidError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file
        .metadata()
        .map_err(|source| UserItemUidError::ReadFailed {
            path: path.to_path_buf(),
            source,
        })?;
    let mut object_id = [0; 16];
    object_id[8..].copy_from_slice(&metadata.ino().to_be_bytes());
    stable_file_identity_from_parts(metadata.dev(), object_id, path)
}

#[cfg(windows)]
fn stable_file_identity(file: &File, path: &Path) -> Result<StableFileIdentity, UserItemUidError> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandleEx(
            file: *mut core::ffi::c_void,
            information_class: i32,
            information: *mut core::ffi::c_void,
            buffer_size: u32,
        ) -> i32;
    }

    let mut information = MaybeUninit::<WindowsFileIdInfo>::uninit();
    let result = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            WINDOWS_FILE_ID_INFO_CLASS,
            information.as_mut_ptr().cast(),
            std::mem::size_of::<WindowsFileIdInfo>() as u32,
        )
    };
    if result == 0 {
        return Err(UserItemUidError::ReadFailed {
            path: path.to_path_buf(),
            source: io::Error::last_os_error(),
        });
    }
    let information = unsafe { information.assume_init() };
    stable_file_identity_from_parts(
        information.volume_serial_number,
        information.file_id.identifier,
        path,
    )
}

fn open_authority_directory(path: &Path) -> Result<File, UserItemUidError> {
    reject_link_or_reparse_chain(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    apply_directory_no_follow_flags(&mut options);
    let file = options
        .open(path)
        .map_err(|source| UserItemUidError::ReadFailed {
            path: path.to_path_buf(),
            source,
        })?;
    let metadata = file
        .metadata()
        .map_err(|source| UserItemUidError::ReadFailed {
            path: path.to_path_buf(),
            source,
        })?;
    if !metadata.is_dir() || metadata_is_reparse_point(&metadata) {
        return Err(UserItemUidError::UnsafePath {
            path: path.to_path_buf(),
            reason: "authority parent is not a regular non-reparse directory".to_owned(),
        });
    }
    Ok(file)
}

fn normalize_state_path(path: &Path) -> Result<PathBuf, UserItemUidError> {
    if path.as_os_str().is_empty() {
        return Err(UserItemUidError::InvalidPath {
            path: path.to_path_buf(),
            reason: "path is empty".to_owned(),
        });
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(UserItemUidError::InvalidPath {
            path: path.to_path_buf(),
            reason: "parent-directory components are not allowed".to_owned(),
        });
    }
    #[cfg(windows)]
    reject_windows_unsafe_prefix(path)?;
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| UserItemUidError::InvalidPath {
                path: path.to_path_buf(),
                reason: format!("cannot resolve current directory: {source}"),
            })?
            .join(path)
    };
    #[cfg(windows)]
    reject_windows_unsafe_prefix(&absolute)?;
    let file_name = absolute
        .file_name()
        .ok_or_else(|| UserItemUidError::InvalidPath {
            path: absolute.clone(),
            reason: "path has no file name".to_owned(),
        })?;
    #[cfg(windows)]
    if file_name.to_string_lossy().contains(':') {
        return Err(UserItemUidError::InvalidPath {
            path: absolute,
            reason: "Windows alternate data streams are not allowed".to_owned(),
        });
    }
    let parent = absolute
        .parent()
        .ok_or_else(|| UserItemUidError::InvalidPath {
            path: absolute.clone(),
            reason: "path has no parent directory".to_owned(),
        })?;
    reject_link_or_reparse_chain(parent)?;
    let canonical_parent =
        parent
            .canonicalize()
            .map_err(|source| UserItemUidError::InvalidPath {
                path: parent.to_path_buf(),
                reason: format!("parent directory is unavailable: {source}"),
            })?;
    #[cfg(windows)]
    reject_windows_unsafe_prefix(&canonical_parent)?;
    reject_link_or_reparse_chain(&canonical_parent)?;
    Ok(canonical_parent.join(file_name))
}

#[cfg(windows)]
fn reject_windows_unsafe_prefix(path: &Path) -> Result<(), UserItemUidError> {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Prefix;

    let prefix = match path.components().next() {
        Some(Component::Prefix(prefix)) => prefix.kind(),
        _ => return Ok(()),
    };
    let drive = match prefix {
        Prefix::UNC(_, _)
        | Prefix::VerbatimUNC(_, _)
        | Prefix::DeviceNS(_)
        | Prefix::Verbatim(_) => {
            return Err(UserItemUidError::InvalidPath {
                path: path.to_path_buf(),
                reason: "UNC, device, and non-disk verbatim paths are not allowed".to_owned(),
            });
        }
        Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
    };

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDriveTypeW(root_path_name: *const u16) -> u32;
    }

    const DRIVE_UNKNOWN: u32 = 0;
    const DRIVE_NO_ROOT_DIR: u32 = 1;
    const DRIVE_REMOTE: u32 = 4;
    let root = format!("{}:\\", char::from(drive));
    let root_wide: Vec<u16> = std::ffi::OsStr::new(&root)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let drive_type = unsafe { GetDriveTypeW(root_wide.as_ptr()) };
    if drive_type == DRIVE_REMOTE {
        return Err(UserItemUidError::InvalidPath {
            path: path.to_path_buf(),
            reason: "network-backed drive paths are not allowed".to_owned(),
        });
    }
    if matches!(drive_type, DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR) {
        return Err(UserItemUidError::InvalidPath {
            path: path.to_path_buf(),
            reason: "drive root is unavailable or has unknown durability semantics".to_owned(),
        });
    }
    Ok(())
}

fn lock_path_for(state_path: &Path) -> Result<PathBuf, UserItemUidError> {
    let lock_path = state_path.with_extension("lock");
    if lock_path == state_path {
        return Err(UserItemUidError::InvalidPath {
            path: state_path.to_path_buf(),
            reason: "state and lock paths would be identical".to_owned(),
        });
    }
    Ok(lock_path)
}

fn reject_link_or_reparse_chain(path: &Path) -> Result<(), UserItemUidError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(UserItemUidError::InvalidPath {
                path: path.to_path_buf(),
                reason: "parent-directory components are not allowed".to_owned(),
            });
        }
        #[cfg(windows)]
        if matches!(component, Component::Prefix(_)) {
            current.push(component.as_os_str());
            continue;
        }
        current.push(component.as_os_str());
        let metadata =
            fs::symlink_metadata(&current).map_err(|source| UserItemUidError::InvalidPath {
                path: current.clone(),
                reason: format!("path component is unavailable: {source}"),
            })?;
        if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
            return Err(UserItemUidError::UnsafePath {
                path: current,
                reason: "symbolic links and reparse points are not allowed".to_owned(),
            });
        }
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn secure_file_kind(path: &Path) -> Result<Option<ExistingFileKind>, UserItemUidError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
                return Err(UserItemUidError::UnsafePath {
                    path: path.to_path_buf(),
                    reason: "symbolic links and reparse points are not allowed".to_owned(),
                });
            }
            if !metadata.is_file() {
                return Err(UserItemUidError::UnsafePath {
                    path: path.to_path_buf(),
                    reason: "path is not a regular file".to_owned(),
                });
            }
            Ok(Some(ExistingFileKind::Regular))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(UserItemUidError::ReadFailed {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn open_or_create_initialization_lock(path: &Path) -> Result<File, UserItemUidError> {
    match create_new_regular_file(path) {
        Ok(file) => {
            file.sync_all()
                .map_err(|source| UserItemUidError::PersistenceFailed {
                    path: path.to_path_buf(),
                    source,
                })?;
            Ok(file)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            open_existing_regular_file(path, FilePurpose::Lock)
        }
        Err(source) => Err(UserItemUidError::PersistenceFailed {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn open_existing_regular_file(path: &Path, purpose: FilePurpose) -> Result<File, UserItemUidError> {
    match secure_file_kind(path)? {
        Some(ExistingFileKind::Regular) => {}
        None => {
            return Err(UserItemUidError::MissingState {
                path: path.to_path_buf(),
            });
        }
    }

    let mut options = OpenOptions::new();
    options.read(true);
    if matches!(purpose, FilePurpose::Lock) {
        options.write(true);
    }
    apply_no_follow_flags(&mut options);
    let file = options.open(path).map_err(|source| match purpose {
        FilePurpose::Lock => UserItemUidError::LockFailed {
            path: path.to_path_buf(),
            source,
        },
        FilePurpose::State => UserItemUidError::ReadFailed {
            path: path.to_path_buf(),
            source,
        },
    })?;
    ensure_open_file_is_regular(&file, path)?;
    Ok(file)
}

fn create_new_regular_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    apply_no_follow_flags(&mut options);
    options.open(path)
}

fn ensure_open_file_is_regular(file: &File, path: &Path) -> Result<(), UserItemUidError> {
    let metadata = file
        .metadata()
        .map_err(|source| UserItemUidError::ReadFailed {
            path: path.to_path_buf(),
            source,
        })?;
    if !metadata.is_file() || metadata_is_reparse_point(&metadata) {
        return Err(UserItemUidError::UnsafePath {
            path: path.to_path_buf(),
            reason: "opened object is not a regular non-reparse file".to_owned(),
        });
    }
    Ok(())
}

fn acquire_exclusive_lock(file: &File, path: &Path) -> Result<(), UserItemUidError> {
    file.lock().map_err(|source| UserItemUidError::LockFailed {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn apply_no_follow_flags(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
}

#[cfg(windows)]
fn apply_no_follow_flags(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}

#[cfg(unix)]
fn apply_directory_no_follow_flags(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
}

#[cfg(windows)]
fn apply_directory_no_follow_flags(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
}

#[cfg(not(any(unix, windows)))]
compile_error!("file UID authority requires Windows or Unix file-lock semantics");

#[cfg(unix)]
fn atomic_replace(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn atomic_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    options.open(parent)?.sync_all()
}

#[cfg(windows)]
fn sync_parent_directory(_parent: &Path) -> io::Result<()> {
    // MOVEFILE_WRITE_THROUGH above waits for the atomic replacement to reach disk.
    Ok(())
}

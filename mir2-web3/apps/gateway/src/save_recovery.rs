use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, KeyInit, Mac};
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};
use mir2_simulation::{CharacterSaveRecord, SimulationConfig, SimulationSession};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

const JOURNAL_SCHEMA: &str = "mir2.character-recovery-journal";
const JOURNAL_VERSION: u16 = 1;
const CHECKPOINT_SCHEMA_VERSION: u16 = 1;
const HASH_ALGORITHM: &str = "sha256";
const MAC_ALGORITHM: &str = "hmac-sha256";
const JOURNAL_DIRECTORY: &str = ".mir2-save-recovery-v1";
const QUARANTINE_DIRECTORY: &str = "quarantine";
const DIRECTORY_DURABILITY_MARKER: &str = ".mir2-directory-durability";
const DIRECTORY_OWNER_SENTINEL: &str = ".mir2-recovery-owner-v1.json";
const DIRECTORY_LOCK_FILE: &str = ".mir2-recovery-exclusive.lock";
const DIRECTORY_OWNER_SCHEMA: &str = "mir2.save-recovery-directory";
const DIRECTORY_OWNER_VERSION: u16 = 1;
const DIRECTORY_OWNER_MAX_BYTES: u64 = 16 * 1024;
const MAX_ENTRIES: usize = 256;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RECORD_BYTES: u64 = 4 * 1024 * 1024;

static JOURNAL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static DIRECTORY_LEASES: OnceLock<Mutex<HashMap<PathBuf, Arc<RecoveryDirectoryLease>>>> =
    OnceLock::new();
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static SAVED_TOTAL: AtomicU64 = AtomicU64::new(0);
static JOURNALED_TOTAL: AtomicU64 = AtomicU64::new(0);
static QUARANTINED_TOTAL: AtomicU64 = AtomicU64::new(0);
static FATAL_TOTAL: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistenceStatus {
    Saved,
    Journaled,
    Quarantined,
    Fatal,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ReplaySummary {
    pub replayed: usize,
    pub already_committed: usize,
    pub quarantined: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JournalReceipt {
    pub key_hash: String,
    pub payload_hash: String,
    pub already_durable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryEnvelope {
    journal_schema: String,
    journal_version: u16,
    hash_algorithm: String,
    payload_sha256: String,
    mac_algorithm: String,
    payload_hmac_sha256: String,
    payload: RecoveryPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryDirectorySentinel {
    schema: String,
    version: u16,
    directory_id: String,
    canonical_path_sha256: String,
    mac_algorithm: String,
    sentinel_hmac_sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryDirectorySentinelPayload<'a> {
    schema: &'a str,
    version: u16,
    directory_id: &'a str,
    canonical_path_sha256: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryPayload {
    checkpoint_schema_version: u16,
    account_id: String,
    character_index: i32,
    authoritative_transform: RecoveryTransform,
    checkpoint: CharacterSaveRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryTransform {
    map_file_name: String,
    position: Point,
    direction: MirDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationKind {
    Corrupt,
    UnsupportedVersion,
    MacMismatch,
    HashMismatch,
    IdentityMismatch,
    Oversized,
}

#[derive(Debug)]
struct ValidationError {
    kind: ValidationKind,
    message: String,
}

impl ValidationError {
    fn reason(&self) -> &'static str {
        match self.kind {
            ValidationKind::Corrupt => "corrupt",
            ValidationKind::UnsupportedVersion => "unsupported-version",
            ValidationKind::MacMismatch => "mac-mismatch",
            ValidationKind::HashMismatch => "hash-mismatch",
            ValidationKind::IdentityMismatch => "identity-mismatch",
            ValidationKind::Oversized => "oversized",
        }
    }
}

#[derive(Clone)]
struct RecoveryJournal {
    root: PathBuf,
    quarantine: PathBuf,
    mac_key: [u8; 32],
    lease: Arc<RecoveryDirectoryLease>,
    max_entries: usize,
    max_total_bytes: u64,
    max_record_bytes: u64,
}

impl std::fmt::Debug for RecoveryJournal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoveryJournal")
            .field("root", &self.root)
            .field("quarantine", &self.quarantine)
            .field("mac_key", &"[REDACTED]")
            .field("max_entries", &self.max_entries)
            .field("max_total_bytes", &self.max_total_bytes)
            .field("max_record_bytes", &self.max_record_bytes)
            .finish()
    }
}

struct RecoveryDirectoryLease {
    root: PathBuf,
    canonical_root: PathBuf,
    root_identity: StableFileIdentity,
    root_handle: File,
    lock_identity: StableFileIdentity,
    lock_file: File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtomicWriteReceipt {
    published_identity: StableFileIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicWriteHookPhase {
    TempSynced,
    Published,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayDecision {
    Replayed,
    AlreadyCommitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayCleanupAction {
    Commit,
    Quarantine,
}

struct AuthenticatedRecoveryEntry {
    envelope: RecoveryEnvelope,
    identity: StableFileIdentity,
}

pub(crate) fn record_persistence_status(
    status: PersistenceStatus,
    transport: &'static str,
    detail: &str,
) {
    let count = match status {
        PersistenceStatus::Saved => SAVED_TOTAL.fetch_add(1, Ordering::Relaxed) + 1,
        PersistenceStatus::Journaled => JOURNALED_TOTAL.fetch_add(1, Ordering::Relaxed) + 1,
        PersistenceStatus::Quarantined => QUARANTINED_TOTAL.fetch_add(1, Ordering::Relaxed) + 1,
        PersistenceStatus::Fatal => FATAL_TOTAL.fetch_add(1, Ordering::Relaxed) + 1,
    };
    let status = match status {
        PersistenceStatus::Saved => "saved",
        PersistenceStatus::Journaled => "journaled",
        PersistenceStatus::Quarantined => "quarantined",
        PersistenceStatus::Fatal => "fatal",
    };
    eprintln!(
        "[save-recovery] status={status} transport={transport} count={count} detail={detail}"
    );
}

pub(crate) fn journal_checkpoint(
    config: &SimulationConfig,
    account_id: &str,
    character_index: i32,
    checkpoint: &CharacterSaveRecord,
) -> Result<JournalReceipt, String> {
    let _guard = journal_guard()?;
    let journal = RecoveryJournal::from_config(config)?.ok_or_else(|| {
        "recovery journal requires an explicit recovery directory or file account-store path"
            .to_string()
    })?;
    journal.ensure_layout()?;
    journal.store(account_id, character_index, checkpoint)
}

pub(crate) fn replay_startup(config: &SimulationConfig) -> Result<ReplaySummary, String> {
    let _guard = journal_guard()?;
    let Some(journal) = RecoveryJournal::from_config(config)? else {
        return Ok(ReplaySummary::default());
    };
    journal.ensure_layout()?;
    journal.replay_matching(config, None, false)
}

pub(crate) fn replay_account(
    config: &SimulationConfig,
    account_id: &str,
) -> Result<ReplaySummary, String> {
    let _guard = journal_guard()?;
    let Some(journal) = RecoveryJournal::from_config(config)? else {
        return Ok(ReplaySummary::default());
    };
    journal.ensure_layout()?;
    let account_hash = account_hash(account_id);
    if journal.has_quarantine_for_account(&account_hash)? {
        return Err(format!(
            "account recovery is quarantined; operator review required (accountHash={account_hash})"
        ));
    }
    let summary = journal.replay_matching(config, Some((account_id, &account_hash)), true)?;
    if journal.has_quarantine_for_account(&account_hash)? {
        return Err(format!(
            "account recovery was quarantined; refusing login overwrite (accountHash={account_hash})"
        ));
    }
    Ok(summary)
}

pub(crate) fn provision_account_if_recovery_clear<T, F>(
    config: &SimulationConfig,
    account_id: &str,
    provision: F,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    let _guard = journal_guard()?;
    if let Some(journal) = RecoveryJournal::from_config(config)? {
        journal.ensure_layout()?;
        if journal.account_has_recovery_state(account_id)? {
            return Err(
                "passkey account provisioning is blocked by pending recovery state".to_string(),
            );
        }
        provision()
    } else {
        provision()
    }
}

impl RecoveryJournal {
    fn from_config(config: &SimulationConfig) -> Result<Option<Self>, String> {
        let root = if let Some(explicit) = config.save_recovery_dir.as_ref() {
            explicit.clone()
        } else if let Some(account_store_path) = config.account_store_path.as_deref() {
            let parent = account_store_path
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            parent.join(JOURNAL_DIRECTORY)
        } else if config.account_store_database_url.is_some() {
            return Err(
                "MIR2_SAVE_RECOVERY_DIR is required for PostgreSQL account-store deployments"
                    .to_string(),
            );
        } else {
            return Ok(None);
        };
        let mac_key = config.save_recovery_mac_key().copied().ok_or_else(|| {
            "save recovery is enabled but no dedicated 32-byte MAC key is configured".to_string()
        })?;
        reject_recovery_directory_overlap(config, &root)?;
        let lease = initialize_and_acquire_recovery_directory(&root, &mac_key)?;
        Ok(Some(Self {
            quarantine: root.join(QUARANTINE_DIRECTORY),
            root,
            mac_key,
            lease,
            max_entries: MAX_ENTRIES,
            max_total_bytes: MAX_TOTAL_BYTES,
            max_record_bytes: MAX_RECORD_BYTES,
        }))
    }

    #[cfg(test)]
    fn with_limits(
        config: &SimulationConfig,
        max_entries: usize,
        max_total_bytes: u64,
        max_record_bytes: u64,
    ) -> Result<Self, String> {
        let mut journal = Self::from_config(config)?.ok_or_else(|| {
            "test recovery journal requires an explicit recovery directory or file account-store path"
                .to_string()
        })?;
        journal.max_entries = max_entries;
        journal.max_total_bytes = max_total_bytes;
        journal.max_record_bytes = max_record_bytes;
        Ok(journal)
    }

    fn ensure_layout(&self) -> Result<(), String> {
        self.verify_directory_trust()?;
        ensure_durable_directory(&self.quarantine).map_err(|error| {
            format!(
                "recovery quarantine directory {} is not durable: {error}",
                self.quarantine.display()
            )
        })?;
        self.verify_directory_trust()?;
        Ok(())
    }

    fn verify_directory_trust(&self) -> Result<(), String> {
        verify_recovery_directory_lease(&self.lease).map_err(|error| {
            format!("recovery directory ownership/identity verification failed: {error}")
        })?;
        validate_directory_sentinel(&self.root, &self.mac_key)
    }

    fn reconcile_publication_temps(&self) -> Result<(), String> {
        self.verify_directory_trust()?;
        let mut temps = Vec::new();
        for entry in fs::read_dir(&self.root)
            .map_err(|error| format!("read recovery root for publication recovery: {error}"))?
        {
            let entry =
                entry.map_err(|error| format!("read recovery publication entry: {error}"))?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return Err("recovery root contains a non-UTF-8 entry".to_string());
            };
            if publication_temp_target_name(&name).is_some() {
                temps.push((entry.path(), name));
            }
        }
        temps.sort_by(|left, right| left.1.cmp(&right.1));
        for (temp_path, temp_name) in temps {
            let final_name = publication_temp_target_name(&temp_name)
                .ok_or_else(|| "invalid recovery publication temp name".to_string())?;
            let final_path = self.root.join(final_name);
            let temp_entry = self
                .read_authenticated_entry_bound(&temp_path)
                .map_err(|error| untrusted_recovery_entry_error(&error))?;
            self.validate_entry_filename(&final_path, &temp_entry.envelope)
                .map_err(|error| untrusted_recovery_entry_error(&error))?;
            if final_path.exists() {
                let final_entry = self
                    .read_authenticated_entry_bound(&final_path)
                    .map_err(|error| untrusted_recovery_entry_error(&error))?;
                self.validate_entry_filename(&final_path, &final_entry.envelope)
                    .map_err(|error| untrusted_recovery_entry_error(&error))?;
                if final_entry.envelope.payload_hmac_sha256
                    != temp_entry.envelope.payload_hmac_sha256
                    || final_entry.identity != temp_entry.identity
                {
                    return Err(
                        "recovery publication contains a final record and a different temp; operator review required"
                            .to_string(),
                    );
                }
                ensure_path_identity(&temp_path, temp_entry.identity).map_err(|error| {
                    format!("recovery publication temp changed before cleanup: {error}")
                })?;
                return Err(
                    "recovery publication has two authenticated paths to the same record; automatic path cleanup is disabled because identity-bound unlink is unavailable"
                        .to_string(),
                );
            }
            ensure_path_identity(&temp_path, temp_entry.identity).map_err(|error| {
                format!("recovery publication temp changed before promotion: {error}")
            })?;
            durable_rename(&temp_path, &final_path, false)
                .map_err(|error| format!("publish recovered temp without overwrite: {error}"))?;
            ensure_path_identity(&final_path, temp_entry.identity).map_err(|error| {
                format!("recovery publication changed during promotion: {error}")
            })?;
            sync_directory(&self.root)
                .map_err(|error| format!("sync recovered publication: {error}"))?;
        }
        self.verify_directory_trust()
    }

    fn store(
        &self,
        account_id: &str,
        character_index: i32,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<JournalReceipt, String> {
        self.store_with_post_publish_hook(account_id, character_index, checkpoint, |_path| {})
    }

    fn store_with_post_publish_hook<F>(
        &self,
        account_id: &str,
        character_index: i32,
        checkpoint: &CharacterSaveRecord,
        mut after_publish: F,
    ) -> Result<JournalReceipt, String>
    where
        F: FnMut(&Path),
    {
        self.verify_directory_trust()?;
        let payload = RecoveryPayload::new(account_id, character_index, checkpoint)?;
        let payload_bytes = canonical_payload_bytes(&payload)?;
        let payload_hash = sha256_hex(&payload_bytes);
        let key_hash = character_key_hash(account_id, character_index);
        let path = self.journal_path(account_id, character_index);
        let committed_path = self.committed_path(account_id, character_index);

        if self.has_quarantine_for_account(&account_hash(account_id))? {
            return Err("recovery journal account is quarantined; refusing overwrite".to_string());
        }
        if committed_path.exists() {
            return Err(format!(
                "recovery journal committed cleanup is still pending for key {key_hash}"
            ));
        }
        if path.exists() {
            let existing = self
                .read_authenticated_entry_bound(&path)
                .map_err(|error| {
                    format!(
                        "existing recovery journal must be quarantined before reuse: {}",
                        error.message
                    )
                })?;
            self.validate_entry_filename(&path, &existing.envelope)
                .map_err(|error| {
                    format!(
                        "existing recovery journal must be quarantined before reuse: {}",
                        error.message
                    )
                })?;
            if existing.envelope.payload_sha256 == payload_hash
                && existing.envelope.payload.account_id == account_id
                && existing.envelope.payload.character_index == character_index
            {
                sync_directory(&self.root).map_err(|error| {
                    format!(
                        "existing recovery journal could not be proven directory-durable: {error}"
                    )
                })?;
                self.verify_published_entry(&path, existing.identity, &existing.envelope)
                    .map_err(|error| {
                        format!(
                            "existing recovery journal changed during durability proof: {error}"
                        )
                    })?;
                self.verify_directory_trust()?;
                return Ok(JournalReceipt {
                    key_hash,
                    payload_hash,
                    already_durable: true,
                });
            }
            return Err(format!(
                "recovery journal key conflict for {key_hash}; refusing overwrite"
            ));
        }

        let envelope = RecoveryEnvelope {
            journal_schema: JOURNAL_SCHEMA.to_string(),
            journal_version: JOURNAL_VERSION,
            hash_algorithm: HASH_ALGORITHM.to_string(),
            payload_sha256: payload_hash.clone(),
            mac_algorithm: MAC_ALGORITHM.to_string(),
            payload_hmac_sha256: hmac_sha256_hex(
                &self.mac_key,
                b"mir2-recovery-payload-v1\0",
                &payload_bytes,
            )?,
            payload,
        };
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|error| format!("encode recovery journal: {error}"))?;
        if bytes.len() as u64 > self.max_record_bytes {
            return Err(format!(
                "recovery journal record exceeds {} byte limit",
                self.max_record_bytes
            ));
        }
        let (entries, total_bytes) = self.capacity_usage()?;
        if entries >= self.max_entries {
            return Err(format!(
                "recovery journal capacity reached: {entries}/{} entries; no entry was evicted",
                self.max_entries
            ));
        }
        if total_bytes.saturating_add(bytes.len() as u64) > self.max_total_bytes {
            return Err(format!(
                "recovery journal capacity reached: {} + {} > {} bytes; no entry was evicted",
                total_bytes,
                bytes.len(),
                self.max_total_bytes
            ));
        }
        let write_receipt = atomic_write_new(&path, &bytes, &self.root).map_err(|error| {
            format!("durable recovery journal write failed for key {key_hash}: {error}")
        })?;
        after_publish(&path);
        self.verify_published_entry(&path, write_receipt.published_identity, &envelope)
            .map_err(|error| {
                format!(
                    "published recovery journal verification failed for key {key_hash}: {error}"
                )
            })?;
        self.verify_directory_trust()?;
        Ok(JournalReceipt {
            key_hash,
            payload_hash,
            already_durable: false,
        })
    }

    fn verify_published_entry(
        &self,
        path: &Path,
        published_identity: StableFileIdentity,
        expected: &RecoveryEnvelope,
    ) -> Result<(), String> {
        let entry = self
            .read_authenticated_entry_bound(path)
            .map_err(|error| error.message)?;
        if entry.identity != published_identity {
            return Err(
                "published recovery journal path no longer identifies the written file".to_string(),
            );
        }
        self.validate_entry_filename(path, &entry.envelope)
            .map_err(|error| error.message)?;
        if entry.envelope.payload_hmac_sha256 != expected.payload_hmac_sha256
            || entry.envelope.payload_sha256 != expected.payload_sha256
            || entry.envelope.payload.account_id != expected.payload.account_id
            || entry.envelope.payload.character_index != expected.payload.character_index
        {
            return Err(
                "published recovery journal does not match the authenticated checkpoint"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn replay_matching(
        &self,
        config: &SimulationConfig,
        account: Option<(&str, &str)>,
        reject_quarantine: bool,
    ) -> Result<ReplaySummary, String> {
        self.replay_matching_with_hook(
            config,
            account,
            reject_quarantine,
            |_path, _envelope, _action| {},
        )
    }

    fn replay_matching_with_hook<F>(
        &self,
        config: &SimulationConfig,
        account: Option<(&str, &str)>,
        reject_quarantine: bool,
        mut before_cleanup: F,
    ) -> Result<ReplaySummary, String>
    where
        F: FnMut(&Path, &RecoveryEnvelope, ReplayCleanupAction),
    {
        self.verify_directory_trust()?;
        self.reconcile_publication_temps()?;
        let mut paths = self.root_files()?;
        paths.sort();
        let mut summary = ReplaySummary::default();
        for path in paths {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return Err(
                    "recovery root contains an unattributable non-UTF-8 entry; operator review required"
                        .to_string(),
                );
            };
            if name.ends_with(".tmp") {
                return Err(
                    "unreconciled recovery publication temp blocks replay; operator review required"
                        .to_string(),
                );
            }
            if name.ends_with(".committed.json") {
                let entry = self
                    .read_authenticated_entry_bound(&path)
                    .map_err(|error| untrusted_recovery_entry_error(&error))?;
                self.validate_entry_filename(&path, &entry.envelope)
                    .map_err(|error| untrusted_recovery_entry_error(&error))?;
                self.remove_committed(&path, entry.identity)?;
                continue;
            }
            if !name.ends_with(".journal.json") {
                return Err(
                    "recovery root contains unknown content; refusing to move or overwrite it"
                        .to_string(),
                );
            }

            let entry = self
                .read_authenticated_entry_bound(&path)
                .map_err(|error| untrusted_recovery_entry_error(&error))?;
            let envelope = &entry.envelope;
            if let Err(error) = self.validate_entry_filename(&path, &envelope) {
                before_cleanup(&path, envelope, ReplayCleanupAction::Quarantine);
                self.quarantine_authenticated_entry(
                    &path,
                    envelope,
                    entry.identity,
                    error.reason(),
                )?;
                summary.quarantined += 1;
                continue;
            }
            if let Some((account_id, target_account_hash)) = account {
                if envelope.payload.account_id != account_id {
                    if account_hash(&envelope.payload.account_id) == target_account_hash {
                        before_cleanup(&path, envelope, ReplayCleanupAction::Quarantine);
                        self.quarantine_authenticated_entry(
                            &path,
                            envelope,
                            entry.identity,
                            "account-identity-mismatch",
                        )?;
                        summary.quarantined += 1;
                    }
                    continue;
                }
            }
            match replay_envelope(config, &envelope) {
                Ok(ReplayDecision::Replayed) => {
                    before_cleanup(&path, envelope, ReplayCleanupAction::Commit);
                    self.commit_and_remove(&path, entry.identity)?;
                    summary.replayed += 1;
                    record_persistence_status(
                        PersistenceStatus::Saved,
                        "recovery-replay",
                        "journal checkpoint committed to account store",
                    );
                }
                Ok(ReplayDecision::AlreadyCommitted) => {
                    before_cleanup(&path, envelope, ReplayCleanupAction::Commit);
                    self.commit_and_remove(&path, entry.identity)?;
                    summary.already_committed += 1;
                    record_persistence_status(
                        PersistenceStatus::Saved,
                        "recovery-replay",
                        "journal checkpoint was already committed",
                    );
                }
                Err(error) if is_replay_conflict(&error) => {
                    before_cleanup(&path, envelope, ReplayCleanupAction::Quarantine);
                    self.quarantine_authenticated_entry(
                        &path,
                        envelope,
                        entry.identity,
                        "state-conflict",
                    )?;
                    summary.quarantined += 1;
                }
                Err(error) => return Err(error),
            }
        }
        if reject_quarantine && summary.quarantined > 0 {
            return Err(
                "account recovery entry was quarantined; login remains blocked".to_string(),
            );
        }
        self.verify_directory_trust()?;
        Ok(summary)
    }

    fn read_valid_entry(&self, path: &Path) -> Result<RecoveryEnvelope, ValidationError> {
        let envelope = self.read_authenticated_entry(path)?;
        self.validate_entry_filename(path, &envelope)?;
        Ok(envelope)
    }

    fn read_authenticated_entry(&self, path: &Path) -> Result<RecoveryEnvelope, ValidationError> {
        self.read_authenticated_entry_bound(path)
            .map(|entry| entry.envelope)
    }

    fn read_authenticated_entry_bound(
        &self,
        path: &Path,
    ) -> Result<AuthenticatedRecoveryEntry, ValidationError> {
        let (bytes, identity) =
            read_stable_regular_file_with_identity(path, self.max_record_bytes)?;
        let envelope = serde_json::from_slice::<RecoveryEnvelope>(&bytes).map_err(|error| {
            ValidationError {
                kind: ValidationKind::Corrupt,
                message: format!("decode recovery journal: {error}"),
            }
        })?;
        if envelope.journal_schema != JOURNAL_SCHEMA
            || envelope.journal_version != JOURNAL_VERSION
            || envelope.payload.checkpoint_schema_version != CHECKPOINT_SCHEMA_VERSION
        {
            return Err(ValidationError {
                kind: ValidationKind::UnsupportedVersion,
                message: "unsupported recovery journal schema/version".to_string(),
            });
        }
        if envelope.hash_algorithm != HASH_ALGORITHM {
            return Err(ValidationError {
                kind: ValidationKind::UnsupportedVersion,
                message: "unsupported recovery journal hash algorithm".to_string(),
            });
        }
        if envelope.mac_algorithm != MAC_ALGORITHM {
            return Err(ValidationError {
                kind: ValidationKind::UnsupportedVersion,
                message: "unsupported recovery journal MAC algorithm".to_string(),
            });
        }
        let payload_bytes =
            canonical_payload_bytes(&envelope.payload).map_err(|message| ValidationError {
                kind: ValidationKind::Corrupt,
                message,
            })?;
        verify_hmac_sha256_hex(
            &self.mac_key,
            b"mir2-recovery-payload-v1\0",
            &payload_bytes,
            &envelope.payload_hmac_sha256,
        )
        .map_err(|message| ValidationError {
            kind: ValidationKind::MacMismatch,
            message,
        })?;
        if sha256_hex(&payload_bytes) != envelope.payload_sha256 {
            return Err(ValidationError {
                kind: ValidationKind::HashMismatch,
                message: "recovery journal payload hash mismatch".to_string(),
            });
        }
        envelope
            .payload
            .validate()
            .map_err(|message| ValidationError {
                kind: ValidationKind::IdentityMismatch,
                message,
            })?;
        Ok(AuthenticatedRecoveryEntry { envelope, identity })
    }

    fn validate_entry_filename(
        &self,
        path: &Path,
        envelope: &RecoveryEnvelope,
    ) -> Result<(), ValidationError> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| ValidationError {
                kind: ValidationKind::IdentityMismatch,
                message: "recovery journal filename is not UTF-8".to_string(),
            })?;
        let expected_journal = self
            .journal_path(
                &envelope.payload.account_id,
                envelope.payload.character_index,
            )
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let expected_committed = self
            .committed_path(
                &envelope.payload.account_id,
                envelope.payload.character_index,
            )
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        if name != expected_journal && name != expected_committed {
            return Err(ValidationError {
                kind: ValidationKind::IdentityMismatch,
                message: "recovery journal filename/key mismatch".to_string(),
            });
        }
        Ok(())
    }

    fn commit_and_remove(
        &self,
        journal_path: &Path,
        expected_identity: StableFileIdentity,
    ) -> Result<(), String> {
        self.verify_directory_trust()?;
        ensure_path_identity(journal_path, expected_identity)
            .map_err(|error| format!("recovery journal changed before commit: {error}"))?;
        let file_name = journal_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "recovery journal filename is not UTF-8".to_string())?;
        let committed_name = file_name
            .strip_suffix(".journal.json")
            .map(|prefix| format!("{prefix}.committed.json"))
            .ok_or_else(|| "recovery journal filename has no journal suffix".to_string())?;
        let committed_path = self.root.join(committed_name);
        if committed_path.exists() {
            return Err("recovery committed marker already exists; refusing overwrite".to_string());
        }
        durable_rename(journal_path, &committed_path, false).map_err(|error| {
            format!(
                "mark recovery journal committed {}: {error}",
                journal_path.display()
            )
        })?;
        ensure_path_identity(&committed_path, expected_identity)
            .map_err(|error| format!("recovery journal changed during commit: {error}"))?;
        sync_directory(&self.root).map_err(|error| {
            format!(
                "sync recovery committed marker directory {}: {error}",
                self.root.display()
            )
        })?;
        self.verify_directory_trust()?;
        self.remove_committed(&committed_path, expected_identity)
    }

    fn remove_committed(
        &self,
        committed_path: &Path,
        expected_identity: StableFileIdentity,
    ) -> Result<(), String> {
        self.verify_directory_trust()?;
        ensure_path_identity(committed_path, expected_identity)
            .map_err(|error| format!("committed recovery evidence changed: {error}"))?;
        fs::remove_file(committed_path).map_err(|error| {
            format!(
                "remove committed recovery evidence {}: {error}",
                committed_path.display()
            )
        })?;
        sync_directory(&self.root).map_err(|error| {
            format!(
                "sync recovery journal directory after committed cleanup {}: {error}",
                self.root.display()
            )
        })?;
        self.verify_directory_trust()
    }

    fn quarantine_authenticated_entry(
        &self,
        path: &Path,
        envelope: &RecoveryEnvelope,
        expected_identity: StableFileIdentity,
        reason: &'static str,
    ) -> Result<(), String> {
        self.verify_directory_trust()?;
        ensure_path_identity(path, expected_identity).map_err(|error| {
            format!("authenticated recovery entry changed before quarantine: {error}")
        })?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "cannot quarantine non-UTF-8 recovery filename".to_string())?;
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let claimed_account_hash = name.split('-').next().filter(|prefix| {
            prefix.len() == 64 && prefix.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
        let authenticated_account_hash = account_hash(&envelope.payload.account_id);
        let claimed_suffix = claimed_account_hash
            .filter(|claimed| *claimed != authenticated_account_hash)
            .map(|claimed| format!("-claimed-{claimed}"))
            .unwrap_or_default();
        let name_hash = sha256_hex(name.as_bytes());
        let destination = self.quarantine.join(format!(
            "blocked-{authenticated_account_hash}{claimed_suffix}-{}.{reason}.{}.{sequence}.q",
            &name_hash[..24],
            now_millis(),
        ));
        durable_rename(path, &destination, false).map_err(|error| {
            format!(
                "quarantine recovery entry {} -> {}: {error}",
                path.display(),
                destination.display()
            )
        })?;
        ensure_path_identity(&destination, expected_identity).map_err(|error| {
            format!("authenticated recovery entry changed during quarantine: {error}")
        })?;
        sync_directory(&self.root)
            .map_err(|error| format!("sync recovery root after quarantine: {error}"))?;
        sync_directory(&self.quarantine)
            .map_err(|error| format!("sync recovery quarantine after move: {error}"))?;
        self.verify_directory_trust()?;
        record_persistence_status(PersistenceStatus::Quarantined, "recovery-replay", reason);
        Ok(())
    }

    fn has_quarantine_for_account(&self, account_hash: &str) -> Result<bool, String> {
        self.verify_directory_trust()?;
        let owned = format!("blocked-{account_hash}-");
        let claimed = format!("-claimed-{account_hash}-");
        let mut found = false;
        for entry in fs::read_dir(&self.quarantine).map_err(|error| {
            format!(
                "read recovery quarantine directory {}: {error}",
                self.quarantine.display()
            )
        })? {
            let entry =
                entry.map_err(|error| format!("read recovery quarantine entry: {error}"))?;
            if is_recovery_control_artifact(&entry.file_name()) {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| format!("read recovery quarantine entry metadata: {error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("recovery quarantine contains a non-file entry".to_string());
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return Err("recovery quarantine contains a non-UTF-8 entry".to_string());
            };
            if !name.starts_with("blocked-") || !name.ends_with(".q") {
                return Err("recovery quarantine contains unknown/untrusted content".to_string());
            }
            if name.starts_with(&owned) || name.contains(&claimed) {
                found = true;
                break;
            }
        }
        self.verify_directory_trust()?;
        Ok(found)
    }

    fn root_files(&self) -> Result<Vec<PathBuf>, String> {
        self.verify_directory_trust()?;
        let mut paths = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|error| {
            format!(
                "read recovery journal root {}: {error}",
                self.root.display()
            )
        })? {
            let entry = entry.map_err(|error| format!("read recovery journal entry: {error}"))?;
            if entry.path() == self.quarantine {
                continue;
            }
            if is_recovery_control_artifact(&entry.file_name()) {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| format!("read recovery entry metadata: {error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "recovery journal root contains unsupported entry {}",
                    entry.path().display()
                ));
            }
            paths.push(entry.path());
        }
        self.verify_directory_trust()?;
        Ok(paths)
    }

    fn account_has_recovery_state(&self, account_id: &str) -> Result<bool, String> {
        self.verify_directory_trust()?;
        self.reconcile_publication_temps()?;
        if self.has_quarantine_for_account(&account_hash(account_id))? {
            return Ok(true);
        }
        for path in self.root_files()? {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return Err("recovery root contains a non-UTF-8 entry".to_string());
            };
            if !name.ends_with(".journal.json") && !name.ends_with(".committed.json") {
                return Err(
                    "recovery root contains unknown content; refusing account provisioning"
                        .to_string(),
                );
            }
            let entry = self
                .read_authenticated_entry_bound(&path)
                .map_err(|error| untrusted_recovery_entry_error(&error))?;
            self.validate_entry_filename(&path, &entry.envelope)
                .map_err(|error| untrusted_recovery_entry_error(&error))?;
            if entry.envelope.payload.account_id == account_id {
                return Ok(true);
            }
        }
        self.verify_directory_trust()?;
        Ok(false)
    }

    fn capacity_usage(&self) -> Result<(usize, u64), String> {
        self.verify_directory_trust()?;
        let mut entries = 0usize;
        let mut bytes = 0u64;
        for directory in [&self.root, &self.quarantine] {
            for entry in fs::read_dir(directory).map_err(|error| {
                format!(
                    "read recovery capacity directory {}: {error}",
                    directory.display()
                )
            })? {
                let entry =
                    entry.map_err(|error| format!("read recovery capacity entry: {error}"))?;
                if entry.path() == self.quarantine {
                    continue;
                }
                if is_recovery_control_artifact(&entry.file_name()) {
                    continue;
                }
                let metadata = fs::symlink_metadata(entry.path())
                    .map_err(|error| format!("read recovery capacity metadata: {error}"))?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(format!(
                        "recovery capacity scan rejected non-file {}",
                        entry.path().display()
                    ));
                }
                entries = entries.saturating_add(1);
                bytes = bytes.saturating_add(metadata.len());
            }
        }
        self.verify_directory_trust()?;
        Ok((entries, bytes))
    }

    fn journal_path(&self, account_id: &str, character_index: i32) -> PathBuf {
        self.root.join(format!(
            "{}-{character_index}-{}.journal.json",
            account_hash(account_id),
            character_key_hash(account_id, character_index)
        ))
    }

    fn committed_path(&self, account_id: &str, character_index: i32) -> PathBuf {
        self.root.join(format!(
            "{}-{character_index}-{}.committed.json",
            account_hash(account_id),
            character_key_hash(account_id, character_index)
        ))
    }
}

impl RecoveryPayload {
    fn new(
        account_id: &str,
        character_index: i32,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<Self, String> {
        let payload = Self {
            checkpoint_schema_version: CHECKPOINT_SCHEMA_VERSION,
            account_id: account_id.to_string(),
            character_index,
            authoritative_transform: RecoveryTransform {
                map_file_name: checkpoint.map_file_name.clone(),
                position: checkpoint.position.clone(),
                direction: checkpoint.direction,
            },
            checkpoint: checkpoint.clone(),
        };
        payload.validate()?;
        Ok(payload)
    }

    fn validate(&self) -> Result<(), String> {
        if self.account_id.trim().is_empty() {
            return Err("recovery journal account id is empty".to_string());
        }
        if self.character_index != self.checkpoint.character.index {
            return Err("recovery journal character index/checkpoint mismatch".to_string());
        }
        if self.authoritative_transform.map_file_name != self.checkpoint.map_file_name
            || self.authoritative_transform.position != self.checkpoint.position
            || self.authoritative_transform.direction != self.checkpoint.direction
        {
            return Err("recovery journal authoritative transform/checkpoint mismatch".to_string());
        }
        Ok(())
    }
}

fn replay_envelope(
    config: &SimulationConfig,
    envelope: &RecoveryEnvelope,
) -> Result<ReplayDecision, String> {
    let payload = &envelope.payload;
    let mut session = SimulationSession::new(config.clone());
    session
        .select_account_for_recovery(&payload.account_id)
        .map_err(|error| replay_conflict(format!("recovery account selection failed: {error}")))?;
    let packets = session.handle_packet(ClientPacket::StartGame {
        character_index: payload.character_index,
    });
    if !packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. }))
    {
        return Err(replay_conflict(
            "recovery StartGame did not select the journal character".to_string(),
        ));
    }
    let current = session.active_character_checkpoint().ok_or_else(|| {
        replay_conflict("recovery runtime produced no active checkpoint".to_string())
    })?;
    let journal_revision = payload.checkpoint.revision;
    if current.revision > journal_revision {
        if checkpoints_equivalent(&current, &payload.checkpoint)? {
            return Ok(ReplayDecision::AlreadyCommitted);
        }
        return Err(replay_conflict(format!(
            "durable checkpoint revision {} is newer than journal revision {journal_revision}",
            current.revision
        )));
    }
    if current.revision < journal_revision {
        return Err(replay_conflict(format!(
            "durable checkpoint revision {} is older than journal revision {journal_revision}",
            current.revision
        )));
    }

    session
        .restore_active_character_checkpoint(&payload.checkpoint)
        .map_err(|error| replay_conflict(format!("restore recovery checkpoint: {error}")))?;
    session
        .save_active_character_for_logout()
        .map_err(|error| format!("recovery DB save failed; journal retained: {error}"))?;
    let committed = session.active_character_checkpoint().ok_or_else(|| {
        "recovery DB save returned success but active checkpoint disappeared".to_string()
    })?;
    if committed.revision <= journal_revision
        || !checkpoints_equivalent(&committed, &payload.checkpoint)?
    {
        return Err(
            "recovery DB save verification failed; journal retained and login blocked".to_string(),
        );
    }
    Ok(ReplayDecision::Replayed)
}

fn checkpoints_equivalent(
    left: &CharacterSaveRecord,
    right: &CharacterSaveRecord,
) -> Result<bool, String> {
    let mut left = left.clone();
    let mut right = right.clone();
    left.revision = 0;
    right.revision = 0;
    let left = serde_json::to_vec(&left)
        .map_err(|error| format!("encode current recovery checkpoint: {error}"))?;
    let right = serde_json::to_vec(&right)
        .map_err(|error| format!("encode journal recovery checkpoint: {error}"))?;
    Ok(left == right)
}

fn replay_conflict(message: String) -> String {
    format!("recovery-state-conflict: {message}")
}

fn is_replay_conflict(error: &str) -> bool {
    error.starts_with("recovery-state-conflict:")
}

fn canonical_payload_bytes(payload: &RecoveryPayload) -> Result<Vec<u8>, String> {
    serde_json::to_vec(payload).map_err(|error| format!("encode recovery payload: {error}"))
}

fn account_hash(account_id: &str) -> String {
    sha256_hex(format!("mir2-recovery-account-v1\0{account_id}").as_bytes())
}

fn character_key_hash(account_id: &str, character_index: i32) -> String {
    sha256_hex(format!("mir2-recovery-character-v1\0{account_id}\0{character_index}").as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn hmac_sha256_hex(key: &[u8; 32], domain: &[u8], bytes: &[u8]) -> Result<String, String> {
    let mut mac =
        HmacSha256::new_from_slice(key).map_err(|_| "invalid recovery MAC key".to_string())?;
    mac.update(domain);
    mac.update(bytes);
    let signature = mac.finalize().into_bytes();
    Ok(hex_encode(&signature))
}

fn verify_hmac_sha256_hex(
    key: &[u8; 32],
    domain: &[u8],
    bytes: &[u8],
    encoded_signature: &str,
) -> Result<(), String> {
    let signature = decode_hex_32(encoded_signature)
        .ok_or_else(|| "recovery journal MAC encoding is invalid".to_string())?;
    let mut mac =
        HmacSha256::new_from_slice(key).map_err(|_| "invalid recovery MAC key".to_string())?;
    mac.update(domain);
    mac.update(bytes);
    mac.verify_slice(&signature)
        .map_err(|_| "recovery journal MAC verification failed".to_string())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut decoded = [0u8; 32];
    for (index, output) in decoded.iter_mut().enumerate() {
        let offset = index * 2;
        *output = u8::from_str_radix(&value[offset..offset + 2], 16).ok()?;
    }
    Some(decoded)
}

fn untrusted_recovery_entry_error(error: &ValidationError) -> String {
    format!(
        "untrusted or unattributable recovery content ({}) blocks replay; operator review required",
        error.reason()
    )
}

fn reject_recovery_directory_overlap(
    config: &SimulationConfig,
    recovery_root: &Path,
) -> Result<(), String> {
    let recovery_root = absolute_without_parent_components(recovery_root)?;
    if let Some(account_store_path) = config.account_store_path.as_deref() {
        let account_store_path = absolute_without_parent_components(account_store_path)?;
        if account_store_path.starts_with(&recovery_root)
            || account_store_path.parent() == Some(recovery_root.as_path())
        {
            return Err(
                "save recovery directory must be dedicated and must not contain the account store"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn absolute_without_parent_components(path: &Path) -> Result<PathBuf, String> {
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("save recovery paths must not contain parent-directory components".to_string());
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|error| format!("resolve save recovery path: {error}"))
    }
}

fn initialize_and_acquire_recovery_directory(
    root: &Path,
    mac_key: &[u8; 32],
) -> Result<Arc<RecoveryDirectoryLease>, String> {
    ensure_durable_directory(root)
        .map_err(|error| format!("create durable recovery directory: {error}"))?;
    initialize_or_validate_directory_sentinel(root, mac_key)?;
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("canonicalize recovery directory: {error}"))?;
    let leases = DIRECTORY_LEASES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut registry = leases
        .lock()
        .map_err(|_| "recovery directory lease registry was poisoned".to_string())?;
    if let Some(lease) = registry.get(&canonical_root).cloned() {
        verify_recovery_directory_lease(&lease)
            .map_err(|error| format!("verify shared recovery directory lease: {error}"))?;
        return Ok(lease);
    }
    let lease = Arc::new(acquire_recovery_directory_lease(
        root,
        canonical_root.clone(),
    )?);
    registry.insert(canonical_root, Arc::clone(&lease));
    Ok(lease)
}

#[cfg(test)]
pub(crate) fn release_directory_lease_for_tests(root: &Path) {
    let Ok(canonical_root) = fs::canonicalize(root) else {
        return;
    };
    if let Some(leases) = DIRECTORY_LEASES.get() {
        if let Ok(mut leases) = leases.lock() {
            leases.remove(&canonical_root);
        }
    }
}

fn initialize_or_validate_directory_sentinel(
    root: &Path,
    mac_key: &[u8; 32],
) -> Result<(), String> {
    let sentinel_path = root.join(DIRECTORY_OWNER_SENTINEL);
    if sentinel_path.exists() {
        return validate_directory_sentinel(root, mac_key);
    }
    for entry in fs::read_dir(root)
        .map_err(|error| format!("inspect unowned recovery directory: {error}"))?
    {
        let entry = entry.map_err(|error| format!("inspect recovery directory entry: {error}"))?;
        if !is_directory_durability_artifact(&entry.file_name()) {
            return Err(
                "recovery directory has no ownership sentinel and is not empty; refusing adoption"
                    .to_string(),
            );
        }
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("canonicalize new recovery directory: {error}"))?;
    let canonical_path_sha256 = sha256_hex(canonical_root.as_os_str().to_string_lossy().as_bytes());
    let mut directory_id_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut directory_id_bytes);
    let directory_id = hex_encode(&directory_id_bytes);
    let payload = RecoveryDirectorySentinelPayload {
        schema: DIRECTORY_OWNER_SCHEMA,
        version: DIRECTORY_OWNER_VERSION,
        directory_id: &directory_id,
        canonical_path_sha256: &canonical_path_sha256,
    };
    let payload_bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("encode recovery directory sentinel: {error}"))?;
    let sentinel = RecoveryDirectorySentinel {
        schema: DIRECTORY_OWNER_SCHEMA.to_string(),
        version: DIRECTORY_OWNER_VERSION,
        directory_id,
        canonical_path_sha256,
        mac_algorithm: MAC_ALGORITHM.to_string(),
        sentinel_hmac_sha256: hmac_sha256_hex(
            mac_key,
            b"mir2-recovery-directory-v1\0",
            &payload_bytes,
        )?,
    };
    let bytes = serde_json::to_vec(&sentinel)
        .map_err(|error| format!("encode recovery directory sentinel envelope: {error}"))?;
    match atomic_write_new(&sentinel_path, &bytes, root) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("publish recovery directory sentinel: {error}")),
    }
    validate_directory_sentinel(root, mac_key)
}

fn validate_directory_sentinel(root: &Path, mac_key: &[u8; 32]) -> Result<(), String> {
    let sentinel_path = root.join(DIRECTORY_OWNER_SENTINEL);
    let bytes =
        read_stable_regular_file(&sentinel_path, DIRECTORY_OWNER_MAX_BYTES).map_err(|error| {
            format!(
                "read recovery directory ownership sentinel: {}",
                error.message
            )
        })?;
    let sentinel = serde_json::from_slice::<RecoveryDirectorySentinel>(&bytes)
        .map_err(|_| "decode recovery directory ownership sentinel failed".to_string())?;
    if sentinel.schema != DIRECTORY_OWNER_SCHEMA
        || sentinel.version != DIRECTORY_OWNER_VERSION
        || sentinel.mac_algorithm != MAC_ALGORITHM
        || decode_hex_32(&sentinel.directory_id).is_none()
    {
        return Err("unsupported or malformed recovery directory ownership sentinel".to_string());
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("canonicalize recovery directory for ownership check: {error}"))?;
    let expected_path_hash = sha256_hex(canonical_root.as_os_str().to_string_lossy().as_bytes());
    if sentinel.canonical_path_sha256 != expected_path_hash {
        return Err("recovery directory ownership sentinel is bound to another path".to_string());
    }
    let payload = RecoveryDirectorySentinelPayload {
        schema: &sentinel.schema,
        version: sentinel.version,
        directory_id: &sentinel.directory_id,
        canonical_path_sha256: &sentinel.canonical_path_sha256,
    };
    let payload_bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("encode recovery directory ownership proof: {error}"))?;
    verify_hmac_sha256_hex(
        mac_key,
        b"mir2-recovery-directory-v1\0",
        &payload_bytes,
        &sentinel.sentinel_hmac_sha256,
    )
    .map_err(|_| "recovery directory ownership sentinel MAC verification failed".to_string())
}

fn acquire_recovery_directory_lease(
    root: &Path,
    canonical_root: PathBuf,
) -> Result<RecoveryDirectoryLease, String> {
    validate_existing_directory_ancestors(root)
        .map_err(|error| format!("validate recovery directory ancestors: {error}"))?;
    let root_handle = open_directory_no_follow(root)
        .map_err(|error| format!("open stable recovery directory handle: {error}"))?;
    let root_identity = stable_file_identity(&root_handle)
        .map_err(|error| format!("identify recovery directory handle: {error}"))?;
    lock_recovery_root_directory(&root_handle).map_err(|error| {
        format!("recovery directory is already owned by another manager: {error}")
    })?;
    let lock_file =
        open_exclusive_recovery_lock(&root.join(DIRECTORY_LOCK_FILE)).map_err(|error| {
            format!("recovery directory is already owned by another manager: {error}")
        })?;
    let lock_identity = stable_file_identity(&lock_file)
        .map_err(|error| format!("identify recovery directory lock handle: {error}"))?;
    let lease = RecoveryDirectoryLease {
        root: root.to_path_buf(),
        canonical_root,
        root_identity,
        root_handle,
        lock_identity,
        lock_file,
    };
    verify_recovery_directory_lease(&lease).map_err(|error| {
        format!("recovery directory changed while acquiring ownership: {error}")
    })?;
    Ok(lease)
}

fn verify_recovery_directory_lease(lease: &RecoveryDirectoryLease) -> io::Result<()> {
    validate_existing_directory_ancestors(&lease.root)?;
    if fs::canonicalize(&lease.root)? != lease.canonical_root {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "recovery directory canonical path changed",
        ));
    }
    let held_root_identity = stable_file_identity(&lease.root_handle)?;
    if !held_root_identity.same_object(lease.root_identity) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "held recovery directory identity changed",
        ));
    }
    let current_root = open_directory_no_follow(&lease.root)?;
    if !stable_file_identity(&current_root)?.same_object(lease.root_identity) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "recovery directory identity changed",
        ));
    }
    let held_lock_identity = stable_file_identity(&lease.lock_file)?;
    if !held_lock_identity.same_object(lease.lock_identity) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "held recovery directory lock identity changed",
        ));
    }
    let current_lock = open_no_follow(&lease.root.join(DIRECTORY_LOCK_FILE))?;
    validate_lock_file_metadata(&current_lock.metadata()?)?;
    if !stable_file_identity(&current_lock)?.same_object(lease.lock_identity) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "recovery directory lock path no longer identifies the held lock",
        ));
    }
    Ok(())
}

fn publication_temp_target_name(name: &str) -> Option<String> {
    let marker = ".journal.json.";
    let marker_index = name.rfind(marker)?;
    let suffix = &name[marker_index + marker.len()..];
    let mut fields = suffix.split('.');
    let pid = fields.next()?;
    let sequence = fields.next()?;
    if fields.next()? != "tmp"
        || fields.next().is_some()
        || pid.is_empty()
        || sequence.is_empty()
        || !pid.bytes().all(|byte| byte.is_ascii_digit())
        || !sequence.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some(format!("{}.journal.json", &name[..marker_index]))
}

fn is_directory_durability_artifact(name: &OsStr) -> bool {
    name == DIRECTORY_DURABILITY_MARKER
        || name.to_str().is_some_and(|name| {
            name.starts_with(".mir2-directory-durability.") && name.ends_with(".tmp")
        })
}

fn is_recovery_control_artifact(name: &OsStr) -> bool {
    is_directory_durability_artifact(name)
        || name == DIRECTORY_OWNER_SENTINEL
        || name == DIRECTORY_LOCK_FILE
}

fn journal_guard() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    JOURNAL_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "recovery journal lock poisoned; refusing persistence".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StableFileIdentity {
    volume_id: u64,
    object_id: [u8; 16],
    length: u64,
    modified_high: u64,
    modified_low: u64,
}

impl StableFileIdentity {
    fn same_object(self, other: Self) -> bool {
        self.volume_id == other.volume_id && self.object_id == other.object_id
    }
}

fn read_stable_regular_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>, ValidationError> {
    read_stable_regular_file_with_hook(path, max_bytes, || {})
}

fn read_stable_regular_file_with_identity(
    path: &Path,
    max_bytes: u64,
) -> Result<(Vec<u8>, StableFileIdentity), ValidationError> {
    read_stable_regular_file_with_identity_and_hook(path, max_bytes, || {})
}

fn read_stable_regular_file_with_hook(
    path: &Path,
    max_bytes: u64,
    after_open: impl FnOnce(),
) -> Result<Vec<u8>, ValidationError> {
    read_stable_regular_file_with_identity_and_hook(path, max_bytes, after_open)
        .map(|(bytes, _identity)| bytes)
}

fn read_stable_regular_file_with_identity_and_hook(
    path: &Path,
    max_bytes: u64,
    after_open: impl FnOnce(),
) -> Result<(Vec<u8>, StableFileIdentity), ValidationError> {
    let before = fs::symlink_metadata(path).map_err(|error| ValidationError {
        kind: ValidationKind::Corrupt,
        message: format!("read recovery journal metadata before open: {error}"),
    })?;
    validate_regular_metadata(&before, max_bytes, "before open")?;

    let mut file = open_no_follow(path).map_err(|error| ValidationError {
        kind: ValidationKind::Corrupt,
        message: format!("open recovery journal without following links: {error}"),
    })?;
    let handle_metadata = file.metadata().map_err(|error| ValidationError {
        kind: ValidationKind::Corrupt,
        message: format!("read recovery journal handle metadata: {error}"),
    })?;
    validate_regular_metadata(&handle_metadata, max_bytes, "open handle")?;
    let identity_before = stable_file_identity(&file).map_err(|error| ValidationError {
        kind: ValidationKind::Corrupt,
        message: format!("read recovery journal handle identity: {error}"),
    })?;
    if before.len() != identity_before.length {
        return Err(ValidationError {
            kind: ValidationKind::IdentityMismatch,
            message: "recovery journal changed between metadata check and open".to_string(),
        });
    }

    after_open();
    let mut bytes = Vec::with_capacity(identity_before.length as usize);
    file.read_to_end(&mut bytes)
        .map_err(|error| ValidationError {
            kind: ValidationKind::Corrupt,
            message: format!("read recovery journal from stable handle: {error}"),
        })?;
    let identity_after_handle = stable_file_identity(&file).map_err(|error| ValidationError {
        kind: ValidationKind::Corrupt,
        message: format!("re-read recovery journal handle identity: {error}"),
    })?;
    if identity_before != identity_after_handle || bytes.len() as u64 != identity_before.length {
        return Err(ValidationError {
            kind: ValidationKind::IdentityMismatch,
            message: "recovery journal changed while its handle was being read".to_string(),
        });
    }

    let reopened = open_no_follow(path).map_err(|error| ValidationError {
        kind: ValidationKind::IdentityMismatch,
        message: format!("reopen recovery journal for stable identity: {error}"),
    })?;
    let reopened_metadata = reopened.metadata().map_err(|error| ValidationError {
        kind: ValidationKind::IdentityMismatch,
        message: format!("read reopened recovery journal metadata: {error}"),
    })?;
    validate_regular_metadata(&reopened_metadata, max_bytes, "after read")?;
    let identity_after_path = stable_file_identity(&reopened).map_err(|error| ValidationError {
        kind: ValidationKind::IdentityMismatch,
        message: format!("read reopened recovery journal identity: {error}"),
    })?;
    let after = fs::symlink_metadata(path).map_err(|error| ValidationError {
        kind: ValidationKind::IdentityMismatch,
        message: format!("read recovery journal metadata after read: {error}"),
    })?;
    validate_regular_metadata(&after, max_bytes, "after read")?;
    if identity_before != identity_after_path || after.len() != identity_before.length {
        return Err(ValidationError {
            kind: ValidationKind::IdentityMismatch,
            message: "recovery journal path identity changed during validation".to_string(),
        });
    }
    Ok((bytes, identity_before))
}

fn ensure_path_identity(path: &Path, expected: StableFileIdentity) -> io::Result<()> {
    let file = open_no_follow(path)?;
    let actual = stable_file_identity(&file)?;
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "recovery entry path no longer identifies the authenticated file",
        ));
    }
    Ok(())
}

fn validate_regular_metadata(
    metadata: &fs::Metadata,
    max_bytes: u64,
    phase: &str,
) -> Result<(), ValidationError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ValidationError {
            kind: ValidationKind::Corrupt,
            message: format!("recovery journal is not a regular file {phase}"),
        });
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(ValidationError {
                kind: ValidationKind::Corrupt,
                message: format!("recovery journal is a Windows reparse point {phase}"),
            });
        }
    }
    if metadata.len() > max_bytes {
        return Err(ValidationError {
            kind: ValidationKind::Oversized,
            message: format!(
                "recovery journal is {} bytes {phase} (limit {max_bytes})",
                metadata.len()
            ),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn open_no_follow(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(windows)]
fn open_directory_no_follow(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(windows)]
fn lock_recovery_root_directory(_root_handle: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn lock_recovery_root_directory(root_handle: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;

    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    if unsafe { flock(root_handle.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
fn open_exclusive_recovery_lock(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .share_mode(0x0000_0001)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    validate_lock_file_metadata(&metadata)?;
    Ok(file)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_no_follow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0002_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_directory_no_follow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0002_0000;
    const O_DIRECTORY: i32 = 0x0001_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW | O_DIRECTORY)
        .open(path)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_exclusive_recovery_lock(path: &Path) -> io::Result<File> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0002_0000;
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)?;
    validate_lock_file_metadata(&file.metadata()?)?;
    if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(file)
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn open_no_follow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0000_0100;
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn open_directory_no_follow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0000_0100;
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn open_exclusive_recovery_lock(path: &Path) -> io::Result<File> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0000_0100;
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)?;
    validate_lock_file_metadata(&file.metadata()?)?;
    if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(file)
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn open_no_follow(_path: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no-follow recovery journal open is unsupported on this Unix platform",
    ))
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn open_directory_no_follow(_path: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no-follow recovery directory open is unsupported on this Unix platform",
    ))
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn open_exclusive_recovery_lock(_path: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "exclusive recovery directory locking is unsupported on this Unix platform",
    ))
}

fn validate_lock_file_metadata(metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "recovery lock is not a regular file",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "recovery lock is a Windows reparse point",
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn stable_file_identity(file: &File) -> io::Result<StableFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata()?;
    let mut object_id = [0u8; 16];
    object_id[..8].copy_from_slice(&metadata.ino().to_ne_bytes());
    Ok(StableFileIdentity {
        volume_id: metadata.dev(),
        object_id,
        length: metadata.len(),
        modified_high: metadata.mtime() as u64,
        modified_low: metadata.mtime_nsec() as u64,
    })
}

#[cfg(windows)]
fn validate_windows_file_id(file_id: [u8; 16]) -> io::Result<[u8; 16]> {
    if file_id.iter().all(|byte| *byte == 0) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Windows returned an all-zero recovery file identifier",
        ));
    }
    Ok(file_id)
}

#[cfg(windows)]
fn stable_file_identity(file: &File) -> io::Result<StableFileIdentity> {
    use std::ffi::c_void;
    use std::mem::{size_of, MaybeUninit};
    use std::os::windows::io::AsRawHandle;

    const FILE_ID_INFO_CLASS: i32 = 18;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    #[repr(C)]
    struct FileIdInfo {
        volume_serial_number: u64,
        file_id: [u8; 16],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandle(
            file: *mut c_void,
            information: *mut ByHandleFileInformation,
        ) -> i32;
        fn GetFileInformationByHandleEx(
            file: *mut c_void,
            information_class: i32,
            information: *mut c_void,
            buffer_size: u32,
        ) -> i32;
    }

    let raw_handle = file.as_raw_handle();
    let mut information = MaybeUninit::<ByHandleFileInformation>::uninit();
    if unsafe { GetFileInformationByHandle(raw_handle, information.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let information = unsafe { information.assume_init() };

    let mut file_id = MaybeUninit::<FileIdInfo>::uninit();
    if unsafe {
        GetFileInformationByHandleEx(
            raw_handle,
            FILE_ID_INFO_CLASS,
            file_id.as_mut_ptr().cast::<c_void>(),
            size_of::<FileIdInfo>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let file_id = unsafe { file_id.assume_init() };
    let object_id = validate_windows_file_id(file_id.file_id)?;

    Ok(StableFileIdentity {
        volume_id: file_id.volume_serial_number,
        object_id,
        length: (u64::from(information.file_size_high) << 32)
            | u64::from(information.file_size_low),
        modified_high: u64::from(information.last_write_time.high),
        modified_low: u64::from(information.last_write_time.low),
    })
}

fn ensure_durable_directory(path: &Path) -> io::Result<()> {
    validate_existing_directory_ancestors(path)?;
    ensure_durable_directory_inner(path)?;
    validate_existing_directory_ancestors(path)?;
    sync_directory(path)
}

fn ensure_durable_directory_inner(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_directory_metadata(path, &metadata)?;
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        ensure_durable_directory_inner(parent)?;
    }
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path)?;
            validate_directory_metadata(path, &metadata)?;
        }
        Err(error) => return Err(error),
    }
    if let Some(parent) = parent {
        sync_directory(parent)?;
    }
    Ok(())
}

fn validate_existing_directory_ancestors(path: &Path) -> io::Result<()> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let ancestors = absolute.ancestors().collect::<Vec<_>>();
    for ancestor in ancestors.into_iter().rev() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => validate_directory_metadata(ancestor, &metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn validate_directory_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "recovery directory path contains a non-directory or symbolic-link component: {}",
                path.display()
            ),
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "recovery directory path contains a Windows reparse-point component: {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

fn atomic_write_new(path: &Path, bytes: &[u8], directory: &Path) -> io::Result<AtomicWriteReceipt> {
    atomic_write_new_with_callback(path, bytes, directory, |_phase, _temp, _final_path| {})
}

#[cfg(test)]
fn atomic_write_new_with_hook<F>(
    path: &Path,
    bytes: &[u8],
    directory: &Path,
    hook: F,
) -> io::Result<AtomicWriteReceipt>
where
    F: FnMut(AtomicWriteHookPhase, &Path, &Path),
{
    atomic_write_new_with_callback(path, bytes, directory, hook)
}

fn atomic_write_new_with_callback<F>(
    path: &Path,
    bytes: &[u8],
    directory: &Path,
    mut hook: F,
) -> io::Result<AtomicWriteReceipt>
where
    F: FnMut(AtomicWriteHookPhase, &Path, &Path),
{
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non-UTF-8 journal path"))?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = directory.join(format!("{name}.{}.{}.tmp", std::process::id(), sequence));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()?;
    let written_identity = stable_file_identity(&file)?;

    hook(AtomicWriteHookPhase::TempSynced, &temp, path);
    if stable_file_identity(&file)? != written_identity {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "recovery publication handle changed after durable write",
        ));
    }
    ensure_path_identity(&temp, written_identity).map_err(|error| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "recovery publication temp no longer identifies the written handle; evidence retained: {error}"
            ),
        )
    })?;

    durable_rename(&temp, path, false)?;
    hook(AtomicWriteHookPhase::Published, &temp, path);
    let published = open_no_follow(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("reopen published recovery object; outcome is unknown: {error}"),
        )
    })?;
    validate_lock_file_metadata(&published.metadata()?)?;
    if stable_file_identity(&published)? != written_identity {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "published recovery path does not identify the written handle; evidence retained",
        ));
    }
    sync_directory(directory)?;
    let durable = open_no_follow(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("reopen directory-durable recovery object; outcome is unknown: {error}"),
        )
    })?;
    if stable_file_identity(&durable)? != written_identity {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "directory-durable recovery path changed before publication receipt; evidence retained",
        ));
    }
    Ok(AtomicWriteReceipt {
        published_identity: written_identity,
    })
}

#[cfg(windows)]
fn sync_directory(path: &Path) -> io::Result<()> {
    let marker = path.join(DIRECTORY_DURABILITY_MARKER);
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = path.join(format!(
        ".mir2-directory-durability.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    let publish_result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(b"mir2-directory-durability-v1\n")?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        durable_rename(&temp, &marker, true)
    })();
    match publish_result {
        Ok(()) => Ok(()),
        Err(publish_error) => Err(io::Error::new(
            publish_error.kind(),
            format!(
                "directory durability publish failed ({publish_error}); temp evidence was retained at {}",
                temp.display()
            ),
        )),
    }
}

#[cfg(not(windows))]
fn sync_directory(path: &Path) -> io::Result<()> {
    OpenOptions::new().read(true).open(path)?.sync_all()
}

#[cfg(windows)]
fn durable_rename(from: &Path, to: &Path, replace_existing: bool) -> io::Result<()> {
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    let existing = windows_extended_path(from)?;
    let new = windows_extended_path(to)?;
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace_existing {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    let moved = unsafe { MoveFileExW(existing.as_ptr(), new.as_ptr(), flags) };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn windows_extended_path(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let wide = absolute.as_os_str().encode_wide().collect::<Vec<_>>();
    let mut extended = if wide.starts_with(&[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16])
    {
        wide
    } else if wide.starts_with(&[b'\\' as u16, b'\\' as u16]) {
        let mut value = "\\\\?\\UNC\\".encode_utf16().collect::<Vec<_>>();
        value.extend_from_slice(&wide[2..]);
        value
    } else {
        let mut value = "\\\\?\\".encode_utf16().collect::<Vec<_>>();
        value.extend_from_slice(&wide);
        value
    };
    extended.push(0);
    Ok(extended)
}

#[cfg(target_os = "linux")]
fn durable_rename(from: &Path, to: &Path, replace_existing: bool) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    if replace_existing {
        return fs::rename(from, to);
    }

    const AT_FDCWD: i32 = -100;
    const RENAME_NOREPLACE: u32 = 1;
    extern "C" {
        fn renameat2(
            old_directory: i32,
            old_path: *const i8,
            new_directory: i32,
            new_path: *const i8,
            flags: u32,
        ) -> i32;
    }

    let old_path = CString::new(from.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "recovery publication source path contains an embedded NUL",
        )
    })?;
    let new_path = CString::new(to.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "recovery publication destination path contains an embedded NUL",
        )
    })?;
    if unsafe {
        renameat2(
            AT_FDCWD,
            old_path.as_ptr(),
            AT_FDCWD,
            new_path.as_ptr(),
            RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "linux")))]
fn durable_rename(from: &Path, to: &Path, replace_existing: bool) -> io::Result<()> {
    if replace_existing {
        return fs::rename(from, to);
    }
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace recovery publication is unsupported on this Unix platform",
    ))
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
#[path = "save_recovery_tests.rs"]
mod tests;

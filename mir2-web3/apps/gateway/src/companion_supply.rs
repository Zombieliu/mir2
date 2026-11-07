//! Server-owned, conservatively reserved companion supply permissions.
//!
//! This journal never executes an economic action. The caller must hold its
//! single-writer guard across reserve -> authoritative bounded buy -> resolve.
//! A reservation is persisted before a buy is allowed; an unknown result never
//! releases it or permits another operation. Repeated reservations are QUERY
//! ONLY, even if the game has not produced a receipt yet.

use crate::companion_relay::{CharacterBinding, Enrollment};
use hmac::{Hmac, KeyInit, Mac};
use mir2_simulation::npc_purchase_journal::NpcPurchaseOperation;
use mir2_simulation::{NpcPurchaseCurrency, NpcPurchaseProcessingOutcome, NpcPurchaseSource};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[path = "companion_life.rs"]
mod life;
pub use life::{LifeGrant, LifeProjectionHandle, LifeScope, LifeScopeProjection, LifeScopeStatus};
use life::{LifeRecord, LifeResumeRecord, LifeRevocationRecord};

const SCHEMA: u32 = 1;
const RECORD_CAP: usize = 2_048;
const MAX_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SAFE_MS: u64 = 9_007_199_254_740_991;
const MAC_DOMAIN: &[u8] = b"mir2/companion-supply-journal/v1\0";
const MARKER_DOMAIN: &[u8] = b"mir2/companion-supply-initialized/v1\0";
type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplyError {
    Invalid,
    Conflict,
    Missing,
    Expired,
    Revoked,
    AdmissionChanged,
    Unresolved,
    Capacity,
    Integrity,
    Unavailable,
}
impl fmt::Display for SupplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Invalid => "Invalid bounded supply request.",
            Self::Conflict => "Supply identity or operation conflicts with its original record.",
            Self::Missing => "No matching supply permission exists.",
            Self::Expired => "Supply permission has expired.",
            Self::Revoked => "Supply permission has been revoked.",
            Self::AdmissionChanged => "Supply permission belongs to a retired actor admission.",
            Self::Unresolved => "The original supply outcome must be reconciled first.",
            Self::Capacity => {
                "Supply journal capacity reached; retained records cannot be evicted."
            }
            Self::Integrity => "Supply journal integrity or world binding is unavailable.",
            Self::Unavailable => "Supply journal publication is unavailable; execution is refused.",
        })
    }
}
impl std::error::Error for SupplyError {}
type Result<T> = std::result::Result<T, SupplyError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyPoint {
    pub x: i32,
    pub y: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}
impl SupplyBounds {
    fn village() -> Self {
        Self {
            min_x: 270,
            min_y: 595,
            max_x: 310,
            max_y: 635,
        }
    }
    fn contains(self, p: SupplyPoint) -> bool {
        (self.min_x..=self.max_x).contains(&p.x) && (self.min_y..=self.max_y).contains(&p.y)
    }
}

/// Safe public result. Account, admission, HMAC and private journal fields are
/// deliberately absent, and this value alone is not an executable capability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyGrant {
    pub permit_id: String,
    pub world_epoch: String,
    pub actor_character_index: i32,
    pub map_file_name: String,
    pub npc_index: u32,
    pub npc_name: String,
    pub npc_position: SupplyPoint,
    pub item_index: i32,
    pub item_name: String,
    pub quantity: u16,
    pub max_gold: u32,
    pub expires_at_ms: u64,
    pub return_anchor: SupplyPoint,
    pub bounds: SupplyBounds,
}

pub struct SupplyReservation {
    pub grant: SupplyGrant,
    /// True means query the original operation; do not initiate a buy again.
    pub already_reserved: bool,
    pub known_outcome: Option<NpcPurchaseProcessingOutcome>,
}
impl fmt::Debug for SupplyReservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupplyReservation")
            .field("permit_id", &self.grant.permit_id)
            .field("already_reserved", &self.already_reserved)
            .field("outcome_known", &self.known_outcome.is_some())
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PermitRecord {
    owner_request_key: String,
    owner: CharacterBinding,
    actor: CharacterBinding,
    actor_admission_id: String,
    resident_id: String,
    archive_id: String,
    authorized_at_ms: u64,
    grant: SupplyGrant,
    revoked_at_ms: Option<u64>,
    reserved_gold: u32,
    operation: Option<NpcPurchaseOperation>,
    outcome: Option<NpcPurchaseProcessingOutcome>,
    // Legacy records were issued by the one-phase authorize API. Omitting
    // their default true value preserves their original canonical HMAC bytes.
    #[serde(default = "legacy_activated", skip_serializing_if = "is_true")]
    activated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prepared_boot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_life_permit_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    life_execution_boot_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    life_execution_retired: bool,
}
fn legacy_activated() -> bool {
    true
}
fn is_true(value: &bool) -> bool {
    *value
}
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RevocationRecord {
    owner_request_key: String,
    owner: CharacterBinding,
    resident_id: String,
    recorded_at_ms: u64,
    revoked_permit_ids: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    schema_version: u32,
    realm_id: String,
    world_epoch: String,
    journal_id: String,
    generation: u64,
    records: Vec<PermitRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    revocations: Vec<RevocationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lives: Vec<LifeRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    life_resumptions: Vec<LifeResumeRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    life_revocations: Vec<LifeRevocationRecord>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope<T> {
    document: T,
    mac: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Marker {
    schema_version: u32,
    realm_id: String,
    world_epoch: String,
    journal_id: String,
    generation: u64,
    document_mac: String,
}

/// No Debug implementation: neither the key nor private identity/operation
/// records may be accidentally included in a diagnostic.
pub struct SupplyLedger {
    path: PathBuf,
    key: [u8; 32],
    document: Document,
    document_mac: String,
    poisoned: Cell<bool>,
    _file_lock: File,
    // Drafts can be activated only in the ledger instance that prepared them.
    // A process/reopen boundary cannot silently convert a pending reply into
    // fresh authority, even if a caller mistakenly reuses an admission string.
    boot_id: String,
    projection: LifeProjectionHandle,
    #[cfg(test)]
    fail_next_publish: Option<bool>,
    #[cfg(test)]
    fail_after_head: bool,
}

fn reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._:-".contains(&c))
}
fn private_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && value.bytes().all(|c| (0x21..=0x7e).contains(&c))
}
fn character(value: &CharacterBinding) -> bool {
    !value.account_id.is_empty()
        && value.account_id.len() <= 160
        && value.account_id.trim() == value.account_id
        && !value.account_id.chars().any(char::is_control)
        && value.character_index >= 0
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 15) as usize] as char);
    }
    out
}
fn unhex(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(SupplyError::Integrity);
    }
    let mut out = [0; 32];
    let nibble = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
    for (i, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        out[i] = nibble(pair[0]) * 16 + nibble(pair[1]);
    }
    Ok(out)
}
fn random_reference(prefix: &str) -> Result<String> {
    let mut bytes = [0; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| SupplyError::Unavailable)?;
    if bytes == [0; 32] {
        return Err(SupplyError::Unavailable);
    }
    Ok(format!("{prefix}{}", hex(&bytes)))
}
fn suffix(path: &Path, value: &str) -> Result<PathBuf> {
    let name = path.file_name().ok_or(SupplyError::Invalid)?;
    let mut next = name.to_os_string();
    next.push(value);
    Ok(path.with_file_name(next))
}
fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(SupplyError::Integrity)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(SupplyError::Unavailable),
    }
}
fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    reject_symlink(path)?;
    let file = File::open(path).map_err(|_| SupplyError::Integrity)?;
    if file.metadata().map_err(|_| SupplyError::Integrity)?.len() > MAX_BYTES {
        return Err(SupplyError::Integrity);
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SupplyError::Integrity)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_BYTES {
        return Err(SupplyError::Integrity);
    }
    Ok(bytes)
}
fn sign<T: Serialize>(value: &T, key: &[u8; 32], domain: &[u8]) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|_| SupplyError::Integrity)?;
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| SupplyError::Invalid)?;
    mac.update(domain);
    mac.update(&bytes);
    Ok(hex(&mac.finalize().into_bytes()))
}
fn verify<T: Serialize>(value: &T, signature: &str, key: &[u8; 32], domain: &[u8]) -> Result<()> {
    let signature = unhex(signature)?;
    let bytes = serde_json::to_vec(value).map_err(|_| SupplyError::Integrity)?;
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| SupplyError::Invalid)?;
    mac.update(domain);
    mac.update(&bytes);
    mac.verify_slice(&signature)
        .map_err(|_| SupplyError::Integrity)
}

fn valid_operation(operation: &NpcPurchaseOperation) -> bool {
    operation.actor != [0; 32]
        && operation.request_scope != [0; 32]
        && operation.sequence > 0
        && operation.intent.service_catalog_proof != [0; 32]
        && operation.intent.currency == NpcPurchaseCurrency::Gold
        && operation.intent.source == NpcPurchaseSource::Trade
        && operation.intent.request.count == 1
        && operation.intent.request.panel_type == 0
}
fn operation_key(a: &NpcPurchaseOperation, b: &NpcPurchaseOperation) -> bool {
    a.actor == b.actor && a.request_scope == b.request_scope && a.sequence == b.sequence
}
fn valid_outcome(record: &PermitRecord, outcome: &NpcPurchaseProcessingOutcome) -> bool {
    let Some(operation) = record.operation else {
        return false;
    };
    match outcome {
        NpcPurchaseProcessingOutcome::Committed {
            request,
            currency,
            source,
            charged,
            admitted_count,
            incoming_unique_id,
        } => {
            *request == operation.intent.request
                && *currency == NpcPurchaseCurrency::Gold
                && *source == NpcPurchaseSource::Trade
                && *charged <= record.grant.max_gold
                && *admitted_count == 1
                && *incoming_unique_id != 0
        }
        NpcPurchaseProcessingOutcome::Rejected { request, .. } => {
            *request == operation.intent.request
        }
    }
}
fn valid_document(document: &Document, realm: &str, epoch: &str, journal_id: &str) -> Result<()> {
    if document.schema_version != SCHEMA
        || document.realm_id != realm
        || document.world_epoch != epoch
        || document.journal_id != journal_id
        || !reference(journal_id)
        || life::record_count(document) > RECORD_CAP
        || document.generation < life::record_count(document) as u64
    {
        return Err(SupplyError::Integrity);
    }
    let mut requests = BTreeSet::new();
    let mut permits = BTreeSet::new();
    let mut operations = BTreeSet::new();
    for record in &document.records {
        let grant = &record.grant;
        if !character(&record.owner)
            || !character(&record.actor)
            || record.owner == record.actor
            || !reference(&record.resident_id)
            || !reference(&record.archive_id)
            || !private_reference(&record.actor_admission_id)
            || !private_reference(&record.owner_request_key)
            || !requests.insert((record.owner.clone(), record.owner_request_key.clone()))
            || !permits.insert(&grant.permit_id)
            || !reference(&grant.permit_id)
            || grant.world_epoch != epoch
            || grant.actor_character_index != record.actor.character_index
            || grant.map_file_name != "0"
            || grant.npc_index != 9
            || grant.npc_name != "Merchant_Ruben"
            || grant.npc_position != (SupplyPoint { x: 288, y: 608 })
            || grant.item_index != 658
            || grant.item_name != "(HP)DrugSmall"
            || grant.quantity != 1
            || !(1..=50).contains(&grant.max_gold)
            || grant.bounds != SupplyBounds::village()
            || !grant.bounds.contains(grant.return_anchor)
            || record.authorized_at_ms > MAX_SAFE_MS
            || record.authorized_at_ms.checked_add(300_000) != Some(grant.expires_at_ms)
            || grant.expires_at_ms > MAX_SAFE_MS
            || record
                .revoked_at_ms
                .is_some_and(|v| v < record.authorized_at_ms || v > MAX_SAFE_MS)
            || record.operation.is_none() && (record.reserved_gold != 0 || record.outcome.is_some())
            || record.operation.is_some() && record.reserved_gold != grant.max_gold
            || !record.activated
                && (record.prepared_boot_id.is_none() || record.operation.is_some())
            || record
                .prepared_boot_id
                .as_ref()
                .is_some_and(|value| !reference(value))
        {
            return Err(SupplyError::Integrity);
        }
        if let Some(operation) = &record.operation {
            if !valid_operation(operation)
                || !operations.insert((
                    operation.actor,
                    operation.request_scope,
                    operation.sequence,
                ))
            {
                return Err(SupplyError::Integrity);
            }
        }
        if record
            .outcome
            .as_ref()
            .is_some_and(|outcome| !valid_outcome(record, outcome))
        {
            return Err(SupplyError::Integrity);
        }
    }
    let mut cancellation_keys = BTreeSet::new();
    let mut cancelled_permits = BTreeSet::new();
    for cancellation in &document.revocations {
        if !character(&cancellation.owner)
            || !private_reference(&cancellation.owner_request_key)
            || !reference(&cancellation.resident_id)
            || cancellation.recorded_at_ms > MAX_SAFE_MS
            || !cancellation_keys.insert((
                cancellation.owner.clone(),
                cancellation.owner_request_key.clone(),
            ))
            || requests.contains(&(
                cancellation.owner.clone(),
                cancellation.owner_request_key.clone(),
            ))
        {
            return Err(SupplyError::Integrity);
        }
        for permit_id in &cancellation.revoked_permit_ids {
            if !reference(permit_id) || !cancelled_permits.insert(permit_id) {
                return Err(SupplyError::Integrity);
            }
            let Some(record) = document
                .records
                .iter()
                .find(|record| &record.grant.permit_id == permit_id)
            else {
                return Err(SupplyError::Integrity);
            };
            if record.owner != cancellation.owner
                || record.resident_id != cancellation.resident_id
                || record.revoked_at_ms != Some(cancellation.recorded_at_ms)
            {
                return Err(SupplyError::Integrity);
            }
        }
    }
    life::validate_life_document(document)?;
    Ok(())
}

impl SupplyLedger {
    /// Both path and key are trusted configuration supplied by the host. A
    /// missing journal alongside its initialization marker is never recreated.
    pub fn open(
        path: PathBuf,
        key: [u8; 32],
        realm_id: String,
        world_epoch: String,
    ) -> Result<Self> {
        if key == [0; 32] || !reference(&realm_id) || !reference(&world_epoch) {
            return Err(SupplyError::Invalid);
        }
        let path = std::path::absolute(path).map_err(|_| SupplyError::Invalid)?;
        let parent = path.parent().ok_or(SupplyError::Invalid)?;
        fs::create_dir_all(parent).map_err(|_| SupplyError::Unavailable)?;
        let lock_path = suffix(&path, ".lock")?;
        reject_symlink(&lock_path)?;
        let file_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|_| SupplyError::Unavailable)?;
        file_lock.try_lock().map_err(|_| SupplyError::Unavailable)?;
        let marker_path = suffix(&path, ".initialized")?;
        reject_symlink(&path)?;
        reject_symlink(&marker_path)?;
        let exists = path.try_exists().map_err(|_| SupplyError::Unavailable)?;
        let marked = marker_path
            .try_exists()
            .map_err(|_| SupplyError::Unavailable)?;
        let (document, document_mac) = if exists || marked {
            if !exists || !marked {
                return Err(SupplyError::Integrity);
            }
            let marker: Envelope<Marker> = serde_json::from_slice(&read_bounded(&marker_path)?)
                .map_err(|_| SupplyError::Integrity)?;
            verify(&marker.document, &marker.mac, &key, MARKER_DOMAIN)?;
            if marker.document.schema_version != SCHEMA
                || marker.document.realm_id != realm_id
                || marker.document.world_epoch != world_epoch
                || !reference(&marker.document.journal_id)
            {
                return Err(SupplyError::Integrity);
            }
            let envelope: Envelope<Document> = serde_json::from_slice(&read_bounded(&path)?)
                .map_err(|_| SupplyError::Integrity)?;
            verify(&envelope.document, &envelope.mac, &key, MAC_DOMAIN)?;
            if marker.document.generation != envelope.document.generation
                || marker.document.document_mac != envelope.mac
            {
                return Err(SupplyError::Integrity);
            }
            valid_document(
                &envelope.document,
                &realm_id,
                &world_epoch,
                &marker.document.journal_id,
            )?;
            (envelope.document, envelope.mac)
        } else {
            let journal_id = random_reference("journal:")?;
            let document = Document {
                schema_version: SCHEMA,
                realm_id,
                world_epoch,
                journal_id,
                generation: 0,
                records: Vec::new(),
                revocations: Vec::new(),
                lives: Vec::new(),
                life_resumptions: Vec::new(),
                life_revocations: Vec::new(),
            };
            let mac = sign(&document, &key, MAC_DOMAIN)?;
            let marker = Marker {
                schema_version: SCHEMA,
                realm_id: document.realm_id.clone(),
                world_epoch: document.world_epoch.clone(),
                journal_id: document.journal_id.clone(),
                generation: document.generation,
                document_mac: mac.clone(),
            };
            // The signed head is durable first. A crash before genesis
            // publication blocks future opens rather than initializing anew.
            publish(
                &marker_path,
                &Envelope {
                    mac: sign(&marker, &key, MARKER_DOMAIN)?,
                    document: marker,
                },
                false,
            )?;
            publish(
                &path,
                &Envelope {
                    document: document.clone(),
                    mac: mac.clone(),
                },
                false,
            )?;
            (document, mac)
        };
        let boot_id = random_reference("boot:")?;
        let projection = life::new_projection(&document, &boot_id);
        Ok(Self {
            path,
            key,
            document,
            document_mac,
            poisoned: Cell::new(false),
            _file_lock: file_lock,
            boot_id,
            projection,
            #[cfg(test)]
            fail_next_publish: None,
            #[cfg(test)]
            fail_after_head: false,
        })
    }

    fn ready(&self) -> Result<()> {
        if self.poisoned.get() {
            return Err(SupplyError::Unavailable);
        }
        let checked = (|| {
            let marker: Envelope<Marker> =
                serde_json::from_slice(&read_bounded(&suffix(&self.path, ".initialized")?)?)
                    .map_err(|_| SupplyError::Integrity)?;
            verify(&marker.document, &marker.mac, &self.key, MARKER_DOMAIN)?;
            if marker.document.schema_version != SCHEMA
                || marker.document.realm_id != self.document.realm_id
                || marker.document.world_epoch != self.document.world_epoch
                || marker.document.journal_id != self.document.journal_id
                || marker.document.generation != self.document.generation
                || marker.document.document_mac != self.document_mac
            {
                return Err(SupplyError::Integrity);
            }
            let envelope: Envelope<Document> = serde_json::from_slice(&read_bounded(&self.path)?)
                .map_err(|_| SupplyError::Integrity)?;
            verify(&envelope.document, &envelope.mac, &self.key, MAC_DOMAIN)?;
            if envelope.mac != self.document_mac
                || envelope.document.generation != self.document.generation
            {
                return Err(SupplyError::Integrity);
            }
            Ok(())
        })();
        if checked.is_err() {
            self.poison();
        }
        checked
    }
    fn commit(&mut self, mut next: Document) -> Result<()> {
        self.ready()?;
        next.generation = self
            .document
            .generation
            .checked_add(1)
            .ok_or(SupplyError::Capacity)?;
        valid_document(&next, &next.realm_id, &next.world_epoch, &next.journal_id)?;
        let mac = sign(&next, &self.key, MAC_DOMAIN)?;
        #[cfg(test)]
        if self.fail_next_publish == Some(false) {
            self.fail_next_publish = None;
            self.poison();
            return Err(SupplyError::Unavailable);
        }
        // Advance the authenticated head first. If publication is interrupted
        // between the two files, startup refuses the mismatch. An older valid
        // document cannot quietly erase a reservation during recovery.
        let marker = Marker {
            schema_version: SCHEMA,
            realm_id: next.realm_id.clone(),
            world_epoch: next.world_epoch.clone(),
            journal_id: next.journal_id.clone(),
            generation: next.generation,
            document_mac: mac.clone(),
        };
        let marker_path = suffix(&self.path, ".initialized")?;
        let marker_envelope = Envelope {
            mac: sign(&marker, &self.key, MARKER_DOMAIN)?,
            document: marker,
        };
        if publish(&marker_path, &marker_envelope, true).is_err() {
            self.poison();
            return Err(SupplyError::Unavailable);
        }
        #[cfg(test)]
        if self.fail_after_head {
            self.fail_after_head = false;
            self.poison();
            return Err(SupplyError::Unavailable);
        }
        let result = publish(
            &self.path,
            &Envelope {
                document: next.clone(),
                mac: mac.clone(),
            },
            true,
        );
        if result.is_err() {
            self.poison();
            return Err(SupplyError::Unavailable);
        }
        #[cfg(test)]
        if self.fail_next_publish == Some(true) {
            self.fail_next_publish = None;
            self.poison();
            return Err(SupplyError::Unavailable);
        }
        self.document = next;
        self.document_mac = mac;
        self.refresh_projection()?;
        Ok(())
    }

    pub fn authorize(
        &mut self,
        owner_request_key: &str,
        enrollment: &Enrollment,
        actor_admission_id: &str,
        return_anchor: SupplyPoint,
        now_ms: u64,
        max_gold: u32,
        expires_in_seconds: u32,
    ) -> Result<SupplyGrant> {
        self.issue(
            owner_request_key,
            enrollment,
            actor_admission_id,
            return_anchor,
            now_ms,
            max_gold,
            expires_in_seconds,
            true,
        )
    }

    /// Prepare a durable draft only. A runtime conflict, refusal, or timeout
    /// cannot revoke or displace an existing usable grant.
    pub fn prepare(
        &mut self,
        owner_request_key: &str,
        enrollment: &Enrollment,
        actor_admission_id: &str,
        return_anchor: SupplyPoint,
        now_ms: u64,
        max_gold: u32,
        expires_in_seconds: u32,
    ) -> Result<SupplyGrant> {
        self.issue(
            owner_request_key,
            enrollment,
            actor_admission_id,
            return_anchor,
            now_ms,
            max_gold,
            expires_in_seconds,
            false,
        )
    }

    fn issue(
        &mut self,
        owner_request_key: &str,
        enrollment: &Enrollment,
        actor_admission_id: &str,
        return_anchor: SupplyPoint,
        now_ms: u64,
        max_gold: u32,
        expires_in_seconds: u32,
        activated: bool,
    ) -> Result<SupplyGrant> {
        self.ready()?;
        if !private_reference(owner_request_key)
            || !private_reference(actor_admission_id)
            || !character(&enrollment.owner)
            || !character(&enrollment.actor)
            || enrollment.owner == enrollment.actor
            || !reference(&enrollment.resident_id)
            || !reference(&enrollment.archive_id)
            || !SupplyBounds::village().contains(return_anchor)
            || !(1..=50).contains(&max_gold)
            || expires_in_seconds != 300
            || now_ms
                .checked_add(300_000)
                .is_none_or(|end| end > MAX_SAFE_MS)
        {
            return Err(SupplyError::Invalid);
        }
        if self.document.revocations.iter().any(|record| {
            record.owner == enrollment.owner && record.owner_request_key == owner_request_key
        }) {
            return Err(SupplyError::Conflict);
        }
        self.reject_life_request_key(&enrollment.owner, owner_request_key)?;
        if let Some(prior) = self.document.records.iter().find(|record| {
            record.owner == enrollment.owner && record.owner_request_key == owner_request_key
        }) {
            if prior.actor != enrollment.actor
                || prior.resident_id != enrollment.resident_id
                || prior.archive_id != enrollment.archive_id
                || prior.grant.max_gold != max_gold
            {
                return Err(SupplyError::Conflict);
            }
            // Deliberately ignore the new clock, position and admission: a
            // duplicate request cannot refresh an old grant or its budget.
            return Ok(prior.grant.clone());
        }
        self.ensure_standalone_allowed(&enrollment.actor, now_ms)?;
        if life::record_count(&self.document) >= RECORD_CAP {
            return Err(SupplyError::Capacity);
        }
        if self.document.records.iter().any(|record| {
            record.actor == enrollment.actor
                && record.operation.is_some()
                && record.outcome.is_none()
        }) {
            return Err(SupplyError::Unresolved);
        }
        let grant = SupplyGrant {
            permit_id: random_reference("supply:")?,
            world_epoch: self.document.world_epoch.clone(),
            actor_character_index: enrollment.actor.character_index,
            map_file_name: "0".into(),
            npc_index: 9,
            npc_name: "Merchant_Ruben".into(),
            npc_position: SupplyPoint { x: 288, y: 608 },
            item_index: 658,
            item_name: "(HP)DrugSmall".into(),
            quantity: 1,
            max_gold,
            expires_at_ms: now_ms + 300_000,
            return_anchor,
            bounds: SupplyBounds::village(),
        };
        let mut next = self.document.clone();
        for record in &mut next.records {
            if activated
                && record.activated
                && record.actor == enrollment.actor
                && record.revoked_at_ms.is_none()
            {
                record.revoked_at_ms = Some(now_ms.max(record.authorized_at_ms));
            }
        }
        next.records.push(PermitRecord {
            owner_request_key: owner_request_key.into(),
            owner: enrollment.owner.clone(),
            actor: enrollment.actor.clone(),
            actor_admission_id: actor_admission_id.into(),
            resident_id: enrollment.resident_id.clone(),
            archive_id: enrollment.archive_id.clone(),
            authorized_at_ms: now_ms,
            grant: grant.clone(),
            revoked_at_ms: None,
            reserved_gold: 0,
            operation: None,
            outcome: None,
            activated,
            prepared_boot_id: (!activated).then(|| self.boot_id.clone()),
            parent_life_permit_id: None,
            life_execution_boot_id: None,
            life_execution_retired: false,
        });
        self.commit(next)?;
        Ok(grant)
    }

    /// The authenticated relay invokes this only after the matching runtime
    /// accepted the expected policy version. Repeated ACKs cannot refresh a
    /// grant or revive a previously revoked/expired grant.
    pub fn activate(
        &mut self,
        owner_request_key: &str,
        actor: &CharacterBinding,
        current_admission_id: &str,
        permit_id: &str,
        now_ms: u64,
    ) -> Result<SupplyGrant> {
        self.ready()?;
        if !private_reference(owner_request_key)
            || !character(actor)
            || !private_reference(current_admission_id)
            || !reference(permit_id)
            || now_ms > MAX_SAFE_MS
        {
            return Err(SupplyError::Invalid);
        }
        let index = self
            .document
            .records
            .iter()
            .position(|record| record.grant.permit_id == permit_id)
            .ok_or(SupplyError::Missing)?;
        let record = &self.document.records[index];
        if record.owner_request_key != owner_request_key || &record.actor != actor {
            return Err(SupplyError::Conflict);
        }
        if record.actor_admission_id != current_admission_id {
            return Err(SupplyError::AdmissionChanged);
        }
        if record.activated {
            return Ok(record.grant.clone());
        }
        if record.parent_life_permit_id.is_none() {
            self.ensure_standalone_allowed(actor, now_ms)?;
        }
        if record.prepared_boot_id.as_deref() != Some(self.boot_id.as_str()) {
            return Err(SupplyError::AdmissionChanged);
        }
        // current also checks expiry/revocation, but a draft is intentionally
        // not considered active by ordinary purchase gates.
        if now_ms < record.authorized_at_ms {
            return Err(SupplyError::Invalid);
        }
        if record.revoked_at_ms.is_some() {
            return Err(SupplyError::Revoked);
        }
        if now_ms >= record.grant.expires_at_ms {
            return Err(SupplyError::Expired);
        }
        if self.document.records.iter().any(|prior| {
            &prior.actor == actor && prior.operation.is_some() && prior.outcome.is_none()
        }) {
            return Err(SupplyError::Unresolved);
        }
        if self.document.records[index + 1..]
            .iter()
            .any(|later| &later.actor == actor && later.activated)
        {
            return Err(SupplyError::Conflict);
        }
        let grant = record.grant.clone();
        let mut next = self.document.clone();
        for (prior_index, prior) in next.records.iter_mut().enumerate() {
            if prior_index != index
                && prior.activated
                && &prior.actor == actor
                && prior.revoked_at_ms.is_none()
            {
                prior.revoked_at_ms = Some(now_ms.max(prior.authorized_at_ms));
            }
        }
        next.records[index].activated = true;
        self.commit(next)?;
        Ok(grant)
    }

    /// Cancel only the matching unactivated draft. This is used for a failed
    /// runtime reply and must never cancel a previous or later active grant.
    pub fn abandon_prepared(
        &mut self,
        owner_request_key: &str,
        actor: &CharacterBinding,
        permit_id: &str,
        now_ms: u64,
    ) -> Result<bool> {
        self.ready()?;
        if !private_reference(owner_request_key)
            || !character(actor)
            || !reference(permit_id)
            || now_ms > MAX_SAFE_MS
        {
            return Err(SupplyError::Invalid);
        }
        let Some(index) = self
            .document
            .records
            .iter()
            .position(|record| record.grant.permit_id == permit_id)
        else {
            return Ok(false);
        };
        let record = &self.document.records[index];
        if record.owner_request_key != owner_request_key || &record.actor != actor {
            return Err(SupplyError::Conflict);
        }
        if record.activated || record.revoked_at_ms.is_some() {
            return Ok(false);
        }
        if now_ms < record.authorized_at_ms {
            return Err(SupplyError::Invalid);
        }
        let mut next = self.document.clone();
        next.records[index].revoked_at_ms = Some(now_ms);
        self.commit(next)?;
        Ok(true)
    }

    /// Public view only; does not authorize any expenditure.
    pub fn grant_for_actor(&self, actor: &CharacterBinding) -> Result<Option<SupplyGrant>> {
        self.ready()?;
        Ok(self
            .document
            .records
            .iter()
            .rev()
            .find(|record| &record.actor == actor && record.activated)
            .map(|record| record.grant.clone()))
    }
    pub fn active_grant(
        &self,
        actor: &CharacterBinding,
        actor_admission_id: &str,
        now_ms: u64,
    ) -> Result<Option<SupplyGrant>> {
        self.ready()?;
        let Some(record) = self
            .document
            .records
            .iter()
            .rev()
            .find(|record| &record.actor == actor && record.activated)
        else {
            return Ok(None);
        };
        Self::current(record, actor_admission_id, now_ms)?;
        self.current_life_child(record, actor_admission_id, now_ms)?;
        Ok(Some(record.grant.clone()))
    }
    fn current(record: &PermitRecord, admission: &str, now_ms: u64) -> Result<()> {
        if !record.activated {
            return Err(SupplyError::Missing);
        }
        if now_ms > MAX_SAFE_MS || now_ms < record.authorized_at_ms {
            return Err(SupplyError::Invalid);
        }
        if record.actor_admission_id != admission {
            return Err(SupplyError::AdmissionChanged);
        }
        if record.revoked_at_ms.is_some() {
            return Err(SupplyError::Revoked);
        }
        if now_ms >= record.grant.expires_at_ms {
            return Err(SupplyError::Expired);
        }
        Ok(())
    }

    pub fn reserve(
        &mut self,
        actor: &CharacterBinding,
        actor_admission_id: &str,
        operation: NpcPurchaseOperation,
        now_ms: u64,
    ) -> Result<SupplyReservation> {
        self.ready()?;
        if !character(actor)
            || !private_reference(actor_admission_id)
            || !valid_operation(&operation)
        {
            return Err(SupplyError::Invalid);
        }
        if let Some(prior) = self.document.records.iter().find(|record| {
            record
                .operation
                .as_ref()
                .is_some_and(|old| operation_key(old, &operation))
        }) {
            if &prior.actor != actor || prior.operation != Some(operation) {
                return Err(SupplyError::Conflict);
            }
            if prior.actor_admission_id != actor_admission_id {
                return Err(SupplyError::AdmissionChanged);
            }
            self.current_life_child(prior, actor_admission_id, now_ms)?;
            return Ok(SupplyReservation {
                grant: prior.grant.clone(),
                already_reserved: true,
                known_outcome: prior.outcome.clone(),
            });
        }
        let index = self
            .document
            .records
            .iter()
            .rposition(|record| &record.actor == actor && record.activated)
            .ok_or(SupplyError::Missing)?;
        let record = &self.document.records[index];
        Self::current(record, actor_admission_id, now_ms)?;
        self.current_life_child(record, actor_admission_id, now_ms)?;
        if record.operation.is_some() {
            return Err(SupplyError::Conflict);
        }
        if self.document.records.iter().any(|prior| {
            &prior.actor == actor && prior.operation.is_some() && prior.outcome.is_none()
        }) {
            return Err(SupplyError::Unresolved);
        }
        let grant = record.grant.clone();
        let mut next = self.document.clone();
        next.records[index].operation = Some(operation);
        next.records[index].reserved_gold = grant.max_gold;
        self.commit(next)?;
        Ok(SupplyReservation {
            grant,
            already_reserved: false,
            known_outcome: None,
        })
    }

    /// Read-only recovery deliberately ignores expiry, revocation and the old
    /// admission. Its authenticated actor must still match the exact record.
    pub fn query_allowed(
        &self,
        actor: &CharacterBinding,
        operation: NpcPurchaseOperation,
    ) -> Result<SupplyGrant> {
        self.ready()?;
        let record = self
            .document
            .records
            .iter()
            .find(|record| {
                record
                    .operation
                    .as_ref()
                    .is_some_and(|old| operation_key(old, &operation))
            })
            .ok_or(SupplyError::Missing)?;
        if &record.actor != actor || record.operation != Some(operation) {
            return Err(SupplyError::Conflict);
        }
        Ok(record.grant.clone())
    }

    pub fn resolve(
        &mut self,
        actor: &CharacterBinding,
        operation: NpcPurchaseOperation,
        outcome: &NpcPurchaseProcessingOutcome,
    ) -> Result<()> {
        self.ready()?;
        self.query_allowed(actor, operation)?;
        let index = self
            .document
            .records
            .iter()
            .position(|record| record.operation == Some(operation))
            .ok_or(SupplyError::Missing)?;
        let record = &self.document.records[index];
        if !valid_outcome(record, outcome) {
            return Err(SupplyError::Invalid);
        }
        if let Some(prior) = &record.outcome {
            return if prior == outcome {
                Ok(())
            } else {
                Err(SupplyError::Conflict)
            };
        }
        let mut next = self.document.clone();
        next.records[index].outcome = Some(outcome.clone());
        self.commit(next)
    }

    pub fn revoke(
        &mut self,
        owner: &CharacterBinding,
        resident_id: &str,
        now_ms: u64,
    ) -> Result<usize> {
        self.ready()?;
        if !character(owner) || !reference(resident_id) || now_ms > MAX_SAFE_MS {
            return Err(SupplyError::Invalid);
        }
        let mut next = self.document.clone();
        let mut changed = 0;
        for record in &mut next.records {
            if &record.owner == owner
                && record.resident_id == resident_id
                && record.revoked_at_ms.is_none()
            {
                if now_ms < record.authorized_at_ms {
                    return Err(SupplyError::Invalid);
                }
                record.revoked_at_ms = Some(now_ms);
                changed += 1;
            }
        }
        if changed > 0 {
            self.commit(next)?;
        }
        Ok(changed)
    }

    /// Owner request IDs occupy a separate permanent cancellation domain.
    /// Even an offline/no-op cancellation is retained, so retrying it cannot
    /// revoke a later grant that did not exist when the request was accepted.
    pub fn revoke_request(
        &mut self,
        owner_request_key: &str,
        owner: &CharacterBinding,
        resident_id: &str,
        now_ms: u64,
    ) -> Result<usize> {
        self.ready()?;
        if !private_reference(owner_request_key)
            || !character(owner)
            || !reference(resident_id)
            || now_ms > MAX_SAFE_MS
        {
            return Err(SupplyError::Invalid);
        }
        if self
            .document
            .records
            .iter()
            .any(|record| &record.owner == owner && record.owner_request_key == owner_request_key)
        {
            return Err(SupplyError::Conflict);
        }
        self.reject_life_request_key(owner, owner_request_key)?;
        if let Some(previous) =
            self.document.revocations.iter().find(|record| {
                &record.owner == owner && record.owner_request_key == owner_request_key
            })
        {
            if previous.resident_id != resident_id {
                return Err(SupplyError::Conflict);
            }
            return Ok(previous.revoked_permit_ids.len());
        }
        if life::record_count(&self.document) >= RECORD_CAP {
            return Err(SupplyError::Capacity);
        }
        let mut next = self.document.clone();
        let mut revoked_permit_ids = Vec::new();
        for record in &mut next.records {
            if &record.owner == owner
                && record.resident_id == resident_id
                && record.revoked_at_ms.is_none()
            {
                if now_ms < record.authorized_at_ms {
                    return Err(SupplyError::Invalid);
                }
                record.revoked_at_ms = Some(now_ms);
                revoked_permit_ids.push(record.grant.permit_id.clone());
            }
        }
        let count = revoked_permit_ids.len();
        next.revocations.push(RevocationRecord {
            owner_request_key: owner_request_key.into(),
            owner: owner.clone(),
            resident_id: resident_id.into(),
            recorded_at_ms: now_ms,
            revoked_permit_ids,
        });
        self.commit(next)?;
        Ok(count)
    }

    /// The relay checks this before forwarding a cancellation to the runtime;
    /// retrying an old cancellation must not pause a subsequently granted task.
    pub fn revocation_seen(
        &self,
        owner_request_key: &str,
        owner: &CharacterBinding,
        resident_id: &str,
    ) -> Result<bool> {
        self.ready()?;
        if !private_reference(owner_request_key) || !character(owner) || !reference(resident_id) {
            return Err(SupplyError::Invalid);
        }
        if self
            .document
            .records
            .iter()
            .any(|record| &record.owner == owner && record.owner_request_key == owner_request_key)
        {
            return Err(SupplyError::Conflict);
        }
        self.reject_life_request_key(owner, owner_request_key)?;
        match self
            .document
            .revocations
            .iter()
            .find(|record| &record.owner == owner && record.owner_request_key == owner_request_key)
        {
            Some(record) if record.resident_id == resident_id => Ok(true),
            Some(_) => Err(SupplyError::Conflict),
            None => Ok(false),
        }
    }
}
impl Drop for SupplyLedger {
    fn drop(&mut self) {
        // A retained in-memory handle must not outlive its authority writer.
        self.poison();
        self.key.fill(0);
        std::hint::black_box(&mut self.key);
    }
}

fn publish<T: Serialize>(path: &Path, value: &T, replace: bool) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| SupplyError::Integrity)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(SupplyError::Capacity);
    }
    let temporary = suffix(path, &format!(".tmp.{}", random_reference("")?))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| SupplyError::Unavailable)?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| SupplyError::Unavailable)?;
    drop(file);
    if !replace && path.try_exists().map_err(|_| SupplyError::Unavailable)? {
        return Err(SupplyError::Integrity);
    }
    durable_rename(&temporary, path, replace).map_err(|_| SupplyError::Unavailable)?;
    Ok(())
}

#[cfg(not(windows))]
fn durable_rename(from: &Path, to: &Path, _replace: bool) -> io::Result<()> {
    fs::rename(from, to)?;
    File::open(
        to.parent()
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?,
    )?
    .sync_all()
}
#[cfg(windows)]
fn durable_rename(from: &Path, to: &Path, replace: bool) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let wide = |path: &Path| -> io::Result<Vec<u16>> {
        let mut value = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if value.contains(&0) {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        value.push(0);
        Ok(value)
    };
    let from = wide(from)?;
    let to = wide(to)?;
    let flags = 0x0000_0008 | if replace { 0x0000_0001 } else { 0 };
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), flags) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "companion_supply_tests.rs"]
mod tests;

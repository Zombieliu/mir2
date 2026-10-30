//! Fenced proof that a complete custody set and canonical source produced an
//! exact, durable Host base. A receipt is historical evidence, never authority
//! to replay a source transition or to unfreeze a writer.
use super::*;
use crate::zone_rpc::{ZONE_REPLICATION_HEAD_VERSION, ZoneBaseSnapshot, ZoneReplicationHead};
use flate2::read::GzDecoder;
use mir2_simulation::{CharacterSaveRecord, PreparedZoneSourceRecovery, Stage5SystemsState};
use postgres::GenericClient;
use std::io::Read;

const PAGE_SIZE: usize = 256;
const MAX_OPERATIONS: usize = 1_000_000;
const MAX_RECEIPT_LIFETIME_MS: u64 = 5_000;
const MAX_BASE_JSON_BYTES: usize = 96 * 1024 * 1024;

fn sorted_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(fields) => serde_json::Value::Object(
            fields
                .into_iter()
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .map(|(key, value)| (key, sorted_json(value)))
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(sorted_json).collect())
        }
        value => value,
    }
}
fn hash_bytes(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    hex_lower(&hash.finalize())
}
fn hash_value(domain: &[u8], value: &impl Serialize) -> Result<String, String> {
    let value = sorted_json(serde_json::to_value(value).map_err(|e| e.to_string())?);
    Ok(hash_bytes(
        domain,
        &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
    ))
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > 512
        || value.chars().any(char::is_control)
    {
        return Err("invalid player drop recovery identity".into());
    }
    Ok(())
}
fn status_name(status: PlayerDropOperationStatus) -> &'static str {
    match status {
        PlayerDropOperationStatus::Prepared => "Prepared",
        PlayerDropOperationStatus::Committed => "Committed",
        PlayerDropOperationStatus::Consumed => "Consumed",
        PlayerDropOperationStatus::Expired => "Expired",
        PlayerDropOperationStatus::Aborted => "Aborted",
    }
}

/// Provisional complete enumeration. Final publication re-reads and verifies
/// every row under the current lease; separate initial pages are not a snapshot.
#[derive(Debug, Clone)]
pub struct PlayerDropRecoverySet {
    source_namespace: String,
    connection_binding_digest: String,
    zone_id: String,
    records: Vec<PlayerDropOperationRecord>,
    operation_set_digest: String,
    required_characters: Vec<(String, i32)>,
    max_operations: usize,
}
impl PlayerDropRecoverySet {
    pub fn records(&self) -> &[PlayerDropOperationRecord] {
        &self.records
    }
    pub fn operation_set_digest(&self) -> &str {
        &self.operation_set_digest
    }
    pub fn required_characters(&self) -> &[(String, i32)] {
        &self.required_characters
    }
    pub fn account_namespace(&self) -> &str {
        &self.source_namespace
    }
    pub fn zone_id(&self) -> &str {
        &self.zone_id
    }
    pub fn max_operations(&self) -> usize {
        self.max_operations
    }
}
fn make_set(
    namespace: String,
    database_url: &str,
    zone: &str,
    records: Vec<PlayerDropOperationRecord>,
    max_operations: usize,
) -> Result<PlayerDropRecoverySet, String> {
    let mut subjects = BTreeSet::new();
    let mut evidence = Vec::with_capacity(records.len());
    let mut previous: Option<String> = None;
    for record in &records {
        let op = record.operation();
        let key = op.binding().operation_key();
        if record.account_namespace() != namespace
            || op.binding().zone_id() != zone
            || previous.as_ref().is_some_and(|previous| previous >= &key)
        {
            return Err("player drop recovery enumeration is not a complete ordered set".into());
        }
        previous = Some(key.clone());
        subjects.insert((
            op.binding().account_id().to_owned(),
            op.binding().character_index(),
        ));
        if let Some((account, index)) = record.terminal_recipient() {
            subjects.insert((account.to_owned(), index));
        }
        evidence.push((
            key,
            op.operation_digest(),
            op.intent().intent_digest(),
            status_name(record.status()),
            record.receipt(),
            record.terminal_event_id(),
            &record.terminal_recipient,
            record
                .abort_audit()
                .map(|audit| audit.digest())
                .transpose()?,
        ));
    }
    let digest = hash_value(
        b"mir2.player-drop-recovery-set.v1\0",
        &(&namespace, zone, evidence),
    )?;
    Ok(PlayerDropRecoverySet {
        source_namespace: namespace,
        connection_binding_digest: connection_binding_digest(database_url),
        zone_id: zone.into(),
        records,
        operation_set_digest: digest,
        required_characters: subjects.into_iter().collect(),
        max_operations,
    })
}
fn read_set(
    client: &mut impl GenericClient,
    database_url: &str,
    zone: &str,
    max_operations: usize,
    lock: bool,
) -> Result<PlayerDropRecoverySet, String> {
    validate_id(zone)?;
    if !(1..=MAX_OPERATIONS).contains(&max_operations) {
        return Err("invalid player drop recovery total bound".into());
    }
    let namespace: String = client
        .query_one("SELECT current_schema()", &[])
        .map_err(|e| e.to_string())?
        .get(0);
    let mut records = Vec::new();
    let mut cursor = String::new();
    loop {
        // Read one excess row at the configured total bound; never silently
        // treat a truncated recovery set as complete.
        let limit = PAGE_SIZE.min(max_operations.saturating_sub(records.len()) + 1);
        let locking = if lock { " FOR UPDATE OF op" } else { "" };
        let rows = client.query(&format!("{RECORD_QUERY} WHERE op.zone_id=$1 AND op.operation_key>$2 ORDER BY op.operation_key LIMIT $3{locking}"),
            &[&zone, &cursor, &(limit as i64)]).map_err(|e| e.to_string())?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let record = decode_record(row, connection_binding_digest(database_url))?;
            cursor = record.operation().binding().operation_key();
            records.push(record);
            if records.len() > max_operations {
                return Err("player drop recovery exceeds its explicit total bound".into());
            }
        }
    }
    make_set(namespace, database_url, zone, records, max_operations)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostMetadata {
    host_id: String,
    pre_recovery_head: ZoneReplicationHead,
    pre_recovery_head_digest: String,
    post_world_digest: String,
    base_snapshot_id: String,
    base_snapshot_digest: String,
    base_snapshot_bytes: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BasePayload {
    version: u32,
    zone_id: String,
    zone_state_bytes: Vec<u8>,
    sessions: Vec<BaseSession>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BaseSession {
    session_id: String,
    zone_id: String,
    snapshot_digest: String,
    durable_snapshot: Box<WorldSnapshot>,
    active_character_bytes: Option<Vec<u8>>,
    active_identity: Option<ActiveSessionIdentity>,
}
fn base_payload(base: &ZoneBaseSnapshot) -> Result<BasePayload, String> {
    base.verify()?;
    let mut decoded = Vec::with_capacity(base.uncompressed_bytes.min(1024 * 1024));
    GzDecoder::new(base.payload.as_slice())
        .take((crate::zone_rpc::DEFAULT_ZONE_BASE_SNAPSHOT_MAX_UNCOMPRESSED_BYTES as u64) + 1)
        .read_to_end(&mut decoded)
        .map_err(|e| format!("player drop recovery base decode failed: {e}"))?;
    serde_json::from_slice(&decoded)
        .map_err(|e| format!("player drop recovery base payload invalid: {e}"))
}

/// Trusted Host input, independently checked against the real snapshot checksum
/// and complete source stage. The serialized base itself is durably retained.
#[derive(Debug, Clone)]
pub struct PlayerDropRecoveryHostHeads {
    metadata: HostMetadata,
    base_bytes: Vec<u8>,
}
impl PlayerDropRecoveryHostHeads {
    pub fn new(
        host_id: String,
        pre_recovery_head: ZoneReplicationHead,
        recovered_base: ZoneBaseSnapshot,
    ) -> Result<Self, String> {
        validate_id(&host_id)?;
        validate_id(&pre_recovery_head.zone_id)?;
        validate_id(&pre_recovery_head.build_id)?;
        if pre_recovery_head.version != ZONE_REPLICATION_HEAD_VERSION
            || !valid_digest(&pre_recovery_head.latest_digest)
            || recovered_base.zone_id != pre_recovery_head.zone_id
            || recovered_base.build_id != pre_recovery_head.build_id
            || recovered_base.mutation_coverage != pre_recovery_head.mutation_coverage
            || recovered_base.base_sequence != pre_recovery_head.next_sequence
            || recovered_base.latest_digest != pre_recovery_head.latest_digest
            || !recovered_base.apply_ready
        {
            return Err("player drop recovery Host heads disagree".into());
        }
        let payload = base_payload(&recovered_base)?;
        let bytes = serde_json::to_vec(&recovered_base).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BASE_JSON_BYTES
            || payload.zone_id != recovered_base.zone_id
            || payload.version != recovered_base.version
        {
            return Err("player drop recovery base exceeds its bound or identity".into());
        }
        Ok(Self {
            metadata: HostMetadata {
                host_id,
                pre_recovery_head_digest: hash_value(
                    b"mir2.player-drop-recovery-pre-head.v1\0",
                    &pre_recovery_head,
                )?,
                pre_recovery_head,
                post_world_digest: hash_bytes(
                    b"mir2.player-drop-recovery-world.v1\0",
                    &payload.zone_state_bytes,
                ),
                base_snapshot_id: recovered_base.snapshot_id,
                base_snapshot_digest: hash_bytes(b"mir2.player-drop-recovery-base.v1\0", &bytes),
                base_snapshot_bytes: bytes.len(),
            },
            base_bytes: bytes,
        })
    }
    pub fn host_id(&self) -> &str {
        &self.metadata.host_id
    }
    pub fn pre_recovery_head(&self) -> &ZoneReplicationHead {
        &self.metadata.pre_recovery_head
    }
    pub fn post_world_digest(&self) -> &str {
        &self.metadata.post_world_digest
    }
    pub fn base_snapshot_digest(&self) -> &str {
        &self.metadata.base_snapshot_digest
    }
    pub fn base_snapshot_id(&self) -> &str {
        &self.metadata.base_snapshot_id
    }
    pub fn base_snapshot_bytes(&self) -> &[u8] {
        &self.base_bytes
    }
    pub fn base_snapshot(&self) -> Result<ZoneBaseSnapshot, String> {
        serde_json::from_slice(&self.base_bytes)
            .map_err(|_| "player drop recovery base is invalid".into())
    }
    fn validate_source(
        &self,
        source: &PreparedZoneSourceRecovery,
    ) -> Result<Vec<(String, i32)>, String> {
        let payload = base_payload(&self.base_snapshot()?)?;
        let mut hosted = BTreeSet::new();
        for session in payload.sessions {
            // These fields are authenticated by ZoneBaseSnapshot::verify above.
            let _ = (
                &session.session_id,
                &session.zone_id,
                &session.snapshot_digest,
                &session.durable_snapshot,
            );
            match (session.active_identity, session.active_character_bytes) {
                (None, None) => {}
                (Some(identity), Some(bytes)) => {
                    let checkpoint: CharacterSaveRecord = serde_json::from_slice(&bytes)
                        .map_err(|_| "recovery hosted source checkpoint invalid")?;
                    let canonical = source
                        .checkpoint(&identity.account_id, identity.character_index)
                        .ok_or("recovery source stage omits a hosted character")?;
                    if identity.character_index != canonical.character.index
                        || identity.character_name != canonical.character.name
                    {
                        return Err(
                            "recovery Host base contains a different character incarnation".into(),
                        );
                    }
                    // Runtime bytes retain shared world pose/vitals and their
                    // validated clock representation. The opaque source stage
                    // still compares every business field and revision, while
                    // its separate PG digest/CAS read remains byte-exact.
                    source.validate_runtime_checkpoint(
                        &identity.account_id,
                        identity.character_index,
                        &checkpoint,
                    )?;
                    if !hosted.insert((identity.account_id, identity.character_index)) {
                        return Err("recovery Host base duplicates an active character".into());
                    }
                }
                _ => return Err("recovery Host base has an incomplete active character".into()),
            }
        }
        Ok(hosted.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptData {
    version: u32,
    source_namespace: String,
    zone_id: String,
    recovery_id: String,
    owner_id: String,
    owner_epoch: u64,
    source_digest: String,
    source_accounts: Vec<String>,
    source_characters: Vec<(String, i32)>,
    operation_set_digest: String,
    operation_count: usize,
    max_operations: usize,
    state_counts: BTreeMap<String, u64>,
    observation_sequence: u64,
    host: HostMetadata,
    observed_at_ms: u64,
    valid_until_ms: u64,
    requested_lifetime_ms: u64,
}
/// Only database lookup/publication returns this verified receipt. Its durable
/// data can restore the exact base after a crash, but fresh lease/source/head
/// validation is still mandatory before activation.
#[derive(Debug, Clone)]
pub struct PlayerDropRecoveryReceipt {
    data: ReceiptData,
    receipt_digest: String,
    base_bytes: Vec<u8>,
    connection_binding_digest: String,
}
impl PlayerDropRecoveryReceipt {
    pub fn recovery_id(&self) -> &str {
        &self.data.recovery_id
    }
    pub fn account_namespace(&self) -> &str {
        &self.data.source_namespace
    }
    pub fn zone_id(&self) -> &str {
        &self.data.zone_id
    }
    pub fn owner_id(&self) -> &str {
        &self.data.owner_id
    }
    pub fn owner_epoch(&self) -> u64 {
        self.data.owner_epoch
    }
    pub fn source_digest(&self) -> &str {
        &self.data.source_digest
    }
    pub fn operation_set_digest(&self) -> &str {
        &self.data.operation_set_digest
    }
    pub fn operation_count(&self) -> usize {
        self.data.operation_count
    }
    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }
    pub fn observed_at_ms(&self) -> u64 {
        self.data.observed_at_ms
    }
    pub fn valid_until_ms(&self) -> u64 {
        self.data.valid_until_ms
    }
    pub fn observation_sequence(&self) -> u64 {
        self.data.observation_sequence
    }
    pub fn base_snapshot_bytes(&self) -> &[u8] {
        &self.base_bytes
    }
    pub fn host_heads(&self) -> Result<PlayerDropRecoveryHostHeads, String> {
        let base = serde_json::from_slice(&self.base_bytes)
            .map_err(|_| "recovery base snapshot invalid")?;
        let heads = PlayerDropRecoveryHostHeads::new(
            self.data.host.host_id.clone(),
            self.data.host.pre_recovery_head.clone(),
            base,
        )?;
        if heads.metadata != self.data.host || heads.base_bytes != self.base_bytes {
            return Err("recovery base proof differs from receipt".into());
        }
        Ok(heads)
    }
}
const RECEIPT_QUERY: &str = "SELECT * FROM public.zone_player_drop_recovery_receipts WHERE source_namespace=current_schema() AND zone_id=$1 AND recovery_id=$2";
fn decode_receipt(
    row: postgres::Row,
    database_url: &str,
) -> Result<PlayerDropRecoveryReceipt, String> {
    let data: ReceiptData = serde_json::from_value(row.get("receipt"))
        .map_err(|_| "invalid player drop recovery receipt")?;
    let digest: String = row.get("receipt_digest");
    let bytes: Vec<u8> = row.get("base_snapshot_bytes");
    if data.version != 1
        || bytes.is_empty()
        || bytes.len() > MAX_BASE_JSON_BYTES
        || data.valid_until_ms <= data.observed_at_ms
        || data.requested_lifetime_ms == 0
        || data.requested_lifetime_ms > MAX_RECEIPT_LIFETIME_MS
        || data.valid_until_ms
            > data
                .observed_at_ms
                .saturating_add(data.requested_lifetime_ms)
        || !valid_digest(&data.source_digest)
        || !valid_digest(&data.operation_set_digest)
        || data.source_namespace != row.get::<_, String>("source_namespace")
        || data.zone_id != row.get::<_, String>("zone_id")
        || data.recovery_id != row.get::<_, String>("recovery_id")
        || data.owner_id != row.get::<_, String>("owner_id")
        || data.owner_epoch != row.get::<_, i64>("owner_epoch") as u64
        || data.source_digest != row.get::<_, String>("source_digest")
        || data.operation_set_digest != row.get::<_, String>("operation_set_digest")
        || data.operation_count as i64 != row.get::<_, i64>("operation_count")
        || data.observation_sequence as i64 != row.get::<_, i64>("observation_sequence")
        || data.host.base_snapshot_id != row.get::<_, String>("base_snapshot_id")
        || data.host.base_snapshot_digest != row.get::<_, String>("base_snapshot_digest")
        || data.observed_at_ms as i64 != row.get::<_, i64>("observed_at_ms")
        || data.valid_until_ms as i64 != row.get::<_, i64>("valid_until_ms")
        || hash_value(b"mir2.player-drop-recovery-receipt.v1\0", &data)? != digest
    {
        return Err("player drop recovery receipt immutable evidence conflict".into());
    }
    let receipt = PlayerDropRecoveryReceipt {
        data,
        receipt_digest: digest,
        base_bytes: bytes,
        connection_binding_digest: connection_binding_digest(database_url),
    };
    receipt.host_heads()?;
    Ok(receipt)
}

fn validate_bindings(
    permit: &ZoneEconomyWritePermit,
    source: &PreparedZoneSourceRecovery,
    database_url: &str,
    expected: &PlayerDropRecoverySet,
    heads: &PlayerDropRecoveryHostHeads,
) -> Result<(), String> {
    permit.validate_target_database(database_url)?;
    source.validate_target_database(database_url)?;
    let fence = permit.fence();
    if expected.connection_binding_digest != connection_binding_digest(database_url)
        || expected.zone_id != fence.zone_id()
        || heads.pre_recovery_head().zone_id != fence.zone_id()
        || source.fence() != fence
        || source.source_namespace() != expected.source_namespace
        || heads.host_id() != fence.owner_id()
    {
        return Err("player drop recovery source, Host or lease binding conflict".into());
    }
    for record in &expected.records {
        let actor = record.operation().binding();
        if source
            .checkpoint(actor.account_id(), actor.character_index())
            .is_none()
            && (!terminal_source_may_retire(record.status())
                || !source.is_retired_character(actor.account_id(), actor.character_index())?)
        {
            return Err(
                "player drop recovery source omits a live or unresolved custody actor".into(),
            );
        }
        if let Some((account, index)) = record.terminal_recipient() {
            if source.checkpoint(account, index).is_none()
                && !source.is_retired_character(account, index)?
            {
                return Err("player drop recovery source omits its terminal recipient".into());
            }
        }
    }
    Ok(())
}
fn terminal_source_may_retire(status: PlayerDropOperationStatus) -> bool {
    matches!(
        status,
        PlayerDropOperationStatus::Consumed
            | PlayerDropOperationStatus::Expired
            | PlayerDropOperationStatus::Aborted
    )
}

/// A deleted recipient may not discard an undelivered economic credit. The
/// terminal receipt/outbox must still identify its exact projected delivery.
fn validate_retired_recipient_delivery(
    tx: &mut Transaction<'_>,
    record: &PlayerDropOperationRecord,
    account: &str,
    index: i32,
) -> Result<(), String> {
    let event_id = record
        .terminal_event_id()
        .ok_or("retired recipient has no terminal event")?;
    let row=tx.query_opt("SELECT projection.account_id,projection.character_index,projection.status,projection.projected_at_ms,projection.intent,tx.receipt,outbox.payload FROM game_economy_ground_drop_projections projection JOIN game_economy_transactions tx ON tx.event_id=projection.event_id JOIN game_economy_outbox outbox ON outbox.event_id=projection.event_id WHERE projection.event_id=$1 AND projection.account_id=$2 AND projection.character_index=$3 FOR SHARE OF projection",
        &[&event_id,&account,&index]).map_err(|e|e.to_string())?.ok_or("retired recipient terminal delivery proof missing")?;
    if row.get::<_, String>("status") != "projected"
        || !row
            .get::<_, Option<i64>>("projected_at_ms")
            .is_some_and(|time| time > 0)
    {
        return Err("retired recipient still has unresolved terminal delivery".into());
    }
    let event: EconomyOutboxEvent =
        serde_json::from_value(row.get("payload")).map_err(|e| e.to_string())?;
    let receipt: EconomyTransactionReceipt =
        serde_json::from_value(row.get("receipt")).map_err(|e| e.to_string())?;
    validate_stored_economy_transaction(&event, &receipt)?;
    let expected = ground_drop_projection_from_envelope(&event.envelope, event_id)?
        .ok_or("retired recipient terminal projection missing")?;
    let intent: GroundDropProjectionIntent =
        serde_json::from_value(row.get("intent")).map_err(|e| e.to_string())?;
    if event.event_id != event_id
        || expected.account_id != account
        || expected.character_index != index
        || expected.intent != intent
        || row.get::<_, String>("account_id") != account
        || row.get::<_, i32>("character_index") != index
    {
        return Err("retired recipient terminal delivery identity conflict".into());
    }
    validate_pickup_intent(record.operation(), &event.envelope, &intent)?;
    if tx.query_one("SELECT EXISTS(SELECT 1 FROM game_economy_ground_drop_projections WHERE account_id=$1 AND character_index=$2 AND status='pending')",&[&account,&index]).map_err(|e|e.to_string())?.get::<_,bool>(0) {
        return Err("retired recipient has pending pickup delivery".into());
    }
    Ok(())
}
fn validate_final_set(
    tx: &mut Transaction<'_>,
    permit: &ZoneEconomyWritePermit,
    source: &PreparedZoneSourceRecovery,
    expected: &PlayerDropRecoverySet,
    heads: &PlayerDropRecoveryHostHeads,
    sequence: u64,
    database_url: &str,
) -> Result<(u64, u64, BTreeMap<String, u64>), String> {
    validate_bindings(permit, source, database_url, expected, heads)?;
    if sequence > i64::MAX as u64 {
        return Err("invalid recovery observation sequence".into());
    }
    // Receipt/projection writers take these character locks before a pickup's
    // effect row. Match that order, including for a rejected cross-Zone claim.
    // Source operation commits in this Zone are already blocked by our lease.
    lock_economy_characters(
        tx,
        &source
            .character_keys()
            .iter()
            .chain(&expected.required_characters)
            .cloned()
            .collect(),
    )?;
    let actual = read_set(
        tx,
        database_url,
        &expected.zone_id,
        expected.max_operations,
        true,
    )?;
    if actual.operation_set_digest != expected.operation_set_digest
        || actual.source_namespace != expected.source_namespace
    {
        return Err("player drop recovery custody changed after enumeration".into());
    }
    // No per-operation advisory locks here: ordinary commit/abort lock those
    // before the lease. The held lease serializes all custody mutations.
    // The character locks also block another Zone crediting a hosted recipient
    // between our pending-delivery check and receipt COMMIT.
    source.lock_and_validate_postgres(tx, permit, database_url)?;
    let hosted = heads.validate_source(source)?;
    let now: i64 = tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .map_err(|e| e.to_string())?
        .get(0);
    let now = u64::try_from(now).map_err(|_| "recovery database clock unavailable")?;
    let expiry: i64 = tx
        .query_one(
            "SELECT expires_at_ms FROM public.zone_owner_leases WHERE zone_id=$1",
            &[&expected.zone_id],
        )
        .map_err(|e| e.to_string())?
        .get(0);
    let mut valid_until = u64::try_from(expiry).map_err(|_| "recovery lease expiry invalid")?;
    let mut counts = BTreeMap::new();
    for record in &actual.records {
        *counts
            .entry(status_name(record.status()).into())
            .or_insert(0) += 1;
        if record.operation().binding().owner_epoch() > permit.fence().fencing_token() {
            return Err("recovery encountered a newer custody writer".into());
        }
        if record.status() == PlayerDropOperationStatus::Prepared {
            return Err("recovery still contains an unresolved Prepared operation".into());
        }
        if record.status() == PlayerDropOperationStatus::Committed {
            let expires = record.operation().intent().expires_at_unix_ms();
            if expires <= now {
                return Err("recovery must persist database expiry before promotion".into());
            }
            valid_until = valid_until.min(expires);
        }
        if record.status() != PlayerDropOperationStatus::Aborted {
            if record.observation.as_ref()
                != Some(&(
                    permit.fence().owner_id().into(),
                    permit.fence().fencing_token(),
                    sequence,
                ))
            {
                return Err("recovery custody lacks the exact successor observation".into());
            }
            let audit = tx.query_opt("SELECT observer_owner_id,observed_state FROM public.zone_player_drop_observation_audit WHERE source_namespace=current_schema() AND world_effect_id=$1 AND observer_epoch=$2 AND observer_sequence=$3",
                &[&record.operation().intent().world_effect_id(), &(permit.fence().fencing_token() as i64), &(sequence as i64)]).map_err(|e|e.to_string())?.ok_or("recovery observation audit missing")?;
            if audit.get::<_, String>(0) != permit.fence().owner_id()
                || audit.get::<_, String>(1) != status_name(record.status())
            {
                return Err("recovery observation audit differs from custody".into());
            }
        }
        if let Some((account, index)) = record.terminal_recipient() {
            if source.checkpoint(account, index).is_none() {
                if !source.is_retired_character(account, index)? {
                    return Err("unproven retired terminal recipient".into());
                }
                validate_retired_recipient_delivery(tx, record, account, index)?;
            }
            if hosted.contains(&(account.into(), index)) {
                let save = source
                    .checkpoint(account, index)
                    .ok_or("recovery terminal recipient source missing")?;
                let systems = save
                    .stage5_systems_json
                    .as_ref()
                    .map(|json| serde_json::from_str::<Stage5SystemsState>(json))
                    .transpose()
                    .map_err(|_| "recovery terminal recipient markers invalid")?;
                if !systems.is_some_and(|systems| {
                    record
                        .terminal_event_id()
                        .is_some_and(|id| systems.economy_projection_event_ids.contains(id))
                }) {
                    return Err(
                        "recovery hosted recipient has not persisted its terminal pickup".into(),
                    );
                }
            }
        }
    }
    for (account, index) in &hosted {
        if tx.query_one("SELECT EXISTS(SELECT 1 FROM game_economy_ground_drop_projections WHERE account_id=$1 AND character_index=$2 AND status='pending')",&[account,index]).map_err(|e|e.to_string())?.get::<_,bool>(0) {
            return Err("recovery hosted character has pending pickup delivery".into());
        }
    }
    if now == 0 || valid_until <= now {
        return Err("recovery lease or live effect expired".into());
    }
    Ok((now, valid_until, counts))
}

impl PostgresEconomyStore {
    pub fn read_zone_player_drop_recovery_set(
        &self,
        zone: &str,
        max_operations: usize,
    ) -> Result<PlayerDropRecoverySet, String> {
        read_set(
            &mut self.player_drop_connect()?,
            &self.database_url,
            zone,
            max_operations,
            false,
        )
    }
    pub fn lookup_zone_player_drop_recovery_receipt(
        &self,
        zone: &str,
        recovery_id: &str,
    ) -> Result<Option<PlayerDropRecoveryReceipt>, String> {
        validate_id(zone)?;
        validate_id(recovery_id)?;
        self.player_drop_connect()?
            .query_opt(RECEIPT_QUERY, &[&zone, &recovery_id])
            .map_err(|e| e.to_string())?
            .map(|row| decode_receipt(row, &self.database_url))
            .transpose()
    }
    pub fn commit_zone_player_drop_recovery(
        &self,
        permit: &ZoneEconomyWritePermit,
        source: &PreparedZoneSourceRecovery,
        expected: &PlayerDropRecoverySet,
        heads: &PlayerDropRecoveryHostHeads,
        recovery_id: &str,
        observation_sequence: u64,
        requested_lifetime_ms: u64,
    ) -> Result<PlayerDropRecoveryReceipt, String> {
        self.commit_recovery_inner(
            permit,
            source,
            expected,
            heads,
            recovery_id,
            observation_sequence,
            requested_lifetime_ms,
            false,
        )
    }
    #[cfg(test)]
    pub(in crate::economy::player_drop) fn commit_recovery_losing_acknowledgement(
        &self,
        permit: &ZoneEconomyWritePermit,
        source: &PreparedZoneSourceRecovery,
        expected: &PlayerDropRecoverySet,
        heads: &PlayerDropRecoveryHostHeads,
        recovery_id: &str,
        observation_sequence: u64,
        requested_lifetime_ms: u64,
    ) -> Result<PlayerDropRecoveryReceipt, String> {
        self.commit_recovery_inner(
            permit,
            source,
            expected,
            heads,
            recovery_id,
            observation_sequence,
            requested_lifetime_ms,
            true,
        )
    }
    fn commit_recovery_inner(
        &self,
        permit: &ZoneEconomyWritePermit,
        source: &PreparedZoneSourceRecovery,
        expected: &PlayerDropRecoverySet,
        heads: &PlayerDropRecoveryHostHeads,
        recovery_id: &str,
        observation_sequence: u64,
        requested_lifetime_ms: u64,
        lose_acknowledgement: bool,
    ) -> Result<PlayerDropRecoveryReceipt, String> {
        validate_id(recovery_id)?;
        validate_bindings(permit, source, &self.database_url, expected, heads)?;
        if !(1..=MAX_RECEIPT_LIFETIME_MS).contains(&requested_lifetime_ms) {
            return Err("invalid recovery receipt lifetime".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        zone_fence::bound_transaction(&mut tx)?;
        let mutex = format!("player-drop-recovery:{}:{}", expected.zone_id, recovery_id);
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,491827))",
            &[&mutex],
        )
        .map_err(|e| e.to_string())?;
        permit.lock_and_validate(
            &mut tx,
            &self.database_url,
            &expected.zone_id,
            permit.fence().fencing_token(),
        )?;
        let (now, expiry, counts) = validate_final_set(
            &mut tx,
            permit,
            source,
            expected,
            heads,
            observation_sequence,
            &self.database_url,
        )?;
        if let Some(row) = tx
            .query_opt(RECEIPT_QUERY, &[&expected.zone_id, &recovery_id])
            .map_err(|e| e.to_string())?
        {
            let receipt = decode_receipt(row, &self.database_url)?;
            self.match_recovery_receipt(
                &receipt,
                permit,
                source,
                expected,
                heads,
                observation_sequence,
            )?;
            if receipt.data.requested_lifetime_ms != requested_lifetime_ms
                || receipt.valid_until_ms() <= now
            {
                return Err("recovery identity already has different or expired evidence".into());
            }
            return Ok(receipt);
        }
        let data = ReceiptData {
            version: 1,
            source_namespace: expected.source_namespace.clone(),
            zone_id: expected.zone_id.clone(),
            recovery_id: recovery_id.into(),
            owner_id: permit.fence().owner_id().into(),
            owner_epoch: permit.fence().fencing_token(),
            source_digest: source.source_digest().into(),
            source_accounts: source.account_ids().to_vec(),
            source_characters: source.character_keys().to_vec(),
            operation_set_digest: expected.operation_set_digest.clone(),
            operation_count: expected.records.len(),
            max_operations: expected.max_operations,
            state_counts: counts,
            observation_sequence,
            host: heads.metadata.clone(),
            observed_at_ms: now,
            valid_until_ms: expiry.min(now.saturating_add(requested_lifetime_ms)),
            requested_lifetime_ms,
        };
        let digest = hash_value(b"mir2.player-drop-recovery-receipt.v1\0", &data)?;
        let json = serde_json::to_value(&data).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO public.zone_player_drop_recovery_receipts(zone_id,recovery_id,owner_id,owner_epoch,source_digest,operation_set_digest,operation_count,observation_sequence,base_snapshot_id,base_snapshot_digest,base_snapshot_bytes,receipt,receipt_digest,observed_at_ms,valid_until_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
            &[&data.zone_id,&data.recovery_id,&data.owner_id,&(data.owner_epoch as i64),&data.source_digest,&data.operation_set_digest,&(data.operation_count as i64),&(data.observation_sequence as i64),&data.host.base_snapshot_id,&data.host.base_snapshot_digest,&heads.base_bytes,&json,&digest,&(data.observed_at_ms as i64),&(data.valid_until_ms as i64)]).map_err(|e|e.to_string())?;
        let receipt = decode_receipt(
            tx.query_one(RECEIPT_QUERY, &[&data.zone_id, &data.recovery_id])
                .map_err(|e| e.to_string())?,
            &self.database_url,
        )?;
        tx.commit().map_err(|_| permit.commit_outcome_unknown())?;
        // Bounded test fault: the real transaction committed, but its success
        // acknowledgement is withheld. This is not a native socket fault claim.
        if lose_acknowledgement {
            return Err(permit.commit_outcome_unknown());
        }
        Ok(receipt)
    }
    fn match_recovery_receipt(
        &self,
        receipt: &PlayerDropRecoveryReceipt,
        permit: &ZoneEconomyWritePermit,
        source: &PreparedZoneSourceRecovery,
        expected: &PlayerDropRecoverySet,
        heads: &PlayerDropRecoveryHostHeads,
        sequence: u64,
    ) -> Result<(), String> {
        let data = &receipt.data;
        if receipt.connection_binding_digest != connection_binding_digest(&self.database_url)
            || data.source_namespace != expected.source_namespace
            || data.zone_id != expected.zone_id
            || data.owner_id != permit.fence().owner_id()
            || data.owner_epoch != permit.fence().fencing_token()
            || data.source_digest != source.source_digest()
            || data.source_accounts != source.account_ids()
            || data.source_characters != source.character_keys()
            || data.operation_set_digest != expected.operation_set_digest
            || data.operation_count != expected.records.len()
            || data.max_operations != expected.max_operations
            || data.host != heads.metadata
            || receipt.base_bytes != heads.base_bytes
            || data.observation_sequence != sequence
        {
            return Err("player drop recovery receipt does not match current source, custody, lease or Host heads".into());
        }
        Ok(())
    }
    /// Fresh validation is read-only. The caller must retain its quiesced Host
    /// gate through activation and preserve source CAS versions. A historical
    /// receipt by itself never enables a runtime or clears an Unknown freeze.
    pub fn validate_zone_player_drop_recovery_receipt(
        &self,
        permit: &ZoneEconomyWritePermit,
        source: &PreparedZoneSourceRecovery,
        receipt: &PlayerDropRecoveryReceipt,
        heads: &PlayerDropRecoveryHostHeads,
    ) -> Result<u64, String> {
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        permit.lock_and_validate(
            &mut tx,
            &self.database_url,
            receipt.zone_id(),
            permit.fence().fencing_token(),
        )?;
        let stored = decode_receipt(
            tx.query_opt(RECEIPT_QUERY, &[&receipt.zone_id(), &receipt.recovery_id()])
                .map_err(|e| e.to_string())?
                .ok_or("recovery receipt missing")?,
            &self.database_url,
        )?;
        if stored.receipt_digest != receipt.receipt_digest {
            return Err("recovery receipt changed".into());
        }
        let expected = read_set(
            &mut tx,
            &self.database_url,
            receipt.zone_id(),
            receipt.data.max_operations,
            false,
        )?;
        self.match_recovery_receipt(
            receipt,
            permit,
            source,
            &expected,
            heads,
            receipt.observation_sequence(),
        )?;
        let (now, _, _) = validate_final_set(
            &mut tx,
            permit,
            source,
            &expected,
            heads,
            receipt.observation_sequence(),
            &self.database_url,
        )?;
        if now >= receipt.valid_until_ms() {
            return Err("player drop recovery receipt expired".into());
        }
        // Fresh DB time gives the caller a conservative remaining lifetime;
        // retrying an existing receipt must never renew its original TTL.
        Ok(receipt.valid_until_ms() - now)
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
pub(in crate::economy::player_drop) mod tests;

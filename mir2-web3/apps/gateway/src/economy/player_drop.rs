//! Durable ordinary-player-drop source publication and world custody.
//!
//! The Prepared row freezes the entire source transition and absolute world
//! intent. Only the opaque SourceOfTruth participant can perform its debit.
//! Observation never removes a live effect from restart enumeration.
use super::*;
use mir2_simulation::{
    PlayerDropCommitReceipt, PlayerDropCommittedEffect, PlayerDropEffectState,
    PlayerDropOperationBinding, PlayerDropOutcome, PlayerDropPreparedOperation,
    PreparedPlayerDropAccountSource,
};
mod abort;
mod item_uid;
mod service;
pub use abort::PlayerDropAbortAudit;

fn connection_binding_digest(database_url: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"mir2.player-drop-source-namespace.v1\0");
    digest.update(database_url.as_bytes());
    hex_lower(&digest.finalize())
}

impl PostgresEconomyStore {
    fn player_drop_connect(&self) -> Result<Client, String> {
        let mut client = self.connect()?;
        client.batch_execute("SET lock_timeout='250ms'; SET statement_timeout='2000ms'; SET idle_in_transaction_session_timeout='5000ms'")
            .map_err(|_|"player drop database query limits unavailable")?;
        Ok(client)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerDropSourceProjection {
    version: u8,
    operation: serde_json::Value,
}

impl PlayerDropSourceProjection {
    fn new(operation: &PlayerDropPreparedOperation) -> Result<Self, String> {
        operation.validate_integrity()?;
        Ok(Self {
            version: 1,
            operation: serde_json::to_value(operation).map_err(|e| e.to_string())?,
        })
    }

    pub fn operation(&self) -> Result<PlayerDropPreparedOperation, String> {
        if self.version != 1 {
            return Err("unsupported player drop source proof".into());
        }
        let operation: PlayerDropPreparedOperation =
            serde_json::from_value(self.operation.clone()).map_err(|e| e.to_string())?;
        operation.validate_integrity()?;
        Ok(operation)
    }

    pub(super) fn digest(&self) -> Result<String, String> {
        self.operation()?;
        let mut hash = Sha256::new();
        hash.update(b"mir2.player-drop-source-projection.v1\0");
        fn canonical(value: serde_json::Value) -> serde_json::Value {
            match value {
                serde_json::Value::Object(fields) => serde_json::Value::Object(
                    fields
                        .into_iter()
                        .collect::<BTreeMap<_, _>>()
                        .into_iter()
                        .map(|(key, value)| (key, canonical(value)))
                        .collect(),
                ),
                serde_json::Value::Array(values) => {
                    serde_json::Value::Array(values.into_iter().map(canonical).collect())
                }
                value => value,
            }
        }
        hash.update(
            serde_json::to_vec(&canonical(
                serde_json::to_value(self).map_err(|e| e.to_string())?,
            ))
            .map_err(|e| e.to_string())?,
        );
        Ok(hex_lower(&hash.finalize()))
    }

    pub(super) fn validate_envelope(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<(), String> {
        let operation = self.operation()?;
        let (legs, _) = source_delta(&operation)?;
        if envelope != &source_envelope(&operation, legs) {
            return Err("player drop source proof does not bind the ledger envelope".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerDropOperationStatus {
    Prepared,
    Committed,
    Consumed,
    Expired,
    Aborted,
}
impl PlayerDropOperationStatus {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "Prepared" => Ok(Self::Prepared),
            "Committed" => Ok(Self::Committed),
            "Consumed" => Ok(Self::Consumed),
            "Expired" => Ok(Self::Expired),
            "Aborted" => Ok(Self::Aborted),
            _ => Err("invalid player drop operation status".into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayerDropOperationRecord {
    source_namespace: String,
    // Transient read capability binding; never persisted or serialized. Thus
    // ordinary credential rotation does not change durable custody identity.
    connection_binding_digest: String,
    operation: PlayerDropPreparedOperation,
    status: PlayerDropOperationStatus,
    receipt: Option<EconomyTransactionReceipt>,
    terminal_event_id: Option<String>,
    abort_audit: Option<PlayerDropAbortAudit>,
}
impl PlayerDropOperationRecord {
    pub fn abort_audit(&self) -> Option<&PlayerDropAbortAudit> {
        self.abort_audit.as_ref()
    }
    pub fn account_namespace(&self) -> &str {
        &self.source_namespace
    }
    pub fn operation(&self) -> &PlayerDropPreparedOperation {
        &self.operation
    }
    pub fn status(&self) -> PlayerDropOperationStatus {
        self.status
    }
    pub fn receipt(&self) -> Option<&EconomyTransactionReceipt> {
        self.receipt.as_ref()
    }
    pub fn terminal_event_id(&self) -> Option<&str> {
        self.terminal_event_id.as_deref()
    }
    pub fn committed_effect(
        &self,
        permit: &ZoneEconomyWritePermit,
        database_url: &str,
    ) -> Result<PlayerDropCommittedEffect, String> {
        permit.validate_target_database(database_url)?;
        if self.connection_binding_digest != connection_binding_digest(database_url) {
            return Err("player drop receipt belongs to a different source authority".into());
        }
        let receipt = self.commit_receipt()?;
        PlayerDropCommittedEffect::from_verified_receipt(
            permit,
            database_url,
            self.operation.clone(),
            receipt,
        )
    }
    pub fn commit_receipt(&self) -> Result<PlayerDropCommitReceipt, String> {
        let state = match self.status {
            PlayerDropOperationStatus::Prepared | PlayerDropOperationStatus::Aborted => {
                return Err("uncommitted player drop has no world custody".into())
            }
            PlayerDropOperationStatus::Committed => PlayerDropEffectState::Live,
            PlayerDropOperationStatus::Consumed => PlayerDropEffectState::Consumed,
            PlayerDropOperationStatus::Expired => PlayerDropEffectState::Expired,
        };
        Ok(PlayerDropCommitReceipt::new(
            self.operation.binding().operation_key(),
            self.operation.intent().world_effect_id().into(),
            self.operation.operation_digest().into(),
            self.operation.intent().intent_digest().into(),
            self.operation.checkpoint().revision,
            state,
        ))
    }
}

pub(super) struct PlayerDropLedgerSource<'a> {
    pub(super) source: &'a PreparedPlayerDropAccountSource,
    pub(super) projection: PlayerDropSourceProjection,
    pub(super) expected_before: BTreeMap<EconomyBalanceKey, i64>,
    legs: Vec<EconomyLeg>,
}
impl<'a> PlayerDropLedgerSource<'a> {
    fn new(source: &'a PreparedPlayerDropAccountSource) -> Result<Self, String> {
        let (legs, expected_before) = source_delta(source.operation())?;
        Ok(Self {
            source,
            projection: PlayerDropSourceProjection::new(source.operation())?,
            expected_before,
            legs,
        })
    }
    pub(super) fn validate_envelope(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<(), String> {
        self.projection.validate_envelope(envelope)
    }
    pub(super) fn lock_prepared_operation(&self, tx: &mut Transaction<'_>) -> Result<(), String> {
        let row = tx.query_opt("SELECT operation,status FROM public.zone_player_drop_operations WHERE operation_key=$1 AND source_namespace=current_schema() FOR UPDATE",
            &[&self.source.binding().operation_key()]).map_err(|e| e.to_string())?.ok_or("player drop was not durably prepared")?;
        if row.get::<_, serde_json::Value>(0) != self.projection.operation
            || row.get::<_, String>(1) != "Prepared"
        {
            return Err("player drop prepared source conflict; exact recovery required".into());
        }
        Ok(())
    }
    pub(super) fn validate_committed(
        &self,
        tx: &mut Transaction<'_>,
        event_id: &str,
    ) -> Result<(), String> {
        let row = tx.query_opt("SELECT operation,status,event_id FROM public.zone_player_drop_operations WHERE operation_key=$1 AND source_namespace=current_schema()",
            &[&self.source.binding().operation_key()]).map_err(|e| e.to_string())?.ok_or("player drop custody row missing")?;
        if row.get::<_, serde_json::Value>(0) != self.projection.operation
            || row.get::<_, String>(1) == "Prepared"
            || row.get::<_, Option<String>>(2).as_deref() != Some(event_id)
        {
            return Err("player drop committed custody proof conflict".into());
        }
        Ok(())
    }
    pub(super) fn mark_committed(
        &self,
        tx: &mut Transaction<'_>,
        event_id: &str,
        committed_at_ms: u64,
    ) -> Result<(), String> {
        let updated = tx.execute("UPDATE public.zone_player_drop_operations SET status='Committed',event_id=$2,committed_at_ms=$3 WHERE operation_key=$1 AND source_namespace=current_schema() AND status='Prepared'",
            &[&self.source.binding().operation_key(), &event_id, &(committed_at_ms as i64)]).map_err(|e| e.to_string())?;
        if updated != 1 {
            return Err("player drop commit lost its prepared operation".into());
        }
        Ok(())
    }
}

fn source_balances(
    save: &mir2_simulation::CharacterSaveRecord,
) -> Result<BTreeMap<(String, String), i64>, String> {
    #[derive(Deserialize)]
    struct Item {
        key: String,
        quantity: u32,
    }
    let mut balances = BTreeMap::from([
        (("gold".into(), "gold".into()), i64::from(save.gold)),
        (("experience".into(), "experience".into()), save.experience),
    ]);
    for encoded in save
        .inventory_items_json
        .iter()
        .chain(&save.belt_items_json)
        .chain(&save.storage_items_json)
        .chain(&save.hero_inventory_items_json)
        .chain(&save.equipment_items_json)
    {
        let item: Item = serde_json::from_str(encoded).map_err(|e| e.to_string())?;
        if item.key.trim().is_empty() {
            return Err("player drop source item key missing".into());
        }
        let count = balances
            .entry(("item_quantity".into(), item.key))
            .or_default();
        *count = count
            .checked_add(i64::from(item.quantity))
            .ok_or("player drop quantity overflow")?;
    }
    Ok(balances)
}

fn source_delta(
    operation: &PlayerDropPreparedOperation,
) -> Result<(Vec<EconomyLeg>, BTreeMap<EconomyBalanceKey, i64>), String> {
    operation.validate_integrity()?;
    let before = source_balances(operation.before())?;
    let after = source_balances(operation.checkpoint())?;
    let mut legs = Vec::new();
    let mut expected = BTreeMap::new();
    for (kind, key) in before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let pair = (kind.clone(), key.clone());
        let old = before.get(&pair).copied().unwrap_or(0);
        let delta = after
            .get(&pair)
            .copied()
            .unwrap_or(0)
            .checked_sub(old)
            .ok_or("player drop source delta overflow")?;
        if delta == 0 {
            continue;
        }
        if delta > 0 {
            return Err("player drop source unexpectedly credits an asset".into());
        }
        let balance = EconomyBalanceKey {
            account_id: operation.binding().account_id().into(),
            character_index: operation.binding().character_index(),
            asset_kind: kind,
            asset_key: key,
        };
        balance.validate()?;
        expected.insert(balance.clone(), old);
        legs.push(EconomyLeg { balance, delta });
    }
    let valid = match operation.intent().outcome() {
        PlayerDropOutcome::Gold { amount } => {
            legs.len() == 1
                && legs[0].balance.asset_kind == "gold"
                && legs[0].delta == -i64::from(*amount)
        }
        PlayerDropOutcome::Item {
            quantity,
            loot: GroundDropLootSnapshot::InventoryItem { key, .. },
        } => {
            legs.len() == 1
                && legs[0].balance.asset_kind == "item_quantity"
                && legs[0].balance.asset_key == *key
                && legs[0].delta == -i64::from(*quantity)
        }
        PlayerDropOutcome::DestroyedItem {
            quantity, item_key, ..
        } => {
            legs.len() == 1
                && legs[0].balance.asset_kind == "item_quantity"
                && legs[0].balance.asset_key == *item_key
                && legs[0].delta == -i64::from(*quantity)
        }
        _ => false,
    };
    if !valid {
        return Err("player drop source debit does not match its exact frozen outcome".into());
    }
    Ok((legs, expected))
}

fn source_envelope(
    operation: &PlayerDropPreparedOperation,
    legs: Vec<EconomyLeg>,
) -> EconomyTransactionEnvelope {
    let binding = operation.binding();
    EconomyTransactionEnvelope {
        idempotency_key: binding.operation_key(),
        transaction_kind: EconomyTransactionKind::Consume,
        zone_id: binding.zone_id().into(),
        fencing_generation: binding.owner_epoch(),
        source_sequence: binding.source_sequence(),
        created_at_ms: operation.intent().created_at_unix_ms(),
        legs,
        metadata: BTreeMap::from([
            ("operation".into(), "preparedPlayerDrop".into()),
            (
                "playerDropOperationDigest".into(),
                operation.operation_digest().into(),
            ),
            (
                "playerDropIntentDigest".into(),
                operation.intent().intent_digest().into(),
            ),
            (
                "playerDropWorldEffectId".into(),
                operation.intent().world_effect_id().into(),
            ),
        ]),
    }
}

const RECORD_QUERY:&str="SELECT op.source_namespace,op.operation_key,op.world_effect_id,op.zone_id,op.owner_id,op.owner_epoch,op.operation_digest,op.intent_digest,op.operation,op.expires_at_ms,op.status,op.event_id,op.terminal_event_id,op.terminal_account_id,op.terminal_character_index,op.aborted_at_ms,op.abort_audit,op.abort_digest,abort_audit.audited_receipt AS audited_abort_receipt,abort_audit.audit_digest AS audited_abort_digest,abort_audit.recovery_id AS audited_recovery_id,abort_audit.recovery_owner_id AS audited_recovery_owner_id,abort_audit.recovery_epoch AS audited_recovery_epoch,EXISTS(SELECT 1 FROM game_economy_transactions original_tx WHERE original_tx.idempotency_key=op.operation_key OR original_tx.event_id=op.abort_audit->>'unpublishedEventId') AS original_ledger_exists,EXISTS(SELECT 1 FROM game_economy_outbox original_outbox WHERE original_outbox.idempotency_key=op.operation_key OR original_outbox.event_id=op.abort_audit->>'unpublishedEventId') AS original_outbox_exists,tx.receipt,outbox.payload,terminal_tx.receipt AS terminal_receipt,terminal_outbox.payload AS terminal_payload FROM (SELECT * FROM public.zone_player_drop_operations WHERE source_namespace=current_schema()) op LEFT JOIN game_economy_transactions tx ON tx.event_id=op.event_id LEFT JOIN game_economy_outbox outbox ON outbox.event_id=op.event_id LEFT JOIN game_economy_transactions terminal_tx ON terminal_tx.event_id=op.terminal_event_id LEFT JOIN game_economy_outbox terminal_outbox ON terminal_outbox.event_id=op.terminal_event_id LEFT JOIN public.zone_player_drop_abort_audit abort_audit ON abort_audit.source_namespace=op.source_namespace AND abort_audit.operation_key=op.operation_key";

fn decode_record(
    row: postgres::Row,
    connection_binding_digest: String,
) -> Result<PlayerDropOperationRecord, String> {
    let operation: PlayerDropPreparedOperation =
        serde_json::from_value(row.get("operation")).map_err(|e| e.to_string())?;
    operation.validate_integrity()?;
    let binding = operation.binding();
    let intent = operation.intent();
    if row.get::<_, String>("operation_key") != binding.operation_key()
        || row.get::<_, String>("world_effect_id") != intent.world_effect_id()
        || row.get::<_, String>("zone_id") != binding.zone_id()
        || row.get::<_, String>("owner_id") != binding.owner_id()
        || row.get::<_, i64>("owner_epoch") as u64 != binding.owner_epoch()
        || row.get::<_, String>("operation_digest") != operation.operation_digest()
        || row.get::<_, String>("intent_digest") != intent.intent_digest()
        || row.get::<_, i64>("expires_at_ms") as u64 != intent.expires_at_unix_ms()
    {
        return Err("player drop operation immutable binding conflict".into());
    }
    let status = PlayerDropOperationStatus::parse(&row.get::<_, String>("status"))?;
    let receipt = if matches!(
        status,
        PlayerDropOperationStatus::Prepared | PlayerDropOperationStatus::Aborted
    ) {
        if row.get::<_, Option<String>>("event_id").is_some() {
            return Err("prepared player drop has a committed event".into());
        }
        None
    } else {
        let receipt: EconomyTransactionReceipt = serde_json::from_value(
            row.get::<_, Option<serde_json::Value>>("receipt")
                .ok_or("player drop receipt missing")?,
        )
        .map_err(|e| e.to_string())?;
        let event: EconomyOutboxEvent = serde_json::from_value(
            row.get::<_, Option<serde_json::Value>>("payload")
                .ok_or("player drop outbox proof missing")?,
        )
        .map_err(|e| e.to_string())?;
        validate_stored_economy_transaction(&event, &receipt)?;
        if event.player_drop_source.as_ref() != Some(&PlayerDropSourceProjection::new(&operation)?)
            || row.get::<_, Option<String>>("event_id").as_deref()
                != Some(receipt.event_id.as_str())
        {
            return Err("player drop source receipt differs from custody row".into());
        }
        Some(receipt)
    };
    if status == PlayerDropOperationStatus::Consumed {
        let receipt: EconomyTransactionReceipt = serde_json::from_value(
            row.get::<_, Option<serde_json::Value>>("terminal_receipt")
                .ok_or("consumed player drop receipt missing")?,
        )
        .map_err(|e| e.to_string())?;
        let event: EconomyOutboxEvent = serde_json::from_value(
            row.get::<_, Option<serde_json::Value>>("terminal_payload")
                .ok_or("consumed player drop outbox missing")?,
        )
        .map_err(|e| e.to_string())?;
        validate_stored_economy_transaction(&event, &receipt)?;
        let projection = ground_drop_projection_from_envelope(&event.envelope, &event.event_id)?
            .ok_or("consumed player drop projection missing")?;
        validate_pickup_intent(&operation, &event.envelope, &projection.intent)?;
        if row.get::<_, Option<String>>("terminal_event_id").as_deref()
            != Some(event.event_id.as_str())
            || row
                .get::<_, Option<String>>("terminal_account_id")
                .as_deref()
                != Some(projection.account_id.as_str())
            || row.get::<_, Option<i32>>("terminal_character_index")
                != Some(projection.character_index)
        {
            return Err("player drop terminal recipient proof conflict".into());
        }
    }
    let abort_audit = if status == PlayerDropOperationStatus::Aborted {
        let value = row
            .get::<_, Option<serde_json::Value>>("abort_audit")
            .ok_or("aborted player drop receipt missing")?;
        let digest = row
            .get::<_, Option<String>>("abort_digest")
            .ok_or("aborted player drop digest missing")?;
        let audit: PlayerDropAbortAudit =
            serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        audit.validate_operation(&operation)?;
        if row
            .get::<_, Option<String>>("audited_recovery_id")
            .as_deref()
            != Some(audit.recovery_id())
            || row
                .get::<_, Option<String>>("audited_recovery_owner_id")
                .as_deref()
                != Some(audit.recovery_owner_id())
            || row.get::<_, Option<i64>>("audited_recovery_epoch")
                != Some(audit.recovery_epoch() as i64)
            || audit.digest()? != digest
            || row.get::<_, Option<serde_json::Value>>("audited_abort_receipt") != Some(value)
            || row.get::<_, Option<String>>("audited_abort_digest") != Some(digest)
            || row.get::<_, Option<i64>>("aborted_at_ms") != Some(audit.aborted_at_ms() as i64)
            || row.get::<_, bool>("original_ledger_exists")
            || row.get::<_, bool>("original_outbox_exists")
        {
            return Err("player drop aborted receipt or nonpublication proof conflict".into());
        }
        Some(audit)
    } else {
        None
    };
    Ok(PlayerDropOperationRecord {
        source_namespace: row.get("source_namespace"),
        connection_binding_digest,
        operation,
        status,
        receipt,
        terminal_event_id: row.get("terminal_event_id"),
        abort_audit,
    })
}

impl PostgresEconomyStore {
    /// A fresh preparation is fenced; an exact old operation is recovered by
    /// lookup, never by treating a missing receipt as permission to replay.
    pub fn prepare_player_drop_operation(
        &self,
        permit: &ZoneEconomyWritePermit,
        source: &PreparedPlayerDropAccountSource,
    ) -> Result<(), String> {
        permit.validate_target_database(&self.database_url)?;
        if source.database_url() != self.database_url {
            return Err("player drop source and ledger require the same PostgreSQL connection configuration".into());
        }
        let operation = source.operation();
        operation.validate_integrity()?;
        let binding = operation.binding();
        if permit.fence().owner_id() != binding.owner_id() {
            return Err("player drop source owner mismatch".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        permit.lock_and_validate(
            &mut tx,
            &self.database_url,
            binding.zone_id(),
            binding.owner_epoch(),
        )?;
        let encoded = serde_json::to_value(operation).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO public.zone_player_drop_operations(operation_key,world_effect_id,zone_id,owner_id,owner_epoch,operation_digest,intent_digest,operation,expires_at_ms,status,prepared_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'Prepared',$10) ON CONFLICT(source_namespace,operation_key) DO NOTHING",
            &[&binding.operation_key(),&operation.intent().world_effect_id(),&binding.zone_id(),&binding.owner_id(),&(binding.owner_epoch() as i64),&operation.operation_digest(),&operation.intent().intent_digest(),&encoded,&(operation.intent().expires_at_unix_ms() as i64),&(now_ms() as i64)]).map_err(|e|e.to_string())?;
        let stored: serde_json::Value = tx
            .query_one(
                "SELECT operation FROM public.zone_player_drop_operations WHERE operation_key=$1 AND source_namespace=current_schema()",
                &[&binding.operation_key()],
            )
            .map_err(|e| e.to_string())?
            .get(0);
        if stored != encoded {
            return Err("player drop prepared operation identity conflict".into());
        }
        tx.commit().map_err(|_| permit.commit_outcome_unknown())
    }

    pub fn commit_player_drop_operation(
        &self,
        permit: &ZoneEconomyWritePermit,
        source: &PreparedPlayerDropAccountSource,
    ) -> Result<EconomyTransactionReceipt, String> {
        permit.validate_target_database(&self.database_url)?;
        if source.database_url() != self.database_url
            || permit.fence().owner_id() != source.binding().owner_id()
        {
            return Err("player drop trusted source binding mismatch".into());
        }
        let participant = PlayerDropLedgerSource::new(source)?;
        let envelope = source_envelope(source.operation(), participant.legs.clone());
        self.transact_with_sources(&envelope, None, Some(permit), Some(&participant))
            .map(|(receipt, _)| receipt)
    }

    /// Read-only after takeover/Unknown. No DDL, new preparation, debit or refund.
    pub fn lookup_player_drop_operation(
        &self,
        binding: &PlayerDropOperationBinding,
    ) -> Result<Option<PlayerDropOperationRecord>, String> {
        let mut client = self.player_drop_connect()?;
        let row = client
            .query_opt(
                &format!("{RECORD_QUERY} WHERE op.operation_key=$1"),
                &[&binding.operation_key()],
            )
            .map_err(|e| e.to_string())?;
        let record = row
            .map(|row| decode_record(row, connection_binding_digest(&self.database_url)))
            .transpose()?;
        if record
            .as_ref()
            .is_some_and(|record| record.operation.binding() != binding)
        {
            return Err("player drop lookup binding conflict".into());
        }
        Ok(record)
    }

    /// Includes Prepared, all live Committed effects (whether ACKed or not),
    /// and terminal tombstones needed to remove stale checkpoint objects.
    pub fn enumerate_zone_player_drop_operations(
        &self,
        zone_id: &str,
        after_operation_key: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PlayerDropOperationRecord>, String> {
        if zone_id.is_empty() || !(1..=256).contains(&limit) {
            return Err("invalid bounded player drop enumeration".into());
        }
        let mut client = self.player_drop_connect()?;
        let rows=client.query(&format!("{RECORD_QUERY} WHERE op.zone_id=$1 AND op.operation_key>$2 ORDER BY op.operation_key LIMIT $3"),
            &[&zone_id,&after_operation_key.unwrap_or(""),&(limit as i64)]).map_err(|e|e.to_string())?;
        rows.into_iter()
            .map(|row| decode_record(row, connection_binding_digest(&self.database_url)))
            .collect()
    }

    /// ACK is observation only. It never changes custody, expiry or assets.
    pub fn acknowledge_player_drop_effect(
        &self,
        permit: &ZoneEconomyWritePermit,
        record: &PlayerDropOperationRecord,
        sequence: u64,
    ) -> Result<(), String> {
        if record.connection_binding_digest != connection_binding_digest(&self.database_url) {
            return Err("player drop observation source authority mismatch".into());
        }
        if sequence > i64::MAX as u64
            || matches!(
                record.status,
                PlayerDropOperationStatus::Prepared | PlayerDropOperationStatus::Aborted
            )
        {
            return Err("invalid player drop observation".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        let zone = record.operation.binding().zone_id();
        let fence = permit.fence();
        permit.lock_and_validate(&mut tx, &self.database_url, zone, fence.fencing_token())?;
        let state = match record.status {
            PlayerDropOperationStatus::Committed => "Committed",
            PlayerDropOperationStatus::Consumed => "Consumed",
            PlayerDropOperationStatus::Expired => "Expired",
            _ => unreachable!(),
        };
        let now = now_ms() as i64;
        let epoch = fence.fencing_token() as i64;
        let sequence = sequence as i64;
        let updated=tx.execute("UPDATE public.zone_player_drop_operations SET ack_owner_id=$2,ack_owner_epoch=$3,ack_sequence=$4,ack_at_ms=$5 WHERE source_namespace=current_schema() AND world_effect_id=$1 AND intent_digest=$6 AND status=$7 AND (ack_owner_epoch IS NULL OR ack_owner_epoch<$3 OR (ack_owner_epoch=$3 AND ack_owner_id=$2 AND ack_sequence<=$4))",
            &[&record.operation.intent().world_effect_id(),&fence.owner_id(),&epoch,&sequence,&now,&record.operation.intent().intent_digest(),&state]).map_err(|e|e.to_string())?;
        if updated != 1 {
            return Err("player drop observation is stale or conflicts with custody".into());
        }
        tx.execute("INSERT INTO public.zone_player_drop_observation_audit(world_effect_id,observer_owner_id,observer_epoch,observer_sequence,observed_state,observed_at_ms) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
            &[&record.operation.intent().world_effect_id(),&fence.owner_id(),&epoch,&sequence,&state,&now]).map_err(|e|e.to_string())?;
        let audit=tx.query_one("SELECT observer_owner_id,observed_state FROM public.zone_player_drop_observation_audit WHERE source_namespace=current_schema() AND world_effect_id=$1 AND observer_epoch=$2 AND observer_sequence=$3",
            &[&record.operation.intent().world_effect_id(),&epoch,&sequence]).map_err(|e|e.to_string())?;
        if audit.get::<_, String>(0) != fence.owner_id() || audit.get::<_, String>(1) != state {
            return Err(
                "player drop observation sequence was already used for different evidence".into(),
            );
        }
        tx.commit().map_err(|_| permit.commit_outcome_unknown())
    }

    pub fn expire_player_drop_effect(
        &self,
        permit: &ZoneEconomyWritePermit,
        record: &PlayerDropOperationRecord,
    ) -> Result<(), String> {
        if record.connection_binding_digest != connection_binding_digest(&self.database_url) {
            return Err("player drop expiry source authority mismatch".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        permit.lock_and_validate(
            &mut tx,
            &self.database_url,
            record.operation.binding().zone_id(),
            permit.fence().fencing_token(),
        )?;
        let updated=tx.execute("UPDATE public.zone_player_drop_operations SET status='Expired',terminal_at_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint WHERE source_namespace=current_schema() AND world_effect_id=$1 AND intent_digest=$2 AND status='Committed' AND expires_at_ms<=floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[&record.operation.intent().world_effect_id(),&record.operation.intent().intent_digest()]).map_err(|e|e.to_string())?;
        if updated != 1 {
            return Err("player drop is not an expired live effect".into());
        }
        tx.commit().map_err(|_| permit.commit_outcome_unknown())
    }
}

/// Runs within the pickup's balance/receipt/outbox transaction. A changed
/// presentation generation cannot create another terminal credit.
pub(super) fn consume_effect_in_transaction(
    tx: &mut Transaction<'_>,
    envelope: &EconomyTransactionEnvelope,
    event_id: &str,
    projection: &GroundDropProjectionIntent,
) -> Result<(), String> {
    let Some(effect) = projection.player_drop_effect.as_ref() else {
        return Ok(());
    };
    if envelope.idempotency_key != format!("player-drop-pickup:{}", effect.world_effect_id()) {
        return Err("player drop pickup key does not bind its world effect".into());
    }
    let row=tx.query_opt("SELECT operation,status,terminal_event_id FROM public.zone_player_drop_operations WHERE source_namespace=current_schema() AND world_effect_id=$1 FOR UPDATE",&[&effect.world_effect_id()]).map_err(|e|e.to_string())?.ok_or("player drop effect has no durable source custody")?;
    let operation: PlayerDropPreparedOperation =
        serde_json::from_value(row.get(0)).map_err(|e| e.to_string())?;
    validate_pickup_intent(&operation, envelope, projection)?;
    let status: String = row.get(1);
    if status != "Committed" {
        return Err("player drop world effect is already terminal or not committed".into());
    }
    let updated=tx.execute("UPDATE public.zone_player_drop_operations SET status='Consumed',terminal_event_id=$2,terminal_at_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint,terminal_account_id=$3,terminal_character_index=$4 WHERE source_namespace=current_schema() AND world_effect_id=$1 AND status='Committed' AND expires_at_ms>floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
        &[&effect.world_effect_id(),&event_id,&projection.identity.account_id,&projection.identity.character_index]).map_err(|e|e.to_string())?;
    if updated != 1 {
        return Err("player drop world effect expired before pickup commit".into());
    }
    Ok(())
}

fn validate_pickup_intent(
    operation: &PlayerDropPreparedOperation,
    envelope: &EconomyTransactionEnvelope,
    projection: &GroundDropProjectionIntent,
) -> Result<(), String> {
    operation.validate_integrity()?;
    let effect = projection
        .player_drop_effect
        .as_ref()
        .ok_or("player drop pickup immutable effect identity missing")?;
    if envelope.idempotency_key != format!("player-drop-pickup:{}", effect.world_effect_id())
        || envelope
            .metadata
            .get("playerDropWorldEffectId")
            .map(String::as_str)
            != Some(effect.world_effect_id())
        || envelope
            .metadata
            .get("playerDropIntentDigest")
            .map(String::as_str)
            != Some(effect.intent_digest())
        || envelope.metadata.get("playerDropExpiresAtUnixMs")
            != Some(&effect.expires_at_unix_ms().to_string())
    {
        return Err("player drop pickup immutable envelope binding conflict".into());
    }
    let intent = operation.intent();
    let drop = &projection.drop;
    if intent.world_effect_id() != effect.world_effect_id()
        || intent.intent_digest() != effect.intent_digest()
        || intent.expires_at_unix_ms() != effect.expires_at_unix_ms()
        || operation.binding().zone_id() != envelope.zone_id
        || intent.position().x != drop.x
        || intent.position().y != drop.y
    {
        return Err("player drop pickup differs from immutable world intent".into());
    }
    let exact = match intent.outcome() {
        PlayerDropOutcome::Gold { amount } => {
            drop.quantity == *amount
                && drop.loot == (GroundDropLootSnapshot::Gold { amount: *amount })
        }
        PlayerDropOutcome::Item { quantity, loot } => {
            drop.quantity == *quantity && drop.loot == *loot
        }
        _ => false,
    };
    if !exact {
        return Err("player drop pickup payload differs from frozen source asset".into());
    }
    Ok(())
}

/// Ignore only the temporary presentation ID. The canonical recipient remains
/// in the stored projection and a different claimant receives no asset projection.
pub(super) fn duplicate_pickup_effect(
    stored: &EconomyTransactionEnvelope,
    requested: &EconomyTransactionEnvelope,
) -> Result<Option<DuplicateBusinessEffect>, String> {
    if stored.metadata.get("playerDropWorldEffectId").is_none()
        && requested.metadata.get("playerDropWorldEffectId").is_none()
    {
        return Ok(None);
    }
    let decode =
        |envelope: &EconomyTransactionEnvelope| -> Result<GroundDropProjectionIntent, String> {
            serde_json::from_str(
                envelope
                    .metadata
                    .get("groundDropProjectionV1")
                    .ok_or("player drop pickup projection missing")?,
            )
            .map_err(|e| e.to_string())
        };
    let mut left = decode(stored)?;
    let mut right = decode(requested)?;
    if left.player_drop_effect.is_none() || left.player_drop_effect != right.player_drop_effect {
        return Err("player drop duplicate effect identity conflict".into());
    }
    let same_recipient = left.identity == right.identity;
    left.drop.object_id = 0;
    right.drop.object_id = 0;
    // Comparison cannot grant projection ownership: it only chooses Exact vs
    // SettledElsewhere, whose latter path returns no recipient asset mutation.
    right.identity = left.identity.clone();
    let mut left_metadata = stored.metadata.clone();
    let mut right_metadata = requested.metadata.clone();
    for metadata in [&mut left_metadata, &mut right_metadata] {
        metadata.remove("objectId");
        metadata.remove("groundDropProjectionV1");
        metadata.remove("characterName");
    }
    if left != right
        || left_metadata != right_metadata
        || stored.idempotency_key != requested.idempotency_key
        || stored.zone_id != requested.zone_id
        || stored.transaction_kind != requested.transaction_kind
        || stored.legs.len() != requested.legs.len()
        || !stored
            .legs
            .iter()
            .zip(&requested.legs)
            .all(|(left, right)| {
                left.delta == right.delta
                    && left.balance.asset_kind == right.balance.asset_kind
                    && left.balance.asset_key == right.balance.asset_key
            })
        || (same_recipient && stored.legs != requested.legs)
    {
        return Err("player drop pickup duplicate immutable payload conflict".into());
    }
    Ok(Some(if same_recipient {
        DuplicateBusinessEffect::Exact
    } else {
        DuplicateBusinessEffect::SettledElsewhere
    }))
}

#[cfg(test)]
mod pg_tests;
#[cfg(test)]
mod tests;

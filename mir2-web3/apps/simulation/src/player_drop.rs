//! Immutable durable player-drop evidence. Deserializing these values confers
//! no authority to debit a character or publish a world object.
use crate::{
    CharacterSaveRecord, GroundDropItemPayload, GroundDropLootSnapshot, ZoneEconomyWritePermit,
    ZoneKey,
};
use mir2_protocol::Point;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

fn digest<T: Serialize>(domain: &str, value: &T) -> Result<String, String> {
    fn canonical(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(fields) => serde_json::Value::Object(
                fields
                    .into_iter()
                    .collect::<std::collections::BTreeMap<_, _>>()
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
    let value = canonical(serde_json::to_value(value).map_err(|e| e.to_string())?);
    // Explicit recursive key ordering also works when another workspace crate
    // enables serde_json/preserve_order through Cargo feature unification.
    let mut hash = Sha256::new();
    hash.update(domain.as_bytes());
    hash.update([0]);
    hash.update(serde_json::to_vec(&value).map_err(|e| e.to_string())?);
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Shared checked successor used by the trusted ordinary-item UID authority.
pub fn player_drop_next_ordinary_item_uid(previous: u64) -> Result<u64, String> {
    let next = previous
        .checked_add(1)
        .ok_or("player-drop item UID exhausted")?;
    let next = crate::monthly_card::skip_monthly_card_reserved_unique_id(next);
    if next == 0 || next <= previous || next > crate::USER_ITEM_UID_MAX {
        return Err("player-drop item UID exhausted".into());
    }
    Ok(next)
}

fn assigned_item_ids_valid(
    item: &mir2_protocol::UserItem,
    ids: &mut std::collections::BTreeSet<u64>,
) -> bool {
    item.unique_id > 0
        && item.unique_id <= crate::USER_ITEM_UID_MAX
        && ids.insert(item.unique_id)
        && item
            .slots
            .iter()
            .flatten()
            .all(|child| assigned_item_ids_valid(child, ids))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDropOperationBinding {
    operation_nonce: String,
    source_rpc_session: String,
    account_id: String,
    character_index: i32,
    zone_id: String,
    owner_id: String,
    owner_epoch: u64,
    source_sequence: u64,
}
impl PlayerDropOperationBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation_nonce: String,
        source_rpc_session: String,
        account_id: String,
        character_index: i32,
        zone_id: String,
        owner_id: String,
        owner_epoch: u64,
        source_sequence: u64,
    ) -> Result<Self, String> {
        let value = Self {
            operation_nonce,
            source_rpc_session,
            account_id,
            character_index,
            zone_id,
            owner_id,
            owner_epoch,
            source_sequence,
        };
        value.validate_integrity()?;
        Ok(value)
    }
    pub fn validate_integrity(&self) -> Result<(), String> {
        let valid = |s: &str| {
            !s.is_empty() && s.len() <= 1024 && s.trim() == s && !s.chars().any(char::is_control)
        };
        if [
            &self.operation_nonce,
            &self.source_rpc_session,
            &self.account_id,
            &self.zone_id,
            &self.owner_id,
        ]
        .into_iter()
        .any(|s| !valid(s))
            || self.character_index < 0
            || self.owner_epoch == 0
            || self.owner_epoch > i64::MAX as u64
            || self.source_sequence == 0
        {
            return Err("invalid player-drop operation binding".into());
        }
        Ok(())
    }
    pub fn operation_nonce(&self) -> &str {
        &self.operation_nonce
    }
    pub fn source_rpc_session(&self) -> &str {
        &self.source_rpc_session
    }
    pub fn account_id(&self) -> &str {
        &self.account_id
    }
    pub fn character_index(&self) -> i32 {
        self.character_index
    }
    pub fn zone_id(&self) -> &str {
        &self.zone_id
    }
    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }
    pub fn owner_epoch(&self) -> u64 {
        self.owner_epoch
    }
    pub fn source_sequence(&self) -> u64 {
        self.source_sequence
    }
    pub fn operation_key(&self) -> String {
        format!(
            "player-drop:{}",
            digest("mir2/player-drop/operation/v1", self).expect("binding serializes")
        )
    }
    pub fn world_effect_id(&self) -> String {
        format!(
            "player-drop-effect:{}",
            digest("mir2/player-drop/world-effect/v1", self).expect("binding serializes")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum PlayerDropOutcome {
    Gold {
        amount: u32,
    },
    Item {
        quantity: u32,
        loot: GroundDropLootSnapshot,
    },
    DestroyedItem {
        unique_id: u64,
        quantity: u32,
        item_key: String,
        exact_item: GroundDropItemPayload,
    },
}
impl PlayerDropOutcome {
    pub fn quantity(&self) -> u32 {
        match self {
            Self::Gold { amount } => *amount,
            Self::Item { quantity, .. } | Self::DestroyedItem { quantity, .. } => *quantity,
        }
    }
    pub fn validate_integrity(&self) -> Result<(), String> {
        if self.quantity() == 0 {
            return Err("empty player-drop outcome".into());
        }
        match self {
            Self::Gold { .. } => Ok(()),
            Self::Item {
                quantity,
                loot:
                    GroundDropLootSnapshot::InventoryItem {
                        exact_item: Some(exact),
                        ..
                    },
            } if u32::from(exact.item.count) == *quantity
                && exact.uid_assigned
                && assigned_item_ids_valid(&exact.item, &mut Default::default()) =>
            {
                crate::runtime::validate_player_drop_exact_item(&exact.item)
            }
            Self::DestroyedItem {
                unique_id,
                quantity,
                item_key,
                exact_item,
            } if *unique_id > 0
                && !item_key.is_empty()
                && u32::from(exact_item.item.count) == *quantity =>
            {
                crate::runtime::validate_player_drop_exact_item(&exact_item.item)
            }
            _ => Err("player-drop requires its complete exact item carrier".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDropWorldIntent {
    binding: PlayerDropOperationBinding,
    world_effect_id: String,
    zone: ZoneKey,
    position: Point,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    outcome: PlayerDropOutcome,
    intent_digest: String,
}
impl PlayerDropWorldIntent {
    pub fn new(
        binding: PlayerDropOperationBinding,
        zone: ZoneKey,
        position: Point,
        created_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        outcome: PlayerDropOutcome,
    ) -> Result<Self, String> {
        let world_effect_id = binding.world_effect_id();
        let mut value = Self {
            binding,
            world_effect_id,
            zone,
            position,
            created_at_unix_ms,
            expires_at_unix_ms,
            outcome,
            intent_digest: String::new(),
        };
        value.intent_digest = value.compute_digest()?;
        value.validate_integrity()?;
        Ok(value)
    }
    fn compute_digest(&self) -> Result<String, String> {
        digest(
            "mir2/player-drop/world-intent/v1",
            &(
                &self.binding,
                &self.world_effect_id,
                &self.zone,
                &self.position,
                self.created_at_unix_ms,
                self.expires_at_unix_ms,
                &self.outcome,
            ),
        )
    }
    pub fn validate_integrity(&self) -> Result<(), String> {
        self.binding.validate_integrity()?;
        self.outcome.validate_integrity()?;
        if self.world_effect_id != self.binding.world_effect_id()
            || self.zone.shard_id.trim().is_empty()
            || self.zone.map_file_name.trim().is_empty()
            || self.created_at_unix_ms == 0
            || self.expires_at_unix_ms <= self.created_at_unix_ms
            || self.expires_at_unix_ms > i64::MAX as u64
            || self.compute_digest()? != self.intent_digest
        {
            return Err("player-drop intent integrity mismatch".into());
        }
        Ok(())
    }
    pub fn binding(&self) -> &PlayerDropOperationBinding {
        &self.binding
    }
    pub fn world_effect_id(&self) -> &str {
        &self.world_effect_id
    }
    pub fn zone(&self) -> &ZoneKey {
        &self.zone
    }
    pub fn position(&self) -> &Point {
        &self.position
    }
    pub fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }
    pub fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }
    pub fn outcome(&self) -> &PlayerDropOutcome {
        &self.outcome
    }
    pub fn intent_digest(&self) -> &str {
        &self.intent_digest
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDropPreparedOperation {
    intent: PlayerDropWorldIntent,
    /// Actual complete durable CAS preimage (which may lag runtime movement or
    /// a previously committed ledger projection at the same save revision).
    before: CharacterSaveRecord,
    /// Exact trusted working source from which this operation's debit derives.
    runtime_before: CharacterSaveRecord,
    checkpoint: CharacterSaveRecord,
    operation_digest: String,
}
impl PlayerDropPreparedOperation {
    pub(crate) fn new(
        intent: PlayerDropWorldIntent,
        before: CharacterSaveRecord,
        runtime_before: CharacterSaveRecord,
        checkpoint: CharacterSaveRecord,
    ) -> Result<Self, String> {
        let mut value = Self {
            intent,
            before,
            runtime_before,
            checkpoint,
            operation_digest: String::new(),
        };
        value.operation_digest = value.compute_digest()?;
        value.validate_integrity()?;
        Ok(value)
    }
    fn compute_digest(&self) -> Result<String, String> {
        digest(
            "mir2/player-drop/prepared-source/v1",
            &(
                &self.intent,
                &self.before,
                &self.runtime_before,
                &self.checkpoint,
            ),
        )
    }
    pub fn validate_integrity(&self) -> Result<(), String> {
        self.intent.validate_integrity()?;
        if self.before.character.index != self.binding().character_index()
            || self.before.character.index != self.runtime_before.character.index
            || self.before.character.name != self.runtime_before.character.name
            || self.before.character.class != self.runtime_before.character.class
            || self.before.character.gender != self.runtime_before.character.gender
            || self.runtime_before.character != self.checkpoint.character
            || self.before.revision != self.runtime_before.revision
            || self.before.revision.checked_add(1) != Some(self.checkpoint.revision)
            || self.runtime_before.map_file_name != self.checkpoint.map_file_name
            || self.compute_digest()? != self.operation_digest
        {
            return Err("player-drop complete source integrity mismatch".into());
        }
        Ok(())
    }
    pub fn binding(&self) -> &PlayerDropOperationBinding {
        self.intent.binding()
    }
    pub fn intent(&self) -> &PlayerDropWorldIntent {
        &self.intent
    }
    pub fn before(&self) -> &CharacterSaveRecord {
        &self.before
    }
    pub fn runtime_before(&self) -> &CharacterSaveRecord {
        &self.runtime_before
    }
    pub fn checkpoint(&self) -> &CharacterSaveRecord {
        &self.checkpoint
    }
    pub fn operation_digest(&self) -> &str {
        &self.operation_digest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerDropEffectState {
    Live,
    Consumed,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDropCommitReceipt {
    operation_key: String,
    world_effect_id: String,
    operation_digest: String,
    intent_digest: String,
    source_revision: u64,
    state: PlayerDropEffectState,
}
impl PlayerDropCommitReceipt {
    pub fn new(
        operation_key: String,
        world_effect_id: String,
        operation_digest: String,
        intent_digest: String,
        source_revision: u64,
        state: PlayerDropEffectState,
    ) -> Self {
        Self {
            operation_key,
            world_effect_id,
            operation_digest,
            intent_digest,
            source_revision,
            state,
        }
    }
    pub fn operation_key(&self) -> &str {
        &self.operation_key
    }
    pub fn world_effect_id(&self) -> &str {
        &self.world_effect_id
    }
    pub fn operation_digest(&self) -> &str {
        &self.operation_digest
    }
    pub fn intent_digest(&self) -> &str {
        &self.intent_digest
    }
    pub fn source_revision(&self) -> u64 {
        self.source_revision
    }
    pub fn state(&self) -> PlayerDropEffectState {
        self.state
    }
    pub fn validate_operation(
        &self,
        operation: &PlayerDropPreparedOperation,
    ) -> Result<(), String> {
        operation.validate_integrity()?;
        if self.operation_key != operation.binding().operation_key()
            || self.world_effect_id != operation.intent().world_effect_id()
            || self.operation_digest != operation.operation_digest()
            || self.intent_digest != operation.intent().intent_digest()
            || self.source_revision != operation.checkpoint().revision
        {
            return Err("player-drop canonical receipt binding mismatch".into());
        }
        Ok(())
    }
}

/// Frozen internal journal result. It carries evidence only; the replica
/// installer separately requires its bound isolated shadow configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDropJournalResult {
    pub operation: PlayerDropPreparedOperation,
    pub receipt: PlayerDropCommitReceipt,
    pub request: PlayerDropRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation: Option<crate::PlayerDropPlacementReservation>,
}
impl PlayerDropJournalResult {
    pub fn validate_integrity(&self) -> Result<(), String> {
        self.receipt.validate_operation(&self.operation)?;
        self.request.validate_operation(&self.operation)?;
        let intent = self.operation.intent();
        let expected_kind = match intent.outcome() {
            PlayerDropOutcome::Gold { .. } => crate::PlayerDropPlacementKind::Gold,
            PlayerDropOutcome::Item { .. } => crate::PlayerDropPlacementKind::Item,
            PlayerDropOutcome::DestroyedItem { .. } => {
                return if self.reservation.is_none() {
                    Ok(())
                } else {
                    Err("destroyed player-drop journal unexpectedly reserves a world slot".into())
                };
            }
        };
        let reservation = self
            .reservation
            .as_ref()
            .ok_or("player-drop journal lacks frozen placement")?;
        let range = match expected_kind {
            crate::PlayerDropPlacementKind::Gold => 5,
            crate::PlayerDropPlacementKind::Item => 1,
        };
        if reservation.world_effect_id() != intent.world_effect_id()
            || reservation.zone() != intent.zone()
            || reservation.position() != intent.position()
            || reservation.kind() != expected_kind
            || reservation.reserved_at_unix_ms() == 0
            || reservation.reserved_at_unix_ms() > intent.created_at_unix_ms()
            || reservation.player_object_id() == 0
            || reservation.session_id().as_str() != self.operation.binding().source_rpc_session()
            || reservation.origin() != &self.operation.runtime_before().position
            || (i64::from(reservation.origin().x) - i64::from(reservation.position().x)).abs()
                > range
            || (i64::from(reservation.origin().y) - i64::from(reservation.position().y)).abs()
                > range
        {
            return Err("player-drop journal frozen placement binding mismatch".into());
        }
        Ok(())
    }
}

/// Non-serializable evidence adapter used only after the server's ledger has
/// verified a canonical committed receipt (including during quiesced recovery).
#[derive(Debug, Clone)]
pub struct PlayerDropCommittedEffect {
    intent: PlayerDropWorldIntent,
    state: PlayerDropEffectState,
}
impl PlayerDropCommittedEffect {
    pub(crate) fn from_replica_evidence(
        operation: PlayerDropPreparedOperation,
        receipt: PlayerDropCommitReceipt,
    ) -> Result<Self, String> {
        receipt.validate_operation(&operation)?;
        Ok(Self {
            intent: operation.intent,
            state: receipt.state,
        })
    }
    pub fn from_verified_receipt(
        permit: &ZoneEconomyWritePermit,
        database_url: &str,
        operation: PlayerDropPreparedOperation,
        receipt: PlayerDropCommitReceipt,
    ) -> Result<Self, String> {
        permit.validate_target_database(database_url)?;
        receipt.validate_operation(&operation)?;
        let intent = operation.intent;
        // A successor owner may reconcile an earlier owner's committed effect.
        // Requiring the old epoch here would make durable recovery impossible.
        if permit.fence().zone_id() != intent.binding().zone_id() {
            return Err("player-drop recovery Zone mismatch".into());
        }
        Ok(Self {
            intent,
            state: receipt.state,
        })
    }
    pub fn intent(&self) -> &PlayerDropWorldIntent {
        &self.intent
    }
    pub fn state(&self) -> PlayerDropEffectState {
        self.state
    }
    #[cfg(test)]
    pub(crate) fn for_zone_test(
        intent: PlayerDropWorldIntent,
        state: PlayerDropEffectState,
    ) -> Self {
        intent
            .validate_integrity()
            .expect("valid Zone fixture intent");
        Self { intent, state }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum PlayerDropRequest {
    Gold {
        amount: u32,
    },
    Item {
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
    },
}
impl PlayerDropRequest {
    /// Pure validation of the already recorded source selection. This does
    /// not admit a write or reevaluate current gameplay/drop eligibility.
    pub fn validate_operation(
        &self,
        operation: &PlayerDropPreparedOperation,
    ) -> Result<(), String> {
        operation.validate_integrity()?;
        crate::runtime::validate_player_drop_recorded_request(self, operation)
    }
}

#[cfg(test)]
#[path = "player_drop_tests.rs"]
mod tests;

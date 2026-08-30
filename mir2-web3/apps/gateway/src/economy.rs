//! Gate 17 durable game-economy transaction, outbox, inbox, and reconciliation.
//!
//! This module is intentionally separate from the administrative event bus.
//! Gold, unique items, consumptions, and two-sided trades are authoritative
//! gameplay effects: balance writes, the idempotency receipt, and the outbox
//! event are committed in one PostgreSQL transaction.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::ServerPacket;
use mir2_simulation::{
    ActiveSessionIdentity, GroundDropLootSnapshot, InProcessWorldRuntime,
    SharedAccountInventoryTransactionKind, SharedAccountInventoryTransactionReceipt,
    SharedTradeOffer, WorldRuntime, WorldSnapshot,
};
use postgres::{Client, NoTls, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::routing::{
    SharedAccountInventoryCommand, SharedAccountInventoryCommandEnvelope,
    SharedAccountInventoryCommitOutcome, SharedAccountInventoryExecutionContext,
    SharedAccountInventoryService, SharedTradeSettlementOutcome,
};

const ECONOMY_EVENT_DOMAIN: &[u8] = b"obelisk.mir2.game-economy-event.v1\0";
const ECONOMY_BUSINESS_EFFECT_DOMAIN: &[u8] = b"obelisk.mir2.game-economy-business-effect.v1\0";
const ECONOMY_RECEIPT_DOMAIN: &[u8] = b"obelisk.mir2.game-economy-receipt.v1\0";
const ECONOMY_TRADE_SETTLEMENT_DOMAIN: &[u8] = b"obelisk.mir2.game-economy-trade-settlement.v1\0";
const ECONOMY_BOOTSTRAP_DOMAIN: &[u8] = b"obelisk.mir2.game-economy-bootstrap.v1\0";
const ECONOMY_TRADE_DOMAIN: &[u8] = b"obelisk.mir2.game-economy-trade.v1\0";
const MAX_ECONOMY_LEGS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EconomyTransactionKind {
    Reward,
    Consume,
    Trade,
    Adjustment,
}

impl EconomyTransactionKind {
    fn as_db(self) -> &'static str {
        match self {
            Self::Reward => "reward",
            Self::Consume => "consume",
            Self::Trade => "trade",
            Self::Adjustment => "adjustment",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomyBalanceKey {
    pub account_id: String,
    pub character_index: i32,
    pub asset_kind: String,
    pub asset_key: String,
}

impl EconomyBalanceKey {
    pub fn gold(account_id: impl Into<String>, character_index: i32) -> Self {
        Self {
            account_id: account_id.into(),
            character_index,
            asset_kind: "gold".to_string(),
            asset_key: "gold".to_string(),
        }
    }

    pub fn item(
        account_id: impl Into<String>,
        character_index: i32,
        unique_item_id: impl Into<String>,
    ) -> Self {
        Self {
            account_id: account_id.into(),
            character_index,
            asset_kind: "item".to_string(),
            asset_key: unique_item_id.into(),
        }
    }

    pub fn item_quantity(
        account_id: impl Into<String>,
        character_index: i32,
        item_key: impl Into<String>,
    ) -> Self {
        Self {
            account_id: account_id.into(),
            character_index,
            asset_kind: "item_quantity".to_string(),
            asset_key: item_key.into(),
        }
    }

    pub fn experience(account_id: impl Into<String>, character_index: i32) -> Self {
        Self {
            account_id: account_id.into(),
            character_index,
            asset_kind: "experience".to_string(),
            asset_key: "experience".to_string(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("account id", self.account_id.as_str()),
            ("asset kind", self.asset_kind.as_str()),
            ("asset key", self.asset_key.as_str()),
        ] {
            if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
                return Err(format!("invalid economy {label}"));
            }
        }
        if self.character_index < 0 {
            return Err("economy character index must be non-negative".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomyLeg {
    pub balance: EconomyBalanceKey,
    pub delta: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomyTransactionEnvelope {
    pub idempotency_key: String,
    pub transaction_kind: EconomyTransactionKind,
    pub zone_id: String,
    pub fencing_generation: u64,
    pub source_sequence: u64,
    pub created_at_ms: u64,
    pub legs: Vec<EconomyLeg>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl EconomyTransactionEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        if self.idempotency_key.trim().is_empty()
            || self.idempotency_key.len() > 512
            || self.idempotency_key.chars().any(char::is_control)
        {
            return Err("invalid economy idempotency key".to_string());
        }
        if self.zone_id.trim().is_empty() || self.zone_id.len() > 256 {
            return Err("invalid economy Zone id".to_string());
        }
        if self.fencing_generation == 0 {
            return Err("economy fencing generation must be positive".to_string());
        }
        if self.legs.is_empty() || self.legs.len() > MAX_ECONOMY_LEGS {
            return Err(format!(
                "economy transaction must contain 1..={MAX_ECONOMY_LEGS} legs"
            ));
        }
        let mut accounts = BTreeSet::new();
        let mut net_by_asset = BTreeMap::<(String, String), i128>::new();
        for leg in &self.legs {
            leg.balance.validate()?;
            if leg.delta == 0 {
                return Err("economy transaction leg delta cannot be zero".to_string());
            }
            accounts.insert((leg.balance.account_id.clone(), leg.balance.character_index));
            *net_by_asset
                .entry((
                    leg.balance.asset_kind.clone(),
                    leg.balance.asset_key.clone(),
                ))
                .or_default() += i128::from(leg.delta);
        }
        match self.transaction_kind {
            EconomyTransactionKind::Reward if self.legs.iter().any(|leg| leg.delta < 0) => {
                return Err("reward transaction cannot debit an asset".to_string());
            }
            EconomyTransactionKind::Consume if self.legs.iter().any(|leg| leg.delta > 0) => {
                return Err("consume transaction cannot credit an asset".to_string());
            }
            EconomyTransactionKind::Trade => {
                if accounts.len() < 2 {
                    return Err("trade transaction requires at least two characters".to_string());
                }
                if net_by_asset.values().any(|net| *net != 0) {
                    return Err("trade transaction must conserve every asset".to_string());
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn event_id(&self) -> Result<String, String> {
        self.validate()?;
        let payload = serde_json::to_vec(self)
            .map_err(|error| format!("encode economy transaction: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(ECONOMY_EVENT_DOMAIN);
        hasher.update(payload);
        Ok(hex_lower(&hasher.finalize()))
    }

    /// Identifies the immutable asset mutation independently of the producer
    /// attempt that delivered it. Fence, sequence, and wall-clock fields still
    /// bind `event_id`, but retries after a Host restart compare this digest so
    /// the same authoritative effect can recover without becoming a new debit
    /// or credit.
    pub fn business_effect_id(&self) -> Result<String, String> {
        self.validate()?;
        let payload = serde_json::to_vec(&EconomyBusinessEffect {
            idempotency_key: &self.idempotency_key,
            transaction_kind: self.transaction_kind,
            zone_id: &self.zone_id,
            legs: &self.legs,
            metadata: &self.metadata,
        })
        .map_err(|error| format!("encode economy business effect: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(ECONOMY_BUSINESS_EFFECT_DOMAIN);
        hasher.update(payload);
        Ok(hex_lower(&hasher.finalize()))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EconomyBusinessEffect<'a> {
    idempotency_key: &'a str,
    transaction_kind: EconomyTransactionKind,
    zone_id: &'a str,
    legs: &'a [EconomyLeg],
    metadata: &'a BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomyTransactionReceipt {
    pub idempotency_key: String,
    pub event_id: String,
    pub transaction_kind: EconomyTransactionKind,
    pub committed_at_ms: u64,
    pub balances_after: BTreeMap<String, i64>,
    pub duplicate: bool,
    /// The same authoritative world drop was already settled to a different
    /// recipient. Gateway must remove the stale reappearing drop but must not
    /// project the original recipient's asset mutation into this character.
    #[serde(default)]
    pub settled_elsewhere: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomyBootstrapReceipt {
    pub account_id: String,
    pub character_index: i32,
    pub snapshot_digest: String,
    pub gold: i64,
    pub experience: i64,
    pub item_quantity: i64,
    pub item_kind_count: usize,
    pub bootstrapped_at_ms: u64,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomyOutboxEvent {
    pub event_id: String,
    pub idempotency_key: String,
    pub envelope: EconomyTransactionEnvelope,
    /// Integrity commitment for the transaction row used during projection
    /// recovery. This prevents a damaged receipt balance from authorizing a
    /// second private projection.
    #[serde(default)]
    pub receipt_digest: String,
}
/// Canonical typed materialization instructions held by a trade outbox event.
/// They are serialized into `EconomyTransactionEnvelope::metadata`, so the
/// canonical event id and receipt/outbox integrity proof bind both offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TradeProjectionIntent {
    version: u8,
    first: SharedTradeOffer,
    second: SharedTradeOffer,
}

/// Canonical private materialization instructions for a ground-drop pickup.
///
/// A Zone removes a drop as soon as the ledger transaction commits. The
/// character save is a separate durable boundary, so this intent is written in
/// the same transaction as the receipt/outbox and is the recovery authority if
/// the immediate character projection cannot be saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundDropProjectionIntent {
    version: u8,
    identity: ActiveSessionIdentity,
    drop: mir2_simulation::GroundDropSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claim_idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DurableGroundDropProjection {
    event_id: String,
    intent: GroundDropProjectionIntent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GroundDropProjectionRow {
    account_id: String,
    character_index: i32,
    intent: GroundDropProjectionIntent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DurableTradeProjection {
    event_id: String,
    own_offer: SharedTradeOffer,
    incoming_offer: SharedTradeOffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TradeProjectionRow {
    account_id: String,
    character_index: i32,
    own_offer: SharedTradeOffer,
    incoming_offer: SharedTradeOffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DuplicateBusinessEffect {
    Exact,
    SettledElsewhere,
}

fn receipt_integrity_digest(receipt: &EconomyTransactionReceipt) -> Result<String, String> {
    let payload = serde_json::to_vec(receipt)
        .map_err(|error| format!("encode economy receipt integrity payload: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(ECONOMY_RECEIPT_DOMAIN);
    hasher.update(payload);
    Ok(hex_lower(&hasher.finalize()))
}

fn same_ground_drop_effect_except_recipient(
    stored: &EconomyTransactionEnvelope,
    requested: &EconomyTransactionEnvelope,
) -> bool {
    let stored_operation = stored.metadata.get("operation").map(String::as_str);
    let requested_operation = requested.metadata.get("operation").map(String::as_str);
    let is_ground_drop = |value: Option<&str>| {
        matches!(value, Some("groundDropGoldPickup" | "groundDropItemPickup"))
    };
    if !is_ground_drop(stored_operation)
        || stored_operation != requested_operation
        || stored.idempotency_key != requested.idempotency_key
        || stored.transaction_kind != requested.transaction_kind
        || stored.zone_id != requested.zone_id
        || stored.legs.len() != requested.legs.len()
    {
        return false;
    }
    let mut stored_metadata = stored.metadata.clone();
    let mut requested_metadata = requested.metadata.clone();
    stored_metadata.remove("characterName");
    requested_metadata.remove("characterName");
    // The recovery intent contains the recipient identity. It is deliberately
    // excluded here because a stale reappearing world drop must classify as
    // settled elsewhere instead of being able to credit a second claimant.
    stored_metadata.remove("groundDropProjectionV1");
    requested_metadata.remove("groundDropProjectionV1");
    stored_metadata == requested_metadata
        && stored
            .legs
            .iter()
            .zip(&requested.legs)
            .all(|(left, right)| {
                left.delta == right.delta
                    && left.balance.asset_kind == right.balance.asset_kind
                    && left.balance.asset_key == right.balance.asset_key
            })
}

fn validate_duplicate_business_effect(
    stored_event: &EconomyOutboxEvent,
    stored_receipt: &EconomyTransactionReceipt,
    requested: &EconomyTransactionEnvelope,
) -> Result<DuplicateBusinessEffect, String> {
    let stored_event_id = stored_event.envelope.event_id()?;
    let stored_business_effect_id = stored_event.envelope.business_effect_id()?;
    let requested_business_effect_id = requested.business_effect_id()?;
    let expected_balance_keys = aggregate_legs(&stored_event.envelope.legs)?
        .keys()
        .map(balance_receipt_key)
        .collect::<Result<BTreeSet<_>, _>>()?;
    let receipt_balance_keys = stored_receipt
        .balances_after
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if stored_receipt.idempotency_key != stored_event.idempotency_key
        || stored_receipt.transaction_kind != stored_event.envelope.transaction_kind
        || stored_receipt.event_id != stored_event.event_id
        || stored_event.event_id != stored_event_id
        || stored_event.idempotency_key != stored_event.envelope.idempotency_key
        || stored_event.envelope.idempotency_key != requested.idempotency_key
        || stored_event.receipt_digest.is_empty()
        || receipt_integrity_digest(stored_receipt)? != stored_event.receipt_digest
        || expected_balance_keys != receipt_balance_keys
    {
        return Err(format!(
            "economy idempotency conflict for {}",
            requested.idempotency_key
        ));
    }
    if stored_business_effect_id == requested_business_effect_id {
        Ok(DuplicateBusinessEffect::Exact)
    } else if same_ground_drop_effect_except_recipient(&stored_event.envelope, requested) {
        Ok(DuplicateBusinessEffect::SettledElsewhere)
    } else {
        Err(format!(
            "economy idempotency conflict for {}",
            requested.idempotency_key
        ))
    }
}

fn duplicate_receipt_from_stored(
    stored_event: &EconomyOutboxEvent,
    stored_receipt: &EconomyTransactionReceipt,
    requested: &EconomyTransactionEnvelope,
) -> Result<EconomyTransactionReceipt, String> {
    let duplicate_effect =
        validate_duplicate_business_effect(stored_event, stored_receipt, requested)?;
    let mut receipt = stored_receipt.clone();
    receipt.duplicate = true;
    receipt.settled_elsewhere = duplicate_effect == DuplicateBusinessEffect::SettledElsewhere;
    Ok(receipt)
}
fn trade_projection_rows_from_envelope(
    envelope: &EconomyTransactionEnvelope,
    event_id: &str,
) -> Result<Vec<TradeProjectionRow>, String> {
    if envelope.transaction_kind != EconomyTransactionKind::Trade {
        return Ok(Vec::new());
    }
    if envelope.metadata.get("operation").map(String::as_str) != Some("playerTrade") {
        return Err("economy trade projection requires playerTrade operation".to_string());
    }
    let encoded = envelope
        .metadata
        .get("tradeProjectionV1")
        .ok_or_else(|| "economy trade projection intent is missing".to_string())?;
    let intent: TradeProjectionIntent = serde_json::from_str(encoded)
        .map_err(|error| format!("decode economy trade projection intent: {error}"))?;
    if intent.version != 1 {
        return Err("unsupported economy trade projection intent version".to_string());
    }
    let context = SharedAccountInventoryExecutionContext {
        zone_id: crate::ZoneId::new(envelope.zone_id.clone()),
        fencing_generation: envelope.fencing_generation,
        source_sequence: envelope.source_sequence,
        created_at_ms: envelope.created_at_ms,
        external_commit_authorized: true,
    };
    let expected = economy_transaction_for_trade(&context, &intent.first, &intent.second)?
        .ok_or_else(|| "economy trade projection cannot be empty".to_string())?;
    if &expected != envelope || expected.event_id()? != event_id {
        return Err("economy trade projection intent does not bind the envelope".to_string());
    }
    Ok(vec![
        TradeProjectionRow {
            account_id: intent.first.account_id.clone(),
            character_index: intent.first.character_index,
            own_offer: intent.first.clone(),
            incoming_offer: intent.second.clone(),
        },
        TradeProjectionRow {
            account_id: intent.second.account_id.clone(),
            character_index: intent.second.character_index,
            own_offer: intent.second,
            incoming_offer: intent.first,
        },
    ])
}

fn ground_drop_projection_from_envelope(
    envelope: &EconomyTransactionEnvelope,
    event_id: &str,
) -> Result<Option<GroundDropProjectionRow>, String> {
    let operation = envelope.metadata.get("operation").map(String::as_str);
    if !matches!(
        operation,
        Some("groundDropGoldPickup" | "groundDropItemPickup")
    ) {
        return Ok(None);
    }
    if envelope.transaction_kind != EconomyTransactionKind::Reward {
        return Err("economy ground-drop projection requires reward transaction".to_string());
    }
    let encoded = envelope
        .metadata
        .get("groundDropProjectionV1")
        .ok_or_else(|| "economy ground-drop projection intent is missing".to_string())?;
    let intent: GroundDropProjectionIntent = serde_json::from_str(encoded)
        .map_err(|error| format!("decode economy ground-drop projection intent: {error}"))?;
    if intent.version != 1 {
        return Err("unsupported economy ground-drop projection intent version".to_string());
    }
    if intent
        .claim_idempotency_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        return Err("economy ground-drop projection claim key is empty".to_string());
    }
    let command = SharedAccountInventoryCommandEnvelope {
        identity: intent.identity.clone(),
        command: match &intent.claim_idempotency_key {
            Some(claim_idempotency_key) => SharedAccountInventoryCommand::GroundDropClaimPickup {
                drop: intent.drop.clone(),
                claim_idempotency_key: claim_idempotency_key.clone(),
            },
            None => SharedAccountInventoryCommand::GroundDropPickup(intent.drop.clone()),
        },
    };
    let context = SharedAccountInventoryExecutionContext {
        zone_id: crate::ZoneId::new(envelope.zone_id.clone()),
        fencing_generation: envelope.fencing_generation,
        source_sequence: envelope.source_sequence,
        created_at_ms: envelope.created_at_ms,
        external_commit_authorized: true,
    };
    let expected = economy_transaction_for_command(&context, &command, None)
        .ok_or_else(|| "economy ground-drop projection cannot be empty".to_string())?;
    if &expected != envelope || expected.event_id()? != event_id {
        return Err("economy ground-drop projection intent does not bind the envelope".to_string());
    }
    Ok(Some(GroundDropProjectionRow {
        account_id: intent.identity.account_id.clone(),
        character_index: intent.identity.character_index,
        intent,
    }))
}

fn validate_stored_economy_transaction(
    event: &EconomyOutboxEvent,
    receipt: &EconomyTransactionReceipt,
) -> Result<(), String> {
    if validate_duplicate_business_effect(event, receipt, &event.envelope)?
        != DuplicateBusinessEffect::Exact
    {
        return Err("stored economy transaction is not an exact effect".to_string());
    }
    let _ = trade_projection_rows_from_envelope(&event.envelope, &event.event_id)?;
    let _ = ground_drop_projection_from_envelope(&event.envelope, &event.event_id)?;
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedEconomyEvent {
    pub event: EconomyOutboxEvent,
    pub attempt_count: u32,
    pub locked_until_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EconomyInboxReceipt {
    pub consumer_id: String,
    pub event_id: String,
    pub duplicate: bool,
    pub processed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomyReconciliationReport {
    pub run_id: String,
    pub started_at_ms: u64,
    pub completed_at_ms: u64,
    pub pending_count: i64,
    pub expired_delivery_count: i64,
    pub dead_letter_count: i64,
    pub transaction_without_outbox_count: i64,
    pub negative_balance_count: i64,
    pub healthy: bool,
}

#[derive(Debug, Clone)]
pub struct PostgresEconomyStore {
    database_url: String,
}

impl PostgresEconomyStore {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }

    pub fn ensure_migrated(&self) -> Result<(), String> {
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)
    }

    pub fn bootstrap_character(
        &self,
        identity: &ActiveSessionIdentity,
        snapshot: &WorldSnapshot,
        bootstrapped_at_ms: u64,
    ) -> Result<EconomyBootstrapReceipt, String> {
        let opening = EconomyOpeningSnapshot::from_runtime(identity, snapshot)?;
        let snapshot_digest = opening.digest()?;
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)?;
        let mut transaction = client
            .transaction()
            .map_err(|error| format!("economy bootstrap begin failed: {error}"))?;
        lock_economy_characters(
            &mut transaction,
            &BTreeSet::from([(identity.account_id.clone(), identity.character_index)]),
        )?;

        if let Some(row) = transaction
            .query_opt(
                "SELECT snapshot_digest,gold,experience,item_quantity,item_kind_count,
                        bootstrapped_at_ms
                 FROM game_economy_bootstraps
                 WHERE account_id=$1 AND character_index=$2",
                &[&identity.account_id, &identity.character_index],
            )
            .map_err(|error| format!("economy bootstrap lookup failed: {error}"))?
        {
            return Ok(EconomyBootstrapReceipt {
                account_id: identity.account_id.clone(),
                character_index: identity.character_index,
                snapshot_digest: row.get("snapshot_digest"),
                gold: row.get("gold"),
                experience: row.get("experience"),
                item_quantity: row.get("item_quantity"),
                item_kind_count: usize::try_from(row.get::<_, i32>("item_kind_count").max(0))
                    .unwrap_or_default(),
                bootstrapped_at_ms: row.get::<_, i64>("bootstrapped_at_ms").max(0) as u64,
                duplicate: true,
            });
        }

        let expected = opening.balance_amounts();
        let existing = transaction
            .query(
                "SELECT asset_kind,asset_key,amount
                 FROM game_economy_balances
                 WHERE account_id=$1 AND character_index=$2
                   AND asset_kind IN ('gold','experience','item_quantity')
                   AND amount <> 0",
                &[&identity.account_id, &identity.character_index],
            )
            .map_err(|error| format!("economy bootstrap balance lookup failed: {error}"))?
            .into_iter()
            .map(|row| {
                (
                    (
                        row.get::<_, String>("asset_kind"),
                        row.get::<_, String>("asset_key"),
                    ),
                    row.get::<_, i64>("amount"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if !existing.is_empty() && existing != expected {
            return Err(format!(
                "economy bootstrap conflict for {}/{}: ledger does not match live runtime",
                identity.account_id, identity.character_index
            ));
        }

        for ((asset_kind, asset_key), amount) in &expected {
            transaction
                .execute(
                    "INSERT INTO game_economy_balances
                     (account_id,character_index,asset_kind,asset_key,amount,balance_version)
                     VALUES ($1,$2,$3,$4,$5,0)
                     ON CONFLICT (account_id,character_index,asset_kind,asset_key)
                     DO UPDATE SET amount=EXCLUDED.amount,
                                   balance_version=game_economy_balances.balance_version+1,
                                   updated_at=now()",
                    &[
                        &identity.account_id,
                        &identity.character_index,
                        asset_kind,
                        asset_key,
                        amount,
                    ],
                )
                .map_err(|error| format!("economy bootstrap balance write failed: {error}"))?;
        }

        let details = serde_json::to_value(&opening)
            .map_err(|error| format!("encode economy bootstrap details: {error}"))?;
        transaction
            .execute(
                "INSERT INTO game_economy_bootstraps
                 (account_id,character_index,snapshot_digest,gold,experience,item_quantity,
                  item_kind_count,bootstrapped_at_ms,details)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
                &[
                    &identity.account_id,
                    &identity.character_index,
                    &snapshot_digest,
                    &opening.gold,
                    &opening.experience,
                    &opening.total_item_quantity,
                    &(i32::try_from(opening.item_quantities.len()).unwrap_or(i32::MAX)),
                    &(bootstrapped_at_ms as i64),
                    &details,
                ],
            )
            .map_err(|error| format!("economy bootstrap receipt write failed: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("economy bootstrap commit failed: {error}"))?;

        Ok(EconomyBootstrapReceipt {
            account_id: identity.account_id.clone(),
            character_index: identity.character_index,
            snapshot_digest,
            gold: opening.gold,
            experience: opening.experience,
            item_quantity: opening.total_item_quantity,
            item_kind_count: opening.item_quantities.len(),
            bootstrapped_at_ms,
            duplicate: false,
        })
    }

    /// Read a previously committed effect before runtime capacity preflight.
    /// The same integrity checks used by `transact` fail closed on damaged
    /// receipt/outbox rows, while the subsequent `transact` still serializes a
    /// race with a concurrent producer under the idempotency advisory lock.
    pub fn lookup(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<Option<EconomyTransactionReceipt>, String> {
        envelope.validate()?;
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)?;
        let mut transaction = client
            .transaction()
            .map_err(|error| format!("economy lookup transaction begin failed: {error}"))?;
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
                &[&envelope.idempotency_key],
            )
            .map_err(|error| format!("economy idempotency lookup lock failed: {error}"))?;
        let row = transaction
            .query_opt(
                "SELECT tx.receipt,outbox.payload
                 FROM game_economy_transactions tx
                 JOIN game_economy_outbox outbox ON outbox.event_id=tx.event_id
                 WHERE tx.idempotency_key=$1",
                &[&envelope.idempotency_key],
            )
            .map_err(|error| format!("economy idempotency lookup failed: {error}"))?;
        let result = if let Some(row) = row {
            let receipt: EconomyTransactionReceipt = serde_json::from_value(row.get("receipt"))
                .map_err(|error| format!("decode stored economy receipt: {error}"))?;
            let event: EconomyOutboxEvent = serde_json::from_value(row.get("payload"))
                .map_err(|error| format!("decode stored economy outbox event: {error}"))?;
            Some(duplicate_receipt_from_stored(&event, &receipt, envelope)?)
        } else {
            None
        };
        transaction
            .commit()
            .map_err(|error| format!("economy idempotency lookup commit failed: {error}"))?;
        Ok(result)
    }
    pub fn transact(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<EconomyTransactionReceipt, String> {
        envelope.validate()?;
        let event_id = envelope.event_id()?;
        let trade_projection_rows = trade_projection_rows_from_envelope(envelope, &event_id)?;
        let ground_drop_projection = ground_drop_projection_from_envelope(envelope, &event_id)?;
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)?;
        let mut transaction = client
            .transaction()
            .map_err(|error| format!("economy transaction begin failed: {error}"))?;
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
                &[&envelope.idempotency_key],
            )
            .map_err(|error| format!("economy idempotency lock failed: {error}"))?;
        if let Some(row) = transaction
            .query_opt(
                "SELECT tx.receipt,outbox.payload
                 FROM game_economy_transactions tx
                 JOIN game_economy_outbox outbox ON outbox.event_id=tx.event_id
                 WHERE tx.idempotency_key = $1",
                &[&envelope.idempotency_key],
            )
            .map_err(|error| format!("economy idempotency lookup failed: {error}"))?
        {
            let receipt: EconomyTransactionReceipt = serde_json::from_value(row.get("receipt"))
                .map_err(|error| format!("decode stored economy receipt: {error}"))?;
            let event: EconomyOutboxEvent = serde_json::from_value(row.get("payload"))
                .map_err(|error| format!("decode stored economy outbox event: {error}"))?;
            return duplicate_receipt_from_stored(&event, &receipt, envelope);
        }

        let characters = envelope
            .legs
            .iter()
            .map(|leg| (leg.balance.account_id.clone(), leg.balance.character_index))
            .collect::<BTreeSet<_>>();
        lock_economy_characters(&mut transaction, &characters)?;
        let aggregated = aggregate_legs(&envelope.legs)?;
        let unique_item_ids = aggregated
            .keys()
            .filter(|key| key.asset_kind == "item")
            .map(|key| key.asset_key.clone())
            .collect::<BTreeSet<_>>();
        for item_id in &unique_item_ids {
            let lock_key = format!("obelisk.mir2.unique-item:{item_id}");
            transaction
                .query_one(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
                    &[&lock_key],
                )
                .map_err(|error| format!("unique item ownership lock failed: {error}"))?;
        }
        let mut balances_after = BTreeMap::new();
        for (key, delta) in aggregated {
            ensure_balance_row(&mut transaction, &key)?;
            let row = transaction
                .query_one(
                    "SELECT amount FROM game_economy_balances
                     WHERE account_id=$1 AND character_index=$2 AND asset_kind=$3 AND asset_key=$4
                     FOR UPDATE",
                    &[
                        &key.account_id,
                        &key.character_index,
                        &key.asset_kind,
                        &key.asset_key,
                    ],
                )
                .map_err(|error| format!("economy balance lock failed: {error}"))?;
            let current: i64 = row.get("amount");
            let after = current
                .checked_add(delta)
                .ok_or_else(|| "economy balance overflow".to_string())?;
            if after < 0 {
                return Err(format!(
                    "insufficient {}:{} balance for {}/{}: current {}, delta {}",
                    key.asset_kind,
                    key.asset_key,
                    key.account_id,
                    key.character_index,
                    current,
                    delta
                ));
            }
            if key.asset_kind == "item" && after > 1 {
                return Err(format!(
                    "unique item {} cannot have balance {} for {}/{}",
                    key.asset_key, after, key.account_id, key.character_index
                ));
            }
            transaction
                .execute(
                    "UPDATE game_economy_balances
                     SET amount=$5, balance_version=balance_version+1, updated_at=now()
                     WHERE account_id=$1 AND character_index=$2 AND asset_kind=$3 AND asset_key=$4",
                    &[
                        &key.account_id,
                        &key.character_index,
                        &key.asset_kind,
                        &key.asset_key,
                        &after,
                    ],
                )
                .map_err(|error| format!("economy balance update failed: {error}"))?;
            balances_after.insert(balance_receipt_key(&key)?, after);
        }
        for item_id in unique_item_ids {
            let total: i64 = transaction
                .query_one(
                    "SELECT COALESCE(SUM(amount),0)::bigint AS total
                     FROM game_economy_balances
                     WHERE asset_kind='item' AND asset_key=$1",
                    &[&item_id],
                )
                .map_err(|error| format!("unique item ownership check failed: {error}"))?
                .get("total");
            if total > 1 {
                return Err(format!(
                    "unique item {item_id} cannot be owned by more than one character"
                ));
            }
        }
        let committed_at_ms = now_ms();
        let receipt = EconomyTransactionReceipt {
            idempotency_key: envelope.idempotency_key.clone(),
            event_id: event_id.clone(),
            transaction_kind: envelope.transaction_kind,
            committed_at_ms,
            balances_after,
            duplicate: false,
            settled_elsewhere: false,
        };
        let receipt_json = serde_json::to_value(&receipt)
            .map_err(|error| format!("encode economy receipt: {error}"))?;
        let event = EconomyOutboxEvent {
            event_id: event_id.clone(),
            idempotency_key: envelope.idempotency_key.clone(),
            envelope: envelope.clone(),
            receipt_digest: receipt_integrity_digest(&receipt)?,
        };
        let event_json = serde_json::to_value(&event)
            .map_err(|error| format!("encode economy outbox event: {error}"))?;
        transaction
            .execute(
                "INSERT INTO game_economy_transactions
                 (idempotency_key,event_id,transaction_kind,receipt,committed_at_ms)
                 VALUES ($1,$2,$3,$4,$5)",
                &[
                    &envelope.idempotency_key,
                    &event_id,
                    &envelope.transaction_kind.as_db(),
                    &receipt_json,
                    &(committed_at_ms as i64),
                ],
            )
            .map_err(|error| format!("economy receipt insert failed: {error}"))?;
        transaction
            .execute(
                "INSERT INTO game_economy_outbox
                 (event_id,idempotency_key,payload,status,attempt_count,next_attempt_at_ms,created_at_ms)
                 VALUES ($1,$2,$3,'pending',0,0,$4)",
                &[
                    &event_id,
                    &envelope.idempotency_key,
                    &event_json,
                    &(committed_at_ms as i64),
                ],
            )
            .map_err(|error| format!("economy outbox insert failed: {error}"))?;
        for projection in &trade_projection_rows {
            let own_offer = serde_json::to_value(&projection.own_offer)
                .map_err(|error| format!("encode own trade projection offer: {error}"))?;
            let incoming_offer = serde_json::to_value(&projection.incoming_offer)
                .map_err(|error| format!("encode incoming trade projection offer: {error}"))?;
            transaction
                .execute(
                    "INSERT INTO game_economy_trade_projections
                     (event_id,account_id,character_index,own_offer,incoming_offer,status)
                     VALUES ($1,$2,$3,$4,$5,'pending')",
                    &[
                        &event_id,
                        &projection.account_id,
                        &projection.character_index,
                        &own_offer,
                        &incoming_offer,
                    ],
                )
                .map_err(|error| format!("economy trade projection insert failed: {error}"))?;
        }
        if let Some(projection) = ground_drop_projection {
            let intent = serde_json::to_value(&projection.intent)
                .map_err(|error| format!("encode ground-drop projection intent: {error}"))?;
            transaction
                .execute(
                    "INSERT INTO game_economy_ground_drop_projections
                     (event_id,account_id,character_index,intent,status)
                     VALUES ($1,$2,$3,$4,'pending')",
                    &[
                        &event_id,
                        &projection.account_id,
                        &projection.character_index,
                        &intent,
                    ],
                )
                .map_err(|error| {
                    format!("economy ground-drop projection insert failed: {error}")
                })?;
        }
        transaction
            .commit()
            .map_err(|error| format!("economy transaction commit failed: {error}"))?;
        Ok(receipt)
    }

    fn pending_trade_projections(
        &self,
        identity: &ActiveSessionIdentity,
    ) -> Result<Vec<DurableTradeProjection>, String> {
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)?;
        let rows = client
            .query(
                "SELECT projection.event_id,projection.own_offer,projection.incoming_offer,
                        transaction.receipt,outbox.payload
                 FROM game_economy_trade_projections projection
                 JOIN game_economy_transactions transaction
                   ON transaction.event_id=projection.event_id
                 JOIN game_economy_outbox outbox
                   ON outbox.event_id=projection.event_id
                 WHERE projection.account_id=$1
                   AND projection.character_index=$2
                   AND projection.status='pending'
                 ORDER BY projection.event_id",
                &[&identity.account_id, &identity.character_index],
            )
            .map_err(|error| format!("economy pending trade projection query failed: {error}"))?;
        rows.into_iter()
            .map(|row| {
                let event_id: String = row.get("event_id");
                let own_offer: SharedTradeOffer = serde_json::from_value(row.get("own_offer"))
                    .map_err(|error| format!("decode own trade projection offer: {error}"))?;
                let incoming_offer: SharedTradeOffer =
                    serde_json::from_value(row.get("incoming_offer")).map_err(|error| {
                        format!("decode incoming trade projection offer: {error}")
                    })?;
                let receipt: EconomyTransactionReceipt = serde_json::from_value(row.get("receipt"))
                    .map_err(|error| format!("decode trade projection receipt: {error}"))?;
                let event: EconomyOutboxEvent = serde_json::from_value(row.get("payload"))
                    .map_err(|error| format!("decode trade projection outbox: {error}"))?;
                validate_stored_economy_transaction(&event, &receipt)?;
                let expected = trade_projection_rows_from_envelope(&event.envelope, &event_id)?;
                if event.event_id != event_id
                    || !expected.iter().any(|projection| {
                        projection.account_id == identity.account_id
                            && projection.character_index == identity.character_index
                            && projection.own_offer == own_offer
                            && projection.incoming_offer == incoming_offer
                    })
                {
                    return Err("economy pending trade projection integrity conflict".to_string());
                }
                Ok(DurableTradeProjection {
                    event_id,
                    own_offer,
                    incoming_offer,
                })
            })
            .collect()
    }

    pub fn mark_trade_projection_projected(
        &self,
        identity: &ActiveSessionIdentity,
        event_id: &str,
    ) -> Result<(), String> {
        if event_id.len() != 64
            || !event_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("invalid economy trade projection event id".to_string());
        }
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)?;
        let updated = client
            .execute(
                "UPDATE game_economy_trade_projections
                 SET status='projected',projected_at_ms=$4,updated_at=now()
                 WHERE event_id=$1 AND account_id=$2 AND character_index=$3 AND status='pending'",
                &[
                    &event_id,
                    &identity.account_id,
                    &identity.character_index,
                    &(now_ms() as i64),
                ],
            )
            .map_err(|error| format!("economy trade projection mark failed: {error}"))?;
        if updated == 1 {
            return Ok(());
        }
        let status = client
            .query_opt(
                "SELECT status FROM game_economy_trade_projections
                 WHERE event_id=$1 AND account_id=$2 AND character_index=$3",
                &[&event_id, &identity.account_id, &identity.character_index],
            )
            .map_err(|error| format!("economy trade projection status read failed: {error}"))?
            .map(|row| row.get::<_, String>("status"));
        if status.as_deref() == Some("projected") {
            Ok(())
        } else {
            Err("economy trade projection row is missing or invalid".to_string())
        }
    }
    fn pending_ground_drop_projections(
        &self,
        identity: &ActiveSessionIdentity,
    ) -> Result<Vec<DurableGroundDropProjection>, String> {
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)?;
        let rows = client
            .query(
                "SELECT projection.event_id,projection.intent,transaction.receipt,outbox.payload
                 FROM game_economy_ground_drop_projections projection
                 JOIN game_economy_transactions transaction
                   ON transaction.event_id=projection.event_id
                 JOIN game_economy_outbox outbox
                   ON outbox.event_id=projection.event_id
                 WHERE projection.account_id=$1
                   AND projection.character_index=$2
                   AND projection.status='pending'
                 ORDER BY projection.event_id",
                &[&identity.account_id, &identity.character_index],
            )
            .map_err(|error| {
                format!("economy pending ground-drop projection query failed: {error}")
            })?;
        rows.into_iter()
            .map(|row| {
                let event_id: String = row.get("event_id");
                let intent: GroundDropProjectionIntent = serde_json::from_value(row.get("intent"))
                    .map_err(|error| format!("decode ground-drop projection intent: {error}"))?;
                let receipt: EconomyTransactionReceipt = serde_json::from_value(row.get("receipt"))
                    .map_err(|error| format!("decode ground-drop projection receipt: {error}"))?;
                let event: EconomyOutboxEvent = serde_json::from_value(row.get("payload"))
                    .map_err(|error| format!("decode ground-drop projection outbox: {error}"))?;
                validate_stored_economy_transaction(&event, &receipt)?;
                let expected = ground_drop_projection_from_envelope(&event.envelope, &event_id)?
                    .ok_or_else(|| "ground-drop projection event is not a pickup".to_string())?;
                if event.event_id != event_id
                    || expected.account_id != identity.account_id
                    || expected.character_index != identity.character_index
                    || expected.intent != intent
                {
                    return Err(
                        "economy pending ground-drop projection integrity conflict".to_string()
                    );
                }
                Ok(DurableGroundDropProjection { event_id, intent })
            })
            .collect()
    }

    pub fn mark_ground_drop_projection_projected(
        &self,
        identity: &ActiveSessionIdentity,
        event_id: &str,
    ) -> Result<(), String> {
        if event_id.len() != 64
            || !event_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("invalid economy ground-drop projection event id".to_string());
        }
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)?;
        let updated = client
            .execute(
                "UPDATE game_economy_ground_drop_projections
                 SET status='projected',projected_at_ms=$4,updated_at=now()
                 WHERE event_id=$1 AND account_id=$2 AND character_index=$3 AND status='pending'",
                &[
                    &event_id,
                    &identity.account_id,
                    &identity.character_index,
                    &(now_ms() as i64),
                ],
            )
            .map_err(|error| format!("economy ground-drop projection mark failed: {error}"))?;
        if updated == 1 {
            return Ok(());
        }
        let status = client
            .query_opt(
                "SELECT status FROM game_economy_ground_drop_projections
                 WHERE event_id=$1 AND account_id=$2 AND character_index=$3",
                &[&event_id, &identity.account_id, &identity.character_index],
            )
            .map_err(|error| format!("economy ground-drop projection status read failed: {error}"))?
            .map(|row| row.get::<_, String>("status"));
        if status.as_deref() == Some("projected") {
            Ok(())
        } else {
            Err("economy ground-drop projection row is missing or invalid".to_string())
        }
    }

    pub fn balance(&self, key: &EconomyBalanceKey) -> Result<i64, String> {
        let mut client = self.connect()?;
        let row = client
            .query_opt(
                "SELECT amount FROM game_economy_balances
                 WHERE account_id=$1 AND character_index=$2 AND asset_kind=$3 AND asset_key=$4",
                &[
                    &key.account_id,
                    &key.character_index,
                    &key.asset_kind,
                    &key.asset_key,
                ],
            )
            .map_err(|error| format!("economy balance read failed: {error}"))?;
        Ok(row.map(|row| row.get("amount")).unwrap_or_default())
    }

    pub fn claim_outbox(
        &self,
        worker_id: &str,
        now_ms: u64,
        limit: usize,
        lease_ms: u64,
    ) -> Result<Vec<ClaimedEconomyEvent>, String> {
        validate_worker_or_consumer("worker", worker_id)?;
        let mut client = self.connect()?;
        let rows = client
            .query(
                "WITH candidates AS (
                    SELECT event_id
                    FROM game_economy_outbox
                    WHERE (
                        (status='pending' AND next_attempt_at_ms <= $1)
                        OR (status='delivering' AND locked_until_ms < $1)
                    )
                    ORDER BY created_at_ms,event_id
                    FOR UPDATE SKIP LOCKED
                    LIMIT $2
                 )
                 UPDATE game_economy_outbox AS outbox
                 SET status='delivering',
                     attempt_count=outbox.attempt_count+1,
                     locked_by=$3,
                     locked_until_ms=$4,
                     updated_at=now()
                 FROM candidates
                 WHERE outbox.event_id=candidates.event_id
                 RETURNING outbox.payload,outbox.attempt_count,outbox.locked_until_ms",
                &[
                    &(now_ms as i64),
                    &(i64::try_from(limit.max(1).min(1_000)).unwrap_or(1_000)),
                    &worker_id,
                    &(now_ms.saturating_add(lease_ms.max(1)) as i64),
                ],
            )
            .map_err(|error| format!("economy outbox claim failed: {error}"))?;
        rows.into_iter()
            .map(|row| {
                let event = serde_json::from_value(row.get("payload"))
                    .map_err(|error| format!("decode claimed economy event: {error}"))?;
                let attempt_count: i32 = row.get("attempt_count");
                let locked_until_ms: i64 = row.get("locked_until_ms");
                Ok(ClaimedEconomyEvent {
                    event,
                    attempt_count: attempt_count.max(0) as u32,
                    locked_until_ms: locked_until_ms.max(0) as u64,
                })
            })
            .collect()
    }

    pub fn mark_dispatched(
        &self,
        worker_id: &str,
        event_id: &str,
        dispatched_at_ms: u64,
    ) -> Result<(), String> {
        let mut client = self.connect()?;
        let updated = client
            .execute(
                "UPDATE game_economy_outbox
                 SET status='dispatched',dispatched_at_ms=$3,locked_by=NULL,locked_until_ms=NULL,
                     last_error=NULL,updated_at=now()
                 WHERE event_id=$1 AND status='delivering' AND locked_by=$2",
                &[&event_id, &worker_id, &(dispatched_at_ms as i64)],
            )
            .map_err(|error| format!("economy outbox acknowledge failed: {error}"))?;
        if updated != 1 {
            return Err("economy outbox acknowledge lost its delivery lease".to_string());
        }
        Ok(())
    }

    pub fn mark_delivery_failed(
        &self,
        worker_id: &str,
        event_id: &str,
        error: &str,
        now_ms: u64,
        max_attempts: u32,
    ) -> Result<(), String> {
        let mut client = self.connect()?;
        let updated = client
            .execute(
                "UPDATE game_economy_outbox
                 SET status=CASE WHEN attempt_count >= $4 THEN 'dead_letter' ELSE 'pending' END,
                     next_attempt_at_ms=$3 + LEAST(60000, 250 * (1::bigint << LEAST(attempt_count,8))),
                     locked_by=NULL,locked_until_ms=NULL,last_error=$5,updated_at=now()
                 WHERE event_id=$1 AND status='delivering' AND locked_by=$2",
                &[
                    &event_id,
                    &worker_id,
                    &(now_ms as i64),
                    &(i32::try_from(max_attempts.max(1)).unwrap_or(i32::MAX)),
                    &truncate_error(error),
                ],
            )
            .map_err(|db_error| format!("economy outbox failure update failed: {db_error}"))?;
        if updated != 1 {
            return Err("economy outbox failure update lost its delivery lease".to_string());
        }
        Ok(())
    }

    pub fn redrive_dead_letter(&self, event_id: &str, now_ms: u64) -> Result<(), String> {
        let mut client = self.connect()?;
        let updated = client
            .execute(
                "UPDATE game_economy_outbox
                 SET status='pending',attempt_count=0,next_attempt_at_ms=$2,
                     locked_by=NULL,locked_until_ms=NULL,last_error=NULL,updated_at=now()
                 WHERE event_id=$1 AND status='dead_letter'",
                &[&event_id, &(now_ms as i64)],
            )
            .map_err(|error| format!("economy dead-letter redrive failed: {error}"))?;
        if updated != 1 {
            return Err("economy event is not in dead-letter state".to_string());
        }
        Ok(())
    }

    pub fn ingest(
        &self,
        consumer_id: &str,
        event: &EconomyOutboxEvent,
        processed_at_ms: u64,
    ) -> Result<EconomyInboxReceipt, String> {
        validate_worker_or_consumer("consumer", consumer_id)?;
        if event.event_id != event.envelope.event_id()? {
            return Err("economy inbox event digest mismatch".to_string());
        }
        let payload = serde_json::to_value(event)
            .map_err(|error| format!("encode economy inbox event: {error}"))?;
        let mut client = self.connect()?;
        let inserted = client
            .execute(
                "INSERT INTO game_economy_inbox
                 (consumer_id,event_id,payload_digest,processed_at_ms,payload)
                 VALUES ($1,$2,$2,$3,$4)
                 ON CONFLICT (consumer_id,event_id) DO NOTHING",
                &[
                    &consumer_id,
                    &event.event_id,
                    &(processed_at_ms as i64),
                    &payload,
                ],
            )
            .map_err(|error| format!("economy inbox insert failed: {error}"))?;
        Ok(EconomyInboxReceipt {
            consumer_id: consumer_id.to_string(),
            event_id: event.event_id.clone(),
            duplicate: inserted == 0,
            processed_at_ms,
        })
    }

    pub fn dispatch_once(
        &self,
        worker_id: &str,
        consumer_id: &str,
        now_ms: u64,
        limit: usize,
    ) -> Result<usize, String> {
        let events = self.claim_outbox(worker_id, now_ms, limit, 5_000)?;
        let mut dispatched = 0;
        for claimed in events {
            match self.ingest(consumer_id, &claimed.event, now_ms) {
                Ok(_) => {
                    self.mark_dispatched(worker_id, &claimed.event.event_id, now_ms)?;
                    dispatched += 1;
                }
                Err(error) => {
                    self.mark_delivery_failed(
                        worker_id,
                        &claimed.event.event_id,
                        &error,
                        now_ms,
                        8,
                    )?;
                }
            }
        }
        Ok(dispatched)
    }

    pub fn reconcile(&self, observed_at_ms: u64) -> Result<EconomyReconciliationReport, String> {
        let mut client = self.connect()?;
        let run_id = format!("economy-reconcile-{observed_at_ms}-{}", std::process::id());
        let started_at_ms = observed_at_ms;
        let row = client
            .query_one(
                "SELECT
                    COUNT(*) FILTER (WHERE status='pending')::bigint AS pending_count,
                    COUNT(*) FILTER (WHERE status='delivering' AND locked_until_ms < $1)::bigint AS expired_delivery_count,
                    COUNT(*) FILTER (WHERE status='dead_letter')::bigint AS dead_letter_count
                 FROM game_economy_outbox",
                &[&(observed_at_ms as i64)],
            )
            .map_err(|error| format!("economy outbox reconciliation query failed: {error}"))?;
        let pending_count: i64 = row.get("pending_count");
        let expired_delivery_count: i64 = row.get("expired_delivery_count");
        let dead_letter_count: i64 = row.get("dead_letter_count");
        let transaction_without_outbox_count: i64 = client
            .query_one(
                "SELECT COUNT(*)::bigint AS count
                 FROM game_economy_transactions tx
                 LEFT JOIN game_economy_outbox outbox ON outbox.event_id=tx.event_id
                 WHERE outbox.event_id IS NULL",
                &[],
            )
            .map_err(|error| format!("economy transaction reconciliation query failed: {error}"))?
            .get("count");
        let negative_balance_count: i64 = client
            .query_one(
                "SELECT COUNT(*)::bigint AS count FROM game_economy_balances WHERE amount < 0",
                &[],
            )
            .map_err(|error| format!("economy balance reconciliation query failed: {error}"))?
            .get("count");
        let completed_at_ms = now_ms();
        let healthy = expired_delivery_count == 0
            && dead_letter_count == 0
            && transaction_without_outbox_count == 0
            && negative_balance_count == 0;
        let report = EconomyReconciliationReport {
            run_id: run_id.clone(),
            started_at_ms,
            completed_at_ms,
            pending_count,
            expired_delivery_count,
            dead_letter_count,
            transaction_without_outbox_count,
            negative_balance_count,
            healthy,
        };
        let details = serde_json::to_value(&report)
            .map_err(|error| format!("encode economy reconciliation report: {error}"))?;
        client
            .execute(
                "INSERT INTO game_economy_reconciliation_runs
                 (run_id,started_at_ms,completed_at_ms,pending_count,expired_delivery_count,
                  dead_letter_count,transaction_without_outbox_count,negative_balance_count,
                  healthy,details)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                &[
                    &run_id,
                    &(started_at_ms as i64),
                    &(completed_at_ms as i64),
                    &pending_count,
                    &expired_delivery_count,
                    &dead_letter_count,
                    &transaction_without_outbox_count,
                    &negative_balance_count,
                    &healthy,
                    &details,
                ],
            )
            .map_err(|error| format!("economy reconciliation record failed: {error}"))?;
        Ok(report)
    }

    pub fn outbox_status(&self, event_id: &str) -> Result<Option<String>, String> {
        let mut client = self.connect()?;
        client
            .query_opt(
                "SELECT status FROM game_economy_outbox WHERE event_id=$1",
                &[&event_id],
            )
            .map(|row| row.map(|row| row.get("status")))
            .map_err(|error| format!("economy outbox status read failed: {error}"))
    }

    pub fn inbox_count(&self, consumer_id: &str, event_id: &str) -> Result<i64, String> {
        let mut client = self.connect()?;
        client
            .query_one(
                "SELECT COUNT(*)::bigint AS count FROM game_economy_inbox
                 WHERE consumer_id=$1 AND event_id=$2",
                &[&consumer_id, &event_id],
            )
            .map(|row| row.get("count"))
            .map_err(|error| format!("economy inbox count failed: {error}"))
    }

    fn connect(&self) -> Result<Client, String> {
        if self.database_url.trim().is_empty() {
            return Err("economy PostgreSQL URL is required".to_string());
        }
        Client::connect(&self.database_url, NoTls)
            .map_err(|error| format!("economy PostgreSQL connect failed: {error}"))
    }
}

trait EconomySettlementStore: std::fmt::Debug + Send + Sync {
    fn ensure_migrated(&self) -> Result<(), String>;

    fn bootstrap_character(
        &self,
        identity: &ActiveSessionIdentity,
        snapshot: &WorldSnapshot,
        bootstrapped_at_ms: u64,
    ) -> Result<EconomyBootstrapReceipt, String>;

    fn lookup(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<Option<EconomyTransactionReceipt>, String>;

    fn transact(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<EconomyTransactionReceipt, String>;

    fn pending_trade_projections(
        &self,
        identity: &ActiveSessionIdentity,
    ) -> Result<Vec<DurableTradeProjection>, String>;

    fn mark_trade_projection_projected(
        &self,
        identity: &ActiveSessionIdentity,
        event_id: &str,
    ) -> Result<(), String>;

    fn pending_ground_drop_projections(
        &self,
        identity: &ActiveSessionIdentity,
    ) -> Result<Vec<DurableGroundDropProjection>, String>;

    fn mark_ground_drop_projection_projected(
        &self,
        identity: &ActiveSessionIdentity,
        event_id: &str,
    ) -> Result<(), String>;
}

impl EconomySettlementStore for PostgresEconomyStore {
    fn ensure_migrated(&self) -> Result<(), String> {
        PostgresEconomyStore::ensure_migrated(self)
    }

    fn bootstrap_character(
        &self,
        identity: &ActiveSessionIdentity,
        snapshot: &WorldSnapshot,
        bootstrapped_at_ms: u64,
    ) -> Result<EconomyBootstrapReceipt, String> {
        PostgresEconomyStore::bootstrap_character(self, identity, snapshot, bootstrapped_at_ms)
    }

    fn lookup(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<Option<EconomyTransactionReceipt>, String> {
        PostgresEconomyStore::lookup(self, envelope)
    }

    fn transact(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<EconomyTransactionReceipt, String> {
        PostgresEconomyStore::transact(self, envelope)
    }

    fn pending_trade_projections(
        &self,
        identity: &ActiveSessionIdentity,
    ) -> Result<Vec<DurableTradeProjection>, String> {
        PostgresEconomyStore::pending_trade_projections(self, identity)
    }

    fn mark_trade_projection_projected(
        &self,
        identity: &ActiveSessionIdentity,
        event_id: &str,
    ) -> Result<(), String> {
        PostgresEconomyStore::mark_trade_projection_projected(self, identity, event_id)
    }

    fn pending_ground_drop_projections(
        &self,
        identity: &ActiveSessionIdentity,
    ) -> Result<Vec<DurableGroundDropProjection>, String> {
        PostgresEconomyStore::pending_ground_drop_projections(self, identity)
    }

    fn mark_ground_drop_projection_projected(
        &self,
        identity: &ActiveSessionIdentity,
        event_id: &str,
    ) -> Result<(), String> {
        PostgresEconomyStore::mark_ground_drop_projection_projected(self, identity, event_id)
    }
}
/// Gate 18 bridge from real Mir2 Zone rewards/pickups to the Gate 17 ledger.
///
/// The active owner commits PostgreSQL before mutating its private character
/// projection. A verified standby replay never writes PostgreSQL; it only
/// rebuilds the same private projection. On active-owner restart, the durable
/// receipt is the immutable recovery witness: a projection exactly one delta
/// behind is replayed once, an equal projection is already materialized, and
/// every other state fails closed.
#[derive(Debug)]
pub struct PostgresEconomyAccountInventoryService {
    store: Arc<dyn EconomySettlementStore>,
    projected_receipts: Mutex<BTreeMap<String, SharedAccountInventoryTransactionReceipt>>,
}

impl PostgresEconomyAccountInventoryService {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self::with_store(PostgresEconomyStore::new(database_url))
    }

    pub fn with_store(store: PostgresEconomyStore) -> Self {
        Self::with_backend(Arc::new(store))
    }

    fn with_backend(store: Arc<dyn EconomySettlementStore>) -> Self {
        Self {
            store,
            projected_receipts: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn ensure_migrated(&self) -> Result<(), String> {
        self.store.ensure_migrated()
    }

    fn apply_projection(
        runtime: &mut InProcessWorldRuntime,
        envelope: &SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        match &envelope.command {
            SharedAccountInventoryCommand::GoldDrop { amount, .. } => {
                runtime.commit_shared_gold_drop_transaction(*amount)
            }
            SharedAccountInventoryCommand::InventoryItemDrop { drop, .. } => {
                runtime.commit_shared_inventory_item_drop_transaction(drop)
            }
            SharedAccountInventoryCommand::GroundDropPickup(drop)
            | SharedAccountInventoryCommand::GroundDropClaimPickup { drop, .. } => {
                runtime.commit_shared_ground_drop_pickup_transaction(drop)
            }
            SharedAccountInventoryCommand::MonsterKillAward(award) => runtime
                .commit_shared_monster_kill_award_transaction(
                    award.monster_object_id,
                    &award.monster_name,
                    award.experience,
                ),
            SharedAccountInventoryCommand::SkillItemConsume { spell, .. } => {
                runtime.commit_shared_skill_item_consumption_transaction(*spell)
            }
        }
    }

    fn apply_ground_projection_atomically(
        runtime: &mut InProcessWorldRuntime,
        event_id: &str,
        envelope: &SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        let drop = match &envelope.command {
            SharedAccountInventoryCommand::GroundDropPickup(drop)
            | SharedAccountInventoryCommand::GroundDropClaimPickup { drop, .. } => drop,
            _ => return Self::failed_receipt(&envelope.command),
        };
        match runtime.apply_shared_ground_drop_projection(event_id, drop) {
            Ok(packets) => SharedAccountInventoryTransactionReceipt {
                kind: SharedAccountInventoryTransactionKind::GroundDropPickup,
                committed: true,
                packets,
            },
            Err(_) => Self::failed_receipt(&envelope.command),
        }
    }

    fn failed_receipt(
        command: &SharedAccountInventoryCommand,
    ) -> SharedAccountInventoryTransactionReceipt {
        SharedAccountInventoryTransactionReceipt {
            kind: command_kind(command),
            committed: false,
            packets: Vec::new(),
        }
    }

    fn already_materialized_receipt(
        command: &SharedAccountInventoryCommand,
    ) -> SharedAccountInventoryTransactionReceipt {
        SharedAccountInventoryTransactionReceipt {
            kind: command_kind(command),
            committed: true,
            packets: Vec::new(),
        }
    }

    fn persist_projection_marker(
        runtime: &mut InProcessWorldRuntime,
        event_id: &str,
        command: &SharedAccountInventoryCommand,
        receipt: SharedAccountInventoryTransactionReceipt,
    ) -> SharedAccountInventoryTransactionReceipt {
        if !receipt.committed {
            return receipt;
        }
        if runtime
            .persist_shared_economy_projection_event(event_id)
            .is_err()
        {
            return Self::failed_receipt(command);
        }
        receipt
    }
    fn materialize_external_receipt(
        &self,
        runtime: &mut InProcessWorldRuntime,
        envelope: &SharedAccountInventoryCommandEnvelope,
        transaction_receipt: &EconomyTransactionReceipt,
    ) -> SharedAccountInventoryTransactionReceipt {
        // Another character already owns this exact world drop.  It is
        // terminally committed at the Zone layer but must never be projected
        // into this character.
        if transaction_receipt.settled_elsewhere {
            return Self::already_materialized_receipt(&envelope.command);
        }
        let receipt = if Self::is_ground_drop_pickup(&envelope.command) {
            Self::apply_ground_projection_atomically(
                runtime,
                &transaction_receipt.event_id,
                envelope,
            )
        } else {
            let projection =
                if runtime.has_shared_economy_projection_event(&transaction_receipt.event_id) {
                    Self::already_materialized_receipt(&envelope.command)
                } else {
                    Self::apply_projection(runtime, envelope)
                };
            Self::persist_projection_marker(
                runtime,
                &transaction_receipt.event_id,
                &envelope.command,
                projection,
            )
        };
        if receipt.committed && Self::is_ground_drop_pickup(&envelope.command) {
            let _ = self.store.mark_ground_drop_projection_projected(
                &envelope.identity,
                &transaction_receipt.event_id,
            );
        }
        receipt
    }

    fn is_ground_drop_pickup(command: &SharedAccountInventoryCommand) -> bool {
        matches!(
            command,
            SharedAccountInventoryCommand::GroundDropPickup(_)
                | SharedAccountInventoryCommand::GroundDropClaimPickup { .. }
        )
    }

    fn projection_envelope(
        intent: &GroundDropProjectionIntent,
    ) -> SharedAccountInventoryCommandEnvelope {
        SharedAccountInventoryCommandEnvelope {
            identity: intent.identity.clone(),
            command: match &intent.claim_idempotency_key {
                Some(claim_idempotency_key) => {
                    SharedAccountInventoryCommand::GroundDropClaimPickup {
                        drop: intent.drop.clone(),
                        claim_idempotency_key: claim_idempotency_key.clone(),
                    }
                }
                None => SharedAccountInventoryCommand::GroundDropPickup(intent.drop.clone()),
            },
        }
    }

    fn materialize_pending_ground_drop_projection(
        &self,
        runtime: &mut InProcessWorldRuntime,
        projection: &DurableGroundDropProjection,
    ) -> SharedAccountInventoryTransactionReceipt {
        let envelope = Self::projection_envelope(&projection.intent);
        let receipt =
            Self::apply_ground_projection_atomically(runtime, &projection.event_id, &envelope);
        if receipt.committed {
            // A status-write failure is deliberately retained as pending. A
            // retry observes the durable character marker and only re-attempts
            // this idempotent status update.
            let _ = self.store.mark_ground_drop_projection_projected(
                &projection.intent.identity,
                &projection.event_id,
            );
        }
        receipt
    }

    /// Replay committed ground-drop rewards that were not yet persisted in the
    /// private character checkpoint. The coordinator should call this after a
    /// character becomes active and before allowing a stale Zone snapshot to
    /// recreate or offer the old drop.
    pub fn reconcile_ground_drop_projections_fenced(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> Vec<ServerPacket> {
        let Some(context) = context else {
            return Vec::new();
        };
        if !context.external_commit_authorized {
            return Vec::new();
        }
        let Some(identity) = runtime.active_identity() else {
            return Vec::new();
        };
        let pending = match self.store.pending_ground_drop_projections(&identity) {
            Ok(pending) => pending,
            Err(_) => return Vec::new(),
        };
        let mut packets = Vec::new();
        for projection in pending {
            let mut receipt = self.materialize_pending_ground_drop_projection(runtime, &projection);
            if receipt.committed {
                packets.append(&mut receipt.packets);
            }
        }
        packets
    }

    /// Fail closed while a durable ground-drop projection cannot be read. This
    /// lets the coordinator block only stale-drop restoration/reclaim paths.
    pub fn has_pending_ground_drop_projection_fenced(
        &self,
        runtime: &InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> bool {
        let Some(context) = context else {
            return true;
        };
        if !context.external_commit_authorized {
            return true;
        }
        let Some(identity) = runtime.active_identity() else {
            return false;
        };
        match self.store.pending_ground_drop_projections(&identity) {
            Ok(pending) => !pending.is_empty(),
            Err(_) => true,
        }
    }
}

impl SharedAccountInventoryService for PostgresEconomyAccountInventoryService {
    fn commit(
        &self,
        _runtime: &mut InProcessWorldRuntime,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        // A PostgreSQL producer without an ordered, finalized Zone context is
        // unsafe. Local development keeps using InProcessAccountInventoryService.
        Self::failed_receipt(&envelope.command)
    }

    fn commit_fenced(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        let Some(context) = context else {
            return SharedAccountInventoryCommitOutcome::Deferred {
                receipt: Self::failed_receipt(&envelope.command),
            };
        };
        let mut outcome_unknown = None;
        let receipt = (|| -> SharedAccountInventoryTransactionReceipt {
            if runtime.active_identity().as_ref() != Some(&envelope.identity) {
                return Self::failed_receipt(&envelope.command);
            }
            let stable_key = envelope.stable_idempotency_key();
            let ground_outcome_key = Self::is_ground_drop_pickup(&envelope.command)
                .then(|| economy_transaction_for_command(context, &envelope, None))
                .flatten()
                .map(|transaction| transaction.idempotency_key);
            if !self.bootstrap_fenced(runtime, Some(context)) {
                outcome_unknown = ground_outcome_key;
                return Self::failed_receipt(&envelope.command);
            }
            if self
                .projected_receipts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&stable_key)
                .is_some()
            {
                return Self::already_materialized_receipt(&envelope.command);
            }

            if !context.external_commit_authorized {
                let receipt = Self::apply_projection(runtime, &envelope);
                if receipt.committed {
                    self.projected_receipts
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .insert(stable_key, receipt.clone());
                }
                return receipt;
            }

            let experience_balance_delta = match &envelope.command {
                SharedAccountInventoryCommand::MonsterKillAward(award) => {
                    Some(runtime.shared_monster_kill_experience_balance_delta(award.experience))
                }
                _ => None,
            };
            let Some(transaction) =
                economy_transaction_for_command(context, &envelope, experience_balance_delta)
            else {
                // Commands without an external asset delta remain deterministic
                // Zone-only effects. Skill-item consumption is deliberately
                // rejected until its exact inventory component IDs are included.
                return match &envelope.command {
                    SharedAccountInventoryCommand::MonsterKillAward(award)
                        if award.experience == 0 =>
                    {
                        Self::apply_projection(runtime, &envelope)
                    }
                    _ => Self::failed_receipt(&envelope.command),
                };
            };

            // An integrity-validated durable receipt is authoritative even when a
            // later stale checkpoint has a full inventory or capped gold. Lookup
            // deliberately precedes runtime preflight; `transact` repeats the
            // check under its advisory lock to close the producer race.
            let transaction_receipt = match self.store.lookup(&transaction) {
                Ok(Some(receipt)) => receipt,
                Ok(None) => {
                    if !preflight_projection(runtime, &envelope.command) {
                        return Self::failed_receipt(&envelope.command);
                    }
                    match self.store.transact(&transaction) {
                        Ok(receipt) => receipt,
                        // A PostgreSQL commit acknowledgement can be lost after the
                        // transaction is already durable. Re-read the same stable
                        // envelope before allowing a Zone pickup to be restored.
                        Err(_) => match self.store.lookup(&transaction) {
                            Ok(Some(receipt)) => receipt,
                            Ok(None) => return Self::failed_receipt(&envelope.command),
                            Err(_) => {
                                outcome_unknown = Some(transaction.idempotency_key.clone());
                                return Self::failed_receipt(&envelope.command);
                            }
                        },
                    }
                }
                Err(_) => {
                    if Self::is_ground_drop_pickup(&envelope.command) {
                        outcome_unknown = Some(transaction.idempotency_key.clone());
                    }
                    return Self::failed_receipt(&envelope.command);
                }
            };
            let receipt =
                self.materialize_external_receipt(runtime, &envelope, &transaction_receipt);
            if receipt.committed {
                self.projected_receipts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(stable_key, receipt.clone());
                return receipt;
            }
            if Self::is_ground_drop_pickup(&envelope.command) {
                // The ledger/outbox/pending-projection row are already atomically
                // committed. Do not let a full bag or failed character save restore
                // a Zone drop that another player could steal; reconciliation owns
                // the later private materialization.
                return Self::already_materialized_receipt(&envelope.command);
            }
            receipt
        })();
        match outcome_unknown {
            Some(idempotency_key) => SharedAccountInventoryCommitOutcome::OutcomeUnknown {
                idempotency_key,
                execution_context: context.clone(),
                receipt,
            },
            None => SharedAccountInventoryCommitOutcome::Confirmed(receipt),
        }
    }

    fn retry_commit_fenced(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
        expected_idempotency_key: &str,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        let Some(context) = context else {
            return SharedAccountInventoryCommitOutcome::Deferred {
                receipt: Self::failed_receipt(&envelope.command),
            };
        };
        let generated_key = economy_transaction_for_command(context, &envelope, None)
            .map(|transaction| transaction.idempotency_key);
        if generated_key.as_deref() != Some(expected_idempotency_key) {
            return SharedAccountInventoryCommitOutcome::Deferred {
                receipt: Self::failed_receipt(&envelope.command),
            };
        }
        self.commit_fenced(runtime, Some(context), envelope)
    }

    fn bootstrap_fenced(
        &self,
        runtime: &InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> bool {
        let Some(context) = context else {
            return false;
        };
        if !context.external_commit_authorized {
            return true;
        }
        let Some(identity) = runtime.active_identity() else {
            return false;
        };
        self.store
            .bootstrap_character(&identity, &runtime.world_snapshot(), context.created_at_ms)
            .is_ok()
    }

    fn reconcile_ground_drop_projections_fenced(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> Vec<ServerPacket> {
        PostgresEconomyAccountInventoryService::reconcile_ground_drop_projections_fenced(
            self, runtime, context,
        )
    }

    fn has_pending_ground_drop_projection_fenced(
        &self,
        runtime: &InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> bool {
        PostgresEconomyAccountInventoryService::has_pending_ground_drop_projection_fenced(
            self, runtime, context,
        )
    }

    fn settle_trade_fenced(
        &self,
        context: Option<&SharedAccountInventoryExecutionContext>,
        first: &SharedTradeOffer,
        second: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        let Some(context) = context else {
            return SharedTradeSettlementOutcome::Deferred;
        };
        if !context.external_commit_authorized {
            return SharedTradeSettlementOutcome::Committed;
        }
        let transaction = match economy_transaction_for_trade(context, first, second) {
            Ok(Some(transaction)) => transaction,
            Ok(None) => return SharedTradeSettlementOutcome::Committed,
            Err(_) => return SharedTradeSettlementOutcome::Rejected,
        };
        match self.store.transact(&transaction) {
            Ok(receipt) if receipt.duplicate => SharedTradeSettlementOutcome::DurableDuplicate {
                event_id: receipt.event_id,
            },
            Ok(receipt) => SharedTradeSettlementOutcome::DurableCommitted {
                event_id: receipt.event_id,
            },
            // A committed PostgreSQL transaction may still report an error when
            // its acknowledgement is lost. Only an integrity-validated lookup
            // may resolve that ambiguity; callers retain the debited offers if
            // the outcome remains unknown.
            Err(_) => match self.store.lookup(&transaction) {
                Ok(Some(receipt)) => SharedTradeSettlementOutcome::DurableDuplicate {
                    event_id: receipt.event_id,
                },
                Ok(None) => SharedTradeSettlementOutcome::Rejected,
                Err(_) => SharedTradeSettlementOutcome::OutcomeUnknown {
                    idempotency_key: transaction.idempotency_key,
                    execution_context: context.clone(),
                },
            },
        }
    }

    fn retry_trade_fenced(
        &self,
        context: Option<&SharedAccountInventoryExecutionContext>,
        expected_idempotency_key: &str,
        first: &SharedTradeOffer,
        second: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        let Some(context) = context else {
            return SharedTradeSettlementOutcome::Deferred;
        };
        let generated_key = match economy_transaction_for_trade(context, first, second) {
            Ok(Some(transaction)) => transaction.idempotency_key,
            Ok(None) | Err(_) => return SharedTradeSettlementOutcome::Deferred,
        };
        if generated_key != expected_idempotency_key {
            return SharedTradeSettlementOutcome::Deferred;
        }
        self.settle_trade_fenced(Some(context), first, second)
    }

    fn reconcile_trade_projections_fenced(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> Vec<ServerPacket> {
        let Some(context) = context else {
            return Vec::new();
        };
        if !context.external_commit_authorized {
            return Vec::new();
        }
        let Some(identity) = runtime.active_identity() else {
            return Vec::new();
        };
        let pending = match self.store.pending_trade_projections(&identity) {
            Ok(pending) => pending,
            Err(_) => return Vec::new(),
        };
        let mut packets = Vec::new();
        for projection in pending {
            match runtime.apply_shared_trade_settlement_projection(
                &projection.event_id,
                &projection.own_offer,
                &projection.incoming_offer,
            ) {
                Ok(mut projected_packets) => {
                    // The runtime call persists the event marker together with
                    // the character state. A mark failure is intentionally
                    // left pending; retry observes the marker and only repeats
                    // this idempotent durable status update. The already-durable
                    // client update must not be suppressed by that retry work.
                    packets.append(&mut projected_packets);
                    let _ = self
                        .store
                        .mark_trade_projection_projected(&identity, &projection.event_id);
                }
                Err(_) => {}
            }
        }
        packets
    }

    fn has_pending_trade_projection_fenced(
        &self,
        runtime: &InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> bool {
        let Some(context) = context else {
            return true;
        };
        if !context.external_commit_authorized {
            return true;
        }
        let Some(identity) = runtime.active_identity() else {
            return false;
        };
        match self.store.pending_trade_projections(&identity) {
            Ok(pending) => !pending.is_empty(),
            // The World Director must not discard session-bound trade state if
            // the durable query is unavailable or integrity verification fails.
            Err(_) => true,
        }
    }
}

fn command_kind(command: &SharedAccountInventoryCommand) -> SharedAccountInventoryTransactionKind {
    match command {
        SharedAccountInventoryCommand::GoldDrop { .. } => {
            SharedAccountInventoryTransactionKind::GoldDrop
        }
        SharedAccountInventoryCommand::InventoryItemDrop { .. } => {
            SharedAccountInventoryTransactionKind::InventoryItemDrop
        }
        SharedAccountInventoryCommand::GroundDropPickup(_)
        | SharedAccountInventoryCommand::GroundDropClaimPickup { .. } => {
            SharedAccountInventoryTransactionKind::GroundDropPickup
        }
        SharedAccountInventoryCommand::MonsterKillAward(_) => {
            SharedAccountInventoryTransactionKind::MonsterKillAward
        }
        SharedAccountInventoryCommand::SkillItemConsume { .. } => {
            SharedAccountInventoryTransactionKind::SkillItemConsumption
        }
    }
}

fn preflight_projection(
    runtime: &InProcessWorldRuntime,
    command: &SharedAccountInventoryCommand,
) -> bool {
    match command {
        SharedAccountInventoryCommand::GoldDrop { amount, .. } => {
            runtime.can_commit_shared_gold_drop(*amount)
        }
        SharedAccountInventoryCommand::InventoryItemDrop { drop, .. } => {
            runtime.can_commit_shared_inventory_item_drop(drop)
        }
        SharedAccountInventoryCommand::GroundDropPickup(drop)
        | SharedAccountInventoryCommand::GroundDropClaimPickup { drop, .. } => {
            runtime.can_commit_shared_ground_drop_pickup(drop)
        }
        SharedAccountInventoryCommand::MonsterKillAward(_) => runtime.active_identity().is_some(),
        SharedAccountInventoryCommand::SkillItemConsume {
            spell, components, ..
        } => {
            !components.is_empty()
                && runtime.shared_skill_item_consumption_components(*spell)
                    == Some(components.clone())
        }
    }
}

fn economy_transaction_for_command(
    context: &SharedAccountInventoryExecutionContext,
    command: &SharedAccountInventoryCommandEnvelope,
    experience_balance_delta: Option<i64>,
) -> Option<EconomyTransactionEnvelope> {
    let identity = &command.identity;
    // Ground-drop economic identity belongs to the authoritative Zone object
    // generation, not the player who happens to claim it. A stale checkpoint
    // can therefore never credit the same drop to a second account.
    let stable_key = match &command.command {
        SharedAccountInventoryCommand::GroundDropClaimPickup {
            claim_idempotency_key,
            ..
        } => claim_idempotency_key.clone(),
        _ => command.stable_idempotency_key(),
    };
    let mut metadata = BTreeMap::from([
        ("producer".to_string(), "mir2-zone".to_string()),
        ("characterName".to_string(), identity.character_name.clone()),
    ]);
    let ground_drop_projection_intent = match &command.command {
        SharedAccountInventoryCommand::GroundDropPickup(drop) => Some(GroundDropProjectionIntent {
            version: 1,
            identity: identity.clone(),
            drop: drop.clone(),
            claim_idempotency_key: None,
        }),
        SharedAccountInventoryCommand::GroundDropClaimPickup {
            drop,
            claim_idempotency_key,
        } => Some(GroundDropProjectionIntent {
            version: 1,
            identity: identity.clone(),
            drop: drop.clone(),
            claim_idempotency_key: Some(claim_idempotency_key.clone()),
        }),
        _ => None,
    };
    if let Some(intent) = ground_drop_projection_intent {
        metadata.insert(
            "groundDropProjectionV1".to_string(),
            serde_json::to_string(&intent).ok()?,
        );
    }
    let (transaction_kind, legs) = match &command.command {
        SharedAccountInventoryCommand::GoldDrop { amount, request_id } if *amount > 0 => {
            metadata.insert("operation".to_string(), "playerGoldDrop".to_string());
            metadata.insert("requestId".to_string(), request_id.to_string());
            (
                EconomyTransactionKind::Consume,
                vec![EconomyLeg {
                    balance: EconomyBalanceKey::gold(
                        identity.account_id.clone(),
                        identity.character_index,
                    ),
                    delta: -i64::from(*amount),
                }],
            )
        }
        SharedAccountInventoryCommand::InventoryItemDrop { drop, request_id }
            if drop.quantity > 0 =>
        {
            metadata.insert("operation".to_string(), "playerItemDrop".to_string());
            metadata.insert("requestId".to_string(), request_id.to_string());
            metadata.insert("itemKey".to_string(), drop.item_key.clone());
            metadata.insert("uniqueId".to_string(), drop.unique_id.to_string());
            metadata.insert("quantity".to_string(), drop.quantity.to_string());
            (
                EconomyTransactionKind::Consume,
                vec![EconomyLeg {
                    balance: EconomyBalanceKey::item_quantity(
                        identity.account_id.clone(),
                        identity.character_index,
                        drop.item_key.clone(),
                    ),
                    delta: -i64::from(drop.quantity),
                }],
            )
        }
        SharedAccountInventoryCommand::GroundDropPickup(drop)
        | SharedAccountInventoryCommand::GroundDropClaimPickup { drop, .. } => match &drop.loot {
            GroundDropLootSnapshot::Gold { amount } => {
                metadata.insert("operation".to_string(), "groundDropGoldPickup".to_string());
                metadata.insert("objectId".to_string(), drop.object_id.to_string());
                (
                    EconomyTransactionKind::Reward,
                    vec![EconomyLeg {
                        balance: EconomyBalanceKey::gold(
                            identity.account_id.clone(),
                            identity.character_index,
                        ),
                        delta: i64::from(*amount),
                    }],
                )
            }
            GroundDropLootSnapshot::InventoryItem { key, name, .. } => {
                metadata.insert("operation".to_string(), "groundDropItemPickup".to_string());
                metadata.insert("objectId".to_string(), drop.object_id.to_string());
                metadata.insert("itemKey".to_string(), key.clone());
                metadata.insert("itemName".to_string(), name.clone());
                metadata.insert("quantity".to_string(), drop.quantity.to_string());
                (
                    EconomyTransactionKind::Reward,
                    vec![EconomyLeg {
                        balance: EconomyBalanceKey::item_quantity(
                            identity.account_id.clone(),
                            identity.character_index,
                            key.clone(),
                        ),
                        delta: i64::from(drop.quantity),
                    }],
                )
            }
        },
        SharedAccountInventoryCommand::MonsterKillAward(award) if award.experience > 0 => {
            metadata.insert("operation".to_string(), "monsterKillExperience".to_string());
            metadata.insert(
                "monsterObjectId".to_string(),
                award.monster_object_id.to_string(),
            );
            metadata.insert("monsterName".to_string(), award.monster_name.clone());
            let balance_delta =
                experience_balance_delta.unwrap_or_else(|| i64::from(award.experience));
            metadata.insert("rawExperience".to_string(), award.experience.to_string());
            metadata.insert(
                "experienceBalanceDelta".to_string(),
                balance_delta.to_string(),
            );
            (
                EconomyTransactionKind::Reward,
                vec![EconomyLeg {
                    balance: EconomyBalanceKey::experience(
                        identity.account_id.clone(),
                        identity.character_index,
                    ),
                    delta: balance_delta,
                }],
            )
        }
        SharedAccountInventoryCommand::SkillItemConsume {
            spell,
            request_id,
            components,
        } if !components.is_empty() => {
            metadata.insert("operation".to_string(), "skillItemConsume".to_string());
            metadata.insert("spell".to_string(), (*spell as u8).to_string());
            metadata.insert("requestId".to_string(), request_id.to_string());
            metadata.insert(
                "components".to_string(),
                components
                    .iter()
                    .map(|component| format!("{}:{}", component.item_key, component.quantity))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            (
                EconomyTransactionKind::Consume,
                components
                    .iter()
                    .map(|component| EconomyLeg {
                        balance: EconomyBalanceKey::item_quantity(
                            identity.account_id.clone(),
                            identity.character_index,
                            component.item_key.clone(),
                        ),
                        delta: -i64::from(component.quantity),
                    })
                    .collect(),
            )
        }
        SharedAccountInventoryCommand::GoldDrop { .. }
        | SharedAccountInventoryCommand::InventoryItemDrop { .. }
        | SharedAccountInventoryCommand::MonsterKillAward(_)
        | SharedAccountInventoryCommand::SkillItemConsume { .. } => return None,
    };
    Some(EconomyTransactionEnvelope {
        idempotency_key: format!("zone:{}:{stable_key}", context.zone_id),
        transaction_kind,
        zone_id: context.zone_id.as_str().to_string(),
        fencing_generation: context.fencing_generation,
        source_sequence: context.source_sequence,
        created_at_ms: context.created_at_ms,
        legs,
        metadata,
    })
}

fn economy_transaction_for_trade(
    context: &SharedAccountInventoryExecutionContext,
    first: &SharedTradeOffer,
    second: &SharedTradeOffer,
) -> Result<Option<EconomyTransactionEnvelope>, String> {
    if first.account_id == second.account_id && first.character_index == second.character_index {
        return Err("economy trade requires two different characters".to_string());
    }
    if !first
        .partner_name
        .eq_ignore_ascii_case(&second.character_name)
        || !second
            .partner_name
            .eq_ignore_ascii_case(&first.character_name)
    {
        return Err("economy trade partners are not reciprocal".to_string());
    }

    let intent = TradeProjectionIntent {
        version: 1,
        first: first.clone(),
        second: second.clone(),
    };
    let intent_json = serde_json::to_string(&intent)
        .map_err(|error| format!("encode economy trade projection intent: {error}"))?;
    let mut legs = Vec::new();
    append_trade_offer_legs(first, second, &mut legs)?;
    append_trade_offer_legs(second, first, &mut legs)?;
    if legs.is_empty() {
        return Ok(None);
    }
    let business_digest = trade_business_digest(first, second)?;
    let settlement_digest = trade_settlement_digest(first, second, &business_digest)?;
    let metadata = BTreeMap::from([
        ("operation".to_string(), "playerTrade".to_string()),
        ("producer".to_string(), "mir2-zone".to_string()),
        ("tradeDigest".to_string(), business_digest.clone()),
        ("settlementDigest".to_string(), settlement_digest.clone()),
        (
            "participants".to_string(),
            format!(
                "{}/{}|{}/{}",
                first.account_id, first.character_index, second.account_id, second.character_index
            ),
        ),
        ("tradeProjectionV1".to_string(), intent_json),
    ]);
    Ok(Some(EconomyTransactionEnvelope {
        idempotency_key: format!("zone:{}:trade:{settlement_digest}", context.zone_id),
        transaction_kind: EconomyTransactionKind::Trade,
        zone_id: context.zone_id.as_str().to_string(),
        fencing_generation: context.fencing_generation,
        source_sequence: context.source_sequence,
        created_at_ms: context.created_at_ms,
        legs,
        metadata,
    }))
}

fn append_trade_offer_legs(
    source: &SharedTradeOffer,
    destination: &SharedTradeOffer,
    legs: &mut Vec<EconomyLeg>,
) -> Result<(), String> {
    if source.gold > 0 {
        let amount = i64::from(source.gold);
        legs.push(EconomyLeg {
            balance: EconomyBalanceKey::gold(source.account_id.clone(), source.character_index),
            delta: -amount,
        });
        legs.push(EconomyLeg {
            balance: EconomyBalanceKey::gold(
                destination.account_id.clone(),
                destination.character_index,
            ),
            delta: amount,
        });
    }
    for item in &source.items {
        if item.key.trim().is_empty() {
            return Err("economy trade item key cannot be empty".to_string());
        }
        let value: serde_json::Value = serde_json::from_str(&item.item_state_json)
            .map_err(|error| format!("decode economy trade item state: {error}"))?;
        let quantity = value
            .get("quantity")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        if quantity == 0 || quantity > i64::MAX as u64 {
            return Err("economy trade item quantity is invalid".to_string());
        }
        let quantity = quantity as i64;
        legs.push(EconomyLeg {
            balance: EconomyBalanceKey::item_quantity(
                source.account_id.clone(),
                source.character_index,
                item.key.clone(),
            ),
            delta: -quantity,
        });
        legs.push(EconomyLeg {
            balance: EconomyBalanceKey::item_quantity(
                destination.account_id.clone(),
                destination.character_index,
                item.key.clone(),
            ),
            delta: quantity,
        });
    }
    Ok(())
}

fn trade_settlement_digest(
    first: &SharedTradeOffer,
    second: &SharedTradeOffer,
    business_digest: &str,
) -> Result<String, String> {
    let valid_nonce = |nonce: &str| {
        nonce.len() == 32
            && nonce
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    if !valid_nonce(&first.settlement_nonce)
        || !valid_nonce(&second.settlement_nonce)
        || first.settlement_nonce == second.settlement_nonce
    {
        return Err("economy trade requires two distinct persistent settlement nonces".to_string());
    }
    let mut nonces = [
        first.settlement_nonce.as_str(),
        second.settlement_nonce.as_str(),
    ];
    nonces.sort_unstable();
    let payload = serde_json::to_vec(&(nonces, business_digest))
        .map_err(|error| format!("encode economy trade settlement: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(ECONOMY_TRADE_SETTLEMENT_DOMAIN);
    hasher.update(payload);
    Ok(hex_lower(&hasher.finalize()))
}
fn trade_business_digest(
    first: &SharedTradeOffer,
    second: &SharedTradeOffer,
) -> Result<String, String> {
    let mut offers = [first, second];
    offers.sort_by(|left, right| {
        (&left.account_id, left.character_index, &left.character_name).cmp(&(
            &right.account_id,
            right.character_index,
            &right.character_name,
        ))
    });
    let payload =
        serde_json::to_vec(&offers).map_err(|error| format!("encode economy trade: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(ECONOMY_TRADE_DOMAIN);
    hasher.update(payload);
    Ok(hex_lower(&hasher.finalize()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EconomyOpeningSnapshot {
    account_id: String,
    character_index: i32,
    character_name: String,
    gold: i64,
    experience: i64,
    item_quantities: BTreeMap<String, i64>,
    total_item_quantity: i64,
}

impl EconomyOpeningSnapshot {
    fn from_runtime(
        identity: &ActiveSessionIdentity,
        snapshot: &WorldSnapshot,
    ) -> Result<Self, String> {
        if identity.account_id.trim().is_empty() || identity.character_index < 0 {
            return Err("economy bootstrap requires a valid active identity".to_string());
        }
        if snapshot.player_experience < 0 {
            return Err("economy bootstrap experience cannot be negative".to_string());
        }
        let mut item_quantities = BTreeMap::<String, i64>::new();
        for item in snapshot
            .inventory_items
            .iter()
            .chain(snapshot.belt_items.iter())
            .chain(snapshot.storage_items.iter())
            .chain(snapshot.hero_inventory_items.iter())
        {
            if item.key.trim().is_empty() {
                return Err("economy bootstrap item key cannot be empty".to_string());
            }
            let quantity = i64::from(item.quantity);
            let total = item_quantities.entry(item.key.clone()).or_default();
            *total = total
                .checked_add(quantity)
                .ok_or_else(|| "economy bootstrap item quantity overflow".to_string())?;
        }
        for item in &snapshot.equipment_items {
            if item.key.trim().is_empty() {
                return Err("economy bootstrap equipment key cannot be empty".to_string());
            }
            let total = item_quantities.entry(item.key.clone()).or_default();
            *total = total
                .checked_add(i64::from(item.quantity))
                .ok_or_else(|| "economy bootstrap equipment quantity overflow".to_string())?;
        }
        item_quantities.retain(|_, quantity| *quantity > 0);
        let total_item_quantity = item_quantities
            .values()
            .try_fold(0_i64, |total, quantity| {
                total
                    .checked_add(*quantity)
                    .ok_or_else(|| "economy bootstrap total item quantity overflow".to_string())
            })?;
        Ok(Self {
            account_id: identity.account_id.clone(),
            character_index: identity.character_index,
            character_name: identity.character_name.clone(),
            gold: i64::from(snapshot.gold),
            experience: snapshot.player_experience,
            item_quantities,
            total_item_quantity,
        })
    }

    fn digest(&self) -> Result<String, String> {
        let payload = serde_json::to_vec(self)
            .map_err(|error| format!("encode economy bootstrap snapshot: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(ECONOMY_BOOTSTRAP_DOMAIN);
        hasher.update(payload);
        Ok(hex_lower(&hasher.finalize()))
    }

    fn balance_amounts(&self) -> BTreeMap<(String, String), i64> {
        let mut balances = BTreeMap::new();
        if self.gold > 0 {
            balances.insert(("gold".to_string(), "gold".to_string()), self.gold);
        }
        if self.experience > 0 {
            balances.insert(
                ("experience".to_string(), "experience".to_string()),
                self.experience,
            );
        }
        for (key, quantity) in &self.item_quantities {
            if *quantity > 0 {
                balances.insert(("item_quantity".to_string(), key.clone()), *quantity);
            }
        }
        balances
    }
}

fn lock_economy_characters(
    transaction: &mut Transaction<'_>,
    characters: &BTreeSet<(String, i32)>,
) -> Result<(), String> {
    for (account_id, character_index) in characters {
        let lock_key = format!("obelisk.mir2.economy-character:{account_id}:{character_index}");
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
                &[&lock_key],
            )
            .map_err(|error| format!("economy character lock failed: {error}"))?;
    }
    Ok(())
}

fn aggregate_legs(legs: &[EconomyLeg]) -> Result<BTreeMap<EconomyBalanceKey, i64>, String> {
    let mut aggregated = BTreeMap::<EconomyBalanceKey, i64>::new();
    for leg in legs {
        let value = aggregated.entry(leg.balance.clone()).or_default();
        *value = value
            .checked_add(leg.delta)
            .ok_or_else(|| "economy aggregate delta overflow".to_string())?;
    }
    aggregated.retain(|_, delta| *delta != 0);
    if aggregated.is_empty() {
        return Err("economy transaction has no net effect".to_string());
    }
    Ok(aggregated)
}

fn balance_receipt_key(key: &EconomyBalanceKey) -> Result<String, String> {
    serde_json::to_string(&(
        key.account_id.as_str(),
        key.character_index,
        key.asset_kind.as_str(),
        key.asset_key.as_str(),
    ))
    .map_err(|error| format!("encode economy balance receipt key: {error}"))
}

fn ensure_balance_row(
    transaction: &mut Transaction<'_>,
    key: &EconomyBalanceKey,
) -> Result<(), String> {
    transaction
        .execute(
            "INSERT INTO game_economy_balances
             (account_id,character_index,asset_kind,asset_key,amount,balance_version)
             VALUES ($1,$2,$3,$4,0,0)
             ON CONFLICT DO NOTHING",
            &[
                &key.account_id,
                &key.character_index,
                &key.asset_kind,
                &key.asset_key,
            ],
        )
        .map_err(|error| format!("economy balance bootstrap failed: {error}"))?;
    Ok(())
}

fn validate_worker_or_consumer(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(format!("invalid economy {label} id"));
    }
    Ok(())
}

fn truncate_error(error: &str) -> String {
    error.chars().take(2_048).collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::ClientPacket;
    use mir2_simulation::{
        AccountStoreTransactionFault, ActiveSessionIdentity, GroundDropSnapshot,
    };

    fn leg(account: &str, asset: &str, delta: i64) -> EconomyLeg {
        EconomyLeg {
            balance: EconomyBalanceKey::item(account, 0, asset),
            delta,
        }
    }

    #[derive(Debug, Default)]
    struct FakeEconomyStore {
        state: Mutex<FakeEconomyState>,
    }

    #[derive(Debug, Default, Clone)]
    struct FakeEconomyState {
        balances: BTreeMap<EconomyBalanceKey, i64>,
        bootstrapped: BTreeSet<(String, i32)>,
        transactions: BTreeMap<String, (EconomyOutboxEvent, EconomyTransactionReceipt)>,
        trade_projections: BTreeMap<(String, String, i32), FakeTradeProjectionRow>,
        ground_drop_projections: BTreeMap<(String, String, i32), FakeGroundDropProjectionRow>,
        fail_next_transact: bool,
        commit_then_fail_next_transact: bool,
        fail_lookup_after_commit_then_error: bool,
        fail_next_lookup: bool,
        fail_next_trade_projection_mark: bool,
        fail_next_ground_drop_projection_mark: bool,
        fail_pending_trade_projection_query: bool,
        fail_pending_ground_drop_projection_query: bool,
        fail_next_bootstrap: bool,
        delay_commit_visibility_until_lookup: bool,
        pending_transaction: Option<(String, EconomyOutboxEvent, EconomyTransactionReceipt)>,
        store_calls: usize,
    }

    #[derive(Debug, Clone)]
    struct FakeTradeProjectionRow {
        own_offer: SharedTradeOffer,
        incoming_offer: SharedTradeOffer,
        projected: bool,
    }

    #[derive(Debug, Clone)]
    struct FakeGroundDropProjectionRow {
        intent: GroundDropProjectionIntent,
        projected: bool,
    }
    impl FakeEconomyStore {
        fn fail_next_bootstrap(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fail_next_bootstrap = true;
        }

        fn delay_commit_visibility_until_lookup(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .delay_commit_visibility_until_lookup = true;
        }

        fn fail_next_trade_projection_mark(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fail_next_trade_projection_mark = true;
        }

        fn fail_pending_trade_projection_query(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fail_pending_trade_projection_query = true;
        }

        fn fail_next_ground_drop_projection_mark(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fail_next_ground_drop_projection_mark = true;
        }

        fn fail_pending_ground_drop_projection_query(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fail_pending_ground_drop_projection_query = true;
        }
        fn fail_next_transact(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fail_next_transact = true;
        }

        fn commit_then_fail_next_transact(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .commit_then_fail_next_transact = true;
        }

        fn commit_then_fail_next_transact_and_lookup(&self) {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.commit_then_fail_next_transact = true;
            state.fail_lookup_after_commit_then_error = true;
        }

        fn fail_next_lookup(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fail_next_lookup = true;
        }

        fn balance(&self, key: &EconomyBalanceKey) -> i64 {
            *self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .balances
                .get(key)
                .unwrap_or(&0)
        }

        fn transaction_count(&self) -> usize {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .transactions
                .len()
        }

        fn transaction_keys(&self) -> Vec<String> {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .transactions
                .keys()
                .cloned()
                .collect()
        }

        fn store_call_count(&self) -> usize {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .store_calls
        }

        fn record_store_call(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .store_calls += 1;
        }
    }

    impl EconomySettlementStore for FakeEconomyStore {
        fn ensure_migrated(&self) -> Result<(), String> {
            self.record_store_call();
            Ok(())
        }

        fn bootstrap_character(
            &self,
            identity: &ActiveSessionIdentity,
            snapshot: &WorldSnapshot,
            bootstrapped_at_ms: u64,
        ) -> Result<EconomyBootstrapReceipt, String> {
            self.record_store_call();
            let opening = EconomyOpeningSnapshot::from_runtime(identity, snapshot)?;
            let snapshot_digest = opening.digest()?;
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.fail_next_bootstrap {
                state.fail_next_bootstrap = false;
                return Err("injected bootstrap failure".to_string());
            }
            let character = (identity.account_id.clone(), identity.character_index);
            let duplicate = !state.bootstrapped.insert(character);
            if !duplicate {
                for ((asset_kind, asset_key), amount) in opening.balance_amounts() {
                    state.balances.insert(
                        EconomyBalanceKey {
                            account_id: identity.account_id.clone(),
                            character_index: identity.character_index,
                            asset_kind,
                            asset_key,
                        },
                        amount,
                    );
                }
            }
            Ok(EconomyBootstrapReceipt {
                account_id: identity.account_id.clone(),
                character_index: identity.character_index,
                snapshot_digest,
                gold: opening.gold,
                experience: opening.experience,
                item_quantity: opening.total_item_quantity,
                item_kind_count: opening.item_quantities.len(),
                bootstrapped_at_ms,
                duplicate,
            })
        }

        fn lookup(
            &self,
            envelope: &EconomyTransactionEnvelope,
        ) -> Result<Option<EconomyTransactionReceipt>, String> {
            self.record_store_call();
            envelope.validate()?;
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some((idempotency_key, event, receipt)) = state.pending_transaction.take() {
                state.transactions.insert(idempotency_key, (event, receipt));
            }
            if state.fail_next_lookup {
                state.fail_next_lookup = false;
                return Err("injected lookup failure".to_string());
            }
            state
                .transactions
                .get(&envelope.idempotency_key)
                .map(|(event, receipt)| duplicate_receipt_from_stored(event, receipt, envelope))
                .transpose()
        }

        fn transact(
            &self,
            envelope: &EconomyTransactionEnvelope,
        ) -> Result<EconomyTransactionReceipt, String> {
            self.record_store_call();
            envelope.validate()?;
            let event_id = envelope.event_id()?;
            let projection_rows = trade_projection_rows_from_envelope(envelope, &event_id)?;
            let ground_drop_projection = ground_drop_projection_from_envelope(envelope, &event_id)?;
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.fail_next_transact {
                state.fail_next_transact = false;
                return Err("injected pre-commit failure".to_string());
            }
            if let Some((stored_event, stored_receipt)) =
                state.transactions.get(&envelope.idempotency_key)
            {
                return duplicate_receipt_from_stored(stored_event, stored_receipt, envelope);
            }

            let aggregated = aggregate_legs(&envelope.legs)?;
            let mut next_balances = state.balances.clone();
            let mut balances_after = BTreeMap::new();
            for (key, delta) in aggregated {
                let current = *next_balances.get(&key).unwrap_or(&0);
                let after = current
                    .checked_add(delta)
                    .ok_or_else(|| "economy balance overflow".to_string())?;
                if after < 0 || (key.asset_kind == "item" && after > 1) {
                    return Err("invalid fake economy balance transition".to_string());
                }
                next_balances.insert(key.clone(), after);
                balances_after.insert(balance_receipt_key(&key)?, after);
            }
            let receipt = EconomyTransactionReceipt {
                idempotency_key: envelope.idempotency_key.clone(),
                event_id: event_id.clone(),
                transaction_kind: envelope.transaction_kind,
                committed_at_ms: envelope.created_at_ms,
                balances_after,
                duplicate: false,
                settled_elsewhere: false,
            };
            let event = EconomyOutboxEvent {
                event_id: receipt.event_id.clone(),
                idempotency_key: envelope.idempotency_key.clone(),
                envelope: envelope.clone(),
                receipt_digest: receipt_integrity_digest(&receipt)?,
            };
            state.balances = next_balances;
            let delayed_visibility = state.delay_commit_visibility_until_lookup;
            state.delay_commit_visibility_until_lookup = false;
            if delayed_visibility {
                state.pending_transaction =
                    Some((envelope.idempotency_key.clone(), event, receipt.clone()));
            } else {
                state
                    .transactions
                    .insert(envelope.idempotency_key.clone(), (event, receipt.clone()));
            }
            for projection in projection_rows {
                state.trade_projections.insert(
                    (
                        receipt.event_id.clone(),
                        projection.account_id,
                        projection.character_index,
                    ),
                    FakeTradeProjectionRow {
                        own_offer: projection.own_offer,
                        incoming_offer: projection.incoming_offer,
                        projected: false,
                    },
                );
            }
            if let Some(projection) = ground_drop_projection {
                state.ground_drop_projections.insert(
                    (
                        receipt.event_id.clone(),
                        projection.account_id,
                        projection.character_index,
                    ),
                    FakeGroundDropProjectionRow {
                        intent: projection.intent,
                        projected: false,
                    },
                );
            }
            if delayed_visibility {
                return Err("injected delayed commit visibility".to_string());
            }
            if state.commit_then_fail_next_transact {
                state.commit_then_fail_next_transact = false;
                if state.fail_lookup_after_commit_then_error {
                    state.fail_lookup_after_commit_then_error = false;
                    state.fail_next_lookup = true;
                }
                return Err("injected post-commit acknowledgement failure".to_string());
            }
            Ok(receipt)
        }

        fn pending_trade_projections(
            &self,
            identity: &ActiveSessionIdentity,
        ) -> Result<Vec<DurableTradeProjection>, String> {
            self.record_store_call();
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.fail_pending_trade_projection_query {
                state.fail_pending_trade_projection_query = false;
                return Err("injected pending trade projection query failure".to_string());
            }
            let mut pending = Vec::new();
            for ((event_id, account_id, character_index), projection) in &state.trade_projections {
                if account_id != &identity.account_id
                    || *character_index != identity.character_index
                    || projection.projected
                {
                    continue;
                }
                let (event, receipt) = state
                    .transactions
                    .values()
                    .find(|(event, _)| &event.event_id == event_id)
                    .ok_or_else(|| "fake economy trade projection event is missing".to_string())?;
                validate_stored_economy_transaction(event, receipt)?;
                let expected = trade_projection_rows_from_envelope(&event.envelope, event_id)?;
                if !expected.iter().any(|expected| {
                    expected.account_id == *account_id
                        && expected.character_index == *character_index
                        && expected.own_offer == projection.own_offer
                        && expected.incoming_offer == projection.incoming_offer
                }) {
                    return Err("fake economy trade projection integrity conflict".to_string());
                }
                pending.push(DurableTradeProjection {
                    event_id: event_id.clone(),
                    own_offer: projection.own_offer.clone(),
                    incoming_offer: projection.incoming_offer.clone(),
                });
            }
            Ok(pending)
        }

        fn mark_trade_projection_projected(
            &self,
            identity: &ActiveSessionIdentity,
            event_id: &str,
        ) -> Result<(), String> {
            self.record_store_call();
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.fail_next_trade_projection_mark {
                state.fail_next_trade_projection_mark = false;
                return Err("injected trade projection mark failure".to_string());
            }
            let projection = state
                .trade_projections
                .get_mut(&(
                    event_id.to_string(),
                    identity.account_id.clone(),
                    identity.character_index,
                ))
                .ok_or_else(|| "fake economy trade projection row is missing".to_string())?;
            projection.projected = true;
            Ok(())
        }

        fn pending_ground_drop_projections(
            &self,
            identity: &ActiveSessionIdentity,
        ) -> Result<Vec<DurableGroundDropProjection>, String> {
            self.record_store_call();
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.fail_pending_ground_drop_projection_query {
                state.fail_pending_ground_drop_projection_query = false;
                return Err("injected pending ground-drop projection query failure".to_string());
            }
            let mut pending = Vec::new();
            for ((event_id, account_id, character_index), projection) in
                &state.ground_drop_projections
            {
                if account_id != &identity.account_id
                    || *character_index != identity.character_index
                    || projection.projected
                {
                    continue;
                }
                let (event, receipt) = state
                    .transactions
                    .values()
                    .find(|(event, _)| &event.event_id == event_id)
                    .ok_or_else(|| {
                        "fake economy ground-drop projection event is missing".to_string()
                    })?;
                validate_stored_economy_transaction(event, receipt)?;
                let expected = ground_drop_projection_from_envelope(&event.envelope, event_id)?
                    .ok_or_else(|| {
                        "fake economy ground-drop projection is not a pickup".to_string()
                    })?;
                if expected.account_id != *account_id
                    || expected.character_index != *character_index
                    || expected.intent != projection.intent
                {
                    return Err(
                        "fake economy ground-drop projection integrity conflict".to_string()
                    );
                }
                pending.push(DurableGroundDropProjection {
                    event_id: event_id.clone(),
                    intent: projection.intent.clone(),
                });
            }
            Ok(pending)
        }

        fn mark_ground_drop_projection_projected(
            &self,
            identity: &ActiveSessionIdentity,
            event_id: &str,
        ) -> Result<(), String> {
            self.record_store_call();
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.fail_next_ground_drop_projection_mark {
                state.fail_next_ground_drop_projection_mark = false;
                return Err("injected ground-drop projection mark failure".to_string());
            }
            let projection = state
                .ground_drop_projections
                .get_mut(&(
                    event_id.to_string(),
                    identity.account_id.clone(),
                    identity.character_index,
                ))
                .ok_or_else(|| "fake economy ground-drop projection row is missing".to_string())?;
            projection.projected = true;
            Ok(())
        }
    }
    fn start_test_runtime(
        account_id: &str,
        character_name: &str,
    ) -> Result<InProcessWorldRuntime, String> {
        start_test_runtime_with_config(crate::GatewayConfig::default(), account_id, character_name)
    }

    fn start_test_runtime_with_config(
        config: crate::GatewayConfig,
        account_id: &str,
        character_name: &str,
    ) -> Result<InProcessWorldRuntime, String> {
        use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
        use mir2_simulation::WorldCommand;

        let mut runtime = InProcessWorldRuntime::new(config);
        runtime.execute(WorldCommand::ClientPacket(ClientPacket::NewAccount {
            account_id: account_id.to_string(),
            password: account_id.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        }))?;
        runtime.execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: account_id.to_string(),
        }))?;
        let character_index = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::NewCharacter {
                name: character_name.to_string(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            }))?
            .into_iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .ok_or_else(|| "economy test character creation returned no index".to_string())?;
        runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index,
        }))?;
        Ok(runtime)
    }

    fn claimed_gold_pickup(
        identity: ActiveSessionIdentity,
        object_id: u32,
        amount: u32,
        claim_idempotency_key: &str,
    ) -> SharedAccountInventoryCommandEnvelope {
        SharedAccountInventoryCommandEnvelope {
            identity,
            command: SharedAccountInventoryCommand::GroundDropClaimPickup {
                drop: GroundDropSnapshot {
                    object_id,
                    name: format!("{amount} Gold"),
                    name_colour_argb: -1,
                    icon: 0,
                    x: 10,
                    y: 20,
                    quantity: amount,
                    source_monster: "Economy recovery test".to_string(),
                    owner_object_id: None,
                    ownership_remaining_ticks: None,
                    loot: GroundDropLootSnapshot::Gold { amount },
                },
                claim_idempotency_key: claim_idempotency_key.to_string(),
            },
        }
    }

    fn external_context(
        fencing_generation: u64,
        source_sequence: u64,
        created_at_ms: u64,
    ) -> SharedAccountInventoryExecutionContext {
        SharedAccountInventoryExecutionContext {
            zone_id: crate::ZoneId::new("map:0"),
            fencing_generation,
            source_sequence,
            created_at_ms,
            external_commit_authorized: true,
        }
    }

    fn gold_trade_offer(
        identity: ActiveSessionIdentity,
        partner_name: impl Into<String>,
        settlement_nonce: impl Into<String>,
        gold: u32,
    ) -> SharedTradeOffer {
        SharedTradeOffer {
            settlement_nonce: settlement_nonce.into(),
            account_id: identity.account_id,
            character_index: identity.character_index,
            character_name: identity.character_name,
            partner_name: partner_name.into(),
            gold,
            items: Vec::new(),
        }
    }

    fn bootstrap_trade_balance(
        store: &FakeEconomyStore,
        identity: &ActiveSessionIdentity,
        runtime: &InProcessWorldRuntime,
        context: &SharedAccountInventoryExecutionContext,
        gold: i64,
    ) {
        store
            .bootstrap_character(identity, &runtime.world_snapshot(), context.created_at_ms)
            .expect("bootstrap trade character");
        store
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .balances
            .insert(
                EconomyBalanceKey::gold(&identity.account_id, identity.character_index),
                gold,
            );
    }

    #[test]
    fn missing_context_ground_drop_is_explicitly_deferred_without_store_access() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut runtime =
            start_test_runtime("economy-ground-no-context", "NoContextGround").unwrap();
        let identity = runtime.active_identity().unwrap();
        let command = claimed_gold_pickup(identity, 99_006, 25, "ground-no-context");
        let initial_gold = runtime.world_snapshot().gold;
        let initial_store_calls = store.store_call_count();

        let outcome = service.commit_fenced(&mut runtime, None, command);

        assert!(matches!(
            outcome,
            SharedAccountInventoryCommitOutcome::Deferred { receipt }
                if receipt.kind == SharedAccountInventoryTransactionKind::GroundDropPickup
                && !receipt.committed
        ));
        assert_eq!(runtime.world_snapshot().gold, initial_gold);
        assert_eq!(store.transaction_count(), 0);
        assert_eq!(store.store_call_count(), initial_store_calls);
    }

    #[test]
    fn missing_context_trade_is_explicitly_deferred_without_store_access() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let alice = start_test_runtime("economy-trade-no-context-alice", "NoContextAlice").unwrap();
        let bob = start_test_runtime("economy-trade-no-context-bob", "NoContextBob").unwrap();
        let alice_identity = alice.active_identity().unwrap();
        let bob_identity = bob.active_identity().unwrap();
        let first = gold_trade_offer(
            alice_identity,
            bob_identity.character_name.clone(),
            "00000000000000000000000000000085",
            10,
        );
        let second = gold_trade_offer(
            bob_identity,
            first.character_name.clone(),
            "00000000000000000000000000000086",
            0,
        );
        let initial_store_calls = store.store_call_count();
        let outcome = service.settle_trade_fenced(None, &first, &second);

        assert_eq!(outcome, SharedTradeSettlementOutcome::Deferred);
        assert_eq!(store.transaction_count(), 0);
        assert_eq!(store.store_call_count(), initial_store_calls);
    }

    #[test]
    fn ground_retry_rejects_regenerated_zone_key_before_store_and_uses_original_key() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut runtime = start_test_runtime("economy-ground-retry-key", "RetryGround").unwrap();
        let command = claimed_gold_pickup(
            runtime.active_identity().unwrap(),
            99_007,
            25,
            "ground-retry-key",
        );
        let original_context = external_context(7, 81, 8_100);
        let expected_key = economy_transaction_for_command(&original_context, &command, None)
            .expect("ground recovery transaction")
            .idempotency_key;
        let mut replacement_context = original_context.clone();
        replacement_context.zone_id = crate::ZoneId::new("map:replacement");
        replacement_context.fencing_generation = 8;
        replacement_context.source_sequence = 82;
        let initial_store_calls = store.store_call_count();

        let mismatched = service.retry_commit_fenced(
            &mut runtime,
            Some(&replacement_context),
            &expected_key,
            command.clone(),
        );
        assert!(matches!(
            mismatched,
            SharedAccountInventoryCommitOutcome::Deferred { .. }
        ));
        assert_eq!(store.store_call_count(), initial_store_calls);
        assert_eq!(store.transaction_count(), 0);

        let recovered = service.retry_commit_fenced(
            &mut runtime,
            Some(&original_context),
            &expected_key,
            command,
        );
        assert!(matches!(
            recovered,
            SharedAccountInventoryCommitOutcome::Confirmed(ref receipt) if receipt.committed
        ));
        assert_eq!(store.transaction_keys(), vec![expected_key]);
    }

    #[test]
    fn trade_retry_rejects_regenerated_zone_key_before_store_and_uses_original_key() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let alice = start_test_runtime("economy-trade-retry-alice", "RetryAlice").unwrap();
        let bob = start_test_runtime("economy-trade-retry-bob", "RetryBob").unwrap();
        let alice_identity = alice.active_identity().unwrap();
        let bob_identity = bob.active_identity().unwrap();
        let first = gold_trade_offer(
            alice_identity.clone(),
            bob_identity.character_name.clone(),
            "00000000000000000000000000000087",
            10,
        );
        let second = gold_trade_offer(
            bob_identity.clone(),
            alice_identity.character_name.clone(),
            "00000000000000000000000000000088",
            0,
        );
        let original_context = external_context(9, 91, 9_100);
        bootstrap_trade_balance(&store, &alice_identity, &alice, &original_context, 10);
        bootstrap_trade_balance(&store, &bob_identity, &bob, &original_context, 0);
        let expected_key = economy_transaction_for_trade(&original_context, &first, &second)
            .expect("valid trade recovery transaction")
            .expect("non-empty trade recovery transaction")
            .idempotency_key;
        let mut replacement_context = original_context.clone();
        replacement_context.zone_id = crate::ZoneId::new("map:replacement");
        replacement_context.fencing_generation = 10;
        replacement_context.source_sequence = 92;
        let initial_store_calls = store.store_call_count();

        assert_eq!(
            service.retry_trade_fenced(Some(&replacement_context), &expected_key, &first, &second,),
            SharedTradeSettlementOutcome::Deferred
        );
        assert_eq!(store.store_call_count(), initial_store_calls);
        assert_eq!(store.transaction_count(), 0);

        assert!(matches!(
            service.retry_trade_fenced(Some(&original_context), &expected_key, &first, &second,),
            SharedTradeSettlementOutcome::DurableCommitted { .. }
        ));
        assert_eq!(store.transaction_keys(), vec![expected_key]);
    }

    #[test]
    fn committed_then_ack_error_trade_recovers_by_lookup_and_keeps_projections_pending() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let alice = start_test_runtime("economy-trade-ack-alice", "AckAlice").unwrap();
        let bob = start_test_runtime("economy-trade-ack-bob", "AckBob").unwrap();
        let alice_identity = alice.active_identity().unwrap();
        let bob_identity = bob.active_identity().unwrap();
        let context = external_context(81, 901, 90_001);
        bootstrap_trade_balance(&store, &alice_identity, &alice, &context, 10);
        bootstrap_trade_balance(&store, &bob_identity, &bob, &context, 0);
        let first = gold_trade_offer(
            alice_identity.clone(),
            bob_identity.character_name.clone(),
            "00000000000000000000000000000081",
            10,
        );
        let second = gold_trade_offer(
            bob_identity.clone(),
            alice_identity.character_name.clone(),
            "00000000000000000000000000000082",
            0,
        );

        store.commit_then_fail_next_transact();
        let outcome = service.settle_trade_fenced(Some(&context), &first, &second);

        assert!(matches!(
            outcome,
            SharedTradeSettlementOutcome::DurableDuplicate { .. }
        ));
        assert_eq!(store.transaction_count(), 1);
        assert!(service.has_pending_trade_projection_fenced(&alice, Some(&context)));
        assert!(service.has_pending_trade_projection_fenced(&bob, Some(&context)));
    }

    #[test]
    fn committed_then_ack_error_ground_pickup_recovers_exactly_once() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut runtime = start_test_runtime("economy-ground-ack", "AckGround").unwrap();
        let identity = runtime.active_identity().unwrap();
        let context = external_context(82, 902, 90_002);
        let command = claimed_gold_pickup(identity.clone(), 99_001, 25, "ground-ack-claim");
        let initial_gold = runtime.world_snapshot().gold;

        store.commit_then_fail_next_transact();
        let first = service.commit_fenced(&mut runtime, Some(&context), command.clone());
        assert!(first.committed);
        assert_eq!(runtime.world_snapshot().gold, initial_gold + 25);
        assert_eq!(store.transaction_count(), 1);
        assert_eq!(
            store.balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            )),
            i64::from(initial_gold) + 25
        );

        let replay = service.commit_fenced(&mut runtime, Some(&context), command);
        assert!(replay.committed);
        assert!(replay.packets.is_empty());
        assert_eq!(runtime.world_snapshot().gold, initial_gold + 25);
        assert_eq!(store.transaction_count(), 1);
    }

    #[test]
    fn delayed_commit_visibility_is_resolved_before_absent_can_be_observed() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut runtime = start_test_runtime("economy-ground-delayed", "DelayedGround").unwrap();
        let identity = runtime.active_identity().unwrap();
        let context = external_context(85, 905, 90_005);
        let command = claimed_gold_pickup(identity, 99_003, 25, "ground-delayed-claim");
        let initial_gold = runtime.world_snapshot().gold;

        store.delay_commit_visibility_until_lookup();
        let outcome = service.commit_fenced(&mut runtime, Some(&context), command);

        assert!(outcome.committed);
        assert_eq!(runtime.world_snapshot().gold, initial_gold + 25);
        assert_eq!(store.transaction_count(), 1);
    }

    #[test]
    fn ground_bootstrap_failure_stays_unknown_until_retry() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut runtime =
            start_test_runtime("economy-ground-bootstrap", "BootstrapGround").unwrap();
        let identity = runtime.active_identity().unwrap();
        let context = external_context(86, 906, 90_006);
        let command = claimed_gold_pickup(identity, 99_004, 25, "ground-bootstrap-claim");
        let expected_key = economy_transaction_for_command(&context, &command, None)
            .expect("transaction")
            .idempotency_key;
        let initial_gold = runtime.world_snapshot().gold;

        store.fail_next_bootstrap();
        assert!(matches!(
            service.commit_fenced(&mut runtime, Some(&context), command.clone()),
            SharedAccountInventoryCommitOutcome::OutcomeUnknown { idempotency_key, .. }
                if idempotency_key == expected_key
        ));
        assert_eq!(runtime.world_snapshot().gold, initial_gold);
        assert_eq!(store.transaction_count(), 0);

        assert!(
            service
                .commit_fenced(&mut runtime, Some(&context), command)
                .committed
        );
        assert_eq!(runtime.world_snapshot().gold, initial_gold + 25);
        assert_eq!(store.transaction_count(), 1);
    }

    #[test]
    fn ground_initial_lookup_failure_stays_unknown_until_retry() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut runtime = start_test_runtime("economy-ground-lookup", "LookupGround").unwrap();
        let identity = runtime.active_identity().unwrap();
        let context = external_context(87, 907, 90_007);
        let command = claimed_gold_pickup(identity, 99_005, 25, "ground-lookup-claim");
        let expected_key = economy_transaction_for_command(&context, &command, None)
            .expect("transaction")
            .idempotency_key;
        let initial_gold = runtime.world_snapshot().gold;

        store.fail_next_lookup();
        assert!(matches!(
            service.commit_fenced(&mut runtime, Some(&context), command.clone()),
            SharedAccountInventoryCommitOutcome::OutcomeUnknown { idempotency_key, .. }
                if idempotency_key == expected_key
        ));
        assert_eq!(runtime.world_snapshot().gold, initial_gold);
        assert_eq!(store.transaction_count(), 0);

        assert!(
            service
                .commit_fenced(&mut runtime, Some(&context), command)
                .committed
        );
        assert_eq!(runtime.world_snapshot().gold, initial_gold + 25);
        assert_eq!(store.transaction_count(), 1);
    }

    #[test]
    fn commit_ack_error_with_lookup_failure_returns_ground_outcome_unknown() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut runtime = start_test_runtime("economy-ground-unknown", "UnknownGround").unwrap();
        let identity = runtime.active_identity().unwrap();
        let context = external_context(84, 904, 90_004);
        let command = claimed_gold_pickup(identity.clone(), 99_002, 25, "ground-unknown-claim");
        let transaction =
            economy_transaction_for_command(&context, &command, None).expect("transaction");
        let expected_key = transaction.idempotency_key.clone();
        let initial_gold = runtime.world_snapshot().gold;

        store.commit_then_fail_next_transact_and_lookup();
        let outcome = service.commit_fenced(&mut runtime, Some(&context), command);

        assert!(matches!(
            outcome,
            SharedAccountInventoryCommitOutcome::OutcomeUnknown {
                idempotency_key,
                ..
            } if idempotency_key == expected_key
        ));
        assert_eq!(runtime.world_snapshot().gold, initial_gold);
        assert_eq!(store.transaction_count(), 1);
        assert_eq!(
            store.balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            )),
            i64::from(initial_gold) + 25
        );
    }

    #[test]
    fn commit_ack_error_with_lookup_failure_returns_trade_outcome_unknown() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let alice = start_test_runtime("economy-trade-unknown-alice", "UnknownAlice").unwrap();
        let bob = start_test_runtime("economy-trade-unknown-bob", "UnknownBob").unwrap();
        let alice_identity = alice.active_identity().unwrap();
        let bob_identity = bob.active_identity().unwrap();
        let context = external_context(83, 903, 90_003);
        bootstrap_trade_balance(&store, &alice_identity, &alice, &context, 10);
        bootstrap_trade_balance(&store, &bob_identity, &bob, &context, 0);
        let first = gold_trade_offer(
            alice_identity.clone(),
            bob_identity.character_name.clone(),
            "00000000000000000000000000000083",
            10,
        );
        let second = gold_trade_offer(
            bob_identity.clone(),
            alice_identity.character_name.clone(),
            "00000000000000000000000000000084",
            0,
        );
        let expected_key = economy_transaction_for_trade(&context, &first, &second)
            .unwrap()
            .unwrap()
            .idempotency_key;

        store.commit_then_fail_next_transact();
        store.fail_next_lookup();
        let outcome = service.settle_trade_fenced(Some(&context), &first, &second);

        assert_eq!(
            outcome,
            SharedTradeSettlementOutcome::OutcomeUnknown {
                idempotency_key: expected_key,
                execution_context: context,
            }
        );
        assert_eq!(store.transaction_count(), 1);
    }

    #[test]
    fn business_effect_id_ignores_attempt_fields_but_binds_payload() {
        let mut first = EconomyTransactionEnvelope {
            idempotency_key: "reward:stable-generation".to_string(),
            transaction_kind: EconomyTransactionKind::Reward,
            zone_id: "map:0".to_string(),
            fencing_generation: 7,
            source_sequence: 12,
            created_at_ms: 1,
            legs: vec![leg("alice", "drop:1", 1)],
            metadata: BTreeMap::from([("operation".to_string(), "pickup".to_string())]),
        };
        let first_business_effect = first.business_effect_id().unwrap();
        let first_event = first.event_id().unwrap();
        first.fencing_generation = 99;
        first.source_sequence = 8_000;
        first.created_at_ms = 77_777;
        assert_eq!(first_business_effect, first.business_effect_id().unwrap());
        assert_ne!(first_event, first.event_id().unwrap());
        first.legs[0].delta = 2;
        assert_ne!(first_business_effect, first.business_effect_id().unwrap());
    }

    #[test]
    fn durable_commit_before_projection_replays_under_new_context_exactly_once() {
        let store = Arc::new(FakeEconomyStore::default());
        let original_runtime =
            start_test_runtime("economy-recovery", "RecoveryA").expect("original runtime");
        let identity = original_runtime.active_identity().expect("active identity");
        let command = claimed_gold_pickup(identity.clone(), 9_001, 25, "drop-generation:7");
        let original_context = external_context(7, 12, 1_000);
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        assert!(service.bootstrap_fenced(&original_runtime, Some(&original_context)));
        let original_gold = original_runtime.world_snapshot().gold;
        let transaction = economy_transaction_for_command(&original_context, &command, None)
            .expect("economy transaction");

        // Durable ledger/outbox commit succeeds, then the producer crashes
        // before apply_projection and before its process-local receipt cache.
        let first_receipt = store.transact(&transaction).expect("durable commit");
        assert!(!first_receipt.duplicate);
        assert_eq!(
            store.balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            )),
            i64::from(original_gold) + 25
        );
        assert_eq!(original_runtime.world_snapshot().gold, original_gold);
        drop(service);

        let mut recovered_runtime =
            start_test_runtime("economy-recovery", "RecoveryA").expect("recovered runtime");
        let recovered_identity = recovered_runtime
            .active_identity()
            .expect("recovered identity");
        assert_eq!(recovered_identity, identity);
        let recovered_command =
            claimed_gold_pickup(recovered_identity, 9_001, 25, "drop-generation:7");
        let recovered_service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let recovered_context = external_context(99, 8_000, 77_777);
        let recovered = recovered_service.commit_fenced(
            &mut recovered_runtime,
            Some(&recovered_context),
            recovered_command.clone(),
        );
        assert!(recovered.committed);
        assert_eq!(recovered_runtime.world_snapshot().gold, original_gold + 25);
        assert_eq!(store.transaction_count(), 1);

        // A fresh producer restoring a projection that already includes the
        // credit and marker treats the same durable receipt as materialized.
        let checkpoint = recovered_runtime
            .active_character_checkpoint()
            .expect("durable recovered checkpoint");
        let mut materialized_runtime =
            start_test_runtime("economy-recovery", "RecoveryA").expect("materialized runtime");
        materialized_runtime
            .restore_active_character_checkpoint(&checkpoint)
            .expect("restore recovered checkpoint");
        assert!(materialized_runtime.has_shared_economy_projection_event(&first_receipt.event_id));
        let materialized_service =
            PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let materialized = materialized_service.commit_fenced(
            &mut materialized_runtime,
            Some(&external_context(100, 8_001, 88_888)),
            recovered_command,
        );
        assert!(materialized.committed);
        assert!(materialized.packets.is_empty());
        assert_eq!(
            materialized_runtime.world_snapshot().gold,
            original_gold + 25
        );
        assert_eq!(store.transaction_count(), 1);
    }

    #[test]
    fn stale_cross_player_reclaim_removes_drop_without_second_credit() {
        let store = Arc::new(FakeEconomyStore::default());
        let alice_runtime =
            start_test_runtime("economy-alice", "EconomyAlice").expect("Alice runtime");
        let alice_identity = alice_runtime.active_identity().expect("Alice identity");
        let claim_key = "ground-drop:map:0:9004:generation:10:digest";
        let alice_command = claimed_gold_pickup(alice_identity.clone(), 9_004, 25, claim_key);
        let alice_context = external_context(10, 15, 4_000);
        store
            .bootstrap_character(&alice_identity, &alice_runtime.world_snapshot(), 4_000)
            .expect("Alice bootstrap");
        let alice_transaction =
            economy_transaction_for_command(&alice_context, &alice_command, None)
                .expect("Alice transaction");
        store
            .transact(&alice_transaction)
            .expect("Alice durable commit");

        let mut bob_runtime = start_test_runtime("economy-bob", "EconomyBob").expect("Bob runtime");
        let bob_identity = bob_runtime.active_identity().expect("Bob identity");
        let bob_initial_gold = bob_runtime.world_snapshot().gold;
        let bob_command = claimed_gold_pickup(bob_identity.clone(), 9_004, 25, claim_key);
        let bob_transaction =
            economy_transaction_for_command(&external_context(11, 16, 5_000), &bob_command, None)
                .expect("Bob transaction");
        assert_eq!(
            alice_transaction.idempotency_key,
            bob_transaction.idempotency_key
        );
        assert_ne!(
            alice_transaction.business_effect_id().unwrap(),
            bob_transaction.business_effect_id().unwrap()
        );

        let bob_service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let result = bob_service.commit_fenced(
            &mut bob_runtime,
            Some(&external_context(11, 16, 5_000)),
            bob_command,
        );
        assert!(result.committed);
        assert!(result.packets.is_empty());
        assert_eq!(bob_runtime.world_snapshot().gold, bob_initial_gold);
        assert_eq!(store.transaction_count(), 1);
        assert_eq!(
            store.balance(&EconomyBalanceKey::gold(
                &bob_identity.account_id,
                bob_identity.character_index
            )),
            i64::from(bob_initial_gold)
        );
    }

    #[test]
    fn durable_marked_pickup_bypasses_capacity_preflight_after_state_changes() {
        let store = Arc::new(FakeEconomyStore::default());
        let mut runtime = start_test_runtime("economy-marked-capacity", "CapacityA").unwrap();
        let identity = runtime.active_identity().unwrap();
        let command = claimed_gold_pickup(identity, 9_007, 25, "drop-generation:capacity");
        let context = external_context(14, 20, 8_000);
        let first_service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        assert!(
            first_service
                .commit_fenced(&mut runtime, Some(&context), command.clone())
                .committed
        );

        // A later saved state can be at the gold cap. The durable event marker
        // remains authoritative; retry must not re-run the capacity preflight
        // and resurrect the already-settled ground drop.
        let mut checkpoint = runtime.active_character_checkpoint().unwrap();
        checkpoint.gold = u32::MAX;
        runtime
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        let drop = match &command.command {
            SharedAccountInventoryCommand::GroundDropClaimPickup { drop, .. } => drop,
            _ => unreachable!("claimed gold pickup"),
        };
        assert!(!runtime.can_commit_shared_ground_drop_pickup(drop));

        let retry_service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let retry_context = external_context(99, 9_999, 99_999);
        let retry = retry_service.commit_fenced(&mut runtime, Some(&retry_context), command);
        assert!(retry.committed);
        assert!(retry.packets.is_empty());
        assert_eq!(store.transaction_count(), 1);
    }
    #[test]
    fn committed_ground_drop_survives_full_projection_and_reconciles_later() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut runtime = start_test_runtime("economy-ground-capacity", "GroundCapacity").unwrap();
        let identity = runtime.active_identity().unwrap();
        let context = external_context(21, 31, 10_001);
        let command = claimed_gold_pickup(identity.clone(), 91_001, 25, "ground-capacity-claim");
        let checkpoint = runtime.active_character_checkpoint().unwrap();
        let initial_gold = runtime.world_snapshot().gold;
        store
            .bootstrap_character(&identity, &runtime.world_snapshot(), context.created_at_ms)
            .unwrap();
        let transaction = economy_transaction_for_command(&context, &command, None).unwrap();
        store.transact(&transaction).unwrap();

        // The Zone receipt already committed; make the private character state
        // unable to accept it. The immediate call must still remove the world
        // drop and leave a durable pending projection rather than restoring it.
        let mut full_checkpoint = checkpoint.clone();
        full_checkpoint.gold = u32::MAX;
        runtime
            .restore_active_character_checkpoint(&full_checkpoint)
            .unwrap();
        let zone_receipt = service.commit_fenced(&mut runtime, Some(&context), command.clone());
        assert!(zone_receipt.committed);
        assert!(service.has_pending_ground_drop_projection_fenced(&runtime, Some(&context)));

        runtime
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        let _ = service.reconcile_ground_drop_projections_fenced(&mut runtime, Some(&context));
        assert_eq!(runtime.world_snapshot().gold, initial_gold + 25);
        assert!(!service.has_pending_ground_drop_projection_fenced(&runtime, Some(&context)));
        assert_eq!(store.transaction_count(), 1);
    }

    #[test]
    fn stale_cross_player_reclaim_keeps_original_ground_projection_pending() {
        let store = Arc::new(FakeEconomyStore::default());
        let alice_service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let bob_service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut alice = start_test_runtime("economy-ground-alice", "GroundAlice").unwrap();
        let alice_identity = alice.active_identity().unwrap();
        let alice_context = external_context(22, 32, 10_002);
        let claim_key = "ground-cross-player-claim";
        let alice_command = claimed_gold_pickup(alice_identity.clone(), 91_002, 25, claim_key);
        store
            .bootstrap_character(
                &alice_identity,
                &alice.world_snapshot(),
                alice_context.created_at_ms,
            )
            .unwrap();
        let alice_transaction =
            economy_transaction_for_command(&alice_context, &alice_command, None).unwrap();
        store.transact(&alice_transaction).unwrap();

        let mut bob = start_test_runtime("economy-ground-bob", "GroundBob").unwrap();
        let bob_identity = bob.active_identity().unwrap();
        let bob_initial_gold = bob.world_snapshot().gold;
        let bob_context = external_context(23, 33, 10_003);
        let bob_command = claimed_gold_pickup(bob_identity, 91_002, 25, claim_key);
        let stale_receipt = bob_service.commit_fenced(&mut bob, Some(&bob_context), bob_command);
        assert!(stale_receipt.committed);
        assert_eq!(bob.world_snapshot().gold, bob_initial_gold);
        assert!(
            alice_service.has_pending_ground_drop_projection_fenced(&alice, Some(&alice_context))
        );
        let _ = alice_service
            .reconcile_ground_drop_projections_fenced(&mut alice, Some(&alice_context));
        assert!(
            !alice_service.has_pending_ground_drop_projection_fenced(&alice, Some(&alice_context))
        );
        assert_eq!(store.transaction_count(), 1);
    }

    #[test]
    fn ground_drop_projection_mark_failure_retries_without_double_credit() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut runtime = start_test_runtime("economy-ground-mark", "GroundMark").unwrap();
        let identity = runtime.active_identity().unwrap();
        let context = external_context(24, 34, 10_004);
        let command = claimed_gold_pickup(identity.clone(), 91_003, 25, "ground-mark-claim");
        let initial_gold = runtime.world_snapshot().gold;
        store
            .bootstrap_character(&identity, &runtime.world_snapshot(), context.created_at_ms)
            .unwrap();
        let transaction = economy_transaction_for_command(&context, &command, None).unwrap();
        store.transact(&transaction).unwrap();

        // Query unavailability is fail-closed: a coordinator must not restore
        // or re-offer the world drop while it cannot inspect this durable row.
        store.fail_pending_ground_drop_projection_query();
        assert!(service.has_pending_ground_drop_projection_fenced(&runtime, Some(&context)));
        store.fail_next_ground_drop_projection_mark();
        let _ = service.reconcile_ground_drop_projections_fenced(&mut runtime, Some(&context));
        assert_eq!(runtime.world_snapshot().gold, initial_gold + 25);
        assert!(service.has_pending_ground_drop_projection_fenced(&runtime, Some(&context)));

        let retry_packets =
            service.reconcile_ground_drop_projections_fenced(&mut runtime, Some(&context));
        assert!(retry_packets.is_empty());
        assert_eq!(runtime.world_snapshot().gold, initial_gold + 25);
        assert!(!service.has_pending_ground_drop_projection_fenced(&runtime, Some(&context)));
    }

    #[test]
    fn gateway_ground_projection_save_failure_rolls_back_then_retries_once() {
        let config = crate::GatewayConfig::default();
        let mut runtime = start_test_runtime_with_config(
            config.clone(),
            "economy-ground-save-fail",
            "GroundSaveFail",
        )
        .expect("runtime");
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let identity = runtime.active_identity().expect("identity");
        let context = external_context(88, 908, 90_008);
        let command = claimed_gold_pickup(identity, 99_006, 25, "gateway-ground-save-failure");
        let event_id = economy_transaction_for_command(&context, &command, None)
            .expect("transaction")
            .event_id()
            .expect("event id");
        let before_world = runtime.world_snapshot();
        let before_checkpoint = serde_json::to_vec(
            &runtime
                .active_character_checkpoint()
                .expect("active checkpoint"),
        )
        .expect("encode checkpoint");

        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
        let first = service.commit_fenced(&mut runtime, Some(&context), command);

        assert!(first.committed);
        assert!(first.packets.is_empty());
        assert_eq!(runtime.world_snapshot(), before_world);
        assert_eq!(
            serde_json::to_vec(
                &runtime
                    .active_character_checkpoint()
                    .expect("rolled-back checkpoint"),
            )
            .expect("encode rolled-back checkpoint"),
            before_checkpoint,
        );
        assert!(!runtime.has_shared_economy_projection_event(&event_id));
        assert!(service.has_pending_ground_drop_projection_fenced(&runtime, Some(&context)));
        assert_eq!(store.transaction_count(), 1);

        let retry = service.reconcile_ground_drop_projections_fenced(&mut runtime, Some(&context));
        assert!(retry
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 25 })));
        assert_eq!(runtime.world_snapshot().gold, before_world.gold + 25);
        assert!(runtime.has_shared_economy_projection_event(&event_id));
        assert!(!service.has_pending_ground_drop_projection_fenced(&runtime, Some(&context)));
        assert_eq!(store.transaction_count(), 1);

        assert!(service
            .reconcile_ground_drop_projections_fenced(&mut runtime, Some(&context))
            .is_empty());
        assert_eq!(runtime.world_snapshot().gold, before_world.gold + 25);
        assert_eq!(store.transaction_count(), 1);
    }

    #[test]
    fn durable_projection_marker_prevents_replay_after_offsetting_transaction() {
        let store = Arc::new(FakeEconomyStore::default());
        let mut runtime = start_test_runtime("economy-marker", "MarkerA").expect("runtime");
        let identity = runtime.active_identity().expect("identity");
        let initial_gold = runtime.world_snapshot().gold;
        let claim = claimed_gold_pickup(identity.clone(), 9_005, 25, "drop-generation:11");
        let first_context = external_context(12, 17, 6_000);
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let first = service.commit_fenced(&mut runtime, Some(&first_context), claim.clone());
        assert!(first.committed);
        assert_eq!(runtime.world_snapshot().gold, initial_gold + 25);
        let first_transaction = economy_transaction_for_command(&first_context, &claim, None)
            .expect("first transaction");
        let first_event_id = first_transaction.event_id().unwrap();
        assert!(runtime.has_shared_economy_projection_event(&first_event_id));

        let debit = EconomyTransactionEnvelope {
            idempotency_key: "marker-offset-debit".to_string(),
            transaction_kind: EconomyTransactionKind::Consume,
            zone_id: "map:0".to_string(),
            fencing_generation: 12,
            source_sequence: 18,
            created_at_ms: 6_100,
            legs: vec![EconomyLeg {
                balance: EconomyBalanceKey::gold(
                    identity.account_id.clone(),
                    identity.character_index,
                ),
                delta: -25,
            }],
            metadata: BTreeMap::from([("operation".to_string(), "offsetDebit".to_string())]),
        };
        store.transact(&debit).expect("offset ledger debit");
        let debit_projection = runtime.commit_shared_gold_drop_transaction(25);
        assert!(debit_projection.committed);
        runtime
            .save_active_character()
            .expect("persist offset state");
        assert_eq!(runtime.world_snapshot().gold, initial_gold);

        let recovered_service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let retry = recovered_service.commit_fenced(
            &mut runtime,
            Some(&external_context(99, 9_999, 99_999)),
            claim,
        );
        assert!(retry.committed);
        assert!(retry.packets.is_empty());
        assert_eq!(runtime.world_snapshot().gold, initial_gold);
        assert_eq!(store.transaction_count(), 2);
    }

    #[test]
    fn corrupted_duplicate_receipt_fields_fail_closed() {
        let store = FakeEconomyStore::default();
        let runtime = start_test_runtime("economy-integrity", "IntegrityA").expect("runtime");
        let identity = runtime.active_identity().expect("identity");
        store
            .bootstrap_character(&identity, &runtime.world_snapshot(), 7_000)
            .expect("bootstrap");
        let command = claimed_gold_pickup(identity, 9_006, 25, "drop-generation:12");
        let transaction =
            economy_transaction_for_command(&external_context(13, 19, 7_000), &command, None)
                .expect("transaction");
        store.transact(&transaction).expect("first commit");
        let original = store
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();

        for corruption in 0..3 {
            {
                let mut state = store
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *state = original.clone();
                let (_, receipt) = state
                    .transactions
                    .get_mut(&transaction.idempotency_key)
                    .expect("stored transaction");
                match corruption {
                    0 => receipt.idempotency_key.push_str(":tampered"),
                    1 => receipt.transaction_kind = EconomyTransactionKind::Consume,
                    _ => {
                        *receipt
                            .balances_after
                            .values_mut()
                            .next()
                            .expect("stored balance") += 1;
                    }
                }
            }
            assert!(
                store.transact(&transaction).is_err(),
                "corruption {corruption}"
            );
        }
    }
    #[test]
    fn store_failure_before_commit_leaves_ledger_and_projection_unchanged() {
        let store = Arc::new(FakeEconomyStore::default());
        let mut runtime = start_test_runtime("economy-precommit", "PrecommitA").expect("runtime");
        let identity = runtime.active_identity().expect("active identity");
        let command = claimed_gold_pickup(identity.clone(), 9_002, 25, "drop-generation:8");
        let initial_gold = runtime.world_snapshot().gold;
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        store.fail_next_transact();
        let receipt =
            service.commit_fenced(&mut runtime, Some(&external_context(8, 13, 2_000)), command);

        assert!(!receipt.committed);
        assert_eq!(runtime.world_snapshot().gold, initial_gold);
        assert_eq!(
            store.balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            )),
            i64::from(initial_gold)
        );
        assert_eq!(store.transaction_count(), 0);
    }

    #[test]
    fn reused_idempotency_key_with_changed_payload_fails_closed() {
        let store = FakeEconomyStore::default();
        let runtime = start_test_runtime("economy-conflict", "ConflictA").expect("runtime");
        let identity = runtime.active_identity().expect("active identity");
        store
            .bootstrap_character(&identity, &runtime.world_snapshot(), 1_000)
            .expect("bootstrap");
        let command = claimed_gold_pickup(identity.clone(), 9_003, 25, "drop-generation:9");
        let context = external_context(9, 14, 3_000);
        let first =
            economy_transaction_for_command(&context, &command, None).expect("first transaction");
        store.transact(&first).expect("first commit");
        let ledger_after = store.balance(&EconomyBalanceKey::gold(
            &identity.account_id,
            identity.character_index,
        ));
        let mut conflict = first.clone();
        conflict.legs[0].delta = 26;
        assert!(store.transact(&conflict).is_err());
        assert_eq!(
            store.balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            )),
            ledger_after
        );
        assert_eq!(store.transaction_count(), 1);
    }
    #[test]
    fn durable_trade_rows_reconcile_each_party_after_ledger_commit_and_retry_mark() {
        use mir2_simulation::WorldCommand;

        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let mut alice = start_test_runtime("economy-trade-alice", "TradeAlice").unwrap();
        let mut bob = start_test_runtime("economy-trade-bob", "TradeBob").unwrap();
        let alice_identity = alice.active_identity().unwrap();
        let bob_identity = bob.active_identity().unwrap();
        let context = external_context(51, 8_001, 55_000);
        assert!(
            service
                .commit_fenced(
                    &mut alice,
                    Some(&context),
                    claimed_gold_pickup(alice_identity.clone(), 90_001, 25, "trade-fixture-gold"),
                )
                .committed
        );
        assert!(service.bootstrap_fenced(&alice, Some(&context)));
        assert!(service.bootstrap_fenced(&bob, Some(&context)));

        assert!(!alice.trade_request(&bob_identity.character_name).is_empty());
        assert!(!bob.trade_request(&alice_identity.character_name).is_empty());
        let deposited = alice
            .execute(WorldCommand::ClientPacket(ClientPacket::TradeGold {
                amount: 10,
            }))
            .unwrap();
        assert!(deposited
            .iter()
            .any(|packet| matches!(packet, ServerPacket::TradeGold { amount: 10 })));
        let (_, alice_offer) = alice.shared_trade_confirm();
        let (_, bob_offer) = bob.shared_trade_confirm();
        let alice_offer = alice_offer.expect("Alice trade offer");
        let bob_offer = bob_offer.expect("Bob trade offer");
        let transaction = economy_transaction_for_trade(&context, &alice_offer, &bob_offer)
            .unwrap()
            .expect("nonempty trade transaction");
        let event_id = transaction.event_id().unwrap();

        // Simulated crash after the atomic ledger/outbox/projection-intent
        // transaction, before either character receives its delivery.
        let receipt = store.transact(&transaction).unwrap();
        assert_eq!(receipt.event_id, event_id);
        assert!(service.has_pending_trade_projection_fenced(&alice, Some(&context)));
        assert!(service.has_pending_trade_projection_fenced(&bob, Some(&context)));

        let alice_packets = service.reconcile_trade_projections_fenced(&mut alice, Some(&context));
        assert!(alice_packets.is_empty());
        assert!(!service.has_pending_trade_projection_fenced(&alice, Some(&context)));

        // A database mark failure must preserve the pending row but cannot hide
        // the just-persisted client delivery. The following retry sees the
        // durable character marker and only marks the row projected.
        store.fail_next_trade_projection_mark();
        let bob_packets = service.reconcile_trade_projections_fenced(&mut bob, Some(&context));
        assert!(!bob_packets.is_empty());
        assert!(service.has_pending_trade_projection_fenced(&bob, Some(&context)));
        assert!(service
            .reconcile_trade_projections_fenced(&mut bob, Some(&context))
            .is_empty());
        assert!(!service.has_pending_trade_projection_fenced(&bob, Some(&context)));
    }

    #[test]
    fn pending_trade_projection_query_failure_fails_closed() {
        let store = Arc::new(FakeEconomyStore::default());
        let service = PostgresEconomyAccountInventoryService::with_backend(store.clone());
        let runtime = start_test_runtime("economy-trade-query", "TradeQuery").unwrap();
        let context = external_context(52, 8_002, 55_001);
        store.fail_pending_trade_projection_query();
        assert!(service.has_pending_trade_projection_fenced(&runtime, Some(&context)));
    }
    #[test]
    fn trade_requires_two_sided_asset_conservation() {
        let valid = EconomyTransactionEnvelope {
            idempotency_key: "trade:1".to_string(),
            transaction_kind: EconomyTransactionKind::Trade,
            zone_id: "map:0".to_string(),
            fencing_generation: 7,
            source_sequence: 11,
            created_at_ms: 1,
            legs: vec![leg("alice", "sword:7", -1), leg("bob", "sword:7", 1)],
            metadata: BTreeMap::new(),
        };
        valid.validate().unwrap();
        let mut invalid = valid;
        invalid.legs.pop();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn event_id_is_deterministic_and_binds_the_fence() {
        let mut envelope = EconomyTransactionEnvelope {
            idempotency_key: "reward:1".to_string(),
            transaction_kind: EconomyTransactionKind::Reward,
            zone_id: "map:0".to_string(),
            fencing_generation: 7,
            source_sequence: 12,
            created_at_ms: 1,
            legs: vec![leg("alice", "drop:1", 1)],
            metadata: BTreeMap::new(),
        };
        assert_eq!(envelope.event_id().unwrap(), envelope.event_id().unwrap());
        let first = envelope.event_id().unwrap();
        envelope.fencing_generation += 1;
        assert_ne!(first, envelope.event_id().unwrap());
    }

    #[test]
    fn zone_gold_pickup_binds_business_key_fence_and_source_sequence() {
        let context = SharedAccountInventoryExecutionContext {
            zone_id: crate::ZoneId::new("map:0"),
            fencing_generation: 9,
            source_sequence: 42,
            created_at_ms: 77,
            external_commit_authorized: true,
        };
        let command = SharedAccountInventoryCommandEnvelope {
            identity: ActiveSessionIdentity {
                account_id: "alice".to_string(),
                character_index: 3,
                character_name: "Blade".to_string(),
            },
            command: SharedAccountInventoryCommand::GroundDropPickup(GroundDropSnapshot {
                object_id: 9001,
                name: "25 Gold".to_string(),
                name_colour_argb: -1,
                icon: 0,
                x: 10,
                y: 20,
                quantity: 25,
                source_monster: "Field Wasp".to_string(),
                owner_object_id: None,
                ownership_remaining_ticks: None,
                loot: GroundDropLootSnapshot::Gold { amount: 25 },
            }),
        };
        let envelope =
            economy_transaction_for_command(&context, &command, None).expect("gold transaction");

        assert_eq!(envelope.zone_id, "map:0");
        assert_eq!(envelope.fencing_generation, 9);
        assert_eq!(envelope.source_sequence, 42);
        assert_eq!(envelope.created_at_ms, 77);
        assert!(envelope
            .idempotency_key
            .starts_with("zone:map:0:alice:3:ground-drop-pickup:9001:"));
        assert_eq!(
            envelope.idempotency_key.len(),
            "zone:map:0:alice:3:ground-drop-pickup:9001:".len() + 64
        );
        let mut changed_command = command.clone();
        let SharedAccountInventoryCommand::GroundDropPickup(drop) = &mut changed_command.command
        else {
            unreachable!("ground drop command")
        };
        drop.loot = GroundDropLootSnapshot::Gold { amount: 26 };
        assert_ne!(
            command.stable_idempotency_key(),
            changed_command.stable_idempotency_key()
        );
        assert_eq!(envelope.legs.len(), 1);
        assert_eq!(
            envelope.legs[0].balance,
            EconomyBalanceKey::gold("alice", 3)
        );
        assert_eq!(envelope.legs[0].delta, 25);
        envelope.validate().expect("valid economy envelope");
    }

    #[test]
    fn player_gold_and_item_drops_map_to_fenced_debits() {
        let context = SharedAccountInventoryExecutionContext {
            zone_id: crate::ZoneId::new("map:0"),
            fencing_generation: 9,
            source_sequence: 44,
            created_at_ms: 79,
            external_commit_authorized: true,
        };
        let identity = ActiveSessionIdentity {
            account_id: "alice".to_string(),
            character_index: 3,
            character_name: "Blade".to_string(),
        };
        let gold = economy_transaction_for_command(
            &context,
            &SharedAccountInventoryCommandEnvelope {
                identity: identity.clone(),
                command: SharedAccountInventoryCommand::GoldDrop {
                    amount: 25,
                    request_id: 44,
                },
            },
            None,
        )
        .expect("gold drop transaction");
        assert_eq!(gold.transaction_kind, EconomyTransactionKind::Consume);
        assert_eq!(gold.source_sequence, 44);
        assert_eq!(gold.legs[0].balance, EconomyBalanceKey::gold("alice", 3));
        assert_eq!(gold.legs[0].delta, -25);
        gold.validate().expect("gold drop transaction is valid");

        let item = economy_transaction_for_command(
            &context,
            &SharedAccountInventoryCommandEnvelope {
                identity,
                command: SharedAccountInventoryCommand::InventoryItemDrop {
                    drop: mir2_simulation::SharedInventoryItemDrop {
                        item_key: "iron-ore".to_string(),
                        unique_id: 77,
                        quantity: 2,
                        hero_inventory: false,
                    },
                    request_id: 44,
                },
            },
            None,
        )
        .expect("item drop transaction");
        assert_eq!(item.transaction_kind, EconomyTransactionKind::Consume);
        assert_eq!(
            item.legs[0].balance,
            EconomyBalanceKey::item_quantity("alice", 3, "iron-ore")
        );
        assert_eq!(item.legs[0].delta, -2);
        item.validate().expect("item drop transaction is valid");
    }

    #[test]
    fn standby_context_cannot_authorize_an_external_commit() {
        let context = SharedAccountInventoryExecutionContext {
            zone_id: crate::ZoneId::new("map:0"),
            fencing_generation: 9,
            source_sequence: 42,
            created_at_ms: 77,
            external_commit_authorized: false,
        };
        assert!(!context.external_commit_authorized);
    }

    #[test]
    fn skill_consumption_maps_exact_runtime_components_to_negative_ledger_legs() {
        let context = SharedAccountInventoryExecutionContext {
            zone_id: crate::ZoneId::new("map:0"),
            fencing_generation: 10,
            source_sequence: 43,
            created_at_ms: 78,
            external_commit_authorized: true,
        };
        let command = SharedAccountInventoryCommandEnvelope {
            identity: ActiveSessionIdentity {
                account_id: "taoist".to_string(),
                character_index: 2,
                character_name: "Sage".to_string(),
            },
            command: SharedAccountInventoryCommand::SkillItemConsume {
                spell: mir2_protocol::Spell::PoisonCloud,
                request_id: 7,
                components: vec![
                    mir2_simulation::SharedSkillItemConsumptionComponent {
                        item_key: "amulet".to_string(),
                        quantity: 5,
                    },
                    mir2_simulation::SharedSkillItemConsumptionComponent {
                        item_key: "green-poison".to_string(),
                        quantity: 5,
                    },
                ],
            },
        };
        let envelope =
            economy_transaction_for_command(&context, &command, None).expect("skill transaction");

        assert_eq!(envelope.transaction_kind, EconomyTransactionKind::Consume);
        assert_eq!(
            envelope.idempotency_key,
            format!(
                "zone:map:0:taoist:2:skill-item-consume:{}:7",
                mir2_protocol::Spell::PoisonCloud as u8
            )
        );
        assert_eq!(envelope.legs.len(), 2);
        assert_eq!(
            envelope.legs[0].balance,
            EconomyBalanceKey::item_quantity("taoist", 2, "amulet")
        );
        assert_eq!(envelope.legs[0].delta, -5);
        assert_eq!(
            envelope.legs[1].balance,
            EconomyBalanceKey::item_quantity("taoist", 2, "green-poison")
        );
        assert_eq!(envelope.legs[1].delta, -5);
        envelope
            .validate()
            .expect("skill consumption envelope must be valid");
    }

    #[test]
    fn player_trade_maps_gold_and_item_quantities_to_conserved_ledger_legs() {
        let context = SharedAccountInventoryExecutionContext {
            zone_id: crate::ZoneId::new("map:0"),
            fencing_generation: 12,
            source_sequence: 99,
            created_at_ms: 123,
            external_commit_authorized: true,
        };
        let alice = SharedTradeOffer {
            settlement_nonce: "00000000000000000000000000000001".to_string(),
            account_id: "alice".to_string(),
            character_index: 0,
            character_name: "Alice".to_string(),
            partner_name: "Bob".to_string(),
            gold: 30,
            items: vec![mir2_simulation::SharedTradeOfferItem {
                item_state_json: r#"{"quantity":2}"#.to_string(),
                key: "iron-ore".to_string(),
                unique_id: 7,
            }],
        };
        let bob = SharedTradeOffer {
            settlement_nonce: "00000000000000000000000000000002".to_string(),
            account_id: "bob".to_string(),
            character_index: 1,
            character_name: "Bob".to_string(),
            partner_name: "Alice".to_string(),
            gold: 10,
            items: Vec::new(),
        };
        let transaction = economy_transaction_for_trade(&context, &alice, &bob)
            .expect("trade should map")
            .expect("non-empty trade should create a transaction");

        assert_eq!(transaction.transaction_kind, EconomyTransactionKind::Trade);
        assert_eq!(transaction.source_sequence, 99);
        assert_eq!(transaction.legs.len(), 6);
        transaction.validate().expect("trade must conserve assets");
        let aggregated = aggregate_legs(&transaction.legs).expect("trade legs aggregate");
        assert_eq!(aggregated[&EconomyBalanceKey::gold("alice", 0)], -20);
        assert_eq!(aggregated[&EconomyBalanceKey::gold("bob", 1)], 20);
        assert_eq!(
            aggregated[&EconomyBalanceKey::item_quantity("alice", 0, "iron-ore")],
            -2
        );
        assert_eq!(
            aggregated[&EconomyBalanceKey::item_quantity("bob", 1, "iron-ore")],
            2
        );

        let changed_context = SharedAccountInventoryExecutionContext {
            fencing_generation: 88,
            source_sequence: 9_999,
            created_at_ms: 88_888,
            ..context.clone()
        };
        let retry = economy_transaction_for_trade(&changed_context, &alice, &bob)
            .expect("retry should map")
            .expect("retry remains non-empty");
        assert_eq!(transaction.idempotency_key, retry.idempotency_key);
        assert_eq!(
            transaction.business_effect_id().unwrap(),
            retry.business_effect_id().unwrap()
        );
        assert_ne!(transaction.event_id().unwrap(), retry.event_id().unwrap());

        let mut later_alice = alice.clone();
        later_alice.settlement_nonce = "00000000000000000000000000000003".to_string();
        let later_trade = economy_transaction_for_trade(&changed_context, &later_alice, &bob)
            .expect("later trade should map")
            .expect("later trade remains non-empty");
        assert_ne!(transaction.idempotency_key, later_trade.idempotency_key);
    }
}

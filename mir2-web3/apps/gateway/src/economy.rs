//! Gate 17 durable game-economy transaction, outbox, inbox, and reconciliation.
//!
//! This module is intentionally separate from the administrative event bus.
//! Gold, unique items, consumptions, and two-sided trades are authoritative
//! gameplay effects: balance writes, the idempotency receipt, and the outbox
//! event are committed in one PostgreSQL transaction.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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
    SharedAccountInventoryExecutionContext, SharedAccountInventoryService,
    SharedTradeSettlementOutcome,
};

const ECONOMY_EVENT_DOMAIN: &[u8] = b"obelisk.mir2.game-economy-event.v1\0";
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

    pub fn transact(
        &self,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<EconomyTransactionReceipt, String> {
        envelope.validate()?;
        let event_id = envelope.event_id()?;
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
                "SELECT receipt FROM game_economy_transactions WHERE idempotency_key = $1",
                &[&envelope.idempotency_key],
            )
            .map_err(|error| format!("economy idempotency lookup failed: {error}"))?
        {
            let mut receipt: EconomyTransactionReceipt = serde_json::from_value(row.get("receipt"))
                .map_err(|error| format!("decode stored economy receipt: {error}"))?;
            if receipt.event_id != event_id {
                return Err(format!(
                    "economy idempotency conflict for {}",
                    envelope.idempotency_key
                ));
            }
            receipt.duplicate = true;
            return Ok(receipt);
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
        };
        let receipt_json = serde_json::to_value(&receipt)
            .map_err(|error| format!("encode economy receipt: {error}"))?;
        let event = EconomyOutboxEvent {
            event_id: event_id.clone(),
            idempotency_key: envelope.idempotency_key.clone(),
            envelope: envelope.clone(),
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
        transaction
            .commit()
            .map_err(|error| format!("economy transaction commit failed: {error}"))?;
        Ok(receipt)
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

/// Gate 18 bridge from real Mir2 Zone rewards/pickups to the Gate 17 ledger.
///
/// The active owner commits PostgreSQL before mutating its private character
/// projection. A verified standby replay never writes PostgreSQL; it only
/// rebuilds the same private projection. A duplicate authoritative transaction
/// without a local cached projection is treated as already materialized (the
/// base snapshot or outbox projector owns recovery), so a post-promotion retry
/// cannot credit the character twice.
#[derive(Debug)]
pub struct PostgresEconomyAccountInventoryService {
    store: PostgresEconomyStore,
    projected_receipts: Mutex<BTreeMap<String, SharedAccountInventoryTransactionReceipt>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectionRecoveryState {
    Materialized,
    NeedsReplay,
    Diverged,
}

impl PostgresEconomyAccountInventoryService {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self::with_store(PostgresEconomyStore::new(database_url))
    }

    pub fn with_store(store: PostgresEconomyStore) -> Self {
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
            SharedAccountInventoryCommand::GroundDropPickup(drop) => {
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

    fn projection_recovery_state(
        &self,
        runtime: &InProcessWorldRuntime,
        transaction: &EconomyTransactionEnvelope,
    ) -> Result<ProjectionRecoveryState, String> {
        let identity = runtime
            .active_identity()
            .ok_or_else(|| "economy projection recovery requires an active identity".to_string())?;
        let snapshot = runtime.world_snapshot();
        let deltas = aggregate_legs(&transaction.legs)?;
        let mut all_materialized = true;
        let mut all_need_replay = true;
        for (balance, delta) in deltas {
            if balance.account_id != identity.account_id
                || balance.character_index != identity.character_index
            {
                return Ok(ProjectionRecoveryState::Diverged);
            }
            let ledger_after = self.store.balance(&balance)?;
            let runtime_amount = runtime_balance_amount(&snapshot, &balance)?;
            all_materialized &= runtime_amount == ledger_after;
            all_need_replay &= runtime_amount
                .checked_add(delta)
                .is_some_and(|after| after == ledger_after);
        }
        match (all_materialized, all_need_replay) {
            (true, _) => Ok(ProjectionRecoveryState::Materialized),
            (false, true) => Ok(ProjectionRecoveryState::NeedsReplay),
            (false, false) => Ok(ProjectionRecoveryState::Diverged),
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
    ) -> SharedAccountInventoryTransactionReceipt {
        if runtime.active_identity().as_ref() != Some(&envelope.identity) {
            return Self::failed_receipt(&envelope.command);
        }
        let Some(context) = context else {
            return Self::failed_receipt(&envelope.command);
        };
        if !self.bootstrap_fenced(runtime, Some(context)) {
            return Self::failed_receipt(&envelope.command);
        }
        let stable_key = envelope.stable_idempotency_key();
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

        if !preflight_projection(runtime, &envelope.command) {
            return Self::failed_receipt(&envelope.command);
        }
        let Some(transaction) = economy_transaction_for_command(context, &envelope) else {
            // Commands without an external asset delta remain deterministic
            // Zone-only effects. Skill-item consumption is deliberately
            // rejected until its exact inventory component IDs are included.
            return match &envelope.command {
                SharedAccountInventoryCommand::MonsterKillAward(award) if award.experience == 0 => {
                    Self::apply_projection(runtime, &envelope)
                }
                _ => Self::failed_receipt(&envelope.command),
            };
        };
        let transaction_receipt = match self.store.transact(&transaction) {
            Ok(receipt) => receipt,
            Err(_) => return Self::failed_receipt(&envelope.command),
        };
        if transaction_receipt.duplicate {
            // A new Host cannot infer projection state from its empty
            // process-local receipt cache. Compare the restored character
            // balances with PostgreSQL: equal means the checkpoint already
            // contains the effect; exactly one transaction delta behind means
            // the crash happened before projection; every other state is a
            // split-brain/divergence and fails closed.
            let receipt = match self.projection_recovery_state(runtime, &transaction) {
                Ok(ProjectionRecoveryState::Materialized) => {
                    Self::already_materialized_receipt(&envelope.command)
                }
                Ok(ProjectionRecoveryState::NeedsReplay) => {
                    Self::apply_projection(runtime, &envelope)
                }
                Ok(ProjectionRecoveryState::Diverged) | Err(_) => {
                    return Self::failed_receipt(&envelope.command);
                }
            };
            if receipt.committed {
                self.projected_receipts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(stable_key, receipt.clone());
            }
            return receipt;
        }

        let receipt = Self::apply_projection(runtime, &envelope);
        if receipt.committed {
            self.projected_receipts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(stable_key, receipt.clone());
        }
        receipt
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

    fn settle_trade_fenced(
        &self,
        context: Option<&SharedAccountInventoryExecutionContext>,
        first: &SharedTradeOffer,
        second: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        let Some(context) = context else {
            return SharedTradeSettlementOutcome::Rejected;
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
            Ok(receipt) if receipt.duplicate => SharedTradeSettlementOutcome::Duplicate,
            Ok(_) => SharedTradeSettlementOutcome::Committed,
            Err(_) => SharedTradeSettlementOutcome::Rejected,
        }
    }
}

fn runtime_balance_amount(
    snapshot: &WorldSnapshot,
    balance: &EconomyBalanceKey,
) -> Result<i64, String> {
    match balance.asset_kind.as_str() {
        "gold" if balance.asset_key == "gold" => Ok(i64::from(snapshot.gold)),
        "experience" if balance.asset_key == "experience" => Ok(snapshot.player_experience),
        "item_quantity" => snapshot
            .inventory_items
            .iter()
            .chain(snapshot.belt_items.iter())
            .chain(snapshot.storage_items.iter())
            .chain(snapshot.hero_inventory_items.iter())
            .filter(|item| item.key == balance.asset_key)
            .try_fold(0_i64, |total, item| {
                total
                    .checked_add(i64::from(item.quantity))
                    .ok_or_else(|| "runtime item quantity overflow".to_string())
            })
            .and_then(|total| {
                snapshot
                    .equipment_items
                    .iter()
                    .filter(|item| item.key == balance.asset_key)
                    .try_fold(total, |total, item| {
                        total
                            .checked_add(i64::from(item.quantity))
                            .ok_or_else(|| "runtime equipment quantity overflow".to_string())
                    })
            }),
        kind => Err(format!(
            "unsupported runtime economy balance {kind}/{}",
            balance.asset_key
        )),
    }
}

fn command_kind(command: &SharedAccountInventoryCommand) -> SharedAccountInventoryTransactionKind {
    match command {
        SharedAccountInventoryCommand::GroundDropPickup(_) => {
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
        SharedAccountInventoryCommand::GroundDropPickup(drop) => {
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
) -> Option<EconomyTransactionEnvelope> {
    let identity = &command.identity;
    let stable_key = command.stable_idempotency_key();
    let mut metadata = BTreeMap::from([
        ("producer".to_string(), "mir2-zone".to_string()),
        ("characterName".to_string(), identity.character_name.clone()),
    ]);
    let (transaction_kind, legs) = match &command.command {
        SharedAccountInventoryCommand::GroundDropPickup(drop) => match &drop.loot {
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
            (
                EconomyTransactionKind::Reward,
                vec![EconomyLeg {
                    balance: EconomyBalanceKey::experience(
                        identity.account_id.clone(),
                        identity.character_index,
                    ),
                    delta: i64::from(award.experience),
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
        SharedAccountInventoryCommand::MonsterKillAward(_)
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

    let mut legs = Vec::new();
    append_trade_offer_legs(first, second, &mut legs)?;
    append_trade_offer_legs(second, first, &mut legs)?;
    if legs.is_empty() {
        return Ok(None);
    }
    let business_digest = trade_business_digest(first, second)?;
    let metadata = BTreeMap::from([
        ("operation".to_string(), "playerTrade".to_string()),
        ("producer".to_string(), "mir2-zone".to_string()),
        ("tradeDigest".to_string(), business_digest.clone()),
        (
            "participants".to_string(),
            format!(
                "{}/{}|{}/{}",
                first.account_id, first.character_index, second.account_id, second.character_index
            ),
        ),
    ]);
    Ok(Some(EconomyTransactionEnvelope {
        idempotency_key: format!(
            "zone:{}:trade:{}:{business_digest}",
            context.zone_id, context.source_sequence
        ),
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
    use mir2_simulation::{ActiveSessionIdentity, GroundDropSnapshot};

    fn leg(account: &str, asset: &str, delta: i64) -> EconomyLeg {
        EconomyLeg {
            balance: EconomyBalanceKey::item(account, 0, asset),
            delta,
        }
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
            economy_transaction_for_command(&context, &command).expect("gold transaction");

        assert_eq!(envelope.zone_id, "map:0");
        assert_eq!(envelope.fencing_generation, 9);
        assert_eq!(envelope.source_sequence, 42);
        assert_eq!(envelope.created_at_ms, 77);
        assert_eq!(
            envelope.idempotency_key,
            "zone:map:0:alice:3:ground-drop-pickup:9001"
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
            economy_transaction_for_command(&context, &command).expect("skill transaction");

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
    }
}

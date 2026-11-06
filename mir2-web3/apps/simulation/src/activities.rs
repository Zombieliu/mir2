//! Account-owned campaign reservations. These are not delivered game items.
//! Qualification is observed from committed saves and survives character deletion.
//! Only a later, separately authorized public-launch settlement may mint rewards.

use std::collections::{BTreeMap, BTreeSet};

use mir2_protocol::{MirClass, MirGender};
use serde::{Deserialize, Serialize};

use crate::billing::RechargeOrderState;
use crate::{AccountRecord, CharacterSaveRecord, SimulationConfig};

const MAX_RECHARGE_RECORDS: usize = 4_096;
const MAX_PUBLIC_RECHARGE_RECORDS: usize = 50;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityPhase {
    #[default]
    Disabled,
    Closed,
    Beta,
    Public,
}

impl ActivityPhase {
    fn key(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Closed => "closed-v1",
            Self::Beta => "beta-v1",
            Self::Public => "public-v1",
        }
    }
    fn rank(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Closed => 1,
            Self::Beta => 2,
            Self::Public => 3,
        }
    }
}

/// Trusted operator configuration, never accepted in a player request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityPolicy {
    pub phase: ActivityPhase,
    pub phase_start_ms: u64,
    pub phase_end_ms: Option<u64>,
}

impl ActivityPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.phase == ActivityPhase::Disabled {
            if self.phase_start_ms != 0 || self.phase_end_ms.is_some() {
                return Err("activityPolicyInvalid".into());
            }
        } else if self.phase_start_ms == 0
            || self
                .phase_end_ms
                .is_some_and(|end| end <= self.phase_start_ms)
        {
            return Err("activityPolicyInvalid".into());
        }
        Ok(())
    }

    pub fn from_env() -> Result<Self, String> {
        fn number(key: &str) -> Result<Option<u64>, String> {
            match std::env::var(key) {
                Err(std::env::VarError::NotPresent) => Ok(None),
                Ok(value) if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                    value
                        .parse()
                        .map(Some)
                        .map_err(|_| "activityPolicyInvalid".into())
                }
                _ => Err("activityPolicyInvalid".into()),
            }
        }
        let phase = match std::env::var("MIR2_ACTIVITY_PHASE").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("disabled") => ActivityPhase::Disabled,
            Ok("closed") => ActivityPhase::Closed,
            Ok("beta") => ActivityPhase::Beta,
            Ok("public") => ActivityPhase::Public,
            _ => return Err("activityPolicyInvalid".into()),
        };
        let policy = Self {
            phase,
            phase_start_ms: number("MIR2_ACTIVITY_PHASE_START_MS")?.unwrap_or(0),
            phase_end_ms: number("MIR2_ACTIVITY_PHASE_END_MS")?,
        };
        policy.validate()?;
        Ok(policy)
    }

    fn active(&self, now_ms: u64) -> bool {
        self.phase != ActivityPhase::Disabled
            && now_ms >= self.phase_start_ms
            && self.phase_end_ms.is_none_or(|end| now_ms < end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityRewardKind {
    MonthlyDays,
    Title,
    Cosmetic,
}

#[derive(Debug, Clone, Copy)]
struct RewardDefinition {
    id: &'static str,
    phase: ActivityPhase,
    kind: ActivityRewardKind,
    level: u16,
    amount: u16,
}

const REWARDS: [RewardDefinition; 5] = [
    RewardDefinition {
        id: "closed-v1-lv30",
        phase: ActivityPhase::Closed,
        kind: ActivityRewardKind::MonthlyDays,
        level: 30,
        amount: 30,
    },
    RewardDefinition {
        id: "closed-v1-lv40",
        phase: ActivityPhase::Closed,
        kind: ActivityRewardKind::Title,
        level: 40,
        amount: 1,
    },
    RewardDefinition {
        id: "closed-v1-lv50",
        phase: ActivityPhase::Closed,
        kind: ActivityRewardKind::Cosmetic,
        level: 50,
        amount: 1,
    },
    RewardDefinition {
        id: "beta-v1-lv40",
        phase: ActivityPhase::Beta,
        kind: ActivityRewardKind::Title,
        level: 40,
        amount: 1,
    },
    RewardDefinition {
        id: "beta-v1-lv50",
        phase: ActivityPhase::Beta,
        kind: ActivityRewardKind::Cosmetic,
        level: 50,
        amount: 1,
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityPhaseRecord {
    pub phase: ActivityPhase,
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub max_level: u16,
    pub qualified_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityClaimRecord {
    pub reward_id: String,
    pub phase: ActivityPhase,
    pub kind: ActivityRewardKind,
    pub amount: u16,
    pub qualified_level: u16,
    pub character_index: i32,
    pub character_name: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub claimed_at_ms: u64,
}

/// Frozen, genuine provider payment evidence; no browser values reach this row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityRechargeRecord {
    pub order_id: String,
    pub character_index: i32,
    pub amount_usd_minor: u64,
    pub credits: u32,
    pub created_at_ms: u64,
    pub paid_at_ms: u64,
    pub recorded_at_ms: u64,
    pub checkout_session_id: String,
    pub payment_intent_id: String,
    #[serde(default)]
    pub review_events: BTreeSet<String>,
    /// Existing wallets do not identify whether spent Credits were live or test.
    /// Applied funds therefore need reconciliation before any launch settlement.
    pub reconciliation_required: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityLedger {
    #[serde(default)]
    pub phases: BTreeMap<String, ActivityPhaseRecord>,
    #[serde(default)]
    pub claims: BTreeMap<String, ActivityClaimRecord>,
    #[serde(default)]
    pub recharge_records: BTreeMap<String, ActivityRechargeRecord>,
}

impl ActivityLedger {
    pub fn validate(&self) -> Result<(), String> {
        if self.phases.len() > 3
            || self.claims.len() > REWARDS.len()
            || self.recharge_records.len() > MAX_RECHARGE_RECORDS
        {
            return Err("activityLedgerInvalid".into());
        }
        let mut phases = self.phases.values().collect::<Vec<_>>();
        phases.sort_by_key(|phase| phase.phase.rank());
        for (index, record) in phases.iter().enumerate() {
            if record.phase == ActivityPhase::Disabled
                || self.phases.get(record.phase.key()) != Some(*record)
                || record.start_ms == 0
                || record.end_ms.is_some_and(|end| end <= record.start_ms)
                || record.qualified_at_ms < record.start_ms
                || record
                    .end_ms
                    .is_some_and(|end| record.qualified_at_ms >= end)
                || index > 0
                    && phases[index - 1]
                        .end_ms
                        .is_none_or(|end| end > record.start_ms)
            {
                return Err("activityLedgerInvalid".into());
            }
        }
        for (id, claim) in &self.claims {
            let definition = REWARDS
                .iter()
                .find(|reward| reward.id == id)
                .ok_or("activityLedgerInvalid")?;
            let phase = self
                .phases
                .get(definition.phase.key())
                .ok_or("activityLedgerInvalid")?;
            if claim.reward_id != *id
                || claim.phase != definition.phase
                || claim.kind != definition.kind
                || claim.amount != definition.amount
                || claim.qualified_level < definition.level
                || claim.qualified_level > phase.max_level
                || claim.character_index < 0
                || claim.character_name.is_empty()
                || claim.character_name.len() > 256
                || claim.character_name.chars().any(char::is_control)
                || claim.claimed_at_ms < phase.start_ms
            {
                return Err("activityLedgerInvalid".into());
            }
        }
        let mut sessions = BTreeSet::new();
        let mut payments = BTreeSet::new();
        for (id, record) in &self.recharge_records {
            let phase = self
                .phases
                .get(ActivityPhase::Beta.key())
                .ok_or("activityLedgerInvalid")?;
            if id != &record.order_id
                || !valid_order_id(id)
                || record.character_index < 0
                || record.amount_usd_minor == 0
                || record.credits == 0
                || record.created_at_ms < phase.start_ms
                || record.paid_at_ms < record.created_at_ms
                || phase.end_ms.is_some_and(|end| record.paid_at_ms >= end)
                || record.recorded_at_ms < record.paid_at_ms
                || !valid_provider_id(&record.checkout_session_id, "cs_")
                || !valid_provider_id(&record.payment_intent_id, "pi_")
                || !sessions.insert(&record.checkout_session_id)
                || !payments.insert(&record.payment_intent_id)
                || record.review_events.len() > 128
                || record
                    .review_events
                    .iter()
                    .any(|id| !valid_provider_id(id, "evt_"))
            {
                return Err("activityLedgerInvalid".into());
            }
        }
        Ok(())
    }
}

fn valid_order_id(id: &str) -> bool {
    id.len() == 68
        && id.starts_with("rch_")
        && id[4..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn valid_provider_id(id: &str, prefix: &str) -> bool {
    id.starts_with(prefix)
        && id.len() > prefix.len()
        && id.len() <= 255
        && !id.contains("_secret_")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}

fn owned_save(account: &AccountRecord, index: i32) -> Result<&CharacterSaveRecord, String> {
    let character = account
        .characters
        .iter()
        .find(|character| character.index == index)
        .ok_or("activityCharacterMissing")?;
    let save = account
        .saves
        .get(&index)
        .ok_or("activityCharacterMissing")?;
    if character.index != save.character.index
        || character.name != save.character.name
        || character.class != save.character.class
        || character.gender != save.character.gender
    {
        return Err("activityCharacterIdentityMismatch".into());
    }
    Ok(save)
}

fn account_characters(account: &AccountRecord) -> Result<Vec<ActivityCharacter>, String> {
    let mut indices = BTreeSet::new();
    account
        .characters
        .iter()
        .map(|character| {
            if !indices.insert(character.index) {
                return Err("activityCharacterIdentityMismatch".into());
            }
            let character = &owned_save(account, character.index)?.character;
            Ok(ActivityCharacter {
                index: character.index,
                name: character.name.clone(),
                class: character.class,
                gender: character.gender,
                level: character.level,
            })
        })
        .collect()
}

/// Called inside the existing account publication after Source changes. No nested
/// save is allowed: qualification and the committed character must share one CAS.
pub fn observe_account(
    account: &mut AccountRecord,
    policy: &ActivityPolicy,
    now_ms: u64,
) -> Result<(), String> {
    policy.validate()?;
    if let Some(ledger) = &account.activities {
        ledger.validate()?;
    }
    let active = policy.active(now_ms);
    if !active && account.activities.is_none() {
        return Ok(());
    }
    // Disabled campaigns do not add roster/save requirements to ordinary
    // legacy gameplay. Existing payment reviews can still be retained without
    // consulting a character that may have been deleted since the campaign.
    let characters = if active {
        account_characters(account)?
    } else {
        Vec::new()
    };
    let mut ledger = account.activities.clone().unwrap_or_default();
    if active {
        // Campaign windows are immutable. Reusing a phase key with a new epoch
        // would let future characters acquire already-sealed test rewards.
        if let Some(existing) = ledger.phases.get(policy.phase.key()) {
            if existing.start_ms != policy.phase_start_ms || existing.end_ms != policy.phase_end_ms
            {
                return Err("activityPhaseSealed".into());
            }
        } else {
            for old in ledger.phases.values_mut() {
                if old.phase.rank() >= policy.phase.rank()
                    || old.qualified_at_ms >= policy.phase_start_ms
                {
                    return Err("activityPhaseSealed".into());
                }
                old.end_ms = Some(
                    old.end_ms
                        .map_or(policy.phase_start_ms, |end| end.min(policy.phase_start_ms)),
                );
            }
            ledger.phases.insert(
                policy.phase.key().into(),
                ActivityPhaseRecord {
                    phase: policy.phase,
                    start_ms: policy.phase_start_ms,
                    end_ms: policy.phase_end_ms,
                    max_level: 0,
                    qualified_at_ms: policy.phase_start_ms,
                },
            );
        }
        let max_level = characters
            .iter()
            .map(|character| character.level)
            .max()
            .unwrap_or(0);
        let phase = ledger
            .phases
            .get_mut(policy.phase.key())
            .expect("phase inserted");
        if max_level > phase.max_level {
            phase.max_level = max_level;
            phase.qualified_at_ms = now_ms;
        }
    }
    if let Some(billing) = &account.billing {
        billing.validate()?;
        // Reviews are sticky even if the payment's original phase has ended.
        for record in ledger.recharge_records.values_mut() {
            if let Some(order) = billing.orders.get(&record.order_id) {
                if !order.livemode
                    || order.offer.currency != "usd"
                    || order.character_index != record.character_index
                    || order.offer.amount_minor != record.amount_usd_minor
                    || order.offer.credits != record.credits
                    || order.created_at_ms != record.created_at_ms
                    || order.paid_at_ms != Some(record.paid_at_ms)
                    || order.checkout_session_id.as_deref()
                        != Some(record.checkout_session_id.as_str())
                    || order.payment_intent_id.as_deref() != Some(record.payment_intent_id.as_str())
                {
                    return Err("activityPaymentEvidenceChanged".into());
                }
                record
                    .review_events
                    .extend(order.review_events.keys().cloned());
                record.reconciliation_required |=
                    order.applied.is_some() || order.state == RechargeOrderState::Review;
            } else {
                // Historical proof survives a character-data wipe, but an
                // absent provider ledger cannot process later refunds safely.
                record.reconciliation_required = true;
            }
        }
        // Closed-window payments may become visible after the phase ends (for
        // example on another Gateway's refresh). Their durable beta window,
        // rather than today's enabled phase, determines the recorded entitlement.
        if let Some(beta) = ledger.phases.get(ActivityPhase::Beta.key()) {
            for order in billing.orders.values() {
                let Some(paid_at_ms) = order.paid_at_ms else {
                    continue;
                };
                if !order.livemode
                    || order.offer.currency != "usd"
                    || order.created_at_ms < beta.start_ms
                    || paid_at_ms < beta.start_ms
                    || paid_at_ms > now_ms
                    || beta.end_ms.is_some_and(|end| paid_at_ms >= end)
                    || !matches!(
                        order.state,
                        RechargeOrderState::PaidPending
                            | RechargeOrderState::Applied
                            | RechargeOrderState::Review
                    )
                {
                    continue;
                }
                let Some(session) = &order.checkout_session_id else {
                    continue;
                };
                let Some(payment) = &order.payment_intent_id else {
                    continue;
                };
                let record = ActivityRechargeRecord {
                    order_id: order.id.clone(),
                    character_index: order.character_index,
                    amount_usd_minor: order.offer.amount_minor,
                    credits: order.offer.credits,
                    created_at_ms: order.created_at_ms,
                    paid_at_ms,
                    recorded_at_ms: now_ms,
                    checkout_session_id: session.clone(),
                    payment_intent_id: payment.clone(),
                    review_events: order.review_events.keys().cloned().collect(),
                    reconciliation_required: order.applied.is_some()
                        || order.state == RechargeOrderState::Review,
                };
                ledger
                    .recharge_records
                    .entry(order.id.clone())
                    .or_insert(record);
            }
        }
    } else if !ledger.recharge_records.is_empty() {
        for record in ledger.recharge_records.values_mut() {
            record.reconciliation_required = true;
        }
    }
    ledger.validate()?;
    account.activities = Some(ledger);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCharacter {
    pub index: i32,
    pub name: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRewardStatus {
    pub id: String,
    pub phase: ActivityPhase,
    pub kind: ActivityRewardKind,
    pub required_level: u16,
    pub amount: u16,
    pub qualified_level: u16,
    pub eligible: bool,
    pub claimed: bool,
    pub claimed_at_ms: Option<u64>,
    pub cosmetic_class: Option<MirClass>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivitySettlement {
    Recorded,
    ReconciliationRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRechargeStatus {
    pub order_id: String,
    pub amount_usd_minor: u64,
    pub credits: u32,
    pub recorded_at_ms: u64,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCarryoverStatus {
    /// Historical verified base entitlement, not a newly credited wallet.
    pub real_verified_usd_minor: u64,
    pub verified_credits: u64,
    pub monthly_days: u32,
    pub test_orders_excluded: u32,
    pub review_orders: u32,
    pub records: Vec<ActivityRechargeStatus>,
    pub records_total: usize,
    pub settlement: ActivitySettlement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityStatus {
    pub schema_version: u16,
    pub phase: ActivityPhase,
    pub phase_start_ms: u64,
    pub phase_end_ms: Option<u64>,
    pub characters: Vec<ActivityCharacter>,
    pub max_level: u16,
    pub rewards: Vec<ActivityRewardStatus>,
    pub carryover: ActivityCarryoverStatus,
}

fn reward_claimable(
    ledger: &ActivityLedger,
    policy: &ActivityPolicy,
    definition: &RewardDefinition,
    now_ms: u64,
) -> bool {
    policy.active(now_ms)
        && policy.phase.rank() >= definition.phase.rank()
        && ledger
            .phases
            .get(definition.phase.key())
            .is_some_and(|phase| phase.max_level >= definition.level)
}

fn status_for(
    account: &AccountRecord,
    policy: &ActivityPolicy,
    now_ms: u64,
) -> Result<ActivityStatus, String> {
    if account.active_ban(now_ms).is_some() {
        return Err("activityAccountBanned".into());
    }
    let characters = account_characters(account)?;
    let ledger = account.activities.clone().unwrap_or_default();
    ledger.validate()?;
    let rewards = REWARDS
        .iter()
        .map(|definition| {
            let claim = ledger.claims.get(definition.id);
            ActivityRewardStatus {
                id: definition.id.into(),
                phase: definition.phase,
                kind: definition.kind,
                required_level: definition.level,
                amount: definition.amount,
                qualified_level: ledger
                    .phases
                    .get(definition.phase.key())
                    .map_or(0, |phase| phase.max_level),
                eligible: claim.is_none() && reward_claimable(&ledger, policy, definition, now_ms),
                claimed: claim.is_some(),
                claimed_at_ms: claim.map(|claim| claim.claimed_at_ms),
                cosmetic_class: claim
                    .filter(|claim| claim.kind == ActivityRewardKind::Cosmetic)
                    .map(|claim| claim.class),
            }
        })
        .collect();
    let mut usd = 0u64;
    let mut credits = 0u64;
    let mut reviews = 0u32;
    let mut reconciliation = false;
    for record in ledger.recharge_records.values() {
        if record.review_events.is_empty() {
            usd = usd
                .checked_add(record.amount_usd_minor)
                .ok_or("activityCarryoverOverflow")?;
            credits = credits
                .checked_add(u64::from(record.credits))
                .ok_or("activityCarryoverOverflow")?;
        } else {
            reviews = reviews.checked_add(1).ok_or("activityCarryoverOverflow")?;
        }
        reconciliation |= record.reconciliation_required || !record.review_events.is_empty();
    }
    let test_orders = account.billing.as_ref().map_or(0, |billing| {
        billing
            .orders
            .values()
            .filter(|order| !order.livemode && order.paid_at_ms.is_some())
            .count()
    });
    // A monthly receipt records debit/UID, not live-vs-sandbox funding. Never
    // convert it to extra days and also return the same payment's full Credits.
    if !ledger.recharge_records.is_empty()
        && account
            .monthly_card
            .as_ref()
            .is_some_and(|card| !card.item_receipts.is_empty())
    {
        reconciliation = true;
    }
    let mut rows = ledger.recharge_records.values().collect::<Vec<_>>();
    rows.sort_by_key(|record| std::cmp::Reverse((record.paid_at_ms, &record.order_id)));
    let records = rows
        .into_iter()
        .take(MAX_PUBLIC_RECHARGE_RECORDS)
        .map(|record| ActivityRechargeStatus {
            order_id: record.order_id.clone(),
            amount_usd_minor: record.amount_usd_minor,
            credits: record.credits,
            recorded_at_ms: record.recorded_at_ms,
            state: if record.review_events.is_empty() {
                "recorded"
            } else {
                "review"
            }
            .into(),
        })
        .collect();
    Ok(ActivityStatus {
        schema_version: crate::billing::BILLING_SCHEMA_VERSION,
        phase: policy.phase,
        phase_start_ms: policy.phase_start_ms,
        phase_end_ms: policy.phase_end_ms,
        max_level: characters
            .iter()
            .map(|character| character.level)
            .max()
            .unwrap_or(0),
        characters,
        rewards,
        carryover: ActivityCarryoverStatus {
            real_verified_usd_minor: usd,
            verified_credits: credits,
            monthly_days: 0,
            test_orders_excluded: u32::try_from(test_orders)
                .map_err(|_| "activityCarryoverOverflow")?,
            review_orders: reviews,
            records,
            records_total: ledger.recharge_records.len(),
            settlement: if reconciliation {
                ActivitySettlement::ReconciliationRequired
            } else {
                ActivitySettlement::Recorded
            },
        },
    })
}

impl SimulationConfig {
    pub fn with_activity_policy(mut self, policy: ActivityPolicy) -> Self {
        self.activity_policy = policy;
        self
    }

    pub fn activity_status(
        &self,
        account_id: &str,
        policy: &ActivityPolicy,
        now_ms: u64,
    ) -> Result<ActivityStatus, String> {
        policy.validate()?;
        // In-memory eligibility may be older than an uncertain durable COMMIT.
        // A frozen authority must not publish that stale view via the fast path.
        self.ensure_account_store_writable()?;
        self.refresh_account_store_account(account_id)?;
        {
            let store = self
                .account_store
                .lock()
                .map_err(|_| "activityUnavailable")?;
            let account = store
                .accounts
                .get(account_id)
                .ok_or("activityAccountMissing")?;
            let mut observed = account.clone();
            observe_account(&mut observed, policy, now_ms)?;
            if observed.activities == account.activities {
                self.ensure_account_store_writable()?;
                return status_for(account, policy, now_ms);
            }
        }
        // Only a specifically rejected source CAS is safe to retry. A lost
        // commit response remains frozen/outcome-unknown in the common store.
        for attempt in 0..3 {
            if attempt != 0 {
                self.refresh_account_store_account(account_id)?;
            }
            let result = self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
                let account = store
                    .accounts
                    .get_mut(account_id)
                    .ok_or("activityAccountMissing")?;
                observe_account(account, policy, now_ms)?;
                status_for(account, policy, now_ms)
            });
            match result {
                Err(error)
                    if attempt < 2 && error.starts_with("stale postgres account-store write") =>
                {
                    continue
                }
                result => return result,
            }
        }
        unreachable!("bounded retry returns")
    }

    /// Idempotently reserve one account/phase reward. This intentionally does
    /// not change a save, balance, mail, equipped title, appearance or expiry.
    pub fn activity_claim(
        &self,
        account_id: &str,
        character_index: i32,
        reward_id: &str,
        policy: &ActivityPolicy,
        now_ms: u64,
    ) -> Result<ActivityStatus, String> {
        policy.validate()?;
        self.ensure_account_store_writable()?;
        let definition = REWARDS
            .iter()
            .find(|reward| reward.id == reward_id)
            .ok_or("activityRewardMissing")?;
        for attempt in 0..3 {
            self.refresh_account_store_account(account_id)?;
            let result = self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
                let account = store
                    .accounts
                    .get_mut(account_id)
                    .ok_or("activityAccountMissing")?;
                if account.active_ban(now_ms).is_some() {
                    return Err("activityAccountBanned".into());
                }
                let character = owned_save(account, character_index)?.character.clone();
                observe_account(account, policy, now_ms)?;
                let ledger = account.activities.as_ref().ok_or("activityDisabled")?;
                if let Some(claim) = ledger.claims.get(reward_id) {
                    if claim.character_index != character.index
                        || claim.character_name != character.name
                        || claim.class != character.class
                        || claim.gender != character.gender
                    {
                        return Err("activityClaimRecipientChanged".into());
                    }
                    return status_for(account, policy, now_ms);
                }
                if !reward_claimable(ledger, policy, definition, now_ms) {
                    return Err(if !policy.active(now_ms) {
                        "activityDisabled"
                    } else {
                        "activityLevelRequired"
                    }
                    .into());
                }
                let qualified_level = ledger.phases[definition.phase.key()].max_level;
                account
                    .activities
                    .as_mut()
                    .expect("ledger observed")
                    .claims
                    .insert(
                        reward_id.into(),
                        ActivityClaimRecord {
                            reward_id: reward_id.into(),
                            phase: definition.phase,
                            kind: definition.kind,
                            amount: definition.amount,
                            qualified_level,
                            character_index: character.index,
                            character_name: character.name,
                            class: character.class,
                            gender: character.gender,
                            claimed_at_ms: now_ms,
                        },
                    );
                status_for(account, policy, now_ms)
            });
            match result {
                Err(error)
                    if attempt < 2 && error.starts_with("stale postgres account-store write") =>
                {
                    continue
                }
                result => return result,
            }
        }
        unreachable!("bounded retry returns")
    }
}

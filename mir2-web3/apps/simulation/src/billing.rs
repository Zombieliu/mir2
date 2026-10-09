//! Durable, server-bound one-time recharge receipts. Provider authentication
//! belongs to the gateway; this module never accepts browser-supplied prices.
use std::collections::{BTreeMap, BTreeSet};

use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::{AccountRecord, AccountStore, CharacterSaveRecord, SimulationConfig};

pub const BILLING_SCHEMA_VERSION: u16 = 8;
const MAX_ORDERS_PER_ACCOUNT: usize = 4_096;
const MAX_REVIEW_EVENTS_PER_ORDER: usize = 128;
pub(crate) const BILLING_COMMIT_OUTCOME_UNKNOWN: &str = "BILLING_COMMIT_OUTCOME_UNKNOWN";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RechargeOffer {
    pub id: String,
    pub label: String,
    pub currency: String,
    pub amount_minor: u64,
    pub credits: u32,
}
impl RechargeOffer {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_key(&self.id, 64)
            || self.label.trim().is_empty()
            || self.label.len() > 256
            || self.label.chars().any(char::is_control)
            || !valid_currency(&self.currency)
            || self.amount_minor == 0
            || self.credits == 0
        {
            return Err("billingOfferInvalid".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RechargeOrderState {
    Prepared,
    CheckoutBound,
    PaidPending,
    Applied,
    Review,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RechargeAppliedReceipt {
    pub character_index: i32,
    pub credits: u32,
    pub credit_before: u32,
    pub credit_after: u32,
    pub revision: u64,
    pub at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RechargeReviewReceipt {
    pub kind: String,
    pub event_id: String,
    pub at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RechargeOrder {
    pub id: String,
    pub request_id: String,
    pub character_index: i32,
    pub offer: RechargeOffer,
    pub created_at_ms: u64,
    pub livemode: bool,
    pub checkout_session_id: Option<String>,
    pub payment_intent_id: Option<String>,
    pub state: RechargeOrderState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paid_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied: Option<RechargeAppliedReceipt>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub review_events: BTreeMap<String, RechargeReviewReceipt>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRechargePayment {
    pub session_id: String,
    pub payment_intent_id: String,
    pub amount_minor: u64,
    pub currency: String,
    pub livemode: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillingLedger {
    pub orders: BTreeMap<String, RechargeOrder>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RechargeCreditApplication {
    pub credit: u32,
    pub granted: u32,
    pub revision: u64,
    pub order_ids: Vec<String>,
}

fn valid_key(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn valid_currency(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|b| b.is_ascii_lowercase())
}
fn valid_provider_id(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() > prefix.len()
        && !value.contains("_secret_")
        && valid_key(value, 255)
}
fn valid_order_id(value: &str) -> bool {
    value.len() == 68
        && value.starts_with("rch_")
        && value[4..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_review_kind(value: &str) -> bool {
    matches!(value, "refund" | "dispute" | "charge.refunded")
        || (valid_key(value, 96)
            && (value.starts_with("charge.dispute.") || value.starts_with("refund.")))
}
fn new_order_id() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| "billingEntropyUnavailable")?;
    let mut id = String::from("rch_");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut id, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(id)
}

impl BillingLedger {
    pub fn validate(&self) -> Result<(), String> {
        if self.orders.len() > MAX_ORDERS_PER_ACCOUNT {
            return Err("billingLedgerFull".into());
        }
        let mut requests = BTreeSet::new();
        let mut sessions = BTreeSet::new();
        let mut payments = BTreeSet::new();
        for (id, order) in &self.orders {
            order.offer.validate()?;
            if id != &order.id
                || !valid_order_id(id)
                || !valid_key(&order.request_id, 128)
                || !requests.insert(&order.request_id)
                || order.character_index < 0
                || order.review_events.len() > MAX_REVIEW_EVENTS_PER_ORDER
            {
                return Err("billingLedgerInvalid".into());
            }
            if let Some(session) = &order.checkout_session_id {
                if !valid_provider_id(session, "cs_") || !sessions.insert(session) {
                    return Err("billingCheckoutBindingInvalid".into());
                }
            }
            if let Some(payment) = &order.payment_intent_id {
                if !valid_provider_id(payment, "pi_") || !payments.insert(payment) {
                    return Err("billingPaymentBindingInvalid".into());
                }
            }
            if order.payment_intent_id.is_some() != order.paid_at_ms.is_some()
                || (order.payment_intent_id.is_some() && order.checkout_session_id.is_none())
                || order.paid_at_ms.is_some_and(|at| at < order.created_at_ms)
            {
                return Err("billingPaymentReceiptInvalid".into());
            }
            if let Some(applied) = &order.applied {
                if applied.character_index != order.character_index
                    || applied.credits != order.offer.credits
                    || applied.credit_before.checked_add(applied.credits)
                        != Some(applied.credit_after)
                    || applied.revision == 0
                    || order.paid_at_ms.is_none_or(|at| applied.at_ms < at)
                {
                    return Err("billingApplicationReceiptInvalid".into());
                }
            }
            for (event, review) in &order.review_events {
                if event != &review.event_id
                    || !valid_provider_id(event, "evt_")
                    || !valid_review_kind(&review.kind)
                    || review.at_ms < order.created_at_ms
                {
                    return Err("billingReviewReceiptInvalid".into());
                }
            }
            let valid_state = match order.state {
                RechargeOrderState::Prepared => {
                    order.checkout_session_id.is_none()
                        && order.payment_intent_id.is_none()
                        && order.applied.is_none()
                        && order.review_events.is_empty()
                }
                RechargeOrderState::CheckoutBound => {
                    order.checkout_session_id.is_some()
                        && order.payment_intent_id.is_none()
                        && order.applied.is_none()
                        && order.review_events.is_empty()
                }
                RechargeOrderState::PaidPending => {
                    order.payment_intent_id.is_some()
                        && order.applied.is_none()
                        && order.review_events.is_empty()
                }
                RechargeOrderState::Applied => {
                    order.applied.is_some() && order.review_events.is_empty()
                }
                RechargeOrderState::Review => !order.review_events.is_empty(),
            };
            if !valid_state {
                return Err("billingOrderStateInvalid".into());
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_account_record(account: &AccountRecord) -> Result<(), String> {
    if let Some(ledger) = &account.billing {
        ledger.validate()?;
    }
    Ok(())
}

fn reserves_credit_capacity(order: &RechargeOrder) -> bool {
    order.applied.is_none()
        && matches!(
            order.state,
            RechargeOrderState::Prepared
                | RechargeOrderState::CheckoutBound
                | RechargeOrderState::PaidPending
        )
}

/// Run inside the character-deletion transaction so a concurrent checkout or
/// item purchase cannot strand its cash, wallet balance or unused access unit.
pub(crate) fn ensure_character_billing_deletable(
    account: &AccountRecord,
    character_index: i32,
) -> Result<(), String> {
    validate_account_record(account)?;
    if let Some(ledger) = &account.billing {
        if ledger.orders.values().any(|order| {
            order.character_index == character_index
                && (reserves_credit_capacity(order)
                    || (order.state == RechargeOrderState::Review && order.applied.is_none()))
        }) {
            return Err("billingCharacterHasUnsettledOrders".into());
        }
        let has_payment_history = ledger.orders.values().any(|order| {
            order.character_index == character_index && order.payment_intent_id.is_some()
        });
        if has_payment_history
            && account
                .saves
                .get(&character_index)
                .is_some_and(|save| save.credit != 0)
        {
            return Err("billingCharacterHasRemainingCredits".into());
        }
    }
    if let Some(card) = &account.monthly_card {
        card.validate()?;
        if card
            .item_receipts
            .values()
            .any(|unit| unit.character_index == character_index && unit.redeemed_at_ms.is_none())
        {
            return Err("billingCharacterHasUnusedMonthlyCard".into());
        }
    }
    Ok(())
}
/// In-process uniqueness is checked over the complete store. Cross-process
/// payment uniqueness also requires the gateway/schema leader's database fence.
pub(crate) fn validate_complete_store(store: &AccountStore) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    let mut sessions = BTreeSet::new();
    let mut payments = BTreeSet::new();
    for account in store.accounts.values() {
        if store.schema_version != BILLING_SCHEMA_VERSION
            && account
                .monthly_card
                .as_ref()
                .is_some_and(|card| !card.item_receipts.is_empty())
        {
            return Err("billingSchemaInvalid".into());
        }
        let Some(ledger) = &account.billing else {
            continue;
        };
        if store.schema_version != BILLING_SCHEMA_VERSION {
            return Err("billingSchemaInvalid".into());
        }
        ledger.validate()?;
        for order in ledger.orders.values() {
            if !ids.insert(&order.id)
                || order
                    .checkout_session_id
                    .as_ref()
                    .is_some_and(|id| !sessions.insert(id))
                || order
                    .payment_intent_id
                    .as_ref()
                    .is_some_and(|id| !payments.insert(id))
            {
                return Err("billingProviderBindingAlreadyUsed".into());
            }
        }
    }
    Ok(())
}

fn owned_save(
    account: &AccountRecord,
    character_index: i32,
) -> Result<&CharacterSaveRecord, String> {
    let character = account
        .characters
        .iter()
        .find(|c| c.index == character_index)
        .ok_or("billingCharacterMissing")?;
    let save = account
        .saves
        .get(&character_index)
        .ok_or("billingCharacterSaveMissing")?;
    if save.character.index != character.index
        || save.character.name != character.name
        || save.character.class != character.class
        || save.character.gender != character.gender
    {
        return Err("billingCharacterIdentityMismatch".into());
    }
    Ok(save)
}
pub(crate) fn has_pending(account: &AccountRecord, character_index: i32) -> bool {
    account.billing.as_ref().is_some_and(|ledger| {
        ledger.orders.values().any(|order| {
            order.character_index == character_index
                && order.state == RechargeOrderState::PaidPending
        })
    })
}
pub(crate) fn apply_pending_to_save(
    account: &mut AccountRecord,
    save: &mut CharacterSaveRecord,
    now_ms: u64,
) -> Result<RechargeCreditApplication, String> {
    validate_account_record(account)?;
    let mut pending = account
        .billing
        .as_ref()
        .map(|ledger| {
            ledger
                .orders
                .values()
                .filter(|order| {
                    order.character_index == save.character.index
                        && order.state == RechargeOrderState::PaidPending
                })
                .map(|order| order.id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    pending.sort();
    if pending.is_empty() {
        return Ok(RechargeCreditApplication {
            credit: save.credit,
            granted: 0,
            revision: save.revision,
            order_ids: pending,
        });
    }
    let ledger = account
        .billing
        .as_ref()
        .expect("pending entries require a ledger");
    let mut credit = save.credit;
    let mut granted = 0u32;
    let mut applicable = Vec::new();
    // Reservations can be overtaken by ordinary earnings. Apply each complete
    // payment that fits, retaining the others so spending can make room later.
    // Finish all fallible checks before changing any receipt or balance.
    for id in &pending {
        let order = &ledger.orders[id];
        if now_ms < order.paid_at_ms.ok_or("billingPaymentReceiptInvalid")? {
            return Err("billingClockBeforePayment".into());
        }
        if let Some(next_credit) = credit.checked_add(order.offer.credits) {
            granted = granted
                .checked_add(order.offer.credits)
                .ok_or("billingCreditOverflow")?;
            credit = next_credit;
            applicable.push(id.clone());
        }
    }
    if applicable.is_empty() {
        return Ok(RechargeCreditApplication {
            credit: save.credit,
            granted: 0,
            revision: save.revision,
            order_ids: Vec::new(),
        });
    }
    let revision = save
        .revision
        .checked_add(1)
        .ok_or("billingCharacterRevisionExhausted")?;
    let ledger = account
        .billing
        .as_mut()
        .expect("pending entries require a ledger");
    let mut before = save.credit;
    for id in &applicable {
        let order = ledger.orders.get_mut(id).expect("pending entry exists");
        let after = before
            .checked_add(order.offer.credits)
            .expect("preflight checked the batch");
        order.applied = Some(RechargeAppliedReceipt {
            character_index: save.character.index,
            credits: order.offer.credits,
            credit_before: before,
            credit_after: after,
            revision,
            at_ms: now_ms,
        });
        order.state = RechargeOrderState::Applied;
        before = after;
    }
    save.credit = credit;
    save.revision = revision;
    Ok(RechargeCreditApplication {
        credit,
        granted,
        revision,
        order_ids: applicable,
    })
}

impl SimulationConfig {
    pub fn with_billing_monthly_card_credit_price(
        mut self,
        price: Option<u32>,
    ) -> Result<Self, String> {
        if price == Some(0) {
            return Err("billingMonthlyCardPriceInvalid".into());
        }
        self.billing_monthly_card_credit_price = price;
        Ok(self)
    }
    pub fn with_billing_environment(self) -> Result<Self, String> {
        let price = match std::env::var("MIR2_MONTHLY_CARD_CREDIT_PRICE") {
            Err(std::env::VarError::NotPresent) => None,
            Ok(value) => Some(
                value
                    .parse::<u32>()
                    .map_err(|_| "billingMonthlyCardPriceInvalid")?,
            ),
            Err(_) => return Err("billingMonthlyCardPriceInvalid".into()),
        };
        self.with_billing_monthly_card_credit_price(price)
    }
    pub fn prepare_recharge_order(
        &self,
        account_id: &str,
        character_index: i32,
        request_id: &str,
        offer: &RechargeOffer,
        now_ms: u64,
        livemode: bool,
    ) -> Result<RechargeOrder, String> {
        offer.validate()?;
        if !valid_key(request_id, 128) {
            return Err("billingRequestIdInvalid".into());
        }
        self.refresh_account_store_account(account_id)?;
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            let account = store
                .accounts
                .get_mut(account_id)
                .ok_or("billingAccountMissing")?;
            if account.active_ban(now_ms).is_some() {
                return Err("billingAccountBanned".into());
            }
            let durable_credit = owned_save(account, character_index)?.credit;
            validate_account_record(account)?;
            let ledger = account.billing.get_or_insert_with(BillingLedger::default);
            if let Some(existing) = ledger
                .orders
                .values()
                .find(|order| order.request_id == request_id)
            {
                if existing.character_index != character_index
                    || &existing.offer != offer
                    || existing.livemode != livemode
                {
                    return Err("billingRequestIdReused".into());
                }
                return Ok(existing.clone());
            }
            if ledger.orders.len() >= MAX_ORDERS_PER_ACCOUNT {
                return Err("billingLedgerFull".into());
            }
            ledger
                .orders
                .values()
                .filter(|order| {
                    order.character_index == character_index && reserves_credit_capacity(order)
                })
                .try_fold(durable_credit, |credit, order| {
                    credit
                        .checked_add(order.offer.credits)
                        .ok_or("billingCreditCapacityExceeded")
                })?
                .checked_add(offer.credits)
                .ok_or("billingCreditCapacityExceeded")?;
            let order = RechargeOrder {
                id: new_order_id()?,
                request_id: request_id.into(),
                character_index,
                offer: offer.clone(),
                created_at_ms: now_ms,
                livemode,
                checkout_session_id: None,
                payment_intent_id: None,
                state: RechargeOrderState::Prepared,
                paid_at_ms: None,
                applied: None,
                review_events: BTreeMap::new(),
            };
            ledger.orders.insert(order.id.clone(), order.clone());
            Ok(order)
        })
    }
    pub fn bind_recharge_checkout(
        &self,
        account_id: &str,
        order_id: &str,
        session_id: &str,
        now_ms: u64,
    ) -> Result<RechargeOrder, String> {
        if !valid_provider_id(session_id, "cs_") {
            return Err("billingCheckoutIdInvalid".into());
        }
        self.refresh_account_store_account(account_id)?;
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            for (owner, account) in &store.accounts {
                if let Some(ledger) = &account.billing {
                    if ledger.orders.values().any(|order| {
                        order.checkout_session_id.as_deref() == Some(session_id)
                            && (owner != account_id || order.id != order_id)
                    }) {
                        return Err("billingProviderBindingAlreadyUsed".into());
                    }
                }
            }
            let order = store
                .accounts
                .get_mut(account_id)
                .and_then(|a| a.billing.as_mut())
                .and_then(|b| b.orders.get_mut(order_id))
                .ok_or("billingOrderMissing")?;
            if now_ms < order.created_at_ms {
                return Err("billingClockBeforeOrder".into());
            }
            if let Some(bound) = &order.checkout_session_id {
                if bound != session_id {
                    return Err("billingCheckoutBindingMismatch".into());
                }
                return Ok(order.clone());
            }
            if order.state != RechargeOrderState::Prepared {
                return Err("billingOrderTerminal".into());
            }
            order.checkout_session_id = Some(session_id.into());
            order.state = RechargeOrderState::CheckoutBound;
            Ok(order.clone())
        })
    }
    pub fn confirm_recharge_payment(
        &self,
        account_id: &str,
        order_id: &str,
        payment: &VerifiedRechargePayment,
        now_ms: u64,
    ) -> Result<RechargeOrder, String> {
        if !valid_provider_id(&payment.session_id, "cs_")
            || !valid_provider_id(&payment.payment_intent_id, "pi_")
            || !valid_currency(&payment.currency)
        {
            return Err("billingPaymentInvalid".into());
        }
        self.refresh_account_store_account(account_id)?;
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            for (owner, account) in &store.accounts {
                if let Some(ledger) = &account.billing {
                    if ledger.orders.values().any(|order| {
                        (order.checkout_session_id.as_deref() == Some(&payment.session_id)
                            || order.payment_intent_id.as_deref()
                                == Some(&payment.payment_intent_id))
                            && (owner != account_id || order.id != order_id)
                    }) {
                        return Err("billingProviderBindingAlreadyUsed".into());
                    }
                }
            }
            let order = store
                .accounts
                .get_mut(account_id)
                .and_then(|a| a.billing.as_mut())
                .and_then(|b| b.orders.get_mut(order_id))
                .ok_or("billingOrderMissing")?;
            if payment.amount_minor != order.offer.amount_minor
                || payment.currency != order.offer.currency
                || payment.livemode != order.livemode
                || order
                    .checkout_session_id
                    .as_ref()
                    .is_some_and(|id| id != &payment.session_id)
                || order
                    .payment_intent_id
                    .as_ref()
                    .is_some_and(|id| id != &payment.payment_intent_id)
            {
                return Err("billingPaymentBindingMismatch".into());
            }
            if now_ms < order.created_at_ms {
                return Err("billingClockBeforeOrder".into());
            }
            // A signed payment can precede the gateway's checkout-bind response.
            // Gateway verification must bind its metadata to this saved order.
            if order.payment_intent_id.is_none() {
                order.checkout_session_id = Some(payment.session_id.clone());
                order.payment_intent_id = Some(payment.payment_intent_id.clone());
                order.paid_at_ms = Some(now_ms);
                if order.state != RechargeOrderState::Review {
                    order.state = RechargeOrderState::PaidPending;
                }
            }
            Ok(order.clone())
        })
    }
    pub fn mark_recharge_review(
        &self,
        account_id: &str,
        order_id: &str,
        kind: &str,
        event_id: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        if !valid_review_kind(kind) || !valid_provider_id(event_id, "evt_") {
            return Err("billingReviewEventInvalid".into());
        }
        self.refresh_account_store_account(account_id)?;
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            let order = store
                .accounts
                .get_mut(account_id)
                .and_then(|a| a.billing.as_mut())
                .and_then(|b| b.orders.get_mut(order_id))
                .ok_or("billingOrderMissing")?;
            if now_ms < order.created_at_ms {
                return Err("billingClockBeforeOrder".into());
            }
            if let Some(existing) = order.review_events.get(event_id) {
                if existing.kind != kind {
                    return Err("billingReviewEventReused".into());
                }
                return Ok(());
            }
            if order.review_events.len() >= MAX_REVIEW_EVENTS_PER_ORDER {
                return Err("billingReviewLedgerFull".into());
            }
            order.review_events.insert(
                event_id.into(),
                RechargeReviewReceipt {
                    kind: kind.into(),
                    event_id: event_id.into(),
                    at_ms: now_ms,
                },
            );
            order.state = RechargeOrderState::Review;
            Ok(())
        })
    }
    /// Provider verification uses the permanent frozen ledger even after an
    /// archived character has disappeared. Callers must still compare its
    /// character index with the authenticated provider metadata.
    pub fn recharge_order_for_payment(
        &self,
        account_id: &str,
        order_id: &str,
    ) -> Result<RechargeOrder, String> {
        self.refresh_account_store_account(account_id)?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "billingStoreUnavailable")?;
        validate_complete_store(&store)?;
        store
            .accounts
            .get(account_id)
            .and_then(|account| account.billing.as_ref())
            .and_then(|ledger| ledger.orders.get(order_id))
            .cloned()
            .ok_or_else(|| "billingOrderMissing".into())
    }

    pub fn recharge_orders(
        &self,
        account_id: &str,
        character_index: i32,
    ) -> Result<Vec<RechargeOrder>, String> {
        self.refresh_account_store_account(account_id)?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "billingStoreUnavailable")?;
        validate_complete_store(&store)?;
        let account = store
            .accounts
            .get(account_id)
            .ok_or("billingAccountMissing")?;
        owned_save(account, character_index)?;
        Ok(account
            .billing
            .as_ref()
            .map(|ledger| {
                ledger
                    .orders
                    .values()
                    .filter(|order| order.character_index == character_index)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }
    pub fn recharge_credit_status(
        &self,
        account_id: &str,
        character_index: i32,
    ) -> Result<(u32, u32), String> {
        self.refresh_account_store_account(account_id)?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "billingStoreUnavailable")?;
        validate_complete_store(&store)?;
        let account = store
            .accounts
            .get(account_id)
            .ok_or("billingAccountMissing")?;
        let save = owned_save(account, character_index)?;
        let pending = account
            .billing
            .as_ref()
            .map(|ledger| {
                ledger
                    .orders
                    .values()
                    .filter(|order| {
                        order.character_index == character_index
                            && order.state == RechargeOrderState::PaidPending
                    })
                    .try_fold(0u32, |sum, order| {
                        sum.checked_add(order.offer.credits)
                            .ok_or("billingCreditOverflow")
                    })
            })
            .transpose()?
            .unwrap_or_default();
        Ok((save.credit, pending))
    }
    /// Only the gateway's inactive/character-select authority may call this.
    /// An online owner must use its current Source checkpoint instead.
    pub fn consume_inactive_pending_recharge_credits(
        &self,
        account_id: &str,
        character_index: i32,
        now_ms: u64,
    ) -> Result<RechargeCreditApplication, String> {
        self.refresh_account_store_account(account_id)?;
        {
            let store = self
                .account_store
                .lock()
                .map_err(|_| "billingStoreUnavailable")?;
            validate_complete_store(&store)?;
            let account = store
                .accounts
                .get(account_id)
                .ok_or("billingAccountMissing")?;
            let save = owned_save(account, character_index)?;
            if !has_pending(account, character_index) {
                return Ok(RechargeCreditApplication {
                    credit: save.credit,
                    granted: 0,
                    revision: save.revision,
                    order_ids: Vec::new(),
                });
            }
        }
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            let account = store
                .accounts
                .get_mut(account_id)
                .ok_or("billingAccountMissing")?;
            let mut save = owned_save(account, character_index)?.clone();
            let result = apply_pending_to_save(account, &mut save, now_ms)?;
            if result.granted != 0 {
                account.saves.insert(character_index, save);
            }
            Ok(result)
        })
    }
}

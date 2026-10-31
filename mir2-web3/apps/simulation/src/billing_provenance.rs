//! Durable credit funding, committed with the existing account/save CAS.
//! Historical balances and ordinary earnings never become cash by inference.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    billing::{RechargeOrder, RechargeOrderState},
    AccountRecord, SimulationConfig,
};

const MAX_RECORDS: usize = 32_768;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreditProvenance {
    #[serde(default)]
    pub wallets: BTreeMap<i32, CreditWallet>,
    #[serde(default)]
    pub payments: BTreeMap<String, VerifiedPaymentRevenue>,
    #[serde(default)]
    pub adverse_events: BTreeMap<String, VerifiedPaymentAdverseEvent>,
    #[serde(default)]
    pub monthly_items: BTreeMap<u64, MonthlyCreditAllocation>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreditWallet {
    pub next_sequence: u64,
    pub lots: Vec<CreditLot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CreditFundingKind {
    Paid,
    Gift,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreditLot {
    pub lot_id: String,
    pub kind: CreditFundingKind,
    pub original_credits: u32,
    pub consumed_credits: u32,
    pub source_order_id: Option<String>,
    pub acquired_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreditLotUse {
    pub lot_id: String,
    pub kind: CreditFundingKind,
    pub source_order_id: Option<String>,
    pub credits_offset: u32,
    pub credits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonthlyCreditAllocation {
    pub unique_id: u64,
    pub owner_account_id: String,
    pub payer_account_id: String,
    pub character_index: i32,
    pub credit_price: u32,
    pub issued_at_ms: u64,
    pub gift: bool,
    pub parts: Vec<CreditLotUse>,
}

/// Constructed only after gateway signature validation AND provider re-query.
/// Missing tax or an unapproved FX conversion remains a pending funding source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifiedPaymentRevenue {
    pub provider: String,
    pub source_event_id: String,
    pub order_id: String,
    pub checkout_session_id: String,
    pub payment_intent_id: String,
    pub livemode: bool,
    pub currency: String,
    pub gross_amount_minor: u64,
    pub indirect_tax_minor: Option<u64>,
    pub discount_minor: Option<u64>,
    pub paid_at_ms: u64,
    pub observed_at_ms: u64,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifiedPaymentAdverseEvent {
    pub provider: String,
    pub source_event_id: String,
    pub order_id: String,
    pub payment_intent_id: String,
    pub livemode: bool,
    pub currency: String,
    pub gross_amount_minor: u64,
    pub cumulative_refunded_minor: u64,
    pub disputed_minor: u64,
    pub dispute_status: String,
    pub occurred_at_ms: u64,
    pub observed_at_ms: u64,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyRevenuePart {
    pub source_order_id: String,
    pub lot_id: String,
    pub credits: u32,
    pub credits_offset: u32,
    pub original_currency: String,
    pub original_amount_minor: u64,
    pub indirect_tax_minor: Option<u64>,
    pub eligible_micro_usd: u64,
    pub reversed_micro_usd: u64,
    pub paid_at_ms: u64,
    pub fx_snapshot_ref: String,
    pub evidence_ref: String,
    pub funding_status: String,
    pub reason_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SuiRevenueEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_transfer_source: Option<SuiNativeTransferRevenueEvidence>,
}

/// Immutable native-chain payment facts. Testnet value is test accounting only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuiRevenueEvidence {
    pub provider: String,
    pub network: String,
    pub module: String,
    pub receipt: crate::sui_usdc_billing::VerifiedUsdcReceipt,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuiNativeTransferRevenueEvidence {
    pub provider: String,
    pub network: String,
    pub receipt: crate::sui_usdc_billing::VerifiedNativeUsdcTransferReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyRevenueSource {
    pub unique_id: u64,
    pub owner_account_id: String,
    pub payer_account_id: String,
    pub character_index: i32,
    pub entitlement_at_ms: u64,
    pub service_start_ms: u64,
    pub service_end_ms: u64,
    pub paid_at_ms: u64,
    pub eligible_micro_usd: u64,
    pub reversed_micro_usd: u64,
    pub unknown_credits: u32,
    pub gift_credits: u32,
    pub parts: Vec<MonthlyRevenuePart>,
    pub funding_status: String,
    pub reason_code: String,
    pub source_evidence_ref: String,
    pub source_observed_at_ms: u64,
}

fn key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn account_key(s: &str) -> bool {
    !s.is_empty() && s.trim() == s && s.len() <= 256 && !s.chars().any(char::is_control)
}

impl CreditProvenance {
    pub fn is_empty(&self) -> bool {
        self.wallets.is_empty()
            && self.payments.is_empty()
            && self.adverse_events.is_empty()
            && self.monthly_items.is_empty()
    }

    pub fn validate(
        &self,
        orders: &BTreeMap<String, RechargeOrder>,
        sui_orders: &BTreeMap<String, crate::sui_usdc_billing::UsdcRechargeOrder>,
    ) -> Result<(), String> {
        if self.wallets.len() > MAX_RECORDS
            || self.payments.len() > MAX_RECORDS
            || self.adverse_events.len() > MAX_RECORDS
            || self.monthly_items.len() > MAX_RECORDS
        {
            return Err("billingProvenanceLedgerFull".into());
        }
        let mut lot_ids = BTreeSet::new();
        for (character, wallet) in &self.wallets {
            if *character < 0 || wallet.lots.len() > MAX_RECORDS {
                return Err("billingProvenanceInvalid".into());
            }
            for lot in &wallet.lots {
                if !key(&lot.lot_id)
                    || !lot_ids.insert(&lot.lot_id)
                    || lot.original_credits == 0
                    || lot.consumed_credits > lot.original_credits
                {
                    return Err("billingProvenanceInvalid".into());
                }
                if lot.kind == CreditFundingKind::Paid {
                    let id = lot
                        .source_order_id
                        .as_ref()
                        .ok_or("billingProvenanceOrderMissing")?;
                    let (index, applied) = if let Some(order) = orders.get(id) {
                        (order.character_index, order.applied.as_ref())
                    } else if let Some(order) = sui_orders.get(id) {
                        (order.character_index, order.applied.as_ref())
                    } else {
                        return Err("billingProvenanceOrderMissing".into());
                    };
                    if index != *character
                        || applied.is_none_or(|r| r.credits != lot.original_credits)
                    {
                        return Err("billingProvenanceOrderMismatch".into());
                    }
                } else if lot.source_order_id.is_some() {
                    return Err("billingProvenanceInvalid".into());
                }
            }
            wallet.balance()?;
        }
        for (order_id, receipt) in &self.payments {
            let order = orders
                .get(order_id)
                .ok_or("billingProvenanceOrderMissing")?;
            validate_payment(receipt, order)?;
        }
        for (event_id, event) in &self.adverse_events {
            if event_id != &event.source_event_id {
                return Err("billingProvenanceEventConflict".into());
            }
            validate_adverse(
                event,
                orders
                    .get(&event.order_id)
                    .ok_or("billingProvenanceOrderMissing")?,
            )?;
        }
        for (uid, item) in &self.monthly_items {
            if *uid != item.unique_id
                || item.credit_price == 0
                || item.parts.iter().map(|p| u64::from(p.credits)).sum::<u64>()
                    != u64::from(item.credit_price)
                || !account_key(&item.owner_account_id)
                || !account_key(&item.payer_account_id)
                || item.parts.iter().any(|p| p.credits == 0 || !key(&p.lot_id))
            {
                return Err("billingProvenanceMonthlyInvalid".into());
            }
            if !item.gift && item.owner_account_id != item.payer_account_id {
                return Err("billingProvenanceMonthlyInvalid".into());
            }
            if !item.gift {
                let wallet = self
                    .wallets
                    .get(&item.character_index)
                    .ok_or("billingProvenanceMonthlyInvalid")?;
                for part in &item.parts {
                    let lot = wallet
                        .lots
                        .iter()
                        .find(|l| l.lot_id == part.lot_id)
                        .ok_or("billingProvenanceMonthlyInvalid")?;
                    if lot.kind != part.kind
                        || lot.source_order_id != part.source_order_id
                        || part
                            .credits_offset
                            .checked_add(part.credits)
                            .is_none_or(|end| end > lot.consumed_credits)
                    {
                        return Err("billingProvenanceMonthlyInvalid".into());
                    }
                }
            }
        }
        let mut intervals: BTreeMap<&str, Vec<(u32, u32)>> = BTreeMap::new();
        for item in self.monthly_items.values().filter(|i| !i.gift) {
            for part in &item.parts {
                intervals
                    .entry(&part.lot_id)
                    .or_default()
                    .push((part.credits_offset, part.credits_offset + part.credits));
            }
        }
        for entries in intervals.values_mut() {
            entries.sort();
            if entries.windows(2).any(|pair| pair[0].1 > pair[1].0) {
                return Err("billingProvenanceAllocationReused".into());
            }
        }
        Ok(())
    }
}

impl CreditWallet {
    pub fn balance(&self) -> Result<u32, String> {
        self.lots.iter().try_fold(0u32, |sum, lot| {
            sum.checked_add(
                lot.original_credits
                    .checked_sub(lot.consumed_credits)
                    .ok_or("billingProvenanceInvalid")?,
            )
            .ok_or_else(|| "billingProvenanceOverflow".into())
        })
    }

    fn append(
        &mut self,
        character: i32,
        kind: CreditFundingKind,
        credits: u32,
        order: Option<&str>,
        at: u64,
    ) -> Result<(), String> {
        if credits == 0 {
            return Ok(());
        }
        if self.lots.len() >= MAX_RECORDS {
            return Err("billingProvenanceLedgerFull".into());
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or("billingProvenanceOverflow")?;
        self.lots.push(CreditLot {
            lot_id: format!("credit:{character}:{}", self.next_sequence),
            kind,
            original_credits: credits,
            consumed_credits: 0,
            source_order_id: order.map(str::to_owned),
            acquired_at_ms: at,
        });
        Ok(())
    }

    /// Existing credit is seeded unknown; a balance increase is a non-cash gift.
    /// Every persistent debit drains the same fixed FIFO, including other items.
    fn sync(&mut self, character: i32, balance: u32, at: u64) -> Result<(), String> {
        let current = self.balance()?;
        if balance > current {
            let kind = if self.lots.is_empty() {
                CreditFundingKind::Unknown
            } else {
                CreditFundingKind::Gift
            };
            self.append(character, kind, balance - current, None, at)?;
        } else if balance < current {
            self.consume(current - balance)?;
        }
        Ok(())
    }

    fn consume(&mut self, mut amount: u32) -> Result<Vec<CreditLotUse>, String> {
        if self.balance()? < amount {
            return Err("billingProvenanceInsufficientCredit".into());
        }
        let mut result = Vec::new();
        for lot in &mut self.lots {
            let take = amount.min(lot.original_credits - lot.consumed_credits);
            if take > 0 {
                result.push(CreditLotUse {
                    lot_id: lot.lot_id.clone(),
                    kind: lot.kind.clone(),
                    source_order_id: lot.source_order_id.clone(),
                    credits_offset: lot.consumed_credits,
                    credits: take,
                });
                lot.consumed_credits += take;
                amount -= take;
            }
            if amount == 0 {
                break;
            }
        }
        Ok(result)
    }
}

fn validate_payment(receipt: &VerifiedPaymentRevenue, order: &RechargeOrder) -> Result<(), String> {
    if receipt.provider != "stripe"
        || !key(&receipt.source_event_id)
        || receipt.order_id != order.id
        || order.checkout_session_id.as_deref() != Some(&receipt.checkout_session_id)
        || order.payment_intent_id.as_deref() != Some(&receipt.payment_intent_id)
        || receipt.livemode != order.livemode
        || receipt.currency != order.offer.currency
        || receipt.gross_amount_minor != order.offer.amount_minor
        || receipt
            .indirect_tax_minor
            .is_some_and(|tax| tax > receipt.gross_amount_minor)
        || receipt.paid_at_ms < order.created_at_ms
        || receipt.observed_at_ms < receipt.paid_at_ms
        || receipt.evidence_ref.is_empty()
        || receipt.evidence_ref.len() > 1024
    {
        return Err("billingRevenueEvidenceMismatch".into());
    }
    Ok(())
}

fn validate_adverse(
    event: &VerifiedPaymentAdverseEvent,
    order: &RechargeOrder,
) -> Result<(), String> {
    if event.provider != "stripe"
        || !key(&event.source_event_id)
        || event.order_id != order.id
        || order
            .payment_intent_id
            .as_ref()
            .is_some_and(|id| id != &event.payment_intent_id)
        || !event.payment_intent_id.starts_with("pi_")
        || event.livemode != order.livemode
        || event.currency != order.offer.currency
        || event.gross_amount_minor != order.offer.amount_minor
        || event.cumulative_refunded_minor > event.gross_amount_minor
        || event.disputed_minor > event.gross_amount_minor
        || !matches!(
            event.dispute_status.as_str(),
            "none"
                | "warning_needs_response"
                | "warning_under_review"
                | "warning_closed"
                | "needs_response"
                | "under_review"
                | "won"
                | "lost"
        )
        || event.occurred_at_ms < order.created_at_ms
        || event.observed_at_ms < event.occurred_at_ms
        || event.evidence_ref.is_empty()
        || event.evidence_ref.len() > 1024
    {
        return Err("billingRevenueAdverseEvidenceMismatch".into());
    }
    Ok(())
}

impl SimulationConfig {
    /// This method is a trusted server boundary; never expose its fields to a client route.
    pub fn record_verified_payment_revenue(
        &self,
        account_id: &str,
        receipt: &VerifiedPaymentRevenue,
    ) -> Result<(), String> {
        self.refresh_account_store_account(account_id)?;
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            let ledger = store
                .accounts
                .get_mut(account_id)
                .and_then(|a| a.billing.as_mut())
                .ok_or("billingOrderMissing")?;
            let order = ledger
                .orders
                .get(&receipt.order_id)
                .ok_or("billingOrderMissing")?;
            validate_payment(receipt, order)?;
            if let Some(old) = ledger.provenance.payments.get(&receipt.order_id) {
                // Distinct paid callbacks may describe the same provider object.
                let mut comparable = receipt.clone();
                comparable.source_event_id = old.source_event_id.clone();
                comparable.observed_at_ms = old.observed_at_ms;
                comparable.paid_at_ms = old.paid_at_ms;
                comparable.evidence_ref = old.evidence_ref.clone();
                if &comparable != old {
                    return Err("billingRevenueEvidenceConflict".into());
                }
                return Ok(());
            }
            ledger
                .provenance
                .payments
                .insert(receipt.order_id.clone(), receipt.clone());
            Ok(())
        })
    }

    pub fn record_verified_payment_adverse_event(
        &self,
        account_id: &str,
        event: &VerifiedPaymentAdverseEvent,
    ) -> Result<(), String> {
        self.refresh_account_store_account(account_id)?;
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            let ledger = store
                .accounts
                .get_mut(account_id)
                .and_then(|a| a.billing.as_mut())
                .ok_or("billingOrderMissing")?;
            validate_adverse(
                event,
                ledger
                    .orders
                    .get(&event.order_id)
                    .ok_or("billingOrderMissing")?,
            )?;
            if let Some(old) = ledger.provenance.adverse_events.get(&event.source_event_id) {
                // Re-fetches can observe a later cumulative refund. Keep separate immutable observations.
                if old.order_id != event.order_id
                    || old.payment_intent_id != event.payment_intent_id
                    || old.livemode != event.livemode
                {
                    return Err("billingRevenueEvidenceConflict".into());
                }
                return Ok(());
            }
            ledger
                .provenance
                .adverse_events
                .insert(event.source_event_id.clone(), event.clone());
            Ok(())
        })
    }
}

/// Called in the same transaction immediately before each verified recharge is applied.
pub(crate) fn apply_paid_credit_lot(
    account: &mut AccountRecord,
    character: i32,
    credit_before: u32,
    order_id: &str,
    amount: u32,
    at: u64,
) -> Result<(), String> {
    let ledger = account.billing.as_mut().ok_or("billingOrderMissing")?;
    let wallet = ledger.provenance.wallets.entry(character).or_default();
    wallet.sync(character, credit_before, at)?;
    if wallet
        .lots
        .iter()
        .any(|lot| lot.source_order_id.as_deref() == Some(order_id))
    {
        return Err("billingProvenanceCreditReplay".into());
    }
    wallet.append(
        character,
        CreditFundingKind::Paid,
        amount,
        Some(order_id),
        at,
    )
}

/// Self-purchase only. Gift issuance uses excluded allocation and drains payer FIFO separately.
pub(crate) fn allocate_monthly_purchase(
    account: &mut AccountRecord,
    owner: &str,
    character: i32,
    credit_before: u32,
    uids: &[u64],
    unit_price: u32,
    at: u64,
) -> Result<(), String> {
    let ledger = account.billing.get_or_insert_with(Default::default);
    let provenance = &mut ledger.provenance;
    let wallet = provenance.wallets.entry(character).or_default();
    wallet.sync(character, credit_before, at)?;
    for uid in uids {
        if provenance.monthly_items.contains_key(uid) {
            return Err("billingProvenanceMonthlyReplay".into());
        }
        let parts = wallet.consume(unit_price)?;
        provenance.monthly_items.insert(
            *uid,
            MonthlyCreditAllocation {
                unique_id: *uid,
                owner_account_id: owner.into(),
                payer_account_id: owner.into(),
                character_index: character,
                credit_price: unit_price,
                issued_at_ms: at,
                gift: false,
                parts,
            },
        );
    }
    Ok(())
}

pub(crate) fn consume_ordinary_credit(
    account: &mut AccountRecord,
    character: i32,
    credit_before: u32,
    amount: u32,
    at: u64,
) -> Result<(), String> {
    let ledger = account.billing.get_or_insert_with(Default::default);
    let wallet = ledger.provenance.wallets.entry(character).or_default();
    wallet.sync(character, credit_before, at)?;
    wallet.consume(amount)?;
    Ok(())
}

pub(crate) fn record_gift_monthly_units(
    account: &mut AccountRecord,
    owner: &str,
    payer: &str,
    character: i32,
    uids: &[u64],
    price: u32,
    at: u64,
) -> Result<(), String> {
    let provenance = &mut account
        .billing
        .get_or_insert_with(Default::default)
        .provenance;
    for uid in uids {
        if provenance.monthly_items.contains_key(uid) {
            return Err("billingProvenanceMonthlyReplay".into());
        }
        provenance.monthly_items.insert(
            *uid,
            MonthlyCreditAllocation {
                unique_id: *uid,
                owner_account_id: owner.into(),
                payer_account_id: payer.into(),
                character_index: character,
                credit_price: price,
                issued_at_ms: at,
                gift: true,
                parts: vec![CreditLotUse {
                    lot_id: format!("gift:{uid}"),
                    kind: CreditFundingKind::Gift,
                    source_order_id: None,
                    credits_offset: 0,
                    credits: price,
                }],
            },
        );
    }
    Ok(())
}

/// Reconcile every durable save change, so ordinary spending also drains paid lots.
/// A newly observed balance is historical/unknown; no old order is retroactively paid.
pub(crate) fn observe_account_commit(
    before: Option<&AccountRecord>,
    after: &mut AccountRecord,
    at: u64,
) -> Result<(), String> {
    let relevant = after.billing.is_some()
        || after
            .monthly_card
            .as_ref()
            .is_some_and(|c| !c.item_receipts.is_empty());
    if !relevant {
        return Ok(());
    }
    if let Some(old) = before.and_then(|a| a.billing.as_ref()) {
        let next = &after
            .billing
            .as_ref()
            .ok_or("billingProvenanceHistoryRemoved")?
            .provenance;
        if old
            .provenance
            .payments
            .iter()
            .any(|(k, v)| next.payments.get(k) != Some(v))
            || old
                .provenance
                .adverse_events
                .iter()
                .any(|(k, v)| next.adverse_events.get(k) != Some(v))
            || old
                .provenance
                .monthly_items
                .iter()
                .any(|(k, v)| next.monthly_items.get(k) != Some(v))
        {
            return Err("billingProvenanceHistoryChanged".into());
        }
        for (character, wallet) in &old.provenance.wallets {
            let new = next
                .wallets
                .get(character)
                .ok_or("billingProvenanceHistoryChanged")?;
            if new.next_sequence < wallet.next_sequence || new.lots.len() < wallet.lots.len() {
                return Err("billingProvenanceHistoryChanged".into());
            }
            for (previous, current) in wallet.lots.iter().zip(&new.lots) {
                let mut comparable = current.clone();
                comparable.consumed_credits = previous.consumed_credits;
                if &comparable != previous || current.consumed_credits < previous.consumed_credits {
                    return Err("billingProvenanceHistoryChanged".into());
                }
            }
        }
    }
    let saves: Vec<_> = after.saves.iter().map(|(i, s)| (*i, s.credit)).collect();
    let ledger = after.billing.get_or_insert_with(Default::default);
    for (character, balance) in saves {
        let wallet = ledger.provenance.wallets.entry(character).or_default();
        wallet.sync(character, balance, at)?;
    }
    Ok(())
}

fn portion(total: u64, offset: u32, credits: u32, total_credits: u32) -> Result<u64, String> {
    let end = offset
        .checked_add(credits)
        .ok_or("billingProvenanceOverflow")?;
    if total_credits == 0 || end > total_credits {
        return Err("billingProvenanceAllocationExceeded".into());
    }
    let q = u128::from(total_credits);
    let a = u128::from(total) * u128::from(offset) / q;
    let b = u128::from(total) * u128::from(end) / q;
    u64::try_from(b - a).map_err(|_| "billingProvenanceOverflow".into())
}

/// Only actual owned, redeemed monthly-card units yield source rows. Recharge alone yields none.
fn sui_monthly_revenue_part(
    order: &crate::sui_usdc_billing::UsdcRechargeOrder,
    part: &CreditLotUse,
    owner: &str,
    issued_at_ms: u64,
    livemode: bool,
) -> Result<MonthlyRevenuePart, String> {
    use crate::sui_usdc_billing::{account_ref, UsdcOrderState};
    order.validate()?;
    let (paid_at_ms, amount_micro, evidence_ref, invoice_source, transfer_source) =
        if let Some(receipt) = &order.receipt {
            (
                receipt.confirmed_at_ms,
                receipt.amount_micro,
                format!(
                    "sui:testnet:{}:{}:{}",
                    receipt.chain_identifier, receipt.digest, receipt.event_seq
                ),
                Some(SuiRevenueEvidence {
                    provider: "sui_native_usdc".into(),
                    network: "sui:testnet".into(),
                    module: "recharge".into(),
                    receipt: receipt.clone(),
                }),
                None,
            )
        } else if let Some(receipt) = &order.transfer_receipt {
            (
                receipt.confirmed_at_ms,
                receipt.amount_micro,
                format!(
                    "sui-native-transfer:testnet:{}:{}",
                    receipt.chain_identifier, receipt.digest
                ),
                None,
                Some(SuiNativeTransferRevenueEvidence {
                    provider: "sui_native_usdc_transfer".into(),
                    network: "sui:testnet".into(),
                    receipt: receipt.clone(),
                }),
            )
        } else {
            return Err("billingProvenanceUsdcReceiptMissing".into());
        };
    if order.account_ref != account_ref(owner, order.character_index)
        || paid_at_ms > issued_at_ms
        || order
            .applied
            .as_ref()
            .is_none_or(|r| r.at_ms > issued_at_ms)
    {
        return Err("billingProvenanceMonthlyPaymentTimeInvalid".into());
    }
    let eligible = !livemode && order.state == UsdcOrderState::Applied;
    Ok(MonthlyRevenuePart {
        source_order_id: order.id.clone(),
        lot_id: part.lot_id.clone(),
        credits: part.credits,
        credits_offset: part.credits_offset,
        original_currency: "testnet-usdc".into(),
        original_amount_minor: order.offer.amount_minor,
        // No cash tax claim is made: this is the explicit Testnet unit convention.
        indirect_tax_minor: if !livemode { Some(0) } else { None },
        eligible_micro_usd: if eligible {
            portion(
                amount_micro,
                part.credits_offset,
                part.credits,
                order.offer.credits,
            )?
        } else {
            0
        },
        reversed_micro_usd: 0,
        paid_at_ms,
        fx_snapshot_ref: "testnet-native-usdc6:1testUSDC=1000000testMicroUSD:v1".into(),
        evidence_ref,
        funding_status: if eligible { "verified_paid" } else { "pending" }.into(),
        reason_code: if livemode {
            "payment_environment_mismatch"
        } else if !eligible {
            "payment_review_evidence_pending"
        } else {
            "verified_paid_credit_fifo"
        }
        .into(),
        source: invoice_source,
        native_transfer_source: transfer_source,
    })
}

pub fn monthly_revenue_sources(
    account: &AccountRecord,
    owner: &str,
    livemode: bool,
) -> Result<Vec<MonthlyRevenueSource>, String> {
    let Some(ledger) = &account.billing else {
        return Ok(Vec::new());
    };
    ledger.validate()?;
    if let Some(monthly) = &account.monthly_card {
        monthly.validate()?;
    }
    let mut result = Vec::new();
    for receipt in account
        .monthly_card
        .iter()
        .flat_map(|c| c.item_receipts.values())
    {
        let (Some(entitlement_at), Some(service_end)) =
            (receipt.redeemed_at_ms, receipt.credited_until_ms)
        else {
            continue;
        };
        let service_start = service_end
            .checked_sub(crate::monthly_card::MONTHLY_CARD_DURATION_MS)
            .ok_or("billingProvenanceMonthlyInvalid")?;
        if receipt.account_id != owner || service_start < entitlement_at {
            return Err("billingProvenanceMonthlyMismatch".into());
        }
        let mut source = MonthlyRevenueSource {
            unique_id: receipt.unique_id,
            owner_account_id: owner.into(),
            payer_account_id: owner.into(),
            character_index: receipt.character_index,
            entitlement_at_ms: entitlement_at,
            service_start_ms: service_start,
            service_end_ms: service_end,
            paid_at_ms: 0,
            eligible_micro_usd: 0,
            reversed_micro_usd: 0,
            unknown_credits: 0,
            gift_credits: 0,
            parts: Vec::new(),
            funding_status: "pending".into(),
            reason_code: "historical_funding_unknown".into(),
            source_evidence_ref: format!("monthly-item:{}", receipt.unique_id),
            source_observed_at_ms: receipt.redeemed_at_ms.unwrap_or(receipt.issued_at_ms),
        };
        let Some(allocation) = ledger.provenance.monthly_items.get(&receipt.unique_id) else {
            source.unknown_credits = receipt.credit_price;
            result.push(source);
            continue;
        };
        if allocation.owner_account_id != owner
            || allocation.character_index != receipt.character_index
            || allocation.credit_price != receipt.credit_price
            || allocation.issued_at_ms != receipt.issued_at_ms
        {
            return Err("billingProvenanceMonthlyMismatch".into());
        }
        source.payer_account_id = allocation.payer_account_id.clone();
        if allocation.gift || allocation.owner_account_id != allocation.payer_account_id {
            source.funding_status = "excluded".into();
            source.reason_code = "gift_or_transferred_subscription".into();
            source.gift_credits = receipt.credit_price;
            result.push(source);
            continue;
        }
        let mut pending = false;
        for part in &allocation.parts {
            if part.kind != CreditFundingKind::Paid {
                match part.kind {
                    CreditFundingKind::Gift => {
                        source.gift_credits = source
                            .gift_credits
                            .checked_add(part.credits)
                            .ok_or("billingProvenanceOverflow")?
                    }
                    _ => {
                        source.unknown_credits = source
                            .unknown_credits
                            .checked_add(part.credits)
                            .ok_or("billingProvenanceOverflow")?
                    }
                }
                continue;
            }
            let order_id = part
                .source_order_id
                .as_ref()
                .ok_or("billingProvenanceOrderMissing")?;
            if let Some(order) = ledger.sui_usdc_orders.get(order_id) {
                let output =
                    sui_monthly_revenue_part(order, part, owner, receipt.issued_at_ms, livemode)?;
                if output.funding_status != "verified_paid" {
                    pending = true;
                }
                source.source_observed_at_ms = source
                    .source_observed_at_ms
                    .max(order.applied.as_ref().map_or(0, |r| r.at_ms));
                source.paid_at_ms = source.paid_at_ms.max(output.paid_at_ms);
                source.eligible_micro_usd = source
                    .eligible_micro_usd
                    .checked_add(output.eligible_micro_usd)
                    .ok_or("billingProvenanceOverflow")?;
                source.parts.push(output);
                continue;
            }
            let order = ledger
                .orders
                .get(order_id)
                .ok_or("billingProvenanceOrderMissing")?;
            let mut output = MonthlyRevenuePart {
                source_order_id: order_id.clone(),
                lot_id: part.lot_id.clone(),
                credits: part.credits,
                credits_offset: part.credits_offset,
                original_currency: order.offer.currency.clone(),
                original_amount_minor: order.offer.amount_minor,
                indirect_tax_minor: None,
                eligible_micro_usd: 0,
                reversed_micro_usd: 0,
                paid_at_ms: order.paid_at_ms.unwrap_or(0),
                fx_snapshot_ref: String::new(),
                evidence_ref: format!("recharge:{order_id}"),
                funding_status: "pending".into(),
                reason_code: "payment_revenue_evidence_missing".into(),
                source: None,
                native_transfer_source: None,
            };
            if order.livemode != livemode {
                output.reason_code = "payment_environment_mismatch".into();
                pending = true;
                source.parts.push(output);
                continue;
            }
            let Some(payment) = ledger.provenance.payments.get(order_id) else {
                pending = true;
                source.parts.push(output);
                continue;
            };
            output.indirect_tax_minor = payment.indirect_tax_minor;
            source.source_observed_at_ms = source.source_observed_at_ms.max(payment.observed_at_ms);
            output.evidence_ref = payment.evidence_ref.clone();
            output.paid_at_ms = payment.paid_at_ms;
            source.paid_at_ms = source.paid_at_ms.max(payment.paid_at_ms);
            if payment.paid_at_ms > receipt.issued_at_ms
                || order.applied.is_none()
                || !matches!(
                    order.state,
                    RechargeOrderState::Applied | RechargeOrderState::Review
                )
            {
                return Err("billingProvenanceMonthlyPaymentTimeInvalid".into());
            }
            let Some(tax) = payment.indirect_tax_minor else {
                output.reason_code = "payment_tax_unverified".into();
                pending = true;
                source.parts.push(output);
                continue;
            };
            if payment.currency != "usd" {
                output.reason_code = "payment_fx_unapproved".into();
                pending = true;
                source.parts.push(output);
                continue;
            }
            output.fx_snapshot_ref = "iso4217:usd:minor-2:1usd=1000000microUSD:v1".into();
            let total_net = payment
                .gross_amount_minor
                .checked_sub(tax)
                .and_then(|a| a.checked_mul(10_000))
                .ok_or("billingProvenanceOverflow")?;
            output.eligible_micro_usd = portion(
                total_net,
                part.credits_offset,
                part.credits,
                order.offer.credits,
            )?;
            let adverse: Vec<_> = ledger
                .provenance
                .adverse_events
                .values()
                .filter(|e| e.order_id == *order_id)
                .collect();
            let unproven_review = order.review_events.keys().any(|event_id| {
                !adverse.iter().any(|e| {
                    e.source_event_id == *event_id
                        || e.source_event_id
                            .strip_prefix(event_id)
                            .is_some_and(|tail| tail.starts_with(':'))
                })
            });
            let refunded = adverse
                .iter()
                .map(|e| e.cumulative_refunded_minor)
                .max()
                .unwrap_or(0);
            source.source_observed_at_ms = source
                .source_observed_at_ms
                .max(adverse.iter().map(|e| e.observed_at_ms).max().unwrap_or(0));
            let dispute = adverse
                .iter()
                .filter(|e| e.dispute_status != "none")
                .max_by_key(|e| (e.observed_at_ms, e.occurred_at_ms, &e.source_event_id));
            let lost = dispute
                .filter(|e| e.dispute_status == "lost")
                .map_or(0, |e| e.disputed_minor);
            // A refund and a lost dispute can refer to overlapping money.
            // Their amounts alone do not prove additive cash loss: retain the
            // proven refund and hold this source for provider reconciliation.
            let overlap_requires_review = refunded > 0 && lost > 0;
            let reversal_minor = if overlap_requires_review {
                refunded
            } else {
                refunded.saturating_add(lost)
            }
            .min(payment.gross_amount_minor);
            // Apply cumulative provider refunds to this frozen original lot
            // cost. Repartitioning the rounded refunded pool by credit offset
            // can make a slice's reversal decrease as the refund grows.
            output.reversed_micro_usd = u64::try_from(
                u128::from(output.eligible_micro_usd) * u128::from(reversal_minor)
                    / u128::from(payment.gross_amount_minor),
            )
            .map_err(|_| "billingProvenanceOverflow")?;
            if unproven_review {
                output.reason_code = "payment_review_evidence_pending".into();
                pending = true;
            } else if overlap_requires_review {
                output.reason_code = "payment_refund_dispute_reconciliation_required".into();
                pending = true;
            } else if dispute.is_some_and(|e| {
                matches!(
                    e.dispute_status.as_str(),
                    "needs_response"
                        | "under_review"
                        | "warning_needs_response"
                        | "warning_under_review"
                )
            }) {
                output.reason_code = "payment_dispute_under_review".into();
                pending = true;
            } else {
                output.funding_status = "verified_paid".into();
                output.reason_code = "verified_paid_credit_fifo".into();
            }
            source.eligible_micro_usd = source
                .eligible_micro_usd
                .checked_add(output.eligible_micro_usd)
                .ok_or("billingProvenanceOverflow")?;
            source.reversed_micro_usd = source
                .reversed_micro_usd
                .checked_add(output.reversed_micro_usd)
                .ok_or("billingProvenanceOverflow")?;
            source.parts.push(output);
        }
        if pending {
            source.reason_code = "payment_source_review_required".into();
        } else if source.eligible_micro_usd > 0 {
            source.funding_status = "verified_paid".into();
            source.reason_code = if source.unknown_credits > 0 {
                "mixed_paid_unknown_credit"
            } else if source.gift_credits > 0 {
                "mixed_paid_gift_credit"
            } else {
                "verified_paid_credit_fifo"
            }
            .into();
        } else if source.gift_credits > 0 && source.unknown_credits == 0 {
            source.funding_status = "excluded".into();
            source.reason_code = "gift_credit_not_cash".into();
        }
        result.push(source);
    }
    result.sort_by_key(|s| (s.entitlement_at_ms, s.unique_id));
    Ok(result)
}

#[cfg(test)]
#[path = "billing_provenance_tests.rs"]
mod tests;

//! Pure procurement package pricing and source attribution.
//!
//! The reference is an existing, explicitly selected RechargeOffer, never a
//! hardcoded USD-to-Credits ratio. This module performs no recharge, credit
//! deduction, quota grant, database write or payment-provider call. A quote is
//! not an authorization to enable production sales.
use crate::billing::{RechargeOffer, RechargeOrder, RechargeOrderState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PricingRule {
    pub version: u64,
    pub package_units: u32,
    /// Proposed total package price in the selected currency's minor units.
    /// No default USD price or exchange ratio is supplied.
    pub suggested_total_minor: u64,
    /// Same canonical lowercase ISO currency as the existing RechargeOffer.
    pub suggested_currency: String,
    /// Explicit choice is required when recharge packages have different rates.
    pub reference_offer_id: String,
    pub enabled: bool,
    /// Explicit approval applies to this rule version. Configuration authors
    /// must increase `version` whenever price, source or refund terms change.
    pub approved_rule_version: Option<u64>,
    /// None means no expiry. Zero-length validity is rejected.
    pub quota_valid_for_ms: Option<u64>,
    pub refund_policy: RefundPolicy,
}

impl Default for PricingRule {
    fn default() -> Self {
        Self {
            version: 0,
            package_units: 0,
            suggested_total_minor: 0,
            suggested_currency: String::new(),
            reference_offer_id: String::new(),
            enabled: false,
            approved_rule_version: None,
            quota_valid_for_ms: None,
            refund_policy: RefundPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefundPolicy {
    pub refund_failed_publication: bool,
    pub refund_cancel_after_publication: bool,
    pub refund_expiry_after_publication: bool,
    pub charge_technical_retry: bool,
}
impl Default for RefundPolicy {
    fn default() -> Self {
        Self {
            refund_failed_publication: true,
            refund_cancel_after_publication: false,
            refund_expiry_after_publication: false,
            charge_technical_retry: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CreditOrigin {
    Paid,
    Gift,
    Test,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePriceQuote {
    pub rule_version: u64,
    pub package_units: u32,
    pub credit_cost: u32,
    pub target_amount_minor: u64,
    pub currency: String,
    /// Complete reference snapshot, so future recharge catalogue edits cannot
    /// silently change the ratio used for an already confirmed package.
    pub reference_offer: RechargeOffer,
    pub required_credit_origin: CreditOrigin,
    pub quota_expires_at_ms: Option<u64>,
    pub refund_policy: RefundPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PricingError {
    Disabled,
    ApprovalRequired,
    ApprovalVersionMismatch,
    InvalidRule,
    ReferenceOfferMissing,
    AmbiguousReferenceOffer,
    InvalidReferenceOffer,
    CurrencyMismatch,
    CreditCostOverflow,
    QuotaExpiryOverflow,
}

/// Integer quote calculation only. The complete package is rounded up once;
/// rounding each publication separately would accidentally overcharge bundles.
/// Callers must use `quote_package` for the explicit sale/approval gates.
pub fn credits_for_minor_amount(
    amount_minor: u64,
    currency: &str,
    offer: &RechargeOffer,
) -> Result<u32, PricingError> {
    offer
        .validate()
        .map_err(|_| PricingError::InvalidReferenceOffer)?;
    if currency != offer.currency {
        return Err(PricingError::CurrencyMismatch);
    }
    if amount_minor == 0 {
        return Err(PricingError::InvalidRule);
    }
    let numerator = (amount_minor as u128)
        .checked_mul(offer.credits as u128)
        .ok_or(PricingError::CreditCostOverflow)?;
    let denominator = offer.amount_minor as u128;
    // Avoid the usual `(numerator + denominator - 1)` overflow pattern.
    let cost = (numerator / denominator)
        .checked_add(u128::from(numerator % denominator != 0))
        .ok_or(PricingError::CreditCostOverflow)?;
    u32::try_from(cost).map_err(|_| PricingError::CreditCostOverflow)
}

/// Produce a versioned, source-pinned price only after both independent gates
/// are enabled. Suggested prices remain non-saleable without explicit approval.
pub fn quote_package(
    rule: &PricingRule,
    offers: &[RechargeOffer],
    now_ms: u64,
) -> Result<PackagePriceQuote, PricingError> {
    if !rule.enabled {
        return Err(PricingError::Disabled);
    }
    let approved = rule
        .approved_rule_version
        .ok_or(PricingError::ApprovalRequired)?;
    if approved != rule.version {
        return Err(PricingError::ApprovalVersionMismatch);
    }
    if rule.version == 0
        || rule.package_units == 0
        || rule.suggested_total_minor == 0
        || rule.suggested_currency.len() != 3
        || !rule
            .suggested_currency
            .bytes()
            .all(|b| b.is_ascii_lowercase())
        || rule.reference_offer_id.is_empty()
        || rule.reference_offer_id.len() > 64
        || !rule
            .reference_offer_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
        || rule.quota_valid_for_ms == Some(0)
        || !rule.refund_policy.refund_failed_publication
        || rule.refund_policy.charge_technical_retry
    {
        return Err(PricingError::InvalidRule);
    }
    let mut references = offers
        .iter()
        .filter(|offer| offer.id == rule.reference_offer_id);
    let offer = references
        .next()
        .ok_or(PricingError::ReferenceOfferMissing)?;
    if references.next().is_some() {
        return Err(PricingError::AmbiguousReferenceOffer);
    }
    let credit_cost =
        credits_for_minor_amount(rule.suggested_total_minor, &rule.suggested_currency, offer)?;
    let quota_expires_at_ms = rule
        .quota_valid_for_ms
        .map(|ttl| {
            now_ms
                .checked_add(ttl)
                .ok_or(PricingError::QuotaExpiryOverflow)
        })
        .transpose()?;
    Ok(PackagePriceQuote {
        rule_version: rule.version,
        package_units: rule.package_units,
        credit_cost,
        target_amount_minor: rule.suggested_total_minor,
        currency: rule.suggested_currency.clone(),
        reference_offer: offer.clone(),
        required_credit_origin: CreditOrigin::Paid,
        quota_expires_at_ms,
        refund_policy: rule.refund_policy,
    })
}

/// Classification of an already existing credit batch. Consumption of prepaid
/// Credits is not a second Stripe cash receipt; the existing recharge ledger
/// remains the source of cash receipts. No balance or revenue ledger is created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreditSourcePolicy {
    pub effective_origin: CreditOrigin,
    pub can_fund_paid_package: bool,
    pub can_fund_test_package: bool,
    pub cash_backed_service_consumption: bool,
    pub recognize_new_cash_receipt: bool,
}

fn valid_applied_recharge(order: &RechargeOrder) -> bool {
    let Some(applied) = &order.applied else {
        return false;
    };
    let Some(paid_at) = order.paid_at_ms else {
        return false;
    };
    order.state == RechargeOrderState::Applied
        && order.offer.validate().is_ok()
        && !order.id.is_empty()
        && order
            .checkout_session_id
            .as_ref()
            .is_some_and(|id| !id.is_empty())
        && order
            .payment_intent_id
            .as_ref()
            .is_some_and(|id| !id.is_empty())
        && applied.character_index == order.character_index
        && applied.credits == order.offer.credits
        && applied.credit_before.checked_add(applied.credits) == Some(applied.credit_after)
        && paid_at >= order.created_at_ms
        && applied.at_ms >= paid_at
}

/// `recharge` must be the batch's source reference read from the existing
/// authenticated billing ledger, never a client-provided receipt. A Paid label
/// alone cannot promote gifted, testing, unverified or legacy Credits to cash.
pub fn credit_source_policy(
    declared_origin: CreditOrigin,
    recharge: Option<&RechargeOrder>,
) -> CreditSourcePolicy {
    let effective_origin = match declared_origin {
        CreditOrigin::Paid => match recharge.filter(|order| valid_applied_recharge(order)) {
            Some(order) if order.livemode => CreditOrigin::Paid,
            Some(_) => CreditOrigin::Test,
            None => CreditOrigin::Unknown,
        },
        other => other,
    };
    CreditSourcePolicy {
        effective_origin,
        can_fund_paid_package: effective_origin == CreditOrigin::Paid,
        can_fund_test_package: effective_origin == CreditOrigin::Test,
        cash_backed_service_consumption: effective_origin == CreditOrigin::Paid,
        recognize_new_cash_receipt: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn offer(id: &str, minor: u64, credits: u32) -> RechargeOffer {
        RechargeOffer {
            id: id.into(),
            label: "Existing recharge fixture".into(),
            currency: "usd".into(),
            amount_minor: minor,
            credits,
        }
    }
    fn approved_rule() -> PricingRule {
        PricingRule {
            version: 7,
            package_units: 10,
            suggested_total_minor: 100,
            suggested_currency: "usd".into(),
            reference_offer_id: "existing".into(),
            enabled: true,
            approved_rule_version: Some(7),
            quota_valid_for_ms: Some(86_400_000),
            refund_policy: RefundPolicy::default(),
        }
    }
    #[test]
    fn procurement_fees_use_actual_selected_offer_ratio_not_a_usd_constant() {
        let rule = approved_rule();
        assert_eq!(
            quote_package(&rule, &[offer("existing", 1000, 1000)], 100)
                .unwrap()
                .credit_cost,
            100
        );
        assert_eq!(
            quote_package(&rule, &[offer("existing", 1000, 1500)], 100)
                .unwrap()
                .credit_cost,
            150
        );
        assert_eq!(
            quote_package(&rule, &[offer("existing", 1200, 997)], 100)
                .unwrap()
                .credit_cost,
            84
        );
        let quote = quote_package(
            &rule,
            &[offer("other", 1, 4000), offer("existing", 1000, 1500)],
            100,
        )
        .unwrap();
        assert_eq!(quote.reference_offer.id, "existing");
        assert_eq!(quote.credit_cost, 150);
        assert_eq!(quote.quota_expires_at_ms, Some(86_400_100));
        assert_eq!(quote.rule_version, 7);
    }
    #[test]
    fn procurement_fees_round_whole_package_once_and_reject_integer_overflow() {
        let reference = offer("existing", 300, 100);
        assert_eq!(credits_for_minor_amount(3, "usd", &reference), Ok(1));
        assert_eq!(credits_for_minor_amount(4, "usd", &reference), Ok(2));
        let mut rule = approved_rule();
        rule.suggested_total_minor = 10;
        assert_eq!(
            quote_package(&rule, &[reference], 100).unwrap().credit_cost,
            4
        );
        assert_eq!(
            credits_for_minor_amount(u64::MAX, "usd", &offer("existing", u64::MAX, u32::MAX)),
            Ok(u32::MAX)
        );
        assert_eq!(
            credits_for_minor_amount(u64::MAX, "usd", &offer("existing", 1, u32::MAX)),
            Err(PricingError::CreditCostOverflow)
        );
        assert_eq!(
            quote_package(&approved_rule(), &[offer("existing", 1000, 1000)], u64::MAX),
            Err(PricingError::QuotaExpiryOverflow)
        );
    }
    #[test]
    fn procurement_fees_default_disabled_and_require_explicit_version_approval() {
        assert!(!PricingRule::default().enabled);
        assert_eq!(
            quote_package(&PricingRule::default(), &[], 0),
            Err(PricingError::Disabled)
        );
        let mut rule = approved_rule();
        rule.approved_rule_version = None;
        assert_eq!(
            quote_package(&rule, &[offer("existing", 1000, 1000)], 0),
            Err(PricingError::ApprovalRequired)
        );
        rule.approved_rule_version = Some(6);
        assert_eq!(
            quote_package(&rule, &[offer("existing", 1000, 1000)], 0),
            Err(PricingError::ApprovalVersionMismatch)
        );
        rule = approved_rule();
        rule.refund_policy.charge_technical_retry = true;
        assert_eq!(
            quote_package(&rule, &[offer("existing", 1000, 1000)], 0),
            Err(PricingError::InvalidRule)
        );
        rule = approved_rule();
        rule.refund_policy.refund_failed_publication = false;
        assert_eq!(
            quote_package(&rule, &[offer("existing", 1000, 1000)], 0),
            Err(PricingError::InvalidRule)
        );
    }
    #[test]
    fn procurement_fees_missing_ambiguous_invalid_offer_or_currency_never_produces_sale_quote() {
        let rule = approved_rule();
        assert_eq!(
            quote_package(&rule, &[], 0),
            Err(PricingError::ReferenceOfferMissing)
        );
        let duplicate = offer("existing", 1000, 1000);
        assert_eq!(
            quote_package(&rule, &[duplicate.clone(), duplicate], 0),
            Err(PricingError::AmbiguousReferenceOffer)
        );
        assert_eq!(
            quote_package(&rule, &[offer("existing", 0, 1000)], 0),
            Err(PricingError::InvalidReferenceOffer)
        );
        let mut wrong_currency = offer("existing", 1000, 1000);
        wrong_currency.currency = "eur".into();
        assert_eq!(
            quote_package(&rule, &[wrong_currency], 0),
            Err(PricingError::CurrencyMismatch)
        );
        assert_eq!(
            credits_for_minor_amount(100, "USD", &offer("existing", 1000, 1000)),
            Err(PricingError::CurrencyMismatch)
        );
    }
    fn applied_recharge(livemode: bool) -> RechargeOrder {
        RechargeOrder {
            id: "recharge-source".into(),
            request_id: "request-source".into(),
            character_index: 3,
            offer: offer("existing", 1000, 1000),
            created_at_ms: 10,
            livemode,
            checkout_session_id: Some("cs_verified".into()),
            payment_intent_id: Some("pi_verified".into()),
            state: RechargeOrderState::Applied,
            paid_at_ms: Some(20),
            applied: Some(crate::billing::RechargeAppliedReceipt {
                character_index: 3,
                credits: 1000,
                credit_before: 100,
                credit_after: 1100,
                revision: 1,
                at_ms: 30,
            }),
            review_events: Default::default(),
        }
    }
    #[test]
    fn procurement_fees_free_testing_and_unknown_batches_never_become_cash() {
        let live = applied_recharge(true);
        for origin in [
            CreditOrigin::Gift,
            CreditOrigin::Test,
            CreditOrigin::Unknown,
        ] {
            let policy = credit_source_policy(origin, Some(&live));
            assert_eq!(policy.effective_origin, origin);
            assert!(!policy.cash_backed_service_consumption);
            assert!(!policy.can_fund_paid_package);
            assert!(!policy.recognize_new_cash_receipt);
        }
        let policy = credit_source_policy(CreditOrigin::Paid, Some(&applied_recharge(false)));
        assert_eq!(policy.effective_origin, CreditOrigin::Test);
        assert!(policy.can_fund_test_package);
        assert!(!policy.cash_backed_service_consumption);
        assert!(!policy.can_fund_paid_package);
        let policy = credit_source_policy(CreditOrigin::Paid, None);
        assert_eq!(policy.effective_origin, CreditOrigin::Unknown);
        assert!(!policy.can_fund_paid_package);
    }
    #[test]
    fn procurement_fees_only_existing_verified_live_applied_receipt_backs_paid_consumption() {
        let live = applied_recharge(true);
        let policy = credit_source_policy(CreditOrigin::Paid, Some(&live));
        assert!(policy.can_fund_paid_package);
        assert!(policy.cash_backed_service_consumption);
        assert!(
            !policy.recognize_new_cash_receipt,
            "prepaid service consumption is not another Stripe receipt"
        );
        for mutation in 0..5 {
            let mut bad = live.clone();
            match mutation {
                0 => bad.state = RechargeOrderState::Prepared,
                1 => bad.applied = None,
                2 => bad.payment_intent_id = None,
                3 => bad.applied.as_mut().unwrap().credit_after = 1099,
                _ => bad.applied.as_mut().unwrap().character_index = 4,
            }
            let policy = credit_source_policy(CreditOrigin::Paid, Some(&bad));
            assert_eq!(policy.effective_origin, CreditOrigin::Unknown);
            assert!(!policy.can_fund_paid_package);
        }
    }
}

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::ALGORITHM_VERSION;

pub const MICRO_USD_PER_CENT: u64 = 10_000;
pub const BASIS_POINTS: u64 = 10_000;
pub const TESTNET_USDC_COIN_TYPE: &str =
    "0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC";
pub const TESTNET_CHAIN_IDENTIFIER: &str = "69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD";

fn wallet_enrollment_disabled(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionRules {
    /// Explicit test-only enrollment using a verified wallet, without a game role.
    /// This never supplies gameplay qualification or payment evidence.
    #[serde(default, skip_serializing_if = "wallet_enrollment_disabled")]
    pub test_wallet_enrollment: bool,
    /// Explicit short Testnet acceptance only; unchanged legacy cent rounding by default.
    #[serde(default, skip_serializing_if = "wallet_enrollment_disabled")]
    pub test_usdc_micro_payouts: bool,
    /// Campaign-scoped first-address exception; never changes the global address record.
    #[serde(default, skip_serializing_if = "wallet_enrollment_disabled")]
    pub test_first_payout_address_no_cooldown: bool,
    pub minimum_level: u16,
    pub active_utc_days: u16,
    pub candidate_ttl_ms: u64,
    pub confirmation_window_ms: u64,
    pub maximum_coverage_ms: u64,
    pub reserve_bps: u16,
    pub wallet_cooldown_ms: u64,
}

impl Default for PromotionRules {
    /// Engineering defaults are draft only. They cannot enable a campaign.
    fn default() -> Self {
        Self {
            test_wallet_enrollment: false,
            test_usdc_micro_payouts: false,
            test_first_payout_address_no_cooldown: false,
            minimum_level: 30,
            active_utc_days: 3,
            candidate_ttl_ms: 7 * 86_400_000,
            confirmation_window_ms: 86_400_000,
            maximum_coverage_ms: 92 * 86_400_000,
            reserve_bps: 1000,
            wallet_cooldown_ms: 72 * 3_600_000,
        }
    }
}

impl PromotionRules {
    pub fn validate(&self) -> Result<(), String> {
        if self.minimum_level == 0
            || self.active_utc_days == 0
            || self.active_utc_days > 366
            || self.candidate_ttl_ms == 0
            || self.confirmation_window_ms == 0
            || self.maximum_coverage_ms == 0
            || self.reserve_bps > 10_000
            || self.wallet_cooldown_ms < 72 * 3_600_000
            || (self.test_first_payout_address_no_cooldown && !self.test_usdc_micro_payouts)
        {
            return Err("promotionRulesInvalid".into());
        }
        Ok(())
    }
    pub fn validate_environment(&self, environment: &str) -> Result<(), String> {
        self.validate()?;
        if environment != "test"
            && (self.test_usdc_micro_payouts || self.test_first_payout_address_no_cooldown)
        {
            return Err("promotionEnvironmentMismatch".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevenueInput {
    pub allocation_id: String,
    pub paid_at_ms: u64,
    /// Committed entitlement allocation, distinct from its funding payment.
    #[serde(default)]
    pub entitlement_at_ms: u64,
    pub service_start_ms: u64,
    pub service_end_ms: u64,
    /// Verified net source payment allocated to this full service period.
    pub amount_micro_usd: u64,
    /// Reversals retain the original order's fixed FX rate.
    pub reversed_micro_usd: u64,
    pub is_card_payment: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemberInput {
    pub account_id: String,
    pub level_completed_at_ms: Option<u64>,
    pub play_completed_at_ms: Option<u64>,
    #[serde(default)]
    pub established_qualification_at_ms: Option<u64>,
    pub excluded: bool,
    pub revenue: Vec<RevenueInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommissionInput {
    pub environment: String,
    pub campaign_id: String,
    pub terms_version: String,
    pub starts_at_ms: u64,
    pub ends_at_ms: u64,
    pub maximum_coverage_ms: u64,
    pub per_member_revenue_cap_micro_usd: u64,
    pub reserve_bps: u16,
    pub members: Vec<MemberInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberCommission {
    pub account_id: String,
    pub qualified_at_ms: u64,
    pub rank: u64,
    pub rate_bps: u16,
    pub eligible_revenue_micro_usd: u64,
    pub commission_micro_usd: u64,
    /// Exact numerator over 10,000 microUSD. Preserved for aggregate rounding.
    pub commission_numerator: String,
    pub card_commission_numerator: String,
    pub allocation_ids: Vec<String>,
    #[serde(default)]
    pub allocation_basis: Vec<AllocationBasis>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationBasis {
    pub allocation_id: String,
    pub service_duration_ms: u64,
    pub covered_ms: u64,
    pub original_amount_micro_usd: u64,
    pub reversed_at_snapshot_micro_usd: u64,
    pub eligible_at_snapshot_micro_usd: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommissionResult {
    pub algorithm_version: String,
    pub members: Vec<MemberCommission>,
    pub eligible_revenue_micro_usd: u64,
    pub commission_micro_usd: u64,
    pub commission_numerator: String,
    pub total_cents: u64,
    pub reserve_micro_usd: u64,
    pub payable_cents: u64,
    /// Present only for the explicitly approved Testnet micro payout policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payable_micro_usd: Option<u64>,
}

fn u64_from(value: u128) -> Result<u64, String> {
    value
        .try_into()
        .map_err(|_| "promotionAmountOverflow".into())
}

/// Inclusive start, exclusive end; all time arithmetic is integer UTC millis.
/// Occupied intervals come from earlier immutable allocation IDs, not arrival order.
fn uncovered_ranges(start: u64, end: u64, occupied: &[(u64, u64)]) -> Vec<(u64, u64)> {
    let mut cursor = start;
    let mut output = Vec::new();
    for &(left, right) in occupied {
        if right <= cursor || left >= end {
            continue;
        }
        if left > cursor {
            output.push((cursor, left.min(end)));
        }
        cursor = cursor.max(right);
        if cursor >= end {
            break;
        }
    }
    if cursor < end {
        output.push((cursor, end));
    }
    output
}

/// Calculate a season without external state or floating point. This function
/// never treats a recharge as a subscription. Inputs must already be backed by
/// authenticated paid-lot/beneficiary evidence in the repository.
pub fn calculate_commission(input: &CommissionInput) -> Result<CommissionResult, String> {
    calculate_commission_precision(input, false)
}

/// Preserve genuine service-period proration, qualification and rates. Only the
/// final Testnet payout precision changes; this never creates minimum rewards.
pub fn calculate_test_usdc_micro_commission(
    input: &CommissionInput,
) -> Result<CommissionResult, String> {
    if input.environment != "test" {
        return Err("promotionEnvironmentMismatch".into());
    }
    calculate_commission_precision(input, true)
}

fn calculate_commission_precision(
    input: &CommissionInput,
    micro: bool,
) -> Result<CommissionResult, String> {
    if input.starts_at_ms == 0
        || input.ends_at_ms <= input.starts_at_ms
        || input.maximum_coverage_ms == 0
        || input.reserve_bps > 10_000
        || !matches!(
            input.environment.as_str(),
            "development" | "test" | "production"
        )
        || input.per_member_revenue_cap_micro_usd == 0
    {
        return Err("promotionCalculationInputInvalid".into());
    }
    let mut accounts = BTreeSet::new();
    let mut allocations = BTreeSet::new();
    let mut eligible = Vec::new();
    for member in &input.members {
        if member.account_id.is_empty() || !accounts.insert(&member.account_id) {
            return Err("promotionDuplicateAccount".into());
        }
        for item in &member.revenue {
            if item.allocation_id.is_empty() || !allocations.insert(&item.allocation_id) {
                return Err("promotionDuplicateAllocation".into());
            }
            if item.service_end_ms <= item.service_start_ms
                || item.reversed_micro_usd > item.amount_micro_usd
            {
                return Err("promotionRevenueInputInvalid".into());
            }
        }
        if member.excluded {
            continue;
        }
        let (Some(level), Some(play)) = (member.level_completed_at_ms, member.play_completed_at_ms)
        else {
            continue;
        };
        if !(input.starts_at_ms..input.ends_at_ms).contains(&level)
            || !(input.starts_at_ms..input.ends_at_ms).contains(&play)
        {
            continue;
        }
        let mut sorted = member.revenue.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| {
            (a.service_start_ms, &a.allocation_id).cmp(&(b.service_start_ms, &b.allocation_id))
        });
        let mut occupied = Vec::new();
        let mut coverage_left = input.maximum_coverage_ms;
        let mut revenue_left = input.per_member_revenue_cap_micro_usd;
        let mut amount = 0u64;
        let mut card_amount = 0u64;
        let mut used = Vec::new();
        let mut basis = Vec::new();
        let mut first_payment: Option<u64> = None;
        for item in sorted {
            if coverage_left == 0 || revenue_left == 0 {
                break;
            }
            if !(input.starts_at_ms..input.ends_at_ms).contains(&item.paid_at_ms)
                || !(input.starts_at_ms..input.ends_at_ms).contains(&item.entitlement_at_ms)
                || item.amount_micro_usd == item.reversed_micro_usd
            {
                continue;
            }
            let start = item.service_start_ms.max(input.starts_at_ms);
            let end = item.service_end_ms.min(input.ends_at_ms);
            if end <= start {
                continue;
            }
            let mut covered = 0u64;
            for (left, right) in uncovered_ranges(start, end, &occupied) {
                let duration = (right - left).min(coverage_left);
                if duration == 0 {
                    break;
                }
                occupied.push((left, left + duration));
                coverage_left -= duration;
                covered += duration;
            }
            occupied.sort_unstable();
            let full = u128::from(item.amount_micro_usd - item.reversed_micro_usd);
            let prorated = u64_from(
                full * u128::from(covered)
                    / u128::from(item.service_end_ms - item.service_start_ms),
            )?
            .min(revenue_left);
            amount = amount
                .checked_add(prorated)
                .ok_or("promotionAmountOverflow")?;
            if item.is_card_payment {
                card_amount = card_amount
                    .checked_add(prorated)
                    .ok_or("promotionAmountOverflow")?;
            }
            revenue_left -= prorated;
            if prorated > 0 {
                first_payment = Some(
                    first_payment
                        .map_or(item.entitlement_at_ms, |at| at.min(item.entitlement_at_ms)),
                );
                basis.push(AllocationBasis {
                    allocation_id: item.allocation_id.clone(),
                    service_duration_ms: item.service_end_ms - item.service_start_ms,
                    covered_ms: covered,
                    original_amount_micro_usd: item.amount_micro_usd,
                    reversed_at_snapshot_micro_usd: item.reversed_micro_usd,
                    eligible_at_snapshot_micro_usd: prorated,
                });
                used.push(item.allocation_id.clone());
            }
        }
        if let Some(first_payment) = first_payment {
            let completed = level.max(play).max(first_payment);
            let established = member.established_qualification_at_ms.filter(|at| {
                (input.starts_at_ms..input.ends_at_ms).contains(at) && *at >= level.max(play)
            });
            eligible.push((
                established.map_or(completed, |at| at.min(completed)),
                member.account_id.clone(),
                amount,
                card_amount,
                used,
                basis,
            ));
        }
    }
    eligible.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    let mut total_revenue = 0u128;
    let mut numerator = 0u128;
    let mut card_numerator = 0u128;
    let mut members = Vec::new();
    for (index, (at, account, amount, card_amount, used, basis)) in eligible.into_iter().enumerate()
    {
        let rank = index as u64 + 1;
        let bps: u16 = if rank <= 50 {
            1000
        } else if rank <= 200 {
            2000
        } else {
            3000
        };
        let member_numerator = u128::from(amount) * u128::from(bps);
        let member_card_numerator = u128::from(card_amount) * u128::from(bps);
        numerator = numerator
            .checked_add(member_numerator)
            .ok_or("promotionAmountOverflow")?;
        card_numerator = card_numerator
            .checked_add(member_card_numerator)
            .ok_or("promotionAmountOverflow")?;
        total_revenue += u128::from(amount);
        members.push(MemberCommission {
            account_id: account,
            qualified_at_ms: at,
            rank,
            rate_bps: bps,
            eligible_revenue_micro_usd: amount,
            commission_micro_usd: u64_from(member_numerator / 10_000)?,
            commission_numerator: member_numerator.to_string(),
            card_commission_numerator: member_card_numerator.to_string(),
            allocation_ids: used,
            allocation_basis: basis,
        });
    }
    let commission_micro = u64_from(numerator / 10_000)?;
    // Hold fractional reserve conservatively; only the season's final payable
    // total is rounded to cents, half up, once.
    let reserve_numerator = card_numerator
        .checked_mul(u128::from(input.reserve_bps))
        .ok_or("promotionAmountOverflow")?
        / 10_000;
    let cent_denominator = 10_000u128 * 10_000;
    let total_cents = u64_from((numerator + cent_denominator / 2) / cent_denominator)?;
    // Round gross once. Conservatively reserve complete cents and subtract them
    // from that same rounded gross, so reserve + payable cannot create money.
    let reserve_cents = u64_from(reserve_numerator.div_ceil(cent_denominator))?.min(total_cents);
    let reserve_micro_usd = if micro {
        u64_from(
            card_numerator
                .checked_mul(u128::from(input.reserve_bps))
                .ok_or("promotionAmountOverflow")?
                .div_ceil(cent_denominator),
        )?
        .min(commission_micro)
    } else {
        reserve_cents
            .checked_mul(MICRO_USD_PER_CENT)
            .ok_or("promotionAmountOverflow")?
    };
    Ok(CommissionResult {
        algorithm_version: ALGORITHM_VERSION.into(),
        members,
        eligible_revenue_micro_usd: u64_from(total_revenue)?,
        commission_micro_usd: commission_micro,
        commission_numerator: numerator.to_string(),
        total_cents,
        reserve_micro_usd,
        payable_cents: total_cents - reserve_cents,
        payable_micro_usd: micro.then_some(commission_micro - reserve_micro_usd),
    })
}

/// Locked seasons never rerank unrelated members. Reverse only the original
/// member's original rate, capped by the still unreversed original commission.
pub fn locked_reversal_micro_usd(
    original_revenue_micro_usd: u64,
    reversed_revenue_micro_usd: u64,
    rate_bps: u16,
    previously_reversed_commission_micro_usd: u64,
) -> Result<u64, String> {
    if rate_bps > 3000 || reversed_revenue_micro_usd > original_revenue_micro_usd {
        return Err("promotionReversalInvalid".into());
    }
    let original =
        u64_from(u128::from(original_revenue_micro_usd) * u128::from(rate_bps) / 10_000)?;
    let reversal =
        u64_from(u128::from(reversed_revenue_micro_usd) * u128::from(rate_bps) / 10_000)?;
    if previously_reversed_commission_micro_usd > original {
        return Err("promotionReversalInvalid".into());
    }
    Ok(reversal
        .saturating_sub(previously_reversed_commission_micro_usd)
        .min(original - previously_reversed_commission_micro_usd))
}

/// Revalue only the covered slices/caps frozen into the original settlement.
/// Late allocations, current qualification changes and other members cannot
/// replace the old source income or change this member's locked rate.
pub fn frozen_member_reversal_micro_usd(
    member: &MemberCommission,
    cumulative_reversals: &BTreeMap<String, u64>,
    previously_reversed_micro_usd: u64,
) -> Result<u64, String> {
    if member.allocation_basis.is_empty() {
        return Err("promotionSnapshotBasisMissing".into());
    }
    let mut ids = BTreeSet::new();
    let mut original = 0u64;
    let mut remaining = 0u64;
    for basis in &member.allocation_basis {
        if !ids.insert(&basis.allocation_id)
            || basis.service_duration_ms == 0
            || basis.covered_ms > basis.service_duration_ms
            || basis.reversed_at_snapshot_micro_usd > basis.original_amount_micro_usd
        {
            return Err("promotionSnapshotInvalid".into());
        }
        let reversed = cumulative_reversals
            .get(&basis.allocation_id)
            .copied()
            .unwrap_or(basis.reversed_at_snapshot_micro_usd);
        if reversed < basis.reversed_at_snapshot_micro_usd
            || reversed > basis.original_amount_micro_usd
        {
            return Err("promotionReversalInvalid".into());
        }
        let current = u64_from(
            u128::from(basis.original_amount_micro_usd - reversed) * u128::from(basis.covered_ms)
                / u128::from(basis.service_duration_ms),
        )?
        .min(basis.eligible_at_snapshot_micro_usd);
        original = original
            .checked_add(basis.eligible_at_snapshot_micro_usd)
            .ok_or("promotionAmountOverflow")?;
        remaining = remaining
            .checked_add(current)
            .ok_or("promotionAmountOverflow")?;
    }
    if original != member.eligible_revenue_micro_usd {
        return Err("promotionSnapshotInvalid".into());
    }
    locked_reversal_micro_usd(
        original,
        original
            .checked_sub(remaining)
            .ok_or("promotionSnapshotInvalid")?,
        member.rate_bps,
        previously_reversed_micro_usd,
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreditLot {
    pub lot_id: String,
    pub issued_at_ms: u64,
    pub original_credits: u64,
    pub remaining_credits: u64,
    /// None means unknown funding; Some(0) means gift, both earn no revenue.
    pub original_net_micro_usd: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditLotConsumption {
    pub lot_id: String,
    pub credits: u64,
    pub net_micro_usd: u64,
    pub funding_unknown: bool,
}

/// Deterministic FIFO with cumulative allocation rounding: consuming one credit
/// at a time and consuming the same batch together yields identical cash cost.
pub fn allocate_credit_lots(
    lots: &[CreditLot],
    credits: u64,
) -> Result<Vec<CreditLotConsumption>, String> {
    let mut by_id = BTreeMap::new();
    for lot in lots {
        if lot.original_credits == 0
            || lot.remaining_credits > lot.original_credits
            || by_id.insert(&lot.lot_id, lot).is_some()
        {
            return Err("promotionCreditLotInvalid".into());
        }
    }
    let mut sorted = lots.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| (a.issued_at_ms, &a.lot_id).cmp(&(b.issued_at_ms, &b.lot_id)));
    let mut remaining = credits;
    let mut result = Vec::new();
    for lot in sorted {
        let consumed = lot.remaining_credits.min(remaining);
        if consumed == 0 {
            continue;
        }
        let already = lot.original_credits - lot.remaining_credits;
        let net = if let Some(cost) = lot.original_net_micro_usd {
            u64_from(
                u128::from(cost) * u128::from(already + consumed)
                    / u128::from(lot.original_credits)
                    - u128::from(cost) * u128::from(already) / u128::from(lot.original_credits),
            )?
        } else {
            0
        };
        result.push(CreditLotConsumption {
            lot_id: lot.lot_id.clone(),
            credits: consumed,
            net_micro_usd: net,
            funding_unknown: lot.original_net_micro_usd.is_none(),
        });
        remaining -= consumed;
        if remaining == 0 {
            break;
        }
    }
    if remaining != 0 {
        return Err("promotionCreditInsufficient".into());
    }
    Ok(result)
}

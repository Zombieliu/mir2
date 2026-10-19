//! Pure, deterministic hunting-permit arithmetic. This crate does not perform
//! authentication, authorization, transactions, scheduling, or durable idempotency.
//! The authoritative server must check `Policy::enabled`, lock its real balances,
//! and commit each accepted quote or daily plan exactly once. Calling a planning
//! function twice with the same inputs intentionally returns the same plan.
//!
//! One permit is 1,000,000 atoms; one gold converted consumes one atom. The policy
//! K means `minted_permits = destroyed_gold * K / 1,000,000`. Consequently minting
//! atoms is exactly `destroyed_gold * K`. No amount uses floating point.

use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

pub const LICENSE_ATOMS_PER_UNIT: u64 = 1_000_000;
pub const GOLD_PER_LICENSE: u64 = 1_000_000;
pub const BUSINESS_UTC_OFFSET_SECONDS: i64 = 8 * 60 * 60;
pub const BUSINESS_ROLLOVER_SECONDS: i64 = 4 * 60 * 60;
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rational {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema_version: u32,
    pub policy_version: String,
    pub enabled: bool,
    pub k: Rational,
    pub conversion_fee: Rational,
    pub daily_decay: Rational,
    pub baseline_daily_mint_atoms: u64,
    pub daily_mint_cap_multiplier: Rational,
    pub business_timezone: String,
    pub settlement_hour: u8,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PoolState {
    pub stock_atoms: u64,
    /// Fractional mint numerator, strictly smaller than the current K denominator.
    pub mint_remainder: u64,
    /// Fractional destruction numerator, smaller than the daily decay denominator.
    pub decay_remainder: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementPlan {
    pub destruction_gold_equivalent: u64,
    pub opening_stock_atoms: u64,
    pub raw_mint_atoms: u64,
    pub daily_mint_cap_atoms: u64,
    pub minted_atoms: u64,
    /// Whole atoms rejected by the cap. These are discarded, never carried forward.
    pub capped_atoms: u64,
    pub pre_decay_stock_atoms: u64,
    pub decayed_atoms: u64,
    pub next_pool: PoolState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionQuote {
    pub bound_gold_debit: u64,
    pub license_atoms_debit: u64,
    /// Taken from this conversion's gross output and recorded as actual destruction.
    pub fee_unbound_gold: u64,
    pub net_unbound_gold_credit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyError {
    InvalidPolicy(&'static str),
    InvalidRemainder(&'static str),
    InvalidAmount,
    Overflow(&'static str),
}

impl PolicyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidPolicy(_) => "invalid_policy",
            Self::InvalidRemainder(_) => "invalid_remainder",
            Self::InvalidAmount => "invalid_amount",
            Self::Overflow(_) => "overflow",
        }
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy(field) => write!(f, "invalid economy policy: {field}"),
            Self::InvalidRemainder(field) => write!(f, "invalid pool remainder: {field}"),
            Self::InvalidAmount => f.write_str("conversion amount must be positive"),
            Self::Overflow(operation) => write!(f, "economy arithmetic overflow: {operation}"),
        }
    }
}

impl Error for PolicyError {}

impl Policy {
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.schema_version != 1 {
            return Err(PolicyError::InvalidPolicy("schema_version"));
        }
        if self.policy_version.trim().is_empty() || self.policy_version.len() > 128 {
            return Err(PolicyError::InvalidPolicy("policy_version"));
        }
        validate_ratio(self.k, "k")?;
        validate_ratio(self.conversion_fee, "conversion_fee")?;
        validate_ratio(self.daily_decay, "daily_decay")?;
        validate_ratio(self.daily_mint_cap_multiplier, "daily_mint_cap_multiplier")?;
        if self.conversion_fee.numerator >= self.conversion_fee.denominator {
            return Err(PolicyError::InvalidPolicy("conversion_fee"));
        }
        if self.daily_decay.numerator >= self.daily_decay.denominator {
            return Err(PolicyError::InvalidPolicy("daily_decay"));
        }
        if self.baseline_daily_mint_atoms == 0 {
            return Err(PolicyError::InvalidPolicy("baseline_daily_mint_atoms"));
        }
        if self.daily_mint_cap_multiplier.numerator < self.daily_mint_cap_multiplier.denominator {
            return Err(PolicyError::InvalidPolicy("daily_mint_cap_multiplier"));
        }
        if self.business_timezone != "Asia/Shanghai" || self.settlement_hour != 4 {
            return Err(PolicyError::InvalidPolicy("business_clock"));
        }
        checked_cap(self)?;
        Ok(())
    }

    pub fn daily_mint_cap_atoms(&self) -> Result<u64, PolicyError> {
        self.validate()?;
        checked_cap(self)
    }
}

fn validate_ratio(ratio: Rational, field: &'static str) -> Result<(), PolicyError> {
    if ratio.numerator == 0 || ratio.denominator == 0 {
        return Err(PolicyError::InvalidPolicy(field));
    }
    Ok(())
}

fn checked_u64(value: u128, operation: &'static str) -> Result<u64, PolicyError> {
    u64::try_from(value).map_err(|_| PolicyError::Overflow(operation))
}

fn checked_cap(policy: &Policy) -> Result<u64, PolicyError> {
    let product = u128::from(policy.baseline_daily_mint_atoms)
        .checked_mul(u128::from(policy.daily_mint_cap_multiplier.numerator))
        .ok_or(PolicyError::Overflow("mint_cap_product"))?;
    let cap = product / u128::from(policy.daily_mint_cap_multiplier.denominator);
    checked_u64(cap, "mint_cap")
}

/// Produce a daily plan: mint from the closed day's destruction, apply the hard
/// cap, then decay the entire resulting stock. It does not consume player permits.
/// The caller must bind rounding state to a policy version; policy changes require
/// an explicit transactional migration or reset of each fractional remainder.
pub fn plan_settlement(
    policy: &Policy,
    pool: PoolState,
    destruction_gold_equivalent: u64,
) -> Result<SettlementPlan, PolicyError> {
    policy.validate()?;
    if pool.mint_remainder >= policy.k.denominator {
        return Err(PolicyError::InvalidRemainder("mint_remainder"));
    }
    if pool.decay_remainder >= policy.daily_decay.denominator {
        return Err(PolicyError::InvalidRemainder("decay_remainder"));
    }
    let mint_numerator = u128::from(destruction_gold_equivalent)
        .checked_mul(u128::from(policy.k.numerator))
        .and_then(|value| value.checked_add(u128::from(pool.mint_remainder)))
        .ok_or(PolicyError::Overflow("mint_numerator"))?;
    let raw_mint_atoms = checked_u64(
        mint_numerator / u128::from(policy.k.denominator),
        "raw_mint_atoms",
    )?;
    let mint_remainder = checked_u64(
        mint_numerator % u128::from(policy.k.denominator),
        "mint_remainder",
    )?;
    let daily_mint_cap_atoms = checked_cap(policy)?;
    let minted_atoms = raw_mint_atoms.min(daily_mint_cap_atoms);
    let capped_atoms = raw_mint_atoms - minted_atoms;
    let pre_decay_stock_atoms = pool
        .stock_atoms
        .checked_add(minted_atoms)
        .ok_or(PolicyError::Overflow("pre_decay_stock_atoms"))?;
    let decay_numerator = u128::from(pre_decay_stock_atoms)
        .checked_mul(u128::from(policy.daily_decay.numerator))
        .and_then(|value| value.checked_add(u128::from(pool.decay_remainder)))
        .ok_or(PolicyError::Overflow("decay_numerator"))?;
    let decayed_atoms = checked_u64(
        decay_numerator / u128::from(policy.daily_decay.denominator),
        "decayed_atoms",
    )?;
    let decay_remainder = checked_u64(
        decay_numerator % u128::from(policy.daily_decay.denominator),
        "decay_remainder",
    )?;
    let stock_atoms = pre_decay_stock_atoms
        .checked_sub(decayed_atoms)
        .ok_or(PolicyError::Overflow("closing_stock_atoms"))?;
    Ok(SettlementPlan {
        destruction_gold_equivalent,
        opening_stock_atoms: pool.stock_atoms,
        raw_mint_atoms,
        daily_mint_cap_atoms,
        minted_atoms,
        capped_atoms,
        pre_decay_stock_atoms,
        decayed_atoms,
        next_pool: PoolState {
            stock_atoms,
            mint_remainder,
            decay_remainder,
        },
    })
}

/// Quote only; no balance or permission is checked here. The server must atomically
/// debit bound gold and the live global pool, credit the net unbound output, and
/// append the fee's destruction entry. Fees round upward to one gold, so splitting
/// requests cannot reduce the fee. A one-gold request has a zero-gold net output.
pub fn quote_conversion(policy: &Policy, gross_gold: u64) -> Result<ConversionQuote, PolicyError> {
    policy.validate()?;
    if gross_gold == 0 {
        return Err(PolicyError::InvalidAmount);
    }
    let numerator = u128::from(gross_gold)
        .checked_mul(u128::from(policy.conversion_fee.numerator))
        .ok_or(PolicyError::Overflow("fee_numerator"))?;
    let denominator = u128::from(policy.conversion_fee.denominator);
    let fee = (numerator / denominator)
        .checked_add(u128::from(numerator % denominator != 0))
        .ok_or(PolicyError::Overflow("fee_rounding"))?;
    let fee_unbound_gold = checked_u64(fee, "fee_unbound_gold")?;
    let net_unbound_gold_credit = gross_gold
        .checked_sub(fee_unbound_gold)
        .ok_or(PolicyError::Overflow("net_unbound_gold_credit"))?;
    Ok(ConversionQuote {
        bound_gold_debit: gross_gold,
        license_atoms_debit: gross_gold,
        fee_unbound_gold,
        net_unbound_gold_credit,
    })
}

/// Integer date key for the local calendar date whose business day begins at
/// 04:00 China time. Euclidean division handles pre-epoch dates correctly.
/// This is the explicit fixed UTC+08:00 business clock, not historical timezone data.
pub fn business_day_key(unix_seconds: i64) -> Result<i64, PolicyError> {
    let shifted = unix_seconds
        .checked_add(BUSINESS_UTC_OFFSET_SECONDS)
        .and_then(|value| value.checked_sub(BUSINESS_ROLLOVER_SECONDS))
        .ok_or(PolicyError::Overflow("business_day_key"))?;
    Ok(shifted.div_euclid(SECONDS_PER_DAY))
}

/// UTC [start, end) seconds for one business-day key. At a settlement boundary,
/// settle the preceding key, rather than the just-opened business day.
pub fn settlement_window(day_key: i64) -> Result<(i64, i64), PolicyError> {
    let start = day_key
        .checked_mul(SECONDS_PER_DAY)
        .and_then(|value| value.checked_sub(BUSINESS_UTC_OFFSET_SECONDS))
        .and_then(|value| value.checked_add(BUSINESS_ROLLOVER_SECONDS))
        .ok_or(PolicyError::Overflow("settlement_window_start"))?;
    let end = start
        .checked_add(SECONDS_PER_DAY)
        .ok_or(PolicyError::Overflow("settlement_window_end"))?;
    Ok((start, end))
}

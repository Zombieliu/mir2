//! Fixed server valuations and pure material conversion arithmetic.
//! No wallet, inventory, permit balance, authentication or publication is changed.
use crate::{quote_conversion, Policy, PolicyError};
use serde::{Deserialize, Deserializer, Serialize};
use std::{error::Error, fmt};

/// Quotes must fit the authoritative PostgreSQL BIGINT amount columns.
pub const MAX_MATERIAL_GOLD_EQUIVALENT: u64 = i64::MAX as u64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialValuationEntry {
    pub item_index: i32,
    pub unit_gold: u64,
}

/// Immutable after validation; serialization always emits entries sorted by index.
/// Index positivity is checked here; the game server still checks its real catalog.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MaterialValuationSet {
    schema_version: u32,
    valuation_version: String,
    entries: Vec<MaterialValuationEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValuationWire {
    schema_version: u32,
    valuation_version: String,
    entries: Vec<MaterialValuationEntry>,
}

impl MaterialValuationSet {
    pub fn new(
        valuation_version: impl Into<String>,
        mut entries: Vec<MaterialValuationEntry>,
    ) -> Result<Self, MaterialConversionError> {
        let valuation_version = valuation_version.into();
        if valuation_version.is_empty()
            || valuation_version.trim() != valuation_version
            || valuation_version.len() > 128
            || valuation_version.chars().any(char::is_control)
        {
            return Err(MaterialConversionError::InvalidValuation(
                "valuation_version",
            ));
        }
        for entry in &entries {
            if entry.item_index <= 0 {
                return Err(MaterialConversionError::InvalidValuation("item_index"));
            }
            validate_unit_gold(entry.unit_gold)?;
        }
        entries.sort_unstable_by_key(|entry| entry.item_index);
        if entries
            .windows(2)
            .any(|pair| pair[0].item_index == pair[1].item_index)
        {
            return Err(MaterialConversionError::InvalidValuation(
                "duplicate_item_index",
            ));
        }
        Ok(Self {
            schema_version: 1,
            valuation_version,
            entries,
        })
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }
    pub fn valuation_version(&self) -> &str {
        &self.valuation_version
    }
    pub fn entries(&self) -> &[MaterialValuationEntry] {
        &self.entries
    }

    /// Missing prices are refusal, never an invitation to infer NPC/market value.
    pub fn unit_gold(&self, item_index: i32) -> Result<u64, MaterialConversionError> {
        if item_index <= 0 {
            return Err(MaterialConversionError::InvalidValuation("item_index"));
        }
        self.entries
            .binary_search_by_key(&item_index, |entry| entry.item_index)
            .map(|index| self.entries[index].unit_gold)
            .map_err(|_| MaterialConversionError::UnconfiguredMaterial(item_index))
    }
}

impl<'de> Deserialize<'de> for MaterialValuationSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = ValuationWire::deserialize(deserializer)?;
        if value.schema_version != 1 {
            return Err(serde::de::Error::custom(
                "unsupported material valuation schema_version",
            ));
        }
        Self::new(value.valuation_version, value.entries).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialConversionQuote {
    pub gold_equivalent: u64,
    pub license_atoms_debit: u64,
    /// Debited from existing unbound gold by the authoritative transaction.
    /// The material quote creates no bound debit and no unbound gold credit.
    pub fee_unbound_gold: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaterialConversionError {
    InvalidValuation(&'static str),
    UnconfiguredMaterial(i32),
    InvalidQuantity,
    Overflow(&'static str),
    Policy(PolicyError),
}

impl MaterialConversionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidValuation(_) => "invalid_valuation",
            Self::UnconfiguredMaterial(_) => "unconfigured_material",
            Self::InvalidQuantity => "invalid_amount",
            Self::Overflow(_) => "overflow",
            Self::Policy(error) => error.code(),
        }
    }
}
impl fmt::Display for MaterialConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValuation(field) => {
                write!(formatter, "invalid material valuation: {field}")
            }
            Self::UnconfiguredMaterial(index) => {
                write!(formatter, "material has no configured valuation: {index}")
            }
            Self::InvalidQuantity => formatter.write_str("material quantity must be positive"),
            Self::Overflow(field) => write!(
                formatter,
                "material amount exceeds the PostgreSQL limit: {field}"
            ),
            Self::Policy(error) => fmt::Display::fmt(error, formatter),
        }
    }
}
impl Error for MaterialConversionError {}

fn validate_unit_gold(unit_gold: u64) -> Result<(), MaterialConversionError> {
    if unit_gold == 0 {
        return Err(MaterialConversionError::InvalidValuation("unit_gold"));
    }
    if unit_gold > MAX_MATERIAL_GOLD_EQUIVALENT {
        return Err(MaterialConversionError::Overflow("unit_gold"));
    }
    Ok(())
}

/// `unit_gold` must come from a validated, version-pinned server valuation.
/// `E = unit_gold * quantity` consumes E permit atoms. Only the rounded fee is
/// destroyed; E is an asset valuation, not destruction or a gold payout.
pub fn quote_material_conversion(
    policy: &Policy,
    unit_gold: u64,
    quantity: u64,
) -> Result<MaterialConversionQuote, MaterialConversionError> {
    policy.validate().map_err(MaterialConversionError::Policy)?;
    validate_unit_gold(unit_gold)?;
    if quantity == 0 {
        return Err(MaterialConversionError::InvalidQuantity);
    }
    let gold_equivalent = unit_gold
        .checked_mul(quantity)
        .filter(|amount| *amount <= MAX_MATERIAL_GOLD_EQUIVALENT)
        .ok_or(MaterialConversionError::Overflow("gold_equivalent"))?;
    // Reuse only the pure, versioned rounding formula, never a wallet conversion.
    let fee_unbound_gold = quote_conversion(policy, gold_equivalent)
        .map_err(MaterialConversionError::Policy)?
        .fee_unbound_gold;
    Ok(MaterialConversionQuote {
        gold_equivalent,
        license_atoms_debit: gold_equivalent,
        fee_unbound_gold,
    })
}

pub fn quote_material_conversion_for_item(
    policy: &Policy,
    valuations: &MaterialValuationSet,
    item_index: i32,
    quantity: u64,
) -> Result<MaterialConversionQuote, MaterialConversionError> {
    let unit_gold = valuations.unit_gold(item_index)?;
    quote_material_conversion(policy, unit_gold, quantity)
}

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;

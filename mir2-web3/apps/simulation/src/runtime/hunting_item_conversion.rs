//! Private, immutable provenance for a material whose bound custody was converted.
//! This module does not authorize a conversion or charge its fee: the trusted
//! owner transaction must persist the returned complete item with its receipt.
use super::hunting_item_binding::belongs_to_current_owner;
use super::{validate_committed_item_state_carrier, ItemState};
use bevy_ecs::world::World;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "ConversionWire")]
pub(in crate::runtime) struct HuntingItemConversionProvenance {
    version: u8,
    realm: String,
    operation_id: String,
    quote_id: String,
    valuation_version: String,
    valuation_digest: String,
    source_binding_digest: String,
    input_unique_id: u64,
    lot_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConversionWire {
    version: u8,
    realm: String,
    operation_id: String,
    quote_id: String,
    valuation_version: String,
    valuation_digest: String,
    source_binding_digest: String,
    input_unique_id: u64,
    lot_id: String,
}

/// These identities come from the server's retained, persisted quote, never
/// from a client item payload. Digests and opaque IDs use raw lowercase hex.
pub(in crate::runtime) struct ConversionIdentity<'a> {
    pub operation_id: &'a str,
    pub quote_id: &'a str,
    pub valuation_version: &'a str,
    pub valuation_digest: &'a str,
    pub lot_id: &'a str,
}

fn token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn opaque_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

impl HuntingItemConversionProvenance {
    fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || !token(&self.realm, 128)
            || !token(&self.valuation_version, 128)
            || !opaque_id(&self.operation_id)
            || !opaque_id(&self.quote_id)
            || !opaque_id(&self.valuation_digest)
            || !opaque_id(&self.source_binding_digest)
            || !opaque_id(&self.lot_id)
            || self.input_unique_id == 0
        {
            return Err("invalid hunting item conversion provenance".into());
        }
        Ok(())
    }

    pub(in crate::runtime) fn realm(&self) -> &str {
        &self.realm
    }
}

impl TryFrom<ConversionWire> for HuntingItemConversionProvenance {
    type Error = String;
    fn try_from(raw: ConversionWire) -> Result<Self, Self::Error> {
        let value = Self {
            version: raw.version,
            realm: raw.realm,
            operation_id: raw.operation_id,
            quote_id: raw.quote_id,
            valuation_version: raw.valuation_version,
            valuation_digest: raw.valuation_digest,
            source_binding_digest: raw.source_binding_digest,
            input_unique_id: raw.input_unique_id,
            lot_id: raw.lot_id,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Hash the existing strict Bound v1 object with an explicit versioned domain.
/// This is provenance only; a digest cannot establish conversion eligibility.
pub(in crate::runtime) fn source_binding_digest(item: &ItemState) -> Result<String, String> {
    if item.economic_conversion.is_some() {
        return Err("item already has conversion provenance".into());
    }
    let binding = item
        .economic_binding
        .as_ref()
        .ok_or("item has no bound source")?;
    let bytes = serde_json::to_vec(&("hunting-item-binding-v1", binding))
        .map_err(|_| "bound source digest unavailable")?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Return a complete changed clone, leaving the input untouched on every error.
/// No child is recursively unbound. Partial-stack callers retain the original
/// input UID here and allocate a fresh output UID in their complete item plan.
pub(in crate::runtime) fn convert_bound_root(
    world: &World,
    item: &ItemState,
    identity: ConversionIdentity<'_>,
) -> Result<ItemState, String> {
    validate_committed_item_state_carrier(item).map_err(|_| "invalid conversion item carrier")?;
    if item.unique_id == 0
        || item.economic_binding.is_none()
        || item.economic_conversion.is_some()
        || !item.socketed.is_empty()
        || item.user_item_metadata.as_ref().is_some_and(|metadata| {
            metadata.slots.iter().any(Option::is_some)
                || metadata
                    .captured_socket_positions
                    .as_ref()
                    .is_some_and(|slots| slots.iter().any(Option::is_some))
        })
        || !belongs_to_current_owner(world, item)
    {
        return Err("conversion requires a current-owner plain bound root".into());
    }
    let binding = item
        .economic_binding
        .as_ref()
        .ok_or("item has no bound source")?;
    let value = HuntingItemConversionProvenance {
        version: 1,
        realm: binding.realm().to_string(),
        operation_id: identity.operation_id.into(),
        quote_id: identity.quote_id.into(),
        valuation_version: identity.valuation_version.into(),
        valuation_digest: identity.valuation_digest.into(),
        source_binding_digest: source_binding_digest(item)?,
        input_unique_id: item.unique_id,
        lot_id: identity.lot_id.into(),
    };
    value.validate()?;
    let mut converted = item.clone();
    converted.economic_binding = None;
    converted.economic_conversion = Some(value);
    Ok(converted)
}

// Keep a single field declaration for each existing carrier. Deserialize uses
// the same defaults as before, but rejects mutually exclusive private marks
// before any load, ground claim or registry can accept a mixed carrier.
macro_rules! economic_state {
    ($visibility:vis struct $name:ident, $raw:ident {
        $($(#[$attribute:meta])* $field_visibility:vis $field:ident: $type:ty,)*
    }) => {
        #[derive(Debug, Clone, serde::Serialize)]
        $visibility struct $name {
            $($(#[$attribute])* $field_visibility $field: $type,)*
        }
        #[derive(serde::Deserialize)]
        struct $raw {
            $($(#[$attribute])* $field_visibility $field: $type,)*
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = <$raw as serde::Deserialize>::deserialize(deserializer)?;
                if raw.economic_binding.is_some() && raw.economic_conversion.is_some() {
                    return Err(serde::de::Error::custom("item cannot be both bound and converted"));
                }
                Ok(Self { $($field: raw.$field,)* })
            }
        }
    };
}
pub(in crate::runtime) use economic_state;

#[cfg(test)]
#[path = "hunting_item_conversion_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "hunting_item_conversion_outflow_tests.rs"]
mod outflow_tests;

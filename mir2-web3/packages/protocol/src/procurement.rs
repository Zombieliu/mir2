//! Personal procurement V1 JSON contracts. Game assets remain server custody.
//! Monetary u64 values and instance IDs use canonical decimal strings on JSON.
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const INTENT_DOMAIN: &str = "MIR2_PROCUREMENT_INTENT_V1";
pub const TARGET_TEMPLATE_ID: u32 = 245;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcurementRequest {
    pub protocol_version: u16,
    pub request_id: String,
    pub character_index: i32,
    pub action: ProcurementAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProcurementAction {
    Status {
        npc_object_id: u32,
    },
    List {
        template_id: Option<u32>,
    },
    QuotePublish {
        template_id: u32,
        price_gold: String,
        expires_at_ms: String,
        conditions: EquipmentConditions,
        purpose_public_key: String,
    },
    QuoteDeliver {
        order_id: String,
        item_uid: String,
        purpose_public_key: String,
    },
    QuoteCancel {
        order_id: String,
        purpose_public_key: String,
    },
    QuoteBuyQuota {
        package_id: String,
    },
    Confirm {
        quote_id: String,
        signature: String,
    },
    Claim {
        delivery_id: String,
    },
    Reconcile {
        order_id: Option<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentConditions {
    pub min_current_durability: u16,
    pub min_max_durability: u16,
    pub min_added_stats: Vec<EquipmentStatCondition>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentStatCondition {
    pub stat: u8,
    pub value: i32,
}

impl EquipmentConditions {
    /// Canonical BCS condition commitment: u16,u16,vector<(u8,i32)>.
    /// Stats must be strictly increasing to avoid multiple encodings of one quote.
    pub fn canonical_bcs(&self) -> Result<Vec<u8>, &'static str> {
        if self.min_added_stats.len() > 64
            || self
                .min_added_stats
                .windows(2)
                .any(|p| p[0].stat >= p[1].stat)
        {
            return Err("invalid conditions");
        }
        let mut bytes = self.min_current_durability.to_le_bytes().to_vec();
        bytes.extend(self.min_max_durability.to_le_bytes());
        length(self.min_added_stats.len(), &mut bytes);
        for stat in &self.min_added_stats {
            bytes.push(stat.stat);
            bytes.extend(stat.value.to_le_bytes());
        }
        Ok(bytes)
    }
}

/// Exact Move IntentV1 fields and order. Addresses are fixed 32 bytes in BCS;
/// all other byte fields are vectors with an explicit ULEB128 length.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentV1 {
    pub domain: String,
    pub network_hash: String,
    pub package_id: String,
    pub dapp_storage_id: String,
    pub operation: u8,
    pub order_id: String,
    pub fill_id: String,
    pub buyer_ref: String,
    pub seller_ref: String,
    pub template_id: u32,
    pub conditions_hash: String,
    pub quantity: String,
    pub price_gold: String,
    pub expires_at_ms: String,
    pub rule_version: String,
    pub nonce: String,
    pub valid_until_ms: String,
    pub actor_public_key: String,
}
impl IntentV1 {
    pub fn canonical_bcs(&self) -> Result<Vec<u8>, &'static str> {
        if self.domain != INTENT_DOMAIN
            || !(1..=3).contains(&self.operation)
            || self.template_id != TARGET_TEMPLATE_ID
            || decimal(&self.quantity)? != 1
        {
            return Err("unsupported intent");
        }
        let mut out = Vec::with_capacity(512);
        vector(self.domain.as_bytes(), &mut out);
        vector(&hex32(&self.network_hash)?, &mut out);
        out.extend(hex32(&self.package_id)?);
        out.extend(hex32(&self.dapp_storage_id)?);
        out.push(self.operation);
        for value in [
            &self.order_id,
            &self.fill_id,
            &self.buyer_ref,
            &self.seller_ref,
        ] {
            vector(&hex32(value)?, &mut out);
        }
        out.extend(self.template_id.to_le_bytes());
        vector(&hex32(&self.conditions_hash)?, &mut out);
        for value in [
            &self.quantity,
            &self.price_gold,
            &self.expires_at_ms,
            &self.rule_version,
        ] {
            out.extend(decimal(value)?.to_le_bytes());
        }
        vector(&hex32(&self.nonce)?, &mut out);
        out.extend(decimal(&self.valid_until_ms)?.to_le_bytes());
        vector(&hex32(&self.actor_public_key)?, &mut out);
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcurementQuote {
    pub quote_id: String,
    pub operation: String,
    pub intent: Option<IntentV1>,
    pub canonical_intent_hex: Option<String>,
    pub conditions: EquipmentConditions,
    pub item_uid: Option<String>,
    pub item_description: Option<String>,
    pub service_fee: ServiceFeeQuote,
    pub quota_package_id: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFeeQuote {
    pub rule_version: String,
    pub quota_units: u32,
    pub quota_source: String,
    pub credit_cost: u32,
    pub refund_on_cancel: bool,
    pub refund_on_expiry: bool,
    pub quota_valid_until_ms: Option<String>,
    pub production_billing_enabled: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcurementView {
    pub enabled: bool,
    pub test_mode: bool,
    pub production_billing_enabled: bool,
    pub actor_ref: String,
    pub network_hash: String,
    pub package_id: String,
    pub dapp_storage_id: String,
    pub available_gold: String,
    pub escrow_gold: String,
    pub test_quota: u32,
    pub paid_quota: u32,
    pub reserved_quota: u32,
    pub fee_rule: ServiceFeeQuote,
    pub quota_packages: Vec<QuotaPackage>,
    pub public_orders: Vec<ProcurementOrder>,
    pub my_orders: Vec<ProcurementOrder>,
    pub deliveries: Vec<ProcurementDelivery>,
    pub fee_history: Vec<ServiceFeeEntry>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuotaPackage {
    pub id: String,
    pub label: String,
    pub units: u32,
    pub credit_cost: u32,
    pub source: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcurementOrder {
    pub order_id: String,
    pub buyer_ref: String,
    pub seller_ref: Option<String>,
    pub template_id: u32,
    pub price_gold: String,
    pub expires_at_ms: String,
    pub rule_version: String,
    pub conditions: EquipmentConditions,
    pub chain_state: String,
    pub delivery_state: String,
    pub fill_id: Option<String>,
    pub chain_transaction: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcurementDelivery {
    pub delivery_id: String,
    pub order_id: String,
    pub item_uid: String,
    pub template_id: u32,
    pub description: String,
    pub state: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFeeEntry {
    pub order_id: String,
    pub rule_version: String,
    pub source: String,
    pub units: u32,
    pub state: String,
    pub credit_cost: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcurementReply {
    pub protocol_version: u16,
    pub request_id: String,
    pub character_index: i32,
    pub status: Option<ProcurementView>,
    pub quote: Option<ProcurementQuote>,
    pub error: Option<String>,
    pub replayed: bool,
}

pub fn decimal(value: &str) -> Result<u64, &'static str> {
    if value.is_empty()
        || value.len() > 20
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("invalid decimal");
    }
    value.parse().map_err(|_| "decimal overflow")
}
pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 15) as usize] as char);
    }
    out
}
pub fn decode_hex(value: &str) -> Result<Vec<u8>, &'static str> {
    let value = value.strip_prefix("0x").ok_or("hex prefix")?;
    if value.len() > 16384 || value.len() % 2 != 0 {
        return Err("hex size");
    }
    let nibble = |b| match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err("noncanonical hex"),
    };
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|p| Ok(nibble(p[0])? * 16 + nibble(p[1])?))
        .collect()
}
pub fn hex32(value: &str) -> Result<[u8; 32], &'static str> {
    decode_hex(value)?
        .try_into()
        .map_err(|_| "expected 32 bytes")
}
fn length(mut len: usize, out: &mut Vec<u8>) {
    loop {
        let part = (len & 127) as u8;
        len >>= 7;
        out.push(part | if len > 0 { 128 } else { 0 });
        if len == 0 {
            break;
        }
    }
}
fn vector(value: &[u8], out: &mut Vec<u8>) {
    length(value.len(), out);
    out.extend(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_ids_and_conditions_reject_ambiguous_inputs() {
        for value in ["", "01", "-1", "18446744073709551616", " 1"] {
            assert!(decimal(value).is_err());
        }
        assert_eq!(decimal("18446744073709551615"), Ok(u64::MAX));
        assert_eq!(decode_hex("0x00ff"), Ok(vec![0, 255]));
        assert!(decode_hex("0xFF").is_err());
        let conditions = EquipmentConditions {
            min_current_durability: 1,
            min_max_durability: 2,
            min_added_stats: vec![
                EquipmentStatCondition { stat: 4, value: 3 },
                EquipmentStatCondition { stat: 4, value: 4 },
            ],
        };
        assert!(conditions.canonical_bcs().is_err());
        assert_eq!(
            EquipmentConditions::default().canonical_bcs().unwrap(),
            vec![0, 0, 0, 0, 0]
        );
    }
}

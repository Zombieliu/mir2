//! Authenticated material conversion carrier. UID and quantity are client intent;
//! prices and original conversion identities are server-issued evidence only.
//! Decoding does not prove owner authority or that a snapshot has been applied.
use super::{Opaque32, WireError, U64};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_REQUEST_BYTES: usize = 4096;
pub const MAX_FRAME_BYTES: usize = super::MAX_SERVER_FRAME_BYTES;
pub const QUOTE_TTL_MS: i64 = 30_000;

type Result<T> = std::result::Result<T, WireError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvelopeType {
    #[serde(rename = "huntingMaterialOwner")]
    HuntingMaterialOwner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    Quote { unique_id: U64, quantity: U64 },
    Confirm { quote_id: String },
    Query { quote_id: String },
}
impl Action {
    pub fn quote_id(&self) -> Option<&str> {
        match self {
            Self::Confirm { quote_id } | Self::Query { quote_id } => Some(quote_id),
            Self::Quote { .. } => None,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Quote {
                unique_id,
                quantity,
            } => {
                positive_uid(*unique_id)?;
                quantity_range(*quantity, *quantity)
            }
            Self::Confirm { quote_id } | Self::Query { quote_id } => {
                Opaque32::parse(quote_id)?;
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientRequest {
    #[serde(rename = "type")]
    pub wire_type: EnvelopeType,
    pub protocol_version: u16,
    pub request_id: U64,
    pub action: Action,
}
impl ClientRequest {
    pub fn new(request_id: U64, action: Action) -> Result<Self> {
        let request = Self {
            wire_type: EnvelopeType::HuntingMaterialOwner,
            protocol_version: PROTOCOL_VERSION,
            request_id,
            action,
        };
        request.validate()?;
        Ok(request)
    }
    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != PROTOCOL_VERSION || self.request_id.get() == 0 {
            return Err(WireError::Invalid(
                "unsupported material protocol or request identity",
            ));
        }
        self.action.validate()
    }
}

fn positive_uid(value: U64) -> Result<()> {
    if value.get() == 0 {
        return Err(WireError::Invalid(
            "material instance identity must be positive",
        ));
    }
    Ok(())
}
fn quantity_range(quantity: U64, input_count: U64) -> Result<()> {
    if quantity.get() == 0 || quantity > input_count || input_count.get() > u64::from(u32::MAX) {
        return Err(WireError::Invalid(
            "invalid material quantity or input count",
        ));
    }
    Ok(())
}
fn amounts(gold_equivalent: U64, license_atoms_debit: U64, fee: U64) -> Result<()> {
    if gold_equivalent.get() == 0
        || gold_equivalent.get() > i64::MAX as u64
        || license_atoms_debit != gold_equivalent
        || fee.get() == 0
        || fee > gold_equivalent
    {
        return Err(WireError::Invalid(
            "invalid material valuation, permit debit or fee",
        ));
    }
    Ok(())
}
fn version(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(WireError::Invalid("invalid material valuation version"));
    }
    Ok(())
}
fn request_digest(value: &str) -> Result<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(WireError::Invalid("invalid material request digest"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(WireError::Invalid("invalid material request digest"));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Quote {
    pub quote_id: String,
    pub unique_id: U64,
    pub item_index: i32,
    pub input_count: U64,
    pub quantity: U64,
    pub unit_gold: U64,
    pub gold_equivalent: U64,
    pub fee_unbound_gold: U64,
    pub license_atoms_debit: U64,
    pub valuation_version: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
}
impl Quote {
    pub fn validate(&self) -> Result<()> {
        Opaque32::parse(&self.quote_id)?;
        positive_uid(self.unique_id)?;
        quantity_range(self.quantity, self.input_count)?;
        amounts(
            self.gold_equivalent,
            self.license_atoms_debit,
            self.fee_unbound_gold,
        )?;
        version(&self.valuation_version)?;
        if self.item_index <= 0
            || self.unit_gold.get() == 0
            || self.unit_gold.get().checked_mul(self.quantity.get())
                != Some(self.gold_equivalent.get())
            || self.created_at_ms < 0
            || self.created_at_ms.checked_add(QUOTE_TTL_MS) != Some(self.expires_at_ms)
        {
            return Err(WireError::Invalid("invalid server material quote"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub quote_id: String,
    pub input_unique_id: U64,
    pub output_unique_id: U64,
    pub input_count: U64,
    pub quantity: U64,
    pub gold_equivalent: U64,
    pub fee_unbound_gold: U64,
    pub license_atoms_debit: U64,
    pub valuation_version: String,
    pub operation_key: String,
    pub request_digest: String,
    pub business_day: i64,
    pub committed_at_ms: i64,
}
impl Receipt {
    pub fn validate(&self) -> Result<()> {
        Opaque32::parse(&self.quote_id)?;
        Opaque32::parse(&self.operation_key)?;
        request_digest(&self.request_digest)?;
        positive_uid(self.input_unique_id)?;
        positive_uid(self.output_unique_id)?;
        quantity_range(self.quantity, self.input_count)?;
        amounts(
            self.gold_equivalent,
            self.license_atoms_debit,
            self.fee_unbound_gold,
        )?;
        version(&self.valuation_version)?;
        if self.gold_equivalent.get() % self.quantity.get() != 0
            || (self.quantity == self.input_count)
                != (self.input_unique_id == self.output_unique_id)
            || self.committed_at_ms < 0
        {
            return Err(WireError::Invalid("invalid immutable material receipt"));
        }
        Ok(())
    }
    /// A retained server quote can prove more than envelope correlation. This
    /// compares its immutable amounts and original source identity; it grants no
    /// actor authority and does not claim the current inventory matches history.
    pub fn validate_for_quote(&self, quote: &Quote) -> Result<()> {
        self.validate()?;
        quote.validate()?;
        if self.quote_id != quote.quote_id
            || self.input_unique_id != quote.unique_id
            || self.input_count != quote.input_count
            || self.quantity != quote.quantity
            || self.gold_equivalent != quote.gold_equivalent
            || self.fee_unbound_gold != quote.fee_unbound_gold
            || self.license_atoms_debit != quote.license_atoms_debit
            || self.valuation_version != quote.valuation_version
        {
            return Err(WireError::Invalid(
                "material receipt does not match original quote",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rejection {
    Disabled,
    Unauthenticated,
    Unavailable,
    InvalidRequest,
    Unsupported,
    Expired,
    ItemChanged,
    NotSubmitted,
    Unconfigured,
    InsufficientPermits,
    InsufficientUnboundGold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Reply {
    Quoted {
        quote: Quote,
    },
    Pending {
        quote_id: String,
    },
    /// No terminal outcome is established. Retain the original quote for Query;
    /// an unknown response never authorizes a replacement Confirm.
    Unknown {
        quote_id: Option<String>,
    },
    Committed {
        receipt: Receipt,
        replayed: bool,
        reload_required: bool,
    },
    Rejected {
        quote_id: Option<String>,
        code: Rejection,
    },
}
impl Reply {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Quoted { quote } => quote.validate(),
            Self::Committed { receipt, .. } => receipt.validate(),
            Self::Pending { quote_id } => {
                Opaque32::parse(quote_id)?;
                Ok(())
            }
            Self::Rejected { quote_id, .. } | Self::Unknown { quote_id } => {
                if let Some(id) = quote_id {
                    Opaque32::parse(id)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerFrame {
    #[serde(rename = "type")]
    pub wire_type: EnvelopeType,
    pub protocol_version: u16,
    pub request_id: U64,
    pub reply: Reply,
    /// Opaque current-owner state captured with the completed turn. Null keeps
    /// a known historical receipt without claiming a current applied snapshot.
    #[serde(deserialize_with = "super::required_option")]
    pub snapshot: Option<Value>,
}
impl ServerFrame {
    pub fn new(request: &ClientRequest, reply: Reply, snapshot: Option<Value>) -> Result<Self> {
        let frame = Self {
            wire_type: EnvelopeType::HuntingMaterialOwner,
            protocol_version: PROTOCOL_VERSION,
            request_id: request.request_id,
            reply,
            snapshot,
        };
        frame.validate_for_request(request)?;
        Ok(frame)
    }
    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != PROTOCOL_VERSION || self.request_id.get() == 0 {
            return Err(WireError::Invalid(
                "unsupported material response protocol or identity",
            ));
        }
        self.reply.validate()?;
        if self.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.is_null()
                || !matches!(
                    &self.reply,
                    Reply::Committed {
                        reload_required: false,
                        ..
                    }
                )
        }) {
            return Err(WireError::Invalid(
                "material snapshot requires a completed current owner result",
            ));
        }
        Ok(())
    }
    pub fn validate_for_request(&self, request: &ClientRequest) -> Result<()> {
        self.validate()?;
        request.validate()?;
        if self.request_id != request.request_id {
            return Err(WireError::Invalid("uncorrelated material response"));
        }
        match (&request.action, &self.reply) {
            (
                Action::Quote {
                    unique_id,
                    quantity,
                },
                Reply::Quoted { quote },
            ) if unique_id == &quote.unique_id && quantity == &quote.quantity => Ok(()),
            (Action::Quote { .. }, Reply::Rejected { quote_id: None, .. })
            | (Action::Quote { .. }, Reply::Unknown { quote_id: None }) => Ok(()),
            (
                Action::Confirm { quote_id } | Action::Query { quote_id },
                Reply::Pending { quote_id: actual },
            ) if quote_id == actual => Ok(()),
            (
                Action::Confirm { quote_id } | Action::Query { quote_id },
                Reply::Committed { receipt, .. },
            ) if quote_id == &receipt.quote_id => Ok(()),
            (
                Action::Confirm { quote_id } | Action::Query { quote_id },
                Reply::Rejected {
                    quote_id: Some(actual),
                    ..
                }
                | Reply::Unknown {
                    quote_id: Some(actual),
                },
            ) if quote_id == actual => Ok(()),
            _ => Err(WireError::Invalid(
                "material response does not match original action",
            )),
        }
    }
}

pub fn parse_client_request(text: &str) -> Result<ClientRequest> {
    let request: ClientRequest = super::parse(text, MAX_REQUEST_BYTES)?;
    request.validate()?;
    Ok(request)
}
pub fn encode_client_request(request: &ClientRequest) -> Result<String> {
    request.validate()?;
    encode(request, MAX_REQUEST_BYTES)
}
pub fn parse_server_frame(text: &str) -> Result<ServerFrame> {
    let frame: ServerFrame = super::parse(text, MAX_FRAME_BYTES)?;
    frame.validate()?;
    Ok(frame)
}
pub fn encode_server_frame(frame: &ServerFrame) -> Result<String> {
    frame.validate()?;
    encode(frame, MAX_FRAME_BYTES)
}
fn encode(value: &impl Serialize, limit: usize) -> Result<String> {
    let text = serde_json::to_string(value)?;
    if text.len() > limit {
        return Err(WireError::Invalid("material message exceeds size limit"));
    }
    Ok(text)
}

#[cfg(test)]
#[path = "hunting_material_tests.rs"]
mod tests;

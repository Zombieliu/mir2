//! Strict external schema for durable NPC purchase messages.
//!
//! Decoding establishes shape and correlation, never authentication, accepted
//! server capability, current physical connection, or complete applied state.
//! The host supplies those proofs. A snapshot is an uninterpreted carrier;
//! authority paired with it does not certify that a client has applied it.
//! Missing recovery is unknown and never authorizes a replacement operation.

use std::collections::BTreeSet;
use std::fmt;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub const NPC_PURCHASE_OWNER_PROTOCOL_VERSION: u16 = 1;
pub const NPC_PURCHASE_OWNER_CAPABILITY: &str = "durableNpcPurchaseOwnerV1";
pub const MAX_CLIENT_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_SERVER_FRAME_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub enum WireError {
    Invalid(&'static str),
    Json(serde_json::Error),
}
impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { Self::Invalid(reason) => f.write_str(reason), Self::Json(error) => write!(f, "{error}") }
    }
}
impl std::error::Error for WireError {}
impl From<serde_json::Error> for WireError {
    fn from(error: serde_json::Error) -> Self { Self::Json(error) }
}
type Result<T> = std::result::Result<T, WireError>;

/// Every external u64 is a canonical decimal JSON string, including zero.
/// The private numeric backing cannot represent a malformed spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct U64(u64);
impl U64 {
    pub const fn new(value: u64) -> Self { Self(value) }
    pub const fn get(self) -> u64 { self.0 }
    pub fn parse(value: &str) -> Result<Self> {
        if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0')) {
            return Err(WireError::Invalid("noncanonical u64 string"));
        }
        value.parse::<u64>().map(Self).map_err(|_| WireError::Invalid("u64 overflow"))
    }
}
impl Serialize for U64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for U64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Canonical nonzero lower-case hexadecimal opaque bytes. Nonzero is a schema
/// condition, not proof that a server authorized an actor, scope or catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Opaque32([u8; 32]);
impl Opaque32 {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self> {
        if bytes == [0; 32] { return Err(WireError::Invalid("zero opaque identity")); }
        Ok(Self(bytes))
    }
    pub const fn get(self) -> [u8; 32] { self.0 }
    pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }
    pub fn parse(value: &str) -> Result<Self> {
        if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return Err(WireError::Invalid("noncanonical opaque identity"));
        }
        let mut bytes = [0; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let nibble = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            bytes[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
        }
        Self::from_bytes(bytes)
    }
    fn hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(64);
        for byte in self.0 { text.push(HEX[(byte >> 4) as usize] as char); text.push(HEX[(byte & 15) as usize] as char); }
        text
    }
}
impl Serialize for Opaque32 {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hex())
    }
}
impl<'de> Deserialize<'de> for Opaque32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

// Checking both Deserialize and Serialize prevents public DTO construction
// from bypassing semantic validation. Raw representations are private.
macro_rules! checked_struct {
    ($name:ident, $raw:ident, { $($(#[$meta:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name { $(pub $field: $ty),* }
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct $raw { $($(#[$meta])* $field: $ty),* }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
                let raw = $raw::deserialize(deserializer)?;
                let result = Self { $($field: raw.$field),* };
                result.validate().map_err(de::Error::custom)?;
                Ok(result)
            }
        }
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                self.validate().map_err(serde::ser::Error::custom)?;
                $raw { $($field: self.$field.clone()),* }.serialize(serializer)
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvelopeType { #[serde(rename = "npcPurchaseOwner")] NpcPurchaseOwner }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Currency { Gold, Pearls }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source { Trade, BuyBack, Used }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rejection {
    InvalidRequest, PlayerDead, ServiceUnavailable, UnsupportedService,
    UnknownGood, InvalidQuantity, InsufficientCurrency, ClockUnavailable, InvalidDelivery,
}

checked_struct!(PurchaseRequest, RawPurchaseRequest, { item_index: U64, count: u16, panel_type: u8 });
impl PurchaseRequest {
    pub fn new(item_index: U64, count: u16, panel_type: u8) -> Result<Self> {
        let value = Self { item_index, count, panel_type }; value.validate()?; Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.count == 0 { return Err(WireError::Invalid("zero purchase count")); }
        Ok(())
    }
}
checked_struct!(Intent, RawIntent, {
    request: PurchaseRequest, currency: Currency, source: Source, service_catalog_proof: Opaque32,
});
impl Intent { pub fn validate(&self) -> Result<()> { self.request.validate() } }
checked_struct!(Operation, RawOperation, {
    actor: Opaque32, request_scope: Opaque32, sequence: U64, intent: Intent,
});
impl Operation {
    pub fn validate(&self) -> Result<()> {
        if self.sequence.get() == 0 { return Err(WireError::Invalid("zero operation sequence")); }
        self.intent.validate()
    }
}
checked_struct!(Producer, RawProducer, { actor: Opaque32, producer_scope: Opaque32, server_revision: U64 });
impl Producer {
    pub fn validate(&self) -> Result<()> {
        if self.server_revision.get() == u64::MAX { return Err(WireError::Invalid("unknown revision sentinel")); }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Outcome {
    Committed { request: PurchaseRequest, currency: Currency, source: Source, charged: u32,
        admitted_count: u16, incoming_unique_id: U64 },
    Rejected { request: PurchaseRequest, reason: Rejection },
}
impl Outcome {
    pub fn validate_for_intent(&self, intent: &Intent) -> Result<()> {
        intent.validate()?;
        match self {
            Self::Committed { request, currency, source, admitted_count, .. } => {
                if request != &intent.request || currency != &intent.currency || source != &intent.source
                    || *admitted_count == 0 || *admitted_count > intent.request.count {
                    return Err(WireError::Invalid("committed outcome does not match intent"));
                }
            }
            Self::Rejected { request, .. } if request != &intent.request => {
                return Err(WireError::Invalid("rejected outcome does not match request"));
            }
            Self::Rejected { .. } => {}
        }
        Ok(())
    }
}
checked_struct!(ReceiptEntry, RawReceiptEntry, { operation: Operation, server_revision: U64, outcome: Outcome });
impl ReceiptEntry {
    pub fn validate(&self) -> Result<()> {
        self.operation.validate()?;
        if self.server_revision.get() == 0 || self.server_revision.get() == u64::MAX {
            return Err(WireError::Invalid("invalid terminal revision"));
        }
        self.outcome.validate_for_intent(&self.operation.intent)
    }
}
checked_struct!(Receipt, RawReceipt, { producer_scope: Opaque32, entry: ReceiptEntry });
impl Receipt {
    pub fn validate(&self) -> Result<()> { self.entry.validate() }
    pub fn validate_for_operation(&self, operation: &Operation) -> Result<()> {
        self.validate()?; operation.validate()?;
        if &self.entry.operation != operation { return Err(WireError::Invalid("receipt belongs to another operation")); }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Action {
    Begin,
    Quote { request: PurchaseRequest },
    Query { operation: Operation },
    Purchase { operation: Operation },
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
enum RawAction {
    Begin {},
    Quote { request: PurchaseRequest },
    Query { operation: Operation },
    Purchase { operation: Operation },
}
impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Ok(match RawAction::deserialize(deserializer)? {
            RawAction::Begin {} => Self::Begin,
            RawAction::Quote { request } => Self::Quote { request },
            RawAction::Query { operation } => Self::Query { operation },
            RawAction::Purchase { operation } => Self::Purchase { operation },
        })
    }
}
impl Action {
    pub fn is_mutation(&self) -> bool { matches!(self, Self::Begin | Self::Purchase { .. }) }
    pub fn operation(&self) -> Option<&Operation> {
        match self { Self::Query { operation } | Self::Purchase { operation } => Some(operation), _ => None }
    }
    pub fn validate(&self) -> Result<()> {
        match self { Self::Begin => Ok(()), Self::Quote { request } => request.validate(),
            Self::Query { operation } | Self::Purchase { operation } => operation.validate() }
    }
}
checked_struct!(ClientRequest, RawClientRequest, {
    #[serde(rename = "type")] wire_type: EnvelopeType, protocol_version: u16, request_id: U64, action: Action,
});
impl ClientRequest {
    pub fn new(request_id: U64, action: Action) -> Result<Self> {
        let value = Self { wire_type: EnvelopeType::NpcPurchaseOwner,
            protocol_version: NPC_PURCHASE_OWNER_PROTOCOL_VERSION, request_id, action };
        value.validate()?; Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != NPC_PURCHASE_OWNER_PROTOCOL_VERSION { return Err(WireError::Invalid("unsupported purchase protocol")); }
        if self.request_id.get() == 0 { return Err(WireError::Invalid("zero control request id")); }
        self.action.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureState { BeforeExecution, Unknown, PostCommit }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerReply {
    Producer { producer: Producer },
    Quote { intent: Intent },
    Recovery { receipt: Option<Receipt> },
    Purchase { receipt: Receipt, replayed: bool },
    Failure { state: FailureState, receipt: Option<Receipt> },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
enum RawServerReply {
    Producer { producer: Producer },
    Quote { intent: Intent },
    Recovery { #[serde(deserialize_with = "required_option")] receipt: Option<Receipt> },
    Purchase { receipt: Receipt, replayed: bool },
    Failure { state: FailureState, #[serde(deserialize_with = "required_option")] receipt: Option<Receipt> },
}
fn required_option<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where D: Deserializer<'de>, T: Deserialize<'de> {
    Option::<T>::deserialize(deserializer)
}
impl<'de> Deserialize<'de> for ServerReply {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let value = match RawServerReply::deserialize(deserializer)? {
            RawServerReply::Producer { producer } => Self::Producer { producer },
            RawServerReply::Quote { intent } => Self::Quote { intent },
            RawServerReply::Recovery { receipt } => Self::Recovery { receipt },
            RawServerReply::Purchase { receipt, replayed } => Self::Purchase { receipt, replayed },
            RawServerReply::Failure { state, receipt } => Self::Failure { state, receipt },
        };
        value.validate().map_err(de::Error::custom)?; Ok(value)
    }
}
impl Serialize for ServerReply {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.validate().map_err(serde::ser::Error::custom)?;
        match self.clone() {
            Self::Producer { producer } => RawServerReply::Producer { producer },
            Self::Quote { intent } => RawServerReply::Quote { intent },
            Self::Recovery { receipt } => RawServerReply::Recovery { receipt },
            Self::Purchase { receipt, replayed } => RawServerReply::Purchase { receipt, replayed },
            Self::Failure { state, receipt } => RawServerReply::Failure { state, receipt },
        }.serialize(serializer)
    }
}
impl ServerReply {
    pub fn receipt(&self) -> Option<&Receipt> {
        match self {
            Self::Recovery { receipt } | Self::Failure { receipt, .. } => receipt.as_ref(),
            Self::Purchase { receipt, .. } => Some(receipt), _ => None,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Producer { producer } => producer.validate(),
            Self::Quote { intent } => intent.validate(),
            Self::Recovery { receipt } => receipt.as_ref().map_or(Ok(()), Receipt::validate),
            Self::Purchase { receipt, .. } => receipt.validate(),
            Self::Failure { state, receipt } => {
                if matches!(state, FailureState::PostCommit) != receipt.is_some() {
                    return Err(WireError::Invalid("failure state and receipt disagree"));
                }
                receipt.as_ref().map_or(Ok(()), Receipt::validate)
            }
        }
    }
    /// Correlation preserves the original full operation. Query absence is
    /// valid schema, not permission to create another purchase.
    pub fn validate_for_action(&self, action: &Action) -> Result<()> {
        self.validate()?; action.validate()?;
        match (self, action) {
            (Self::Producer { .. }, Action::Begin) => Ok(()),
            (Self::Quote { intent }, Action::Quote { request }) if &intent.request == request => Ok(()),
            (Self::Recovery { receipt }, Action::Query { operation }) =>
                receipt.as_ref().map_or(Ok(()), |receipt| receipt.validate_for_operation(operation)),
            (Self::Purchase { receipt, .. }, Action::Purchase { operation }) => receipt.validate_for_operation(operation),
            (Self::Failure { receipt: None, .. }, _) => Ok(()),
            (Self::Failure { receipt: Some(receipt), .. }, Action::Query { operation } | Action::Purchase { operation }) =>
                receipt.validate_for_operation(operation),
            _ => Err(WireError::Invalid("reply does not match action")),
        }
    }
}
checked_struct!(ServerFrame, RawServerFrame, {
    #[serde(rename = "type")] wire_type: EnvelopeType, protocol_version: u16, request_id: U64, reply: ServerReply,
    #[serde(deserialize_with = "required_option")] snapshot: Option<Value>,
    #[serde(deserialize_with = "required_option")] authority: Option<Producer>,
});
impl ServerFrame {
    pub fn new(request_id: U64, reply: ServerReply, snapshot: Option<Value>, authority: Option<Producer>) -> Result<Self> {
        let value = Self { wire_type: EnvelopeType::NpcPurchaseOwner,
            protocol_version: NPC_PURCHASE_OWNER_PROTOCOL_VERSION, request_id, reply, snapshot, authority };
        value.validate()?; Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != NPC_PURCHASE_OWNER_PROTOCOL_VERSION { return Err(WireError::Invalid("unsupported purchase protocol")); }
        if self.request_id.get() == 0 { return Err(WireError::Invalid("zero control request id")); }
        self.reply.validate()?;
        if self.snapshot.is_some() != self.authority.is_some() || self.snapshot.as_ref().is_some_and(Value::is_null) {
            return Err(WireError::Invalid("snapshot and authority must be present together"));
        }
        if let Some(authority) = &self.authority {
            authority.validate()?;
            if let ServerReply::Producer { producer } = &self.reply {
                if producer != authority { return Err(WireError::Invalid("producer and authority disagree")); }
            }
            if let Some(receipt) = self.reply.receipt() {
                if receipt.entry.operation.actor != authority.actor || receipt.producer_scope != authority.producer_scope
                    || receipt.entry.server_revision > authority.server_revision {
                    return Err(WireError::Invalid("receipt and authority disagree"));
                }
            }
        }
        Ok(())
    }
    /// Control correlation is separate from the operation's durable scope and
    /// sequence. A delayed Begin or Quote cannot answer a newer control call.
    pub fn validate_for_request(&self, request: &ClientRequest) -> Result<()> {
        self.validate()?; request.validate()?;
        if self.request_id != request.request_id { return Err(WireError::Invalid("control request id mismatch")); }
        self.reply.validate_for_action(&request.action)
    }
}

// Keys are compared after JSON escape decoding. Never first parse into Value:
// its map would silently overwrite duplicates, including escaped duplicates.
struct StrictValue(Value);
impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictValue;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("JSON without duplicate keys") }
            fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Self::Value, E> { Ok(StrictValue(value.into())) }
            fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Self::Value, E> { Ok(StrictValue(value.into())) }
            fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Self::Value, E> { Ok(StrictValue(value.into())) }
            fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Self::Value, E> {
                serde_json::Number::from_f64(value).map(|number| StrictValue(Value::Number(number)))
                    .ok_or_else(|| E::custom("nonfinite JSON number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Self::Value, E> { Ok(StrictValue(value.into())) }
            fn visit_string<E: de::Error>(self, value: String) -> std::result::Result<Self::Value, E> { Ok(StrictValue(value.into())) }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Self::Value, E> { Ok(StrictValue(Value::Null)) }
            fn visit_none<E: de::Error>(self) -> std::result::Result<Self::Value, E> { Ok(StrictValue(Value::Null)) }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(StrictValue(value)) = sequence.next_element()? { values.push(value); }
                Ok(StrictValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Self::Value, A::Error> {
                let mut keys = BTreeSet::new(); let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !keys.insert(key.clone()) { return Err(de::Error::custom("duplicate JSON key")); }
                    let StrictValue(value) = map.next_value()?; values.insert(key, value);
                }
                Ok(StrictValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(StrictVisitor)
    }
}
fn parse<T: serde::de::DeserializeOwned>(text: &str, maximum: usize) -> Result<T> {
    if text.is_empty() || text.len() > maximum { return Err(WireError::Invalid("wire message size exceeded")); }
    let StrictValue(value) = serde_json::from_str(text)?;
    Ok(serde_json::from_value(value)?)
}
pub fn parse_client_request(text: &str) -> Result<ClientRequest> { parse(text, MAX_CLIENT_REQUEST_BYTES) }
pub fn parse_server_frame(text: &str) -> Result<ServerFrame> {
    if text.is_empty() || text.len() > MAX_SERVER_FRAME_BYTES {
        return Err(WireError::Invalid("wire message size exceeded"));
    }
    let StrictValue(value) = serde_json::from_str(text)?;
    value_decode::server_frame(&value)
}

/// Compact decoding of already duplicate-checked JSON values. These functions
/// establish the same typed schema and semantic validation as serde, not peer
/// authentication, connection custody, or a complete Applied checkpoint.
/// Raw external messages must still pass the bounded StrictValue walker above;
/// a Value alone cannot reveal keys overwritten by a different JSON parser.
pub mod value_decode {
    use super::*;
    use serde_json::Map;

    pub fn object<'a>(value: &'a Value, names: &[&str]) -> Result<&'a Map<String, Value>> {
        let value = value.as_object().ok_or(WireError::Invalid("expected object"))?;
        if value.len() != names.len() || names.iter().any(|name| !value.contains_key(*name)) {
            return Err(WireError::Invalid("missing or unknown field"));
        }
        Ok(value)
    }
    pub fn field<'a>(value: &'a Map<String, Value>, name: &str) -> Result<&'a Value> {
        value.get(name).ok_or(WireError::Invalid("missing field"))
    }
    pub fn string(value: &Value) -> Result<&str> {
        value.as_str().ok_or(WireError::Invalid("expected string"))
    }
    pub fn boolean(value: &Value) -> Result<bool> {
        value.as_bool().ok_or(WireError::Invalid("expected boolean"))
    }
    fn unsigned(value: &Value) -> Result<std::primitive::u64> {
        value.as_u64().ok_or(WireError::Invalid("expected unsigned integer"))
    }
    pub fn u8(value: &Value) -> Result<std::primitive::u8> {
        std::primitive::u8::try_from(unsigned(value)?).map_err(|_| WireError::Invalid("u8 overflow"))
    }
    pub fn u16(value: &Value) -> Result<std::primitive::u16> {
        std::primitive::u16::try_from(unsigned(value)?).map_err(|_| WireError::Invalid("u16 overflow"))
    }
    pub fn u32(value: &Value) -> Result<std::primitive::u32> {
        std::primitive::u32::try_from(unsigned(value)?).map_err(|_| WireError::Invalid("u32 overflow"))
    }
    pub fn u64(value: &Value) -> Result<U64> { U64::parse(string(value)?) }
    pub fn opaque(value: &Value) -> Result<Opaque32> { Opaque32::parse(string(value)?) }

    // ValueDeserializer accepts unit enums as strings or single null-valued
    // enum maps. Preserve that existing typed-serde input shape; serialization
    // remains the canonical string. Internally tagged kind/op tags are strings.
    pub fn unit_enum_name(value: &Value) -> Result<&str> { unit_enum_name_in(value, false) }
    fn unit_enum_name_in(value: &Value, buffered: bool) -> Result<&str> {
        match value {
            Value::String(value) => Ok(value),
            Value::Object(value) if value.len() == 1 => {
                let (name, payload) = value.iter().next().ok_or(WireError::Invalid("empty enum"))?;
                // ContentDeserializer's unit path additionally accepts an
                // empty map, for internally tagged newtype compatibility.
                // It does not ignore arbitrary enum payloads.
                if payload.is_null() || buffered && payload.as_object().is_some_and(Map::is_empty) {
                    Ok(name)
                } else { Err(WireError::Invalid("unit enum payload")) }
            }
            _ => Err(WireError::Invalid("expected unit enum")),
        }
    }
    pub fn currency(value: &Value) -> Result<Currency> { currency_in(value, false) }
    fn currency_in(value: &Value, buffered: bool) -> Result<Currency> {
        match unit_enum_name_in(value, buffered)? {
            "gold" => Ok(Currency::Gold), "pearls" => Ok(Currency::Pearls),
            _ => Err(WireError::Invalid("unknown currency")),
        }
    }
    pub fn source(value: &Value) -> Result<Source> { source_in(value, false) }
    fn source_in(value: &Value, buffered: bool) -> Result<Source> {
        match unit_enum_name_in(value, buffered)? {
            "trade" => Ok(Source::Trade), "buyBack" => Ok(Source::BuyBack), "used" => Ok(Source::Used),
            _ => Err(WireError::Invalid("unknown source")),
        }
    }
    pub fn rejection(value: &Value) -> Result<Rejection> { rejection_in(value, false) }
    fn rejection_in(value: &Value, buffered: bool) -> Result<Rejection> {
        match unit_enum_name_in(value, buffered)? {
            "invalidRequest" => Ok(Rejection::InvalidRequest), "playerDead" => Ok(Rejection::PlayerDead),
            "serviceUnavailable" => Ok(Rejection::ServiceUnavailable), "unsupportedService" => Ok(Rejection::UnsupportedService),
            "unknownGood" => Ok(Rejection::UnknownGood), "invalidQuantity" => Ok(Rejection::InvalidQuantity),
            "insufficientCurrency" => Ok(Rejection::InsufficientCurrency), "clockUnavailable" => Ok(Rejection::ClockUnavailable),
            "invalidDelivery" => Ok(Rejection::InvalidDelivery),
            _ => Err(WireError::Invalid("unknown rejection")),
        }
    }
    pub fn failure_state(value: &Value) -> Result<FailureState> { failure_state_in(value, false) }
    fn failure_state_in(value: &Value, buffered: bool) -> Result<FailureState> {
        match unit_enum_name_in(value, buffered)? {
            "beforeExecution" => Ok(FailureState::BeforeExecution), "unknown" => Ok(FailureState::Unknown),
            "postCommit" => Ok(FailureState::PostCommit),
            _ => Err(WireError::Invalid("unknown failure state")),
        }
    }

    // Derived ordinary structs also accept exact positional sequences. Keep
    // that compatibility for typed DTOs without weakening object field checks.
    enum Fields<'a> { Object(&'a Map<String, Value>), Sequence(&'a [Value]) }
    impl<'a> Fields<'a> {
        fn get(&self, name: &str, index: usize) -> Result<&'a Value> {
            match self {
                Self::Object(value) => field(value, name),
                Self::Sequence(value) => value.get(index).ok_or(WireError::Invalid("missing field")),
            }
        }
    }
    fn fields<'a>(value: &'a Value, names: &[&str]) -> Result<Fields<'a>> {
        match value {
            Value::Array(value) if value.len() == names.len() => Ok(Fields::Sequence(value)),
            _ => object(value, names).map(Fields::Object),
        }
    }

    pub fn purchase_request(value: &Value) -> Result<PurchaseRequest> {
        let value = fields(value, &["itemIndex", "count", "panelType"])?;
        PurchaseRequest::new(u64(value.get("itemIndex", 0)?)?, u16(value.get("count", 1)?)?, u8(value.get("panelType", 2)?)?)
    }
    pub fn intent(value: &Value) -> Result<Intent> { intent_in(value, false) }
    /// Match the ContentDeserializer context used inside a tagged bridge
    /// Request or ServerReply, including null/empty-map unit enum payloads.
    pub fn intent_buffered(value: &Value) -> Result<Intent> { intent_in(value, true) }
    fn intent_in(value: &Value, buffered: bool) -> Result<Intent> {
        let value = fields(value, &["request", "currency", "source", "serviceCatalogProof"])?;
        let value = Intent { request: purchase_request(value.get("request", 0)?)?, currency: currency_in(value.get("currency", 1)?, buffered)?,
            source: source_in(value.get("source", 2)?, buffered)?, service_catalog_proof: opaque(value.get("serviceCatalogProof", 3)?)? };
        value.validate()?; Ok(value)
    }
    pub fn operation(value: &Value) -> Result<Operation> { operation_in(value, false) }
    /// Match the ContentDeserializer context of a tagged bridge Request.
    pub fn operation_buffered(value: &Value) -> Result<Operation> { operation_in(value, true) }
    fn operation_in(value: &Value, buffered: bool) -> Result<Operation> {
        let value = fields(value, &["actor", "requestScope", "sequence", "intent"])?;
        let value = Operation { actor: opaque(value.get("actor", 0)?)?, request_scope: opaque(value.get("requestScope", 1)?)?,
            sequence: u64(value.get("sequence", 2)?)?, intent: intent_in(value.get("intent", 3)?, buffered)? };
        value.validate()?; Ok(value)
    }
    pub fn producer(value: &Value) -> Result<Producer> {
        let value = fields(value, &["actor", "producerScope", "serverRevision"])?;
        let value = Producer { actor: opaque(value.get("actor", 0)?)?, producer_scope: opaque(value.get("producerScope", 1)?)?,
            server_revision: u64(value.get("serverRevision", 2)?)? };
        value.validate()?; Ok(value)
    }
    pub fn outcome(value: &Value) -> Result<Outcome> { outcome_in(value, false) }
    // Internally tagged ServerReply buffers fields through serde Content,
    // whose external struct-variant payloads accept positional sequences.
    // Standalone ValueDeserializer accepts object payloads only. Preserve
    // both existing entry-point shapes rather than broadening either decoder.
    fn outcome_in(value: &Value, buffered: bool) -> Result<Outcome> {
        let value = value.as_object().ok_or(WireError::Invalid("expected outcome object"))?;
        if value.len() != 1 { return Err(WireError::Invalid("expected one outcome")); }
        let (kind, payload) = value.iter().next().ok_or(WireError::Invalid("empty outcome"))?;
        match kind.as_str() {
            "committed" => {
                let names = &["request", "currency", "source", "charged", "admittedCount", "incomingUniqueId"];
                let value = if buffered { fields(payload, names)? } else { Fields::Object(object(payload, names)?) };
                Ok(Outcome::Committed { request: purchase_request(value.get("request", 0)?)?, currency: currency_in(value.get("currency", 1)?, buffered)?,
                    source: source_in(value.get("source", 2)?, buffered)?, charged: u32(value.get("charged", 3)?)?,
                    admitted_count: u16(value.get("admittedCount", 4)?)?, incoming_unique_id: u64(value.get("incomingUniqueId", 5)?)? })
            }
            "rejected" => {
                let names = &["request", "reason"];
                let value = if buffered { fields(payload, names)? } else { Fields::Object(object(payload, names)?) };
                Ok(Outcome::Rejected { request: purchase_request(value.get("request", 0)?)?, reason: rejection_in(value.get("reason", 1)?, buffered)? })
            }
            _ => Err(WireError::Invalid("unknown outcome")),
        }
    }
    pub fn receipt_entry(value: &Value) -> Result<ReceiptEntry> { receipt_entry_in(value, false) }
    fn receipt_entry_in(value: &Value, buffered: bool) -> Result<ReceiptEntry> {
        let value = fields(value, &["operation", "serverRevision", "outcome"])?;
        let value = ReceiptEntry { operation: operation_in(value.get("operation", 0)?, buffered)?, server_revision: u64(value.get("serverRevision", 1)?)?,
            outcome: outcome_in(value.get("outcome", 2)?, buffered)? };
        value.validate()?; Ok(value)
    }
    pub fn receipt(value: &Value) -> Result<Receipt> { receipt_in(value, false) }
    fn receipt_in(value: &Value, buffered: bool) -> Result<Receipt> {
        let value = fields(value, &["producerScope", "entry"])?;
        let value = Receipt { producer_scope: opaque(value.get("producerScope", 0)?)?, entry: receipt_entry_in(value.get("entry", 1)?, buffered)? };
        value.validate()?; Ok(value)
    }
    fn nullable_receipt(value: &Value) -> Result<Option<Receipt>> {
        if value.is_null() { Ok(None) } else { receipt_in(value, true).map(Some) }
    }
    pub fn server_reply(value: &Value) -> Result<ServerReply> {
        // TaggedContentVisitor also accepts [tag, variant fields...]. The tag
        // is still a string, and each variant keeps its exact positional length.
        let kind = match value {
            Value::Array(value) => value.first().ok_or(WireError::Invalid("missing reply kind"))?,
            Value::Object(value) => field(value, "kind")?,
            _ => return Err(WireError::Invalid("expected reply")),
        };
        let value = match string(kind)? {
            "producer" => {
                let value = fields(value, &["kind", "producer"])?;
                ServerReply::Producer { producer: producer(value.get("producer", 1)?)? }
            }
            "quote" => {
                let value = fields(value, &["kind", "intent"])?;
                ServerReply::Quote { intent: intent_in(value.get("intent", 1)?, true)? }
            }
            "recovery" => {
                let value = fields(value, &["kind", "receipt"])?;
                ServerReply::Recovery { receipt: nullable_receipt(value.get("receipt", 1)?)? }
            }
            "purchase" => {
                let value = fields(value, &["kind", "receipt", "replayed"])?;
                ServerReply::Purchase { receipt: receipt_in(value.get("receipt", 1)?, true)?, replayed: boolean(value.get("replayed", 2)?)? }
            }
            "failure" => {
                let value = fields(value, &["kind", "state", "receipt"])?;
                ServerReply::Failure { state: failure_state_in(value.get("state", 1)?, true)?, receipt: nullable_receipt(value.get("receipt", 2)?)? }
            }
            _ => return Err(WireError::Invalid("unknown reply")),
        };
        value.validate()?; Ok(value)
    }
    pub fn server_frame(value: &Value) -> Result<ServerFrame> {
        let value = fields(value, &["type", "protocolVersion", "requestId", "reply", "snapshot", "authority"])?;
        if unit_enum_name(value.get("type", 0)?)? != "npcPurchaseOwner" {
            return Err(WireError::Invalid("unknown envelope type"));
        }
        let snapshot = value.get("snapshot", 4)?;
        let authority = value.get("authority", 5)?;
        let value = ServerFrame { wire_type: EnvelopeType::NpcPurchaseOwner, protocol_version: u16(value.get("protocolVersion", 1)?)?,
            request_id: u64(value.get("requestId", 2)?)?, reply: server_reply(value.get("reply", 3)?)?,
            snapshot: if snapshot.is_null() { None } else { Some(snapshot.clone()) },
            authority: if authority.is_null() { None } else { Some(producer(authority)?) } };
        value.validate()?; Ok(value)
    }
}
/// Compact Value construction with the existing typed serialization rules.
/// This changes neither authentication nor receipt/Applied custody. Validate
/// before emitting any public DTO, and keep snapshots uninterpreted carriers.
pub mod value_encode {
    use super::*;

    macro_rules! object {
        ($($key:literal => $value:expr),* $(,)?) => {{
            let mut fields = serde_json::Map::new();
            $(fields.insert($key.to_owned(), $value);)*
            Value::Object(fields)
        }};
    }
    pub fn u64(value: U64) -> Value { Value::String(value.get().to_string()) }
    pub fn opaque(value: Opaque32) -> Value { Value::String(value.hex()) }
    pub fn currency_name(value: Currency) -> &'static str {
        match value { Currency::Gold => "gold", Currency::Pearls => "pearls" }
    }
    pub fn source_name(value: Source) -> &'static str {
        match value { Source::Trade => "trade", Source::BuyBack => "buyBack", Source::Used => "used" }
    }
    pub fn rejection_name(value: Rejection) -> &'static str {
        match value {
            Rejection::InvalidRequest => "invalidRequest", Rejection::PlayerDead => "playerDead",
            Rejection::ServiceUnavailable => "serviceUnavailable", Rejection::UnsupportedService => "unsupportedService",
            Rejection::UnknownGood => "unknownGood", Rejection::InvalidQuantity => "invalidQuantity",
            Rejection::InsufficientCurrency => "insufficientCurrency", Rejection::ClockUnavailable => "clockUnavailable",
            Rejection::InvalidDelivery => "invalidDelivery",
        }
    }
    pub fn failure_state_name(value: FailureState) -> &'static str {
        match value { FailureState::BeforeExecution => "beforeExecution", FailureState::Unknown => "unknown", FailureState::PostCommit => "postCommit" }
    }
    pub fn currency(value: Currency) -> Value { Value::String(currency_name(value).into()) }
    pub fn source(value: Source) -> Value { Value::String(source_name(value).into()) }
    pub fn rejection(value: Rejection) -> Value { Value::String(rejection_name(value).into()) }
    pub fn failure_state(value: FailureState) -> Value { Value::String(failure_state_name(value).into()) }
    pub fn purchase_request(value: &PurchaseRequest) -> Result<Value> {
        value.validate()?;
        Ok(object!("itemIndex" => u64(value.item_index), "count" => value.count.into(), "panelType" => value.panel_type.into()))
    }
    pub fn intent(value: &Intent) -> Result<Value> {
        value.validate()?;
        Ok(object!("request" => purchase_request(&value.request)?, "currency" => currency(value.currency),
            "source" => source(value.source), "serviceCatalogProof" => opaque(value.service_catalog_proof)))
    }
    pub fn operation(value: &Operation) -> Result<Value> {
        value.validate()?;
        Ok(object!("actor" => opaque(value.actor), "requestScope" => opaque(value.request_scope),
            "sequence" => u64(value.sequence), "intent" => intent(&value.intent)?))
    }
    pub fn producer(value: &Producer) -> Result<Value> {
        value.validate()?;
        Ok(object!("actor" => opaque(value.actor), "producerScope" => opaque(value.producer_scope), "serverRevision" => u64(value.server_revision)))
    }
    pub fn outcome(value: &Outcome) -> Result<Value> {
        // Standalone Outcome serde validates its nested checked request, not
        // a missing external intent. ReceiptEntry supplies tuple validation.
        Ok(match value {
            Outcome::Committed { request, currency: paid_currency, source: paid_source, charged, admitted_count, incoming_unique_id } =>
                object!("committed" => object!("request" => purchase_request(request)?, "currency" => currency(*paid_currency),
                    "source" => source(*paid_source), "charged" => (*charged).into(), "admittedCount" => (*admitted_count).into(),
                    "incomingUniqueId" => u64(*incoming_unique_id))),
            Outcome::Rejected { request, reason } => object!("rejected" => object!("request" => purchase_request(request)?, "reason" => rejection(*reason))),
        })
    }
    pub fn receipt_entry(value: &ReceiptEntry) -> Result<Value> {
        value.validate()?;
        Ok(object!("operation" => operation(&value.operation)?, "serverRevision" => u64(value.server_revision), "outcome" => outcome(&value.outcome)?))
    }
    pub fn receipt(value: &Receipt) -> Result<Value> {
        value.validate()?;
        Ok(object!("producerScope" => opaque(value.producer_scope), "entry" => receipt_entry(&value.entry)?))
    }
    pub fn action(value: &Action) -> Result<Value> {
        value.validate()?;
        Ok(match value {
            Action::Begin => object!("kind" => Value::String("begin".into())),
            Action::Quote { request } => object!("kind" => Value::String("quote".into()), "request" => purchase_request(request)?),
            Action::Query { operation: original } => object!("kind" => Value::String("query".into()), "operation" => operation(original)?),
            Action::Purchase { operation: original } => object!("kind" => Value::String("purchase".into()), "operation" => operation(original)?),
        })
    }
    pub fn client_request(value: &ClientRequest) -> Result<Value> {
        value.validate()?;
        Ok(object!("type" => Value::String("npcPurchaseOwner".into()), "protocolVersion" => value.protocol_version.into(),
            "requestId" => u64(value.request_id), "action" => action(&value.action)?))
    }
    pub fn server_reply(value: &ServerReply) -> Result<Value> {
        value.validate()?;
        Ok(match value {
            ServerReply::Producer { producer: current } => object!("kind" => Value::String("producer".into()), "producer" => producer(current)?),
            ServerReply::Quote { intent: quoted } => object!("kind" => Value::String("quote".into()), "intent" => intent(quoted)?),
            ServerReply::Recovery { receipt: terminal } => object!("kind" => Value::String("recovery".into()),
                "receipt" => terminal.as_ref().map(receipt).transpose()?.unwrap_or(Value::Null)),
            ServerReply::Purchase { receipt: terminal, replayed } => object!("kind" => Value::String("purchase".into()),
                "receipt" => receipt(terminal)?, "replayed" => Value::Bool(*replayed)),
            ServerReply::Failure { state, receipt: terminal } => object!("kind" => Value::String("failure".into()),
                "state" => failure_state(*state), "receipt" => terminal.as_ref().map(receipt).transpose()?.unwrap_or(Value::Null)),
        })
    }
    pub fn server_frame(value: &ServerFrame) -> Result<Value> {
        value.validate()?;
        Ok(object!("type" => Value::String("npcPurchaseOwner".into()), "protocolVersion" => value.protocol_version.into(),
            "requestId" => u64(value.request_id), "reply" => server_reply(&value.reply)?,
            "snapshot" => value.snapshot.as_ref().cloned().unwrap_or(Value::Null),
            "authority" => value.authority.as_ref().map(producer).transpose()?.unwrap_or(Value::Null)))
    }
}

/// Web-only call sites may use this additive encoder to avoid reaching typed
/// serde serializers. Its byte order is the original checked DTO serializer's
/// order, including tag-first Action. Every interpolated string is canonical
/// decimal, lowercase hex, or a closed enum spelling after full validation;
/// no arbitrary user text, snapshot, or general JSON escaping enters this body.
pub fn encode_client_request_compact(value: &ClientRequest) -> Result<String> {
    value.validate()?;
    fn request(value: &PurchaseRequest) -> String {
        format!("{{\"itemIndex\":\"{}\",\"count\":{},\"panelType\":{}}}", value.item_index.get(), value.count, value.panel_type)
    }
    fn operation(value: &Operation) -> String {
        format!("{{\"actor\":\"{}\",\"requestScope\":\"{}\",\"sequence\":\"{}\",\"intent\":{{\"request\":{},\"currency\":\"{}\",\"source\":\"{}\",\"serviceCatalogProof\":\"{}\"}}}}",
            value.actor.hex(), value.request_scope.hex(), value.sequence.get(), request(&value.intent.request),
            value_encode::currency_name(value.intent.currency), value_encode::source_name(value.intent.source), value.intent.service_catalog_proof.hex())
    }
    let action = match &value.action {
        Action::Begin => "{\"kind\":\"begin\"}".into(),
        Action::Quote { request: value } => format!("{{\"kind\":\"quote\",\"request\":{}}}", request(value)),
        Action::Query { operation: value } => format!("{{\"kind\":\"query\",\"operation\":{}}}", operation(value)),
        Action::Purchase { operation: value } => format!("{{\"kind\":\"purchase\",\"operation\":{}}}", operation(value)),
    };
    let text = format!("{{\"type\":\"npcPurchaseOwner\",\"protocolVersion\":{},\"requestId\":\"{}\",\"action\":{}}}",
        value.protocol_version, value.request_id.get(), action);
    if text.len() > MAX_CLIENT_REQUEST_BYTES { return Err(WireError::Invalid("wire message size exceeded")); }
    Ok(text)
}

pub fn encode_client_request(value: &ClientRequest) -> Result<String> {
    let text = serde_json::to_string(value)?;
    if text.len() > MAX_CLIENT_REQUEST_BYTES { return Err(WireError::Invalid("wire message size exceeded")); }
    Ok(text)
}
pub fn encode_server_frame(value: &ServerFrame) -> Result<String> {
    let text = serde_json::to_string(value)?;
    if text.len() > MAX_SERVER_FRAME_BYTES { return Err(WireError::Invalid("wire message size exceeded")); }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // These are schema specimens only. They are neither captured execution
    // receipts nor evidence of an authenticated or fully applied snapshot.
    fn opaque(byte: u8) -> Opaque32 { Opaque32::from_bytes([byte; 32]).unwrap() }
    fn intent(currency: Currency, source: Source) -> Intent {
        Intent { request: PurchaseRequest::new(U64::new(u64::MAX), 3, 0).unwrap(),
            currency, source, service_catalog_proof: opaque(3) }
    }
    fn operation() -> Operation {
        Operation { actor: opaque(1), request_scope: opaque(2), sequence: U64::new(u64::MAX),
            intent: intent(Currency::Gold, Source::Trade) }
    }
    fn receipt() -> Receipt {
        let operation = operation();
        Receipt { producer_scope: opaque(4), entry: ReceiptEntry {
            outcome: Outcome::Committed { request: operation.intent.request.clone(), currency: Currency::Gold,
                source: Source::Trade, charged: 0, admitted_count: 2, incoming_unique_id: U64::new(0) },
            operation, server_revision: U64::new(8) } }
    }
    fn producer() -> Producer { Producer { actor: opaque(1), producer_scope: opaque(4), server_revision: U64::new(9) } }
    fn frame() -> ServerFrame {
        ServerFrame::new(U64::new(11), ServerReply::Purchase { receipt: receipt(), replayed: false },
            Some(json!({"schemaSpecimen": true})), Some(producer())).unwrap()
    }
    fn rejected(mut receipt: Receipt, reason: Rejection) -> Receipt {
        receipt.entry.outcome = Outcome::Rejected { request: receipt.entry.operation.intent.request.clone(), reason };
        receipt
    }

    fn assert_value_decode<T>(value: &Value, decode: fn(&Value) -> Result<T>, accepted: bool)
    where T: serde::de::DeserializeOwned + fmt::Debug + PartialEq {
        let original = serde_json::from_value::<T>(value.clone());
        let compact = decode(value);
        assert_eq!(original.is_ok(), accepted, "serde oracle: {value}: {original:?}");
        assert_eq!(compact.is_ok(), accepted, "compact decoder: {value}: {compact:?}");
        if let (Ok(original), Ok(compact)) = (original, compact) { assert_eq!(compact, original); }
    }
    fn assert_frame_decode(raw: &str, accepted: bool) {
        let original = parse::<ServerFrame>(raw, MAX_SERVER_FRAME_BYTES);
        let compact = parse_server_frame(raw);
        assert_eq!(original.is_ok(), accepted, "serde raw oracle: {original:?}");
        assert_eq!(compact.is_ok(), accepted, "compact raw decoder: {compact:?}");
        if let (Ok(original), Ok(compact)) = (original, compact) { assert_eq!(compact, original); }
    }

    fn assert_value_encode<T>(value: &T, encode: fn(&T) -> Result<Value>, accepted: bool)
    where T: Serialize + fmt::Debug {
        let original = serde_json::to_value(value);
        let compact = encode(value);
        assert_eq!(original.is_ok(), accepted, "serde encode oracle: {value:?}: {original:?}");
        assert_eq!(compact.is_ok(), accepted, "compact encode: {value:?}: {compact:?}");
        if let (Ok(original), Ok(compact)) = (original, compact) { assert_eq!(compact, original); }
    }

    #[test]
    fn schema_value_encode_all_valid_variants_and_boundaries_match_serde() {
        for currency in [Currency::Gold, Currency::Pearls] {
            assert_eq!(value_encode::currency(currency), serde_json::to_value(currency).unwrap());
            for source in [Source::Trade, Source::BuyBack, Source::Used] {
                assert_eq!(value_encode::source(source), serde_json::to_value(source).unwrap());
                for (selector, count, panel) in [(0, 1, 0), (u64::MAX, u16::MAX, u8::MAX)] {
                    let mut terminal = receipt();
                    terminal.entry.operation.intent = Intent { request: PurchaseRequest::new(U64::new(selector), count, panel).unwrap(),
                        currency, source, service_catalog_proof: opaque(3) };
                    terminal.entry.outcome = Outcome::Committed { request: terminal.entry.operation.intent.request.clone(), currency, source,
                        charged: u32::MAX, admitted_count: count, incoming_unique_id: U64::new(selector) };
                    terminal.entry.server_revision = U64::new(u64::MAX - 1);
                    assert_value_encode(&terminal.entry.operation.intent.request, value_encode::purchase_request, true);
                    assert_value_encode(&terminal.entry.operation.intent, value_encode::intent, true);
                    assert_value_encode(&terminal.entry.operation, value_encode::operation, true);
                    assert_value_encode(&terminal.entry.outcome, value_encode::outcome, true);
                    assert_value_encode(&terminal.entry, value_encode::receipt_entry, true);
                    assert_value_encode(&terminal, value_encode::receipt, true);
                    for action in [Action::Begin, Action::Quote { request: terminal.entry.operation.intent.request.clone() },
                        Action::Query { operation: terminal.entry.operation.clone() }, Action::Purchase { operation: terminal.entry.operation.clone() }] {
                        assert_value_encode(&action, value_encode::action, true);
                        let request = ClientRequest::new(U64::new(u64::MAX), action).unwrap();
                        assert_value_encode(&request, value_encode::client_request, true);
                    }
                }
            }
        }
        for revision in [0, u64::MAX - 1] {
            assert_value_encode(&Producer { server_revision: U64::new(revision), ..producer() }, value_encode::producer, true);
        }
        for reason in [Rejection::InvalidRequest, Rejection::PlayerDead, Rejection::ServiceUnavailable,
            Rejection::UnsupportedService, Rejection::UnknownGood, Rejection::InvalidQuantity,
            Rejection::InsufficientCurrency, Rejection::ClockUnavailable, Rejection::InvalidDelivery] {
            assert_eq!(value_encode::rejection(reason), serde_json::to_value(reason).unwrap());
            let terminal = rejected(receipt(), reason);
            assert_value_encode(&terminal.entry.outcome, value_encode::outcome, true);
            assert_value_encode(&terminal, value_encode::receipt, true);
        }
        for state in [FailureState::BeforeExecution, FailureState::Unknown, FailureState::PostCommit] {
            assert_eq!(value_encode::failure_state(state), serde_json::to_value(state).unwrap());
        }
        for raw in [0, 1, u64::MAX] { assert_eq!(value_encode::u64(U64::new(raw)), serde_json::to_value(U64::new(raw)).unwrap()); }
        let mut bytes = [0; 32]; bytes[0] = 0xab; bytes[31] = 0x09;
        let identity = Opaque32::from_bytes(bytes).unwrap();
        assert_eq!(value_encode::opaque(identity), serde_json::to_value(identity).unwrap());
        let replies = [ServerReply::Producer { producer: producer() }, ServerReply::Quote { intent: operation().intent },
            ServerReply::Recovery { receipt: None }, ServerReply::Recovery { receipt: Some(receipt()) },
            ServerReply::Purchase { receipt: receipt(), replayed: false }, ServerReply::Purchase { receipt: receipt(), replayed: true },
            ServerReply::Failure { state: FailureState::BeforeExecution, receipt: None },
            ServerReply::Failure { state: FailureState::Unknown, receipt: None },
            ServerReply::Failure { state: FailureState::PostCommit, receipt: Some(receipt()) }];
        for reply in replies {
            assert_value_encode(&reply, value_encode::server_reply, true);
            for paired in [false, true] {
                let snapshot = json!({"public":[null,true,u64::MAX,i64::MIN,-1,1.25,{"arbitrary":"quote\" slash\\ Unicode英雄"}]});
                let frame = ServerFrame::new(U64::new(11), reply.clone(), paired.then_some(snapshot), paired.then_some(producer())).unwrap();
                assert_value_encode(&frame, value_encode::server_frame, true);
                let encoded = value_encode::server_frame(&frame).unwrap();
                assert_eq!(encoded["snapshot"], frame.snapshot.clone().unwrap_or(Value::Null));
                assert!(encoded.as_object().unwrap().contains_key("snapshot"));
                assert!(encoded.as_object().unwrap().contains_key("authority"));
            }
        }
        // Outcome has no external intent when serialized alone. Preserve the
        // old distinction: ReceiptEntry, not this serializer, rejects zero.
        let standalone = Outcome::Committed { request: operation().intent.request, currency: Currency::Gold, source: Source::Trade,
            charged: 0, admitted_count: 0, incoming_unique_id: U64::new(0) };
        assert_value_encode(&standalone, value_encode::outcome, true);
    }

    #[test]
    fn schema_value_encode_invalid_public_dtos_match_serde_rejection() {
        let invalid_request = PurchaseRequest { item_index: U64::new(0), count: 0, panel_type: u8::MAX };
        assert_value_encode(&invalid_request, value_encode::purchase_request, false);
        let invalid_intent = Intent { request: invalid_request.clone(), ..operation().intent };
        assert_value_encode(&invalid_intent, value_encode::intent, false);
        let invalid_operation = Operation { sequence: U64::new(0), ..operation() };
        assert_value_encode(&invalid_operation, value_encode::operation, false);
        let invalid_producer = Producer { server_revision: U64::new(u64::MAX), ..producer() };
        assert_value_encode(&invalid_producer, value_encode::producer, false);
        for outcome in [Outcome::Committed { request: invalid_request.clone(), currency: Currency::Gold, source: Source::Trade,
            charged: 0, admitted_count: 1, incoming_unique_id: U64::new(0) },
            Outcome::Rejected { request: invalid_request.clone(), reason: Rejection::InvalidRequest }] {
            assert_value_encode(&outcome, value_encode::outcome, false);
        }
        let mut invalid_receipts = Vec::new();
        for revision in [0, u64::MAX] {
            let mut value = receipt(); value.entry.server_revision = U64::new(revision); invalid_receipts.push(value);
        }
        for change in 0..9 {
            let mut value = receipt();
            if let Outcome::Committed { request, currency, source, admitted_count, .. } = &mut value.entry.outcome {
                match change {
                    0 => request.item_index = U64::new(0), 1 => request.count = 2, 2 => request.panel_type = 1,
                    3 => *currency = Currency::Pearls, 4 => *source = Source::Used, 5 => *admitted_count = 0,
                    6 => *admitted_count = 4, 7 => value.entry.operation.sequence = U64::new(0),
                    _ => value.entry.operation.intent.request.count = 0,
                }
            }
            invalid_receipts.push(value);
        }
        for value in invalid_receipts {
            assert_value_encode(&value.entry, value_encode::receipt_entry, false);
            assert_value_encode(&value, value_encode::receipt, false);
            for reply in [ServerReply::Recovery { receipt: Some(value.clone()) }, ServerReply::Purchase { receipt: value.clone(), replayed: false },
                ServerReply::Failure { state: FailureState::PostCommit, receipt: Some(value.clone()) }] {
                assert_value_encode(&reply, value_encode::server_reply, false);
            }
        }
        for action in [Action::Quote { request: invalid_request }, Action::Query { operation: invalid_operation.clone() },
            Action::Purchase { operation: invalid_operation }] {
            assert_value_encode(&action, value_encode::action, false);
            let request = ClientRequest { action, ..ClientRequest::new(U64::new(1), Action::Begin).unwrap() };
            assert_value_encode(&request, value_encode::client_request, false);
            assert!(encode_client_request(&request).is_err()); assert!(encode_client_request_compact(&request).is_err());
        }
        for request in [ClientRequest { request_id: U64::new(0), ..ClientRequest::new(U64::new(1), Action::Begin).unwrap() },
            ClientRequest { protocol_version: 2, ..ClientRequest::new(U64::new(1), Action::Begin).unwrap() }] {
            assert_value_encode(&request, value_encode::client_request, false);
            assert!(encode_client_request(&request).is_err()); assert!(encode_client_request_compact(&request).is_err());
        }
        for reply in [ServerReply::Producer { producer: invalid_producer }, ServerReply::Quote { intent: invalid_intent },
            ServerReply::Failure { state: FailureState::PostCommit, receipt: None },
            ServerReply::Failure { state: FailureState::Unknown, receipt: Some(receipt()) },
            ServerReply::Failure { state: FailureState::BeforeExecution, receipt: Some(receipt()) }] {
            assert_value_encode(&reply, value_encode::server_reply, false);
        }
        for change in 0..8 {
            let mut value = frame();
            match change {
                0 => value.snapshot = None, 1 => value.authority = None, 2 => value.snapshot = Some(Value::Null),
                3 => value.authority.as_mut().unwrap().actor = opaque(9),
                4 => value.authority.as_mut().unwrap().producer_scope = opaque(9),
                5 => value.authority.as_mut().unwrap().server_revision = U64::new(7),
                6 => value.protocol_version = 2, _ => value.request_id = U64::new(0),
            }
            assert_value_encode(&value, value_encode::server_frame, false);
        }
        let mut mismatch = ServerFrame::new(U64::new(1), ServerReply::Producer { producer: producer() },
            Some(json!({})), Some(producer())).unwrap();
        mismatch.authority.as_mut().unwrap().server_revision = U64::new(8);
        assert_value_encode(&mismatch, value_encode::server_frame, false);
    }

    #[test]
    fn schema_compact_client_request_bytes_match_original_encoder() {
        let mut bytes = [0; 32]; bytes[0] = 0xab; bytes[31] = 0x09;
        let identity = Opaque32::from_bytes(bytes).unwrap();
        for currency in [Currency::Gold, Currency::Pearls] {
            for source in [Source::Trade, Source::BuyBack, Source::Used] {
                for selector in [0, 1, u64::MAX] {
                    for count in [1, u16::MAX] {
                        for panel in [0, 1, u8::MAX] {
                            let original = Operation { actor: identity, request_scope: opaque(2), sequence: U64::new(u64::MAX),
                                intent: Intent { request: PurchaseRequest::new(U64::new(selector), count, panel).unwrap(), currency, source,
                                    service_catalog_proof: identity } };
                            for action in [Action::Begin, Action::Quote { request: original.intent.request.clone() },
                                Action::Query { operation: original.clone() }, Action::Purchase { operation: original.clone() }] {
                                for control in [1, u64::MAX] {
                                    let value = ClientRequest::new(U64::new(control), action.clone()).unwrap();
                                    let expected = encode_client_request(&value).unwrap();
                                    assert_eq!(expected, serde_json::to_string(&value).unwrap());
                                    let compact = encode_client_request_compact(&value).unwrap();
                                    assert_eq!(compact.as_bytes(), expected.as_bytes());
                                    assert_eq!(parse_client_request(&compact).unwrap(), value);
                                    assert_eq!(serde_json::from_str::<Value>(&compact).unwrap(), value_encode::client_request(&value).unwrap());
                                    assert!(compact.len() <= MAX_CLIENT_REQUEST_BYTES);
                                }
                            }
                        }
                    }
                }
            }
        }
        let begin = ClientRequest::new(U64::new(1), Action::Begin).unwrap();
        assert_eq!(encode_client_request_compact(&begin).unwrap(),
            "{\"type\":\"npcPurchaseOwner\",\"protocolVersion\":1,\"requestId\":\"1\",\"action\":{\"kind\":\"begin\"}}");
    }

    #[test]
    fn schema_value_decode_all_replies_and_integer_boundaries_match_serde() {
        let replies = [ServerReply::Producer { producer: producer() }, ServerReply::Quote { intent: operation().intent },
            ServerReply::Recovery { receipt: None }, ServerReply::Recovery { receipt: Some(receipt()) },
            ServerReply::Purchase { receipt: receipt(), replayed: false }, ServerReply::Purchase { receipt: receipt(), replayed: true },
            ServerReply::Failure { state: FailureState::BeforeExecution, receipt: None },
            ServerReply::Failure { state: FailureState::Unknown, receipt: None },
            ServerReply::Failure { state: FailureState::PostCommit, receipt: Some(receipt()) }];
        for reply in replies {
            assert_value_decode(&serde_json::to_value(&reply).unwrap(), value_decode::server_reply, true);
            for paired in [false, true] {
                let carrier = json!({"nested":[null, true, -1, u64::MAX, {"public":"source","fraction":1.25}]});
                let specimen = ServerFrame::new(U64::new(u64::MAX), reply.clone(), paired.then_some(carrier), paired.then_some(producer())).unwrap();
                let value = serde_json::to_value(&specimen).unwrap();
                assert_value_decode(&value, value_decode::server_frame, true);
                assert_frame_decode(&value.to_string(), true);
                assert_eq!(value_decode::server_frame(&value).unwrap().snapshot, specimen.snapshot);
            }
        }
        for (currency, source) in [(Currency::Gold, Source::Trade), (Currency::Pearls, Source::Trade),
            (Currency::Gold, Source::BuyBack), (Currency::Gold, Source::Used), (Currency::Pearls, Source::Used)] {
            for selector in [0, u64::MAX] {
                let mut specimen = receipt();
                specimen.entry.operation.intent = Intent { request: PurchaseRequest::new(U64::new(selector), u16::MAX, u8::MAX).unwrap(),
                    currency, source, service_catalog_proof: opaque(3) };
                specimen.entry.server_revision = U64::new(u64::MAX - 1);
                specimen.entry.outcome = Outcome::Committed { request: specimen.entry.operation.intent.request.clone(), currency, source,
                    charged: u32::MAX, admitted_count: u16::MAX, incoming_unique_id: U64::new(selector) };
                assert_value_decode(&serde_json::to_value(&specimen.entry.operation.intent.request).unwrap(), value_decode::purchase_request, true);
                assert_value_decode(&serde_json::to_value(&specimen.entry.operation.intent).unwrap(), value_decode::intent, true);
                assert_value_decode(&serde_json::to_value(&specimen.entry.operation).unwrap(), value_decode::operation, true);
                assert_value_decode(&serde_json::to_value(&specimen.entry.outcome).unwrap(), value_decode::outcome, true);
                assert_value_decode(&serde_json::to_value(&specimen.entry).unwrap(), value_decode::receipt_entry, true);
                assert_value_decode(&serde_json::to_value(&specimen).unwrap(), value_decode::receipt, true);
            }
        }
        for revision in [0, u64::MAX - 1] {
            let specimen = Producer { server_revision: U64::new(revision), ..producer() };
            assert_value_decode(&serde_json::to_value(specimen).unwrap(), value_decode::producer, true);
        }
        for reason in [Rejection::InvalidRequest, Rejection::PlayerDead, Rejection::ServiceUnavailable,
            Rejection::UnsupportedService, Rejection::UnknownGood, Rejection::InvalidQuantity,
            Rejection::InsufficientCurrency, Rejection::ClockUnavailable, Rejection::InvalidDelivery] {
            assert_value_decode(&serde_json::to_value(rejected(receipt(), reason)).unwrap(), value_decode::receipt, true);
        }
    }

    #[test]
    fn schema_value_decode_missing_extra_and_wrong_shapes_match_serde() {
        let valid = serde_json::to_value(frame()).unwrap();
        for path in ["", "/reply", "/reply/receipt", "/reply/receipt/entry", "/reply/receipt/entry/operation",
            "/reply/receipt/entry/operation/intent", "/reply/receipt/entry/operation/intent/request",
            "/reply/receipt/entry/outcome/committed", "/reply/receipt/entry/outcome/committed/request", "/authority"] {
            let keys: Vec<_> = valid.pointer(path).unwrap().as_object().unwrap().keys().cloned().collect();
            for key in keys {
                let mut value = valid.clone(); value.pointer_mut(path).unwrap().as_object_mut().unwrap().remove(&key);
                assert_value_decode(&value, value_decode::server_frame, false);
            }
            let mut value = valid.clone(); value.pointer_mut(path).unwrap().as_object_mut().unwrap().insert("extra".into(), Value::Null);
            assert_value_decode(&value, value_decode::server_frame, false);
        }
        for (path, malformed) in [("/reply/kind", json!("unknown")), ("/reply/kind", json!({"purchase":null})),
            ("/reply/replayed", json!(0)), ("/reply/receipt", Value::Null), ("/protocolVersion", json!(1.0)),
            ("/authority", json!([])), ("/reply/receipt/entry/outcome", json!({"unknown":{}})),
            ("/reply/receipt/entry/outcome/committed", Value::Null)] {
            let mut value = valid.clone(); *value.pointer_mut(path).unwrap() = malformed;
            assert_value_decode(&value, value_decode::server_frame, false);
        }
        for reply in [json!({"kind":"recovery","receipt":null}), json!({"kind":"failure","state":"unknown","receipt":null})] {
            assert_value_decode(&reply, value_decode::server_reply, true);
            let mut missing = reply; missing.as_object_mut().unwrap().remove("receipt");
            assert_value_decode(&missing, value_decode::server_reply, false);
        }
        for value in [Value::Null, json!(true), json!(1), json!("frame"), json!([])] {
            assert_value_decode(&value, value_decode::server_frame, false);
        }
    }

    #[test]
    fn schema_value_decode_semantic_tuple_mutations_match_serde() {
        let valid = serde_json::to_value(frame()).unwrap();
        for (path, malformed) in [("/protocolVersion", json!(2)), ("/requestId", json!("0")),
            ("/reply/receipt/entry/operation/sequence", json!("0")), ("/reply/receipt/entry/serverRevision", json!("0")),
            ("/reply/receipt/entry/serverRevision", json!(u64::MAX.to_string())),
            ("/reply/receipt/entry/outcome/committed/request/itemIndex", json!("0")),
            ("/reply/receipt/entry/outcome/committed/request/count", json!(2)),
            ("/reply/receipt/entry/outcome/committed/request/panelType", json!(1)),
            ("/reply/receipt/entry/outcome/committed/currency", json!("pearls")),
            ("/reply/receipt/entry/outcome/committed/source", json!("used")),
            ("/reply/receipt/entry/outcome/committed/admittedCount", json!(0)),
            ("/reply/receipt/entry/outcome/committed/admittedCount", json!(4)),
            ("/authority/actor", serde_json::to_value(opaque(9)).unwrap()),
            ("/authority/producerScope", serde_json::to_value(opaque(9)).unwrap()),
            ("/authority/serverRevision", json!("7")), ("/authority/serverRevision", json!(u64::MAX.to_string())),
            ("/snapshot", Value::Null), ("/authority", Value::Null)] {
            let mut value = valid.clone(); *value.pointer_mut(path).unwrap() = malformed;
            assert_value_decode(&value, value_decode::server_frame, false);
        }
        for reply in [json!({"kind":"failure","state":"postCommit","receipt":null}),
            json!({"kind":"failure","state":"unknown","receipt":receipt()}),
            json!({"kind":"failure","state":"beforeExecution","receipt":receipt()})] {
            assert_value_decode(&reply, value_decode::server_reply, false);
        }
        let mut initial = serde_json::to_value(ServerFrame::new(U64::new(1), ServerReply::Producer { producer: producer() },
            Some(json!({})), Some(producer())).unwrap()).unwrap();
        initial["authority"]["serverRevision"] = json!("8");
        assert_value_decode(&initial, value_decode::server_frame, false);
    }

    #[test]
    fn schema_value_decode_unit_enum_compatibility_matches_serde() {
        for value in [json!("gold"), json!({"gold":null}), json!("pearls"), json!({"pearls":null})] {
            assert_value_decode(&value, value_decode::currency, true);
        }
        for name in ["trade", "buyBack", "used"] {
            assert_value_decode(&json!({name:null}), value_decode::source, true);
        }
        for name in ["invalidRequest", "playerDead", "serviceUnavailable", "unsupportedService", "unknownGood",
            "invalidQuantity", "insufficientCurrency", "clockUnavailable", "invalidDelivery"] {
            assert_value_decode(&json!({name:null}), value_decode::rejection, true);
        }
        for name in ["beforeExecution", "unknown", "postCommit"] {
            assert_value_decode(&json!({name:null}), value_decode::failure_state, true);
        }
        let mut value = serde_json::to_value(frame()).unwrap();
        value["type"] = json!({"npcPurchaseOwner":null});
        for path in ["/reply/receipt/entry/operation/intent", "/reply/receipt/entry/outcome/committed"] {
            value.pointer_mut(path).unwrap()["currency"] = json!({"gold":null});
            value.pointer_mut(path).unwrap()["source"] = json!({"trade":null});
        }
        assert_value_decode(&value, value_decode::server_frame, true);
        let mut rejected = serde_json::to_value(rejected(receipt(), Rejection::UnknownGood)).unwrap();
        rejected["entry"]["outcome"]["rejected"]["reason"] = json!({"unknownGood":null});
        assert_value_decode(&rejected, value_decode::receipt, true);
        for value in [json!({"gold":true}), json!({"gold":0}), json!({"gold":{}}), json!({"gold":null,"pearls":null}),
            json!({}), Value::Null, json!([]), json!(1), json!("unknown")] {
            assert_value_decode(&value, value_decode::currency, false);
        }
        assert_buffered_unit_enum_payloads_match_serde();
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
    enum BufferedValue<T> { Value { value: T } }
    fn assert_buffered_value_decode<T>(value: &Value, decode: fn(&Value) -> Result<T>, accepted: bool)
    where T: serde::de::DeserializeOwned + fmt::Debug + PartialEq {
        let original = serde_json::from_value::<BufferedValue<T>>(json!({"kind":"value","value":value}));
        let compact = decode(value);
        assert_eq!(original.is_ok(), accepted, "buffered serde oracle: {value}: {original:?}");
        assert_eq!(compact.is_ok(), accepted, "buffered compact decoder: {value}: {compact:?}");
        if let (Ok(BufferedValue::Value { value: original }), Ok(compact)) = (original, compact) {
            assert_eq!(compact, original);
        }
    }

    fn assert_buffered_unit_enum_payloads_match_serde() {
        // Content's unit compatibility accepts only null or an empty object.
        // Every other JSON payload shape must still fail in tagged contexts.
        for payload in [Value::Null, json!({}), json!(false), json!(true), json!(0), json!(1), json!(-1), json!(1.25),
            json!(""), json!("ignored"), json!([]), json!([null]), json!([1]), json!({"ignored":null}), json!({"ignored":[]})] {
            let direct = payload.is_null();
            let buffered = direct || payload.as_object().is_some_and(serde_json::Map::is_empty);
            for name in ["gold", "pearls"] {
                let enum_value = json!({name:payload});
                assert_value_decode(&enum_value, value_decode::currency, direct);
                let mut value = serde_json::to_value(operation().intent).unwrap(); value["currency"] = enum_value;
                assert_value_decode(&value, value_decode::intent, direct);
                assert_buffered_value_decode(&value, value_decode::intent_buffered, buffered);
                assert_value_decode(&json!({"kind":"quote","intent":value}), value_decode::server_reply, buffered);
                let mut value = serde_json::to_value(operation()).unwrap(); value["intent"]["currency"] = json!({name:payload});
                assert_value_decode(&value, value_decode::operation, direct);
                assert_buffered_value_decode(&value, value_decode::operation_buffered, buffered);
            }
            for name in ["trade", "buyBack", "used"] {
                let enum_value = json!({name:payload});
                assert_value_decode(&enum_value, value_decode::source, direct);
                let mut value = serde_json::to_value(operation().intent).unwrap(); value["source"] = enum_value;
                assert_value_decode(&value, value_decode::intent, direct);
                assert_buffered_value_decode(&value, value_decode::intent_buffered, buffered);
                assert_value_decode(&json!({"kind":"quote","intent":value}), value_decode::server_reply, buffered);
                let mut value = serde_json::to_value(operation()).unwrap(); value["intent"]["source"] = json!({name:payload});
                assert_value_decode(&value, value_decode::operation, direct);
                assert_buffered_value_decode(&value, value_decode::operation_buffered, buffered);
            }
            for (path, name) in [("/entry/operation/intent/currency", "gold"), ("/entry/operation/intent/source", "trade"),
                ("/entry/outcome/committed/currency", "gold"), ("/entry/outcome/committed/source", "trade")] {
                let mut value = serde_json::to_value(receipt()).unwrap(); *value.pointer_mut(path).unwrap() = json!({name:payload});
                assert_value_decode(&value, value_decode::receipt, direct);
                let reply = json!({"kind":"purchase","receipt":value,"replayed":false});
                assert_value_decode(&reply, value_decode::server_reply, buffered);
                let frame = json!({"type":"npcPurchaseOwner","protocolVersion":1,"requestId":"1","reply":reply,"snapshot":null,"authority":null});
                assert_value_decode(&frame, value_decode::server_frame, buffered);
                assert_frame_decode(&frame.to_string(), buffered);
            }
            for name in ["invalidRequest", "playerDead", "serviceUnavailable", "unsupportedService", "unknownGood",
                "invalidQuantity", "insufficientCurrency", "clockUnavailable", "invalidDelivery"] {
                assert_value_decode(&json!({name:payload}), value_decode::rejection, direct);
                let mut value = serde_json::to_value(rejected(receipt(), Rejection::UnknownGood)).unwrap();
                value["entry"]["outcome"]["rejected"]["reason"] = json!({name:payload});
                assert_value_decode(&value, value_decode::receipt, direct);
                let reply = json!({"kind":"purchase","receipt":value,"replayed":false});
                assert_value_decode(&reply, value_decode::server_reply, buffered);
                let frame = json!({"type":"npcPurchaseOwner","protocolVersion":1,"requestId":"1","reply":reply,"snapshot":null,"authority":null});
                assert_value_decode(&frame, value_decode::server_frame, buffered);
            }
            for name in ["beforeExecution", "unknown", "postCommit"] {
                assert_value_decode(&json!({name:payload}), value_decode::failure_state, direct);
                let receipt = if name == "postCommit" { serde_json::to_value(receipt()).unwrap() } else { Value::Null };
                let reply = json!({"kind":"failure","state":{name:payload},"receipt":receipt});
                assert_value_decode(&reply, value_decode::server_reply, buffered);
            }
            // The frame envelope is outside the tagged reply's buffered scope.
            let mut value = serde_json::to_value(frame()).unwrap(); value["type"] = json!({"npcPurchaseOwner":payload});
            assert_value_decode(&value, value_decode::server_frame, direct);
        }
    }

    fn positional(value: &Value, names: &[&str]) -> Value {
        Value::Array(names.iter().map(|name| value[*name].clone()).collect())
    }
    #[test]
    fn schema_value_decode_positional_struct_and_buffered_outcomes_match_serde() {
        let request = serde_json::to_value(operation().intent.request).unwrap();
        let request = positional(&request, &["itemIndex", "count", "panelType"]);
        assert_value_decode(&request, value_decode::purchase_request, true);
        for malformed in [json!(["0",1]), json!(["0",1,0,0])] {
            assert_value_decode(&malformed, value_decode::purchase_request, false);
        }
        let intent = serde_json::to_value(operation().intent).unwrap();
        assert_value_decode(&positional(&intent, &["request", "currency", "source", "serviceCatalogProof"]), value_decode::intent, true);
        let operation = serde_json::to_value(operation()).unwrap();
        assert_value_decode(&positional(&operation, &["actor", "requestScope", "sequence", "intent"]), value_decode::operation, true);
        let producer = serde_json::to_value(producer()).unwrap();
        assert_value_decode(&positional(&producer, &["actor", "producerScope", "serverRevision"]), value_decode::producer, true);
        let receipt_value = serde_json::to_value(receipt()).unwrap();
        assert_value_decode(&positional(&receipt_value["entry"], &["operation", "serverRevision", "outcome"]), value_decode::receipt_entry, true);
        assert_value_decode(&positional(&receipt_value, &["producerScope", "entry"]), value_decode::receipt, true);
        let value = serde_json::to_value(frame()).unwrap();
        assert_value_decode(&positional(&value, &["type", "protocolVersion", "requestId", "reply", "snapshot", "authority"]), value_decode::server_frame, true);
        for rejected_outcome in [false, true] {
            let specimen = if rejected_outcome { rejected(receipt(), Rejection::UnknownGood) } else { receipt() };
            let mut value = serde_json::to_value(ServerFrame::new(U64::new(1), ServerReply::Purchase { receipt: specimen, replayed: false }, None, None).unwrap()).unwrap();
            let key = if rejected_outcome { "rejected" } else { "committed" };
            let names: &[&str] = if rejected_outcome { &["request", "reason"] } else { &["request", "currency", "source", "charged", "admittedCount", "incomingUniqueId"] };
            let outcome = &mut value["reply"]["receipt"]["entry"]["outcome"];
            outcome[key] = positional(&outcome[key], names);
            assert_value_decode(outcome, value_decode::outcome, false);
            assert_value_decode(&value, value_decode::server_frame, true);
            assert_frame_decode(&value.to_string(), true);
            value["reply"]["receipt"] = positional(&value["reply"]["receipt"], &["producerScope", "entry"]);
            assert_value_decode(&value, value_decode::server_frame, true);
        }
        for reply in [ServerReply::Producer { producer: super::tests::producer() }, ServerReply::Quote { intent: super::tests::operation().intent },
            ServerReply::Recovery { receipt: None }, ServerReply::Recovery { receipt: Some(receipt()) },
            ServerReply::Purchase { receipt: receipt(), replayed: true },
            ServerReply::Failure { state: FailureState::Unknown, receipt: None },
            ServerReply::Failure { state: FailureState::PostCommit, receipt: Some(receipt()) }] {
            let names: &[&str] = match &reply {
                ServerReply::Producer { .. } => &["kind", "producer"], ServerReply::Quote { .. } => &["kind", "intent"],
                ServerReply::Recovery { .. } => &["kind", "receipt"], ServerReply::Purchase { .. } => &["kind", "receipt", "replayed"],
                ServerReply::Failure { .. } => &["kind", "state", "receipt"],
            };
            let value = positional(&serde_json::to_value(&reply).unwrap(), names);
            assert_value_decode(&value, value_decode::server_reply, true);
            let frame = json!({"type":"npcPurchaseOwner","protocolVersion":1,"requestId":"1","reply":value,"snapshot":null,"authority":null});
            assert_value_decode(&frame, value_decode::server_frame, true); assert_frame_decode(&frame.to_string(), true);
        }
        for malformed in [json!(["recovery"]), json!(["recovery",null,null]), json!(["failure","unknown"]),
            json!(["purchase",receipt()]), json!([0,null]), json!(["quote",null])] {
            assert_value_decode(&malformed, value_decode::server_reply, false);
        }
    }

    #[test]
    fn schema_value_decode_raw_duplicate_depth_and_limits_match_serde() {
        let valid = encode_server_frame(&frame()).unwrap();
        assert_frame_decode(&valid, true);
        for (needle, replacement) in [("\"requestId\":\"11\"", "\"requestId\":\"11\",\"request\\u0049d\":\"11\""),
            ("\"kind\":\"purchase\"", "\"kind\":\"purchase\",\"k\\u0069nd\":\"purchase\""),
            ("\"schemaSpecimen\":true", "\"schemaSpecimen\":true,\"schema\\u0053pecimen\":true")] {
            let invalid = valid.replacen(needle, replacement, 1);
            assert_ne!(invalid, valid); assert_frame_decode(&invalid, false);
        }
        let escaped = valid.replacen("\"requestId\"", "\"request\\u0049d\"", 1);
        assert_ne!(escaped, valid); assert_frame_decode(&escaped, true);
        let exact = format!("{}{}", " ".repeat(MAX_SERVER_FRAME_BYTES - valid.len()), valid);
        assert_frame_decode(&exact, true); assert_frame_decode(&format!(" {exact}"), false);
        assert_frame_decode(&format!("{valid} {{}}"), false); assert_frame_decode("", false);
        let deep = valid.replace("{\"schemaSpecimen\":true}", &format!("{}0{}", "[".repeat(200), "]".repeat(200)));
        assert_ne!(deep, valid); assert_frame_decode(&deep, false);
    }

    #[test]
    fn schema_value_decode_public_helpers_keep_exact_scalar_shapes() {
        for raw in ["0", "1", "255", "256", "65535", "65536", "4294967295", "4294967296", "-1", "-0", "1.0", "1e0", "\"1\"", "null", "true"] {
            let value: Value = serde_json::from_str(raw).unwrap();
            let unsigned = matches!(raw, "0" | "1" | "255" | "256" | "65535" | "65536" | "4294967295" | "4294967296");
            assert_value_decode(&value, value_decode::u8, unsigned && matches!(raw, "0" | "1" | "255"));
            assert_value_decode(&value, value_decode::u16, unsigned && matches!(raw, "0" | "1" | "255" | "256" | "65535"));
            assert_value_decode(&value, value_decode::u32, unsigned && raw != "4294967296");
        }
        for (value, accepted) in [(json!("0"),true), (json!(u64::MAX.to_string()),true), (json!("01"),false),
            (json!("18446744073709551616"),false), (json!(0),false), (Value::Null,false)] {
            assert_value_decode(&value, value_decode::u64, accepted);
        }
        for (value, accepted) in [(serde_json::to_value(opaque(1)).unwrap(),true), (json!("00".repeat(32)),false),
            (json!("AA".repeat(32)),false), (json!(1),false), (Value::Null,false)] {
            assert_value_decode(&value, value_decode::opaque, accepted);
        }
        let value = json!({"present":null}); let map = value_decode::object(&value, &["present"]).unwrap();
        assert_eq!(value_decode::field(map,"present").unwrap(), &Value::Null);
        assert!(value_decode::field(map,"missing").is_err());
        assert!(value_decode::object(&value,&[]).is_err()); assert!(value_decode::object(&value,&["missing"]).is_err());
        assert!(value_decode::object(&json!([null]),&["present"]).is_err());
        assert_eq!(value_decode::string(&json!("exact")).unwrap(), "exact");
        assert!(value_decode::string(&json!(1)).is_err()); assert!(value_decode::string(&Value::Null).is_err());
        assert!(value_decode::boolean(&json!(true)).unwrap()); assert!(!value_decode::boolean(&json!(false)).unwrap());
        assert!(value_decode::boolean(&json!(1)).is_err()); assert!(value_decode::boolean(&Value::Null).is_err());
    }

    #[test]
    fn schema_u64_is_string_only_canonical_and_covers_zero_and_maximum() {
        for number in [0, 1, 9_007_199_254_740_993, u64::MAX] {
            let value = U64::new(number); let text = serde_json::to_string(&value).unwrap();
            assert_eq!(text, format!("\"{number}\""));
            assert_eq!(serde_json::from_str::<U64>(&text).unwrap(), value);
        }
        for text in ["\"\"", "\"01\"", "\"00\"", "\"+1\"", "\"-1\"", "\" 1\"", "\"1 \"",
            "\"1.0\"", "\"1e0\"", "\"18446744073709551616\"", "1", "1.0", "null", "true", "[]", "{}"] {
            assert!(serde_json::from_str::<U64>(text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn schema_opaque_identity_requires_nonzero_lower_hex_and_roundtrips_actual_bytes() {
        let mut bytes = [0; 32]; bytes[0] = 0xab; bytes[31] = 0x09;
        let value = Opaque32::from_bytes(bytes).unwrap();
        assert_eq!(value.get(), bytes); assert_eq!(value.as_bytes(), &bytes);
        let hex = format!("ab{}09", "00".repeat(30));
        assert_eq!(serde_json::to_string(&value).unwrap(), format!("\"{hex}\""));
        assert_eq!(Opaque32::parse(&hex).unwrap(), value);
        for text in ["00".repeat(32), "AB".repeat(32), "gg".repeat(32), "a".repeat(63),
            "a".repeat(65), format!(" {}", "a".repeat(63))] {
            assert!(Opaque32::parse(&text).is_err(), "accepted {text}");
        }
        assert!(Opaque32::from_bytes([0; 32]).is_err());
        assert!(serde_json::from_str::<Opaque32>("12").is_err());
    }

    #[test]
    fn schema_action_shapes_mutation_classification_and_original_identity_are_exact() {
        let original = operation();
        let actions = [Action::Begin, Action::Quote { request: original.intent.request.clone() },
            Action::Query { operation: original.clone() }, Action::Purchase { operation: original.clone() }];
        for (index, action) in actions.iter().enumerate() {
            let request = ClientRequest::new(U64::new(11), action.clone()).unwrap();
            let text = encode_client_request(&request).unwrap();
            let decoded = parse_client_request(&text).unwrap();
            assert_eq!(decoded, request); assert_eq!(action.is_mutation(), index == 0 || index == 3);
            assert_eq!(action.operation(), if index >= 2 { Some(&original) } else { None });
        }
        assert_eq!(serde_json::to_value(Action::Begin).unwrap(), json!({"kind":"begin"}));
        for action in [json!({"kind":"begin","operation":null}), json!({"kind":"begin","extra":1}),
            json!({"kind":"quote"}), json!({"kind":"query","operation":null}),
            json!({"kind":"purchase","request":original.intent.request}), json!({"kind":"retry"})] {
            assert!(parse_client_request(&json!({"type":"npcPurchaseOwner","protocolVersion":1,"requestId":"11","action":action}).to_string()).is_err());
        }
    }

    #[test]
    fn schema_protocol_and_all_nested_unknown_fields_are_rejected() {
        let valid = serde_json::to_value(ClientRequest::new(U64::new(11), Action::Purchase { operation: operation() }).unwrap()).unwrap();
        for path in ["", "/action", "/action/operation", "/action/operation/intent", "/action/operation/intent/request"] {
            let mut value = valid.clone(); value.pointer_mut(path).unwrap().as_object_mut().unwrap().insert("unknown".into(), json!(false));
            assert!(parse_client_request(&value.to_string()).is_err(), "accepted unknown at {path}");
        }
        for version in [json!(0), json!(2), json!("1"), json!(1.5), Value::Null] {
            let mut value = valid.clone(); value["protocolVersion"] = version;
            assert!(parse_client_request(&value.to_string()).is_err());
        }
        let mut value = valid.clone(); value["type"] = json!("buyItem");
        assert!(parse_client_request(&value.to_string()).is_err());
        let mut typed = ClientRequest::new(U64::new(11), Action::Begin).unwrap(); typed.protocol_version = 2;
        assert!(serde_json::to_string(&typed).is_err());
    }

    #[test]
    fn schema_zero_sequence_and_count_refuse_decode_and_typed_serialization() {
        let mut invalid = operation(); invalid.sequence = U64::new(0);
        assert!(serde_json::to_string(&invalid).is_err());
        let mut value = serde_json::to_value(operation()).unwrap(); value["sequence"] = json!("0");
        assert!(serde_json::from_value::<Operation>(value).is_err());
        let invalid = PurchaseRequest { item_index: U64::new(0), count: 0, panel_type: 255 };
        assert!(serde_json::to_string(&invalid).is_err());
        assert!(PurchaseRequest::new(U64::new(0), 0, 0).is_err());
        for count in [json!(0), json!(-1), json!(65536), json!(1.5), json!("1")] {
            let value = json!({"itemIndex":"0","count":count,"panelType":0});
            assert!(serde_json::from_value::<PurchaseRequest>(value).is_err());
        }
        let valid = PurchaseRequest::new(U64::new(0), u16::MAX, u8::MAX).unwrap();
        assert_eq!(serde_json::from_value::<PurchaseRequest>(serde_json::to_value(&valid).unwrap()).unwrap(), valid);
    }

    #[test]
    fn schema_recursive_duplicate_and_escaped_duplicate_request_keys_are_refused() {
        let valid = encode_client_request(&ClientRequest::new(U64::new(11), Action::Purchase { operation: operation() }).unwrap()).unwrap();
        for (key, escaped) in [("type", "t\\u0079pe"), ("protocolVersion", "protocol\\u0056ersion"), ("requestId", "request\\u0049d"),
            ("kind", "k\\u0069nd"), ("actor", "act\\u006fr"), ("requestScope", "request\\u0053cope"),
            ("sequence", "sequen\\u0063e"), ("itemIndex", "item\\u0049ndex"), ("count", "cou\\u006et"),
            ("serviceCatalogProof", "serviceCatalog\\u0050roof")] {
            let value: Value = serde_json::from_str(&valid).unwrap();
            let original = match key {
                "type" | "protocolVersion" | "requestId" => &value[key], "kind" => &value["action"][key],
                "actor" | "requestScope" | "sequence" => &value["action"]["operation"][key],
                "serviceCatalogProof" => &value["action"]["operation"]["intent"][key],
                _ => &value["action"]["operation"]["intent"]["request"][key],
            }.to_string();
            let needle = format!("\"{key}\":{original}");
            for duplicate in [key, escaped] {
                let text = valid.replacen(&needle, &format!("{needle},\"{duplicate}\":{original}"), 1);
                assert_ne!(text, valid); assert!(parse_client_request(&text).is_err(), "accepted duplicate {duplicate}");
            }
        }
    }

    #[test]
    fn schema_supported_currency_source_receipts_preserve_u64_max_selector_and_uid_zero() {
        for (currency, source) in [(Currency::Gold, Source::Trade), (Currency::Pearls, Source::Trade),
            (Currency::Gold, Source::BuyBack), (Currency::Gold, Source::Used), (Currency::Pearls, Source::Used)] {
            let mut specimen = receipt(); specimen.entry.operation.intent.currency = currency;
            specimen.entry.operation.intent.source = source;
            specimen.entry.outcome = Outcome::Committed { request: specimen.entry.operation.intent.request.clone(),
                currency, source, charged: 0, admitted_count: 3, incoming_unique_id: U64::new(0) };
            let specimen = ServerFrame::new(U64::new(11), ServerReply::Purchase { receipt: specimen, replayed: true }, None, None).unwrap();
            let text = encode_server_frame(&specimen).unwrap(); let decoded = parse_server_frame(&text).unwrap();
            assert_eq!(decoded, specimen);
            let receipt = decoded.reply.receipt().unwrap();
            assert_eq!(receipt.entry.operation.intent.request.item_index.get(), u64::MAX);
            assert_eq!(receipt.entry.operation.sequence.get(), u64::MAX);
            assert!(matches!(receipt.entry.outcome, Outcome::Committed { incoming_unique_id: U64(0), charged: 0, .. }));
        }
    }

    #[test]
    fn schema_rejections_use_exact_nine_actual_terminal_reasons() {
        let reasons = [Rejection::InvalidRequest, Rejection::PlayerDead, Rejection::ServiceUnavailable,
            Rejection::UnsupportedService, Rejection::UnknownGood, Rejection::InvalidQuantity,
            Rejection::InsufficientCurrency, Rejection::ClockUnavailable, Rejection::InvalidDelivery];
        let spellings = ["invalidRequest", "playerDead", "serviceUnavailable", "unsupportedService", "unknownGood",
            "invalidQuantity", "insufficientCurrency", "clockUnavailable", "invalidDelivery"];
        for (reason, spelling) in reasons.into_iter().zip(spellings) {
            let specimen = rejected(receipt(), reason);
            let value = serde_json::to_value(&specimen).unwrap();
            assert_eq!(value["entry"]["outcome"]["rejected"]["reason"], spelling);
            assert_eq!(serde_json::from_value::<Receipt>(value).unwrap(), specimen);
        }
        let mut value = serde_json::to_value(rejected(receipt(), Rejection::InvalidRequest)).unwrap();
        for reason in [json!("insufficientGold"), json!("unknown"), json!(1), Value::Null] {
            value["entry"]["outcome"]["rejected"]["reason"] = reason;
            assert!(serde_json::from_value::<Receipt>(value.clone()).is_err());
        }
    }

    #[test]
    fn schema_terminal_receipt_requires_exact_tuple_currency_source_count_and_revision() {
        let valid = serde_json::to_value(receipt()).unwrap();
        for (path, invalid) in [("/entry/outcome/committed/request/itemIndex", json!("0")),
            ("/entry/outcome/committed/request/count", json!(2)), ("/entry/outcome/committed/request/panelType", json!(1)),
            ("/entry/outcome/committed/currency", json!("pearls")), ("/entry/outcome/committed/source", json!("used")),
            ("/entry/outcome/committed/admittedCount", json!(0)), ("/entry/outcome/committed/admittedCount", json!(4)),
            ("/entry/serverRevision", json!("0")), ("/entry/serverRevision", json!(u64::MAX.to_string())),
            ("/entry/outcome/committed/incomingUniqueId", json!(0))] {
            let mut value = valid.clone(); *value.pointer_mut(path).unwrap() = invalid;
            assert!(serde_json::from_value::<Receipt>(value).is_err(), "accepted malformed {path}");
        }
        let mut typed = receipt(); typed.entry.server_revision = U64::new(0);
        assert!(serde_json::to_string(&typed).is_err());
        let mut rejected = serde_json::to_value(rejected(receipt(), Rejection::UnknownGood)).unwrap();
        rejected["entry"]["outcome"]["rejected"]["request"]["count"] = json!(1);
        assert!(serde_json::from_value::<Receipt>(rejected).is_err());
        let mut value = valid; value["entry"]["outcome"]["rejected"] = json!({"request":operation().intent.request,"reason":"unknownGood"});
        assert!(serde_json::from_value::<Receipt>(value).is_err());
    }

    #[test]
    fn schema_explicit_null_fields_are_required_and_failure_states_cannot_invent_known_results() {
        for reply in [json!({"kind":"recovery","receipt":null}),
            json!({"kind":"failure","state":"beforeExecution","receipt":null}),
            json!({"kind":"failure","state":"unknown","receipt":null})] {
            let value = json!({"type":"npcPurchaseOwner","protocolVersion":1,"requestId":"11","reply":reply,"snapshot":null,"authority":null});
            assert!(parse_server_frame(&value.to_string()).is_ok());
            let mut missing = value.clone(); missing["reply"].as_object_mut().unwrap().remove("receipt");
            assert!(parse_server_frame(&missing.to_string()).is_err());
        }
        for field in ["snapshot", "authority"] {
            let mut value = serde_json::to_value(frame()).unwrap(); value.as_object_mut().unwrap().remove(field);
            assert!(parse_server_frame(&value.to_string()).is_err());
        }
        assert!(ServerReply::Failure { state: FailureState::PostCommit, receipt: None }.validate().is_err());
        for state in [FailureState::BeforeExecution, FailureState::Unknown] {
            let value = ServerReply::Failure { state, receipt: Some(receipt()) };
            assert!(serde_json::to_string(&value).is_err());
        }
        let valid = ServerReply::Failure { state: FailureState::PostCommit, receipt: Some(receipt()) };
        assert_eq!(serde_json::from_value::<ServerReply>(serde_json::to_value(&valid).unwrap()).unwrap(), valid);
    }

    #[test]
    fn schema_authority_pair_actor_scope_revision_and_initial_zero_are_checked() {
        let valid = frame();
        let mut invalid = valid.clone(); invalid.snapshot = None; assert!(invalid.validate().is_err());
        let mut invalid = valid.clone(); invalid.authority = None; assert!(invalid.validate().is_err());
        let mut invalid = valid.clone(); invalid.snapshot = Some(Value::Null); assert!(invalid.validate().is_err());
        for (actor, scope, revision) in [(opaque(9), opaque(4), 9), (opaque(1), opaque(9), 9),
            (opaque(1), opaque(4), 7), (opaque(1), opaque(4), u64::MAX)] {
            let mut invalid = valid.clone(); invalid.authority = Some(Producer { actor, producer_scope: scope, server_revision: U64::new(revision) });
            assert!(serde_json::to_string(&invalid).is_err());
        }
        let baseline = Producer { server_revision: U64::new(0), ..producer() };
        let initial = ServerFrame::new(U64::new(11), ServerReply::Producer { producer: baseline.clone() }, Some(json!({})), Some(baseline)).unwrap();
        assert_eq!(parse_server_frame(&encode_server_frame(&initial).unwrap()).unwrap(), initial);
        let mut mismatch = initial; mismatch.authority.as_mut().unwrap().server_revision = U64::new(1);
        assert!(mismatch.validate().is_err());
        assert_ne!(receipt().producer_scope, receipt().entry.operation.request_scope);
        assert!(valid.validate().is_ok(), "original old scope may differ from current receipt producer");
    }

    #[test]
    fn schema_action_reply_correlation_keeps_original_operation_and_missing_recovery_unknown() {
        let original = operation();
        let purchase = Action::Purchase { operation: original.clone() }; let query = Action::Query { operation: original.clone() };
        assert!(frame().validate_for_request(&ClientRequest::new(U64::new(11), purchase.clone()).unwrap()).is_ok());
        assert!(frame().validate_for_request(&ClientRequest::new(U64::new(11), query.clone()).unwrap()).is_err());
        let recovery = ServerReply::Recovery { receipt: Some(receipt()) };
        assert!(recovery.validate_for_action(&query).is_ok());
        assert!(ServerReply::Recovery { receipt: None }.validate_for_action(&query).is_ok());
        assert!(ServerReply::Recovery { receipt: None }.validate_for_action(&purchase).is_err());
        for changed in 0..6 {
            let mut another = original.clone();
            match changed {
                0 => another.actor = opaque(7), 1 => another.request_scope = opaque(7), 2 => another.sequence = U64::new(1),
                3 => another.intent.request.item_index = U64::new(0), 4 => another.intent.service_catalog_proof = opaque(7),
                _ => another.intent.source = Source::Used,
            }
            assert!(recovery.validate_for_action(&Action::Query { operation: another }).is_err());
        }
        let post = ServerReply::Failure { state: FailureState::PostCommit, receipt: Some(receipt()) };
        assert!(post.validate_for_action(&query).is_ok()); assert!(post.validate_for_action(&purchase).is_ok());
        assert!(post.validate_for_action(&Action::Begin).is_err());
        let quote = ServerReply::Quote { intent: original.intent.clone() };
        assert!(quote.validate_for_action(&Action::Quote { request: original.intent.request }).is_ok());
    }

    #[test]
    fn schema_server_nested_unknown_fields_and_wrong_shapes_are_rejected() {
        let valid = serde_json::to_value(frame()).unwrap();
        for path in ["", "/reply", "/reply/receipt", "/reply/receipt/entry", "/reply/receipt/entry/operation",
            "/reply/receipt/entry/operation/intent", "/reply/receipt/entry/operation/intent/request",
            "/reply/receipt/entry/outcome/committed", "/reply/receipt/entry/outcome/committed/request", "/authority"] {
            let mut value = valid.clone(); value.pointer_mut(path).unwrap().as_object_mut().unwrap().insert("privateJournal".into(), json!([]));
            assert!(parse_server_frame(&value.to_string()).is_err(), "accepted unknown at {path}");
        }
        for (path, malformed) in [("/reply/replayed", json!(1)), ("/reply/receipt", Value::Null),
            ("/reply/receipt/entry/outcome", json!({"unknown":{}})), ("/authority", json!([])),
            ("/protocolVersion", json!(2)), ("/reply/receipt/entry/outcome/committed/charged", json!(-1)),
            ("/reply/receipt/entry/outcome/committed/charged", json!(4_294_967_296u64))] {
            let mut value = valid.clone(); *value.pointer_mut(path).unwrap() = malformed;
            assert!(parse_server_frame(&value.to_string()).is_err(), "accepted malformed {path}");
        }
    }

    #[test]
    fn schema_server_raw_duplicate_keys_include_uninterpreted_snapshot_arrays() {
        let text = encode_server_frame(&frame()).unwrap();
        for (needle, extra) in [("\"protocolVersion\":1", "\"protocolVersion\":1,\"protocol\\u0056ersion\":1"),
            ("\"requestId\":\"11\"", "\"requestId\":\"11\",\"request\\u0049d\":\"11\""),
            ("\"kind\":\"purchase\"", "\"kind\":\"purchase\",\"k\\u0069nd\":\"purchase\""),
            ("\"replayed\":false", "\"replayed\":false,\"replayed\":false"),
            ("\"serverRevision\":\"8\"", "\"serverRevision\":\"8\",\"server\\u0052evision\":\"8\""),
            ("\"incomingUniqueId\":\"0\"", "\"incomingUniqueId\":\"0\",\"incomingUniqueId\":\"0\""),
            ("\"schemaSpecimen\":true", "\"schemaSpecimen\":true,\"schema\\u0053pecimen\":true")] {
            let invalid = text.replacen(needle, extra, 1); assert_ne!(invalid, text);
            assert!(parse_server_frame(&invalid).is_err(), "accepted duplicate {needle}");
        }
        let text = r#"{"type":"npcPurchaseOwner","protocolVersion":1,"requestId":"11","reply":{"kind":"recovery","receipt":null},"snapshot":{"nested":[{"k":0,"\u006b":1}]},"authority":{"actor":"0101010101010101010101010101010101010101010101010101010101010101","producerScope":"0404040404040404040404040404040404040404040404040404040404040404","serverRevision":"0"}}"#;
        assert!(parse_server_frame(text).is_err());
        assert!(parse_server_frame(&text.replace(",\"\\u006b\":1", "")).is_ok(), "the duplicate alone invalidates this carrier");
    }

    #[test]
    fn schema_control_echo_is_nonzero_and_cannot_correlate_delayed_begin_or_quote() {
        for action in [Action::Begin, Action::Quote { request: operation().intent.request.clone() },
            Action::Query { operation: operation() }, Action::Purchase { operation: operation() }] {
            assert!(ClientRequest::new(U64::new(0), action.clone()).is_err());
            let request = ClientRequest::new(U64::new(12), action.clone()).unwrap();
            let reply = match action {
                Action::Begin => ServerReply::Producer { producer: producer() },
                Action::Quote { .. } => ServerReply::Quote { intent: operation().intent },
                Action::Query { .. } => ServerReply::Recovery { receipt: Some(receipt()) },
                Action::Purchase { .. } => ServerReply::Purchase { receipt: receipt(), replayed: false },
            };
            let mut specimen = ServerFrame::new(U64::new(11), reply, None, None).unwrap();
            assert!(specimen.validate_for_request(&request).is_err());
            specimen.request_id = U64::new(12); assert!(specimen.validate_for_request(&request).is_ok());
            specimen.request_id = U64::new(0); assert!(serde_json::to_string(&specimen).is_err());
        }
        let request = ClientRequest::new(U64::new(u64::MAX), Action::Begin).unwrap();
        assert_eq!(parse_client_request(&encode_client_request(&request).unwrap()).unwrap(), request);
        let value = serde_json::to_value(&request).unwrap();
        for malformed in [Value::Null, json!(0), json!("0"), json!("01")] {
            let mut value = value.clone(); value["requestId"] = malformed;
            assert!(parse_client_request(&value.to_string()).is_err());
        }
        let mut missing = value; missing.as_object_mut().unwrap().remove("requestId");
        assert!(parse_client_request(&missing.to_string()).is_err());
        let mut missing = serde_json::to_value(frame()).unwrap(); missing.as_object_mut().unwrap().remove("requestId");
        assert!(parse_server_frame(&missing.to_string()).is_err());
    }

    #[test]
    fn schema_raw_limits_trailing_data_and_deep_json_fail_closed() {
        let valid = encode_client_request(&ClientRequest::new(U64::new(11), Action::Begin).unwrap()).unwrap();
        let exact = format!("{}{}", " ".repeat(MAX_CLIENT_REQUEST_BYTES - valid.len()), valid);
        assert!(parse_client_request(&exact).is_ok());
        assert!(matches!(parse_client_request(&format!(" {exact}")), Err(WireError::Invalid(_))));
        assert!(parse_client_request(&format!("{valid} {{}}")).is_err());
        assert!(parse_client_request("").is_err());
        let deep = format!("{}0{}", "[".repeat(200), "]".repeat(200));
        assert!(parse_server_frame(&deep).is_err());
        let oversized = ServerFrame::new(U64::new(11), ServerReply::Recovery { receipt: None },
            Some(json!({"data":"a".repeat(MAX_SERVER_FRAME_BYTES)})), Some(producer())).unwrap();
        assert!(matches!(encode_server_frame(&oversized), Err(WireError::Invalid(_))));
        assert!(matches!(parse_server_frame(&serde_json::to_string(&oversized).unwrap()), Err(WireError::Invalid(_))));
    }
}

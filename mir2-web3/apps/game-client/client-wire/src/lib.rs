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
pub fn parse_server_frame(text: &str) -> Result<ServerFrame> { parse(text, MAX_SERVER_FRAME_BYTES) }
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

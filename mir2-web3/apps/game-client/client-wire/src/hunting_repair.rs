//! Independent authenticated repair carrier. A server quote is the only
//! confirmation identity; legacy RepairItem and purchase-v1 are not adapters.
use super::{Opaque32, WireError, U64};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_REQUEST_BYTES: usize = 4096;
pub const MAX_FRAME_BYTES: usize = super::MAX_SERVER_FRAME_BYTES;
type Result<T> = std::result::Result<T, WireError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvelopeType {
    #[serde(rename = "huntingRepairOwner")]
    HuntingRepairOwner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    Quote { unique_id: U64, special: bool },
    Confirm { quote_id: String },
    Query { quote_id: String },
}
impl Action {
    pub fn quote_id(&self) -> Option<&str> {
        match self {
            Self::Confirm { quote_id } | Self::Query { quote_id } => Some(quote_id),
            _ => None,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Quote { .. } => Ok(()),
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
            wire_type: EnvelopeType::HuntingRepairOwner,
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
                "unsupported repair protocol or request identity",
            ));
        }
        self.action.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Quote {
    pub quote_id: String,
    pub unique_id: U64,
    pub special: bool,
    pub cost_bound_gold: U64,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
}
impl Quote {
    fn validate(&self) -> Result<()> {
        Opaque32::parse(&self.quote_id)?;
        if self.created_at_ms < 0
            || self.created_at_ms.checked_add(30_000) != Some(self.expires_at_ms)
        {
            return Err(WireError::Invalid("invalid server repair quote"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub quote_id: String,
    pub cost_bound_gold: U64,
    pub free: bool,
    pub operation_key: String,
    pub request_digest: String,
    pub business_day: i64,
    pub committed_at_ms: i64,
}
impl Receipt {
    fn validate(&self) -> Result<()> {
        Opaque32::parse(&self.quote_id)?;
        if self.free != (self.cost_bound_gold.get() == 0)
            || self.committed_at_ms < 0
            || [&self.operation_key, &self.request_digest]
                .iter()
                .any(|value| {
                    value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control)
                })
        {
            return Err(WireError::Invalid("invalid immutable repair receipt"));
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
    /// Transport/owner execution did not establish a terminal result. Keep the
    /// original quote for Query; this never authorizes a replacement Confirm.
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
    fn validate(&self) -> Result<()> {
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
    /// An optional current owner snapshot, captured with this completed turn.
    /// Explicit null preserves a known receipt without claiming synchronization.
    #[serde(deserialize_with = "super::required_option")]
    pub snapshot: Option<Value>,
}
impl ServerFrame {
    pub fn new(request: &ClientRequest, reply: Reply, snapshot: Option<Value>) -> Result<Self> {
        let frame = Self {
            wire_type: EnvelopeType::HuntingRepairOwner,
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
                "unsupported repair response protocol or identity",
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
                "repair snapshot requires a completed current owner result",
            ));
        }
        Ok(())
    }
    pub fn validate_for_request(&self, request: &ClientRequest) -> Result<()> {
        self.validate()?;
        request.validate()?;
        if self.request_id != request.request_id {
            return Err(WireError::Invalid("uncorrelated repair response"));
        }
        match (&request.action, &self.reply) {
            (Action::Quote { unique_id, special }, Reply::Quoted { quote })
                if unique_id == &quote.unique_id && special == &quote.special =>
            {
                Ok(())
            }
            (Action::Quote { .. }, Reply::Rejected { quote_id: None, .. }) => Ok(()),
            (Action::Quote { .. }, Reply::Unknown { quote_id: None }) => Ok(()),
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
                },
            ) if quote_id == actual => Ok(()),
            (
                Action::Confirm { quote_id } | Action::Query { quote_id },
                Reply::Unknown {
                    quote_id: Some(actual),
                },
            ) if quote_id == actual => Ok(()),
            _ => Err(WireError::Invalid(
                "repair response does not match original action",
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
        return Err(WireError::Invalid("repair message exceeds size limit"));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id() -> String {
        "ab".repeat(32)
    }
    fn receipt() -> Receipt {
        Receipt {
            quote_id: id(),
            cost_bound_gold: U64::new(10),
            free: false,
            operation_key: "original-repair-operation".into(),
            request_digest: "original-repair-digest".into(),
            business_day: 1,
            committed_at_ms: 1,
        }
    }
    fn committed_frame(reload_required: bool, snapshot: Option<Value>) -> ServerFrame {
        let request = ClientRequest::new(U64::new(1), Action::Query { quote_id: id() }).unwrap();
        ServerFrame::new(
            &request,
            Reply::Committed {
                receipt: receipt(),
                replayed: true,
                reload_required,
            },
            snapshot,
        )
        .unwrap()
    }
    #[test]
    fn strict_carrier_preserves_full_width_ids_and_refuses_client_prices() {
        let request = ClientRequest::new(
            U64::new(u64::MAX),
            Action::Quote {
                unique_id: U64::new(u64::MAX),
                special: false,
            },
        )
        .unwrap();
        let text = encode_client_request(&request).unwrap();
        assert_eq!(parse_client_request(&text).unwrap(), request);
        assert!(parse_client_request(&text.replace(
            "\"special\":false",
            "\"special\":false,\"costBoundGold\":\"1\""
        ))
        .is_err());
        assert!(parse_client_request(
            &text.replace("\"special\":false", "\"special\":false,\"special\":true")
        )
        .is_err());
        assert!(parse_client_request(
            &text.replace(&format!("\"{}\"", u64::MAX), &u64::MAX.to_string())
        )
        .is_err());
        assert!(
            parse_client_request(&text.replace("huntingRepairOwner", "npcPurchaseOwner")).is_err()
        );
    }
    #[test]
    fn exact_quote_correlation_and_free_receipts_are_required() {
        let request = ClientRequest::new(U64::new(1), Action::Confirm { quote_id: id() }).unwrap();
        let receipt = Receipt {
            quote_id: id(),
            cost_bound_gold: U64::new(0),
            free: true,
            operation_key: "real-op".into(),
            request_digest: "real-digest".into(),
            business_day: 1,
            committed_at_ms: 1,
        };
        let frame = ServerFrame::new(
            &request,
            Reply::Committed {
                receipt,
                replayed: true,
                reload_required: true,
            },
            None,
        )
        .unwrap();
        assert_eq!(
            parse_server_frame(&encode_server_frame(&frame).unwrap()).unwrap(),
            frame
        );
        let mut changed = frame.clone();
        if let Reply::Committed { receipt, .. } = &mut changed.reply {
            receipt.free = false;
        }
        assert!(encode_server_frame(&changed).is_err());
        let other = ClientRequest::new(
            U64::new(1),
            Action::Query {
                quote_id: "cd".repeat(32),
            },
        )
        .unwrap();
        assert!(frame.validate_for_request(&other).is_err());
        assert!(ClientRequest::new(
            U64::new(2),
            Action::Confirm {
                quote_id: "AB".repeat(32)
            }
        )
        .is_err());
    }
    #[test]
    fn real_legacy_weapon_uid_zero_is_valid_but_request_id_zero_is_not() {
        let request = ClientRequest::new(
            U64::new(1),
            Action::Quote {
                unique_id: U64::new(0),
                special: false,
            },
        )
        .unwrap();
        let encoded = encode_client_request(&request).unwrap();
        assert!(encoded.contains("\"uniqueId\":\"0\""));
        assert_eq!(parse_client_request(&encoded).unwrap(), request);
        let frame = ServerFrame::new(
            &request,
            Reply::Quoted {
                quote: Quote {
                    quote_id: id(),
                    unique_id: U64::new(0),
                    special: false,
                    cost_bound_gold: U64::new(10),
                    created_at_ms: 1,
                    expires_at_ms: 30_001,
                },
            },
            None,
        )
        .unwrap();
        assert_eq!(
            parse_server_frame(&encode_server_frame(&frame).unwrap()).unwrap(),
            frame
        );
        assert!(ClientRequest::new(U64::new(0), request.action).is_err());
    }

    #[test]
    fn snapshot_is_required_nullable_and_preserves_known_receipt_without_sync() {
        for reload_required in [false, true] {
            let frame = committed_frame(reload_required, None);
            let encoded = encode_server_frame(&frame).unwrap();
            assert!(encoded.contains("\"snapshot\":null"));
            let parsed = parse_server_frame(&encoded).unwrap();
            assert_eq!(parsed, frame);
            assert_eq!(parsed.snapshot, None);
            assert!(matches!(parsed.reply, Reply::Committed {
                receipt: original,
                replayed: true,
                reload_required: actual,
            } if original == receipt() && actual == reload_required));

            let mut missing = serde_json::to_value(&frame).unwrap();
            missing.as_object_mut().unwrap().remove("snapshot");
            assert!(parse_server_frame(&missing.to_string()).is_err());
            let mut unknown = serde_json::to_value(&frame).unwrap();
            unknown["snapshotAvailable"] = Value::Bool(false);
            assert!(parse_server_frame(&unknown.to_string()).is_err());
        }
    }

    #[test]
    fn snapshot_only_attaches_to_nonretired_committed_results() {
        let snapshot = serde_json::json!({
            "inventory": [{"uniqueId": u64::MAX.to_string(), "durability": 100}],
            "equipment": [],
            "publicValues": [null, true, -1, "英雄"]
        });
        let frame = committed_frame(false, Some(snapshot.clone()));
        let encoded = encode_server_frame(&frame).unwrap();
        assert_eq!(parse_server_frame(&encoded).unwrap(), frame);
        assert_eq!(frame.snapshot, Some(snapshot));

        let quote_request = ClientRequest::new(
            U64::new(2),
            Action::Quote {
                unique_id: U64::new(0),
                special: false,
            },
        )
        .unwrap();
        let query_request =
            ClientRequest::new(U64::new(3), Action::Query { quote_id: id() }).unwrap();
        let illegal = [
            (
                &quote_request,
                Reply::Quoted {
                    quote: Quote {
                        quote_id: id(),
                        unique_id: U64::new(0),
                        special: false,
                        cost_bound_gold: U64::new(10),
                        created_at_ms: 1,
                        expires_at_ms: 30_001,
                    },
                },
            ),
            (&query_request, Reply::Pending { quote_id: id() }),
            (
                &query_request,
                Reply::Unknown {
                    quote_id: Some(id()),
                },
            ),
            (
                &query_request,
                Reply::Rejected {
                    quote_id: Some(id()),
                    code: Rejection::Unavailable,
                },
            ),
            (
                &query_request,
                Reply::Committed {
                    receipt: receipt(),
                    replayed: false,
                    reload_required: true,
                },
            ),
        ];
        for (request, reply) in illegal {
            assert!(ServerFrame::new(request, reply.clone(), Some(serde_json::json!({}))).is_err());
            let mut raw =
                serde_json::to_value(ServerFrame::new(request, reply, None).unwrap()).unwrap();
            raw["snapshot"] = serde_json::json!({});
            assert!(parse_server_frame(&raw.to_string()).is_err());
        }

        let mut explicit_null = frame;
        explicit_null.snapshot = Some(Value::Null);
        assert!(explicit_null.validate().is_err());
        assert!(encode_server_frame(&explicit_null).is_err());
    }

    #[test]
    fn snapshot_rejects_duplicate_plain_escaped_and_nested_keys() {
        let encoded = encode_server_frame(&committed_frame(false, None)).unwrap();
        for replacement in [
            "\"snapshot\":null,\"snapshot\":null",
            "\"snapshot\":null,\"snapsh\\u006ft\":{}",
            "\"snapshot\":{\"inventory\":[{\"uid\":1,\"uid\":2}]}",
            "\"snapshot\":{\"inventory\":[{\"uid\":1,\"u\\u0069d\":2}]}",
        ] {
            let invalid = encoded.replacen("\"snapshot\":null", replacement, 1);
            assert_ne!(invalid, encoded);
            assert!(parse_server_frame(&invalid).is_err());
        }
    }

    #[test]
    fn snapshot_frames_share_the_two_mebibyte_server_limit() {
        assert_eq!(MAX_FRAME_BYTES, super::super::MAX_SERVER_FRAME_BYTES);
        assert_eq!(MAX_FRAME_BYTES, 2 * 1024 * 1024);
        assert_eq!(MAX_REQUEST_BYTES, 4096);
        let frame = committed_frame(
            false,
            Some(serde_json::json!({"data": "a".repeat(32 * 1024)})),
        );
        let encoded = encode_server_frame(&frame).unwrap();
        assert!(encoded.len() > 16_384);
        assert_eq!(parse_server_frame(&encoded).unwrap(), frame);

        let exact = format!("{}{}", " ".repeat(MAX_FRAME_BYTES - encoded.len()), encoded);
        assert_eq!(exact.len(), MAX_FRAME_BYTES);
        assert_eq!(parse_server_frame(&exact).unwrap(), frame);
        assert!(parse_server_frame(&format!(" {exact}")).is_err());

        let oversized = committed_frame(
            false,
            Some(serde_json::json!({"data": "a".repeat(MAX_FRAME_BYTES)})),
        );
        assert!(oversized.validate().is_ok());
        assert!(encode_server_frame(&oversized).is_err());
        assert!(parse_server_frame(&serde_json::to_string(&oversized).unwrap()).is_err());
    }
}

//! Typed owner boundary for durable NPC purchase operations.
//!
//! These messages neither authenticate an owner nor execute an economic action.
//! The host must verify its accepted capability, authenticated actor and current
//! producer authority before dispatch. Recovery retains the original operation
//! key and full intent; a missing receipt never authorizes retry or fallback.
//! Begin can persist a legacy actor, so it is a mutation alongside Purchase.

use serde::{Deserialize, Deserializer, Serialize};

use crate::npc_purchase_journal::{NpcPurchaseIntent, NpcPurchaseOperation};
use crate::runtime::{NpcPurchaseProducer, NpcPurchaseReceipt, NpcPurchaseRequest};

pub const NPC_PURCHASE_OWNER_PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum NpcPurchaseOwnerAction {
    Begin,
    Quote { request: NpcPurchaseRequest },
    Query { operation: NpcPurchaseOperation },
    Purchase { operation: NpcPurchaseOperation },
}

// Serde's internally tagged unit variant can discard extra fields. Decode
// Begin as an empty struct variant so the same strict field policy applies to
// all four actions, while preserving the public unit variant and wire shape.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
enum StrictOwnerAction {
    Begin {},
    Quote { request: NpcPurchaseRequest },
    Query { operation: NpcPurchaseOperation },
    Purchase { operation: NpcPurchaseOperation },
}

impl<'de> Deserialize<'de> for NpcPurchaseOwnerAction {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match StrictOwnerAction::deserialize(deserializer)? {
            StrictOwnerAction::Begin {} => Self::Begin,
            StrictOwnerAction::Quote { request } => Self::Quote { request },
            StrictOwnerAction::Query { operation } => Self::Query { operation },
            StrictOwnerAction::Purchase { operation } => Self::Purchase { operation },
        })
    }
}

impl NpcPurchaseOwnerAction {
    /// Mutations require a single attempt on the selected authenticated owner.
    pub fn is_mutation(&self) -> bool {
        matches!(self, Self::Begin | Self::Purchase { .. })
    }

    /// Returns the unchanged original identity, including its request scope.
    pub fn operation(&self) -> Option<NpcPurchaseOperation> {
        match self {
            Self::Query { operation } | Self::Purchase { operation } => Some(*operation),
            Self::Begin | Self::Quote { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum NpcPurchaseOwnerReply {
    Producer { producer: NpcPurchaseProducer },
    Quote { intent: NpcPurchaseIntent },
    Recovery {
        // Require the field itself. Only an explicit null represents no result;
        // serde's implicit missing-Option default would conceal malformed replies.
        #[serde(deserialize_with = "required_receipt")]
        receipt: Option<NpcPurchaseReceipt>,
    },
    Purchase { receipt: NpcPurchaseReceipt, replayed: bool },
}

fn required_receipt<'de, D>(deserializer: D) -> Result<Option<NpcPurchaseReceipt>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<NpcPurchaseReceipt>::deserialize(deserializer)
}

/// Host-only execution carrier; it is not a serialized full snapshot.
#[derive(Debug, Clone)]
pub struct NpcPurchaseOwnerExecution {
    pub reply: NpcPurchaseOwnerReply,
    pub packets: Vec<mir2_protocol::ServerPacket>,
    /// Actual current producer and complete live authoritative save revision,
    /// supplied only after verified publication. None establishes no complete
    /// applied-snapshot witness. A receipt revision or local clock cannot fill it.
    pub authority: Option<NpcPurchaseProducer>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{NpcPurchaseCurrency, NpcPurchaseProcessingOutcome, NpcPurchaseSource};
    use serde_json::{json, Value};

    // Schema fixtures only: no execution carrier or trusted publication is
    // constructed, and decoding these values does not establish host authority.
    fn operation_fixture(currency: NpcPurchaseCurrency, source: NpcPurchaseSource) -> NpcPurchaseOperation {
        NpcPurchaseOperation {
            actor: [17; 32],
            request_scope: [29; 32],
            sequence: u64::MAX,
            intent: NpcPurchaseIntent {
                request: NpcPurchaseRequest { item_index: u64::MAX, count: u16::MAX, panel_type: 0 },
                currency,
                source,
                service_catalog_proof: [41; 32],
            },
        }
    }

    fn operation_json() -> Value {
        serde_json::to_value(operation_fixture(NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Used)).unwrap()
    }

    fn receipt_json() -> Value {
        let operation = operation_json();
        json!({
            "producerScope": ([53; 32].to_vec()),
            "entry": {
                "operation": operation.clone(),
                "serverRevision": u64::MAX - 1,
                "outcome": { "committed": {
                    "request": operation["intent"]["request"].clone(),
                    "currency": "pearls", "source": "used", "charged": 0,
                    "admittedCount": u16::MAX, "incomingUniqueId": 0
                } }
            }
        })
    }

    fn action_json() -> Value {
        json!({ "kind": "purchase", "operation": operation_json() })
    }

    fn reply_json() -> Value {
        json!({ "kind": "purchase", "receipt": receipt_json(), "replayed": true })
    }

    // Insert the same valid value without parsing the duplicate-bearing text.
    // The caller names the original spelling and its possibly escaped duplicate.
    fn duplicate_field(document: &Value, field: &str, duplicate: &str, value: &Value) -> String {
        let encoded = serde_json::to_string(document).unwrap();
        let needle = format!("\"{field}\":");
        assert!(encoded.contains(&needle));
        let insertion = format!("\"{duplicate}\":{},\"{field}\":", serde_json::to_string(value).unwrap());
        encoded.replacen(&needle, &insertion, 1)
    }

    #[test]
    fn action_wire_names_and_mutation_classification_preserve_original_operations() {
        assert_eq!(NPC_PURCHASE_OWNER_PROTOCOL_VERSION, 1);
        let begin = NpcPurchaseOwnerAction::Begin;
        assert!(begin.is_mutation());
        assert_eq!(begin.operation(), None);
        assert_eq!(serde_json::to_string(&begin).unwrap(), r#"{"kind":"begin"}"#);
        for (currency, source) in [
            (NpcPurchaseCurrency::Gold, NpcPurchaseSource::Trade),
            (NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Trade),
            (NpcPurchaseCurrency::Gold, NpcPurchaseSource::BuyBack),
            (NpcPurchaseCurrency::Gold, NpcPurchaseSource::Used),
            (NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Used),
        ] {
            let operation = operation_fixture(currency, source);
            let quote = NpcPurchaseOwnerAction::Quote { request: operation.intent.request };
            assert!(!quote.is_mutation());
            assert_eq!(quote.operation(), None);
            for (action, kind, mutation) in [
                (quote, "quote", false),
                (NpcPurchaseOwnerAction::Query { operation }, "query", false),
                (NpcPurchaseOwnerAction::Purchase { operation }, "purchase", true),
            ] {
                let encoded = serde_json::to_string(&action).unwrap();
                assert_eq!(serde_json::from_str::<NpcPurchaseOwnerAction>(&encoded).unwrap(), action);
                assert_eq!(serde_json::from_str::<Value>(&encoded).unwrap()["kind"], kind);
                assert_eq!(action.is_mutation(), mutation);
                if kind != "quote" { assert_eq!(action.operation(), Some(operation)); }
            }
        }
    }

    #[test]
    fn replies_roundtrip_all_named_variants_without_private_journal_or_account_fields() {
        let producer = json!({ "actor": ([17; 32].to_vec()), "producerScope": ([53; 32].to_vec()), "serverRevision": 0 });
        let mut rejected = receipt_json();
        rejected["entry"]["outcome"] = json!({ "rejected": {
            "request": operation_json()["intent"]["request"].clone(), "reason": "insufficientCurrency"
        } });
        for value in [
            json!({ "kind": "producer", "producer": producer }),
            json!({ "kind": "quote", "intent": operation_json()["intent"].clone() }),
            json!({ "kind": "recovery", "receipt": null }),
            json!({ "kind": "recovery", "receipt": receipt_json() }),
            json!({ "kind": "recovery", "receipt": rejected }),
            reply_json(),
        ] {
            let reply: NpcPurchaseOwnerReply = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(reply).unwrap(), value);
            let encoded = serde_json::to_string(&value).unwrap();
            for private in ["accountId", "characterName", "entries", "ownerLease"] {
                assert!(!encoded.contains(&format!("\"{private}\":")));
            }
        }
    }

    #[test]
    fn raw_full_u64_identity_and_zero_incoming_uid_decode_losslessly() {
        let encoded = serde_json::to_string(&action_json()).unwrap();
        assert!(encoded.contains("18446744073709551615"));
        let action: NpcPurchaseOwnerAction = serde_json::from_str(&encoded).unwrap();
        let operation = action.operation().unwrap();
        assert_eq!(operation.sequence, u64::MAX);
        assert_eq!(operation.intent.request.item_index, u64::MAX);
        let reply: NpcPurchaseOwnerReply = serde_json::from_str(&serde_json::to_string(&reply_json()).unwrap()).unwrap();
        let NpcPurchaseOwnerReply::Purchase { receipt, replayed } = reply else { panic!("wrong decoded variant") };
        assert!(replayed);
        assert_eq!(receipt.entry.operation, operation);
        assert_eq!(receipt.entry.server_revision, u64::MAX - 1);
        match receipt.entry.outcome {
            NpcPurchaseProcessingOutcome::Committed { request, charged, admitted_count, incoming_unique_id, .. } => {
                assert_eq!(request, operation.intent.request);
                assert_eq!(charged, 0);
                assert_eq!(admitted_count, u16::MAX);
                assert_eq!(incoming_unique_id, 0);
            }
            _ => panic!("wrong decoded outcome"),
        }
    }

    #[test]
    fn unknown_fields_are_rejected_at_every_action_layer() {
        assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(r#"{"kind":"begin","ownerLease":1}"#).is_err());
        for pointer in ["", "/operation", "/operation/intent", "/operation/intent/request"] {
            let mut value = action_json();
            value.pointer_mut(pointer).unwrap().as_object_mut().unwrap().insert("unexpected".into(), json!(true));
            assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(&value.to_string()).is_err(), "{pointer}");
        }
        assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(r#"{"kind":"quote","request":{"itemIndex":0,"count":1,"panelType":0,"name":"invented"}}"#).is_err());
    }

    #[test]
    fn unknown_fields_are_rejected_at_every_reply_layer() {
        for pointer in ["", "/receipt", "/receipt/entry", "/receipt/entry/operation", "/receipt/entry/operation/intent", "/receipt/entry/operation/intent/request", "/receipt/entry/outcome/committed", "/receipt/entry/outcome/committed/request"] {
            let mut value = reply_json();
            value.pointer_mut(pointer).unwrap().as_object_mut().unwrap().insert("unexpected".into(), json!(true));
            assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&value.to_string()).is_err(), "{pointer}");
        }
        for value in [
            json!({ "kind": "producer", "producer": { "actor": ([17; 32].to_vec()), "producerScope": ([53; 32].to_vec()), "serverRevision": 0, "ownerLease": 1 } }),
            json!({ "kind": "quote", "intent": operation_json()["intent"].clone(), "packets": [] }),
            json!({ "kind": "recovery", "receipt": null, "retry": true }),
        ] {
            assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&value.to_string()).is_err());
        }
        let mut quote = json!({ "kind": "quote", "intent": operation_json()["intent"].clone() });
        quote["intent"]["ownerLease"] = json!(1);
        assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&quote.to_string()).is_err());
    }

    #[test]
    fn raw_duplicate_and_escaped_duplicate_action_fields_are_refused() {
        let value = action_json();
        assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(&value.to_string()).is_ok());
        for (field, escaped, pointer) in [
            ("kind", r"k\u0069nd", "/kind"),
            ("operation", r"oper\u0061tion", "/operation"),
            ("actor", r"ac\u0074or", "/operation/actor"),
            ("requestScope", r"request\u0053cope", "/operation/requestScope"),
            ("sequence", r"se\u0071uence", "/operation/sequence"),
            ("intent", r"int\u0065nt", "/operation/intent"),
            ("request", r"req\u0075est", "/operation/intent/request"),
            ("currency", r"cur\u0072ency", "/operation/intent/currency"),
            ("source", r"sou\u0072ce", "/operation/intent/source"),
            ("serviceCatalogProof", r"serviceCatalog\u0050roof", "/operation/intent/serviceCatalogProof"),
            ("itemIndex", r"item\u0049ndex", "/operation/intent/request/itemIndex"),
            ("count", r"co\u0075nt", "/operation/intent/request/count"),
            ("panelType", r"panel\u0054ype", "/operation/intent/request/panelType"),
        ] {
            for duplicate in [field, escaped] {
                let raw = duplicate_field(&value, field, duplicate, value.pointer(pointer).unwrap());
                assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(&raw).is_err(), "{duplicate}");
            }
        }
    }

    #[test]
    fn raw_duplicate_and_escaped_duplicate_reply_fields_are_refused() {
        let value = reply_json();
        assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&value.to_string()).is_ok());
        for (field, escaped, pointer) in [
            ("kind", r"k\u0069nd", "/kind"),
            ("receipt", r"rece\u0069pt", "/receipt"),
            ("replayed", r"repl\u0061yed", "/replayed"),
            ("producerScope", r"producer\u0053cope", "/receipt/producerScope"),
            ("entry", r"en\u0074ry", "/receipt/entry"),
            ("serverRevision", r"server\u0052evision", "/receipt/entry/serverRevision"),
            ("outcome", r"out\u0063ome", "/receipt/entry/outcome"),
            ("charged", r"char\u0067ed", "/receipt/entry/outcome/committed/charged"),
            ("admittedCount", r"admitted\u0043ount", "/receipt/entry/outcome/committed/admittedCount"),
            ("incomingUniqueId", r"incoming\u0055niqueId", "/receipt/entry/outcome/committed/incomingUniqueId"),
        ] {
            for duplicate in [field, escaped] {
                let raw = duplicate_field(&value, field, duplicate, value.pointer(pointer).unwrap());
                assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&raw).is_err(), "{duplicate}");
            }
        }
        let recovery = json!({ "kind": "recovery", "receipt": null });
        for duplicate in ["receipt", r"rece\u0069pt"] {
            assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&duplicate_field(&recovery, "receipt", duplicate, &Value::Null)).is_err());
        }
        let producer = json!({ "kind": "producer", "producer": {
            "actor": ([17; 32].to_vec()), "producerScope": ([53; 32].to_vec()), "serverRevision": 0
        } });
        let quote = json!({ "kind": "quote", "intent": operation_json()["intent"].clone() });
        for (document, field, escaped, pointer) in [
            (&producer, "producer", r"pro\u0064ucer", "/producer"),
            (&producer, "actor", r"ac\u0074or", "/producer/actor"),
            (&producer, "producerScope", r"producer\u0053cope", "/producer/producerScope"),
            (&producer, "serverRevision", r"server\u0052evision", "/producer/serverRevision"),
            (&quote, "intent", r"int\u0065nt", "/intent"),
        ] {
            assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&document.to_string()).is_ok());
            for duplicate in [field, escaped] {
                assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&duplicate_field(document, field, duplicate, document.pointer(pointer).unwrap())).is_err(), "{duplicate}");
            }
        }
    }

    #[test]
    fn recovery_requires_explicit_receipt_and_only_null_represents_a_miss() {
        assert_eq!(serde_json::from_str::<NpcPurchaseOwnerReply>(r#"{"kind":"recovery","receipt":null}"#).unwrap(), NpcPurchaseOwnerReply::Recovery { receipt: None });
        for raw in [r#"{"kind":"recovery"}"#, r#"{"kind":"recovery","receipt":false}"#, r#"{"kind":"recovery","receipt":{}}"#] {
            assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(raw).is_err());
        }
    }

    #[test]
    fn unknown_tags_and_wrong_variant_shapes_are_rejected() {
        for raw in [
            r#"{}"#, r#"{"kind":"retry"}"#, r#"{"kind":"Begin"}"#, r#"{"kind":1}"#,
            r#"{"kind":"quote"}"#, r#"{"kind":"query"}"#, r#"{"kind":"purchase","operation":null}"#,
            r#"{"kind":"begin","operation":{}}"#, r#"{"begin":{}}"#, r#"[]"#,
        ] { assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(raw).is_err(), "{raw}"); }
        for raw in [
            r#"{}"#, r#"{"kind":"unknown"}"#, r#"{"kind":"Recovery","receipt":null}"#,
            r#"{"kind":"producer"}"#, r#"{"kind":"quote","intent":null}"#,
            r#"{"kind":"purchase","receipt":null,"replayed":false}"#, r#"{"recovery":{"receipt":null}}"#,
        ] { assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(raw).is_err(), "{raw}"); }
        let mut purchase = reply_json();
        purchase.as_object_mut().unwrap().remove("replayed");
        assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&purchase.to_string()).is_err());
    }

    #[test]
    fn wrong_numeric_types_and_ranges_are_never_coerced() {
        for (pointer, malformed) in [
            ("/operation/sequence", json!(-1)), ("/operation/sequence", json!(1.0)),
            ("/operation/sequence", json!("18446744073709551615")),
            ("/operation/intent/request/itemIndex", json!("0")),
            ("/operation/intent/request/count", json!(65536)),
            ("/operation/intent/request/panelType", json!(256)),
        ] {
            let mut value = action_json();
            *value.pointer_mut(pointer).unwrap() = malformed;
            assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(&value.to_string()).is_err(), "{pointer}");
        }
        let overflow = action_json().to_string().replacen("\"sequence\":18446744073709551615", "\"sequence\":18446744073709551616", 1);
        assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(&overflow).is_err());
        for (pointer, malformed) in [
            ("/replayed", json!(1)), ("/replayed", json!("true")),
            ("/receipt/entry/serverRevision", json!(1.5)),
            ("/receipt/entry/outcome/committed/charged", json!(4294967296u64)),
            ("/receipt/entry/outcome/committed/admittedCount", json!(-1)),
            ("/receipt/entry/outcome/committed/incomingUniqueId", json!("0")),
        ] {
            let mut value = reply_json();
            *value.pointer_mut(pointer).unwrap() = malformed;
            assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&value.to_string()).is_err(), "{pointer}");
        }
    }

    #[test]
    fn wrong_opaque_width_and_nested_enum_shapes_are_rejected() {
        for pointer in ["/operation/actor", "/operation/requestScope", "/operation/intent/serviceCatalogProof"] {
            for malformed in [json!([17; 31].to_vec()), json!([17; 33].to_vec()), json!("opaque"), json!(([256; 32].to_vec()))] {
                let mut value = action_json();
                *value.pointer_mut(pointer).unwrap() = malformed;
                assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(&value.to_string()).is_err(), "{pointer}");
            }
        }
        for (pointer, malformed) in [
            ("/operation/intent/currency", json!("gems")),
            ("/operation/intent/source", json!("buy_back")),
            ("/operation/intent", json!({ "service_catalog_proof": ([41; 32].to_vec()) })),
        ] {
            let mut value = action_json();
            *value.pointer_mut(pointer).unwrap() = malformed;
            assert!(serde_json::from_str::<NpcPurchaseOwnerAction>(&value.to_string()).is_err());
        }
        let mut reply = reply_json();
        reply["receipt"]["entry"]["outcome"] = json!({ "unknown": {} });
        assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&reply.to_string()).is_err());
        for malformed in [json!([53; 31].to_vec()), json!([53; 33].to_vec()), json!("opaque")] {
            let mut reply = reply_json();
            reply["receipt"]["producerScope"] = malformed.clone();
            assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&reply.to_string()).is_err());
            let producer = json!({ "kind": "producer", "producer": {
                "actor": ([17; 32].to_vec()), "producerScope": malformed, "serverRevision": 0
            } });
            assert!(serde_json::from_str::<NpcPurchaseOwnerReply>(&producer.to_string()).is_err());
        }
    }
}

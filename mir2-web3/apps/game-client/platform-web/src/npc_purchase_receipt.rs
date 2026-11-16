//! Strict JSON edge for one permanent purchase host, retained by the document.
//!
//! Local opaque tokens establish callback custody, never server authentication.
//! Only a trusted adapter receiving an actual authenticated server frame may
//! call receive. It must retain the original socket token with each callback.
//! Applied is a separate trusted-host input after one full economy bundle has
//! completed application. Receipt decode, enqueue, authority headers, movement,
//! and inventory readiness cannot call it. This module performs no I/O.
//! Do not recreate/free this bridge at UI remount or reconnect: its actor map,
//! original operations, sequences and unresolved controls remain in custody.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use mir2_client_core::npc_purchase_host::{BeginTicket, ConnectionToken,
    NpcPurchaseReceiptHost, PurchaseBinding};
use mir2_client_core::npc_purchase_receipt as core;
use mir2_client_wire as wire;
#[cfg(test)]
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;
use crate::strict_json::StrictMailValue;

const MAX_CONTROLS: usize = 64;
// A bounded raw frame encoded inside a JSON string can expand sixfold.
const MAX_INPUT: usize = 6 * wire::MAX_SERVER_FRAME_BYTES + wire::MAX_CLIENT_REQUEST_BYTES;
static LAST_BRIDGE: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase", deny_unknown_fields))]
struct BindingStamp { actor: wire::Opaque32, producer_scope: wire::Opaque32, begin_id: wire::U64 }

#[cfg_attr(test, derive(Debug, PartialEq, Deserialize))]
#[cfg_attr(test, serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields))]
enum Request {
    OpenConnection {},
    Begin { token: wire::Opaque32 },
    Receive { token: wire::Opaque32, frame: String },
    Quote { token: wire::Opaque32, binding: BindingStamp, request: wire::PurchaseRequest },
    Reserve { token: wire::Opaque32, binding: BindingStamp, intent: wire::Intent },
    Enter { token: wire::Opaque32, binding: BindingStamp, operation: wire::Operation },
    CancelUnsent { token: wire::Opaque32, binding: BindingStamp, operation: wire::Operation },
    Unknown { token: wire::Opaque32, binding: BindingStamp, operation: wire::Operation },
    Query { token: wire::Opaque32, binding: BindingStamp },
    Applied { token: wire::Opaque32, binding: BindingStamp, authority: wire::Producer, complete: bool },
    Withdraw { token: wire::Opaque32, disconnect: bool },
    Status {},
}

// The raw duplicate walker runs before this exact shape decoder. Keeping one
// Value edge avoids linking a second enum/content deserializer into the small
// browser policy bundle; the typed wire validators remain authoritative.
impl Request {
    fn field_names(op: &str) -> Result<&'static [&'static str], wire::WireError> {
        Ok(match op {
            "openConnection" | "status" => &["op"],
            "begin" => &["op", "token"],
            "receive" => &["op", "token", "frame"],
            "quote" => &["op", "token", "binding", "request"],
            "reserve" => &["op", "token", "binding", "intent"],
            "enter" | "cancelUnsent" | "unknown" => &["op", "token", "binding", "operation"],
            "query" => &["op", "token", "binding"],
            "applied" => &["op", "token", "binding", "authority", "complete"],
            "withdraw" => &["op", "token", "disconnect"],
            _ => return Err(wire::WireError::Invalid("unknown bridge operation")),
        })
    }

    fn from_value(value: &Value) -> Result<Self, wire::WireError> {
        use wire::value_decode as decode;
        // Internally tagged serde structs also accept a tag-first sequence.
        // Normalize only an exact sequence in the declared field order.
        let normalized;
        let value = if let Some(sequence) = value.as_array() {
            let tag = sequence.first().ok_or(wire::WireError::Invalid("bridge operation required"))?;
            let names = Self::field_names(decode::string(tag)?)?;
            if sequence.len() != names.len() { return Err(wire::WireError::Invalid("bridge request field count")); }
            normalized = Value::Object(names.iter().zip(sequence).map(|(name, value)| ((*name).to_owned(), value.clone())).collect());
            &normalized
        } else { value };
        let input = value.as_object().ok_or(wire::WireError::Invalid("bridge request object required"))?;
        let op = decode::string(decode::field(input, "op")?)?;
        let fields = Self::field_names(op)?;
        let input = decode::object(value, fields)?;
        if op == "openConnection" { return Ok(Self::OpenConnection {}); }
        if op == "status" { return Ok(Self::Status {}); }
        let token = decode::opaque(decode::field(input, "token")?)?;
        if op == "begin" { return Ok(Self::Begin { token }); }
        if op == "receive" {
            return Ok(Self::Receive { token, frame: decode::string(decode::field(input, "frame")?)?.to_owned() });
        }
        if op == "withdraw" {
            let disconnect = decode::field(input, "disconnect")?.as_bool()
                .ok_or(wire::WireError::Invalid("bridge disconnect boolean required"))?;
            return Ok(Self::Withdraw { token, disconnect });
        }
        let binding = BindingStamp::from_value(decode::field(input, "binding")?)?;
        Ok(match op {
            "quote" => Self::Quote { token, binding, request: decode::purchase_request(decode::field(input, "request")?)? },
            "reserve" => Self::Reserve { token, binding, intent: decode::intent_buffered(decode::field(input, "intent")?)? },
            "enter" | "cancelUnsent" | "unknown" => {
                let operation = decode::operation_buffered(decode::field(input, "operation")?)?;
                match op {
                    "enter" => Self::Enter { token, binding, operation },
                    "cancelUnsent" => Self::CancelUnsent { token, binding, operation },
                    _ => Self::Unknown { token, binding, operation },
                }
            }
            "query" => Self::Query { token, binding },
            "applied" => {
                let authority = decode::producer(decode::field(input, "authority")?)?;
                let complete = decode::field(input, "complete")?.as_bool()
                    .ok_or(wire::WireError::Invalid("bridge complete boolean required"))?;
                Self::Applied { token, binding, authority, complete }
            }
            _ => return Err(wire::WireError::Invalid("unknown bridge operation")),
        })
    }
}

impl BindingStamp {
    fn from_value(value: &Value) -> Result<Self, wire::WireError> {
        use wire::value_decode as decode;
        // Serde's existing struct edge also accepts an exact positional tuple.
        // Preserve that spelling while requiring all three original fields.
        if let Some(fields) = value.as_array() {
            if fields.len() != 3 { return Err(wire::WireError::Invalid("bridge binding field count")); }
            return Ok(Self { actor: decode::opaque(&fields[0])?,
                producer_scope: decode::opaque(&fields[1])?, begin_id: decode::u64(&fields[2])? });
        }
        let input = decode::object(value, &["actor", "producerScope", "beginId"])?;
        Ok(Self { actor: decode::opaque(decode::field(input, "actor")?)?,
            producer_scope: decode::opaque(decode::field(input, "producerScope")?)?,
            begin_id: decode::u64(decode::field(input, "beginId")?)? })
    }
}

struct Dispatch {
    token: wire::Opaque32,
    connection: ConnectionToken,
    request: wire::ClientRequest,
    ticket: Option<BeginTicket>,
    binding: Option<(BindingStamp, PurchaseBinding)>,
}

struct Reservation {
    token: wire::Opaque32,
    binding: BindingStamp,
    operation: wire::Operation,
}

#[wasm_bindgen]
pub struct NpcPurchaseReceiptBridge {
    host: NpcPurchaseReceiptHost,
    run: u64,
    last_connection: u64,
    last_control: u64,
    connection: Option<(wire::Opaque32, ConnectionToken)>,
    binding: Option<(BindingStamp, PurchaseBinding)>,
    quote: Option<(PurchaseBinding, wire::Intent)>,
    latest_quote: Option<u64>,
    controls: BTreeMap<u64, Dispatch>,
    reservations: BTreeMap<core::ActorKey, Reservation>,
    actors: BTreeSet<core::ActorKey>,
}

#[wasm_bindgen]
pub fn npc_purchase_receipt_abi_version() -> u32 { 1 }

#[wasm_bindgen]
impl NpcPurchaseReceiptBridge {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, JsValue> {
        let previous = LAST_BRIDGE.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
            |value| value.checked_add(1)).map_err(|_| JsValue::from_str("Purchase bridge identity exhausted"))?;
        let host = NpcPurchaseReceiptHost::new().ok_or_else(|| JsValue::from_str("Purchase host identity exhausted"))?;
        Ok(Self { host, run: (previous + 1) as u64, last_connection: 0, last_control: 0,
            connection: None, binding: None, quote: None, latest_quote: None, controls: BTreeMap::new(),
            reservations: BTreeMap::new(), actors: BTreeSet::new() })
    }

    pub fn transact(&mut self, raw: &str) -> String {
        match self.parse_and_apply(raw) {
            Ok(value) => value.to_string(),
            Err(error) => json!({"ok":false,"error":error}).to_string(),
        }
    }
}

type ResultValue = Result<Value, &'static str>;
impl NpcPurchaseReceiptBridge {
    fn parse_and_apply(&mut self, raw: &str) -> ResultValue {
        if raw.is_empty() || raw.len() > MAX_INPUT { return Err("Invalid purchase bridge input size"); }
        let StrictMailValue(value) = serde_json::from_str(raw).map_err(|_| "Invalid purchase bridge JSON")?;
        let request = Request::from_value(&value).map_err(|_| "Invalid purchase bridge request")?;
        self.apply(request)
    }

    fn connection_for(&self, token: wire::Opaque32) -> Result<ConnectionToken, &'static str> {
        self.connection.filter(|(saved, _)| *saved == token).map(|(_, connection)| connection)
            .ok_or("Stale purchase connection")
    }

    fn binding_for(&self, token: wire::Opaque32, stamp: &BindingStamp) -> Result<PurchaseBinding, &'static str> {
        self.connection_for(token)?;
        let (saved, binding) = self.binding.as_ref().ok_or("Purchase authority unavailable")?;
        if saved != stamp || self.host.current_binding() != Some(*binding) { return Err("Stale purchase binding"); }
        Ok(*binding)
    }

    fn allocate(&mut self, action: wire::Action) -> Result<(wire::ClientRequest, String), &'static str> {
        if self.controls.len() >= MAX_CONTROLS { return Err("Purchase control capacity reached"); }
        let sequence = self.last_control.checked_add(1).ok_or("Purchase control IDs exhausted")?;
        let request = wire::ClientRequest::new(wire::U64::new(sequence), action).map_err(|_| "Invalid purchase wire request")?;
        let body = wire::encode_client_request_compact(&request).map_err(|_| "Invalid purchase wire request")?;
        self.last_control = sequence;
        Ok((request, body))
    }

    fn dispatched(&mut self, token: wire::Opaque32, connection: ConnectionToken,
        request: wire::ClientRequest, body: String, ticket: Option<BeginTicket>,
        binding: Option<(BindingStamp, PurchaseBinding)>) -> Value
    {
        let response = json!({"ok":true,"request":wire::value_encode::client_request(&request).expect("validated purchase request"),"body":body});
        self.controls.insert(request.request_id.get(), Dispatch { token, connection, request, ticket, binding });
        response
    }

    fn reservation_matches(&self, token: wire::Opaque32, binding: &BindingStamp, operation: &wire::Operation)
        -> Result<core::OperationKey, &'static str>
    {
        let key = operation_key(operation);
        let saved = self.reservations.get(&key.actor).ok_or("Original purchase custody missing")?;
        if saved.token != token || &saved.binding != binding || &saved.operation != operation {
            return Err("Original purchase custody mismatch");
        }
        Ok(key)
    }

    fn withdraw_selected_authority(&mut self) {
        if let Some((_, connection)) = self.connection { self.host.withdraw_current(connection); }
        self.binding = None; self.quote = None; self.latest_quote = None;
    }

    fn apply(&mut self, request: Request) -> ResultValue {
        match request {
            Request::OpenConnection {} => {
                self.withdraw_selected_authority();
                let sequence = self.last_connection.checked_add(1).ok_or("Purchase connection IDs exhausted")?;
                let connection = self.host.open_connection().ok_or("Purchase host connections exhausted")?;
                let mut bytes = [0; 32];
                bytes[..8].copy_from_slice(&self.run.to_be_bytes());
                bytes[8..16].copy_from_slice(&sequence.to_be_bytes());
                let token = wire::Opaque32::from_bytes(bytes).map_err(|_| "Invalid local connection token")?;
                self.last_connection = sequence;
                self.connection = Some((token, connection));
                self.binding = None; self.quote = None; self.latest_quote = None;
                Ok(json!({"ok":true,"token":wire::value_encode::opaque(token)}))
            }
            Request::Begin { token } => {
                let connection = self.connection_for(token)?;
                self.withdraw_selected_authority();
                let (request, body) = self.allocate(wire::Action::Begin)?;
                let ticket = self.host.request_begin(connection).ok_or("Purchase host Begin unavailable")?;
                self.binding = None; self.quote = None; self.latest_quote = None;
                Ok(self.dispatched(token, connection, request, body, Some(ticket), None))
            }
            Request::Receive { token, frame } => self.receive(token, &frame),
            Request::Quote { token, binding, request } => {
                let current = self.binding_for(token, &binding)?;
                let connection = self.connection_for(token)?;
                let (request, body) = self.allocate(wire::Action::Quote { request })?;
                // A previous quote is unavailable while its replacement is pending.
                self.quote = None;
                self.latest_quote = Some(request.request_id.get());
                Ok(self.dispatched(token, connection, request, body, None, Some((binding, current))))
            }
            Request::Reserve { token, binding, intent } => {
                let current = self.binding_for(token, &binding)?;
                if !self.quote.as_ref().is_some_and(|(saved, quoted)| *saved == current && quoted == &intent) {
                    return Err("Exact current server quote required");
                }
                let purchase = core_intent(&intent);
                let key = self.host.reserve(current, purchase).ok_or("Purchase reservation unavailable")?;
                let operation = operation(key, purchase);
                self.reservations.insert(key.actor, Reservation { token, binding, operation: operation.clone() });
                self.quote = None;
                self.latest_quote = None;
                Ok(json!({"ok":true,"operation":wire::value_encode::operation(&operation).expect("validated reserved operation")}))
            }
            Request::Enter { token, binding, operation } => {
                let current = self.binding_for(token, &binding)?;
                let key = self.reservation_matches(token, &binding, &operation)?;
                let connection = self.connection_for(token)?;
                // Allocate and encode before changing the economic flight; after
                // entry this body is returned once and is never regenerated.
                let (request, body) = self.allocate(wire::Action::Purchase { operation: operation.clone() })?;
                if !self.host.begin_entry(current, key, core_intent(&operation.intent)) {
                    return Err("Purchase entry refused");
                }
                Ok(self.dispatched(token, connection, request, body, None, Some((binding, current))))
            }
            Request::CancelUnsent { token, binding, operation } => {
                let key = self.reservation_matches(token, &binding, &operation)?;
                let matched = self.host.cancel_definitely_unsent(key, core_intent(&operation.intent));
                if matched { self.reservations.remove(&key.actor); }
                Ok(json!({"ok":true,"matched":matched}))
            }
            Request::Unknown { token, binding, operation } => {
                let key = self.reservation_matches(token, &binding, &operation)?;
                Ok(json!({"ok":true,"matched":self.host.mark_unknown(key)}))
            }
            Request::Query { token, binding } => {
                let current = self.binding_for(token, &binding)?;
                let query = self.host.recovery_query(current).ok_or("Entered original operation required")?;
                let connection = self.connection_for(token)?;
                let (request, body) = self.allocate(wire::Action::Query { operation: operation(query.key, query.intent) })?;
                Ok(self.dispatched(token, connection, request, body, None, Some((binding, current))))
            }
            Request::Applied { token, binding, authority, complete } => {
                let current = self.binding_for(token, &binding)?;
                if authority.actor != binding.actor || authority.producer_scope != binding.producer_scope {
                    return Err("Applied authority mismatch");
                }
                let witness = core::SnapshotWitness { actor: current.actor(), producer_scope: current.producer_scope(),
                    server_revision: authority.server_revision.get(), complete };
                let observed = self.host.observe_applied_snapshot(current, witness);
                Ok(json!({"ok":true,"observation":self.observation(observed)}))
            }
            Request::Withdraw { token, disconnect } => {
                let connection = self.connection_for(token)?;
                let matched = if disconnect { self.host.withdraw_connection(connection) }
                    else { self.host.withdraw_current(connection) };
                self.binding = None; self.quote = None; self.latest_quote = None;
                if disconnect { self.connection = None; }
                Ok(json!({"ok":true,"matched":matched}))
            }
            Request::Status {} => Ok(self.status()),
        }
    }

    fn receive(&mut self, token: wire::Opaque32, raw: &str) -> ResultValue {
        let connection = self.connection_for(token)?;
        let frame = decode_server_frame(raw)?;
        let control = frame.request_id.get();
        let saved = self.controls.get(&control).ok_or("Purchase control custody missing")?;
        if saved.token != token || saved.connection != connection { return Err("Purchase reply connection mismatch"); }
        frame.validate_for_request(&saved.request).map_err(|_| "Purchase reply correlation mismatch")?;
        if let Some((stamp, binding)) = &saved.binding {
            if self.binding_for(token, stamp)? != *binding { return Err("Stale purchase reply binding"); }
            if frame.authority.as_ref().is_some_and(|authority|
                authority.actor != stamp.actor || authority.producer_scope != stamp.producer_scope) {
                return Err("Purchase reply authority mismatch");
            }
            if frame.reply.receipt().is_some_and(|receipt| receipt.producer_scope != stamp.producer_scope) {
                return Err("Purchase receipt producer mismatch");
            }
        }
        if let wire::ServerReply::Producer { producer } = &frame.reply {
            // Initial authority must be the actual producer+snapshot pair. This
            // validates pairing, never applies the snapshot or opens admission.
            if frame.authority.as_ref() != Some(producer) || frame.snapshot.is_none() {
                return Err("Accepted Begin requires paired producer snapshot");
            }
        }
        // Exact parsed response retires only this control; economic custody is
        // held independently in Core and never cleared by miss or failure.
        let saved = self.controls.remove(&control).expect("checked control custody");
        let mut output = json!({"ok":true,"frame":wire::value_encode::server_frame(&frame).expect("validated purchase frame"),"observation":{"kind":"ignored"}});
        match &frame.reply {
            wire::ServerReply::Producer { producer } => {
                let ticket = saved.ticket.ok_or("Begin ticket custody missing")?;
                let binding = self.host.bind(ticket, actor(producer.actor), scope(producer.producer_scope))
                    .map_err(|_| "Accepted Begin binding refused")?;
                let stamp = BindingStamp { actor: producer.actor, producer_scope: producer.producer_scope,
                    begin_id: frame.request_id };
                self.actors.insert(binding.actor());
                self.binding = Some((stamp.clone(), binding)); self.quote = None; self.latest_quote = None;
                output["kind"] = json!("producer"); output["binding"] = binding_value(&stamp);
            }
            wire::ServerReply::Quote { intent } => {
                if self.latest_quote != Some(control) { return Err("Superseded purchase quote response"); }
                let (_, binding) = saved.binding.ok_or("Quote binding custody missing")?;
                self.quote = Some((binding, intent.clone()));
                output["kind"] = json!("quote"); output["intent"] = wire::value_encode::intent(intent).expect("validated purchase intent");
            }
            _ => {
                output["kind"] = json!("result");
                if let Some(receipt) = frame.reply.receipt() {
                    let (_, binding) = saved.binding.ok_or("Receipt binding custody missing")?;
                    let observed = self.host.observe_receipt(binding, core_receipt(receipt));
                    output["observation"] = self.observation(observed);
                } else if let Some(operation) = saved.request.action.operation() {
                    // Query miss, Unknown or accepted BeforeExecution preserves
                    // the entered barrier and never becomes an unsent cancel.
                    let retained = self.host.mark_unknown(operation_key(operation));
                    output["observation"] = json!({"kind":if retained { "pending" } else { "ignored" }});
                }
            }
        }
        Ok(output)
    }

    fn observation(&mut self, observed: core::Observation) -> Value {
        match observed {
            core::Observation::Ignored => json!({"kind":"ignored"}),
            core::Observation::Pending => json!({"kind":"pending"}),
            core::Observation::Settled(value) => {
                self.reservations.remove(&value.key.actor);
                let result = match value.result {
                    core::EconomicResult::Committed { server_revision, currency, source, charged, admitted_count, incoming_uid } =>
                        json!({"kind":"committed","serverRevision":wire::value_encode::u64(wire::U64::new(server_revision)),
                            "currency":wire::value_encode::currency(wire_currency(currency)),"source":wire::value_encode::source(wire_source(source)),"charged":charged,
                            "admittedCount":admitted_count,"incomingUniqueId":wire::value_encode::u64(wire::U64::new(incoming_uid))}),
                    core::EconomicResult::Rejected { server_revision } =>
                        json!({"kind":"rejected","serverRevision":wire::value_encode::u64(wire::U64::new(server_revision))}),
                    core::EconomicResult::Unknown => json!({"kind":"unknown"}),
                };
                json!({"kind":"settled","settlement":{"operation":wire::value_encode::operation(&operation(value.key, value.intent)).expect("validated retained operation"),"result":result}})
            }
        }
    }

    fn status(&self) -> Value {
        let actors: Vec<_> = self.actors.iter().map(|actor| {
            let ledger = self.host.ledger(*actor).expect("retained actor");
            json!({"actor":wire::value_encode::opaque(opaque(actor.bytes())),"lastSequence":wire::value_encode::u64(wire::U64::new(ledger.last_sequence())),
                "appliedBaseline":ledger.applied_baseline().map(|revision|wire::value_encode::u64(wire::U64::new(revision))),"pending":ledger.pending().map(pending)})
        }).collect();
        json!({"ok":true,"token":self.connection.map(|(token, _)|wire::value_encode::opaque(token)),
            "binding":self.binding.as_ref().map(|(stamp, _)|binding_value(stamp)),
            "pending":self.host.pending_current().map(pending),"actors":actors,"controls":self.controls.len()})
    }
}


// Both JSON documents retain their own bounded, recursive duplicate check.
// Reuse the already-linked strict DOM walker rather than instantiate a second
// equivalent visitor in the small browser bundle.
fn decode_server_frame(raw: &str) -> Result<wire::ServerFrame, &'static str> {
    if raw.is_empty() || raw.len() > wire::MAX_SERVER_FRAME_BYTES {
        return Err("Invalid purchase server frame");
    }
    let StrictMailValue(value) = serde_json::from_str(raw).map_err(|_| "Invalid purchase server frame")?;
    wire::value_decode::server_frame(&value).map_err(|_| "Invalid purchase server frame")
}

fn binding_value(value: &BindingStamp) -> Value {
    json!({"actor":wire::value_encode::opaque(value.actor),
        "producerScope":wire::value_encode::opaque(value.producer_scope),
        "beginId":wire::value_encode::u64(value.begin_id)})
}

fn actor(value: wire::Opaque32) -> core::ActorKey { core::ActorKey::from_server_bytes(value.get()).expect("strict actor") }
fn scope(value: wire::Opaque32) -> core::OwnerScope { core::OwnerScope::from_server_bytes(value.get()).expect("strict scope") }
fn opaque(bytes: &[u8; 32]) -> wire::Opaque32 { wire::Opaque32::from_bytes(*bytes).expect("nonzero Core identity") }
fn core_currency(value: wire::Currency) -> core::PurchaseCurrency {
    match value { wire::Currency::Gold => core::PurchaseCurrency::Gold, wire::Currency::Pearls => core::PurchaseCurrency::Pearls }
}
fn wire_currency(value: core::PurchaseCurrency) -> wire::Currency {
    match value { core::PurchaseCurrency::Gold => wire::Currency::Gold, core::PurchaseCurrency::Pearls => wire::Currency::Pearls }
}
fn core_source(value: wire::Source) -> core::PurchaseSource {
    match value { wire::Source::Trade => core::PurchaseSource::Trade, wire::Source::BuyBack => core::PurchaseSource::BuyBack,
        wire::Source::Used => core::PurchaseSource::Used }
}
fn wire_source(value: core::PurchaseSource) -> wire::Source {
    match value { core::PurchaseSource::Trade => wire::Source::Trade, core::PurchaseSource::BuyBack => wire::Source::BuyBack,
        core::PurchaseSource::Used => wire::Source::Used }
}
fn core_intent(value: &wire::Intent) -> core::PurchaseIntent {
    core::PurchaseIntent { item_index: value.request.item_index.get(), requested_count: value.request.count,
        panel: value.request.panel_type, currency: core_currency(value.currency), source: core_source(value.source),
        service_catalog_proof: core::ServiceCatalogProof::from_server_bytes(value.service_catalog_proof.get()).expect("strict proof") }
}
fn operation_key(value: &wire::Operation) -> core::OperationKey {
    core::OperationKey { actor: actor(value.actor), request_id: core::RequestId::from_parts(scope(value.request_scope),
        value.sequence.get()).expect("strict nonzero sequence") }
}
fn operation(key: core::OperationKey, intent: core::PurchaseIntent) -> wire::Operation {
    wire::Operation { actor: opaque(key.actor.bytes()), request_scope: opaque(key.request_id.scope().bytes()),
        sequence: wire::U64::new(key.request_id.sequence()), intent: wire::Intent {
            request: wire::PurchaseRequest::new(wire::U64::new(intent.item_index), intent.requested_count, intent.panel).expect("Core count"),
            currency: wire_currency(intent.currency), source: wire_source(intent.source),
            service_catalog_proof: opaque(intent.service_catalog_proof.bytes()) } }
}
fn core_receipt(receipt: &wire::Receipt) -> core::PurchaseReceipt {
    let entry = &receipt.entry;
    let result = match &entry.outcome {
        wire::Outcome::Committed { currency, source, charged, admitted_count, incoming_unique_id, .. } =>
            core::EconomicResult::Committed { server_revision: entry.server_revision.get(),
                currency: core_currency(*currency), source: core_source(*source), charged: *charged,
                admitted_count: *admitted_count, incoming_uid: incoming_unique_id.get() },
        wire::Outcome::Rejected { .. } => core::EconomicResult::Rejected { server_revision: entry.server_revision.get() },
    };
    core::PurchaseReceipt { actor: actor(entry.operation.actor), producer_scope: scope(receipt.producer_scope),
        request_id: operation_key(&entry.operation).request_id, intent: core_intent(&entry.operation.intent), result }
}
fn pending(value: core::PendingPurchase) -> Value {
    let phase = match value.phase { core::PurchasePhase::Queued => "queued", core::PurchasePhase::Entered => "entered",
        core::PurchasePhase::Unknown => "unknown", core::PurchasePhase::AwaitingSnapshot => "awaitingSnapshot" };
    json!({"operation":wire::value_encode::operation(&operation(value.key, value.intent)).expect("validated retained operation"),"minimumRevision":wire::value_encode::u64(wire::U64::new(value.minimum_revision)),"phase":phase})
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure bridge/schema fixtures only, not fabricated production authority.
    fn opaque_fixture(value: u8) -> wire::Opaque32 { wire::Opaque32::from_bytes([value; 32]).unwrap() }
    fn producer(a: u8, p: u8, revision: u64) -> wire::Producer {
        wire::Producer { actor: opaque_fixture(a), producer_scope: opaque_fixture(p), server_revision: wire::U64::new(revision) }
    }
    fn intent() -> wire::Intent {
        wire::Intent { request: wire::PurchaseRequest::new(wire::U64::new(u64::MAX), 3, 0).unwrap(),
            currency: wire::Currency::Pearls, source: wire::Source::Used, service_catalog_proof: opaque_fixture(7) }
    }
    fn call(bridge: &mut NpcPurchaseReceiptBridge, value: Value) -> Value {
        serde_json::from_str(&bridge.transact(&value.to_string())).unwrap()
    }
    fn successful(bridge: &mut NpcPurchaseReceiptBridge, value: Value) -> Value {
        let result = call(bridge, value); assert_eq!(result["ok"], true, "{result}"); result
    }
    fn frame(request: &wire::ClientRequest, reply: wire::ServerReply, authority: Option<wire::Producer>) -> String {
        let snapshot = authority.as_ref().map(|_| json!({"pureStateFixture":true}));
        wire::encode_server_frame(&wire::ServerFrame::new(request.request_id, reply, snapshot, authority).unwrap()).unwrap()
    }
    fn dispatched(value: &Value) -> wire::ClientRequest { wire::parse_client_request(value["body"].as_str().unwrap()).unwrap() }
    fn begin(bridge: &mut NpcPurchaseReceiptBridge, token: &Value, a: u8, p: u8) -> Value {
        let sent = successful(bridge, json!({"op":"begin","token":token}));
        let request = dispatched(&sent); let authority = producer(a, p, 10);
        let raw = frame(&request, wire::ServerReply::Producer { producer: authority.clone() }, Some(authority));
        successful(bridge, json!({"op":"receive","token":token,"frame":raw}))["binding"].clone()
    }
    fn applied(bridge: &mut NpcPurchaseReceiptBridge, token: &Value, binding: &Value, revision: u64, complete: bool) -> Value {
        let authority = wire::Producer { actor: serde_json::from_value(binding["actor"].clone()).unwrap(),
            producer_scope: serde_json::from_value(binding["producerScope"].clone()).unwrap(), server_revision: wire::U64::new(revision) };
        successful(bridge, json!({"op":"applied","token":token,"binding":binding,"authority":authority,"complete":complete}))
    }
    fn ready() -> (NpcPurchaseReceiptBridge, Value, Value) {
        let mut bridge = NpcPurchaseReceiptBridge::new().unwrap();
        let token = successful(&mut bridge, json!({"op":"openConnection"}))["token"].clone();
        let binding = begin(&mut bridge, &token, 1, 2);
        applied(&mut bridge, &token, &binding, 10, true);
        (bridge, token, binding)
    }
    fn quote(bridge: &mut NpcPurchaseReceiptBridge, token: &Value, binding: &Value) {
        let sent = successful(bridge, json!({"op":"quote","token":token,"binding":binding,"request":intent().request}));
        let raw = frame(&dispatched(&sent), wire::ServerReply::Quote { intent: intent() }, None);
        successful(bridge, json!({"op":"receive","token":token,"frame":raw}));
    }
    fn reserve(bridge: &mut NpcPurchaseReceiptBridge, token: &Value, binding: &Value) -> Value {
        quote(bridge, token, binding);
        successful(bridge, json!({"op":"reserve","token":token,"binding":binding,"intent":intent()}))["operation"].clone()
    }
    fn enter(bridge: &mut NpcPurchaseReceiptBridge, token: &Value, binding: &Value, operation: &Value) -> wire::ClientRequest {
        dispatched(&successful(bridge, json!({"op":"enter","token":token,"binding":binding,"operation":operation})))
    }
    fn receipt(request: &wire::ClientRequest, p: u8, revision: u64) -> wire::Receipt {
        let operation = request.action.operation().unwrap().clone();
        wire::Receipt { producer_scope: opaque_fixture(p), entry: wire::ReceiptEntry { operation: operation.clone(),
            server_revision: wire::U64::new(revision), outcome: wire::Outcome::Committed {
                request: operation.intent.request, currency: operation.intent.currency, source: operation.intent.source,
                charged: 9, admitted_count: 2, incoming_unique_id: wire::U64::new(0) } } }
    }
    fn status(bridge: &mut NpcPurchaseReceiptBridge) -> Value { successful(bridge, json!({"op":"status"})) }

    #[test]
    fn strict_abi_rejects_duplicate_escaped_duplicate_unknown_and_noncanonical_ids() {
        let mut bridge = NpcPurchaseReceiptBridge::new().unwrap();
        for raw in [r#"{"op":"status","op":"openConnection"}"#, r#"{"op":"status","\u006fp":"openConnection"}"#,
            r#"{"op":"status","extra":null}"#, r#"{"op":"begin","token":1}"#,
            r#"{"op":"query","token":"00","binding":{}}"#] {
            let response: Value = serde_json::from_str(&bridge.transact(raw)).unwrap();
            assert_eq!(response["ok"], false, "{raw}");
        }
        assert_eq!(status(&mut bridge)["token"], Value::Null);
        let (mut bridge, token, binding) = ready();
        let operation = reserve(&mut bridge, &token, &binding);
        for bad in [json!(1), json!("01"), json!("+1"), json!("18446744073709551616")] {
            let mut wrong = operation.clone(); wrong["sequence"] = bad;
            assert_eq!(call(&mut bridge, json!({"op":"enter","token":token,"binding":binding,"operation":wrong}))["ok"], false);
        }
        let duplicate = format!(r#"{{"op":"unknown","token":{token},"binding":{binding},"operation":{{"sequence":"1","\u0073equence":"1"}}}}"#);
        let response: Value = serde_json::from_str(&bridge.transact(&duplicate)).unwrap();
        assert_eq!(response["ok"], false);
        assert_eq!(status(&mut bridge)["pending"]["phase"], "queued");
    }

    #[test]
    fn actual_begin_pair_never_implies_full_application_or_admission() {
        let mut bridge = NpcPurchaseReceiptBridge::new().unwrap();
        let token = successful(&mut bridge, json!({"op":"openConnection"}))["token"].clone();
        let sent = successful(&mut bridge, json!({"op":"begin","token":token}));
        let authority = producer(1, 2, 10);
        let unpaired = frame(&dispatched(&sent), wire::ServerReply::Producer { producer: authority.clone() }, None);
        assert_eq!(call(&mut bridge, json!({"op":"receive","token":token,"frame":unpaired}))["ok"], false);
        assert_eq!(status(&mut bridge)["binding"], Value::Null);
        let raw = frame(&dispatched(&sent), wire::ServerReply::Producer { producer: authority.clone() }, Some(authority));
        let binding = successful(&mut bridge, json!({"op":"receive","token":token,"frame":raw}))["binding"].clone();
        quote(&mut bridge, &token, &binding);
        assert_eq!(call(&mut bridge, json!({"op":"reserve","token":token,"binding":binding,"intent":intent()}))["ok"], false);
        applied(&mut bridge, &token, &binding, 10, false);
        assert_eq!(call(&mut bridge, json!({"op":"reserve","token":token,"binding":binding,"intent":intent()}))["ok"], false);
        applied(&mut bridge, &token, &binding, 10, true);
        assert_eq!(successful(&mut bridge, json!({"op":"reserve","token":token,"binding":binding,"intent":intent()}))["operation"]["intent"]["request"]["itemIndex"], u64::MAX.to_string());
    }

    #[test]
    fn full_u64_operation_settles_exactly_once_in_both_arrival_orders() {
        for snapshot_first in [false, true] {
            let (mut bridge, token, binding) = ready();
            let operation = reserve(&mut bridge, &token, &binding);
            let request = enter(&mut bridge, &token, &binding, &operation);
            assert!(request.action.is_mutation());
            assert_eq!(call(&mut bridge, json!({"op":"enter","token":token,"binding":binding,"operation":operation}))["ok"], false);
            let raw = frame(&request, wire::ServerReply::Purchase { receipt: receipt(&request, 2, 11), replayed: false }, Some(producer(1, 2, 11)));
            let last = if snapshot_first {
                assert_eq!(applied(&mut bridge, &token, &binding, 11, true)["observation"]["kind"], "pending");
                successful(&mut bridge, json!({"op":"receive","token":token,"frame":raw}))
            } else {
                assert_eq!(successful(&mut bridge, json!({"op":"receive","token":token,"frame":raw}))["observation"]["kind"], "pending");
                assert_eq!(status(&mut bridge)["pending"]["phase"], "awaitingSnapshot");
                applied(&mut bridge, &token, &binding, 11, true)
            };
            assert_eq!(last["observation"]["kind"], "settled");
            assert_eq!(last["observation"]["settlement"]["operation"], operation);
            assert_eq!(status(&mut bridge)["pending"], Value::Null);
            assert_eq!(call(&mut bridge, json!({"op":"receive","token":token,"frame":raw}))["ok"], false);
            let next = reserve(&mut bridge, &token, &binding);
            assert_eq!(next["sequence"], "2");
            assert_eq!(status(&mut bridge)["pending"]["operation"], next);
        }
    }

    #[test]
    fn same_producer_reconnect_queries_original_operation_and_miss_never_retries() {
        let (mut bridge, token, binding) = ready();
        let operation = reserve(&mut bridge, &token, &binding);
        let purchase = enter(&mut bridge, &token, &binding, &operation);
        successful(&mut bridge, json!({"op":"withdraw","token":token,"disconnect":true}));
        let next_token = successful(&mut bridge, json!({"op":"openConnection"}))["token"].clone();
        let next_binding = begin(&mut bridge, &next_token, 1, 2);
        assert_eq!(status(&mut bridge)["pending"]["phase"], "unknown");
        assert_eq!(call(&mut bridge, json!({"op":"enter","token":next_token,"binding":next_binding,"operation":operation}))["ok"], false);
        let sent = successful(&mut bridge, json!({"op":"query","token":next_token,"binding":next_binding}));
        let query = dispatched(&sent);
        assert_ne!(query.request_id, purchase.request_id);
        assert_eq!(query.action.operation(), purchase.action.operation());
        assert!(!query.action.is_mutation());
        let missing = frame(&query, wire::ServerReply::Recovery { receipt: None }, None);
        successful(&mut bridge, json!({"op":"receive","token":next_token,"frame":missing}));
        assert_eq!(status(&mut bridge)["pending"]["operation"], operation);
        assert_eq!(successful(&mut bridge, json!({"op":"cancelUnsent","token":token,"binding":binding,"operation":operation}))["matched"], false);
        let recovered_query = dispatched(&successful(&mut bridge, json!({"op":"query","token":next_token,"binding":next_binding})));
        let recovered = frame(&recovered_query, wire::ServerReply::Recovery { receipt: Some(receipt(&recovered_query, 2, 11)) }, None);
        assert_eq!(successful(&mut bridge, json!({"op":"receive","token":next_token,"frame":recovered}))["observation"]["kind"], "pending");
        assert_eq!(applied(&mut bridge, &next_token, &next_binding, 11, true)["observation"]["kind"], "settled");
        assert_eq!(status(&mut bridge)["actors"][0]["lastSequence"], "1");
    }

    #[test]
    fn delayed_control_wrong_action_and_old_transport_never_select_authority() {
        let (mut bridge, token, _) = ready();
        let stale = dispatched(&successful(&mut bridge, json!({"op":"begin","token":token})));
        let latest = dispatched(&successful(&mut bridge, json!({"op":"begin","token":token})));
        let authority = producer(9, 8, 10);
        let stale_raw = frame(&stale, wire::ServerReply::Producer { producer: authority.clone() }, Some(authority.clone()));
        assert_eq!(call(&mut bridge, json!({"op":"receive","token":token,"frame":stale_raw}))["ok"], false);
        assert_eq!(status(&mut bridge)["binding"], Value::Null);
        let wrong = frame(&latest, wire::ServerReply::Quote { intent: intent() }, None);
        assert_eq!(call(&mut bridge, json!({"op":"receive","token":token,"frame":wrong}))["ok"], false);
        let new_token = successful(&mut bridge, json!({"op":"openConnection"}))["token"].clone();
        let old_raw = frame(&latest, wire::ServerReply::Producer { producer: authority.clone() }, Some(authority));
        assert_eq!(call(&mut bridge, json!({"op":"receive","token":token,"frame":old_raw}))["ok"], false);
        assert_eq!(call(&mut bridge, json!({"op":"receive","token":new_token,"frame":old_raw}))["ok"], false);
        assert_eq!(status(&mut bridge)["binding"], Value::Null);
    }

    #[test]
    fn foreign_actor_and_ui_status_calls_preserve_all_entered_history() {
        let (mut bridge, token, binding) = ready();
        let original = reserve(&mut bridge, &token, &binding);
        enter(&mut bridge, &token, &binding, &original);
        for _ in 0..3 { assert_eq!(status(&mut bridge)["pending"]["operation"], original); }
        let foreign = begin(&mut bridge, &token, 9, 8);
        applied(&mut bridge, &token, &foreign, 10, true);
        let second = reserve(&mut bridge, &token, &foreign);
        enter(&mut bridge, &token, &foreign, &second);
        assert_eq!(status(&mut bridge)["actors"].as_array().unwrap().len(), 2);
        let resumed = begin(&mut bridge, &token, 1, 3);
        let query = dispatched(&successful(&mut bridge, json!({"op":"query","token":token,"binding":resumed})));
        assert_eq!(serde_json::to_value(query.action.operation().unwrap()).unwrap(), original);
        assert_eq!(status(&mut bridge)["actors"][1]["pending"]["operation"], second);
    }

    #[test]
    fn queued_same_scope_reconnect_only_cancels_exact_original_custody() {
        let (mut bridge, token, binding) = ready();
        let original = reserve(&mut bridge, &token, &binding);
        let new_token = successful(&mut bridge, json!({"op":"openConnection"}))["token"].clone();
        let new_binding = begin(&mut bridge, &new_token, 1, 2);
        applied(&mut bridge, &new_token, &new_binding, 10, true);
        assert_eq!(call(&mut bridge, json!({"op":"enter","token":new_token,"binding":new_binding,"operation":original}))["ok"], false);
        assert_eq!(call(&mut bridge, json!({"op":"query","token":new_token,"binding":new_binding}))["ok"], false);
        let mut wrong = original.clone(); wrong["intent"]["request"]["panelType"] = json!(1);
        assert_eq!(call(&mut bridge, json!({"op":"cancelUnsent","token":token,"binding":binding,"operation":wrong}))["ok"], false);
        assert_eq!(successful(&mut bridge, json!({"op":"cancelUnsent","token":token,"binding":binding,"operation":original}))["matched"], true);
        assert_eq!(reserve(&mut bridge, &new_token, &new_binding)["sequence"], "2");
    }

    #[test]
    fn before_execution_unknown_and_bad_receipts_never_clear_entered_operation() {
        for state in [wire::FailureState::BeforeExecution, wire::FailureState::Unknown] {
            let (mut bridge, token, binding) = ready();
            let operation = reserve(&mut bridge, &token, &binding);
            let request = enter(&mut bridge, &token, &binding, &operation);
            let mut bad = receipt(&request, 2, 11); bad.entry.operation.sequence = wire::U64::new(2);
            let raw = frame(&request, wire::ServerReply::Purchase { receipt: bad, replayed: false }, None);
            assert_eq!(call(&mut bridge, json!({"op":"receive","token":token,"frame":raw}))["ok"], false);
            let failure = frame(&request, wire::ServerReply::Failure { state, receipt: None }, None);
            successful(&mut bridge, json!({"op":"receive","token":token,"frame":failure}));
            applied(&mut bridge, &token, &binding, 99, true);
            assert_eq!(status(&mut bridge)["pending"]["operation"], operation);
            assert_eq!(status(&mut bridge)["pending"]["phase"], "unknown");
        }
    }

    #[test]
    fn unresolved_control_capacity_and_checked_exhaustion_refuse_without_eviction() {
        let (mut bridge, token, binding) = ready();
        let operation = reserve(&mut bridge, &token, &binding);
        enter(&mut bridge, &token, &binding, &operation);
        for _ in 1..MAX_CONTROLS {
            successful(&mut bridge, json!({"op":"query","token":token,"binding":binding}));
        }
        let before = status(&mut bridge);
        assert_eq!(before["controls"], MAX_CONTROLS);
        assert_eq!(call(&mut bridge, json!({"op":"query","token":token,"binding":binding}))["ok"], false);
        assert_eq!(status(&mut bridge), before);
        bridge.last_control = u64::MAX;
        assert_eq!(call(&mut bridge, json!({"op":"begin","token":token}))["ok"], false);
        let withdrawn = status(&mut bridge);
        assert_eq!(withdrawn["binding"], Value::Null);
        assert_eq!(withdrawn["pending"], Value::Null);
        assert_eq!(withdrawn["actors"][0]["pending"]["operation"], operation);
        assert_eq!(withdrawn["actors"][0]["lastSequence"], before["actors"][0]["lastSequence"]);
        assert_eq!(withdrawn["actors"][0]["pending"]["phase"], "unknown");
        assert_eq!(withdrawn["controls"], before["controls"]);
        let (mut exhausted, token, _) = ready();
        exhausted.last_control = u64::MAX;
        let before = status(&mut exhausted);
        assert_eq!(call(&mut exhausted, json!({"op":"begin","token":token}))["ok"], false);
        let after = status(&mut exhausted);
        assert_eq!(after["binding"], Value::Null);
        assert_eq!(after["token"], before["token"]);
        assert_eq!(after["controls"], before["controls"]);
        assert_eq!(after["actors"][0]["actor"], before["actors"][0]["actor"]);
        assert_eq!(after["actors"][0]["lastSequence"], before["actors"][0]["lastSequence"]);
        assert_eq!(after["actors"][0]["pending"], before["actors"][0]["pending"]);
        assert_eq!(after["actors"][0]["appliedBaseline"], Value::Null);
        let (mut exhausted_connection, token, _) = ready();
        exhausted_connection.last_connection = u64::MAX;
        assert_eq!(call(&mut exhausted_connection, json!({"op":"openConnection"}))["ok"], false);
        let after = status(&mut exhausted_connection);
        assert_eq!(after["token"], token);
        assert_eq!(after["binding"], Value::Null);
        assert_eq!(after["actors"][0]["lastSequence"], "0");
        assert_eq!(after["actors"][0]["appliedBaseline"], Value::Null);
    }

    #[test]
    fn bounded_input_and_nested_duplicate_frame_leave_saved_request_and_flight_intact() {
        let (mut bridge, token, binding) = ready();
        let operation = reserve(&mut bridge, &token, &binding);
        let request = enter(&mut bridge, &token, &binding, &operation);
        let raw = frame(&request, wire::ServerReply::Purchase { receipt: receipt(&request, 2, 11), replayed: false }, None);
        let duplicate = raw.replacen("\"requestId\":", "\"requestId\":\"999\",\"requestId\":", 1);
        assert_eq!(call(&mut bridge, json!({"op":"receive","token":token,"frame":duplicate}))["ok"], false);
        let oversized = " ".repeat(MAX_INPUT + 1);
        let response: Value = serde_json::from_str(&bridge.transact(&oversized)).unwrap();
        assert_eq!(response["ok"], false);
        assert_eq!(status(&mut bridge)["pending"]["operation"], operation);
        assert_eq!(status(&mut bridge)["controls"], 1);
        assert_eq!(successful(&mut bridge, json!({"op":"receive","token":token,"frame":raw}))["observation"]["kind"], "pending");
    }

    #[test]
    fn delayed_same_binding_quote_cannot_replace_latest_selection() {
        let (mut bridge, token, binding) = ready();
        let earlier = dispatched(&successful(&mut bridge, json!({"op":"quote","token":token,"binding":binding,"request":intent().request})));
        let mut latest_intent = intent();
        latest_intent.request.item_index = wire::U64::new(0);
        let latest = dispatched(&successful(&mut bridge, json!({"op":"quote","token":token,"binding":binding,"request":latest_intent.request})));
        let old = frame(&earlier, wire::ServerReply::Quote { intent: intent() }, None);
        assert_eq!(call(&mut bridge, json!({"op":"receive","token":token,"frame":old}))["ok"], false);
        assert_eq!(call(&mut bridge, json!({"op":"reserve","token":token,"binding":binding,"intent":intent()}))["ok"], false);
        let raw = frame(&latest, wire::ServerReply::Quote { intent: latest_intent.clone() }, None);
        successful(&mut bridge, json!({"op":"receive","token":token,"frame":raw}));
        assert_eq!(call(&mut bridge, json!({"op":"reserve","token":token,"binding":binding,"intent":intent()}))["ok"], false);
        let reserved = successful(&mut bridge, json!({"op":"reserve","token":token,"binding":binding,"intent":latest_intent}));
        assert_eq!(reserved["operation"]["intent"]["request"]["itemIndex"], "0");
    }

    #[test]
    fn terminal_rejection_requires_complete_witness_in_both_orders() {
        for snapshot_first in [false, true] {
            let (mut bridge, token, binding) = ready();
            let original = reserve(&mut bridge, &token, &binding);
            let request = enter(&mut bridge, &token, &binding, &original);
            let mut rejected = receipt(&request, 2, 11);
            rejected.entry.outcome = wire::Outcome::Rejected { request: intent().request,
                reason: wire::Rejection::InsufficientCurrency };
            let raw = frame(&request, wire::ServerReply::Failure { state: wire::FailureState::PostCommit, receipt: Some(rejected) }, None);
            let last = if snapshot_first {
                applied(&mut bridge, &token, &binding, 11, true);
                successful(&mut bridge, json!({"op":"receive","token":token,"frame":raw}))
            } else {
                assert_eq!(successful(&mut bridge, json!({"op":"receive","token":token,"frame":raw}))["observation"]["kind"], "pending");
                applied(&mut bridge, &token, &binding, 11, true)
            };
            assert_eq!(last["observation"]["kind"], "settled");
            assert_eq!(last["observation"]["settlement"]["operation"], original);
            assert_eq!(last["observation"]["settlement"]["result"]["kind"], "rejected");
            assert_eq!(status(&mut bridge)["pending"], Value::Null);
        }
    }

    #[test]
    fn delayed_original_purchase_control_cannot_clear_successor_after_query_settlement() {
        let (mut bridge, token, binding) = ready();
        let original = reserve(&mut bridge, &token, &binding);
        let purchase = enter(&mut bridge, &token, &binding, &original);
        successful(&mut bridge, json!({"op":"unknown","token":token,"binding":binding,"operation":original}));
        let query = dispatched(&successful(&mut bridge, json!({"op":"query","token":token,"binding":binding})));
        let recovered = frame(&query, wire::ServerReply::Recovery { receipt: Some(receipt(&query, 2, 11)) }, None);
        successful(&mut bridge, json!({"op":"receive","token":token,"frame":recovered}));
        applied(&mut bridge, &token, &binding, 11, true);
        let successor = reserve(&mut bridge, &token, &binding);
        let late = frame(&purchase, wire::ServerReply::Purchase { receipt: receipt(&purchase, 2, 11), replayed: false }, None);
        assert_eq!(successful(&mut bridge, json!({"op":"receive","token":token,"frame":late}))["observation"]["kind"], "ignored");
        assert_eq!(status(&mut bridge)["pending"]["operation"], successor);
        assert_eq!(status(&mut bridge)["pending"]["phase"], "queued");
        assert_eq!(successor["sequence"], "2");
    }

    #[test]
    fn compact_request_decoder_preserves_all_legacy_operations_and_exact_field_failures() {
        let token = json!(opaque_fixture(1));
        let binding = json!({"actor":opaque_fixture(1),"producerScope":opaque_fixture(2),"beginId":"0"});
        let operation = wire::Operation { actor:opaque_fixture(1),request_scope:opaque_fixture(2),
            sequence:wire::U64::new(1),intent:intent() };
        let requests = [
            json!({"op":"openConnection"}),json!({"op":"status"}),
            json!({"op":"begin","token":token}),
            json!({"op":"receive","token":token,"frame":"original raw frame"}),
            json!({"op":"quote","token":token,"binding":binding,"request":intent().request}),
            json!({"op":"reserve","token":token,"binding":binding,"intent":intent()}),
            json!({"op":"enter","token":token,"binding":binding,"operation":operation}),
            json!({"op":"cancelUnsent","token":token,"binding":binding,"operation":operation}),
            json!({"op":"unknown","token":token,"binding":binding,"operation":operation}),
            json!({"op":"query","token":token,"binding":binding}),
            json!({"op":"applied","token":token,"binding":binding,"authority":producer(1,2,0),"complete":false}),
            json!({"op":"withdraw","token":token,"disconnect":false}),
        ];
        for value in requests {
            assert_eq!(Request::from_value(&value).unwrap(), serde_json::from_value::<Request>(value.clone()).unwrap());
            let names = Request::field_names(value["op"].as_str().unwrap()).unwrap();
            let tuple = Value::Array(names.iter().map(|name| value[*name].clone()).collect());
            assert_eq!(Request::from_value(&tuple).unwrap(),serde_json::from_value::<Request>(tuple.clone()).unwrap());
            let mut short = tuple.clone();short.as_array_mut().unwrap().pop();
            assert!(Request::from_value(&short).is_err());assert!(serde_json::from_value::<Request>(short).is_err());
            let mut long = tuple;long.as_array_mut().unwrap().push(Value::Null);
            assert!(Request::from_value(&long).is_err());assert!(serde_json::from_value::<Request>(long).is_err());
            let mut extra = value.clone();extra["extra"] = Value::Null;
            assert!(Request::from_value(&extra).is_err());assert!(serde_json::from_value::<Request>(extra).is_err());
            for field in value.as_object().unwrap().keys() {
                let mut missing = value.clone();missing.as_object_mut().unwrap().remove(field);
                assert!(Request::from_value(&missing).is_err(),"missing {field}: {value}");
                assert!(serde_json::from_value::<Request>(missing).is_err());
                let mut null = value.clone();null[field] = Value::Null;
                assert!(Request::from_value(&null).is_err(),"null {field}: {value}");
                assert!(serde_json::from_value::<Request>(null).is_err());
            }
            if value.get("binding").is_some() {
                let mut tuple = value.clone();tuple["binding"] = json!([binding["actor"],binding["producerScope"],binding["beginId"]]);
                assert_eq!(Request::from_value(&tuple).unwrap(),serde_json::from_value::<Request>(tuple.clone()).unwrap());
                for invalid in [json!([]),json!([binding["actor"],binding["producerScope"]]),
                    json!([binding["actor"],binding["producerScope"],binding["beginId"],null])] {
                    tuple["binding"] = invalid;
                    assert!(Request::from_value(&tuple).is_err());assert!(serde_json::from_value::<Request>(tuple.clone()).is_err());
                }
            }
        }
        for malformed in [json!([]),json!([1]),json!(["unknownOperation"]),json!({"op":{"status":null}}),json!({"op":"unknownOperation"})] {
            assert!(Request::from_value(&malformed).is_err());assert!(serde_json::from_value::<Request>(malformed).is_err());
        }
    }

    #[test]
    fn compact_request_nested_numeric_proofs_and_enum_spellings_match_existing_serde() {
        let token = json!(opaque_fixture(1));
        let binding = json!({"actor":opaque_fixture(1),"producerScope":opaque_fixture(2),"beginId":"18446744073709551615"});
        let original = json!({"op":"reserve","token":token,"binding":binding,"intent":intent()});
        for (pointer, samples) in [
            ("/intent/request/count",vec![json!(0),json!(65536),json!(-1),json!(3.0),json!("3"),Value::Null]),
            ("/intent/request/panelType",vec![json!(256),json!(-1),json!(0.0),json!("0"),Value::Null]),
            ("/intent/request/itemIndex",vec![json!(1),json!("01"),json!("18446744073709551616"),Value::Null]),
            ("/binding/beginId",vec![json!(1),json!("01"),json!("18446744073709551616"),Value::Null]),
            ("/intent/serviceCatalogProof",vec![json!(0),json!("00"),json!("0000000000000000000000000000000000000000000000000000000000000000"),Value::Null]),
            ("/intent/currency",vec![json!({"pearls":null}),json!({"pearls":{}}),json!({"pearls":false}),json!({"pearls":0}),json!({"pearls":""}),json!({"pearls":[]}),json!({"pearls":{"x":1}}),json!({"pearls":null,"gold":null}),json!("unknown"),Value::Null]),
            ("/intent/source",vec![json!({"used":null}),json!({"used":{}}),json!({"used":true}),json!({"used":1}),json!({"used":""}),json!({"used":[]}),json!({"used":{"x":1}}),json!("unknown"),Value::Null]),
        ] {
            for replacement in samples {
                let mut value = original.clone();*value.pointer_mut(pointer).unwrap() = replacement;
                let compact = Request::from_value(&value);let legacy = serde_json::from_value::<Request>(value.clone());
                assert_eq!(compact.is_ok(),legacy.is_ok(),"different schema for {pointer}: {value}");
                if let (Ok(compact),Ok(legacy)) = (compact,legacy) {assert_eq!(compact,legacy);}
            }
        }
        for count in [1,65535] {
            let mut value = original.clone();value["intent"]["request"]["count"] = json!(count);
            assert_eq!(Request::from_value(&value).unwrap(),serde_json::from_value::<Request>(value).unwrap());
        }
    }

    #[test]
    fn reused_raw_frame_walker_matches_original_limits_duplicates_and_legacy_shapes() {
        let operation = wire::Operation { actor:opaque_fixture(1),request_scope:opaque_fixture(2),
            sequence:wire::U64::new(u64::MAX),intent:intent() };
        let request = wire::ClientRequest::new(wire::U64::new(u64::MAX),wire::Action::Purchase { operation }).unwrap();
        let authority = producer(1,2,u64::MAX-1);
        let snapshot = json!({"fixture":{"text":"中文\\\"\n😀","wide":u64::MAX,"signed":i64::MIN},"array":[null,true,0]});
        let replies = [
            wire::ServerReply::Producer { producer:authority.clone() },
            wire::ServerReply::Quote { intent:intent() },
            wire::ServerReply::Recovery { receipt:None },
            wire::ServerReply::Recovery { receipt:Some(receipt(&request,2,u64::MAX-1)) },
            wire::ServerReply::Purchase { receipt:receipt(&request,2,u64::MAX-1),replayed:false },
            wire::ServerReply::Failure { state:wire::FailureState::BeforeExecution,receipt:None },
            wire::ServerReply::Failure { state:wire::FailureState::Unknown,receipt:None },
            wire::ServerReply::Failure { state:wire::FailureState::PostCommit,receipt:Some(receipt(&request,2,u64::MAX-1)) },
        ];
        let compare = |raw: &str| {
            let compact = decode_server_frame(raw);
            let original = wire::parse_server_frame(raw);
            assert_eq!(compact.is_ok(),original.is_ok(),"different raw acceptance ({} bytes)",raw.len());
            if let (Ok(compact),Ok(original)) = (compact,original) { assert_eq!(compact,original); }
        };
        for reply in replies {
            let original = wire::ServerFrame::new(request.request_id,reply,Some(snapshot.clone()),Some(authority.clone())).unwrap();
            let raw = wire::encode_server_frame(&original).unwrap();
            assert_eq!(decode_server_frame(&raw).unwrap(),original);compare(&raw);
            let value:Value = serde_json::from_str(&raw).unwrap();
            let tuple = json!([value["type"],value["protocolVersion"],value["requestId"],value["reply"],value["snapshot"],value["authority"]]);
            assert_eq!(decode_server_frame(&tuple.to_string()).unwrap(),original);compare(&tuple.to_string());
        }
        let base = wire::ServerFrame::new(request.request_id,
            wire::ServerReply::Quote { intent:intent() },Some(snapshot),Some(authority)).unwrap();
        let raw = wire::encode_server_frame(&base).unwrap();
        let mut value:Value = serde_json::from_str(&raw).unwrap();
        for spelling in [json!({"pearls":null}),json!({"pearls":{}})] {
            value["reply"]["intent"]["currency"] = spelling;
            assert_eq!(decode_server_frame(&value.to_string()).unwrap(),base);compare(&value.to_string());
        }
        value["reply"] = json!(["quote",value["reply"]["intent"]]);
        assert_eq!(decode_server_frame(&value.to_string()).unwrap(),base);compare(&value.to_string());
        value = serde_json::from_str(&raw).unwrap();value["type"] = json!({"npcPurchaseOwner":null});
        assert_eq!(decode_server_frame(&value.to_string()).unwrap(),base);compare(&value.to_string());
        value["type"] = json!({"npcPurchaseOwner":{}});
        assert!(decode_server_frame(&value.to_string()).is_err());compare(&value.to_string());
        let boundary = format!("{raw}{}"," ".repeat(wire::MAX_SERVER_FRAME_BYTES-raw.len()));
        assert_eq!(boundary.len(),wire::MAX_SERVER_FRAME_BYTES);
        assert_eq!(decode_server_frame(&boundary).unwrap(),base);compare(&boundary);
        let too_long = format!("{boundary} ");
        assert!(decode_server_frame(&too_long).is_err());compare(&too_long);
        for invalid in [String::new()," ".to_owned(),format!("{raw} null"),
            raw.replacen("\"requestId\":","\"requestId\":\"1\",\"requestId\":",1),
            raw.replacen("\"requestId\":","\"requestId\":\"1\",\"\\u0072equestId\":",1),
            raw.replacen("\"text\":","\"x\":1,\"x\":2,\"text\":",1),
            raw.replacen("\"text\":","\"x\":1,\"\\u0078\":2,\"text\":",1)] {
            assert!(decode_server_frame(&invalid).is_err());compare(&invalid);
        }
        for depth in [16,160] {
            let nested = format!("{}null{}","[".repeat(depth),"]".repeat(depth));
            let deep_raw = format!(r#"{{"type":"npcPurchaseOwner","protocolVersion":1,"requestId":"1","reply":{{"kind":"recovery","receipt":null}},"snapshot":{nested},"authority":{{"actor":{},"producerScope":{},"serverRevision":"0"}}}}"#,
                serde_json::to_string(&opaque_fixture(1)).unwrap(),serde_json::to_string(&opaque_fixture(2)).unwrap());
            assert_eq!(decode_server_frame(&deep_raw).is_ok(),depth==16);compare(&deep_raw);
        }
    }

    #[test]
    fn compact_bridge_outputs_preserve_original_body_and_complete_frame_carriers() {
        let mut bridge = NpcPurchaseReceiptBridge::new().unwrap();
        let opened = successful(&mut bridge,json!({"op":"openConnection"}));
        let token = opened["token"].clone();
        let typed_token:wire::Opaque32 = serde_json::from_value(token.clone()).unwrap();
        assert_eq!(token,serde_json::to_value(typed_token).unwrap());
        let sent = successful(&mut bridge,json!({"op":"begin","token":token}));
        let begin_request = dispatched(&sent);
        assert_eq!(sent["request"],serde_json::to_value(&begin_request).unwrap());
        assert_eq!(sent["body"],wire::encode_client_request(&begin_request).unwrap());
        let snapshot = json!({"completeFixture":{"text":"中文\\\"\n😀","wide":u64::MAX,"signed":i64::MIN},"array":[null,true,0]});
        let authority = producer(1,2,10);
        let begin_frame = wire::ServerFrame::new(begin_request.request_id,
            wire::ServerReply::Producer { producer:authority.clone() },Some(snapshot.clone()),Some(authority)).unwrap();
        let received = successful(&mut bridge,json!({"op":"receive","token":token,"frame":wire::encode_server_frame(&begin_frame).unwrap()}));
        assert_eq!(received["frame"],serde_json::to_value(&begin_frame).unwrap());
        assert_eq!(received["frame"]["snapshot"],snapshot);
        let binding = received["binding"].clone();
        let typed_binding:BindingStamp = serde_json::from_value(binding.clone()).unwrap();
        assert_eq!(binding,serde_json::to_value(&typed_binding).unwrap());
        assert_eq!(status(&mut bridge)["actors"][0]["appliedBaseline"],Value::Null);
        applied(&mut bridge,&token,&binding,10,true);
        let quoted = successful(&mut bridge,json!({"op":"quote","token":token,"binding":binding,"request":intent().request}));
        let quote_request = dispatched(&quoted);
        assert_eq!(quoted["request"],serde_json::to_value(&quote_request).unwrap());
        assert_eq!(quoted["body"],wire::encode_client_request(&quote_request).unwrap());
        let quote_frame = wire::ServerFrame::new(quote_request.request_id,wire::ServerReply::Quote { intent:intent() },None,None).unwrap();
        let received = successful(&mut bridge,json!({"op":"receive","token":token,"frame":wire::encode_server_frame(&quote_frame).unwrap()}));
        assert_eq!(received["intent"],serde_json::to_value(intent()).unwrap());
        assert_eq!(received["frame"],serde_json::to_value(&quote_frame).unwrap());
        assert_eq!(received["frame"]["snapshot"],Value::Null);
        assert_eq!(received["frame"]["authority"],Value::Null);
        let reserved = successful(&mut bridge,json!({"op":"reserve","token":token,"binding":binding,"intent":intent()}));
        let operation:wire::Operation = serde_json::from_value(reserved["operation"].clone()).unwrap();
        assert_eq!(reserved["operation"],serde_json::to_value(&operation).unwrap());
        assert_eq!(status(&mut bridge)["pending"]["operation"],reserved["operation"]);
        let entered = successful(&mut bridge,json!({"op":"enter","token":token,"binding":binding,"operation":operation}));
        let purchase_request = dispatched(&entered);
        assert_eq!(entered["request"],serde_json::to_value(&purchase_request).unwrap());
        assert_eq!(entered["body"],wire::encode_client_request(&purchase_request).unwrap());
        let mut committed = receipt(&purchase_request,2,u64::MAX-1);
        if let wire::Outcome::Committed { charged, .. } = &mut committed.entry.outcome { *charged = u32::MAX; }
        let purchase_frame = wire::ServerFrame::new(purchase_request.request_id,
            wire::ServerReply::Purchase { receipt:committed,replayed:false },Some(snapshot.clone()),Some(producer(1,2,u64::MAX-1))).unwrap();
        let received = successful(&mut bridge,json!({"op":"receive","token":token,"frame":wire::encode_server_frame(&purchase_frame).unwrap()}));
        assert_eq!(received["frame"],serde_json::to_value(&purchase_frame).unwrap());
        assert_eq!(received["frame"]["snapshot"],snapshot);
        assert_eq!(received["observation"]["kind"],"pending");
        let settled = applied(&mut bridge,&token,&binding,u64::MAX-1,true);
        assert_eq!(settled["observation"]["settlement"]["operation"],serde_json::to_value(operation).unwrap());
        let result = json!({"kind":"committed","serverRevision":wire::U64::new(u64::MAX-1),
            "currency":wire::Currency::Pearls,"source":wire::Source::Used,"charged":u32::MAX,
            "admittedCount":2,"incomingUniqueId":wire::U64::new(0)});
        assert_eq!(settled["observation"]["settlement"]["result"],result);
        let final_status = status(&mut bridge);
        assert_eq!(final_status["binding"],serde_json::to_value(typed_binding).unwrap());
        assert_eq!(final_status["actors"][0]["actor"],serde_json::to_value(opaque_fixture(1)).unwrap());
        assert_eq!(final_status["actors"][0]["lastSequence"],serde_json::to_value(wire::U64::new(1)).unwrap());
        assert_eq!(final_status["actors"][0]["appliedBaseline"],serde_json::to_value(wire::U64::new(u64::MAX-1)).unwrap());
        assert_eq!(final_status["pending"],Value::Null);
    }

}

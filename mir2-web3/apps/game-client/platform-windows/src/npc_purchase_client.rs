//! Permanent, single-writer native custody for durable NPC purchases.
//!
//! Keep this client outside physical WebSocket lifetimes. Only the authenticated
//! socket reader may provide raw frames, with its original connection token.
//! Receiving a paired snapshot is not application: the main thread separately
//! supplies `applied` after the entire authoritative economy bundle is applied.
//! This module performs no I/O, retry, automatic query or snapshot application.

use std::collections::BTreeMap;
pub use mir2_client_core::npc_purchase_host::{ConnectionToken, PurchaseBinding};
use mir2_client_core::npc_purchase_host::{BeginTicket, NpcPurchaseReceiptHost};
use mir2_client_core::npc_purchase_receipt as core;
use mir2_client_wire as wire;

const MAX_CONTROLS: usize = 64;
type Result<T> = std::result::Result<T, &'static str>;

/// Encoded before economic entry and returned once. The caller sends `body`
/// only on the physical connection identified by `token`.
#[derive(Debug)]
pub struct Dispatch {
    pub token: ConnectionToken,
    pub request: wire::ClientRequest,
    pub body: String,
}

/// Strictly correlated full frame; its snapshot is still unapplied.
#[derive(Debug)]
pub struct Received {
    pub frame: wire::ServerFrame,
    pub binding: Option<PurchaseBinding>,
    pub observation: core::Observation,
}

#[derive(Debug)]
struct Control {
    token: ConnectionToken,
    request: wire::ClientRequest,
    ticket: Option<BeginTicket>,
    binding: Option<PurchaseBinding>,
}

#[derive(Debug)]
struct Reservation {
    token: ConnectionToken,
    binding: PurchaseBinding,
    operation: wire::Operation,
}

#[derive(Debug)]
pub struct NativeNpcPurchaseClient {
    host: NpcPurchaseReceiptHost,
    last_control: u64,
    latest_begin: Option<u64>,
    latest_quote: Option<u64>,
    quote: Option<(PurchaseBinding, wire::Intent)>,
    controls: BTreeMap<u64, Control>,
    reservations: BTreeMap<core::ActorKey, Reservation>,
}

impl NativeNpcPurchaseClient {
    pub fn new() -> Option<Self> {
        Some(Self { host: NpcPurchaseReceiptHost::new()?, last_control: 0,
            latest_begin: None, latest_quote: None, quote: None,
            controls: BTreeMap::new(), reservations: BTreeMap::new() })
    }

    pub fn connection(&self) -> Option<ConnectionToken> { self.host.connection() }
    pub fn current_binding(&self) -> Option<PurchaseBinding> { self.host.current_binding() }
    pub fn pending_current(&self) -> Option<core::PendingPurchase> { self.host.pending_current() }

    /// Withdraw before the checked allocator, including its exhaustion path.
    pub fn open_connection(&mut self) -> Result<ConnectionToken> {
        if let Some(token) = self.connection() { self.withdraw_current(token); }
        self.host.open_connection().ok_or("Purchase connection IDs exhausted")
    }

    pub fn withdraw_current(&mut self, token: ConnectionToken) -> bool {
        if !self.host.withdraw_current(token) { return false; }
        self.latest_begin = None;
        self.latest_quote = None;
        self.quote = None;
        true
    }

    pub fn disconnect(&mut self, token: ConnectionToken) -> bool {
        if !self.withdraw_current(token) { return false; }
        self.host.withdraw_connection(token)
    }

    pub fn begin(&mut self, token: ConnectionToken) -> Result<Dispatch> {
        self.connection_for(token)?;
        self.withdraw_current(token);
        let (request, body) = self.allocate(wire::Action::Begin)?;
        let ticket = self.host.request_begin(token).ok_or("Purchase Begin unavailable")?;
        self.latest_begin = Some(request.request_id.get());
        Ok(self.dispatch(token, request, body, Some(ticket), None))
    }

    pub fn quote(&mut self, token: ConnectionToken, binding: PurchaseBinding,
        request: wire::PurchaseRequest) -> Result<Dispatch>
    {
        self.binding_for(token, binding)?;
        let (request, body) = self.allocate(wire::Action::Quote { request })?;
        self.quote = None;
        self.latest_quote = Some(request.request_id.get());
        Ok(self.dispatch(token, request, body, None, Some(binding)))
    }

    pub fn reserve(&mut self, token: ConnectionToken, binding: PurchaseBinding,
        intent: wire::Intent) -> Result<wire::Operation>
    {
        self.binding_for(token, binding)?;
        intent.validate().map_err(|_| "Invalid purchase intent")?;
        if !self.quote.as_ref().is_some_and(|(saved, quoted)| *saved == binding && quoted == &intent) {
            return Err("Exact current server quote required");
        }
        let purchase = core_intent(&intent);
        let key = self.host.reserve(binding, purchase).ok_or("Purchase reservation unavailable")?;
        let operation = operation(key, purchase);
        self.reservations.insert(key.actor, Reservation { token, binding, operation: operation.clone() });
        self.quote = None;
        self.latest_quote = None;
        Ok(operation)
    }

    /// Call immediately before one physical send. Encoding/capacity failures
    /// leave the queued operation unsent; successful entry cannot be repeated.
    pub fn enter(&mut self, token: ConnectionToken, binding: PurchaseBinding,
        operation: &wire::Operation) -> Result<Dispatch>
    {
        self.binding_for(token, binding)?;
        let key = self.reservation_matches(token, binding, operation)?;
        let (request, body) = self.allocate(wire::Action::Purchase { operation: operation.clone() })?;
        if !self.host.begin_entry(binding, key, core_intent(&operation.intent)) {
            return Err("Purchase entry refused");
        }
        Ok(self.dispatch(token, request, body, None, Some(binding)))
    }

    /// Read-only recovery always carries the original scope/sequence/full intent.
    pub fn query(&mut self, token: ConnectionToken, binding: PurchaseBinding) -> Result<Dispatch> {
        self.binding_for(token, binding)?;
        let query = self.host.recovery_query(binding).ok_or("Entered original operation required")?;
        let (request, body) = self.allocate(wire::Action::Query { operation: operation(query.key, query.intent) })?;
        Ok(self.dispatch(token, request, body, None, Some(binding)))
    }

    /// Definite pre-entry feedback may arrive while offline or another actor is
    /// selected, but must retain the reservation's original token and binding.
    pub fn cancel_unsent(&mut self, token: ConnectionToken, binding: PurchaseBinding,
        operation: &wire::Operation) -> bool
    {
        let Ok(key) = self.reservation_matches(token, binding, operation) else { return false; };
        if !self.host.cancel_definitely_unsent(key, core_intent(&operation.intent)) { return false; }
        self.reservations.remove(&key.actor);
        true
    }

    pub fn unknown(&mut self, token: ConnectionToken, binding: PurchaseBinding,
        operation: &wire::Operation) -> bool
    {
        let Ok(key) = self.reservation_matches(token, binding, operation) else { return false; };
        self.host.mark_unknown(key)
    }

    pub fn applied(&mut self, binding: PurchaseBinding, witness: core::SnapshotWitness) -> core::Observation {
        let observed = self.host.observe_applied_snapshot(binding, witness);
        self.cleanup_settled(observed);
        observed
    }

    pub fn receive(&mut self, token: ConnectionToken, raw: &str) -> Result<Received> {
        self.connection_for(token)?;
        let frame = wire::parse_server_frame(raw).map_err(|_| "Invalid purchase server frame")?;
        let id = frame.request_id.get();
        let saved = self.controls.get(&id).ok_or("Purchase control custody missing")?;
        if saved.token != token { return Err("Purchase reply connection mismatch"); }
        frame.validate_for_request(&saved.request).map_err(|_| "Purchase reply correlation mismatch")?;
        if let Some(binding) = saved.binding {
            self.binding_for(token, binding)?;
            if frame.authority.as_ref().is_some_and(|authority|
                actor(authority.actor) != binding.actor() || scope(authority.producer_scope) != binding.producer_scope()) {
                return Err("Purchase reply authority mismatch");
            }
            if frame.reply.receipt().is_some_and(|receipt| scope(receipt.producer_scope) != binding.producer_scope()) {
                return Err("Purchase receipt producer mismatch");
            }
        }
        if saved.ticket.is_some() && self.latest_begin != Some(id) {
            return Err("Superseded purchase Begin response");
        }
        if matches!(&frame.reply, wire::ServerReply::Quote { .. }) && self.latest_quote != Some(id) {
            return Err("Superseded purchase quote response");
        }

        // Validate every correlation/pair before consuming control custody.
        // Binding is also checked by Core (including retired producer A/B/A).
        if let wire::ServerReply::Producer { producer } = &frame.reply {
            if frame.authority.as_ref() != Some(producer) || frame.snapshot.is_none() {
                return Err("Accepted Begin requires paired producer snapshot");
            }
            let ticket = saved.ticket.ok_or("Begin ticket custody missing")?;
            self.host.bind(ticket, actor(producer.actor), scope(producer.producer_scope))
                .map_err(|_| "Accepted Begin binding refused")?;
        }

        let saved = self.controls.remove(&id).expect("checked control custody");
        let mut observation = core::Observation::Ignored;
        match &frame.reply {
            wire::ServerReply::Producer { .. } => {
                self.latest_begin = None;
                self.quote = None;
                self.latest_quote = None;
            }
            wire::ServerReply::Quote { intent } => {
                let binding = saved.binding.expect("correlated Quote binding");
                self.quote = Some((binding, intent.clone()));
            }
            _ => {
                if let Some(receipt) = frame.reply.receipt() {
                    let binding = saved.binding.expect("correlated receipt binding");
                    observation = self.host.observe_receipt(binding, core_receipt(receipt));
                    self.cleanup_settled(observation);
                } else if let Some(operation) = saved.request.action.operation() {
                    if self.host.mark_unknown(operation_key(operation)) { observation = core::Observation::Pending; }
                }
            }
        }
        Ok(Received { frame, binding: self.current_binding(), observation })
    }

    fn connection_for(&self, token: ConnectionToken) -> Result<()> {
        if self.connection() != Some(token) { return Err("Stale purchase connection"); }
        Ok(())
    }

    fn binding_for(&self, token: ConnectionToken, binding: PurchaseBinding) -> Result<()> {
        self.connection_for(token)?;
        if binding.connection() != token || self.current_binding() != Some(binding) {
            return Err("Stale purchase binding");
        }
        Ok(())
    }

    fn allocate(&mut self, action: wire::Action) -> Result<(wire::ClientRequest, String)> {
        if self.controls.len() >= MAX_CONTROLS { return Err("Purchase control capacity reached"); }
        let id = self.last_control.checked_add(1).ok_or("Purchase control IDs exhausted")?;
        let request = wire::ClientRequest::new(wire::U64::new(id), action).map_err(|_| "Invalid purchase wire request")?;
        let body = wire::encode_client_request(&request).map_err(|_| "Invalid purchase wire request")?;
        self.last_control = id;
        Ok((request, body))
    }

    fn dispatch(&mut self, token: ConnectionToken, request: wire::ClientRequest,
        body: String, ticket: Option<BeginTicket>, binding: Option<PurchaseBinding>) -> Dispatch
    {
        self.controls.insert(request.request_id.get(), Control { token, request: request.clone(), ticket, binding });
        Dispatch { token, request, body }
    }

    fn reservation_matches(&self, token: ConnectionToken, binding: PurchaseBinding,
        operation: &wire::Operation) -> Result<core::OperationKey>
    {
        operation.validate().map_err(|_| "Invalid purchase operation")?;
        let key = operation_key(operation);
        let saved = self.reservations.get(&key.actor).ok_or("Original purchase custody missing")?;
        if saved.token != token || saved.binding != binding || &saved.operation != operation {
            return Err("Original purchase custody mismatch");
        }
        Ok(key)
    }

    fn cleanup_settled(&mut self, observed: core::Observation) {
        if let core::Observation::Settled(value) = observed { self.reservations.remove(&value.key.actor); }
    }
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
        value.sequence.get()).expect("validated nonzero sequence") }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Pure adapter/schema specimens. No fixture is network authentication or
    // an actual complete application of production economic models.
    fn fixture(value: u8) -> wire::Opaque32 { wire::Opaque32::from_bytes([value; 32]).unwrap() }
    fn producer(a: u8, p: u8, revision: u64) -> wire::Producer {
        wire::Producer { actor: fixture(a), producer_scope: fixture(p), server_revision: wire::U64::new(revision) }
    }
    fn intent() -> wire::Intent {
        wire::Intent { request: wire::PurchaseRequest::new(wire::U64::new(u64::MAX), 3, 0).unwrap(),
            currency: wire::Currency::Pearls, source: wire::Source::Used, service_catalog_proof: fixture(7) }
    }
    fn frame(request: &wire::ClientRequest, reply: wire::ServerReply, authority: Option<wire::Producer>) -> String {
        let snapshot = authority.as_ref().map(|_| json!({"pureAdapterFixture":true}));
        wire::encode_server_frame(&wire::ServerFrame::new(request.request_id, reply, snapshot, authority).unwrap()).unwrap()
    }
    fn accept(client: &mut NativeNpcPurchaseClient, token: ConnectionToken, a: u8, p: u8) -> PurchaseBinding {
        let dispatched = client.begin(token).unwrap();
        assert_eq!(dispatched.token, token);
        assert_eq!(wire::parse_client_request(&dispatched.body).unwrap(), dispatched.request);
        let producer = producer(a, p, 10);
        let raw = frame(&dispatched.request, wire::ServerReply::Producer { producer: producer.clone() }, Some(producer));
        let received = client.receive(token, &raw).unwrap();
        assert_eq!(received.observation, core::Observation::Ignored);
        received.binding.unwrap()
    }
    fn witness(binding: PurchaseBinding, revision: u64) -> core::SnapshotWitness {
        core::SnapshotWitness { actor: binding.actor(), producer_scope: binding.producer_scope(),
            server_revision: revision, complete: true }
    }
    fn ready() -> (NativeNpcPurchaseClient, ConnectionToken, PurchaseBinding) {
        let mut client = NativeNpcPurchaseClient::new().unwrap();
        let token = client.open_connection().unwrap();
        let binding = accept(&mut client, token, 1, 2);
        assert_eq!(client.applied(binding, witness(binding, 10)), core::Observation::Pending);
        (client, token, binding)
    }
    fn quoted(client: &mut NativeNpcPurchaseClient, token: ConnectionToken, binding: PurchaseBinding,
        intent: &wire::Intent)
    {
        let dispatched = client.quote(token, binding, intent.request.clone()).unwrap();
        let raw = frame(&dispatched.request, wire::ServerReply::Quote { intent: intent.clone() }, None);
        assert_eq!(client.receive(token, &raw).unwrap().binding, Some(binding));
    }
    fn reserve(client: &mut NativeNpcPurchaseClient, token: ConnectionToken, binding: PurchaseBinding) -> wire::Operation {
        quoted(client, token, binding, &intent());
        client.reserve(token, binding, intent()).unwrap()
    }
    fn receipt(operation: &wire::Operation, p: u8, revision: u64) -> wire::Receipt {
        wire::Receipt { producer_scope: fixture(p), entry: wire::ReceiptEntry { operation: operation.clone(),
            server_revision: wire::U64::new(revision), outcome: wire::Outcome::Committed {
                request: operation.intent.request.clone(), currency: operation.intent.currency,
                source: operation.intent.source, charged: 9, admitted_count: 2, incoming_unique_id: wire::U64::new(0) } } }
    }
    fn purchase_frame(dispatched: &Dispatch, p: u8, revision: u64) -> String {
        frame(&dispatched.request, wire::ServerReply::Purchase {
            receipt: receipt(dispatched.request.action.operation().unwrap(), p, revision), replayed: false }, None)
    }
    fn settled(observation: core::Observation, operation: &wire::Operation) {
        let core::Observation::Settled(value) = observation else { panic!("expected settlement: {observation:?}"); };
        assert_eq!(value.key, operation_key(operation));
        assert_eq!(value.intent, core_intent(&operation.intent));
    }

    #[test]
    fn native_begin_pair_is_not_complete_application_and_exact_quote_is_required() {
        let mut client = NativeNpcPurchaseClient::new().unwrap();
        let token = client.open_connection().unwrap();
        let binding = accept(&mut client, token, 1, 2);
        assert!(client.reserve(token, binding, intent()).is_err());
        quoted(&mut client, token, binding, &intent());
        assert!(client.reserve(token, binding, intent()).is_err());
        assert_eq!(client.applied(binding, core::SnapshotWitness { complete: false, ..witness(binding, 10) }), core::Observation::Ignored);
        assert!(client.reserve(token, binding, intent()).is_err());
        client.applied(binding, witness(binding, 10));
        let mut wrong = intent(); wrong.service_catalog_proof = fixture(8);
        assert!(client.reserve(token, binding, wrong).is_err());
        let operation = client.reserve(token, binding, intent()).unwrap();
        assert_eq!(operation.intent.request.item_index.get(), u64::MAX);
        assert_eq!(operation.sequence.get(), 1);
        assert!(client.reserve(token, binding, intent()).is_err());
    }

    #[test]
    fn native_receipt_and_complete_witness_settle_in_both_orders_preserving_uid_zero() {
        for snapshot_first in [false, true] {
            let (mut client, token, binding) = ready();
            let operation = reserve(&mut client, token, binding);
            let dispatched = client.enter(token, binding, &operation).unwrap();
            assert_eq!(wire::parse_client_request(&dispatched.body).unwrap(), dispatched.request);
            assert!(client.enter(token, binding, &operation).is_err());
            let raw = purchase_frame(&dispatched, 2, 11);
            let observation = if snapshot_first {
                assert_eq!(client.applied(binding, witness(binding, 11)), core::Observation::Pending);
                client.receive(token, &raw).unwrap().observation
            } else {
                let received = client.receive(token, &raw).unwrap();
                assert_eq!(received.observation, core::Observation::Pending);
                assert_eq!(client.pending_current().unwrap().phase, core::PurchasePhase::AwaitingSnapshot);
                assert_eq!(client.applied(binding, core::SnapshotWitness { complete: false, ..witness(binding, 11) }), core::Observation::Ignored);
                client.applied(binding, witness(binding, 11))
            };
            settled(observation, &operation);
            assert!(matches!(observation, core::Observation::Settled(core::PurchaseSettlement {
                result: core::EconomicResult::Committed { incoming_uid: 0, .. }, .. })));
            assert_eq!(client.pending_current(), None);
            assert!(client.receive(token, &raw).is_err());
        }
    }

    #[test]
    fn native_raw_invalid_or_wrong_operation_reply_never_consumes_saved_control() {
        let (mut client, token, binding) = ready();
        let operation = reserve(&mut client, token, binding);
        let dispatched = client.enter(token, binding, &operation).unwrap();
        let raw = purchase_frame(&dispatched, 2, 11);
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let mut specimens = vec![raw.replacen("{", "{\"requestId\":\"1\",", 1),
            raw.replacen("{", "{\"request\\u0049d\":\"1\",", 1),
            raw.replacen("{", "{\"unknown\":false,", 1)];
        for (path, replacement) in [
            ("/requestId", json!(999)), ("/requestId", json!("999")),
            ("/reply/receipt/entry/operation/actor", json!(fixture(9))),
            ("/reply/receipt/entry/operation/requestScope", json!(fixture(8))),
            ("/reply/receipt/entry/operation/sequence", json!("0")),
            ("/reply/receipt/entry/operation/sequence", json!("2")),
            ("/reply/receipt/entry/operation/intent/serviceCatalogProof", json!(fixture(8))),
            ("/reply/receipt/entry/operation/intent/request/itemIndex", json!("01")),
            ("/reply/receipt/entry/outcome/committed/admittedCount", json!(0)),
            ("/reply/receipt/entry/serverRevision", json!(u64::MAX.to_string())),
            ("/reply/receipt/producerScope", json!(fixture(8))),
        ] {
            let mut specimen = value.clone();
            // Outcome uses an externally tagged enum; every path is checked.
            *specimen.pointer_mut(path).unwrap() = replacement;
            specimens.push(specimen.to_string());
        }
        for specimen in specimens {
            assert!(client.receive(token, &specimen).is_err(), "accepted {specimen}");
            assert_eq!(client.controls.len(), 1);
            assert_eq!(client.pending_current().unwrap().phase, core::PurchasePhase::Entered);
        }
        assert_eq!(client.receive(token, &raw).unwrap().observation, core::Observation::Pending);
        settled(client.applied(binding, witness(binding, 11)), &operation);
    }

    #[test]
    fn native_begin_requires_same_turn_pair_and_superseded_begin_cannot_select() {
        let (mut client, token, _) = ready();
        let old = client.begin(token).unwrap();
        let newest = client.begin(token).unwrap();
        let producer = producer(9, 8, 10);
        let delayed = frame(&old.request, wire::ServerReply::Producer { producer: producer.clone() }, Some(producer.clone()));
        assert!(client.receive(token, &delayed).is_err());
        let missing = frame(&newest.request, wire::ServerReply::Producer { producer: producer.clone() }, None);
        assert!(client.receive(token, &missing).is_err());
        assert_eq!(client.current_binding(), None);
        assert_eq!(client.controls.len(), 2);
        let valid = frame(&newest.request, wire::ServerReply::Producer { producer: producer.clone() }, Some(producer));
        assert_eq!(client.receive(token, &valid).unwrap().binding.unwrap().actor(), actor(fixture(9)));
        assert_eq!(client.controls.len(), 1);
    }

    #[test]
    fn native_quote_supersession_and_new_begin_revoke_exact_quote() {
        let (mut client, token, binding) = ready();
        let old = client.quote(token, binding, intent().request).unwrap();
        let latest = client.quote(token, binding, intent().request).unwrap();
        let reply = wire::ServerReply::Quote { intent: intent() };
        assert!(client.receive(token, &frame(&old.request, reply.clone(), None)).is_err());
        assert!(client.reserve(token, binding, intent()).is_err());
        client.receive(token, &frame(&latest.request, reply, None)).unwrap();
        client.begin(token).unwrap();
        assert_eq!(client.current_binding(), None);
        assert!(client.reserve(token, binding, intent()).is_err());
        assert!(client.quote.is_none());
    }

    #[test]
    fn native_reconnect_same_or_new_producer_queries_original_operation_only() {
        for next_producer in [2, 3] {
            let (mut client, token, binding) = ready();
            let operation = reserve(&mut client, token, binding);
            let dispatched = client.enter(token, binding, &operation).unwrap();
            assert!(client.unknown(token, binding, &operation));
            assert!(client.disconnect(token));
            let next = client.open_connection().unwrap();
            let resumed = accept(&mut client, next, 1, next_producer);
            assert_eq!(client.pending_current().unwrap().key, operation_key(&operation));
            assert!(client.receive(token, &purchase_frame(&dispatched, 2, 11)).is_err());
            assert!(client.receive(next, &purchase_frame(&dispatched, 2, 11)).is_err());
            assert!(client.enter(next, resumed, &operation).is_err());
            assert!(!client.cancel_unsent(next, resumed, &operation));
            assert!(client.unknown(token, binding, &operation));
            let query = client.query(next, resumed).unwrap();
            assert!(!query.request.action.is_mutation());
            assert_eq!(query.request.action.operation(), Some(&operation));
            let absent = frame(&query.request, wire::ServerReply::Recovery { receipt: None }, None);
            assert_eq!(client.receive(next, &absent).unwrap().observation, core::Observation::Pending);
            let query = client.query(next, resumed).unwrap();
            let recovered = frame(&query.request, wire::ServerReply::Recovery {
                receipt: Some(receipt(&operation, next_producer, 11)) }, None);
            assert_eq!(client.receive(next, &recovered).unwrap().observation, core::Observation::Pending);
            settled(client.applied(resumed, witness(resumed, 11)), &operation);
            let successor = reserve(&mut client, next, resumed);
            assert_eq!(successor.sequence.get(), 2);
            assert_eq!(successor.request_scope, fixture(next_producer));
        }
    }

    #[test]
    fn native_foreign_actor_selection_retains_original_and_retired_producer_is_refused() {
        let (mut client, token, original) = ready();
        let operation = reserve(&mut client, token, original);
        let sent = client.enter(token, original, &operation).unwrap();
        let foreign = accept(&mut client, token, 9, 8);
        client.applied(foreign, witness(foreign, 10));
        let foreign_operation = reserve(&mut client, token, foreign);
        assert!(client.receive(token, &purchase_frame(&sent, 2, 11)).is_err());
        assert_eq!(client.host.pending(original.actor()).unwrap().key, operation_key(&operation));
        assert_eq!(client.applied(original, witness(original, 11)), core::Observation::Ignored);
        let resumed = accept(&mut client, token, 1, 3);
        assert_eq!(client.query(token, resumed).unwrap().request.action.operation(), Some(&operation));
        assert_eq!(client.host.pending(foreign.actor()).unwrap().key, operation_key(&foreign_operation));
        let begin = client.begin(token).unwrap();
        let producer = producer(1, 2, 11);
        let raw = frame(&begin.request, wire::ServerReply::Producer { producer: producer.clone() }, Some(producer));
        assert!(client.receive(token, &raw).is_err());
        assert_eq!(client.current_binding(), None);
        assert!(client.controls.contains_key(&begin.request.request_id.get()));
    }

    #[test]
    fn native_unsent_feedback_requires_original_custody_and_entered_never_cancels() {
        let (mut client, token, binding) = ready();
        let operation = reserve(&mut client, token, binding);
        client.withdraw_current(token);
        let next_binding = accept(&mut client, token, 1, 2);
        assert!(!client.cancel_unsent(token, next_binding, &operation));
        let mut malformed = operation.clone(); malformed.sequence = wire::U64::new(0);
        assert!(!client.cancel_unsent(token, binding, &malformed));
        assert!(client.cancel_unsent(token, binding, &operation));
        client.applied(next_binding, witness(next_binding, 10));
        let next_operation = reserve(&mut client, token, next_binding);
        assert_eq!(next_operation.sequence.get(), 2);
        client.enter(token, next_binding, &next_operation).unwrap();
        assert!(!client.cancel_unsent(token, next_binding, &next_operation));
        client.disconnect(token);
        assert!(client.unknown(token, next_binding, &next_operation));
        assert!(!client.cancel_unsent(token, next_binding, &next_operation));
    }

    #[test]
    fn native_control_capacity_and_exhaustion_withdraw_before_refusal_without_eviction() {
        let (mut client, token, binding) = ready();
        let operation = reserve(&mut client, token, binding);
        client.enter(token, binding, &operation).unwrap();
        for _ in 1..MAX_CONTROLS { client.query(token, binding).unwrap(); }
        let ids: Vec<_> = client.controls.keys().copied().collect();
        assert!(client.begin(token).is_err());
        assert_eq!(client.current_binding(), None);
        assert_eq!(client.controls.keys().copied().collect::<Vec<_>>(), ids);
        assert_eq!(client.host.pending(binding.actor()).unwrap().key, operation_key(&operation));
        assert_eq!(client.host.pending(binding.actor()).unwrap().phase, core::PurchasePhase::Unknown);
        let (mut exhausted, token, _) = ready();
        exhausted.last_control = u64::MAX;
        assert!(exhausted.begin(token).is_err());
        assert_eq!(exhausted.current_binding(), None);
        assert_eq!(exhausted.last_control, u64::MAX);
        assert!(exhausted.controls.is_empty());
    }

    #[test]
    fn native_failure_unknown_and_before_execution_preserve_entered_barrier() {
        for state in [wire::FailureState::Unknown, wire::FailureState::BeforeExecution] {
            let (mut client, token, binding) = ready();
            let operation = reserve(&mut client, token, binding);
            let sent = client.enter(token, binding, &operation).unwrap();
            let raw = frame(&sent.request, wire::ServerReply::Failure { state, receipt: None }, None);
            assert_eq!(client.receive(token, &raw).unwrap().observation, core::Observation::Pending);
            assert_eq!(client.pending_current().unwrap().phase, core::PurchasePhase::Unknown);
            assert!(!client.cancel_unsent(token, binding, &operation));
            assert!(client.enter(token, binding, &operation).is_err());
            assert_eq!(client.query(token, binding).unwrap().request.action.operation(), Some(&operation));
        }
    }

    #[test]
    fn native_terminal_rejection_and_post_commit_receipt_still_need_complete_witness() {
        for rejected in [false, true] {
            let (mut client, token, binding) = ready();
            let operation = reserve(&mut client, token, binding);
            let sent = client.enter(token, binding, &operation).unwrap();
            let mut receipt = receipt(&operation, 2, 11);
            if rejected {
                receipt.entry.outcome = wire::Outcome::Rejected { request: operation.intent.request.clone(),
                    reason: wire::Rejection::InsufficientCurrency };
            }
            let raw = frame(&sent.request, wire::ServerReply::Failure {
                state: wire::FailureState::PostCommit, receipt: Some(receipt) }, None);
            assert_eq!(client.receive(token, &raw).unwrap().observation, core::Observation::Pending);
            assert_eq!(client.applied(binding, witness(binding, u64::MAX)), core::Observation::Ignored);
            assert_eq!(client.applied(binding, witness(binding, 10)), core::Observation::Ignored);
            let observed = client.applied(binding, witness(binding, 11));
            settled(observed, &operation);
            if rejected { assert!(matches!(observed, core::Observation::Settled(core::PurchaseSettlement {
                result: core::EconomicResult::Rejected { server_revision: 11 }, .. }))); }
        }
    }

    #[test]
    fn native_currencies_sources_and_zero_selector_roundtrip_without_local_pricing() {
        for currency in [wire::Currency::Gold, wire::Currency::Pearls] {
            for source in [wire::Source::Trade, wire::Source::BuyBack, wire::Source::Used] {
                let (mut client, token, binding) = ready();
                let intent = wire::Intent { currency, source,
                    request: wire::PurchaseRequest::new(wire::U64::new(0), u16::MAX, 1).unwrap(), ..intent() };
                quoted(&mut client, token, binding, &intent);
                let operation = client.reserve(token, binding, intent.clone()).unwrap();
                assert_eq!(operation.intent, intent);
                let sent = client.enter(token, binding, &operation).unwrap();
                assert_eq!(wire::parse_client_request(&sent.body).unwrap().action.operation(), Some(&operation));
                assert_eq!(client.receive(token, &purchase_frame(&sent, 2, 11)).unwrap().observation, core::Observation::Pending);
                settled(client.applied(binding, witness(binding, 11)), &operation);
            }
        }
    }

    #[test]
    fn native_replacement_connection_and_foreign_host_tokens_cannot_rebind_or_consume() {
        let (mut client, old_token, old_binding) = ready();
        let outstanding = client.quote(old_token, old_binding, intent().request).unwrap();
        let old_frame = frame(&outstanding.request, wire::ServerReply::Quote { intent: intent() }, None);
        let token = client.open_connection().unwrap();
        assert_eq!(client.current_binding(), None);
        assert!(!client.disconnect(old_token));
        assert_eq!(client.connection(), Some(token));
        assert!(client.receive(old_token, &old_frame).is_err());
        assert!(client.receive(token, &old_frame).is_err());
        assert_eq!(client.controls.len(), 1);
        let mut other = NativeNpcPurchaseClient::new().unwrap();
        let other_token = other.open_connection().unwrap();
        assert!(client.begin(other_token).is_err());
        assert_eq!(client.connection(), Some(token));
        let binding = accept(&mut client, token, 1, 2);
        assert_eq!(client.applied(old_binding, witness(old_binding, 10)), core::Observation::Ignored);
        assert!(client.reserve(token, binding, intent()).is_err());
        assert_eq!(client.controls.len(), 1);
    }

    #[test]
    fn native_actor_capacity_refusal_retains_original_operation_and_all_history() {
        let mut client = NativeNpcPurchaseClient::new().unwrap();
        client.host = NpcPurchaseReceiptHost::with_capacity(2).unwrap();
        let token = client.open_connection().unwrap();
        let first = accept(&mut client, token, 1, 2);
        client.applied(first, witness(first, 10));
        let operation = reserve(&mut client, token, first);
        client.enter(token, first, &operation).unwrap();
        accept(&mut client, token, 9, 8);
        let dispatched = client.begin(token).unwrap();
        let producer = producer(5, 6, 10);
        let raw = frame(&dispatched.request, wire::ServerReply::Producer { producer: producer.clone() }, Some(producer));
        assert!(client.receive(token, &raw).is_err());
        assert_eq!(client.current_binding(), None);
        assert_eq!(client.host.retained_actor_count(), 2);
        assert_eq!(client.host.pending(first.actor()).unwrap().key, operation_key(&operation));
        assert!(client.controls.contains_key(&dispatched.request.request_id.get()));
        let resumed = accept(&mut client, token, 1, 3);
        assert_eq!(client.query(token, resumed).unwrap().request.action.operation(), Some(&operation));
    }
}

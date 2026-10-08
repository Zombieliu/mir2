//! Native connection-owner integration. Only this single writer owns the
//! permanent purchase client. Read-only controls never consume a UI envelope.
use super::*;
use std::{collections::BTreeMap, pin::Pin};
use crate::npc_purchase_client::{ConnectionToken, Dispatch, NativeNpcPurchaseClient, PurchaseBinding};
use mir2_client_core::npc_purchase_receipt::{ActorKey, Observation, SnapshotWitness};
use mir2_bevy_runtime::npc_purchase_economy::{NativeNpcEconomyGate, NativeNpcEconomyProjection};
use mir2_client_wire as wire;

const MAX_PENDING_SOURCES: usize = 32;

/// Inspect every top-level type, including escaped/duplicate keys. The strict
/// wire parser subsequently rejects duplicates before any control is consumed.
pub(super) fn claims_owner_frame(raw: &str) -> bool {
    struct Marker;
    impl<'de> Deserialize<'de> for Marker {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Marker;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("gateway envelope") }
                fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Marker,A::Error> {
                    while let Some(key) = map.next_key::<String>()? {
                        if key == "type" {
                            let value = map.next_value::<Value>()?;
                            if value.as_str() == Some("npcPurchaseOwner") {
                                return Err(serde::de::Error::custom("native owner marker"));
                            }
                        } else { let _ = map.next_value::<serde::de::IgnoredAny>()?; }
                    }
                    Ok(Marker)
                }
            }
            d.deserialize_map(Visitor)
        }
    }
    serde_json::from_str::<Marker>(raw).is_err_and(|error| error.to_string().starts_with("native owner marker"))
}

#[derive(Debug,Clone)]
pub(super) struct UiSource {
    revision: u64,
    inventory: Value,
    shop: mir2_client_bevy::shop::ShopModel,
    parent: Value,
}
impl UiSource {
    pub(super) fn at_publication(state: &NativeCommandFenceState, stamp: NativeCommandStamp) -> Option<Self> {
        if !state.npc_gold_buy.ready(stamp) { return None; }
        let parent = state.npc_gold_buy.snapshot_dialog.as_ref()?;
        if !parent.get("npcObjectId").and_then(Value::as_u64).is_some_and(|id| id > 0 && id <= u64::from(u32::MAX))
            || !parent.get("scriptKey").and_then(Value::as_str).is_some_and(|key| !key.is_empty())
            || !parent.get("service").and_then(Value::as_str).is_some_and(|key| !key.is_empty())
            || !matches!(parent.get("packetType").and_then(Value::as_str), Some("NPCGoods" | "NPCPearlGoods")) { return None; }
        Some(Self { revision: state.npc_gold_buy.revision,
            inventory: serde_json::to_value(state.npc_gold_buy.inventory.as_ref()?).ok()?,
            shop: state.npc_gold_buy.shop.clone(), parent: parent.clone() })
    }
    fn capture(owned: &OwnedGatewayCommand) -> Option<Self> {
        let state = owned.fence.0.lock().ok()?;
        let source = owned.npc_purchase_source.as_ref()?;
        if !source.matches(&state,owned) { return None; }
        if owned.npc_gold_buy_required && owned.npc_gold_buy.is_none() { return None; }
        if let Some(proof) = &owned.npc_gold_buy {
            if proof.ticket.source_revision != source.revision || proof.inventory != source.inventory { return None; }
        }
        Some(source.clone())
    }
    fn matches(&self, state: &NativeCommandFenceState, owned: &OwnedGatewayCommand) -> bool {
        owned_current(state, owned) && state.npc_gold_buy.ready(owned.stamp)
            && self.revision == state.npc_gold_buy.revision && self.shop == state.npc_gold_buy.shop
            && state.npc_gold_buy.snapshot_dialog.as_ref() == Some(&self.parent)
            && state.npc_gold_buy.inventory.as_ref().and_then(|model| serde_json::to_value(model).ok()).as_ref() == Some(&self.inventory)
    }
    fn allows_intent(&self, intent: &wire::Intent) -> bool {
        // Crystal's Used popup advertises BUY_SUB (1), while its BuyItem
        // request still carries BUY (0). The quoted source must be Used.
        intent.request.panel_type == 0 && self.shop.allows_buy() && self.shop.goods.iter().any(|good|
            good.unique_id == intent.request.item_index.get()
            && (good.panel_type == 0 || (good.panel_type == 1 && intent.source == wire::Source::Used
                && good.stock >= 0 && !good.use_pearls))
            && good.use_pearls == matches!(intent.currency, wire::Currency::Pearls))
    }
}
fn owned_current(state: &NativeCommandFenceState, owned: &OwnedGatewayCommand) -> bool {
    state.outstanding.get(&owned.sequence) == Some(&owned.stamp)
        && NativeCommandFence::matches(state, owned.stamp, NativeCommandScope::World)
}

struct HeldQuote {
    owned: OwnedGatewayCommand,
    source: UiSource,
    token: ConnectionToken,
    binding: PurchaseBinding,
    request: wire::PurchaseRequest,
    control_id: u64,
    quoted: Option<(wire::Intent, SnapshotWitness)>,
}
impl Drop for HeldQuote {
    fn drop(&mut self) { self.owned.fence.retire(&self.owned); }
}
struct PendingSource {
    binding: PurchaseBinding,
    witness: SnapshotWitness,
    projection: NativeNpcEconomyProjection,
}
struct EnteredUiProof {
    operation: wire::Operation,
    command: mir2_client_bevy::npc_shop_buy::NpcGoldBuyCommand,
    proof: Arc<NativeNpcGoldBuyProof>,
}

pub(super) struct NativeNpcPurchaseGateway {
    client: NativeNpcPurchaseClient,
    token: Option<ConnectionToken>,
    attempted_stamp: Option<NativeCommandStamp>,
    binding_stamp: Option<NativeCommandStamp>,
    gate: Option<NativeNpcEconomyGate>,
    applied: Option<SnapshotWitness>,
    sources: VecDeque<PendingSource>,
    held: Option<HeldQuote>,
    entered_ui: BTreeMap<ActorKey, EnteredUiProof>,
    recovery_sent: bool,
    entry_allowed: bool,
    needs_world: bool,
    map_file_name: Option<String>,
}
impl NativeNpcPurchaseGateway {
    pub(super) fn new() -> Result<Self,String> {
        Ok(Self { client: NativeNpcPurchaseClient::new().ok_or("Native purchase host IDs exhausted")?,
            token: None, attempted_stamp: None, binding_stamp: None, gate: None, applied: None,
            sources: VecDeque::new(), held: None, entered_ui: BTreeMap::new(), recovery_sent: false,
            entry_allowed: false, needs_world: true, map_file_name: None })
    }
    pub(super) fn open_connection(&mut self) -> Result<(),String> {
        self.disconnect();
        self.token = Some(self.client.open_connection().map_err(str::to_owned)?);
        Ok(())
    }
    fn retire_binding(&mut self) {
        if let Some(gate) = self.gate.take() { gate.retire(); }
        if let Some(held) = self.held.take() { held.owned.fence.retire(&held.owned); }
        self.sources.clear(); self.applied = None; self.binding_stamp = None; self.recovery_sent = false; self.map_file_name = None;
        if let Some(token) = self.token { self.client.withdraw_current(token); }
    }
    pub(super) fn disconnect(&mut self) {
        self.retire_binding();
        if let Some(token) = self.token.take() { self.client.disconnect(token); }
        self.attempted_stamp = None; self.entry_allowed = false; self.needs_world = true;
        // Entered Actor operations and their exact UI proofs survive disconnect.
    }
    pub(super) fn leave_or_select(&mut self) {
        self.retire_binding(); self.attempted_stamp = None; self.entry_allowed = false; self.needs_world = true;
    }
    pub(super) fn before_ordinary_frame(&mut self, raw: &str) {
        if let Ok(envelope) = serde_json::from_str::<GatewayEnvelope>(raw) {
            if let Some(packet) = envelope.packet.as_deref() {
                if packet_native_reset_scope(packet) == Some(NativeResetScope::Session) { self.leave_or_select(); }
                else if matches!(packet, "MapInformation" | "MapChanged") {
                    self.retire_binding(); self.attempted_stamp = None; self.needs_world = true;
                }
            }
        }
    }
    pub(super) fn after_ordinary_frame(&mut self, raw: &str, phase: ConnectionPhase, bootstrapped: bool) {
        if let Ok(event) = parse_inbound_event(raw) {
            match event {
                InboundEvent::Packet(PacketEvent::StartGameAck(ack)) if ack.result == Some(4) => {
                    self.entry_allowed = true; self.needs_world = true;
                }
                InboundEvent::SessionResumed(_) if phase != ConnectionPhase::AwaitingResume => {
                    self.entry_allowed = true; self.needs_world = true;
                }
                _ => {}
            }
        }
        if bootstrapped && phase == ConnectionPhase::Normal && text_kind(raw).as_deref() == Some("worldSnapshot") {
            self.needs_world = false;
        }
    }
    /// Begin is eligible only after successful server entry and a full active
    /// producer stamp. Opt-in alone grants no binding or purchase permission.
    pub(super) fn begin_if_ready(&mut self, fence: Option<&NativeCommandFence>, stamp: Option<NativeCommandStamp>,
        phase: ConnectionPhase, bootstrapped: bool, current_map_file: Option<&str>) -> Result<Option<(Dispatch,NativeCommandStamp)>,String>
    {
        let active = fence.zip(stamp).filter(|(fence, stamp)| fence.accepts(*stamp, true)
            && fence.0.lock().is_ok_and(|state| !state.npc_entry_pending));
        if self.binding_stamp.is_some_and(|saved| active.is_none_or(|(_, current)| current != saved)) {
            self.retire_binding();
        }
        if !self.entry_allowed || self.needs_world || !bootstrapped || phase != ConnectionPhase::Normal { return Ok(None); }
        let Some((_,stamp)) = active else { return Ok(None); };
        let Some(map_file) = current_map_file.filter(|name| !name.is_empty()) else { return Ok(None); };
        if self.attempted_stamp == Some(stamp) { return Ok(None); }
        self.retire_binding(); self.attempted_stamp = Some(stamp);
        self.map_file_name = Some(normalize_map_file_name(map_file));
        let token = self.token.ok_or("Native purchase connection missing")?;
        let dispatch = self.client.begin(token).map_err(str::to_owned)?;
        Ok(Some((dispatch,stamp)))
    }
    fn observe(&mut self, observation: Observation) {
        if let Observation::Settled(settlement) = observation {
            self.recovery_sent=false;
            if self.entered_ui.get(&settlement.key.actor).is_some_and(|saved|
                crate::npc_purchase_client::settlement_matches(&saved.operation, &settlement)) {
                let saved = self.entered_ui.remove(&settlement.key.actor).expect("exact retained UI operation");
                saved.proof.gate.settle_durable(saved.proof.ticket, &saved.command);
            }
        }
    }
    pub(super) fn receive(&mut self, raw: &str, fence: &NativeCommandFence) -> Result<Option<Value>,String> {
        self.receive_with(raw,fence,mir2_bevy_runtime::native_ingest::push_native_npc_economy_bundle)
    }
    fn receive_with(&mut self, raw: &str, fence: &NativeCommandFence,
        mut enqueue: impl FnMut(mir2_bevy_runtime::npc_purchase_economy::NativeNpcEconomyBundle)->bool) -> Result<Option<Value>,String>
    {
        let token = self.token.ok_or("Native purchase connection missing")?;
        let expected = self.binding_stamp.or(self.attempted_stamp).ok_or("Native purchase entry custody missing")?;
        if !fence.accepts(expected, true) { return Err("Stale Native purchase actor/scene".into()); }
        if fence.0.lock().map_or(true,|state| state.npc_entry_pending) { return Err("Native purchase entry superseded".into()); }
        let received = self.client.receive(token, raw).map_err(str::to_owned)?;
        self.observe(received.observation);
        let Some(binding) = received.binding else { return Ok(None); };
        if matches!(&received.frame.reply, wire::ServerReply::Producer {..}) {
            let gate = NativeNpcEconomyGate::new(binding);
            let mut state = fence.0.lock().map_err(|_| "Native purchase owner fence poisoned")?;
            if !NativeCommandFence::matches(&state,expected,NativeCommandScope::World) { gate.retire(); return Err("Stale Native purchase Begin producer".into()); }
            if let Some(prior)=state.npc_economy_gate.take(){prior.retire();}
            state.npc_economy_gate = Some(gate.clone());
            self.gate = Some(gate); self.binding_stamp = Some(expected);
            self.applied = None; self.recovery_sent = false;
        }
        let mut paired_witness = None;
        let mut accepted_owner = None;
        if let (Some(owner),Some(authority)) = (&received.frame.snapshot,&received.frame.authority) {
            if self.sources.len() >= MAX_PENDING_SOURCES { return Err("Native purchase source completion capacity".into()); }
            let self_entity = owner.get("entities").and_then(Value::as_array).and_then(|entities|
                entities.iter().find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")));
            if self_entity.and_then(|entity| value_u32(entity.get("objectId"))) != expected.actor {
                return Err("Native purchase source actor mismatch".into());
            }
            if !map_file_name(owner).is_some_and(|name| self.map_file_name.as_deref() == Some(normalize_map_file_name(name).as_str())) {
                return Err("Native purchase source scene mismatch".into());
            }
            let witness = SnapshotWitness { actor: binding.actor(), producer_scope: binding.producer_scope(),
                server_revision: authority.server_revision.get(), complete: true };
            let projection = crate::npc_purchase_projection::project_owner(owner)?;
            let gate = self.gate.as_ref().ok_or("Native purchase complete gate missing")?;
            let bundle = gate.prepare(witness,projection.clone()).map_err(|error| format!("Native purchase source: {error:?}"))?;
            let hero_gate = {
                let state=fence.0.lock().map_err(|_|"Native purchase Hero source fence poisoned")?;
                if state.npc_entry_pending || !NativeCommandFence::matches(&state,expected,NativeCommandScope::World) {
                    return Err("Native purchase Hero source lifetime changed".into());
                }
                state.hero_owner_gate.as_ref().filter(|(stamp,_)|*stamp==expected)
                    .map(|(_,gate)|gate.clone()).ok_or("Native purchase current Hero source lifetime missing")?
            };
            // Carry only current producer lifetime provenance. Hero economic
            // fields remain the freshly projected owner in this bundle.
            let bundle=bundle.with_hero_owner_gate(&hero_gate)
                .map_err(|error|format!("Native purchase Hero source: {error:?}"))?;
            let admitted = {
                let state=fence.0.lock().map_err(|_|"Native purchase owner admission fence poisoned")?;
                if state.npc_entry_pending || !NativeCommandFence::matches(&state,expected,NativeCommandScope::World)
                    || !state.hero_owner_gate.as_ref().is_some_and(|(stamp,_)|*stamp==expected) {
                    return Err("Native purchase source lifetime changed before admission".into());
                }
                enqueue(bundle)
            };
            if !admitted {
                return Err("Native purchase full bundle admission refused".into());
            }
            self.sources.push_back(PendingSource { binding,witness,projection });
            paired_witness = Some(witness); accepted_owner = Some(owner.clone());
        }
        if let Some(held) = self.held.as_mut().filter(|held| held.control_id == received.frame.request_id.get()) {
            match &received.frame.reply {
                wire::ServerReply::Quote { intent } if intent.request == held.request && held.source.allows_intent(intent) => {
                    // A read-only Quote need not replace an unchanged economy.
                    // Paired newer source, when supplied, must actually apply.
                    if let Some(witness) = paired_witness.or(self.applied) { held.quoted = Some((intent.clone(),witness)); }
                    else { return Err("Native purchase quote lacks complete baseline".into()); }
                }
                _ => { let held = self.held.take().expect("correlated quote"); held.owned.fence.retire(&held.owned); }
            }
        }
        Ok(accepted_owner)
    }
    pub(super) fn drain_applied(&mut self, fence: &NativeCommandFence) -> Result<(),String> {
        for _ in 0..MAX_PENDING_SOURCES {
            let Some(applied) = self.gate.as_ref().and_then(NativeNpcEconomyGate::try_recv_applied) else { break; };
            if Some(applied.connection()) != self.token || self.client.current_binding() != Some(applied.binding())
                || !self.binding_stamp.is_some_and(|stamp| fence.accepts(stamp,true)) { continue; }
            let Some(index) = self.sources.iter().position(|source| source.binding == applied.binding() && source.witness == applied.witness())
                else { return Err("Native purchase applied source custody missing".into()); };
            let source = self.sources.remove(index).expect("sealed applied source");
            publish_applied_ui_source(fence,self.binding_stamp.expect("current source stamp"),&source.projection)?;
            self.applied = Some(applied.witness());
            let observation = self.client.applied(applied.binding(),applied.witness()); self.observe(observation);
        }
        Ok(())
    }
    pub(super) fn recovery_if_ready(&mut self) -> Result<Option<(Dispatch,NativeCommandStamp)>,String> {
        if self.recovery_sent || self.applied.is_none() { return Ok(None); }
        let Some(binding) = self.client.current_binding() else { return Ok(None); };
        if !self.client.recovery_needed(){return Ok(None);}
        self.recovery_sent = true;
        let token = self.token.ok_or("Native purchase connection missing")?;
        let dispatch = self.client.query(token,binding).map_err(str::to_owned)?;
        Ok(Some((dispatch,self.binding_stamp.ok_or("Native purchase source stamp missing")?)))
    }
    pub(super) fn quote(&mut self, owned: OwnedGatewayCommand) -> Result<(Dispatch,NativeCommandStamp),String> {
        let refuse = |owned: &OwnedGatewayCommand| { owned.fence.retire(owned); };
        if self.held.is_some() || self.applied.is_none() || self.client.pending_current().is_some() {
            refuse(&owned); return Err("Native purchase not ready or pending".into());
        }
        let Some(binding) = self.client.current_binding().filter(|_| self.binding_stamp == Some(owned.stamp)) else {
            refuse(&owned); return Err("Native purchase UI actor binding missing".into());
        };
        if !self.gate.as_ref().is_some_and(|gate| gate.is_current(binding)) {
            refuse(&owned); return Err("Native purchase source retired".into());
        }
        let Some(source) = UiSource::capture(&owned) else { refuse(&owned); return Err("Native purchase original UI source unavailable".into()); };
        let request = match &owned.command {
            GatewayCommand::Wire(NativeOutboundCommand::BuyItem {item_index,count,panel_type}) =>
                match wire::PurchaseRequest::new(wire::U64::new(*item_index),*count,*panel_type) {
                    Ok(request) => request, Err(error) => { refuse(&owned); return Err(error.to_string()); }
                },
            _ => { refuse(&owned); return Err("Native purchase UI command mismatch".into()); }
        };
        let token = self.token.ok_or("Native purchase connection missing")?;
        let dispatch = match self.client.quote(token,binding,request.clone()) {
            Ok(dispatch) => dispatch, Err(error) => { refuse(&owned); return Err(error.into()); }
        };
        let stamp = owned.stamp;
        self.held = Some(HeldQuote { owned,source,token,binding,request,control_id: dispatch.request.request_id.get(),quoted: None });
        Ok((dispatch,stamp))
    }
    pub(super) fn cancel_quote(&mut self) {
        if let Some(held) = self.held.take() { held.owned.fence.retire(&held.owned); }
    }
    pub(super) async fn purchase_if_ready<S>(&mut self,sink: &mut S) -> Option<NativeSinkCommit>
    where S:futures_util::Sink<Message>+Unpin,S::Error:std::fmt::Display {
        let mut budget=TokioNativeWriteBudget::new();
        self.purchase_if_ready_with_budget(sink,&mut budget).await
    }
    async fn purchase_if_ready_with_budget<S>(&mut self, sink: &mut S,budget: &mut impl NativeWriteBudget) -> Option<NativeSinkCommit>
    where S: futures_util::Sink<Message> + Unpin, S::Error: std::fmt::Display {
        let held = self.held.as_ref()?;
        let (_, required) = held.quoted.as_ref()?;
        if !self.applied.is_some_and(|applied| applied.actor == required.actor
            && applied.producer_scope == required.producer_scope && applied.server_revision >= required.server_revision) { return None; }
        let held = self.held.take().expect("ready original Quote");
        let (outcome,_) = commit_purchase_with_budget(sink,&mut self.client,&held,self.gate.as_ref(),&mut self.entered_ui,budget).await;
        Some(outcome)
    }
}

fn publish_applied_ui_source(fence: &NativeCommandFence, stamp: NativeCommandStamp,
    projection: &NativeNpcEconomyProjection) -> Result<(),String>
{
    let inventory: mir2_client_bevy::inventory::InventoryModel = serde_json::from_str(&projection.inventory_json).map_err(|error| error.to_string())?;
    let shop: mir2_client_bevy::shop::ShopModel = serde_json::from_str(&projection.shop_json).map_err(|error| error.to_string())?;
    let owner: Value = serde_json::from_str(&projection.owner_json).map_err(|error| error.to_string())?;
    let mut state = fence.0.lock().map_err(|_| "Native purchase owner fence poisoned")?;
    if !NativeCommandFence::matches(&state,stamp,NativeCommandScope::World) { return Err("Stale Native purchase full Applied".into()); }
    state.npc_gold_buy.sync_owner(stamp);
    let mut original = state.npc_gold_buy.inventory.clone(); if let Some(model) = &mut original { model.npc_gold_trade_capacity = None; }
    let mut economic = inventory.clone(); economic.npc_gold_trade_capacity = None;
    let parent = npc_snapshot_service_parent(Some(&owner));
    if original.as_ref().and_then(|model| serde_json::to_value(model).ok()) != serde_json::to_value(&economic).ok()
        || state.npc_gold_buy.shop != shop || state.npc_gold_buy.snapshot_dialog != parent { state.npc_gold_buy.advance(); }
    state.npc_gold_buy.inventory = Some(inventory); state.npc_gold_buy.shop = shop;
    state.npc_gold_buy.snapshot_dialog = parent;
    state.npc_gold_buy.dialog_retired_since_snapshot = false;
    state.npc_gold_buy.inventory_ready = true; state.npc_gold_buy.shop_ready = true;
    for waker in state.waiters.values() { waker.wake_by_ref(); }
    Ok(())
}

/// No UI sequence/gate claim occurs for Begin/Quote/Query. The exact physical
/// generation is checked after readiness, immediately before start_send.
pub(super) async fn commit_control<S>(sink: &mut S,fence: &NativeCommandFence,
    stamp: NativeCommandStamp,dispatch: Dispatch) -> NativeSinkCommit
where S:futures_util::Sink<Message>+Unpin,S::Error:std::fmt::Display {
    let mut budget=TokioNativeWriteBudget::new();
    commit_control_with_budget(sink,fence,stamp,dispatch,&mut budget).await
}

async fn commit_control_with_budget<S>(sink: &mut S, fence: &NativeCommandFence,
    stamp: NativeCommandStamp, dispatch: Dispatch, budget: &mut impl NativeWriteBudget) -> NativeSinkCommit
where S: futures_util::Sink<Message> + Unpin, S::Error: std::fmt::Display {
    struct Waiter(NativeCommandFence);
    impl Drop for Waiter { fn drop(&mut self) { if let Ok(mut state) = self.0.0.lock() { state.waiters.remove(&0); } } }
    let waiter = Waiter(fence.clone());
    let ready = await_native_write_phase(budget,|cx| {
        let Ok(mut state) = fence.0.lock() else { return std::task::Poll::Ready(None); };
        if !NativeCommandFence::matches(&state,stamp,NativeCommandScope::World) { return std::task::Poll::Ready(None); }
        // Command sequences are nonzero; zero is the sole owner's control waiter.
        state.waiters.insert(0,cx.waker().clone());
        drop(state);
        Pin::new(&mut *sink).poll_ready(cx).map(Some)
    }).await;
    drop(waiter);
    match ready {
        Err(_) => return NativeSinkCommit::Unavailable(NATIVE_TRANSPORT_TIMEOUT_ERROR.into()),
        Ok(None) => return NativeSinkCommit::DefinitelyUnsent,
        Ok(Some(Err(error))) => return NativeSinkCommit::Unavailable(error.to_string()),
        Ok(Some(Ok(()))) => {}
    }
    let started = {
        let Ok(state) = fence.0.lock() else { return NativeSinkCommit::DefinitelyUnsent; };
        if !NativeCommandFence::matches(&state,stamp,NativeCommandScope::World) { return NativeSinkCommit::DefinitelyUnsent; }
        if budget.expired() {return NativeSinkCommit::Unavailable(NATIVE_TRANSPORT_TIMEOUT_ERROR.into());}
        Pin::new(&mut *sink).start_send(Message::Text(dispatch.body.into()))
    };
    if let Err(error) = started { return NativeSinkCommit::Unknown(error.to_string()); }
    match await_native_write_phase(budget,|cx| Pin::new(&mut *sink).poll_flush(cx)).await {
        Ok(Ok(())) => NativeSinkCommit::Flushed, Ok(Err(error)) => NativeSinkCommit::Unknown(error.to_string()),
        Err(_) => NativeSinkCommit::Unknown(NATIVE_TRANSPORT_TIMEOUT_ERROR.into())
    }
}

async fn commit_purchase_with_budget<S>(sink: &mut S, client: &mut NativeNpcPurchaseClient, held: &HeldQuote,
    gate: Option<&NativeNpcEconomyGate>, entered_ui: &mut BTreeMap<ActorKey,EnteredUiProof>, budget: &mut impl NativeWriteBudget) -> (NativeSinkCommit,Option<wire::Operation>)
where S: futures_util::Sink<Message> + Unpin, S::Error: std::fmt::Display {
    struct PurchaseWaiter<'a>(&'a OwnedGatewayCommand);
    impl Drop for PurchaseWaiter<'_> {fn drop(&mut self){
        if let Ok(mut state)=self.0.fence.0.lock(){state.waiters.remove(&self.0.sequence);}
        if let Some(proof)=&self.0.npc_gold_buy{proof.gate.forget_waiter(proof.ticket);}
    }}
    let waiter=PurchaseWaiter(&held.owned);
    let valid = || gate.is_some_and(|gate| gate.is_current(held.binding))
        && client.connection() == Some(held.token) && client.current_binding() == Some(held.binding);
    if !valid() { held.owned.fence.retire(&held.owned); return (NativeSinkCommit::DefinitelyUnsent,None); }
    let ready = await_native_write_phase(budget,|cx| {
        let Ok(mut state) = held.owned.fence.0.lock() else { return std::task::Poll::Ready(None); };
        if !held.source.matches(&state,&held.owned) || !valid()
            || held.owned.npc_gold_buy.as_ref().is_some_and(|proof| !proof.gate.watch(proof.ticket,cx.waker())) {
            return std::task::Poll::Ready(None);
        }
        state.waiters.insert(held.owned.sequence,cx.waker().clone());
        Pin::new(&mut *sink).poll_ready(cx).map(Some)
    }).await;
    drop(waiter);
    match ready {
        Err(_) => {held.owned.fence.retire(&held.owned);return (NativeSinkCommit::Unavailable(NATIVE_TRANSPORT_TIMEOUT_ERROR.into()),None);}
        Ok(None) => { held.owned.fence.retire(&held.owned); return (NativeSinkCommit::DefinitelyUnsent,None); }
        Ok(Some(Err(error))) => { held.owned.fence.retire(&held.owned); return (NativeSinkCommit::Unavailable(error.to_string()),None); }
        Ok(Some(Ok(()))) => {}
    }
    let (start,operation) = {
        let Ok(mut state) = held.owned.fence.0.lock() else { return (NativeSinkCommit::DefinitelyUnsent,None); };
        if !held.source.matches(&state,&held.owned) || !valid() {
            drop(state); held.owned.fence.retire(&held.owned); return (NativeSinkCommit::DefinitelyUnsent,None);
        }
        if budget.expired() {drop(state);held.owned.fence.retire(&held.owned);return (NativeSinkCommit::Unavailable(NATIVE_TRANSPORT_TIMEOUT_ERROR.into()),None);}
        let intent = held.quoted.as_ref().expect("ready server quote").0.clone();
        let operation = match client.reserve(held.token,held.binding,intent) {
            Ok(operation) => operation,
            Err(_) => { drop(state); held.owned.fence.retire(&held.owned); return (NativeSinkCommit::DefinitelyUnsent,None); }
        };
        let prepared = match client.prepare_enter(held.token,held.binding,&operation) {
            Ok(prepared) => prepared,
            Err(_) => { client.cancel_unsent(held.token,held.binding,&operation);drop(state);held.owned.fence.retire(&held.owned);return (NativeSinkCommit::DefinitelyUnsent,None); }
        };
        // Recheck after synchronous durable preflight, before either local claim.
        if budget.expired() {client.cancel_unsent(held.token,held.binding,&operation);drop(state);held.owned.fence.retire(&held.owned);return (NativeSinkCommit::Unavailable(NATIVE_TRANSPORT_TIMEOUT_ERROR.into()),None);}
        let mut entered = false;
        let send = || -> Result<(),String> {
            let dispatch = client.commit_prepared_entry(prepared).map_err(str::to_owned)?;
            entered = true;
            Pin::new(&mut *sink).start_send(Message::Text(dispatch.body.into())).map_err(|error| error.to_string())
        };
        let start = if let Some(proof) = &held.owned.npc_gold_buy { proof.gate.commit(proof.ticket,send) }
            else { Some(send()) };
        state.outstanding.remove(&held.owned.sequence);
        if !entered {
            client.cancel_unsent(held.token,held.binding,&operation);
            drop(state);
            if let Some(proof) = &held.owned.npc_gold_buy {
                // If its local gate entered but durable preflight refused,
                // retain that conservative legacy barrier; never invent unsent.
                proof.publish(if start.is_some() { mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::Unknown }
                    else { mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::DefinitelyUnsent });
            }
            return (NativeSinkCommit::DefinitelyUnsent,None);
        }
        (start.expect("durable entry belongs to one UI claim"),operation)
    };
    if let Some(proof) = held.owned.npc_gold_buy.clone() {
        let command = mir2_client_bevy::npc_shop_buy::NpcGoldBuyCommand {
            command_type: mir2_client_bevy::npc_shop_buy::NpcGoldBuyCommandType::BuyItem,
            item_index: held.request.item_index.get(), count: held.request.count, panel_type: held.request.panel_type };
        entered_ui.insert(held.binding.actor(),EnteredUiProof { operation: operation.clone(), command, proof });
    }
    // Drop during flush preserves original durable custody as Unknown; the UI
    // proof was retained above before the first post-entry suspension point.
    struct Entered<'a> { client: &'a mut NativeNpcPurchaseClient, token: ConnectionToken,
        binding: PurchaseBinding, operation: &'a wire::Operation, proof: Option<&'a NativeNpcGoldBuyProof>, completed: bool }
    impl Drop for Entered<'_> { fn drop(&mut self) { if !self.completed {
        self.client.unknown(self.token,self.binding,self.operation);
        if let Some(proof) = self.proof { proof.publish(mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::Unknown); }
    } } }
    let mut entered = Entered {client,token:held.token,binding:held.binding,operation:&operation,
        proof:held.owned.npc_gold_buy.as_deref(),completed:false};
    let outcome = match start {
        Err(error) => NativeSinkCommit::Unknown(error),
        Ok(()) => match await_native_write_phase(budget,|cx| Pin::new(&mut *sink).poll_flush(cx)).await {
            Ok(Ok(())) => NativeSinkCommit::Flushed, Ok(Err(error)) => NativeSinkCommit::Unknown(error.to_string()),
            Err(_) => NativeSinkCommit::Unknown(NATIVE_TRANSPORT_TIMEOUT_ERROR.into())
        }
    };
    if matches!(&outcome,NativeSinkCommit::Unknown(_)) { entered.client.unknown(held.token,held.binding,&operation); }
    if let Some(proof) = &held.owned.npc_gold_buy {
        proof.publish(if matches!(&outcome,NativeSinkCommit::Flushed) { mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::Flushed }
            else { mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::Unknown });
    }
    entered.completed = true;
    drop(entered);
    (outcome,Some(operation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{FutureExt,Sink};
    use std::future::Future;
    use std::task::{Context,Poll};
    use mir2_client_bevy::npc_gold_buy_attempt::{NpcGoldBuyGate,NpcGoldBuyAttemptPhase,npc_gold_buy_model_authority};
    use mir2_bevy_runtime::npc_purchase_economy::{NativeNpcEconomyBundle,apply_native_npc_economy_bundle};

    #[derive(Default)]
    struct SinkState { ready_pending:bool, start_error:bool, flush_pending:bool, flush_error:bool, frames:Vec<String> }
    #[derive(Clone,Default)]
    struct LocalSink(Arc<Mutex<SinkState>>);
    impl Sink<Message> for LocalSink {
        type Error = &'static str;
        fn poll_ready(self:Pin<&mut Self>,_:&mut Context<'_>)->Poll<Result<(),Self::Error>> {
            if self.0.lock().unwrap().ready_pending {Poll::Pending}else{Poll::Ready(Ok(()))}
        }
        fn start_send(self:Pin<&mut Self>,frame:Message)->Result<(),Self::Error> {
            let mut state=self.0.lock().unwrap(); state.frames.push(frame.to_text().unwrap().to_owned());
            if state.start_error {Err("local start failure")}else{Ok(())}
        }
        fn poll_flush(self:Pin<&mut Self>,_:&mut Context<'_>)->Poll<Result<(),Self::Error>> {
            let state=self.0.lock().unwrap(); if state.flush_pending {Poll::Pending}
            else {Poll::Ready(if state.flush_error {Err("local flush failure")}else{Ok(())})}
        }
        fn poll_close(self:Pin<&mut Self>,cx:&mut Context<'_>)->Poll<Result<(),Self::Error>> {self.poll_flush(cx)}
    }
    async fn control_local(sink:&mut LocalSink,fence:&NativeCommandFence,stamp:NativeCommandStamp,dispatch:Dispatch)->NativeSinkCommit {
        let mut budget=ManualNativeWriteBudget::new(u64::MAX);
        commit_control_with_budget(sink,fence,stamp,dispatch,&mut budget).await
    }
    async fn purchase_local(gateway:&mut NativeNpcPurchaseGateway,sink:&mut LocalSink)->Option<NativeSinkCommit> {
        let mut budget=ManualNativeWriteBudget::new(u64::MAX);
        gateway.purchase_if_ready_with_budget(sink,&mut budget).await
    }
    fn opaque(value:u8)->wire::Opaque32 {wire::Opaque32::from_bytes([value;32]).unwrap()}
    fn producer(scope:u8,revision:u64)->wire::Producer {
        wire::Producer {actor:opaque(1),producer_scope:opaque(scope),server_revision:wire::U64::new(revision)}
    }
    fn owner()->Value {
        let mut owner=crate::npc_purchase_projection::tests::owner();
        let item=mir2_client_bevy::inventory::CrystalUserItemModel {unique_id:0,item_index:658,count:1,is_shop_item:true,..Default::default()};
        owner["nativeNpcShop"]=json!({"npcObjectId":4990,"scriptKey":"BichonProvince/NaturalCave/WickedTrader","service":"BUY",
            "packetType":"NPCGoods","list":[item],"rate":1.0_f32,"panelType":0,"hideAddedStats":false});
        owner
    }
    fn server(request:&wire::ClientRequest,reply:wire::ServerReply,snapshot:Option<Value>,authority:Option<wire::Producer>)->String {
        wire::encode_server_frame(&wire::ServerFrame::new(request.request_id,reply,snapshot,authority).unwrap()).unwrap()
    }
    fn receive_local(gateway:&mut NativeNpcPurchaseGateway,fence:&NativeCommandFence,raw:&str)->Vec<NativeNpcEconomyBundle> {
        let mut bundles=vec![];
        gateway.receive_with(raw,fence,|bundle| {bundles.push(bundle);true}).unwrap(); bundles
    }
    fn apply_local(world:&mut bevy::prelude::World,gateway:&mut NativeNpcPurchaseGateway,fence:&NativeCommandFence,bundles:Vec<NativeNpcEconomyBundle>) {
        for bundle in bundles {assert!(apply_native_npc_economy_bundle(world,bundle));}
        gateway.drain_applied(fence).unwrap();
    }
    fn begin_local(gateway:&mut NativeNpcPurchaseGateway,fence:&NativeCommandFence,stamp:NativeCommandStamp,scope:u8,revision:u64)->Vec<NativeNpcEconomyBundle> {
        // Entry/authentication is a source specimen here, never a live login.
        gateway.entry_allowed=true;gateway.needs_world=false;
        assert!(fence.prepare_hero_owner(stamp,&owner()).unwrap().is_some());
        let (dispatch,_)=gateway.begin_if_ready(Some(fence),Some(stamp),ConnectionPhase::Normal,true,Some("0")).unwrap().unwrap();
        assert!(gateway.begin_if_ready(Some(fence),Some(stamp),ConnectionPhase::Normal,true,Some("0")).unwrap().is_none());
        let producer=producer(scope,revision);
        receive_local(gateway,fence,&server(&dispatch.request,wire::ServerReply::Producer {producer:producer.clone()},Some(owner()),Some(producer)))
    }
    fn ready()->(NativeNpcPurchaseGateway,GatewayCommandSender,GatewayCommandReceiver,NativeCommandFence,NativeCommandStamp,bevy::prelude::World) {
        let (sender,receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();let stamp=fence.test_world_ready(3,0);
        let mut gateway=NativeNpcPurchaseGateway::new().unwrap();gateway.open_connection().unwrap();let mut world=bevy::prelude::World::new();
        let bundles=begin_local(&mut gateway,&fence,stamp,2,10);apply_local(&mut world,&mut gateway,&fence,bundles);
        (gateway,sender,receiver,fence,stamp,world)
    }
    fn ui_gate(fence:&NativeCommandFence,stamp:NativeCommandStamp)->NpcGoldBuyGate {
        let state=fence.0.lock().unwrap();let gate=NpcGoldBuyGate::default();
        assert!(gate.observe_connection(mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyConnectionEpoch {run:stamp.run,connection:stamp.connection}));
        assert!(gate.observe("complete owner",state.npc_gold_buy.revision,
            &npc_gold_buy_model_authority(&state.npc_gold_buy.shop,state.npc_gold_buy.inventory.as_ref().unwrap()).unwrap(),true));gate
    }
    fn owned_buy(sender:&GatewayCommandSender,receiver:&mut GatewayCommandReceiver,fence:&NativeCommandFence,stamp:NativeCommandStamp,gate:&NpcGoldBuyGate)->OwnedGatewayCommand {
        let (mut shop,inventory,revision)={let state=fence.0.lock().unwrap();(state.npc_gold_buy.shop.clone(),state.npc_gold_buy.inventory.clone().unwrap(),state.npc_gold_buy.revision)};
        shop.selected_id=Some(0);let (token,command)=gate.reserve(&shop,&inventory,1).unwrap();
        sender.send_npc_gold_buy_with_bind(GatewayCommand::Wire(NativeOutboundCommand::BuyItem {item_index:command.item_index,count:command.count,panel_type:command.panel_type}),
            Some(stamp),token,revision,gate.clone(),|ticket|gate.bind(token,ticket)).unwrap();
        receiver.try_recv().unwrap().into_parts().1.unwrap()
    }
    fn quote_local(gateway:&mut NativeNpcPurchaseGateway,owned:OwnedGatewayCommand,fence:&NativeCommandFence,sink:&mut LocalSink)->wire::ClientRequest {
        let (dispatch,stamp)=gateway.quote(owned).unwrap();let request=dispatch.request.clone();
        assert_eq!(control_local(sink,fence,stamp,dispatch).now_or_never().unwrap(),NativeSinkCommit::Flushed);
        let wire::Action::Quote {request:purchase}=request.action.clone() else {panic!("readonly quote")};
        let intent=wire::Intent {request:purchase,currency:wire::Currency::Gold,source:wire::Source::Trade,service_catalog_proof:opaque(7)};
        assert!(receive_local(gateway,fence,&server(&request,wire::ServerReply::Quote {intent},None,None)).is_empty());request
    }
    fn source_profile(service:&str)->Value {
        let mut source=owner();source["nativeNpcShop"]["service"]=json!(service);
        if matches!(service,"BUYUSED"|"BUYBACK") {
            source["nativeNpcShop"]["list"][0]["count"]=json!(3);
            source["nativeNpcShop"]["list"][0]["is_shop_item"]=json!(false);
        }
        if service=="BUYUSED" {source["nativeNpcShop"]["panelType"]=json!(1);}
        if service=="PEARLBUY" {source["nativeNpcShop"]["packetType"]=json!("NPCPearlGoods");}
        source
    }
    fn ready_profile(source:&Value)->(NativeNpcPurchaseGateway,GatewayCommandSender,GatewayCommandReceiver,NativeCommandFence,NativeCommandStamp,bevy::prelude::World) {
        let (sender,receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();let stamp=fence.test_world_ready(3,0);
        assert!(fence.prepare_hero_owner(stamp,source).unwrap().is_some());
        let mut gateway=NativeNpcPurchaseGateway::new().unwrap();gateway.open_connection().unwrap();gateway.entry_allowed=true;gateway.needs_world=false;
        let (begin,_)=gateway.begin_if_ready(Some(&fence),Some(stamp),ConnectionPhase::Normal,true,Some("0")).unwrap().unwrap();
        let producer=producer(2,10);let bundles=receive_local(&mut gateway,&fence,&server(&begin.request,
            wire::ServerReply::Producer {producer:producer.clone()},Some(source.clone()),Some(producer)));
        let mut world=bevy::prelude::World::new();apply_local(&mut world,&mut gateway,&fence,bundles);
        (gateway,sender,receiver,fence,stamp,world)
    }
    fn profile_buy(service:&str,sender:&GatewayCommandSender,receiver:&mut GatewayCommandReceiver,
        fence:&NativeCommandFence,stamp:NativeCommandStamp)->OwnedGatewayCommand {
        if service=="BUY" {return owned_buy(sender,receiver,fence,stamp,&ui_gate(fence,stamp));}
        sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::BuyItem {item_index:0,count:1,panel_type:0}),Some(stamp)).unwrap();
        let owned=receiver.try_recv().unwrap().into_parts().1.unwrap();
        assert!(owned.npc_gold_buy.is_none());assert!(owned.npc_purchase_source.is_some());owned
    }
    fn change_source(source:&Value,field:u8)->Value {
        let mut changed=source.clone();
        match field {
            0=>changed["nativeNpcShop"]["rate"]=json!(2.0_f32),
            1=>changed["nativeNpcShop"]["list"][0]["unique_id"]=json!(9),
            2=>changed["nativeNpcShop"]["npcObjectId"]=json!(4991),
            3=>changed["nativeNpcShop"]["scriptKey"]=json!("BichonProvince/NaturalCave/OtherTrader"),
            4=>changed["gold"]=json!(89),
            _=>changed["nativeNpcShop"]["service"]=json!("BUYNEW"),
        } changed
    }
    fn apply_source_local(world:&mut bevy::prelude::World,gateway:&mut NativeNpcPurchaseGateway,fence:&NativeCommandFence,
        stamp:NativeCommandStamp,source:&Value,revision:u64) {
        let binding=gateway.client.current_binding().unwrap();
        let witness=SnapshotWitness {actor:binding.actor(),producer_scope:binding.producer_scope(),server_revision:revision,complete:true};
        let projection=crate::npc_purchase_projection::project_owner(source).unwrap();
        let hero_gate={let state=fence.0.lock().unwrap();let (current,gate)=state.hero_owner_gate.as_ref().unwrap();assert_eq!(*current,stamp);gate.clone()};
        let bundle=gateway.gate.as_ref().unwrap().prepare(witness,projection.clone()).unwrap().with_hero_owner_gate(&hero_gate).unwrap();
        gateway.sources.push_back(PendingSource {binding,witness,projection});
        apply_local(world,gateway,fence,vec![bundle]);
    }
    #[test]
    fn native_npc_gateway_publication_source_change_refuses_quote_without_reserve() {
        for service in ["BUY","BUYUSED","BUYBACK","PEARLBUY"] {
            for field in 0..6 {
                if field==5&&service!="BUY" {continue;}
                let source=source_profile(service);let (mut gateway,sender,mut receiver,fence,stamp,mut world)=ready_profile(&source);
                let owned=profile_buy(service,&sender,&mut receiver,&fence,stamp);let original=owned.npc_purchase_source.as_ref().unwrap().clone();
                apply_source_local(&mut world,&mut gateway,&fence,stamp,&change_source(&source,field),11);
                assert!(!original.matches(&fence.0.lock().unwrap(),&owned),"{service}/{field}");
                let sink=LocalSink::default();assert!(gateway.quote(owned).is_err(),"{service}/{field}");
                assert!(gateway.held.is_none());assert!(gateway.client.pending_current().is_none());assert!(gateway.entered_ui.is_empty());
                assert!(sink.0.lock().unwrap().frames.is_empty());
            }
        }
        // Unavailable publication data cannot be filled from a later full
        // owner, even for an envelope without a Gold UI ticket.
        let source=source_profile("PEARLBUY");let (mut gateway,sender,mut receiver,fence,stamp,mut world)=ready_profile(&source);
        fence.0.lock().unwrap().npc_gold_buy.shop_ready=false;
        sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::BuyItem {item_index:0,count:1,panel_type:0}),Some(stamp)).unwrap();
        let owned=receiver.try_recv().unwrap().into_parts().1.unwrap();assert!(owned.npc_purchase_source.is_none());
        apply_source_local(&mut world,&mut gateway,&fence,stamp,&source,11);
        assert!(gateway.quote(owned).is_err());assert!(gateway.client.pending_current().is_none());
    }
    #[test]
    fn native_npc_gateway_quoted_source_change_refuses_purchase_without_reserve() {
        for service in ["BUY","BUYUSED","BUYBACK","PEARLBUY"] {
            for field in 0..6 {
                if field==5&&service!="BUY" {continue;}
                let source=source_profile(service);let (mut gateway,sender,mut receiver,fence,stamp,mut world)=ready_profile(&source);
                let owned=profile_buy(service,&sender,&mut receiver,&fence,stamp);
                let (dispatch,quote_stamp)=gateway.quote(owned).unwrap();let request=dispatch.request.clone();let mut sink=LocalSink::default();
                assert_eq!(control_local(&mut sink,&fence,quote_stamp,dispatch).now_or_never().unwrap(),NativeSinkCommit::Flushed);
                let wire::Action::Quote {request:purchase}=request.action.clone() else {panic!("read-only Quote")};
                let intent=wire::Intent {request:purchase,currency:if service=="PEARLBUY" {wire::Currency::Pearls}else{wire::Currency::Gold},
                    source:match service {"BUYUSED"=>wire::Source::Used,"BUYBACK"=>wire::Source::BuyBack,_=>wire::Source::Trade},service_catalog_proof:opaque(7)};
                assert!(receive_local(&mut gateway,&fence,&server(&request,wire::ServerReply::Quote {intent},None,None)).is_empty());
                apply_source_local(&mut world,&mut gateway,&fence,stamp,&change_source(&source,field),11);
                assert_eq!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap(),Some(NativeSinkCommit::DefinitelyUnsent),"{service}/{field}");
                assert!(gateway.client.pending_current().is_none());assert!(gateway.entered_ui.is_empty());
                let sink_state=sink.0.lock().unwrap();assert_eq!(sink_state.frames.len(),1);
                assert!(matches!(wire::parse_client_request(&sink_state.frames[0]).unwrap().action,wire::Action::Quote {..}));
            }
        }
    }
    #[test]
    fn native_npc_gateway_identical_quote_source_preserves_publication_custody() {
        let (mut gateway,sender,mut receiver,fence,stamp,mut world)=ready();let ui=ui_gate(&fence,stamp);let mut sink=LocalSink::default();
        let owned=owned_buy(&sender,&mut receiver,&fence,stamp,&ui);let revision=owned.npc_purchase_source.as_ref().unwrap().revision;
        let (dispatch,quote_stamp)=gateway.quote(owned).unwrap();let request=dispatch.request.clone();
        assert_eq!(control_local(&mut sink,&fence,quote_stamp,dispatch).now_or_never().unwrap(),NativeSinkCommit::Flushed);
        let wire::Action::Quote {request:purchase}=request.action.clone() else {panic!("read-only Quote")};
        let intent=wire::Intent {request:purchase,currency:wire::Currency::Gold,source:wire::Source::Trade,service_catalog_proof:opaque(7)};
        let bundles=receive_local(&mut gateway,&fence,&server(&request,wire::ServerReply::Quote {intent},Some(owner()),Some(producer(2,10))));
        apply_local(&mut world,&mut gateway,&fence,bundles);
        let state=fence.0.lock().unwrap();assert_eq!(state.npc_gold_buy.revision,revision);
        let held=gateway.held.as_ref().unwrap();assert!(held.source.matches(&state,&held.owned));drop(state);
        assert_eq!(ui.feedback().phase,Some(NpcGoldBuyAttemptPhase::Bound));
        assert_eq!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap(),Some(NativeSinkCommit::Flushed));
        assert_eq!(sink.0.lock().unwrap().frames.len(),2);
        assert!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap().is_none());
    }
    fn terminal(request:&wire::ClientRequest,scope:u8,revision:u64,snapshot:Option<Value>)->String {
        let operation=request.action.operation().unwrap().clone();
        let receipt=wire::Receipt {producer_scope:opaque(scope),entry:wire::ReceiptEntry {operation:operation.clone(),server_revision:wire::U64::new(revision),
            outcome:wire::Outcome::Rejected {request:operation.intent.request,reason:wire::Rejection::InsufficientCurrency}}};
        let authority=snapshot.as_ref().map(|_|producer(scope,revision));
        let reply=match &request.action {wire::Action::Query {..}=>wire::ServerReply::Recovery {receipt:Some(receipt)},_=>wire::ServerReply::Purchase {receipt,replayed:false}};
        server(request,reply,snapshot,authority)
    }
    #[test]
    fn native_npc_gateway_transport_readiness_expiry_never_reserves_gold_or_pearl_operation() {
        for service in ["BUY","PEARLBUY"] {
            let source=source_profile(service);let (mut gateway,sender,mut receiver,fence,stamp,_world)=ready_profile(&source);
            let owned=profile_buy(service,&sender,&mut receiver,&fence,stamp);let sequence=owned.sequence;let ui=owned.npc_gold_buy.as_ref().map(|proof|proof.gate.clone());
            let (quote,quote_stamp)=gateway.quote(owned).unwrap();let request=quote.request.clone();let mut sink=LocalSink::default();
            assert_eq!(control_local(&mut sink,&fence,quote_stamp,quote).now_or_never().unwrap(),NativeSinkCommit::Flushed);
            let wire::Action::Quote{request:purchase}=request.action.clone() else{panic!("readonly Quote")};
            let intent=wire::Intent{request:purchase,currency:if service=="BUY"{wire::Currency::Gold}else{wire::Currency::Pearls},source:wire::Source::Trade,service_catalog_proof:opaque(7)};
            receive_local(&mut gateway,&fence,&server(&request,wire::ServerReply::Quote{intent},None,None));
            sink.0.lock().unwrap().ready_pending=true;let mut budget=ManualNativeWriteBudget::new(10);let clock=budget.clone();let mut cx=Context::from_waker(futures_util::task::noop_waker_ref());
            let mut future=Box::pin(gateway.purchase_if_ready_with_budget(&mut sink,&mut budget));assert!(future.as_mut().poll(&mut cx).is_pending());
            assert!(fence.0.lock().unwrap().waiters.contains_key(&sequence));clock.advance_to(10);
            assert!(matches!(future.as_mut().poll(&mut cx),Poll::Ready(Some(NativeSinkCommit::Unavailable(_)))));drop(future);
            assert!(gateway.client.pending_current().is_none());assert!(gateway.entered_ui.is_empty());assert!(gateway.held.is_none());
            assert_eq!(sink.0.lock().unwrap().frames.len(),1);assert!(fence.0.lock().unwrap().waiters.is_empty());assert!(!fence.0.lock().unwrap().outstanding.contains_key(&sequence));
            if let Some(ui)=ui{assert!(!ui.feedback().pending);}
        }
    }
    #[test]
    fn native_npc_gateway_transport_flush_expiry_retains_gold_ui_and_queries_original_after_reconnect() {
        let (mut gateway,sender,mut receiver,fence,stamp,mut world)=ready();let ui=ui_gate(&fence,stamp);let mut sink=LocalSink::default();
        quote_local(&mut gateway,owned_buy(&sender,&mut receiver,&fence,stamp,&ui),&fence,&mut sink);sink.0.lock().unwrap().flush_pending=true;
        let mut budget=ManualNativeWriteBudget::new(10);let clock=budget.clone();let mut cx=Context::from_waker(futures_util::task::noop_waker_ref());
        let mut future=Box::pin(gateway.purchase_if_ready_with_budget(&mut sink,&mut budget));assert!(future.as_mut().poll(&mut cx).is_pending());clock.advance_to(10);
        assert!(matches!(future.as_mut().poll(&mut cx),Poll::Ready(Some(NativeSinkCommit::Unknown(_)))));drop(future);
        let original=wire::parse_client_request(&sink.0.lock().unwrap().frames[1]).unwrap().action.operation().unwrap().clone();
        assert_eq!(gateway.client.pending_current().unwrap().phase,mir2_client_core::npc_purchase_receipt::PurchasePhase::Unknown);
        assert_eq!(ui.feedback().phase,Some(NpcGoldBuyAttemptPhase::Unknown));assert!(ui.feedback().pending);assert_eq!(gateway.entered_ui.len(),1);
        assert!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap().is_none());assert_eq!(sink.0.lock().unwrap().frames.len(),2);
        gateway.disconnect();gateway.open_connection().unwrap();let fresh=fence.test_reconnect(3,0);
        let bundles=begin_local(&mut gateway,&fence,fresh,3,10);apply_local(&mut world,&mut gateway,&fence,bundles);
        let (query,_)=gateway.recovery_if_ready().unwrap().unwrap();assert!(matches!(&query.request.action,wire::Action::Query{..}));assert_eq!(query.request.action.operation(),Some(&original));
        assert!(gateway.recovery_if_ready().unwrap().is_none());assert_eq!(gateway.entered_ui.len(),1);
        let bundles=receive_local(&mut gateway,&fence,&terminal(&query.request,3,11,Some(owner())));assert!(gateway.client.pending_current().is_some());
        apply_local(&mut world,&mut gateway,&fence,bundles);assert!(gateway.client.pending_current().is_none());assert!(gateway.entered_ui.is_empty());assert_eq!(sink.0.lock().unwrap().frames.len(),2);
    }
    #[test]
    fn native_npc_gateway_transport_one_budget_spans_readiness_and_pearl_flush() {
        let source=source_profile("PEARLBUY");let (mut gateway,sender,mut receiver,fence,stamp,_world)=ready_profile(&source);
        let owned=profile_buy("PEARLBUY",&sender,&mut receiver,&fence,stamp);let (quote,quote_stamp)=gateway.quote(owned).unwrap();let request=quote.request.clone();let mut sink=LocalSink::default();
        assert_eq!(control_local(&mut sink,&fence,quote_stamp,quote).now_or_never().unwrap(),NativeSinkCommit::Flushed);
        let wire::Action::Quote{request:purchase}=request.action.clone() else{panic!("readonly Quote")};
        let intent=wire::Intent{request:purchase,currency:wire::Currency::Pearls,source:wire::Source::Trade,service_catalog_proof:opaque(7)};
        receive_local(&mut gateway,&fence,&server(&request,wire::ServerReply::Quote{intent},None,None));
        sink.0.lock().unwrap().ready_pending=true;sink.0.lock().unwrap().flush_pending=true;let state=sink.0.clone();let mut budget=ManualNativeWriteBudget::new(10);let clock=budget.clone();let mut cx=Context::from_waker(futures_util::task::noop_waker_ref());
        let mut future=Box::pin(gateway.purchase_if_ready_with_budget(&mut sink,&mut budget));assert!(future.as_mut().poll(&mut cx).is_pending());clock.advance_to(9);state.lock().unwrap().ready_pending=false;
        assert!(future.as_mut().poll(&mut cx).is_pending());assert_eq!(state.lock().unwrap().frames.len(),2);clock.advance_to(10);
        assert!(matches!(future.as_mut().poll(&mut cx),Poll::Ready(Some(NativeSinkCommit::Unknown(_)))));drop(future);
        assert_eq!(gateway.client.pending_current().unwrap().phase,mir2_client_core::npc_purchase_receipt::PurchasePhase::Unknown);assert!(gateway.entered_ui.is_empty());
        assert!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap().is_none());assert_eq!(state.lock().unwrap().frames.len(),2);assert!(fence.0.lock().unwrap().waiters.is_empty());
    }
    #[test]
    fn native_npc_gateway_transport_control_expiry_has_no_ui_claim_or_purchase() {
        for after_entry in [false,true] {
            let (mut gateway,sender,mut receiver,fence,stamp,_world)=ready();let ui=ui_gate(&fence,stamp);
            let owned=owned_buy(&sender,&mut receiver,&fence,stamp,&ui);let sequence=owned.sequence;let (quote,quote_stamp)=gateway.quote(owned).unwrap();let mut sink=LocalSink::default();let state=sink.0.clone();
            state.lock().unwrap().ready_pending=!after_entry;state.lock().unwrap().flush_pending=after_entry;
            let mut budget=ManualNativeWriteBudget::new(10);let clock=budget.clone();let mut cx=Context::from_waker(futures_util::task::noop_waker_ref());
            let mut future=Box::pin(commit_control_with_budget(&mut sink,&fence,quote_stamp,quote,&mut budget));assert!(future.as_mut().poll(&mut cx).is_pending());clock.advance_to(10);
            let result=future.as_mut().poll(&mut cx);if after_entry{assert!(matches!(result,Poll::Ready(NativeSinkCommit::Unknown(_))));}else{assert!(matches!(result,Poll::Ready(NativeSinkCommit::Unavailable(_))));}drop(future);
            assert_eq!(state.lock().unwrap().frames.len(),usize::from(after_entry));assert!(gateway.client.pending_current().is_none());assert!(gateway.entered_ui.is_empty());
            assert!(fence.0.lock().unwrap().outstanding.contains_key(&sequence));assert!(fence.0.lock().unwrap().waiters.is_empty());assert_eq!(ui.feedback().phase,Some(NpcGoldBuyAttemptPhase::Bound));
            gateway.disconnect();assert!(!ui.feedback().pending);assert!(!fence.0.lock().unwrap().outstanding.contains_key(&sequence));
        }
    }
    #[test]
    fn native_npc_gateway_raw_marker_catches_escaped_and_duplicate_type_before_decoder() {
        for raw in [r#"{"type":"npcPurchaseOwner"}"#,r#"{"ty\u0070e":"npcPurchaseOwner"}"#,
            r#"{"type":"npcPurchaseOwner","type":"worldSnapshot","payload":{}}"#,
            r#"{"type":"worldSnapshot","ty\u0070e":"npcPurchaseOwner"}"#] {assert!(claims_owner_frame(raw));}
        assert!(!claims_owner_frame(r#"{"type":"worldSnapshot","payload":{"text":"npcPurchaseOwner"}}"#));
    }
    #[test]
    fn native_npc_gateway_begin_pair_and_enqueue_never_publish_applied() {
        let (_,_,_,fence,stamp,_) = ready();let mut gateway=NativeNpcPurchaseGateway::new().unwrap();gateway.open_connection().unwrap();
        assert!(gateway.begin_if_ready(Some(&fence),Some(stamp),ConnectionPhase::Normal,true,Some("0")).unwrap().is_none());
        let bundles=begin_local(&mut gateway,&fence,stamp,2,10);
        assert!(gateway.applied.is_none());assert!(gateway.gate.as_ref().unwrap().try_recv_applied().is_none());
        assert!(gateway.recovery_if_ready().unwrap().is_none());
        let mut world=bevy::prelude::World::new();apply_local(&mut world,&mut gateway,&fence,bundles);
        assert_eq!(gateway.applied.unwrap().server_revision,10);
        assert_eq!(world.resource::<mir2_client_bevy::inventory::InventoryModel>().gold,90);
        assert!(world.resource::<mir2_bevy_runtime::npc_purchase_economy::NativeNpcEconomySource>().owner().get("nativeNpcShop").is_some());
    }
    #[test]
    fn native_npc_gateway_readonly_quote_retains_ui_proof_and_exact_settlement_allows_repeat() {
        let (mut gateway,sender,mut receiver,fence,stamp,mut world)=ready();let ui=ui_gate(&fence,stamp);let mut sink=LocalSink::default();
        let owned=owned_buy(&sender,&mut receiver,&fence,stamp,&ui);let sequence=owned.sequence;
        quote_local(&mut gateway,owned,&fence,&mut sink);
        assert!(fence.0.lock().unwrap().outstanding.contains_key(&sequence));assert_eq!(ui.feedback().phase,Some(NpcGoldBuyAttemptPhase::Bound));
        assert_eq!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap(),Some(NativeSinkCommit::Flushed));
        let purchase=wire::parse_client_request(&sink.0.lock().unwrap().frames[1]).unwrap();
        assert!(matches!(&purchase.action,wire::Action::Purchase {operation} if operation.intent.request.item_index.get()==0));
        assert!(!fence.0.lock().unwrap().outstanding.contains_key(&sequence));assert_eq!(ui.feedback().phase,Some(NpcGoldBuyAttemptPhase::Flushed));
        let bundles=receive_local(&mut gateway,&fence,&terminal(&purchase,2,11,Some(owner())));
        assert!(ui.feedback().pending);assert!(gateway.client.pending_current().is_some());
        apply_local(&mut world,&mut gateway,&fence,bundles);
        assert!(!ui.feedback().pending);assert!(gateway.client.pending_current().is_none());
        let next=owned_buy(&sender,&mut receiver,&fence,stamp,&ui);quote_local(&mut gateway,next,&fence,&mut sink);
        assert_eq!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap(),Some(NativeSinkCommit::Flushed));
        let second=wire::parse_client_request(&sink.0.lock().unwrap().frames[3]).unwrap();
        assert!(second.action.operation().unwrap().sequence.get()>purchase.action.operation().unwrap().sequence.get());
        assert!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap().is_none());
        assert_eq!(sink.0.lock().unwrap().frames.len(),4);
    }
    #[test]
    fn native_npc_gateway_readiness_scene_or_inventory_change_refuses_final_purchase() {
        for scene in [false,true] {
            let (mut gateway,sender,mut receiver,fence,stamp,_world)=ready();let ui=ui_gate(&fence,stamp);let mut sink=LocalSink::default();
            quote_local(&mut gateway,owned_buy(&sender,&mut receiver,&fence,stamp,&ui),&fence,&mut sink);
            sink.0.lock().unwrap().ready_pending=true;
            let mut future=Box::pin(purchase_local(&mut gateway,&mut sink));let mut cx=Context::from_waker(futures_util::task::noop_waker_ref());
            assert!(future.as_mut().poll(&mut cx).is_pending());
            if scene {fence.test_scene_boundary(0);}else{fence.0.lock().unwrap().npc_gold_buy.inventory.as_mut().unwrap().gold-=1;}
            assert_eq!(future.as_mut().poll(&mut cx),Poll::Ready(Some(NativeSinkCommit::DefinitelyUnsent)));drop(future);
            assert!(gateway.client.pending_current().is_none());assert!(!ui.feedback().pending);
            assert_eq!(sink.0.lock().unwrap().frames.len(),1);
        }
    }
    #[test]
    fn native_npc_gateway_unknown_reconnect_queries_original_without_purchase_retry() {
        let (mut gateway,sender,mut receiver,fence,stamp,mut world)=ready();let ui=ui_gate(&fence,stamp);let mut sink=LocalSink::default();
        quote_local(&mut gateway,owned_buy(&sender,&mut receiver,&fence,stamp,&ui),&fence,&mut sink);sink.0.lock().unwrap().start_error=true;
        assert!(matches!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap(),Some(NativeSinkCommit::Unknown(_))));
        let original=wire::parse_client_request(&sink.0.lock().unwrap().frames[1]).unwrap().action.operation().unwrap().clone();
        gateway.disconnect();gateway.open_connection().unwrap();let fresh=fence.test_reconnect(3,0);
        let bundles=begin_local(&mut gateway,&fence,fresh,3,10);apply_local(&mut world,&mut gateway,&fence,bundles);
        let (query,_)=gateway.recovery_if_ready().unwrap().unwrap();assert_eq!(query.request.action.operation(),Some(&original));assert!(matches!(&query.request.action,wire::Action::Query {..}));
        assert!(gateway.recovery_if_ready().unwrap().is_none());assert!(gateway.client.pending_current().is_some());
        let bundles=receive_local(&mut gateway,&fence,&terminal(&query.request,3,11,Some(owner())));apply_local(&mut world,&mut gateway,&fence,bundles);
        assert!(gateway.client.pending_current().is_none());assert_eq!(sink.0.lock().unwrap().frames.len(),2);
    }
    #[test]
    fn native_npc_gateway_used_sub_catalog_quotes_buy_panel_zero_and_enters_once() {
        let mut source=owner();source["nativeNpcShop"]["service"]=json!("BUYUSED");
        source["nativeNpcShop"]["panelType"]=json!(1);
        source["nativeNpcShop"]["list"][0]["unique_id"]=json!(u64::MAX);
        source["nativeNpcShop"]["list"][0]["count"]=json!(3);
        source["nativeNpcShop"]["list"][0]["is_shop_item"]=json!(false);
        let (sender,mut receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();let stamp=fence.test_world_ready(3,0);
        assert!(fence.prepare_hero_owner(stamp,&source).unwrap().is_some());
        let mut gateway=NativeNpcPurchaseGateway::new().unwrap();gateway.open_connection().unwrap();gateway.entry_allowed=true;gateway.needs_world=false;
        let (begin,_)=gateway.begin_if_ready(Some(&fence),Some(stamp),ConnectionPhase::Normal,true,Some("0")).unwrap().unwrap();
        let producer=producer(2,10);let bundles=receive_local(&mut gateway,&fence,&server(&begin.request,
            wire::ServerReply::Producer {producer:producer.clone()},Some(source),Some(producer)));
        let mut world=bevy::prelude::World::new();apply_local(&mut world,&mut gateway,&fence,bundles);
        assert_eq!((world.resource::<mir2_client_bevy::shop::ShopModel>().goods[0].panel_type,
            world.resource::<mir2_client_bevy::shop::ShopModel>().goods[0].stock),(1,3));
        sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::BuyItem {item_index:u64::MAX,count:3,panel_type:0}),Some(stamp)).unwrap();
        let owned=receiver.try_recv().unwrap().into_parts().1.unwrap();assert!(owned.npc_gold_buy.is_none());
        let (quote,quote_stamp)=gateway.quote(owned).unwrap();let quote_request=quote.request.clone();let mut sink=LocalSink::default();
        assert_eq!(control_local(&mut sink,&fence,quote_stamp,quote).now_or_never().unwrap(),NativeSinkCommit::Flushed);
        let wire::Action::Quote {request}=quote_request.action.clone() else{panic!("read-only quote")};
        let intent=wire::Intent {request,currency:wire::Currency::Gold,source:wire::Source::Used,service_catalog_proof:opaque(7)};
        for invalid in 0..5 {
            let mut wrong=intent.clone();match invalid {0=>wrong.source=wire::Source::Trade,1=>wrong.source=wire::Source::BuyBack,
                2=>wrong.request.panel_type=1,3=>wrong.currency=wire::Currency::Pearls,_=>wrong.request.item_index=wire::U64::new(u64::MAX-1)};
            assert!(!gateway.held.as_ref().unwrap().source.allows_intent(&wrong));
        }
        receive_local(&mut gateway,&fence,&server(&quote_request,wire::ServerReply::Quote {intent:intent.clone()},None,None));
        assert_eq!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap(),Some(NativeSinkCommit::Flushed));
        let purchase=wire::parse_client_request(&sink.0.lock().unwrap().frames[1]).unwrap();
        assert_eq!(purchase.action.operation().unwrap().intent,intent);
        assert_eq!(purchase.action.operation().unwrap().intent.request.panel_type,0);
        assert!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap().is_none());assert_eq!(sink.0.lock().unwrap().frames.len(),2);
    }
    #[test]
    fn native_npc_gateway_healthy_unknown_queries_original_once_without_purchase_retry() {
        let (mut gateway,sender,mut receiver,fence,stamp,_world)=ready();let ui=ui_gate(&fence,stamp);let mut sink=LocalSink::default();
        quote_local(&mut gateway,owned_buy(&sender,&mut receiver,&fence,stamp,&ui),&fence,&mut sink);
        assert_eq!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap(),Some(NativeSinkCommit::Flushed));
        assert!(gateway.recovery_if_ready().unwrap().is_none());
        let purchase=wire::parse_client_request(&sink.0.lock().unwrap().frames[1]).unwrap();
        receive_local(&mut gateway,&fence,&server(&purchase,wire::ServerReply::Failure {state:wire::FailureState::Unknown,receipt:None},None,None));
        let (query,_)=gateway.recovery_if_ready().unwrap().unwrap();
        assert_eq!(query.request.action.operation(),purchase.action.operation());
        receive_local(&mut gateway,&fence,&server(&query.request,wire::ServerReply::Recovery {receipt:None},None,None));
        assert!(gateway.recovery_if_ready().unwrap().is_none());assert!(ui.feedback().pending);
        assert_eq!(sink.0.lock().unwrap().frames.len(),2);
    }
    #[test]
    fn native_npc_gateway_dropped_flush_keeps_original_ui_and_durable_unknown_custody() {
        let (mut gateway,sender,mut receiver,fence,stamp,_world)=ready();let ui=ui_gate(&fence,stamp);let mut sink=LocalSink::default();
        quote_local(&mut gateway,owned_buy(&sender,&mut receiver,&fence,stamp,&ui),&fence,&mut sink);sink.0.lock().unwrap().flush_pending=true;
        {let mut future=Box::pin(purchase_local(&mut gateway,&mut sink));let mut cx=Context::from_waker(futures_util::task::noop_waker_ref());assert!(future.as_mut().poll(&mut cx).is_pending());}
        assert_eq!(gateway.client.pending_current().unwrap().phase,mir2_client_core::npc_purchase_receipt::PurchasePhase::Unknown);
        assert_eq!(ui.feedback().phase,Some(NpcGoldBuyAttemptPhase::Unknown));assert_eq!(gateway.entered_ui.len(),1);
        assert!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap().is_none());assert_eq!(sink.0.lock().unwrap().frames.len(),2);
    }
    #[test]
    fn native_npc_gateway_pending_entry_retires_bundle_before_world_and_refuses_late_owner() {
        let (_,sender,_receiver,fence,stamp,_) = ready();let mut gateway=NativeNpcPurchaseGateway::new().unwrap();gateway.open_connection().unwrap();
        let bundles=begin_local(&mut gateway,&fence,stamp,2,10);let gate=gateway.gate.clone().unwrap();
        sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::StartGame {character_index:4}),Some(stamp)).unwrap();
        assert!(!gate.is_current(gateway.client.current_binding().unwrap()));
        let mut world=bevy::prelude::World::new();for bundle in bundles {assert!(!apply_native_npc_economy_bundle(&mut world,bundle));}
        assert!(world.get_resource::<mir2_client_bevy::inventory::InventoryModel>().is_none());
        gateway.drain_applied(&fence).unwrap();assert!(gateway.applied.is_none());
    }
    #[test]
    fn native_npc_gateway_wrong_durable_tuple_cannot_release_original_ui_proof() {
        use mir2_client_core::npc_purchase_receipt::{PurchaseSettlement,EconomicResult,ActorKey,OwnerScope,RequestId,PurchaseCurrency,PurchaseSource,ServiceCatalogProof};
        let (mut gateway,sender,mut receiver,fence,stamp,_world)=ready();let ui=ui_gate(&fence,stamp);let mut sink=LocalSink::default();
        quote_local(&mut gateway,owned_buy(&sender,&mut receiver,&fence,stamp,&ui),&fence,&mut sink);
        assert_eq!(purchase_local(&mut gateway,&mut sink).now_or_never().unwrap(),Some(NativeSinkCommit::Flushed));
        let pending=gateway.client.pending_current().unwrap();
        let original=PurchaseSettlement {key:pending.key,intent:pending.intent,result:EconomicResult::Rejected {server_revision:11}};
        for field in 0..9 {
            let mut wrong=original;
            match field {
                0=>wrong.key.actor=ActorKey::from_server_bytes([9;32]).unwrap(),
                1=>wrong.key.request_id=RequestId::from_parts(OwnerScope::from_server_bytes([9;32]).unwrap(),wrong.key.request_id.sequence()).unwrap(),
                2=>wrong.key.request_id=RequestId::from_parts(wrong.key.request_id.scope(),wrong.key.request_id.sequence()+1).unwrap(),
                3=>wrong.intent.item_index+=1,4=>wrong.intent.requested_count+=1,5=>wrong.intent.panel+=1,
                6=>wrong.intent.currency=PurchaseCurrency::Pearls,7=>wrong.intent.source=PurchaseSource::Used,
                _=>wrong.intent.service_catalog_proof=ServiceCatalogProof::from_server_bytes([9;32]).unwrap(),
            }
            gateway.observe(Observation::Settled(wrong));assert!(ui.feedback().pending);assert_eq!(gateway.entered_ui.len(),1);
        }
    }
    #[test]
    fn native_npc_gateway_malformed_owner_never_consumes_original_begin_control() {
        let (sender,_receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();let stamp=fence.test_world_ready(3,0);
        let mut gateway=NativeNpcPurchaseGateway::new().unwrap();gateway.open_connection().unwrap();gateway.entry_allowed=true;gateway.needs_world=false;
        assert!(fence.prepare_hero_owner(stamp,&owner()).unwrap().is_some());
        let (dispatch,_)=gateway.begin_if_ready(Some(&fence),Some(stamp),ConnectionPhase::Normal,true,Some("0")).unwrap().unwrap();
        let producer=producer(2,10);let raw=server(&dispatch.request,wire::ServerReply::Producer {producer:producer.clone()},Some(owner()),Some(producer));
        let malformed=raw.replacen("\"protocolVersion\":1","\"protocolVersion\":1,\"protocolVersion\":1",1);assert_ne!(malformed,raw);
        assert!(gateway.receive_with(&malformed,&fence,|_|panic!("invalid raw frame cannot enqueue")).is_err());
        assert!(gateway.client.current_binding().is_none());assert_eq!(receive_local(&mut gateway,&fence,&raw).len(),1);
    }
    #[test]
    fn native_npc_gateway_receipt_hero_gate_requires_current_unretired_owner_lifetime() {
        for invalid in [0,1,2] {
            let (sender,_receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();let stamp=fence.test_world_ready(3,0);
            assert!(fence.prepare_hero_owner(stamp,&owner()).unwrap().is_some());
            let mut gateway=NativeNpcPurchaseGateway::new().unwrap();gateway.open_connection().unwrap();gateway.entry_allowed=true;gateway.needs_world=false;
            let (dispatch,_)=gateway.begin_if_ready(Some(&fence),Some(stamp),ConnectionPhase::Normal,true,Some("0")).unwrap().unwrap();
            if invalid==1 {fence.0.lock().unwrap().hero_owner_gate.as_ref().unwrap().1.retire();}
            if invalid==2 {fence.0.lock().unwrap().hero_owner_gate.as_mut().unwrap().0.scene_epoch+=1;}
            let producer=producer(2,10);let raw=server(&dispatch.request,wire::ServerReply::Producer {producer:producer.clone()},Some(owner()),Some(producer));
            if invalid==0 {
                let bundles=receive_local(&mut gateway,&fence,&raw);assert_eq!(bundles.len(),1);
                assert!(gateway.applied.is_none());let mut world=bevy::prelude::World::new();apply_local(&mut world,&mut gateway,&fence,bundles);
                assert_eq!(gateway.applied.unwrap().server_revision,10);
            } else {
                assert!(gateway.receive_with(&raw,&fence,|_|panic!("retired or mismatched Hero lifetime cannot enqueue")).is_err());
                assert!(gateway.sources.is_empty());assert!(gateway.applied.is_none());
            }
        }
    }
    #[test]
    fn native_npc_gateway_ordinary_hero_source_uses_owner_lifetime_without_begin() {
        let (sender,_receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();let stamp=fence.test_world_ready(3,0);
        let gateway=NativeNpcPurchaseGateway::new().unwrap();let owner=owner();
        assert!(fence.prepare_hero_owner(stamp,&owner).unwrap().is_some());
        let gate=fence.0.lock().unwrap().hero_owner_gate.as_ref().unwrap().1.clone();
        assert!(gateway.client.current_binding().is_none());assert!(gateway.applied.is_none());
        sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::StartGame {character_index:4}),Some(stamp)).unwrap();
        assert!(gate.prepare(&owner).is_err());assert!(fence.prepare_hero_owner(stamp,&owner).unwrap().is_none());
        assert!(gateway.applied.is_none());
    }
    #[test]
    fn native_npc_gateway_store_parent_survives_dismissed_main_text_dialog() {
        let (_gateway,_sender,_receiver,fence,stamp,_world)=ready();let mut source=owner();assert!(source["activeNpcDialog"].is_null());
        fence.begin_npc_gold_buy_snapshot(Some(&source));
        {let state=fence.0.lock().unwrap();assert!(state.npc_gold_buy.shop_ready);assert!(state.npc_gold_buy.shop.allows_buy());assert_eq!(state.npc_gold_buy.shop.goods.len(),1);}
        source["nativeNpcShop"]=Value::Null;
        source["activeNpcDialog"]=json!({"npcObjectId":4990,"body":"visible text is not a buying service"});
        fence.begin_npc_gold_buy_snapshot(Some(&source));
        assert!(!fence.0.lock().unwrap().npc_gold_buy.shop_ready);assert!(fence.npc_gold_buy_source().is_none());assert!(fence.accepts(stamp,true));
    }
}

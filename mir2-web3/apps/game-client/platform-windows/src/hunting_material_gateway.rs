//! Physical material owner. Metadata, inventory custody and Runtime completion
//! are independent from the NPC repair advertisement and selection gate.
use super::*;
use crate::hunting_material_client::{
    MaterialActor, MaterialApplicationCandidate, MaterialConnection, MaterialDispatch,
    MaterialInventoryCandidate, MaterialPhase, MaterialSource, MaterialSourceTracker,
    NativeHuntingMaterialClient,
};
use crate::hunting_material_connection::{HuntingMaterialConnection, MaterialIdentityEvidence};
use mir2_bevy_runtime::hunting_material_economy::{
    NativeHuntingMaterialApplied, NativeHuntingMaterialEpoch, NativeHuntingMaterialGate,
    NativeHuntingMaterialOwner,
};
use mir2_bevy_runtime::npc_purchase_economy::NativeNpcEconomyProjection;
use mir2_client_bevy::hunting_material::{
    HuntingMaterialActorScope, HuntingMaterialModel, HuntingMaterialPhase,
};
use mir2_client_bevy::inventory::InventoryModel;
use mir2_client_wire::hunting_material as wire;
use std::pin::Pin;

#[derive(Debug, Default)]
pub(super) struct MaterialSourceState {
    pub(super) connection: Option<MaterialConnection>,
    tracker: MaterialSourceTracker,
}
impl MaterialSourceState {
    pub(super) fn retire(&mut self) {
        self.tracker.invalidate();
        self.connection = None;
    }
    pub(super) fn invalidate(&self) {
        self.tracker.invalidate();
    }
    pub(super) fn source_for_actual(
        &self,
        stamp: NativeCommandStamp,
        actual: &InventoryModel,
        uid: u64,
        quantity: u64,
    ) -> Option<MaterialSource> {
        let connection = self.connection.as_ref().filter(|c| c.owner() == stamp)?;
        self.tracker
            .source_for_actual(connection, actual, uid, quantity)
            .ok()
    }
}
pub(super) struct MaterialProducerCandidate {
    stamp: NativeCommandStamp,
    candidate: MaterialInventoryCandidate,
}
impl NativeCommandFence {
    pub(crate) fn hunting_material_source(
        &self,
        actual: &InventoryModel,
        uid: u64,
        quantity: u64,
    ) -> Option<MaterialSource> {
        let state = self.0.lock().ok()?;
        if state.npc_entry_pending
            || !Self::matches(&state, state.current, NativeCommandScope::World)
        {
            return None;
        }
        state
            .hunting_material_source
            .source_for_actual(state.current, actual, uid, quantity)
    }
    /// Called before this exact complete producer enters any inventory queue.
    pub(super) fn stage_hunting_material_inventory(
        &self,
        value: &Value,
        raw_complete: bool,
    ) -> Option<MaterialProducerCandidate> {
        let state = self.0.lock().ok()?;
        let source = &state.hunting_material_source;
        if !raw_complete
            || state.npc_entry_pending
            || !Self::matches(&state, state.current, NativeCommandScope::World)
        {
            source.invalidate();
            return None;
        }
        let connection = source.connection.as_ref()?;
        if connection.owner() != state.current {
            source.invalidate();
            return None;
        }
        let Ok(inventory) = serde_json::from_value::<InventoryModel>(value.clone()) else {
            source.invalidate();
            return None;
        };
        source
            .tracker
            .stage_inventory(connection, &inventory)
            .ok()
            .map(|candidate| MaterialProducerCandidate {
                stamp: state.current,
                candidate,
            })
    }
    /// This consumes the private ticket staged before the successful producer
    /// send; partial queues and a later reconstructed frame cannot publish it.
    pub(super) fn publish_hunting_material_inventory(
        &self,
        staged: MaterialProducerCandidate,
        producer_stamp: NativeCommandStamp,
    ) {
        if let Ok(state) = self.0.lock() {
            if staged.stamp == producer_stamp
                && !state.npc_entry_pending
                && Self::matches(&state, producer_stamp, NativeCommandScope::World)
                && state
                    .hunting_material_source
                    .connection
                    .as_ref()
                    .is_some_and(|c| c.owner() == producer_stamp)
            {
                let _ = state
                    .hunting_material_source
                    .tracker
                    .publish_inventory(staged.candidate);
            } else {
                state.hunting_material_source.invalidate();
            }
        }
    }
}

pub(super) fn claims_owner_frame(raw: &str) -> bool {
    struct Marker;
    impl<'de> Deserialize<'de> for Marker {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Marker;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("material envelope")
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<Marker, A::Error> {
                    while let Some(key) = map.next_key::<String>()? {
                        if key == "type" {
                            if map.next_value::<Value>()?.as_str() == Some("huntingMaterialOwner") {
                                return Err(serde::de::Error::custom(
                                    "native material owner marker",
                                ));
                            }
                        } else {
                            let _ = map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                    Ok(Marker)
                }
            }
            d.deserialize_map(Visitor)
        }
    }
    serde_json::from_str::<Marker>(raw)
        .is_err_and(|e| e.to_string().starts_with("native material owner marker"))
}

struct PendingSource {
    candidate: MaterialApplicationCandidate,
    projection: NativeNpcEconomyProjection,
}
pub(super) struct NativeHuntingMaterialGateway {
    client: NativeHuntingMaterialClient,
    connection: HuntingMaterialConnection,
    binding: Option<MaterialConnection>,
    identity: Option<MaterialIdentityEvidence>,
    binding_fence: Option<NativeCommandFence>,
    gate: Option<NativeHuntingMaterialGate>,
    sources: VecDeque<PendingSource>,
    purchase_suspended: bool,
    confirm_prepared: bool,
    notice: Option<&'static str>,
    last_model: Option<HuntingMaterialModel>,
    // A real delivery acknowledges UI input even when its value is unchanged.
    // Backpressure must preserve that obligation until admission succeeds.
    publish_pending: bool,
}
impl NativeHuntingMaterialGateway {
    pub(super) fn new() -> Self {
        Self {
            client: NativeHuntingMaterialClient::new(),
            connection: HuntingMaterialConnection::new(),
            binding: None,
            identity: None,
            binding_fence: None,
            gate: None,
            sources: VecDeque::new(),
            purchase_suspended: false,
            confirm_prepared: false,
            notice: None,
            last_model: None,
            publish_pending: false,
        }
    }
    pub(super) fn open_connection(&mut self) {
        self.disconnect();
        self.connection = HuntingMaterialConnection::new();
        self.publish(true);
    }
    pub(super) fn disconnect(&mut self) {
        self.withdraw();
        self.connection.clear_identity();
        self.purchase_suspended = false;
        self.last_model = None;
        self.publish(true);
    }
    fn withdraw(&mut self) {
        if let Some(gate) = self.gate.take() {
            gate.retire();
        }
        if let Some(fence) = self.binding_fence.take() {
            if let Ok(mut state) = fence.0.lock() {
                state.hunting_material_source.retire();
                state.hunting_material_busy = false;
                if let Some(gate) = state.hunting_material_gate.take() {
                    gate.retire();
                }
            }
        }
        self.client.withdraw();
        self.binding = None;
        self.identity = None;
        self.sources.clear();
        self.confirm_prepared = false;
        self.notice = None;
    }
    pub(super) fn reset_entry(&mut self) {
        self.withdraw();
        self.purchase_suspended = false;
        self.publish(true);
    }
    pub(super) fn clear_identity_and_leave(&mut self) {
        self.connection.clear_identity();
        self.reset_entry();
    }
    pub(super) fn before_ordinary_frame(&mut self, raw: &str) {
        if let Ok(envelope) = serde_json::from_str::<GatewayEnvelope>(raw) {
            match envelope.packet.as_deref() {
                Some("LogOutSuccess") => self.reset_entry(),
                Some("ReturnToLogin" | "Disconnect") => self.clear_identity_and_leave(),
                _ => {}
            }
        }
    }
    pub(super) fn observe_connection(&mut self, raw: &str) -> Result<bool, String> {
        let observed = self.connection.observe(raw, gateway_unix_ms());
        if observed.as_ref().is_ok_and(|v| *v) || observed.is_err() {
            if self
                .identity
                .as_ref()
                .is_some_and(|e| !self.connection.matches(e, gateway_unix_ms()))
            {
                self.withdraw();
            }
            self.publish(true);
        }
        observed.map_err(str::to_owned)
    }
    fn identity_current(&self) -> bool {
        self.identity
            .as_ref()
            .is_some_and(|e| self.connection.matches(e, gateway_unix_ms()))
    }
    pub(super) fn blocks_purchase(&self) -> bool {
        self.client.model().pending
            || self.confirm_prepared
            || self.binding.is_none() && self.purchase_suspended
    }
    pub(super) fn purchase_suspended(&self) -> bool {
        self.purchase_suspended
    }
    pub(super) fn retire_idle_for_purchase(
        &mut self,
        purchases: &mut npc_purchase_gateway::NativeNpcPurchaseGateway,
    ) {
        if !self.blocks_purchase() {
            if let Some(binding) = &self.binding {
                self.client.close_view(binding.owner());
            }
            self.purchase_suspended = false;
            purchases.resume_after_repair();
            self.publish(true);
        }
    }
    fn model(&self) -> HuntingMaterialModel {
        let mut model = HuntingMaterialModel {
            protocol_v1: self.connection.supports_protocol_v1(),
            ..Default::default()
        };
        if let Some(binding) = &self.binding {
            let actor = binding.actor();
            model.actor_scope = HuntingMaterialActorScope::new(
                actor.realm().into(),
                actor.account_id().into(),
                actor.character_index(),
            )
            .ok();
            let state = self.client.model();
            model.phase = if self.confirm_prepared {
                HuntingMaterialPhase::Pending
            } else {
                match state.phase {
                    MaterialPhase::Ready => HuntingMaterialPhase::Ready,
                    MaterialPhase::Quoting => HuntingMaterialPhase::Quoting,
                    MaterialPhase::Quoted => HuntingMaterialPhase::Quoted,
                    MaterialPhase::Pending => HuntingMaterialPhase::Pending,
                    MaterialPhase::Complete => HuntingMaterialPhase::Committed,
                    MaterialPhase::Rejected => HuntingMaterialPhase::Rejected,
                }
            };
            model.quote_id = state.quote_id;
            model.reload_required = state.reload_required;
            model.notice = state.notice;
            if let Some(q) = state.quote {
                model.quote_id = Some(q.quote_id);
                model.unique_id = Some(q.unique_id.get());
                model.input_count = Some(q.input_count.get());
                model.quantity = Some(q.quantity.get());
                model.unit_gold = Some(q.unit_gold.get());
                model.gold_equivalent = Some(q.gold_equivalent.get());
                model.fee_unbound_gold = Some(q.fee_unbound_gold.get());
                model.license_atoms_debit = Some(q.license_atoms_debit.get());
                model.valuation_version = Some(q.valuation_version);
                model.created_at_ms = Some(q.created_at_ms);
                model.expires_at_ms = Some(q.expires_at_ms);
            }
        }
        if let Some(notice) = self.notice {
            model.notice = Some(notice.into());
            if !self.blocks_purchase() {
                model.phase = HuntingMaterialPhase::Rejected;
            }
        }
        model
    }
    fn publish(&mut self, force: bool) {
        self.publish_pending |= force;
        let model = self.model();
        if !self.publish_pending && self.last_model.as_ref() == Some(&model) {
            return;
        }
        self.publish_pending = true;
        let guard = if let Some(binding) = &self.binding {
            let Some(fence) = self.binding_fence.as_ref() else {
                return;
            };
            let Ok(state) = fence.0.lock() else {
                return;
            };
            if !NativeCommandFence::matches(&state, binding.owner(), NativeCommandScope::World) {
                return;
            }
            Some(state)
        } else {
            None
        };
        if serde_json::to_string(&model)
            .ok()
            .is_some_and(mir2_bevy_runtime::native_ingest::push_native_hunting_material_model)
        {
            self.last_model = Some(model);
            self.publish_pending = false;
        }
        drop(guard);
    }
    pub(super) fn tick(
        &mut self,
        context: &GatewaySessionContext,
        fence: &NativeCommandFence,
        stamp: Option<NativeCommandStamp>,
        phase: ConnectionPhase,
        bootstrapped: bool,
        purchases: &mut npc_purchase_gateway::NativeNpcPurchaseGateway,
    ) -> Result<(), String> {
        let evidence = self.connection.identity(gateway_unix_ms());
        let active = stamp.filter(|stamp| {
            phase == ConnectionPhase::Normal
                && bootstrapped
                && evidence.as_ref().is_some_and(|e| {
                    context
                        .account_id
                        .as_deref()
                        .is_some_and(|a| a.eq_ignore_ascii_case(e.account_id()))
                })
                && fence.0.lock().is_ok_and(|s| {
                    !s.npc_entry_pending
                        && NativeCommandFence::matches(&s, *stamp, NativeCommandScope::World)
                })
        });
        let actor = active
            .zip(context.character_index)
            .zip(evidence.as_ref())
            .and_then(|((stamp, index), e)| {
                MaterialActor::new(e.realm_id().into(), e.account_id().into(), index)
                    .ok()
                    .map(|actor| (stamp, actor))
            });
        let same = self
            .binding
            .as_ref()
            .zip(actor.as_ref())
            .is_some_and(|(c, (s, a))| c.owner() == *s && c.actor() == a)
            && self.identity_current();
        if !same && (self.binding.is_some() || actor.is_some()) {
            self.withdraw();
            if let Some((stamp, actor)) = actor {
                let connection = MaterialConnection::from_verified_reader(
                    stamp,
                    actor.clone(),
                    wire::PROTOCOL_VERSION,
                    fence,
                )
                .map_err(str::to_owned)?;
                let owner = runtime_owner(&actor)?;
                let gate = NativeHuntingMaterialGate::new(
                    epoch(stamp).ok_or("Material owner missing")?,
                    owner,
                )
                .map_err(|_| "Material gate unavailable")?;
                let mut state = fence
                    .0
                    .lock()
                    .map_err(|_| "Material owner fence poisoned")?;
                if !state.npc_entry_pending
                    && NativeCommandFence::matches(&state, stamp, NativeCommandScope::World)
                {
                    self.client
                        .bind(connection.clone())
                        .map_err(str::to_owned)?;
                    state.hunting_material_source.retire();
                    state.hunting_material_source.connection = Some(connection.clone());
                    if let Some(prior) = state.hunting_material_gate.replace(gate.clone()) {
                        prior.retire();
                    }
                    self.binding = Some(connection);
                    self.identity = evidence;
                    self.binding_fence = Some(fence.clone());
                    self.gate = Some(gate);
                }
            }
            self.publish(true);
        }
        if self.client.check_timeout(gateway_unix_ms()) {
            self.confirm_prepared = false;
        }
        self.drain_applied(fence)?;
        if let Ok(mut state) = fence.0.lock() {
            state.hunting_material_busy = self.blocks_purchase();
        }
        if self.purchase_suspended
            && !self.blocks_purchase()
            && self.client.model().phase == MaterialPhase::Complete
        {
            self.purchase_suspended = false;
            purchases.resume_after_repair();
        }
        self.publish(false);
        Ok(())
    }
    fn reject(&mut self, notice: &'static str) {
        self.notice = Some(notice);
        self.publish(true);
    }
    pub(super) fn prepare(
        &mut self,
        owned: &OwnedGatewayCommand,
        purchases: &mut npc_purchase_gateway::NativeNpcPurchaseGateway,
    ) -> Option<MaterialDispatch> {
        let binding = self.binding.clone()?;
        let stamp = binding.owner();
        let GatewayCommand::HuntingMaterial(action) = &owned.command else {
            return None;
        };
        if owned.stamp != stamp || !self.identity_current() {
            self.reject("ownerChanged");
            return None;
        }
        if matches!(action, HuntingMaterialCommand::Close) {
            self.client.close_view(stamp);
            self.notice = None;
            if !self.blocks_purchase() {
                self.purchase_suspended = false;
                purchases.resume_after_repair();
            }
            self.publish(true);
            return None;
        }
        let query = matches!(action, HuntingMaterialCommand::Query { .. });
        let valid = owned.fence.0.lock().is_ok_and(|s| {
            s.outstanding.get(&owned.sequence) == Some(&stamp)
                && NativeCommandFence::matches(&s, stamp, NativeCommandScope::World)
                && !s.npc_entry_pending
                && (query || !s.hunting_repair_busy)
                && Self::source_matches(owned)
        });
        if !valid {
            self.reject("itemOrOwnerChanged");
            return None;
        }
        // Original Query is read-only recovery. It neither acquires a charge
        // lane nor retires a different lane's retained Pending operation.
        if !query && !self.purchase_suspended {
            if !purchases.suspend_for_repair() {
                self.reject("purchasePending");
                return None;
            }
            self.purchase_suspended = true;
        }
        let owner = runtime_owner(binding.actor()).ok()?;
        let e = epoch(stamp)?;
        if self.gate.as_ref().is_none_or(|g| !g.is_current(e, &owner)) {
            let gate = NativeHuntingMaterialGate::new(e, owner).ok()?;
            let mut state = owned.fence.0.lock().ok()?;
            if !NativeCommandFence::matches(&state, stamp, NativeCommandScope::World)
                || state.npc_entry_pending
            {
                return None;
            }
            if let Some(prior) = state.hunting_material_gate.replace(gate.clone()) {
                prior.retire();
            }
            self.gate = Some(gate);
            self.sources.clear();
        }
        let now = gateway_unix_ms();
        let result = match action {
            HuntingMaterialCommand::Quote {
                source: Some(source),
                ..
            } => self.client.prepare_quote(source, now),
            HuntingMaterialCommand::Confirm {
                quote_id,
                source: Some(source),
            } => self.client.prepare_confirm(stamp, quote_id, source, now),
            HuntingMaterialCommand::Query { quote_id } => {
                self.client.prepare_query(stamp, quote_id, now)
            }
            _ => return None,
        };
        match result {
            Ok(dispatch) => {
                self.confirm_prepared =
                    matches!(dispatch.request.action, wire::Action::Confirm { .. });
                self.notice = None;
                if let Ok(mut state) = owned.fence.0.lock() {
                    state.hunting_material_busy = self.blocks_purchase();
                }
                self.publish(true);
                Some(dispatch)
            }
            Err(_) => {
                self.reject("materialControlUnavailable");
                None
            }
        }
    }
    fn source_matches(owned: &OwnedGatewayCommand) -> bool {
        match &owned.command {
            GatewayCommand::HuntingMaterial(HuntingMaterialCommand::Quote {
                unique_id,
                quantity,
                source: Some(source),
            }) => {
                source.owner() == owned.stamp
                    && source.unique_id() == *unique_id
                    && source.quantity() == *quantity
                    && source.current()
            }
            GatewayCommand::HuntingMaterial(HuntingMaterialCommand::Confirm {
                source: Some(source),
                ..
            }) => source.owner() == owned.stamp && source.current(),
            GatewayCommand::HuntingMaterial(HuntingMaterialCommand::Query { .. }) => true,
            _ => false,
        }
    }
    fn final_valid(&self, state: &NativeCommandFenceState, owned: &OwnedGatewayCommand) -> bool {
        state.outstanding.get(&owned.sequence) == Some(&owned.stamp)
            && !state.npc_entry_pending
            && NativeCommandFence::matches(state, owned.stamp, NativeCommandScope::World)
            && self.identity_current()
            && self
                .binding
                .as_ref()
                .is_some_and(|c| c.owner() == owned.stamp)
            && (matches!(
                owned.command,
                GatewayCommand::HuntingMaterial(HuntingMaterialCommand::Query { .. })
            ) || !state.hunting_repair_busy)
            && Self::source_matches(owned)
    }
    pub(super) async fn send<S>(
        &mut self,
        sink: &mut S,
        owned: &OwnedGatewayCommand,
        dispatch: MaterialDispatch,
    ) -> NativeSinkCommit
    where
        S: futures_util::Sink<Message> + Unpin,
        S::Error: std::fmt::Display,
    {
        let mut budget = TokioNativeWriteBudget::new();
        let outcome = self
            .send_with_budget(sink, owned, dispatch, &mut budget)
            .await;
        self.confirm_prepared = false;
        if let Ok(mut state) = owned.fence.0.lock() {
            state.hunting_material_busy = self.blocks_purchase();
        }
        self.publish(true);
        outcome
    }
    async fn send_with_budget<S>(
        &mut self,
        sink: &mut S,
        owned: &OwnedGatewayCommand,
        dispatch: MaterialDispatch,
        budget: &mut impl NativeWriteBudget,
    ) -> NativeSinkCommit
    where
        S: futures_util::Sink<Message> + Unpin,
        S::Error: std::fmt::Display,
    {
        struct Waiter<'a>(&'a OwnedGatewayCommand);
        impl Drop for Waiter<'_> {
            fn drop(&mut self) {
                if let Ok(mut s) = self.0.fence.0.lock() {
                    s.waiters.remove(&self.0.sequence);
                }
            }
        }
        let waiter = Waiter(owned);
        let ready = await_native_write_phase(budget, |cx| {
            let Ok(mut state) = owned.fence.0.lock() else {
                return std::task::Poll::Ready(None);
            };
            if !self.final_valid(&state, owned) {
                return std::task::Poll::Ready(None);
            }
            state.waiters.insert(owned.sequence, cx.waker().clone());
            drop(state);
            Pin::new(&mut *sink).poll_ready(cx).map(Some)
        })
        .await;
        drop(waiter);
        let failure = match ready {
            Err(_) => Some(NativeSinkCommit::Unavailable(
                NATIVE_TRANSPORT_TIMEOUT_ERROR.into(),
            )),
            Ok(None) => Some(NativeSinkCommit::DefinitelyUnsent),
            Ok(Some(Err(e))) => Some(NativeSinkCommit::Unavailable(e.to_string())),
            Ok(Some(Ok(()))) => None,
        };
        if let Some(failure) = failure {
            self.client
                .definitely_unsent(dispatch.request.request_id.get());
            owned.fence.retire(owned);
            return failure;
        }
        let start = {
            let Ok(mut state) = owned.fence.0.lock() else {
                self.client
                    .definitely_unsent(dispatch.request.request_id.get());
                return NativeSinkCommit::DefinitelyUnsent;
            };
            if !self.final_valid(&state, owned)
                || budget.expired()
                || self.client.enter(&dispatch, gateway_unix_ms()).is_err()
            {
                drop(state);
                self.client
                    .definitely_unsent(dispatch.request.request_id.get());
                owned.fence.retire(owned);
                return NativeSinkCommit::DefinitelyUnsent;
            }
            state.outstanding.remove(&owned.sequence);
            state.hunting_material_busy = self.blocks_purchase();
            // No await separates entered custody from the physical send.
            Pin::new(&mut *sink).start_send(Message::Text(dispatch.body.into()))
        };
        if let Err(e) = start {
            return NativeSinkCommit::Unknown(e.to_string());
        }
        match await_native_write_phase(budget, |cx| Pin::new(&mut *sink).poll_flush(cx)).await {
            Ok(Ok(())) => NativeSinkCommit::Flushed,
            Ok(Err(e)) => NativeSinkCommit::Unknown(e.to_string()),
            Err(_) => NativeSinkCommit::Unknown(NATIVE_TRANSPORT_TIMEOUT_ERROR.into()),
        }
    }
    pub(super) fn receive(&mut self, raw: &str, fence: &NativeCommandFence) -> Result<(), String> {
        self.receive_with(
            raw,
            fence,
            mir2_bevy_runtime::native_ingest::push_native_hunting_material_bundle,
        )
    }
    fn receive_with(
        &mut self,
        raw: &str,
        fence: &NativeCommandFence,
        mut enqueue: impl FnMut(
            mir2_bevy_runtime::hunting_material_economy::NativeHuntingMaterialBundle,
        ) -> bool,
    ) -> Result<(), String> {
        let binding = self.binding.clone().ok_or("Material reply has no owner")?;
        let stamp = binding.owner();
        if !self.identity_current() || !fence.accepts(stamp, true) {
            return Err("Material reply owner retired".into());
        }
        let received = self.client.receive(stamp, raw).map_err(str::to_owned)?;
        if let Some(candidate) = received.application_candidate {
            if self.sources.len() < 32 {
                if let (Some(gate), Some(quote), Some(receipt), Ok(projection)) = (
                    self.gate.as_ref(),
                    self.client.model().quote,
                    self.client.model().receipt,
                    crate::npc_purchase_projection::project_owner(candidate.snapshot()),
                ) {
                    let state = fence
                        .0
                        .lock()
                        .map_err(|_| "Material owner fence poisoned")?;
                    if self.identity_current()
                        && NativeCommandFence::matches(&state, stamp, NativeCommandScope::World)
                        && self.client.application_is_current(&candidate)
                    {
                        if let Ok(mut bundle) = gate.prepare(
                            candidate.request_id(),
                            &quote,
                            &receipt,
                            projection.clone(),
                        ) {
                            if let Some((saved, hero)) = &state.hero_owner_gate {
                                if *saved == stamp {
                                    bundle = match bundle.with_hero_owner_gate(hero) {
                                        Ok(b) => b,
                                        Err(_) => return Ok(()),
                                    };
                                }
                            }
                            if enqueue(bundle) {
                                self.sources.push_back(PendingSource {
                                    candidate,
                                    projection,
                                });
                            }
                        }
                    }
                }
            }
        }
        if let Ok(mut state) = fence.0.lock() {
            state.hunting_material_busy = self.blocks_purchase();
        }
        self.notice = None;
        self.publish(true);
        Ok(())
    }
    fn drain_applied(&mut self, fence: &NativeCommandFence) -> Result<(), String> {
        for _ in 0..32 {
            let Some(gate) = self.gate.clone() else {
                break;
            };
            let Some(applied) = gate.try_recv_applied() else {
                break;
            };
            self.complete_applied(fence, &gate, &applied)?;
        }
        Ok(())
    }
    fn complete_applied(
        &mut self,
        fence: &NativeCommandFence,
        gate: &NativeHuntingMaterialGate,
        applied: &NativeHuntingMaterialApplied,
    ) -> Result<bool, String> {
        let Some(binding) = self.binding.clone() else {
            return Ok(false);
        };
        let stamp = binding.owner();
        if !self.identity_current()
            || epoch(stamp) != Some(applied.epoch())
            || runtime_owner(binding.actor()).ok().as_ref() != Some(applied.owner())
        {
            return Ok(false);
        }
        let Some(index) = self.sources.iter().position(|s| {
            s.candidate.request_id() == applied.request_id()
                && s.candidate.quote_id() == applied.quote_id()
        }) else {
            return Ok(false);
        };
        let source = self.sources.remove(index).expect("paired material source");
        if !self.client.application_is_current(&source.candidate) {
            return Ok(false);
        }
        let mut state = fence
            .0
            .lock()
            .map_err(|_| "Material owner fence poisoned")?;
        if state.npc_entry_pending
            || !NativeCommandFence::matches(&state, stamp, NativeCommandScope::World)
        {
            return Ok(false);
        }
        let complete =
            self.identity_current() && self.client.applied(&source.candidate, gate, applied);
        // Gate custody is released before buffer admission; the outer
        // physical owner fence remains held throughout publication.
        if complete {
            state.hunting_material_source.invalidate();
            state.hunting_material_busy = false;
            npc_purchase_gateway::synchronize_applied_ui_source(
                &mut state,
                stamp,
                &source.projection,
            )?;
            self.notice = None;
            self.publish_pending = true;
            let model = self.model();
            if serde_json::to_string(&model)
                .ok()
                .is_some_and(mir2_bevy_runtime::native_ingest::push_native_hunting_material_model)
            {
                self.last_model = Some(model);
                self.publish_pending = false;
            }
        }
        Ok(complete)
    }
}
fn runtime_owner(actor: &MaterialActor) -> Result<NativeHuntingMaterialOwner, String> {
    NativeHuntingMaterialOwner::new(actor.realm(), actor.account_id(), actor.character_index())
        .map_err(|_| "Material owner unavailable".into())
}
fn epoch(s: NativeCommandStamp) -> Option<NativeHuntingMaterialEpoch> {
    Some(NativeHuntingMaterialEpoch {
        run: s.run,
        connection: s.connection,
        procedure: s.procedure,
        owner_epoch: s.owner_epoch,
        scene_epoch: s.scene_epoch,
        cancellation: s.cancellation,
        actor: s.actor?,
        map: s.map?,
    })
}

#[cfg(test)]
#[path = "hunting_material_gateway_tests.rs"]
mod tests;

//! Physical native owner integration for the independent repair carrier.
use super::*;
use crate::hunting_repair_client::{NativeHuntingRepairClient, RepairActor, RepairDispatch};
use crate::hunting_repair_connection::{HuntingRepairConnection, WalletResponse};
use mir2_bevy_runtime::hunting_repair_economy::{
    NativeHuntingRepairEpoch, NativeHuntingRepairGate,
};
use mir2_bevy_runtime::npc_purchase_economy::NativeNpcEconomyProjection;
use mir2_client_bevy::{
    hunting_repair::HuntingRepairModel,
    inventory::InventoryModel,
    shop::{NpcShopServiceMode, NpcShopServiceSignal},
};
use mir2_client_wire::hunting_repair as wire;
use sha2::{Digest, Sha256};
use std::pin::Pin;

#[derive(Debug)]
pub(super) struct RepairSourceState {
    stamp: Option<NativeCommandStamp>,
    revision: u64,
    inventory: Option<InventoryModel>,
    pub(super) ready: bool,
    signal: Option<NpcShopServiceSignal>,
}
impl Default for RepairSourceState {
    fn default() -> Self {
        Self {
            stamp: None,
            revision: 1,
            inventory: None,
            ready: false,
            signal: None,
        }
    }
}
impl RepairSourceState {
    fn advance(&mut self) {
        if self.revision != 0 {
            self.revision = self.revision.checked_add(1).unwrap_or(0);
        }
    }
    pub(super) fn retire(&mut self) {
        self.advance();
        self.ready = false;
        self.signal = None;
        self.inventory = None;
    }
    pub(super) fn sync_owner(&mut self, stamp: NativeCommandStamp) {
        if self.stamp != Some(stamp) {
            self.retire();
            self.stamp = Some(stamp);
        }
    }
    pub(super) fn stage_inventory(
        &mut self,
        stamp: NativeCommandStamp,
        inventory: &InventoryModel,
    ) {
        self.sync_owner(stamp);
        self.ready = false;
        let mut inventory = inventory.clone();
        inventory.npc_gold_trade_capacity = None;
        if self
            .inventory
            .as_ref()
            .and_then(|old| serde_json::to_value(old).ok())
            != serde_json::to_value(&inventory).ok()
        {
            self.advance();
        }
        self.inventory = Some(inventory);
    }
    pub(super) fn observe_service(
        &mut self,
        stamp: NativeCommandStamp,
        signal: NpcShopServiceSignal,
        delivered: bool,
    ) {
        self.sync_owner(stamp);
        self.advance();
        self.signal = if delivered
            && matches!(
                signal.mode,
                NpcShopServiceMode::Repair | NpcShopServiceMode::SpecialRepair
            ) {
            Some(signal)
        } else {
            None
        };
    }
    pub(super) fn observe_packet(&mut self, stamp: NativeCommandStamp, packet: &str) {
        self.sync_owner(stamp);
        if packet.starts_with("NPC") {
            self.advance();
            self.signal = None;
            self.ready = false;
            return;
        }
        if matches!(
            packet,
            "GainedGold"
                | "LoseGold"
                | "UserInformation"
                | "GainedItem"
                | "DeleteItem"
                | "DeleteItems"
                | "ItemChanged"
                | "ItemDurability"
                | "MoveItem"
                | "MergeItem"
                | "SplitItem"
                | "SplitItem1"
                | "DropItem"
                | "SellItem"
                | "EquipItem"
                | "RemoveItem"
                | "StoreItem"
                | "StoreItemV2"
                | "TakeBackItem"
                | "TakeBackItemV2"
                | "ResizeInventory"
                | "NewItem"
                | "UserSlotsRefresh"
                | "UseItem"
                | "RemoveSlotItem"
                | "EquipSlotItem"
                | "DepositRefineItem"
                | "RetrieveRefineItem"
                | "DepositTradeItem"
                | "RetrieveTradeItem"
                | "TakeBackHeroItem"
                | "TransferHeroItem"
                | "CombineItem"
                | "ItemUpgraded"
                | "RefreshItem"
                | "DuraChanged"
                | "ItemRepaired"
                | "ItemSlotSizeChanged"
                | "ItemSealChanged"
                | "DepositRentalItem"
                | "RetrieveRentalItem"
                | "UpdateRentalItem"
                | "ConfirmItemRental"
                | "MailLockedItem"
                | "AwakeningLockedItem"
                | "Awakening"
        ) {
            self.advance();
            self.ready = false;
        }
    }
    pub(super) fn signal(&self) -> Option<NpcShopServiceSignal> {
        self.signal
    }
    fn special(&self) -> Option<bool> {
        self.signal
            .map(|signal| signal.mode == NpcShopServiceMode::SpecialRepair)
    }
    fn selection(&self, stamp: NativeCommandStamp) -> Option<String> {
        if self.stamp != Some(stamp) || self.revision == 0 || !self.ready {
            return None;
        }
        let data =
            serde_json::to_vec(&(self.revision, self.special()?, self.inventory.as_ref()?)).ok()?;
        Some(format!("{:x}", Sha256::digest(data)))
    }
    pub(super) fn selection_for_actual(
        &self,
        stamp: NativeCommandStamp,
        actual: &InventoryModel,
    ) -> Option<String> {
        let mut actual = actual.clone();
        actual.npc_gold_trade_capacity = None;
        if self
            .inventory
            .as_ref()
            .and_then(|old| serde_json::to_value(old).ok())
            != serde_json::to_value(actual).ok()
        {
            return None;
        }
        self.selection(stamp)
    }
    fn allows_uid(&self, uid: u64, special: bool) -> bool {
        self.special() == Some(special)
            && self.inventory.as_ref().is_some_and(|model| {
                model.items.iter().any(|item| {
                    item.unique_id == Some(uid)
                        && item.container == 0
                        && item.quantity == 1
                        && item
                            .tooltip_source
                            .as_ref()
                            .is_some_and(|source| source.user_item.is_some())
                })
            })
    }
}

/// Recognize the namespace before generic decoding, including escaped or
/// duplicate top-level type fields. Strict wire decoding then rejects them.
pub(super) fn claims_owner_frame(raw: &str) -> bool {
    struct Marker;
    impl<'de> Deserialize<'de> for Marker {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Marker;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("repair envelope")
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<Marker, A::Error> {
                    while let Some(key) = map.next_key::<String>()? {
                        if key == "type" {
                            if map.next_value::<Value>()?.as_str() == Some("huntingRepairOwner") {
                                return Err(serde::de::Error::custom("native repair owner marker"));
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
        .is_err_and(|e| e.to_string().starts_with("native repair owner marker"))
}

struct PendingSource {
    request_id: u64,
    projection: NativeNpcEconomyProjection,
}

pub(super) struct NativeHuntingRepairGateway {
    client: NativeHuntingRepairClient,
    connection: HuntingRepairConnection,
    binding: Option<(NativeCommandStamp, RepairActor)>,
    binding_fence: Option<NativeCommandFence>,
    gate: Option<NativeHuntingRepairGate>,
    sources: VecDeque<PendingSource>,
    purchase_suspended: bool,
    confirm_prepared: bool,
    notice: Option<&'static str>,
    last_model: Option<HuntingRepairModel>,
    wallet_url: Option<String>,
    wallet_origin: Option<String>,
    wallet_refresh: bool,
    wallet_job: Option<(
        NativeCommandStamp,
        RepairActor,
        tokio::task::JoinHandle<Result<WalletResponse, &'static str>>,
    )>,
}

impl NativeHuntingRepairGateway {
    pub(super) fn new() -> Self {
        Self {
            client: NativeHuntingRepairClient::new(),
            connection: HuntingRepairConnection::new(),
            binding: None,
            binding_fence: None,
            gate: None,
            sources: VecDeque::new(),
            purchase_suspended: false,
            confirm_prepared: false,
            notice: None,
            last_model: None,
            wallet_url: None,
            wallet_origin: None,
            wallet_refresh: false,
            wallet_job: None,
        }
    }
    pub(super) fn configure_wallet(&mut self, gateway_ws_url: &str) {
        self.wallet_url = Some(gateway_ws_url.into());
        self.wallet_origin = std::env::var("MIR2_NATIVE_WALLET_ORIGIN")
            .ok()
            .filter(|value| !value.is_empty());
    }
    pub(super) async fn poll_wallet(&mut self) {
        if self
            .wallet_job
            .as_ref()
            .is_some_and(|(_, _, job)| job.is_finished())
        {
            let (stamp, actor, job) = self.wallet_job.take().expect("finished wallet request");
            if let Ok(Ok(response)) = job.await {
                if self.binding.as_ref() == Some(&(stamp, actor.clone()))
                    && self.connection.accept_wallet(
                        response,
                        &actor.account_id,
                        actor.character_index,
                        gateway_unix_ms(),
                    )
                {
                    self.publish(true);
                }
            }
        }
        if self.wallet_refresh && self.wallet_job.is_none() {
            self.wallet_refresh = false;
            if let (Some((stamp, actor)), Some(url), Some(origin)) =
                (&self.binding, &self.wallet_url, &self.wallet_origin)
            {
                if let Some(request) = self.connection.wallet_request(
                    url,
                    origin,
                    &actor.account_id,
                    actor.character_index,
                    gateway_unix_ms(),
                ) {
                    self.wallet_job = Some((
                        *stamp,
                        actor.clone(),
                        tokio::task::spawn_blocking(move || request.fetch()),
                    ));
                    self.publish(true);
                }
            }
        }
    }
    pub(super) fn open_connection(&mut self) {
        self.disconnect();
        self.connection = HuntingRepairConnection::new();
        self.last_model = None;
        self.publish(true);
    }
    pub(super) fn disconnect(&mut self) {
        self.withdraw();
        self.connection.clear_identity();
        self.purchase_suspended = false;
        self.last_model = None;
        let _ = mir2_bevy_runtime::native_ingest::push_native_hunting_repair_model(
            serde_json::to_string(&HuntingRepairModel::default())
                .expect("repair model serialization"),
        );
    }
    fn withdraw(&mut self) {
        if let Some(gate) = self.gate.take() {
            gate.retire();
        }
        self.client.withdraw();
        self.binding = None;
        self.binding_fence = None;
        self.sources.clear();
        self.notice = None;
        self.confirm_prepared = false;
        self.connection.invalidate_wallet();
        self.wallet_refresh = false;
    }
    fn publish(&mut self, force: bool) {
        let mode = self.connection.mode();
        let mut model = if mode == HuntingRepairMode::HuntingV1 && self.binding.is_some() {
            self.client.model()
        } else {
            HuntingRepairModel {
                mode,
                ..Default::default()
            }
        };
        if self.confirm_prepared {
            model.phase = HuntingRepairPhase::Pending;
        }
        if let Some((_, actor)) = &self.binding {
            if let Some((bound, unbound)) = self.connection.wallet_balance(
                &actor.account_id,
                actor.character_index,
                gateway_unix_ms(),
            ) {
                model.bound_gold = Some(bound);
                model.unbound_gold = Some(unbound);
            }
        }
        if let Some(notice) = self.notice {
            model.notice = Some(notice.into());
            if model.phase != HuntingRepairPhase::Pending {
                model.phase = HuntingRepairPhase::Rejected;
            }
        }
        if force || self.last_model.as_ref() != Some(&model) {
            // Projection admission shares the physical lifetime lock with
            // logout/actor changes. A late read model cannot cross their reset.
            let owner_guard = if let Some((stamp, _)) = &self.binding {
                let Some(fence) = self.binding_fence.as_ref() else {
                    return;
                };
                let Ok(state) = fence.0.lock() else {
                    return;
                };
                if !NativeCommandFence::matches(&state, *stamp, NativeCommandScope::World) {
                    return;
                }
                Some(state)
            } else {
                None
            };
            if let Ok(json) = serde_json::to_string(&model) {
                if mir2_bevy_runtime::native_ingest::push_native_hunting_repair_model(json) {
                    self.last_model = Some(model);
                }
            }
            drop(owner_guard);
        }
    }
    pub(super) fn observe_connection(
        &mut self,
        raw: &str,
        fence: Option<&NativeCommandFence>,
    ) -> Result<bool, String> {
        let observed = self.connection.observe(raw).map_err(str::to_owned)?;
        if observed {
            if let Some(fence) = fence {
                if let Ok(mut state) = fence.0.lock() {
                    state.hunting_repair_mode = self.connection.mode();
                }
            }
            self.publish(true);
        }
        Ok(observed)
    }
    pub(super) fn resume_identity(
        &self,
        now_ms: u64,
    ) -> Option<crate::hunting_repair_connection::NativeResumeIdentity> {
        self.connection.resume_identity(now_ms)
    }
    pub(super) fn blocks_purchase(&self) -> bool {
        self.client.model().phase == HuntingRepairPhase::Pending
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
            if let Some((stamp, _)) = &self.binding {
                self.client.close_view(*stamp);
            }
            self.purchase_suspended = false;
            purchases.resume_after_repair();
            self.publish(true);
        }
    }
    pub(super) fn reset_entry(&mut self) {
        self.withdraw();
        self.purchase_suspended = false;
        self.publish(true);
    }
    pub(super) fn leave_or_select(&mut self) {
        self.withdraw();
        self.purchase_suspended = false;
        self.publish(true);
    }
    pub(super) fn clear_identity_and_leave(&mut self) {
        self.connection.clear_identity();
        self.leave_or_select();
    }
    pub(super) fn before_ordinary_frame(&mut self, raw: &str) {
        if let Ok(envelope) = serde_json::from_str::<GatewayEnvelope>(raw) {
            if matches!(envelope.packet.as_deref(), Some("NPCRepair" | "NPCSRepair")) {
                self.wallet_refresh = true;
            }
            match envelope.packet.as_deref() {
                Some("LogOutSuccess") => self.leave_or_select(),
                Some("ReturnToLogin" | "Disconnect") => self.clear_identity_and_leave(),
                _ => {}
            }
        }
    }
    fn owner_ready(
        &self,
        stamp: NativeCommandStamp,
        account: &str,
        fence: &NativeCommandFence,
    ) -> bool {
        self.connection.mode() == HuntingRepairMode::HuntingV1
            && self.connection.identity_matches(account, gateway_unix_ms())
            && fence.0.lock().is_ok_and(|state| {
                !state.npc_entry_pending
                    && NativeCommandFence::matches(&state, stamp, NativeCommandScope::World)
            })
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
        let active = stamp.filter(|stamp| {
            phase == ConnectionPhase::Normal
                && bootstrapped
                && context
                    .account_id
                    .as_deref()
                    .is_some_and(|account| self.owner_ready(*stamp, account, fence))
        });
        let actor = active
            .zip(context.account_id.as_ref())
            .zip(context.character_index)
            .and_then(|((stamp, _account), index)| {
                Some((
                    stamp,
                    RepairActor {
                        realm: self.connection.realm_id()?.into(),
                        account_id: self.connection.account_id()?.into(),
                        character_index: index,
                    },
                ))
            });
        if self.binding != actor {
            self.withdraw();
            if let Some((stamp, actor)) = actor {
                let epoch = epoch(stamp).ok_or("Native repair full owner missing")?;
                let gate = NativeHuntingRepairGate::new(epoch)
                    .map_err(|_| "Native repair epoch unavailable")?;
                {
                    let mut state = fence
                        .0
                        .lock()
                        .map_err(|_| "Native repair owner fence poisoned")?;
                    if state.npc_entry_pending
                        || !NativeCommandFence::matches(&state, stamp, NativeCommandScope::World)
                    {
                        return Ok(());
                    }
                    self.client
                        .bind(stamp, actor.clone())
                        .map_err(str::to_owned)?;
                    if let Some(prior) = state.hunting_repair_gate.replace(gate.clone()) {
                        prior.retire();
                    }
                }
                self.gate = Some(gate);
                self.binding = Some((stamp, actor));
                self.binding_fence = Some(fence.clone());
                self.wallet_refresh = true;
            }
            self.publish(true);
        }
        let timed_out = self.check_timeout(gateway_unix_ms());
        self.drain_applied(fence)?;
        if let Ok(mut state) = fence.0.lock() {
            state.hunting_repair_busy = self.blocks_purchase();
        }
        if self.purchase_suspended
            && !self.blocks_purchase()
            && self.client.model().phase == HuntingRepairPhase::Committed
        {
            self.purchase_suspended = false;
            purchases.resume_after_repair();
        }
        self.publish(timed_out);
        Ok(())
    }
    fn check_timeout(&mut self, now_ms: u64) -> bool {
        let expired = self.client.check_timeout(now_ms);
        if expired {
            self.confirm_prepared = false;
        }
        expired
    }
    fn reject(&mut self, notice: &'static str) {
        self.notice = Some(notice);
        self.publish(true);
    }
    pub(super) fn prepare(
        &mut self,
        owned: &OwnedGatewayCommand,
        purchases: &mut npc_purchase_gateway::NativeNpcPurchaseGateway,
    ) -> Option<RepairDispatch> {
        let Some((stamp, actor)) = self.binding.clone() else {
            self.reject("authenticatedEntryRequired");
            return None;
        };
        if owned.stamp != stamp || !self.owner_ready(stamp, &actor.account_id, &owned.fence) {
            self.reject("ownerChanged");
            return None;
        }
        let GatewayCommand::HuntingRepair(action) = &owned.command else {
            return None;
        };
        if matches!(action, HuntingRepairCommand::Close) {
            self.client.close_view(stamp);
            self.notice = None;
            if !self.blocks_purchase() {
                self.purchase_suspended = false;
                purchases.resume_after_repair();
            }
            self.publish(true);
            return None;
        }
        let state = match owned.fence.0.lock() {
            Ok(state) => state,
            Err(_) => {
                self.reject("ownerUnavailable");
                return None;
            }
        };
        if state.outstanding.get(&owned.sequence) != Some(&stamp)
            || !NativeCommandFence::matches(&state, stamp, NativeCommandScope::World)
            || state.npc_entry_pending
            || state.hunting_repair_mode != HuntingRepairMode::HuntingV1
            || !matches!(action,HuntingRepairCommand::Query{..}) && state.hunting_material_busy
        {
            drop(state);
            self.reject("ownerChanged");
            return None;
        }
        let selection = match action {
            HuntingRepairCommand::Quote { selection, .. }
            | HuntingRepairCommand::Confirm { selection, .. } => {
                let current = state.hunting_repair_source.selection(stamp);
                if selection.is_none() || selection != &current {
                    drop(state);
                    self.reject("itemOrServiceChanged");
                    return None;
                }
                current
            }
            _ => None,
        };
        if let HuntingRepairCommand::Quote {
            unique_id, special, ..
        } = action
        {
            if !state.hunting_repair_source.allows_uid(*unique_id, *special) {
                drop(state);
                self.reject("itemOrServiceChanged");
                return None;
            }
        }
        // No purchase state lock is acquired while holding the physical fence.
        drop(state);
        // A retained original Query must remain available independently of a
        // different lane's Pending operation; it never takes charge ownership.
        if !matches!(action,HuntingRepairCommand::Query{..}) && !self.purchase_suspended {
            if !purchases.suspend_for_repair() {
                self.reject("purchasePending");
                return None;
            }
            self.purchase_suspended = true;
        }
        // A completed purchase legitimately retires the previous repair
        // source. Recreate only this local gate for the same verified owner;
        // the permanent client's original quote/receipt ledger is unchanged.
        let current_epoch = epoch(stamp).expect("verified full repair owner");
        if self
            .gate
            .as_ref()
            .is_none_or(|gate| !gate.is_current(current_epoch))
        {
            let Ok(gate) = NativeHuntingRepairGate::new(current_epoch) else {
                self.reject("ownerUnavailable");
                return None;
            };
            let Ok(mut state) = owned.fence.0.lock() else {
                self.reject("ownerUnavailable");
                return None;
            };
            if state.npc_entry_pending
                || !NativeCommandFence::matches(&state, stamp, NativeCommandScope::World)
            {
                drop(state);
                self.reject("ownerChanged");
                return None;
            }
            if let Some(prior) = state.hunting_repair_gate.replace(gate.clone()) {
                prior.retire();
            }
            drop(state);
            self.gate = Some(gate);
            self.sources.clear();
        }
        let now = gateway_unix_ms();
        let result = match action {
            HuntingRepairCommand::Quote {
                unique_id, special, ..
            } => self
                .client
                .prepare_quote(stamp, *unique_id, *special, selection.unwrap(), now),
            HuntingRepairCommand::Confirm { quote_id, .. } => {
                self.client
                    .prepare_confirm(stamp, quote_id, selection.as_deref().unwrap(), now)
            }
            HuntingRepairCommand::Query { quote_id } => {
                self.client.prepare_query(stamp, quote_id, now)
            }
            HuntingRepairCommand::Close => unreachable!(),
        };
        match result {
            Ok(dispatch) => {
                if matches!(&dispatch.request.action, wire::Action::Confirm { .. }) {
                    self.confirm_prepared = true;
                }
                if let Ok(mut state)=owned.fence.0.lock(){state.hunting_repair_busy=self.blocks_purchase();}
                self.notice = None;
                self.publish(true);
                Some(dispatch)
            }
            Err(_) => {
                self.reject("repairControlUnavailable");
                None
            }
        }
    }
    fn final_source_matches(state: &NativeCommandFenceState, owned: &OwnedGatewayCommand) -> bool {
        match &owned.command {
            GatewayCommand::HuntingRepair(
                HuntingRepairCommand::Quote { selection, .. }
                | HuntingRepairCommand::Confirm { selection, .. },
            ) => {
                selection.is_some()
                    && selection == &state.hunting_repair_source.selection(owned.stamp)
            }
            GatewayCommand::HuntingRepair(HuntingRepairCommand::Query { .. }) => true,
            _ => false,
        }
    }
    pub(super) async fn send<S>(
        &mut self,
        sink: &mut S,
        owned: &OwnedGatewayCommand,
        dispatch: RepairDispatch,
    ) -> NativeSinkCommit
    where
        S: futures_util::Sink<Message> + Unpin,
        S::Error: std::fmt::Display,
    {
        if matches!(&dispatch.request.action, wire::Action::Confirm { .. }) {
            self.connection.invalidate_wallet();
            self.wallet_refresh = false;
            self.publish(true);
        }
        let mut budget = TokioNativeWriteBudget::new();
        let outcome = self
            .send_with_budget(sink, owned, dispatch, &mut budget)
            .await;
        self.confirm_prepared = false;
        if let Ok(mut state)=owned.fence.0.lock(){state.hunting_repair_busy=self.blocks_purchase();}
        self.publish(true);
        outcome
    }
    async fn send_with_budget<S>(
        &mut self,
        sink: &mut S,
        owned: &OwnedGatewayCommand,
        dispatch: RepairDispatch,
        budget: &mut impl NativeWriteBudget,
    ) -> NativeSinkCommit
    where
        S: futures_util::Sink<Message> + Unpin,
        S::Error: std::fmt::Display,
    {
        struct Waiter<'a>(&'a OwnedGatewayCommand);
        impl Drop for Waiter<'_> {
            fn drop(&mut self) {
                if let Ok(mut state) = self.0.fence.0.lock() {
                    state.waiters.remove(&self.0.sequence);
                }
            }
        }
        let waiter = Waiter(owned);
        let ready = await_native_write_phase(budget, |cx| {
            let Ok(mut state) = owned.fence.0.lock() else {
                return std::task::Poll::Ready(None);
            };
            if state.outstanding.get(&owned.sequence) != Some(&owned.stamp)
                || !NativeCommandFence::matches(&state, owned.stamp, NativeCommandScope::World)
                || state.hunting_repair_mode != HuntingRepairMode::HuntingV1
                || state.npc_entry_pending
                || !matches!(&owned.command,GatewayCommand::HuntingRepair(HuntingRepairCommand::Query{..})) && state.hunting_material_busy
                || !Self::final_source_matches(&state, owned)
            {
                return std::task::Poll::Ready(None);
            }
            state.waiters.insert(owned.sequence, cx.waker().clone());
            drop(state);
            Pin::new(&mut *sink).poll_ready(cx).map(Some)
        })
        .await;
        drop(waiter);
        let pre_send = match ready {
            Err(_) => Some(NativeSinkCommit::Unavailable(
                NATIVE_TRANSPORT_TIMEOUT_ERROR.into(),
            )),
            Ok(None) => Some(NativeSinkCommit::DefinitelyUnsent),
            Ok(Some(Err(error))) => Some(NativeSinkCommit::Unavailable(error.to_string())),
            Ok(Some(Ok(()))) => None,
        };
        if let Some(outcome) = pre_send {
            self.client
                .definitely_unsent(dispatch.request.request_id.get());
            owned.fence.retire(owned);
            return outcome;
        }
        let started = {
            let Ok(mut state) = owned.fence.0.lock() else {
                self.client
                    .definitely_unsent(dispatch.request.request_id.get());
                return NativeSinkCommit::DefinitelyUnsent;
            };
            if state.outstanding.get(&owned.sequence) != Some(&owned.stamp)
                || !NativeCommandFence::matches(&state, owned.stamp, NativeCommandScope::World)
                || state.hunting_repair_mode != HuntingRepairMode::HuntingV1
                || state.npc_entry_pending
                || !matches!(&owned.command,GatewayCommand::HuntingRepair(HuntingRepairCommand::Query{..})) && state.hunting_material_busy
                || !Self::final_source_matches(&state, owned)
                || budget.expired()
                || self.binding.as_ref().is_none_or(|(stamp, actor)| {
                    *stamp != owned.stamp
                        || !self
                            .connection
                            .identity_matches(&actor.account_id, gateway_unix_ms())
                })
            {
                drop(state);
                self.client
                    .definitely_unsent(dispatch.request.request_id.get());
                owned.fence.retire(owned);
                return NativeSinkCommit::DefinitelyUnsent;
            }
            if self.client.enter(&dispatch, gateway_unix_ms()).is_err() {
                drop(state);
                self.client
                    .definitely_unsent(dispatch.request.request_id.get());
                owned.fence.retire(owned);
                return NativeSinkCommit::DefinitelyUnsent;
            }
            state.outstanding.remove(&owned.sequence);
            state.hunting_repair_busy = self.blocks_purchase();
            // No await or externally supplied callback can separate entry from
            // the irreversible physical send under the same revocation lock.
            Pin::new(&mut *sink).start_send(Message::Text(dispatch.body.into()))
        };
        if let Err(error) = started {
            return NativeSinkCommit::Unknown(error.to_string());
        }
        match await_native_write_phase(budget, |cx| Pin::new(&mut *sink).poll_flush(cx)).await {
            Ok(Ok(())) => NativeSinkCommit::Flushed,
            Ok(Err(error)) => NativeSinkCommit::Unknown(error.to_string()),
            Err(_) => NativeSinkCommit::Unknown(NATIVE_TRANSPORT_TIMEOUT_ERROR.into()),
        }
    }
    pub(super) fn receive(&mut self, raw: &str, fence: &NativeCommandFence) -> Result<(), String> {
        self.receive_with(
            raw,
            fence,
            mir2_bevy_runtime::native_ingest::push_native_hunting_repair_bundle,
        )
    }
    fn receive_with(
        &mut self,
        raw: &str,
        fence: &NativeCommandFence,
        mut enqueue: impl FnMut(
            mir2_bevy_runtime::hunting_repair_economy::NativeHuntingRepairBundle,
        ) -> bool,
    ) -> Result<(), String> {
        let Some((stamp, actor)) = self.binding.clone() else {
            return Err("Native repair reply has no active owner".into());
        };
        if !self.owner_ready(stamp, &actor.account_id, fence) {
            return Err("Native repair reply owner retired".into());
        }
        let received = self.client.receive(stamp, raw).map_err(str::to_owned)?;
        self.wallet_refresh = true;
        let frame = received.frame;
        if let (
            wire::Reply::Committed {
                receipt,
                reload_required: false,
                ..
            },
            Some(owner),
        ) = (&frame.reply, &frame.snapshot)
        {
            // Preserve the known receipt even if projection/admission fails.
            if self.sources.len() < 32 {
                if let (Some(gate), Ok(projection)) = (
                    &self.gate,
                    crate::npc_purchase_projection::project_owner(owner),
                ) {
                    if let Ok(mut bundle) = gate.prepare(
                        frame.request_id.get(),
                        &receipt.quote_id,
                        projection.clone(),
                    ) {
                        let display = fence.0.lock().ok().and_then(|state| {
                            state
                                .hero_owner_gate
                                .as_ref()
                                .filter(|(saved, _)| *saved == stamp)
                                .map(|(_, gate)| gate.clone())
                        });
                        if let Some(display) = display {
                            if let Ok(attached) = bundle.clone().with_hero_owner_gate(&display) {
                                bundle = attached;
                            } else {
                                self.publish(true);
                                return Ok(());
                            }
                        }
                        if enqueue(bundle) {
                            self.sources.push_back(PendingSource {
                                request_id: frame.request_id.get(),
                                projection,
                            });
                        }
                    }
                }
            }
        }
        if let Ok(mut state) = fence.0.lock() {
            state.hunting_repair_busy = self.blocks_purchase();
        }
        self.notice = None;
        self.publish(true);
        Ok(())
    }
    fn drain_applied(&mut self, fence: &NativeCommandFence) -> Result<(), String> {
        for _ in 0..32 {
            let Some(gate) = self.gate.clone() else { break; };
            let Some(applied) = gate.try_recv_applied() else { break; };
            self.complete_applied(fence, &gate, &applied)?;
        }
        Ok(())
    }

    fn complete_applied(
        &mut self,
        fence: &NativeCommandFence,
        gate: &NativeHuntingRepairGate,
        applied: &mir2_bevy_runtime::hunting_repair_economy::NativeHuntingRepairApplied,
    ) -> Result<bool, String> {
        let Some((stamp, actor)) = self.binding.as_ref() else { return Ok(false); };
        if epoch(*stamp) != Some(applied.epoch())
            || !self.owner_ready(*stamp, &actor.account_id, fence)
        {
            return Ok(false);
        }
        let Some(index) = self.sources.iter()
            .position(|source| source.request_id == applied.request_id())
        else { return Err("Native repair complete source custody missing".into()); };
        let source = self.sources.remove(index).expect("paired repair source");
        let stamp = *stamp;
        let synchronized = npc_purchase_gateway::with_applied_ui_source(
            fence,
            stamp,
            &source.projection,
            |state| {
                // Lock order is physical fence -> Runtime gate. This only
                // retires local selection data, never the gate held here.
                gate.with_current_applied(applied, || {
                    if self
                        .client
                        .applied(stamp, applied.request_id(), applied.quote_id())
                    {
                        state.hunting_repair_source.retire();
                        state.hunting_repair_busy = false;
                        true
                    } else {
                        false
                    }
                }).unwrap_or(false)
            },
        )?;
        if synchronized {
            self.notice = None;
            self.wallet_refresh = true;
            self.publish(true);
        }
        Ok(synchronized)
    }
}

fn epoch(stamp: NativeCommandStamp) -> Option<NativeHuntingRepairEpoch> {
    Some(NativeHuntingRepairEpoch {
        run: stamp.run,
        connection: stamp.connection,
        procedure: stamp.procedure,
        owner_epoch: stamp.owner_epoch,
        scene_epoch: stamp.scene_epoch,
        cancellation: stamp.cancellation,
        actor: stamp.actor?,
        map: stamp.map?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::Sink;
    use std::task::{Context, Poll};
    include!("native_hunting_repair_inventory_tests.rs");
    include!("hunting_repair_resume_tests.rs");

    #[derive(Default)]
    struct SinkState {
        frames: Vec<String>,
        start_error: bool,
        ready_hook: Option<Box<dyn FnOnce() + Send>>,
    }
    #[derive(Clone, Default)]
    struct TestSink(Arc<Mutex<SinkState>>);
    impl Sink<Message> for TestSink {
        type Error = &'static str;
        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            let hook = self.0.lock().unwrap().ready_hook.take();
            if let Some(hook) = hook {
                hook();
            }
            Poll::Ready(Ok(()))
        }
        fn start_send(self: Pin<&mut Self>, frame: Message) -> Result<(), Self::Error> {
            let mut state = self.0.lock().unwrap();
            state.frames.push(frame.to_text().unwrap().into());
            if state.start_error {
                Err("synthetic send outcome unknown")
            } else {
                Ok(())
            }
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.poll_flush(cx)
        }
    }
    struct Fixture {
        gateway: NativeHuntingRepairGateway,
        fence: NativeCommandFence,
        stamp: NativeCommandStamp,
        purchases: npc_purchase_gateway::NativeNpcPurchaseGateway,
        selection: String,
    }
    fn quote_id() -> String {
        "ab".repeat(32)
    }
    fn ready() -> Fixture {
        let (sender, _) = command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        let stamp = fence.test_world_ready(3, 0);
        let mut gateway = NativeHuntingRepairGateway::new();
        gateway
            .observe_connection(
                &json!({"type":"realmInfo","payload":{"schema":"mir2-realm-handshake/1",
            "realmId":"synthetic-native-realm","npcRepairMode":"hunting-v1"}})
                .to_string(),
                Some(&fence),
            )
            .unwrap();
        gateway
            .observe_connection(
                &json!({"type":"identitySession","token":"synthetic-native-test-token",
            "session":{"accountId":"synthetic-player","sessionId":"synthetic-session",
                "expiresAtMs":gateway_unix_ms()+600_000,"revokedAtMs":null}})
                .to_string(),
                Some(&fence),
            )
            .unwrap();
        let context = GatewaySessionContext {
            account_id: Some("SYNTHETIC-PLAYER".into()),
            character_index: Some(0),
            ..Default::default()
        };
        let mut purchases = npc_purchase_gateway::NativeNpcPurchaseGateway::new().unwrap();
        gateway
            .tick(
                &context,
                &fence,
                Some(stamp),
                ConnectionPhase::Normal,
                true,
                &mut purchases,
            )
            .unwrap();
        let selection = {
            let mut state = fence.0.lock().unwrap();
            state
                .hunting_repair_source
                .stage_inventory(stamp, &InventoryModel::default());
            state.hunting_repair_source.observe_service(
                stamp,
                NpcShopServiceSignal {
                    mode: NpcShopServiceMode::Repair,
                    repair_rate: Some(1.0),
                },
                true,
            );
            state.hunting_repair_source.ready = true;
            state.hunting_repair_source.selection(stamp).unwrap()
        };
        assert_eq!(
            gateway.binding.as_ref().unwrap().1.account_id,
            "synthetic-player"
        );
        Fixture {
            gateway,
            fence,
            stamp,
            purchases,
            selection,
        }
    }
    fn frame(
        request: &mir2_client_wire::hunting_repair::ClientRequest,
        reply: wire::Reply,
        snapshot: Option<Value>,
    ) -> String {
        wire::encode_server_frame(&wire::ServerFrame::new(request, reply, snapshot).unwrap())
            .unwrap()
    }
    fn seed_quote(f: &mut Fixture) {
        let now = gateway_unix_ms();
        let dispatch = f
            .gateway
            .client
            .prepare_quote(f.stamp, 900_001, false, f.selection.clone(), now)
            .unwrap();
        f.gateway.client.enter(&dispatch, now).unwrap();
        let reply = wire::Reply::Quoted {
            quote: wire::Quote {
                quote_id: quote_id(),
                unique_id: mir2_client_wire::U64::new(900_001),
                special: false,
                cost_bound_gold: mir2_client_wire::U64::new(6),
                created_at_ms: now as i64,
                expires_at_ms: now as i64 + 30_000,
            },
        };
        f.gateway
            .receive(&frame(&dispatch.request, reply, None), &f.fence)
            .unwrap();
    }
    fn own_action(f: &Fixture, command: HuntingRepairCommand) -> Box<OwnedGatewayCommand> {
        let GatewayCommand::Owned(owned) = f
            .fence
            .prepare(f.stamp, GatewayCommand::HuntingRepair(command))
            .unwrap()
        else {
            unreachable!()
        };
        owned
    }
    fn confirm(f: &mut Fixture) -> (Box<OwnedGatewayCommand>, RepairDispatch) {
        let owned = own_action(
            f,
            HuntingRepairCommand::Confirm {
                quote_id: quote_id(),
                selection: Some(f.selection.clone()),
            },
        );
        let dispatch = f.gateway.prepare(&owned, &mut f.purchases).unwrap();
        (owned, dispatch)
    }
    fn receipt() -> wire::Receipt {
        wire::Receipt {
            quote_id: quote_id(),
            cost_bound_gold: mir2_client_wire::U64::new(6),
            free: false,
            operation_key: "synthetic-original-repair".into(),
            request_digest: "synthetic-original-source".into(),
            business_day: 1,
            committed_at_ms: 1,
        }
    }

    #[test]
    fn native_hunting_repair_host_marker_preserves_escaped_and_duplicate_namespace() {
        for raw in [
            r#"{"type":"huntingRepairOwner"}"#,
            r#"{"ty\u0070e":"huntingRepairOwner"}"#,
            r#"{"type":"worldSnapshot","type":"huntingRepairOwner"}"#,
        ] {
            assert!(claims_owner_frame(raw));
        }
        assert!(!claims_owner_frame(
            r#"{"type":"worldSnapshot","payload":{"text":"huntingRepairOwner"}}"#
        ));
    }

    #[test]
    fn native_hunting_repair_host_source_refuses_inventory_aba_and_failed_delivery() {
        let f = ready();
        let mut state = f.fence.0.lock().unwrap();
        assert_eq!(
            state
                .hunting_repair_source
                .selection_for_actual(f.stamp, &InventoryModel::default()),
            Some(f.selection.clone())
        );
        state
            .hunting_repair_source
            .observe_packet(f.stamp, "ItemDurability");
        assert!(state.hunting_repair_source.selection(f.stamp).is_none());
        state
            .hunting_repair_source
            .stage_inventory(f.stamp, &InventoryModel::default());
        state.hunting_repair_source.ready = true;
        assert_ne!(
            state.hunting_repair_source.selection(f.stamp),
            Some(f.selection)
        );
        state.hunting_repair_source.ready = false;
        assert!(state
            .hunting_repair_source
            .selection_for_actual(f.stamp, &InventoryModel::default())
            .is_none());
    }

    #[tokio::test]
    async fn native_hunting_repair_host_same_socket_logout_startgame_preserves_identity() {
        let mut f = ready();
        let mut context = GatewaySessionContext {
            account_id: Some("SYNTHETIC-PLAYER".into()),
            character_index: Some(0),
            ..Default::default()
        };
        let mut resume = NativeResumeClientState::default();
        let GatewayCommand::Owned(logout) = f
            .fence
            .prepare(f.stamp, GatewayCommand::Wire(NativeOutboundCommand::LogOut))
            .unwrap()
        else {
            unreachable!()
        };
        f.gateway.reset_entry();
        let mut sink = TestSink::default();
        assert!(matches!(
            commit_owned_frame(
                &mut sink,
                Some(&logout),
                Message::Text(json!({"type":"logOut"}).to_string().into())
            )
            .await,
            NativeSinkCommit::Flushed
        ));
        assert!(apply_flushed_control_context(
            Some(&logout),
            &NativeOutboundCommand::LogOut,
            &mut context,
            &mut resume
        ));
        f.gateway
            .before_ordinary_frame(r#"{"type":"packet","packet":"LogOutSuccess"}"#);
        f.fence.revoke_owner();
        assert!(f.gateway.binding.is_none());
        assert!(f
            .gateway
            .connection
            .identity_matches("synthetic-player", gateway_unix_ms()));
        assert_eq!(context.character_index, None);

        let wire = NativeOutboundCommand::StartGame { character_index: 0 };
        let GatewayCommand::Owned(start) = f
            .fence
            .prepare(f.fence.stamp().unwrap(), GatewayCommand::Wire(wire.clone()))
            .unwrap()
        else {
            unreachable!()
        };
        f.gateway.reset_entry();
        assert!(matches!(
            commit_owned_frame(
                &mut sink,
                Some(&start),
                Message::Text(
                    json!({"type":"startGame","characterIndex":0})
                        .to_string()
                        .into()
                )
            )
            .await,
            NativeSinkCommit::Flushed
        ));
        assert!(apply_flushed_control_context(
            Some(&start),
            &wire,
            &mut context,
            &mut resume
        ));
        f.fence.authorize_entry(false);
        f.fence.observe_scene(0, false);
        let fresh = f.fence.confirm_owner(3).unwrap();
        assert_eq!(fresh.connection, f.stamp.connection);
        assert_ne!(fresh.owner_epoch, f.stamp.owner_epoch);
        f.gateway
            .tick(
                &context,
                &f.fence,
                Some(fresh),
                ConnectionPhase::Normal,
                true,
                &mut f.purchases,
            )
            .unwrap();
        assert_eq!(f.gateway.binding.as_ref().unwrap().0, fresh);
        assert_eq!(
            f.gateway.binding.as_ref().unwrap().1.account_id,
            "synthetic-player"
        );
        assert_eq!(f.gateway.connection.mode(), HuntingRepairMode::HuntingV1);
        // An actual return to Login still revokes the private bearer grant.
        f.gateway
            .before_ordinary_frame(r#"{"type":"packet","packet":"ReturnToLogin"}"#);
        assert!(!f
            .gateway
            .connection
            .identity_matches("synthetic-player", gateway_unix_ms()));
        assert!(f.gateway.binding.is_none());
    }

    #[tokio::test]
    async fn native_hunting_repair_host_new_local_gate_preserves_original_pending_query() {
        let mut f = ready();
        seed_quote(&mut f);
        let (owned, dispatch) = confirm(&mut f);
        let mut sink = TestSink::default();
        assert!(matches!(
            f.gateway.send(&mut sink, &owned, dispatch).await,
            NativeSinkCommit::Flushed
        ));
        assert!(f.gateway.check_timeout(gateway_unix_ms() + 10_001));
        let prior = f.gateway.gate.as_ref().unwrap().clone();
        prior.retire();
        let query = own_action(
            &f,
            HuntingRepairCommand::Query {
                quote_id: quote_id(),
            },
        );
        let dispatch = f.gateway.prepare(&query, &mut f.purchases).unwrap();
        assert!(!prior.is_current(epoch(f.stamp).unwrap()));
        assert!(f
            .gateway
            .gate
            .as_ref()
            .unwrap()
            .is_current(epoch(f.stamp).unwrap()));
        assert!(matches!(
            f.gateway.send(&mut sink, &query, dispatch).await,
            NativeSinkCommit::Flushed
        ));
        let state = sink.0.lock().unwrap();
        assert_eq!(state.frames.len(), 2);
        let request = wire::parse_client_request(&state.frames[1]).unwrap();
        assert!(matches!(request.action, wire::Action::Query {quote_id: id} if id == quote_id()));
        assert_eq!(f.gateway.client.model().phase, HuntingRepairPhase::Pending);
    }

    #[tokio::test]
    async fn native_hunting_repair_host_original_query_preserves_true_other_lane_pending() {
        let mut f=ready();seed_quote(&mut f);let (confirm,dispatch)=confirm(&mut f);
        let mut sink=TestSink::default();
        assert!(matches!(f.gateway.send(&mut sink,&confirm,dispatch).await,NativeSinkCommit::Flushed));
        f.gateway.reset_entry();
        f.gateway.tick(&GatewaySessionContext{account_id:Some("synthetic-player".into()),character_index:Some(0),..Default::default()},
            &f.fence,Some(f.stamp),ConnectionPhase::Normal,true,&mut f.purchases).unwrap();
        assert_eq!(f.gateway.client.model().phase,HuntingRepairPhase::Pending);assert!(!f.gateway.purchase_suspended);
        f.purchases=npc_purchase_gateway::pending_purchase_fixture();assert!(!f.purchases.suspend_for_repair());
        f.fence.0.lock().unwrap().hunting_material_busy=true;
        let query=own_action(&f,HuntingRepairCommand::Query{quote_id:quote_id()});
        let dispatch=f.gateway.prepare(&query,&mut f.purchases).expect("original repair Query bypasses new charge arbitration");
        assert!(!f.gateway.purchase_suspended);
        assert!(matches!(f.gateway.send(&mut sink,&query,dispatch).await,NativeSinkCommit::Flushed));
        assert!(!f.purchases.suspend_for_repair());assert!(f.fence.0.lock().unwrap().hunting_material_busy);
        assert_eq!(f.gateway.client.model().phase,HuntingRepairPhase::Pending);
        let frames=&sink.0.lock().unwrap().frames;assert_eq!(frames.len(),2);
        assert!(matches!(wire::parse_client_request(&frames[0]).unwrap().action,wire::Action::Confirm{..}));
        assert!(matches!(wire::parse_client_request(&frames[1]).unwrap().action,wire::Action::Query{..}));
    }

    #[tokio::test]
    async fn native_hunting_repair_host_quotes_real_carried_uid_without_legacy_gold_gate() {
        use mir2_client_bevy::inventory::{
            CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel, ItemModel,
        };
        let mut f = ready();
        let mut inventory = InventoryModel::default();
        inventory.gold = 0;
        inventory.items.push(ItemModel {
            unique_id: Some(900_001),
            quantity: 1,
            container: 0,
            slot: 20,
            durability_current: Some(500),
            durability_max: Some(970),
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 221,
                    price: 20,
                    durability: 970,
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id: 900_001,
                    item_index: 221,
                    count: 1,
                    current_dura: 500,
                    max_dura: 970,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });
        {
            let mut state = f.fence.0.lock().unwrap();
            state
                .hunting_repair_source
                .stage_inventory(f.stamp, &inventory);
            state.hunting_repair_source.ready = true;
        }
        let selection = f.fence.hunting_repair_selection(&inventory).unwrap();
        let owned = own_action(
            &f,
            HuntingRepairCommand::Quote {
                unique_id: 900_001,
                special: false,
                selection: Some(selection),
            },
        );
        let dispatch = f.gateway.prepare(&owned, &mut f.purchases).unwrap();
        let mut sink = TestSink::default();
        assert!(matches!(
            f.gateway.send(&mut sink, &owned, dispatch).await,
            NativeSinkCommit::Flushed
        ));
        let request = wire::parse_client_request(&sink.0.lock().unwrap().frames[0]).unwrap();
        assert!(
            matches!(request.action,wire::Action::Quote{unique_id,special:false} if unique_id.get()==900_001)
        );
        assert_eq!(f.gateway.client.model().phase, HuntingRepairPhase::Quoting);
        assert!(f.gateway.client.model().cost_bound_gold.is_none());
    }

    #[tokio::test]
    async fn native_hunting_repair_host_final_ready_rechecks_original_source_without_sending() {
        let mut f = ready();
        seed_quote(&mut f);
        let (owned, dispatch) = confirm(&mut f);
        let mut sink = TestSink::default();
        let fence = f.fence.clone();
        let stamp = f.stamp;
        sink.0.lock().unwrap().ready_hook = Some(Box::new(move || {
            fence
                .0
                .lock()
                .unwrap()
                .hunting_repair_source
                .observe_packet(stamp, "ItemDurability");
        }));
        assert!(matches!(
            f.gateway.send(&mut sink, &owned, dispatch).await,
            NativeSinkCommit::DefinitelyUnsent
        ));
        assert!(sink.0.lock().unwrap().frames.is_empty());
        assert_eq!(f.gateway.client.model().phase, HuntingRepairPhase::Quoted);
        assert!(!f.gateway.blocks_purchase());
    }

    #[tokio::test]
    async fn native_hunting_repair_host_start_error_retains_original_query_and_never_reconfirms() {
        let mut f = ready();
        seed_quote(&mut f);
        let (owned, dispatch) = confirm(&mut f);
        let mut sink = TestSink::default();
        sink.0.lock().unwrap().start_error = true;
        assert!(matches!(
            f.gateway.send(&mut sink, &owned, dispatch.clone()).await,
            NativeSinkCommit::Unknown(_)
        ));
        assert_eq!(f.gateway.client.model().phase, HuntingRepairPhase::Pending);
        assert!(f.gateway.blocks_purchase());
        assert!(f.gateway.check_timeout(gateway_unix_ms() + 10_001));
        let again = own_action(
            &f,
            HuntingRepairCommand::Confirm {
                quote_id: quote_id(),
                selection: Some(f.selection.clone()),
            },
        );
        assert!(f.gateway.prepare(&again, &mut f.purchases).is_none());
        f.fence.retire(&again);
        let query = own_action(
            &f,
            HuntingRepairCommand::Query {
                quote_id: quote_id(),
            },
        );
        let dispatch = f.gateway.prepare(&query, &mut f.purchases).unwrap();
        sink.0.lock().unwrap().start_error = false;
        assert!(matches!(
            f.gateway.send(&mut sink, &query, dispatch).await,
            NativeSinkCommit::Flushed
        ));
        let frames = &sink.0.lock().unwrap().frames;
        assert_eq!(frames.len(), 2);
        assert!(matches!(
            wire::parse_client_request(&frames[0]).unwrap().action,
            wire::Action::Confirm { .. }
        ));
        assert!(matches!(
            wire::parse_client_request(&frames[1]).unwrap().action,
            wire::Action::Query { .. }
        ));
    }

    #[tokio::test]
    async fn native_hunting_repair_host_receipt_requires_actual_complete_world_application() {
        let mut f = ready();
        seed_quote(&mut f);
        let (owned, dispatch) = confirm(&mut f);
        assert!(matches!(
            f.gateway
                .send(&mut TestSink::default(), &owned, dispatch.clone())
                .await,
            NativeSinkCommit::Flushed
        ));
        let owner = crate::npc_purchase_projection::tests::owner();
        let mut bundles = Vec::new();
        f.gateway
            .receive_with(
                &frame(
                    &dispatch.request,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: false,
                        reload_required: false,
                    },
                    Some(owner.clone()),
                ),
                &f.fence,
                |bundle| {
                    bundles.push(bundle);
                    true
                },
            )
            .unwrap();
        assert_eq!(bundles.len(), 1);
        assert!(f.gateway.blocks_purchase());
        f.gateway.drain_applied(&f.fence).unwrap();
        assert!(f.gateway.blocks_purchase());
        let mut world = bevy::prelude::World::new();
        assert!(
            mir2_bevy_runtime::hunting_repair_economy::apply_native_hunting_repair_bundle(
                &mut world,
                bundles.pop().unwrap()
            )
        );
        f.gateway.drain_applied(&f.fence).unwrap();
        assert_eq!(
            f.gateway.client.model().phase,
            HuntingRepairPhase::Committed
        );
        assert!(!f.gateway.blocks_purchase());
        assert_eq!(
            world
                .resource::<mir2_bevy_runtime::hunting_repair_economy::NativeHuntingRepairSource>()
                .owner(),
            &owner
        );
    }

    #[tokio::test]
    async fn native_hunting_repair_host_popped_applied_retired_before_completion_stays_pending() {
        let mut f = ready();
        seed_quote(&mut f);
        let (owned, dispatch) = confirm(&mut f);
        assert!(matches!(f.gateway.send(&mut TestSink::default(), &owned, dispatch.clone()).await,
            NativeSinkCommit::Flushed));
        let mut bundles = Vec::new();
        f.gateway.receive_with(
            &frame(&dispatch.request, wire::Reply::Committed {
                receipt: receipt(), replayed: false, reload_required: false,
            }, Some(crate::npc_purchase_projection::tests::owner())),
            &f.fence,
            |bundle| { bundles.push(bundle); true },
        ).unwrap();
        let mut world = bevy::prelude::World::new();
        assert!(mir2_bevy_runtime::hunting_repair_economy::apply_native_hunting_repair_bundle(
            &mut world, bundles.pop().unwrap()));
        let gate = f.gateway.gate.clone().unwrap();
        let applied = gate.try_recv_applied().unwrap();
        // Reset admission retires this independent Runtime gate even if the
        // native physical fence has not changed since the proof was popped.
        gate.retire();
        assert!(f.fence.accepts(f.stamp, true));
        assert!(!f.gateway.complete_applied(&f.fence, &gate, &applied).unwrap());
        assert_eq!(f.gateway.client.model().phase, HuntingRepairPhase::Pending);
        assert!(f.gateway.blocks_purchase());
        assert!(f.fence.0.lock().unwrap().hunting_repair_busy);
        assert!(f.gateway.client.prepare_confirm(f.stamp, &quote_id(), &f.selection, gateway_unix_ms()).is_err());
        assert!(f.gateway.client.prepare_query(f.stamp, &quote_id(), gateway_unix_ms()).is_ok());
    }

    #[tokio::test]
    async fn native_hunting_repair_host_bad_snapshot_and_retired_actor_never_complete() {
        let mut f = ready();
        seed_quote(&mut f);
        let (owned, dispatch) = confirm(&mut f);
        f.gateway
            .send(&mut TestSink::default(), &owned, dispatch.clone())
            .await;
        let mut count = 0;
        f.gateway
            .receive_with(
                &frame(
                    &dispatch.request,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: false,
                        reload_required: false,
                    },
                    Some(json!({})),
                ),
                &f.fence,
                |_| {
                    count += 1;
                    true
                },
            )
            .unwrap();
        assert_eq!(count, 0);
        assert!(f.gateway.blocks_purchase());
        let query = own_action(
            &f,
            HuntingRepairCommand::Query {
                quote_id: quote_id(),
            },
        );
        let dispatch = f.gateway.prepare(&query, &mut f.purchases).unwrap();
        f.gateway
            .send(&mut TestSink::default(), &query, dispatch.clone())
            .await;
        let mut bundles = Vec::new();
        f.gateway
            .receive_with(
                &frame(
                    &dispatch.request,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: true,
                        reload_required: false,
                    },
                    Some(crate::npc_purchase_projection::tests::owner()),
                ),
                &f.fence,
                |bundle| {
                    bundles.push(bundle);
                    true
                },
            )
            .unwrap();
        f.fence.revoke_owner();
        let mut world = bevy::prelude::World::new();
        assert!(
            !mir2_bevy_runtime::hunting_repair_economy::apply_native_hunting_repair_bundle(
                &mut world,
                bundles.pop().unwrap()
            )
        );
        f.gateway.drain_applied(&f.fence).unwrap();
        assert!(f.gateway.blocks_purchase());
    }

    #[tokio::test]
    async fn native_hunting_repair_host_old_raw_repair_requires_explicit_legacy_at_final_send() {
        let (sender, _) = command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        let stamp = fence.test_world_ready(3, 0);
        let command =
            GatewayCommand::Wire(NativeOutboundCommand::RepairItem { unique_id: 900_001 });
        assert!(fence.prepare(stamp, command.clone()).is_err());
        fence.0.lock().unwrap().hunting_repair_mode = HuntingRepairMode::Legacy;
        let GatewayCommand::Owned(owned) = fence.prepare(stamp, command.clone()).unwrap() else {
            unreachable!()
        };
        fence.0.lock().unwrap().hunting_repair_mode = HuntingRepairMode::HuntingV1;
        let mut sink = TestSink::default();
        assert!(matches!(
            commit_owned_frame(
                &mut sink,
                Some(&owned),
                Message::Text(
                    json!({"type":"repairItem","uniqueId":900001})
                        .to_string()
                        .into()
                )
            )
            .await,
            NativeSinkCommit::DefinitelyUnsent
        ));
        assert!(sink.0.lock().unwrap().frames.is_empty());
        assert!(fence.prepare(stamp, command).is_err());
    }
}

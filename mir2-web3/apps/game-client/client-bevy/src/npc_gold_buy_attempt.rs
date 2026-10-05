//! Host-neutral local attempt gate for ordinary NPC gold purchases.
//! Transport progress is not a Crystal purchase acknowledgement.
use std::sync::{Arc, Mutex};

use crate::inventory::InventoryModel;
use crate::npc_shop_buy::{plan_npc_gold_buy, NpcGoldBuyCommand};
use crate::shop::ShopModel;
pub use mir2_client_core::npc_gold_buy_attempt::{NpcGoldBuyAttemptOutcome,
    NpcGoldBuyAttemptPhase, NpcGoldBuyAttemptToken, NpcGoldBuyConnectionEpoch};
use mir2_client_core::npc_gold_buy_attempt::NpcGoldBuyAttemptSlot;

/// Native transport identity only; never added to BuyItem's four wire fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NpcGoldBuyTicket {
    pub run: u64, pub connection: u64, pub procedure: u64,
    pub owner_epoch: u64, pub scene_epoch: u64, pub cancellation: u64,
    pub actor: Option<u32>, pub map: Option<i32>, pub sequence: u64,
    pub local_attempt_token: u64, pub source_revision: u64,
}
impl NpcGoldBuyTicket {
    pub fn is_valid(self) -> bool {
        self.run > 0 && self.connection > 0 && self.procedure > 0
            && self.owner_epoch > 0 && self.scene_epoch > 0 && self.cancellation > 0
            && self.actor.is_some_and(|id| id > 0) && self.map.is_some()
            && self.sequence > 0 && self.local_attempt_token > 0 && self.source_revision > 0
    }
}

/// Keep all planner models; only local selection/drag state is removed.
/// Neither quantity, pagination, visibility nor read-only capacity attestation changes authority.
pub fn npc_gold_buy_model_authority(shop: &ShopModel, inventory: &InventoryModel) -> Option<String> {
    let mut shop = shop.clone();
    shop.selected_id = None;
    shop.selected_bag_slot_for_sell = None;
    shop.selected_bag_slot_for_repair = None;
    let mut inventory = inventory.clone();
    inventory.npc_gold_trade_capacity = None;
    let value = serde_json::to_string(&(shop, inventory)).ok()?;
    (value.len() <= 1024 * 1024).then_some(value)
}

#[derive(Debug, Default)]
struct GateState {
    attempts: NpcGoldBuyAttemptSlot<NpcGoldBuyTicket>,
    model: Option<String>, source_revision: u64,
    command: Option<(NpcGoldBuyAttemptToken, NpcGoldBuyCommand, String)>,
    waiter:Option<(NpcGoldBuyTicket,std::task::Waker)>,
}

impl GateState {
    fn cancelled_waiter(&mut self)->Option<std::task::Waker>{
        let cancelled=self.waiter.as_ref().is_some_and(|(ticket,_)|!self.attempts.flight()
            .is_some_and(|attempt|attempt.phase==NpcGoldBuyAttemptPhase::Bound&&attempt.ticket==Some(*ticket)));
        if cancelled{self.waiter.take().map(|(_,waker)|waker)}else{None}
    }
}

/// The gate stays shared with an admitted envelope. Its short lock is held
/// through begin_entry and start_send, so a local withdrawal cannot win between
/// the final check and transport entry. No await or receipt callback runs there.
#[derive(Clone, Debug, Default)]
pub struct NpcGoldBuyGate(Arc<Mutex<GateState>>);
impl PartialEq for NpcGoldBuyGate {
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
}
impl Eq for NpcGoldBuyGate {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NpcGoldBuyFeedback {
    pub phase: Option<NpcGoldBuyAttemptPhase>, pub pending: bool,
    pub can_reserve: bool, pub previous_unknown: usize,
}
impl NpcGoldBuyGate {
    /// The host grants only a complete actually applied source projection.

    pub fn observe_connection(&self, epoch: NpcGoldBuyConnectionEpoch) -> bool {
        let Ok(mut state) = self.0.lock() else { return false; };
        let prior = state.attempts.connection_epoch();
        let accepted = state.attempts.observe_connection(epoch);
        if accepted && prior != Some(epoch) {
            state.model = None; state.source_revision = 0; state.command = None;
        }
        let wake = state.cancelled_waiter(); drop(state);
        if let Some(wake) = wake { wake.wake(); }
        accepted
    }
    pub fn connection_epoch(&self) -> Option<NpcGoldBuyConnectionEpoch> {
        self.0.lock().ok()?.attempts.connection_epoch()
    }
    pub fn observe(&self, owner: &str, source_revision: u64, model: &str, available: bool) -> bool {
        let Ok(mut state) = self.0.lock() else { return false; };
        let authority = if owner.is_empty() || source_revision == 0 || model.is_empty() || model.len() > 1024 * 1024 {
            None
        } else { serde_json::to_string(&(owner, source_revision, model)).ok() };
        let accepted = state.attempts.connection_epoch().is_some()
            && authority.as_deref().is_some_and(|value| state.attempts.observe_authority(value));
        if accepted {
            state.model = Some(model.to_owned()); state.source_revision = source_revision;
        }
        state.attempts.set_available(accepted && available);
        let wake = state.cancelled_waiter(); drop(state);
        if let Some(wake) = wake { wake.wake(); } accepted
    }
    pub fn withdraw(&self) {
        let wake = self.0.lock().ok().and_then(|mut state| {
            state.attempts.set_available(false); state.cancelled_waiter()
        });
        if let Some(wake) = wake { wake.wake(); }
    }
    /// Register a readiness waiter without a callback under the owner's fence.
    pub fn watch(&self, ticket: NpcGoldBuyTicket, waker: &std::task::Waker) -> bool {
        let Ok(mut state) = self.0.lock() else { return false; };
        if !state.attempts.flight().is_some_and(|flight| flight.phase == NpcGoldBuyAttemptPhase::Bound
            && flight.ticket == Some(ticket)) { return false; }
        state.waiter = Some((ticket, waker.clone())); true
    }
    pub fn forget_waiter(&self, ticket: NpcGoldBuyTicket) {
        let stale = self.0.lock().ok().and_then(|mut state| {
            if state.waiter.as_ref().is_some_and(|(current,_)| *current == ticket) { state.waiter.take() } else { None }
        });
        drop(stale);
    }
    pub fn reserve(&self, shop: &ShopModel, inventory: &InventoryModel, quantity: u16)
        -> Option<(NpcGoldBuyAttemptToken, NpcGoldBuyCommand)> {
        let model = npc_gold_buy_model_authority(shop, inventory)?;
        let command = plan_npc_gold_buy(shop, inventory, quantity).command?;
        let inventory_json = serde_json::to_string(inventory).ok()?;
        if inventory_json.len() > 1024 * 1024 { return None; }
        let mut state = self.0.lock().ok()?;
        if state.model.as_deref() != Some(&model) { return None; }
        let token = state.attempts.reserve()?;
        state.command = Some((token, command.clone(), inventory_json)); Some((token, command))
    }
    pub fn source_revision(&self, token: NpcGoldBuyAttemptToken) -> Option<u64> {
        let state = self.0.lock().ok()?;
        state.attempts.flight().filter(|attempt| attempt.token == token)?;
        Some(state.source_revision)
    }
    pub fn command(&self, token: NpcGoldBuyAttemptToken) -> Option<NpcGoldBuyCommand> {
        let state = self.0.lock().ok()?;
        state.attempts.flight().filter(|attempt| attempt.token == token)?;
        state.command.as_ref().filter(|(current, _, _)| *current == token).map(|(_, command, _)| command.clone())
    }
    /// A capacity-only update cannot retire Core authority, but must invalidate
    /// the exact unentered request captured at reserve before channel publication.
    pub fn matches_inventory(&self, token: NpcGoldBuyAttemptToken, inventory: &InventoryModel) -> bool {
        let Ok(value) = serde_json::to_string(inventory) else { return false; };
        let Ok(state) = self.0.lock() else { return false; };
        state.attempts.flight().is_some_and(|attempt| attempt.token == token)
            && state.command.as_ref().is_some_and(|(current, _, captured)| *current == token && *captured == value)
    }
    pub fn bind(&self, token: NpcGoldBuyAttemptToken, ticket: NpcGoldBuyTicket) -> bool {
        let Ok(mut state) = self.0.lock() else { return false; };
        ticket.is_valid() && state.attempts.connection_epoch() == Some(NpcGoldBuyConnectionEpoch { run: ticket.run, connection: ticket.connection })
            && ticket.local_attempt_token == token.value()
            && ticket.source_revision == state.source_revision && state.attempts.bind(token, ticket)
    }
    pub fn reject_unpublished(&self, token: NpcGoldBuyAttemptToken) -> bool {
        self.0.lock().ok().is_some_and(|mut state| state.attempts.reject_unpublished(token))
    }
    pub fn receipt(&self, ticket: NpcGoldBuyTicket, outcome: NpcGoldBuyAttemptOutcome) -> bool {
        let Ok(mut state) = self.0.lock() else { return false; };
        let token = state.attempts.flight().into_iter().chain(state.attempts.history().iter())
            .find(|attempt| attempt.token.value() == ticket.local_attempt_token && attempt.ticket == Some(ticket))
            .map(|attempt| attempt.token);
        token.is_some_and(|token| state.attempts.apply_receipt(token, &ticket, outcome))
    }
    /// Caller holds the owner's transport fence before acquiring this gate.
    /// `action` must synchronously call start_send, without an await or callback.
    pub fn commit<R>(&self, ticket: NpcGoldBuyTicket, action: impl FnOnce() -> R) -> Option<R> {
        let mut state = self.0.lock().ok()?;
        let token = state.attempts.flight()?.token;
        if state.attempts.connection_epoch() != Some(NpcGoldBuyConnectionEpoch { run: ticket.run, connection: ticket.connection })
            || ticket.local_attempt_token != token.value() || ticket.source_revision != state.source_revision
            || !state.attempts.begin_entry(token, &ticket) { return None; }
        Some(action())
    }
    /// Monotonic Core observation clock for rejecting stale painted controls.
    /// This read-only value grants no purchase or transport authority.
    pub fn ui_authority_revision(&self) -> Option<u64> {
        let state = self.0.lock().ok()?;
        let revision = state.attempts.authority_revision();
        (revision > 0).then_some(revision)
    }

    pub fn feedback(&self) -> NpcGoldBuyFeedback {
        let Ok(state) = self.0.lock() else { return NpcGoldBuyFeedback::default(); };
        let phase = state.attempts.flight().or_else(|| state.attempts.history().back()).map(|attempt| attempt.phase);
        NpcGoldBuyFeedback { phase, pending: state.attempts.flight().is_some(),
            can_reserve: state.attempts.can_reserve(), previous_unknown: state.attempts.history().iter()
                .filter(|attempt| attempt.phase == NpcGoldBuyAttemptPhase::Unknown).count() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
    use crate::shop::{NpcShopServiceMode, ShopGood};
    fn models() -> (ShopModel, InventoryModel) {
        let source = CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel { item_index: 658, price: 3, stack_size: 99, item_type: 13, ..Default::default() },
            user_item: Some(CrystalUserItemModel { unique_id: 0, item_index: 658, count: 1,
                is_shop_item: true, ..Default::default() }), ..Default::default()
        };
        (ShopModel { selected_id: Some(0), service_mode: NpcShopServiceMode::Buy,
            goods: vec![ShopGood { unique_id: 0, count: 1, stock: -1, price: 4,
                purchase_rate: Some(1.5), requires_gold_buy_plan: true, tooltip_source: Some(source),
                ..Default::default() }], ..Default::default() },
            InventoryModel { gold: 100, ..Default::default() })
    }
    fn connected_gate() -> NpcGoldBuyGate {
        let gate = NpcGoldBuyGate::default();
        assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: 1 }));
        gate
    }
    fn ticket(token: NpcGoldBuyAttemptToken, revision: u64) -> NpcGoldBuyTicket {
        NpcGoldBuyTicket { run: 1, connection: 1, procedure: 1, owner_epoch: 1, scene_epoch: 1,
            cancellation: 1, actor: Some(10), map: Some(0), sequence: token.value(),
            local_attempt_token: token.value(), source_revision: revision }
    }
    #[test]
    fn npc_gold_gate_uses_planner_and_rejects_changed_full_inventory() {
        let (shop, mut inventory) = models(); let gate = connected_gate();
        let model = npc_gold_buy_model_authority(&shop, &inventory).unwrap();
        assert!(gate.observe("owner", 1, &model, true));
        inventory.gold = 99; assert!(gate.reserve(&shop, &inventory, 2).is_none());
        inventory.gold = 100; let (_, command) = gate.reserve(&shop, &inventory, 2).unwrap();
        assert_eq!(command, plan_npc_gold_buy(&shop, &inventory, 2).command.unwrap());
        assert!(gate.reserve(&shop, &inventory, 3).is_none());
    }
    #[test]
    fn npc_gold_gate_preserves_entered_on_close_and_exact_old_unsent() {
        let (shop, inventory) = models(); let gate = connected_gate();
        let model = npc_gold_buy_model_authority(&shop, &inventory).unwrap();
        gate.observe("owner", 1, &model, true); let (token, _) = gate.reserve(&shop, &inventory, 1).unwrap();
        let old = ticket(token, 1); assert!(gate.bind(token, old));
        assert_eq!(gate.commit(old, || 7), Some(7)); gate.withdraw();
        assert!(!gate.receipt(old, NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
        gate.observe("owner", 1, &model, true); assert!(gate.reserve(&shop, &inventory, 2).is_none());
        assert!(gate.observe("owner", 2, &model, true));
        assert!(gate.reserve(&shop, &inventory, 2).is_none());
        assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: 2 }));
        assert!(!gate.feedback().can_reserve);
        assert!(gate.observe("owner", 2, &model, true));
        let (fresh, _) = gate.reserve(&shop, &inventory, 2).unwrap();
        let current = NpcGoldBuyTicket { connection: 2, ..ticket(fresh, 2) };
        assert!(gate.bind(fresh, current)); assert!(!gate.receipt(old, NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
        assert_eq!(gate.commit(current, || 8), Some(8)); assert_eq!(gate.feedback().previous_unknown, 1);
    }
    #[test]
    fn npc_gold_gate_final_entry_and_local_withdrawal_share_the_gate() {
        let (shop, inventory) = models(); let gate = connected_gate();
        gate.observe("owner", 1, &npc_gold_buy_model_authority(&shop, &inventory).unwrap(), true);
        let (token, _) = gate.reserve(&shop, &inventory, 1).unwrap(); let ticket = ticket(token, 1);
        assert!(gate.bind(token, ticket));
        assert_eq!(gate.commit(ticket, || { assert!(gate.0.try_lock().is_err()); 9 }), Some(9));
        assert!(gate.receipt(ticket, NpcGoldBuyAttemptOutcome::Flushed));
        assert_eq!(gate.feedback().phase, Some(NpcGoldBuyAttemptPhase::Flushed));
        assert!(gate.commit(ticket, || panic!("duplicate entry")).is_none());
    }
    #[test]
    fn npc_gold_gate_unpublished_and_bound_rejection_allow_manual_retry() {
        let (shop, inventory) = models(); let gate = connected_gate();
        gate.observe("owner", 1, &npc_gold_buy_model_authority(&shop, &inventory).unwrap(), true);
        let (first, _) = gate.reserve(&shop, &inventory, 1).unwrap(); assert!(gate.reject_unpublished(first));
        let (second, _) = gate.reserve(&shop, &inventory, 2).unwrap(); let ticket = ticket(second, 1);
        assert!(gate.bind(second, ticket)); assert!(gate.receipt(ticket, NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
        assert!(gate.feedback().can_reserve); assert!(gate.reserve(&shop, &inventory, 3).is_some());
    }
    #[test]
    fn npc_gold_gate_capacity_evidence_is_not_authority_or_ack_but_items_are() {
        let (shop, mut inventory) = models();
        let original = npc_gold_buy_model_authority(&shop,&inventory).unwrap();
        for valid in [false,true] {
            inventory.npc_gold_trade_capacity=Some(crate::inventory::NpcGoldTradeCapacity {
                roster_valid:valid,fresh_compatible_unique_ids:vec![]});
            assert_eq!(npc_gold_buy_model_authority(&shop,&inventory).unwrap(),original);
        }
        inventory.gold-=1;assert_ne!(npc_gold_buy_model_authority(&shop,&inventory).unwrap(),original);
    }
    #[test]
    fn npc_gold_gate_requires_connection_and_rejects_ticket_from_another_pair() {
        let (shop, inventory) = models(); let gate = NpcGoldBuyGate::default();
        let model = npc_gold_buy_model_authority(&shop, &inventory).unwrap();
        assert!(!gate.observe("owner", 1, &model, true));
        assert!(gate.reserve(&shop, &inventory, 1).is_none());
        assert!(!gate.observe_connection(NpcGoldBuyConnectionEpoch { run: 0, connection: 1 }));
        assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: 1 }));
        assert!(!gate.feedback().can_reserve);
        assert!(gate.observe("owner", 1, &model, true));
        let (token, _) = gate.reserve(&shop, &inventory, 1).unwrap();
        let current = ticket(token, 1);
        for wrong in [NpcGoldBuyTicket { run: 2, ..current },
            NpcGoldBuyTicket { connection: 2, ..current }] {
            assert!(!gate.bind(token, wrong));
            assert_eq!(gate.feedback().phase, Some(NpcGoldBuyAttemptPhase::Queued));
        }
        assert!(gate.bind(token, current));
        assert_eq!(gate.commit(NpcGoldBuyTicket { connection: 2, ..current }, || panic!("wrong pair")), None::<()>);
        assert_eq!(gate.commit(current, || 1), Some(1));
    }
    #[test]
    fn npc_gold_gate_same_or_old_connection_cannot_clear_changed_owner_unknown() {
        let (shop, inventory) = models(); let gate = connected_gate();
        let model = npc_gold_buy_model_authority(&shop, &inventory).unwrap();
        assert!(gate.observe("owner", 1, &model, true));
        let (token, _) = gate.reserve(&shop, &inventory, 1).unwrap(); let old = ticket(token, 1);
        assert!(gate.bind(token, old)); assert_eq!(gate.commit(old, || ()), Some(()));
        assert!(gate.receipt(old, NpcGoldBuyAttemptOutcome::Unknown));
        assert!(gate.observe("different-owner/service", 2, &model, true));
        assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: 1 }));
        assert!(!gate.observe_connection(NpcGoldBuyConnectionEpoch { run: 0, connection: 2 }));
        assert_eq!(gate.feedback().phase, Some(NpcGoldBuyAttemptPhase::Unknown));
        assert!(gate.reserve(&shop, &inventory, 2).is_none());
        assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch { run: 2, connection: 1 }));
        assert!(gate.observe("different-owner/service", 2, &model, true));
        let (fresh, _) = gate.reserve(&shop, &inventory, 2).unwrap();
        assert!(fresh.value() > token.value());
        assert!(!gate.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: u64::MAX }));
        assert!(!gate.receipt(old, NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
        assert_eq!(gate.feedback().phase, Some(NpcGoldBuyAttemptPhase::Queued));
    }

}

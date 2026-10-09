//! Exact local ordinary-buy admission/transport tests. No socket or renderer is created.
use super::*;
use futures_util::Sink;
use mir2_client_bevy::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel,
    CrystalUserItemModel, InventoryModel, ItemModel};
use mir2_client_bevy::npc_gold_buy_attempt::{NpcGoldBuyAttemptOutcome as BuyOutcome,
    NpcGoldBuyAttemptPhase as BuyPhase, NpcGoldBuyAttemptToken, NpcGoldBuyGate, NpcGoldBuyTicket};
use mir2_client_bevy::npc_shop_buy::{plan_npc_gold_buy_json, plan_npc_gold_buy};
use mir2_client_bevy::shop::{NpcShopServiceMode, NpcShopServiceSignal, ShopGood, ShopModel};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

#[derive(Default)]
struct BuySinkState {
    pending_ready: bool, ready_error: bool, pending_flush: bool,
    start_error: bool, flush_error: bool, starts: usize, flushes: usize,
    frames: Vec<Message>, ready_hook: Option<Box<dyn FnOnce() + Send>>,
    start_hook: Option<Box<dyn FnOnce() + Send>>,
}
#[derive(Clone, Default)]
struct BuySink(Arc<Mutex<BuySinkState>>);
impl Sink<Message> for BuySink {
    type Error = &'static str;
    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let (pending, error, hook) = {
            let mut state = self.0.lock().unwrap();
            (state.pending_ready, state.ready_error, state.ready_hook.take())
        };
        if let Some(hook) = hook { hook(); }
        if pending { Poll::Pending } else if error { Poll::Ready(Err("ready failed")) }
        else { Poll::Ready(Ok(())) }
    }
    fn start_send(self: Pin<&mut Self>, frame: Message) -> Result<(), Self::Error> {
        let (error, hook) = {
            let mut state = self.0.lock().unwrap();
            state.starts += 1; state.frames.push(frame);
            (state.start_error, state.start_hook.take())
        };
        if let Some(hook) = hook { hook(); }
        if error { Err("start failed") } else { Ok(()) }
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut state = self.0.lock().unwrap(); state.flushes += 1;
        if state.pending_flush { Poll::Pending } else if state.flush_error { Poll::Ready(Err("flush failed")) }
        else { Poll::Ready(Ok(())) }
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}
#[derive(Default)]
struct BuyWake(AtomicUsize);
impl Wake for BuyWake {
    fn wake(self: Arc<Self>) { self.0.fetch_add(1, Ordering::SeqCst); }
    fn wake_by_ref(self: &Arc<Self>) { self.0.fetch_add(1, Ordering::SeqCst); }
}
fn models() -> (ShopModel, InventoryModel) {
    // Full typed raw projection, including UID zero as a valid NPC catalog UID.
    // price 1 is floor(1 * 1.5); quantity two costs 3, not display-price * two.
    let source = CrystalItemTooltipSourceModel {
        info: CrystalItemInfoModel { item_index: 658, price: 1, stack_size: 99,
            item_type: 13, ..Default::default() },
        user_item: Some(CrystalUserItemModel { unique_id: 0, item_index: 658,
            count: 1, is_shop_item: true, ..Default::default() }), ..Default::default()
    };
    (ShopModel { selected_id: Some(0), service_mode: NpcShopServiceMode::Buy,
        supports_buy: true, goods: vec![ShopGood { unique_id: 0, name: "Raw potion".into(),
            count: 1, stock: -1, price: 1, purchase_rate: Some(1.5),
            requires_gold_buy_plan: true, tooltip_source: Some(source), ..Default::default() }],
        ..Default::default() },
     InventoryModel { capacity: 46, gold: 100, items: vec![ItemModel { unique_id: Some(100),
         key: "carried".into(), name: "Owned".into(), quantity: 1, slot: 0, container: 0,
         ..Default::default() }], npc_gold_trade_capacity: None })
}
fn buy_wire(count: u16) -> NativeOutboundCommand {
    NativeOutboundCommand::BuyItem { item_index: 0, count, panel_type: 0 }
}
fn buy_frame(count: u16) -> Message {
    Message::Text(serde_json::to_string(&buy_wire(count)).unwrap().into())
}
fn run_commit(sink: &mut BuySink, owned: &OwnedGatewayCommand, count: u16) -> NativeSinkCommit {
    tokio::runtime::Builder::new_current_thread().build().unwrap()
        .block_on(commit_owned_frame(sink, Some(owned), buy_frame(count)))
}
fn owned(command: GatewayCommand) -> OwnedGatewayCommand {
    match command { GatewayCommand::Owned(command) => *command,
        other => panic!("expected production owned envelope, got {other:?}") }
}
struct BuyHarness {
    sender: GatewayCommandSender, receiver: GatewayCommandReceiver,
    fence: NativeCommandFence, stamp: NativeCommandStamp,
    shop: ShopModel, inventory: InventoryModel, gate: NpcGoldBuyGate,
}
impl BuyHarness {
    fn new() -> Self {
        let (sender, receiver) = command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        let stamp = fence.test_world_ready(10, 0);
        let (shop, inventory) = models();
        let gate = NpcGoldBuyGate::default();
        let result = Self { sender, receiver, fence, stamp, shop, inventory, gate };
        result.apply_source(); result.observe_gate(); result
    }
    fn apply_source(&self) {
        let stage = self.fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&self.inventory).unwrap());
        assert!(stage.is_some()); self.fence.finish_npc_gold_buy_inventory(stage, true);
        let stage = self.fence.stage_npc_gold_buy_catalog(&serde_json::to_value(&self.shop).unwrap(), true);
        assert!(stage.is_some()); self.fence.finish_npc_gold_buy_catalog(stage, true);
        self.fence.observe_npc_gold_buy_service(NpcShopServiceSignal {
            mode: NpcShopServiceMode::Buy, repair_rate: None }, true);
        assert!(self.fence.npc_gold_buy_source().is_some());
    }
    fn observe_gate(&self) {
        let source = self.fence.npc_gold_buy_source().unwrap();
        assert!(self.gate.observe_connection(source.connection));
        assert!(self.gate.observe(&format!("{:?}", source.stamp), source.revision, &source.model, true));
    }
    fn reserve(&self, quantity: u16) -> NpcGoldBuyAttemptToken {
        let plan = plan_npc_gold_buy(&self.shop, &self.inventory, quantity);
        assert!(plan.can_buy, "fixture planner rejected: {plan:?}");
        let (token, command) = self.gate.reserve(&self.shop, &self.inventory, quantity).unwrap();
        assert_eq!(command, plan.command.unwrap()); token
    }
    fn admit(&mut self, quantity: u16) -> (OwnedGatewayCommand, NpcGoldBuyTicket) {
        let token = self.reserve(quantity);
        let revision = self.gate.source_revision(token).unwrap();
        let ticket = self.sender.send_npc_gold_buy_with_bind(GatewayCommand::Wire(buy_wire(quantity)),
            Some(self.stamp), token, revision, self.gate.clone(), |ticket| self.gate.bind(token, ticket)).unwrap();
        (owned(self.receiver.try_recv_for_test().unwrap()), ticket)
    }
    fn fill_normal_lane(&self) {
        for id in 100..108 {
            self.sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::MailLockedItem {
                unique_id: id, locked: true }), Some(self.stamp)).unwrap();
        }
    }
}

#[test]
fn npc_gold_prebind_precedes_publish_and_wire_has_only_four_fields() {
    let mut h = BuyHarness::new(); let token = h.reserve(2);
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Queued));
    let revision = h.gate.source_revision(token).unwrap(); let mut calls = 0;
    let ticket = h.sender.send_npc_gold_buy_with_bind(GatewayCommand::Wire(buy_wire(2)),
        Some(h.stamp), token, revision, h.gate.clone(), |ticket| {
            calls += 1;
            assert!(matches!(h.receiver.try_recv_for_test(), Err(std::sync::mpsc::TryRecvError::Empty)));
            assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Queued));
            assert!(h.gate.bind(token, ticket));
            assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Bound)); true
        }).unwrap();
    assert_eq!(calls, 1); let envelope = owned(h.receiver.try_recv_for_test().unwrap());
    assert_eq!(envelope.npc_gold_buy.as_ref().unwrap().ticket, ticket);
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Bound));
    let value = serde_json::to_value(buy_wire(2)).unwrap();
    let keys: std::collections::BTreeSet<_> = value.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, std::collections::BTreeSet::from(["type", "itemIndex", "count", "panelType"]));
    assert_eq!(value, serde_json::json!({"type":"buyItem","itemIndex":0,"count":2,"panelType":0}));
    let mut sink = BuySink::default(); let fence = h.fence.clone();
    sink.0.lock().unwrap().start_hook = Some(Box::new(move || {
        assert!(fence.0.try_lock().is_err(), "start_send must share final owner fence");
    }));
    assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::Flushed));
    let state = sink.0.lock().unwrap(); assert_eq!(state.starts, 1);
    let Message::Text(text) = &state.frames[0] else { panic!("wire frame"); };
    assert_eq!(serde_json::from_str::<Value>(text.as_str()).unwrap(), value);
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Flushed));
    assert!(h.gate.feedback().pending); // Flushed is transport evidence, never a Buy ACK.
}

#[test]
fn npc_gold_full_admission_retires_only_exact_attempt_and_allows_manual_retry() {
    let h = BuyHarness::new(); h.fill_normal_lane(); let token = h.reserve(2);
    let mut bound = None;
    assert!(h.sender.send_npc_gold_buy_with_bind(GatewayCommand::Wire(buy_wire(2)), Some(h.stamp),
        token, h.gate.source_revision(token).unwrap(), h.gate.clone(), |ticket| {
            bound = Some(ticket); h.gate.bind(token, ticket)
        }).is_err());
    assert!(bound.is_some()); assert_eq!(h.gate.feedback().phase, Some(BuyPhase::DefinitelyUnsent));
    assert!(!h.gate.feedback().pending); assert!(h.gate.feedback().can_reserve);
    let (fresh, _) = h.gate.reserve(&h.shop, &h.inventory, 3).unwrap(); assert_ne!(fresh, token);
    assert!(!h.gate.receipt(bound.unwrap(), BuyOutcome::DefinitelyUnsent));
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Queued));
    let mut receiver = h.receiver;
    for _ in 0..8 { assert!(matches!(owned(receiver.try_recv_for_test().unwrap()).command,
        GatewayCommand::Wire(NativeOutboundCommand::MailLockedItem { .. }))); }
    assert!(matches!(receiver.try_recv_for_test(), Err(std::sync::mpsc::TryRecvError::Empty)));
}

#[test]
fn npc_gold_disconnected_admission_is_unsent_without_automatic_republication() {
    let h = BuyHarness::new(); let token = h.reserve(2); drop(h.receiver); let mut bound = None;
    assert!(h.sender.send_npc_gold_buy_with_bind(GatewayCommand::Wire(buy_wire(2)), Some(h.stamp),
        token, h.gate.source_revision(token).unwrap(), h.gate.clone(), |ticket| {
            bound = Some(ticket); h.gate.bind(token, ticket)
        }).is_err());
    assert!(bound.is_some()); assert_eq!(h.gate.feedback().phase, Some(BuyPhase::DefinitelyUnsent));
    assert!(h.gate.feedback().can_reserve);
    let (fresh, _) = h.gate.reserve(&h.shop, &h.inventory, 3).unwrap(); assert_ne!(fresh, token);
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Queued));
}

#[test]
fn npc_gold_wrong_wire_or_declined_bind_never_publishes() {
    for wrong in [NativeOutboundCommand::BuyItem { item_index: 658, count: 2, panel_type: 0 },
        NativeOutboundCommand::BuyItem { item_index: 0, count: 3, panel_type: 0 },
        NativeOutboundCommand::BuyItem { item_index: 0, count: 2, panel_type: 1 }] {
        let mut h = BuyHarness::new(); let token = h.reserve(2); let mut calls = 0;
        assert!(h.sender.send_npc_gold_buy_with_bind(GatewayCommand::Wire(wrong), Some(h.stamp),
            token, h.gate.source_revision(token).unwrap(), h.gate.clone(), |_| { calls += 1; false }).is_err());
        assert_eq!(calls, 0); assert!(h.gate.reject_unpublished(token));
        assert!(matches!(h.receiver.try_recv_for_test(), Err(std::sync::mpsc::TryRecvError::Empty)));
    }
    let mut h = BuyHarness::new(); let token = h.reserve(2);
    assert!(h.sender.send_npc_gold_buy_with_bind(GatewayCommand::Wire(buy_wire(2)), Some(h.stamp),
        token, h.gate.source_revision(token).unwrap(), h.gate.clone(), |_| false).is_err());
    assert!(h.gate.reject_unpublished(token));
    assert!(matches!(h.receiver.try_recv_for_test(), Err(std::sync::mpsc::TryRecvError::Empty)));
}

#[test]
fn npc_gold_owner_service_catalog_and_full_inventory_changes_refuse_bound_frame() {
    for change in 0..5 {
        let mut h = BuyHarness::new(); let (envelope, _) = h.admit(2);
        match change {
            0 => { h.fence.test_owner_change(11, 0); }
            1 => h.fence.withdraw_npc_gold_buy_service(),
            2 => { h.shop.goods[0].description = "changed full catalog".into();
                let stage = h.fence.stage_npc_gold_buy_catalog(&serde_json::to_value(&h.shop).unwrap(), true);
                h.fence.finish_npc_gold_buy_catalog(stage, true); }
            3 => { h.inventory.gold -= 1;
                let stage = h.fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&h.inventory).unwrap());
                h.fence.finish_npc_gold_buy_inventory(stage, true); }
            _ => { h.inventory.items[0].name = "same UID, changed raw inventory row".into();
                let stage = h.fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&h.inventory).unwrap());
                h.fence.finish_npc_gold_buy_inventory(stage, true); }
        }
        let mut sink = BuySink::default();
        assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent), "change {change}");
        assert_eq!(sink.0.lock().unwrap().starts, 0); assert!(!h.gate.feedback().pending);
    }
}

#[test]
fn npc_gold_same_model_failed_delivery_disables_without_revision_change() {
    for failed in 0..3 {
        let mut h = BuyHarness::new(); let (envelope, ticket) = h.admit(2);
        match failed {
            0 => { let stage = h.fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&h.inventory).unwrap());
                assert_eq!(stage.unwrap().1, ticket.source_revision);
                h.fence.finish_npc_gold_buy_inventory(stage, false); }
            1 => { let stage = h.fence.stage_npc_gold_buy_catalog(&serde_json::to_value(&h.shop).unwrap(), true);
                assert_eq!(stage.unwrap().1, ticket.source_revision);
                h.fence.finish_npc_gold_buy_catalog(stage, false); }
            _ => h.fence.observe_npc_gold_buy_service(NpcShopServiceSignal {
                mode: NpcShopServiceMode::Buy, repair_rate: None }, false),
        }
        assert_eq!(h.fence.0.lock().unwrap().npc_gold_buy.revision, ticket.source_revision);
        assert!(h.fence.npc_gold_buy_source().is_none());
        let mut sink = BuySink::default();
        assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent));
        assert_eq!(sink.0.lock().unwrap().starts, 0);
    }
}

#[test]
fn npc_gold_local_withdraw_before_entry_allows_only_fresh_manual_attempt() {
    let mut h = BuyHarness::new(); let (envelope, ticket) = h.admit(2); h.gate.withdraw();
    let mut sink = BuySink::default();
    assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent));
    assert_eq!(sink.0.lock().unwrap().starts, 0); h.observe_gate();
    let (fresh, _) = h.gate.reserve(&h.shop, &h.inventory, 3).unwrap();
    assert_ne!(fresh.value(), ticket.local_attempt_token);
    assert!(!h.gate.receipt(ticket, BuyOutcome::DefinitelyUnsent));
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Queued));
}

#[test]
fn npc_gold_pending_ready_withdraw_wakes_and_retires_without_start() {
    let mut h = BuyHarness::new(); let (envelope, _) = h.admit(2);
    let mut sink = BuySink::default(); sink.0.lock().unwrap().pending_ready = true;
    let wake = Arc::new(BuyWake::default()); let waker = Waker::from(wake.clone());
    let mut cx = Context::from_waker(&waker);
    let mut future = Box::pin(commit_owned_frame(&mut sink, Some(&envelope), buy_frame(2)));
    assert!(future.as_mut().poll(&mut cx).is_pending()); assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Bound));
    h.gate.withdraw(); assert!(wake.0.load(Ordering::SeqCst) > 0);
    assert!(matches!(future.as_mut().poll(&mut cx), Poll::Ready(NativeSinkCommit::DefinitelyUnsent)));
    drop(future); assert_eq!(sink.0.lock().unwrap().starts, 0);
    h.observe_gate(); assert!(h.gate.reserve(&h.shop, &h.inventory, 3).is_some());
}

#[test]
fn npc_gold_pending_ready_failed_source_delivery_wakes_and_retires() {
    for failed in 0..3 {
        let mut h = BuyHarness::new(); let (envelope, _) = h.admit(2);
        let mut sink = BuySink::default(); sink.0.lock().unwrap().pending_ready = true;
        let wake = Arc::new(BuyWake::default()); let waker = Waker::from(wake.clone());
        let mut cx = Context::from_waker(&waker);
        let mut future = Box::pin(commit_owned_frame(&mut sink, Some(&envelope), buy_frame(2)));
        assert!(future.as_mut().poll(&mut cx).is_pending());
        match failed {
            0 => { let stage = h.fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&h.inventory).unwrap());
                h.fence.finish_npc_gold_buy_inventory(stage, false); }
            1 => { let stage = h.fence.stage_npc_gold_buy_catalog(&serde_json::to_value(&h.shop).unwrap(), true);
                h.fence.finish_npc_gold_buy_catalog(stage, false); }
            _ => h.fence.observe_npc_gold_buy_service(NpcShopServiceSignal {
                mode: NpcShopServiceMode::Buy, repair_rate: None }, false),
        }
        assert!(wake.0.load(Ordering::SeqCst) > 0, "failed source {failed} did not wake its waiter");
        assert!(matches!(future.as_mut().poll(&mut cx), Poll::Ready(NativeSinkCommit::DefinitelyUnsent)));
        drop(future); assert_eq!(sink.0.lock().unwrap().starts, 0);
    }
}

#[test]
fn npc_gold_final_ready_recheck_refuses_change_after_poll_ready() {
    let mut h = BuyHarness::new(); let (envelope, _) = h.admit(2);
    let mut sink = BuySink::default(); let fence = h.fence.clone();
    sink.0.lock().unwrap().ready_hook = Some(Box::new(move || fence.withdraw_npc_gold_buy_service()));
    assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent));
    assert_eq!(sink.0.lock().unwrap().starts, 0);
}

#[test]
fn npc_gold_entered_close_and_quantity_change_keep_barrier_and_flushed_is_not_ack() {
    let mut h = BuyHarness::new(); let (envelope, ticket) = h.admit(2);
    let mut sink = BuySink::default(); sink.0.lock().unwrap().pending_flush = true;
    let sink_state = sink.0.clone();
    let wake = Arc::new(BuyWake::default()); let waker = Waker::from(wake);
    let mut cx = Context::from_waker(&waker);
    let mut future = Box::pin(commit_owned_frame(&mut sink, Some(&envelope), buy_frame(2)));
    assert!(future.as_mut().poll(&mut cx).is_pending()); assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Entered));
    h.gate.withdraw(); h.observe_gate();
    assert!(h.gate.reserve(&h.shop, &h.inventory, 3).is_none());
    assert!(!h.gate.receipt(ticket, BuyOutcome::DefinitelyUnsent));
    assert!(envelope.fence.0.try_lock().is_ok(), "no fence held while awaiting flush");
    // The test owns a memory Sink; this only makes its pending flush ready.
    sink_state.lock().unwrap().pending_flush = false;
    assert!(matches!(future.as_mut().poll(&mut cx), Poll::Ready(NativeSinkCommit::Flushed)));
    drop(future); assert_eq!(sink.0.lock().unwrap().starts, 1);
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Flushed));
    assert!(h.gate.feedback().pending); assert!(!h.gate.feedback().can_reserve);
    assert!(h.gate.reserve(&h.shop, &h.inventory, 3).is_none());
}

#[test]
fn npc_gold_start_and_flush_errors_are_unknown_and_never_retry() {
    for start_error in [true, false] {
        let mut h = BuyHarness::new(); let (envelope, ticket) = h.admit(2); let mut sink = BuySink::default();
        { let mut state = sink.0.lock().unwrap(); state.start_error = start_error; state.flush_error = !start_error; }
        assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::Unknown(_)));
        assert_eq!(sink.0.lock().unwrap().starts, 1); assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Unknown));
        assert!(!h.gate.receipt(ticket, BuyOutcome::DefinitelyUnsent)); h.gate.withdraw(); h.observe_gate();
        assert!(h.gate.reserve(&h.shop, &h.inventory, 3).is_none());
        assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent));
        assert_eq!(sink.0.lock().unwrap().starts, 1);
    }
}

#[test]
fn npc_gold_poll_ready_error_remains_fatal_unavailable_and_definitely_unsent() {
    let mut h = BuyHarness::new(); let (envelope, _) = h.admit(2); let mut sink = BuySink::default();
    sink.0.lock().unwrap().ready_error = true;
    assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::Unavailable(_)));
    assert_eq!(sink.0.lock().unwrap().starts, 0); assert_eq!(h.gate.feedback().phase, Some(BuyPhase::DefinitelyUnsent));
    assert!(h.gate.feedback().can_reserve);
}

#[test]
fn npc_gold_old_envelope_drop_and_old_receipt_cannot_clear_new_bound_token() {
    let mut h = BuyHarness::new(); let (old, ticket) = h.admit(2);
    h.inventory.gold = 99; h.apply_source(); h.observe_gate();
    let (fresh, fresh_ticket) = h.admit(3); assert_ne!(fresh_ticket.local_attempt_token, ticket.local_attempt_token);
    drop(old); assert!(!h.gate.receipt(ticket, BuyOutcome::DefinitelyUnsent));
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Bound));
    assert!(h.fence.0.lock().unwrap().outstanding.contains_key(&fresh.sequence));
    let mut sink = BuySink::default(); assert!(matches!(run_commit(&mut sink, &fresh, 3), NativeSinkCommit::Flushed));
    assert_eq!(sink.0.lock().unwrap().starts, 1);
}

#[test]
fn npc_gold_naked_ordinary_is_refused_even_after_service_retirement() {
    for retire in [false, true] {
        let mut h = BuyHarness::new();
        h.sender.send_with_stamp(GatewayCommand::Wire(buy_wire(2)), Some(h.stamp)).unwrap();
        let envelope = owned(h.receiver.try_recv_for_test().unwrap()); assert!(envelope.npc_gold_buy.is_none());
        if retire { h.fence.withdraw_npc_gold_buy_service(); }
        let mut sink = BuySink::default();
        assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent));
        assert_eq!(sink.0.lock().unwrap().starts, 0); assert!(!h.gate.feedback().pending);
    }
}

#[test]
fn npc_gold_real_queue_wrapper_full_false_keeps_other_buy_pending_and_token_position() {
    use mir2_client_bevy::crystal_ui::overlays::{NativePlayerUiIntent, NativePlayerUiIntentQueue};
    use mir2_client_bevy::pending_operations::{PendingOperationKey, PendingOperations};
    let h = BuyHarness::new(); let source = h.fence.npc_gold_buy_source().unwrap();
    let mut queue = NativePlayerUiIntentQueue::default();
    assert!(queue.observe_npc_gold_buy_connection(source.connection));
    assert!(queue.observe_npc_gold_buy(&format!("{:?}", source.stamp), source.revision, &source.model, true));
    let key = PendingOperationKey::Buy { item_index: 999, count: 1 };
    let mut pending = PendingOperations::default(); assert!(pending.try_begin(key.clone()));
    assert!(queue.push_intent(NativePlayerUiIntent::BuyItem { item_index: 999, count: 1 }));
    assert!(queue.push_npc_gold_buy(&h.shop, &h.inventory, 2));
    let drained = queue.drain_for_gateway_with_npc_gold_buy(); assert_eq!(drained.len(), 2);
    assert!(matches!(drained[0].0, NativePlayerUiIntent::BuyItem { item_index: 999, count: 1 }));
    assert!(drained[0].3.is_none()); assert!(drained[1].3.is_some());
    assert!(matches!(drained[1].0, NativePlayerUiIntent::NpcGoldBuy { item_index: 0, count: 2, .. }));
    let token = drained[1].3.unwrap();
    h.fill_normal_lane(); let commands = crate::input::GatewayCommands::new(h.sender.clone());
    assert!(commands.activate_world_stamp(h.stamp));
    assert!(!commands.send_npc_gold_buy(buy_wire(2), token, &mut queue));
    assert_eq!(queue.npc_gold_buy_feedback().phase, Some(BuyPhase::DefinitelyUnsent));
    assert!(pending.contains(&key)); assert_eq!(pending.len(), 1);
    assert!(queue.drain_for_gateway_with_npc_gold_buy().is_empty()); // No automatic resend.
    assert!(queue.push_npc_gold_buy(&h.shop, &h.inventory, 3));
    let next = queue.drain_for_gateway_with_npc_gold_buy(); assert_eq!(next.len(), 1);
    assert_ne!(next[0].3.unwrap(), token); assert!(pending.contains(&key));
}

#[test]
fn npc_gold_raw_inventory_presence_and_full_raw_planner_never_invent_fields() {
    let raw = serde_json::json!({"inventoryCapacity":46,"gold":100,
        "inventoryItems":[{"slot":0,"container":"bag1","uniqueId":100,"quantity":1}],
        "beltItems":[{"slot":0,"container":"belt","uniqueId":101,"quantity":1}],
        "equipmentItems":[{"slot":0,"container":"equipment","uniqueId":102,"quantity":1}]});
    assert!(full_npc_gold_buy_inventory_snapshot(&raw));
    for key in ["inventoryCapacity", "gold", "inventoryItems", "beltItems", "equipmentItems"] {
        let mut missing = raw.clone(); missing.as_object_mut().unwrap().remove(key);
        assert!(!full_npc_gold_buy_inventory_snapshot(&missing), "missing root {key}");
    }
    for array in ["inventoryItems", "beltItems", "equipmentItems"] {
        for key in ["slot", "uniqueId", "quantity"] {
            let mut missing = raw.clone(); missing[array][0].as_object_mut().unwrap().remove(key);
            assert!(!full_npc_gold_buy_inventory_snapshot(&missing), "missing {array}.{key}");
        }
        let mut count = raw.clone(); count[array][0].as_object_mut().unwrap().remove("quantity");
        count[array][0]["count"] = Value::from(1); assert!(full_npc_gold_buy_inventory_snapshot(&count));
        count[array][0].as_object_mut().unwrap().remove("count"); assert!(!full_npc_gold_buy_inventory_snapshot(&count));
    }
    let (shop, inventory) = models();
    let request = serde_json::json!({"shop":shop,"inventory":inventory,"quantity":2});
    let plan: Value = serde_json::from_str(&plan_npc_gold_buy_json(&request.to_string())).unwrap();
    assert_eq!(plan["canBuy"], true); assert_eq!(plan["totalGold"], 3);
    assert_eq!(plan["command"], serde_json::json!({"type":"buyItem","itemIndex":0,"count":2,"panelType":0}));
    for key in ["price", "stack_size", "item_index"] {
        let mut missing = request.clone(); missing["shop"]["goods"][0]["tooltip_source"]["info"].as_object_mut().unwrap().remove(key);
        let plan: Value = serde_json::from_str(&plan_npc_gold_buy_json(&missing.to_string())).unwrap();
        assert_eq!(plan["canBuy"], false); assert!(plan["command"].is_null());
    }
}

#[test]
fn npc_gold_canceled_queued_variant_cannot_rebind_or_clear_fresh_wrapper_attempt() {
    use mir2_client_bevy::crystal_ui::overlays::{NativePlayerUiIntent, NativePlayerUiIntentQueue};
    let mut h = BuyHarness::new(); let source = h.fence.npc_gold_buy_source().unwrap();
    let owner = format!("{:?}", source.stamp); let mut queue = NativePlayerUiIntentQueue::default();
    assert!(queue.observe_npc_gold_buy_connection(source.connection));
    assert!(queue.observe_npc_gold_buy(&owner, source.revision, &source.model, true));
    assert!(queue.push_npc_gold_buy(&h.shop, &h.inventory, 2));
    queue.withdraw_npc_gold_buy();
    assert!(queue.observe_npc_gold_buy(&owner, source.revision, &source.model, true));
    assert!(queue.push_npc_gold_buy(&h.shop, &h.inventory, 3));
    let drained = queue.drain_for_gateway_with_npc_gold_buy(); assert_eq!(drained.len(), 2);
    let old = drained[0].3.unwrap(); let fresh = drained[1].3.unwrap(); assert_ne!(old, fresh);
    assert!(matches!(drained[0].0, NativePlayerUiIntent::NpcGoldBuy { token, count: 2, .. } if token == old));
    assert!(matches!(drained[1].0, NativePlayerUiIntent::NpcGoldBuy { token, count: 3, .. } if token == fresh));
    let commands = crate::input::GatewayCommands::new(h.sender.clone());
    assert!(commands.activate_world_stamp(h.stamp));
    assert!(!commands.send_npc_gold_buy(buy_wire(2), old, &mut queue));
    assert_eq!(queue.npc_gold_buy_feedback().phase, Some(BuyPhase::Queued));
    assert!(matches!(h.receiver.try_recv_for_test(), Err(std::sync::mpsc::TryRecvError::Empty)));
    assert!(commands.send_npc_gold_buy(buy_wire(3), fresh, &mut queue));
    let envelope = owned(h.receiver.try_recv_for_test().unwrap());
    assert_eq!(envelope.npc_gold_buy.as_ref().unwrap().ticket.local_attempt_token, fresh.value());
    let mut sink = BuySink::default();
    assert!(matches!(run_commit(&mut sink, &envelope, 3), NativeSinkCommit::Flushed));
    assert_eq!(sink.0.lock().unwrap().starts, 1);
    assert!(queue.npc_gold_buy_feedback().pending);
    assert!(queue.drain_for_gateway_with_npc_gold_buy().is_empty());
}

fn dialog_payload(npc: u32, text: &str) -> Value {
    serde_json::json!({"activeNpcDialog": {"npcObjectId":npc,"text":text,
        "options":[{"label":"Buy","token":"[Buy]"}],"portrait":1}})
}
fn apply_inventory_only(h: &BuyHarness) {
    let stage = h.fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&h.inventory).unwrap());
    assert!(stage.is_some()); h.fence.finish_npc_gold_buy_inventory(stage, true);
}
fn initialize_dialog(h: &BuyHarness) {
    h.fence.begin_npc_gold_buy_snapshot(Some(&dialog_payload(20, "First service")));
    h.apply_source(); h.observe_gate();
}

#[test]
fn npc_gold_complete_raw_dialog_npc_or_content_change_retires_same_owner_frame() {
    for next in [dialog_payload(21, "First service"), dialog_payload(20, "Changed service")] {
        let mut h = BuyHarness::new(); initialize_dialog(&h);
        let (envelope, ticket) = h.admit(2);
        h.fence.begin_npc_gold_buy_snapshot(Some(&next)); apply_inventory_only(&h);
        assert_eq!(h.fence.0.lock().unwrap().current, h.stamp); // Same owner does not authorize a new dialog.
        assert!(h.fence.0.lock().unwrap().npc_gold_buy.revision > ticket.source_revision);
        assert!(h.fence.npc_gold_buy_source().is_none());
        let mut sink = BuySink::default();
        assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent));
        assert_eq!(sink.0.lock().unwrap().starts, 0);
        assert_eq!(h.gate.feedback().phase, Some(BuyPhase::DefinitelyUnsent));
    }
}

#[test]
fn npc_gold_closed_missing_null_or_invalid_raw_parent_cannot_retain_catalog() {
    let invalid = [None, Some(serde_json::json!({})), Some(serde_json::json!({"activeNpcDialog":null})),
        Some(serde_json::json!({"activeNpcDialog":{"npcObjectId":0}})),
        Some(serde_json::json!({"activeNpcDialog":{"npcObjectId":-1}})),
        Some(serde_json::json!({"activeNpcDialog":{"npcObjectId":1.5}})),
        Some(serde_json::json!({"activeNpcDialog":{"npcObjectId":4294967296_u64}})),
        Some(serde_json::json!({"activeNpcDialog":{"npcObjectId":"20"}})),
        Some(serde_json::json!({"activeNpcDialog":[]}))];
    for next in invalid {
        let mut h = BuyHarness::new(); initialize_dialog(&h);
        // Also cover an explicit known retirement and subsequently delivered catalog:
        // it cannot excuse a missing/null/invalid snapshot parent.
        h.fence.invalidate_npc_gold_buy_packet("NPCResponse"); h.apply_source(); h.observe_gate();
        let (envelope, ticket) = h.admit(2);
        h.fence.begin_npc_gold_buy_snapshot(next.as_ref()); apply_inventory_only(&h);
        assert!(h.fence.npc_gold_buy_source().is_none());
        assert!(h.fence.0.lock().unwrap().npc_gold_buy.revision > ticket.source_revision);
        let mut sink = BuySink::default();
        assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent));
        assert_eq!(sink.0.lock().unwrap().starts, 0);
        // Missing must retire on every receipt, even when the last parent was also missing.
        h.apply_source(); h.observe_gate(); let (second, second_ticket) = h.admit(3);
        h.fence.begin_npc_gold_buy_snapshot(next.as_ref()); apply_inventory_only(&h);
        assert!(h.fence.0.lock().unwrap().npc_gold_buy.revision > second_ticket.source_revision);
        assert!(matches!(run_commit(&mut sink, &second, 3), NativeSinkCommit::DefinitelyUnsent));
        assert_eq!(sink.0.lock().unwrap().starts, 0);
    }
}

#[test]
fn npc_gold_unchanged_complete_dialog_restores_same_lease_only_after_inventory_delivery() {
    let mut h = BuyHarness::new(); initialize_dialog(&h); let (envelope, ticket) = h.admit(2);
    h.fence.begin_npc_gold_buy_snapshot(Some(&dialog_payload(20, "First service")));
    assert!(h.fence.npc_gold_buy_source().is_none());
    assert_eq!(h.fence.0.lock().unwrap().npc_gold_buy.revision, ticket.source_revision);
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Bound));
    apply_inventory_only(&h); let source = h.fence.npc_gold_buy_source().unwrap();
    assert_eq!(source.revision, ticket.source_revision); assert_eq!(source.stamp, h.stamp);
    let mut sink = BuySink::default();
    assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::Flushed));
    assert_eq!(sink.0.lock().unwrap().starts, 1); assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Flushed));
}

#[test]
fn npc_gold_both_normal_response_catalog_snapshot_orders_keep_only_new_lease() {
    for catalog_first in [false, true] {
        let mut h = BuyHarness::new(); initialize_dialog(&h); let (old, old_ticket) = h.admit(2);
        h.fence.invalidate_npc_gold_buy_packet("NPCResponse");
        if catalog_first {
            h.fence.invalidate_npc_gold_buy_packet("NPCGoods"); h.apply_source();
        }
        h.fence.begin_npc_gold_buy_snapshot(Some(&dialog_payload(20, "Selected buy service")));
        apply_inventory_only(&h);
        if !catalog_first {
            h.fence.invalidate_npc_gold_buy_packet("NPCGoods"); h.apply_source();
        }
        let source = h.fence.npc_gold_buy_source().unwrap(); assert!(source.revision > old_ticket.source_revision);
        assert_eq!(source.stamp, old.stamp);
        h.observe_gate(); let (fresh, fresh_ticket) = h.admit(3);
        assert_ne!(fresh_ticket.local_attempt_token, old_ticket.local_attempt_token);
        let mut sink = BuySink::default();
        assert!(matches!(run_commit(&mut sink, &old, 2), NativeSinkCommit::DefinitelyUnsent));
        assert_eq!(sink.0.lock().unwrap().starts, 0);
        assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Bound));
        assert!(matches!(run_commit(&mut sink, &fresh, 3), NativeSinkCommit::Flushed));
        assert_eq!(sink.0.lock().unwrap().starts, 1);
    }
}

#[test]
fn npc_gold_real_ui_close_and_panel_replacement_cancel_before_final_entry() {
    use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
    for close in 0..4 {
        let mut h = BuyHarness::new(); let (envelope, _) = h.admit(2);
        let mut ui = NativePlayerUiState::default(); ui.toggle_npc_shop(); assert!(ui.npc_shop_open());
        ui.bind_npc_gold_buy_gate(h.gate.clone());
        let mut sink = BuySink::default();
        sink.0.lock().unwrap().ready_hook = Some(Box::new(move || {
            match close { 0 => ui.close_windows(), 1 => ui.close_all_windows(),
                2 => ui.toggle_npc_shop(), _ => ui.toggle_storage() }
            assert!(!ui.npc_shop_open());
        }));
        assert!(matches!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent), "close {close}");
        assert_eq!(sink.0.lock().unwrap().starts, 0);
        assert_eq!(h.gate.feedback().phase, Some(BuyPhase::DefinitelyUnsent));
        h.observe_gate(); assert!(h.gate.reserve(&h.shop, &h.inventory, 3).is_some());
    }
}

#[test]
fn npc_gold_real_ui_close_after_entry_preserves_no_ack_barrier() {
    use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
    for close in 0..4 {
        let mut h = BuyHarness::new(); let (envelope, ticket) = h.admit(2);
        let mut ui = NativePlayerUiState::default(); ui.toggle_npc_shop(); ui.bind_npc_gold_buy_gate(h.gate.clone());
        let mut sink = BuySink::default(); let handle = sink.0.clone(); handle.lock().unwrap().pending_flush = true;
        let waker = Waker::from(Arc::new(BuyWake::default())); let mut cx = Context::from_waker(&waker);
        let mut future = Box::pin(commit_owned_frame(&mut sink, Some(&envelope), buy_frame(2)));
        assert!(future.as_mut().poll(&mut cx).is_pending()); assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Entered));
        match close { 0 => ui.close_windows(), 1 => ui.close_all_windows(),
            2 => ui.toggle_npc_shop(), _ => ui.toggle_storage() }
        assert!(!ui.npc_shop_open()); assert!(h.gate.feedback().pending);
        h.observe_gate(); assert!(h.gate.reserve(&h.shop, &h.inventory, 3).is_none());
        assert!(!h.gate.receipt(ticket, BuyOutcome::DefinitelyUnsent));
        handle.lock().unwrap().pending_flush = false;
        assert!(matches!(future.as_mut().poll(&mut cx), Poll::Ready(NativeSinkCommit::Flushed)));
        drop(future); assert_eq!(sink.0.lock().unwrap().starts, 1);
        assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Flushed)); assert!(h.gate.feedback().pending);
    }
}

#[test]
fn npc_gold_capacity_snapshot_preserves_explicit_zero_and_strict_packed_raw() {
    let payload=json!({"inventoryCapacity":46,"gold":100,
        "inventoryItems":[{"slot":0,"container":"bag1","uniqueId":0,"quantity":1}],
        "beltItems":[{"slot":0,"uniqueId":0,"quantity":1}],
        "equipmentItems":[{"slot":0,"uniqueId":0,"quantity":1}],
        "npcGoldTradeCapacity":{"rosterValid":true,"freshCompatibleUniqueIds":[]}});
    assert!(full_npc_gold_buy_inventory_snapshot(&payload));
    let projected=transform_inventory_model(&payload);
    assert_eq!(projected["items"][0]["uniqueId"],0);assert_eq!(projected["items"][1]["uniqueId"],0);
    let inv:InventoryModel=serde_json::from_value(projected).unwrap();
    assert!(mir2_client_bevy::npc_shop_buy::valid_npc_gold_buy_inventory(&inv));
    let mut high=payload.clone();
    high["inventoryItems"][0]["uniqueId"]=u64::MAX.into();
    high["inventoryItems"][0]["tooltipSource"]=serde_json::to_value(CrystalItemTooltipSourceModel {
        info:CrystalItemInfoModel {item_index:658,price:1,stack_size:99,item_type:13,..Default::default()},
        user_item:Some(CrystalUserItemModel {unique_id:u64::MAX,item_index:658,count:1,identified:false,
            soul_bound_id:-1,wedding_ring:-1,..Default::default()}),..Default::default()}).unwrap();
    high["npcGoldTradeCapacity"]["freshCompatibleUniqueIds"]=json!([u64::MAX]);
    assert!(full_npc_gold_buy_inventory_snapshot(&high));
    let high_model:InventoryModel=serde_json::from_value(transform_inventory_model(&high)).unwrap();
    assert!(mir2_client_bevy::npc_shop_buy::valid_npc_gold_buy_inventory(&high_model));
    assert_eq!(high_model.items[0].unique_id,Some(u64::MAX));
    let mut null=payload.clone();null["beltItems"]=json!([]);null["equipmentItems"]=json!([]);
    null["npcGoldTradeCapacity"]=Value::Null;assert!(full_npc_gold_buy_inventory_snapshot(&null));
    let null_model:InventoryModel=serde_json::from_value(transform_inventory_model(&null)).unwrap();
    assert!(mir2_client_bevy::npc_shop_buy::valid_npc_gold_buy_inventory(&null_model));
    let mut missing=payload.clone();missing["inventoryItems"][0].as_object_mut().unwrap().remove("uniqueId");
    assert!(!full_npc_gold_buy_inventory_snapshot(&missing));
    for raw in [json!({"rosterValid":true}),json!({"rosterValid":true,"freshCompatibleUniqueIds":[0,0]}),
        json!({"rosterValid":true,"freshCompatibleUniqueIds":[],"extra":1}),json!({"rosterValid":true,"freshCompatibleUniqueIds":[-1]})] {
        let mut bad=payload.clone();bad["npcGoldTradeCapacity"]=raw;
        assert!(!full_npc_gold_buy_inventory_snapshot(&bad));
    }
}
#[test]
fn npc_gold_capacity_evidence_change_after_ready_rejects_exact_bound_before_start() {
    for mode in 0..3 {
        let mut h=BuyHarness::new();let (envelope,_)=h.admit(2);
        let before=h.fence.npc_gold_buy_source().unwrap();let fence=h.fence.clone();let mut inv=h.inventory.clone();
        inv.npc_gold_trade_capacity=Some(mir2_client_bevy::inventory::NpcGoldTradeCapacity {
            roster_valid:mode!=1,fresh_compatible_unique_ids:vec![]});
        let value=serde_json::to_value(inv).unwrap();let mut sink=BuySink::default();
        sink.0.lock().unwrap().ready_hook=Some(Box::new(move|| {
            let staged=fence.stage_npc_gold_buy_inventory(&value);assert!(staged.is_some());
            fence.finish_npc_gold_buy_inventory(staged,mode!=2);
        }));
        assert_eq!(run_commit(&mut sink,&envelope,2),NativeSinkCommit::DefinitelyUnsent);
        assert_eq!(sink.0.lock().unwrap().starts,0);assert!(!h.gate.feedback().pending);
        if mode==2 {assert!(h.fence.npc_gold_buy_source().is_none());}
        else {let after=h.fence.npc_gold_buy_source().unwrap();assert_eq!(before.revision,after.revision);assert_eq!(before.model,after.model);}
    }
}
#[test]
fn npc_gold_capacity_evidence_only_and_owned_packet_do_not_release_entered_or_unknown() {
    for unknown in [false,true] {
        let mut h=BuyHarness::new();let (envelope,ticket)=h.admit(2);
        let mut sink=BuySink::default();let handle=sink.0.clone();handle.lock().unwrap().pending_flush=true;
        let waker=Waker::from(Arc::new(BuyWake::default()));let mut cx=Context::from_waker(&waker);
        let mut future=Box::pin(commit_owned_frame(&mut sink,Some(&envelope),buy_frame(2)));
        assert!(future.as_mut().poll(&mut cx).is_pending());assert_eq!(h.gate.feedback().phase,Some(BuyPhase::Entered));
        if unknown {handle.lock().unwrap().pending_flush=false;handle.lock().unwrap().flush_error=true;
            assert!(matches!(future.as_mut().poll(&mut cx),Poll::Ready(NativeSinkCommit::Unknown(_))));}
        let before=h.fence.npc_gold_buy_source().unwrap();
        h.fence.invalidate_npc_gold_buy_packet("MergeItem");assert!(h.fence.npc_gold_buy_source().is_none());
        {let locked=h.fence.0.lock().unwrap();assert!(locked.npc_gold_buy.inventory.is_some());assert_eq!(locked.npc_gold_buy.revision,before.revision);}
        h.gate.withdraw();
        for valid in [false,true] {
            h.inventory.npc_gold_trade_capacity=Some(mir2_client_bevy::inventory::NpcGoldTradeCapacity {
                roster_valid:valid,fresh_compatible_unique_ids:vec![]});
            let staged=h.fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&h.inventory).unwrap());
            h.fence.finish_npc_gold_buy_inventory(staged,true);h.observe_gate();
            let now=h.fence.npc_gold_buy_source().unwrap();assert_eq!(now.revision,before.revision);assert_eq!(now.model,before.model);
            assert!(h.gate.feedback().pending);assert_eq!(h.gate.feedback().phase,Some(if unknown {BuyPhase::Unknown}else{BuyPhase::Entered}));
            assert!(h.gate.reserve(&h.shop,&h.inventory,2).is_none());
            assert!(!h.gate.receipt(ticket,BuyOutcome::DefinitelyUnsent));
        }
        drop(future);
        h.inventory.items[0].quantity=2;
        let staged=h.fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&h.inventory).unwrap());
        h.fence.finish_npc_gold_buy_inventory(staged,true);h.observe_gate();
        assert_ne!(h.fence.npc_gold_buy_source().unwrap().revision,before.revision);
        assert!(h.gate.reserve(&h.shop,&h.inventory,2).is_none(), "a changed real roster is not a processing receipt");
        assert_eq!(h.gate.feedback().phase,Some(if unknown {BuyPhase::Unknown}else{BuyPhase::Entered}));
        assert_eq!(handle.lock().unwrap().starts,1,"no automatic second transport entry");
    }
}


#[test]
fn npc_gold_capacity_changed_after_reserve_is_rejected_before_bind_and_publish() {
    let mut h=BuyHarness::new();let token=h.reserve(2);let revision=h.gate.source_revision(token).unwrap();
    h.inventory.npc_gold_trade_capacity=Some(mir2_client_bevy::inventory::NpcGoldTradeCapacity {
        roster_valid:true,fresh_compatible_unique_ids:vec![]});
    let staged=h.fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&h.inventory).unwrap());
    h.fence.finish_npc_gold_buy_inventory(staged,true);h.observe_gate();
    let mut bound=false;
    assert!(h.sender.send_npc_gold_buy_with_bind(GatewayCommand::Wire(buy_wire(2)),Some(h.stamp),token,revision,
        h.gate.clone(),|ticket|{bound=true;h.gate.bind(token,ticket)}).is_err());
    assert!(!bound);assert!(matches!(h.receiver.try_recv_for_test(),Err(std::sync::mpsc::TryRecvError::Empty)));
    assert!(h.gate.reject_unpublished(token));
    let (fresh,_)=h.admit(2);let mut sink=BuySink::default();
    assert_eq!(run_commit(&mut sink,&fresh,2),NativeSinkCommit::Flushed);assert_eq!(sink.0.lock().unwrap().starts,1);
}


#[test]
fn npc_gold_real_connection_retires_unresolved_only_after_fresh_model_and_manual_intent() {
    for phase in [BuyPhase::Entered, BuyPhase::Flushed, BuyPhase::Unknown] {
        let mut h = BuyHarness::new(); let (old, old_ticket) = h.admit(2);
        let mut sink = BuySink::default();
        if phase == BuyPhase::Entered {
            sink.0.lock().unwrap().pending_flush = true;
            let waker = Waker::from(Arc::new(BuyWake::default())); let mut cx = Context::from_waker(&waker);
            let mut pending = Box::pin(commit_owned_frame(&mut sink, Some(&old), buy_frame(2)));
            assert!(pending.as_mut().poll(&mut cx).is_pending()); drop(pending);
        } else {
            sink.0.lock().unwrap().start_error = phase == BuyPhase::Unknown;
            let result = run_commit(&mut sink, &old, 2);
            assert!(matches!((phase, result), (BuyPhase::Flushed, NativeSinkCommit::Flushed)
                | (BuyPhase::Unknown, NativeSinkCommit::Unknown(_))));
        }
        let epoch = h.gate.connection_epoch().unwrap();
        h.inventory.gold = 99; h.shop.goods[0].description.push_str(" changed catalog");
        h.stamp = h.fence.test_owner_change(11, 0); h.apply_source(); h.observe_gate();
        assert_eq!(h.gate.connection_epoch(), Some(epoch));
        assert_eq!(h.gate.feedback().phase, Some(phase));
        assert!(h.gate.reserve(&h.shop, &h.inventory, 3).is_none());
        assert!(!h.gate.receipt(old_ticket, BuyOutcome::DefinitelyUnsent));
        h.stamp = h.fence.test_reconnect(11, 0); h.apply_source();
        let source = h.fence.npc_gold_buy_source().unwrap();
        assert!(source.connection > epoch);
        assert!(h.gate.observe_connection(source.connection));
        assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Unknown));
        assert!(!h.gate.feedback().pending); assert!(!h.gate.feedback().can_reserve);
        assert!(h.gate.reserve(&h.shop, &h.inventory, 3).is_none());
        assert!(matches!(h.receiver.try_recv_for_test(), Err(std::sync::mpsc::TryRecvError::Empty)));
        h.observe_gate(); let (fresh, fresh_ticket) = h.admit(3);
        assert!(fresh_ticket.local_attempt_token > old_ticket.local_attempt_token);
        assert!(!h.gate.observe_connection(epoch)); drop(old);
        assert!(!h.gate.receipt(old_ticket, BuyOutcome::DefinitelyUnsent));
        assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Bound));
        let mut fresh_sink = BuySink::default();
        assert_eq!(run_commit(&mut fresh_sink, &fresh, 3), NativeSinkCommit::Flushed);
        assert_eq!(fresh_sink.0.lock().unwrap().starts, 1);
        assert_eq!(sink.0.lock().unwrap().starts, 1, "no automatic replay on a new socket");
    }
}

#[test]
fn npc_gold_actual_fence_pair_is_required_at_prebind_and_final_ready() {
    let mut h = BuyHarness::new(); let token = h.reserve(2);
    let revision = h.gate.source_revision(token).unwrap();
    let mut wrong_bind_rejected = false;
    let ticket = h.sender.send_npc_gold_buy_with_bind(GatewayCommand::Wire(buy_wire(2)),
        Some(h.stamp), token, revision, h.gate.clone(), |ticket| {
            let wrong = NpcGoldBuyTicket { connection: ticket.connection.checked_add(1).unwrap(), ..ticket };
            wrong_bind_rejected = !h.gate.bind(token, wrong);
            assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Queued));
            h.gate.bind(token, ticket)
        }).unwrap();
    assert!(wrong_bind_rejected);
    let envelope = owned(h.receiver.try_recv_for_test().unwrap());
    let fence = h.fence.clone(); let gate = h.gate.clone();
    let shop = h.shop.clone(); let inventory = h.inventory.clone();
    let mut sink = BuySink::default();
    sink.0.lock().unwrap().ready_hook = Some(Box::new(move || {
        fence.test_reconnect(10, 0);
        let stage = fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&inventory).unwrap());
        fence.finish_npc_gold_buy_inventory(stage, true);
        let stage = fence.stage_npc_gold_buy_catalog(&serde_json::to_value(&shop).unwrap(), true);
        fence.finish_npc_gold_buy_catalog(stage, true);
        fence.observe_npc_gold_buy_service(NpcShopServiceSignal { mode: NpcShopServiceMode::Buy, repair_rate: None }, true);
        let source = fence.npc_gold_buy_source().unwrap();
        assert!(gate.observe_connection(source.connection));
        assert!(gate.observe(&format!("{:?}", source.stamp), source.revision, &source.model, true));
    }));
    assert_eq!(run_commit(&mut sink, &envelope, 2), NativeSinkCommit::DefinitelyUnsent);
    assert_eq!(sink.0.lock().unwrap().starts, 0);
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::DefinitelyUnsent));
    assert!(!h.gate.receipt(ticket, BuyOutcome::DefinitelyUnsent));
    assert!(h.gate.reserve(&h.shop, &h.inventory, 3).is_some());
}

#[test]
fn npc_gold_pending_ready_real_reconnect_wakes_exact_old_waiter_without_start() {
    let mut h = BuyHarness::new(); let (old, ticket) = h.admit(2);
    let mut sink = BuySink::default(); sink.0.lock().unwrap().pending_ready = true;
    let wake = Arc::new(BuyWake::default()); let waker = Waker::from(wake.clone());
    let mut cx = Context::from_waker(&waker);
    let mut pending = Box::pin(commit_owned_frame(&mut sink, Some(&old), buy_frame(2)));
    assert!(pending.as_mut().poll(&mut cx).is_pending());
    h.stamp = h.fence.test_reconnect(10, 0); h.apply_source(); h.observe_gate();
    assert!(wake.0.load(Ordering::SeqCst) > 0);
    assert!(matches!(pending.as_mut().poll(&mut cx), Poll::Ready(NativeSinkCommit::DefinitelyUnsent)));
    drop(pending); assert_eq!(sink.0.lock().unwrap().starts, 0);
    let (fresh, fresh_ticket) = h.admit(3);
    h.gate.forget_waiter(ticket); drop(old);
    assert!(!h.gate.receipt(ticket, BuyOutcome::DefinitelyUnsent));
    assert_eq!(h.gate.feedback().phase, Some(BuyPhase::Bound));
    let mut next_sink = BuySink::default();
    assert_eq!(run_commit(&mut next_sink, &fresh, 3), NativeSinkCommit::Flushed);
    assert_eq!(next_sink.0.lock().unwrap().starts, 1);
    assert!(fresh_ticket.local_attempt_token > ticket.local_attempt_token);
}

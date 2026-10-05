//! Actual Native keyboard/button systems with the production local attempt gate.
//! No socket, Window, renderer, or substitute purchase state machine.
use super::*;
use crate::npc_gold_buy_attempt::{npc_gold_buy_model_authority, NpcGoldBuyGate,
    NpcGoldBuyTicket, NpcGoldBuyAttemptOutcome as Outcome, NpcGoldBuyAttemptPhase as Phase};
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::task::{Wake, Waker};

#[derive(Default)]
struct ReadinessWake(AtomicUsize);
impl Wake for ReadinessWake {
    fn wake(self: Arc<Self>) { self.0.fetch_add(1, Ordering::SeqCst); }
    fn wake_by_ref(self: &Arc<Self>) { self.0.fetch_add(1, Ordering::SeqCst); }
}
fn ordinary_good(id: u64, stack_size: u16) -> ShopGood {
    ShopGood { unique_id: id, name: format!("Potion {id}"), price: 1,
        purchase_rate: Some(1.5), requires_gold_buy_plan: true, count: 1, stock: -1,
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel { item_index: 658, name: "Potion".into(), price: 1,
                stack_size, item_type: 13, ..Default::default() },
            user_item: Some(CrystalUserItemModel { unique_id: id, item_index: 658,
                count: 1, is_shop_item: true, ..Default::default() }),
            ..Default::default()
        }), ..Default::default() }
}
fn fixture(keyboard: bool) -> App {
    let mut app = if keyboard { tests::help_keyboard_test_app() } else { tests::help_button_test_app() };
    app.init_resource::<ShopUiState>();
    if keyboard {
        tests::init_overlay_button_test_resources(&mut app);
        app.init_resource::<crate::social::SocialModel>()
            .add_systems(Update, process_overlay_buttons.before(process_overlay_keyboard));
    }
    {
        let mut shop = app.world_mut().resource_mut::<ShopModel>();
        shop.service_mode = NpcShopServiceMode::Buy;
        shop.goods = (0..10).map(|id| ordinary_good(id, 99)).collect();
        shop.selected_id = Some(0);
    }
    app.world_mut().resource_mut::<InventoryModel>().gold = 100;
    {
        let gate = app.world().resource::<NativePlayerUiIntentQueue>().npc_gold_buy_gate();
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.bind_npc_gold_buy_gate(gate);
        state.toggle_npc_shop();
        state.shop_quantity = 1;
    }
    observe(&app, 1);
    prepare_quantity_two(&mut app);
    app
}
// Simulate activate_native_command_provenance before NativePlayerUiSet::Mutate:
// the real producer reobserves the same applied source every Update, not a new ACK.
fn observe(app: &App, revision: u64) -> NpcGoldBuyGate {
    let gate = app.world().resource::<NativePlayerUiIntentQueue>().npc_gold_buy_gate();
    let model = npc_gold_buy_model_authority(app.world().resource::<ShopModel>(),
        app.world().resource::<InventoryModel>()).unwrap();
    assert!(gate.observe_connection(crate::npc_gold_buy_attempt::NpcGoldBuyConnectionEpoch { run: 1, connection: 1 }));
    assert!(gate.observe("npc-shop-ui-owner", revision, &model, true)); gate
}
fn bound(app: &App) -> (NpcGoldBuyGate, NpcGoldBuyTicket, Arc<ReadinessWake>) { bound_at(app, 1) }
fn bound_at(app: &App, source_revision: u64) -> (NpcGoldBuyGate, NpcGoldBuyTicket, Arc<ReadinessWake>) {
    let gate = observe(app, source_revision);
    let quantity = app.world().resource::<NativePlayerUiState>().shop_quantity;
    let (token, _) = gate.reserve(app.world().resource::<ShopModel>(),
        app.world().resource::<InventoryModel>(), quantity).unwrap();
    let ticket = NpcGoldBuyTicket { run: 1, connection: 1, procedure: 1, owner_epoch: 1,
        scene_epoch: 1, cancellation: 1, actor: Some(10), map: Some(0), sequence: token.value(),
        local_attempt_token: token.value(), source_revision };
    assert!(gate.bind(token, ticket));
    let wake = Arc::new(ReadinessWake::default());
    assert!(gate.watch(ticket, &Waker::from(wake.clone())));
    assert_eq!(gate.feedback().phase, Some(Phase::Bound));
    (gate, ticket, wake)
}
fn press(app: &mut App, action: OverlayButton) { press_npc_shop_button_fixture(app, action); }
fn prepare_quantity_two(app: &mut App) {
    press(app, OverlayButton::SelectShopGood(0));
    press(app, OverlayButton::ShopQuantityInc);
    assert_eq!(app.world().resource::<NativePlayerUiState>().shop_quantity, 2);
}

// Obtain the proof by the real Native host's reconciliation, not a fixed clock.
fn current_proof(app: &mut App) -> (crate::npc_shop_ui::NpcShopUiStamp, NativeNpcShopSource) {
    prepare_npc_shop_button_fixture(app).expect("actual applied common presentation")
}

fn key(app: &mut App, code: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(code);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(code); keys.clear();
}
fn assert_retired(gate: &NpcGoldBuyGate, ticket: NpcGoldBuyTicket, wake: &ReadinessWake) {
    assert!(wake.0.load(Ordering::SeqCst) > 0, "Bound readiness waiter must be woken synchronously");
    assert!(!gate.feedback().pending);
    assert_eq!(gate.feedback().phase, Some(Phase::DefinitelyUnsent));
    assert_eq!(gate.commit(ticket, || panic!("withdrawn ticket reached transport entry")), None::<()>);
}

#[test]
fn npc_shop_native_quantity_hotkeys_retire_bound_ticket_and_wake_readiness() {
    for (code, expected) in [(KeyCode::BracketRight, 3), (KeyCode::Equal, 3),
        (KeyCode::BracketLeft, 1), (KeyCode::Minus, 1)] {
        let mut app = fixture(true); let (gate, ticket, wake) = bound(&app);
        key(&mut app, code);
        assert_eq!(app.world().resource::<NativePlayerUiState>().shop_quantity, expected);
        assert_retired(&gate, ticket, &wake);
        observe(&app, 1);
        let (fresh, command) = gate.reserve(app.world().resource::<ShopModel>(),
            app.world().resource::<InventoryModel>(), expected).unwrap();
        assert_ne!(fresh.value(), ticket.local_attempt_token); assert_eq!(command.count, expected);
        assert!(app.world().resource::<NativePlayerUiIntentQueue>().intents.is_empty(), "quantity does not resend");
    }
}

#[test]
fn npc_shop_native_buttons_selection_paging_and_close_retire_only_unentered_ticket() {
    for action in [OverlayButton::ShopQuantityInc, OverlayButton::ShopQuantityDec,
        OverlayButton::SelectShopGood(1), OverlayButton::ShopPageDown, OverlayButton::ShopPageUp,
        OverlayButton::CloseShop, OverlayButton::ShopCancel, OverlayButton::CloseWindows,
        OverlayButton::ToggleNpcShop, OverlayButton::ToggleStorage] {
        let mut app = fixture(false);
        if matches!(action, OverlayButton::ShopPageUp) {
            press(&mut app, OverlayButton::ShopPageDown);
            prepare_quantity_two(&mut app);
        }
        let unrelated = crate::pending_operations::PendingOperationKey::Buy { item_index: 999, count: 1 };
        assert!(app.world_mut().resource_mut::<PendingOperations>().try_begin(unrelated.clone()));
        let (gate, ticket, wake) = bound(&app); press(&mut app, action);
        assert_retired(&gate, ticket, &wake);
        assert_eq!(app.world().resource::<PendingOperations>().len(), 1, "local withdrawal must not release another Buy key");
    }
}

#[test]
fn npc_shop_native_hotkeys_and_buttons_preserve_entered_and_unknown_same_authority_barrier() {
    for phase in [Phase::Entered, Phase::Unknown, Phase::Flushed] {
        for keyboard in [false, true] {
            let mut app = fixture(keyboard); let (gate, ticket, _) = bound(&app);
            assert_eq!(gate.commit(ticket, || 1), Some(1));
            if phase == Phase::Unknown { assert!(gate.receipt(ticket, Outcome::Unknown)); }
            if phase == Phase::Flushed { assert!(gate.receipt(ticket, Outcome::Flushed)); }
            if keyboard { key(&mut app, KeyCode::Equal); }
            else { press(&mut app, OverlayButton::ShopQuantityInc); press(&mut app, OverlayButton::CloseWindows); }
            observe(&app, 1);
            assert_eq!(gate.feedback().phase, Some(phase));
            assert!(gate.feedback().pending); assert!(!gate.feedback().can_reserve);
            assert!(gate.reserve(app.world().resource::<ShopModel>(), app.world().resource::<InventoryModel>(), 3).is_none());
            assert!(!gate.receipt(ticket, Outcome::DefinitelyUnsent));
            assert_eq!(gate.commit(ticket, || panic!("entered flight retried")), None::<()>);
            assert!(app.world().resource::<NativePlayerUiIntentQueue>().intents.is_empty());
        }
    }
}

#[test]
fn npc_shop_native_old_receipt_and_waiter_cleanup_cannot_retire_new_bound_ticket() {
    let mut app = fixture(false); let (gate, old, _) = bound(&app);
    press(&mut app, OverlayButton::ShopQuantityInc); observe(&app, 1);
    let (fresh, _) = gate.reserve(app.world().resource::<ShopModel>(), app.world().resource::<InventoryModel>(), 3).unwrap();
    let ticket = NpcGoldBuyTicket { sequence: fresh.value(), local_attempt_token: fresh.value(), ..old };
    assert!(gate.bind(fresh, ticket)); let wake = Arc::new(ReadinessWake::default());
    assert!(gate.watch(ticket, &Waker::from(wake.clone())));
    gate.forget_waiter(old); assert!(!gate.receipt(old, Outcome::DefinitelyUnsent));
    assert_eq!(gate.feedback().phase, Some(Phase::Bound)); assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    gate.withdraw(); assert!(wake.0.load(Ordering::SeqCst) > 0);
}

#[test]
fn npc_shop_native_buy_and_confirm_use_raw_flooring_uid_zero_and_one_attempt() {
    for action in [OverlayButton::ShopBuy, OverlayButton::ShopConfirm] {
        let mut app = fixture(false);
        app.world_mut().resource_mut::<ShopModel>().goods = vec![ordinary_good(0, 2)];
        app.world_mut().resource_mut::<InventoryModel>().gold = 2; observe(&app, 2);
        prepare_quantity_two(&mut app); observe(&app, 2);
        press(&mut app, action);
        assert!(app.world().resource::<NativePlayerUiIntentQueue>().intents.is_empty());
        app.world_mut().resource_mut::<InventoryModel>().gold = 3; observe(&app, 3);
        prepare_quantity_two(&mut app); observe(&app, 3);
        press(&mut app, action);
        let queue = app.world().resource::<NativePlayerUiIntentQueue>();
        assert_eq!(queue.intents.len(), 1);
        assert!(matches!(queue.intents.front(), Some(NativePlayerUiIntent::NpcGoldBuy { item_index: 0, count: 2, .. })));
        assert_eq!(queue.npc_gold_buy_feedback().phase, Some(Phase::Queued));
        assert_eq!(app.world().resource::<PendingOperations>().len(), 0);
        press(&mut app, action); assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().intents.len(), 1);
    }
}

#[test]
fn npc_shop_native_buy_and_confirm_in_same_update_publish_only_one_dedicated_intent() {
    let mut app = fixture(false); observe(&app, 1);
    let (stamp, source) = current_proof(&mut app);
    app.world_mut().spawn((Button, Interaction::Pressed, OverlayButton::ShopBuy, stamp, source.clone()));
    app.world_mut().spawn((Button, Interaction::Pressed, OverlayButton::ShopConfirm, stamp, source));
    app.update();
    let queue = app.world().resource::<NativePlayerUiIntentQueue>();
    assert_eq!(queue.intents.len(), 1);
    assert!(matches!(queue.intents.front(), Some(NativePlayerUiIntent::NpcGoldBuy { item_index: 0, count: 2, .. })));
    assert_eq!(app.world().resource::<PendingOperations>().len(), 0);
}

#[test]
fn npc_shop_native_missing_source_or_stamp_cannot_mutate_or_cancel_current_bound_attempt() {
    for (stamp_present, source_present) in [(false, false), (true, false), (false, true)] {
        let mut app = fixture(false); let (stamp, source) = current_proof(&mut app);
        let (gate, ticket, wake) = bound(&app);
        let entity = app.world_mut().spawn((Button, Interaction::Pressed, OverlayButton::ShopQuantityInc)).id();
        if stamp_present { app.world_mut().entity_mut(entity).insert(stamp); }
        if source_present { app.world_mut().entity_mut(entity).insert(source); }
        app.update();
        assert_eq!(app.world().resource::<NativePlayerUiState>().shop_quantity, 2);
        assert_eq!(gate.feedback().phase, Some(Phase::Bound));
        assert_eq!(wake.0.load(Ordering::SeqCst), 0);
        assert!(app.world().resource::<NativePlayerUiIntentQueue>().intents.is_empty());
        assert_eq!(gate.commit(ticket, || 1), Some(1), "missing node proof must not cancel the current legitimate ticket");
    }
}

#[test]
fn npc_shop_native_old_presentation_cannot_withdraw_a_fresh_bound_ticket() {
    let mut app = fixture(false); let (old_stamp, source) = current_proof(&mut app);
    press(&mut app, OverlayButton::ShopQuantityInc);
    let (new_stamp, _) = current_proof(&mut app); assert_ne!(old_stamp, new_stamp);
    let (gate, ticket, wake) = bound(&app);
    app.world_mut().spawn((Button, Interaction::Pressed, OverlayButton::ShopQuantityDec, old_stamp, source));
    app.update();
    assert_eq!(app.world().resource::<NativePlayerUiState>().shop_quantity, 3);
    assert_eq!(gate.feedback().phase, Some(Phase::Bound)); assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    assert!(app.world().resource::<NativePlayerUiIntentQueue>().intents.is_empty());
    assert_eq!(gate.commit(ticket, || 1), Some(1));
}

#[test]
fn npc_shop_native_same_revision_replacement_gate_rejects_old_node_without_cancelling_new_gate() {
    let mut app = fixture(false); let (old_stamp, old_source) = current_proof(&mut app);
    let old_gate = old_source.gate.clone();
    app.insert_resource(NativePlayerUiIntentQueue::default());
    let new_gate = observe(&app, 1); assert_ne!(old_gate, new_gate);
    assert_eq!(old_gate.ui_authority_revision(), new_gate.ui_authority_revision());
    let (_, current_source) = current_proof(&mut app); assert_eq!(current_source.gate, new_gate);
    prepare_quantity_two(&mut app);
    let (gate, ticket, wake) = bound(&app);
    app.world_mut().spawn((Button, Interaction::Pressed, OverlayButton::ShopQuantityInc, old_stamp, old_source));
    app.update();
    assert_eq!(app.world().resource::<NativePlayerUiState>().shop_quantity, 2);
    assert_eq!(gate.feedback().phase, Some(Phase::Bound)); assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    assert_eq!(gate.commit(ticket, || 1), Some(1));
    assert!(app.world().resource::<NativePlayerUiIntentQueue>().intents.is_empty());
}

#[test]
fn npc_shop_native_offpage_special_goods_handoff_the_complete_catalog_to_legacy() {
    for special in 0..3 {
        let mut app = tests::overlay_render_test_app();
        let mut shop = ShopModel { goods: (0..10).map(|id| ordinary_good(id, 99)).collect(),
            service_mode: NpcShopServiceMode::Buy, ..Default::default() };
        match special { 0 => shop.goods[9].use_pearls = true, 1 => shop.goods[9].stock = 1,
            _ => shop.goods[9].panel_type = 1 }
        let original_goods = shop.goods.clone(); app.insert_resource(shop);
        app.world_mut().resource_mut::<InventoryModel>().gold = 100;
        app.world_mut().resource_mut::<NativePlayerUiState>().core.panel = mir2_ui_core::state::UiPanel::NpcShop;
        observe(&app, 1); app.update(); let world = app.world_mut();
        assert_eq!(world.query::<&crate::crystal_ui::shop_paint::ShopPaintPanel>().iter(world).count(), 0);
        assert_eq!(world.query::<&OverlayNpcShopGoodCell>().iter(world).count(), 8);
        assert_eq!(world.resource::<ShopModel>().goods, original_goods, "legacy handoff must not filter or reorder the directory");
    }
}

#[test]
fn npc_shop_native_old_source_clock_node_cannot_cancel_a_new_observed_bound_flight() {
    let mut app = fixture(false); let (old_stamp, old_source) = current_proof(&mut app);
    observe(&app, 2); prepare_quantity_two(&mut app);
    let (current_stamp, _) = current_proof(&mut app); assert_ne!(old_stamp.source_revision, current_stamp.source_revision);
    let (gate, ticket, wake) = bound_at(&app, 2);
    app.world_mut().spawn((Button, Interaction::Pressed, OverlayButton::ShopQuantityInc, old_stamp, old_source));
    app.update();
    assert_eq!(app.world().resource::<NativePlayerUiState>().shop_quantity, 2);
    assert_eq!(gate.feedback().phase, Some(Phase::Bound)); assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    assert_eq!(gate.commit(ticket, || 1), Some(1));
}

#[test]
fn npc_shop_native_same_clock_full_model_change_withdraws_old_bound_without_resending() {
    let mut app = fixture(false); let (old_stamp, old_source) = current_proof(&mut app);
    let (gate, ticket, wake) = bound(&app);
    app.world_mut().resource_mut::<ShopModel>().goods[0].description.push_str(" replacement full source");
    app.world_mut().spawn((Button, Interaction::Pressed, OverlayButton::ShopBuy, old_stamp, old_source));
    app.update(); assert_retired(&gate, ticket, &wake);
    assert!(app.world().resource::<NativePlayerUiIntentQueue>().intents.is_empty());
    assert_ne!(app.world().resource::<ShopUiState>().common.stamp(), Some(old_stamp));
}

#[test]
fn npc_shop_native_marked_missing_raw_source_is_blocked_without_legacy_buy_fallback() {
    let mut app = fixture(false);
    {
        let mut shop = app.world_mut().resource_mut::<ShopModel>();
        shop.goods[0].purchase_rate = None; shop.goods[0].tooltip_source = None;
    }
    observe(&app, 2); press(&mut app, OverlayButton::SelectShopGood(0)); observe(&app, 2);
    assert!(app.world().resource::<ShopUiState>().common.stamp().is_some());
    for action in [OverlayButton::ShopBuy, OverlayButton::ShopConfirm] {
        press(&mut app, action);
        assert!(app.world().resource::<NativePlayerUiIntentQueue>().intents.is_empty());
        assert_eq!(app.world().resource::<PendingOperations>().len(), 0);
    }
}

#[test]
fn npc_shop_native_entered_unknown_and_flushed_survive_full_source_changes_on_same_connection() {
    for phase in [Phase::Entered, Phase::Unknown, Phase::Flushed] {
        let mut app = fixture(false); let (gate, ticket, _) = bound(&app);
        assert_eq!(gate.commit(ticket, || ()), Some(()));
        if phase == Phase::Unknown { assert!(gate.receipt(ticket, Outcome::Unknown)); }
        if phase == Phase::Flushed { assert!(gate.receipt(ticket, Outcome::Flushed)); }
        app.world_mut().resource_mut::<InventoryModel>().gold = 99;
        app.world_mut().resource_mut::<ShopModel>().goods[0].description.push_str(" new catalog");
        assert_eq!(observe(&app, 2), gate);
        press(&mut app, OverlayButton::CloseWindows);
        app.world_mut().resource_mut::<NativePlayerUiState>().toggle_npc_shop();
        observe(&app, 3); press(&mut app, OverlayButton::SelectShopGood(0));
        press(&mut app, OverlayButton::ShopBuy);
        assert_eq!(gate.feedback().phase, Some(phase)); assert!(gate.feedback().pending);
        assert!(!gate.feedback().can_reserve);
        assert!(gate.reserve(app.world().resource::<ShopModel>(),
            app.world().resource::<InventoryModel>(), 1).is_none());
        assert!(!gate.receipt(ticket, Outcome::DefinitelyUnsent));
        assert!(app.world().resource::<NativePlayerUiIntentQueue>().intents.is_empty());
        assert_eq!(gate.connection_epoch(), Some(crate::npc_gold_buy_attempt::NpcGoldBuyConnectionEpoch { run: 1, connection: 1 }));
    }
}

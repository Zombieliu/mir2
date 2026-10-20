//! Actual native selection/drag/button and renderer regressions. No network,
//! economic state mutation or success inference from a displayed inventory.
use super::*;

fn item(unique_id: u64, slot: u32) -> ItemModel {
    ItemModel {
        unique_id: Some(unique_id),
        slot,
        container: 0,
        quantity: 1,
        name: "Damaged sword".into(),
        durability_current: Some(40),
        durability_max: Some(100),
        ..Default::default()
    }
}

fn host_model(phase: HuntingRepairPhase) -> HuntingRepairModel {
    HuntingRepairModel {
        mode: HuntingRepairMode::HuntingV1,
        phase,
        quote_id: (phase != HuntingRepairPhase::Idle).then(|| "ab".repeat(32)),
        unique_id: Some(42),
        special: false,
        cost_bound_gold: Some(u64::MAX),
        bound_gold: Some(0),
        unbound_gold: Some(u64::MAX),
        expires_at_ms: Some(30_000),
        ..Default::default()
    }
}

fn button_app() -> App {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<MailComposeUi>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<NativeUiIntentQueue>()
        .init_resource::<InventoryModel>()
        .init_resource::<MailModel>()
        .init_resource::<MapModel>()
        .init_resource::<ShopModel>()
        .init_resource::<StorageModel>()
        .init_resource::<crate::social::SocialModel>()
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        });
    super::super::tests::init_overlay_button_test_resources(&mut app);
    app.insert_resource(host_model(HuntingRepairPhase::Idle));
    app.add_systems(
        Update,
        (sync_hunting_model, process_overlay_buttons).chain(),
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .toggle_npc_shop();
    *app.world_mut().resource_mut::<ShopModel>() = ShopModel {
        service_mode: NpcShopServiceMode::Repair,
        // No legacy price or rate is available. Bound repair must still Quote.
        repair_rate: None,
        ..Default::default()
    };
    app.world_mut().resource_mut::<InventoryModel>().items = vec![item(42, 4), item(43, 5)];
    app
}

fn press(app: &mut App, action: OverlayButton) {
    let button = app
        .world_mut()
        .spawn((Button, action, Interaction::Pressed))
        .id();
    app.update();
    app.world_mut().despawn(button);
}

fn drain(app: &mut App) -> Vec<NativePlayerUiIntent> {
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
}

#[test]
fn hunting_native_picker_and_hold_only_quote_without_legacy_gold_or_rate() {
    let mut app = button_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .npc_service_hold = Some(NpcShopServiceMode::Repair);
    press(&mut app, OverlayButton::ShopToggleHold);
    press(&mut app, OverlayButton::SelectBagForRepair(4));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingRepairQuote {
            unique_id: 42,
            special: false
        }]
    );
    let state = app.world().resource::<NativePlayerUiState>();
    assert_eq!(state.npc_service_hold, None);
    assert_eq!(state.shop_repair_slot, Some(4));
    assert_eq!(
        state.hunting_repair_ui.input,
        Some(HuntingRepairInput::Quote)
    );
    assert!(app.world().resource::<PendingOperations>().is_empty());
    press(&mut app, OverlayButton::ShopConfirm);
    assert!(
        drain(&mut app).is_empty(),
        "a queued Quote cannot confirm itself"
    );
}

#[test]
fn hunting_native_confirm_uses_original_quote_once_and_keeps_target() {
    let mut app = button_app();
    press(&mut app, OverlayButton::SelectBagForRepair(4));
    drain(&mut app);
    app.insert_resource(host_model(HuntingRepairPhase::Quoted));
    press(&mut app, OverlayButton::ShopRepair);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingRepairConfirm {
            quote_id: "ab".repeat(32)
        }]
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .shop_repair_slot,
        Some(4)
    );
    assert_eq!(app.world().resource::<InventoryModel>().gold, 0);
    press(&mut app, OverlayButton::ShopRepair);
    press(&mut app, OverlayButton::SelectBagForRepair(5));
    assert!(
        drain(&mut app).is_empty(),
        "no second confirmation or replacement while outcome is pending"
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .shop_repair_slot,
        Some(4)
    );
    assert!(app.world().resource::<PendingOperations>().is_empty());
}

#[test]
fn hunting_native_pending_close_and_missing_npc_item_keep_original_query() {
    let mut app = button_app();
    app.insert_resource(host_model(HuntingRepairPhase::Pending));
    press(&mut app, OverlayButton::HuntingRepairClose);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingRepairClose]
    );
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hunting_repair_ui
        .recovery_visible());
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .clear();
    assert_eq!(
        app.world().resource::<ShopModel>().service_mode,
        NpcShopServiceMode::Closed
    );
    press(&mut app, OverlayButton::HuntingRepairQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingRepairQuery {
            quote_id: "ab".repeat(32)
        }]
    );
    press(&mut app, OverlayButton::HuntingRepairQuery);
    assert!(drain(&mut app).is_empty());
    app.insert_resource(host_model(HuntingRepairPhase::Pending));
    press(&mut app, OverlayButton::HuntingRepairQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingRepairQuery {
            quote_id: "ab".repeat(32)
        }]
    );
    assert!(app.world().resource::<InventoryModel>().items.is_empty());
}

#[test]
fn hunting_native_drag_only_quotes_and_unknown_never_uses_legacy() {
    let mut state = NativePlayerUiState::default();
    state.toggle_npc_shop();
    state.npc_service_hold = Some(NpcShopServiceMode::SpecialRepair);
    let mut shop = ShopModel {
        service_mode: NpcShopServiceMode::SpecialRepair,
        repair_rate: Some(2.0),
        ..Default::default()
    };
    let inventory = InventoryModel {
        gold: u32::MAX,
        items: vec![item(42, 4)],
        ..Default::default()
    };
    let mut queue = NativePlayerUiIntentQueue::default();
    let mut pending = PendingOperations::default();
    assert!(!select_bag_dragged_for_service(
        &mut shop,
        &inventory,
        &mut state,
        4,
        42,
        &mut queue,
        &mut pending
    ));
    assert!(!submit_selection(
        &mut shop,
        &inventory,
        &mut state,
        &mut queue,
        &mut pending
    ));
    assert!(queue.drain_intents().is_empty());
    state
        .hunting_repair_ui
        .observe(&host_model(HuntingRepairPhase::Idle));
    assert!(select_bag_dragged_for_service(
        &mut shop,
        &inventory,
        &mut state,
        4,
        42,
        &mut queue,
        &mut pending
    ));
    assert_eq!(
        queue.drain_intents(),
        vec![NativePlayerUiIntent::HuntingRepairQuote {
            unique_id: 42,
            special: true
        }]
    );
    assert!(pending.is_empty());
}

#[test]
fn hunting_native_committed_is_readonly_until_ordinary_close() {
    let mut app = button_app();
    app.insert_resource(host_model(HuntingRepairPhase::Committed));
    press(&mut app, OverlayButton::SelectBagForRepair(4));
    press(&mut app, OverlayButton::ShopConfirm);
    assert!(drain(&mut app).is_empty());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hunting_repair_ui
        .recovery_visible());
    press(&mut app, OverlayButton::CloseShop);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingRepairClose]
    );
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .hunting_repair_ui
        .recovery_visible());
}

#[test]
fn hunting_native_actual_renderer_keeps_query_and_exact_wallet_without_npc_or_target() {
    let mut app = super::super::tests::overlay_render_test_app();
    app.insert_resource(host_model(HuntingRepairPhase::Pending));
    app.add_systems(
        Update,
        sync_hunting_model.before(super::super::render_overlays),
    );
    app.update();
    let world = app.world_mut();
    let actions: Vec<_> = world
        .query_filtered::<&OverlayButton, With<Button>>()
        .iter(world)
        .copied()
        .collect();
    assert!(actions.contains(&OverlayButton::HuntingRepairQuery));
    assert!(actions.contains(&OverlayButton::HuntingRepairClose));
    assert!(!actions.iter().any(|action| matches!(
        action,
        OverlayButton::ShopRepair | OverlayButton::ShopSRepair | OverlayButton::ShopToggleHold
    )));
    let labels: Vec<_> = world
        .query::<(&Text, &TextFont)>()
        .iter(world)
        .map(|(text, font)| {
            (
                text.0.clone(),
                matches!(&font.font_size, bevy::text::FontSize::Px(size) if *size >= 11.0),
            )
        })
        .collect();
    assert!(labels
        .iter()
        .any(|(text, readable)| text == "Bound: 0\nUnbound: 18446744073709551615" && *readable));
    assert!(labels.iter().any(|(text, _)| text.contains("Pending")));
    assert!(world.resource::<InventoryModel>().items.is_empty());
}

#[test]
fn hunting_native_queue_backpressure_preserves_the_original_confirmation() {
    let mut queue = NativePlayerUiIntentQueue::default();
    let confirm = NativePlayerUiIntent::HuntingRepairConfirm {
        quote_id: "ab".repeat(32),
    };
    assert!(queue.push_transient_unique(confirm.clone()));
    for index in 1..MAX_QUEUED {
        assert!(queue.push_intent(NativePlayerUiIntent::Chat {
            message: index.to_string()
        }));
    }
    assert!(
        !queue.push_transient_unique(NativePlayerUiIntent::HuntingRepairQuery {
            quote_id: "ab".repeat(32)
        })
    );
    assert!(queue.push_intent(NativePlayerUiIntent::Chat {
        message: "new ordinary message".into()
    }));
    let emitted = queue.drain_intents();
    assert_eq!(emitted.len(), MAX_QUEUED);
    assert_eq!(emitted.first(), Some(&confirm));
    assert_eq!(
        emitted
            .iter()
            .filter(|intent| matches!(intent, NativePlayerUiIntent::HuntingRepairConfirm { .. }))
            .count(),
        1
    );
}

// This fixture starts at the Runtime-to-UI boundary: apply_service_signal plus
// UiSurfaceSignals is the actual validated delivery consumed by the production
// sync/keyboard systems. It does not simulate a wire response or an Applied.
fn service_takeover_app() -> App {
    let mut app = button_app();
    app.world_mut().resource_mut::<NativePlayerUiState>().reset_session();
    *app.world_mut().resource_mut::<ShopModel>() = ShopModel::default();
    app.init_resource::<NpcDialogModel>()
        .init_resource::<UiSurfaceSignals>()
        .init_resource::<SessionResetRevision>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<KeyboardInput>()
        .add_systems(Update,
            (sync_npc_dialog_inventory_location, process_overlay_keyboard)
                .chain().before(process_overlay_buttons));
    set_takeover_dialog(&mut app, Some(4990));
    app.update();
    app
}

fn set_takeover_dialog(app: &mut App, npc: Option<u32>) {
    let mut dialog = app.world_mut().resource_mut::<NpcDialogModel>();
    if let Some(npc_object_id) = npc {
        dialog.apply(crate::quest_model::NpcDialogUpdate {
            npc_object_id, npc_name: Some("Ordinary repair NPC".into()),
            lines: vec!["Choose a service".into()], options: Vec::new(),
            open: true, replace: true,
        });
    } else {
        // A complete authoritative empty dialog, as the service snapshot sends.
        *dialog = NpcDialogModel::default();
    }
}

fn deliver_takeover_service(app: &mut App, mode: NpcShopServiceMode) {
    let signal = NpcShopServiceSignal { mode, repair_rate: Some(1.5) };
    assert!(app.world_mut().resource_mut::<ShopModel>().apply_service_signal(signal));
    app.world_mut().resource_mut::<UiSurfaceSignals>().npc_shop_open_requested = true;
}

fn assert_repair_surface(app: &App, mode: NpcShopServiceMode) {
    let state = app.world().resource::<NativePlayerUiState>();
    assert!(state.npc_shop_open());
    assert!(state.inventory_open());
    assert_eq!((state.inventory_window.left, state.inventory_window.top), (445.0, 0.0));
    assert_eq!(app.world().resource::<ShopModel>().service_mode, mode);
}

#[test]
fn hunting_native_service_takeover_preserves_split_and_same_update_delivery() {
    for mode in [NpcShopServiceMode::Repair, NpcShopServiceMode::SpecialRepair] {
        for split_updates in [true, false] {
            let mut app = service_takeover_app();
            deliver_takeover_service(&mut app, mode);
            if split_updates {
                app.update();
                assert_repair_surface(&app, mode);
                assert!(!app.world().resource::<UiSurfaceSignals>().npc_shop_open_requested);
            }
            set_takeover_dialog(&mut app, None);
            app.update();
            assert_repair_surface(&app, mode);
            for _ in 0..3 { app.update(); assert_repair_surface(&app, mode); }
            assert!(drain(&mut app).is_empty(), "opening a service never submits assets");
            // The preserved surface still goes through ordinary Bag selection:
            // even zero legacy gold only requests a server quote.
            app.world_mut().resource_mut::<InventoryModel>().gold = 0;
            press(&mut app, OverlayButton::SelectBagForRepair(4));
            assert_eq!(drain(&mut app), vec![NativePlayerUiIntent::HuntingRepairQuote {
                unique_id: 42, special: mode == NpcShopServiceMode::SpecialRepair,
            }]);
        }
    }
}

#[test]
fn hunting_native_service_takeover_does_not_survive_explicit_close() {
    for close_before_snapshot in [true, false] {
        let mut app = service_takeover_app();
        deliver_takeover_service(&mut app, NpcShopServiceMode::Repair);
        app.update();
        if !close_before_snapshot { set_takeover_dialog(&mut app, None); app.update(); }
        press(&mut app, OverlayButton::CloseShop);
        set_takeover_dialog(&mut app, None);
        for _ in 0..3 { app.update(); }
        assert!(!app.world().resource::<NativePlayerUiState>().npc_shop_open());
        assert_eq!(app.world().resource::<ShopModel>().service_mode, NpcShopServiceMode::Closed);
        assert!(!drain(&mut app).iter().any(|intent| matches!(intent,
            NativePlayerUiIntent::HuntingRepairConfirm { .. }
                | NativePlayerUiIntent::RepairItem { .. } | NativePlayerUiIntent::SRepairItem { .. })));
    }
}

#[test]
fn hunting_native_service_takeover_respects_explicit_parent_exit_epoch() {
    let mut app = service_takeover_app();
    deliver_takeover_service(&mut app, NpcShopServiceMode::Repair);
    app.update();
    // Every existing Quest dialog Close/@Exit calls this retirement before
    // closing its model. It must win over the prior service delivery.
    app.world().resource::<NativePlayerUiState>().withdraw_bound_npc_gold_buy();
    app.world_mut().resource_mut::<NpcDialogModel>().close();
    app.update();
    assert!(!app.world().resource::<NativePlayerUiState>().npc_shop_open());
    assert_eq!(app.world().resource::<ShopModel>().service_mode, NpcShopServiceMode::Closed);
}

#[test]
fn hunting_native_service_takeover_new_dialog_cannot_reuse_old_service() {
    for after_handoff in [true, false] {
        let mut app = service_takeover_app();
        deliver_takeover_service(&mut app, NpcShopServiceMode::Repair);
        app.update();
        if after_handoff { set_takeover_dialog(&mut app, None); app.update(); }
        set_takeover_dialog(&mut app, Some(4991));
        app.update();
        assert!(!app.world().resource::<NativePlayerUiState>().npc_shop_open());
        assert_eq!(app.world().resource::<ShopModel>().service_mode, NpcShopServiceMode::Closed);
        set_takeover_dialog(&mut app, None);
        app.update();
        assert!(!app.world().resource::<NativePlayerUiState>().npc_shop_open());
    }
}

#[test]
fn hunting_native_service_takeover_session_reset_retires_pending_handoff() {
    let mut app = service_takeover_app();
    deliver_takeover_service(&mut app, NpcShopServiceMode::Repair);
    app.update();
    app.world_mut().resource_mut::<NativePlayerUiState>().reset_session();
    *app.world_mut().resource_mut::<ShopModel>() = ShopModel::default();
    set_takeover_dialog(&mut app, None);
    app.world_mut().resource_mut::<SessionResetRevision>().0 += 1;
    app.update();
    assert!(!app.world().resource::<NativePlayerUiState>().npc_shop_open());
    assert_eq!(app.world().resource::<ShopModel>().service_mode, NpcShopServiceMode::Closed);
    set_takeover_dialog(&mut app, Some(4990));
    app.update();
    set_takeover_dialog(&mut app, None);
    app.update();
    assert!(!app.world().resource::<NativePlayerUiState>().npc_shop_open());
}

//! Actual ordinary Bag/button/intent and renderer tests. These prove local UI
//! behavior only, without a native socket, economic transaction or Applied proof.
use super::*;

fn item(uid: u64, slot: u32, count: u32) -> ItemModel {
    ItemModel {
        unique_id: Some(uid),
        container: 0,
        slot,
        key: "test-material".into(),
        name: "Test material".into(),
        quantity: count,
        ..Default::default()
    }
}

fn host(phase: HuntingMaterialPhase, quantity: u64) -> HuntingMaterialModel {
    let created = now_ms().saturating_sub(100);
    HuntingMaterialModel {
        actor_scope: Some(
            crate::hunting_material::HuntingMaterialActorScope::new(
                "fixture".into(),
                "alice".into(),
                0,
            )
            .unwrap(),
        ),
        protocol_v1: true,
        phase,
        quote_id: (!matches!(
            phase,
            HuntingMaterialPhase::Ready | HuntingMaterialPhase::Unknown
        ))
        .then(|| "ab".repeat(32)),
        unique_id: Some(42),
        input_count: Some(12),
        quantity: Some(quantity),
        unit_gold: Some(100),
        gold_equivalent: Some(100 * quantity),
        fee_unbound_gold: Some(2 * quantity),
        license_atoms_debit: Some(100 * quantity),
        valuation_version: Some("fixture-fixed-values-v1".into()),
        created_at_ms: Some(created),
        expires_at_ms: Some(created + 30_000),
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
    app.insert_resource(host(HuntingMaterialPhase::Ready, 1));
    app.add_systems(Update, (sync_model, process_overlay_buttons).chain());
    app.world_mut().resource_mut::<InventoryModel>().items = vec![item(42, 4, 12), item(43, 5, 3)];
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

fn select(app: &mut App) {
    press(
        app,
        OverlayButton::InspectBagSnapshot {
            slot: 4,
            unique_id: Some(42),
        },
    );
}

#[test]
fn hunting_material_bag_selection_never_auto_quotes_and_typed_quote_preserves_full_partial_qty() {
    let mut app = button_app();
    select(&mut app);
    assert!(drain(&mut app).is_empty());
    press(&mut app, OverlayButton::HuntingMaterialQuantityInc);
    press(&mut app, OverlayButton::HuntingMaterialQuantityInc);
    assert!(drain(&mut app).is_empty());
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuote {
            unique_id: 42,
            quantity: 3
        }]
    );
    assert_eq!(
        app.world().resource::<InventoryModel>().items[0].quantity,
        12
    );
    assert_eq!(app.world().resource::<InventoryModel>().gold, 0);
    assert!(app.world().resource::<PendingOperations>().is_empty());
    for action in [
        OverlayButton::HuntingMaterialQuote,
        OverlayButton::HuntingMaterialConfirm,
        OverlayButton::HuntingMaterialAll,
    ] {
        press(&mut app, action);
    }
    assert!(
        drain(&mut app).is_empty(),
        "unacknowledged Quote is never auto-confirmed"
    );

    let mut whole = button_app();
    select(&mut whole);
    press(&mut whole, OverlayButton::HuntingMaterialAll);
    press(&mut whole, OverlayButton::HuntingMaterialQuote);
    assert_eq!(
        drain(&mut whole),
        vec![NativePlayerUiIntent::HuntingMaterialQuote {
            unique_id: 42,
            quantity: 12
        }]
    );
}

#[test]
fn hunting_material_bag_quantity_clamps_to_actual_stack_and_resets_for_new_selection() {
    let mut app = button_app();
    select(&mut app);
    press(&mut app, OverlayButton::HuntingMaterialQuantityDec);
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .hunting_material_ui
            .quantity,
        1
    );
    press(&mut app, OverlayButton::HuntingMaterialAll);
    press(&mut app, OverlayButton::HuntingMaterialQuantityInc);
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .hunting_material_ui
            .quantity,
        12
    );
    press(
        &mut app,
        OverlayButton::InspectBagSnapshot {
            slot: 5,
            unique_id: Some(43),
        },
    );
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuote {
            unique_id: 43,
            quantity: 1
        }]
    );
}

#[test]
fn hunting_material_bag_confirm_is_single_original_intent_and_unchanged_frames_do_not_ack() {
    let mut app = button_app();
    select(&mut app);
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    drain(&mut app);
    app.insert_resource(host(HuntingMaterialPhase::Quoted, 1));
    press(&mut app, OverlayButton::HuntingMaterialConfirm);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialConfirm {
            quote_id: "ab".repeat(32)
        }]
    );
    for _ in 0..3 {
        app.update();
    }
    press(&mut app, OverlayButton::HuntingMaterialConfirm);
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert!(
        drain(&mut app).is_empty(),
        "unchanged host resource cannot acknowledge entered intent"
    );
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hunting_material_ui
        .guard
        .pending());
    assert_eq!(app.world().resource::<InventoryModel>().gold, 0);
    assert_eq!(
        app.world().resource::<InventoryModel>().items[0].quantity,
        12
    );
}

#[test]
fn hunting_material_bag_pending_close_missing_item_and_npc_keep_only_original_query() {
    let mut app = button_app();
    app.insert_resource(host(HuntingMaterialPhase::Pending, 3));
    press(&mut app, OverlayButton::HuntingMaterialClose);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialClose]
    );
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .clear();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .inspect = None;
    assert_eq!(
        app.world().resource::<ShopModel>().service_mode,
        NpcShopServiceMode::Closed
    );
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuery {
            quote_id: "ab".repeat(32)
        }]
    );
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    press(&mut app, OverlayButton::HuntingMaterialConfirm);
    assert!(drain(&mut app).is_empty());
    app.insert_resource(host(HuntingMaterialPhase::Pending, 3));
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuery {
            quote_id: "ab".repeat(32)
        }]
    );
}

#[test]
fn hunting_material_bag_confirm_rejects_changed_item_count_quantity_or_expired_quote() {
    for change in 0..5 {
        let mut app = button_app();
        select(&mut app);
        press(&mut app, OverlayButton::HuntingMaterialQuote);
        drain(&mut app);
        let mut quote = host(HuntingMaterialPhase::Quoted, 1);
        if change == 4 {
            quote.expires_at_ms = Some(now_ms() - 1);
            quote.created_at_ms = quote.expires_at_ms.map(|expiry| expiry - 30_000);
        }
        app.insert_resource(quote);
        if change == 0 {
            app.world_mut().resource_mut::<InventoryModel>().items[0].quantity = 11;
        }
        if change == 1 {
            app.world_mut().resource_mut::<InventoryModel>().items[0].name =
                "Changed server presentation".into();
        }
        if change == 2 {
            app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(99);
        }
        if change == 3 {
            press(&mut app, OverlayButton::HuntingMaterialQuantityInc);
        }
        press(&mut app, OverlayButton::HuntingMaterialConfirm);
        assert!(
            drain(&mut app).is_empty(),
            "changed source or quote case {change}"
        );
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .hunting_material_ui
            .guard
            .pending());
    }
}

#[test]
fn hunting_material_bag_unsupported_zero_uid_duplicate_and_nonbag_never_fall_back_to_shop() {
    for invalid in 0..5 {
        let mut app = button_app();
        select(&mut app);
        match invalid {
            0 => {
                app.insert_resource(HuntingMaterialModel::default());
            }
            1 => {
                app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(0);
            }
            2 => {
                let duplicate = app.world().resource::<InventoryModel>().items[0].clone();
                app.world_mut()
                    .resource_mut::<InventoryModel>()
                    .items
                    .push(duplicate);
            }
            3 => {
                app.world_mut().resource_mut::<InventoryModel>().items[0].quantity = 0;
            }
            _ => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .inspect
                    .as_mut()
                    .unwrap()
                    .container = 2;
            }
        }
        press(&mut app, OverlayButton::HuntingMaterialQuote);
        press(&mut app, OverlayButton::HuntingMaterialConfirm);
        assert!(
            drain(&mut app).is_empty(),
            "unsupported source case {invalid}"
        );
        assert!(app.world().resource::<PendingOperations>().is_empty());
    }
}

#[test]
fn hunting_material_bag_other_pending_lane_and_backpressure_do_not_consume_input() {
    let mut app = button_app();
    select(&mut app);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .hunting_repair_ui
        .observe(&HuntingRepairModel {
            mode: HuntingRepairMode::HuntingV1,
            phase: HuntingRepairPhase::Pending,
            quote_id: Some("cd".repeat(32)),
            ..Default::default()
        });
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert!(drain(&mut app).is_empty());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hunting_material_ui
        .guard
        .input()
        .is_none());
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .hunting_repair_ui
        .observe(&HuntingRepairModel::default());
    for value in 0..MAX_QUEUED {
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .push_intent(NativePlayerUiIntent::Chat {
                message: value.to_string()
            }));
    }
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hunting_material_ui
        .guard
        .input()
        .is_none());
    let old = drain(&mut app);
    assert_eq!(old.len(), MAX_QUEUED);
    assert!(!old
        .iter()
        .any(|intent| matches!(intent, NativePlayerUiIntent::HuntingMaterialQuote { .. })));
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuote {
            unique_id: 42,
            quantity: 1
        }]
    );
}

#[test]
fn hunting_material_typed_queue_backpressure_never_evicts_or_duplicates_material_confirm() {
    let mut queue = NativePlayerUiIntentQueue::default();
    let confirm = NativePlayerUiIntent::HuntingMaterialConfirm {
        quote_id: "ab".repeat(32),
    };
    assert_eq!(confirm.pending_key(), None);
    assert!(queue.push_transient_unique(confirm.clone()));
    assert!(!queue.push_intent(confirm.clone()));
    for value in 1..MAX_QUEUED {
        assert!(queue.push_intent(NativePlayerUiIntent::Chat {
            message: value.to_string()
        }));
    }
    assert!(
        !queue.push_transient_unique(NativePlayerUiIntent::HuntingMaterialQuery {
            quote_id: "ab".repeat(32)
        })
    );
    assert!(queue.push_intent(NativePlayerUiIntent::Chat {
        message: "ordinary replacement".into()
    }));
    let intents = queue.drain_intents();
    assert_eq!(intents.len(), MAX_QUEUED);
    assert_eq!(intents.first(), Some(&confirm));
    assert_eq!(
        intents.iter().filter(|intent| **intent == confirm).count(),
        1
    );
}

#[test]
fn hunting_material_actual_recovery_renderer_has_query_without_npc_inventory_or_success_guess() {
    let mut app = super::super::tests::overlay_render_test_app();
    app.insert_resource(host(HuntingMaterialPhase::Pending, 3));
    app.add_systems(
        Update,
        (sync_model, render_recovery)
            .chain()
            .after(super::super::render_overlays),
    );
    app.update();
    let world = app.world_mut();
    let actions: Vec<_> = world
        .query_filtered::<&OverlayButton, With<Button>>()
        .iter(world)
        .copied()
        .collect();
    assert!(actions.contains(&OverlayButton::HuntingMaterialQuery));
    assert!(actions.contains(&OverlayButton::HuntingMaterialClose));
    assert!(!actions.contains(&OverlayButton::HuntingMaterialConfirm));
    let labels: Vec<_> = world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.clone())
        .collect();
    assert!(labels
        .iter()
        .any(|label| label == "Fee: 6 existing unbound gold"));
    assert!(labels
        .iter()
        .any(|label| label == "Total value: 300 gold equivalent"));
    assert!(labels
        .iter()
        .any(|label| label == "Hunting permits: 0.000300"));
    assert!(labels.iter().any(|label| label.contains("Pending")));
    assert!(!labels.iter().any(|label| label.contains("completed")));
    assert!(world.resource::<InventoryModel>().items.is_empty());
    assert_eq!(
        world.resource::<ShopModel>().service_mode,
        NpcShopServiceMode::Closed
    );
}

fn scoped_host(account: &str, phase: HuntingMaterialPhase, quantity: u64) -> HuntingMaterialModel {
    let mut model = host(phase, quantity);
    model.actor_scope =
        Some(HuntingMaterialActorScope::new("fixture".into(), account.into(), 0).unwrap());
    if account != "alice" && model.quote_id.is_some() {
        model.quote_id = Some("cd".repeat(32));
    }
    model
}

#[test]
fn hunting_material_actual_ui_actor_a_b_a_partitions_both_originals_quantity_and_source() {
    let mut app = button_app();
    select(&mut app);
    press(&mut app, OverlayButton::HuntingMaterialQuantityInc);
    press(&mut app, OverlayButton::HuntingMaterialQuantityInc);
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuote {
            unique_id: 42,
            quantity: 3
        }]
    );
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Quoted, 3));
    press(&mut app, OverlayButton::HuntingMaterialConfirm);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialConfirm {
            quote_id: "ab".repeat(32)
        }]
    );
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Pending, 3));
    press(&mut app, OverlayButton::HuntingMaterialClose);
    drain(&mut app);

    // Identical Bag UID/count on another actor must not reuse A's quantity,
    // quoted source, input latch or original recovery presentation.
    app.insert_resource(scoped_host("bob", HuntingMaterialPhase::Ready, 1));
    app.update();
    {
        let service = &app
            .world()
            .resource::<NativePlayerUiState>()
            .hunting_material_ui;
        assert!(!service.guard.pending() && !service.guard.recovery_visible());
        assert!(service.quote_source.is_none());
        assert_eq!(service.quantity, 1);
    }
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert!(
        drain(&mut app).is_empty(),
        "B must never issue A's original Query"
    );
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuote {
            unique_id: 42,
            quantity: 1
        }]
    );
    app.insert_resource(scoped_host("bob", HuntingMaterialPhase::Quoted, 1));
    press(&mut app, OverlayButton::HuntingMaterialConfirm);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialConfirm {
            quote_id: "cd".repeat(32)
        }]
    );
    app.insert_resource(scoped_host("bob", HuntingMaterialPhase::Pending, 1));
    app.update();

    // A generic new Ready view is not evidence that A's submitted original
    // disappeared. It restores only A's original Query and saved quantity.
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Ready, 1));
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuery {
            quote_id: "ab".repeat(32)
        }]
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .hunting_material_ui
            .quantity,
        3
    );
    press(&mut app, OverlayButton::HuntingMaterialConfirm);
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert!(drain(&mut app).is_empty());
    app.insert_resource(scoped_host("bob", HuntingMaterialPhase::Ready, 1));
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuery {
            quote_id: "cd".repeat(32)
        }]
    );
}

#[test]
fn hunting_material_actual_ui_unknown_scope_hides_actions_without_erasing_original_actor() {
    let mut app = button_app();
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Pending, 3));
    app.update();
    app.insert_resource(HuntingMaterialModel::default());
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert!(drain(&mut app).is_empty());
    {
        let service = &app
            .world()
            .resource::<NativePlayerUiState>()
            .hunting_material_ui;
        assert!(
            !service.guard.recovery_visible(),
            "unbound view must not show A's query"
        );
        assert!(service.retained.values().any(|actor| actor.guard.pending()
            && actor.guard.original_quote_id() == Some("ab".repeat(32).as_str())));
    }
    let mut unknown = scoped_host("alice", HuntingMaterialPhase::Unknown, 1);
    unknown.protocol_v1 = false;
    app.insert_resource(unknown);
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert!(drain(&mut app).is_empty());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hunting_material_ui
        .guard
        .pending());
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Ready, 1));
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuery {
            quote_id: "ab".repeat(32)
        }]
    );
}

#[test]
fn hunting_material_actual_ui_actor_capacity_fails_closed_and_never_evicts_pending() {
    let mut app = button_app();
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Pending, 3));
    app.update();
    for value in 1..MAX_DISPLAY_ACTORS {
        app.insert_resource(scoped_host(
            &format!("actor-{value}"),
            HuntingMaterialPhase::Ready,
            1,
        ));
        app.update();
    }
    app.insert_resource(scoped_host("overflow", HuntingMaterialPhase::Ready, 1));
    select(&mut app);
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert!(drain(&mut app).is_empty());
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .hunting_material_ui
            .retained
            .len(),
        MAX_DISPLAY_ACTORS
    );
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Ready, 1));
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuery {
            quote_id: "ab".repeat(32)
        }]
    );
}

#[test]
fn hunting_material_actual_ui_session_reset_preserves_original_but_retires_active_display() {
    let mut app = button_app();
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Pending, 3));
    app.update();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .reset_session();
    // The unchanged old host resource is not a fresh delivery after reset.
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert!(drain(&mut app).is_empty());
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .hunting_material_ui
        .guard
        .recovery_visible());
    let mut other_character = scoped_host("alice", HuntingMaterialPhase::Ready, 1);
    other_character
        .actor_scope
        .as_mut()
        .unwrap()
        .character_index = 1;
    app.insert_resource(other_character);
    select(&mut app);
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuote {
            unique_id: 42,
            quantity: 1
        }]
    );
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Ready, 1));
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuery {
            quote_id: "ab".repeat(32)
        }]
    );
}

#[test]
fn hunting_material_actual_ui_exact_game_shop_receipt_reset_keeps_original_and_confirm_history() {
    let mut app = button_app();
    select(&mut app);
    press(&mut app, OverlayButton::HuntingMaterialQuote);
    drain(&mut app);
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Quoted, 1));
    press(&mut app, OverlayButton::HuntingMaterialConfirm);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialConfirm {
            quote_id: "ab".repeat(32),
        }]
    );
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Pending, 1));
    app.update();

    let receipt = crate::game_shop::GameShopReceipt {
        protocol: "nativeGameShopReceiptV1".into(),
        request_id: "gs-material-reset".into(),
        success: false,
        g_index: 31,
        quantity: 2,
        price_type: 1,
        new_stock_level: None,
        mail_id: None,
        code: Some(crate::game_shop::GameShopFailureCode::InsufficientCurrency),
    };
    let mut reset = SessionResetRevision::default();
    let mut revisions = AuthoritativeModelRevisions::default();
    let mut pending = PendingOperations::default();
    assert!(pending.try_begin(PendingOperationKey::DeleteMail(9)));
    let mut preservation = SessionResetGameShopPreservation::default();
    assert!(
        crate::pending_operations::request_session_reset_preserving_exact_game_shop_receipt(
            &mut reset,
            &mut revisions,
            &mut pending,
            &mut preservation,
            receipt.clone(),
        )
    );
    app.insert_resource(reset)
        .insert_resource(pending)
        .insert_resource(preservation)
        .init_resource::<OverlayResetTracker>()
        .init_resource::<InventoryOperationFeedback>()
        .init_resource::<UiEffectQueue>()
        .add_systems(
            Update,
            crate::pending_operations::apply_overlay_session_reset.before(sync_model),
        );
    app.update();
    {
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(!state.hunting_material_ui.guard.recovery_visible());
        assert_eq!(
            state.core.game_shop_pending,
            crate::game_shop::GameShopRequest::new(
                receipt.request_id.clone(),
                receipt.g_index,
                receipt.quantity,
                receipt.price_type,
            )
        );
        assert!(!state.core.game_shop_unknown);
        assert!(state.core.game_shop_last_receipt.is_none());
        let pending = app.world().resource::<PendingOperations>();
        assert_eq!(pending.len(), 1);
        assert!(pending.contains(&PendingOperationKey::GameShop(receipt.request_id.clone())));
    }
    // The special reset's subsequent unbound metadata must not erase A's
    // original or make it visible/actionable for an unknown actor.
    app.insert_resource(HuntingMaterialModel::default());
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert!(drain(&mut app).is_empty());
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Ready, 1));
    press(&mut app, OverlayButton::HuntingMaterialQuery);
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::HuntingMaterialQuery {
            quote_id: "ab".repeat(32),
        }]
    );
    // Even after the host explicitly publishes completion, a later stale
    // Quoted delivery cannot reset the consumed original Confirm guard.
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Committed, 1));
    app.update();
    app.insert_resource(scoped_host("alice", HuntingMaterialPhase::Quoted, 1));
    app.update();
    let state = app.world().resource::<NativePlayerUiState>();
    assert!(!state.hunting_material_ui.guard.pending());
    assert!(!state.hunting_material_ui.guard.can_confirm(42, 1, now_ms()));
    assert_eq!(
        state
            .core
            .game_shop_pending
            .as_ref()
            .map(|request| request.request_id.as_str()),
        Some("gs-material-reset")
    );
}

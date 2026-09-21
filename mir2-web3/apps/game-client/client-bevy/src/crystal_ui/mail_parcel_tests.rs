use super::*;

fn source_item(unique_id: u64, slot: u32) -> ItemModel {
    ItemModel {
        unique_id: Some(unique_id),
        slot,
        container: 0,
        quantity: 1,
        icon: 7,
        tooltip_source: Some(crate::inventory::CrystalItemTooltipSourceModel {
            info: crate::inventory::CrystalItemInfoModel {
                item_index: 77,
                stack_size: 1,
                image: 7,
                ..default()
            },
            user_item: Some(crate::inventory::CrystalUserItemModel {
                unique_id,
                item_index: 77,
                count: 1,
                ..default()
            }),
            ..default()
        }),
        ..default()
    }
}

#[test]
fn parcel_renderer_uses_independent_source_frame_cells_and_bag_offset() {
    let mut app = super::tests::overlay_render_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.mail_inventory_visible = true;
        state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Receiver".into(),
            message: "first\nsecond".into(),
            attachment_unique_ids: vec![41],
            ..default()
        });
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Parcel;
    app.world_mut().resource_mut::<InventoryModel>().items = vec![source_item(41, 0), source_item(42, 1)];
    app.update();

    let world = app.world_mut();
    let node = world
        .query_filtered::<&Node, With<OverlayMailComposeParcel>>()
        .single(world)
        .expect("source parcel root");
    assert_eq!(node.display, Display::Flex);
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (Val::Px(326.0), Val::Px(0.0), Val::Px(236.0), Val::Px(384.0)),
    );
    assert!(world.query::<&ImageNode>().iter(world).any(|image| {
        image.image.path().is_some_and(|path| path.to_string() == "original-ui/Title/674.png")
    }));
    let mut buttons = world.query::<(&OverlayButton, &Node)>();
    let (_, send) = buttons
        .iter(world)
        .find(|(button, _)| matches!(button, OverlayButton::SubmitMail))
        .expect("source send");
    assert_eq!(
        (send.left, send.top, send.width, send.height),
        (Val::Px(30.0), Val::Px(350.0), Val::Px(76.0), Val::Px(25.0)),
    );
    let (_, cell) = buttons
        .iter(world)
        .find(|(button, _)| matches!(button, OverlayButton::MailParcelSlot(0)))
        .expect("first source parcel cell");
    assert_eq!(
        (cell.left, cell.top, cell.width, cell.height),
        (Val::Px(27.0), Val::Px(311.0), Val::Px(35.0), Val::Px(31.0)),
    );
    let bag = world
        .query_filtered::<&Node, With<OverlayInventory>>()
        .single(world)
        .expect("ordinary bag root");
    assert_eq!((bag.left, bag.top), (Val::Px(0.0), Val::Px(0.0)));
}

#[test]
fn open_parcel_uses_recipient_prompt_without_overwriting_a_saved_draft() {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<MailComposeUi>()
        .init_resource::<InventoryModel>()
        .init_resource::<mail_parcel::MailParcelUi>()
        .init_resource::<MailServiceInbox>()
        .init_resource::<PendingOperations>()
        .add_systems(Update, process_mail_service_inbox);
    app.world_mut()
        .resource_mut::<MailServiceInbox>()
        .push(MailServiceEvent::OpenParcel);
    app.update();
    let state = app.world().resource::<NativePlayerUiState>();
    let compose = app.world().resource::<MailComposeUi>();
    let prompt = compose.recipient_prompt.as_ref().expect("source recipient prompt");
    assert_eq!(prompt.target_kind, MailComposeKind::Parcel);
    assert!(state.mail_recipient_prompt_active);

    drop(compose);
    drop(state);
    {
        let mut compose = app.world_mut().resource_mut::<MailComposeUi>();
        compose.recipient_prompt = None;
        compose.parcel_draft = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Saved".into(),
            message: "do not retarget".into(),
            ..default()
        });
    }
    app.world_mut()
        .resource_mut::<MailServiceInbox>()
        .push(MailServiceEvent::OpenParcel);
    app.update();
    assert!(app.world().resource::<MailComposeUi>().recipient_prompt.is_none());
    assert_eq!(
        app.world().resource::<MailComposeUi>().last_notice.as_deref(),
        Some("Finish or cancel the current parcel first"),
    );
}

#[test]
fn quote_singleflight_ignores_stale_cost_and_requires_current_live_ids() {
    let mut ui = mail_parcel::MailParcelUi::default();
    let inventory = InventoryModel { items: vec![source_item(41, 0)], ..default() };
    let mut draft = mir2_ui_core::state::MailComposeDraft {
        gold: 100,
        attachment_unique_ids: vec![41],
        ..default()
    };
    assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
    draft.gold = 200;
    // This response arrived before the synchronizer has refreshed its cached
    // desired fingerprint; the live draft still rejects the old quote.
    ui.apply_cost(15, Some(&draft), &inventory);
    assert!(!ui.quote_is_current(&draft, &inventory));
    assert!(ui.begin_quote(&draft, &inventory, 3).is_some());
    ui.apply_cost(25, Some(&draft), &inventory);
    assert!(ui.quote_is_current(&draft, &inventory));

    let stale_inventory = InventoryModel::default();
    assert!(!ui.quote_is_current(&draft, &stale_inventory));
}

#[test]
fn quote_is_invalidated_by_live_attachment_pricing_changes() {
    let mut ui = mail_parcel::MailParcelUi::default();
    let mut item = source_item(41, 0);
    item.tooltip_source.as_mut().expect("concrete source").info.price = 1_000;
    let mut inventory = InventoryModel { items: vec![item], ..default() };
    let draft = mir2_ui_core::state::MailComposeDraft {
        attachment_unique_ids: vec![41],
        ..default()
    };

    assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
    ui.apply_cost(50, Some(&draft), &inventory);
    assert!(ui.quote_is_current(&draft, &inventory));

    // A V2 item refresh can change a stack's current quantity without
    // changing its UID. MailCost prices the whole live item, so that old
    // server result must no longer enable Send.
    inventory.items[0].quantity = 2;
    assert!(!ui.quote_is_current(&draft, &inventory));
    assert!(ui.begin_quote(&draft, &inventory, 2).is_some());
}

#[test]
fn parcel_lock_is_exact_and_local_cancel_recovers_if_echoes_are_dropped() {
    let inventory = InventoryModel {
        items: vec![source_item(41, 0), source_item(42, 1)],
        ..default()
    };
    let mut ui = mail_parcel::MailParcelUi::default();
    let mut draft = mir2_ui_core::state::MailComposeDraft::default();

    assert!(ui.attach(&mut draft, &inventory, 41));
    assert!(ui.blocks_item(41), "selection locks before the true echo");
    assert!(!ui.blocks_item(42));
    ui.apply_lock_receipt(41, true);
    ui.apply_lock_receipt(9_999, true);
    assert!(ui.blocks_item(41));
    assert!(!ui.blocks_item(9_999), "unknown echoed IDs cannot create a permanent lock");

    assert_eq!(ui.detach_at(&mut draft, 0), Some(41));
    assert!(!ui.blocks_item(41), "cancel is locally authoritative when false echo is lost");
    ui.apply_lock_receipt(41, true);
    ui.apply_lock_receipt(41, false);
    assert!(!ui.blocks_item(41), "late cancel echoes stay harmless after release");

    assert!(ui.attach(&mut draft, &inventory, 41));
    ui.apply_lock_receipt(41, false);
    assert!(ui.blocks_item(41), "a delayed old false cannot unlock a reselected item");
    ui.complete_send();
    assert!(!ui.blocks_item(41));
    draft.attachment_unique_ids.clear();

    assert!(ui.attach(&mut draft, &inventory, 41));
    assert!(ui.session_reset(1).is_none());
    assert!(ui.session_reset(2).is_some());
    ui.apply_lock_receipt(41, true);
    assert!(!ui.blocks_item(41), "reset rejects a previous session's true echo");
}

#[test]
fn parcel_locked_bag_item_is_dimmed_and_cannot_start_a_drag() {
    let inventory = InventoryModel { items: vec![source_item(41, 0)], ..default() };
    let mut state = NativePlayerUiState::default();
    let mut parcel = mail_parcel::MailParcelUi::default();
    let mut draft = mir2_ui_core::state::MailComposeDraft::default();
    assert!(parcel.attach(&mut draft, &inventory, 41));

    let cursor = Vec2::new(
        state.inventory_window.left + INVENTORY_GRID_ORIGIN.x as f32 + 2.0,
        state.inventory_window.top + INVENTORY_GRID_ORIGIN.y as f32 + 2.0,
    );
    assert!(
        inventory_item_drag_at_cursor(&state, &inventory, Some(&parcel), cursor).is_none(),
        "the precise selected UID cannot enter the bag drag pipeline"
    );
    assert!(
        inventory_item_drag_at_cursor(&state, &inventory, None, cursor).is_some(),
        "ordinary operation locking remains separate from mail selection"
    );

    let mut app = super::tests::overlay_render_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.mail_inventory_visible = true;
        state.core.mail_compose = Some(draft);
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Parcel;
    app.world_mut().resource_mut::<InventoryModel>().items = inventory.items;
    *app.world_mut().resource_mut::<mail_parcel::MailParcelUi>() = parcel;
    app.update();

    let dim_gray = Color::srgba(105.0 / 255.0, 105.0 / 255.0, 105.0 / 255.0, 0.8);
    assert!(
        app.world_mut()
            .query::<&ImageNode>()
            .iter(app.world())
            .any(|image| image.color == dim_gray),
        "a locked carried cell keeps Crystal's DimGray 0.8 presentation"
    );
}

#[test]
fn parcel_selected_uid_blocks_the_overlay_action_chain() {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<MailComposeUi>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<NativeUiIntentQueue>()
        .init_resource::<InventoryModel>()
        .init_resource::<MailModel>()
        .init_resource::<ShopModel>()
        .init_resource::<StorageModel>()
        .init_resource::<crate::social::SocialModel>();
    app.insert_resource(NativeShellModel {
        screen: NativeShellScreen::InGame,
        ..default()
    });
    super::tests::init_overlay_button_test_resources(&mut app);
    app.add_systems(Update, process_overlay_buttons);
    app.world_mut().resource_mut::<InventoryModel>().items = vec![source_item(41, 0)];
    {
        let inventory = app.world().resource::<InventoryModel>().clone();
        let mut parcel = app.world_mut().resource_mut::<mail_parcel::MailParcelUi>();
        assert!(parcel.attach(
            &mut mir2_ui_core::state::MailComposeDraft::default(),
            &inventory,
            41,
        ));
    }

    fn press(app: &mut App, button: OverlayButton) {
        let entity = app.world_mut().spawn((Interaction::Pressed, button, Button)).id();
        app.update();
        app.world_mut().despawn(entity);
    }

    press(&mut app, OverlayButton::InspectBag(0));
    assert!(app.world().resource::<NativePlayerUiState>().inspect.is_none());

    {
        let item = app.world().resource::<InventoryModel>().items[0].clone();
        app.world_mut().resource_mut::<NativePlayerUiState>().inspect = Some(inspect_from_item(&item));
    }
    press(&mut app, OverlayButton::UseInspected);
    press(&mut app, OverlayButton::EquipInspected);
    press(&mut app, OverlayButton::DropInspected);
    press(&mut app, OverlayButton::SplitInspected);
    press(&mut app, OverlayButton::ArmMoveInspected);
    press(&mut app, OverlayButton::ArmMergeInspected);
    {
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(state.drop_confirmation.is_none());
        assert!(state.inventory_operation.is_none());
    }
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());

    app.world_mut().resource_mut::<NativePlayerUiState>().inventory_delete_mode = true;
    press(&mut app, OverlayButton::InspectBag(0));
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .inventory_delete_prompt
        .is_none());

    app.world_mut().resource_mut::<NativePlayerUiState>().inventory_operation = Some(
        InventoryOperationDraft::Move {
            source_slot: 1,
            unique_id: 42,
        },
    );
    press(&mut app, OverlayButton::InspectBag(0));
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty(), "a non-mail source cannot swap into a locked attachment cell");
}

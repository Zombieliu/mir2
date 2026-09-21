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
    app.world_mut().resource_mut::<InventoryModel>().items = vec![source_item(41, 0)];
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

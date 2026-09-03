//! Headless source geometry and control-state tests; no GUI or screenshots.
use super::super::tests as overlay_tests;
use super::*;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel};
use crate::social::{SocialModel, SocialPendingOperation, TradeItemModel};
use bevy::ecs::system::RunSystemOnce;
use serde_json::json;

fn social() -> SocialModel {
    let mut model = SocialModel::default();
    assert!(model.apply_packet("TradeAccept", &json!({"name":"Guest"})));
    model
}

fn state(model: &SocialModel) -> NativePlayerUiState {
    let mut state = NativePlayerUiState::default();
    assert!(state.trade_dialog.observe(model));
    state
}

fn app() -> App {
    let mut app = overlay_tests::overlay_render_test_app();
    *app.world_mut().resource_mut::<SocialModel>() = social();
    app.world_mut().resource_mut::<UiReadModel>().player.name = Some("Host".into());
    app.world_mut().run_system_once(sync).unwrap();
    app
}

fn rect(node: &Node) -> (Val, Val, Val, Val) {
    (node.left, node.top, node.width, node.height)
}
fn values(r: CrystalRect) -> (Val, Val, Val, Val) {
    (
        Val::Px(r.left),
        Val::Px(r.top),
        Val::Px(r.width),
        Val::Px(r.height),
    )
}
fn descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut result = vec![root];
    let mut at = 0;
    while at < result.len() {
        if let Some(children) = world.get::<Children>(result[at]) {
            result.extend(children.iter());
        }
        at += 1;
    }
    result
}

#[test]
fn trade_dialog_ecs_has_original_pair_positions_and_twenty_uncompacted_cells() {
    let mut app = app();
    app.update();
    let world = app.world_mut();
    let windows = world
        .query::<(&TradeSide, &Node)>()
        .iter(world)
        .map(|(s, n)| (*s, rect(n)))
        .collect::<Vec<_>>();
    assert_eq!(windows.len(), 2);
    assert!(windows.contains(&(TradeSide::Own, values(OWN_RECT))));
    assert!(windows.contains(&(TradeSide::Guest, values(GUEST_RECT))));
    for side in [TradeSide::Own, TradeSide::Guest] {
        let mut slots = world
            .query::<(&TradeCell, &Node)>()
            .iter(world)
            .filter(|(c, _)| c.side == side)
            .map(|(c, n)| {
                assert_eq!(rect(n), values(cell_rect(c.slot).unwrap()));
                assert_eq!(n.overflow, Overflow::DEFAULT);
                c.slot
            })
            .collect::<Vec<_>>();
        slots.sort_unstable();
        assert_eq!(slots, (0..10).collect::<Vec<_>>());
    }
    assert!(cell_rect(10).is_none());
    assert_eq!(
        cell_rect(1).unwrap(),
        CrystalRect::new(10.0, 72.0, 36.0, 32.0)
    );
    assert_eq!(
        cell_rect(2).unwrap(),
        CrystalRect::new(47.0, 39.0, 36.0, 32.0)
    );
    let inventory = world
        .query_filtered::<&Node, With<OverlayInventory>>()
        .single(world)
        .unwrap();
    assert_ne!(inventory.display, Display::None);
    assert_eq!(
        (inventory.left, inventory.top),
        (Val::Px(708.0), Val::Px(0.0))
    );
}

#[test]
fn trade_dialog_ecs_uses_original_art_names_gold_and_only_own_confirm_close() {
    let mut app = app();
    {
        let mut model = app.world_mut().resource_mut::<SocialModel>();
        model.trade.my_gold = 1_234_567;
        model.trade.partner_gold = 98_765;
    }
    app.update();
    let world = app.world_mut();
    for frame in [389, 390] {
        let mut images = world.query::<(&ImageNode, &Node)>();
        let (_, node) = images
            .iter(world)
            .find(|(i, _)| {
                i.image
                    .path()
                    .is_some_and(|p| p.to_string() == format!("original-ui/Prguse/{frame}.png"))
            })
            .unwrap();
        assert_eq!((node.width, node.height), (Val::Px(204.0), Val::Px(152.0)));
    }
    for (action, r, library, normal, hover, pressed) in [
        (OverlayButton::TradeConfirm, CONFIRM, "Title", 520, 521, 522),
        (OverlayButton::TradeCancel, CLOSE, "Prguse2", 360, 361, 362),
    ] {
        let mut buttons = world.query::<(&OverlayButton, &Node, &CrystalImageButton)>();
        let matches = buttons
            .iter(world)
            .filter(|(b, _, _)| **b == action)
            .collect::<Vec<_>>();
        assert_eq!(
            matches.len(),
            1,
            "guest has no constructed confirm or close button"
        );
        let (_, node, button) = matches[0];
        assert_eq!(rect(node), values(r));
        assert_eq!(
            button.assets.normal,
            format!("original-ui/{library}/{normal}.png")
        );
        assert_eq!(
            button.assets.hover,
            format!("original-ui/{library}/{hover}.png")
        );
        assert_eq!(
            button.assets.pressed,
            format!("original-ui/{library}/{pressed}.png")
        );
    }
    for (side, r, label_text) in [
        (TradeSide::Own, OWN_NAME, "Host"),
        (TradeSide::Guest, GUEST_NAME, "Guest"),
    ] {
        let mut names = world.query::<(&TradeName, &Node, &Children)>();
        let (_, node, children) = names
            .iter(world)
            .find(|(name, _, _)| name.0 == side)
            .unwrap();
        assert_eq!(rect(node), values(r));
        assert_eq!(world.get::<Text>(children[0]).unwrap().0, label_text);
        assert_eq!(node.justify_content, JustifyContent::Center);
    }
    for (side, text) in [(TradeSide::Own, "1,234,567"), (TradeSide::Guest, "98,765")] {
        let mut labels = world.query::<(&TradeGold, &Children)>();
        let (_, children) = labels.iter(world).find(|(g, _)| g.0 == side).unwrap();
        assert_eq!(world.get::<Text>(children[0]).unwrap().0, text);
    }
    let gold_buttons = world
        .query::<(&OverlayButton, &Node)>()
        .iter(world)
        .filter(|(b, _)| **b == OverlayButton::TradeGoldOffer)
        .map(|(_, n)| rect(n))
        .collect::<Vec<_>>();
    assert_eq!(gold_buttons, vec![values(GOLD)]);
    assert!(!world
        .query::<&Text>()
        .iter(world)
        .any(|t| t.0.contains("Offer 100 gold") || t.0.contains("Confirmed:")));
}

#[test]
fn trade_dialog_sparse_cells_keep_hints_current_quantity_and_no_text_rows() {
    let mut app = app();
    let source = CrystalItemTooltipSourceModel {
        info: CrystalItemInfoModel {
            name: "Poison".into(),
            item_index: 27,
            item_type: 8,
            shape: 2,
            stack_size: 150,
            image: 2961,
            ..default()
        },
        ..default()
    };
    {
        let mut model = app.world_mut().resource_mut::<SocialModel>();
        model.trade.partner_items = vec![None; 10];
        model.trade.partner_items[9] = Some(TradeItemModel {
            unique_id: Some(123),
            name: Some("Poison".into()),
            count: 50,
            tooltip_source: Some(source),
            ..default()
        });
    }
    app.update();
    super::super::primary_item_image_tests::load_original_images(app.world_mut());
    let world = app.world_mut();
    let cells = world
        .query::<(Entity, &TradeCell, Option<&CrystalItemHint>)>()
        .iter(world)
        .map(|(e, c, h)| (e, c.side, c.slot, h.is_some()))
        .collect::<Vec<_>>();
    assert_eq!(cells.iter().filter(|(_, _, _, hint)| *hint).count(), 1);
    let (entity, _, _, _) = cells
        .iter()
        .find(|(_, side, slot, _)| *side == TradeSide::Guest && *slot == 9)
        .unwrap();
    let children = descendants(world, *entity);
    assert!(children
        .iter()
        .any(|child| world.get::<Text>(*child).is_some_and(|t| t.0 == "50")));
    let image = children
        .iter()
        .find_map(|child| world.get::<ImageNode>(*child))
        .unwrap();
    assert_eq!(
        image.image.path().unwrap().to_string(),
        "original-ui/Items/3671.png"
    );
    assert!(!world
        .query::<&Text>()
        .iter(world)
        .any(|text| text.0.contains("Poison x")));
}

#[test]
fn trade_dialog_accept_shows_inventory_without_switching_back_on_every_snapshot() {
    let mut app = app();
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().core.panel,
        mir2_ui_core::state::UiPanel::Inventory
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::None;
    app.world_mut().run_system_once(sync).unwrap();
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().core.panel,
        mir2_ui_core::state::UiPanel::None
    );
    app.world_mut()
        .resource_mut::<SocialModel>()
        .apply_packet("TradeGold", &json!({"amount":70}));
    app.world_mut().run_system_once(sync).unwrap();
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().core.panel,
        mir2_ui_core::state::UiPanel::None
    );
    assert!(
        app.world()
            .resource::<NativePlayerUiState>()
            .trade_dialog
            .open
    );
}

#[test]
fn trade_dialog_unlock_updates_even_when_partner_amount_is_equal() {
    let mut model = social();
    let mut ui = state(&model);
    for _ in 0..2 {
        ui.trade_dialog.local_locked = Some(true);
        model.apply_packet("TradeGold", &json!({"amount":0}));
        assert!(!ui.trade_dialog.observe(&model));
        assert!(!ui.trade_dialog.locked(&model.trade));
        assert!(ui.trade_dialog.open);
    }
}

#[test]
fn trade_dialog_unrelated_social_packet_in_same_frame_cannot_hide_source_unlock() {
    let mut model = social();
    let mut ui = state(&model);
    ui.trade_dialog.local_locked = Some(true);
    assert!(model.apply_packet("TradeGold", &json!({"amount":0})));
    assert!(model.apply_packet("SwitchGroup", &json!({"allowGroup":true})));
    assert_eq!(model.last_event.as_ref().unwrap().packet, "SwitchGroup");
    ui.trade_dialog.observe(&model);
    assert!(!ui.trade_dialog.locked(&model.trade));
    assert!(ui.trade_dialog.open);
}

#[test]
fn trade_dialog_owner_change_and_terminal_packets_dispose_amount_prompt_not_positions() {
    for (packet, payload) in [
        ("TradeConfirm", json!({})),
        ("TradeCancel", json!({"unlock":false})),
        ("TradeAccept", json!({"name":"Other"})),
    ] {
        let mut model = social();
        let mut ui = state(&model);
        ui.trade_dialog.positions[0] = Vec2::new(70.0, 80.0);
        assert!(open_gold(&mut ui, &model, 500));
        model.apply_packet(packet, &payload);
        ui.trade_dialog.observe(&model);
        assert!(ui.trade_dialog.gold_prompt.is_none());
        assert_eq!(ui.trade_dialog.positions[0], Vec2::new(70.0, 80.0));
        assert_eq!(ui.trade_dialog.open, packet == "TradeAccept");
    }
}

#[test]
fn trade_gold_same_partner_new_exchange_rejects_stale_prompt_even_before_ui_sync() {
    for sync_first in [false, true] {
        let mut model = social();
        let mut ui = state(&model);
        let mut queue = NativePlayerUiIntentQueue::default();
        assert!(open_gold(&mut ui, &model, 500));
        model.apply_packet("TradeConfirm", &json!({}));
        model.apply_packet("TradeAccept", &json!({"name":"Guest"}));
        if sync_first {
            assert!(ui.trade_dialog.observe(&model));
            assert!(ui.trade_dialog.gold_prompt.is_none());
        }
        assert!(!confirm_gold(&mut ui, &mut model, 500, &mut queue));
        assert!(queue.drain_intents().is_empty());
        assert_eq!(model.trade.my_gold, 0);
    }
}

#[test]
fn trade_dialog_locked_confirm_uses_source_521_without_a_partner_button() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .trade_dialog
        .local_locked = Some(true);
    app.update();
    let world = app.world_mut();
    let buttons = world
        .query::<(&OverlayButton, &CrystalImageButton)>()
        .iter(world)
        .filter(|(action, _)| **action == OverlayButton::TradeConfirm)
        .map(|(_, button)| button.assets.normal.clone())
        .collect::<Vec<_>>();
    assert_eq!(buttons, vec!["original-ui/Title/521.png"]);
}

#[test]
fn trade_dialog_cancel_hides_pair_sends_one_request_and_keeps_backpack_and_offers() {
    let mut model = social();
    let mut ui = state(&model);
    ui.core.panel = mir2_ui_core::state::UiPanel::Inventory;
    model.trade.my_gold = 12;
    let before = model.trade.clone();
    let mut queue = NativePlayerUiIntentQueue::default();
    assert!(cancel(&mut ui, &mut model, &mut queue));
    assert!(!cancel(&mut ui, &mut model, &mut queue));
    assert_eq!(
        queue.drain_intents(),
        vec![NativePlayerUiIntent::TradeCancel]
    );
    assert_eq!(model.trade, before);
    assert!(!ui.trade_dialog.open);
    assert!(ui.inventory_open());
    model.apply_packet("TradeGold", &json!({"amount":10}));
    ui.trade_dialog.observe(&model);
    assert!(
        !ui.trade_dialog.open,
        "an offer after local cancel must not resurrect windows"
    );
}

#[test]
fn trade_dialog_confirm_toggles_source_bool_without_claiming_settlement() {
    let model = social();
    let mut ui = state(&model);
    let mut queue = NativePlayerUiIntentQueue::default();
    assert!(toggle_lock(&mut ui, &model, &mut queue));
    assert!(ui.trade_dialog.locked(&model.trade));
    assert_eq!(
        queue.drain_intents(),
        vec![NativePlayerUiIntent::TradeConfirm { locked: true }]
    );
    assert!(toggle_lock(&mut ui, &model, &mut queue));
    assert!(!ui.trade_dialog.locked(&model.trade));
    assert_eq!(
        queue.drain_intents(),
        vec![NativePlayerUiIntent::TradeConfirm { locked: false }]
    );
    assert_eq!(model.trade.state, "open");
    assert!(!model.trade.my_confirmed);
    assert!(ui.trade_dialog.open);
    assert!(open_gold(&mut ui, &model, 100));
    assert!(!toggle_lock(&mut ui, &model, &mut queue));
}

#[test]
fn trade_dialog_drag_owns_pair_overlap_cells_and_clamps_whole_window() {
    let mut ui = TradeDialogUi::default();
    assert!(ui.begin_drag(Vec2::new(303.0, 440.0)));
    ui.drag_to(Vec2::new(-100.0, -100.0));
    assert_eq!(ui.positions[0], Vec2::ZERO);
    assert_eq!(ui.positions[1], Vec2::new(522.0, 418.0));
    ui.drag_to(Vec2::new(4000.0, 4000.0));
    assert_eq!(ui.positions[0], Vec2::new(820.0, 616.0));
    ui.drag = None;
    ui.positions = [Vec2::new(100.0, 100.0); 2];
    ui.front = TradeSide::Guest;
    assert!(
        !ui.begin_drag(Vec2::new(115.0, 145.0)),
        "guest cell owns overlap, not the covered own window"
    );
    assert!(ui.drag.is_none());
    assert_eq!(ui.front, TradeSide::Guest);
    assert!(
        ui.begin_drag(Vec2::new(150.0, 227.0)),
        "guest gold is NotControl and drags"
    );
    assert_eq!(ui.drag.unwrap().0, TradeSide::Guest);
    ui.drag = None;
    ui.front = TradeSide::Own;
    assert!(
        !ui.begin_drag(Vec2::new(150.0, 227.0)),
        "own gold is a control"
    );
    assert!(
        !ui.begin_drag(Vec2::new(240.0, 225.0)),
        "own confirm is a control"
    );
    assert!(
        !ui.begin_drag(Vec2::new(287.0, 110.0)),
        "own close is a control"
    );
    assert!(
        ui.begin_drag(Vec2::new(170.0, 110.0)),
        "own name is NotControl and drags"
    );
}

#[test]
fn trade_gold_prompt_submits_edited_amount_once_without_mutating_wallet_or_offer() {
    let mut model = social();
    let mut ui = state(&model);
    let mut queue = NativePlayerUiIntentQueue::default();
    assert!(open_gold(&mut ui, &model, 500));
    let prompt = ui.trade_dialog.gold_prompt.as_mut().unwrap();
    assert_eq!(prompt.input.draft, "500");
    assert!(prompt.input.select_all);
    prompt.input.push_text("321");
    assert!(confirm_gold(&mut ui, &mut model, 500, &mut queue));
    assert_eq!(
        queue.drain_intents(),
        vec![NativePlayerUiIntent::TradeGold { amount: 321 }]
    );
    assert_eq!(model.trade.my_gold, 0);
    assert_eq!(
        model.pending,
        vec![SocialPendingOperation::TradeGold { amount: 321 }]
    );
    assert!(!confirm_gold(&mut ui, &mut model, 500, &mut queue));
    assert!(
        !open_gold(&mut ui, &model, 500),
        "one unresolved own offer at a time"
    );
}

#[test]
fn trade_gold_invalid_zero_owner_change_balance_drop_and_pending_fail_without_packets() {
    for case in 0..6 {
        let mut model = social();
        let mut ui = state(&model);
        let mut queue = NativePlayerUiIntentQueue::default();
        assert!(open_gold(&mut ui, &model, 500));
        match case {
            0 => ui.trade_dialog.gold_prompt.as_mut().unwrap().input.draft = "4294967296".into(),
            1 => ui.trade_dialog.gold_prompt.as_mut().unwrap().input.draft = "".into(),
            2 => ui.trade_dialog.gold_prompt.as_mut().unwrap().input.draft = "0".into(),
            3 => model.trade.partner = Some("Other".into()),
            4 => {}
            _ => {
                model.begin_pending(SocialPendingOperation::TradeGold { amount: 20 });
            }
        }
        let sent = confirm_gold(
            &mut ui,
            &mut model,
            if case == 4 { 499 } else { 500 },
            &mut queue,
        );
        assert_eq!(sent, case == 2);
        assert!(queue.drain_intents().is_empty());
        assert!(ui.trade_dialog.gold_prompt.is_none());
        assert_eq!(model.trade.my_gold, 0);
    }
    let model = social();
    let mut ui = state(&model);
    assert!(!open_gold(&mut ui, &model, 0));
    ui.trade_dialog.hide();
    assert!(!open_gold(&mut ui, &model, 100));
}

#[test]
fn trade_gold_modal_ecs_uses_original_amount_box_and_hides_ok_for_invalid_input() {
    let mut app = app();
    let model = social();
    assert!(open_gold(
        &mut app.world_mut().resource_mut::<NativePlayerUiState>(),
        &model,
        500
    ));
    app.update();
    let world = app.world_mut();
    let modal = world
        .query_filtered::<(&Node, &GlobalZIndex, Option<&Button>), With<OverlayTradeGoldModal>>()
        .single(world)
        .unwrap();
    assert_ne!(modal.0.display, Display::None);
    assert_eq!(modal.1 .0, OVERLAY_INVENTORY_DELETE_MODAL_Z);
    assert!(modal.2.is_some());
    let field = world
        .query_filtered::<&Node, With<OverlayTradeGoldInput>>()
        .single(world)
        .unwrap();
    assert_eq!(
        rect(field),
        values(CrystalRect::new(58.0, 43.0, 132.0, 19.0))
    );
    assert!(world
        .query::<&Text>()
        .iter(world)
        .any(|t| t.0 == "Trade Amount:"));
    for path in [
        "original-ui/Prguse/238.png",
        "original-ui/Items/116.png",
        "original-ui/Title/200.png",
        "original-ui/Title/203.png",
    ] {
        assert!(
            world
                .query::<&ImageNode>()
                .iter(world)
                .any(|i| i.image.path().is_some_and(|p| p.to_string() == path)),
            "{path}"
        );
    }
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .trade_dialog
        .gold_prompt
        .as_mut()
        .unwrap()
        .input
        .draft = "bad".into();
    app.update();
    let world = app.world_mut();
    assert!(!world
        .query::<&OverlayButton>()
        .iter(world)
        .any(|b| *b == OverlayButton::TradeGoldConfirm));
}

fn keyboard_app() -> App {
    let mut app = overlay_tests::help_keyboard_test_app();
    app.init_resource::<SocialModel>()
        .init_resource::<UiReadModel>();
    let model = social();
    let mut ui = state(&model);
    assert!(open_gold(&mut ui, &model, 500));
    *app.world_mut().resource_mut::<NativePlayerUiState>() = ui;
    *app.world_mut().resource_mut::<SocialModel>() = model;
    app.world_mut().resource_mut::<UiReadModel>().player.gold = 500;
    app
}

fn key(app: &mut App, code: KeyCode, text: Option<&str>, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: bevy::input::keyboard::Key::Character(text.unwrap_or("").into()),
        state,
        text: text.map(Into::into),
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

#[test]
fn trade_gold_keyboard_respects_coalesced_text_backspace_enter_and_drains_remainder() {
    for enter in [KeyCode::Enter, KeyCode::NumpadEnter] {
        let mut app = keyboard_app();
        for (code, text) in [
            (KeyCode::Digit1, Some("1")),
            (KeyCode::Digit2, Some("2")),
            (KeyCode::Backspace, None),
            (KeyCode::Digit3, Some("3")),
            (enter, None),
            (KeyCode::Digit9, Some("9")),
        ] {
            key(&mut app, code, text, ButtonState::Pressed);
        }
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![NativePlayerUiIntent::TradeGold { amount: 13 }]
        );
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .trade_dialog
            .gold_prompt
            .is_none());
        assert_eq!(app.world().resource::<UiReadModel>().player.gold, 500);
        app.world_mut()
            .resource_mut::<SocialModel>()
            .pending
            .clear();
        let model = app.world().resource::<SocialModel>().clone();
        assert!(open_gold(
            &mut app.world_mut().resource_mut::<NativePlayerUiState>(),
            &model,
            500
        ));
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .trade_dialog
                .gold_prompt
                .as_ref()
                .unwrap()
                .input
                .draft,
            "500"
        );
    }
}

#[test]
fn trade_gold_keyboard_control_a_uses_ordered_modifier_state_and_zero_is_silent() {
    let mut app = keyboard_app();
    {
        let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
        let p = &mut ui.trade_dialog.gold_prompt.as_mut().unwrap().input;
        p.draft = "250".into();
        p.select_all = false;
    }
    for (code, text, state) in [
        (KeyCode::ControlLeft, None, ButtonState::Pressed),
        (KeyCode::KeyA, Some("a"), ButtonState::Pressed),
        (KeyCode::ControlLeft, None, ButtonState::Released),
        (KeyCode::Digit0, Some("0"), ButtonState::Pressed),
        (KeyCode::Enter, None, ButtonState::Pressed),
    ] {
        key(&mut app, code, text, state);
    }
    app.update();
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .trade_dialog
        .gold_prompt
        .is_none());
}

#[test]
fn trade_gold_escape_only_dismisses_modal_and_general_escape_keeps_trade_pair() {
    let mut app = keyboard_app();
    key(&mut app, KeyCode::Escape, None, ButtonState::Pressed);
    key(&mut app, KeyCode::Digit9, Some("9"), ButtonState::Pressed);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .trade_dialog
        .gold_prompt
        .is_none());
    assert!(
        app.world()
            .resource::<NativePlayerUiState>()
            .trade_dialog
            .open
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::Inventory;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    let ui = app.world().resource::<NativePlayerUiState>();
    assert_eq!(ui.core.panel, mir2_ui_core::state::UiPanel::None);
    assert!(ui.trade_dialog.open);
    assert!(!ui.menu_open());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
}

#[test]
fn trade_gold_buttons_cancel_close_and_modal_edge_never_leak_other_actions() {
    for close in [
        OverlayButton::TradeGoldCancel,
        OverlayButton::TradeGoldClose,
    ] {
        let mut app = overlay_tests::help_button_test_app();
        app.init_resource::<UiReadModel>();
        let model = social();
        let mut ui = state(&model);
        assert!(open_gold(&mut ui, &model, 100));
        *app.world_mut().resource_mut::<SocialModel>() = model;
        *app.world_mut().resource_mut::<NativePlayerUiState>() = ui;
        app.world_mut().spawn((Button, Interaction::Pressed, close));
        app.world_mut()
            .spawn((Button, Interaction::Pressed, OverlayButton::TradeConfirm));
        app.world_mut()
            .spawn((Button, Interaction::Pressed, OverlayButton::TradeCancel));
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .trade_dialog
            .gold_prompt
            .is_none());
        assert!(
            app.world()
                .resource::<NativePlayerUiState>()
                .trade_dialog
                .open
        );
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }
}

#[test]
fn trade_dialog_sync_leaves_session_without_leaking_positions_or_prompt() {
    let mut app = app();
    let model = social();
    {
        let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
        ui.trade_dialog.positions[0] = Vec2::ZERO;
        assert!(open_gold(&mut ui, &model, 100));
    }
    app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::Login;
    app.world_mut().run_system_once(sync).unwrap();
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().trade_dialog,
        TradeDialogUi::default()
    );
}

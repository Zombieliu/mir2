use super::*;
use serde_json::json;
fn fixture() -> (
    NativePlayerUiState,
    InventoryModel,
    crate::social::SocialModel,
    NativePlayerUiIntentQueue,
) {
    let mut social = crate::social::SocialModel::default();
    social.apply_packet("TradeAccept", &json!({"name":"Guest"}));
    let mut state = NativePlayerUiState::default();
    state.trade_dialog.observe(&social);
    state.core.panel = mir2_ui_core::state::UiPanel::Inventory;
    let mut inventory = InventoryModel::default();
    inventory.items.push(ItemModel {
        unique_id: Some(42),
        slot: 2,
        quantity: 1,
        ..default()
    });
    (
        state,
        inventory,
        social,
        NativePlayerUiIntentQueue::default(),
    )
}
#[test]
fn trade_items_send_exact_slots_and_wait_for_failure_ack_without_editing_assets() {
    let (mut s, i, mut m, mut q) = fixture();
    let before = i.clone();
    assert!(!activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Bag(2)
    ));
    assert!(activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Own(7)
    ));
    assert_eq!(
        q.drain_intents(),
        vec![NativePlayerUiIntent::TradeDepositItem { from: 2, to: 7 }]
    );
    assert_eq!(i.items, before.items);
    assert!(m.trade.my_items.is_empty());
    activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Bag(2),
    );
    activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Own(8),
    );
    assert!(q.drain_intents().is_empty());
    let mut incoming = m.clone();
    incoming.pending.clear();
    incoming.apply_packet(
        "DepositTradeItem",
        &json!({"from":2,"to":7,"success":false}),
    );
    m.apply_authoritative(incoming);
    assert!(ready(&s, &m));
}
#[test]
fn trade_items_stale_uid_quantity_and_exchange_fail_closed() {
    for change in 0..4 {
        let (mut s, mut i, mut m, mut q) = fixture();
        activate(
            &mut s,
            &i,
            &mut m,
            &mut q,
            &mut PendingOperations::default(),
            Cell::Bag(2),
        );
        match change {
            0 => i.items[0].unique_id = Some(43),
            1 => i.items[0].quantity = 2,
            2 => m.trade.open_revision += 1,
            _ => m.trade.partner = Some("Other".into()),
        }
        assert!(!activate(
            &mut s,
            &i,
            &mut m,
            &mut q,
            &mut PendingOperations::default(),
            Cell::Own(0)
        ));
        assert!(q.drain_intents().is_empty());
    }
}
#[test]
fn trade_items_guest_and_both_locks_are_read_only() {
    for lock in 0..3 {
        let (mut s, i, mut m, mut q) = fixture();
        if lock == 1 {
            s.trade_dialog.local_locked = Some(true)
        } else if lock == 2 {
            m.trade.partner_confirmed = true
        }
        activate(
            &mut s,
            &i,
            &mut m,
            &mut q,
            &mut PendingOperations::default(),
            Cell::Guest,
        );
        assert!(s.trade_dialog.item_input.selected.is_none());
        activate(
            &mut s,
            &i,
            &mut m,
            &mut q,
            &mut PendingOperations::default(),
            Cell::Bag(2),
        );
        activate(
            &mut s,
            &i,
            &mut m,
            &mut q,
            &mut PendingOperations::default(),
            Cell::Guest,
        );
        assert!(q.drain_intents().is_empty());
    }
}
#[test]
fn trade_items_retrieve_exact_empty_slot_full_bag_and_reserved_origin() {
    let (mut s, mut i, mut m, mut q) = fixture();
    m.trade.my_items = vec![None; 10];
    m.trade.my_items[3] = Some(crate::social::TradeItemModel {
        unique_id: Some(42),
        count: 1,
        ..default()
    });
    assert!(snapshot(Cell::Bag(2), &i, &m).is_none());
    activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Own(3),
    );
    assert!(activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Bag(12)
    ));
    assert_eq!(
        q.drain_intents(),
        vec![NativePlayerUiIntent::TradeRetrieveItem { from: 3, to: 12 }]
    );
    m.pending.clear();
    i.items = (0..u32::from(i.bag_slot_capacity()))
        .map(|slot| ItemModel {
            unique_id: Some(100 + u64::from(slot)),
            slot,
            quantity: 1,
            ..default()
        })
        .collect();
    activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Own(3),
    );
    assert!(!activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Bag(12)
    ));
    assert!(q.drain_intents().is_empty());
    i.items[2].unique_id = Some(42);
    assert!(activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Bag(2)
    ));
}
#[test]
fn trade_items_front_guest_blocks_own_and_bag_and_page_slots_are_normalized() {
    let (mut s, i, _, _) = fixture();
    s.trade_dialog.positions = [Vec2::ZERO, Vec2::ZERO];
    assert_eq!(hit(&s, &i, Vec2::new(15., 45.)), Some(Cell::Guest));
    s.trade_dialog.front = TradeSide::Own;
    assert_eq!(hit(&s, &i, Vec2::new(15., 45.)), Some(Cell::Own(0)));
}
#[test]
fn trade_items_headless_pointer_focus_loss_clears_selection() {
    let (mut s, i, m, q) = fixture();
    let mut model = m.clone();
    let mut queue = NativePlayerUiIntentQueue::default();
    activate(
        &mut s,
        &i,
        &mut model,
        &mut queue,
        &mut PendingOperations::default(),
        Cell::Bag(2),
    );
    let mut app = App::new();
    app.insert_resource(s)
        .insert_resource(i)
        .insert_resource(m)
        .insert_resource(q)
        .init_resource::<PendingOperations>()
        .init_resource::<NativeShellModel>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<CursorMoved>()
        .add_systems(Update, process_items);
    app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
    let mut window = Window::default();
    window.focused = false;
    app.world_mut().spawn((window, PrimaryWindow));
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .trade_dialog
        .item_input
        .selected
        .is_none());
}

#[test]
fn trade_items_headless_click_and_held_drag_emit_one_exact_transfer() {
    for drag in [false, true] {
        let (mut state, inventory, social, queue) = fixture();
        state.inventory_window.left = 708.;
        state.inventory_window.top = 0.;
        let from = Vec2::new(
            708. + INVENTORY_GRID_ORIGIN.x as f32 + 2. * INVENTORY_GRID_STEP.x as f32 + 4.,
            INVENTORY_GRID_ORIGIN.y as f32 + 4.,
        );
        let to = state.trade_dialog.positions[0]
            + Vec2::new(
                cell_rect(7).unwrap().left + 4.,
                cell_rect(7).unwrap().top + 4.,
            );
        let mut app = App::new();
        app.insert_resource(state)
            .insert_resource(inventory)
            .insert_resource(social)
            .insert_resource(queue)
            .init_resource::<PendingOperations>()
            .init_resource::<NativeShellModel>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<CursorMoved>()
            .add_systems(Update, process_items);
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
        let mut window = Window::default();
        window.resolution.set(1024., 768.);
        window.focused = true;
        window.set_cursor_position(Some(from));
        let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .trade_dialog
            .item_input
            .selected
            .is_some());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        if !drag {
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
        }
        app.world_mut()
            .get_mut::<Window>(entity)
            .unwrap()
            .set_cursor_position(Some(to));
        if drag {
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
        } else {
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
        }
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![NativePlayerUiIntent::TradeDepositItem { from: 2, to: 7 }]
        );
    }
}
#[test]
fn trade_items_zero_uid_is_concrete_and_only_authoritative_offers_hide_bag_item() {
    let (mut s, mut i, mut m, mut q) = fixture();
    i.items[0].unique_id = Some(0);
    assert!(!offered_bag_item(&m, &i.items[0]));
    activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Bag(2),
    );
    assert!(activate(
        &mut s,
        &i,
        &mut m,
        &mut q,
        &mut PendingOperations::default(),
        Cell::Own(0)
    ));
    assert!(
        !offered_bag_item(&m, &i.items[0]),
        "a request never hides assets"
    );
    m.trade.my_items = vec![Some(crate::social::TradeItemModel {
        unique_id: Some(0),
        count: 1,
        ..default()
    })];
    assert!(offered_bag_item(&m, &i.items[0]));
    m.trade.my_items.clear();
    assert!(!offered_bag_item(&m, &i.items[0]));
    assert_eq!(i.items[0].unique_id, Some(0));
}
#[test]
fn trade_items_headless_npc_modal_and_snapshot_replacement_discard_selection() {
    for modal in [false, true] {
        let (mut s, mut i, mut m, mut q) = fixture();
        activate(
            &mut s,
            &i,
            &mut m,
            &mut q,
            &mut PendingOperations::default(),
            Cell::Bag(2),
        );
        if !modal {
            i.items[0].unique_id = Some(999);
        }
        let mut app = App::new();
        app.insert_resource(s)
            .insert_resource(i)
            .insert_resource(m)
            .insert_resource(q)
            .init_resource::<PendingOperations>()
            .init_resource::<NativeShellModel>()
            .init_resource::<NpcDialogModel>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<CursorMoved>()
            .add_systems(Update, process_items);
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
        app.world_mut().resource_mut::<NpcDialogModel>().is_open = modal;
        let mut window = Window::default();
        window.focused = true;
        app.world_mut().spawn((window, PrimaryWindow));
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .trade_dialog
            .item_input
            .selected
            .is_none());
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }
}
#[test]
fn trade_items_inventory_button_in_same_frame_does_not_open_inspect_popup() {
    let (mut state, inventory, social, queue) = fixture();
    state.inspect = None;
    let mut app = super::super::super::tests::help_button_test_app();
    app.insert_resource(state)
        .insert_resource(inventory)
        .insert_resource(social)
        .insert_resource(queue);
    app.world_mut()
        .spawn((Button, Interaction::Pressed, OverlayButton::InspectBag(2)));
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .inspect
        .is_none());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .inventory_operation
        .is_none());
}
fn stack_offer(id: u64, count: u16, index: i32) -> crate::social::TradeItemModel {
    crate::social::TradeItemModel {
        unique_id: Some(id),
        count,
        tooltip_source: Some(crate::inventory::CrystalItemTooltipSourceModel {
            info: crate::inventory::CrystalItemInfoModel {
                item_index: index,
                stack_size: 10,
                ..default()
            },
            ..default()
        }),
        ..default()
    }
}
#[test]
fn trade_items_merge_maps_all_three_grid_directions_and_waits_for_exact_ack() {
    use crate::pending_operations::{apply_inventory_operation_ack, InventoryOperationAck};
    for direction in 0..3 {
        let (mut s, mut i, mut m, mut q) = fixture();
        let mut p = PendingOperations::default();
        i.items[0].tooltip_source = stack_offer(42, 1, 7).tooltip_source;
        m.trade.my_items = vec![Some(stack_offer(50, 4, 7)), Some(stack_offer(51, 3, 7))];
        let (from, to, id_from, id_to, gf, gt) = match direction {
            0 => (Cell::Bag(2), Cell::Own(0), 42, 50, "inventory", "Trade"),
            1 => (Cell::Own(0), Cell::Bag(2), 50, 42, "Trade", "inventory"),
            _ => (Cell::Own(0), Cell::Own(1), 50, 51, "Trade", "Trade"),
        };
        let before = i.items.clone();
        let offers = m.trade.my_items.clone();
        activate(&mut s, &i, &mut m, &mut q, &mut p, from);
        assert!(activate(&mut s, &i, &mut m, &mut q, &mut p, to));
        assert_eq!(
            q.drain_intents(),
            vec![NativePlayerUiIntent::MergeItem {
                grid_from: gf.into(),
                grid_to: gt.into(),
                id_from,
                id_to
            }]
        );
        assert_eq!(i.items, before);
        assert_eq!(m.trade.my_items, offers);
        let mut feedback = InventoryOperationFeedback::default();
        assert_eq!(
            apply_inventory_operation_ack(
                &mut p,
                &mut feedback,
                InventoryOperationAck::Merge {
                    grid_from: gf.into(),
                    grid_to: gt.into(),
                    id_from,
                    id_to: 999,
                    success: true
                }
            ),
            0
        );
        assert!(!p.is_empty());
        assert_eq!(
            apply_inventory_operation_ack(
                &mut p,
                &mut feedback,
                InventoryOperationAck::Merge {
                    grid_from: gf.into(),
                    grid_to: gt.into(),
                    id_from,
                    id_to,
                    success: false
                }
            ),
            1
        );
        assert!(p.is_empty());
        activate(&mut s, &i, &mut m, &mut q, &mut p, from);
        assert!(activate(&mut s, &i, &mut m, &mut q, &mut p, to));
    }
}
#[test]
fn trade_items_reorders_empty_or_incompatible_trade_slots_without_optimistic_swap() {
    for occupied in [false, true] {
        let (mut s, i, mut m, mut q) = fixture();
        let mut p = PendingOperations::default();
        m.trade.my_items = vec![
            Some(stack_offer(50, 4, 7)),
            occupied.then(|| stack_offer(51, 3, 8)),
        ];
        let before = m.trade.my_items.clone();
        activate(&mut s, &i, &mut m, &mut q, &mut p, Cell::Own(0));
        assert!(activate(&mut s, &i, &mut m, &mut q, &mut p, Cell::Own(1)));
        assert_eq!(
            q.drain_intents(),
            vec![NativePlayerUiIntent::MoveItem {
                grid: "Trade".into(),
                unique_id: 50,
                from: 0,
                to: 1
            }]
        );
        assert_eq!(m.trade.my_items, before);
    }
}
#[test]
fn trade_items_full_cross_grid_stack_rejects_and_partial_snapshot_invalidates_selection() {
    let (mut s, mut i, mut m, mut q) = fixture();
    let mut p = PendingOperations::default();
    i.items[0].tooltip_source = stack_offer(42, 1, 7).tooltip_source;
    m.trade.my_items = vec![Some(stack_offer(50, 10, 7))];
    activate(&mut s, &i, &mut m, &mut q, &mut p, Cell::Bag(2));
    assert!(!activate(&mut s, &i, &mut m, &mut q, &mut p, Cell::Own(0)));
    assert!(q.drain_intents().is_empty());
    assert!(s.trade_dialog.item_input.notice.is_some());
    i.items[0].quantity = 2;
    m.trade.my_items[0].as_mut().unwrap().count = 8;
    assert!(!activate(&mut s, &i, &mut m, &mut q, &mut p, Cell::Own(0)));
    assert!(q.drain_intents().is_empty());
}
#[test]
fn trade_items_merge_pending_blocks_confirm_and_gold_buttons() {
    let (state, inventory, social, queue) = fixture();
    let mut app = super::super::super::tests::help_button_test_app();
    app.insert_resource(state)
        .insert_resource(inventory)
        .insert_resource(social)
        .insert_resource(queue)
        .init_resource::<UiReadModel>();
    app.world_mut().resource_mut::<UiReadModel>().player.gold = 100;
    app.world_mut()
        .resource_mut::<PendingOperations>()
        .try_begin(crate::pending_operations::PendingOperationKey::Merge {
            grid_from: "Trade".into(),
            grid_to: "inventory".into(),
            id_from: 50,
            id_to: 42,
        });
    app.world_mut()
        .spawn((Button, Interaction::Pressed, OverlayButton::TradeConfirm));
    app.world_mut()
        .spawn((Button, Interaction::Pressed, OverlayButton::TradeGoldOffer));
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

fn ordered_pointer_fixture() -> (App, Entity, Vec2, Vec2) {
    let (mut state, inventory, social, queue) = fixture();
    state.inventory_window.left = 708.;
    state.inventory_window.top = 0.;
    let from = Vec2::new(
        708. + INVENTORY_GRID_ORIGIN.x as f32 + 2. * INVENTORY_GRID_STEP.x as f32 + 4.,
        INVENTORY_GRID_ORIGIN.y as f32 + 4.,
    );
    let rect = cell_rect(7).unwrap();
    let to = state.trade_dialog.positions[0] + Vec2::new(rect.left + 4., rect.top + 4.);
    let mut app = App::new();
    app.insert_resource(state)
        .insert_resource(inventory)
        .insert_resource(social)
        .insert_resource(queue)
        .init_resource::<PendingOperations>()
        .init_resource::<NativeShellModel>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<CursorMoved>()
        .add_message::<bevy::window::WindowEvent>()
        .add_systems(Update, process_items);
    app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
    let mut window = Window::default();
    window.focused = true;
    window.resolution.set(1024., 768.);
    let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
    (app, entity, from, to)
}

fn ordered_move(app: &mut App, window: Entity, position: Vec2) {
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(position));
    app.world_mut()
        .write_message(bevy::window::WindowEvent::CursorMoved(CursorMoved {
            window,
            position,
            delta: None,
        }));
}

fn ordered_button(app: &mut App, window: Entity, pressed: bool) {
    app.world_mut()
        .write_message(bevy::window::WindowEvent::MouseButtonInput(
            bevy::input::mouse::MouseButtonInput {
                window,
                button: MouseButton::Left,
                state: if pressed {
                    bevy::input::ButtonState::Pressed
                } else {
                    bevy::input::ButtonState::Released
                },
            },
        ));
}

#[test]
fn trade_items_ordered_previous_frame_source_and_batched_drag_keep_press_origin() {
    let (mut app, window, from, to) = ordered_pointer_fixture();
    ordered_move(&mut app, window, from);
    app.update();
    ordered_button(&mut app, window, true);
    ordered_move(&mut app, window, to);
    ordered_button(&mut app, window, false);
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::TradeDepositItem { from: 2, to: 7 }]
    );
    app.update();
    assert!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty(),
        "consumed ordered edges must not replay on a later frame"
    );
}

#[test]
fn trade_items_ordered_preselected_source_can_be_dragged_without_deselecting() {
    let (mut app, window, from, to) = ordered_pointer_fixture();
    ordered_move(&mut app, window, from);
    ordered_button(&mut app, window, true);
    ordered_button(&mut app, window, false);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .trade_dialog
        .item_input
        .selected
        .is_some());
    ordered_button(&mut app, window, true);
    app.update();
    assert!(
        app.world()
            .resource::<NativePlayerUiState>()
            .trade_dialog
            .item_input
            .selected
            .is_some(),
        "pressing an already selected source must retain it until the release destination is known"
    );
    ordered_move(&mut app, window, to);
    ordered_button(&mut app, window, false);
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::TradeDepositItem { from: 2, to: 7 }]
    );
}

#[test]
fn trade_items_ordered_second_stationary_click_still_deselects() {
    let (mut app, window, from, _) = ordered_pointer_fixture();
    ordered_move(&mut app, window, from);
    for click in 0..2 {
        ordered_button(&mut app, window, true);
        ordered_button(&mut app, window, false);
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .trade_dialog
                .item_input
                .selected
                .is_some(),
            click == 0
        );
    }
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
}

#[test]
fn trade_items_ordered_bare_release_cannot_place_or_select() {
    for selected in [false, true] {
        let (mut app, window, from, to) = ordered_pointer_fixture();
        if selected {
            ordered_move(&mut app, window, from);
            ordered_button(&mut app, window, true);
            ordered_button(&mut app, window, false);
            app.update();
        }
        ordered_move(&mut app, window, if selected { to } else { from });
        ordered_button(&mut app, window, false);
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .trade_dialog
                .item_input
                .selected
                .is_some(),
            selected
        );
    }
}

#[test]
fn trade_items_ordered_consecutive_drags_after_authoritative_deposit_and_retrieve() {
    let (mut app, window, from, to) = ordered_pointer_fixture();
    ordered_move(&mut app, window, from);
    ordered_button(&mut app, window, true);
    ordered_move(&mut app, window, to);
    ordered_button(&mut app, window, false);
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::TradeDepositItem { from: 2, to: 7 }]
    );
    {
        let mut model = app.world_mut().resource_mut::<crate::social::SocialModel>();
        let mut incoming = model.clone();
        incoming.pending.clear();
        incoming.apply_packet("DepositTradeItem", &json!({"from":2,"to":7,"success":true}));
        incoming.trade.my_items = vec![None; 10];
        incoming.trade.my_items[7] = Some(crate::social::TradeItemModel {
            unique_id: Some(42),
            count: 1,
            ..default()
        });
        model.apply_authoritative(incoming);
        assert!(model.pending.is_empty());
    }
    ordered_button(&mut app, window, true);
    ordered_move(&mut app, window, from);
    ordered_button(&mut app, window, false);
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::TradeRetrieveItem { from: 7, to: 2 }]
    );
    {
        let mut model = app.world_mut().resource_mut::<crate::social::SocialModel>();
        let mut incoming = model.clone();
        incoming.pending.clear();
        incoming.apply_packet(
            "RetrieveTradeItem",
            &json!({"from":7,"to":2,"success":true}),
        );
        incoming.trade.my_items[7] = None;
        model.apply_authoritative(incoming);
        assert!(model.pending.is_empty());
    }
    ordered_button(&mut app, window, true);
    ordered_move(&mut app, window, to);
    ordered_button(&mut app, window, false);
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::TradeDepositItem { from: 2, to: 7 }]
    );
}

#[test]
fn trade_items_ordered_pointer_tracks_while_pending_and_next_press_needs_no_motion() {
    let (mut app, window, from, to) = ordered_pointer_fixture();
    app.world_mut()
        .resource_mut::<crate::social::SocialModel>()
        .begin_pending(crate::social::SocialPendingOperation::TradeDeposit { from: 2, to: 7 });
    ordered_move(&mut app, window, from);
    ordered_button(&mut app, window, true);
    app.update();
    {
        let state = app.world().resource::<NativePlayerUiState>();
        assert_eq!(state.trade_dialog.item_input.cursor, Some(from));
        assert!(state.trade_dialog.item_input.selected.is_none());
        assert!(state.trade_dialog.item_input.pressed.is_none());
    }
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
    {
        let mut model = app.world_mut().resource_mut::<crate::social::SocialModel>();
        let mut incoming = model.clone();
        incoming.pending.clear();
        incoming.apply_packet(
            "DepositTradeItem",
            &json!({"from":2,"to":7,"success":false}),
        );
        model.apply_authoritative(incoming);
        assert!(model.pending.is_empty());
    }
    // The blocked press cannot leak through on release after the ACK.
    ordered_button(&mut app, window, false);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .trade_dialog
        .item_input
        .selected
        .is_none());
    ordered_button(&mut app, window, true);
    ordered_move(&mut app, window, to);
    ordered_button(&mut app, window, false);
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::TradeDepositItem { from: 2, to: 7 }]
    );
}

//! Native player-bag double-clicks feed the same validated Use action as its menu.

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BagItemClick {
    item: ItemModel,
    position: Vec2,
    origin: Vec2,
    page: u8,
    at_ms: u64,
}

fn ordinary_bag(state: &NativePlayerUiState) -> bool {
    state.inventory_open()
        && !state.npc_shop_open()
        && !state.storage_open()
        && !state.mail_open()
        && !state.shop_repair_mode
        && !state.inventory_delete_mode
        && state.inventory_operation.is_none()
        && !state.amount_modal_open()
        && !state.procurement_open
        && !state.procurement_input_consumed
        && !state.billing_open
        && !state.billing_input_consumed
}

pub(super) fn invalidate(
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    parcel: Option<&mail_parcel::MailParcelUi>,
    pending: &PendingOperations,
) {
    let invalid = state.inventory_item_click.as_ref().is_some_and(|click| {
        let selection = crate::bag_ui::BagSelection::from_item(&click.item);
        !ordinary_bag(state)
            || click.page != state.inventory_page
            || click.origin != Vec2::new(state.inventory_window.left, state.inventory_window.top)
            || selection.and_then(|source| crate::bag_ui::resolve_exact(inventory, source))
                != Some(&click.item)
            || click.item.unique_id.is_some_and(|uid| {
                pending.has_pending_equipment_instance(uid)
                    || parcel.is_some_and(|parcel| parcel.blocks_item(uid))
            })
    });
    if invalid {
        state.inventory_item_click = None;
    }
}

pub(super) fn motion(state: &mut NativePlayerUiState, point: Option<Vec2>) {
    if let (Some(drag), Some(point)) = (state.inventory_item_drag.as_mut(), point) {
        if drag.start.distance_squared(point) >= 16.0 {
            drag.moved = true;
            state.inventory_item_click = None;
        }
    }
}

pub(super) fn short_click(
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    source: &ItemModel,
    position: Vec2,
    now_ms: u64,
    interval_ms: u64,
    parcel: Option<&mail_parcel::MailParcelUi>,
    intents: &mut NativePlayerUiIntentQueue,
    pending: &mut PendingOperations,
) {
    let previous = state.inventory_item_click.take();
    if !ordinary_bag(state) {
        return;
    }
    let Some(selection) = crate::bag_ui::BagSelection::from_item(source) else {
        return;
    };
    if crate::bag_ui::resolve_exact(inventory, selection) != Some(source)
        || inventory
            .items
            .iter()
            .filter(|item| item.unique_id == source.unique_id)
            .count()
            != 1
        || pending.has_pending_equipment_instance(selection.unique_id)
        || parcel.is_some_and(|parcel| parcel.blocks_item(selection.unique_id))
    {
        return;
    }
    let origin = Vec2::new(state.inventory_window.left, state.inventory_window.top);
    let double = previous.is_some_and(|click| {
        click.item == *source
            && click.origin == origin
            && click.page == state.inventory_page
            && click.position.distance_squared(position) < 16.0
            && now_ms >= click.at_ms
            && now_ms - click.at_ms <= interval_ms
    });
    if double {
        state.inventory_operation = None;
        use_inspected_item(state, inventory, parcel, intents, pending);
    } else {
        state.inventory_item_click = Some(BagItemClick {
            item: source.clone(),
            position,
            origin,
            page: state.inventory_page,
            at_ms: now_ms,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(equipment: bool) -> (App, Entity, Vec2) {
        let mut app = App::new();
        let mut window = Window::default();
        window.focused = true;
        window.resolution.set(1024.0, 768.0);
        let window = app.world_mut().spawn((window, PrimaryWindow)).id();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<InventoryModel>()
            .init_resource::<super::super::super::hud::CrystalBeltPresentation>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<CursorMoved>()
            .add_message::<bevy::window::WindowEvent>()
            .add_systems(Update, process_inventory_item_drag);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_inventory();
        app.world_mut()
            .resource_mut::<InventoryModel>()
            .items
            .push(ItemModel {
                unique_id: Some(0),
                slot: 0,
                container: 0,
                key: "owned-item".into(),
                name: "Owned item".into(),
                quantity: 1,
                equip_slot: equipment.then(|| "Weapon".into()),
                ..default()
            });
        let point = Vec2::new(
            INVENTORY_GRID_ORIGIN.x as f32 + 2.0,
            INVENTORY_GRID_ORIGIN.y as f32 + 2.0,
        );
        (app, window, point)
    }

    fn click(app: &mut App, window: Entity, point: Vec2) {
        app.world_mut()
            .entity_mut(window)
            .get_mut::<Window>()
            .unwrap()
            .set_cursor_position(Some(point));
        app.world_mut()
            .write_message(bevy::window::WindowEvent::CursorMoved(CursorMoved {
                window,
                position: point,
                delta: None,
            }));
        for state in [
            bevy::input::ButtonState::Pressed,
            bevy::input::ButtonState::Released,
        ] {
            app.world_mut()
                .write_message(bevy::window::WindowEvent::MouseButtonInput(
                    bevy::input::mouse::MouseButtonInput {
                        window,
                        button: MouseButton::Left,
                        state,
                    },
                ));
        }
        app.update();
    }

    fn install_buttons(app: &mut App) {
        app.init_resource::<MailComposeUi>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<MailModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            });
        super::super::tests::init_overlay_button_test_resources(app);
        app.add_systems(
            Update,
            process_overlay_buttons.after(process_inventory_item_drag),
        );
    }

    fn press(app: &mut App, button: OverlayButton) {
        let entity = app
            .world_mut()
            .spawn((Button, Interaction::Pressed, button))
            .id();
        app.update();
        app.world_mut().despawn(entity);
    }

    #[test]
    fn native_inventory_double_click_and_use_fill_ring_cells_and_keep_pending_instance_safe() {
        for metadata in ["slot", "tooltip"] {
            for occupied in [&[][..], &[7][..], &[8][..], &[7, 8][..]] {
                for entry in ["double_click", "use_button"] {
                    let (mut app, window, point) = fixture(true);
                    if entry == "use_button" {
                        install_buttons(&mut app);
                    }
                    {
                        let mut inventory = app.world_mut().resource_mut::<InventoryModel>();
                        inventory.items[0].equip_slot =
                            (metadata == "slot").then(|| "RingLeft".into());
                        if metadata == "tooltip" {
                            let mut source =
                                crate::inventory::CrystalItemTooltipSourceModel::default();
                            source.info.item_type = 7;
                            inventory.items[0].tooltip_source = Some(source);
                        }
                        for slot in occupied {
                            inventory.items.push(ItemModel {
                                unique_id: Some(100 + u64::from(*slot)),
                                container: 2,
                                slot: *slot,
                                ..default()
                            });
                        }
                    }
                    click(&mut app, window, point);
                    if entry == "double_click" {
                        click(&mut app, window, point);
                    } else {
                        press(&mut app, OverlayButton::UseInspected);
                    }
                    let expected = if occupied.contains(&8) { 7 } else { 8 };
                    let intents = app
                        .world_mut()
                        .resource_mut::<NativePlayerUiIntentQueue>()
                        .drain_intents();
                    assert_eq!(
                        intents,
                        vec![NativePlayerUiIntent::EquipItem {
                            unique_id: 0,
                            grid: "inventory".into(),
                            to: expected,
                        }],
                        "{metadata}/{occupied:?}/{entry}"
                    );
                    assert!(app
                        .world()
                        .resource::<PendingOperations>()
                        .has_pending_equipment_instance(0));
                    click(&mut app, window, point);
                    if entry == "double_click" {
                        click(&mut app, window, point);
                    } else {
                        press(&mut app, OverlayButton::UseInspected);
                    }
                    assert!(
                        app.world_mut()
                            .resource_mut::<NativePlayerUiIntentQueue>()
                            .drain_intents()
                            .is_empty(),
                        "pending {metadata}/{occupied:?}/{entry}"
                    );
                }
            }
        }
    }

    #[test]
    fn native_inventory_double_click_ring_name_without_reliable_metadata_is_not_guessed() {
        let (mut app, window, point) = fixture(false);
        {
            let mut inventory = app.world_mut().resource_mut::<InventoryModel>();
            inventory.items[0].key = "CopperRing".into();
            inventory.items[0].name = "Copper Ring".into();
        }
        click(&mut app, window, point);
        click(&mut app, window, point);
        assert!(matches!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents()
                .as_slice(),
            [NativePlayerUiIntent::UseItem {
                unique_id: Some(0),
                ..
            }]
        ));
    }

    #[test]
    fn native_inventory_double_click_preserves_explicit_equip_destination() {
        let (mut app, window, point) = fixture(true);
        install_buttons(&mut app);
        app.world_mut().resource_mut::<InventoryModel>().items[0].equip_slot =
            Some("RingLeft".into());
        click(&mut app, window, point);
        press(&mut app, OverlayButton::EquipInspected);
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![NativePlayerUiIntent::EquipItem {
                unique_id: 0,
                grid: "inventory".into(),
                to: 7
            }]
        );
    }

    #[test]
    fn native_inventory_double_click_real_player_pointer_equips_uid_zero_once() {
        let (mut app, window, point) = fixture(true);
        click(&mut app, window, point);
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
        // An idle frame must preserve the first short click.
        app.update();
        click(&mut app, window, point);
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![NativePlayerUiIntent::EquipItem {
                unique_id: 0,
                grid: "inventory".into(),
                to: 0
            }]
        );
        assert!(app
            .world()
            .resource::<PendingOperations>()
            .has_pending_equipment_instance(0));
        click(&mut app, window, point);
        click(&mut app, window, point);
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }

    #[test]
    fn native_inventory_double_click_real_player_pointer_uses_consumable() {
        let (mut app, window, point) = fixture(false);
        click(&mut app, window, point);
        click(&mut app, window, point);
        let intents = app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        assert!(matches!(
            intents.as_slice(),
            [NativePlayerUiIntent::UseItem {
                unique_id: Some(0),
                ..
            }]
        ));
    }

    #[test]
    fn native_inventory_double_click_real_pointer_drag_out_and_back_never_uses() {
        let (mut app, window, point) = fixture(true);
        click(&mut app, window, point);
        app.world_mut()
            .write_message(bevy::window::WindowEvent::MouseButtonInput(
                bevy::input::mouse::MouseButtonInput {
                    window,
                    button: MouseButton::Left,
                    state: bevy::input::ButtonState::Pressed,
                },
            ));
        for position in [point + Vec2::new(20.0, 0.0), point] {
            app.world_mut()
                .write_message(bevy::window::WindowEvent::CursorMoved(CursorMoved {
                    window,
                    position,
                    delta: None,
                }));
        }
        app.world_mut()
            .write_message(bevy::window::WindowEvent::MouseButtonInput(
                bevy::input::mouse::MouseButtonInput {
                    window,
                    button: MouseButton::Left,
                    state: bevy::input::ButtonState::Released,
                },
            ));
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_item_click
            .is_none());
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_item_drag
            .is_none());
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }

    #[test]
    fn native_inventory_double_click_rejects_replacement_focus_close_page_and_services() {
        for case in [
            "replacement",
            "focus",
            "focus_batch",
            "closed",
            "session",
            "page",
            "service",
            "storage",
            "delete",
            "move",
            "mail_lock",
            "backwards_clock",
            "procurement",
            "procurement_close_frame",
            "billing",
            "billing_close_frame",
        ] {
            let (mut app, window, point) = fixture(true);
            if case == "mail_lock" {
                app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(1);
            }
            click(&mut app, window, point);
            match case {
                "replacement" => {
                    app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(1)
                }
                "focus" => {
                    app.world_mut()
                        .entity_mut(window)
                        .get_mut::<Window>()
                        .unwrap()
                        .focused = false;
                    app.update();
                    app.world_mut()
                        .entity_mut(window)
                        .get_mut::<Window>()
                        .unwrap()
                        .focused = true;
                }
                "focus_batch" => {
                    app.world_mut()
                        .write_message(bevy::window::WindowEvent::WindowFocused(
                            bevy::window::WindowFocused {
                                window,
                                focused: false,
                            },
                        ));
                    app.world_mut()
                        .write_message(bevy::window::WindowEvent::WindowFocused(
                            bevy::window::WindowFocused {
                                window,
                                focused: true,
                            },
                        ));
                }
                "closed" => app
                    .world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .close_windows(),
                "session" => {
                    let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
                    state.reset_session();
                    state.toggle_inventory();
                }
                "page" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .inventory_page = 1
                }
                "service" => app
                    .world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .toggle_npc_shop(),
                "storage" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .core
                        .panel = mir2_ui_core::state::UiPanel::Storage
                }
                "delete" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .inventory_delete_mode = true
                }
                "move" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .inventory_operation = Some(InventoryOperationDraft::Move {
                        source_slot: 1,
                        unique_id: 1,
                    })
                }
                "procurement" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .procurement_open = true
                }
                "procurement_close_frame" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .procurement_input_consumed = true
                }
                "billing" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .billing_open = true
                }
                "billing_close_frame" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .billing_input_consumed = true
                }
                "mail_lock" => {
                    let mut parcel = mail_parcel::MailParcelUi::default();
                    let mut draft = mir2_ui_core::state::MailComposeDraft::default();
                    assert!(parcel.attach(&mut draft, app.world().resource::<InventoryModel>(), 1));
                    parcel.apply_lock_receipt(1, true);
                    assert!(parcel.blocks_item(1));
                    app.insert_resource(parcel);
                }
                "backwards_clock" => {
                    let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
                    state.inventory_item_click.as_mut().unwrap().at_ms = u64::MAX;
                }
                _ => unreachable!(),
            }
            click(&mut app, window, point);
            assert!(
                app.world_mut()
                    .resource_mut::<NativePlayerUiIntentQueue>()
                    .drain_intents()
                    .is_empty(),
                "{case}"
            );
        }
    }

    #[test]
    fn native_inventory_double_click_uses_host_interval_and_drag_return_is_not_a_click() {
        let (mut app, window, point) = fixture(true);
        app.insert_resource(hero_dialog::pointer::HeroPointerSettings {
            double_click_ms: 900,
        });
        click(&mut app, window, point);
        let previous = app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_item_click
            .clone()
            .unwrap();
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.inventory_item_drag = Some(InventoryItemDrag {
            source_slot: 0,
            unique_id: 0,
            start: point,
            moved: false,
        });
        motion(&mut state, Some(point + Vec2::new(20.0, 0.0)));
        assert!(state.inventory_item_click.is_none());
        assert!(state.inventory_item_drag.unwrap().moved);
        state.inventory_item_drag = None;
        // Pure timing check uses the same short-click function as native release.
        state.inventory_item_click = Some(previous.clone());
        drop(state);
        let inventory = app.world().resource::<InventoryModel>().clone();
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        let mut intents = NativePlayerUiIntentQueue::default();
        let mut pending = PendingOperations::default();
        short_click(
            &mut state,
            &inventory,
            &inventory.items[0],
            point,
            previous.at_ms + 901,
            900,
            None,
            &mut intents,
            &mut pending,
        );
        assert!(
            intents.drain_intents().is_empty(),
            "expired native interval"
        );
        state.inventory_item_click = Some(previous.clone());
        short_click(
            &mut state,
            &inventory,
            &inventory.items[0],
            point,
            previous.at_ms + 700,
            900,
            None,
            &mut intents,
            &mut pending,
        );
        assert!(matches!(
            intents.drain_intents().as_slice(),
            [NativePlayerUiIntent::EquipItem { unique_id: 0, .. }]
        ));
    }
}

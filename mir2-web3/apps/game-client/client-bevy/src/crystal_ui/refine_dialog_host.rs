use super::super::*;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Press {
    Item(Vec2),
    Confirm,
    Close,
}

#[derive(Default, Resource)]
pub struct RefineHost {
    dimensions: std::collections::HashMap<(&'static str, u16), Vec2>,
    press: Option<Press>,
    last_cursor: Option<Vec2>,
    was_open: bool,
}

impl RefineHost {
    fn size(&self, library: &'static str, index: u16) -> Option<Vec2> {
        self.dimensions.get(&(library, index)).copied()
    }
}

fn enqueue(
    state: &mut NativePlayerUiState,
    queue: &mut NativePlayerUiIntentQueue,
    packet: ClientPacket,
) {
    if !queue.push_intent(NativePlayerUiIntent::RefinePacket(packet.clone())) {
        state.refine.release_unsent(&packet);
    }
}

fn inventory_cell(
    state: &NativePlayerUiState,
    cursor: Vec2,
    belt: Option<&super::super::super::hud::CrystalBeltPresentation>,
) -> Option<u8> {
    if let Some(slot) = inventory_bag_slot_at_cursor(state, cursor) {
        return u8::try_from(slot.checked_add(6)?).ok();
    }
    belt.and_then(|belt| belt_slot_at_cursor(*belt, cursor))
}

fn press_at(
    state: &mut NativePlayerUiState,
    host: &mut RefineHost,
    inventory: &InventoryModel,
    cursor: Vec2,
    belt: Option<&super::super::super::hud::CrystalBeltPresentation>,
) {
    if !state.refine.open {
        return;
    }
    if view::close_rect().contains(cursor.x, cursor.y) {
        host.press = Some(Press::Close);
    } else if view::confirm_rect().contains(cursor.x, cursor.y) {
        host.press = Some(Press::Confirm);
    } else if !state.refine.pending() {
        if let Some(slot) =
            view::material_cell_at(cursor).filter(|_| state.refine.mode == Some(RefineMode::Refine))
        {
            if state.refine.materials[usize::from(slot)].is_some() {
                state.refine.select_material(slot);
            }
            host.press = Some(Press::Item(cursor));
        } else if let Some(cell) = inventory_cell(state, cursor, belt) {
            if matches!(
                state.refine.selected,
                Some(RefineSelection::Material { .. })
            ) && item_at_wire_cell(inventory, cell).is_none()
            {
                host.press = Some(Press::Item(cursor));
            } else if state.refine.select_inventory(inventory, cell) {
                state.inspect = None;
                host.press = Some(Press::Item(cursor));
            }
        } else if view::target_click_rect().contains(cursor.x, cursor.y) {
            host.press = Some(Press::Item(cursor));
        }
    }
    if host.press.is_some() || view::covers(&state.refine, cursor) {
        state.menu_pointer_consumed = true;
        state.refine.input_consumed = true;
    }
}

fn release_at(
    state: &mut NativePlayerUiState,
    host: &mut RefineHost,
    inventory: &InventoryModel,
    cursor: Vec2,
    belt: Option<&super::super::super::hud::CrystalBeltPresentation>,
    queue: &mut NativePlayerUiIntentQueue,
) {
    let Some(press) = host.press.take() else {
        return;
    };
    state.menu_pointer_consumed = true;
    state.refine.input_consumed = true;
    match press {
        Press::Close if view::close_rect().contains(cursor.x, cursor.y) => {
            state.refine.defer_cancel();
        }
        Press::Confirm if view::confirm_rect().contains(cursor.x, cursor.y) => {
            if let Some(packet) = state.refine.confirm(inventory) {
                enqueue(state, queue, packet);
            }
        }
        Press::Item(_) => {
            if view::target_click_rect().contains(cursor.x, cursor.y) {
                state.refine.select_target(inventory);
            } else if let Some(slot) = view::material_cell_at(cursor) {
                if let Some(packet) = state.refine.deposit_selected(inventory, slot) {
                    enqueue(state, queue, packet);
                }
            } else if let Some(cell) = inventory_cell(state, cursor, belt) {
                if let Some(packet) = state.refine.retrieve_selected(inventory, cell) {
                    enqueue(state, queue, packet);
                }
            }
        }
        _ => {}
    }
}

pub(in super::super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<RefineHost>,
    inventory: Res<InventoryModel>,
    shell: Option<Res<NativeShellModel>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    belt: Option<Res<super::super::super::hud::CrystalBeltPresentation>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    events: Option<Res<Messages<bevy::window::WindowEvent>>>,
    mut reader: Local<bevy::ecs::message::MessageCursor<bevy::window::WindowEvent>>,
    mut queue: ResMut<NativePlayerUiIntentQueue>,
    mut chat: Option<ResMut<crate::chat::ChatModel>>,
) {
    state.refine.input_consumed = false;
    let ordered: Vec<_> = events
        .as_ref()
        .map(|events| reader.read(events).cloned().collect())
        .unwrap_or_default();
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        host.press = None;
        host.last_cursor = None;
        host.was_open = false;
        return;
    }
    if let Some(packet) = state.refine.deferred_cancel() {
        enqueue(&mut state, &mut queue, packet);
    }
    if let Some(chat) = chat.as_deref_mut() {
        if let Some(text) = state.refine.notice.take() {
            chat.push(crate::chat::ChatLine {
                text,
                channel: "system".into(),
            });
        }
    }
    let Ok((entity, window)) = windows.single() else {
        host.press = None;
        host.last_cursor = None;
        return;
    };
    if !window.focused || state.leave_game.blocks() || state.amount_modal_open() {
        host.press = None;
        host.last_cursor = None;
        return;
    }
    if !state.refine.open {
        host.press = None;
        host.was_open = false;
        host.last_cursor = help_cursor_logical(window);
        return;
    }
    // NPCDropDialog.Show uses the existing right-side InventoryDialog origin.
    // The main NPC text page and the two refinement frames occupy the left.
    if !host.was_open {
        state.inventory_window.left = NPC_SERVICE_INVENTORY_ORIGIN.x as f32;
        state.inventory_window.top = NPC_SERVICE_INVENTORY_ORIGIN.y as f32;
        host.was_open = true;
    }
    let mut cursor = host.last_cursor.or_else(|| help_cursor_logical(window));
    let mut saw_buttons = false;
    for event in ordered {
        match event {
            bevy::window::WindowEvent::CursorMoved(event) if event.window == entity => {
                cursor = Some(cursor_logical(window, event.position));
            }
            bevy::window::WindowEvent::CursorLeft(event) if event.window == entity => {
                cursor = None;
                host.press = None;
            }
            bevy::window::WindowEvent::MouseButtonInput(event)
                if event.window == entity && event.button == MouseButton::Left =>
            {
                saw_buttons = true;
                if let Some(cursor) = cursor {
                    if event.state == ButtonState::Pressed {
                        press_at(&mut state, &mut host, &inventory, cursor, belt.as_deref());
                    } else {
                        release_at(
                            &mut state,
                            &mut host,
                            &inventory,
                            cursor,
                            belt.as_deref(),
                            &mut queue,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    if !saw_buttons {
        if let (Some(mouse), Some(cursor)) = (mouse, cursor) {
            if mouse.just_pressed(MouseButton::Left) {
                press_at(&mut state, &mut host, &inventory, cursor, belt.as_deref());
            }
            if mouse.just_released(MouseButton::Left) {
                release_at(
                    &mut state,
                    &mut host,
                    &inventory,
                    cursor,
                    belt.as_deref(),
                    &mut queue,
                );
            }
        }
    }
    host.last_cursor = cursor;
}

pub(in super::super) fn render_system(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    panels: Query<Entity, With<view::RefinePanel>>,
    state: Res<NativePlayerUiState>,
    shell: Option<Res<NativeShellModel>>,
    mut host: ResMut<RefineHost>,
    assets: Option<Res<AssetServer>>,
    images: Option<Res<Assets<Image>>>,
    mut handles: Local<std::collections::HashMap<(&'static str, u16), Handle<Image>>>,
) {
    for entity in &panels {
        commands.entity(entity).despawn();
    }
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) || !state.refine.open {
        return;
    }
    let (Some(assets), Some(images)) = (assets, images) else {
        return;
    };
    for &(library, indexes) in view::REQUIRED_ASSETS {
        for &index in indexes {
            let handle = handles
                .entry((library, index))
                .or_insert_with(|| assets.load(format!("original-ui/{library}/{index}.png")));
            if let Some(image) = images.get(handle) {
                host.dimensions.insert(
                    (library, index),
                    Vec2::new(image.width() as f32, image.height() as f32),
                );
            }
        }
    }
    let Ok(root) = roots.single() else {
        return;
    };
    commands.entity(root).with_children(|parent| {
        view::render(parent, &assets, &state.refine, |library, index| {
            host.size(library, index)
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (App, Entity, Vec2, Vec2) {
        let mut app = App::new();
        let mut window = Window::default();
        window.focused = true;
        window.resolution.set(1024.0, 768.0);
        let window = app.world_mut().spawn((window, PrimaryWindow)).id();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<RefineHost>()
            .init_resource::<InventoryModel>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<super::super::super::super::hud::CrystalBeltPresentation>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<CursorMoved>()
            .add_message::<bevy::window::WindowEvent>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_systems(Update, (process, process_inventory_item_drag).chain());
        let context = RefineContext {
            npc_object_id: 1149,
            map_epoch: 7,
        };
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .refine
            .begin_service_request(context, "@Refine");
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .refine
            .observe(&ServerPacket::NPCRefine {
                rate: 125.0,
                refining: false,
            });
        app.world_mut()
            .resource_mut::<InventoryModel>()
            .items
            .push(ItemModel {
                unique_id: Some(7003),
                container: 0,
                slot: 2,
                quantity: 1,
                name: "Refine host fixture".into(),
                key: "fixture".into(),
                ..Default::default()
            });
        let bag = Vec2::new(
            NPC_SERVICE_INVENTORY_ORIGIN.x as f32
                + INVENTORY_GRID_ORIGIN.x as f32
                + 2.0 * INVENTORY_GRID_STEP.x as f32
                + 2.0,
            INVENTORY_GRID_ORIGIN.y as f32 + 2.0,
        );
        let cell = view::material_rect(0);
        let material = Vec2::new(cell.left + 2.0, cell.top + 2.0);
        (app, window, bag, material)
    }

    fn motion(app: &mut App, window: Entity, position: Vec2) {
        app.world_mut()
            .write_message(bevy::window::WindowEvent::CursorMoved(CursorMoved {
                window,
                position,
                delta: None,
            }));
    }
    fn button(app: &mut App, window: Entity, state: ButtonState) {
        app.world_mut()
            .write_message(bevy::window::WindowEvent::MouseButtonInput(
                bevy::input::mouse::MouseButtonInput {
                    window,
                    button: MouseButton::Left,
                    state,
                },
            ));
    }
    fn drain(app: &mut App) -> Vec<NativePlayerUiIntent> {
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
    }

    #[test]
    fn refine_host_ordered_drag_queues_once_without_generic_inventory_mutation() {
        let (mut app, window, bag, material) = app();
        motion(&mut app, window, bag);
        button(&mut app, window, ButtonState::Pressed);
        motion(&mut app, window, material);
        button(&mut app, window, ButtonState::Released);
        app.update();
        assert_eq!(
            drain(&mut app),
            vec![NativePlayerUiIntent::RefinePacket(
                ClientPacket::DepositRefineItem { from: 8, to: 0 }
            )]
        );
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(state.inventory_item_drag.is_none());
        assert!(state.inspect.is_none());
        assert!(
            state.refine.materials[0].is_none(),
            "an ordinary click cannot take authoritative custody"
        );
        app.update();
        assert!(drain(&mut app).is_empty());
    }

    #[test]
    fn refine_host_replaced_uid_between_press_and_release_never_deposits() {
        let (mut app, window, bag, material) = app();
        motion(&mut app, window, bag);
        button(&mut app, window, ButtonState::Pressed);
        app.update();
        app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(9999);
        motion(&mut app, window, material);
        button(&mut app, window, ButtonState::Released);
        app.update();
        assert!(drain(&mut app).is_empty());
        assert!(
            !app.world()
                .resource::<NativePlayerUiState>()
                .refine
                .pending()
        );
    }

    #[test]
    fn refine_host_material_click_then_empty_bag_click_returns_only_on_release() {
        let (mut app, window, bag, material) = app();
        let held = app.world().resource::<InventoryModel>().items[0].clone();
        app.world_mut()
            .resource_mut::<InventoryModel>()
            .items
            .clear();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .refine
            .observe_custody(&RefineCustodyReadback {
                materials: vec![(0, held)],
                ..Default::default()
            });
        motion(&mut app, window, material);
        button(&mut app, window, ButtonState::Pressed);
        button(&mut app, window, ButtonState::Released);
        app.update();
        assert!(drain(&mut app).is_empty());
        motion(&mut app, window, bag);
        button(&mut app, window, ButtonState::Pressed);
        app.update();
        assert!(drain(&mut app).is_empty());
        assert!(
            !app.world()
                .resource::<NativePlayerUiState>()
                .refine
                .pending()
        );
        button(&mut app, window, ButtonState::Released);
        app.update();
        assert_eq!(
            drain(&mut app),
            vec![NativePlayerUiIntent::RefinePacket(
                ClientPacket::RetrieveRefineItem { from: 0, to: 8 }
            )]
        );
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .refine
                .materials[0]
                .as_ref()
                .unwrap()
                .unique_id,
            Some(7003)
        );
    }

    #[test]
    fn refine_host_close_during_transfer_defers_one_cancel_until_ack() {
        let (mut app, window, bag, material) = app();
        motion(&mut app, window, bag);
        button(&mut app, window, ButtonState::Pressed);
        motion(&mut app, window, material);
        button(&mut app, window, ButtonState::Released);
        app.update();
        assert_eq!(drain(&mut app).len(), 1);
        let close = view::close_rect();
        motion(
            &mut app,
            window,
            Vec2::new(close.left + 2.0, close.top + 2.0),
        );
        button(&mut app, window, ButtonState::Pressed);
        button(&mut app, window, ButtonState::Released);
        app.update();
        assert!(drain(&mut app).is_empty());
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .refine
            .observe(&ServerPacket::DepositRefineItem {
                from: 8,
                to: 0,
                success: true,
            });
        app.update();
        assert_eq!(
            drain(&mut app),
            vec![NativePlayerUiIntent::RefinePacket(
                ClientPacket::RefineCancel
            )]
        );
        app.update();
        assert!(drain(&mut app).is_empty());
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .refine
                .materials[0]
                .as_ref()
                .unwrap()
                .unique_id,
            Some(7003)
        );
    }

    #[test]
    fn refine_host_initial_show_positions_inventory_without_resetting_player_drag() {
        let (mut app, _, _, _) = app();
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .inventory_window
                .left,
            NPC_SERVICE_INVENTORY_ORIGIN.x as f32
        );
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .inventory_window
            .left = 600.0;
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .inventory_window
                .left,
            600.0
        );
    }
}

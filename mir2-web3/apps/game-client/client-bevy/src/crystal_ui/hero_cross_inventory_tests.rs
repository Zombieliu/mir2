use super::*;
fn fixture() -> (App, Entity, Vec2, Vec2) {
    let info=serde_json::from_value(serde_json::json!({"object_id":12,"name":"Hero","class":"Warrior","gender":"Male","level":1,"hair":0,"hp":10,"mp":10,"experience":0,"max_experience":100,"inventory":vec![serde_json::Value::Null;10],"equipment":null,"magics":[],"auto_pot":false,"auto_hp_percent":30,"auto_mp_percent":30,"hp_item_index":0,"mp_item_index":0})).unwrap();
    let hero = HeroModel {
        info: Some(info),
        ..Default::default()
    };
    let mut state = NativePlayerUiState::default();
    state.toggle_inventory();
    state.hero.bootstrap(hero.info.clone().unwrap());
    state.hero.inventory_open = true;
    state.hero.inventory_position = [600, 0];
    let mut player = InventoryModel::default();
    player.items.push(ItemModel {
        container: 0,
        slot: 0,
        unique_id: Some(71),
        quantity: 1,
        ..Default::default()
    });
    let from = Vec2::new(
        state.inventory_window.left + INVENTORY_GRID_ORIGIN.x as f32 + 4.,
        state.inventory_window.top + INVENTORY_GRID_ORIGIN.y as f32 + 4.,
    );
    let to = Vec2::new(600. + 14. + 37. + 4., 23. + 4.);
    let mut app = App::new();
    app.insert_resource(state)
        .insert_resource(player)
        .insert_resource(hero)
        .init_resource::<crate::hero_model::HeroModelReceipts>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .add_message::<bevy::window::WindowEvent>()
        .add_systems(Update, (host::observe, process).chain());
    let mut window = Window::default();
    window.focused = true;
    window.resolution.set(1024., 768.);
    let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
    (app, entity, from, to)
}
fn motion(app: &mut App, window: Entity, position: Vec2) {
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
fn button(app: &mut App, window: Entity, button: MouseButton, pressed: bool) {
    app.world_mut()
        .write_message(bevy::window::WindowEvent::MouseButtonInput(
            bevy::input::mouse::MouseButtonInput {
                window,
                button,
                state: if pressed {
                    bevy::input::ButtonState::Pressed
                } else {
                    bevy::input::ButtonState::Released
                },
            },
        ));
}
#[test]
fn ordered_batched_drag_keeps_player_origin_and_hero_raw_destination() {
    let (mut app, w, from, to) = fixture();
    motion(&mut app, w, from);
    button(&mut app, w, MouseButton::Left, true);
    motion(&mut app, w, to);
    button(&mut app, w, MouseButton::Left, false);
    app.update();
    assert!(matches!(
        app.world().resource::<NativePlayerUiState>().hero.pending,
        Some(C::TransferHeroItem { from: 0, to: 3 })
    ));
    assert_eq!(
        app.world().resource::<InventoryModel>().items[0].unique_id,
        Some(71)
    );
}
#[test]
fn exact_failed_transfer_then_other_success_does_not_clear_player_selection() {
    let (mut app, w, from, to) = fixture();
    motion(&mut app, w, from);
    button(&mut app, w, MouseButton::Left, true);
    motion(&mut app, w, to);
    button(&mut app, w, MouseButton::Left, false);
    app.update();
    let mut failed = app.world().resource::<HeroModel>().clone();
    failed.item_result_serial = 1;
    failed.last_item_result = Some((
        "TransferHeroItem".into(),
        serde_json::json!({"from":0,"to":3,"success":false}),
    ));
    let mut other = failed.clone();
    other.item_result_serial = 2;
    other.last_item_result = Some((
        "MoveItem".into(),
        serde_json::json!({"grid":"HeroInventory","from":5,"to":6,"success":true}),
    ));
    app.world_mut()
        .resource_mut::<crate::hero_model::HeroModelReceipts>()
        .0
        .extend([failed, other.clone()]);
    *app.world_mut().resource_mut::<HeroModel>() = other;
    app.update();
    let ui = app.world().resource::<NativePlayerUiState>();
    assert!(ui.hero.pending.is_none());
    assert_eq!(ui.hero.cross.selected_cell(), Some(Cell::Player(0)));
}
#[test]
fn right_cancel_and_closing_player_bag_clear_cross_selection() {
    let (mut app, w, from, _) = fixture();
    motion(&mut app, w, from);
    button(&mut app, w, MouseButton::Left, true);
    button(&mut app, w, MouseButton::Left, false);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hero
        .cross
        .selected_cell()
        .is_some());
    button(&mut app, w, MouseButton::Right, true);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hero
        .cross
        .selected_cell()
        .is_none());
    button(&mut app, w, MouseButton::Left, true);
    app.update();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .toggle_inventory();
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hero
        .cross
        .selected_cell()
        .is_none());
}
#[test]
fn hit_order_matches_player_front_and_hero_window_z() {
    let (mut app, _, _, _) = fixture();
    let from = Vec2::new(20., 41.);
    let player = app.world().resource::<InventoryModel>().clone();
    let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
    state.hero.inventory_position = [0, 0];
    state.hero.front = geometry::HeroWindow::Inventory;
    assert!(matches!(hit(&state, &player, from), Some(Cell::Hero(_))));
    state.hero.cross.player_front = true;
    assert_eq!(hit(&state, &player, from), Some(Cell::Player(0)));
    assert!(
        geometry::window_z(&state.hero, geometry::HeroWindow::Inventory)
            > geometry::window_z(&state.hero, geometry::HeroWindow::Belt)
    );
}

#[test]
fn right_cancel_does_not_also_use_the_covered_hero_potion() {
    let (mut app, w, from, to) = fixture();
    app.add_message::<KeyboardInput>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_systems(Update, host::process.after(process));
    let mut source = crate::inventory::CrystalItemTooltipSourceModel::default();
    source.info.item_type = 13;
    source.info.required_class = 31;
    source.info.required_gender = 3;
    app.world_mut()
        .resource_mut::<HeroModel>()
        .inventory_view
        .items
        .push(ItemModel {
            container: 0,
            slot: 3,
            unique_id: Some(88),
            quantity: 1,
            tooltip_source: Some(source),
            ..Default::default()
        });
    motion(&mut app, w, from);
    button(&mut app, w, MouseButton::Left, true);
    button(&mut app, w, MouseButton::Left, false);
    app.update();
    motion(&mut app, w, to);
    button(&mut app, w, MouseButton::Right, true);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .hero
        .cross
        .selected_cell()
        .is_none());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    app.update();
    button(&mut app, w, MouseButton::Right, true);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.update();
    assert!(matches!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .as_slice(),
        [NativePlayerUiIntent::HeroPacket(C::UseItem {
            grid: G::HeroInventory,
            unique_id: 88
        })]
    ));
}

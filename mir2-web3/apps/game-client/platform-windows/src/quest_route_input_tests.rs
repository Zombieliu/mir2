use super::*;
use mir2_client_bevy::big_map::BigMapModel;
use mir2_client_bevy::chat::ChatModel;
use mir2_client_bevy::quest_ui::QuestUiState;

fn d401_route_app() -> (
    bevy::prelude::App,
    std::sync::mpsc::Receiver<GatewayCommand>,
    QuestRouteNavigationIntent,
) {
    let (mut app, receiver) = input_app();
    install_movement_clock_and_inbox(&mut app);
    app.world_mut().spawn(stage_window(bevy::prelude::Vec2::new(150., 280.)));
    app.init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<NativePlayerUiState>()
        .init_resource::<NpcDialogModel>()
        .init_resource::<UiReadModel>()
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<QuestRouteNavigationIntentQueue>()
        .init_resource::<QuestUiState>()
        .init_resource::<ChatModel>();
    assert!(!app.world().resource::<NativePlayerUiState>().quest_open());
    assert!(!app.world().resource::<NativePlayerUiState>().bigmap_open());
    let mut big_map = BigMapModel::default();
    big_map.set_current_map(47);
    // MapInformation exists immediately; NewMapInfo is requested only when
    // opening the Big Map. A visible task card must not depend on that cache.
    assert!(big_map.current_map().is_none());
    let intent = QuestRouteNavigationIntent {
        quest_index: 2_110_010,
        reset_epoch: big_map.reset_epoch,
        map_index: 47,
        x: 24,
        y: 182,
    };
    app.insert_resource(big_map);
    let mut entities = app.world_mut().resource_mut::<EntityModelSet>();
    entities.entities[0].x = 23;
    entities.entities[0].y = 176;
    entities.entities[0].direction = Some("down".into());
    app.world_mut().resource_mut::<NativeEntityPresentation>()
        .observe_packet_payload(serde_json::json!({"mapFileName":"D401"}), 0);
    app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
    (app, receiver, intent)
}

#[test]
fn quest_route_card_without_diary_or_big_map_cache_reaches_d401_exit() {
    let (mut app, receiver, intent) = d401_route_app();
    app.world_mut().resource_mut::<QuestRouteNavigationIntentQueue>().push(intent);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();
    assert!(receiver.try_recv().is_err(), "a task-card press must not become a world click");
    assert!(!app.world().resource::<QuestRouteNavigationIntentQueue>().is_empty());
    {
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.clear_just_pressed(MouseButton::Left);
        mouse.release(MouseButton::Left);
    }
    app.update();
    let movement = app.world().resource::<WorldPointerMovementState>();
    assert_eq!(movement.auto_path_destination, Some((24, 182)),
        "the closed-diary card must start using the real D401 collision map");
    assert_eq!(movement.map_auto_path.as_ref().unwrap().steps.last(), Some(&(24, 182)));
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    for _ in 0..12 {
        app.update();
        let command = receiver.try_recv().expect("route must progress after mouse-up while cursor stays over card");
        assert!(matches!(command, GatewayCommand::Player(PlayerIntent::Walk { .. } | PlayerIntent::Run { .. })),
            "only ordinary movement may execute a quest route: {command:?}");
        let pending = app.world().resource::<WorldPointerMovementState>().pending.back().unwrap().clone();
        app.update();
        assert!(receiver.try_recv().is_err(), "wait for authoritative acknowledgement");
        {
            let mut entities = app.world_mut().resource_mut::<EntityModelSet>();
            entities.entities[0].x = pending.to.0;
            entities.entities[0].y = pending.to.1;
        }
        push_test_movement_ack(&app, pending.to.0, pending.to.1, pending.direction);
        advance_movement_clock(&mut app, 600);
        if pending.to == (24, 182) {
            app.update();
            assert!(receiver.try_recv().is_err());
            assert!(app.world().resource::<WorldPointerMovementState>().auto_path_destination.is_none());
            return;
        }
    }
    panic!("D401 exit route did not arrive within its bounded steps");
}

#[test]
fn quest_route_uses_current_identity_even_when_a_remote_map_was_browsed() {
    let (mut app, _, intent) = d401_route_app();
    app.world_mut().resource_mut::<BigMapModel>().active_map_index = Some(1);
    app.world_mut().resource_mut::<QuestRouteNavigationIntentQueue>().push(intent);
    app.update();
    assert_eq!(app.world().resource::<WorldPointerMovementState>().auto_path_destination, Some((24, 182)));
}

#[test]
fn quest_route_rejects_stale_map_with_visible_feedback_and_no_movement() {
    let (mut app, receiver, mut intent) = d401_route_app();
    intent.reset_epoch = intent.reset_epoch.wrapping_add(1);
    app.world_mut().resource_mut::<QuestRouteNavigationIntentQueue>().push(intent);
    app.update();
    assert!(receiver.try_recv().is_err());
    assert!(app.world().resource::<WorldPointerMovementState>().auto_path_destination.is_none());
    assert!(app.world().resource::<QuestUiState>().feedback.as_ref()
        .is_some_and(|feedback| feedback.is_error && feedback.message.contains("入口导航已过期")));
}

fn bichon_entrance_app() -> (bevy::prelude::App, std::sync::mpsc::Receiver<GatewayCommand>) {
    let (mut app, receiver, mut intent) = d401_route_app();
    app.world_mut().resource_mut::<BigMapModel>().set_current_map(1);
    intent.map_index = 1;
    intent.reset_epoch = app.world().resource::<BigMapModel>().reset_epoch;
    intent.x = 147;
    intent.y = 33;
    {
        let mut entities = app.world_mut().resource_mut::<EntityModelSet>();
        entities.entities[0].x = 205;
        entities.entities[0].y = 62;
    }
    app.world_mut().resource_mut::<NativeEntityPresentation>()
        .observe_packet_payload(serde_json::json!({"mapFileName":"0"}), 0);
    app.world_mut().resource_mut::<QuestRouteNavigationIntentQueue>().push(intent);
    app.update();
    assert_eq!(app.world().resource::<WorldPointerMovementState>().auto_path_destination, Some((147, 33)));
    (app, receiver)
}

#[test]
fn bichon_route_survives_passive_ui_hover_without_sending_extra_moves() {
    for surface in ["status", "buff", "skill_state", "skill_geometry", "hero", "inventory"] {
        let (mut app, receiver) = bichon_entrance_app();
        {
            let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
            match surface {
                "status" => ui.status_hud.hovered = true,
                "buff" => ui.hero_buffs.rows.hovered = true,
                "skill_state" => ui.skill_bars.hovered = true,
                "skill_geometry" => {
                    ui.core.options.skill_bar = true;
                    ui.core.options.skill_bar_positions[0] = [100, 270];
                    ui.skill_bars.visible[0] = true;
                }
                "hero" => {
                    ui.hero.interactive = true;
                    ui.hero.inventory_open = true;
                    ui.hero.inventory_position = [100, 200];
                }
                "inventory" => ui.toggle_inventory(),
                _ => unreachable!(),
            }
        }
        // The cursor is inside each configured hover surface; no button is down.
        if surface == "inventory" {
            app.world_mut().query::<&mut Window>().single_mut(app.world_mut()).unwrap()
                .set_cursor_position(Some(bevy::prelude::Vec2::new(150., 100.)));
        }
        app.update();
        assert!(app.world().resource::<WorldPointerMovementState>().map_auto_path.is_some(),
            "passive {surface} hover discarded the route");
        assert!(matches!(receiver.try_recv(), Ok(GatewayCommand::Player(PlayerIntent::Walk { .. } | PlayerIntent::Run { .. }))),
            "passive {surface} hover stalled movement");
        app.update();
        assert!(receiver.try_recv().is_err(), "{surface}: still wait for the real movement ACK");
    }
}

#[test]
fn bichon_route_reaches_oma_entrance_around_static_and_new_entity_obstacles() {
    let (mut app, receiver) = bichon_entrance_app();
    let initially_next = app.world().resource::<WorldPointerMovementState>()
        .map_auto_path.as_ref().unwrap().steps[0];
    // A newly visible entity occupies the originally planned next tile.
    app.world_mut().resource_mut::<EntityModelSet>().entities.push(EntityModel {
        object_id: "9001".into(), kind: EntityKind::Monster, name: "Obstacle".into(),
        x: initially_next.0, y: initially_next.1, level: Some(1), direction: Some("up".into()),
    });
    let map = crate::map_parser::load_map("0").expect("packaged real Bichon collision map");
    let mut moved = 0;
    for _ in 0..256 {
        app.update();
        let command = receiver.try_recv().expect("ordinary entrance path must keep progressing");
        assert!(matches!(command, GatewayCommand::Player(PlayerIntent::Walk { .. } | PlayerIntent::Run { .. })));
        let pending = app.world().resource::<WorldPointerMovementState>().pending.back().unwrap().clone();
        let (dx, dy) = direction_to_delta(pending.direction);
        let distance = chebyshev_distance(pending.from, pending.to);
        for step in 1..=distance {
            let tile = (pending.from.0 + dx * step, pending.from.1 + dy * step);
            assert!(!map.cell_blocks_movement(tile.0, tile.1), "route crosses static obstacle at {tile:?}");
            assert_ne!(tile, initially_next, "route crosses the newly occupied tile");
        }
        app.update();
        assert!(receiver.try_recv().is_err(), "one authoritative movement at a time");
        {
            let mut entities = app.world_mut().resource_mut::<EntityModelSet>();
            entities.entities[0].x = pending.to.0;
            entities.entities[0].y = pending.to.1;
        }
        push_test_movement_ack(&app, pending.to.0, pending.to.1, pending.direction);
        advance_movement_clock(&mut app, 600);
        moved += 1;
        if pending.to == (147, 33) {
            app.update();
            assert!(receiver.try_recv().is_err());
            assert!(app.world().resource::<WorldPointerMovementState>().map_auto_path.is_none());
            eprintln!("Bichon (205,62) -> Oma entrance (147,33): {moved} acknowledged steps, static collision and new entity avoided");
            return;
        }
    }
    panic!("ordinary entrance route did not arrive within its fixed test limit");
}

#[test]
fn bichon_route_keeps_modal_drag_and_manual_cancel_guards() {
    for blocker in ["chat", "options", "storage", "npc_shop", "mail", "hero_modal", "hero_drag", "skill_drag", "menu_consumed", "npc", "dead", "escape", "ui_press", "ui_press_release", "middle_press", "unfocused"] {
        let (mut app, receiver) = bichon_entrance_app();
        {
            let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
            ui.status_hud.hovered = true;
            match blocker {
                "chat" => ui.core.chat_focused = true,
                "options" => ui.toggle_options(),
                "storage" => ui.core.panel = mir2_ui_core::state::UiPanel::Storage,
                "npc_shop" => ui.core.panel = mir2_ui_core::state::UiPanel::NpcShop,
                "mail" => ui.core.panel = mir2_ui_core::state::UiPanel::Mail,
                "hero_modal" => ui.hero.assign.open = true,
                "hero_drag" => ui.hero.dragging = Some((Default::default(), [0., 0.])),
                "skill_drag" => ui.skill_bars.dragging = Some((0, [0., 0.])),
                "menu_consumed" => ui.menu_pointer_consumed = true,
                _ => (),
            }
        }
        match blocker {
            "npc" => app.world_mut().resource_mut::<NpcDialogModel>().is_open = true,
            "dead" => {
                let mut model = app.world_mut().resource_mut::<UiReadModel>();
                model.player.hp = 0;
                model.player.max_hp = 162;
            }
            "escape" => app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape),
            "ui_press" => app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left),
            "middle_press" => app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Middle),
            "ui_press_release" => {
                let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
                mouse.press(MouseButton::Left);
                mouse.release(MouseButton::Left);
            }
            "unfocused" => app.world_mut().query::<&mut Window>().single_mut(app.world_mut()).unwrap().focused = false,
            _ => (),
        }
        app.update();
        assert!(receiver.try_recv().is_err(), "{blocker} leaked a movement command");
        assert!(app.world().resource::<WorldPointerMovementState>().map_auto_path.is_none(),
            "{blocker} must still cancel the route");
    }
}

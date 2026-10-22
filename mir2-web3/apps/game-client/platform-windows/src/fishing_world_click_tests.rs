use super::*;
use bevy::prelude::Vec2;
use mir2_client_bevy::fishing_click::{fishing_walk_candidates, fishing_water_target};
use mir2_client_bevy::inventory::{
    CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel, ItemModel,
};

const SOUTH_WEST_SHORE: (i32, i32) = (340, 242);

fn shore_payload(origin: (i32, i32), facing: &str, transform: i16) -> serde_json::Value {
    serde_json::json!({
        "mapFileName": "0",
        "sceneView": {"center": {"x": origin.0, "y": origin.1}},
        "entities": [{
            "objectId": "1000", "kind": "selfPlayer", "x": origin.0, "y": origin.1,
            "direction": facing, "dead": false, "transformType": transform,
            "sprite": {"bodyLibrary": "CArmour/00", "directionStride": 4, "frameBaseOffset": 0}
        }]
    })
}

fn assert_source_shore(origin: (i32, i32), raw_direction: u8) {
    // These integration tests require the ordinary native asset pack. Missing
    // map evidence must fail instead of silently manufacturing fishable water.
    let map = crate::map_parser::load_map("0").expect("the native 0.map asset pack is required");
    let collision = map.player_movement_collision("0");
    assert!(!collision.cell_blocks_movement(origin.0, origin.1));
    for candidate in fishing_walk_candidates(origin, raw_direction).unwrap() {
        assert!(collision.cell_blocks_movement(candidate.cell.0, candidate.cell.1));
    }
    let water = fishing_water_target(origin, raw_direction).unwrap();
    assert!(crate::map_parser::fishing_attribute("0", water.0, water.1).is_some());
}

fn shore_app(
    origin: (i32, i32),
    delta: (i32, i32),
    facing: &str,
) -> (
    bevy::prelude::App,
    std::sync::mpsc::Receiver<GatewayCommand>,
) {
    let (mut app, receiver) = input_app();
    install_movement_clock_and_inbox(&mut app);
    let cursor = Vec2::new(
        (10 + delta.0) as f32 * 48.0 + 24.0,
        (11 + delta.1) as f32 * 32.0 + 16.0,
    );
    app.world_mut().spawn(stage_window(cursor));
    app.insert_resource(ButtonInput::<MouseButton>::default());
    app.insert_resource(NpcDialogModel::default());
    app.init_resource::<QuestUiIntentQueue>();
    let mut read = UiReadModel::default();
    read.player.hp = 100;
    read.player.max_hp = 100;
    app.insert_resource(read);
    app.insert_resource(EntityModelSet {
        entities: vec![EntityModel {
            object_id: "1000".into(),
            kind: EntityKind::SelfPlayer,
            name: "Fisher".into(),
            x: origin.0,
            y: origin.1,
            level: Some(30),
            direction: Some(facing.into()),
        }],
    });
    let rod = mir2_game_data::crystal_item_by_name("BlueFishingRod").unwrap();
    let inventory = InventoryModel {
        items: vec![ItemModel {
            unique_id: Some(700),
            key: format!("crystal-item-{}", rod.item_index),
            quantity: 1,
            container: 2,
            slot: 0,
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: rod.item_index,
                    item_type: rod.item_type,
                    shape: rod.shape,
                    name: rod.name.clone(),
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id: 700,
                    item_index: rod.item_index,
                    slots: vec![None; 5],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut ui = NativePlayerUiState::default();
    ui.equipment_dialogs.sync_equipment(&inventory);
    assert!(ui.equipment_dialogs.rod.is_some());
    app.insert_resource(ui);
    app.insert_resource(inventory);
    let mut presentation = NativeEntityPresentation::default();
    presentation.set_hover_grid_context_for_test(origin, (cursor.x, cursor.y));
    presentation.observe_packet_payload(shore_payload(origin, facing, 0), 0);
    assert_eq!(presentation.self_equipment_pose("1000"), Some((true, 0)));
    assert_eq!(
        presentation.hovered_grid_position(),
        Some((origin.0 + delta.0, origin.1 + delta.1))
    );
    app.insert_resource(presentation);
    advance_movement_clock(&mut app, 1000);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
    (app, receiver)
}

fn assert_only_cast(receiver: &std::sync::mpsc::Receiver<GatewayCommand>) {
    let commands = receiver.try_iter().collect::<Vec<_>>();
    assert_eq!(commands.len(), 1, "{commands:?}");
    assert!(
        matches!(
            commands[0],
            GatewayCommand::Wire(NativeOutboundCommand::FishingCast { cast_out: true })
        ),
        "{commands:?}"
    );
}

#[test]
fn fishing_world_click_lowercase_south_west_turn_then_cast_uses_actual_input_path() {
    assert_source_shore(SOUTH_WEST_SHORE, 5);
    assert_eq!(
        movement_direction_toward(Some((339, 243)), SOUTH_WEST_SHORE),
        Some("downleft")
    );
    let (mut app, receiver) = shore_app(SOUTH_WEST_SHORE, (-1, 1), "Up");
    app.update();
    let commands = receiver.try_iter().collect::<Vec<_>>();
    assert_eq!(commands.len(), 1, "{commands:?}");
    assert!(
        matches!(&commands[0], GatewayCommand::Player(PlayerIntent::Turn { direction }) if direction == "downleft"),
        "{commands:?}"
    );
    assert_eq!(
        app.world()
            .resource::<WorldPointerMovementState>()
            .next_move_send_at_ms,
        1200.0
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .equipment_dialogs
            .last_cast_ms,
        0
    );

    // The next click pass consumes the actual authoritative facing update.
    app.world_mut().resource_mut::<EntityModelSet>().entities[0].direction =
        Some("DownLeft".into());
    app.world_mut()
        .resource_mut::<NativeEntityPresentation>()
        .observe_packet_payload(shore_payload(SOUTH_WEST_SHORE, "DownLeft", 0), 1200);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_pressed(MouseButton::Left);
    advance_movement_clock(&mut app, 200);
    app.update();
    assert_only_cast(&receiver);
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .equipment_dialogs
            .last_cast_ms,
        1200
    );
    assert!(app
        .world()
        .resource::<WorldPointerMovementState>()
        .pending
        .is_empty());
}

#[test]
fn fishing_world_click_already_facing_south_west_casts_without_a_turn() {
    assert_source_shore(SOUTH_WEST_SHORE, 5);
    let (mut app, receiver) = shore_app(SOUTH_WEST_SHORE, (-1, 1), "DownLeft");
    app.update();
    assert_only_cast(&receiver);
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .equipment_dialogs
            .last_cast_ms,
        1000
    );
}

#[test]
fn fishing_world_click_all_eight_directions_share_case_insensitive_facing() {
    // Actual source-map shores, each blocked in direct/+1/-1 with water three ahead.
    for (raw, origin, delta, title) in [
        (0, (252, 684), (0, -1), "Up"),
        (1, (324, 398), (1, -1), "UpRight"),
        (2, (248, 682), (1, 0), "Right"),
        (3, (85, 630), (1, 1), "DownRight"),
        (4, (18, 391), (0, 1), "Down"),
        (5, (17, 389), (-1, 1), "DownLeft"),
        (6, (17, 388), (-1, 0), "Left"),
        (7, (250, 686), (-1, -1), "UpLeft"),
    ] {
        assert_source_shore(origin, raw);
        let target = (origin.0 + delta.0, origin.1 + delta.1);
        let lower = movement_direction_toward(Some(target), origin).unwrap();
        assert_eq!(lower, title.to_ascii_lowercase());
        let mixed = title
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i % 2 == 0 {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect::<String>();
        for facing in [
            lower.to_owned(),
            title.to_owned(),
            title.to_ascii_uppercase(),
            mixed,
        ] {
            let (mut app, receiver) = shore_app(origin, delta, &facing);
            app.update();
            assert_only_cast(&receiver);
        }
    }
}

#[test]
fn fishing_world_click_unknown_facing_does_not_turn_or_cast() {
    assert_source_shore(SOUTH_WEST_SHORE, 5);
    for facing in ["", "southwest", "down-left", "downleft ", "8"] {
        let (mut app, receiver) = shore_app(SOUTH_WEST_SHORE, (-1, 1), facing);
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "invalid direction {facing:?} must stay unknown"
        );
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .equipment_dialogs
                .last_cast_ms,
            0
        );
    }
}

#[test]
fn fishing_world_click_original_rod_fishing_transform_pose_and_clock_guards_remain() {
    assert_source_shore(SOUTH_WEST_SHORE, 5);
    for guard in ["rod", "fishing", "transform", "pose", "clock"] {
        let (mut app, receiver) = shore_app(SOUTH_WEST_SHORE, (-1, 1), "DownLeft");
        match guard {
            "rod" => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .equipment_dialogs
                    .rod = None
            }
            "fishing" => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .equipment_dialogs
                    .fishing = true
            }
            "clock" => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .equipment_dialogs
                    .last_cast_ms = 999
            }
            "transform" | "pose" => {
                let mut payload = shore_payload(
                    SOUTH_WEST_SHORE,
                    "DownLeft",
                    if guard == "transform" { 6 } else { 0 },
                );
                if guard == "pose" {
                    payload["entities"][0]["_nativeAnimationAction"] = serde_json::json!("walking");
                    payload["entities"][0]["_nativeAnimationSequence"] = serde_json::json!(1);
                }
                app.world_mut()
                    .resource_mut::<NativeEntityPresentation>()
                    .observe_packet_payload(payload, 1000);
            }
            _ => unreachable!(),
        }
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "{guard} must not produce a turn or cast"
        );
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .equipment_dialogs
                .last_cast_ms,
            if guard == "clock" { 999 } else { 0 }
        );
    }
}

#[test]
fn fishing_world_click_missing_facing_stays_unknown_on_a_down_shore() {
    let origin = (18, 391);
    assert_source_shore(origin, 4);
    assert_eq!(
        movement_direction_toward(Some((18, 392)), origin),
        Some("down")
    );
    let (mut app, receiver) = shore_app(origin, (0, 1), "Down");
    app.world_mut().resource_mut::<EntityModelSet>().entities[0].direction = None;
    let mut payload = shore_payload(origin, "Down", 0);
    payload["entities"][0]["direction"] = serde_json::Value::Null;
    app.world_mut()
        .resource_mut::<NativeEntityPresentation>()
        .observe_packet_payload(payload, 1000);
    app.update();
    assert!(receiver.try_recv().is_err());
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .equipment_dialogs
            .last_cast_ms,
        0
    );
}

#[test]
fn fishing_world_click_dry_source_shore_refuses_turn_and_cast() {
    let origin = (13, 333);
    let map = crate::map_parser::load_map("0").unwrap();
    let collision = map.player_movement_collision("0");
    assert!(!collision.cell_blocks_movement(origin.0, origin.1));
    for candidate in fishing_walk_candidates(origin, 5).unwrap() {
        assert!(collision.cell_blocks_movement(candidate.cell.0, candidate.cell.1));
    }
    let water = fishing_water_target(origin, 5).unwrap();
    assert_eq!(
        crate::map_parser::fishing_raw_light("0", water.0, water.1),
        Some(0)
    );
    for facing in ["Up", "DownLeft"] {
        let (mut app, receiver) = shore_app(origin, (-1, 1), facing);
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "dry source cell cannot {facing} turn/cast"
        );
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .equipment_dialogs
                .last_cast_ms,
            0
        );
    }
}

#[test]
fn fishing_world_click_definitely_unsent_cast_releases_only_local_clock() {
    assert_source_shore(SOUTH_WEST_SHORE, 5);
    let (mut app, receiver) = shore_app(SOUTH_WEST_SHORE, (-1, 1), "DownLeft");
    drop(receiver);
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .equipment_dialogs
            .last_cast_ms,
        0
    );
    assert!(
        !app.world()
            .resource::<NativePlayerUiState>()
            .equipment_dialogs
            .fishing
    );
    assert!(app
        .world()
        .resource::<WorldPointerMovementState>()
        .pending
        .is_empty());
}

#[test]
fn fishing_world_click_current_owner_sends_but_stale_owner_and_scene_do_not() {
    assert_source_shore(SOUTH_WEST_SHORE, 5);
    for scope in ["current", "owner", "scene"] {
        let (mut app, _plain_receiver) = shore_app(SOUTH_WEST_SHORE, (-1, 1), "DownLeft");
        let (sender, mut receiver) = crate::gateway::command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        let stamp = fence.test_world_ready(1000, 0);
        let commands = GatewayCommands::new(sender);
        assert!(commands.activate_world_stamp(stamp));
        app.insert_resource(commands);
        match scope {
            "owner" => {
                fence.test_owner_change(2000, 0);
            }
            "scene" => fence.test_scene_boundary(1),
            _ => {}
        }
        app.update();
        if scope == "current" {
            let GatewayCommand::Owned(command) = receiver.try_recv_for_test().unwrap() else {
                panic!("production fence must retain command ownership");
            };
            assert!(matches!(
                crate::gateway::test_owned_wire(&command),
                Some(NativeOutboundCommand::FishingCast { cast_out: true })
            ));
            assert!(receiver.try_recv_for_test().is_err());
            assert_eq!(
                app.world()
                    .resource::<NativePlayerUiState>()
                    .equipment_dialogs
                    .last_cast_ms,
                1000
            );
        } else {
            assert!(
                receiver.try_recv_for_test().is_err(),
                "stale {scope} must be refused"
            );
            assert_eq!(
                app.world()
                    .resource::<NativePlayerUiState>()
                    .equipment_dialogs
                    .last_cast_ms,
                0
            );
        }
    }
}

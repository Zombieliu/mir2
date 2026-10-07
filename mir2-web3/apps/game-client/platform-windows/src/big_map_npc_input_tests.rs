use mir2_client_bevy::big_map::{BigMapInfo, BigMapNpc, BigMapPoint};

fn map_npc_app() -> (
    bevy::prelude::App,
    std::sync::mpsc::Receiver<GatewayCommand>,
    QuestRouteNavigationIntent,
) {
    let (mut app, receiver, _) = d401_route_app();
    let mut model = BigMapModel::default();
    model.set_current_map(1);
    model.apply_new_map_info(
        1,
        BigMapInfo {
            title: "BichonProvince".into(),
            width: 800,
            height: 800,
            big_map: 101,
            movements: vec![],
            npcs: vec![BigMapNpc {
                index: 5,
                file_name: "BichonProvince/BorderVillage/Blacksmith".into(),
                name: "Blacksmith_Smith".into(),
                map_index: 1,
                location: BigMapPoint { x: 296, y: 613 },
                image: 0,
                rate: 0,
                show_on_big_map: true,
                big_map_icon: 112,
                object_id: 5,
                icon: 0,
                can_teleport_to: false,
            }],
        },
    );
    assert!(model.select_npc(5));
    let intent = QuestRouteNavigationIntent {
        target: QuestRouteTarget::MapNpc { object_id: 5 },
        quest_index: 0,
        reset_epoch: model.reset_epoch,
        map_index: 1,
        x: 296,
        y: 613,
    };
    app.insert_resource(model);
    {
        let mut entities = app.world_mut().resource_mut::<EntityModelSet>();
        entities
            .entities
            .retain(|entity| entity.kind == EntityKind::SelfPlayer);
        entities.entities[0].x = 289;
        entities.entities[0].y = 618;
        entities.entities.push(EntityModel {
            object_id: "5".into(),
            kind: EntityKind::Npc,
            name: "Blacksmith_Smith".into(),
            x: 296,
            y: 613,
            level: None,
            direction: None,
        });
    }
    app.world_mut()
        .resource_mut::<NativeEntityPresentation>()
        .observe_packet_payload(serde_json::json!({"mapFileName":"0"}), 0);
    (app, receiver, intent)
}

#[test]
fn big_map_npc_go_to_releases_mouse_and_reaches_occupied_smith_with_ordinary_acknowledged_moves() {
    let (mut app, receiver, intent) = map_npc_app();
    assert_eq!(
        crate::map_parser::map_cell_blocks_player_movement("0", 289, 618),
        Some(false)
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .local_keys
        .auto_run = true;
    app.world_mut()
        .resource_mut::<QuestRouteNavigationIntentQueue>()
        .push(intent);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(receiver.try_recv().is_err());
    assert!(!app
        .world()
        .resource::<QuestRouteNavigationIntentQueue>()
        .is_empty());
    {
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.clear_just_pressed(MouseButton::Left);
        mouse.release(MouseButton::Left);
    }
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    assert!(
        !app.world()
            .resource::<NativePlayerUiState>()
            .local_keys
            .auto_run,
        "an earlier auto-run must not replace the accepted NPC route on the next frame"
    );
    let destination = app
        .world()
        .resource::<WorldPointerMovementState>()
        .auto_path_destination
        .unwrap();
    assert_ne!(destination, (296, 613));
    assert!(chebyshev_distance(destination, (296, 613)) <= 2);
    for step in 0..32 {
        app.update();
        let command = receiver.try_recv().expect("ordinary route must advance");
        assert!(
            matches!(
                command,
                GatewayCommand::Player(PlayerIntent::Walk { .. } | PlayerIntent::Run { .. })
            ),
            "{command:?}"
        );
        if step == 0 {
            assert!(matches!(
                command,
                GatewayCommand::Player(PlayerIntent::Walk { .. })
            ));
        }
        let pending = app
            .world()
            .resource::<WorldPointerMovementState>()
            .pending
            .back()
            .unwrap()
            .clone();
        let (dx, dy) = direction_to_delta(pending.direction);
        for n in 1..=chebyshev_distance(pending.from, pending.to) {
            let tile = (pending.from.0 + dx * n, pending.from.1 + dy * n);
            assert_eq!(
                crate::map_parser::map_cell_blocks_player_movement("0", tile.0, tile.1),
                Some(false)
            );
            assert_ne!(tile, (296, 613));
        }
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "no extra move before the server ACK"
        );
        {
            let mut entities = app.world_mut().resource_mut::<EntityModelSet>();
            entities.entities[0].x = pending.to.0;
            entities.entities[0].y = pending.to.1;
        }
        push_test_movement_ack(&app, pending.to.0, pending.to.1, pending.direction);
        advance_movement_clock(&mut app, 600);
        if pending.to == destination {
            app.update();
            assert!(receiver.try_recv().is_err());
            let movement = app.world().resource::<WorldPointerMovementState>();
            assert!(
                movement.map_auto_path.is_none()
                    && movement.attack_target.is_none()
                    && movement.hunt_arrival.is_none()
            );
            assert!(!app.world().resource::<NpcDialogModel>().is_open);
            return;
        }
    }
    panic!("Smith approach exceeded its bounded movement budget");
}

#[test]
fn big_map_npc_go_to_rejects_changed_metadata_after_mouse_release() {
    for changed in [
        "coordinate",
        "id",
        "removed",
        "hidden",
        "epoch",
        "map",
        "scene-map",
        "remote",
    ] {
        let (mut app, receiver, mut intent) = map_npc_app();
        if changed == "coordinate" {
            intent.x += 1;
        }
        if changed == "id" {
            intent.target = QuestRouteTarget::MapNpc { object_id: 19 };
        }
        app.world_mut()
            .resource_mut::<QuestRouteNavigationIntentQueue>()
            .push(intent);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(receiver.try_recv().is_err());
        match changed {
            "removed" => app
                .world_mut()
                .resource_mut::<BigMapModel>()
                .maps
                .get_mut(&1)
                .unwrap()
                .info
                .npcs
                .clear(),
            "hidden" => {
                app.world_mut()
                    .resource_mut::<BigMapModel>()
                    .maps
                    .get_mut(&1)
                    .unwrap()
                    .info
                    .npcs[0]
                    .show_on_big_map = false
            }
            "epoch" => app.world_mut().resource_mut::<BigMapModel>().reset_epoch += 1,
            "map" => app
                .world_mut()
                .resource_mut::<BigMapModel>()
                .set_current_map(2),
            "remote" => {
                app.world_mut()
                    .resource_mut::<BigMapModel>()
                    .active_map_index = Some(2)
            }
            "scene-map" => app
                .world_mut()
                .resource_mut::<NativeEntityPresentation>()
                .observe_packet_payload(serde_json::json!({"mapFileName":"D401"}), 0),
            _ => (),
        }
        {
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            mouse.clear_just_pressed(MouseButton::Left);
            mouse.release(MouseButton::Left);
        }
        app.update();
        assert!(receiver.try_recv().is_err(), "{changed}");
        assert!(app
            .world()
            .resource::<QuestRouteNavigationIntentQueue>()
            .is_empty());
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .map_auto_path
            .is_none());
        assert!(
            app.world()
                .resource::<QuestUiState>()
                .feedback
                .as_ref()
                .is_some_and(|f| f.is_error),
            "{changed}"
        );
    }
}

#[test]
fn big_map_npc_route_cancels_on_escape_or_map_reset() {
    for changed in ["escape", "reset"] {
        let (mut app, receiver, intent) = map_npc_app();
        app.world_mut()
            .resource_mut::<QuestRouteNavigationIntentQueue>()
            .push(intent);
        app.update();
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .map_auto_path
            .is_some());
        match changed {
            "escape" => app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape),
            "reset" => app
                .world_mut()
                .resource_mut::<BigMapModel>()
                .reset_for_map(1, None),
            _ => unreachable!(),
        }
        app.update();
        assert!(receiver.try_recv().is_err(), "{changed}");
        let movement = app.world().resource::<WorldPointerMovementState>();
        assert!(movement.map_auto_path.is_none() && movement.auto_path_destination.is_none());
    }
}

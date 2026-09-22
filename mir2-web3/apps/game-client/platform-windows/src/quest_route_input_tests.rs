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

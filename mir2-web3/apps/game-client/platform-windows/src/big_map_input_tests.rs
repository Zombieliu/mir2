use super::*;
use mir2_client_bevy::big_map::{BigMapInfo, BigMapModel, BigMapView};
use mir2_client_bevy::chat::ChatModel;

fn model(width: i32, height: i32) -> BigMapModel {
    let mut model = BigMapModel::default();
    model.set_current_map(0);
    model.apply_new_map_info(
        0,
        BigMapInfo {
            title: "Bichon".into(),
            width,
            height,
            big_map: 1,
            movements: Vec::new(),
            npcs: Vec::new(),
        },
    );
    model
}

#[test]
fn map_image_coordinates_follow_stage_fit_and_exclude_panel_controls() {
    let model = model(700, 700);
    for (width, height) in [(1024., 768.), (1600., 900.), (800., 600.)] {
        let transform =
            mir2_client_bevy::crystal_ui::metrics::CrystalStageTransform::fit(width, height);
        let logical = (146. + 568. * 350.5 / 700., 186. + 380. * 350.5 / 700.);
        let physical = transform.logical_to_physical(logical.0, logical.1);
        let mut window = stage_window(bevy::prelude::Vec2::new(physical.0, physical.1));
        window.resolution.set(width, height);
        assert_eq!(
            big_map_input::destination(&model, big_map_input::image_position(&window).unwrap()),
            Ok((350, 350))
        );
        for point in [
            (145., 200.),
            (714.1, 200.),
            (200., 185.),
            (200., 566.1),
            (850., 210.),
        ] {
            let physical = transform.logical_to_physical(point.0, point.1);
            window.set_cursor_position(Some(bevy::prelude::Vec2::new(physical.0, physical.1)));
            assert!(big_map_input::image_position(&window).is_none());
        }
    }
    let mut remote = model;
    remote.active_map_index = Some(99);
    assert!(big_map_input::destination(&remote, (200., 200.)).is_err());
    remote.view = BigMapView::WorldMap;
    assert!(big_map_input::destination(&remote, (200., 200.)).is_err());
}

fn navigation_app(
    button: MouseButton,
) -> (
    bevy::prelude::App,
    std::sync::mpsc::Receiver<GatewayCommand>,
    (i32, i32),
) {
    let map = crate::map_parser::load_map("0").expect("packaged Bichon collision map");
    let origin = (1..i32::from(map.height) - 1)
        .find_map(|y| {
            (1..i32::from(map.width) - 7)
                .find(|x| (0..7).all(|dx| !map.cell_blocks_movement(x + dx, y)))
                .map(|x| (x, y))
        })
        .expect("open row for deterministic movement");
    let destination = (origin.0 + 6, origin.1);
    let (mut app, receiver) = input_app();
    install_movement_clock_and_inbox(&mut app);
    let mut ui = NativePlayerUiState::default();
    ui.core.panel = mir2_ui_core::state::UiPanel::BigMap;
    assert!(!ui.core.options.new_move);
    app.insert_resource(ui);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<NpcDialogModel>();
    app.init_resource::<UiReadModel>();
    app.init_resource::<QuestUiIntentQueue>();
    app.init_resource::<ChatModel>();
    app.insert_resource(model(i32::from(map.width), i32::from(map.height)));
    let x = 146. + (destination.0 as f32 + 0.5) / f32::from(map.width) * 568.;
    let y = 186. + (destination.1 as f32 + 0.5) / f32::from(map.height) * 380.;
    app.world_mut()
        .spawn(stage_window(bevy::prelude::Vec2::new(x, y)));
    let mut entities = movement_entities();
    entities
        .entities
        .retain(|entity| entity.kind == EntityKind::SelfPlayer);
    let player = entities
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::SelfPlayer)
        .unwrap();
    player.x = origin.0;
    player.y = origin.1;
    app.insert_resource(entities);
    app.world_mut()
        .resource_mut::<NativeEntityPresentation>()
        .observe_packet_payload(serde_json::json!({ "mapFileName": "0" }), 0);
    app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(button);
    (app, receiver, destination)
}

#[test]
fn left_and_right_map_clicks_send_ack_bounded_movement_and_survive_mouse_up_without_new_move() {
    for button in [MouseButton::Left, MouseButton::Right] {
        let (mut app, receiver, destination) = navigation_app(button);
        app.update();
        let first_command = receiver.try_recv();
        assert!(
            matches!(&first_command, Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "right"),
            "button={button:?}, command={first_command:?}, movement={:?}, feedback={:?}",
            app.world().resource::<WorldPointerMovementState>(),
            app.world().resource::<ChatModel>().recent_text(3)
        );
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .auto_path_destination,
            Some(destination)
        );
        assert!(
            !app.world()
                .resource::<WorldPointerMovementState>()
                .pointer_auto_path
        );
        let first = app
            .world()
            .resource::<WorldPointerMovementState>()
            .pending
            .back()
            .unwrap()
            .to;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(button);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(button);
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "must await first step acknowledgement"
        );
        push_test_movement_ack(&app, first.0, first.1, "right");
        advance_movement_clock(&mut app, 600);
        app.update();
        assert!(
            matches!(receiver.try_recv(), Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right")
        );
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .map_auto_path
            .is_some());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .map_auto_path
            .is_none());
        assert!(receiver.try_recv().is_err());
    }
}

#[test]
fn remote_map_and_modal_clicks_never_fall_through_to_world_input() {
    let (mut app, receiver, _) = navigation_app(MouseButton::Right);
    app.world_mut()
        .resource_mut::<BigMapModel>()
        .active_map_index = Some(99);
    app.update();
    assert!(receiver.try_recv().is_err());
    assert!(app
        .world()
        .resource::<WorldPointerMovementState>()
        .map_auto_path
        .is_none());
    assert!(!app
        .world()
        .resource::<ChatModel>()
        .recent_text(1)
        .is_empty());

    let (mut app, receiver, _) = navigation_app(MouseButton::Left);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .keyboard
        .open = true;
    app.update();
    assert!(receiver.try_recv().is_err());
    assert!(app
        .world()
        .resource::<WorldPointerMovementState>()
        .auto_path_destination
        .is_none());
}

use super::*;
use mir2_client_bevy::big_map::BigMapModel;
use mir2_client_bevy::chat::ChatModel;
use mir2_client_bevy::crystal_ui::minimap::{
    mini_map_profile, source_crop, MiniMapView, MiniMapViewState,
};
use mir2_client_bevy::map::MapModel;

fn minimap_app(
    button: MouseButton,
    size: (f32, f32),
) -> (
    bevy::prelude::App,
    std::sync::mpsc::Receiver<GatewayCommand>,
    (i32, i32),
) {
    let (mut app, receiver, target) = super::big_map_input_tests::navigation_app(button);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::None;
    let current_map = mir2_game_data::crystal_map_respawns_ref("0")
        .unwrap()
        .map_index;
    app.world_mut()
        .resource_mut::<BigMapModel>()
        .set_current_map(current_map);
    let map = crate::map_parser::load_map("0").unwrap();
    let player = app
        .world()
        .resource::<EntityModelSet>()
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SelfPlayer)
        .unwrap();
    let profile = mini_map_profile(Some(1), Some(map.width), Some(map.height), 1050, 699).unwrap();
    let crop = source_crop(profile, player.x, player.y);
    let identity = app.world().resource::<BigMapModel>();
    let view = MiniMapView {
        map_index: identity.current_map_index.unwrap(),
        map_epoch: identity.reset_epoch,
        profile,
        crop,
    };
    app.insert_resource(MapModel {
        center_x: player.x,
        center_y: player.y,
        mini_map_index: Some(1),
        map_width: Some(map.width),
        map_height: Some(map.height),
        ..Default::default()
    });
    app.insert_resource(MiniMapViewState {
        displayed: Some(view),
    });
    // MiniMap navigation must work before NewMapInfo or opening the Big Map.
    app.world_mut().resource_mut::<BigMapModel>().maps.clear();
    let cursor = (
        901.0
            + (((target.0 as f32 + 0.5) * profile.image_width / profile.map_width) - crop.left)
                * 120.0
                / crop.width,
        22.0 + (((target.1 as f32 + 0.5) * profile.image_height / profile.map_height) - crop.top)
            * 108.0
            / crop.height,
    );
    let t = mir2_client_bevy::crystal_ui::CrystalStageTransform::fit(size.0, size.1);
    let physical = t.logical_to_physical(cursor.0, cursor.1);
    let mut window = app
        .world_mut()
        .query::<&mut Window>()
        .single_mut(app.world_mut())
        .unwrap();
    window.resolution.set(size.0, size.1);
    window.set_cursor_position(Some(bevy::prelude::Vec2::new(physical.0, physical.1)));
    (app, receiver, target)
}

#[test]
fn minimap_both_buttons_route_without_bigmap_metadata_or_new_move_at_all_stage_scales() {
    for button in [MouseButton::Left, MouseButton::Right] {
        for size in [(1024.0, 768.0), (1600.0, 900.0), (800.0, 600.0)] {
            let (mut app, receiver, target) = minimap_app(button, size);
            app.update();
            assert!(
                matches!(receiver.try_recv(), Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "right"),
                "{button:?}/{size:?}: {:?}",
                app.world().resource::<ChatModel>().recent_text(3)
            );
            let movement = app.world().resource::<WorldPointerMovementState>();
            assert_eq!(movement.auto_path_destination, Some(target));
            assert!(movement.map_auto_path.is_some());
            assert!(!movement.pointer_auto_path);
            let first = movement.pending.back().unwrap().to;
            {
                let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
                mouse.clear_just_pressed(button);
                mouse.release(button);
            }
            app.update();
            assert!(
                receiver.try_recv().is_err(),
                "next step must wait for server ACK"
            );
            push_test_movement_ack(&app, first.0, first.1, "right");
            advance_movement_clock(&mut app, 600);
            app.update();
            assert!(
                matches!(receiver.try_recv(), Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right")
            );
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
            app.update();
            assert!(app
                .world()
                .resource::<WorldPointerMovementState>()
                .map_auto_path
                .is_none());
        }
    }
}

#[test]
fn minimap_stale_or_loading_image_is_consumed_without_world_actions() {
    for case in 0..9 {
        let (mut app, receiver, _) = minimap_app(MouseButton::Left, (1024.0, 768.0));
        match case {
            0 => app.world_mut().resource_mut::<MiniMapViewState>().displayed = None,
            1 => app.world_mut().resource_mut::<BigMapModel>().reset_epoch += 1,
            2 => {
                app.world_mut()
                    .resource_mut::<BigMapModel>()
                    .current_map_index = Some(999)
            }
            3 => app.world_mut().resource_mut::<MapModel>().mini_map_index = Some(8),
            4 => app.world_mut().resource_mut::<MapModel>().map_width = Some(701),
            5 => app.world_mut().resource_mut::<MapModel>().mini_map_index = None,
            6 => app.world_mut().resource_mut::<MapModel>().mini_map_index = Some(0),
            7 => app.world_mut().resource_mut::<MapModel>().map_width = None,
            _ => app.world_mut().resource_mut::<MapModel>().map_height = None,
        }
        app.update();
        assert!(receiver.try_recv().is_err());
        let movement = app.world().resource::<WorldPointerMovementState>();
        assert!(movement.map_auto_path.is_none() && movement.active.is_none());
        assert!(movement.attack_target.is_none());
        assert!(app
            .world()
            .resource::<ChatModel>()
            .recent_text(1)
            .iter()
            .any(|line| line.contains("小地图")));
    }
}

#[test]
fn minimap_click_respects_modals_dead_player_focus_and_overlapping_inventory() {
    for case in 0..6 {
        let (mut app, receiver, _) = minimap_app(MouseButton::Right, (1024.0, 768.0));
        match case {
            0 => app.world_mut().resource_mut::<NpcDialogModel>().is_open = true,
            1 => {
                let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
                ui.core.panel = mir2_ui_core::state::UiPanel::Character;
            }
            2 => {
                let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
                ui.core.panel = mir2_ui_core::state::UiPanel::Inventory;
                ui.inventory_window.left = 710.0;
                ui.inventory_window.top = 0.0;
            }
            3 => {
                let mut model = app.world_mut().resource_mut::<UiReadModel>();
                model.player.max_hp = 30;
                model.player.hp = 0;
            }
            4 => {
                app.world_mut()
                    .query::<&mut Window>()
                    .single_mut(app.world_mut())
                    .unwrap()
                    .focused = false
            }
            _ => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .skill_assign
                    .open = true;
            }
        }
        app.update();
        assert!(receiver.try_recv().is_err(), "case {case}");
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .map_auto_path
            .is_none());
    }
}

#[test]
fn minimap_can_replace_a_route_while_big_map_is_open_without_clicking_world() {
    let (mut app, receiver, target) = minimap_app(MouseButton::Right, (1024.0, 768.0));
    app.update();
    let _ = receiver.try_recv();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::BigMap;
    // A fresh click while the first move awaits its ACK replaces the route,
    // but cannot create an extra world movement or disregard the ACK window.
    app.update();
    assert!(receiver.try_recv().is_err());
    let movement = app.world().resource::<WorldPointerMovementState>();
    assert_eq!(movement.auto_path_destination, Some(target));
    assert!(movement.map_auto_path.is_some());
    assert!(movement.active.is_none());
    assert_eq!(movement.pending.len(), 1);
}

#[test]
fn minimap_occupied_destination_gives_feedback_without_world_click_fallback() {
    let (mut app, receiver, target) = minimap_app(MouseButton::Left, (1024.0, 768.0));
    let mut monster = app.world().resource::<EntityModelSet>().entities[0].clone();
    monster.object_id = "987654".to_owned();
    monster.kind = EntityKind::Monster;
    monster.x = target.0;
    monster.y = target.1;
    app.world_mut()
        .resource_mut::<EntityModelSet>()
        .entities
        .push(monster);
    app.update();
    assert!(receiver.try_recv().is_err());
    let movement = app.world().resource::<WorldPointerMovementState>();
    assert!(movement.map_auto_path.is_none() && movement.active.is_none());
    assert!(app
        .world()
        .resource::<ChatModel>()
        .recent_text(1)
        .iter()
        .any(|line| line.contains("占用")));
}

#[test]
fn minimap_frame_and_collapsed_buttons_consume_press_at_displayed_positions() {
    for (expanded, points) in [
        (true, vec![(960.0, 5.0), (1000.0, 140.0), (900.0, 75.0)]),
        (false, vec![(910.0, 30.0), (933.0, 30.0), (1000.0, 30.0)]),
    ] {
        for (x, y) in points {
            let (mut app, receiver, _) = minimap_app(MouseButton::Right, (1024.0, 768.0));
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .core
                .minimap_visible = expanded;
            app.world_mut()
                .query::<&mut Window>()
                .single_mut(app.world_mut())
                .unwrap()
                .set_cursor_position(Some(bevy::prelude::Vec2::new(x, y)));
            app.update();
            assert!(receiver.try_recv().is_err());
            let movement = app.world().resource::<WorldPointerMovementState>();
            assert!(movement.map_auto_path.is_none() && movement.active.is_none());
        }
    }
    let window = stage_window(bevy::prelude::Vec2::new(933.0, 30.0));
    assert!(cursor_over_native_hud_button(&window, false));
    assert!(cursor_over_big_map_hud_button(&window, false));
    assert!(!cursor_over_big_map_hud_button(&window, true));

    for (x, y) in [(900.0, 75.0), (960.0, 140.0)] {
        let (mut app, receiver, _) = minimap_app(MouseButton::Left, (1024.0, 768.0));
        app.world_mut().resource_mut::<MapModel>().mini_map_index = None;
        app.world_mut()
            .query::<&mut Window>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(bevy::prelude::Vec2::new(x, y)));
        app.update();
        assert!(receiver.try_recv().is_err());
        let movement = app.world().resource::<WorldPointerMovementState>();
        assert!(movement.map_auto_path.is_none() && movement.active.is_none());
    }
}

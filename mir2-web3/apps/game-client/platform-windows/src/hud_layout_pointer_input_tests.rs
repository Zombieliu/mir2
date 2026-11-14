// Pointer-only regressions for the selectable HUD. Reuses the existing native
// input fixture/channel; no renderer, fixture sender, gateway or authority added.
use super::*;
use bevy::prelude::Vec2;

fn layout_window(point: (f32, f32), width: f32, height: f32) -> Window {
    let mut window = stage_window(Vec2::ZERO);
    window.resolution.set(width, height);
    let transform = mir2_client_bevy::crystal_ui::CrystalStageTransform::fit_native(
        window.resolution.width(),
        window.resolution.height(),
    );
    let (x, y) = transform.logical_to_physical(point.0, point.1);
    window.set_cursor_position(Some(Vec2::new(x, y)));
    window
}

fn enable_skill_bars(ui: &mut NativePlayerUiState) {
    ui.core.options.skill_bar = true;
    ui.skill_bars.visible[0] = true;
    ui.skill_bars.visible[1] = true;
    ui.core.options.skill_bar_positions = [[20, 60], [270, 100]];
}

fn reference_positions() -> ReferenceHudPositions {
    ReferenceHudPositions {
        skill_bars: [[420, 400], [650, 440]],
        ..Default::default()
    }
}

#[test]
fn reference_seven_entries_capture_their_resolved_rects_and_keep_nonmodal_policy() {
    let layout = HudLayoutSpec::reference().resolve();
    for (width, height) in [(1024., 768.), (1536., 1152.), (2048., 1536.), (1280., 720.)] {
        for (index, rect) in layout.actions.into_iter().enumerate() {
            let window = layout_window(rect.center(), width, height);
            assert!(
                cursor_over_native_hud_button_with_layout(&window, true, layout),
                "resolved entry {index} leaked at {width}x{height}"
            );
            assert_eq!(
                cursor_over_nonmodal_view_hud_button_with_layout(&window, true, layout),
                index < 6,
                "shop must keep its existing automatic-action cancellation policy"
            );
        }
    }
}

#[test]
fn original_wrappers_keep_original_buttons_and_vacated_reference_areas_have_no_old_hitbox() {
    let original = HudLayoutSpec::default().resolve();
    let reference = HudLayoutSpec::reference().resolve();
    for (index, rect) in original.actions.into_iter().enumerate() {
        let window = layout_window(rect.center(), 1024., 768.);
        assert!(cursor_over_native_hud_button(&window, true));
        assert_eq!(
            cursor_over_nonmodal_view_hud_button(&window, true),
            index < 6
        );
    }
    let vacated = original.actions[..6]
        .iter()
        .map(|rect| rect.center())
        .filter(|(x, y)| !reference.actions.iter().any(|rect| rect.contains(*x, *y)))
        .collect::<Vec<_>>();
    assert!(
        !vacated.is_empty(),
        "fixture must include a moved legacy entry"
    );
    for point in vacated {
        let window = layout_window(point, 1024., 768.);
        assert!(cursor_over_native_hud_button(&window, true));
        assert!(!cursor_over_native_hud_button_with_layout(
            &window, true, reference
        ));
        assert!(!cursor_over_nonmodal_view_hud_button_with_layout(
            &window, true, reference
        ));
    }
    assert!(!cursor_over_native_hud_button_with_layout(
        &layout_window((512., 400.), 1024., 768.),
        true,
        reference
    ));
}

#[test]
fn original_press_origin_wrapper_keeps_world_and_button_ownership() {
    let mut world = bevy::prelude::World::new();
    let mut queries = bevy::ecs::system::SystemState::<NativeQuestPointerUi>::new(&mut world);
    let quest_ui = queries
        .get(&world)
        .expect("valid empty pointer fixture queries");
    let ui = NativePlayerUiState::default();
    let window = layout_window((512., 400.), 1024., 768.);
    let menu = spec::hud::MENU.rect.center();
    for (point, is_world) in [
        (Some(Vec2::new(512., 400.)), true),
        (Some(Vec2::new(menu.0, menu.1)), false),
        (None, false),
    ] {
        assert_eq!(
            press_origin_is_world(&window, point, Some(&ui), &quest_ui, true, true),
            is_world
        );
    }
}

#[test]
fn reference_skill_bars_use_separate_positions_and_exact_resolved_frames() {
    let mut ui = NativePlayerUiState::default();
    enable_skill_bars(&mut ui);
    let positions = reference_positions();
    let reference = HudLayoutSpec::reference().resolve();
    let original = HudLayoutSpec::default().resolve();
    let saved_legacy = ui.core.options.skill_bar_positions;
    for (bar, position) in positions.skill_bars.into_iter().enumerate() {
        let frame = reference.skills.frame(position);
        for point in [
            frame.center(),
            (frame.left + 0.25, frame.top + 0.25),
            (
                frame.left + frame.width - 0.25,
                frame.top + frame.height - 0.25,
            ),
        ] {
            let window = layout_window(point, 1280., 720.);
            assert!(cursor_over_native_skill_bar_with_layout(
                &window, &ui, reference, &positions
            ));
            assert!(!cursor_over_native_skill_bar_with_layout(
                &window, &ui, original, &positions
            ));
        }
        for point in [
            (frame.left + frame.width, frame.top + 2.),
            (frame.left + 2., frame.top + frame.height),
        ] {
            assert!(!cursor_over_native_skill_bar_with_layout(
                &layout_window(point, 1280., 720.),
                &ui,
                reference,
                &positions
            ));
        }
        ui.skill_bars.visible[bar] = false;
        assert!(!cursor_over_native_skill_bar_with_layout(
            &layout_window(frame.center(), 1024., 768.),
            &ui,
            reference,
            &positions
        ));
        ui.skill_bars.visible[bar] = true;
    }
    for position in saved_legacy {
        let window = layout_window(original.skills.frame(position).center(), 1024., 768.);
        assert!(cursor_over_native_skill_bar(&window, &ui));
        assert!(!cursor_over_native_skill_bar_with_layout(
            &window, &ui, reference, &positions
        ));
    }
    ui.core.options.skill_bar = false;
    assert!(!cursor_over_native_skill_bar_with_layout(
        &layout_window(
            reference.skills.frame(positions.skill_bars[0]).center(),
            1024.,
            768.
        ),
        &ui,
        reference,
        &positions
    ));
    assert_eq!(ui.core.options.skill_bar_positions, saved_legacy);
}

#[test]
fn hud_pointer_geometry_rejects_letterbox_and_unfocused_cursor() {
    let layout = HudLayoutSpec::reference().resolve();
    let mut ui = NativePlayerUiState::default();
    enable_skill_bars(&mut ui);
    // Even stale/invalid placement spanning outside the stage cannot capture
    // a point in the letterbox. The stage boundary is independent of bar rects.
    let positions = ReferenceHudPositions {
        skill_bars: [[-40, 100], [0, 0]],
        ..Default::default()
    };
    let window = layout_window((-10., 110.), 1280., 720.);
    assert!(layout
        .skills
        .frame(positions.skill_bars[0])
        .contains(-10., 110.));
    assert!(!cursor_over_native_skill_bar_with_layout(
        &window, &ui, layout, &positions
    ));
    assert!(!cursor_over_native_hud_button_with_layout(
        &window, true, layout
    ));
    assert!(!cursor_over_nonmodal_view_hud_button_with_layout(
        &window, true, layout
    ));
    let mut unfocused = layout_window(layout.actions[0].center(), 1024., 768.);
    unfocused.focused = false;
    assert!(!cursor_over_native_hud_button_with_layout(
        &unfocused, true, layout
    ));
    assert!(!cursor_over_nonmodal_view_hud_button_with_layout(
        &unfocused, true, layout
    ));
    let positions = reference_positions();
    let mut unfocused = layout_window(
        layout.skills.frame(positions.skill_bars[0]).center(),
        1024.,
        768.,
    );
    unfocused.focused = false;
    assert!(!cursor_over_native_skill_bar_with_layout(
        &unfocused, &ui, layout, &positions
    ));
}

#[test]
fn reference_hud_press_then_world_release_never_sends_move_or_arms_hold() {
    let layout = HudLayoutSpec::reference().resolve();
    let positions = reference_positions();
    let points = layout.actions.into_iter().map(|rect| rect.center()).chain(
        positions
            .skill_bars
            .into_iter()
            .map(|p| layout.skills.frame(p).center()),
    );
    for point in points {
        for button in [MouseButton::Left, MouseButton::Right] {
            for new_move in [false, true] {
                let (mut app, receiver) = ordered_pointer_fixture();
                app.insert_resource(HudLayoutSpec::reference());
                app.insert_resource(positions.clone());
                {
                    let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
                    enable_skill_bars(&mut ui);
                    ui.core.options.new_move = new_move;
                }
                ordered_pointer_motion(&mut app, Vec2::new(point.0, point.1));
                ordered_pointer_button(&mut app, button, true);
                ordered_pointer_motion(&mut app, Vec2::new(640., 550.));
                ordered_pointer_button(&mut app, button, false);
                app.update();
                assert!(
                    receiver.try_recv().is_err(),
                    "{point:?} {button:?} new_move={new_move} leaked"
                );
                assert!(drain_world_intents(&mut app).is_empty());
                let movement = app.world().resource::<WorldPointerMovementState>();
                assert_eq!(movement.active, None);
                assert_eq!(movement.auto_path_destination, None);
                assert!(movement.pending.is_empty());
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .clear();
                app.update();
                assert!(
                    receiver.try_recv().is_err(),
                    "UI release replayed a world move"
                );
                // Positive control: the next genuine down at that world point
                // is still accepted; the guard did not disable world input.
                ordered_pointer_button(&mut app, button, true);
                app.update();
                assert!(matches!(
                    receiver.try_recv(),
                    Ok(GatewayCommand::Player(PlayerIntent::Walk { .. }))
                ));
            }
        }
    }
}

#[test]
fn reference_hud_hold_cannot_become_a_world_gesture_on_the_next_frame() {
    let layout = HudLayoutSpec::reference().resolve();
    let positions = reference_positions();
    let points = layout.actions.into_iter().map(|rect| rect.center()).chain(
        positions
            .skill_bars
            .into_iter()
            .map(|p| layout.skills.frame(p).center()),
    );
    for point in points {
        for button in [MouseButton::Left, MouseButton::Right] {
            let (mut app, receiver) = ordered_pointer_fixture();
            app.insert_resource(HudLayoutSpec::reference());
            app.insert_resource(positions.clone());
            enable_skill_bars(&mut app.world_mut().resource_mut::<NativePlayerUiState>());
            ordered_pointer_motion(&mut app, Vec2::new(point.0, point.1));
            ordered_pointer_button(&mut app, button, true);
            ordered_pointer_motion(&mut app, Vec2::new(640., 550.));
            app.update();
            assert!(receiver.try_recv().is_err());
            assert!(drain_world_intents(&mut app).is_empty());
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            advance_movement_clock(&mut app, 600);
            app.update();
            assert!(
                receiver.try_recv().is_err(),
                "UI hold started world movement"
            );
            assert!(drain_world_intents(&mut app).is_empty());
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            ordered_pointer_button(&mut app, button, false);
            app.update();
            assert!(
                receiver.try_recv().is_err(),
                "UI release started world movement"
            );
            assert!(drain_world_intents(&mut app).is_empty());
            let movement = app.world().resource::<WorldPointerMovementState>();
            assert_eq!(movement.active, None);
            assert_eq!(movement.auto_path_destination, None);
            assert!(movement.pending.is_empty());
        }
    }
}

#[test]
fn reference_first_frame_button_press_does_not_wait_for_bevy_interaction() {
    let layout = HudLayoutSpec::reference().resolve();
    for rect in layout.actions {
        let (mut app, receiver) =
            quest_pointer_test_app(Vec2::new(rect.center().0, rect.center().1));
        app.insert_resource(HudLayoutSpec::reference());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(receiver.try_recv().is_err());
        assert!(drain_world_intents(&mut app).is_empty());
        let movement = app.world().resource::<WorldPointerMovementState>();
        assert_eq!(movement.active, None);
        assert!(movement.pending.is_empty());
    }
}

#[test]
fn layout_switch_revokes_pointer_hold_but_preserves_committed_route_and_attack_target() {
    let (mut app, receiver) = ordered_pointer_fixture();
    app.insert_resource(HudLayoutSpec::reference());
    app.insert_resource(HudLayoutInputGuard {
        switched_this_frame: true,
    });
    {
        let mut movement = app.world_mut().resource_mut::<WorldPointerMovementState>();
        movement.active = Some(WorldPointerMovementMode::Walk);
        movement.auto_path_destination = Some((12, 10));
        movement.pointer_auto_path = true;
        movement.attack_target = Some(2001);
        movement.harvest_target = Some(3001);
        movement.harvest_direction = Some("right");
    }
    ordered_pointer_button(&mut app, MouseButton::Left, true);
    app.update();
    assert!(receiver.try_recv().is_err());
    let movement = app.world().resource::<WorldPointerMovementState>();
    assert_eq!(movement.active, None);
    assert_eq!(movement.harvest_target, None);
    assert_eq!(movement.harvest_direction, None);
    assert_eq!(movement.auto_path_destination, Some((12, 10)));
    assert!(movement.pointer_auto_path);
    assert_eq!(movement.attack_target, Some(2001));
}

#[test]
fn reference_layout_switch_does_not_capture_keyboard_movement() {
    let (mut app, receiver) = input_app();
    let point = HudLayoutSpec::reference().resolve().actions[0].center();
    app.world_mut().spawn(layout_window(point, 1024., 768.));
    app.insert_resource(HudLayoutSpec::reference());
    app.insert_resource(HudLayoutInputGuard {
        switched_this_frame: true,
    });
    app.insert_resource(movement_entities());
    app.add_systems(bevy::prelude::Update, keyboard_movement_system);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyD);
    app.update();
    assert!(
        matches!(receiver.try_recv(), Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "right")
    );
}

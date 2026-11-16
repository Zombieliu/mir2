use super::*;

fn player_click_app(
    class: &str,
    distance: i32,
    shift: bool,
) -> (
    bevy::prelude::App,
    std::sync::mpsc::Receiver<GatewayCommand>,
) {
    let (mut app, receiver) = input_app();
    install_movement_clock_and_inbox(&mut app);
    app.world_mut().spawn(Window::default());
    app.insert_resource(ButtonInput::<MouseButton>::default());
    let mut entities = world_entities();
    entities.entities[2].object_id = "3001".to_owned();
    entities.entities[2].kind = EntityKind::Player;
    entities.entities[2].name = "Other Player".to_owned();
    entities.entities[2].x = 10 + distance;
    entities.entities[2].y = 10;
    app.insert_resource(entities);
    app.insert_resource(UiReadModel {
        player: mir2_client_bevy::read_model::PlayerStats {
            hp: 10,
            max_hp: 10,
            ..Default::default()
        },
    });
    let mut state = NativeWorldClickState {
        class: Some(class.to_owned()),
        has_class_weapon: Some(true),
        riding_mount: Some(false),
        dazed: Some(false),
        fishing: Some(false),
        ..Default::default()
    };
    state.targets.insert(
        3001,
        crate::gameplay_bridge::CrystalWorldClickTarget {
            kind: EntityKind::Player,
            object_id: 3001,
            x: 10 + distance,
            y: 10,
            dead: Some(false),
            ai: None,
            harvestable: None,
        },
    );
    app.insert_resource(state);
    app.init_resource::<QuestUiIntentQueue>();
    app.world_mut()
        .resource_mut::<NativeEntityPresentation>()
        .set_hovered_object_id_for_test(Some("3001"));
    if shift {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
    }
    app.add_systems(
        bevy::prelude::Update,
        (
            mouse_world_interaction_system,
            crate::gameplay_bridge::forward_quest_ui_intents,
        )
            .chain(),
    );
    (app, receiver)
}

#[test]
fn native_player_click_requires_shift_and_uses_shared_melee_or_range_resolution() {
    for (class, distance) in [("Warrior", 1), ("Archer", 9)] {
        let (mut app, receiver) = player_click_app(class, distance, false);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "plain player clicks must not attack or walk through them"
        );
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .attack_target,
            None
        );

        let (mut app, receiver) = player_click_app(class, distance, true);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let command = receiver
            .try_recv()
            .expect("Shift player click must emit a combat intent");
        if class == "Archer" {
            assert!(matches!(
                command,
                GatewayCommand::Wire(NativeOutboundCommand::RangeAttack {
                    target_id: 3001,
                    target_x: 19,
                    target_y: 10,
                    ..
                })
            ));
        } else {
            assert!(
                matches!(command, GatewayCommand::Wire(NativeOutboundCommand::AttackDirection {
                direction, spell: None,
            }) if direction == "right")
            );
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ShiftLeft);
        advance_movement_clock(&mut app, 1500);
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "Shift release must not emit another attack"
        );
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .attack_target,
            None
        );
    }
}

#[test]
fn native_player_pursuit_stops_on_shift_release_without_revoking_an_accepted_move() {
    let (mut app, receiver) = player_click_app("Warrior", 5, true);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(matches!(receiver.try_recv(), Ok(GatewayCommand::Player(_))));
    let pending = app
        .world()
        .resource::<WorldPointerMovementState>()
        .pending
        .len();
    assert!(pending > 0);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_pressed(MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ShiftLeft);
    advance_movement_clock(&mut app, 600);
    app.update();
    let movement = app.world().resource::<WorldPointerMovementState>();
    assert_eq!(movement.attack_target, None);
    assert_eq!(movement.auto_path_destination, None);
    assert_eq!(
        movement.pending.len(),
        pending,
        "sent movement still awaits its authoritative receipt"
    );
    assert!(receiver.try_recv().is_err());
}

#[test]
fn native_player_intent_cannot_bypass_shift_with_a_queued_ui_action_or_alt() {
    for (shift, alt) in [(false, false), (false, true), (true, true)] {
        let (mut app, receiver) = player_click_app("Warrior", 1, shift);
        if alt {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::AltLeft);
        }
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AttackTarget { object_id: 3001 });
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "invalid modifiers cannot promote a queued player target"
        );
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .attack_target,
            None
        );
    }
}

#[test]
fn native_player_click_keeps_modal_and_dead_target_gates() {
    for modal in [false, true] {
        let (mut app, receiver) = player_click_app("Warrior", 1, true);
        if modal {
            app.insert_resource(NpcDialogModel {
                is_open: true,
                ..Default::default()
            });
        } else {
            app.world_mut()
                .resource_mut::<NativeWorldClickState>()
                .targets
                .get_mut(&3001)
                .unwrap()
                .dead = Some(true);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(receiver.try_recv().is_err());
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .attack_target,
            None
        );
    }
}

#[test]
fn native_player_pursuit_stops_when_focus_is_lost_or_escape_cancels() {
    for blur in [false, true] {
        let (mut app, receiver) = player_click_app("Warrior", 1, true);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(receiver.try_recv().is_ok());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        if blur {
            let world = app.world_mut();
            let mut windows = world.query::<&mut Window>();
            windows.single_mut(world).unwrap().focused = false;
        } else {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
        }
        advance_movement_clock(&mut app, 1500);
        app.update();
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .attack_target,
            None
        );
        assert!(receiver.try_recv().is_err());
    }
}

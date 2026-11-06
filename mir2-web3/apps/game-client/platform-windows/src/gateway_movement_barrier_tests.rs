use super::*;

fn drain(commands: Vec<GatewayCommand>, limit: usize) -> Vec<GatewayCommand> {
    let (sender, mut receiver) = std::sync::mpsc::channel();
    for command in commands {
        sender.send(command).expect("enqueue input");
    }
    drain_command_batch(&mut receiver, limit)
}

fn combat_commands() -> Vec<NativeOutboundCommand> {
    vec![
        NativeOutboundCommand::Attack { object_id: 2001 },
        NativeOutboundCommand::AttackDirection {
            direction: "right".into(),
            spell: None,
        },
        NativeOutboundCommand::RangeAttack {
            direction: "right".into(),
            x: 10,
            y: 10,
            target_id: 2001,
            target_x: 11,
            target_y: 10,
        },
        NativeOutboundCommand::Magic {
            object_id: 1000,
            spell: "FireBall".into(),
            direction: "right".into(),
            target_id: 2001,
            x: 11,
            y: 10,
            spell_target_lock: true,
        },
    ]
}

#[test]
fn combat_barrier_discards_buffered_walk_and_run_before_every_physical_action() {
    for combat in combat_commands() {
        for prior in [
            GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".into(),
            }),
            GatewayCommand::Player(PlayerIntent::Run {
                direction: "up".into(),
            }),
            GatewayCommand::Wire(NativeOutboundCommand::Walk {
                direction: "up".into(),
            }),
            GatewayCommand::Wire(NativeOutboundCommand::Run {
                direction: "up".into(),
            }),
        ] {
            let expected = combat.to_wire_json();
            let batch = drain(vec![prior, GatewayCommand::Wire(combat.clone())], 8);
            assert_eq!(batch.len(), 1);
            assert!(matches!(&batch[0], GatewayCommand::Wire(command)
                if command.to_wire_json() == expected));
        }
    }
}

#[test]
fn combat_barrier_preserves_new_walk_after_attack_and_its_direction() {
    let batch = drain(
        vec![
            GatewayCommand::Player(PlayerIntent::Run {
                direction: "up".into(),
            }),
            GatewayCommand::Wire(NativeOutboundCommand::Attack { object_id: 2001 }),
            GatewayCommand::Player(PlayerIntent::Walk {
                direction: "down".into(),
            }),
        ],
        8,
    );
    assert_eq!(batch.len(), 2);
    assert!(matches!(
        batch[0],
        GatewayCommand::Wire(NativeOutboundCommand::Attack { object_id: 2001 })
    ));
    assert!(
        matches!(&batch[1], GatewayCommand::Player(PlayerIntent::Walk { direction }) if direction == "down")
    );
}

#[test]
fn combat_barrier_preserves_prior_turn_order_and_new_movement_after_cast() {
    let batch = drain(
        vec![
            GatewayCommand::Player(PlayerIntent::Run {
                direction: "up".into(),
            }),
            GatewayCommand::Player(PlayerIntent::Turn {
                direction: "right".into(),
            }),
            GatewayCommand::Wire(combat_commands().pop().unwrap()),
            GatewayCommand::Player(PlayerIntent::Walk {
                direction: "left".into(),
            }),
        ],
        8,
    );
    assert_eq!(batch.len(), 3);
    assert!(
        matches!(&batch[0], GatewayCommand::Player(PlayerIntent::Turn { direction }) if direction == "right")
    );
    assert!(matches!(
        batch[1],
        GatewayCommand::Wire(NativeOutboundCommand::Magic { .. })
    ));
    assert!(
        matches!(&batch[2], GatewayCommand::Player(PlayerIntent::Walk { direction }) if direction == "left")
    );
}

#[test]
fn explicit_turn_supersedes_unsent_step_without_replaying_run_after_turn() {
    for turn in [
        GatewayCommand::Player(PlayerIntent::Turn {
            direction: "right".into(),
        }),
        GatewayCommand::Wire(NativeOutboundCommand::Turn {
            direction: "right".into(),
        }),
    ] {
        let batch = drain(
            vec![
                GatewayCommand::Player(PlayerIntent::Run {
                    direction: "up".into(),
                }),
                turn,
            ],
            8,
        );
        assert_eq!(batch.len(), 1);
        assert!(matches!(&batch[0],
            GatewayCommand::Player(PlayerIntent::Turn { direction })
                | GatewayCommand::Wire(NativeOutboundCommand::Turn { direction })
                if direction == "right"));
    }
}

#[test]
fn spell_toggle_and_key_binding_do_not_discard_buffered_movement() {
    for command in [
        NativeOutboundCommand::SpellToggle {
            spell: "FlamingSword".into(),
            toggle_state: 1,
        },
        NativeOutboundCommand::MagicKey {
            request_id: 1,
            spell: "FireBall".into(),
            key: 1,
            old_key: 0,
        },
    ] {
        let batch = drain(
            vec![
                GatewayCommand::Player(PlayerIntent::Run {
                    direction: "up".into(),
                }),
                GatewayCommand::Wire(command),
            ],
            8,
        );
        assert!(batch.iter().any(|command|
            matches!(command, GatewayCommand::Player(PlayerIntent::Run { direction }) if direction == "up")));
        assert_eq!(batch.len(), 2);
    }
}

#[test]
fn combat_barrier_keeps_small_batches_bounded_without_reversing_turn_and_attack() {
    for limit in 2..=8 {
        let batch = drain(
            vec![
                GatewayCommand::Player(PlayerIntent::Turn {
                    direction: "right".into(),
                }),
                GatewayCommand::Wire(NativeOutboundCommand::Attack { object_id: 2001 }),
                GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "down".into(),
                }),
            ],
            limit,
        );
        assert!(batch.len() <= limit);
        assert!(matches!(
            batch[0],
            GatewayCommand::Player(PlayerIntent::Turn { .. })
        ));
        assert!(matches!(
            batch[1],
            GatewayCommand::Wire(NativeOutboundCommand::Attack { .. })
        ));
    }
}

#[test]
fn authoritative_combat_correction_releases_sender_dropped_step_through_input_packet_gate() {
    use crate::entity_presentation::NativeEntityPresentation;
    use crate::gameplay_bridge::GameplayEventInbox;
    use crate::input::{
        mouse_world_interaction_system, GatewayCommands, WorldPointerMovementState,
    };
    use bevy::prelude::{App, ButtonInput, KeyCode, MouseButton, Time, Update, Window};
    use mir2_client_bevy::entities::{EntityKind, EntityModel, EntityModelSet};
    use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};

    // This creates an ECS Window component only; no window/plugin/game starts.
    let (sender, mut receiver) = std::sync::mpsc::channel();
    let (_, events) = std::sync::mpsc::channel();
    let mut app = App::new();
    app.insert_resource(ButtonInput::<KeyCode>::default());
    app.insert_resource(ButtonInput::<MouseButton>::default());
    app.insert_resource(GatewayCommands::new(sender.clone()));
    app.insert_resource(GameplayEventInbox::new(events));
    app.init_resource::<WorldPointerMovementState>();
    app.insert_resource(Time::<()>::default());
    app.insert_resource(NativeShellModel {
        screen: NativeShellScreen::InGame,
        ..Default::default()
    });
    app.insert_resource(EntityModelSet {
        entities: vec![EntityModel {
            object_id: "1000".into(),
            kind: EntityKind::SelfPlayer,
            name: "Self".into(),
            x: 0,
            y: 0,
            level: Some(7),
            direction: Some("right".into()),
        }],
    });
    let mut presentation = NativeEntityPresentation::default();
    presentation.set_hover_grid_context_for_test((10, 0), (576.0, 352.0));
    app.insert_resource(presentation);
    app.world_mut().spawn(Window::default());
    app.add_systems(Update, mouse_world_interaction_system);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.update();
    sender
        .send(GatewayCommand::Wire(NativeOutboundCommand::Attack {
            object_id: 2001,
        }))
        .unwrap();
    let batch = drain_command_batch(&mut receiver, 8);
    assert_eq!(
        batch.len(),
        1,
        "the producer-accepted step must be dropped before socket write"
    );
    assert!(matches!(
        batch[0],
        GatewayCommand::Wire(NativeOutboundCommand::Attack { .. })
    ));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_pressed(MouseButton::Right);
    app.world_mut()
        .resource_mut::<NativeEntityPresentation>()
        .set_hover_grid_context_for_test((0, 10), (480.0, 352.0));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(600));
    app.update();
    assert!(
        receiver.try_recv().is_err(),
        "the unsent step still owns the input pending slot"
    );

    let packet = PacketEvent::UserLocation(crate::native_protocol::UserLocation {
        location: mir2_client_bevy::big_map::BigMapPoint { x: 0, y: 0 },
        direction: Some("right".into()),
    });
    let foreign =
        native_self_movement_ack(Some(&packet), &json!({"playerObjectId": 1001})).unwrap();
    app.world()
        .resource::<GameplayEventInbox>()
        .push_movement_ack(foreign);
    app.update();
    assert!(
        receiver.try_recv().is_err(),
        "another actor cannot release this player's pending slot"
    );
    let own = native_self_movement_ack(Some(&packet), &json!({"playerObjectId": 1000})).unwrap();
    app.world()
        .resource::<GameplayEventInbox>()
        .push_movement_ack(own);
    app.update();
    assert!(
        receiver.try_recv().is_err(),
        "authoritative correction keeps the existing input guard"
    );
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(400));
    app.update();
    assert!(
        matches!(receiver.try_recv(),
        Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "down"),
        "fresh movement must work after correction without inventing a transform"
    );
}

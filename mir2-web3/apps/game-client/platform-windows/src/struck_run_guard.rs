use super::*;
use crate::gameplay_bridge::{NativeGameplayAdapter, NativeOwnerStruck};
use crate::native_protocol::PacketEvent;
use serde_json::json;
use std::time::{Duration, Instant};

fn struck_snapshot(generation: u64, sequence: u64, received_at: Instant) -> NativeGameplaySnapshot {
    NativeGameplaySnapshot {
        generation,
        entity_render_payload: Some(json!({"playerObjectId":1000})),
        owner_struck: Some(NativeOwnerStruck {
            generation,
            sequence,
            object_id: 1000,
            received_at,
        }),
        ..Default::default()
    }
}

#[test]
fn owner_struck_requires_strictly_after_2500ms_without_delaying_walk() {
    let receipt = Instant::now();
    let mut movement = WorldPointerMovementState::default();
    movement.observe_owner_struck_snapshot(&struck_snapshot(1, 1, receipt), 1000.0, receipt);
    // Packet drain precedes the first native entity model in the same frame.
    movement.observe_identity("1000", (10, 10), "right");
    movement.run_primed_until_ms = 10_000.0;

    assert!(
        movement.can_send(1000.0),
        "Struck must not create an ActionTime delay"
    );
    assert_eq!(movement.next_move_send_at_ms, 0.0);
    assert_eq!(movement.input_blocked_until_ms, 0.0);
    for now in [1000.0, 3499.0, 3500.0] {
        assert_eq!(
            movement.effective_mode(WorldPointerMovementMode::Run, now, Some(100)),
            WorldPointerMovementMode::Walk,
        );
    }
    assert_eq!(
        movement.effective_mode(WorldPointerMovementMode::Run, 3501.0, Some(100)),
        WorldPointerMovementMode::Run,
    );
    assert_eq!(
        movement.effective_mode(WorldPointerMovementMode::Walk, 1000.0, Some(100)),
        WorldPointerMovementMode::Walk,
    );
}

#[test]
fn repeated_snapshots_do_not_renew_but_new_owner_struck_does() {
    let receipt = Instant::now();
    let snapshot = struck_snapshot(2, 10, receipt);
    let mut movement = WorldPointerMovementState::default();
    movement.observe_owner_struck_snapshot(&snapshot, 1000.0, receipt);
    movement.observe_owner_struck_snapshot(
        &snapshot,
        2500.0,
        receipt + Duration::from_millis(1500),
    );
    assert_eq!(movement.run_allowed_after_ms, Some(3500.0));

    let fresh = struck_snapshot(2, 11, receipt + Duration::from_millis(2000));
    movement.observe_owner_struck_snapshot(&fresh, 3000.0, receipt + Duration::from_millis(2000));
    assert_eq!(movement.run_allowed_after_ms, Some(5500.0));
    movement.observe_owner_struck_snapshot(
        &snapshot,
        6000.0,
        receipt + Duration::from_millis(5000),
    );
    assert_eq!(movement.run_allowed_after_ms, Some(5500.0));
    assert_eq!(
        movement.permitted_run_mode(WorldPointerMovementMode::Run, 5500.0, Some(100)),
        WorldPointerMovementMode::Walk,
    );
    assert_eq!(
        movement.permitted_run_mode(WorldPointerMovementMode::Run, 5501.0, Some(100)),
        WorldPointerMovementMode::Run,
    );
}

#[test]
fn delayed_receipt_keeps_original_deadline_in_the_movement_clock() {
    let receipt = Instant::now();
    let mut movement = WorldPointerMovementState::default();
    movement.observe_owner_struck_snapshot(
        &struck_snapshot(3, 1, receipt),
        5000.0,
        receipt + Duration::from_millis(2000),
    );
    assert_eq!(movement.run_allowed_after_ms, Some(5500.0));
    movement.observe_owner_struck_snapshot(
        &struck_snapshot(3, 2, receipt + Duration::from_millis(100)),
        10_000.0,
        receipt + Duration::from_millis(7000),
    );
    assert_eq!(movement.run_allowed_after_ms, Some(5600.0));
    assert_eq!(
        movement.permitted_run_mode(WorldPointerMovementMode::Run, 10_000.0, Some(100)),
        WorldPointerMovementMode::Run,
        "a packet delayed longer than 2500 ms must not start a new guard",
    );
}

#[test]
fn owner_and_generation_fences_reject_stale_packets_and_reset_new_transport() {
    let receipt = Instant::now();
    let mut movement = WorldPointerMovementState::default();
    movement.observe_owner_struck_snapshot(&struck_snapshot(4, 4, receipt), 1000.0, receipt);

    let mut wrong_event_generation = struck_snapshot(4, 50, receipt);
    wrong_event_generation
        .owner_struck
        .as_mut()
        .unwrap()
        .generation = 3;
    movement.observe_owner_struck_snapshot(&wrong_event_generation, 2000.0, receipt);
    assert_eq!(movement.last_owner_struck_sequence, 4);
    let mut foreign_owner = struck_snapshot(4, 5, receipt);
    foreign_owner.owner_struck.as_mut().unwrap().object_id = 2000;
    movement.observe_owner_struck_snapshot(&foreign_owner, 2000.0, receipt);
    assert_eq!(movement.run_allowed_after_ms, Some(3500.0));
    movement.observe_owner_struck_snapshot(&struck_snapshot(3, 100, receipt), 6000.0, receipt);
    assert_eq!(movement.owner_struck_generation, Some(4));
    assert_eq!(movement.run_allowed_after_ms, Some(3500.0));

    movement.active = Some(WorldPointerMovementMode::Run);
    movement.pending.push_back(pending_test_move(
        (10, 10),
        (12, 10),
        WorldPointerMovementMode::Run,
        1000.0,
    ));
    movement.observe_owner_struck_snapshot(
        &NativeGameplaySnapshot {
            generation: 5,
            big_map_only: true,
            ..Default::default()
        },
        6000.0,
        receipt,
    );
    assert_eq!(movement.run_allowed_after_ms, None);
    assert_eq!(movement.last_owner_struck_sequence, 0);
    assert_eq!(movement.active, None);
    assert!(movement.pending.is_empty());
    movement.observe_owner_struck_snapshot(&struck_snapshot(4, 100, receipt), 6000.0, receipt);
    assert_eq!(movement.run_allowed_after_ms, None);
    movement.observe_owner_struck_snapshot(&struck_snapshot(5, 1, receipt), 6000.0, receipt);
    assert_eq!(movement.run_allowed_after_ms, Some(8500.0));
}

#[test]
fn owner_change_and_local_reset_cannot_replay_an_old_receipt() {
    let receipt = Instant::now();
    let mut movement = WorldPointerMovementState::default();
    movement.observe_owner_struck_snapshot(&struck_snapshot(6, 1, receipt), 1000.0, receipt);
    assert!(movement.observe_identity("2000", (20, 20), "left"));
    assert_eq!(movement.run_allowed_after_ms, None);
    let mut other_character = struck_snapshot(6, 1, receipt);
    other_character.entity_render_payload = Some(json!({"playerObjectId":2000}));
    movement.observe_owner_struck_snapshot(&other_character, 2000.0, receipt);
    assert_eq!(movement.run_allowed_after_ms, None);

    let fresh = struck_snapshot(6, 2, receipt);
    movement.observe_owner_struck_snapshot(&fresh, 3000.0, receipt);
    assert_eq!(movement.run_allowed_after_ms, Some(5500.0));
    movement.reset_controller(3500.0, "notInGame");
    movement.observe_owner_struck_snapshot(&fresh, 5000.0, receipt);
    assert_eq!(movement.run_allowed_after_ms, None);

    let future = struck_snapshot(6, 3, receipt + Duration::from_millis(1));
    movement.observe_owner_struck_snapshot(&future, 5000.0, receipt);
    assert_eq!(movement.last_owner_struck_sequence, 2);
    assert_eq!(movement.run_allowed_after_ms, None);
}

#[test]
fn adapter_records_only_owner_struck_and_keeps_each_packet_receipt() {
    let mut adapter = NativeGameplayAdapter::default();
    adapter.set_generation(7);
    let payload = json!({"playerObjectId":1000,"entities":[{"objectId":1000,"kind":"selfPlayer","x":10,"y":10}]});
    adapter.observe_world_snapshot(&payload);
    for packet in ["ObjectStruck", "DamageIndicator"] {
        adapter.observe_packet(&PacketEvent::Other {
            packet: packet.to_owned(),
            payload: json!({"objectId":1000,"attackerId":2000,"damage":4}),
        });
        assert!(adapter.snapshot(&payload).owner_struck.is_none());
    }
    let struck = PacketEvent::Other {
        packet: "Struck".to_owned(),
        payload: json!({"attackerId":2000}),
    };
    adapter.observe_packet(&struck);
    let first = adapter.snapshot(&payload).owner_struck.unwrap();
    let repeated = adapter.snapshot(&payload).owner_struck.unwrap();
    assert_eq!(first.generation, 7);
    assert_eq!(first.object_id, 1000);
    assert_eq!(first.sequence, repeated.sequence);
    assert_eq!(first.received_at, repeated.received_at);
    // A second authoritative hit refreshes NextRunTime while a flinch is queued.
    adapter.observe_packet(&struck);
    assert!(adapter.snapshot(&payload).owner_struck.unwrap().sequence > first.sequence);
    adapter.set_generation(8);
    assert!(adapter.snapshot(&payload).owner_struck.is_none());
}

#[derive(Clone, Copy, Debug)]
enum RunInput {
    RightHold,
    NewMovePath,
    Keyboard,
    AutoRun,
}

fn guard_input_app(
    input: RunInput,
    hp: i32,
) -> (
    bevy::prelude::App,
    std::sync::mpsc::Receiver<GatewayCommand>,
) {
    let (mut app, receiver) = input_app();
    install_movement_clock_and_inbox(&mut app);
    app.world_mut().spawn(Window::default());
    app.init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<NpcDialogModel>()
        .init_resource::<QuestUiIntentQueue>();
    let mut ui = NativePlayerUiState::default();
    ui.core.options.new_move = matches!(input, RunInput::NewMovePath);
    ui.local_keys.auto_run = matches!(input, RunInput::AutoRun);
    app.insert_resource(ui);
    let mut model = UiReadModel::default();
    model.player.hp = hp;
    model.player.max_hp = 100;
    app.insert_resource(model);
    app.insert_resource(movement_entities());
    app.world_mut()
        .resource_mut::<NativeEntityPresentation>()
        .set_hover_grid_context_for_test((10, 10), (960.0, 352.0));
    app.world_mut()
        .resource_mut::<WorldPointerMovementState>()
        .observe_identity("1000", (10, 10), "right");
    app.add_systems(
        bevy::prelude::Update,
        (mouse_world_interaction_system, keyboard_movement_system).chain(),
    );
    advance_movement_clock(&mut app, 1000);
    (app, receiver)
}

fn press_run(app: &mut bevy::prelude::App, input: RunInput) {
    match input {
        RunInput::RightHold | RunInput::NewMovePath => {
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Right);
        }
        RunInput::Keyboard => {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ArrowRight);
            keys.press(KeyCode::ShiftLeft);
        }
        RunInput::AutoRun => {}
    }
}

fn release_run(app: &mut bevy::prelude::App, input: RunInput) {
    match input {
        RunInput::RightHold | RunInput::NewMovePath => {
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            mouse.release(MouseButton::Right);
            mouse.clear_just_pressed(MouseButton::Right);
        }
        RunInput::Keyboard => {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::ArrowRight);
            keys.release(KeyCode::ShiftLeft);
            keys.clear_just_pressed(KeyCode::ArrowRight);
            keys.clear_just_pressed(KeyCode::ShiftLeft);
        }
        RunInput::AutoRun => {
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .local_keys
                .auto_run = false;
        }
    }
}

fn acknowledge_step(app: &mut bevy::prelude::App) {
    let pending = app
        .world()
        .resource::<WorldPointerMovementState>()
        .pending
        .back()
        .unwrap()
        .clone();
    let mut entities = app.world_mut().resource_mut::<EntityModelSet>();
    entities.entities[0].x = pending.to.0;
    entities.entities[0].y = pending.to.1;
    drop(entities);
    push_test_movement_ack(app, pending.to.0, pending.to.1, pending.direction);
}

#[test]
fn all_run_intents_walk_during_struck_guard_at_600ms_then_resume_running() {
    for input in [
        RunInput::RightHold,
        RunInput::NewMovePath,
        RunInput::Keyboard,
        RunInput::AutoRun,
    ] {
        let (mut app, receiver) = guard_input_app(input, 100);
        let receipt = Instant::now();
        app.world_mut()
            .resource_mut::<WorldPointerMovementState>()
            .observe_owner_struck_snapshot(&struck_snapshot(9, 1, receipt), 1000.0, receipt);
        press_run(&mut app, input);
        app.update();
        for step in 0..5 {
            assert!(
                matches!(receiver.try_recv(), Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "right"),
                "{input:?}, step {step}"
            );
            assert!(receiver.try_recv().is_err(), "one intent per cadence");
            let movement = app.world().resource::<WorldPointerMovementState>();
            assert_eq!(
                movement.pending.front().unwrap().visual_until_ms
                    - movement.pending.front().unwrap().sent_at_ms,
                600.0
            );
            if step == 0 {
                if matches!(input, RunInput::NewMovePath) {
                    // Source NewMove paths survive mouse-up until destination/Esc.
                    release_run(&mut app, input);
                } else {
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .clear_just_pressed(MouseButton::Right);
                    app.world_mut()
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .clear_just_pressed(KeyCode::ArrowRight);
                }
            }
            acknowledge_step(&mut app);
            advance_movement_clock(&mut app, 599);
            app.update();
            assert!(
                receiver.try_recv().is_err(),
                "{input:?} bypassed 600 ms cadence"
            );
            advance_movement_clock(&mut app, 1);
            app.update();
        }
        assert!(
            matches!(receiver.try_recv(), Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right"),
            "{input:?} did not resume Run after 2500 ms"
        );
        assert!(receiver.try_recv().is_err());
    }
}

#[test]
fn low_hp_common_gate_walks_at_9_and_runs_at_10_for_every_input() {
    for input in [
        RunInput::RightHold,
        RunInput::NewMovePath,
        RunInput::Keyboard,
        RunInput::AutoRun,
    ] {
        for hp in [9, 10] {
            let (mut app, receiver) = guard_input_app(input, hp);
            app.world_mut()
                .resource_mut::<WorldPointerMovementState>()
                .run_primed_until_ms = 10_000.0;
            press_run(&mut app, input);
            app.update();
            let command = receiver
                .try_recv()
                .expect("HP floor must still permit a walk");
            assert!(
                if hp == 9 {
                    matches!(command, GatewayCommand::Player(PlayerIntent::Walk { direction }) if direction == "right")
                } else {
                    matches!(command, GatewayCommand::Player(PlayerIntent::Run { direction }) if direction == "right")
                },
                "{input:?}, hp {hp}"
            );
            assert!(receiver.try_recv().is_err());
        }
    }
}

#[test]
fn stale_personal_owner_cannot_clear_new_transport_guard_or_send_movement() {
    for input in [
        RunInput::RightHold,
        RunInput::NewMovePath,
        RunInput::Keyboard,
        RunInput::AutoRun,
    ] {
        let (mut app, receiver) = guard_input_app(input, 100);
        let receipt = Instant::now();
        let mut snapshot = struck_snapshot(13, 1, receipt);
        snapshot.entity_render_payload = Some(json!({"playerObjectId":2000}));
        snapshot.owner_struck.as_mut().unwrap().object_id = 2000;
        {
            let mut movement = app.world_mut().resource_mut::<WorldPointerMovementState>();
            movement.observe_gameplay_generation(12, 1000.0);
            movement.observe_owner_struck_snapshot(&snapshot, 1000.0, receipt);
        }
        press_run(&mut app, input);
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "{input:?} used the previous owner's model"
        );
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .run_allowed_after_ms,
            Some(3500.0),
        );

        app.world_mut().resource_mut::<EntityModelSet>().entities[0].object_id = "2000".to_owned();
        press_run(&mut app, input);
        app.update();
        assert!(
            matches!(
                receiver.try_recv(),
                Ok(GatewayCommand::Player(PlayerIntent::Walk { .. }))
            ),
            "{input:?} lost the new owner's Struck guard"
        );
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .run_allowed_after_ms,
            Some(3500.0),
        );
        assert!(receiver.try_recv().is_err());
    }
}

#[test]
fn release_or_path_cancel_stays_quiet_after_guard_expires() {
    for input in [
        RunInput::RightHold,
        RunInput::NewMovePath,
        RunInput::Keyboard,
        RunInput::AutoRun,
    ] {
        let (mut app, receiver) = guard_input_app(input, 100);
        let receipt = Instant::now();
        let snapshot = struck_snapshot(10, 1, receipt);
        app.world_mut()
            .resource_mut::<WorldPointerMovementState>()
            .observe_owner_struck_snapshot(&snapshot, 1000.0, receipt);
        press_run(&mut app, input);
        app.update();
        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Walk { .. }))
        ));
        release_run(&mut app, input);
        if matches!(input, RunInput::NewMovePath) {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
        }
        acknowledge_step(&mut app);
        app.update();
        assert!(receiver.try_recv().is_err());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear_just_pressed(KeyCode::Escape);
        for _ in 0..10 {
            advance_movement_clock(&mut app, 500);
            let now = app.world().resource::<Time>().elapsed_secs_f64() * 1000.0;
            app.world_mut()
                .resource_mut::<WorldPointerMovementState>()
                .observe_owner_struck_snapshot(&snapshot, now, receipt);
            app.update();
            assert!(
                receiver.try_recv().is_err(),
                "{input:?} resurrected a released/cancelled intent"
            );
        }
        let movement = app.world().resource::<WorldPointerMovementState>();
        assert!(movement.pending.is_empty());
        assert_eq!(movement.active, None);
        assert_eq!(movement.auto_path_destination, None);
    }
}

#[test]
fn gameplay_drain_arms_guard_in_real_time_and_ignores_retained_duplicate() {
    use mir2_client_bevy::big_map::BigMapModel;
    use mir2_client_bevy::pending_operations::{AuthoritativeModelRevisions, PendingOperations};
    use mir2_client_bevy::quest_model::{CompletedQuestTracker, GroundPickupModel, NearbyNpcModel};

    let (sender, receiver) = std::sync::mpsc::channel();
    let mut app = bevy::prelude::App::new();
    app.insert_resource(NativeShellModel {
        screen: NativeShellScreen::InGame,
        ..Default::default()
    });
    app.insert_resource(GameplayEventInbox::new(receiver));
    app.init_resource::<Time>()
        .init_resource::<Time<bevy::time::Real>>()
        .init_resource::<WorldPointerMovementState>()
        .init_resource::<QuestTracker>()
        .init_resource::<CompletedQuestTracker>()
        .init_resource::<NpcDialogModel>()
        .init_resource::<NearbyNpcModel>()
        .init_resource::<CombatTargetModel>()
        .init_resource::<GroundPickupModel>()
        .init_resource::<NoticeDialogState>()
        .init_resource::<NativeEntityPresentation>()
        .init_resource::<crate::entity_overlays::NativeEntityOverlays>()
        .init_resource::<NativeEffects>()
        .init_resource::<BigMapModel>()
        .init_resource::<AuthoritativeModelRevisions>()
        .init_resource::<PendingOperations>();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(250));
    app.world_mut()
        .resource_mut::<Time<bevy::time::Real>>()
        .advance_by(Duration::from_millis(3000));
    app.add_systems(
        bevy::prelude::Update,
        crate::gameplay_bridge::drain_gameplay_events,
    );
    let snapshot = struck_snapshot(11, 1, Instant::now());
    let receipt = snapshot.owner_struck.as_ref().unwrap().received_at;
    sender.send(snapshot.clone()).unwrap();
    app.update();
    let deadline = app
        .world()
        .resource::<WorldPointerMovementState>()
        .run_allowed_after_ms
        .unwrap();
    let earliest = 5500.0 - receipt.elapsed().as_secs_f64() * 1000.0;
    assert!(
        (earliest..=5500.0).contains(&deadline),
        "real clock receipt deadline: {deadline}"
    );
    app.world_mut()
        .resource_mut::<Time<bevy::time::Real>>()
        .advance_by(Duration::from_millis(2000));
    sender.send(snapshot).unwrap();
    app.update();
    assert_eq!(
        app.world()
            .resource::<WorldPointerMovementState>()
            .run_allowed_after_ms,
        Some(deadline)
    );
}

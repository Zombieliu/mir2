use super::*;

#[test]
fn shared_map_layer_gate_keeps_world_mutations_and_skips_owner_only_packets() {
    let owner_only = ServerPacket::GainExperience { amount: 6 };
    let movement = ServerPacket::ObjectWalk {
        movement: ObjectMovement {
            object_id: 41,
            position: Point { x: 120, y: 90 },
            direction: MirDirection::Right,
        },
    };
    let live_health = ServerPacket::ObjectHealth {
        info: ObjectHealthInfo {
            object_id: 41,
            percent: 50,
            expire: 0,
        },
    };
    let death_health = ServerPacket::ObjectHealth {
        info: ObjectHealthInfo {
            object_id: 41,
            percent: 0,
            expire: 0,
        },
    };
    let death = ServerPacket::ObjectDied {
        info: ObjectDiedInfo {
            object_id: 41,
            location: Point { x: 120, y: 90 },
            direction: MirDirection::Right,
            kind: 0,
        },
    };

    assert!(!packet_mutates_shared_map_layer(&owner_only));
    assert!(packet_mutates_shared_map_layer(&movement));
    assert!(packet_mutates_shared_map_layer(&live_health));
    assert!(packet_mutates_shared_map_layer(&death_health));
    assert!(packet_mutates_shared_map_layer(&death));

    assert!(!packets_may_commit_shared_death_drops(&[owner_only]));
    assert!(!packets_may_commit_shared_death_drops(&[live_health]));
    assert!(packets_may_commit_shared_death_drops(&[death_health]));
    assert!(packets_may_commit_shared_death_drops(&[death]));
}

#[test]
fn tick_motion_gate_detects_only_packets_the_suppression_can_remove() {
    let movement = ServerPacket::ObjectRun {
        movement: ObjectMovement {
            object_id: 77,
            position: Point { x: 121, y: 90 },
            direction: MirDirection::Right,
        },
    };
    let health = ServerPacket::ObjectHealth {
        info: ObjectHealthInfo {
            object_id: 77,
            percent: 75,
            expire: 0,
        },
    };

    assert!(coalesced_zone_movement_object_id(&movement).is_some());
    assert!(coalesced_zone_movement_object_id(&health).is_none());
}

#[test]
fn owner_location_uses_its_priority_channel_when_observer_backlog_is_full() {
    let (normal_sender, mut normal_receiver) = tokio::sync::mpsc::channel(1);
    let (owner_sender, mut owner_receiver) = tokio::sync::mpsc::channel(1);
    let sender = crate::routing::SharedZoneLiveOutboundSender::new(normal_sender, owner_sender);

    sender
        .try_send(crate::routing::SharedZoneLiveOutbound::new(
            7,
            ServerPacket::ObjectRemove { object_id: 9 },
        ))
        .expect("observer packet should fill the normal channel");
    sender
        .try_send(crate::routing::SharedZoneLiveOutbound::new(
            7,
            ServerPacket::UserLocation {
                location: mir2_protocol::UserLocation {
                    position: Point { x: 25, y: 23 },
                    direction: MirDirection::Up,
                },
            },
        ))
        .expect("owner location must bypass a saturated observer channel");

    assert!(matches!(
        owner_receiver
            .try_recv()
            .expect("owner location should be immediately available")
            .into_packet(),
        ServerPacket::UserLocation { location }
            if location.position == Point { x: 25, y: 23 }
                && location.direction == MirDirection::Up
    ));
    assert!(matches!(
        normal_receiver
            .try_recv()
            .expect("normal backlog should remain isolated")
            .into_packet(),
        ServerPacket::ObjectRemove { object_id: 9 }
    ));
}

#[test]
fn activating_live_registration_retries_a_location_queued_during_the_registration_gap() {
    let mut state = SharedInProcessZoneState::new();
    let key = ZonePresenceKey {
        account_id: "registration-gap".to_string(),
        character_index: 0,
    };
    state.queue_zone_packets(
        key.clone(),
        vec![ServerPacket::UserLocation {
            location: mir2_protocol::UserLocation {
                position: Point { x: 25, y: 23 },
                direction: MirDirection::Up,
            },
        }],
    );

    let zone_state = Arc::new(Mutex::new(state));
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let registration_id = zone_state
        .lock()
        .unwrap()
        .register_live_zone_outbound(
            key.clone(),
            crate::routing::SharedZoneLiveOutboundSender::single(sender),
        );
    let registration = crate::routing::SharedZoneLiveOutboundRegistration {
        zone_state,
        key,
        registration_id,
    };

    crate::routing::ZoneLiveOutboundRegistration::activate(&registration);

    assert!(matches!(
        receiver
            .try_recv()
            .expect("activation should immediately retry the pending owner location")
            .into_packet(),
        ServerPacket::UserLocation { location }
            if location.position == Point { x: 25, y: 23 }
    ));
}

#[test]
fn saturated_live_outbound_retries_latest_owner_location_before_observer_motion() {
    let mut state = SharedInProcessZoneState::new();
    let key = ZonePresenceKey {
        account_id: "movement-backpressure".to_string(),
        character_index: 0,
    };
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    state.register_live_zone_outbound(
        key.clone(),
        crate::routing::SharedZoneLiveOutboundSender::single(sender),
    );

    state.queue_zone_packets(
        key.clone(),
        vec![ServerPacket::ObjectRemove { object_id: 9 }],
    );
    state.queue_zone_packets(
        key.clone(),
        vec![
            ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: 77,
                    position: Point { x: 120, y: 90 },
                    direction: MirDirection::Right,
                },
            },
            ServerPacket::UserLocation {
                location: mir2_protocol::UserLocation {
                    position: Point { x: 10, y: 10 },
                    direction: MirDirection::Down,
                },
            },
            ServerPacket::UserLocation {
                location: mir2_protocol::UserLocation {
                    position: Point { x: 11, y: 11 },
                    direction: MirDirection::DownRight,
                },
            },
        ],
    );

    assert!(matches!(
        receiver.try_recv().expect("fixture should fill the live channel").into_packet(),
        ServerPacket::ObjectRemove { object_id: 9 }
    ));

    state.retry_pending_realtime_zone_outbounds();
    assert!(matches!(
        receiver
            .try_recv()
            .expect("latest owner correction should be retried first")
            .into_packet(),
        ServerPacket::UserLocation { location }
            if location.position == Point { x: 11, y: 11 }
                && location.direction == MirDirection::DownRight
    ));

    state.retry_pending_realtime_zone_outbounds();
    assert!(matches!(
        receiver
            .try_recv()
            .expect("observer movement should remain retryable")
            .into_packet(),
        ServerPacket::ObjectWalk { movement } if movement.object_id == 77
    ));
    assert!(!state.pending_zone_packets.contains_key(&key));
}

#[test]
fn pending_packet_overflow_never_discards_the_latest_owner_location() {
    let mut state = SharedInProcessZoneState::new();
    let key = ZonePresenceKey {
        account_id: "movement-overflow".to_string(),
        character_index: 0,
    };
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    state.register_live_zone_outbound(
        key.clone(),
        crate::routing::SharedZoneLiveOutboundSender::single(sender),
    );
    state.queue_zone_packets(
        key.clone(),
        vec![ServerPacket::ObjectRemove { object_id: 1 }],
    );
    state.queue_zone_packets(
        key.clone(),
        vec![ServerPacket::UserLocation {
            location: mir2_protocol::UserLocation {
                position: Point { x: 21, y: 34 },
                direction: MirDirection::UpLeft,
            },
        }],
    );
    for object_id in
        10_000..10_000 + crate::routing::MAX_PENDING_ZONE_PACKETS_PER_PLAYER as u32 + 32
    {
        state.queue_zone_packets(
            key.clone(),
            vec![ServerPacket::ObjectRemove { object_id }],
        );
    }

    assert_eq!(
        state.pending_zone_packets[&key].len(),
        crate::routing::MAX_PENDING_ZONE_PACKETS_PER_PLAYER,
    );
    assert!(state.pending_zone_packets[&key].iter().any(|packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == Point { x: 21, y: 34 }
    )));

    let _ = receiver.try_recv().expect("fixture should fill the live channel");
    state.retry_pending_realtime_zone_outbounds();
    assert!(matches!(
        receiver
            .try_recv()
            .expect("owner location should survive overflow and retry first")
            .into_packet(),
        ServerPacket::UserLocation { location }
            if location.position == Point { x: 21, y: 34 }
    ));
}

#[test]
fn client_version_keeps_world_state_and_returns_the_protocol_response_without_outcome_snapshot() {
    let zone = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(zone);
    start_new_runtime(&mut runtime, "client-version-read-only", "VersionReader");
    let before = runtime.inner.world_snapshot();

    let execution = runtime
        .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::ClientVersion {
            version_hash: Vec::new(),
        }))
        .expect("ClientVersion should remain a valid production protocol request");
    let after = runtime.inner.world_snapshot();

    assert!(execution
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ClientVersion { result: 1 })));
    assert_eq!(execution.outcome.snapshot_tick, 0);
    assert_eq!(after.tick, before.tick);
    assert_eq!(after.map_file_name, before.map_file_name);
    assert_eq!(after.player_object_id, before.player_object_id);
    assert_eq!(after.player_hp, before.player_hp);
    assert_eq!(after.player_mp, before.player_mp);
    assert_eq!(after.entities, before.entities);
    assert_eq!(after.inventory_items, before.inventory_items);
    assert_eq!(after.quest_log, before.quest_log);
}

use super::*;

fn self_position(session: &GatewaySession) -> Point {
    let snapshot = session.world_snapshot();
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("snapshot should contain the active player");
    Point {
        x: player.x,
        y: player.y,
    }
}

fn wait_for_zone_position(session: &GatewaySession, expected: Point) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if self_position(session) == expected {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "shared Zone did not reach {expected:?}; last position was {:?}",
        self_position(session)
    );
}

fn map_information_count(packets: &[ServerPacket]) -> usize {
    packets
        .iter()
        .filter(|packet| matches!(packet, ServerPacket::MapInformation { .. }))
        .count()
}

#[test]
fn third_request_can_complete_previous_step_and_leave_newer_step_queued_at_source() {
    let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(zone_state.clone());
    start_demo_runtime(&mut runtime);
    runtime
        .execute(WorldCommand::TransferMap {
            key: "crystal:0115:17:18".to_string(),
        })
        .expect("fixture transfer should execute");
    let key = runtime.current_presence_key().expect("presence should exist");
    let session_id = runtime
        .current_zone_session_id()
        .expect("Zone session should exist");
    let now_ms = shared_gateway_now_ms();

    let first_transform = {
        let mut state = zone_state.lock().expect("Zone state should lock");
        let mut outbounds = state.zone_manager.handle(ZoneCommand::Walk {
            session_id: session_id.clone(),
            direction: MirDirection::Down,
            seq: 1,
            now_ms,
        });
        outbounds.extend(state.zone_manager.handle(ZoneCommand::TickPlayerMovement {
            session_id: session_id.clone(),
            now_ms,
        }));
        state.dispatch_zone_outbounds(outbounds, Some(&key)).1
    };
    runtime.apply_zone_transform(first_transform);
    assert_eq!(
        runtime
            .inner
            .active_zone_join_snapshot(session_id.as_str().to_string())
            .expect("private player should exist")
            .position,
        Point { x: 17, y: 19 }
    );

    // Queue step two before movement is ready.
    let second_packets = {
        let mut state = zone_state.lock().expect("Zone state should lock");
        let outbounds = state.zone_manager.handle(ZoneCommand::Walk {
            session_id: session_id.clone(),
            direction: MirDirection::Down,
            seq: 2,
            now_ms: now_ms + 1,
        });
        let (packets, transform, ..) = state.dispatch_zone_outbounds(outbounds, Some(&key));
        assert!(packets.is_empty());
        assert!(transform.is_none());
        packets
    };
    runtime
        .movement_ingress
        .session_state
        .lock()
        .expect("movement state should lock")
        .note_player_movement_packets(&second_packets, now_ms + 1);
    assert!(
        runtime
            .movement_ingress
            .session_state
            .lock()
            .expect("movement state should lock")
            .pending_zone_player_movement
    );

    // Once step two is ready, submitting step three consumes step two and
    // returns its UserLocation while leaving step three queued for the source
    // cell. This is the overlap that made packet-presence bookkeeping report
    // no pending move.
    let (third_packets, completed_transform) = {
        let mut state = zone_state.lock().expect("Zone state should lock");
        let outbounds = state.zone_manager.handle(ZoneCommand::Walk {
            session_id: session_id.clone(),
            direction: MirDirection::Down,
            seq: 3,
            now_ms: now_ms + 1_000,
        });
        let (packets, transform, ..) = state.dispatch_zone_outbounds(outbounds, Some(&key));
        if let Some(transform) = transform.as_ref() {
            state.pending_zone_transforms.insert(key.clone(), transform.clone());
        }
        (packets, transform)
    };
    assert!(third_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 17, y: 20 })
    )));
    assert_eq!(
        completed_transform,
        Some((Point { x: 17, y: 20 }, MirDirection::Down))
    );
    runtime
        .movement_ingress
        .session_state
        .lock()
        .expect("movement state should lock")
        .note_player_movement_packets(&third_packets, now_ms + 1_000);
    assert!(
        !runtime
            .movement_ingress
            .session_state
            .lock()
            .expect("movement state should lock")
            .pending_zone_player_movement,
        "the historical packet proxy loses the newer queued step"
    );

    // The autonomous cadence later completes the newer third step. Its
    // transform replaces the older pending mirror update and lands exactly on
    // the MageHouse exit source.
    {
        let mut state = zone_state.lock().expect("Zone state should lock");
        let outbounds = state.zone_manager.handle(ZoneCommand::Tick {
            now_ms: now_ms + 2_000,
        });
        let _ = state.dispatch_zone_outbounds(outbounds, None);
        assert_eq!(
            state.pending_zone_transforms.get(&key),
            Some(&(Point { x: 17, y: 21 }, MirDirection::Down))
        );
    }

    let packets = runtime.apply_pending_zone_packets();
    assert_eq!(map_information_count(&packets), 1, "{packets:?}");
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MapInformation { info } if info.file_name == "0"
    )));
    assert_eq!(runtime.inner.world_snapshot().map_file_name.as_deref(), Some("0"));
}

#[test]
fn deferred_zone_movement_onto_mage_house_source_transfers_on_session_tick_once() {
    let registry = ZoneRegistry::in_process();
    let mut session =
        GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    start_demo_character(&mut session);
    session.transfer_map("crystal:0115:17:19");

    let first = session.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Down,
    });
    assert!(first.iter().any(|packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 17, y: 20 })
    )));

    // The second movement is admitted while the first movement cadence is
    // still active. Its immediate command result cannot transfer because the
    // authoritative Zone has not reached the source cell yet.
    let queued = session.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Down,
    });
    assert_eq!(map_information_count(&queued), 0, "{queued:?}");
    assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0115"));

    wait_for_zone_position(&session, Point { x: 17, y: 21 });
    let tick = session.tick();
    assert_eq!(map_information_count(&tick), 1, "{tick:?}");
    assert!(tick.iter().any(|packet| matches!(
        packet,
        ServerPacket::MapInformation { info } if info.file_name == "0"
    )));
    assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0"));
    assert_eq!(self_position(&session), Point { x: 315, y: 476 });

    let idle_tick = session.tick();
    assert_eq!(map_information_count(&idle_tick), 0, "{idle_tick:?}");
    assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0"));
}

#[test]
fn deferred_zone_movement_away_from_a_source_does_not_transfer() {
    let registry = ZoneRegistry::in_process();
    let mut session =
        GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    start_demo_character(&mut session);
    session.transfer_map("crystal:0102:3:7");

    let first = session.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Right,
    });
    assert!(first.iter().any(|packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 4, y: 7 })
    )));
    let queued = session.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Right,
    });
    assert_eq!(map_information_count(&queued), 0, "{queued:?}");

    wait_for_zone_position(&session, Point { x: 5, y: 7 });
    let tick = session.tick();
    assert_eq!(map_information_count(&tick), 0, "{tick:?}");
    assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0102"));
    assert_eq!(self_position(&session), Point { x: 5, y: 7 });
}

#[test]
fn idle_tick_on_a_transfer_source_does_not_transfer_spawned_player() {
    let registry = ZoneRegistry::in_process();
    let mut session =
        GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    start_demo_character(&mut session);
    session.transfer_map("crystal:0115:17:21");

    assert_eq!(self_position(&session), Point { x: 17, y: 21 });
    let tick = session.tick();

    assert_eq!(map_information_count(&tick), 0, "{tick:?}");
    assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0115"));
    assert_eq!(self_position(&session), Point { x: 17, y: 21 });
}

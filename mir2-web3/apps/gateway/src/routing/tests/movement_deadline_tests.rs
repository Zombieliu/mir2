use super::*;

#[test]
fn movement_deadline_owner_wakes_without_maintenance_even_with_pending_inputs() {
    let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(zone_state.clone());
    start_demo_runtime(&mut runtime);
    runtime
        .execute(WorldCommand::TransferMap {
            key: "crystal:0115:17:18".into(),
        })
        .unwrap();
    let id = runtime.current_zone_session_id().unwrap();
    let started_ms = shared_gateway_now_ms();
    {
        let mut state = zone_state.lock().unwrap();
        state.zone_manager.handle(ZoneCommand::Walk {
            session_id: id.clone(),
            direction: MirDirection::Down,
            seq: 1,
            now_ms: started_ms,
        });
        state.zone_manager.tick_pending_movement(started_ms);
        state.zone_manager.handle(ZoneCommand::Walk {
            session_id: id,
            direction: MirDirection::Down,
            seq: 2,
            now_ms: started_ms + 1,
        });
        assert_eq!(
            super::super::shared_zone_movement_deadline(&state),
            Some(started_ms + 600)
        );
    }
    let sender = super::super::spawn_shared_zone_owner_with_cadence(
        &ZoneId::new("deadline-owner"),
        zone_state.clone(),
        Duration::from_secs(60),
    );
    let inactive_session = Arc::new(Mutex::new(
        super::super::SharedZoneMovementSessionState::default(),
    ));
    let timeout = Instant::now() + Duration::from_secs(2);
    loop {
        {
            let state = zone_state.lock().unwrap();
            if super::super::shared_zone_movement_deadline(&state).is_none() {
                assert!(
                    shared_gateway_now_ms() >= started_ms + 600,
                    "must preserve movement cooldown"
                );
                assert_eq!(
                    state.zone_cadence_tick_count, 0,
                    "movement must wake before maintenance"
                );
                break;
            }
        }
        assert!(
            Instant::now() < timeout,
            "pending inputs must not starve the movement deadline"
        );
        let (response_sender, response_receiver) = std::sync::mpsc::sync_channel(1);
        drop(response_receiver);
        let _ = sender.try_send(super::super::SharedZoneMovementRequest {
            packet: ClientPacket::Turn {
                direction: MirDirection::Right,
            },
            expected_presence_epoch: 0,
            session_state: inactive_session.clone(),
            response_sender,
        });
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn movement_deadline_maintenance_cost_does_not_drift_and_late_ticks_skip_burst() {
    let start = Instant::now();
    let cadence = Duration::from_millis(300);
    let mut deadline = start + cadence;
    for n in 1..10 {
        let finish = deadline + Duration::from_millis(30);
        deadline = super::super::next_owner_maintenance_deadline(deadline, finish, cadence);
        assert_eq!(deadline, start + Duration::from_millis((n + 1) * 300));
    }
    let next = super::super::next_owner_maintenance_deadline(
        start + cadence,
        start + Duration::from_millis(2130),
        cadence,
    );
    assert_eq!(next, start + Duration::from_millis(2400));
    assert!(next > start + Duration::from_millis(2130));
}

#[test]
fn movement_deadline_gateway_dispatches_due_move_without_maintenance_and_freeze_suppresses_wake() {
    let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(zone_state.clone());
    start_demo_runtime(&mut runtime);
    runtime
        .execute(WorldCommand::TransferMap {
            key: "crystal:0115:17:18".into(),
        })
        .unwrap();
    let key = runtime.current_presence_key().unwrap();
    let id = runtime.current_zone_session_id().unwrap();
    let time = shared_gateway_now_ms() + 301;
    {
        let mut state = zone_state.lock().unwrap();
        state.zone_manager.handle(ZoneCommand::Walk {
            session_id: id.clone(),
            direction: MirDirection::Down,
            seq: 1,
            now_ms: time,
        });
        state.zone_manager.tick_pending_movement(time);
        state.zone_manager.handle(ZoneCommand::Walk {
            session_id: id.clone(),
            direction: MirDirection::Down,
            seq: 2,
            now_ms: time + 1,
        });
        assert_eq!(
            super::super::shared_zone_movement_deadline(&state),
            Some(time + 600)
        );
        state.begin_teardown_fence(&key).unwrap();
        assert_eq!(super::super::shared_zone_movement_deadline(&state), None);
    }
    super::super::run_shared_zone_owner_tick(&zone_state, time + 600, false).unwrap();
    {
        let mut state = zone_state.lock().unwrap();
        state.release_teardown_fence(&key);
        assert_eq!(
            super::super::shared_zone_movement_deadline(&state),
            Some(time + 600)
        );
    }
    super::super::run_shared_zone_owner_tick(&zone_state, time + 600, false).unwrap();
    let mut state = zone_state.lock().unwrap();
    assert_eq!(
        state.zone_cadence_tick_count, 0,
        "movement wake must not advance maintenance"
    );
    assert_eq!(super::super::shared_zone_movement_deadline(&state), None);
    let packets = state.take_pending_zone_packets(&key);
    assert!(packets.iter().any(
        |p| matches!(p,ServerPacket::UserLocation{location} if location.position==Point{x:17,y:20})
    ));
}

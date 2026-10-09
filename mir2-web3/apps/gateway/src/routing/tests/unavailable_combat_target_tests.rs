//! A rejected ordinary attack still closes the native movement ACK lifecycle.
use super::*;
use crate::routing::{SharedZoneLiveOutbound, SharedZoneLiveOutboundSender};

#[derive(Clone, Copy, Debug)]
enum Delivery {
    Direct,
    Live,
    LiveFull,
}

#[derive(Clone, Copy, Debug)]
enum TargetState {
    Dead,
    Removed,
    Missing,
}

fn rejected_target_case(
    class: MirClass,
    target_state: TargetState,
    queued: bool,
    delivery: Delivery,
) {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_new_runtime_with_class(&mut runtime, "target-loss", "TargetLoss", class);
    let key = runtime.current_presence_key().unwrap();
    let session = runtime.current_zone_session_id().unwrap();
    let owner_id = runtime.current_zone_player_object_id().unwrap();
    let local_id = runtime.local_self_object_id().unwrap();
    let position = Point { x: 330, y: 270 };
    let target_id = 260_917;
    {
        let mut state = shared.lock().unwrap();
        let moved = state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
            session_id: session.clone(),
            position: position.clone(),
            direction: MirDirection::Right,
        });
        state.dispatch_zone_outbounds(moved, Some(&key));
        let mut target = shared_monster_entity(target_id);
        target.ai = Some(2);
        target.x = 331;
        target.y = 270;
        target.dead = true;
        target.hp = Some(0);
        if !matches!(target_state, TargetState::Missing) {
            state.sync_map_layer(
                "0".into(),
                vec![target],
                BTreeSet::new(),
                Vec::new(),
                BTreeSet::new(),
            );
        }
        if matches!(target_state, TargetState::Removed) {
            let map = state.maps.get_mut("0").unwrap();
            map.entities.remove(&target_id);
            map.removed_entity_ids.insert(target_id);
        }
        if matches!(target_state, TargetState::Missing) {
            let map = state.maps.get("0").unwrap();
            assert!(!map.entities.contains_key(&target_id));
            assert!(!map.dead_entity_ids.contains_key(&target_id));
            assert!(!map.removed_entity_ids.contains(&target_id));
        }
        // Bootstrap packets are not evidence of this command's correction.
        state.take_pending_zone_packets(&key);
    }
    assert!(runtime
        .prepare_zone_native_player_attack(&WorldCommand::Attack {
            object_id: target_id,
        })
        .is_none());
    assert_eq!(
        runtime.shared_action_target_available(target_id),
        matches!(target_state, TargetState::Missing)
    );
    assert!(runtime.has_bound_zone_player());

    let now = shared_gateway_now_ms();
    if queued {
        let mut state = shared.lock().unwrap();
        // The ordinary first step establishes the original 600 ms cadence;
        // only the second command belongs to the still-pending movement slot.
        let first = state.zone_manager.handle(ZoneCommand::Walk {
            session_id: session.clone(),
            direction: MirDirection::Down,
            seq: 1,
            now_ms: now,
        });
        state.dispatch_zone_outbounds(first, Some(&key));
        let pending = state.zone_manager.handle(ZoneCommand::Run {
            session_id: session.clone(),
            direction: MirDirection::Down,
            seq: 2,
            now_ms: now + 1,
        });
        state.dispatch_zone_outbounds(pending, Some(&key));
        assert!(state
            .zone_manager
            .next_pending_movement_deadline_ms()
            .is_some());
        state.take_pending_zone_packets(&key);
        runtime
            .movement_ingress
            .session_state
            .lock()
            .unwrap()
            .pending_zone_player_movement = true;
    }
    let mut live_rx: Option<tokio::sync::mpsc::Receiver<SharedZoneLiveOutbound>> = None;
    if !matches!(delivery, Delivery::Direct) {
        let capacity = if matches!(delivery, Delivery::LiveFull) {
            1
        } else {
            64
        };
        let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
        shared
            .lock()
            .unwrap()
            .register_live_zone_outbound(key.clone(), SharedZoneLiveOutboundSender::single(sender));
        live_rx = Some(receiver);
    }
    if matches!(delivery, Delivery::LiveFull) {
        // Fill the real FIFO with an earlier authoritative correction.
        assert!(runtime.authoritative_zone_owner_correction().is_empty());
    }
    let before = shared
        .lock()
        .unwrap()
        .zone_manager
        .player_transform(&session)
        .unwrap();
    let direct = runtime
        .execute(WorldCommand::Attack {
            object_id: target_id,
        })
        .unwrap();
    if live_rx.is_some() {
        assert!(
            !direct
                .iter()
                .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })),
            "a rejected action cannot bypass the established owner FIFO"
        );
    }
    let mut wire = direct;
    if let Some(receiver) = live_rx.as_mut() {
        while let Ok(packet) = receiver.try_recv() {
            wire.push(packet.into_packet());
        }
        shared
            .lock()
            .unwrap()
            .retry_pending_realtime_zone_outbounds();
        while let Ok(packet) = receiver.try_recv() {
            wire.push(packet.into_packet());
        }
    }
    let locations: Vec<_> = wire
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::UserLocation { location } => {
                Some((location.position.clone(), location.direction))
            }
            _ => None,
        })
        .collect();
    let expected_count = if matches!(delivery, Delivery::LiveFull) {
        2
    } else {
        1
    };
    assert_eq!(locations.len(), expected_count,
        "one fresh real correction is required: class={class:?}, target={target_state:?}, queued={queued}, delivery={delivery:?}; {wire:?}");
    assert!(locations.iter().all(|location| location == &before));
    assert!(!wire.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectAttack { info } if info.object_id == owner_id || info.object_id == local_id
    )), "an unavailable target cannot generate a phantom swing");
    assert!(
        !runtime
            .movement_ingress
            .session_state
            .lock()
            .unwrap()
            .pending_zone_player_movement
    );
    let mut state = shared.lock().unwrap();
    assert!(
        state
            .zone_manager
            .next_pending_movement_deadline_ms()
            .is_none(),
        "the rejected attack must cancel an earlier buffered run"
    );
    let after_cooldown = state.zone_manager.tick_pending_movement(now + 5_000);
    assert_eq!(
        state.zone_manager.player_transform(&session).unwrap(),
        before
    );
    assert!(
        !after_cooldown.iter().any(|outbound| matches!(outbound,
            ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. }
                if packets.iter().any(|packet| matches!(packet,
                    ServerPacket::ObjectWalk { .. } | ServerPacket::ObjectRun { .. }))
        )),
        "a discarded pre-attack run must never execute later"
    );
}

fn three_class_cases(delivery: Delivery) {
    for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
        for target in [
            TargetState::Dead,
            TargetState::Removed,
            TargetState::Missing,
        ] {
            for queued in [false, true] {
                rejected_target_case(class, target, queued, delivery);
            }
        }
    }
}

#[test]
fn unavailable_combat_target_closes_direct_native_movement_slot() {
    three_class_cases(Delivery::Direct);
}

#[test]
fn unavailable_combat_target_closes_live_native_movement_slot() {
    three_class_cases(Delivery::Live);
}

#[test]
fn unavailable_combat_target_closes_full_fifo_native_movement_slot() {
    three_class_cases(Delivery::LiveFull);
}

#[test]
fn unavailable_combat_target_guard_requires_actual_zone_binding() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_new_runtime_with_class(
        &mut runtime,
        "binding-guard",
        "BindingGuard",
        MirClass::Warrior,
    );
    let key = runtime.current_presence_key().unwrap();
    let session = runtime.current_zone_session_id().unwrap();
    assert!(runtime.has_bound_zone_player());
    {
        let mut state = shared.lock().unwrap();
        assert_eq!(state.zone_sessions.remove(&key), Some(session.clone()));
    }
    assert!(
        runtime.current_zone_session_id().is_some(),
        "a derived ID is not a live binding"
    );
    assert!(!runtime.has_bound_zone_player());
    {
        let mut state = shared.lock().unwrap();
        state.zone_sessions.insert(key.clone(), session.clone());
        let removed = state.zone_manager.handle(ZoneCommand::Leave {
            session_id: session,
        });
        state.dispatch_zone_outbounds(removed, Some(&key));
    }
    assert!(
        !runtime.has_bound_zone_player(),
        "a map record without a live player is not a binding"
    );
}

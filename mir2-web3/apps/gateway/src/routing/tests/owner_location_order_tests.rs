use super::*;
use crate::routing::{SharedZoneLiveOutbound, SharedZoneLiveOutboundSender};

// Isolated fixture: actual shared Zone Walk/Run/Turn and actual Gateway
// dispatch/queue paths, with deterministic legal movement times. Delaying the
// channel consumer models a suspended WebSocket task; no sleeps or fake ACKs.
struct Owner {
    state: SharedInProcessZoneState,
    key: ZonePresenceKey,
    session: SessionId,
    owner_rx: tokio::sync::mpsc::Receiver<SharedZoneLiveOutbound>,
    _normal_rx: tokio::sync::mpsc::Receiver<SharedZoneLiveOutbound>,
}

impl Owner {
    fn new(owner_capacity: usize) -> Self {
        let mut state = SharedInProcessZoneState::new();
        let key = ZonePresenceKey {
            account_id: "owner-order-fixture".into(),
            character_index: 0,
        };
        let session = SessionId::new("owner-order-fixture");
        let map = "owner-order-fixture".to_string();
        let object_id = state.upsert_player(
            key.clone(),
            "OwnerOrder",
            map.clone(),
            shared_picker_entity(1000, 100, 100),
            40,
        );
        state.zone_sessions.insert(key.clone(), session.clone());
        state.zone_session_keys.insert(session.clone(), key.clone());
        let outbounds = state.zone_manager.join(ZoneJoin {
            session_id: session.clone(),
            account_id: key.account_id.clone(),
            character_index: 0,
            object_id,
            name: "OwnerOrder".into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 1,
            hp: 10,
            max_hp: 10,
            mp: 10,
            map_file_name: map,
            position: Point { x: 100, y: 100 },
            direction: MirDirection::Up,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        });
        state.dispatch_zone_outbounds(outbounds, None);
        state.take_pending_zone_packets(&key);
        let (normal_tx, normal_rx) = tokio::sync::mpsc::channel(32);
        let (owner_tx, owner_rx) = tokio::sync::mpsc::channel(owner_capacity);
        state.register_live_zone_outbound(
            key.clone(),
            SharedZoneLiveOutboundSender::new(normal_tx, owner_tx),
        );
        Self {
            state,
            key,
            session,
            owner_rx,
            _normal_rx: normal_rx,
        }
    }

    fn move_step(
        &mut self,
        run: bool,
        direction: MirDirection,
        seq: u64,
        now_ms: u64,
        current: bool,
    ) -> Vec<ServerPacket> {
        let command = if run {
            ZoneCommand::Run {
                session_id: self.session.clone(),
                direction,
                seq,
                now_ms,
            }
        } else {
            ZoneCommand::Walk {
                session_id: self.session.clone(),
                direction,
                seq,
                now_ms,
            }
        };
        let mut outbounds = self.state.zone_manager.handle(command);
        outbounds.extend(
            self.state
                .zone_manager
                .handle(ZoneCommand::TickPlayerMovement {
                    session_id: self.session.clone(),
                    now_ms,
                }),
        );
        self.state
            .dispatch_zone_outbounds(outbounds, current.then_some(&self.key))
            .0
    }

    fn drain(&mut self) -> Vec<ServerPacket> {
        let mut packets = Vec::new();
        while let Ok(outbound) = self.owner_rx.try_recv() {
            packets.push(outbound.into_packet());
        }
        packets
    }

    fn authoritative(&self) -> (Point, MirDirection) {
        self.state
            .zone_manager
            .player_transform(&self.session)
            .unwrap()
    }
}

fn final_owner_transform(packets: &[ServerPacket]) -> Option<(Point, MirDirection)> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::UserLocation { location } => {
                Some((location.position.clone(), location.direction))
            }
            _ => None,
        })
        .last()
}

#[test]
fn owner_ack_order_old_live_walk_cannot_overwrite_new_direct_run() {
    let mut owner = Owner::new(8);
    assert!(owner
        .move_step(false, MirDirection::Up, 1, 1000, false)
        .is_empty());
    assert_eq!(owner.authoritative().0, Point { x: 100, y: 99 });
    let newer_direct = owner.move_step(true, MirDirection::Right, 2, 1750, true);
    assert_eq!(owner.authoritative().0, Point { x: 102, y: 99 });
    // Reader wins the socket mutex while the older priority-channel consumer
    // is suspended. Both tasks may hold the serial gate's read lock.
    let wire = newer_direct
        .into_iter()
        .chain(owner.drain())
        .collect::<Vec<_>>();
    assert_eq!(
        final_owner_transform(&wire),
        Some(owner.authoritative()),
        "old priority ACK must not undo newer direct ACK: {wire:?}"
    );
}

#[test]
fn owner_ack_order_old_direct_walk_cannot_overwrite_new_live_run() {
    let mut owner = Owner::new(8);
    let older_direct = owner.move_step(false, MirDirection::Up, 1, 1000, true);
    assert_eq!(owner.authoritative().0, Point { x: 100, y: 99 });
    assert!(owner
        .move_step(true, MirDirection::Right, 2, 1750, false)
        .is_empty());
    assert_eq!(owner.authoritative().0, Point { x: 102, y: 99 });
    // Inverse legal task scheduling: live sender finishes the newer ACK before
    // the suspended reader writes its already-returned old Vec.
    let wire = owner
        .drain()
        .into_iter()
        .chain(older_direct)
        .collect::<Vec<_>>();
    assert_eq!(
        final_owner_transform(&wire),
        Some(owner.authoritative()),
        "old direct ACK must not undo newer priority ACK: {wire:?}"
    );
}

#[test]
fn owner_ack_order_open_priority_slot_cannot_bypass_older_pending_ack() {
    let mut owner = Owner::new(1);
    owner.move_step(false, MirDirection::Up, 1, 1000, false);
    owner.move_step(true, MirDirection::Right, 2, 1750, false);
    assert_eq!(
        owner.state.pending_zone_packets[&owner.key]
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::UserLocation { .. }))
            .count(),
        1
    );
    let mut wire = owner.drain(); // Space opens before cadence retries old pending ACK.
    let current = owner.move_step(true, MirDirection::Down, 3, 2500, true);
    assert_eq!(owner.authoritative().0, Point { x: 102, y: 101 });
    wire.extend(current);
    wire.extend(owner.drain());
    owner.state.retry_pending_realtime_zone_outbounds();
    wire.extend(owner.drain());
    assert_eq!(
        final_owner_transform(&wire),
        Some(owner.authoritative()),
        "new owner ACK cannot leave an older ACK pending behind it: {wire:?}"
    );
}

#[test]
fn owner_ack_order_registered_pending_ack_does_not_escape_via_personal_flush() {
    let mut owner = Owner::new(1);
    owner.move_step(false, MirDirection::Up, 1, 1000, false);
    owner.move_step(true, MirDirection::Right, 2, 1750, false);
    let personal = owner.state.take_pending_zone_packets(&owner.key);
    assert!(
        !personal
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })),
        "registered owner backlog belongs to its priority FIFO: {personal:?}"
    );
    let mut wire = owner.drain();
    owner.state.retry_pending_realtime_zone_outbounds();
    wire.extend(owner.drain());
    assert_eq!(final_owner_transform(&wire), Some(owner.authoritative()));
}

#[test]
fn owner_ack_order_same_map_registration_refreshes_only_an_existing_old_ack() {
    let mut owner = Owner::new(1);
    owner.move_step(false, MirDirection::Up, 1, 1000, false);
    owner.move_step(true, MirDirection::Right, 2, 1750, false);
    let old_id = owner.state.live_zone_outbounds[&owner.key].registration_id;
    owner
        .state
        .unregister_live_zone_outbound(&owner.key, old_id);
    // While detached, a later ordinary action advances the authoritative
    // position reflected by the fresh same-map resume/bootstrap snapshot.
    let fresh_bootstrap = owner.move_step(true, MirDirection::Down, 3, 2500, true);
    assert_eq!(
        final_owner_transform(&fresh_bootstrap),
        Some(owner.authoritative())
    );
    assert!(owner.state.pending_zone_packets[&owner.key].iter().any(
        |packet| matches!(packet, ServerPacket::UserLocation { location }
            if location.position == Point { x: 102, y: 99 })
    ));
    let (sender, mut new_live) = tokio::sync::mpsc::channel(8);
    let new_id = owner.state.register_live_zone_outbound(
        owner.key.clone(),
        SharedZoneLiveOutboundSender::single(sender),
    );
    owner
        .state
        .unregister_live_zone_outbound(&owner.key, old_id);
    owner.state.retry_pending_realtime_zone_outbounds();
    let mut wire = fresh_bootstrap;
    while let Ok(outbound) = new_live.try_recv() {
        assert_eq!(outbound.registration_id(), new_id);
        wire.push(outbound.into_packet());
    }
    let old_outbound = owner.owner_rx.try_recv().unwrap();
    assert_ne!(
        old_outbound.registration_id(),
        new_id,
        "old channel is fenced"
    );
    assert_eq!(final_owner_transform(&wire), Some(owner.authoritative()));
    assert!(
        wire.iter().all(|packet| match packet {
            ServerPacket::UserLocation { location } =>
                location.position == Point { x: 102, y: 101 },
            _ => true,
        }),
        "nothing older than the fresh snapshot may follow it"
    );

    // A replacement with no pending acknowledgement must not produce one.
    let (sender, mut empty_live) = tokio::sync::mpsc::channel(8);
    owner.state.register_live_zone_outbound(
        owner.key.clone(),
        SharedZoneLiveOutboundSender::single(sender),
    );
    owner.state.retry_pending_realtime_zone_outbounds();
    assert!(empty_live.try_recv().is_err());

    // No authoritative transform remains after leaving the world. A stale
    // coordinate from the old epoch is never a valid registration payload.
    owner.state.pending_zone_packets.insert(
        owner.key.clone(),
        vec![ServerPacket::UserLocation {
            location: mir2_protocol::UserLocation {
                position: Point { x: 9, y: 9 },
                direction: MirDirection::Up,
            },
        }],
    );
    owner.state.zone_manager.handle(ZoneCommand::Leave {
        session_id: owner.session.clone(),
    });
    let (sender, mut missing_live) = tokio::sync::mpsc::channel(8);
    owner.state.register_live_zone_outbound(
        owner.key.clone(),
        SharedZoneLiveOutboundSender::single(sender),
    );
    owner.state.retry_pending_realtime_zone_outbounds();
    assert!(missing_live.try_recv().is_err());
}

#[test]
fn owner_ack_order_bootstrap_discards_old_location_but_retains_personal_receipt_and_new_ack() {
    let mut owner = Owner::new(1);
    owner.move_step(false, MirDirection::Up, 1, 1000, false);
    owner.move_step(true, MirDirection::Right, 2, 1750, false);
    owner.state.queue_zone_packets(
        owner.key.clone(),
        vec![ServerPacket::GainExperience { amount: 17 }],
    );
    owner.state.begin_zone_bootstrap(&owner.key);
    let retained = owner.state.take_pending_zone_packets(&owner.key);
    assert!(matches!(
        retained.as_slice(),
        [ServerPacket::GainExperience { amount: 17 }]
    ));
    // Unregistered bootstrap/ordinary synchronous consumers retain direct
    // authoritative delivery. The old pending coordinate is already gone.
    let direct = owner.move_step(true, MirDirection::Down, 3, 2500, true);
    assert_eq!(final_owner_transform(&direct), Some(owner.authoritative()));
    let (sender, mut new_live) = tokio::sync::mpsc::channel(8);
    let new_id = owner.state.register_live_zone_outbound(
        owner.key.clone(),
        SharedZoneLiveOutboundSender::single(sender),
    );
    // A genuine movement between registration preparation and activation is
    // not an old-map packet and must remain deliverable.
    assert!(owner
        .move_step(true, MirDirection::Right, 4, 3250, true)
        .is_empty());
    owner.state.retry_pending_realtime_zone_outbounds();
    let outbound = new_live.try_recv().unwrap();
    assert_eq!(outbound.registration_id(), new_id);
    assert_eq!(
        final_owner_transform(&[outbound.into_packet()]),
        Some(owner.authoritative())
    );
    assert!(new_live.try_recv().is_err());
}

#[test]
fn owner_ack_order_fast_ingress_bookkeeping_uses_real_ack_even_when_return_vec_is_empty() {
    let owner = Owner::new(8);
    let Owner {
        state,
        key,
        mut owner_rx,
        ..
    } = owner;
    let shared = Arc::new(Mutex::new(state));
    let mut movement = crate::routing::SharedZoneMovementSessionState::default();
    movement.activate(key, "owner-order-fixture".into(), Vec::new());
    let movement = Arc::new(Mutex::new(movement));
    let before = shared_gateway_now_ms();
    let execution = crate::routing::execute_shared_zone_movement(
        &shared,
        &movement,
        &ClientPacket::Walk {
            direction: MirDirection::Up,
        },
        None,
        true,
        true,
    )
    .unwrap()
    .unwrap();
    assert!(execution.packets.is_empty());
    assert_eq!(
        execution.transform,
        Some((Point { x: 100, y: 99 }, MirDirection::Up))
    );
    let (pending, grace) = {
        let state = movement.lock().unwrap();
        (
            state.pending_zone_player_movement,
            state.recent_zone_player_movement_until_ms,
        )
    };
    assert!(!pending);
    assert!(grace > before);
    assert_eq!(
        final_owner_transform(&[owner_rx.try_recv().unwrap().into_packet()]),
        execution.transform,
    );
}

#[test]
fn owner_ack_order_registered_cancel_keeps_actual_ack_grace_and_correction_fifo() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_demo_runtime(&mut runtime);
    runtime
        .execute(WorldCommand::TransferMap {
            key: "crystal:0115:17:18".into(),
        })
        .unwrap();
    let key = runtime.current_presence_key().unwrap();
    let session = runtime.current_zone_session_id().unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    shared
        .lock()
        .unwrap()
        .register_live_zone_outbound(key.clone(), SharedZoneLiveOutboundSender::single(sender));
    let now = shared_gateway_now_ms();
    let (first_direct, queued_direct) = {
        let mut state = shared.lock().unwrap();
        let mut first = state.zone_manager.handle(ZoneCommand::Walk {
            session_id: session.clone(),
            direction: MirDirection::Down,
            seq: 1,
            now_ms: now,
        });
        first.extend(state.zone_manager.handle(ZoneCommand::TickPlayerMovement {
            session_id: session.clone(),
            now_ms: now,
        }));
        let first_direct = state.dispatch_zone_outbounds(first, Some(&key)).0;
        let queued = state.zone_manager.handle(ZoneCommand::Walk {
            session_id: session.clone(),
            direction: MirDirection::Down,
            seq: 2,
            now_ms: now + 1,
        });
        let queued_direct = state.dispatch_zone_outbounds(queued, Some(&key)).0;
        (first_direct, queued_direct)
    };
    assert!(
        first_direct
            .iter()
            .chain(&queued_direct)
            .all(|packet| !matches!(packet, ServerPacket::UserLocation { .. })),
        "ordinary AOI/bootstrap packets may remain direct, but owner ACKs use the FIFO"
    );
    let cancelled = runtime.cancel_pending_zone_player_movement();
    assert!(!cancelled
        .iter()
        .any(|p| matches!(p, ServerPacket::UserLocation { .. })));
    let (pending, grace) = {
        let movement = runtime.movement_ingress.session_state.lock().unwrap();
        (
            movement.pending_zone_player_movement,
            movement.recent_zone_player_movement_until_ms,
        )
    };
    assert!(!pending);
    assert!(
        grace > now,
        "actual cancellation ACK retains the grace window"
    );
    let mut wire = vec![receiver.try_recv().unwrap().into_packet()];
    // Invalid-action correction is authoritative too and may not escape via
    // a personal response while an older priority ACK is waiting.
    assert!(runtime.authoritative_zone_owner_correction().is_empty());
    shared
        .lock()
        .unwrap()
        .retry_pending_realtime_zone_outbounds();
    wire.push(receiver.try_recv().unwrap().into_packet());
    let authoritative = shared
        .lock()
        .unwrap()
        .zone_manager
        .player_transform(&session)
        .unwrap();
    assert_eq!(authoritative.0, Point { x: 17, y: 19 });
    assert_eq!(final_owner_transform(&wire), Some(authoritative.clone()));
    shared.lock().unwrap().live_zone_outbounds.remove(&key);
    let direct = runtime.authoritative_zone_owner_correction();
    assert_eq!(final_owner_transform(&direct), Some(authoritative));
}

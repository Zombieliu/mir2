use super::*;
use crate::routing::{SharedZoneLiveOutbound, SharedZoneLiveOutboundSender};
use std::sync::atomic::Ordering;

struct Pair {
    shared: Arc<Mutex<SharedInProcessZoneState>>,
    keys: [ZonePresenceKey; 2],
    sessions: [SessionId; 2],
    ids: [u32; 2],
}

impl Pair {
    fn new(distance: i32) -> Self {
        let mut state = SharedInProcessZoneState::new();
        let keys = [0, 1].map(|index| ZonePresenceKey {
            account_id: format!("aoi-order-{index}"),
            character_index: 0,
        });
        let sessions = [0, 1].map(|index| SessionId::new(format!("aoi-order-{index}")));
        let mut ids = [0; 2];
        for index in 0..2 {
            let name = format!("AoiOrder{index}");
            let position = Point {
                x: 100 + index as i32 * 9,
                y: 100 + index as i32 * distance,
            };
            let map = "live-aoi-order-fixture".to_string();
            ids[index] = state.upsert_player(
                keys[index].clone(),
                &name,
                map.clone(),
                shared_picker_entity(1000, position.x, position.y),
                40,
            );
            state
                .zone_sessions
                .insert(keys[index].clone(), sessions[index].clone());
            state
                .zone_session_keys
                .insert(sessions[index].clone(), keys[index].clone());
            let outbounds = state.zone_manager.join(ZoneJoin {
                session_id: sessions[index].clone(),
                account_id: keys[index].account_id.clone(),
                character_index: 0,
                object_id: ids[index],
                name,
                class: MirClass::Warrior,
                gender: MirGender::Male,
                level: 1,
                hp: 10,
                max_hp: 10,
                mp: 10,
                map_file_name: map,
                position,
                direction: MirDirection::Up,
                chat_profile: ZoneChatProfile::default(),
                combat_stats: ZonePlayerCombatStats::default(),
            });
            state.dispatch_zone_outbounds(outbounds, None);
        }
        let pair = Self {
            shared: Arc::new(Mutex::new(state)),
            keys,
            sessions,
            ids,
        };
        // Prime ordinary Run with an ordinary Walk from standstill. No admin
        // relocation occurs in either of the actual boundary-crossing steps.
        pair.step(0, false, 1000);
        pair.step(1, false, 1000);
        for key in &pair.keys {
            pair.shared.lock().unwrap().take_pending_zone_packets(key);
        }
        pair
    }

    fn live(&self, capacity: usize) -> tokio::sync::mpsc::Receiver<SharedZoneLiveOutbound> {
        let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
        self.shared.lock().unwrap().register_live_zone_outbound(
            self.keys[0].clone(),
            SharedZoneLiveOutboundSender::single(sender),
        );
        receiver
    }

    fn step(&self, index: usize, run: bool, now_ms: u64) -> Vec<ServerPacket> {
        let mut state = self.shared.lock().unwrap();
        let command = if run {
            ZoneCommand::Run {
                session_id: self.sessions[index].clone(),
                direction: MirDirection::Up,
                seq: 2,
                now_ms,
            }
        } else {
            ZoneCommand::Walk {
                session_id: self.sessions[index].clone(),
                direction: MirDirection::Up,
                seq: 1,
                now_ms,
            }
        };
        let mut outbounds = state.zone_manager.handle(command);
        outbounds.extend(state.zone_manager.handle(ZoneCommand::TickPlayerMovement {
            session_id: self.sessions[index].clone(),
            now_ms,
        }));
        // This is the same current-key split used by the reader's real
        // movement ingress. Holding the returned Vec models a preempted socket
        // writer after the authoritative lock has already been released.
        state
            .dispatch_zone_outbounds(outbounds, Some(&self.keys[index]))
            .0
    }

    fn visible(&self) -> bool {
        let state = self.shared.lock().unwrap();
        let first = state
            .zone_manager
            .player_transform(&self.sessions[0])
            .unwrap()
            .0;
        let second = state
            .zone_manager
            .player_transform(&self.sessions[1])
            .unwrap()
            .0;
        (first.x - second.x).abs() <= 16 && (first.y - second.y).abs() <= 16
    }

    fn remote_position(&self) -> Point {
        self.shared
            .lock()
            .unwrap()
            .zone_manager
            .player_transform(&self.sessions[1])
            .unwrap()
            .0
    }
}

fn drain_live(
    receiver: &mut tokio::sync::mpsc::Receiver<SharedZoneLiveOutbound>,
) -> Vec<ServerPacket> {
    let mut result = Vec::new();
    while let Ok(outbound) = receiver.try_recv() {
        result.push(outbound.into_packet());
    }
    result
}

fn project(mut position: Option<Point>, object_id: u32, packets: &[ServerPacket]) -> Option<Point> {
    for packet in packets {
        match packet {
            ServerPacket::ObjectPlayer { info } if info.object_id == object_id => {
                position = Some(info.location.clone())
            }
            ServerPacket::ObjectRemove { object_id: removed } if *removed == object_id => {
                position = None
            }
            ServerPacket::ObjectWalk { movement }
            | ServerPacket::ObjectRun { movement }
            | ServerPacket::ObjectTurn { movement }
                if movement.object_id == object_id && position.is_some() =>
            {
                position = Some(movement.position.clone())
            }
            _ => {}
        }
    }
    position
}

#[test]
fn live_aoi_order_old_direct_remove_cannot_delete_new_live_reentry() {
    let pair = Pair::new(15);
    let mut live = pair.live(32);
    let initial = Some(pair.remote_position());
    let delayed_owner_response = pair.step(0, true, 1750); // 15 -> 17, leaves view.
    assert!(!pair.visible());
    pair.step(1, true, 1750); // 17 -> 15, enters view again.
    assert!(pair.visible());

    // Live sender wins scheduling, then the suspended old reader resumes.
    let packets = drain_live(&mut live)
        .into_iter()
        .chain(delayed_owner_response)
        .collect::<Vec<_>>();
    assert_eq!(
        project(initial, pair.ids[1], &packets),
        Some(pair.remote_position()),
        "authoritative reentry must survive an older reader response: {packets:?}"
    );
}

#[test]
fn live_aoi_order_old_live_create_cannot_resurrect_after_new_direct_remove() {
    let pair = Pair::new(17);
    let mut live = pair.live(32);
    assert!(!pair.visible());
    pair.step(1, true, 1750); // 17 -> 15, queues the first visibility entry.
    assert!(pair.visible());
    let newer_owner_response = pair.step(0, true, 1750); // 15 -> 17, leaves again.
    assert!(!pair.visible());

    // Opposite scheduling: the reader wins while the live sender is delayed.
    let packets = newer_owner_response
        .into_iter()
        .chain(drain_live(&mut live))
        .collect::<Vec<_>>();
    assert_eq!(
        project(None, pair.ids[1], &packets),
        None,
        "a delayed old entry must not resurrect an out-of-range player: {packets:?}"
    );
}

#[test]
fn live_aoi_order_no_live_registration_keeps_synchronous_owner_lifecycle() {
    let pair = Pair::new(15);
    let initial = Some(pair.remote_position());
    let owner = pair.step(0, true, 1750);
    assert!(owner.iter().any(|packet| matches!(packet, ServerPacket::ObjectRemove { object_id } if *object_id == pair.ids[1])));
    let absent = project(initial, pair.ids[1], &owner);
    assert_eq!(absent, None);
    pair.step(1, true, 1750);
    let pending = pair
        .shared
        .lock()
        .unwrap()
        .take_pending_zone_packets(&pair.keys[0]);
    assert_eq!(
        project(absent, pair.ids[1], &pending),
        Some(pair.remote_position())
    );
}

#[test]
fn live_aoi_order_full_channel_must_not_let_new_remove_overtake_pending_create() {
    let pair = Pair::new(17);
    let mut live = pair.live(1);
    let filler = ServerPacket::ObjectTurn {
        movement: ObjectMovement {
            object_id: 9999,
            position: Point { x: 0, y: 0 },
            direction: MirDirection::Up,
        },
    };
    pair.shared
        .lock()
        .unwrap()
        .queue_zone_packets(pair.keys[0].clone(), vec![filler]);
    pair.step(1, true, 1750); // ObjectPlayer waits behind the full channel.
    drain_live(&mut live); // Space opens before the cadence retry.
    let remove = ServerPacket::ObjectRemove {
        object_id: pair.ids[1],
    };
    pair.shared
        .lock()
        .unwrap()
        .queue_zone_packets(pair.keys[0].clone(), vec![remove]);
    let mut delivered = drain_live(&mut live);
    for _ in 0..8 {
        pair.shared
            .lock()
            .unwrap()
            .retry_pending_realtime_zone_outbounds();
        delivered.extend(drain_live(&mut live));
    }
    assert_eq!(
        project(None, pair.ids[1], &delivered),
        None,
        "a later lifecycle event cannot bypass an older full-channel backlog: {delivered:?}"
    );
}

#[test]
fn live_aoi_order_current_transform_family_cannot_overtake_later_player_projection() {
    for kind in 0..3 {
        let pair = Pair::new(17);
        let mut live = pair.live(32);
        pair.step(1, true, 1750);
        let player = drain_live(&mut live)
            .into_iter()
            .find(|packet| matches!(packet, ServerPacket::ObjectPlayer { .. }))
            .unwrap();
        let movement = ObjectMovement {
            object_id: pair.ids[1],
            position: Point { x: 108, y: 114 },
            direction: MirDirection::Right,
        };
        let old = match kind {
            0 => ServerPacket::ObjectWalk { movement },
            1 => ServerPacket::ObjectRun { movement },
            _ => ServerPacket::ObjectTurn { movement },
        };
        let delayed_current = {
            let mut state = pair.shared.lock().unwrap();
            let result = state
                .dispatch_zone_outbounds(
                    vec![ZoneOutbound::ToSession {
                        session_id: pair.sessions[0].clone(),
                        packets: vec![old],
                    }],
                    Some(&pair.keys[0]),
                )
                .0;
            state.queue_zone_packets(pair.keys[0].clone(), vec![player]);
            result
        };
        let delivered = drain_live(&mut live)
            .into_iter()
            .chain(delayed_current)
            .collect::<Vec<_>>();
        assert_eq!(
            project(Some(Point { x: 108, y: 114 }), pair.ids[1], &delivered),
            Some(pair.remote_position()),
            "current ObjectWalk/Run/Turn must share the lifecycle FIFO: {delivered:?}"
        );
    }
}

#[test]
fn live_aoi_order_personal_flush_cannot_bypass_backlog_but_owner_location_has_priority() {
    let pair = Pair::new(15);
    let (normal_sender, mut live) = tokio::sync::mpsc::channel(1);
    let (owner_sender, mut owner_live) = tokio::sync::mpsc::channel(1);
    pair.shared.lock().unwrap().register_live_zone_outbound(
        pair.keys[0].clone(),
        SharedZoneLiveOutboundSender::new(normal_sender, owner_sender),
    );
    let filler = ServerPacket::ObjectTurn {
        movement: ObjectMovement {
            object_id: 9999,
            position: Point { x: 0, y: 0 },
            direction: MirDirection::Up,
        },
    };
    pair.shared
        .lock()
        .unwrap()
        .queue_zone_packets(pair.keys[0].clone(), vec![filler]);
    let owner_response = pair.step(0, true, 1750);
    assert!(
        owner_response.is_empty(),
        "registered owner ACK must have only one delivery path"
    );
    assert!(
        matches!(
            owner_live.try_recv().unwrap().into_packet(),
            ServerPacket::UserLocation { .. }
        ),
        "owner acknowledgement must not wait behind a full observer channel"
    );
    pair.step(1, true, 1750);
    let personal = pair
        .shared
        .lock()
        .unwrap()
        .take_pending_zone_packets(&pair.keys[0]);
    assert!(
        !personal.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectPlayer { .. } | ServerPacket::ObjectRemove { .. }
        )),
        "session flush must not create a second lifecycle delivery path"
    );
    drain_live(&mut live);
    let mut delivered = Vec::new();
    for _ in 0..8 {
        pair.shared
            .lock()
            .unwrap()
            .retry_pending_realtime_zone_outbounds();
        delivered.extend(drain_live(&mut live));
    }
    assert_eq!(
        project(None, pair.ids[1], &delivered),
        Some(pair.remote_position())
    );
}

#[test]
fn live_aoi_order_coalescing_does_not_cross_object_lifetime_boundary() {
    let pair = Pair::new(15);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    drop(receiver);
    let mut state = pair.shared.lock().unwrap();
    state.register_live_zone_outbound(
        pair.keys[0].clone(),
        SharedZoneLiveOutboundSender::single(sender),
    );
    let moved = |x| ServerPacket::ObjectWalk {
        movement: ObjectMovement {
            object_id: pair.ids[1],
            position: Point { x, y: 114 },
            direction: MirDirection::Right,
        },
    };
    state.queue_zone_packets(
        pair.keys[0].clone(),
        vec![
            moved(108),
            ServerPacket::ObjectRemove {
                object_id: pair.ids[1],
            },
            moved(109),
            moved(110),
        ],
    );
    let pending = state.take_pending_zone_packets(&pair.keys[0]);
    drop(state);
    let positions = pending
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::ObjectWalk { movement } => Some(movement.position.x),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        positions,
        vec![108, 110],
        "coalesce only inside one object lifetime"
    );
    assert!(matches!(&pending[1], ServerPacket::ObjectRemove { .. }));
}

#[test]
fn live_aoi_order_replacement_is_fenced_and_closed_channel_preserves_pending_lifecycle() {
    let pair = Pair::new(15);
    let (first_sender, first_receiver) = tokio::sync::mpsc::channel(1);
    let first_id = pair.shared.lock().unwrap().register_live_zone_outbound(
        pair.keys[0].clone(),
        SharedZoneLiveOutboundSender::single(first_sender),
    );
    drop(first_receiver);
    pair.step(0, true, 1750);
    pair.step(1, true, 1750);
    let (second_sender, mut second_receiver) = tokio::sync::mpsc::channel(8);
    let (new_id, still_registered) = {
        let mut state = pair.shared.lock().unwrap();
        let id = state.register_live_zone_outbound(
            pair.keys[0].clone(),
            SharedZoneLiveOutboundSender::single(second_sender),
        );
        state.unregister_live_zone_outbound(&pair.keys[0], first_id);
        state.retry_pending_realtime_zone_outbounds();
        (id, state.live_zone_outbounds.contains_key(&pair.keys[0]))
    };
    assert_ne!(first_id, new_id);
    assert!(still_registered);
    let mut delivered = Vec::new();
    while let Ok(outbound) = second_receiver.try_recv() {
        assert_eq!(outbound.registration_id(), new_id);
        delivered.push(outbound.into_packet());
    }
    assert_eq!(
        project(None, pair.ids[1], &delivered),
        Some(pair.remote_position())
    );
    pair.shared
        .lock()
        .unwrap()
        .begin_teardown_fence(&pair.keys[0])
        .unwrap();
    assert!(!pair
        .shared
        .lock()
        .unwrap()
        .live_zone_outbounds
        .contains_key(&pair.keys[0]));
}

#[test]
fn live_aoi_order_new_map_bootstrap_drops_old_viewport_only_and_stays_synchronous() {
    let pair = Pair::new(15);
    let mut old_live = pair.live(1);
    let mut state = pair.shared.lock().unwrap();
    state.queue_zone_packets(
        pair.keys[0].clone(),
        vec![
            ServerPacket::ObjectTurn {
                movement: ObjectMovement {
                    object_id: pair.ids[1],
                    position: Point { x: 109, y: 114 },
                    direction: MirDirection::Up,
                },
            },
            ServerPacket::ObjectRemove {
                object_id: pair.ids[1],
            },
            ServerPacket::GainExperience { amount: 17 },
        ],
    );
    state.begin_zone_bootstrap(&pair.keys[0]);
    let current = state
        .dispatch_zone_outbounds(
            vec![ZoneOutbound::ToSession {
                session_id: pair.sessions[0].clone(),
                packets: vec![ServerPacket::ObjectPlayer {
                    info: shared_object_player_info(98765, "NewMapPlayer", 30, 40),
                }],
            }],
            Some(&pair.keys[0]),
        )
        .0;
    let pending = state.take_pending_zone_packets(&pair.keys[0]);
    drop(state);
    assert!(current.iter().any(
        |packet| matches!(packet, ServerPacket::ObjectPlayer { info } if info.object_id == 98765)
    ));
    assert!(!pending.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectRemove { .. } | ServerPacket::ObjectTurn { .. }
    )));
    assert!(pending
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainExperience { amount: 17 })));
    assert_eq!(
        drain_live(&mut old_live).len(),
        1,
        "new bootstrap must not send into the obsolete registration"
    );
}

#[test]
fn live_aoi_order_monster_lifecycle_and_personal_rewards_keep_the_existing_direct_path() {
    let pair = Pair::new(15);
    let mut live = pair.live(8);
    let monster_id = 9_910_011;
    let expected = vec![
        ServerPacket::ObjectMonster {
            info: shared_monster_info(monster_id, 0),
        },
        ServerPacket::ObjectRemove {
            object_id: monster_id,
        },
        ServerPacket::GainExperience { amount: 17 },
    ];
    let direct = pair
        .shared
        .lock()
        .unwrap()
        .dispatch_zone_outbounds(
            vec![ZoneOutbound::ToSession {
                session_id: pair.sessions[0].clone(),
                packets: expected.clone(),
            }],
            Some(&pair.keys[0]),
        )
        .0;
    assert_eq!(direct, expected);
    assert!(drain_live(&mut live).is_empty());
}

#[test]
fn live_aoi_order_departed_player_remove_stays_fifo_until_delivered_then_releases_identity() {
    let pair = Pair::new(15);
    let mut live = pair.live(1);
    let filler = ServerPacket::ObjectTurn {
        movement: ObjectMovement {
            object_id: 9999,
            position: Point { x: 0, y: 0 },
            direction: MirDirection::Up,
        },
    };
    let personal = {
        let mut state = pair.shared.lock().unwrap();
        state.queue_zone_packets(pair.keys[0].clone(), vec![filler]);
        let outbounds = state.remove_player(&pair.keys[1]);
        state.dispatch_zone_outbounds(outbounds, Some(&pair.keys[1]));
        state.take_pending_zone_packets(&pair.keys[0])
    };
    assert!(
        personal.is_empty(),
        "a departed player must not be mistaken for a monster and rerouted through direct flush"
    );
    drain_live(&mut live);
    pair.shared
        .lock()
        .unwrap()
        .retry_pending_realtime_zone_outbounds();
    let packets = drain_live(&mut live);
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::ObjectRemove { object_id } if *object_id == pair.ids[1])));
    let retained =
        pair.shared.lock().unwrap().live_zone_player_ids[&pair.keys[0]].contains(&pair.ids[1]);
    assert!(
        !retained,
        "delivered leaves must not retain player identities indefinitely"
    );
}

#[test]
fn live_aoi_order_full_remove_reentry_logout_cannot_lose_pending_player_identity() {
    let pair = Pair::new(15);
    let mut live = pair.live(1);
    pair.shared.lock().unwrap().queue_zone_packets(
        pair.keys[0].clone(),
        vec![ServerPacket::ObjectTurn {
            movement: ObjectMovement {
                object_id: 9999,
                position: Point { x: 0, y: 0 },
                direction: MirDirection::Up,
            },
        }],
    );
    pair.step(0, true, 1750);
    pair.step(1, true, 1750);
    drain_live(&mut live);
    pair.shared
        .lock()
        .unwrap()
        .retry_pending_realtime_zone_outbounds();
    // Only the old Remove fits. The later Player and Run still wait, then
    // the peer really leaves the Zone before either can reach the channel.
    let personal = {
        let mut state = pair.shared.lock().unwrap();
        let outbounds = state.remove_player(&pair.keys[1]);
        state.dispatch_zone_outbounds(outbounds, Some(&pair.keys[1]));
        state.take_pending_zone_packets(&pair.keys[0])
    };
    assert!(
        personal.is_empty(),
        "pending reentry/logout must remain classified as player lifecycle: {personal:?}"
    );
    let mut delivered = drain_live(&mut live);
    for _ in 0..8 {
        pair.shared
            .lock()
            .unwrap()
            .retry_pending_realtime_zone_outbounds();
        delivered.extend(drain_live(&mut live));
    }
    assert_eq!(
        project(Some(Point { x: 109, y: 114 }), pair.ids[1], &delivered),
        None,
        "real logout must remain after pending reentry: {delivered:?}"
    );
    assert!(
        !pair.shared.lock().unwrap().live_zone_player_ids[&pair.keys[0]].contains(&pair.ids[1])
    );
}

#[test]
fn live_aoi_order_logout_outside_view_releases_known_identity_without_another_remove() {
    let pair = Pair::new(15);
    let mut live = pair.live(8);
    pair.step(0, true, 1750);
    assert!(!pair.visible());
    assert!(drain_live(&mut live).iter().any(|packet| matches!(packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == pair.ids[1])));
    let retained = {
        let mut state = pair.shared.lock().unwrap();
        let outbounds = state.remove_player(&pair.keys[1]);
        state.dispatch_zone_outbounds(outbounds, Some(&pair.keys[1]));
        state.forget_zone_session(&pair.keys[1]);
        state.live_zone_player_ids[&pair.keys[0]].contains(&pair.ids[1])
    };
    assert!(
        drain_live(&mut live).is_empty(),
        "already-outside peer has no second AOI Remove"
    );
    assert!(
        !retained,
        "idle registered recipients cannot accumulate departed IDs across churn"
    );
}

#[test]
fn live_aoi_order_wss_overflow_stops_broken_epoch_preserves_personal_state_and_fences_replacement()
{
    let pair = Pair::new(15);
    let (sender, mut live) = tokio::sync::mpsc::channel(1);
    let (signal, observed) = tokio::sync::watch::channel(0);
    let transport = SharedZoneLiveOutboundSender::single(sender).with_overload_signal(signal);
    let registration = pair
        .shared
        .lock()
        .unwrap()
        .register_live_zone_outbound(pair.keys[0].clone(), transport.clone());
    let personal = vec![
        ServerPacket::GainExperience { amount: 17 },
        ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 1000,
                percent: 87,
                expire: 5,
            },
        },
    ];
    {
        let mut state = pair.shared.lock().unwrap();
        state.queue_zone_packets(
            pair.keys[0].clone(),
            vec![ServerPacket::ObjectPlayer {
                info: shared_object_player_info(pair.ids[1], "Peer", 109, 114),
            }],
        );
        state.queue_zone_packets(pair.keys[0].clone(), personal.clone());
        for index in 0..crate::routing::MAX_PENDING_ZONE_PACKETS_PER_PLAYER + 20 {
            state.queue_zone_packets(
                pair.keys[0].clone(),
                vec![ServerPacket::ObjectPlayer {
                    info: shared_object_player_info(70_000 + index as u32, "PastPeer", 109, 114),
                }],
            );
        }
        assert!(state.live_zone_outbounds[&pair.keys[0]]
            .overloaded
            .load(Ordering::Acquire));
        assert!(
            state.teardown_fences.is_empty(),
            "a slow viewer must not pause all Zone ticks"
        );
        assert_eq!(state.pending_zone_packets[&pair.keys[0]], personal);
    }
    assert_eq!(*observed.borrow(), registration);
    let queued_before_overflow = live.try_recv().unwrap();
    assert!(
        queued_before_overflow.is_overloaded(),
        "already-channelled projection is poisoned too"
    );
    assert_eq!(queued_before_overflow.registration_id(), registration);

    // The authoritative personal receipt is still available for ordinary
    // teardown, while fresh projection waits for a rebuilt registration.
    let (replacement_sender, mut replacement) = tokio::sync::mpsc::channel(4);
    let (new_id, receipts) = {
        let mut state = pair.shared.lock().unwrap();
        let receipts = state.take_pending_zone_packets(&pair.keys[0]);
        let next = state.register_live_zone_outbound(
            pair.keys[0].clone(),
            SharedZoneLiveOutboundSender::single(replacement_sender)
                .with_overload_signal(transport.overload_signal.clone().unwrap()),
        );
        state.unregister_live_zone_outbound(&pair.keys[0], registration);
        state.queue_zone_packets(
            pair.keys[0].clone(),
            vec![ServerPacket::ObjectPlayer {
                info: shared_object_player_info(pair.ids[1], "Peer", 109, 114),
            }],
        );
        (next, receipts)
    };
    assert_eq!(receipts, personal);
    assert_ne!(new_id, registration);
    let fresh = replacement.try_recv().unwrap();
    assert!(!fresh.is_overloaded());
    assert_eq!(fresh.registration_id(), new_id);
    assert_eq!(
        *observed.borrow(),
        registration,
        "old signal stays old, never relabel it for the new epoch"
    );
}

use super::*;
use crate::routing::ZoneOwnerLease;

#[test]
fn transport_teardown_retires_queued_movement_before_thaw() {
    let (zone_state, mut runtime) = prepared_runtime();
    let movement_state = Arc::clone(&runtime.movement_ingress.session_state);
    let old_epoch = movement_state.lock().unwrap().presence_epoch;
    let identity = runtime.inner.active_identity().unwrap();
    let key = ZonePresenceKey::from_identity(&identity);
    let (queue_tx, queue_rx) = std::sync::mpsc::sync_channel(1);
    let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
    queue_tx.send(crate::routing::SharedZoneMovementRequest {
        packet: ClientPacket::Turn { direction: MirDirection::UpRight },
        expected_presence_epoch: old_epoch,
        session_state: Arc::clone(&movement_state),
        response_sender: reply_tx,
    }).unwrap();
    // A timed-out reply receiver leaves the real request in the owner queue.
    drop(reply_rx);
    let owner_lease = ZoneOwnerLease::in_process(&ZoneId::primary());
    let saved = runtime.prepare_teardown_checkpoint(&owner_lease).unwrap().unwrap();
    runtime.release_teardown_fence().unwrap();
    assert!(!runtime.teardown_is_fenced());
    assert_eq!(movement_state.lock().unwrap().presence_key.as_ref(), Some(&key));
    let request = queue_rx.recv().unwrap();
    let stale = crate::routing::execute_shared_zone_movement(
        &zone_state, &request.session_state, &request.packet,
        Some(request.expected_presence_epoch), false, false,
    ).unwrap();
    assert!(stale.is_none(), "an old request must remain rejected after thaw");
    {
        let state = zone_state.lock().unwrap();
        let session_id = &state.zone_sessions[&key];
        assert_eq!(state.zone_manager.player_transform(session_id),
            Some((saved.checkpoint().position.clone(), saved.checkpoint().direction)));
    }
    let new_epoch = movement_state.lock().unwrap().presence_epoch;
    assert!(new_epoch > old_epoch);
    let fresh = crate::routing::execute_shared_zone_movement(
        &zone_state, &movement_state, &request.packet, Some(new_epoch), false, false,
    ).unwrap();
    assert!(fresh.is_some(), "the resumed presence must still accept new movement");
    let state = zone_state.lock().unwrap();
    let session_id = &state.zone_sessions[&key];
    assert_eq!(state.zone_manager.player_transform(session_id).unwrap().1, MirDirection::UpRight);
}

#[test]
fn transport_teardown_epoch_exhaustion_stays_fenced() {
    let (_zone_state, mut runtime) = prepared_runtime();
    runtime.movement_ingress.session_state.lock().unwrap().presence_epoch = u64::MAX;
    let owner_lease = ZoneOwnerLease::in_process(&ZoneId::primary());
    let error = runtime.prepare_teardown_checkpoint(&owner_lease).unwrap_err();
    assert_eq!(error, "shared movement presence epoch exhausted");
    assert!(runtime.teardown_is_fenced());
}

#[test]
fn transport_teardown_discards_clock_queued_walk_without_losing_completed_position() {
    let (zone_state, mut runtime) = prepared_runtime();
    let identity = runtime.inner.active_identity().unwrap();
    let key = ZonePresenceKey::from_identity(&identity);
    let completed_transform = {
        let mut state = zone_state.lock().unwrap();
        let session_id = state.zone_sessions[&key].clone();
        let before = state.zone_manager.player_transform(&session_id).unwrap();
        let now_ms = crate::routing::shared_gateway_now_ms();
        state.zone_manager.handle(ZoneCommand::Walk {
            session_id: session_id.clone(), direction: MirDirection::Right, seq: 1, now_ms,
        });
        let completed = state.zone_manager.player_transform(&session_id).unwrap();
        assert_eq!(completed.0, Point { x: before.0.x + 1, y: before.0.y });
        state.zone_manager.handle(ZoneCommand::Walk {
            session_id, direction: MirDirection::Right, seq: 2, now_ms,
        });
        assert!(state.zone_manager.next_pending_movement_deadline_ms().is_some(),
            "the second step must really be inside the canonical action-clock queue");
        completed
    };
    let owner_lease = ZoneOwnerLease::in_process(&ZoneId::primary());
    let saved = runtime.prepare_teardown_checkpoint(&owner_lease).unwrap().unwrap();
    assert_eq!(saved.checkpoint().position, completed_transform.0);
    runtime.release_teardown_fence().unwrap();
    let mut state = zone_state.lock().unwrap();
    assert!(state.zone_manager.next_pending_movement_deadline_ms().is_none());
    state.zone_manager.tick_pending_movement(crate::routing::shared_gateway_now_ms() + 10_000);
    let session_id = &state.zone_sessions[&key];
    assert_eq!(state.zone_manager.player_transform(session_id), Some(completed_transform),
        "without a new request, thaw must not execute an old queued step");
}

fn prepared_runtime() -> (
    Arc<Mutex<SharedInProcessZoneState>>,
    SharedInProcessZoneSessionRuntime,
) {
    let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(Arc::clone(&zone_state));
    start_demo_runtime(&mut runtime);
    (zone_state, runtime)
}

#[test]
fn abrupt_tcp_and_web_teardown_drain_authoritative_state_and_economy_once() {
    let (zone_state, mut runtime) = prepared_runtime();
    let before = runtime
        .inner
        .active_character_checkpoint()
        .expect("started runtime has checkpoint");
    let identity = runtime.inner.active_identity().expect("active identity");
    let key = ZonePresenceKey::from_identity(&identity);
    let final_position = Point {
        x: before.position.x + 2,
        y: before.position.y + 1,
    };
    let final_direction = MirDirection::UpRight;
    let final_hp = (before.hp - 4).max(1);
    let final_mp = (before.mp - 1).max(0);
    let ground_drop = shared_gold_drop(0x7f00_0101, final_position.x, final_position.y, None, None);
    let issued_award = cross_map_drop_lifecycle_tests::issue_prepared_award(
        &mut runtime,
        ZoneMonsterKillAward {
            source_receipt_key: None,
            mentor_bank: None, experience_selection: None,
            monster_object_id: 0x7f00_0010,
            killed_at_ms: 123_456,
            monster_name: "Teardown Deer".to_string(),
            experience: 11,
            drops: Vec::new(),
            boss_audit: None,
        },
    ).pop().expect("positive source death issues an owned award");

    {
        let mut state = zone_state.lock().expect("zone state");
        let session_id = state.zone_sessions[&key].clone();
        let _ = state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
            session_id: session_id.clone(),
            position: final_position.clone(),
            direction: final_direction,
        });
        let _ = state.zone_manager.handle(ZoneCommand::SyncPlayerVitals {
            session_id: session_id.clone(),
            hp: final_hp,
            max_hp: before.max_hp,
            mp: final_mp,
        });
        state.pending_zone_transforms.insert(
            key.clone(),
            (
                Point {
                    x: before.position.x + 1,
                    y: before.position.y,
                },
                MirDirection::Right,
            ),
        );
        state.queue_zone_shout_consume(key.clone(), true, true);
        state.queue_zone_player_damage(key.clone(), 7);
        state.queue_zone_player_heal(key.clone(), 3);
        state.queue_zone_monster_kill_award(
            key.clone(),
            issued_award,
        );
        state.queue_zone_ground_drop_claim(
            key.clone(),
            GroundDropClaimTicket {
                claim_id: 1,
                object_id: ground_drop.object_id,
                drop_generation: 1,
                payload_digest: "teardown-ground-drop-payload".to_string(),
                idempotency_key: "teardown-ground-drop-claim".to_string(),
                session_id,
                owner_object_id: ground_drop.owner_object_id,
                drop: ground_drop,
            },
        );
    }

    let owner_lease = ZoneOwnerLease::in_process(&ZoneId::primary());
    let prepared = runtime
        .prepare_teardown_checkpoint(&owner_lease)
        .expect("teardown drain")
        .expect("active checkpoint");
    assert_eq!(prepared.owner_lease(), &owner_lease);
    let checkpoint = prepared.checkpoint();
    assert_eq!(checkpoint.position, final_position);
    assert_eq!(checkpoint.direction, final_direction);
    assert_eq!(checkpoint.hp, final_hp);
    assert_eq!(checkpoint.mp, final_mp);
    assert_eq!(checkpoint.experience, before.experience + 11);
    assert_eq!(checkpoint.gold, before.gold + 25);

    let state = zone_state.lock().expect("zone state");
    assert!(state.teardown_fenced(&key));
    assert!(!state.pending_zone_packets.contains_key(&key));
    assert!(!state.pending_zone_transforms.contains_key(&key));
    assert!(!state.pending_zone_shout_consumes.contains_key(&key));
    assert!(!state.pending_zone_player_damages.contains_key(&key));
    assert!(!state.pending_zone_player_heals.contains_key(&key));
    assert!(!state.pending_zone_monster_kill_awards.contains_key(&key));
    assert!(!state.pending_zone_ground_drop_claims.contains_key(&key));
    drop(state);

    assert!(runtime.execute(WorldCommand::Tick).is_err());
    runtime.release_teardown_fence().expect("saved resume thaw");
    assert!(!zone_state.lock().expect("zone state").teardown_fenced(&key));
}

#[test]
fn abrupt_teardown_rolls_back_unmatched_debited_trade_before_checkpoint() {
    let (zone_state, mut runtime) = prepared_runtime();
    let identity = runtime.inner.active_identity().expect("active identity");
    let key = ZonePresenceKey::from_identity(&identity);
    let starting_gold = runtime.inner.world_snapshot().gold;

    runtime.inner.trade_request("MissingPartner");
    runtime
        .inner
        .execute(WorldCommand::ClientPacket(ClientPacket::TradeReply {
            accept_invite: true,
        }))
        .expect("accept fixture trade");
    runtime
        .inner
        .execute(WorldCommand::ClientPacket(ClientPacket::TradeGold {
            amount: 25,
        }))
        .expect("offer fixture gold");
    let (_, offer) = runtime.inner.shared_trade_confirm();
    let offer = offer.expect("completed unmatched offer");
    assert_eq!(runtime.inner.world_snapshot().gold, starting_gold - 25);
    zone_state
        .lock()
        .expect("zone state")
        .trade_offers
        .insert(key.clone(), offer);

    let owner_lease = ZoneOwnerLease::in_process(&ZoneId::primary());
    let prepared = runtime
        .prepare_teardown_checkpoint(&owner_lease)
        .expect("teardown drain")
        .expect("active checkpoint");

    assert_eq!(prepared.checkpoint().gold, starting_gold);
    assert!(!zone_state
        .lock()
        .expect("zone state")
        .trade_offers
        .contains_key(&key));
}

#[test]
fn web_mail_refresh_cannot_overwrite_zone_vitals_before_teardown_drain() {
    let (zone_state, mut runtime) = prepared_runtime();
    let identity = runtime.inner.active_identity().expect("active identity");
    let key = ZonePresenceKey::from_identity(&identity);
    let before = runtime
        .inner
        .active_character_checkpoint()
        .expect("started runtime has checkpoint");
    let zone_hp = (before.hp - 6).max(1);
    let zone_mp = (before.mp - 2).max(0);
    let session_id = {
        let mut state = zone_state.lock().expect("zone state");
        let session_id = state.zone_sessions[&key].clone();
        let _ = state.zone_manager.handle(ZoneCommand::SyncPlayerVitals {
            session_id: session_id.clone(),
            hp: zone_hp,
            max_hp: before.max_hp,
            mp: zone_mp,
        });
        session_id
    };

    runtime
        .inner
        .force_authoritative_player_vitals(Some(1), Some(0));
    let _ = runtime.refresh_active_external_mail();
    assert_eq!(
        zone_state
            .lock()
            .expect("zone state")
            .zone_manager
            .player_vitals(&session_id),
        Some((zone_hp, before.max_hp, zone_mp))
    );

    let owner_lease = ZoneOwnerLease::in_process(&ZoneId::primary());
    let prepared = runtime
        .prepare_teardown_checkpoint(&owner_lease)
        .expect("teardown drain")
        .expect("active checkpoint");
    let checkpoint = prepared.checkpoint();
    assert_eq!((checkpoint.hp, checkpoint.mp), (zone_hp, zone_mp));
}

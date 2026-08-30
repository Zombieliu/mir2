use super::*;
use crate::routing::ZoneOwnerLease;

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
            ZoneMonsterKillAward {
                monster_object_id: 0x7f00_0010,
                killed_at_ms: 123_456,
                monster_name: "Teardown Deer".to_string(),
                experience: 11,
                drops: Vec::new(),
                boss_audit: None,
            },
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

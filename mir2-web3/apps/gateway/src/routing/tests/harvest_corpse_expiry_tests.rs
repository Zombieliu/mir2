//! Corpse expiry is a global removal even while its native respawn slot survives.
use super::*;

fn exercise_corpse_expiry(empty_zone: bool) {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_demo_runtime(&mut runtime);
    let key = runtime.current_presence_key().unwrap();
    let sid = runtime.current_zone_session_id().unwrap();
    let mut join = runtime
        .inner
        .active_zone_join_snapshot(sid.as_str())
        .unwrap();
    join.position = Point { x: 330, y: 270 };
    join.combat_stats = mir2_simulation::ZonePlayerCombatStats {
        min_dc: 99,
        max_dc: 99,
        accuracy: 100,
        ..Default::default()
    };
    let id = 9_700_109;
    let spawn = ZoneMonsterSpawn {
        object_id: id,
        name: "Deer".into(),
        name_colour_argb: -1,
        image: 2,
        ai: 2,
        disposition: Some(mir2_simulation::WorldEntityDisposition::Neutral),
        level: 1,
        max_hp: 20,
        hp: 20,
        experience: 0,
        move_speed_ms: 500,
        attack_speed_ms: 1000,
        friendly_guild: None,
        defense: Default::default(),
        position: Point { x: 331, y: 270 },
        direction: MirDirection::Left,
        respawn: Some(mir2_simulation::ZoneMonsterRespawnPolicy {
            minimum_delay_ms: 0,
            base_delay_ms: 300_000,
            random_delay_step_ms: 0,
            random_delay_steps: 1,
            random_delay_subtract_steps: 0,
            rule_index: 7,
            slot_index: 3,
        }),
        drops: Vec::new(),
    };
    let mut state = shared.lock().unwrap();
    join.object_id = state.players[&key].zone_object_id;
    state.zone_manager = ZoneManager::new();
    state.zone_manager.handle(ZoneCommand::Join(join.clone()));
    state
        .zone_manager
        .handle(ZoneCommand::sync_player_combat_state(
            sid.clone(),
            MirClass::Warrior,
            true,
            false,
            true,
            false,
            false,
            false,
        ));
    let out = state.zone_manager.handle(ZoneCommand::SpawnMonster {
        session_id: sid.clone(),
        monster: spawn.clone(),
        now_ms: 0,
    });
    state.dispatch_zone_outbounds(out, Some(&key));
    state.zone_manager.handle(ZoneCommand::PlayerAttackObject {
        session_id: sid.clone(),
        object_id: id,
        direction: MirDirection::Right,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 99,
        now_ms: 10,
    });
    let out = state.zone_manager.tick_all(10);
    let (packets, ..) = state.dispatch_zone_outbounds(out, Some(&key));
    assert!(packets
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==id)));
    let corpse = state.maps["0"]
        .entities
        .get(&id)
        .expect("real death enters shared map projection")
        .clone();
    assert!(corpse.dead);
    if empty_zone {
        // Retain the map projection while its only real Zone session leaves.
        state.zone_manager.handle(ZoneCommand::Leave {
            session_id: sid.clone(),
        });
        assert!(state.zone_manager.player_transform(&sid).is_none());
        assert!(state.maps["0"].entities.contains_key(&id));
    }
    let out = state.zone_manager.tick_all(180_010);
    let (packets, ..) = state.dispatch_zone_outbounds(out, Some(&key));
    if empty_zone {
        assert!(
            !packets
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectRemove{object_id}if *object_id==id)),
            "no recipient exists to carry the corpse's retirement packet"
        );
        assert!(
            state.maps["0"].entities.contains_key(&id),
            "fixture preserves the stale corpse until reconciliation"
        );
        state.sync_map_layer(
            "0".into(),
            vec![corpse.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(!state.maps["0"].entities.contains_key(&id),"first personal sync must observe authoritative retirement even without an ObjectRemove recipient");
        state.zone_manager.handle(ZoneCommand::Join(join));
    } else {
        assert!(packets
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectRemove{object_id}if *object_id==id)));
        state.apply_zone_packets_to_map_layer(&key, &packets);
        assert!(!state.maps["0"].entities.contains_key(&id));
    }
    assert!(state
        .zone_manager
        .native_monster_snapshots(&ZoneKey::for_map("0"))
        .iter()
        .any(|m| m.object_id == id && m.dead));
    state.sync_map_layer(
        "0".into(),
        vec![corpse],
        BTreeSet::new(),
        Vec::new(),
        BTreeSet::new(),
    );
    let out = state.zone_manager.handle(ZoneCommand::SyncNativeMonsters {
        session_id: sid,
        monsters: vec![spawn],
        now_ms: 180_011,
    });
    state.dispatch_zone_outbounds(out, Some(&key));
    assert!(
        !state.maps["0"].entities.contains_key(&id),
        "stale personal corpse/native sync cannot flow back into the map"
    );
    drop(state);
    assert!(!runtime
        .world_snapshot()
        .entities
        .iter()
        .any(|e| e.object_id == id));
    let mut state = shared.lock().unwrap();
    let out = state.zone_manager.tick_all(300_010);
    let (packets, ..) = state.dispatch_zone_outbounds(out, Some(&key));
    assert!(packets
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRevived{info}if info.object_id==id)));
    state.apply_zone_packets_to_map_layer(&key, &packets);
    assert!(state.maps["0"].entities.get(&id).is_some_and(|e| !e.dead));
    drop(state);
    assert!(runtime
        .world_snapshot()
        .entities
        .iter()
        .any(|e| e.object_id == id && !e.dead));
}

#[test]
fn expired_shared_harvest_corpse_leaves_gateway_projection_until_scheduled_revive() {
    exercise_corpse_expiry(false);
}
#[test]
fn unobserved_harvest_corpse_expiry_is_reconciled_without_a_remove_recipient() {
    exercise_corpse_expiry(true);
}

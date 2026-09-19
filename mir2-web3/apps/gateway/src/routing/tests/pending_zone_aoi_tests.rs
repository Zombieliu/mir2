use super::*;

fn object_monster_count(packets: &[ServerPacket], object_id: u32) -> usize {
    packets
        .iter()
        .filter(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMonster { info }
                    if info.object_id == object_id && !info.dead
            )
        })
        .count()
}

#[test]
fn town_revive_discards_stale_queued_spawn_and_reentering_aoi_gets_one_fresh_spawn() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_demo_runtime(&mut runtime);
    let key = runtime.current_presence_key().expect("presence should exist");
    let session_id = runtime
        .current_zone_session_id()
        .expect("Zone session should exist");
    let bind = runtime
        .inner
        .active_zone_join_snapshot(session_id.as_str())
        .expect("active player should have a bind transform")
        .position;
    let requested_field = Point {
        x: bind.x.saturating_add(60),
        y: bind.y,
    };
    runtime
        .execute(WorldCommand::ApplyHandoffTransform {
            position: requested_field,
            direction: MirDirection::Right,
            hp: None,
            mp: None,
        })
        .expect("fixture should move both authorities to the field");
    let field = shared
        .lock()
        .expect("shared state should lock")
        .zone_manager
        .player_transform(&session_id)
        .map(|(position, _)| position)
        .expect("Zone should expose the field transform");

    let object_id = 9_880_025;
    let monster_position = Point {
        x: field.x.saturating_add(1),
        y: field.y,
    };
    let spawn = ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id,
        name: "AoiRespawnProbe".to_string(),
        name_colour_argb: -1,
        image: 10,
        ai: 0,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 7,
        max_hp: 28,
        hp: 28,
        experience: 0,
        move_speed_ms: 0,
        attack_speed_ms: 1_000,
        friendly_guild: None,
        defense: Default::default(),
        position: monster_position,
        direction: MirDirection::Left,
        respawn: None,
        drops: Vec::new(),
    };
    let initial_packets = {
        let mut state = shared.lock().expect("shared state should lock");
        let outbounds = state.zone_manager.handle(ZoneCommand::SpawnMonster {
            session_id: session_id.clone(),
            monster: spawn.clone(),
            now_ms: shared_gateway_now_ms(),
        });
        state.dispatch_zone_outbounds(outbounds, Some(&key)).0
    };
    assert_eq!(object_monster_count(&initial_packets, object_id), 1);
    assert_eq!(
        shared
            .lock()
            .expect("shared state should lock")
            .zone_manager
            .player_has_visible_object(&session_id, object_id),
        Some(true)
    );

    let stale_spawn = zone_monster_spawn_packet(&spawn);
    {
        let mut state = shared.lock().expect("shared state should lock");
        // Reproduce an observer projection selected for the death-field AOI
        // and still pending when TownRevive begins.
        state.queue_zone_packets(
            key.clone(),
            vec![
                stale_spawn.clone(),
                ServerPacket::ObjectRemove { object_id },
            ],
        );
    }

    runtime
        .inner
        .execute(WorldCommand::ClientPacket(ClientPacket::Chat {
            message: "@DIE".to_string(),
            linked_items: Vec::new(),
        }))
        .expect("private death fixture should execute");
    let revive_packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
        .expect("TownRevive should execute through the Gateway");
    assert!(revive_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Revived)));
    assert_eq!(object_monster_count(&revive_packets, object_id), 0);
    assert!(revive_packets.iter().any(
        |packet| matches!(packet, ServerPacket::ObjectRemove { object_id: id } if *id == object_id)
    ));
    assert_eq!(
        shared
            .lock()
            .expect("shared state should lock")
            .zone_manager
            .player_transform(&session_id)
            .map(|(position, _)| position),
        Some(bind.clone()),
        "TownRevive must move the authoritative Zone player, not only the private mirror"
    );
    assert_eq!(
        shared
            .lock()
            .expect("shared state should lock")
            .zone_manager
            .player_has_visible_object(&session_id, object_id),
        Some(false)
    );
    assert!(!runtime
        .inner
        .world_snapshot()
        .entities
        .iter()
        .any(|entity| entity.object_id == object_id));

    let returned_packets = {
        let mut state = shared.lock().expect("shared state should lock");
        let outbounds = state
            .zone_manager
            .handle(ZoneCommand::SyncPlayerTransform {
                session_id: session_id.clone(),
                position: field,
                direction: MirDirection::Right,
            });
        state.dispatch_zone_outbounds(outbounds, Some(&key)).0
    };
    assert_eq!(
        object_monster_count(&returned_packets, object_id),
        1,
        "returning to the AOI must emit one fresh authoritative projection"
    );
    assert_eq!(
        object_monster_count(&runtime.apply_pending_zone_packets(), object_id),
        0,
        "the fresh re-entry projection must not be duplicated in the pending queue"
    );

    {
        shared
            .lock()
            .expect("shared state should lock")
            .queue_zone_packets(key, vec![stale_spawn]);
    }
    assert_eq!(
        object_monster_count(&runtime.apply_pending_zone_packets(), object_id),
        1,
        "a currently visible queued projection must remain unchanged"
    );
}

#[test]
fn authoritative_zone_death_discards_queued_live_spawn_and_positive_health() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_demo_runtime(&mut runtime);
    let key = runtime.current_presence_key().expect("presence should exist");
    let session_id = runtime
        .current_zone_session_id()
        .expect("Zone session should exist");
    let bind = runtime
        .inner
        .active_zone_join_snapshot(session_id.as_str())
        .expect("active player should have a bind transform")
        .position;
    let field = Point {
        x: bind.x.saturating_add(60),
        y: bind.y,
    };
    runtime
        .execute(WorldCommand::ApplyHandoffTransform {
            position: field.clone(),
            direction: MirDirection::Right,
            hp: None,
            mp: None,
        })
        .expect("fixture should move both authorities to the field");
    assert!(runtime
        .sync_authoritative_zone_combat_state(&session_id)
        .is_some());

    let object_id = 9_880_026;
    let spawn = ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id,
        name: "PendingDeathProbe".to_string(),
        name_colour_argb: -1,
        image: 10,
        ai: 0,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 7,
        max_hp: 28,
        hp: 28,
        experience: 0,
        move_speed_ms: 0,
        attack_speed_ms: 1_000,
        friendly_guild: None,
        defense: Default::default(),
        position: Point {
            x: field.x.saturating_add(1),
            y: field.y,
        },
        direction: MirDirection::Left,
        respawn: None,
        drops: Vec::new(),
    };
    let death_now_ms = shared_gateway_now_ms();
    let death_packets = {
        let mut state = shared.lock().expect("shared state should lock");
        let mut outbounds = state.zone_manager.handle(ZoneCommand::SpawnMonster {
            session_id: session_id.clone(),
            monster: spawn.clone(),
            now_ms: death_now_ms,
        });
        outbounds.extend(state.zone_manager.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: session_id.clone(),
            stats: mir2_simulation::ZonePlayerCombatStats {
                min_dc: 10_000,
                max_dc: 10_000,
                accuracy: 100,
                ..Default::default()
            },
        }));
        outbounds.extend(state.zone_manager.handle(ZoneCommand::PlayerAttackObject {
            session_id: session_id.clone(),
            object_id,
            direction: MirDirection::Right,
            spell: 0,
            level: 0,
            attack_type: 0,
            damage: 10_000,
            now_ms: death_now_ms,
        }));
        outbounds.extend(state.zone_manager.tick_all(death_now_ms));
        state.dispatch_zone_outbounds(outbounds, Some(&key)).0
    };
    assert!(death_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == object_id
    )));
    assert_eq!(
        shared
            .lock()
            .expect("shared state should lock")
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .and_then(|zone| zone.native_monster_is_alive(object_id)),
        Some(false),
        "the queued projection must be fenced by the authoritative Zone corpse"
    );

    {
        let mut state = shared.lock().expect("shared state should lock");
        state.queue_zone_packets(
            key.clone(),
            vec![
                zone_monster_spawn_packet(&spawn),
                ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id,
                        percent: 100,
                        expire: 0,
                    },
                },
            ],
        );
    }
    let drained = runtime.apply_pending_zone_packets();
    assert!(!drained.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == object_id && !info.dead
    )));
    assert!(!drained.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == object_id && info.percent > 0
    )));

    // A real Zone respawn is a new incarnation. The lifecycle signal and its
    // live projection must survive the filter, while a queued old corpse must
    // not kill the newly admitted object.
    let revived_packets = {
        let mut state = shared.lock().expect("shared state should lock");
        let outbounds = state.zone_manager.handle(ZoneCommand::SpawnMonster {
            session_id: session_id.clone(),
            monster: spawn.clone(),
            now_ms: death_now_ms.saturating_add(1),
        });
        state.dispatch_zone_outbounds(outbounds, Some(&key)).0
    };
    assert!(revived_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectRevived { info } if info.object_id == object_id
    )));
    assert_eq!(object_monster_count(&revived_packets, object_id), 1);
    assert_eq!(
        shared
            .lock()
            .expect("shared state should lock")
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .and_then(|zone| zone.native_monster_is_alive(object_id)),
        Some(true)
    );

    let mut stale_corpse = zone_monster_spawn_packet(&spawn);
    if let ServerPacket::ObjectMonster { info } = &mut stale_corpse {
        info.dead = true;
    }
    {
        let mut state = shared.lock().expect("shared state should lock");
        state.queue_zone_packets(
            key,
            vec![stale_corpse, zone_monster_spawn_packet(&spawn)],
        );
    }
    let after_revive = runtime.apply_pending_zone_packets();
    assert_eq!(object_monster_count(&after_revive, object_id), 1);
    assert!(!after_revive.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == object_id && info.dead
    )));
}

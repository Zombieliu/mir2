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

use super::*;

#[test]
fn random_teleport_item_commits_its_private_transform_to_the_shared_zone() {
    let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(zone_state.clone());
    start_demo_runtime(&mut runtime);
    runtime
        .execute(WorldCommand::Stage5Command {
            action: "qa.giveItem".to_string(),
            args: vec!["crystal-item-717".to_string()],
        })
        .expect("RandomTeleport fixture should be granted");

    let before = runtime
        .inner
        .active_zone_join_snapshot("before".to_string())
        .expect("started runtime should expose a player")
        .position;
    let unique_id = runtime
        .world_snapshot()
        .inventory_items
        .iter()
        .find(|item| item.key == "crystal-item-717")
        .expect("RandomTeleport should exist")
        .unique_id;
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id,
            grid: MirGridType::Inventory,
        }))
        .expect("RandomTeleport should execute");
    let after = runtime
        .inner
        .active_zone_join_snapshot("after".to_string())
        .expect("teleported runtime should expose a player")
        .position;
    let zone_position = runtime
        .current_zone_session_id()
        .and_then(|session_id| {
            zone_state
                .lock()
                .expect("Zone state should lock")
                .zone_manager
                .player_transform(&session_id)
                .map(|transform| transform.0)
        })
        .expect("shared Zone should retain the player transform");

    assert_ne!(after, before);
    assert_eq!(zone_position, after);
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UseItem { unique_id: id, success: true, .. } if *id == unique_id
    )));
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UserLocation { location } if location.position == after
    )));
}

#[test]
fn normal_mana_potion_ticks_commit_personal_recovery_to_the_shared_zone() {
    let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(zone_state.clone());
    start_demo_runtime(&mut runtime);
    runtime
        .execute(WorldCommand::Stage5Command {
            action: "qa.giveItem".to_string(),
            args: vec!["crystal-item-659".to_string()],
        })
        .expect("MP potion fixture should be granted");

    let snapshot = runtime.inner.world_snapshot();
    let position = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .map(|entity| Point {
            x: entity.x,
            y: entity.y,
        })
        .expect("started runtime should expose a player");
    runtime
        .execute(WorldCommand::ApplyHandoffTransform {
            position,
            direction: MirDirection::Down,
            hp: snapshot.player_hp,
            mp: Some(0),
        })
        .expect("fixture should empty both personal and Zone mana");

    let unique_id = runtime
        .world_snapshot()
        .inventory_items
        .iter()
        .find(|item| item.key == "crystal-item-659")
        .expect("MP potion should exist")
        .unique_id;
    let use_packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id,
            grid: MirGridType::Inventory,
        }))
        .expect("MP potion should queue its timed recovery");
    assert!(use_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UseItem { unique_id: id, success: true, .. } if *id == unique_id
    )));

    let tick_packets = runtime
        .execute(WorldCommand::Tick)
        .expect("personal recovery tick should execute");
    let owner_object_id = runtime.local_self_object_id().expect("owner object id");
    assert!(tick_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMana { info }
            if info.object_id == owner_object_id && info.percent > 0
    )));
    let session_id = runtime
        .current_zone_session_id()
        .expect("started runtime should retain a Zone session");
    let restored_mp = zone_state
        .lock()
        .expect("Zone state should lock")
        .zone_manager
        .player_vitals(&session_id)
        .expect("Zone should expose player vitals")
        .2;
    assert!(restored_mp > 0);
    assert_eq!(runtime.inner.world_snapshot().player_mp, Some(restored_mp));
    assert_eq!(runtime.world_snapshot().player_mp, Some(restored_mp));

    runtime
        .execute(WorldCommand::Tick)
        .expect("following tick should preserve the committed recovery");
    let next_mp = zone_state
        .lock()
        .expect("Zone state should lock")
        .zone_manager
        .player_vitals(&session_id)
        .expect("Zone should retain player vitals")
        .2;
    assert!(next_mp >= restored_mp);
    assert_eq!(runtime.inner.world_snapshot().player_mp, Some(next_mp));
    assert_eq!(runtime.world_snapshot().player_mp, Some(next_mp));
}

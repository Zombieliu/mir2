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

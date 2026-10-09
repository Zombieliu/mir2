use super::*;

#[test]
fn zone_deer_death_never_materializes_harvest_drops() {
    let deer = crystal_monster_by_name("Deer").expect("bundled Crystal Deer template");
    assert!(initial_harvest_monster_state(deer.ai).is_some());

    let configured =
        crystal_drop_table_for_monster_name("Deer").expect("bundled Crystal Deer drop table");
    assert!(
        configured
            .sections
            .iter()
            .flat_map(|section| &section.entries)
            .any(|entry| entry.item_name == "Venison"),
        "regression must exercise the real 1/1 Venison configuration"
    );

    assert!(zone_ground_drop_snapshots_for_monster_at_tick(9_102, "Deer", 7).is_empty());
    assert!(
        zone_ground_drop_snapshots_for_monster_at_tick_with_rate(9_102, "Deer", 7, 100).is_empty(),
        "the reward-owner item-drop bonus must not restore corpse-only loot"
    );

    let mut session = SimulationSession::new(SimulationConfig::default());
    let characters = session
        .handle_packet(mir2_protocol::ClientPacket::Login {
            account_id: "demo".to_owned(),
            password: "demo".to_owned(),
        })
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::LoginSuccess { characters } => Some(characters),
            _ => None,
        })
        .expect("demo login");
    let character_index = characters.first().expect("demo character").index;
    let _ = session.handle_packet(mir2_protocol::ClientPacket::StartGame { character_index });
    assert!(
        zone_ground_drop_snapshots_for_monster(session.app.world(), 9_102, "Deer").is_empty(),
        "the World-backed ZoneMonsterSpawn path must not preload Venison"
    );
}

#[test]
fn zone_non_harvest_monster_death_drops_remain_enabled() {
    let scarecrow =
        crystal_monster_by_name("Scarecrow").expect("bundled Crystal Scarecrow template");
    assert!(initial_harvest_monster_state(scarecrow.ai).is_none());

    assert!(
        !zone_ground_drop_snapshots_for_monster_at_tick_with_rate(9_001, "Scarecrow", 7, 100,)
            .is_empty(),
        "the harvest boundary must not suppress ordinary Zone death loot"
    );
}

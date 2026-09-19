use mir2_simulation::SimulationConfig;

#[test]
fn v2_training_spawns_are_real_profile_sources_and_do_not_change_other_cadences() {
    let previous_cadence = std::env::var_os("MIR2_QUEST_CADENCE");
    let config = SimulationConfig::default().with_platinum_176_profile();

    std::env::set_var("MIR2_QUEST_CADENCE", "newcomer-v1");
    let original = config.crystal_respawns_for_map("D022");
    assert!(original.iter().any(|spawn| spawn.respawn_index == 940));
    assert!(original.iter().any(|spawn| spawn.respawn_index == 941));
    assert!(!original.iter().any(|spawn| spawn.respawn_index == 10019));
    assert!(!original.iter().any(|spawn| spawn.respawn_index == 10020));
    assert!(!original.iter().any(|spawn| spawn.respawn_index == 10021));

    std::env::set_var("MIR2_QUEST_CADENCE", "newcomer-v2");
    let v2 = config.crystal_respawns_for_map("D022");
    for (index, name, x, y) in [
        (10019, "Dung", 335, 360),
        (10020, "WoomaSoldier", 320, 345),
        (10021, "WoomaFighter", 300, 335),
    ] {
        let spawn = v2.iter().find(|spawn| spawn.respawn_index == index).unwrap();
        assert_eq!(spawn.monster_name, name);
        assert_eq!(spawn.location.x, x);
        assert_eq!(spawn.location.y, y);
        assert_eq!(spawn.count, 1);
        assert_eq!(spawn.spread, 0);
        assert_eq!(spawn.delay_minutes, 0);
    }
    assert!(v2.iter().any(|spawn| spawn.respawn_index == 940));
    assert!(v2.iter().any(|spawn| spawn.respawn_index == 941));
    assert!(!config
        .crystal_respawns_for_map("D021")
        .iter()
        .any(|spawn| [10019, 10020, 10021].contains(&spawn.respawn_index)));

    if let Some(value) = previous_cadence {
        std::env::set_var("MIR2_QUEST_CADENCE", value);
    } else {
        std::env::remove_var("MIR2_QUEST_CADENCE");
    }
}

use mir2_simulation::SimulationConfig;

#[test]
fn v2_training_spawns_are_real_profile_sources_and_do_not_change_other_cadences() {
    let previous_cadence = std::env::var_os("MIR2_QUEST_CADENCE");
    let config = SimulationConfig::default().with_platinum_176_profile();

    std::env::set_var("MIR2_QUEST_CADENCE", "newcomer-v1");
    let original = config.crystal_respawns_for_map("D022");
    assert!(original.iter().any(|spawn| spawn.respawn_index == 940));
    assert!(original.iter().any(|spawn| spawn.respawn_index == 941));
    let original_soldier_hp = original.iter().find(|spawn| spawn.respawn_index == 940).unwrap().monster_hp;
    let original_fighter_hp = original.iter().find(|spawn| spawn.respawn_index == 941).unwrap().monster_hp;
    assert_eq!(original.iter().find(|spawn| spawn.respawn_index == 939).unwrap().count, 50);
    assert_eq!(original.iter().find(|spawn| spawn.respawn_index == 940).unwrap().count, 50);
    assert_eq!(original.iter().find(|spawn| spawn.respawn_index == 941).unwrap().count, 30);
    assert!(!original.iter().any(|spawn| (10019..=10027).contains(&spawn.respawn_index)));

    std::env::set_var("MIR2_QUEST_CADENCE", "newcomer-v2");
    let v2 = config.crystal_respawns_for_map("D022");
    for index in [939, 940, 941] {
        let imported = v2.iter().find(|spawn| spawn.respawn_index == index).unwrap();
        let original_group = original.iter().find(|spawn| spawn.respawn_index == index).unwrap();
        assert_eq!(imported.count, 1);
        assert_eq!(imported.location, original_group.location);
        assert_eq!(imported.spread, original_group.spread);
        assert_eq!(imported.delay_minutes, original_group.delay_minutes);
        assert_eq!(imported.monster_hp, original_group.monster_hp);
    }
    for (index, name, x, y) in [
        (10019, "Dung", 340, 355),
        (10020, "WoomaSoldier", 370, 340),
        (10021, "WoomaFighter", 300, 385),
        (10022, "Dung", 360, 355),
        (10023, "Dung", 335, 380),
        (10024, "WoomaSoldier", 320, 395),
        (10025, "WoomaSoldier", 370, 325),
        (10026, "WoomaFighter", 280, 340),
        (10027, "WoomaFighter", 300, 330),
    ] {
        let spawn = v2.iter().find(|spawn| spawn.respawn_index == index).unwrap();
        assert_eq!(spawn.monster_name, name);
        assert_eq!(spawn.location.x, x);
        assert_eq!(spawn.location.y, y);
        assert_eq!(spawn.count, 1);
        assert_eq!(spawn.spread, 0);
        assert_eq!(spawn.delay_minutes, 0);
        if [10020, 10021, 10024, 10025, 10026, 10027].contains(&index) {
            assert_eq!(spawn.monster_hp, 120);
        }
    }
    assert_eq!(v2.iter().find(|spawn| spawn.respawn_index == 940).unwrap().monster_hp, original_soldier_hp);
    assert_eq!(v2.iter().find(|spawn| spawn.respawn_index == 941).unwrap().monster_hp, original_fighter_hp);
    assert!(!config
        .crystal_respawns_for_map("D021")
        .iter()
        .any(|spawn| (10019..=10027).contains(&spawn.respawn_index)));

    if let Some(value) = previous_cadence {
        std::env::set_var("MIR2_QUEST_CADENCE", value);
    } else {
        std::env::remove_var("MIR2_QUEST_CADENCE");
    }
}

use mir2_game_data::crystal_respawn_manifest;
use mir2_simulation::SimulationConfig;

#[test]
fn crystal_world_config_imports_every_database_hazard_map() {
    let manifest = crystal_respawn_manifest();
    let expected: Vec<_> = manifest
        .maps
        .iter()
        .filter(|map| map.lightning || map.fire)
        .collect();
    let config = SimulationConfig::default().with_crystal_world_runtime();

    assert_eq!(config.map_hazards.len(), expected.len());
    for map in expected {
        let hazard = config
            .map_hazards
            .iter()
            .find(|hazard| hazard.map_file_name == map.map_file_name)
            .unwrap_or_else(|| panic!("missing hazard config for {}", map.map_file_name));
        assert_eq!(hazard.lightning, map.lightning, "{}", map.map_file_name);
        assert_eq!(hazard.fire, map.fire, "{}", map.map_file_name);
        assert_eq!(
            hazard.lightning_damage, map.lightning_damage,
            "{}",
            map.map_file_name
        );
        assert_eq!(hazard.fire_damage, map.fire_damage, "{}", map.map_file_name);
    }
}

#[test]
fn lightning_map_information_comes_from_crystal_database() {
    let mut base = SimulationConfig::default();
    base.map.file_name = "D2081".to_string();
    let config = base.with_crystal_world_runtime();

    assert_eq!(config.map.map_index, 331);
    assert_eq!(config.map.title, "LightningCave");
    assert_eq!(config.map.mini_map, 211);
    assert_eq!(config.map.big_map, 211);
    assert_eq!(config.map.lights, 2);
    assert!(config.map.has_lightning());
    assert!(!config.map.has_fire());
    assert_eq!(config.map.music, 0);
    assert_eq!(config.map.weather_particles, 0);
}

#[test]
fn fire_map_information_comes_from_crystal_database() {
    let mut base = SimulationConfig::default();
    base.map.file_name = "D2082.map".to_string();
    let config = base.with_crystal_world_runtime();

    assert_eq!(config.map.map_index, 332);
    assert_eq!(config.map.file_name, "D2082.map");
    assert_eq!(config.map.title, "MoltenRockCave");
    assert_eq!(config.map.mini_map, 212);
    assert_eq!(config.map.big_map, 212);
    assert_eq!(config.map.lights, 2);
    assert!(!config.map.has_lightning());
    assert!(config.map.has_fire());
}

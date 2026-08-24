#[cfg(test)]
use crate::config::{MonsterSpawnSource, SimulationConfig};
use mir2_protocol::Point;

use super::{
    collision_data_for_map_or_config, is_static_spawnable_point_with_collision, point_in_bounds,
    runtime_active_map_collision_data, runtime_map_collision_data, runtime_world_map_collision_data,
};

#[test]
fn collision_data_for_map_or_config_uses_crystal_world_full_collision_for_map_zero() {
    let mut config = SimulationConfig::default();
    config.monster_spawn_source = MonsterSpawnSource::CrystalWorld;

    let world_collision =
        runtime_world_map_collision_data("0").expect("full Bichon collision available");
    let starter_collision =
        runtime_map_collision_data("0").expect("fallback starter collision available");

    let point = (world_collision.collision.region_bounds.min_y
        ..=world_collision.collision.region_bounds.max_y)
        .find_map(|y| {
            (world_collision.collision.region_bounds.min_x
                ..=world_collision.collision.region_bounds.max_x)
                .find_map(|x| {
                    let point = Point { x, y };
                    let key = (point.x, point.y);

                    let outside_starter_slice =
                        !point_in_bounds(&starter_collision.collision.region_bounds, &point);
                    let passable_full = !world_collision.blocked_set.contains(&key)
                        && !world_collision.closed_door_set.contains(&key);
                    (outside_starter_slice && passable_full).then_some(point)
                })
        })
        .expect("Crystal world must contain at least one passable tile outside starter bounds");

    let chosen_collision = collision_data_for_map_or_config(&config, "0");

    assert_eq!(
        chosen_collision.collision.region_bounds, world_collision.collision.region_bounds,
        "CrystalWorld should read full Bichon collision for map 0"
    );

    assert!(
        !is_static_spawnable_point_with_collision(&config, "0", &starter_collision, &point,),
        "starter-collision-only path should not allow the chosen exterior tile"
    );
    assert!(
        is_static_spawnable_point_with_collision(&config, "0", &chosen_collision, &point,),
        "CrystalWorld path should allow the chosen exterior tile via full map collision"
    );
}

#[test]
fn collision_data_for_map_or_config_keeps_non_crystalworld_starter_collision_for_map_zero() {
    let mut config = SimulationConfig::default();
    config.monster_spawn_source = MonsterSpawnSource::StarterScenario;

    let starter_collision =
        runtime_map_collision_data("0").expect("starter map collision available");
    let chosen_collision = collision_data_for_map_or_config(&config, "0");

    assert_eq!(
        chosen_collision.collision.region_bounds, starter_collision.collision.region_bounds,
        "non-CrystalWorld path must continue using starter collision"
    );
}

#[test]
fn active_full_bichon_collision_wins_over_starter_config_but_starter_field_keeps_starter_collision()
{
    let config = SimulationConfig::default();
    assert_eq!(config.monster_spawn_source, MonsterSpawnSource::StarterScenario);

    let active_bichon = mir2_protocol::MapInformation {
        map_index: 0,
        file_name: "0".to_owned(),
        title: "BichonProvince".to_owned(),
        mini_map: 0,
        big_map: 0,
        lights: 0,
        flags: 0,
        map_dark_light: 0,
        music: 0,
        weather_particles: 0,
    };
    let starter_field = mir2_protocol::MapInformation {
        title: "Starter Field".to_owned(),
        ..active_bichon.clone()
    };
    let full_world =
        runtime_world_map_collision_data("0").expect("full Bichon collision available");
    let starter = runtime_map_collision_data("0").expect("starter collision available");

    let active_collision = runtime_active_map_collision_data(&active_bichon)
        .expect("active Bichon should resolve a collision source");
    assert_eq!(
        active_collision.collision.region_bounds, full_world.collision.region_bounds,
        "active full Bichon collision must win even with StarterScenario config"
    );

    let starter_field_collision = runtime_active_map_collision_data(&starter_field)
        .expect("Starter Field should resolve a collision source");
    assert_eq!(
        starter_field_collision.collision.region_bounds, starter.collision.region_bounds,
        "true Starter Field must keep starter collision"
    );

    let point = (full_world.collision.region_bounds.min_y
        ..=full_world.collision.region_bounds.max_y)
        .find_map(|y| {
            (full_world.collision.region_bounds.min_x
                ..=full_world.collision.region_bounds.max_x)
                .find_map(|x| {
                    let point = Point { x, y };
                    let outside_starter =
                        !point_in_bounds(&starter.collision.region_bounds, &point);
                    let passable_full = !full_world.blocked_set.contains(&(point.x, point.y))
                        && !full_world.closed_door_set.contains(&(point.x, point.y));
                    (outside_starter && passable_full).then_some(point)
                })
        })
        .expect("full Bichon should contain a passable tile outside starter bounds");

    assert!(!is_static_spawnable_point_with_collision(
        &config, "0", &starter, &point
    ));
    assert!(is_static_spawnable_point_with_collision(
        &config,
        "0",
        &active_collision,
        &point
    ));
}
#[test]
fn map_extension_aliases_share_world_cache_and_resolve_real_collision() {
    let mut crystal_config = SimulationConfig::default();
    crystal_config.monster_spawn_source = MonsterSpawnSource::CrystalWorld;
    let baseline_world =
        runtime_world_map_collision_data("0").expect("full Bichon collision available");
    let baseline_starter =
        runtime_map_collision_data("0").expect("starter map collision available");

    for alias in ["0.map", "0.MAP", "0.Map", "0.mAp"] {
        let aliased_world = runtime_world_map_collision_data(alias)
            .unwrap_or_else(|| panic!("full collision should resolve for {alias}"));
        assert!(
            std::sync::Arc::ptr_eq(&baseline_world, &aliased_world),
            "{alias} should use the same normalized world-collision cache entry"
        );

        let aliased_starter = runtime_map_collision_data(alias)
            .unwrap_or_else(|| panic!("starter collision should resolve for {alias}"));
        assert_eq!(
            aliased_starter.collision.region_bounds, baseline_starter.collision.region_bounds,
            "{alias} should resolve through the normalized starter-map path"
        );

        let chosen_collision = collision_data_for_map_or_config(&crystal_config, alias);
        assert_eq!(
            chosen_collision.collision.region_bounds, baseline_world.collision.region_bounds,
            "{alias} should select the full CrystalWorld collision"
        );
    }
}

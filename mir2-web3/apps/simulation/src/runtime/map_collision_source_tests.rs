#[cfg(test)]
use crate::config::{MonsterSpawnSource, SimulationConfig};
use mir2_protocol::Point;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::super::door::{open_door, tick_doors, DOOR_OPEN_DURATION_TICKS};
use super::super::mining::MiningResource;
use super::super::resources::{
    DoorRegistry, MapRuntimeResource, RuntimeClockResource, RuntimeConfigResource,
};
use bevy_ecs::prelude::World;

use super::{
    collision_data_for_map_or_config, crystal_movement_transfer_records_for_map,
    is_static_spawnable_point_with_collision, point_in_bounds, runtime_active_map_collision_data,
    runtime_map_collision_data, runtime_world_map_collision_data,
};

fn collision_only_world(config: &SimulationConfig) -> World {
    let mut world = World::new();
    world.insert_resource(RuntimeConfigResource::new(config));
    world.insert_resource(RuntimeClockResource::new());
    world.insert_resource(MiningResource::with_builtin_sets());
    world.insert_resource(MapRuntimeResource::new(
        config,
        config.map_collision.region_bounds,
        BTreeSet::new().into(),
        BTreeSet::new(),
        DoorRegistry::default(),
        BTreeMap::new().into(),
    ));
    super::refresh_runtime_map_collision(&mut world);
    world
}

#[test]
fn immutable_collision_concurrent_cold_publish_returns_one_real_map_allocation() {
    use std::sync::{Barrier, Mutex};
    use std::thread;
    const LOADERS: usize = 4;
    let bytes = Arc::new(super::read_crystal_map_pack_bytes("0").expect("bundled full Bichon map"));
    let cache = Arc::new(Mutex::new(BTreeMap::new()));
    let barrier = Arc::new(Barrier::new(LOADERS));
    let handles: Vec<_> = (0..LOADERS)
        .map(|_| {
            let bytes = Arc::clone(&bytes);
            let cache = Arc::clone(&cache);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                assert!(
                    cache.lock().unwrap().get("0").is_none(),
                    "every loader must observe a cold miss"
                );
                let parsed =
                    super::parse_runtime_map_collision("0", &bytes).expect("real map parses");
                let candidate = Arc::new(super::runtime_map_collision_from_template(parsed));
                // Force four independent parses before any second-lock publication.
                barrier.wait();
                let published = super::publish_map_collision(
                    &cache,
                    "0".to_owned(),
                    Some(Arc::clone(&candidate)),
                )
                .expect("published real map");
                assert_eq!(published.collision, candidate.collision);
                assert_eq!(published.blocked_set, candidate.blocked_set);
                assert_eq!(published.fishing_cells, candidate.fishing_cells);
                assert_eq!(published.closed_door_set, candidate.closed_door_set);
                published
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("loader completed"))
        .collect();
    for result in &results[1..] {
        assert!(Arc::ptr_eq(&results[0], result));
        assert!(Arc::ptr_eq(&results[0].collision, &result.collision));
        assert!(Arc::ptr_eq(&results[0].blocked_set, &result.blocked_set));
        assert!(Arc::ptr_eq(
            &results[0].fishing_cells,
            &result.fishing_cells
        ));
    }
    assert_eq!(cache.lock().unwrap().len(), 1);

    // Failed lookups remain cached. First publication wins for both Some and None.
    assert!(super::publish_map_collision(&cache, "missing".to_owned(), None).is_none());
    assert!(super::publish_map_collision(
        &cache,
        "missing".to_owned(),
        Some(Arc::clone(&results[0]))
    )
    .is_none());
    let existing = super::publish_map_collision(&cache, "0".to_owned(), None).unwrap();
    assert!(Arc::ptr_eq(&results[0], &existing));

    // The starter/fallback cache stores the struct directly, not Arc<struct>.
    let owned_cache = Mutex::new(BTreeMap::new());
    let first = super::publish_map_collision(
        &owned_cache,
        "0".to_owned(),
        Some(results[0].as_ref().clone()),
    )
    .unwrap();
    let other_data =
        super::runtime_map_collision_from_template(results[0].collision.as_ref().clone());
    let next =
        super::publish_map_collision(&owned_cache, "0".to_owned(), Some(other_data)).unwrap();
    assert!(Arc::ptr_eq(&first.collision, &next.collision));
    assert!(Arc::ptr_eq(&first.blocked_set, &next.blocked_set));
    assert!(Arc::ptr_eq(&first.fishing_cells, &next.fishing_cells));
}

#[test]
fn immutable_collision_clone_shares_all_static_fields_with_exact_template_contents() {
    let config = SimulationConfig::default().with_crystal_world_runtime();
    let cached = runtime_world_map_collision_data("0").expect("full Bichon collision");
    let selected = collision_data_for_map_or_config(&config, "0.MAP");

    assert!(Arc::ptr_eq(&cached.collision, &selected.collision));
    assert!(Arc::ptr_eq(&cached.blocked_set, &selected.blocked_set));
    assert!(Arc::ptr_eq(&cached.fishing_cells, &selected.fishing_cells));
    let expected_blocked: BTreeSet<_> = cached
        .collision
        .blocked_cells
        .iter()
        .map(|cell| (cell.x, cell.y))
        .collect();
    let expected_fishing: BTreeMap<_, _> = cached
        .collision
        .fishing_cells
        .iter()
        .map(|cell| ((cell.x, cell.y), cell.attribute))
        .collect();
    let expected_doors: BTreeSet<_> = cached
        .collision
        .doors
        .iter()
        .filter(|door| door.closed)
        .map(|door| (door.x, door.y))
        .collect();
    assert_eq!(*selected.blocked_set, expected_blocked);
    assert_eq!(*selected.fishing_cells, expected_fishing);
    assert_eq!(selected.closed_door_set, expected_doors);
    assert!(!expected_blocked.is_empty());
    assert!(!expected_fishing.is_empty());
    assert!(!expected_doors.is_empty());

    let mut clone = selected.clone();
    clone.closed_door_set.clear();
    assert_eq!(selected.closed_door_set, expected_doors);
    assert_eq!(cached.closed_door_set, expected_doors);
}

#[test]
fn immutable_collision_refresh_shares_terrain_but_keeps_doors_private_across_maps() {
    let config = SimulationConfig::default().with_crystal_world_runtime();
    let mut first = collision_only_world(&config);
    let second = collision_only_world(&config);
    let cached = runtime_world_map_collision_data("0").expect("full Bichon collision");
    let (door_index, door_cells) = {
        let first_map = first.resource::<MapRuntimeResource>();
        let second_map = second.resource::<MapRuntimeResource>();
        assert!(Arc::ptr_eq(
            &first_map.blocked_cells,
            &second_map.blocked_cells
        ));
        assert!(Arc::ptr_eq(&first_map.blocked_cells, &cached.blocked_set));
        assert!(Arc::ptr_eq(
            &first_map.fishing_cells,
            &second_map.fishing_cells
        ));
        assert_eq!(first_map.closed_door_cells, second_map.closed_door_cells);
        assert_eq!(first_map.doors, second_map.doors);
        let door = first_map.doors.doors.first().expect("real map door");
        (door.index, door.cells.clone())
    };
    assert_eq!(
        open_door(&mut first, door_index),
        vec![mir2_protocol::ServerPacket::OpenDoor {
            door_index,
            close: false
        }]
    );
    assert!(door_cells.iter().all(|cell| !first
        .resource::<MapRuntimeResource>()
        .closed_door_cells
        .contains(cell)));
    assert_eq!(
        second.resource::<MapRuntimeResource>().closed_door_cells,
        cached.closed_door_set
    );
    assert!(second
        .resource::<MapRuntimeResource>()
        .doors
        .doors
        .iter()
        .all(|door| door.close_at_tick.is_none()));
    first.resource_mut::<RuntimeClockResource>().tick = DOOR_OPEN_DURATION_TICKS;
    let mut closed = Vec::new();
    tick_doors(&mut first, &mut closed);
    assert_eq!(
        closed,
        vec![mir2_protocol::ServerPacket::OpenDoor {
            door_index,
            close: true
        }]
    );
    assert_eq!(
        first.resource::<MapRuntimeResource>().closed_door_cells,
        cached.closed_door_set
    );

    first
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = "D401".to_owned();
    super::refresh_runtime_map_collision(&mut first);
    let mine = runtime_world_map_collision_data("D401").expect("Dead Mine collision");
    assert!(Arc::ptr_eq(
        &first.resource::<MapRuntimeResource>().blocked_cells,
        &mine.blocked_set
    ));
    assert!(Arc::ptr_eq(
        &first.resource::<MapRuntimeResource>().fishing_cells,
        &mine.fishing_cells
    ));
    assert!(!Arc::ptr_eq(
        &first.resource::<MapRuntimeResource>().blocked_cells,
        &cached.blocked_set
    ));
    assert!(Arc::ptr_eq(
        &second.resource::<MapRuntimeResource>().blocked_cells,
        &cached.blocked_set
    ));
    assert_eq!(
        first.resource::<MapRuntimeResource>().closed_door_cells,
        mine.closed_door_set
    );
    assert_eq!(
        first.resource::<MapRuntimeResource>().doors,
        DoorRegistry::from_templates(&mine.collision.doors)
    );

    first
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = "0.MAP".to_owned();
    super::refresh_runtime_map_collision(&mut first);
    assert!(Arc::ptr_eq(
        &first.resource::<MapRuntimeResource>().blocked_cells,
        &cached.blocked_set
    ));
    assert!(Arc::ptr_eq(
        &first.resource::<MapRuntimeResource>().fishing_cells,
        &cached.fishing_cells
    ));
    assert_eq!(
        first.resource::<MapRuntimeResource>().doors,
        second.resource::<MapRuntimeResource>().doors
    );
    assert_eq!(
        first.resource::<MapRuntimeResource>().closed_door_cells,
        cached.closed_door_set
    );
}

#[test]
fn immutable_collision_fixture_copy_on_write_cannot_modify_cache_or_another_world() {
    let config = SimulationConfig::default().with_crystal_world_runtime();
    let mut first = collision_only_world(&config);
    let second = collision_only_world(&config);
    let cached = runtime_world_map_collision_data("0").expect("full Bichon collision");
    let blocked_count = cached.blocked_set.len();
    let fishing_count = cached.fishing_cells.len();
    {
        let mut map = first.resource_mut::<MapRuntimeResource>();
        Arc::make_mut(&mut map.blocked_cells).clear();
        Arc::make_mut(&mut map.fishing_cells).clear();
    }
    assert!(first
        .resource::<MapRuntimeResource>()
        .blocked_cells
        .is_empty());
    assert!(first
        .resource::<MapRuntimeResource>()
        .fishing_cells
        .is_empty());
    assert!(!Arc::ptr_eq(
        &first.resource::<MapRuntimeResource>().blocked_cells,
        &cached.blocked_set
    ));
    assert!(!Arc::ptr_eq(
        &first.resource::<MapRuntimeResource>().fishing_cells,
        &cached.fishing_cells
    ));
    assert_eq!(cached.blocked_set.len(), blocked_count);
    assert_eq!(cached.fishing_cells.len(), fishing_count);
    assert!(Arc::ptr_eq(
        &second.resource::<MapRuntimeResource>().blocked_cells,
        &cached.blocked_set
    ));
    assert!(Arc::ptr_eq(
        &second.resource::<MapRuntimeResource>().fishing_cells,
        &cached.fishing_cells
    ));
}

#[test]
fn immutable_collision_sharing_preserves_missing_map_configuration_fallback() {
    let mut config = SimulationConfig::default().with_crystal_world_runtime();
    let missing = "collision-arc-missing-map-fixture";
    config.map_collision.map_file_name = format!("{missing}.map");
    config.map_collision.blocked_cells = vec![mir2_game_data::BlockedMapCellTemplate {
        x: 12,
        y: 34,
        attribute: mir2_game_data::MapCellAttribute::LowWall,
    }];
    config.map_collision.fishing_cells = vec![mir2_game_data::FishingCellTemplate {
        x: 56,
        y: 78,
        attribute: 19,
    }];
    let fallback = collision_data_for_map_or_config(&config, missing);
    assert_eq!(*fallback.collision, config.map_collision);
    assert_eq!(*fallback.blocked_set, BTreeSet::from([(12, 34)]));
    assert_eq!(*fallback.fishing_cells, BTreeMap::from([((56, 78), 19)]));
    let mut owned = fallback.clone();
    Arc::make_mut(&mut owned.collision).blocked_cells.clear();
    assert_eq!(*fallback.collision, config.map_collision);
}

#[test]
fn immutable_collision_zone_door_projection_does_not_mutate_shared_static_cells() {
    let cached = runtime_world_map_collision_data("2").expect("Serpent Valley collision");
    let point = Point { x: 517, y: 492 };
    assert!(!cached.blocked_set.contains(&(point.x, point.y)));
    let door_index = cached
        .collision
        .doors
        .iter()
        .find(|door| door.x == point.x && door.y == point.y && door.closed)
        .expect("existing shop-exit door fixture")
        .index
        & 0x7F;
    let mut first = crate::ZoneCollision::for_map("2");
    let second = crate::ZoneCollision::for_map("2");
    assert!(first.is_blocked(&point));
    assert!(second.is_blocked(&point));
    first.open_door(door_index);
    assert!(!first.is_blocked(&point));
    assert!(second.is_blocked(&point));
    assert!(!cached.blocked_set.contains(&(point.x, point.y)));
    assert!(cached.closed_door_set.contains(&(point.x, point.y)));
    first.close_door(door_index);
    assert!(first.is_blocked(&point));
    assert_eq!(first, second);
}

#[test]
#[ignore = "explicit real-map collision clone benchmark; timings are evidence, not CI thresholds"]
fn immutable_collision_real_map_clone_benchmark() {
    use std::hint::black_box;
    use std::time::Instant;
    let cached = runtime_world_map_collision_data("0").expect("full Bichon collision");
    const ROUNDS: usize = 25;
    let mut legacy_times = Vec::new();
    let mut shared_times = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        for _ in 0..ROUNDS {
            // The exact owned fields cloned before this change, including private doors.
            black_box((
                cached.collision.as_ref().clone(),
                cached.blocked_set.as_ref().clone(),
                cached.closed_door_set.clone(),
                cached.fishing_cells.as_ref().clone(),
            ));
        }
        legacy_times.push(start.elapsed().as_micros());
        let start = Instant::now();
        for _ in 0..ROUNDS {
            black_box(cached.as_ref().clone());
        }
        shared_times.push(start.elapsed().as_micros());
    }
    legacy_times.sort();
    shared_times.sort();
    let clone = cached.as_ref().clone();
    assert_eq!(clone.collision, cached.collision);
    assert_eq!(clone.blocked_set, cached.blocked_set);
    assert_eq!(clone.fishing_cells, cached.fishing_cells);
    assert_eq!(clone.closed_door_set, cached.closed_door_set);
    assert!(Arc::ptr_eq(&clone.collision, &cached.collision));
    assert!(Arc::ptr_eq(&clone.blocked_set, &cached.blocked_set));
    assert!(Arc::ptr_eq(&clone.fishing_cells, &cached.fishing_cells));
    println!("collision_clone real_map=0 rounds={ROUNDS} trials=5 blocked={} fishing={} closed_doors={} legacy_median_us={} shared_median_us={} static_deep_clones_before={} static_deep_clones_after=0 private_door_clones={ROUNDS}; not RSS or capacity", cached.blocked_set.len(), cached.fishing_cells.len(), cached.closed_door_set.len(), legacy_times[2], shared_times[2], ROUNDS * 3);
}

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
    assert_eq!(
        config.monster_spawn_source,
        MonsterSpawnSource::StarterScenario
    );

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
            (full_world.collision.region_bounds.min_x..=full_world.collision.region_bounds.max_x)
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

#[test]
fn crystal_service_shop_exit_survives_closed_door_destination_validation() {
    let transfers = crystal_movement_transfer_records_for_map("0120");
    let shop_exit = transfers
        .iter()
        .find(|transfer| {
            transfer.to_map_file_name == "2"
                && transfer.from_bounds.min_x == 14
                && transfer.from_bounds.min_y == 15
        })
        .expect("Crystal 0120 blacksmith must retain its only exit to Serpent Valley");

    assert_eq!(shop_exit.to_position, Point { x: 517, y: 492 });
    assert_eq!(shop_exit.from_bounds.max_x, 14);
    assert_eq!(shop_exit.from_bounds.max_y, 15);

    let valley = runtime_world_map_collision_data("2")
        .expect("Serpent Valley collision should be available");
    assert!(point_in_bounds(
        &valley.collision.region_bounds,
        &shop_exit.to_position
    ));
    assert!(!valley.blocked_set.contains(&(517, 492)));
    assert!(
        valley.closed_door_set.contains(&(517, 492)),
        "the regression specifically exercises Crystal's valid closed-door landing cell"
    );
}

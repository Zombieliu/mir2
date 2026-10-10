use super::*;
use mir2_game_data::{
    BlockedMapCellTemplate, DoorMapCellTemplate, MapBounds, MapCellAttribute, StarterMapCollision,
};

const MAP: &str = "respawn-geometry-private";

fn collision(blocked: &[(i32, i32)], closed_doors: &[(i32, i32)]) -> Arc<RuntimeMapCollisionData> {
    let bounds = MapBounds {
        min_x: 0,
        max_x: 63,
        min_y: 0,
        max_y: 63,
    };
    Arc::new(super::super::map::runtime_map_collision_from_template(
        StarterMapCollision {
            map_file_name: MAP.to_string(),
            map_width: 64,
            map_height: 64,
            region_bounds: bounds,
            play_bounds: bounds,
            blocked_cells: blocked
                .iter()
                .map(|&(x, y)| BlockedMapCellTemplate {
                    x,
                    y,
                    attribute: MapCellAttribute::HighWall,
                })
                .collect(),
            doors: closed_doors
                .iter()
                .map(|&(x, y)| DoorMapCellTemplate {
                    x,
                    y,
                    index: 1,
                    closed: true,
                })
                .collect(),
            fishing_cells: Vec::new(),
        },
    ))
}

fn respawn(index: i32, location: Point, count: u16, spread: u16) -> CrystalRespawnTemplate {
    CrystalRespawnTemplate {
        monster_index: 1,
        location,
        count,
        spread,
        delay_minutes: 5,
        direction: MirDirection::Down,
        route_path: None,
        random_delay_minutes: 0,
        respawn_index: index,
        save_respawn_time: false,
        respawn_ticks: 0,
        monster_name: "Yob".to_string(),
        monster_image: 1,
        monster_ai: 1,
        monster_view_range: 7,
        monster_hp: 30,
        monster_attack_speed: 2000,
        monster_move_speed: 1000,
        monster_can_push: true,
        monster_can_tame: false,
        monster_auto_rev: false,
        monster_undead: false,
        monster_agility: 5,
        route: Vec::new(),
    }
}

fn compare(
    cache: &mut VisibleRespawnGeometryCache,
    template: &CrystalRespawnTemplate,
    player: &Point,
    terrain: Option<Arc<RuntimeMapCollisionData>>,
    original_computations: &mut usize,
) -> Vec<(usize, Point, MirDirection)> {
    let expected = original_visible_respawn_spawns(
        MAP,
        template,
        player,
        terrain.clone(),
        original_computations,
    );
    let actual = start_game_visible_respawn_spawns_with_geometry_cache(
        MAP,
        template,
        player,
        Some(cache),
        |_| terrain,
    );
    assert_eq!(
        actual, expected,
        "slot, position, direction and order must match Source baseline"
    );
    actual
}

#[test]
fn repeated_rectangles_reuse_one_count_and_preserve_each_respawn_output() {
    let terrain = collision(&[(10, 10), (20, 20)], &[(12, 12), (16, 16)]);
    let player = Point { x: 20, y: 20 };
    let mut cache = VisibleRespawnGeometryCache::default();
    let mut original_computations = 0;
    for (index, count) in [(50, 23), (96, 37), (136, 61)] {
        let template = respawn(index, player.clone(), count, 20);
        let actual = compare(
            &mut cache,
            &template,
            &player,
            Some(Arc::clone(&terrain)),
            &mut original_computations,
        );
        assert!(!actual.is_empty());
        assert!(actual.iter().all(|(_, point, _)| {
            !terrain.blocked_set.contains(&(point.x, point.y))
                && !terrain.closed_door_set.contains(&(point.x, point.y))
        }));
    }
    assert_eq!(original_computations, 3);
    assert_eq!(cache.walkable_count_computations, 1);

    // Another build cannot inherit even a matching map/collision/rectangle count.
    let mut next_build = VisibleRespawnGeometryCache::default();
    compare(
        &mut next_build,
        &respawn(50, player.clone(), 23, 20),
        &player,
        Some(terrain),
        &mut original_computations,
    );
    assert_eq!(next_build.walkable_count_computations, 1);
}

#[test]
fn different_and_overlapping_rectangles_do_not_share_counts() {
    let terrain = collision(&[(15, 15), (35, 35)], &[(10, 10), (30, 30)]);
    let player = Point { x: 20, y: 20 };
    let mut cache = VisibleRespawnGeometryCache::default();
    let mut original_computations = 0;
    for template in [
        respawn(1, Point { x: 20, y: 20 }, 40, 20),
        respawn(2, Point { x: 21, y: 20 }, 40, 20),
        respawn(3, Point { x: 20, y: 20 }, 40, 19),
        respawn(4, Point { x: 2, y: 2 }, 40, 20),
    ] {
        compare(
            &mut cache,
            &template,
            &player,
            Some(Arc::clone(&terrain)),
            &mut original_computations,
        );
    }
    assert_eq!(original_computations, 4);
    assert_eq!(cache.walkable_count_computations, 4);
}

#[test]
fn same_rectangle_uses_actual_collision_identity_and_closed_door_image() {
    let terrain = collision(&[(10, 10)], &[(12, 12)]);
    let mut changed = terrain.as_ref().clone();
    changed.closed_door_set.insert((13, 13));
    let changed = Arc::new(changed);
    let identical_copy = Arc::new(terrain.as_ref().clone());
    let player = Point { x: 20, y: 20 };
    let template = respawn(17, player.clone(), 40, 20);
    let mut cache = VisibleRespawnGeometryCache::default();
    let mut original_computations = 0;
    for current in [&terrain, &changed, &identical_copy, &terrain, &changed] {
        let actual = compare(
            &mut cache,
            &template,
            &player,
            Some(Arc::clone(current)),
            &mut original_computations,
        );
        assert!(actual.iter().all(|(_, point, _)| {
            !current.blocked_set.contains(&(point.x, point.y))
                && !current.closed_door_set.contains(&(point.x, point.y))
        }));
    }
    assert_eq!(original_computations, 5);
    assert_eq!(cache.walkable_count_computations, 3);
    let counts = &cache.counts[&(0, 40, 0, 40)];
    assert_eq!(counts.len(), 3);
    assert_eq!(counts[0].1, 1681 - 2);
    assert_eq!(counts[1].1, 1681 - 3);
}

#[test]
fn changed_viewport_reuses_only_whole_spawn_count_not_visible_candidates() {
    let terrain = collision(&[(10, 10), (40, 40)], &[(11, 11), (39, 39)]);
    let template = respawn(23, Point { x: 30, y: 30 }, 100, 25);
    let mut cache = VisibleRespawnGeometryCache::default();
    let mut original_computations = 0;
    let first = compare(
        &mut cache,
        &template,
        &Point { x: 10, y: 10 },
        Some(Arc::clone(&terrain)),
        &mut original_computations,
    );
    let second = compare(
        &mut cache,
        &template,
        &Point { x: 45, y: 45 },
        Some(terrain),
        &mut original_computations,
    );
    assert!(!first.is_empty() && !second.is_empty());
    assert_ne!(first, second);
    assert_eq!(original_computations, 2);
    assert_eq!(cache.walkable_count_computations, 1);
}

#[test]
fn missing_collision_keeps_original_fallback_and_never_reuses_loaded_count() {
    let terrain = collision(&[(20, 20)], &[(21, 21)]);
    let player = Point { x: 20, y: 20 };
    let template = respawn(31, player.clone(), 25, 20);
    let mut cache = VisibleRespawnGeometryCache::default();
    let mut original_computations = 0;
    let first = compare(
        &mut cache,
        &template,
        &player,
        None,
        &mut original_computations,
    );
    assert!(!first.is_empty());
    assert_eq!(cache.walkable_count_computations, 0);
    compare(
        &mut cache,
        &template,
        &player,
        Some(terrain),
        &mut original_computations,
    );
    let second = compare(
        &mut cache,
        &template,
        &player,
        None,
        &mut original_computations,
    );
    assert_eq!(first, second);
    assert_eq!(original_computations, 1);
    assert_eq!(cache.walkable_count_computations, 1);
}

#[test]
fn zero_walkable_count_is_cached_without_emitting_blocked_or_closed_spawns() {
    let terrain = collision(
        &[(20, 20)],
        &[
            (19, 19),
            (20, 19),
            (21, 19),
            (19, 20),
            (21, 20),
            (19, 21),
            (20, 21),
            (21, 21),
        ],
    );
    let player = Point { x: 20, y: 20 };
    let template = respawn(40, player.clone(), 10, 1);
    let mut cache = VisibleRespawnGeometryCache::default();
    let mut original_computations = 0;
    for _ in 0..2 {
        assert!(compare(
            &mut cache,
            &template,
            &player,
            Some(Arc::clone(&terrain)),
            &mut original_computations,
        )
        .is_empty());
    }
    assert_eq!(original_computations, 2);
    assert_eq!(cache.walkable_count_computations, 1);
}

#[test]
fn original_early_gates_do_not_load_or_warm_collision() {
    let player = Point { x: 20, y: 20 };
    let mut cache = VisibleRespawnGeometryCache::default();
    let mut original_computations = 0;
    for template in [
        respawn(1, player.clone(), 0, 20),
        respawn(2, player.clone(), 1, 0),
        respawn(3, Point { x: 90, y: 90 }, 1, 0),
        respawn(4, player.clone(), 2, 0),
        respawn(5, Point { x: 90, y: 90 }, 10, 20),
    ] {
        let expected = original_visible_respawn_spawns(
            MAP,
            &template,
            &player,
            None,
            &mut original_computations,
        );
        let actual = start_game_visible_respawn_spawns_with_geometry_cache(
            MAP,
            &template,
            &player,
            Some(&mut cache),
            |_| panic!("original early gates must run before collision loading"),
        );
        assert_eq!(actual, expected);
    }
    assert_eq!(original_computations, 0);
    assert_eq!(cache.walkable_count_computations, 0);
    assert!(cache.counts.is_empty());
}

#[test]
fn bundled_town_respawns_preserve_every_slot_and_reduce_repeated_region_scans() {
    let map = "0";
    let player = Point { x: 300, y: 410 };
    let terrain = super::super::map::runtime_world_map_collision_data(map)
        .expect("the ordinary bundled town collision must be available");
    let templates = SimulationConfig::default().crystal_respawns_for_map(map);
    assert!(!templates.is_empty());

    // Both algorithms receive the same bundled geometry. These component
    // timings exclude loading and do not stand in for the real Gateway scroll
    // deadline; that unchanged test runs in its own fresh process.
    let mut original_computations = 0;
    let before = std::time::Instant::now();
    let expected = templates
        .iter()
        .map(|template| {
            original_visible_respawn_spawns(
                map,
                template,
                &player,
                Some(Arc::clone(&terrain)),
                &mut original_computations,
            )
        })
        .collect::<Vec<_>>();
    let original_elapsed = before.elapsed();
    let mut cache = VisibleRespawnGeometryCache::default();
    let before = std::time::Instant::now();
    let actual = templates
        .iter()
        .map(|template| {
            start_game_visible_respawn_spawns_with_geometry_cache(
                map,
                template,
                &player,
                Some(&mut cache),
                |_| Some(Arc::clone(&terrain)),
            )
        })
        .collect::<Vec<_>>();
    let cached_elapsed = before.elapsed();
    assert_eq!(
        actual, expected,
        "every slot, point, direction and group order"
    );
    assert!(actual.iter().any(|slots| !slots.is_empty()));
    assert!(cache.walkable_count_computations < original_computations);
    eprintln!(
        "respawn-component map={map} viewport=300,410 groups={} baseline_scans={original_computations} cached_scans={} baseline_us={} cached_us={} excludes_load=true",
        templates.len(),
        cache.walkable_count_computations,
        original_elapsed.as_micros(),
        cached_elapsed.as_micros(),
    );
}

// Retained start_game_visible_respawn_spawns algorithm from 6153cb2b802fa700f6a182ce704f76e7568bdd08.
// Only the collision loader is supplied by the fixture and the real count call
// is counted. It does not call the new cache/helper, so placement comparisons
// retain the original quantity, fallback, random selection and iteration rules.
fn original_visible_respawn_spawns(
    map_file_name: &str,
    respawn: &CrystalRespawnTemplate,
    player_position: &Point,
    full_collision: Option<Arc<RuntimeMapCollisionData>>,
    count_computations: &mut usize,
) -> Vec<(usize, Point, MirDirection)> {
    if respawn.count == 0 {
        return Vec::new();
    }

    if respawn.count == 1 && respawn.spread == 0 {
        if point_in_data_range(&respawn.location, player_position) {
            return vec![(0, respawn.location.clone(), respawn.direction)];
        }
        return Vec::new();
    }

    let spread = i32::from(respawn.spread);
    if spread <= 0 {
        return Vec::new();
    }

    let data_min_x = player_position.x - CRYSTAL_DATA_RANGE;
    let data_max_x = player_position.x + CRYSTAL_DATA_RANGE;
    let data_min_y = player_position.y - CRYSTAL_DATA_RANGE;
    let data_max_y = player_position.y + CRYSTAL_DATA_RANGE;
    let spawn_min_x = respawn.location.x - spread;
    let spawn_max_x = respawn.location.x + spread;
    let spawn_min_y = respawn.location.y - spread;
    let spawn_max_y = respawn.location.y + spread;

    let min_x = data_min_x.max(spawn_min_x);
    let max_x = data_max_x.min(spawn_max_x);
    let min_y = data_min_y.max(spawn_min_y);
    let max_y = data_max_y.min(spawn_max_y);
    if min_x > max_x || min_y > max_y {
        return Vec::new();
    }

    let visible_candidates = full_collision.as_ref().map(|collision| {
        walkable_points_in_rect(collision, min_x, max_x, min_y, max_y)
            .into_iter()
            .filter(|point| !crystal_monster_spawn_is_in_arrival_protection(map_file_name, point))
            .collect::<Vec<_>>()
    });
    let spawn_candidate_count = full_collision.as_ref().map(|collision| {
        *count_computations += 1;
        walkable_point_count_in_rect(
            collision,
            spawn_min_x,
            spawn_max_x,
            spawn_min_y,
            spawn_max_y,
        )
    });
    let (visible_count, visible_cells, width) =
        if let (Some(visible_candidates), Some(spawn_candidate_count)) =
            (&visible_candidates, spawn_candidate_count)
        {
            if spawn_candidate_count == 0 || visible_candidates.is_empty() {
                return Vec::new();
            }
            let visible_count = ((usize::from(respawn.count) * visible_candidates.len())
                / spawn_candidate_count)
                .min(usize::from(respawn.count));
            (
                visible_count,
                u64::try_from(visible_candidates.len()).expect("visible cell count should fit u64"),
                0,
            )
        } else {
            let visible_width = i64::from(max_x - min_x + 1);
            let visible_height = i64::from(max_y - min_y + 1);
            let spawn_side = i64::from(spread * 2 + 1);
            let visible_area = visible_width * visible_height;
            let spawn_area = spawn_side * spawn_side;
            let visible_count = ((i64::from(respawn.count) * visible_area) / spawn_area)
                .clamp(0, i64::from(respawn.count)) as usize;
            (
                visible_count,
                u64::try_from(visible_area).expect("visible area should fit u64"),
                i32::try_from(visible_width).expect("visible width should fit i32"),
            )
        };
    let representative_visible_count =
        if visible_count == 0 && point_in_data_range(&respawn.location, player_position) {
            1
        } else {
            visible_count
        };
    if representative_visible_count == 0 {
        return Vec::new();
    }

    let mut used = BTreeSet::new();
    let mut spawns = Vec::new();

    for slot_index in 0..representative_visible_count {
        let base = deterministic_roll(
            0,
            respawn.respawn_index.max(0) as usize,
            slot_index,
            visible_cells,
        );
        let mut cell_index = i32::try_from(base).expect("visible cell index should fit i32");
        for _ in 0..visible_cells {
            let point = if let Some(visible_candidates) = &visible_candidates {
                visible_candidates[cell_index as usize].clone()
            } else {
                Point {
                    x: min_x + (cell_index % width),
                    y: min_y + (cell_index / width),
                }
            };
            let x = point.x;
            let y = point.y;
            if !crystal_monster_spawn_is_in_arrival_protection(map_file_name, &point)
                && used.insert((x, y))
            {
                let direction = crystal_respawn_dynamic_direction(respawn, slot_index);
                spawns.push((slot_index, point, direction));
                break;
            }
            cell_index = (cell_index + 1) % i32::try_from(visible_cells).unwrap_or(i32::MAX);
        }
    }

    spawns
}

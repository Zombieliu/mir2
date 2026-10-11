//! Terrain availability is a transfer prerequisite, not a Map.Load receipt.
use super::*;

#[test]
fn manifest_destination_rejects_missing_terrain() {
    assert!(!crystal_manifest_movement_destination_is_valid(
        "missing-transfer-terrain-20261010",
        &Point { x: 10, y: 10 },
    ));
}

#[test]
fn manifest_destination_checks_world_bounds_and_base_terrain() {
    let collision = runtime_world_map_collision_data("2").expect("real Serpent Valley terrain");
    let bounds = collision.collision.region_bounds;
    assert!(!crystal_manifest_movement_destination_is_valid(
        "2",
        &Point { x: bounds.max_x + 1, y: bounds.min_y },
    ));
    let &(x, y) = collision.blocked_set.iter().next().expect("real blocked terrain");
    assert!(!crystal_manifest_movement_destination_is_valid("2", &Point { x, y }));
}

#[test]
fn manifest_destination_keeps_real_closed_door_landing_cell() {
    let collision = runtime_world_map_collision_data("2").expect("real Serpent Valley terrain");
    let destination = Point { x: 517, y: 492 };
    assert!(collision.closed_door_set.contains(&(destination.x, destination.y)));
    assert!(!collision.blocked_set.contains(&(destination.x, destination.y)));
    assert!(crystal_manifest_movement_destination_is_valid("2", &destination));
}

#[test]
fn gzip_only_destination_uses_real_bounds_and_base_terrain() {
    // The shipping pack contains maps absent from a raw client installation.
    // Inspect the real files without replacing caches or changing environment.
    let mut names: Vec<_> = std::fs::read_dir(crystal_map_pack_dir().expect("shipping map pack"))
        .expect("read shipping map pack")
        .map(|entry| entry.expect("map pack directory entry").file_name().to_string_lossy().into_owned())
        .filter_map(|name| name.strip_suffix(".map.gz").map(str::to_owned))
        .collect();
    names.sort();
    let (name, collision) = names.into_iter()
        .filter(|name| crystal_map_path(name).is_none())
        .find_map(|name| runtime_world_map_collision_data(&name)
            .filter(|terrain| !terrain.blocked_set.is_empty())
            .map(|terrain| (name, terrain)))
        .expect("shipping gzip terrain without a corresponding raw file");
    assert!(runtime_full_map_collision_data(&name).is_none());
    let &(x, y) = collision.blocked_set.iter().next().unwrap();
    assert!(!crystal_manifest_movement_destination_is_valid(&name, &Point { x, y }));
    let bounds = collision.collision.region_bounds;
    assert!(!crystal_manifest_movement_destination_is_valid(
        &name, &Point { x: bounds.max_x + 1, y: bounds.min_y },
    ));
    let valid = collision.collision.region_bounds;
    let point = (valid.min_y..=valid.max_y).find_map(|y|
        (valid.min_x..=valid.max_x)
            .find(|x| !collision.blocked_set.contains(&(*x, y)))
            .map(|x| Point { x, y }))
        .expect("real walkable destination");
    assert!(crystal_manifest_movement_destination_is_valid(&name, &point));
}

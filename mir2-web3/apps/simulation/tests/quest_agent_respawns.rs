use mir2_game_data::crystal_map_respawns_by_file_name;
use mir2_protocol::Point;
use mir2_simulation::{crystal_world_respawn_spawns, SimulationConfig};

fn chebyshev(left: &Point, right: &Point) -> i32 {
    left.x.abs_diff(right.x).max(left.y.abs_diff(right.y)) as i32
}

#[test]
fn q25_cannibal_plant_groups_have_agent_patrol_coverage() {
    let groups = crystal_map_respawns_by_file_name("0")
        .expect("Bichon should have Crystal respawns")
        .respawns
        .into_iter()
        .filter(|respawn| respawn.monster_name == "CannibalPlant")
        .collect::<Vec<_>>();
    assert_eq!(
        groups.len(),
        4,
        "q25 expects four Bichon CannibalPlant groups"
    );

    for group in groups {
        let spawns = crystal_world_respawn_spawns("0", &group);
        assert_eq!(
            spawns.len(),
            usize::from(group.count),
            "every configured q25 slot should have a walkable world position",
        );
        let step = if group.spread > 16 {
            i32::from((group.spread / 2).clamp(12, 28))
        } else {
            0
        };
        let offsets = if step > 0 {
            vec![
                (0, 0),
                (-step, -step),
                (step, -step),
                (-step, step),
                (step, step),
                (-step, 0),
                (step, 0),
                (0, -step),
                (0, step),
            ]
        } else {
            vec![(0, 0)]
        };
        let patrol = offsets
            .into_iter()
            .map(|(dx, dy)| Point {
                x: group.location.x + dx,
                y: group.location.y + dy,
            })
            .collect::<Vec<_>>();
        let nearest = spawns
            .iter()
            .flat_map(|(_, spawn, _)| patrol.iter().map(move |point| chebyshev(spawn, point)))
            .min()
            .expect("q25 group should have at least one spawn and patrol point");
        assert!(
            nearest <= 20,
            "q25 patrol must enter the private runtime's 20-tile activation ring",
        );
    }
}

#[test]
fn audited_serpent_mine_repairs_share_the_runtime_spawn_path() {
    let config = SimulationConfig::default().with_platinum_176_profile();
    for (map_file_name, monster_name, expected_count) in [
        ("D421", "ChainGhoul", 15_u16),
        ("D422", "RotNdZombie", 12_u16),
    ] {
        let matches = config
            .crystal_respawns_for_map(map_file_name)
            .into_iter()
            .filter(|respawn| respawn.monster_name == monster_name)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "the profile repair should be unique");
        let respawn = &matches[0];
        assert_eq!(respawn.count, expected_count);
        assert!(respawn.respawn_index >= 10_000);
        assert_eq!(
            crystal_world_respawn_spawns(map_file_name, respawn).len(),
            usize::from(expected_count),
            "every repaired slot should resolve to a walkable world position",
        );
    }
}

#[test]
fn dead_mine_arrivals_are_clear_of_manifest_monster_spawns() {
    let map = crystal_map_respawns_by_file_name("D401")
        .expect("Dead Mine entrance should have Crystal respawns");
    let arrivals = [
        Point { x: 24, y: 181 },
        Point { x: 25, y: 181 },
        Point { x: 77, y: 14 },
        Point { x: 179, y: 108 },
    ];
    let mut normal_area_spawn_count = 0;

    for respawn in &map.respawns {
        let spawns = crystal_world_respawn_spawns("D401", respawn);
        assert_eq!(
            spawns.len(),
            usize::from(respawn.count),
            "arrival protection should relocate broad-area slots, not remove them",
        );
        for (_, spawn, _) in spawns {
            assert!(
                arrivals
                    .iter()
                    .all(|arrival| chebyshev(&spawn, arrival) > 8),
                "{} spawned at ({},{}) inside an arrival protection ring",
                respawn.monster_name,
                spawn.x,
                spawn.y,
            );
            if arrivals
                .iter()
                .all(|arrival| chebyshev(&spawn, arrival) > 20)
            {
                normal_area_spawn_count += 1;
            }
        }
    }

    assert!(
        normal_area_spawn_count > 0,
        "D401 must keep ordinary monster coverage away from its entrances",
    );
}

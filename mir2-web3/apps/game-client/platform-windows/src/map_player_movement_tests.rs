use super::*;

#[test]
fn ordinary_bichon_entrance_exempts_only_its_authored_raw_wall_cell() {
    let map = load_map("0").expect("packaged Bichon map");
    // Bichon -> 0111 is an actual front-image wall with an ordinary valid
    // movement. The reagent shop at (326,288) has a door flag but no raw wall
    // bit; do not confuse the protocol runner's closed-door bitmap with this
    // parser's immutable back/front collision.
    assert!(
        map.cell_blocks_movement(399, 225),
        "retain the raw art-cell collision"
    );
    assert_eq!(map_cell_blocks_movement("0", 399, 225), Some(true));
    assert!(!map.cell_blocks_movement(326, 288));
    for file in ["0", "0.map", "0.map.gz"] {
        assert_eq!(map_cell_blocks_player_movement(file, 399, 225), Some(false));
        assert_eq!(map_cell_blocks_player_movement(file, 326, 288), Some(false));
        for (x, y) in [
            (398, 225),
            (399, 224),
            (398, 226),
            (-1, 225),
            (700, 225),
            (399, 700),
        ] {
            assert_eq!(
                map_cell_blocks_player_movement(file, x, y),
                Some(true),
                "{file}: {x},{y}"
            );
        }
    }
    assert_eq!(map_cell_blocks_player_movement("../0", 399, 225), None);
    assert!(
        map.cell_blocks_movement(399, 225),
        "player exemption must never mutate map cells"
    );
}

#[test]
fn entrance_exemption_requires_an_ordinary_edge_and_a_valid_landing() {
    let manifest = mir2_game_data::crystal_respawn_manifest_ref();
    let source = manifest
        .maps
        .iter()
        .find(|map| map.map_file_name == "0")
        .unwrap();
    let target = manifest
        .maps
        .iter()
        .find(|map| map.map_file_name == "0111")
        .unwrap();
    let movement = source
        .movements
        .iter()
        .find(|edge| edge.source.x == 399 && edge.source.y == 225)
        .unwrap();
    let map = load_map("0").expect("packaged Bichon map");
    assert!(
        map.cell_blocks_movement(399, 225),
        "negative cases require a real raw obstacle"
    );

    for reason in [
        "hole",
        "move",
        "conquest",
        "unknown-map",
        "zero-landing",
        "outside-landing",
        "wall-landing",
    ] {
        let mut edge = movement.clone();
        match reason {
            "hole" => edge.need_hole = true,
            "move" => edge.need_move = true,
            "conquest" => edge.conquest_index = 1,
            "unknown-map" => edge.map_index = i32::MAX,
            "zero-landing" => {
                edge.destination.x = 0;
                edge.destination.y = 0;
            }
            "outside-landing" => edge.destination.x = i32::MAX,
            "wall-landing" => {
                edge.destination.x = 0;
                edge.destination.y = 1;
            }
            _ => unreachable!(),
        }
        let mut source = source.clone();
        source.movements = vec![edge];
        let index = build_ordinary_player_entrances(&[source, target.clone()]);
        let collision = PlayerMovementCollision {
            map: &map,
            entrances: index.get("0"),
        };
        assert!(
            collision.cell_blocks_movement(399, 225),
            "must reject {reason}"
        );
    }

    let mut source = source.clone();
    let mut edge = movement.clone();
    edge.source.x = i32::from(map.width);
    source.movements = vec![edge];
    let index = build_ordinary_player_entrances(&[source, target.clone()]);
    let collision = PlayerMovementCollision {
        map: &map,
        entrances: index.get("0"),
    };
    assert!(
        collision.cell_blocks_movement(i32::from(map.width), 225),
        "authored cells outside the loaded map stay blocked"
    );
}

#[test]
fn missing_destination_map_does_not_cache_a_permanent_collision_answer() {
    let destination = OrdinaryEntranceDestination {
        map_file_name: "missing-native-entrance-fixture".into(),
        x: 1,
        y: 1,
        valid_landing: OnceLock::new(),
    };
    assert!(!destination.has_valid_landing());
    assert!(destination.valid_landing.get().is_none());
}

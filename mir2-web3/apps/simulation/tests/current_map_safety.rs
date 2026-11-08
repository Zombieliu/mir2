use mir2_game_data::{crystal_map_respawns_ref, MapBounds};
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};
use mir2_simulation::{
    InProcessWorldRuntime, SafeZoneRecord, SimulationConfig, SimulationSession, WorldRuntime,
};

fn contains(bounds: &[MapBounds], point: &Point) -> bool {
    bounds.iter().any(|zone| {
        point.x >= zone.min_x
            && point.x <= zone.max_x
            && point.y >= zone.min_y
            && point.y <= zone.max_y
    })
}

fn configured_zone(map: &str, bounds: MapBounds) -> SafeZoneRecord {
    SafeZoneRecord {
        map_file_name: map.to_string(),
        bounds,
    }
}

#[test]
fn current_map_safety_merges_normalized_config_with_inclusive_crystal_rectangles() {
    let extra = MapBounds {
        min_x: 342,
        max_x: 344,
        min_y: 277,
        max_y: 279,
    };
    let foreign = MapBounds {
        min_x: 450,
        max_x: 452,
        min_y: 450,
        max_y: 452,
    };
    let mut config = SimulationConfig::default();
    config.map.file_name = " 0.MAP ".to_string();
    config.safe_zones = vec![
        configured_zone("0.map", extra),
        configured_zone(" 0.MAP ", extra),
        configured_zone("0141", foreign),
    ];
    let session = SimulationSession::new(config);
    let before = session.world_snapshot();
    let actual = session.current_map_safe_zone_bounds();
    assert_eq!(
        session.world_snapshot(),
        before,
        "query must not tick or mutate player state"
    );
    assert_eq!(actual.iter().filter(|bounds| **bounds == extra).count(), 1);
    assert!(!actual.contains(&foreign));
    let native = crystal_map_respawns_ref("0").expect("Bichon map exists");
    assert!(!native.safe_zones.is_empty());
    for zone in &native.safe_zones {
        let size = i32::from(zone.size);
        let expected = MapBounds {
            min_x: zone.location.x - size,
            max_x: zone.location.x + size,
            min_y: zone.location.y - size,
            max_y: zone.location.y + size,
        };
        assert!(actual.contains(&expected));
        for point in [
            Point {
                x: expected.min_x,
                y: expected.min_y,
            },
            Point {
                x: expected.max_x,
                y: expected.max_y,
            },
        ] {
            assert!(
                contains(&actual, &point),
                "inclusive native boundary {point:?}"
            );
        }
    }
    assert!(contains(&actual, &Point { x: 288, y: 616 }));
    assert!(
        !contains(&actual, &Point { x: 272, y: 632 }),
        "the hunting field is outside town safety"
    );
}

#[test]
fn current_map_safety_filters_other_maps_and_unknown_maps_do_not_inherit_bichon() {
    let indoor = MapBounds {
        min_x: 10,
        max_x: 12,
        min_y: 20,
        max_y: 22,
    };
    let bichon = MapBounds {
        min_x: 100,
        max_x: 102,
        min_y: 100,
        max_y: 102,
    };
    for map_name in ["0141", " 0141.MAP ", "not-a-crystal-map"] {
        let mut config = SimulationConfig::default();
        config.map.file_name = map_name.to_string();
        config.safe_zones = vec![
            configured_zone("0141.map", indoor),
            configured_zone("0", bichon),
        ];
        let session = SimulationSession::new(config);
        let actual = session.current_map_safe_zone_bounds();
        if map_name.trim().to_ascii_lowercase().starts_with("0141") {
            assert_eq!(actual, vec![indoor]);
        } else {
            assert!(actual.is_empty());
        }
        assert!(!actual.contains(&bichon));
        assert!(!contains(&actual, &Point { x: 288, y: 616 }));
    }
}

#[test]
fn current_map_safety_matches_existing_snapshot_point_checks_without_mutating_state() {
    let extra = MapBounds {
        min_x: 342,
        max_x: 344,
        min_y: 277,
        max_y: 279,
    };
    let mut config = SimulationConfig::default();
    config.safe_zones.push(configured_zone(" 0.MAP ", extra));
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string()
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    for point in [
        Point { x: 342, y: 277 },
        Point { x: 344, y: 279 },
        Point { x: 343, y: 278 },
        Point { x: 345, y: 278 },
        Point { x: 278, y: 606 },
        Point { x: 298, y: 626 },
        Point { x: 277, y: 616 },
        Point { x: 272, y: 632 },
    ] {
        session.force_authoritative_player_transform(point.clone(), MirDirection::Down);
        let before = session.world_snapshot();
        let actual = session.current_map_safe_zone_bounds();
        assert_eq!(
            contains(&actual, &point),
            before.in_safe_zone,
            "reference safe-zone semantics at {point:?}"
        );
        assert_eq!(
            session.world_snapshot(),
            before,
            "query must not change tick, HP, position, or other state"
        );
    }
}

#[test]
fn in_process_runtime_exposes_the_same_read_only_safe_zone_bounds() {
    let config = SimulationConfig::default();
    let expected = SimulationSession::new(config.clone()).current_map_safe_zone_bounds();
    let runtime = InProcessWorldRuntime::new(config);
    let before = runtime.world_snapshot();
    assert_eq!(runtime.current_map_safe_zone_bounds(), expected);
    assert_eq!(runtime.world_snapshot(), before);
}

//! Real shared Zone livestock updates. No animation injection, checkpoint root
//! replacement, forced random draw or personal-session monster ticks.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneBounds, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterSpawn, ZoneOutbound, ZoneRuntime,
};
use serde_json::Value;

const MONSTER: u32 = 9_001;
const MAP: &str = "livestock-roam-fixture";
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn state(zone: &ZoneRuntime) -> Value {
    serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap()
}
fn join(zone: &mut ZoneRuntime, id: u32, at: Point) -> Vec<ZoneOutbound> {
    let mut profile = mir2_simulation::ZoneChatProfile::default();
    profile.in_safe_zone = true;
    let map_file_name = zone.key().map_file_name.clone();
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new(format!("observer-{id}")),
        account_id: format!("roam-account-{id}"),
        character_index: id as i32,
        object_id: id,
        name: format!("Roam{id}"),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 1,
        hp: 100,
        max_hp: 100,
        mp: 100,
        map_file_name,
        position: at,
        direction: MirDirection::Up,
        chat_profile: profile,
        combat_stats: Default::default(),
    }))
}
fn fixture(ai: u8, collision: ZoneCollision) -> ZoneRuntime {
    fixture_on_map(ai, collision, MAP, point(10, 10))
}
fn fixture_on_map(ai: u8, collision: ZoneCollision, map: &str, at: Point) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map(map), collision);
    assert!(
        zone.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: MONSTER,
                name: if ai == 2 { "Deer" } else { "Hen" }.into(),
                name_colour_argb: -1,
                image: 1,
                ai,
                disposition: Some(WorldEntityDisposition::Neutral),
                level: 1,
                max_hp: 100,
                hp: 100,
                experience: 0,
                move_speed_ms: 1_400,
                attack_speed_ms: 1_400,
                friendly_guild: None,
                defense: Default::default(),
                position: at,
                direction: MirDirection::Down,
                respawn: None,
                drops: Vec::new(),
            },
            0
        )
        .0
    );
    zone
}
fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .collect()
}
fn walking(packet: &ServerPacket) -> bool {
    matches!(packet, ServerPacket::ObjectWalk { movement } if movement.object_id == MONSTER)
}

#[test]
fn neutral_hen_and_deer_naturally_roam_in_safe_town_with_authoritative_pacing() {
    for ai in [1, 2] {
        let mut zone = fixture(ai, ZoneCollision::unbounded());
        join(&mut zone, 101, point(13, 13));
        // The first normal update initializes the original timid subtype.
        zone.tick(1);
        let speed = state(&zone)["native_monsters"][MONSTER.to_string()]["move_speed_ms"]
            .as_u64()
            .unwrap();
        let mut previous = point(10, 10);
        let mut walked = 0;
        let mut turned = 0;
        let mut last_walk = None;
        for now in (101..=65_001).step_by(100) {
            let out = zone.tick(now);
            for packet in packets(&out) {
                if walking(packet) {
                    let ServerPacket::ObjectWalk { movement } = packet else {
                        unreachable!()
                    };
                    assert_eq!(
                        (movement.position.x - previous.x)
                            .abs()
                            .max((movement.position.y - previous.y).abs()),
                        1
                    );
                    if let Some(last) = last_walk {
                        assert!(now - last > speed);
                    }
                    last_walk = Some(now);
                    walked += 1;
                    previous = movement.position.clone();
                    assert_eq!(zone.native_monster_snapshots()[0].position, previous);
                }
                if matches!(packet, ServerPacket::ObjectTurn { movement } if movement.object_id == MONSTER)
                {
                    turned += 1;
                }
                assert!(!matches!(
                    packet,
                    ServerPacket::ObjectAttack { .. } | ServerPacket::ObjectRangeAttack { .. }
                ));
            }
        }
        assert!(walked > 0, "AI{ai} must inherit natural ProcessRoam");
        assert!(turned > 0, "AI{ai} must sometimes turn");
        assert_eq!(
            state(&zone)["native_monsters"][MONSTER.to_string()]["hostile_to_player"],
            false
        );
    }
}

#[test]
fn empty_or_far_away_map_does_not_advance_roam_draws_or_move_livestock() {
    let mut zone = fixture(1, ZoneCollision::unbounded());
    join(&mut zone, 101, point(100, 100));
    zone.tick(1);
    let initial =
        state(&zone)["native_monsters"][MONSTER.to_string()]["special_ai"]["idle_roam"].clone();
    for now in (101..=60_001).step_by(100) {
        zone.tick(now);
    }
    let saved = state(&zone);
    let monster = &saved["native_monsters"][MONSTER.to_string()];
    assert_eq!(monster["position"], serde_json::json!({"x":10,"y":10}));
    assert_eq!(
        monster["special_ai"]["idle_roam"]["rng_state"],
        initial["rng_state"]
    );
    zone.handle(ZoneCommand::Leave {
        session_id: SessionId::new("observer-101"),
    });
    for now in (60_101..=70_001).step_by(100) {
        assert!(packets(&zone.tick(now)).iter().all(|p| !walking(p)));
    }
    join(&mut zone, 102, point(13, 13));
    let mut walks = 0;
    for now in (70_101..=135_001).step_by(100) {
        walks += packets(&zone.tick(now))
            .iter()
            .filter(|p| walking(p))
            .count();
    }
    assert!(walks > 0, "normal player presence wakes the same animal");
}

#[test]
fn solid_tiles_and_players_block_roaming_without_a_detour_or_fake_walk() {
    let blocked =
        ZoneCollision::unbounded().with_blocked_cells((9..=11).flat_map(|x| {
            (9..=11).filter_map(move |y| ((x, y) != (10, 10)).then_some(point(x, y)))
        }));
    let mut zone = fixture(1, blocked);
    join(&mut zone, 101, point(13, 13));
    for now in (1..=65_001).step_by(100) {
        assert!(!packets(&zone.tick(now)).iter().any(|p| walking(p)));
        assert_eq!(zone.native_monster_snapshots()[0].position, point(10, 10));
    }
    let bounded = ZoneCollision::unbounded().with_bounds(ZoneBounds::new(9, 11, 9, 11));
    let mut occupied = fixture(1, bounded);
    let mut id = 100;
    for x in 9..=11 {
        for y in 9..=11 {
            if (x, y) == (10, 10) {
                continue;
            }
            id += 1;
            join(&mut occupied, id, point(x, y));
        }
    }
    for now in (1..=65_001).step_by(100) {
        assert!(!packets(&occupied.tick(now)).iter().any(|p| walking(p)));
        assert_eq!(
            occupied.native_monster_snapshots()[0].position,
            point(10, 10)
        );
    }
}

#[test]
fn checkpoint_preserves_random_position_deadlines_and_missing_optional_state() {
    // Cold recovery rebuilds terrain from the map key, so use the real imported
    // map rather than an unbounded test arena with a different collision root.
    let mut zone = fixture_on_map(1, ZoneCollision::for_map("0"), "0", point(288, 616));
    assert!(zone.has_available_collision());
    join(&mut zone, 101, point(291, 616));
    // Pre-feature shape: absent optional state does not change canonical bytes.
    let old = zone.checkpoint_bytes().unwrap();
    assert!(state(&zone)["native_monsters"][MONSTER.to_string()]["special_ai"].is_null());
    let restored = ZoneRuntime::restore_checkpoint(&old).unwrap();
    assert_eq!(
        state(&restored)["native_monsters"],
        state(&zone)["native_monsters"]
    );
    for now in (1..=12_001).step_by(100) {
        zone.tick(now);
    }
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    assert!(
        state(&zone)["native_monsters"][MONSTER.to_string()]["special_ai"]["idle_roam"].is_object()
    );
    assert_eq!(
        state(&restored)["native_monsters"],
        state(&zone)["native_monsters"]
    );
    for now in (12_101..=65_001).step_by(100) {
        assert_eq!(zone.tick(now), restored.tick(now));
        // Cold restore deliberately retires live online ownership. Compare the
        // retained animal/packet stream, not the old admission authority.
        assert_eq!(
            state(&zone)["native_monsters"],
            state(&restored)["native_monsters"]
        );
    }
    let mut tampered = state(&zone);
    tampered["native_monsters"][MONSTER.to_string()]["special_ai"]["idle_roam"]["rng_state"] =
        1.into();
    assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&tampered).unwrap()).is_err());
}

#[test]
fn aoi_broadcast_excludes_far_observers_and_reentry_uses_current_transform() {
    let mut zone = fixture(1, ZoneCollision::unbounded());
    join(&mut zone, 101, point(13, 13));
    join(&mut zone, 102, point(14, 13));
    join(&mut zone, 103, point(100, 100));
    let mut walks = 0;
    for now in (1..=65_001).step_by(100) {
        for out in zone.tick(now) {
            if let ZoneOutbound::ToMany {
                session_ids,
                packets,
            } = out
            {
                if packets.iter().any(walking) {
                    assert!(!session_ids.contains(&SessionId::new("observer-103")));
                    walks += 1;
                }
            }
        }
    }
    assert!(walks > 0);
    let current = zone.native_monster_snapshots()[0].position.clone();
    let entered = join(&mut zone, 104, point(current.x + 2, current.y));
    assert!(packets(&entered).iter().any(|p| matches!(p,ServerPacket::ObjectMonster{info} if info.object_id == MONSTER && info.location == current)));
}

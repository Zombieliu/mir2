use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterSpawn, ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;

fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn zone() -> ZoneRuntime {
    zone_with_collision(ZoneCollision::unbounded())
}
fn zone_with_collision(collision: ZoneCollision) -> ZoneRuntime {
    zone_with_collision_and_target(collision, point(10, 11))
}
fn zone_with_collision_and_target(collision: ZoneCollision, target: Point) -> ZoneRuntime {
    zone_with_ai(collision, target, 8, 0)
}
fn zone_with_ai(
    collision: ZoneCollision,
    target: Point,
    ai: u8,
    poison_resist: i32,
) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("reactive-fixture"), collision);
    for (name, id, position) in [("a", 101, target), ("b", 102, point(16, 15))] {
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new(name),
            account_id: name.into(),
            character_index: 1,
            object_id: id,
            name: name.into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: if ai == 8 { 40 } else { 20 },
            hp: 100_000,
            max_hp: 100_000,
            mp: 100,
            map_file_name: "reactive-fixture".into(),
            position,
            direction: MirDirection::Up,
            chat_profile: Default::default(),
            combat_stats: ZonePlayerCombatStats {
                poison_resist,
                ..Default::default()
            },
        }));
    }
    assert!(
        zone.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                object_id: 9008,
                name: match ai {
                    38 => "HolyDeva",
                    45 => "RedFoxman",
                    46 => "WhiteFoxman",
                    _ => "AxeSkeleton",
                }
                .into(),
                name_colour_argb: -1,
                image: 8,
                ai,
                disposition: Some(WorldEntityDisposition::Hostile),
                level: 30,
                max_hp: 100,
                hp: 100,
                experience: 0,
                move_speed_ms: 400,
                attack_speed_ms: 800,
                friendly_guild: None,
                defense: Default::default(),
                position: point(10, 10),
                direction: MirDirection::Down,
                respawn: None,
                drops: Vec::new()
            },
            0
        )
        .0
    );
    zone
}

#[test]
fn blocked_approach_rotates_to_a_free_tile_and_starts_the_attack_window() {
    let mut zone = zone_with_collision_and_target(
        ZoneCollision::unbounded().with_blocked_cells([point(10, 11)]),
        point(10, 16),
    );
    let out = zone.tick(1);
    let position = zone.native_monster_snapshots()[0].position.clone();
    assert_ne!(
        position,
        point(10, 10),
        "blocked direct approach must still try alternatives"
    );
    assert_ne!(
        position,
        point(10, 11),
        "must respect the static obstruction"
    );
    assert_eq!(
        position.y, 11,
        "first free adjacent direction still approaches the distant target"
    );
    assert_eq!(
        monster(&zone)["special_ai"]["reactive"]["fear_until_ms"],
        5_001
    );
    assert!(packets(&out, "a")
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectWalk { .. })));
    assert!(!packets(&out, "a")
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectRangeAttack { .. })));
}

#[test]
fn blocked_retreat_tries_a_free_direction_instead_of_overlapping_or_attacking() {
    let mut zone =
        zone_with_collision(ZoneCollision::unbounded().with_blocked_cells([point(10, 9)]));
    let out = zone.tick(1);
    let position = zone.native_monster_snapshots()[0].position.clone();
    assert_ne!(position, point(10, 9));
    assert_ne!(position, point(10, 10));
    assert_ne!(position, point(10, 11));
    assert!(packets(&out, "a")
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectWalk { .. })));
    assert!(!packets(&out, "a")
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectRangeAttack { .. })));
}
fn packets<'a>(out: &'a [ZoneOutbound], recipient: &str) -> Vec<&'a ServerPacket> {
    let id = SessionId::new(recipient);
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == id => packets.iter().collect(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&id) => packets.iter().collect(),
            ZoneOutbound::ToAll { packets } => packets.iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}
fn monster(zone: &ZoneRuntime) -> Value {
    let all: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    all["native_monsters"]["9008"].clone()
}

fn white_slow_cast(resist: i32) -> (ZoneRuntime, u64) {
    let mut zone = zone_with_ai(ZoneCollision::unbounded(), point(10, 11), 46, resist);
    zone.tick(1);
    for n in 0..96 {
        let now = 402 + n * 801;
        let out = zone.tick(now);
        if packets(&out,"a").iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info} if info.object_id==9008&&info.attack_type==1)) {
            return (zone,now);
        }
    }
    panic!("deterministic white fox fixture did not select its slow spell");
}

#[test]
fn white_fox_slow_is_delayed_non_damage_and_checkpoint_replays_one_cast() {
    let (mut zone, cast) = white_slow_cast(0);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(zone.tick(cast + 299), restored.tick(cast + 299));
    let effect = zone.tick(cast + 300);
    assert_eq!(effect, restored.tick(cast + 300));
    assert!(packets(&effect, "a")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPoisoned{poison,..} if poison&4!=0)));
    assert!(!effect
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    let again = zone.tick(cast + 301);
    assert!(!packets(&again, "a")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPoisoned{poison,..} if poison&4!=0)));
}

#[test]
fn white_fox_full_poison_resistance_blocks_slow_without_replacing_it_with_damage() {
    let (mut zone, cast) = white_slow_cast(10);
    let effect = zone.tick(cast + 300);
    assert!(!packets(&effect, "a")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPoisoned{poison,..} if poison&4!=0)));
    assert!(!effect
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}

#[test]
fn red_fox_adjacent_teleport_uses_effect_two_and_persists_ten_second_cooldown() {
    let mut zone = zone_with_ai(ZoneCollision::unbounded(), point(10, 11), 45, 0);
    zone.tick(1);
    assert_eq!(zone.native_monster_snapshots()[0].position, point(10, 9));
    zone.handle(ZoneCommand::Walk {
        session_id: SessionId::new("a"),
        direction: MirDirection::Up,
        seq: 1,
        now_ms: 100,
    });
    zone.tick(100);
    let teleport_at = monster(&zone)["next_attack_ready_at_ms"].as_u64().unwrap() + 1;
    let out = zone.tick(teleport_at);
    let visible = packets(&out, "a");
    assert!(visible.iter().any(|p| matches!(
        p,
        ServerPacket::ObjectTeleportOut {
            object_id: 9008,
            effect_type: 2
        }
    )));
    assert!(visible
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectRemove { object_id: 9008 })));
    assert!(!visible
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectWalk{movement} if movement.object_id==9008)));
    assert!(!visible
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info} if info.object_id==9008)));
    let order: Vec<_> = visible
        .iter()
        .filter_map(|p| match p {
            ServerPacket::ObjectTeleportOut {
                object_id: 9008, ..
            } => Some("out"),
            ServerPacket::ObjectRemove { object_id: 9008 } => Some("remove"),
            ServerPacket::ObjectMonster { info } if info.object_id == 9008 => Some("spawn"),
            ServerPacket::ObjectTeleportIn {
                object_id: 9008, ..
            } => Some("in"),
            ServerPacket::ObjectHealth { info } if info.object_id == 9008 => Some("health"),
            _ => None,
        })
        .collect();
    assert_eq!(order, vec!["out", "remove", "spawn", "in", "health"]);
    let after = zone.native_monster_snapshots()[0].position.clone();
    assert!((after.x - 10).abs() <= 14 && (after.y - 9).abs() <= 14);
    assert_eq!(
        monster(&zone)["special_ai"]["reactive"]["teleport_ready_at_ms"],
        teleport_at + 10000
    );
    let restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(monster(&zone), monster(&restored));
}

#[test]
fn axe_skeleton_retreats_before_shooting_and_broadcasts_the_same_move() {
    let mut zone = zone();
    let out = zone.tick(1);
    for recipient in ["a", "b"] {
        let p = packets(&out, recipient);
        assert!(p.iter().any(|p| matches!(p, ServerPacket::ObjectWalk { movement } if movement.object_id == 9008 && movement.position == point(10, 9))));
        assert!(!p.iter().any(
            |p| matches!(p, ServerPacket::ObjectRangeAttack { info } if info.object_id == 9008)
        ));
    }
    assert_eq!(
        monster(&zone)["special_ai"]["reactive"]["fear_until_ms"],
        5_001
    );
    let ready = monster(&zone)["next_attack_ready_at_ms"].as_u64().unwrap();
    assert!(packets(&zone.tick(ready), "a")
        .iter()
        .all(|p| !matches!(p, ServerPacket::ObjectRangeAttack { .. })));
    let out = zone.tick(ready + 1);
    assert!(packets(&out, "a")
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectRangeAttack { info } if info.object_id == 9008)));
    let value: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(
        value["pending_native_player_hits"][0]["ready_at_ms"],
        ready + 1 + 600
    );
}

#[test]
fn fear_expiry_is_strict_and_checkpoint_does_not_restart_the_window() {
    let mut zone = zone();
    zone.tick(1);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(monster(&zone), monster(&restored));
    for now in [5_001, 5_002] {
        let a = zone.tick(now);
        let b = restored.tick(now);
        assert_eq!(a, b);
    }
    assert_eq!(
        monster(&zone)["position"],
        serde_json::json!({"x":10,"y":8})
    );
    assert_eq!(
        monster(&zone)["special_ai"]["reactive"]["fear_until_ms"],
        10_001
    );
}

fn rejoin_reactive_target(zone: &mut ZoneRuntime, level: u16) {
    zone.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("a"),
        account_id: "a".into(),
        character_index: 1,
        object_id: 101,
        name: "a".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level,
        hp: 100000,
        max_hp: 100000,
        mp: 100,
        map_file_name: "reactive-fixture".into(),
        position: point(10, 11),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
}
#[test]
fn white_fox_old_life_slow_is_cancelled_before_same_id_rejoin() {
    let (mut zone, cast) = white_slow_cast(0);
    assert!(!monster(&zone)["special_ai"]["reactive"]["pending_slows"]
        .as_array()
        .unwrap()
        .is_empty());
    rejoin_reactive_target(&mut zone, 20);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let out = zone.tick(cast + 300);
    assert_eq!(out, restored.tick(cast + 300));
    assert!(!packets(&out, "a")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPoisoned{object_id:101,poison}if poison&4!=0)));
    assert_eq!(zone.player_vitals(&SessionId::new("a")).unwrap().0, 100000);
}
#[test]
fn axe_projectile_cannot_damage_a_rejoined_player_with_the_same_id() {
    let mut zone = zone();
    zone.tick(1);
    let ready = monster(&zone)["next_attack_ready_at_ms"].as_u64().unwrap();
    zone.tick(ready + 1);
    let saved: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    let due = saved["pending_native_player_hits"][0]["ready_at_ms"]
        .as_u64()
        .unwrap();
    rejoin_reactive_target(&mut zone, 40);
    let out = zone.tick(due);
    assert!(!out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new("a"))));
    assert_eq!(zone.player_vitals(&SessionId::new("a")).unwrap().0, 100000);
}

#[test]
fn wild_holy_deva_retreated_shot_is_500ms_dc_against_mac() {
    let mut zone = zone_with_ai(ZoneCollision::unbounded(), point(10, 11), 38, 0);
    zone.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("a"),
        stats: ZonePlayerCombatStats {
            min_ac: 1000,
            max_ac: 1000,
            ..Default::default()
        },
    });
    assert!(zone
        .tick(1999)
        .iter()
        .all(|o| !matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    let retreat = zone.tick(2001);
    assert!(retreat.iter().any(|o| match o {
        ZoneOutbound::ToMany { packets, .. } => packets
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectWalk{movement}if movement.object_id==9008)),
        _ => false,
    }));
    let cast_at = monster(&zone)["next_ai_ready_at_ms"]
        .as_u64()
        .unwrap()
        .max(monster(&zone)["next_attack_ready_at_ms"].as_u64().unwrap() + 1);
    let cast = zone.tick(cast_at);
    assert!(cast.iter().any(|o| match o {
        ZoneOutbound::ToMany { packets, .. } => packets
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==9008)),
        _ => false,
    }));
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    assert_eq!(zone.tick(cast_at + 499), restored.tick(cast_at + 499));
    let out = zone.tick(cast_at + 500);
    assert_eq!(out, restored.tick(cast_at + 500));
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged { session_id, damage, .. }if *session_id==SessionId::new("a")&&*damage>=23&&*damage<=35)));
    let mut armoured = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    armoured.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("a"),
        stats: ZonePlayerCombatStats {
            min_mac: 1000,
            max_mac: 1000,
            ..Default::default()
        },
    });
    assert!(!armoured.tick(cast_at+500).iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new("a"))));
}

#[test]
fn wild_holy_deva_does_not_chase_outside_its_six_tile_range() {
    let mut zone = zone_with_ai(ZoneCollision::unbounded(), point(17, 10), 38, 0);
    zone.handle(ZoneCommand::Leave {
        session_id: SessionId::new("b"),
    });
    for now in [2001, 2002, 4002, 8002] {
        zone.tick(now);
    }
    assert_eq!(zone.native_monster_snapshots()[0].position, point(10, 10));
    let state: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert!(
        state["native_monsters"]["9008"]["special_ai"]["reactive"]["fear_until_ms"]
            .as_u64()
            .unwrap()
            > 8002
    );
}

//! Shared ownership and AOI contracts for Crystal BoneLord / ZumaTaurus waves.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZoneRuntime,
};
use serde_json::Value;
use std::collections::BTreeSet;

const BOSS: u32 = 9017;
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn join(name: &str, id: u32, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(name),
        account_id: name.into(),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 100_000,
        max_hp: 100_000,
        mp: 100,
        map_file_name: "stage-summon-fixture".into(),
        position,
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}
fn fixture(ai: u8, players: bool) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("stage-summon-fixture"));
    if players {
        zone.handle(ZoneCommand::Join(join("a", 101, point(10, 11))));
        zone.handle(ZoneCommand::Join(join("b", 102, point(14, 12))));
        zone.handle(ZoneCommand::Join(join("far", 103, point(100, 100))));
    }
    let spawn = ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: BOSS,
        name: if ai == 30 { "BoneLord" } else { "ZumaTaurus" }.into(),
        name_colour_argb: -1,
        image: 93,
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 60,
        hp: 30,
        max_hp: 210,
        experience: 0,
        move_speed_ms: 10_000,
        attack_speed_ms: 5_000,
        friendly_guild: None,
        defense: Default::default(),
        position: point(10, 10),
        direction: MirDirection::Down,
        respawn: None,
        drops: Vec::new(),
    };
    assert!(zone.spawn_world_event_monster(&spawn, 0).0);
    zone
}
fn state(zone: &ZoneRuntime) -> Value {
    serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap()
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

#[test]
fn bone_wave_is_one_shared_batch_with_parent_target_and_two_second_startup() {
    let mut zone = fixture(30, true);
    let out = zone.tick(1);
    let snapshot = state(&zone);
    let monsters = snapshot["native_monsters"].as_object().unwrap();
    assert_eq!(
        monsters.len(),
        9,
        "crossing multiple stages emits one wave, not one per threshold"
    );
    let boss = &monsters[&BOSS.to_string()];
    assert_eq!(boss["special_ai"]["stage_summons"]["stage"], 0);
    assert_eq!(boss["next_ai_ready_at_ms"], 301);
    assert_eq!(boss["next_attack_ready_at_ms"], 5_001);
    let ids = boss["special_ai"]["stage_summons"]["slave_object_ids"]
        .as_array()
        .unwrap();
    assert_eq!(ids.len(), 8);
    let mut tiles = BTreeSet::from([(10, 10), (10, 11), (14, 12), (100, 100)]);
    for id in ids {
        let child = &monsters[&id.to_string()];
        assert_eq!(
            child["special_ai"]["stage_summons"]["parent_object_id"],
            BOSS
        );
        assert_eq!(
            child["special_ai"]["stage_summons"]["inherited_target_object_id"],
            101
        );
        assert_eq!(child["next_ai_ready_at_ms"], 2_001);
        assert_eq!(child["next_attack_ready_at_ms"], 2_001);
        assert!(tiles.insert((
            child["position"]["x"].as_i64().unwrap(),
            child["position"]["y"].as_i64().unwrap()
        )));
    }
    for recipient in ["a", "b"] {
        let p = packets(&out, recipient);
        assert!(p.iter().any(|p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == BOSS && info.attack_type == 1)));
        assert_eq!(
            p.iter()
                .filter(
                    |p| matches!(p, ServerPacket::ObjectMonster { info } if info.object_id != BOSS)
                )
                .count(),
            8
        );
    }
    assert!(!packets(&out, "far").iter().any(|p| matches!(
        p,
        ServerPacket::ObjectMonster { .. } | ServerPacket::ObjectAttack { .. }
    )));
    zone.tick(2);
    assert_eq!(zone.native_monster_count(), 9);
}

#[test]
fn bone_requires_a_living_visible_target_but_zuma_does_not() {
    let mut bone = fixture(30, false);
    bone.tick(1);
    assert_eq!(bone.native_monster_count(), 1);
    bone.handle(ZoneCommand::Join(join("target", 101, point(10, 11))));
    bone.tick(2);
    assert_eq!(bone.native_monster_count(), 9);
    let mut zuma = fixture(17, false);
    zuma.tick(1);
    assert_eq!(zuma.native_monster_count(), 9);
}

#[test]
fn checkpoint_keeps_exact_wave_identity_without_replaying_spawn_or_shortening_startup() {
    let mut zone = fixture(30, true);
    zone.tick(1);
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    assert_eq!(state(&zone), state(&restored));
    for now in [2, 300, 301, 1_999, 2_000] {
        let expected = zone.tick(now);
        let actual = restored.tick(now);
        assert_eq!(expected, actual);
        assert_eq!(restored.native_monster_count(), 9);
        let current = state(&restored);
        for (id, child) in current["native_monsters"].as_object().unwrap() {
            if id == &BOSS.to_string() {
                continue;
            }
            assert_eq!(child["next_ai_ready_at_ms"], 2_001);
        }
    }
}

#[test]
fn bone_pet_only_wave_keeps_target_and_startup_through_checkpoint_then_hits_real_pet() {
    let mut zone = fixture(30, false);
    let mut owner = join("owner", 101, point(10, 12));
    owner.class = MirClass::Taoist;
    owner.chat_profile.in_safe_zone = true;
    zone.handle(ZoneCommand::Join(owner));
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Up,
        target: point(10, 11),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    zone.tick(510);
    zone.tick(511);
    let snapshot = state(&zone);
    let monsters = snapshot["native_monsters"].as_object().unwrap();
    let (&ref pet_key, _) = monsters
        .iter()
        .find(|(_, m)| m["master_object_id"] == 101)
        .expect("real owned skeleton");
    let pet: u32 = pet_key.parse().unwrap();
    let children: Vec<u32> = monsters[&BOSS.to_string()]["special_ai"]["stage_summons"]
        ["slave_object_ids"]
        .as_array()
        .expect("pet alone triggers BoneLord wave")
        .iter()
        .map(|v| v.as_u64().unwrap() as u32)
        .collect();
    assert_eq!(children.len(), 8);
    for child in &children {
        let body = &monsters[&child.to_string()];
        assert_eq!(
            body["special_ai"]["stage_summons"]["inherited_target_object_id"],
            pet
        );
        assert_eq!(body["special_ai"]["stage_summons"]["target_bound"], true);
        assert!(body["next_ai_ready_at_ms"].as_u64().unwrap() >= 2510);
    }
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let mut hit = false;
    for now in (600..=6000).step_by(100) {
        let out = zone.tick(now);
        assert_eq!(out, restored.tick(now));
        assert_eq!(
            zone.checkpoint_bytes().unwrap(),
            restored.checkpoint_bytes().unwrap()
        );
        for packet in packets(&out, "owner") {
            match packet {
                ServerPacket::ObjectAttack { info } if children.contains(&info.object_id) => {
                    assert!(now >= 2510, "inheritance must not shorten startup")
                }
                ServerPacket::ObjectStruck { info }
                    if info.object_id == pet && children.contains(&info.attacker_id) =>
                {
                    hit = true
                }
                _ => {}
            }
        }
        if hit {
            break;
        }
    }
    assert!(
        hit,
        "spawned children must actually attack the real inherited pet"
    );
    assert_eq!(
        zone.player_vitals(&SessionId::new("owner")).unwrap().0,
        100_000
    );
}

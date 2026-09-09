//! Shared TrapRock lifecycle and direct-hit contracts.
//! Reference: Crystal TrapRock.cs.
//! These tests inspect genuine checkpoint output; they never rewrite a root.

use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;

const MONSTER_ID: u32 = 9_111;
const SOURCE_MAP: &str = "shared-monster-visibility-ai-fixture";

fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn player(session: &str, object_id: u32, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(session),
        account_id: format!("{session}-account"),
        character_index: object_id as i32,
        object_id,
        name: session.to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 100_000,
        max_hp: 100_000,
        mp: 100,
        map_file_name: SOURCE_MAP.to_string(),
        position,
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 10,
            max_dc: 10,
            accuracy: 100,
            min_ac: 100_000,
            max_ac: 100_000,
            ..Default::default()
        },
    }
}

fn monster(ai: u8, position: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        object_id: MONSTER_ID,
        name: if ai == 11 { "WoomaTaurus" } else { "Scarecrow" }.to_string(),
        name_colour_argb: -1,
        image: if ai == 11 { 11 } else { 5 },
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 30,
        max_hp: 70,
        hp: 70,
        experience: 0,
        move_speed_ms: 3_000,
        attack_speed_ms: 5_000,
        friendly_guild: None,
        position,
        direction: MirDirection::Down,
        defense: Default::default(),
        respawn: None,
        drops: Vec::new(),
    }
}

fn spawn(zone: &mut ZoneRuntime, owner: &str, ai: u8, position: Point) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new(owner),
        monster: monster(ai, position),
        now_ms: 0,
    })
}

fn packets_for<'a>(outbounds: &'a [ZoneOutbound], recipient: &str) -> Vec<&'a ServerPacket> {
    let recipient = SessionId::new(recipient);
    let mut packets = Vec::new();
    for outbound in outbounds {
        match outbound {
            ZoneOutbound::ToSession {
                session_id,
                packets: batch,
            } if session_id == &recipient => {
                packets.extend(batch);
            }
            ZoneOutbound::ToMany {
                session_ids,
                packets: batch,
            } if session_ids.contains(&recipient) => {
                packets.extend(batch);
            }
            ZoneOutbound::ToAll { packets: batch } => packets.extend(batch),
            _ => {}
        }
    }
    packets
}

#[test]
fn trap_rock_delayed_ring_has_no_damage_and_first_parent_hit_kills_group() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
    zone.handle(ZoneCommand::Join(player("target", 101, point(20, 22))));
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("target"),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    let initial = spawn(&mut zone, "target", 47, point(20, 20));
    assert!(!packets_for(&initial, "target")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectMonster{info} if info.object_id==MONSTER_ID)));
    let exact = zone.tick(2000);
    assert!(!packets_for(&exact, "target")
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectShow { .. })));
    let reveal = zone.tick(2001);
    assert_eq!(zone.native_monster_snapshots().len(), 4);
    assert_eq!(
        packets_for(&reveal, "target")
            .iter()
            .filter(|p| matches!(p, ServerPacket::ObjectShow { .. }))
            .count(),
        4
    );
    assert!(packets_for(&reveal, "target")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPoisoned{object_id:101,poison} if poison&32!=0)));
    let before: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    let hp = before["players"]["target"]["hp"].clone();
    let attacks = zone.tick(3001);
    assert!(packets_for(&attacks, "target")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info} if info.object_id==MONSTER_ID)));
    assert_eq!(
        packets_for(&attacks, "target")
            .iter()
            .filter(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id!=MONSTER_ID))
            .count(),
        3
    );
    let after: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(after["players"]["target"]["hp"], hp);
    assert!(after["pending_native_player_hits"]
        .as_array()
        .unwrap()
        .is_empty());
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let target = restored
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == MONSTER_ID)
        .unwrap()
        .position;
    let direction = if target.x > 20 {
        MirDirection::Right
    } else if target.x < 20 {
        MirDirection::Left
    } else if target.y > 22 {
        MirDirection::Down
    } else {
        MirDirection::Up
    };
    restored.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("target"),
        object_id: MONSTER_ID,
        direction,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 9999,
        now_ms: 6000,
    });
    restored.tick(6400);
    assert!(
        restored
            .native_monster_snapshots()
            .iter()
            .all(|m| m.hp == 0),
        "parent first hit collapses every child"
    );
}

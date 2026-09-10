//! Shared YinDevilNode source-filter and timed-stat contracts.
//! Reference: Crystal YinDevilNode.cs and MapObject target/buff rules.
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
        crystal_drop_seed: None,
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

fn fixture(ai: u8) -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
    z.handle(ZoneCommand::Join(player("player", 101, point(20, 22))));
    z.handle(ZoneCommand::sync_player_combat_state(SessionId::new("player"),
        MirClass::Warrior, false, false, true, false, false, false));
    let mut node = monster(ai, point(20, 20));
    node.attack_speed_ms = 100_000;
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("player"),
        monster: node,
        now_ms: 0,
    });
    let mut ally = monster(0, point(21, 22));
    ally.object_id = 9112;
    ally.hp = 10_000;
    ally.max_hp = 10_000;
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("player"),
        monster: ally,
        now_ms: 0,
    });
    z
}
fn checkpoint(z: &ZoneRuntime) -> Value {
    serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap()
}

#[test]
fn node_delayed_source_intersection_buffs_players_not_ordinary_wild_allies() {
    for (ai, ty, stat) in [(41, 7, 1), (42, 9, 5)] {
        let mut z = fixture(ai);
        let start = z.tick(0);
        assert!(packets_for(&start, "player")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==MONSTER_ID)));
        z.tick(499);
        assert!(checkpoint(&z)["players"]["player"]["buffs"][ty.to_string()].is_null());
        let completed = z.tick(500);
        assert!(packets_for(&completed,"player").iter().any(|p|matches!(p,ServerPacket::AddBuff{buff} if buff.object_id==101 && buff.buff_type==ty && buff.stats.iter().any(|s|s.stat==stat && s.value==9))));
        let data = checkpoint(&z);
        assert!(
            data["native_monsters"]["9112"]["buffs"][ty.to_string()].is_null(),
            "preserve Crystal FindAllTargets attackable filter"
        );
        assert_eq!(
            data["players"]["player"]["buffs"][ty.to_string()]["expires_at_ms"],
            5500
        );
        assert_eq!(
            z.native_monster_snapshots()
                .iter()
                .find(|m| m.object_id == MONSTER_ID)
                .unwrap()
                .position,
            point(20, 20)
        );
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert!(!packets_for(&restored.tick(501), "player")
            .iter()
            .any(|p| matches!(p,ServerPacket::AddBuff{buff} if buff.buff_type==ty)));
        let expiry = restored.tick(5500);
        assert!(packets_for(&expiry, "player").iter().any(
            |p| matches!(p,ServerPacket::RemoveBuff{object_id:101,buff_type} if *buff_type==ty)
        ));
        assert!(checkpoint(&restored)["players"]["player"]["buffs"][ty.to_string()].is_null());
    }
}

fn hit(z: &mut ZoneRuntime, now: u64) -> i32 {
    let before = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == 9112)
        .unwrap()
        .hp;
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("player"),
        object_id: 9112,
        direction: MirDirection::Right,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 99999,
        now_ms: now,
    });
    z.tick(now + 400);
    before
        - z.native_monster_snapshots()
            .into_iter()
            .find(|m| m.object_id == 9112)
            .unwrap()
            .hp
}

#[test]
fn node_attack_buff_affects_real_player_hit_and_expiry_restores_base_damage() {
    let mut enhanced = false;
    for offset in 0..12 {
        let mut z = fixture(42);
        z.tick(0);
        z.tick(500);
        let damage = hit(&mut z, 600 + offset);
        assert!(
            damage >= 10 && damage <= 19,
            "real hit uses target-owned MaxDC buff: {damage}"
        );
        enhanced |= damage > 10;
        z.tick(5500);
        assert_eq!(
            hit(&mut z, 6000 + offset),
            10,
            "expiry restores unmodified base DC"
        );
    }
    assert!(
        enhanced,
        "the modifier must change at least one actual damage roll"
    );
}

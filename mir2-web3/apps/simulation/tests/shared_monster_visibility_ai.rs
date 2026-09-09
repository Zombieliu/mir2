//! Public contracts for shared plant visibility and Zuma awakening.
//! Reference: Crystal CannibalPlant.cs, EvilCentipede.cs, ZumaMonster.cs.
//! These tests inspect genuine checkpoint output; they never rewrite a root.

use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
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

fn checkpoint_monster(zone: &ZoneRuntime) -> Value {
    let bytes = zone.checkpoint_bytes().expect("real Zone checkpoint");
    let checkpoint: Value = serde_json::from_slice(&bytes).expect("checkpoint JSON");
    checkpoint["native_monsters"][MONSTER_ID.to_string()].clone()
}

fn has_spawn(out: &[ZoneOutbound], who: &str, id: u32) -> bool {
    packets_for(out, who)
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectMonster { info } if info.object_id == id))
}
fn has_show(out: &[ZoneOutbound], who: &str, id: u32) -> bool {
    packets_for(out, who)
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectShow { object_id } if *object_id == id))
}

#[test]
fn hidden_plants_have_no_spawn_until_shared_player_proximity_reveals_once() {
    for ai in [5, 14] {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
        zone.handle(ZoneCommand::Join(player("far", 101, point(10, 10))));
        let initial = spawn(&mut zone, "far", ai, point(20, 20));
        assert!(!has_spawn(&initial, "far", MONSTER_ID));
        let joined = zone.handle(ZoneCommand::Join(player("near", 102, point(20, 22))));
        assert!(!has_spawn(&joined, "near", MONSTER_ID));
        let reveal = zone.tick(1);
        for who in ["far", "near"] {
            assert!(has_spawn(&reveal, who, MONSTER_ID));
            assert!(has_show(&reveal, who, MONSTER_ID));
            let ps = packets_for(&reveal, who);
            let spawn_index = ps.iter().position(|p| matches!(p, ServerPacket::ObjectMonster { info } if info.object_id == MONSTER_ID)).unwrap();
            let show_index = ps.iter().position(|p| matches!(p, ServerPacket::ObjectShow { object_id } if *object_id == MONSTER_ID)).unwrap();
            assert!(spawn_index < show_index);
        }
        assert!(!has_show(&zone.tick(1), "near", MONSTER_ID));
        assert!(!has_show(&zone.tick(2), "near", MONSTER_ID));
        assert_eq!(
            checkpoint_monster(&zone)["special_ai"]["visibility"]["hidden"],
            false
        );
    }
}

#[test]
fn stone_zuma_visible_at_spawn_and_wake_wave_does_not_recurse() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
    zone.handle(ZoneCommand::Join(player("observer", 101, point(15, 15))));
    let initial = spawn(&mut zone, "observer", 15, point(20, 20));
    assert!(packets_for(&initial, "observer").iter().any(|p| matches!(p, ServerPacket::ObjectMonster { info } if info.object_id == MONSTER_ID && info.extra && !info.hidden)));
    for (id, x) in [(9_112, 34), (9_113, 48)] {
        let mut m = monster(16, point(x, 20));
        m.object_id = id;
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: SessionId::new("observer"),
            monster: m,
            now_ms: 0,
        });
    }
    zone.handle(ZoneCommand::Join(player("trigger", 102, point(20, 22))));
    let awake = zone.tick(1);
    assert!(has_show(&awake, "trigger", MONSTER_ID));
    let bytes = zone.checkpoint_bytes().unwrap();
    let data: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        data["native_monsters"]["9111"]["special_ai"]["visibility"]["stoned"],
        false
    );
    assert_eq!(
        data["native_monsters"]["9112"]["special_ai"]["visibility"]["stoned"],
        false
    );
    assert_eq!(
        data["native_monsters"]["9113"]["special_ai"]["visibility"]["stoned"],
        true
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    assert!(!has_show(&restored.tick(2), "trigger", MONSTER_ID));
    assert_eq!(
        checkpoint_monster(&restored)["special_ai"]["visibility"]["stoned"],
        false
    );
}

#[test]
fn all_supported_stone_families_start_visible_and_unawake() {
    for ai in [15, 16, 17, 173, 174] {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
        zone.handle(ZoneCommand::Join(player("observer", 101, point(10, 10))));
        let initial = spawn(&mut zone, "observer", ai, point(20, 20));
        assert!(packets_for(&initial, "observer").iter().any(|p| matches!(p, ServerPacket::ObjectMonster { info } if info.object_id == MONSTER_ID && info.extra)));
        assert!(!has_show(&zone.tick(10_000), "observer", MONSTER_ID));
        assert_eq!(zone.native_monster_snapshots()[0].position, point(20, 20));
    }
}

#[test]
fn centipede_keeps_seven_tile_visibility_and_plants_hide_on_strict_deadline() {
    for ai in [5, 14] {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
        zone.handle(ZoneCommand::Join(player("observer", 101, point(10, 10))));
        spawn(&mut zone, "observer", ai, point(20, 20));
        zone.handle(ZoneCommand::Join(player("near", 102, point(20, 22))));
        assert!(has_show(&zone.tick(1), "near", MONSTER_ID));
        zone.handle(ZoneCommand::Leave {
            session_id: SessionId::new("near"),
        });
        zone.handle(ZoneCommand::Join(player("edge", 103, point(20, 27))));
        zone.tick(2_001); // Crystal compares Time > VisibleTime, not >=.
        assert_eq!(
            checkpoint_monster(&zone)["special_ai"]["visibility"]["hidden"],
            false
        );
        zone.tick(2_002);
        assert_eq!(
            checkpoint_monster(&zone)["special_ai"]["visibility"]["hidden"],
            ai == 5
        );
        zone.handle(ZoneCommand::Leave {
            session_id: SessionId::new("edge"),
        });
        let hidden = zone.tick(5_003);
        assert_eq!(
            checkpoint_monster(&zone)["special_ai"]["visibility"]["hidden"],
            true
        );
        assert_eq!(zone.native_monster_snapshots()[0].position, point(20, 20));
        if ai == 14 {
            assert!(packets_for(&hidden, "observer").iter().any(
                |p| matches!(p, ServerPacket::ObjectHide { object_id } if *object_id == MONSTER_ID)
            ));
        }
        let joined = zone.handle(ZoneCommand::Join(player("late", 104, point(12, 12))));
        assert!(!has_spawn(&joined, "late", MONSTER_ID));
    }
}

#[test]
fn personal_broadcast_cannot_restone_awakened_zuma_or_expose_hidden_plant() {
    let mut seed = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
    seed.handle(ZoneCommand::Join(player("source", 101, point(10, 10))));
    let initial = spawn(&mut seed, "source", 15, point(20, 20));
    let stale = packets_for(&initial, "source")
        .into_iter()
        .find(|p| matches!(p, ServerPacket::ObjectMonster { info } if info.object_id == MONSTER_ID))
        .unwrap()
        .clone();
    for ai in [5, 15] {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
        zone.handle(ZoneCommand::Join(player("source", 101, point(10, 10))));
        zone.handle(ZoneCommand::Join(player("observer", 102, point(12, 12))));
        spawn(&mut zone, "source", ai, point(20, 20));
        if ai == 15 {
            zone.handle(ZoneCommand::Join(player("trigger", 103, point(20, 22))));
            zone.tick(1);
        }
        let mut forged = stale.clone();
        if let ServerPacket::ObjectMonster { info } = &mut forged {
            info.location = point(13, 13);
            info.hidden = false;
            info.extra = true;
        }
        let broadcast = zone.handle(ZoneCommand::BroadcastPackets {
            session_id: SessionId::new("source"),
            owner_local_object_id: 101,
            packets: vec![forged],
            now_ms: 2,
        });
        let late = zone.handle(ZoneCommand::Join(player("late", 104, point(11, 11))));
        if ai == 5 {
            assert!(!has_spawn(&broadcast, "observer", MONSTER_ID));
            assert!(!has_spawn(&late, "late", MONSTER_ID));
        } else {
            let info = packets_for(&late, "late")
                .into_iter()
                .find_map(|p| match p {
                    ServerPacket::ObjectMonster { info } if info.object_id == MONSTER_ID => {
                        Some(info)
                    }
                    _ => None,
                })
                .expect("awakened monster remains visible");
            assert!(
                !info.extra,
                "personal stone flag must not overwrite shared wake"
            );
            assert_eq!(info.location, point(20, 20));
        }
        assert_eq!(zone.native_monster_snapshots()[0].position, point(20, 20));
    }
}

fn cast_ground(zone: &mut ZoneRuntime, spell: Spell, location: Point) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("caster"),
        object_id: 0,
        spell,
        direction: MirDirection::Right,
        target: location,
        cast: true,
        level: 3,
        damage: 6,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    })
}

#[test]
fn poison_cloud_cannot_poison_hidden_or_stoned_monster() {
    for ai in [5, 15] {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
        zone.handle(ZoneCommand::Join(player("caster", 101, point(10, 20))));
        spawn(&mut zone, "caster", ai, point(20, 20));
        let launch = cast_ground(&mut zone, Spell::PoisonCloud, point(20, 20));
        assert!(packets_for(&launch, "caster").iter().any(|p| matches!(
            p,
            ServerPacket::Magic {
                spell: Spell::PoisonCloud,
                cast: true,
                ..
            }
        )));
        let tick = zone.tick(520);
        assert!(packets_for(&tick, "caster").iter().any(
            |p| matches!(p, ServerPacket::ObjectSpell { info } if info.spell == Spell::PoisonCloud)
        ));
        let checkpoint = checkpoint_monster(&zone);
        assert_eq!(checkpoint["damage_poison"], 0);
        assert_eq!(checkpoint["hp"], 70);
    }
}

#[test]
fn lion_roar_cannot_control_stone_before_wake() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
    zone.handle(ZoneCommand::Join(player("caster", 101, point(18, 20))));
    spawn(&mut zone, "caster", 15, point(20, 20));
    let launch = cast_ground(&mut zone, Spell::LionRoar, point(20, 20));
    assert!(packets_for(&launch, "caster").iter().any(|p| matches!(
        p,
        ServerPacket::Magic {
            spell: Spell::LionRoar,
            cast: true,
            ..
        }
    )));
    let checkpoint = checkpoint_monster(&zone);
    assert_eq!(checkpoint["control_poison"], 0);
    assert_eq!(checkpoint["control_until_ms"], 0);
}

#[test]
fn dig_out_families_reveal_once_with_original_delayed_persistent_ground_effect() {
    for (ai, delay, spell) in [
        (24, 1_000, Spell::DigOutZombie),
        (124, 500, Spell::DigOutArmadillo),
        (125, 500, Spell::DigOutArmadillo),
    ] {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
        zone.handle(ZoneCommand::Join(player("observer", 101, point(10, 10))));
        let initial = spawn(&mut zone, "observer", ai, point(20, 20));
        assert!(!has_spawn(&initial, "observer", MONSTER_ID));
        zone.handle(ZoneCommand::Join(player("trigger", 102, point(20, 22))));
        let reveal = zone.tick(1);
        assert!(has_spawn(&reveal, "trigger", MONSTER_ID));
        assert!(has_show(&reveal, "trigger", MONSTER_ID));
        assert!(!packets_for(&reveal, "trigger")
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectSpell { info } if info.spell == spell)));
        zone.handle(ZoneCommand::Leave {
            session_id: SessionId::new("trigger"),
        });
        let before = zone.tick(1 + delay);
        assert!(!packets_for(&before, "observer")
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectSpell { info } if info.spell == spell)));
        let effect = zone.tick(2 + delay);
        let info = packets_for(&effect, "observer")
            .into_iter()
            .find_map(|p| match p {
                ServerPacket::ObjectSpell { info } if info.spell == spell => Some(info),
                _ => None,
            })
            .expect("delayed ground decoration");
        let effect_id = info.object_id;
        assert_ne!(effect_id, MONSTER_ID);
        assert_eq!(info.location, point(20, 20));
        assert_eq!(info.direction, MirDirection::Down);
        assert!(!info.param);
        let bytes = zone.checkpoint_bytes().unwrap();
        let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
        assert!(!packets_for(&restored.tick(3 + delay), "observer")
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectSpell { info } if info.spell == spell)));
        let late = restored.handle(ZoneCommand::Join(player("late", 103, point(12, 12))));
        assert!(
            has_spawn(&late, "late", MONSTER_ID),
            "dig-out monsters never re-hide when players leave"
        );
        assert!(packets_for(&late, "late").iter().any(
            |p| matches!(p, ServerPacket::ObjectSpell { info } if info.object_id == effect_id)
        ));
        let boundary = restored.tick(300_002 + delay);
        assert!(!packets_for(&boundary, "late").iter().any(
            |p| matches!(p, ServerPacket::ObjectRemove { object_id } if *object_id == effect_id)
        ));
        let expired = restored.tick(300_003 + delay);
        assert!(packets_for(&expired, "late").iter().any(
            |p| matches!(p, ServerPacket::ObjectRemove { object_id } if *object_id == effect_id)
        ));
    }
}

//! Shared Shinsu mode, real line damage and owned-pet lifecycle contracts.
//! Reference: Crystal Shinsu.cs and MonsterObject.Spawned / LineAttack.
//! These tests inspect genuine checkpoint output; they never rewrite a root.

use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};

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

fn owned_shinsu() -> (ZoneRuntime, u32) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
    let mut p = player("owner", 101, point(20, 20));
    p.class = MirClass::Taoist;
    z.handle(ZoneCommand::Join(p));
    let cast = z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonShinsu,
        direction: MirDirection::Right,
        target: point(21, 20),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    assert!(packets_for(&cast, "owner").iter().any(|p| matches!(
        p,
        ServerPacket::Magic {
            spell: Spell::SummonShinsu,
            cast: true,
            ..
        }
    )));
    let out = z.tick(510);
    let id = packets_for(&out, "owner")
        .iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info } if info.name == "Shinsu" => {
                assert_eq!(info.image, 79);
                assert!(!info.hidden);
                assert!(info.extra);
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("owned pet spawn");
    (z, id)
}
fn enemy(z: &mut ZoneRuntime, id: u32, x: i32, y: i32, ac: i32) {
    let mut m = monster(0, point(x, y));
    m.object_id = id;
    m.max_hp = 1000;
    m.hp = 1000;
    m.defense.min_ac = ac;
    m.defense.max_ac = ac;
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("owner"),
        monster: m,
        now_ms: 520,
    });
}
fn hp(z: &ZoneRuntime, id: u32) -> i32 {
    z.native_monster_snapshots()
        .iter()
        .find(|m| m.object_id == id)
        .unwrap()
        .hp
}

#[test]
fn owned_shinsu_transforms_then_hits_both_line_cells_at_550_and_600ms() {
    let (mut z, id) = owned_shinsu();
    enemy(&mut z, 9111, 22, 20, 0);
    enemy(&mut z, 9112, 23, 20, 0);
    let exact = z.tick(2510);
    assert!(!packets_for(&exact, "owner")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectShow{object_id} if *object_id==id)));
    let show = z.tick(2511);
    assert!(packets_for(&show, "owner")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectShow{object_id} if *object_id==id)));
    for now in [2511, 3000, 3511] {
        let locked = z.tick(now);
        assert!(
            !packets_for(&locked, "owner").iter().any(|p| matches!(p,
            ServerPacket::ObjectAttack { info } if info.object_id == id)),
            "Show action lock lasts through the exact deadline"
        );
        assert_eq!(
            z.native_monster_snapshots()
                .iter()
                .find(|m| m.object_id == id)
                .unwrap()
                .position,
            point(21, 20),
            "Show action lock also prevents chase movement"
        );
    }
    let late = z.handle(ZoneCommand::Join(player("observer", 102, point(19, 19))));
    assert!(packets_for(&late,"observer").iter().any(|p|matches!(p,ServerPacket::ObjectMonster{info} if info.object_id==id && info.image==80 && !info.hidden)));
    let attack = z.tick(3512);
    assert!(packets_for(&attack, "owner")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==id)));
    let bytes = z.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    restored.tick(4061);
    assert_eq!(hp(&restored, 9111), 1000);
    assert_eq!(hp(&restored, 9112), 1000);
    restored.tick(4062);
    assert!(hp(&restored, 9111) < 1000);
    assert_eq!(hp(&restored, 9112), 1000);
    let first = 1000 - hp(&restored, 9111);
    restored.tick(4112);
    assert_eq!(
        1000 - hp(&restored, 9112),
        first,
        "one DC roll shared by two cells"
    );
}

#[test]
fn shinsu_ignores_friendly_line_cell_but_damages_hostile_second_cell() {
    let (mut z, id) = owned_shinsu();
    let mut friendly = monster(0, point(22, 20));
    friendly.disposition = Some(WorldEntityDisposition::Friendly);
    friendly.max_hp = 1000;
    friendly.hp = 1000;
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("owner"),
        monster: friendly,
        now_ms: 520,
    });
    let idle = z.tick(2511);
    assert!(
        !packets_for(&idle, "owner")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectShow{object_id} if *object_id==id)),
        "friendly-only scene must not activate attack mode"
    );
    enemy(&mut z, 9112, 23, 20, 0);
    z.tick(2512);
    z.tick(3513);
    z.tick(4113);
    assert_eq!(
        hp(&z, MONSTER_ID),
        1000,
        "friendly first cell is not an attack target"
    );
    assert!(
        hp(&z, 9112) < 1000,
        "line continues to the eligible second cell"
    );
}

#[test]
fn shinsu_line_uses_impact_armour_and_owner_disconnect_cancels_pending_hit() {
    let (mut z, _) = owned_shinsu();
    enemy(&mut z, 9111, 22, 20, 0);
    enemy(&mut z, 9112, 23, 20, 100_000);
    z.tick(2511);
    z.tick(3512);
    z.tick(4112);
    assert!(hp(&z, 9111) < 1000);
    assert_eq!(hp(&z, 9112), 1000, "second cell armour must be respected");
    let (mut z, _) = owned_shinsu();
    enemy(&mut z, 9111, 22, 20, 0);
    z.tick(2511);
    z.tick(3512);
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("owner"),
    });
    z.tick(5000);
    assert_eq!(
        hp(&z, 9111),
        1000,
        "departed owner cannot retain new damage authority"
    );
}

#[test]
fn shinsu_returns_to_small_visible_form_only_after_thirty_seconds_without_target() {
    let (mut z, id) = owned_shinsu();
    let mut m = monster(0, point(22, 20));
    m.hp = 1;
    m.max_hp = 1;
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("owner"),
        monster: m,
        now_ms: 520,
    });
    z.tick(2511);
    z.tick(3512);
    z.tick(4062);
    assert_eq!(hp(&z, MONSTER_ID), 0);
    let exact = z.tick(33512);
    assert!(!packets_for(&exact, "owner")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectHide{object_id} if *object_id==id)));
    let hidden = z.tick(33513);
    assert!(packets_for(&hidden, "owner")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectHide{object_id} if *object_id==id)));
    assert!(!packets_for(&hidden, "owner")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRemove{object_id} if *object_id==id)));
    let late = z.handle(ZoneCommand::Join(player("late", 102, point(19, 19))));
    assert!(packets_for(&late,"late").iter().any(|p|matches!(p,ServerPacket::ObjectMonster{info} if info.object_id==id&&info.image==79&&!info.hidden)));
}

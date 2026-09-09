use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;
const ID: u32 = 9049;
fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}
fn owned_stone_target() -> (ZoneRuntime, u32) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("thunder-owned-target"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 50,
        hp: 10000,
        max_hp: 10000,
        mp: 1000,
        map_file_name: "thunder-owned-target".into(),
        position: p(100, 100),
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("owner"),
        MirClass::Archer,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::Stonetrap,
        direction: MirDirection::Right,
        target: p(104, 100),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    let out = z.tick(1210);
    let pet = packets(&out)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info } if info.master_object_id == 101 => {
                Some(info.object_id)
            }
            _ => None,
        })
        .unwrap();
    let position = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .position;
    z.spawn_world_event_monster(
        &ZoneMonsterSpawn {
            object_id: ID,
            name: "ArcherGuard".into(),
            name_colour_argb: -1,
            image: 132,
            ai: 49,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 37,
            max_hp: 1000,
            hp: 1000,
            experience: 0,
            move_speed_ms: 900,
            attack_speed_ms: 1000,
            friendly_guild: None,
            position: p(position.x + 1, position.y),
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
        1210,
    );
    (z, pet)
}
#[test]
fn thunder_acquires_real_pet_and_bursts_at_300ms_without_fake_player_damage() {
    let (mut z, pet) = owned_stone_target();
    z.tick(2000);
    let before = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .hp;
    let s: Value = serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(
        s["native_monsters"][ID.to_string()]["special_ai"]["thunder"]["pending_bursts"][0]
            ["due_at_ms"],
        2300
    );
    z.tick(2299);
    assert_eq!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == pet)
            .unwrap()
            .hp,
        before
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(2300);
    assert_eq!(out, restored.tick(2300));
    let after = z
        .native_monster_snapshots()
        .iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .hp;
    assert!(
        (235..=240).contains(&(before - after)),
        "DC255 against StoneTrap MAC15..20, no Struck immunity shortcut"
    );
    assert_eq!(z.player_vitals(&SessionId::new("owner")).unwrap().0, 10000);
    assert!(!out.iter().any(|o| matches!(
        o,
        ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::MonsterKillAward { .. }
    )));
}
#[test]
fn thunder_kills_real_pet_without_awarding_the_pet_owner_a_kill() {
    let (mut z, pet) = owned_stone_target();
    let mut died = false;
    for now in (2000..12000).step_by(100) {
        let out = z.tick(now);
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
        if z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == pet)
            .unwrap()
            .dead
        {
            died = true;
            assert!(packets(&out)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==pet)));
            break;
        }
    }
    assert!(
        died,
        "successive native MAC bursts must reach the owned monster's actual HP"
    );
}
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn fixture(hp: i32) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("thunder-fixture"));
    for (name, id, pos) in [
        ("a", 101, p(10, 11)),
        ("b", 102, p(11, 10)),
        ("far", 103, p(25, 25)),
    ] {
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new(name),
            account_id: name.into(),
            character_index: 1,
            object_id: id,
            name: name.into(),
            class: MirClass::Wizard,
            gender: MirGender::Male,
            level: 99,
            hp: 10000,
            max_hp: 10000,
            mp: 10000,
            map_file_name: "thunder-fixture".into(),
            position: pos,
            direction: MirDirection::Up,
            chat_profile: Default::default(),
            combat_stats: ZonePlayerCombatStats {
                min_dc: 1000,
                max_dc: 1000,
                min_mc: 100,
                max_mc: 100,
                accuracy: 100,
                ..Default::default()
            },
        }));
    }
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("a"),
        MirClass::Wizard,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    assert!(
        zone.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                object_id: ID,
                name: "ElectricElement".into(),
                name_colour_argb: -1,
                image: 132,
                ai: 49,
                disposition: Some(WorldEntityDisposition::Hostile),
                level: 1,
                max_hp: hp,
                hp,
                experience: 777,
                move_speed_ms: 900,
                attack_speed_ms: 1000,
                friendly_guild: None,
                defense: Default::default(),
                position: p(10, 10),
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
fn state(zone: &ZoneRuntime) -> Value {
    let all: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    all["native_monsters"][ID.to_string()].clone()
}
fn attacks(out: &[ZoneOutbound]) -> bool {
    out.iter().any(|o| match o {
        ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. }
        | ZoneOutbound::ToAll { packets } => packets
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==ID)),
        _ => false,
    })
}
#[test]
fn thunder_rejects_ordinary_lethal_damage_without_a_reward() {
    let mut zone = fixture(40);
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("a"),
        object_id: ID,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 99999,
        now_ms: 1,
    });
    let out = zone.tick(1);
    assert_eq!(state(&zone)["hp"], 40);
    assert_eq!(state(&zone)["dead"], false);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
}
#[test]
fn thunder_burst_is_delayed_shared_aoe_and_checkpoint_resumes_once() {
    let mut zone = fixture(1000);
    assert!(!attacks(&zone.tick(1)));
    assert_eq!(
        state(&zone)["special_ai"]["thunder"]["pending_bursts"][0]["due_at_ms"],
        301
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(zone.tick(300), restored.tick(300));
    let out = zone.tick(301);
    assert_eq!(out, restored.tick(301));
    assert!(attacks(&out));
    for name in ["a", "b"] {
        assert!(out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..} if *session_id==SessionId::new(name))));
    }
    assert!(!out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..} if *session_id==SessionId::new("far"))));
    assert!(!attacks(&zone.tick(302)));
}
#[test]
fn successful_repulsion_can_kill_thunder_and_rewards_exactly_once() {
    let mut zone = fixture(40);
    let mut out = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("a"),
        object_id: 101,
        spell: Spell::Repulsion,
        direction: MirDirection::Up,
        target: p(10, 11),
        cast: true,
        level: 3,
        damage: 100,
        mp_cost: 0,
        cooldown_ms: 0,
        now_ms: 1,
    });
    out.extend(zone.tick(1));
    assert_eq!(state(&zone)["dead"], true);
    assert_eq!(out.iter().filter(|o|matches!(o,ZoneOutbound::MonsterKillAward{award,..} if award.monster_object_id==ID)).count(),1);
    assert!(!zone
        .tick(2)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
}

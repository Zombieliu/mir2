//! Public shared-world regression: these targets are produced by a real spell,
//! not a player/session proxy or a forged retained ObjectMonster packet.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZoneRuntime,
};

const MAP: &str = "shared-entity-combat";
const SOURCE: u32 = 9000;
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn join(name: &str, id: u32, position: Point) -> ZoneJoin {
    let mut join = ZoneJoin {
        session_id: SessionId::new(name),
        account_id: name.into(),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class: MirClass::Taoist,
        gender: MirGender::Male,
        level: 40,
        hp: 100,
        max_hp: 100,
        mp: 100,
        map_file_name: MAP.into(),
        position,
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    };
    // Wild monsters may attack the real pet while its owner is protected.
    join.chat_profile.in_safe_zone = true;
    join
}
fn packets_for<'a>(out: &'a [ZoneOutbound], name: &str) -> Vec<&'a ServerPacket> {
    let session = SessionId::new(name);
    out.iter()
        .flat_map(|out| match out {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if session_id == &session => packets.iter().collect(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&session) => packets.iter().collect(),
            ZoneOutbound::ToAll { packets } => packets.iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}
fn wild(id: u32, position: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: id,
        name: "ArcherGuard".into(),
        name_colour_argb: -1,
        image: 139,
        ai: 0,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 40,
        hp: 9999,
        max_hp: 9999,
        experience: 0,
        move_speed_ms: 300,
        attack_speed_ms: 2000,
        friendly_guild: None,
        position,
        direction: MirDirection::Left,
        defense: Default::default(),
        respawn: None,
        drops: Vec::new(),
    }
}
fn hp(zone: &ZoneRuntime, id: u32) -> i32 {
    zone.native_monster_snapshots()
        .iter()
        .find(|m| m.object_id == id)
        .unwrap()
        .hp
}
fn summoned() -> (ZoneRuntime, u32) {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    zone.handle(ZoneCommand::Join(join("owner", 101, p(10, 20))));
    zone.handle(ZoneCommand::Join(join("observer", 102, p(14, 20))));
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: p(11, 20),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    let out = zone.tick(510);
    let pet = packets_for(&out, "owner")
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.master_object_id == 101 && info.name == "BoneFamiliar" =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("real shared SummonSkeleton creates an owned monster");
    (zone, pet)
}

#[test]
fn wild_attack_lands_on_real_pet_after_300ms_and_checkpoints_the_pending_life() {
    let (mut zone, pet) = summoned();
    let before = hp(&zone, pet);
    assert!(before > 0 && before < 250);
    zone.spawn_world_event_monster(&wild(SOURCE, p(12, 20)), 511);
    let launch = zone.tick(600);
    for name in ["owner", "observer"] {
        assert!(packets_for(&launch, name).iter().any(
            |packet| matches!(packet,ServerPacket::ObjectAttack{info}if info.object_id==SOURCE)
        ));
    }
    assert_eq!(hp(&zone, pet), before);
    zone.tick(899);
    assert_eq!(hp(&zone, pet), before);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let out = zone.tick(900);
    assert_eq!(out, restored.tick(900));
    assert_eq!(
        zone.checkpoint_bytes().unwrap(),
        restored.checkpoint_bytes().unwrap()
    );
    assert_eq!(
        hp(&zone, pet),
        0,
        "DC255 beats the real skeleton's AC and kills only the pet"
    );
    for name in ["owner", "observer"] {
        let packets = packets_for(&out, name);
        assert!(packets.iter().any(|packet|matches!(packet,ServerPacket::ObjectStruck{info}if info.object_id==pet && info.attacker_id==SOURCE)));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet,ServerPacket::ObjectDied{info}if info.object_id==pet)));
        assert_eq!(zone.player_vitals(&SessionId::new(name)).unwrap().0, 100);
    }
    assert!(
        !out.iter().any(|out| matches!(
            out,
            ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::MonsterKillAward { .. }
        )),
        "a pet is neither its owner's HP nor an owner kill credit"
    );
    for now in [900, 901, 3000] {
        let repeat = zone.tick(now);
        assert!(!packets_for(&repeat, "owner")
            .iter()
            .any(|packet| matches!(packet,ServerPacket::ObjectDied{info}if info.object_id==pet)));
        assert!(!repeat
            .iter()
            .any(|out| matches!(out, ZoneOutbound::MonsterKillAward { .. })));
    }
}

#[test]
fn wild_monsters_do_not_gain_aggro_against_other_unowned_monsters() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    zone.handle(ZoneCommand::Join(join("owner", 101, p(10, 20))));
    zone.spawn_world_event_monster(&wild(SOURCE, p(12, 20)), 1);
    zone.spawn_world_event_monster(&wild(SOURCE + 1, p(13, 20)), 1);
    for now in [2, 302, 602, 1002, 2002] {
        let out = zone.tick(now);
        assert!(!packets_for(&out, "owner")
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })));
        assert_eq!(hp(&zone, SOURCE), 9999);
        assert_eq!(hp(&zone, SOURCE + 1), 9999);
    }
}

#[test]
fn queued_pet_hit_does_not_fabricate_damage_after_owner_leaves() {
    let (mut zone, pet) = summoned();
    zone.spawn_world_event_monster(&wild(SOURCE, p(12, 20)), 511);
    zone.tick(600);
    zone.handle(ZoneCommand::Leave {
        session_id: SessionId::new("owner"),
    });
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let out = zone.tick(900);
    assert_eq!(out, restored.tick(900));
    assert!(!out.iter().any(|out| matches!(
        out,
        ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::MonsterKillAward { .. }
    )));
    assert!(!packets_for(&out, "observer")
        .iter()
        .any(|packet| matches!(packet,ServerPacket::ObjectStruck{info}if info.object_id==pet)));
}

#[test]
fn shared_acquired_pet_target_survives_closer_player_join_and_checkpoint() {
    let (mut zone, pet) = summoned();
    zone.spawn_world_event_monster(&wild(SOURCE, p(17, 20)), 511);
    zone.tick(600);
    assert_eq!(
        zone.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == SOURCE)
            .unwrap()
            .position,
        p(16, 20)
    );
    let mut nearer = join("nearer", 103, p(16, 19));
    nearer.chat_profile.in_safe_zone = false;
    zone.handle(ZoneCommand::Join(nearer));
    let checkpoint = zone.checkpoint_bytes().unwrap();
    let state: serde_json::Value = serde_json::from_slice(&checkpoint).unwrap();
    assert_eq!(
        state["entity_combat"]["targets"][SOURCE.to_string()]["target"]["Monster"]["object_id"],
        pet
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&checkpoint).unwrap();
    let out = zone.tick(901);
    assert_eq!(out, restored.tick(901));
    assert_eq!(
        zone.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == SOURCE)
            .unwrap()
            .position,
        p(15, 20),
        "keeps pursuing the real pet to the left, rather than the nearer player above"
    );
    assert!(!out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if session_id==&SessionId::new("nearer"))));
}

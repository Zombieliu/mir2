use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn join(z: &mut ZoneRuntime, name: &str, id: u32, pos: Point, dc: i32) {
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new(name),
        account_id: name.into(),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 50,
        hp: 1000000,
        max_hp: 1000000,
        mp: 100,
        map_file_name: "evil-mir-fixture".into(),
        position: pos,
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: dc,
            max_dc: dc,
            accuracy: 100,
            ..Default::default()
        },
    }));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new(name),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
}
fn spawn(id: u32, ai: u8, hp: i32, name: &str, pos: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        object_id: id,
        name: name.into(),
        name_colour_argb: -1,
        image: 950,
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 30,
        max_hp: 1000,
        hp,
        experience: 100,
        move_speed_ms: 500,
        attack_speed_ms: 1000,
        friendly_guild: None,
        defense: Default::default(),
        position: pos,
        direction: MirDirection::Down,
        respawn: None,
        drops: Vec::new(),
    }
}
fn fixture(id: u32, ai: u8, hp: i32, name: &str) -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("evil-mir-fixture"));
    join(&mut z, "a", 101, p(100, 101), 100);
    z.spawn_world_event_monster(&spawn(id, ai, hp, name, p(100, 100)), 0);
    z
}
fn state(z: &ZoneRuntime) -> Value {
    serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap()
}
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

#[test]
fn evil_mir_projectile_delays_real_magic_damage_and_keeps_fixed_position() {
    let mut picked = None;
    for id in 94520..94550 {
        let mut z = fixture(id, 52, 1000, "ArcherGuard");
        let out = z.tick(1);
        if packets(&out)
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectRangeAttack { .. }))
        {
            picked = Some((z, id));
            break;
        }
    }
    let (mut z, id) = picked.unwrap();
    assert_eq!(z.native_monster_snapshots()[0].position, p(100, 100));
    let bytes = z.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    assert_eq!(z.tick(670), restored.tick(670));
    let out = z.tick(671);
    assert_eq!(out, restored.tick(671));
    assert!(out
        .iter()
        .any(|o| matches!(o,ZoneOutbound::PlayerDamaged{damage,..}if *damage>0)));
    assert_eq!(
        state(&z)["native_monsters"][id.to_string()]["special_ai"]["evil_mir"]["pending"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}
#[test]
fn evil_mir_pending_cannot_transfer_to_same_id_new_life() {
    let mut z = fixture(9452, 52, 1000, "ArcherGuard");
    z.tick(1);
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    join(&mut z, "a", 101, p(100, 101), 100);
    let out = z.tick(700);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn evil_mir_zero_template_does_not_invent_damage() {
    let mut z = fixture(9452, 52, 1000, "HornedSorceror");
    z.tick(1);
    let out = z.tick(1000);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn evil_mir_pending_checkpoint_tampering_is_rejected() {
    let mut z = fixture(9452, 52, 1000, "ArcherGuard");
    z.tick(1);
    let mut s = state(&z);
    s["native_monsters"]["9452"]["special_ai"]["evil_mir"]["pending"][0] = 2.into();
    assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&s).unwrap()).is_err());
}
#[test]
fn evil_mir_mac_resistance_blocks_hit_and_its_poison_procs() {
    let mut z = fixture(9452, 52, 1000, "ArcherGuard");
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("a"),
        stats: ZonePlayerCombatStats {
            magic_resist: 10,
            ..Default::default()
        },
    });
    z.tick(1);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(700);
    assert_eq!(out, restored.tick(700));
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::DamageIndicator{object_id,damage_type,..}if *object_id==101&&*damage_type==1)));
    assert!(!packets(&out).iter().any(
        |p| matches!(p,ServerPacket::ObjectPoisoned{object_id,poison}if *object_id==101&&*poison!=0)
    ));
}

fn owned_evil_fixture(id: u32) -> (ZoneRuntime, u32, Point) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("evil-owned-fixture"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 60,
        hp: 100000,
        max_hp: 100000,
        mp: 100,
        map_file_name: "evil-owned-fixture".into(),
        position: p(20, 24),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonToad,
        direction: MirDirection::Up,
        target: p(20, 20),
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    let born = z.tick(1500);
    let (pet, pos) = packets(&born)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info } if info.name == "SpittingToad" => {
                Some((info.object_id, info.location.clone()))
            }
            _ => None,
        })
        .expect("real owned toad");
    z.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("owner"),
        position: p(pos.x + 10, pos.y),
        direction: MirDirection::Up,
    });
    let mut profile = z.player_chat_profile(&SessionId::new("owner")).unwrap();
    profile.in_safe_zone = true;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("owner"),
        profile,
    });
    let mut source = spawn(id, 52, 100000, "Guard", p(pos.x, pos.y - 1));
    source.max_hp = 100000;
    source.attack_speed_ms = 10000;
    assert!(z.spawn_world_event_monster(&source, 1500).0);
    (z, pet, pos)
}
fn evil_state(z: &ZoneRuntime, id: u32) -> Value {
    state(z)["native_monsters"][id.to_string()]["special_ai"]["evil_mir"].clone()
}
fn owned_projectile() -> (ZoneRuntime, u32, u32, Point, u64) {
    for id in 96000..96040 {
        let (mut z, pet, pos) = owned_evil_fixture(id);
        let out = z.tick(1501);
        if packets(&out)
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==id))
        {
            let due = evil_state(&z, id)["pending"][0].as_u64().unwrap();
            return (z, id, pet, pos, due);
        }
    }
    panic!("naturally selected 7/8 projectile");
}
#[test]
fn evil_mir_owned_projectile_has_real_life_refs_and_mac_damage() {
    let (mut z, id, pet, _, due) = owned_projectile();
    let before = state(&z)["native_monsters"][pet.to_string()]["hp"]
        .as_i64()
        .unwrap();
    assert_eq!(due, 2171);
    assert!(evil_state(&z, id)["target"].is_null());
    assert_eq!(
        evil_state(&z, id)["target_ref"]["Monster"]["object_id"],
        pet
    );
    assert_eq!(
        evil_state(&z, id)["pending_sources"][0]["source"]["Monster"]["object_id"],
        id
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(due - 1), restored.tick(due - 1));
    assert_eq!(state(&z)["native_monsters"][pet.to_string()]["hp"], before);
    let out = z.tick(due);
    assert_eq!(out, restored.tick(due));
    assert!(
        state(&z)["native_monsters"][pet.to_string()]["hp"]
            .as_i64()
            .unwrap()
            < before
    );
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn evil_mir_completion_updates_global_target_to_last_mixed_victim() {
    let (mut z, id, pet, pos, due) = owned_projectile();
    let mut profile = z.player_chat_profile(&SessionId::new("owner")).unwrap();
    profile.in_safe_zone = false;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("owner"),
        profile,
    });
    z.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("owner"),
        position: p(pos.x + 2, pos.y),
        direction: MirDirection::Up,
    });
    let out = z.tick(due);
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::DamageIndicator{object_id,damage,..}if *object_id==pet&&*damage>0)));
    assert!(out.iter().any(
        |o| matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if session_id.as_str()=="owner")
    ));
    assert_eq!(evil_state(&z, id)["target_ref"]["Player"]["object_id"], 101);
    assert_eq!(evil_state(&z, id)["target"][1], 101);
}
#[test]
fn evil_mir_owned_target_retirement_cancels_old_callback_before_same_id_reuse() {
    let (mut z, id, pet, pos, due) = owned_projectile();
    z.despawn_world_event_monster(pet, 1502);
    let mut replacement = spawn(pet, 2, 1000, "Deer", pos);
    replacement.disposition = Some(WorldEntityDisposition::Neutral);
    assert!(z.spawn_world_event_monster(&replacement, 1502).0);
    let out = z.tick(due);
    assert_eq!(state(&z)["native_monsters"][pet.to_string()]["hp"], 1000);
    assert!(!packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::DamageIndicator{object_id,..}if *object_id==pet)));
    assert!(evil_state(&z, id)["pending_sources"]
        .as_array()
        .is_none_or(|a| a.is_empty()));
}
#[test]
fn evil_mir_can_apply_native_green_and_true_paralysis_without_owner_damage() {
    let mut green = false;
    let mut paralysis = false;
    for id in 96100..96200 {
        let (mut z, pet, _) = owned_evil_fixture(id);
        z.tick(1501);
        let due = evil_state(&z, id)["pending"][0].as_u64().unwrap();
        let out = z.tick(due);
        let mask = state(&z)["native_monsters"][pet.to_string()]["entity_poison"]
            .as_u64()
            .unwrap_or(0);
        green |= mask & 1 != 0;
        paralysis |= mask & 32 != 0;
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
        if green && paralysis {
            break;
        }
    }
    assert!(
        green && paralysis,
        "legal caster IDs exercise the two independent source poison rolls"
    );
}

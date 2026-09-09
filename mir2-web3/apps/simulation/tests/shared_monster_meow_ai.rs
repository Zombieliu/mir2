//! Shared GeneralMeowMeow actions use explicit ArcherGuard nonzero stats.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, SessionId, WorldEntityDisposition, ZoneCommand,
    ZoneJoin, ZoneKey, ZoneMonsterSpawn, ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
const ID: u32 = 9157;
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn fixture(ai: u8, name: &str, player_x: i32, monster_id: u32) -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("meow-fixture"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("player"),
        account_id: "test".into(),
        character_index: 1,
        object_id: 101,
        name: "red".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 10000,
        max_hp: 10000,
        mp: 100,
        map_file_name: "meow-fixture".into(),
        position: point(player_x, 20),
        direction: MirDirection::Left,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("player"),
        MirClass::Warrior,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    let mut profile = z.player_chat_profile(&SessionId::new("player")).unwrap();
    profile.pk_points = 0;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("player"),
        profile,
    });
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("player"),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            object_id: monster_id,
            name: name.into(),
            name_colour_argb: -1,
            image: 139,
            ai,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 0,
            max_hp: 9999,
            hp: 9999,
            experience: 100,
            move_speed_ms: 300,
            attack_speed_ms: 2000,
            friendly_guild: None,
            position: point(20, 20),
            direction: MirDirection::Down,
            defense: Default::default(),
            respawn: None,
            drops: vec![GroundDropSnapshot {
                object_id: 0,
                name: "Gold".into(),
                name_colour_argb: -1,
                icon: 0,
                x: 0,
                y: 0,
                quantity: 1,
                source_monster: name.into(),
                owner_object_id: None,
                ownership_remaining_ticks: None,
                loot: GroundDropLootSnapshot::Gold { amount: 7 },
            }],
        },
    });
    z
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
fn hp(z: &ZoneRuntime) -> i32 {
    z.player_vitals(&SessionId::new("player")).unwrap().0
}

#[test]
fn meow_ranged_hit_is_delayed_and_checkpointed() {
    let mut z = fixture(123, "ArcherGuard", 24, ID);
    z.tick(3000);
    assert_eq!(hp(&z), 10000);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(3500), restored.tick(3500));
    assert_eq!(hp(&z), 9745);
    z.tick(3600);
    assert_eq!(hp(&z), 9745);
}

#[test]
fn meow_shield_changes_actual_armour_without_stacking() {
    let mut z = fixture(123, "ArcherGuard", 21, ID);
    let strike = |damage, now_ms| ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("player"),
        object_id: ID,
        direction: MirDirection::Left,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage,
        now_ms,
    };
    z.handle(strike(2499, 1));
    z.tick(1);
    let out = z.tick(3000);
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::AddBuff{buff} if buff.buff_type==52)));
    let before = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == ID)
        .unwrap()
        .hp;
    // The legacy scalar is already resolved damage; exercise the real Zone
    // armour sampling path using an authoritative fixed attack-power fixture.
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("player"),
        stats: ZonePlayerCombatStats {
            min_dc: 200,
            max_dc: 200,
            accuracy: 100,
            ..Default::default()
        },
    });
    z.handle(strike(200, 3100));
    z.tick(3100);
    let after = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == ID)
        .unwrap()
        .hp;
    assert_eq!(
        before - after,
        100,
        "both armour endpoints gain 100; do not stack or halve final damage"
    );
}

#[test]
fn meow_slaves_spawn_after_sixty_seconds_with_six_cap() {
    let mut z = fixture(123, "ArcherGuard", 24, ID);
    z.tick(60000);
    assert_eq!(z.native_monster_snapshots().len(), 1);
    z.tick(60001);
    assert_eq!(z.native_monster_snapshots().len(), 4);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(120002), restored.tick(120002));
    assert_eq!(z.native_monster_snapshots().len(), 7);
    z.tick(180003);
    assert_eq!(z.native_monster_snapshots().len(), 7);
}

fn owned_stone_target(ranged: bool) -> (ZoneRuntime, u32) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("meow-owned-target"));
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
        map_file_name: "meow-owned-target".into(),
        position: point(100, 100),
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
        target: point(104, 100),
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
            ai: 123,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 37,
            max_hp: 1000,
            hp: 1000,
            experience: 0,
            move_speed_ms: 900,
            attack_speed_ms: 2000,
            friendly_guild: None,
            position: point(position.x + if ranged { 4 } else { 1 }, position.y),
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
        1210,
    );
    let mut profile = z.player_chat_profile(&SessionId::new("owner")).unwrap();
    profile.in_safe_zone = true;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("owner"),
        profile,
    });
    (z, pet)
}
fn pet_hp(z: &ZoneRuntime, pet: u32) -> i32 {
    z.native_monster_snapshots()
        .iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .hp
}
#[test]
fn meow_pet_only_melee_uses_real_ac_and_delayed_incarnation_checkpoint() {
    let (mut z, pet) = owned_stone_target(false);
    let out = z.tick(2000);
    let attack_type = packets(&out)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectAttack { info } if info.object_id == ID => Some(info.attack_type),
            _ => None,
        })
        .expect("real pet alone can trigger Meow melee");
    let before = pet_hp(&z, pet);
    z.tick(2499);
    assert_eq!(pet_hp(&z, pet), before);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(2500);
    assert_eq!(out, restored.tick(2500));
    let lost = before - pet_hp(&z, pet);
    // StoneTrap AC15..20, populated fixture DC255; slam is three times DC.
    let raw = if attack_type == 1 { 765 } else { 255 };
    assert!(
        (raw - 20..=raw - 15).contains(&lost),
        "actual loss {lost}, attack type {attack_type}"
    );
    assert!(!out.iter().any(|o| matches!(
        o,
        ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::MonsterKillAward { .. }
    )));
}
#[test]
fn meow_range_hits_real_pet_and_player_once_in_same_burst() {
    let (mut z, pet) = owned_stone_target(true);
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("near"),
        account_id: "near".into(),
        character_index: 1,
        object_id: 102,
        name: "near".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 10000,
        max_hp: 10000,
        mp: 100,
        map_file_name: "meow-owned-target".into(),
        position: point(103, 100),
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    let out = z.tick(2000);
    assert!(packets(&out).into_iter().any(|p| matches!(p,ServerPacket::ObjectRangeAttack { info } if info.object_id == ID && info.target_id == pet)));
    let before = pet_hp(&z, pet);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(2500);
    assert_eq!(out, restored.tick(2500));
    assert!((235..=240).contains(&(before - pet_hp(&z, pet))));
    assert_eq!(z.player_vitals(&SessionId::new("near")).unwrap().0, 9745);
    assert_eq!(out.iter().filter(|o| matches!(o,ZoneOutbound::PlayerDamaged { session_id,.. } if *session_id == SessionId::new("near"))).count(),1);
    assert_eq!(z.player_vitals(&SessionId::new("owner")).unwrap().0, 10000);
    assert!(!z
        .tick(2501)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn meow_old_player_anchor_cannot_hit_same_id_after_leave_join() {
    let mut z = fixture(123, "ArcherGuard", 24, ID);
    z.tick(3000);
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("player"),
    });
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("player"),
        account_id: "test".into(),
        character_index: 1,
        object_id: 101,
        name: "new life".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 10000,
        max_hp: 10000,
        mp: 100,
        map_file_name: "meow-fixture".into(),
        position: point(24, 20),
        direction: MirDirection::Left,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(3500);
    assert_eq!(out, restored.tick(3500));
    assert_eq!(hp(&z), 10000);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}

fn forced_totem_room() -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("forced-meow-totem"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 50,
        hp: 1000,
        max_hp: 1000,
        mp: 100,
        map_file_name: "forced-meow-totem".into(),
        position: point(100, 100),
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonSnakes,
        direction: MirDirection::Right,
        target: point(104, 100),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    z
}

#[test]
fn full_totem_forces_meow_special_attack_past_a_closer_player() {
    let mut z = forced_totem_room();
    for now in [1210, 4210, 7210, 10210] {
        z.tick(now);
    }
    let totem = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.name == "SnakeTotem")
        .unwrap();
    let checkpoint: serde_json::Value =
        serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
    let due = checkpoint["native_monsters"][totem.object_id.to_string()]["special_ai"]
        ["snake_totem"]["search_at"]
        .as_u64()
        .unwrap();
    let count = checkpoint["native_monsters"]
        .as_object()
        .unwrap()
        .values()
        .filter(|m| m["master_object_id"] == totem.object_id && m["dead"] == false)
        .count();
    assert_eq!(count, 3);
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("closer"),
        account_id: "closer".into(),
        character_index: 2,
        object_id: 102,
        name: "closer".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 50,
        hp: 100000,
        max_hp: 100000,
        mp: 100,
        map_file_name: "forced-meow-totem".into(),
        position: point(108, 100),
        direction: MirDirection::Left,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    let id = 2_000_123;
    assert!(
        z.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                object_id: id,
                name: "ArcherGuard".into(),
                name_colour_argb: -1,
                image: 950,
                ai: 123,
                disposition: Some(WorldEntityDisposition::Hostile),
                level: 30,
                max_hp: 100000,
                hp: 100000,
                experience: 0,
                move_speed_ms: 500,
                attack_speed_ms: 10000,
                friendly_guild: None,
                defense: Default::default(),
                position: point(107, 100),
                direction: MirDirection::Left,
                respawn: None,
                drops: Vec::new()
            },
            due - 1
        )
        .0
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(due);
    assert_eq!(out, restored.tick(due));
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==id&&info.target_id==totem.object_id)),"forced global totem must beat closer player's natural acquisition");
    let saved: serde_json::Value = serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(
        saved["entity_combat"]["targets"][id.to_string()]["target"]["Monster"]["object_id"],
        totem.object_id
    );
}

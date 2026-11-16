//! Shared Kirin / SnowWolfKing actions use explicit ArcherGuard nonzero stats.
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
    let mut z = ZoneRuntime::new(ZoneKey::for_map("kirin-snow-fixture"));
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
        map_file_name: "kirin-snow-fixture".into(),
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
            crystal_drop_seed: None,
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
fn snow_attack_is_delayed_checkpointed_and_consumed_once() {
    let mut z = fixture(180, "ArcherGuard", 21, ID);
    let launch = z.tick(3000);
    assert!(packets(&launch)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==ID)));
    assert_eq!(hp(&z), 10000);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(3499), restored.tick(3499));
    assert_eq!(hp(&z), 10000);
    assert_eq!(z.tick(3500), restored.tick(3500));
    assert_eq!(hp(&z), 9745);
    z.tick(3600);
    assert_eq!(hp(&z), 9745);
}

#[test]
fn snow_pending_strike_does_not_hit_revived_player_life() {
    let mut z = fixture(180, "ArcherGuard", 21, ID);
    z.tick(3000);
    for dead in [true, false] {
        z.handle(ZoneCommand::SyncPlayerVitals {
            session_id: SessionId::new("player"),
            hp: if dead { 0 } else { 10000 },
            max_hp: 10000,
            mp: 100,
        });
    }
    z.tick(3500);
    assert_eq!(hp(&z), 10000);
}

#[test]
fn kirin_zero_stat_fixture_does_not_create_damage() {
    let mut z = fixture(186, "PoisonHugger", 21, ID);
    for now in (3000..10000).step_by(600) {
        z.tick(now);
    }
    assert_eq!(hp(&z), 10000);
}

#[test]
fn kirin_ice_thrust_hits_immediately_with_nonzero_mc_fixture() {
    let mut found = false;
    for id in 9300..9400 {
        let mut z = fixture(186, "ArcherGuard", 23, id);
        let out = z.tick(3000);
        if packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==id&&info.attack_type==2)) {
            assert_eq!(hp(&z),9745,"third forward cell is hit at cast time");
            z.tick(3500);assert_eq!(hp(&z),9745,"ice thrust must not add a delayed duplicate");
            found=true;break;
        }
    }
    assert!(found, "fixture sequence exercises the 1/5 ranged branch");
}

#[test]
fn snow_death_blast_occurs_after_500ms_and_is_not_repeated() {
    let mut z = fixture(180, "ArcherGuard", 21, ID);
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("player"),
        object_id: ID,
        direction: MirDirection::Left,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 20000,
        now_ms: 1,
    });
    z.tick(1);
    assert!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == ID)
            .unwrap()
            .dead
    );
    z.tick(500);
    assert_eq!(hp(&z), 10000);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(501), restored.tick(501));
    assert_eq!(hp(&z), 9745);
    z.tick(1001);
    assert_eq!(hp(&z), 9745);
}

#[test]
fn snow_low_health_spawns_once_and_death_adopts_live_wolves() {
    let mut z = fixture(180, "ArcherGuard", 21, ID);
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
    z.handle(strike(4000, 1));
    z.tick(1);
    z.tick(3000);
    assert_eq!(
        z.native_monster_snapshots()
            .iter()
            .filter(|m| m.name == "SnowWolf")
            .count(),
        3
    );
    z.handle(strike(20000, 4000));
    z.tick(4000);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(4500), restored.tick(4500));
    let wolves: Vec<_> = z
        .native_monster_snapshots()
        .into_iter()
        .filter(|m| m.name == "SnowWolf")
        .collect();
    assert_eq!(wolves.len(), 3);
    assert!(wolves.iter().all(|m| !m.dead
        && !m.hostile_to_player
        && m.disposition == Some(WorldEntityDisposition::Friendly)));
}

fn owned_stone_target(ai: u8, distance: i32, source_id: u32) -> (ZoneRuntime, u32) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("kirin-snow-owned"));
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
        map_file_name: "kirin-snow-owned".into(),
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
            crystal_drop_seed: None,
            object_id: source_id,
            name: "ArcherGuard".into(),
            name_colour_argb: -1,
            image: 132,
            ai,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 37,
            max_hp: 1000,
            hp: 1000,
            experience: 0,
            move_speed_ms: 900,
            attack_speed_ms: 2000,
            friendly_guild: None,
            position: point(position.x + distance, position.y),
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
fn snow_pet_only_melee_delays_actual_ac_damage_and_restores_once() {
    let (mut z, pet) = owned_stone_target(180, 1, ID);
    let out = z.tick(2000);
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==ID)));
    let before = pet_hp(&z, pet);
    z.tick(2499);
    assert_eq!(pet_hp(&z, pet), before);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(2500);
    assert_eq!(out, restored.tick(2500));
    assert!((235..=240).contains(&(before - pet_hp(&z, pet))));
    assert!(!out.iter().any(|o| matches!(
        o,
        ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::MonsterKillAward { .. }
    )));
    let after = pet_hp(&z, pet);
    z.tick(2501);
    assert_eq!(pet_hp(&z, pet), after);
}
#[test]
fn snow_death_blast_hits_real_pet_and_player_with_mac_after_500ms() {
    let (mut z, pet) = owned_stone_target(180, 1, ID);
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("killer"),
        account_id: "killer".into(),
        character_index: 1,
        object_id: 102,
        name: "killer".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 10000,
        max_hp: 10000,
        mp: 100,
        map_file_name: "kirin-snow-owned".into(),
        position: point(105, 101),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("killer"),
        MirClass::Warrior,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("killer"),
        object_id: ID,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 2000,
        now_ms: 1300,
    });
    z.tick(1300);
    assert!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == ID)
            .unwrap()
            .dead
    );
    let before = pet_hp(&z, pet);
    z.tick(1799);
    assert_eq!(pet_hp(&z, pet), before);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(1800);
    assert_eq!(out, restored.tick(1800));
    assert!((235..=240).contains(&(before - pet_hp(&z, pet))));
    assert_eq!(z.player_vitals(&SessionId::new("killer")).unwrap().0, 9745);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
    let after = pet_hp(&z, pet);
    z.tick(1801);
    assert_eq!(pet_hp(&z, pet), after);
}
#[test]
fn kirin_ice_thrust_damages_real_pet_and_applies_checkpointed_slow() {
    let mut found = false;
    for id in 9500..9700 {
        let (mut z, pet) = owned_stone_target(186, 3, id);
        let before = pet_hp(&z, pet);
        let out = z.tick(2000);
        if !packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectPoisoned{object_id,poison}if *object_id==pet && poison&4!=0)) { continue; }
        assert!(
            (235..=240).contains(&(before - pet_hp(&z, pet))),
            "Slow requires positive immediate MAC hit"
        );
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(z.tick(2001), restored.tick(2001));
        let current = pet_hp(&z, pet);
        z.tick(2500);
        assert_eq!(
            pet_hp(&z, pet),
            current,
            "IceThrust adds no delayed duplicate"
        );
        found = true;
        break;
    }
    assert!(
        found,
        "deterministic fixtures must exercise ranged Slow against a real pet"
    );
}

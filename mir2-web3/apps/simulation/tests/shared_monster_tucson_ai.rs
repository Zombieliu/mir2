//! Shared TucsonGeneral actions use explicit ArcherGuard nonzero stats.
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
    let mut z = ZoneRuntime::new(ZoneKey::for_map("tucson-fixture"));
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
        map_file_name: "tucson-fixture".into(),
        position: point(player_x, 21),
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
fn tucson_rage_rocks_are_real_ground_damage_with_checkpointed_clocks() {
    let mut z = fixture(131, "ArcherGuard", 21, ID);
    let out = z.tick(3000);
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info} if info.object_id==ID&&info.attack_type==0)));
    assert_eq!(hp(&z), 10000);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let mut saw = false;
    for now in (3050..=10000).step_by(50) {
        let a = z.tick(now);
        assert_eq!(a, restored.tick(now));
        for p in packets(&a) {
            if let ServerPacket::ObjectSpell { info } = p {
                saw = true;
                assert_ne!(info.location.x, 20);
                assert_ne!(info.location.y, 20);
            }
        }
    }
    assert!(saw);
    assert!(
        hp(&z) < 10000,
        "rocks settle AC damage on the occupied map cell"
    );
    let before = hp(&z);
    z.tick(10001);
    assert_eq!(hp(&z), before);
}

#[test]
fn tucson_zero_dc_rocks_never_invent_damage() {
    let mut z = fixture(131, "PoisonHugger", 21, ID);
    z.tick(3000);
    for now in (3050..=10000).step_by(50) {
        z.tick(now);
    }
    assert_eq!(hp(&z), 10000);
}

#[test]
fn tucson_delayed_rocks_survive_caster_death_until_source_expiry() {
    let mut z = fixture(131, "ArcherGuard", 21, ID);
    z.tick(3000);
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("player"),
        object_id: ID,
        direction: MirDirection::UpLeft,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 20000,
        now_ms: 3001,
    });
    z.tick(3001);
    assert!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == ID)
            .unwrap()
            .dead
    );
    for now in (3050..=10000).step_by(50) {
        z.tick(now);
    }
    assert!(
        hp(&z) < 10000,
        "map spells keep their corpse caster while its Node is retained"
    );
}

fn owned_target(stone: bool, source_id: u32) -> (ZoneRuntime, u32) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("tucson-owned"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: if stone {
            MirClass::Archer
        } else {
            MirClass::Taoist
        },
        gender: MirGender::Male,
        level: 50,
        hp: 10000,
        max_hp: 10000,
        mp: 1000,
        map_file_name: "tucson-owned".into(),
        position: point(100, 100),
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("owner"),
        if stone {
            MirClass::Archer
        } else {
            MirClass::Taoist
        },
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    let cast = z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: if stone {
            Spell::Stonetrap
        } else {
            Spell::SummonShinsu
        },
        direction: MirDirection::Right,
        target: point(if stone { 104 } else { 101 }, 100),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    assert!(packets(&cast).iter().any(|p|matches!(p,ServerPacket::Magic{spell,cast:true,..}if *spell == if stone {Spell::Stonetrap} else {Spell::SummonShinsu})), "summon must be accepted before inspecting spawn");
    let mut out = cast;
    out.extend(z.tick(1210));
    let pet = packets(&out)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info } if info.master_object_id == 101 => {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("accepted summon emits its real owned ObjectMonster");
    let position = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .position;
    z.spawn_world_event_monster(
        &ZoneMonsterSpawn {
            object_id: source_id,
            name: "ArcherGuard".into(),
            name_colour_argb: -1,
            image: 132,
            ai: 131,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 37,
            max_hp: 1000000,
            hp: 1000000,
            experience: 0,
            move_speed_ms: 900,
            attack_speed_ms: 2000,
            friendly_guild: None,
            position: point(position.x + 1, position.y + 1),
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
fn tucson_real_pet_only_launches_and_stonetrap_rejects_rock_struck() {
    let (mut z, pet) = owned_target(true, ID);
    let out = z.tick(2000);
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==ID && info.attack_type==0)));
    let before = pet_hp(&z, pet);
    let checkpoint: serde_json::Value =
        serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
    let pet_point = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .position;
    assert!(
        checkpoint["native_monsters"][ID.to_string()]["special_ai"]["tucson"]["rocks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["point"]["x"] == pet_point.x && r["point"]["y"] == pet_point.y),
        "rage includes real pet cell"
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    for now in (2050..=9000).step_by(50) {
        let out = z.tick(now);
        assert_eq!(out, restored.tick(now));
    }
    assert_eq!(
        pet_hp(&z, pet),
        before,
        "Tucson rock uses Struck, to which StoneTrap is immune"
    );
    z.tick(10001);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(11000), restored.tick(11000));
    assert!(
        pet_hp(&z, pet) < before,
        "ordinary Tucson Attacked must damage that same real pet"
    );
}
#[test]
fn tucson_map_rocks_apply_real_struck_damage_to_owned_shinsu() {
    let (mut z, pet) = owned_target(false, ID);
    let before = pet_hp(&z, pet);
    z.tick(2000);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    for now in (2050..=9000).step_by(50) {
        let out = z.tick(now);
        assert_eq!(out, restored.tick(now));
        assert!(!out.iter().any(|o| matches!(
            o,
            ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::MonsterKillAward { .. }
        )));
    }
    assert!(
        pet_hp(&z, pet) < before,
        "map cell Struck must reach the actual owned monster HP"
    );
}
#[test]
fn tucson_old_delayed_player_anchor_cannot_hit_revived_life() {
    let mut z = fixture(131, "ArcherGuard", 21, ID);
    z.tick(3000);
    for now in (3050..=10000).step_by(50) {
        z.tick(now);
    }
    z.tick(11001);
    for hp in [0, 10000] {
        z.handle(ZoneCommand::SyncPlayerVitals {
            session_id: SessionId::new("player"),
            hp,
            max_hp: 10000,
            mp: 100,
        });
    }
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(12000);
    assert_eq!(out, restored.tick(12000));
    assert_eq!(hp(&z), 10000);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}

#[test]
fn tucson_stomp_paralysis_uses_a_real_pet_lease_after_positive_damage() {
    let mut found = false;
    for id in 9750..9850 {
        let (mut z, pet) = owned_target(true, id);
        z.tick(2000);
        z.tick(10001);
        let before = pet_hp(&z, pet);
        let out = z.tick(10501);
        if !packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectPoisoned{object_id,poison}if *object_id==pet && poison&32!=0)) {continue;}
        assert!(
            (235..=240).contains(&(before - pet_hp(&z, pet))),
            "stomp must use actual ACAgility damage before poisoning"
        );
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(z.tick(10502), restored.tick(10502));
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
        found = true;
        break;
    }
    assert!(
        found,
        "deterministic fixtures must exercise the admitted pet paralysis branch"
    );
}

//! Crystal AI14/54 mixed targets, delayed area damage and life boundaries.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
const ID: u32 = 9954;
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
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
fn owned_target(ai: u8, source_id: u32) -> (ZoneRuntime, u32) {
    let stone = true;
    let mut z = ZoneRuntime::new(ZoneKey::for_map("stationary-owned"));
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
        map_file_name: "stationary-owned".into(),
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
            ai,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 37,
            max_hp: 1000,
            hp: 1000,
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
fn join_near(z: &mut ZoneRuntime, pet: u32) -> ZoneJoin {
    let pos = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .position;
    let p = ZoneJoin {
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
        map_file_name: "stationary-owned".into(),
        position: point(pos.x + 1, pos.y),
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    };
    z.handle(ZoneCommand::Join(p.clone()));
    p
}
#[test]
fn statue_pet_only_completion_broadcasts_at_500ms_and_damages_real_pet() {
    let (mut z, pet) = owned_target(54, ID);
    let before = pet_hp(&z, pet);
    let out = z.tick(2000);
    assert!(!packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==ID)));
    z.tick(2499);
    assert_eq!(pet_hp(&z, pet), before);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(2500);
    assert_eq!(out, restored.tick(2500));
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==ID && info.target_id==pet)));
    assert!((235..=240).contains(&(before - pet_hp(&z, pet))));
    let after = pet_hp(&z, pet);
    z.tick(2501);
    assert_eq!(pet_hp(&z, pet), after);
    assert!(!out.iter().any(|o| matches!(
        o,
        ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::MonsterKillAward { .. }
    )));
}
#[test]
fn statue_completion_hits_pet_and_player_in_target_area() {
    let (mut z, pet) = owned_target(54, ID);
    join_near(&mut z, pet);
    let before = pet_hp(&z, pet);
    z.tick(2000);
    let out = z.tick(2500);
    assert!((235..=240).contains(&(before - pet_hp(&z, pet))));
    assert_eq!(z.player_vitals(&SessionId::new("near")).unwrap().0, 9745);
    assert_eq!(out.iter().filter(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new("near"))).count(),1);
}
#[test]
fn centipede_real_pet_triggers_show_but_cannot_bypass_two_second_lock() {
    let (mut z, pet) = owned_target(14, 9914);
    let before = pet_hp(&z, pet);
    let out = z.tick(2000);
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectShow{object_id}if *object_id==9914)));
    for now in [2001, 3999, 4000] {
        let out = z.tick(now);
        assert_eq!(pet_hp(&z, pet), before);
        assert!(!packets(&out)
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==9914)));
    }
    let out = z.tick(4001);
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==9914)));
    z.tick(4500);
    assert_eq!(pet_hp(&z, pet), before);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(4501);
    assert_eq!(out, restored.tick(4501));
    assert!((235..=240).contains(&(before - pet_hp(&z, pet))));
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn statue_legacy_target_life_clear_survives_checkpoint_without_hitting_rejoin() {
    let (mut z, pet) = owned_target(54, ID);
    let p = join_near(&mut z, pet);
    // Removing the pet owner retires the pet; the nearby player becomes the sole target.
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("owner"),
    });
    z.tick(2000);
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("near"),
    });
    z.handle(ZoneCommand::Join(p));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(2500);
    assert_eq!(out, restored.tick(2500));
    assert_eq!(z.player_vitals(&SessionId::new("near")).unwrap().0, 10000);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}

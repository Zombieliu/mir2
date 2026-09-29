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
        map_file_name: "tree-queen-fixture".into(),
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
        crystal_drop_seed: None,
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
    let mut z = ZoneRuntime::new(ZoneKey::for_map("tree-queen-fixture"));
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
fn queen_spawn_delays_roots_and_never_moves() {
    let mut z = fixture(94142, 142, 1000, "ArcherGuard");
    z.tick(5000);
    let s = state(&z);
    assert!(s["tree_queen_world"].is_null());
    assert_eq!(z.native_monster_snapshots()[0].position, p(100, 100));
    z.tick(5001);
    assert!(!state(&z)["tree_queen_world"]["roots"]
        .as_array()
        .unwrap()
        .is_empty());
}
#[test]
fn queen_mass_roots_are_saved_cells_with_one_center_and_real_mac_damage() {
    let mut picked = None;
    for id in 94142..94400 {
        let mut z = fixture(id, 142, 1000, "ArcherGuard");
        z.tick(5001);
        let s = state(&z);
        let rs = s["tree_queen_world"]["roots"].as_array().unwrap();
        if rs.iter().any(|r| r["kind"] == 1) {
            picked = Some(z);
            break;
        }
    }
    let mut z = picked.unwrap();
    let s = state(&z);
    let roots = s["tree_queen_world"]["roots"].as_array().unwrap();
    assert_eq!(roots.len(), 48);
    assert_eq!(roots.iter().filter(|r| r["show"] == true).count(), 1);
    assert!(roots
        .iter()
        .all(|r| r["value"] == 255 && r["start"] == 5501));
    let bytes = z.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    let out = z.tick(5501);
    assert_eq!(out, restored.tick(5501));
    assert!(packets(&out).iter().any(
        |p| matches!(p,ServerPacket::ObjectSpell{info}if info.spell==Spell::TreeQueenMassRoots)
    ));
    assert!(out
        .iter()
        .any(|o| matches!(o,ZoneOutbound::PlayerDamaged{damage,..}if *damage>0)));
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectStruck{info}if info.attacker_id==0)));
}
#[test]
fn queen_pending_attack_does_not_hit_same_id_new_life() {
    let mut z = fixture(94142, 142, 1000, "ArcherGuard");
    z.tick(1);
    assert_eq!(
        state(&z)["native_monsters"]["94142"]["special_ai"]["tree_queen"]["entity_hits"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    join(&mut z, "a", 101, p(100, 101), 100);
    let out = z.tick(501);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    assert_eq!(
        z.player_position(&SessionId::new("a")).unwrap(),
        p(100, 101)
    );
}
#[test]
fn queen_world_checkpoint_is_integrity_protected() {
    let mut z = fixture(94142, 142, 1000, "ArcherGuard");
    z.tick(5001);
    let mut s = state(&z);
    s["tree_queen_world"]["roots"][0]["value"] = 99999.into();
    assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&s).unwrap()).is_err());
}

fn owned_queen_target(ai: u8, source_id: u32, stone: bool) -> (ZoneRuntime, u32) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("tree-queen-fixture"));
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
        map_file_name: "tree-queen-fixture".into(),
        position: p(100, 100),
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
        target: p(if stone { 104 } else { 101 }, 100),
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
            position: p(position.x + 1, position.y + 1),
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
fn queen_pet_only_captures_real_life_and_delayed_mac_attack_is_checkpoint_stable() {
    let mut found = false;
    for id in 95000..95100 {
        let (mut z, pet) = owned_queen_target(142, id, true);
        let hp = pet_hp(&z, pet);
        let launch = z.tick(2000);
        if !packets(&launch).iter().any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==id && info.attack_type==0)) {continue;}
        let s = state(&z);
        let m = &s["native_monsters"][id.to_string()];
        assert_eq!(
            m["special_ai"]["tree_queen"]["entity_hits"][0]["target"]["Monster"]["object_id"],
            pet
        );
        z.tick(2499);
        assert_eq!(pet_hp(&z, pet), hp);
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        let out = z.tick(2500);
        assert_eq!(out, restored.tick(2500));
        assert!(
            pet_hp(&z, pet) < hp,
            "Attacked MACAgility can damage StoneTrap; it is not Struck"
        );
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
        found = true;
        break;
    }
    assert!(found, "deterministically cover bombardment branch");
}
#[test]
fn queen_retired_anchor_cancels_delayed_area_attack() {
    for id in 95200..95300 {
        let (mut z, _) = owned_queen_target(142, id, true);
        let launch = z.tick(2000);
        if !packets(&launch).iter().any(|p|matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==id && info.attack_type==0)){continue;}
        z.handle(ZoneCommand::Leave {
            session_id: SessionId::new("owner"),
        });
        let pos = z
            .native_monster_snapshots()
            .into_iter()
            .find(|m| m.object_id == id)
            .unwrap()
            .position;
        join(&mut z, "replacement", 102, p(pos.x, pos.y + 1), 0);
        let out = z.tick(2500);
        assert!(
            !out.iter()
                .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })),
            "old pet anchor cannot redirect explosion to new player"
        );
        return;
    }
    panic!("bombardment branch not exercised");
}

#[test]
fn queen_map_root_struck_hits_real_shinsu_and_keeps_safe_owner_immune() {
    for id in 95400..95500 {
        let (mut z, pet) = owned_queen_target(142, id, false);
        // No earlier melee completion: push would legitimately move the pet
        // outside the player-centred root square before this map-spell test.
        z.tick(6211);
        let st = state(&z);
        if !st["tree_queen_world"]["roots"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == 1)
        {
            continue;
        }
        let before = pet_hp(&z, pet);
        let out = z.tick(6711);
        assert!(
            pet_hp(&z, pet) < before,
            "ground MAC Struck must hurt real Shinsu"
        );
        assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==pet && info.attacker_id==0)));
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
        return;
    }
    panic!("mass root branch not exercised");
}

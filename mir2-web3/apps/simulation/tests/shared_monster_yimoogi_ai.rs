//! Shared Yimoogi sibling and attack lifecycle.
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
    let mut z = ZoneRuntime::new(ZoneKey::for_map("yimoogi-fixture"));
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
        map_file_name: "yimoogi-fixture".into(),
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

fn strike(z: &mut ZoneRuntime, id: u32, damage: i32, now_ms: u64) {
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("player"),
        object_id: id,
        direction: MirDirection::Left,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage,
        now_ms,
    });
    z.tick(now_ms);
}

#[test]
fn yimoogi_starts_passive_and_spawns_one_sister_after_four_seconds() {
    let mut z = fixture(36, "Yimoogi", 21, ID);
    for now in [1000, 2000, 4000] {
        let out = z.tick(now);
        assert!(!packets(&out)
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==ID)));
    }
    assert_eq!(z.native_monster_snapshots().len(), 1);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(4001), restored.tick(4001));
    assert_eq!(z.native_monster_snapshots().len(), 2);
    z.tick(9000);
    assert_eq!(z.native_monster_snapshots().len(), 2);
    assert_eq!(hp(&z), 10000);
}

#[test]
fn yimoogi_attacked_wakes_and_delayed_hit_restores_without_duplication() {
    let mut found = false;
    for id in 9400..9500 {
        let mut z = fixture(36, "ArcherGuard", 21, id);
        strike(&mut z, id, 1, 1);
        let out = z.tick(3000);
        if packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==id&&info.attack_type==0)){
            let before=hp(&z);let mut restored=ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();assert_eq!(z.tick(3300),restored.tick(3300));assert_eq!(hp(&z),before-255);z.tick(3400);assert_eq!(hp(&z),before-255);found=true;break;
        }
    }
    assert!(found);
}

#[test]
fn yimoogi_first_sister_death_suppresses_drop() {
    let mut z = fixture(36, "Yimoogi", 21, ID);
    z.tick(4001);
    let out = z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("player"),
        object_id: ID,
        direction: MirDirection::Left,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 30000,
        now_ms: 4500,
    });
    let mut out = out;
    out.extend(z.tick(4500));
    assert!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == ID)
            .unwrap()
            .dead
    );
    assert!(
        !packets(&out)
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectGold { .. })),
        "living sister suppresses the parent's configured Gold drop"
    );
}

fn yimoogi_state(z: &ZoneRuntime, id: u32) -> serde_json::Value {
    let state: serde_json::Value = serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
    state["native_monsters"][id.to_string()].clone()
}
fn yimoogi_owned_spawn(id: u32, position: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: id,
        name: "Guard".into(),
        name_colour_argb: -1,
        image: 139,
        ai: 36,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 20,
        max_hp: 100000,
        hp: 100000,
        experience: 100,
        move_speed_ms: 500,
        attack_speed_ms: 10000,
        friendly_guild: None,
        defense: Default::default(),
        position,
        direction: MirDirection::Down,
        respawn: None,
        drops: vec![GroundDropSnapshot {
            object_id: 0,
            name: "Gold".into(),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: 1,
            source_monster: "Guard".into(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 7 },
        }],
    }
}
fn owned_yimoogi_fixture(id: u32, distance: i32) -> (ZoneRuntime, u32, Point) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("yimoogi-owned-fixture"));
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
        map_file_name: "yimoogi-owned-fixture".into(),
        position: point(20, 24),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonToad,
        direction: MirDirection::Up,
        target: point(20, 20),
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
        position: point(pos.x + 10, pos.y),
        direction: MirDirection::Up,
    });
    let mut profile = z.player_chat_profile(&SessionId::new("owner")).unwrap();
    profile.in_safe_zone = true;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("owner"),
        profile,
    });
    let mut source = yimoogi_owned_spawn(id, point(pos.x, pos.y - distance));
    source.max_hp = 100000;
    source.attack_speed_ms = 10000;
    assert!(z.spawn_world_event_monster(&source, 1500).0);
    (z, pet, pos)
}

fn activated_yimoogi_owned_hit(ranged: bool) -> (ZoneRuntime, u32, u32, Point, u64) {
    for id in 99000..99100 {
        let (mut z, pet, pos) = owned_yimoogi_fixture(id, if ranged { 3 } else { 1 });
        z.tick(1501);
        assert!(yimoogi_state(&z, id)["special_ai"]["yimoogi"]["target_ref"].is_null());
        for now in (1601..=5001).step_by(100) {
            z.tick(now);
            let state = yimoogi_state(&z, id);
            let hits = state["special_ai"]["yimoogi"]["hits"].as_array().unwrap();
            if let Some(hit) = hits.first() {
                if hit["accuracy"].is_null() == ranged {
                    let due = hit["due"].as_u64().unwrap();
                    assert_eq!(due - now, if ranged { 500 } else { 300 });
                    return (z, id, pet, pos, due);
                }
                break;
            }
        }
    }
    panic!("owned pet's actual hit should activate the requested natural branch");
}
#[test]
fn yimoogi_actual_pet_hit_activates_pet_not_owner_and_restores_both_attack_delays() {
    for ranged in [false, true] {
        let (mut z, id, pet, _, due) = activated_yimoogi_owned_hit(ranged);
        let before = yimoogi_state(&z, pet)["hp"].as_i64().unwrap();
        let state = yimoogi_state(&z, id);
        assert_eq!(
            state["special_ai"]["yimoogi"]["target_ref"]["Monster"]["object_id"],
            pet
        );
        assert!(state["special_ai"]["yimoogi"]["hits"][0]["hit"].is_null());
        assert_eq!(
            state["special_ai"]["yimoogi"]["hits"][0]["source_ref"]["Monster"]["object_id"],
            id
        );
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(z.tick(due - 1), restored.tick(due - 1));
        assert_eq!(yimoogi_state(&z, pet)["hp"], before);
        let out = z.tick(due);
        assert_eq!(out, restored.tick(due));
        assert!(yimoogi_state(&z, pet)["hp"].as_i64().unwrap() < before);
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    }
}
#[test]
fn yimoogi_old_native_target_retirement_cancels_captured_hit() {
    let (mut z, id, pet, pos, due) = activated_yimoogi_owned_hit(false);
    z.despawn_world_event_monster(pet, due - 1);
    let mut replacement = yimoogi_owned_spawn(pet, pos);
    replacement.ai = 2;
    replacement.name = "Deer".into();
    replacement.disposition = Some(WorldEntityDisposition::Neutral);
    assert!(z.spawn_world_event_monster(&replacement, due - 1).0);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(due), restored.tick(due));
    assert_eq!(yimoogi_state(&z, pet)["hp"], 100000);
    assert!(yimoogi_state(&z, id)["special_ai"]["yimoogi"]["target_ref"].is_null());
}
#[test]
fn yimoogi_sister_inherits_real_pet_life_and_does_not_rebind_replacement_sister() {
    let (mut z, id, pet, _, due) = activated_yimoogi_owned_hit(false);
    z.tick(due);
    z.tick(5501);
    let state = yimoogi_state(&z, id);
    let sister = state["special_ai"]["yimoogi"]["sister"].as_u64().unwrap() as u32;
    let sister_state = yimoogi_state(&z, sister);
    assert_eq!(
        sister_state["special_ai"]["yimoogi"]["target_ref"]["Monster"]["object_id"],
        pet
    );
    assert_eq!(
        state["special_ai"]["yimoogi"]["sister_incarnation"],
        sister_state["incarnation"]
    );
    assert_eq!(
        sister_state["special_ai"]["yimoogi"]["sister_incarnation"],
        state["incarnation"]
    );
    z.despawn_world_event_monster(sister, 5502);
    let position: Point = serde_json::from_value(sister_state["position"].clone()).unwrap();
    assert!(
        z.spawn_world_event_monster(&yimoogi_owned_spawn(sister, position), 5502)
            .0
    );
    z.despawn_world_event_monster(pet, 5502);
    let parent_position: Point = serde_json::from_value(state["position"].clone()).unwrap();
    z.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("owner"),
        position: point(parent_position.x + 1, parent_position.y),
        direction: MirDirection::Left,
    });
    let mut profile = z.player_chat_profile(&SessionId::new("owner")).unwrap();
    profile.in_safe_zone = false;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("owner"),
        profile,
    });
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
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("owner"),
        object_id: id,
        direction: MirDirection::Left,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 5503,
    });
    z.tick(5503);
    assert_eq!(
        yimoogi_state(&z, id)["special_ai"]["yimoogi"]["target_ref"]["Player"]["object_id"],
        101
    );
    assert!(yimoogi_state(&z, sister)["special_ai"]["yimoogi"]["target_ref"].is_null());
    ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
}
#[test]
fn yimoogi_can_apply_red_to_owned_target_without_second_human_resistance() {
    for id in 99200..99400 {
        let (mut z, pet, _) = owned_yimoogi_fixture(id, 3);
        z.tick(1501);
        for now in (1601..=5001).step_by(100) {
            z.tick(now);
            if yimoogi_state(&z, pet)["entity_poison"]
                .as_u64()
                .unwrap_or(0)
                & 2
                != 0
            {
                ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
                return;
            }
            if !yimoogi_state(&z, id)["special_ai"]["yimoogi"]["hits"]
                .as_array()
                .unwrap()
                .is_empty()
            {
                break;
            }
        }
    }
    panic!("natural ranged poison branch should apply Red");
}

#[test]
fn yimoogi_reused_sister_id_does_not_suppress_original_drop() {
    let mut z = fixture(36, "Yimoogi", 21, ID);
    z.tick(4001);
    let parent = yimoogi_state(&z, ID);
    let sister = parent["special_ai"]["yimoogi"]["sister"].as_u64().unwrap() as u32;
    let old = yimoogi_state(&z, sister);
    let pos: Point = serde_json::from_value(old["position"].clone()).unwrap();
    z.despawn_world_event_monster(sister, 4002);
    assert!(
        z.spawn_world_event_monster(&yimoogi_owned_spawn(sister, pos), 4002)
            .0
    );
    assert_ne!(yimoogi_state(&z, sister)["incarnation"], old["incarnation"]);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let command = ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("player"),
        object_id: ID,
        direction: MirDirection::Left,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 30000,
        now_ms: 4500,
    };
    let mut out = z.handle(command.clone());
    let mut recovered = restored.handle(command);
    out.extend(z.tick(4500));
    recovered.extend(restored.tick(4500));
    assert_eq!(out, recovered);
    assert_eq!(yimoogi_state(&z, ID)["dead"], true);
    assert!(
        packets(&out)
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectGold { .. })),
        "new unrelated sister ID must not suppress configured Gold drop"
    );
}

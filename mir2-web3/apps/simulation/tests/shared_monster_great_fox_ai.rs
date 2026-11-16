use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;
const BOSS: u32 = 9050;
const ROCK: u32 = 9048;
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn spawn(id: u32, ai: u8, hp: i32, position: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: id,
        name: if ai == 50 {
            "GreatFoxSpirit"
        } else {
            "GuardianRock"
        }
        .into(),
        name_colour_argb: -1,
        image: 134,
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 20,
        max_hp: hp,
        hp,
        experience: 100,
        move_speed_ms: 900,
        attack_speed_ms: 1000,
        friendly_guild: None,
        defense: Default::default(),
        position,
        direction: MirDirection::Down,
        respawn: None,
        drops: Vec::new(),
    }
}
fn fixture(near: bool, resist: i32) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("great-fox-shared-fixture"));
    for (name, id, pos) in [
        ("a", 101, if near { p(10, 11) } else { p(10, 14) }),
        ("b", 102, p(12, 14)),
        ("far", 103, p(25, 25)),
    ] {
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new(name),
            account_id: name.into(),
            character_index: 1,
            object_id: id,
            name: name.into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 50,
            hp: 100000,
            max_hp: 100000,
            mp: 100,
            map_file_name: "great-fox-shared-fixture".into(),
            position: pos,
            direction: MirDirection::Up,
            chat_profile: Default::default(),
            combat_stats: ZonePlayerCombatStats {
                min_dc: 1000,
                max_dc: 1000,
                accuracy: 100,
                magic_resist: resist,
                ..Default::default()
            },
        }));
    }
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("a"),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    assert!(
        zone.spawn_world_event_monster(&spawn(BOSS, 50, 4000, p(10, 10)), 0)
            .0
    );
    zone
}
fn state(zone: &ZoneRuntime, id: u32) -> Value {
    let all: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    all["native_monsters"][id.to_string()].clone()
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
fn great_fox_ranged_attack_snapshots_shared_targets_and_resumes_pending_damage_once() {
    let mut zone = fixture(false, 0);
    let cast = zone.tick(1);
    assert!(packets(&cast)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==BOSS)));
    assert_eq!(
        state(&zone, BOSS)["special_ai"]["great_fox"]["pending"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(zone.tick(300), restored.tick(300));
    let impact = zone.tick(301);
    assert_eq!(impact, restored.tick(301));
    for name in ["a", "b"] {
        assert!(impact.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new(name))));
    }
    assert!(!impact.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new("far"))));
    assert!(state(&zone, BOSS)["special_ai"]["great_fox"]["pending"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        state(&zone, BOSS)["position"],
        serde_json::json!({"x":10,"y":10})
    );
}

#[test]
fn great_fox_damage_updates_shared_stage_and_death_deactivates_guardian() {
    let mut zone = fixture(true, 10);
    assert!(
        zone.spawn_world_event_monster(&spawn(ROCK, 48, 500, p(8, 10)), 0)
            .0
    );
    zone.tick(1);
    assert_eq!(
        state(&zone, ROCK)["special_ai"]["great_fox"]["guardian_active"],
        true
    );
    for (index, now) in [2u64, 1002, 2002, 3002].into_iter().enumerate() {
        zone.handle(ZoneCommand::PlayerAttackObject {
            session_id: SessionId::new("a"),
            object_id: BOSS,
            direction: MirDirection::Up,
            spell: 0,
            level: 0,
            attack_type: 0,
            damage: 99999,
            now_ms: now,
        });
        let out = zone.tick(now);
        if index == 0 {
            assert_eq!(state(&zone, BOSS)["special_ai"]["great_fox"]["stage"], 1);
            assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectMonster{info}if info.object_id==BOSS&&info.extra_byte==1)));
        }
    }
    assert_eq!(state(&zone, BOSS)["dead"], true);
    assert_eq!(
        state(&zone, ROCK)["special_ai"]["great_fox"]["guardian_active"],
        false
    );
}

#[test]
fn great_fox_recall_respects_magic_immunity_and_saves_authoritative_transform() {
    let mut immune = fixture(false, 10);
    let mut exposed = fixture(false, 0);
    let mut recalled = false;
    for now in 1..=400u64 {
        let no = immune.tick(now);
        assert!(!no
            .iter()
            .any(|o| matches!(o, ZoneOutbound::SaveTransform { .. })));
        let out = exposed.tick(now);
        if let Some((session, position)) = out.iter().find_map(|o| match o {
            ZoneOutbound::SaveTransform {
                session_id,
                position,
                ..
            } => Some((session_id, position)),
            _ => None,
        }) {
            assert_eq!(exposed.player_position(session).as_ref(), Some(position));
            assert!((position.x - 10).abs() <= 1 && (position.y - 10).abs() <= 1);
            assert_ne!(*position, p(10, 10));
            assert!(packets(&out)
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectTeleportIn { effect_type: 0, .. })));
            assert!(
                state(&exposed, BOSS)["special_ai"]["great_fox"]["recall_ready_ms"]
                    .as_u64()
                    .unwrap()
                    >= 10000
            );
            recalled = true;
            break;
        }
    }
    assert!(
        recalled,
        "seeded public fixture must exercise a successful recall"
    );
}

fn join_at(zone: &mut ZoneRuntime, name: &str, id: u32, pos: Point, resist: i32) {
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new(name),
        account_id: name.into(),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 50,
        hp: 100000,
        max_hp: 100000,
        mp: 100,
        map_file_name: "great-fox-shared-fixture".into(),
        position: pos,
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            magic_resist: resist,
            ..Default::default()
        },
    }));
}
fn for_player<'a>(out: &'a [ZoneOutbound], name: &str) -> Vec<&'a ServerPacket> {
    let sid = SessionId::new(name);
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == sid => packets.iter().collect(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&sid) => packets.iter().collect(),
            ZoneOutbound::ToAll { packets } => packets.iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}
#[test]
fn recall_rebuilds_overlap_and_diffs_old_and_new_observers_and_owner_objects() {
    let mut base = ZoneRuntime::new(ZoneKey::for_map("great-fox-shared-fixture"));
    join_at(&mut base, "recalled", 201, p(40, 50), 0);
    join_at(&mut base, "old", 202, p(40, 65), 10);
    join_at(&mut base, "overlap", 203, p(45, 45), 10);
    join_at(&mut base, "new", 204, p(40, 26), 10);
    base.spawn_world_event_monster(&spawn(BOSS, 50, 4000, p(40, 40)), 0);
    base.spawn_world_event_monster(&spawn(ROCK, 48, 500, p(42, 64)), 0);
    let checkpoint = base.checkpoint_bytes().unwrap();
    let mut success = None;
    // Vary the real first tick; do not mutate a signed checkpoint or its root.
    for now in 1..=2000u64 {
        let mut zone = ZoneRuntime::restore_checkpoint(&checkpoint).unwrap();
        let out = zone.tick(now);
        if out.iter().any(|o| matches!(o,ZoneOutbound::SaveTransform {session_id,..} if *session_id==SessionId::new("recalled"))) {
            success=Some((zone,out)); break;
        }
    }
    let (zone, out) = success.expect("a real first tick must exercise recall");
    for name in ["old", "overlap"] {
        assert!(for_player(&out, name)
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectRemove { object_id: 201 })));
    }
    assert!(!for_player(&out, "old")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPlayer{info}if info.object_id==201)));
    for name in ["overlap", "new"] {
        let received = for_player(&out, name);
        let spawn = received
            .iter()
            .position(|p| matches!(p,ServerPacket::ObjectPlayer{info}if info.object_id==201))
            .expect("visible recalled player must respawn");
        let effect = received
            .iter()
            .position(|p| matches!(p, ServerPacket::ObjectTeleportIn { object_id: 201, .. }))
            .unwrap();
        assert!(spawn < effect);
    }
    let owner = for_player(&out, "recalled");
    for removed in [202, ROCK] {
        assert!(
            owner
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectRemove{object_id}if *object_id==removed)),
            "old-only object {removed}"
        );
    }
    assert!(owner
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPlayer{info}if info.object_id==204)));
    let saved: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    for name in ["overlap", "new"] {
        assert!(saved["players"][name]["visible_object_ids"]
            .as_array()
            .unwrap()
            .contains(&Value::from(201)));
    }
    assert!(!saved["players"]["old"]["visible_object_ids"]
        .as_array()
        .unwrap()
        .contains(&Value::from(201)));
}

#[test]
fn guardian_queues_each_ready_tick_and_checkpoint_preserves_each_completion() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("great-fox-shared-fixture"));
    join_at(&mut zone, "a", 101, p(10, 14), 10);
    join_at(&mut zone, "b", 102, p(12, 15), 10);
    zone.spawn_world_event_monster(&spawn(ROCK, 48, 500, p(10, 10)), 0);
    zone.tick(1);
    zone.tick(2);
    zone.tick(3);
    assert_eq!(
        state(&zone, ROCK)["special_ai"]["great_fox"]["pending"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let out = zone.tick(501);
    assert_eq!(out, restored.tick(501));
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==ROCK&&info.target_id==101)));
    assert_eq!(
        state(&zone, ROCK)["special_ai"]["great_fox"]["pending"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let out = zone.tick(503);
    assert_eq!(out, restored.tick(503));
    assert_eq!(
        packets(&out)
            .iter()
            .filter(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==ROCK))
            .count(),
        2
    );
    assert!(state(&zone, ROCK)["special_ai"]["great_fox"]["pending"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn dead_boss_and_deactivated_guardian_finish_already_cast_actions() {
    let mut zone = fixture(true, 0);
    zone.spawn_world_event_monster(&spawn(ROCK, 48, 500, p(8, 10)), 0);
    zone.tick(1);
    for now in [2u64, 1002, 2002] {
        zone.handle(ZoneCommand::PlayerAttackObject {
            session_id: SessionId::new("a"),
            object_id: BOSS,
            direction: MirDirection::Up,
            spell: 0,
            level: 0,
            attack_type: 0,
            damage: 99999,
            now_ms: now,
        });
        zone.tick(now);
    }
    zone.tick(3001);
    // Ensure both casters have real pending actions immediately before death.
    assert!(!state(&zone, BOSS)["special_ai"]["great_fox"]["pending"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!state(&zone, ROCK)["special_ai"]["great_fox"]["pending"]
        .as_array()
        .unwrap()
        .is_empty());
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("a"),
        object_id: BOSS,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 99999,
        now_ms: 3002,
    });
    zone.tick(3002);
    assert_eq!(state(&zone, BOSS)["dead"], true);
    assert_eq!(
        state(&zone, ROCK)["special_ai"]["great_fox"]["guardian_active"],
        false
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let out = zone.tick(4000);
    assert_eq!(out, restored.tick(4000));
    assert!(out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==ROCK)));
    assert!(state(&zone, BOSS)["special_ai"]["great_fox"]["pending"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(state(&zone, ROCK)["special_ai"]["great_fox"]["pending"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn guardian_pull_emits_every_step_faces_away_and_locks_owner_actions() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("great-fox-shared-fixture"));
    join_at(&mut zone, "pulled", 301, p(10, 15), 0);
    join_at(&mut zone, "observer", 302, p(13, 15), 10);
    zone.spawn_world_event_monster(&spawn(ROCK, 48, 500, p(10, 10)), 0);
    zone.tick(1);
    let out = zone.tick(501);
    let owner = for_player(&out, "pulled");
    let steps: Vec<_> = owner
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::Pushed {
                location,
                direction,
            } => Some((location.clone(), *direction)),
            _ => None,
        })
        .collect();
    assert_eq!(
        steps,
        vec![
            (p(10, 14), MirDirection::Down),
            (p(10, 13), MirDirection::Down),
            (p(10, 12), MirDirection::Down),
            (p(10, 11), MirDirection::Down)
        ]
    );
    assert_eq!(
        for_player(&out, "observer")
            .iter()
            .filter(|p| matches!(p, ServerPacket::ObjectPushed { object_id: 301, .. }))
            .count(),
        4
    );
    assert!(!owner.iter().any(|p| matches!(
        p,
        ServerPacket::ObjectTeleportIn { .. } | ServerPacket::ObjectTeleportOut { .. }
    )));
    assert_eq!(
        zone.player_position(&SessionId::new("pulled")),
        Some(p(10, 11))
    );
    assert_eq!(
        zone.player_direction(&SessionId::new("pulled")),
        Some(MirDirection::Down)
    );
    assert!(!zone.can_player_cast_magic(
        &SessionId::new("pulled"),
        ROCK,
        mir2_protocol::Spell::FireBall,
        MirDirection::Up,
        &p(10, 10),
        true,
        10,
        0,
        0,
        502
    ));
    let saved: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert!(
        saved["players"]["pulled"]["next_spell_ready_at_ms"]
            .as_u64()
            .unwrap()
            >= 1001
    );
    assert!(
        saved["players"]["pulled"]["movement_ready_at_ms"]
            .as_u64()
            .unwrap()
            >= 1001
    );
    assert!(
        saved["players"]["pulled"]["next_attack_ready_at_ms"]
            .as_u64()
            .unwrap()
            >= 1001
    );
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::SaveTransform{session_id,position,direction}if *session_id==SessionId::new("pulled")&&*position==p(10,11)&&*direction==MirDirection::Down)));
}

#[test]
fn great_fox_old_life_damage_is_cancelled_on_leave_and_same_id_join() {
    let mut zone = fixture(false, 0);
    zone.tick(1);
    assert_eq!(
        state(&zone, BOSS)["special_ai"]["great_fox"]["pending"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    zone.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    join_at(&mut zone, "a", 101, p(10, 14), 0);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let out = zone.tick(301);
    assert_eq!(out, restored.tick(301));
    assert!(!out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new("a"))));
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new("b"))));
    assert_eq!(zone.player_vitals(&SessionId::new("a")).unwrap().0, 100000);
}

#[test]
fn guardian_old_life_pending_cannot_transfer_to_rejoined_current_target() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("great-fox-shared-fixture"));
    join_at(&mut zone, "a", 101, p(10, 14), 10);
    zone.spawn_world_event_monster(&spawn(ROCK, 48, 500, p(10, 10)), 0);
    zone.tick(1);
    zone.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    assert!(state(&zone, ROCK)["special_ai"]["great_fox"]["pending"]
        .as_array()
        .unwrap()
        .is_empty());
    join_at(&mut zone, "a", 101, p(10, 14), 10);
    zone.tick(2); // A new life can be acquired and queue a new event due at 502.
    let out = zone.tick(501);
    assert!(!packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==ROCK)));
    let out = zone.tick(502);
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==ROCK&&info.target_id==101)));
}

fn owned_great_fox_fixture(
    id: u32,
    ai: u8,
    distance: i32,
    spell: Spell,
) -> (ZoneRuntime, u32, Point) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("fox-owned-fixture"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: if spell == Spell::SummonToad {
            MirClass::Archer
        } else {
            MirClass::Taoist
        },
        gender: MirGender::Male,
        level: 60,
        hp: 100000,
        max_hp: 100000,
        mp: 100,
        map_file_name: "fox-owned-fixture".into(),
        position: p(20, 24),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell,
        direction: MirDirection::Up,
        target: if spell == Spell::SummonHolyDeva {
            p(20, 23)
        } else {
            p(20, 20)
        },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: if spell == Spell::SummonHolyDeva {
            0
        } else {
            10
        },
    });
    let born = z.tick(1500);
    let (pet, pos) = packets(&born)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info }
                if matches!(info.name.as_str(), "SpittingToad" | "HolyDeva") =>
            {
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
    let mut source = spawn(id, ai, 100000, p(pos.x, pos.y - distance));
    source.name = "Guard".into();
    source.max_hp = 100000;
    source.attack_speed_ms = 10000;
    assert!(z.spawn_world_event_monster(&source, 1500).0);
    (z, pet, pos)
}

#[test]
fn great_fox_owned_hit_is_captured_at_300ms_with_checkpoint_life_refs() {
    let (mut z, pet, _) = owned_great_fox_fixture(98050, 50, 1, Spell::SummonToad);
    z.tick(1501);
    let before = state(&z, pet)["hp"].as_i64().unwrap();
    let pending = state(&z, 98050)["special_ai"]["great_fox"]["pending"].clone();
    assert_eq!(pending[0]["target_ref"]["Monster"]["object_id"], pet);
    assert!(pending[0]["target_session"].is_null());
    assert_eq!(pending[0]["source_ref"]["Monster"]["object_id"], 98050);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(1800), restored.tick(1800));
    assert_eq!(state(&z, pet)["hp"], before);
    let out = z.tick(1801);
    assert_eq!(out, restored.tick(1801));
    assert!(state(&z, pet)["hp"].as_i64().unwrap() < before);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn great_fox_owned_pending_does_not_hit_reused_target_id() {
    let (mut z, pet, pos) = owned_great_fox_fixture(98050, 50, 1, Spell::SummonToad);
    z.tick(1501);
    let old_life = state(&z, pet)["incarnation"].clone();
    z.despawn_world_event_monster(pet, 1502);
    let mut replacement = spawn(pet, 2, 1000, pos);
    replacement.name = "Deer".into();
    replacement.disposition = Some(WorldEntityDisposition::Neutral);
    assert!(z.spawn_world_event_monster(&replacement, 1502).0);
    assert_ne!(state(&z, pet)["incarnation"], old_life);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(1801), restored.tick(1801));
    assert_eq!(state(&z, pet)["hp"], 1000);
    assert_eq!(state(&z, pet)["entity_poison"].as_u64().unwrap_or(0), 0);
}
#[test]
fn great_fox_can_recall_a_real_owned_monster_with_authoritative_teleport() {
    for id in 98100..98300 {
        let (mut z, pet, _) = owned_great_fox_fixture(id, 50, 4, Spell::SummonToad);
        let out = z.tick(1501);
        if packets(&out)
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectTeleportIn{object_id,..} if *object_id==pet))
        {
            let pet_state = state(&z, pet);
            let boss_state = state(&z, id);
            let dx = (pet_state["position"]["x"].as_i64().unwrap()
                - boss_state["position"]["x"].as_i64().unwrap())
            .abs();
            let dy = (pet_state["position"]["y"].as_i64().unwrap()
                - boss_state["position"]["y"].as_i64().unwrap())
            .abs();
            assert_eq!(dx.max(dy), 1);
            assert!(packets(&out)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectRemove{object_id} if *object_id==pet)));
            ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
            return;
        }
    }
    panic!("recall should occur naturally across legal source IDs");
}
#[test]
fn guardian_pulls_pushable_owned_monster_and_respects_cannot_push_template() {
    for spell in [Spell::SummonHolyDeva, Spell::SummonToad] {
        let (mut z, pet, _) = owned_great_fox_fixture(98448, 48, 4, spell);
        z.tick(1501);
        let before = state(&z, pet)["position"].clone();
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        let out = z.tick(2001);
        assert_eq!(out, restored.tick(2001));
        assert!(packets(&out).iter().any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==98448&&info.target_id==pet)));
        if spell == Spell::SummonHolyDeva {
            assert_ne!(state(&z, pet)["position"], before);
            assert!(packets(&out)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectPushed{object_id,..}if *object_id==pet)));
            assert_eq!(
                state(&z, pet)["direction"],
                serde_json::to_value(MirDirection::Down).unwrap()
            );
            assert_eq!(
                state(&z, pet)["special_ai"]["great_fox"]["pushed_move_ready_ms"],
                3501
            );
        } else {
            assert_eq!(state(&z, pet)["position"], before);
            assert!(!packets(&out)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectPushed{object_id,..}if *object_id==pet)));
        }
    }
}
#[test]
fn great_fox_native_slow_and_paralysis_use_shared_control_leases() {
    let mut slow = false;
    let mut paralysis = false;
    for id in 98500..98600 {
        let (mut z, pet, _) = owned_great_fox_fixture(id, 50, 1, Spell::SummonToad);
        z.tick(1501);
        z.tick(1801);
        let mask = state(&z, pet)["entity_poison"].as_u64().unwrap_or(0);
        slow |= mask & 4 != 0;
        paralysis |= mask & 32 != 0;
        if slow && paralysis {
            break;
        }
    }
    assert!(slow && paralysis);
}

#[test]
fn great_fox_mac_attack_respects_full_magic_resistance_on_shared_players() {
    let mut zone = fixture(false, 10);
    zone.tick(1);
    assert_eq!(
        state(&zone, BOSS)["special_ai"]["great_fox"]["pending"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let out = zone.tick(301);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    for id in [101, 102] {
        assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::DamageIndicator{object_id,damage,..}if *object_id==id&&*damage==0)));
    }
}

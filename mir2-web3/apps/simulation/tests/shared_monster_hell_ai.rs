use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;
fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn join_resist(z: &mut ZoneRuntime, name: &str, id: u32, pos: Point, resist: i32) {
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
        map_file_name: "hell-shared-fixture".into(),
        position: pos,
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 100000000,
            max_dc: 100000000,
            accuracy: 100,
            poison_resist: resist,
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
fn join(z: &mut ZoneRuntime, name: &str, id: u32, pos: Point) {
    join_resist(z, name, id, pos, 10);
}
fn spawn(ai: u8, name: &str, pos: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        object_id: 9900 + u32::from(ai),
        name: name.into(),
        name_colour_argb: -1,
        image: match name {
            "HellBomb1" => 903,
            "HellBomb2" => 904,
            "HellBomb3" => 905,
            _ => 902,
        },
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 30,
        max_hp: 1000,
        hp: 1000,
        experience: 10,
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
fn fixture(ai: u8, name: &str) -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("hell-shared-fixture"));
    join(&mut z, "a", 101, p(40, 41));
    join(&mut z, "b", 102, p(42, 42));
    z.spawn_world_event_monster(&spawn(ai, name, p(40, 40)), 0);
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
fn bomb_expires_strictly_after_ten_seconds_and_explodes_after_death_once() {
    let mut z = fixture(99, "HellBomb1");
    let at = z.tick(10000);
    assert!(!packets(&at)
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectDied { .. })));
    let died = z.tick(10001);
    assert!(packets(&died)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==9999)));
    assert!(packets(&died)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==9999)));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(10500), restored.tick(10500));
    let out = z.tick(10501);
    assert_eq!(out, restored.tick(10501));
    for name in ["a", "b"] {
        assert!(out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged { session_id, damage, .. }if *session_id==SessionId::new(name)&&*damage>0)));
    }
    assert!(!z
        .tick(10502)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn killing_bomb_with_damage_does_not_schedule_an_explosion() {
    let mut z = fixture(99, "HellBomb1");
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("a"),
        object_id: 9999,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 1,
    });
    z.tick(1); // Resolve the admitted shared attack before checking death.
    assert_eq!(state(&z)["native_monsters"]["9999"]["dead"], true);
    let out = z.tick(20000);
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn lord_delayed_knight_and_quakes_are_shared_checkpoint_state() {
    let mut z = fixture(98, "HellLord");
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("a"),
        object_id: 9998,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 10000000,
        now_ms: 1,
    });
    assert_eq!(state(&z)["native_monsters"]["9998"]["hp"], 1000);
    z.tick(2);
    assert_eq!(
        state(&z)["hell_world"]["summons"].as_array().unwrap().len(),
        1
    );
    assert!(!state(&z)["hell_world"]["quakes"]
        .as_array()
        .unwrap()
        .is_empty());
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(501), restored.tick(501));
    assert!(!z
        .native_monster_snapshots()
        .iter()
        .any(|m| m.name == "HellKnight1"));
    let out = z.tick(502);
    assert_eq!(out, restored.tick(502));
    assert!(z
        .native_monster_snapshots()
        .iter()
        .any(|m| m.name == "HellKnight1"));
    let checkpoint = state(&z);
    let quake = checkpoint["hell_world"]["quakes"]
        .as_array()
        .unwrap()
        .iter()
        .min_by_key(|q| q["start"].as_u64().unwrap())
        .unwrap();
    let start = quake["start"].as_u64().unwrap().max(503);
    let out = z.tick(start);
    assert_eq!(out, restored.tick(start));
    // A quake already started at 502 is retained for late observers too.
    let late = z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("late"),
        account_id: "late".into(),
        character_index: 1,
        object_id: 103,
        name: "late".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 1,
        hp: 100,
        max_hp: 100,
        mp: 10,
        map_file_name: "hell-shared-fixture".into(),
        position: p(
            quake["position"]["x"].as_i64().unwrap() as i32,
            quake["position"]["y"].as_i64().unwrap() as i32,
        ),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    assert!(packets(&out).iter().chain(packets(&late).iter()).any(|p|matches!(p,ServerPacket::ObjectSpell{info}if matches!(info.spell,Spell::MapQuake1|Spell::MapQuake2))));
}
#[test]
fn knight_death_advances_lord_once_and_empty_map_resets_the_stage() {
    let mut z = fixture(98, "HellLord");
    z.tick(1);
    z.tick(501);
    let knight = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.name == "HellKnight1")
        .unwrap();
    join(
        &mut z,
        "killer",
        104,
        p(knight.position.x, knight.position.y + 1),
    );
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("killer"),
        object_id: knight.object_id,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 502,
    });
    z.tick(502);
    assert_eq!(
        state(&z)["native_monsters"]["9998"]["special_ai"]["hell"]["stage"],
        1
    );
    z.tick(503);
    assert_eq!(
        state(&z)["native_monsters"]["9998"]["special_ai"]["hell"]["stage"],
        1
    );
    for name in ["a", "b", "killer"] {
        z.handle(ZoneCommand::Leave {
            session_id: SessionId::new(name),
        });
    }
    z.tick(504);
    assert_eq!(
        state(&z)["native_monsters"]["9998"]["special_ai"]["hell"]["stage"],
        0
    );
}

#[test]
fn hell_knight_parent_life_survives_checkpoint_without_binding_reused_lord_id() {
    // Exercise both a queued map Spawn and an already spawned child at retirement.
    for retire_before_spawn in [true, false] {
        let mut z = fixture(98, "HellLord");
        z.tick(1);
        let old_life = state(&z)["native_monsters"]["9998"]["incarnation"].clone();
        assert_eq!(
            state(&z)["hell_world"]["summons"][0]["parent_incarnation"],
            old_life
        );
        if !retire_before_spawn {
            z.tick(501);
        }
        z.despawn_world_event_monster(9998, 502);
        assert!(
            z.spawn_world_event_monster(&spawn(98, "HellLord", p(40, 40)), 503)
                .0
        );
        assert_ne!(
            state(&z)["native_monsters"]["9998"]["incarnation"],
            old_life
        );
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        // The old map action still spawns after the original Lord was removed.
        assert_eq!(z.tick(504), restored.tick(504));
        let child = z
            .native_monster_snapshots()
            .into_iter()
            .find(|m| m.name == "HellKnight1")
            .unwrap();
        assert_eq!(
            state(&z)["native_monsters"][child.object_id.to_string()]["special_ai"]["hell"]
                ["parent_incarnation"],
            old_life
        );
        for runtime in [&mut z, &mut restored] {
            join(
                runtime,
                "killer",
                104,
                p(child.position.x, child.position.y + 1),
            );
            runtime.handle(ZoneCommand::PlayerAttackObject {
                session_id: SessionId::new("killer"),
                object_id: child.object_id,
                direction: MirDirection::Up,
                spell: 0,
                level: 0,
                attack_type: 0,
                damage: 1,
                now_ms: 505,
            });
        }
        assert_eq!(z.tick(505), restored.tick(505));
        assert_eq!(
            state(&z)["native_monsters"][child.object_id.to_string()]["dead"],
            true
        );
        assert_eq!(
            state(&z)["native_monsters"]["9998"]["special_ai"]["hell"]["stage"],
            0
        );
        assert_eq!(z.tick(506), restored.tick(506));
        assert_eq!(
            state(&z)["native_monsters"]["9998"]["special_ai"]["hell"]["stage"],
            0
        );
    }
}

#[test]
fn hell_map_spawn_outlives_removed_parent_without_a_replacement() {
    let mut z = fixture(98, "HellLord");
    z.tick(1);
    let old_life = state(&z)["native_monsters"]["9998"]["incarnation"].clone();
    z.despawn_world_event_monster(9998, 2);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(500), restored.tick(500));
    assert!(!z
        .native_monster_snapshots()
        .iter()
        .any(|m| m.name == "HellKnight1"));
    assert_eq!(z.tick(501), restored.tick(501));
    let child = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.name == "HellKnight1")
        .unwrap();
    assert_eq!(
        state(&z)["native_monsters"][child.object_id.to_string()]["special_ai"]["hell"]
            ["parent_incarnation"],
        old_life
    );
    assert!(state(&z)["native_monsters"]["9998"].is_null());
}

#[test]
fn bomb_bleeding_ticks_on_next_update_and_does_not_cross_player_lives() {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("hell-shared-fixture"));
    join_resist(&mut z, "a", 101, p(40, 41), 0);
    z.spawn_world_event_monster(&spawn(99, "HellBomb3", p(40, 40)), 0);
    z.tick(10001);
    z.tick(10501);
    assert_eq!(state(&z)["native_periodic_player_poisons"][0]["count"], 0);
    let out = z.tick(10502);
    assert_eq!(state(&z)["native_periodic_player_poisons"][0]["count"], 1);
    // The shipped HellBomb3 has MinSC=MaxSC=0: preserve a zero-value
    // poison tick instead of inventing damage from its DC explosion stat.
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    assert!(packets(&out).iter().any(
        |p| matches!(p,ServerPacket::ObjectEffect{info}if info.object_id==101&&info.effect==18)
    ));
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("a"),
    });
    join_resist(&mut z, "a", 101, p(40, 41), 0);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(12503);
    assert_eq!(out, restored.tick(12503));
    assert!(!out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,..}if *session_id==SessionId::new("a"))));
    assert_eq!(z.player_vitals(&SessionId::new("a")).unwrap().0, 1000000);
}

#[test]
fn bomb_poison_counted_end_clears_all_legacy_projections_without_reappearing() {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("hell-shared-fixture"));
    join_resist(&mut z, "a", 101, p(40, 41), 0);
    z.spawn_world_event_monster(&spawn(99, "HellBomb3", p(40, 40)), 0);
    z.tick(10001);
    z.tick(10501);
    for now in [10502, 12503, 14504, 16505, 18506, 18507, 18508, 20509] {
        z.tick(now);
    }
    let saved = state(&z);
    let player = &saved["players"]["a"];
    assert_eq!(player["poison"].as_u64().unwrap_or(0) & 128, 0);
    assert_eq!(
        player["native_status_poison"].as_u64().unwrap_or(0) & 128,
        0
    );
    assert!(player["native_status_poison_deadlines"]
        .get("128")
        .is_none());
    assert!(saved["native_periodic_player_poisons"]
        .as_array()
        .is_none_or(|v| v.is_empty()));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(22510), restored.tick(22510));
    assert_eq!(
        state(&z)["players"]["a"]["poison"].as_u64().unwrap_or(0) & 128,
        0
    );
}

#[test]
fn bomb_source_replacement_with_same_id_cannot_keep_the_old_poison_lease() {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("hell-shared-fixture"));
    join_resist(&mut z, "a", 101, p(40, 41), 0);
    z.spawn_world_event_monster(&spawn(99, "HellBomb3", p(40, 40)), 0);
    z.tick(10001);
    z.tick(10501);
    z.tick(10502);
    assert_eq!(state(&z)["native_periodic_player_poisons"][0]["count"], 1);
    // Explicit authoritative world-event spawn replaces the dead instance. It is
    // distinct from a session metadata refresh, despite deliberately reusing ID.
    assert!(
        z.spawn_world_event_monster(&spawn(99, "HellBomb3", p(50, 50)), 11000)
            .0
    );
    z.tick(11001);
    z.tick(12503);
    assert_eq!(
        state(&z)["players"]["a"]["poison"].as_u64().unwrap_or(0) & 128,
        0
    );
    assert!(state(&z)["native_periodic_player_poisons"]
        .as_array()
        .is_none_or(|v| v.is_empty()));
}

// Explicit nonzero SC fixture: AI99 behavior with ArcherGuard's DC/SC=255,
// while the retained image selects Bleeding. This does not alter live data.
fn active_nonzero_bleed() -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("hell-shared-fixture"));
    join_resist(&mut z, "a", 101, p(40, 41), 0);
    let mut bomb = spawn(99, "ArcherGuard", p(40, 40));
    bomb.image = 905;
    z.spawn_world_event_monster(&bomb, 0);
    z.tick(10001);
    z.tick(10501);
    z.tick(10502);
    assert_eq!(state(&z)["native_periodic_player_poisons"][0]["count"], 1);
    z
}
#[test]
fn unrelated_observer_monster_death_and_revive_do_not_retire_player_poison() {
    let mut z = active_nonzero_bleed();
    let mut unrelated = spawn(97, "HellKnight1", p(50, 50));
    unrelated.object_id = 20000;
    z.spawn_world_event_monster(&unrelated, 10503);
    let before = state(&z)["native_periodic_player_poisons"].clone();
    let notifications = vec![
        ServerPacket::ObjectDied {
            info: mir2_protocol::ObjectDiedInfo {
                object_id: 20000,
                location: p(50, 50),
                direction: MirDirection::Up,
                kind: 0,
            },
        },
        ServerPacket::ObjectRevived {
            info: mir2_protocol::ObjectRevivedInfo {
                object_id: 20000,
                effect: false,
            },
        },
    ];
    z.handle(ZoneCommand::BroadcastPackets {
        session_id: SessionId::new("a"),
        owner_local_object_id: 1,
        packets: notifications.clone(),
        now_ms: 10504,
    });
    assert_eq!(state(&z)["native_periodic_player_poisons"], before);
    z.handle(ZoneCommand::BroadcastSharedObjectPackets {
        session_id: SessionId::new("a"),
        local_self_object_id: Some(1),
        packets: notifications,
        now_ms: 10505,
    });
    assert_eq!(state(&z)["native_periodic_player_poisons"], before);
    let out = z.tick(12503);
    assert_eq!(state(&z)["native_periodic_player_poisons"][0]["count"], 2);
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged { session_id, damage, .. }if *session_id==SessionId::new("a")&&*damage==255)));
}
#[test]
fn purification_then_same_bit_reapplication_starts_a_fresh_counted_lease() {
    let mut z = active_nonzero_bleed();
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("a"),
        MirClass::Taoist,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    let clean = z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("a"),
        object_id: 101,
        spell: Spell::Purification,
        direction: MirDirection::Up,
        target: p(40, 41),
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 10503,
    });
    assert!(packets(&clean)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPoisoned{object_id:101,poison}if poison&128==0)));
    assert!(state(&z)["native_periodic_player_poisons"]
        .as_array()
        .is_none_or(|v| v.is_empty()));
    let after = z.tick(12503);
    assert!(!after
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    assert_eq!(
        state(&z)["players"]["a"]["poison"].as_u64().unwrap_or(0) & 128,
        0
    );
    let mut bomb = spawn(99, "ArcherGuard", p(40, 40));
    bomb.object_id = 12345;
    bomb.image = 905;
    z.spawn_world_event_monster(&bomb, 12600);
    z.tick(22601);
    z.tick(23101);
    let leases = state(&z)["native_periodic_player_poisons"].clone();
    assert_eq!(leases.as_array().unwrap().len(), 1);
    assert_eq!(leases[0]["owner"], 12345);
    assert_eq!(leases[0]["count"], 0);
    z.tick(23102);
    assert_eq!(state(&z)["native_periodic_player_poisons"][0]["count"], 1);
}
#[test]
fn positive_bleeding_and_green_periodic_damage_never_emits_object_struck() {
    let mut bleed = active_nonzero_bleed();
    for now in [12503, 14504, 16505, 18506] {
        let out = bleed.tick(now);
        assert!(out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { damage: 255, .. })));
        assert!(!packets(&out)
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectStruck { .. })));
    }
    let mut green = None;
    for id in 9157..9189 {
        let mut z = ZoneRuntime::new(ZoneKey::for_map("hell-shared-fixture"));
        join_resist(&mut z, "a", 101, p(40, 41), 0);
        let mut hugger = spawn(69, "ArcherGuard", p(40, 40));
        hugger.object_id = id;
        hugger.attack_speed_ms = 2000;
        z.spawn_world_event_monster(&hugger, 0);
        z.tick(2001);
        z.tick(2501);
        if state(&z)["native_periodic_player_poisons"]
            .as_array()
            .is_some_and(|v| v.iter().any(|p| p["mask"] == 1))
        {
            green = Some(z);
            break;
        }
    }
    let mut green = green.expect("public seeded Hugger casts must exercise Green");
    for now in [2502, 4503, 6504, 8505, 10506] {
        let out = green.tick(now);
        assert!(out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { damage: 255, .. })));
        assert!(!packets(&out)
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectStruck { .. })));
    }
}
#[test]
fn canonical_checkpoint_rejects_modified_periodic_and_hell_world_state() {
    let z = active_nonzero_bleed();
    let bytes = z.checkpoint_bytes().unwrap();
    assert!(ZoneRuntime::restore_checkpoint(&bytes).is_ok());
    let mut tampered: Value = serde_json::from_slice(&bytes).unwrap();
    tampered["native_periodic_player_poisons"][0]["value"] = serde_json::json!(1);
    assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&tampered).unwrap()).is_err());
    let mut lord = fixture(98, "HellLord");
    lord.tick(1);
    let bytes = lord.checkpoint_bytes().unwrap();
    assert!(ZoneRuntime::restore_checkpoint(&bytes).is_ok());
    let mut tampered: Value = serde_json::from_slice(&bytes).unwrap();
    tampered["hell_world"]["quakes"][0]["value"] = serde_json::json!(987654);
    assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&tampered).unwrap()).is_err());
}

fn owned_hell_fixture(id: u32, image: u16) -> (ZoneRuntime, u32, Point) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("hell-owned-fixture"));
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
        map_file_name: "hell-owned-fixture".into(),
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
    let mut source = spawn(99, "Guard", p(pos.x, pos.y - 1));
    source.object_id = id;
    source.image = image;
    source.hp = 100000;
    source.max_hp = 100000;
    source.attack_speed_ms = 10000;
    assert!(z.spawn_world_event_monster(&source, 1500).0);
    (z, pet, pos)
}

#[test]
fn hell_bomb_shared_owned_target_explosion_finishes_from_corpse_after_checkpoint() {
    let (mut z, pet, _) = owned_hell_fixture(98799, 903);
    z.tick(11501);
    let checkpoint = state(&z);
    assert_eq!(checkpoint["native_monsters"]["98799"]["dead"], true);
    assert_eq!(
        checkpoint["native_monsters"]["98799"]["special_ai"]["hell"]["explosion_source_ref"]
            ["Monster"]["object_id"],
        98799
    );
    let before = checkpoint["native_monsters"][pet.to_string()]["hp"]
        .as_i64()
        .unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(12000), restored.tick(12000));
    assert_eq!(state(&z)["native_monsters"][pet.to_string()]["hp"], before);
    let out = z.tick(12001);
    assert_eq!(out, restored.tick(12001));
    assert!(
        state(&z)["native_monsters"][pet.to_string()]["hp"]
            .as_i64()
            .unwrap()
            < before
    );
    assert_eq!(
        state(&z)["native_monsters"][pet.to_string()]["entity_poison"]
            .as_u64()
            .unwrap_or(0)
            & 8,
        8
    );
    assert!(!out
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
}
#[test]
fn hell_bomb_removed_source_does_not_transfer_queued_blast_to_reused_id() {
    let (mut z, pet, pos) = owned_hell_fixture(98799, 905);
    z.tick(11501);
    let old = state(&z)["native_monsters"]["98799"]["incarnation"].clone();
    z.despawn_world_event_monster(98799, 11502);
    let mut replacement = spawn(99, "Guard", p(pos.x, pos.y - 1));
    replacement.object_id = 98799;
    replacement.max_hp = 100000;
    replacement.hp = 100000;
    replacement.image = 905;
    assert!(z.spawn_world_event_monster(&replacement, 11502).0);
    assert_ne!(state(&z)["native_monsters"]["98799"]["incarnation"], old);
    let before = state(&z)["native_monsters"][pet.to_string()]["hp"].clone();
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(12001), restored.tick(12001));
    assert_eq!(state(&z)["native_monsters"][pet.to_string()]["hp"], before);
    assert_eq!(
        state(&z)["native_monsters"][pet.to_string()]["entity_poison"]
            .as_u64()
            .unwrap_or(0),
        0
    );
}
#[test]
fn hell_bomb_native_dazed_and_bleeding_are_real_periodic_statuses() {
    for (image, bit) in [(904, 1024u64), (905, 128u64)] {
        let (mut z, pet, _) = owned_hell_fixture(98799, image);
        z.tick(11501);
        z.tick(12001);
        assert_eq!(
            state(&z)["native_monsters"][pet.to_string()]["entity_poison"]
                .as_u64()
                .unwrap_or(0)
                & bit,
            bit
        );
        let before = state(&z)["native_monsters"][pet.to_string()]["hp"]
            .as_i64()
            .unwrap();
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        let out = z.tick(12002);
        assert_eq!(out, restored.tick(12002));
        if image == 905 {
            assert!(
                state(&z)["native_monsters"][pet.to_string()]["hp"]
                    .as_i64()
                    .unwrap()
                    < before
            );
            assert!(!packets(&out)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==pet)));
        }
    }
}

#[test]
fn hell_map_quake_strikes_unowned_native_without_caster_and_survives_restore() {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("hell-shared-fixture"));
    join(&mut z, "a", 101, p(40, 41));
    let mut lord = spawn(98, "Guard", p(40, 40));
    lord.attack_speed_ms = 100000;
    assert!(z.spawn_world_event_monster(&lord, 0).0);
    z.tick(1);
    let checkpoint = state(&z);
    let quake = checkpoint["hell_world"]["quakes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| {
            q["value"].as_i64().unwrap() > 0
                && q["start"].as_u64().unwrap() > 2
                && q["position"] != serde_json::to_value(p(40, 41)).unwrap()
                && q["position"] != serde_json::to_value(p(40, 40)).unwrap()
        })
        .expect("populated Guard stats generate a positive delayed quake")
        .clone();
    let position: Point = serde_json::from_value(quake["position"].clone()).unwrap();
    let start = quake["start"].as_u64().unwrap();
    z.despawn_world_event_monster(lord.object_id, 2);
    let mut victim = spawn(0, "Deer", position);
    victim.object_id = 98800;
    victim.disposition = Some(WorldEntityDisposition::Neutral);
    victim.hp = 100000;
    victim.max_hp = 100000;
    assert!(z.spawn_world_event_monster(&victim, 2).0);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    let out = z.tick(start);
    assert_eq!(out, restored.tick(start));
    assert!(
        state(&z)["native_monsters"]["98800"]["hp"]
            .as_i64()
            .unwrap()
            < 100000
    );
    assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==98800&&info.attacker_id==0)));
    let hp = state(&z)["native_monsters"]["98800"]["hp"]
        .as_i64()
        .unwrap();
    assert_eq!(z.tick(start + 500), restored.tick(start + 500));
    assert!(
        state(&z)["native_monsters"]["98800"]["hp"]
            .as_i64()
            .unwrap()
            < hp
    );
}

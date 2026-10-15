//! Public shared-Zone lifecycle contracts for Crystal RevivingZombie (AI25).
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, SessionId, WorldEntityDisposition, ZoneCommand,
    ZoneJoin, ZoneKey, ZoneMonsterSpawn, ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;

const MAP: &str = "shared-reviving-zombie-fixture";

fn player(name: &str, id: u32, x: i32, y: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(name),
        account_id: format!("{name}-account"),
        character_index: id as i32,
        object_id: id,
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 100_000,
        max_hp: 100_000,
        mp: 100,
        map_file_name: MAP.into(),
        position: Point { x, y },
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 100,
            max_dc: 100,
            accuracy: 100,
            min_ac: 100_000,
            max_ac: 100_000,
            ..Default::default()
        },
    }
}

fn fixture(id: u32) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    zone.handle(ZoneCommand::Join(player("killer", 101, 10, 11)));
    zone.handle(ZoneCommand::Join(player("observer", 102, 13, 13)));
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("killer"),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("killer"),
        now_ms: 0,
        monster: zombie_spawn(id),
    });
    zone
}

fn zombie_spawn(id: u32) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: id,
        name: "Zombie3".into(),
        name_colour_argb: -1,
        image: 25,
        ai: 25,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 20,
        hp: 100,
        max_hp: 100,
        experience: 100,
        move_speed_ms: 1_000,
        attack_speed_ms: 1_000,
        friendly_guild: None,
        defense: Default::default(),
        respawn: None,
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Down,
        // This cached spawn-time result must never be recycled on death.
        drops: vec![GroundDropSnapshot {
            object_id: 0,
            name: "STALE SPAWN ROLL".into(),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: 1,
            source_monster: "old-life".into(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 99_999_999 },
        }],
    }
}

fn monster_state(zone: &ZoneRuntime, id: u32) -> Value {
    let bytes = zone.checkpoint_bytes().expect("real checkpoint");
    let state: Value = serde_json::from_slice(&bytes).unwrap();
    state["native_monsters"][id.to_string()].clone()
}

fn revival_state(zone: &ZoneRuntime, id: u32) -> Value {
    monster_state(zone, id)["special_ai"]["revival"].clone()
}

fn fixture_with_lives(lives: u64) -> (ZoneRuntime, u32) {
    for id in 90_000..90_256 {
        let zone = fixture(id);
        if revival_state(&zone, id)["life_count"].as_u64() == Some(lives) {
            return (zone, id);
        }
    }
    panic!("checkpointed constructor RNG did not produce life_count={lives}");
}

fn packets_for<'a>(outbounds: &'a [ZoneOutbound], recipient: &str) -> Vec<&'a ServerPacket> {
    let recipient = SessionId::new(recipient);
    outbounds
        .iter()
        .flat_map(|outbound| match outbound {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == recipient => packets.iter(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&recipient) => packets.iter(),
            ZoneOutbound::ToAll { packets } => packets.iter(),
            _ => [].iter(),
        })
        .collect()
}

fn kill(zone: &mut ZoneRuntime, id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
    let launch = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("killer"),
        object_id: id,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms,
    });
    assert!(
        packets_for(&launch, "killer").iter().any(
            |packet| matches!(packet, ServerPacket::ObjectAttack { info } if info.object_id == 101)
        ),
        "attack must be admitted: {launch:?}"
    );
    let out = zone.tick(now_ms);
    for observer in ["killer", "observer"] {
        assert!(
            packets_for(&out, observer).iter().any(
                |packet| matches!(packet, ServerPacket::ObjectDied { info } if info.object_id == id)
            ),
            "both observers must see the same death: {out:?}"
        );
    }
    out
}

fn did_revive(outbounds: &[ZoneOutbound], recipient: &str, id: u32) -> bool {
    packets_for(outbounds, recipient).iter().any(|packet|
        matches!(packet, ServerPacket::ObjectRevived { info } if info.object_id == id && !info.effect))
}

#[test]
fn random_zero_one_or_two_revivals_scale_each_life_and_reward_independently() {
    for lives in 0..=2 {
        let (mut zone, id) = fixture_with_lives(lives);
        let mut now = 1;
        let mut drop_ids = std::collections::BTreeSet::new();
        let mut previous_roll = None;
        for life in 0..=lives {
            let incarnation = monster_state(&zone, id)["incarnation"].as_u64().unwrap();
            assert!(incarnation > 0);
            let death = kill(&mut zone, id, now);
            assert_eq!(monster_state(&zone, id)["incarnation"], incarnation);
            let awards: Vec<_> = death
                .iter()
                .filter_map(|outbound| match outbound {
                    ZoneOutbound::MonsterKillAward { session_id, award }
                        if session_id.as_str() == "killer" =>
                    {
                        Some(award)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(awards.len(), 1, "one reward per actual death");
            assert_eq!(awards[0].experience, 100 - 25 * life as u32);
            for drop in &awards[0].drops {
                assert_ne!(drop.name, "STALE SPAWN ROLL");
                assert!(
                    drop_ids.insert(drop.object_id),
                    "ground object ID reused between lives"
                );
                if let GroundDropLootSnapshot::InventoryItem { exact_item, .. } = &drop.loot {
                    let exact = exact_item
                        .as_ref()
                        .expect("fresh drop keeps exact metadata");
                    assert!(
                        !exact.uid_assigned,
                        "new life must use durable allocator, not an old assigned UID"
                    );
                    assert_eq!(exact.item.unique_id, 0);
                }
            }
            let state = revival_state(&zone, id);
            assert_eq!(state["death_generation"], life + 1);
            assert_eq!(state["revival_count"], life);
            let roll = state["last_drop_roll"].as_u64().unwrap();
            assert_ne!(
                Some(roll),
                previous_roll,
                "each death obtains a fresh drop roll"
            );
            previous_roll = Some(roll);
            let deadline = state["revive_at_ms"].as_u64().unwrap();
            assert!((4_000..=23_000).contains(&(deadline - now)));
            assert_eq!((deadline - now) % 1_000, 0);
            let equal = zone.tick(deadline);
            assert!(!did_revive(&equal, "killer", id), "Crystal uses strict >");
            let after = zone.tick(deadline + 1);
            for observer in ["killer", "observer"] {
                assert_eq!(did_revive(&after, observer, id), life < lives);
            }
            if life < lives {
                let body = monster_state(&zone, id);
                assert_eq!(body["hp"], 100 - 25 * (life + 1));
                assert_eq!(body["dead"], false);
                assert!(body["incarnation"].as_u64().unwrap() > incarnation);
                assert!(body["next_ai_ready_at_ms"].as_u64().unwrap() >= deadline + 2_001);
                assert!(body["next_attack_ready_at_ms"].as_u64().unwrap() >= deadline + 2_001);
                assert!(!did_revive(&zone.tick(deadline + 1), "killer", id));
                now = deadline + 2;
            } else {
                assert_eq!(monster_state(&zone, id)["dead"], true);
            }
        }
    }
}

#[test]
fn revival_delay_includes_four_and_twenty_three_seconds_without_fixed_count() {
    let mut delays = std::collections::BTreeSet::new();
    let mut counts = std::collections::BTreeSet::new();
    for id in 91_000..91_512 {
        let mut zone = fixture(id);
        counts.insert(revival_state(&zone, id)["life_count"].as_u64().unwrap());
        kill(&mut zone, id, 1);
        delays.insert(revival_state(&zone, id)["revive_at_ms"].as_u64().unwrap() - 1);
        if delays.len() == 20 && counts.len() == 3 {
            break;
        }
    }
    assert_eq!(counts, [0, 1, 2].into_iter().collect());
    assert_eq!(delays, (4..=23).map(|seconds| seconds * 1_000).collect());
}

#[test]
fn checkpointed_dead_zombie_replays_one_revival_without_reroll_or_second_reward() {
    let (mut zone, id) = fixture_with_lives(2);
    kill(&mut zone, id, 1);
    let before = revival_state(&zone, id);
    let deadline = before["revive_at_ms"].as_u64().unwrap();
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut restored =
        ZoneRuntime::restore_checkpoint(&bytes).expect("unmodified checkpoint restore");
    assert_eq!(
        zone.canonical_state_root().unwrap(),
        restored.canonical_state_root().unwrap()
    );
    for now in [1, deadline, deadline + 1, deadline + 1] {
        let expected = zone.tick(now);
        let actual = restored.tick(now);
        assert_eq!(actual, expected);
        assert!(!actual
            .iter()
            .any(|out| matches!(out, ZoneOutbound::MonsterKillAward { .. })));
        assert_eq!(
            zone.canonical_state_root().unwrap(),
            restored.canonical_state_root().unwrap()
        );
        let state = revival_state(&restored, id);
        assert_eq!(state["rng_state"], before["rng_state"]);
        assert_eq!(state["rng_draws"], before["rng_draws"]);
        assert_eq!(state["death_generation"], 1);
    }
    let alive = restored.checkpoint_bytes().unwrap();
    let after_alive_restore = ZoneRuntime::restore_checkpoint(&alive).unwrap();
    assert_eq!(revival_state(&after_alive_restore, id)["revival_count"], 1);
}

#[test]
fn expired_corpse_is_removed_before_overdue_revival_after_long_offline_interval() {
    let (mut zone, id) = fixture_with_lives(2);
    kill(&mut zone, id, 1);
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    let out = restored.tick(180_001);
    for observer in ["killer", "observer"] {
        assert!(!did_revive(&out, observer, id));
        assert!(packets_for(&out, observer).iter().any(
            |packet| matches!(packet, ServerPacket::ObjectRemove { object_id } if *object_id == id)
        ));
    }
    assert!(!restored.has_native_monster(id));
    assert!(!restored.retains_object_id(id));
    assert!(!did_revive(&restored.tick(180_002), "killer", id));
}

#[test]
fn personal_spawn_refresh_cannot_replace_a_pending_self_revival_without_respawn_policy() {
    let (mut zone, id) = fixture_with_lives(2);
    kill(&mut zone, id, 1);
    let before = revival_state(&zone, id);
    let deadline = before["revive_at_ms"].as_u64().unwrap();
    let refresh = zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("observer"),
        monster: zombie_spawn(id),
        now_ms: 2,
    });
    assert_eq!(revival_state(&zone, id), before);
    assert_eq!(monster_state(&zone, id)["dead"], true);
    for observer in ["killer", "observer"] {
        assert!(!packets_for(&refresh, observer).iter().any(
            |packet| matches!(packet, ServerPacket::ObjectRevived { info } if info.object_id == id)
        ));
    }
    let revive = zone.tick(deadline + 1);
    for observer in ["killer", "observer"] {
        assert!(did_revive(&revive, observer, id));
        assert!(packets_for(&revive, observer).iter().any(
            |packet| matches!(packet, ServerPacket::ObjectHealth { info }
                if info.object_id == id && info.percent == 75)
        ));
    }
    let late = zone.handle(ZoneCommand::Join(player("late", 103, 14, 14)));
    assert!(
        packets_for(&late, "late").iter().any(
            |packet| matches!(packet, ServerPacket::ObjectHealth { info }
            if info.object_id == id && info.percent == 75)
        ),
        "ObjectRevived must not leave retained health empty for new AOI observers"
    );
}

#[test]
fn revival_discards_prior_life_control_deadline_and_retained_poison() {
    let (mut zone, id) = fixture_with_lives(2);
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("observer"),
        object_id: id,
        spell: Spell::Trap,
        direction: MirDirection::Up,
        target: Point { x: 10, y: 10 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 1,
    });
    zone.tick(501);
    let controlled = monster_state(&zone, id);
    let control_deadline = controlled["control_until_ms"].as_u64().unwrap();
    assert!(
        control_deadline > 25_000,
        "fixture must apply long control: {controlled}"
    );
    kill(&mut zone, id, 502);
    let dead = monster_state(&zone, id);
    assert_eq!(dead["control_until_ms"], 0);
    assert_eq!(dead["control_poison"], 0);
    let deadline = revival_state(&zone, id)["revive_at_ms"].as_u64().unwrap();
    assert!(deadline + 2_001 < control_deadline);
    let out = zone.tick(deadline + 1);
    let revived = monster_state(&zone, id);
    assert_eq!(revived["next_ai_ready_at_ms"], deadline + 2_001);
    assert_eq!(revived["next_attack_ready_at_ms"], deadline + 2_001);
    for observer in ["killer", "observer"] {
        assert!(did_revive(&out, observer, id));
        assert!(packets_for(&out, observer).iter().any(
            |packet| matches!(packet, ServerPacket::ObjectPoisoned { object_id, poison }
                if *object_id == id && *poison == 0)
        ));
    }
}

#[test]
fn final_death_clears_poison_from_body_and_late_observer_snapshot_once() {
    let (mut zone, id) = fixture_with_lives(0);
    let cast = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: SessionId::new("observer"),
        object_id: id,
        spell: Spell::Poisoning,
        direction: MirDirection::Up,
        target: Point { x: 10, y: 10 },
        cast: true,
        level: 3,
        damage: 12,
        mp_cost: 0,
        cooldown_ms: 500,
        item_param: 1,
        now_ms: 1,
    });
    assert!(packets_for(&cast, "killer").iter().any(
        |packet| matches!(packet, ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == id && *poison == 1)
    ));
    let death = kill(&mut zone, id, 2);
    for observer in ["killer", "observer"] {
        assert!(packets_for(&death, observer).iter().any(
            |packet| matches!(packet, ServerPacket::ObjectPoisoned { object_id, poison }
                if *object_id == id && *poison == 0)
        ));
    }
    let body = monster_state(&zone, id);
    for field in [
        "damage_poison",
        "damage_poison_value",
        "damage_poison_next_damage_at_ms",
        "damage_poison_expires_at_ms",
        "damage_poison_owner_object_id",
    ] {
        assert_eq!(body[field], 0, "death must clear {field}");
    }
    assert!(body["damage_poison_owner_session_id"].is_null());
    let late = zone.handle(ZoneCommand::Join(player("late", 103, 14, 14)));
    assert!(packets_for(&late, "late").iter().any(
        |packet| matches!(packet, ServerPacket::ObjectMonster { info }
            if info.object_id == id && info.dead && info.poison == 0)
    ));
    let repeated = zone.tick(3);
    assert!(!packets_for(&repeated, "killer").iter().any(|packet|
        matches!(packet, ServerPacket::ObjectPoisoned { object_id, .. } if *object_id == id)));
}

#[test]
fn monster_incarnation_survives_metadata_checkpoint_and_actual_replacement() {
    let id = 93000;
    let mut zone = fixture(id);
    let original = monster_state(&zone, id)["incarnation"].as_u64().unwrap();
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("killer"),
        monster: zombie_spawn(id),
        now_ms: 1,
    });
    assert_eq!(monster_state(&zone, id)["incarnation"], original);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(monster_state(&restored, id)["incarnation"], original);
    for runtime in [&mut zone, &mut restored] {
        runtime.despawn_world_event_monster(id, 2);
        runtime.handle(ZoneCommand::SpawnMonster {
            session_id: SessionId::new("killer"),
            monster: zombie_spawn(id),
            now_ms: 3,
        });
        assert!(monster_state(runtime, id)["incarnation"].as_u64().unwrap() > original);
    }
    assert_eq!(
        zone.checkpoint_bytes().unwrap(),
        restored.checkpoint_bytes().unwrap()
    );
}

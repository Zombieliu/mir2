//! AI39/40 lifecycles through public shared Zone commands and real checkpoints.
//! AI41/42 YinDevilNode buffs are not covered by these tests.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, SessionId, WorldEntityDisposition, ZoneBounds,
    ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn, ZoneOutbound,
    ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;

const MAP: &str = "shared-bomb-spider-fixture";
const ID: u32 = 90_040;

fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn player(session: &str, object_id: u32, x: i32, y: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(session),
        account_id: format!("{session}-account"),
        character_index: object_id as i32,
        object_id,
        name: session.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 100_000,
        max_hp: 100_000,
        mp: 100,
        map_file_name: MAP.into(),
        position: point(x, y),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 100,
            max_dc: 100,
            accuracy: 100,
            ..Default::default()
        },
    }
}

fn spawn(id: u32) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: id,
        name: "BombSpider".into(),
        name_colour_argb: -1,
        image: 40,
        ai: 40,
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
        position: point(10, 10),
        direction: MirDirection::Down,
        drops: vec![GroundDropSnapshot {
            object_id: 0,
            name: "Gold".into(),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: 1,
            source_monster: "BombSpider".into(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 7 },
        }],
    }
}

fn fixture(collision: ZoneCollision, target_y: i32) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map(MAP), collision);
    zone.handle(ZoneCommand::Join(player("first", 101, 10, target_y)));
    zone.handle(ZoneCommand::Join(player("second", 102, 11, 10)));
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("first"),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("first"),
        monster: spawn(ID),
        now_ms: 0,
    });
    zone
}

fn packets<'a>(out: &'a [ZoneOutbound], session: &str) -> Vec<&'a ServerPacket> {
    let session = SessionId::new(session);
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&session) => packets.iter(),
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == session => packets.iter(),
            ZoneOutbound::ToAll { packets } => packets.iter(),
            _ => [].iter(),
        })
        .collect()
}

fn checkpoint_monster(zone: &ZoneRuntime) -> Value {
    let value: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    value["native_monsters"][ID.to_string()].clone()
}

fn spider_state(zone: &ZoneRuntime) -> Value {
    checkpoint_monster(zone)["special_ai"]["spider"].clone()
}

fn damage_to(out: &[ZoneOutbound], session: &str) -> Vec<i32> {
    out.iter()
        .filter_map(|o| match o {
            ZoneOutbound::PlayerDamaged {
                session_id, damage, ..
            } if session_id.as_str() == session => Some(*damage),
            _ => None,
        })
        .collect()
}

fn died(out: &[ZoneOutbound], session: &str) -> bool {
    packets(out, session)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info} if info.object_id==ID))
}

fn assert_no_swing(out: &[ZoneOutbound]) {
    for session in ["first", "second"] {
        assert!(
            !packets(out, session)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==ID)),
            "BombSpider must not enter the ordinary melee attack path"
        );
    }
}

#[test]
fn adjacent_spider_dies_then_explodes_once_at_500ms_for_both_observers() {
    let mut zone = fixture(ZoneCollision::unbounded(), 11);
    assert_eq!(spider_state(&zone)["explosion_deadline_ms"], 300_000);
    let death = zone.tick(1);
    for session in ["first", "second"] {
        assert!(died(&death, session));
        assert!(damage_to(&death, session).is_empty());
    }
    assert_no_swing(&death);
    assert!(
        !death
            .iter()
            .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })),
        "untagged suicide has no EXP owner"
    );
    assert_eq!(spider_state(&zone)["explosion_at_ms"], 501);
    let early = zone.tick(500);
    assert!(damage_to(&early, "first").is_empty());
    let explosion = zone.tick(501);
    assert_no_swing(&explosion);
    for session in ["first", "second"] {
        assert_eq!(damage_to(&explosion, session).len(), 1);
        for target_id in [101, 102] {
            assert!(packets(&explosion,session).iter().any(|p|
                matches!(p,ServerPacket::ObjectStruck{info} if info.object_id==target_id && info.attacker_id==ID)));
        }
    }
    assert_eq!(spider_state(&zone)["exploded"], true);
    for now in [501, 502, 5_000] {
        let repeated = zone.tick(now);
        assert!(damage_to(&repeated, "first").is_empty());
        assert!(damage_to(&repeated, "second").is_empty());
        assert_no_swing(&repeated);
        assert!(!repeated
            .iter()
            .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
    }
}

#[test]
fn player_kill_awards_drops_once_and_personal_refresh_cannot_cancel_pending_explosion() {
    let mut zone = fixture(ZoneCollision::unbounded(), 11);
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("first"),
        object_id: ID,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 1,
    });
    let death = zone.tick(1);
    assert!(died(&death, "first"));
    let awards: Vec<_> = death
        .iter()
        .filter_map(|o| match o {
            ZoneOutbound::MonsterKillAward { award, .. } => Some(award),
            _ => None,
        })
        .collect();
    assert_eq!(awards.len(), 1);
    assert_eq!(awards[0].experience, 100);
    assert_eq!(awards[0].drops.len(), 1);
    assert!(awards[0].drops[0].object_id > 0);
    let before = spider_state(&zone);
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("second"),
        monster: spawn(ID),
        now_ms: 2,
    });
    assert_eq!(spider_state(&zone), before);
    assert_eq!(checkpoint_monster(&zone)["dead"], true);
    let explosion = zone.tick(501);
    assert_eq!(damage_to(&explosion, "first").len(), 1);
    assert!(!explosion
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
    assert!(damage_to(&zone.tick(502), "first").is_empty());
}

#[test]
fn pending_explosion_checkpoint_replays_identical_damage_and_never_reexecutes() {
    let mut original = fixture(ZoneCollision::unbounded(), 11);
    original.tick(1);
    let bytes = original.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    for now in [500, 501, 501, 502] {
        assert_eq!(original.tick(now), restored.tick(now));
        assert_eq!(
            original.canonical_state_root().unwrap(),
            restored.canonical_state_root().unwrap()
        );
    }
    let exploded = restored.checkpoint_bytes().unwrap();
    let mut again = ZoneRuntime::restore_checkpoint(&exploded).unwrap();
    assert!(damage_to(&again.tick(503), "first").is_empty());
    assert_eq!(spider_state(&again)["exploded"], true);
}

#[test]
fn explosion_uses_impact_positions_and_never_hits_outside_radius_one() {
    let mut zone = fixture(ZoneCollision::unbounded(), 11);
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("second"),
        position: point(12, 10),
        direction: MirDirection::Left,
    });
    zone.tick(1);
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("first"),
        position: point(14, 10),
        direction: MirDirection::Left,
    });
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("second"),
        position: point(11, 10),
        direction: MirDirection::Left,
    });
    let impact = zone.tick(501);
    assert!(damage_to(&impact, "first").is_empty());
    assert_eq!(damage_to(&impact, "second").len(), 1);
}

#[test]
fn five_minute_lifetime_is_strict_and_survives_blocked_movement() {
    let mut zone = fixture(
        ZoneCollision::unbounded().with_blocked_cells([point(10, 11)]),
        14,
    );
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("second"),
        position: point(14, 14),
        direction: MirDirection::Left,
    });
    let equal = zone.tick(300_000);
    assert!(!died(&equal, "first"));
    assert_eq!(checkpoint_monster(&zone)["dead"], false);
    assert_eq!(checkpoint_monster(&zone)["position"]["x"], 10);
    assert_eq!(checkpoint_monster(&zone)["position"]["y"], 10);
    let after = zone.tick(300_001);
    assert!(died(&after, "first"));
    assert_eq!(spider_state(&zone)["explosion_at_ms"], 300_501);
}

#[test]
fn missing_target_triggers_death_and_corpse_expires_without_resurrection() {
    let mut zone = fixture(ZoneCollision::unbounded(), 14);
    for session in ["first", "second"] {
        zone.handle(ZoneCommand::SyncPlayerTransform {
            session_id: SessionId::new(session),
            position: if session == "first" {
                point(50, 50)
            } else {
                point(52, 50)
            },
            direction: MirDirection::Up,
        });
    }
    zone.tick(1);
    assert_eq!(checkpoint_monster(&zone)["dead"], true);
    assert_eq!(spider_state(&zone)["explosion_at_ms"], 501);
    assert!(damage_to(&zone.tick(501), "first").is_empty());
    zone.tick(180_001);
    assert!(!zone.has_native_monster(ID));
    assert!(!zone.retains_object_id(ID));
}

#[test]
fn full_poison_resistance_blocks_explosion_poison_while_damage_still_lands() {
    let mut zone = fixture(ZoneCollision::unbounded(), 11);
    for session in ["first", "second"] {
        zone.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: SessionId::new(session),
            stats: ZonePlayerCombatStats {
                poison_resist: 10,
                ..Default::default()
            },
        });
    }
    zone.tick(1);
    let out = zone.tick(501);
    assert_eq!(damage_to(&out, "first").len(), 1);
    assert_eq!(damage_to(&out, "second").len(), 1);
    assert!(!packets(&out, "first").iter().any(
        |p| matches!(p,ServerPacket::ObjectPoisoned{object_id,poison}
            if [101,102].contains(object_id) && poison & 1 != 0)
    ));
}

#[test]
fn ordinary_monster_has_no_spider_lifecycle_state() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    zone.handle(ZoneCommand::Join(player("first", 101, 10, 11)));
    let mut ordinary = spawn(ID);
    ordinary.ai = 5;
    ordinary.name = "Scarecrow".into();
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("first"),
        monster: ordinary,
        now_ms: 0,
    });
    zone.tick(1);
    assert!(spider_state(&zone).is_null());
    assert_eq!(checkpoint_monster(&zone)["dead"], false);
}

#[test]
fn damaged_spider_suicide_credits_owner_once_but_expired_tag_does_not() {
    for expire_tag in [false, true] {
        let mut zone = fixture(
            ZoneCollision::unbounded().with_blocked_cells([point(10, 9)]),
            11,
        );
        zone.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: SessionId::new("first"),
            stats: ZonePlayerCombatStats {
                min_dc: 10,
                max_dc: 10,
                accuracy: 100,
                ..Default::default()
            },
        });
        zone.handle(ZoneCommand::PlayerAttackObject {
            session_id: SessionId::new("first"),
            object_id: ID,
            direction: MirDirection::Up,
            spell: 0,
            level: 0,
            attack_type: 0,
            damage: 1,
            now_ms: 1,
        });
        if expire_tag {
            zone.handle(ZoneCommand::SyncPlayerTransform {
                session_id: SessionId::new("first"),
                position: point(10, 6),
                direction: MirDirection::Down,
            });
            zone.handle(ZoneCommand::SyncPlayerTransform {
                session_id: SessionId::new("second"),
                position: point(14, 14),
                direction: MirDirection::Up,
            });
        }
        let first = zone.tick(1);
        let death = if expire_tag {
            assert_eq!(checkpoint_monster(&zone)["hp"], 90);
            assert_eq!(checkpoint_monster(&zone)["dead"], false);
            zone.handle(ZoneCommand::SyncPlayerTransform {
                session_id: SessionId::new("first"),
                position: point(10, 11),
                direction: MirDirection::Up,
            });
            zone.tick(5_002)
        } else {
            first
        };
        let awards: Vec<_> = death
            .iter()
            .filter_map(|o| match o {
                ZoneOutbound::MonsterKillAward { session_id, award }
                    if session_id.as_str() == "first" =>
                {
                    Some(award)
                }
                _ => None,
            })
            .collect();
        assert_eq!(awards.len(), usize::from(!expire_tag));
        if !expire_tag {
            assert_eq!(awards[0].experience, 100);
            assert_eq!(awards[0].drops.len(), 1);
        }
        assert!(died(&death, "first"));
        let due = spider_state(&zone)["explosion_at_ms"].as_u64().unwrap();
        assert!(!zone
            .tick(due)
            .iter()
            .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
    }
}

fn root_spawn() -> ZoneMonsterSpawn {
    let mut root = spawn(ID);
    root.name = "RootSpider".into();
    root.image = 51;
    root.ai = 39;
    root.drops[0].name = "ROOT-ONLY-LOOT".into();
    root
}

fn root_fixture(collision: ZoneCollision, first: Point, second: Point) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map(MAP), collision);
    zone.handle(ZoneCommand::Join(player("first", 101, first.x, first.y)));
    zone.handle(ZoneCommand::Join(player("second", 102, second.x, second.y)));
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("first"),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("first"),
        monster: root_spawn(),
        now_ms: 0,
    });
    zone
}

fn root_state(zone: &ZoneRuntime) -> Value {
    spider_state(zone)["root"].clone()
}

fn child_ids(zone: &ZoneRuntime) -> Vec<u32> {
    root_state(zone)["slave_object_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u32)
        .collect()
}

fn body_state(zone: &ZoneRuntime, id: u32) -> Value {
    let checkpoint: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    checkpoint["native_monsters"][id.to_string()].clone()
}

fn root_attack_count(out: &[ZoneOutbound], session: &str) -> usize {
    packets(out, session)
        .iter()
        .filter(|packet| matches!(packet,ServerPacket::ObjectAttack{info} if info.object_id==ID))
        .count()
}

#[test]
fn root_spider_summons_one_shared_child_after_500ms_with_source_relationship_and_cadence() {
    let mut zone = root_fixture(ZoneCollision::unbounded(), point(10, 14), point(14, 14));
    let direction: MirDirection =
        serde_json::from_value(checkpoint_monster(&zone)["direction"].clone()).unwrap();
    assert!(matches!(
        direction,
        MirDirection::Up | MirDirection::UpRight | MirDirection::Right
    ));
    let late = zone.handle(ZoneCommand::Join(player("late", 103, 15, 14)));
    assert!(packets(&late,"late").iter().any(|packet|
        matches!(packet,ServerPacket::ObjectMonster{info} if info.object_id==ID && info.direction==direction)));
    assert_eq!(root_attack_count(&zone.tick(2_000), "first"), 0);
    let attack = zone.tick(2_001);
    for session in ["first", "second"] {
        assert_eq!(root_attack_count(&attack, session), 1);
    }
    assert!(child_ids(&zone).is_empty());
    let pending = root_state(&zone)["pending_spawn"].clone();
    assert_eq!(pending["due_at_ms"], 2_501);
    let expected = match direction {
        MirDirection::Up => point(10, 11),
        MirDirection::UpRight => point(11, 11),
        _ => point(9, 11),
    };
    assert_eq!(
        pending["position"],
        serde_json::to_value(&expected).unwrap()
    );
    assert_eq!(root_attack_count(&zone.tick(2_001), "first"), 0);
    assert!(child_ids(&zone).is_empty());
    zone.tick(2_500);
    assert!(child_ids(&zone).is_empty());
    let spawned = zone.tick(2_501);
    let children = child_ids(&zone);
    assert_eq!(children.len(), 1);
    let child = children[0];
    for session in ["first", "second"] {
        assert_eq!(packets(&spawned,session).iter().filter(|packet|
            matches!(packet,ServerPacket::ObjectMonster{info} if info.object_id==child)).count(),1);
    }
    let body = body_state(&zone, child);
    assert_eq!(body["ai"], 40);
    assert_eq!(body["position"], serde_json::to_value(&expected).unwrap());
    assert_eq!(body["master_object_id"], 0, "SlaveList is not pet Master");
    assert!(body["owner_session_id"].is_null());
    assert_eq!(body["special_ai"]["spider"]["parent_object_id"], ID);
    assert_eq!(body["special_ai"]["spider"]["target_object_id"], 101);
    assert_eq!(
        body["special_ai"]["spider"]["explosion_deadline_ms"],
        302_001
    );
    assert_eq!(
        body["next_ai_ready_at_ms"], 4_501,
        "Spawned overrides the earlier +1000 value"
    );
    assert!(root_state(&zone)["pending_spawn"].is_null());
    zone.tick(2_501);
    assert_eq!(child_ids(&zone), children);
    assert_eq!(root_attack_count(&zone.tick(5_001), "first"), 0);
    assert_eq!(root_attack_count(&zone.tick(5_002), "first"), 1);
    assert_eq!(
        checkpoint_monster(&zone)["position"],
        serde_json::to_value(point(10, 10)).unwrap()
    );
    assert_eq!(
        checkpoint_monster(&zone)["direction"],
        serde_json::to_value(direction).unwrap()
    );
}

#[test]
fn root_pending_spawn_survives_producer_death_refresh_and_checkpoint_exactly_once() {
    let mut zone = root_fixture(ZoneCollision::unbounded(), point(10, 14), point(14, 14));
    zone.tick(2_001);
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("first"),
        position: point(10, 9),
        direction: MirDirection::Down,
    });
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("first"),
        object_id: ID,
        direction: MirDirection::Down,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 2_002,
    });
    let death = zone.tick(2_002);
    assert!(died(&death, "first"));
    let state = root_state(&zone);
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("second"),
        monster: root_spawn(),
        now_ms: 2_003,
    });
    assert_eq!(root_state(&zone), state);
    assert_eq!(checkpoint_monster(&zone)["dead"], true);
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    for now in [2_500, 2_501, 2_501, 2_502] {
        let actual = zone.tick(now);
        assert_eq!(restored.tick(now), actual);
        assert_eq!(
            restored.canonical_state_root().unwrap(),
            zone.canonical_state_root().unwrap()
        );
    }
    assert_eq!(child_ids(&zone).len(), 1);
    let child = child_ids(&zone)[0];
    assert_eq!(
        body_state(&zone, child)["special_ai"]["spider"]["parent_object_id"],
        ID
    );
    assert!(root_state(&zone)["pending_spawn"].is_null());
    assert_eq!(
        root_attack_count(&zone.tick(9_000), "first"),
        0,
        "a dead producer cannot queue new waves"
    );
}

#[test]
fn root_children_keep_independent_drop_rolls_and_reward_only_their_actual_deaths() {
    let mut zone = root_fixture(ZoneCollision::unbounded(), point(10, 14), point(14, 14));
    let mut child_object_ids = std::collections::BTreeSet::new();
    let mut drop_object_ids = std::collections::BTreeSet::new();
    let mut rolls = std::collections::BTreeSet::new();
    for birth in [2_001, 5_002] {
        zone.tick(birth);
        let pending = root_state(&zone)["pending_spawn"].clone();
        assert!(rolls.insert(pending["drop_roll"].as_u64().unwrap()));
        let spawn_out = zone.tick(birth + 500);
        assert!(!spawn_out
            .iter()
            .any(|out| matches!(out, ZoneOutbound::MonsterKillAward { .. })));
        let child = child_ids(&zone)[0];
        assert!(child_object_ids.insert(child));
        let child_body = body_state(&zone, child);
        let position: Point = serde_json::from_value(child_body["position"].clone()).unwrap();
        zone.handle(ZoneCommand::SyncPlayerTransform {
            session_id: SessionId::new("first"),
            position: point(position.x, position.y + 1),
            direction: MirDirection::Up,
        });
        zone.handle(ZoneCommand::PlayerAttackObject {
            session_id: SessionId::new("first"),
            object_id: child,
            direction: MirDirection::Up,
            spell: 0,
            level: 0,
            attack_type: 0,
            damage: 1,
            now_ms: birth + 501,
        });
        let killed = zone.tick(birth + 501);
        let awards: Vec<_> = killed
            .iter()
            .filter_map(|out| match out {
                ZoneOutbound::MonsterKillAward { award, .. }
                    if award.monster_object_id == child =>
                {
                    Some(award)
                }
                _ => None,
            })
            .collect();
        assert_eq!(awards.len(), 1);
        assert_eq!(
            awards[0].experience, 7,
            "use BombSpider's imported EXP, not the producer's"
        );
        for drop in &awards[0].drops {
            assert_ne!(drop.name, "ROOT-ONLY-LOOT");
            assert!(drop_object_ids.insert(drop.object_id));
            if let GroundDropLootSnapshot::InventoryItem { exact_item, .. } = &drop.loot {
                let exact = exact_item
                    .as_ref()
                    .expect("child drop carries exact metadata");
                assert!(!exact.uid_assigned);
                assert_eq!(exact.item.unique_id, 0);
            }
        }
        assert!(
            child_ids(&zone).is_empty(),
            "dead children stop counting against the cap"
        );
        assert!(!zone
            .tick(birth + 1_001)
            .iter()
            .any(|out| matches!(out, ZoneOutbound::MonsterKillAward { .. })));
    }
}

#[test]
fn root_twenty_live_child_cap_survives_repeated_ticks_and_replenishes_a_dead_child() {
    let mut free = std::collections::BTreeSet::new();
    let first = point(17, 10);
    let second = point(17, 12);
    for y in [4, 7, 10, 13, 16] {
        for x in [4, 7, 10, 13, 16] {
            if (x - first.x).abs().max((y - first.y).abs()) > 1
                && (x - second.x).abs().max((y - second.y).abs()) > 1
            {
                free.insert((x, y));
            }
        }
    }
    free.insert((first.x, first.y));
    free.insert((second.x, second.y));
    let blocked = (0..=31)
        .flat_map(|y| (0..=31).map(move |x| (x, y)))
        .filter(|cell| !free.contains(cell))
        .map(|(x, y)| point(x, y));
    let collision = ZoneCollision::unbounded()
        .with_bounds(ZoneBounds::new(0, 31, 0, 31))
        .with_blocked_cells(blocked);
    let mut zone = root_fixture(collision, first, second);
    for wave in 0..20 {
        let attack_at = 2_001 + 3_001 * wave;
        assert_eq!(root_attack_count(&zone.tick(attack_at), "first"), 1);
        zone.tick(attack_at + 500);
        assert_eq!(child_ids(&zone).len(), wave as usize + 1);
    }
    let next_attack = 2_001 + 3_001 * 20;
    assert_eq!(root_attack_count(&zone.tick(next_attack), "first"), 0);
    zone.tick(next_attack);
    assert_eq!(child_ids(&zone).len(), 20);
    assert!(root_state(&zone)["pending_spawn"].is_null());
    let target = child_ids(&zone)
        .into_iter()
        .min_by_key(|&child| {
            let body = body_state(&zone, child);
            let position: Point = serde_json::from_value(body["position"].clone()).unwrap();
            (position.x - 17).abs().max((position.y - 10).abs())
        })
        .unwrap();
    let position: Point =
        serde_json::from_value(body_state(&zone, target)["position"].clone()).unwrap();
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("first"),
        object_id: target,
        spell: Spell::FireBall,
        direction: MirDirection::UpLeft,
        target: position,
        cast: true,
        level: 3,
        damage: 100,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: next_attack + 1,
    });
    zone.tick(next_attack + 2_000);
    assert_eq!(body_state(&zone, target)["dead"], true);
    assert_eq!(child_ids(&zone).len(), 19);
    assert!(root_state(&zone)["pending_spawn"].is_object());
    zone.tick(next_attack + 2_500);
    assert_eq!(child_ids(&zone).len(), 20);
    assert!(!child_ids(&zone).contains(&target));
}

fn all_packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .collect()
}
fn owned_spider_target(ai: u8, source_id: u32) -> (ZoneRuntime, u32) {
    let stone = true;
    let mut z = ZoneRuntime::new(ZoneKey::for_map("shared-bomb-spider-fixture"));
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
        map_file_name: "shared-bomb-spider-fixture".into(),
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
    assert!(all_packets(&cast).iter().any(|p|matches!(p,ServerPacket::Magic{spell,cast:true,..}if *spell == if stone {Spell::Stonetrap} else {Spell::SummonShinsu})), "summon must be accepted before inspecting spawn");
    let mut out = cast;
    out.extend(z.tick(1210));
    let pet = all_packets(&out)
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
fn bomb_real_pet_only_dies_then_delayed_acagility_hits_pet_not_owner() {
    let (mut z, pet) = owned_spider_target(40, ID);
    let before = pet_hp(&z, pet);
    let out = z.tick(2000);
    assert!(all_packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==ID)));
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
fn root_spider_real_pet_target_is_captured_and_inherited_by_real_child() {
    let (mut z, pet) = owned_spider_target(39, ID);
    let out = z.tick(3211);
    assert_eq!(root_attack_count(&out, "owner"), 1);
    let pending = &root_state(&z)["pending_spawn"];
    assert_eq!(pending["target_object_id"], pet);
    assert_eq!(pending["target"]["Monster"]["object_id"], pet);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(3711), restored.tick(3711));
    let child = child_ids(&z).into_iter().next().unwrap();
    let body = body_state(&z, child);
    assert_eq!(body["special_ai"]["spider"]["target_object_id"], pet);
    assert_eq!(
        body["special_ai"]["spider"]["parent_ref"]["Monster"]["object_id"],
        ID
    );
    assert_eq!(
        body["master_object_id"], 0,
        "slave is not a fake owned monster"
    );
}
#[test]
fn root_pending_spawn_does_not_inherit_retired_pet_life() {
    let (mut z, pet) = owned_spider_target(39, ID);
    z.tick(3211);
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("owner"),
    });
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(3711), restored.tick(3711));
    let child = child_ids(&z).into_iter().next().unwrap();
    assert!(body_state(&z, child)["special_ai"]["spider"]["target_object_id"].is_null());
    assert!(z
        .native_monster_snapshots()
        .iter()
        .all(|m| m.object_id != pet || m.dead));
}
#[test]
fn bomb_positive_real_pet_hit_can_apply_sc_powered_green_poison() {
    let mut found = false;
    for id in 9980..10080 {
        let (mut z, pet) = owned_spider_target(40, id);
        z.tick(2000);
        let out = z.tick(2500);
        if !all_packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectPoisoned{object_id,poison}if *object_id==pet && poison&1!=0)){continue;}
        let before = pet_hp(&z, pet);
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        let out = z.tick(2501);
        assert_eq!(out, restored.tick(2501));
        assert_eq!(
            before - pet_hp(&z, pet),
            255,
            "Green uses source SC255 instead of a tint-only carrier"
        );
        found = true;
        break;
    }
    assert!(
        found,
        "fixtures exercise the source one-in-five positive-hit Green branch"
    );
}

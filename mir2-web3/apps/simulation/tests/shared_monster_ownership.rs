//! Ordinary same-map EXPOwner through public shared commands. Spawned HP,
//! loot, and fallback attack scalars are deterministic server-side fixtures,
//! not an authenticated network journey or human acceptance result.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, SessionId, WorldEntityDisposition, ZoneCollision,
    ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterDefense, ZoneMonsterKillAward, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::{json, Value};

const MONSTER: u32 = 90_101;
const MAP: &str = "same-map-experience-owner-fixture";
const EXP: u32 = 123;

fn sid(name: &str) -> SessionId {
    SessionId::new(name)
}
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn join(name: &str, id: u32, position: Point, class: MirClass) -> ZoneJoin {
    ZoneJoin {
        session_id: sid(name),
        account_id: format!("{name}-account"),
        character_index: 0,
        object_id: id,
        name: name.into(),
        class,
        gender: MirGender::Male,
        level: 40,
        hp: 100_000,
        max_hp: 100_000,
        mp: 100,
        map_file_name: MAP.into(),
        position,
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

fn admit(zone: &mut ZoneRuntime, name: &str, class: MirClass) {
    zone.handle(ZoneCommand::sync_player_combat_state(
        sid(name),
        class,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
}

fn empty_zone() -> ZoneRuntime {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map(MAP), ZoneCollision::unbounded());
    for (name, id, position) in [("a", 101, point(9, 10)), ("b", 102, point(10, 11))] {
        zone.handle(ZoneCommand::Join(join(
            name,
            id,
            position,
            MirClass::Warrior,
        )));
        admit(&mut zone, name, MirClass::Warrior);
    }
    zone
}

fn monster(hp: i32) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: MONSTER,
        name: "Skeleton".into(),
        name_colour_argb: -1,
        image: 3,
        ai: 3,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 20,
        hp,
        max_hp: hp,
        experience: EXP,
        move_speed_ms: 100_000,
        attack_speed_ms: 100_000,
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
            quantity: 7,
            source_monster: "Skeleton".into(),
            owner_object_id: None,
            ownership_remaining_ticks: Some(60),
            loot: GroundDropLootSnapshot::Gold { amount: 7 },
        }],
    }
}

fn spawn(zone: &mut ZoneRuntime, spawn: ZoneMonsterSpawn, now_ms: u64) {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: sid("a"),
        monster: spawn,
        now_ms,
    });
}

fn fixture(hp: i32) -> ZoneRuntime {
    let mut zone = empty_zone();
    spawn(&mut zone, monster(hp), 0);
    zone
}

fn packets<'a>(out: &'a [ZoneOutbound], recipient: &str) -> Vec<&'a ServerPacket> {
    let recipient = sid(recipient);
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&recipient) => packets.iter(),
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if session_id == &recipient => packets.iter(),
            ZoneOutbound::ToAll { packets } => packets.iter(),
            _ => [].iter(),
        })
        .collect()
}

fn melee(zone: &mut ZoneRuntime, name: &str, damage: i32, at: u64) -> Vec<ZoneOutbound> {
    let mut out = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: sid(name),
        object_id: MONSTER,
        direction: if name == "a" {
            MirDirection::Right
        } else {
            MirDirection::Up
        },
        spell: 0,
        level: 0,
        attack_type: 0,
        damage,
        now_ms: at,
    });
    out.extend(zone.tick(at));
    out
}

fn cast(
    zone: &mut ZoneRuntime,
    name: &str,
    spell: Spell,
    damage: i32,
    at: u64,
) -> Vec<ZoneOutbound> {
    let mut out = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: sid(name),
        object_id: MONSTER,
        spell,
        direction: MirDirection::Right,
        target: point(10, 10),
        cast: true,
        level: 2,
        damage,
        mp_cost: 0,
        cooldown_ms: 0,
        now_ms: at,
    });
    out.extend(zone.tick(at));
    out
}

fn damage(out: &[ZoneOutbound]) -> i32 {
    packets(out, "b")
        .into_iter()
        .filter_map(|p| match p {
            ServerPacket::DamageIndicator {
                object_id, damage, ..
            } if *object_id == MONSTER => Some(*damage),
            _ => None,
        })
        .sum()
}

fn award<'a>(
    out: &'a [ZoneOutbound],
    expected: &str,
    expected_owner: u32,
) -> &'a ZoneMonsterKillAward {
    let awards: Vec<_> = out
        .iter()
        .filter_map(|o| match o {
            ZoneOutbound::MonsterKillAward { session_id, award } => Some((session_id, award)),
            _ => None,
        })
        .collect();
    assert_eq!(
        awards.len(),
        1,
        "exactly one EXP/quest-credit award: {out:?}"
    );
    assert_eq!(
        awards[0].0,
        &sid(expected),
        "EXPOwner must receive the kill, not the final actor"
    );
    let award = awards[0].1;
    assert_eq!(award.monster_object_id, MONSTER);
    assert_eq!(
        award.monster_name, "Skeleton",
        "quest credit preserves the actual target"
    );
    assert_eq!(award.experience, EXP);
    assert_eq!(award.drops.len(), 1);
    assert_eq!(award.drops[0].owner_object_id, Some(expected_owner));
    assert_eq!(
        award.drops[0].loot,
        GroundDropLootSnapshot::Gold { amount: 7 }
    );
    assert_ne!(
        award.drops[0].object_id, 0,
        "actual shared ground identity is allocated"
    );
    assert!(packets(out, "b").iter().any(|p| matches!(p,
        ServerPacket::ObjectDied { info } if info.object_id == MONSTER)));
    award
}

fn checkpoint_monster(zone: &ZoneRuntime) -> Value {
    let value: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    value["native_monsters"][MONSTER.to_string()].clone()
}

fn taoist_poison(zone: &mut ZoneRuntime, at: u64) -> Vec<ZoneOutbound> {
    let mut out = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: sid("a"),
        object_id: MONSTER,
        spell: Spell::Poisoning,
        direction: MirDirection::Right,
        target: point(10, 10),
        cast: true,
        level: 2,
        damage: 5,
        mp_cost: 0,
        cooldown_ms: 0,
        item_param: 1,
        now_ms: at,
    });
    out.extend(zone.tick(at));
    out
}

fn taoist_fixture(hp: i32) -> ZoneRuntime {
    let mut zone = empty_zone();
    zone.handle(ZoneCommand::Leave {
        session_id: sid("a"),
    });
    zone.handle(ZoneCommand::Join(join(
        "a",
        101,
        point(9, 10),
        MirClass::Taoist,
    )));
    admit(&mut zone, "a", MirClass::Taoist);
    spawn(&mut zone, monster(hp), 0);
    zone
}

#[test]
fn first_effective_player_hit_retains_exp_quest_and_drop_credit_on_rival_kill() {
    let mut zone = fixture(100);
    assert_eq!(damage(&melee(&mut zone, "a", 20, 200)), 20);
    let killed = melee(&mut zone, "b", 80, 800);
    assert_eq!(damage(&killed), 80);
    award(&killed, "a", 101);
    assert_eq!(checkpoint_monster(&zone)["hp"], 0);
    assert!(!zone
        .tick(801)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
}

#[test]
fn exact_five_second_deadline_keeps_original_owner() {
    let mut zone = fixture(100);
    melee(&mut zone, "a", 20, 200);
    let killed = melee(&mut zone, "b", 80, 5_200);
    assert_eq!(damage(&killed), 80);
    award(&killed, "a", 101);
}

#[test]
fn strictly_after_five_seconds_next_effective_attacker_claims_owner() {
    let mut zone = fixture(100);
    melee(&mut zone, "a", 20, 200);
    let killed = melee(&mut zone, "b", 80, 5_201);
    award(&killed, "b", 102);
}

#[test]
fn same_owner_effective_hit_renews_five_seconds() {
    let mut zone = fixture(100);
    melee(&mut zone, "a", 10, 200);
    assert_eq!(damage(&melee(&mut zone, "a", 10, 4_800)), 10);
    let killed = melee(&mut zone, "b", 80, 5_500);
    award(&killed, "a", 101);
}

#[test]
fn competing_damage_neither_claims_nor_renews_current_owner() {
    let mut zone = fixture(100);
    zone.handle(ZoneCommand::Join(join(
        "c",
        103,
        point(11, 10),
        MirClass::Warrior,
    )));
    admit(&mut zone, "c", MirClass::Warrior);
    melee(&mut zone, "a", 10, 200);
    melee(&mut zone, "b", 10, 4_800);
    let killed = melee(&mut zone, "c", 80, 5_201);
    award(&killed, "c", 103);
}

#[test]
fn armour_rejected_direct_hit_never_claims_or_renews() {
    let mut zone = empty_zone();
    let mut target = monster(100);
    target.defense = ZoneMonsterDefense {
        min_ac: 50,
        max_ac: 50,
        ..Default::default()
    };
    spawn(&mut zone, target, 0);
    for (name, dc) in [("a", 40), ("b", 150)] {
        zone.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: sid(name),
            stats: ZonePlayerCombatStats {
                min_dc: dc,
                max_dc: dc,
                accuracy: 100,
                ..Default::default()
            },
        });
    }
    assert_eq!(damage(&melee(&mut zone, "a", 40, 200)), 0);
    assert_eq!(checkpoint_monster(&zone)["hp"], 100);
    let killed = melee(&mut zone, "b", 150, 800);
    assert_eq!(damage(&killed), 100);
    award(&killed, "b", 102);
}

#[test]
fn only_resolved_damage_claims_before_a_delayed_swing() {
    let mut zone = fixture(100);
    let launch = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: sid("a"),
        object_id: MONSTER,
        direction: MirDirection::Right,
        spell: Spell::TwinDrakeBlade as u8,
        level: 0,
        attack_type: 0,
        damage: 100,
        now_ms: 200,
    });
    assert_eq!(damage(&launch), 0);
    let killed = melee(&mut zone, "b", 100, 400);
    award(&killed, "b", 102);
    assert!(!zone
        .tick(500)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
}

#[test]
fn rival_periodic_poison_does_not_steal_living_melee_owner() {
    let mut zone = taoist_fixture(23);
    melee(&mut zone, "b", 20, 100);
    assert_eq!(
        damage(&taoist_poison(&mut zone, 200)),
        0,
        "casting poison alone does not claim EXP"
    );
    let killed = zone.tick(2_200);
    assert_eq!(damage(&killed), 3);
    award(&killed, "b", 102);
}

#[test]
fn solo_dead_poison_owner_keeps_source_kill_credit() {
    let mut zone = taoist_fixture(3);
    taoist_poison(&mut zone, 200);
    zone.handle(ZoneCommand::SyncPlayerVitals {
        session_id: sid("a"),
        hp: 0,
        max_hp: 100_000,
        mp: 100,
    });
    let killed = zone.tick(2_200);
    assert_eq!(damage(&killed), 3);
    assert_eq!(zone.player_vitals(&sid("a")).unwrap().0, 0);
    award(&killed, "a", 101);
}

#[test]
fn new_effective_attacker_replaces_dead_exp_owner() {
    let mut zone = fixture(100);
    melee(&mut zone, "a", 20, 200);
    zone.handle(ZoneCommand::SyncPlayerVitals {
        session_id: sid("a"),
        hp: 0,
        max_hp: 100_000,
        mp: 100,
    });
    let killed = melee(&mut zone, "b", 80, 800);
    award(&killed, "b", 102);
}

#[test]
fn successful_turn_undead_forces_caster_ownership() {
    let mut zone = fixture(100);
    melee(&mut zone, "a", 20, 100);
    zone.handle(ZoneCommand::Leave {
        session_id: sid("b"),
    });
    zone.handle(ZoneCommand::Join(join(
        "b",
        102,
        point(10, 11),
        MirClass::Wizard,
    )));
    admit(&mut zone, "b", MirClass::Wizard);
    let killed = cast(&mut zone, "b", Spell::TurnUndead, 1, 200);
    assert_eq!(damage(&killed), 80);
    award(&killed, "b", 102);
}

#[test]
fn authenticated_checkpoint_retains_owner_without_extending_deadline() {
    let mut zone = fixture(100);
    melee(&mut zone, "a", 20, 200);
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    assert_eq!(
        restored.canonical_state_root().unwrap(),
        zone.canonical_state_root().unwrap()
    );
    award(&melee(&mut restored, "b", 80, 5_200), "a", 101);
    let mut expired = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    award(&melee(&mut expired, "b", 80, 5_201), "b", 102);
}

#[test]
fn checkpoint_exp_owner_tampering_cannot_bypass_root_authentication() {
    let mut zone = fixture(100);
    melee(&mut zone, "a", 20, 200);
    let mut tampered: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    tampered["native_monsters"][MONSTER.to_string()]["experience_owner"] = json!({
        "session_id": "b", "account_id": "b-account", "character_index": 0,
        "object_id": 102, "expires_at_ms": 99_999,
    });
    assert!(
        ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&tampered).unwrap()).is_err(),
        "typed current-schema EXPOwner must be covered by canonical root"
    );
}

#[test]
fn fresh_same_map_join_revokes_old_claim_and_old_poison_identity() {
    let mut zone = taoist_fixture(100);
    melee(&mut zone, "a", 20, 100);
    taoist_poison(&mut zone, 700);
    zone.handle(ZoneCommand::Leave {
        session_id: sid("a"),
    });
    let mut replacement = join("a", 101, point(9, 10), MirClass::Taoist);
    replacement.account_id = "different-account".into();
    replacement.character_index = 7;
    zone.handle(ZoneCommand::Join(replacement));
    admit(&mut zone, "a", MirClass::Taoist);
    let state = checkpoint_monster(&zone);
    assert!(
        state.get("experience_owner").is_none(),
        "fresh Join cannot inherit a former spawned owner"
    );
    assert_eq!(state["damage_poison_owner_object_id"], 0);
    assert_eq!(state["damage_poison_value"], 0);
    let killed = melee(&mut zone, "b", 80, 800);
    award(&killed, "b", 102);
    assert!(!zone
        .tick(2_700)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
}

#[test]
fn fresh_monster_incarnation_does_not_inherit_previous_life_owner() {
    let mut zone = fixture(100);
    award(&melee(&mut zone, "a", 100, 200), "a", 101);
    spawn(&mut zone, monster(100), 300);
    let state = checkpoint_monster(&zone);
    assert_eq!(state["hp"], 100);
    assert!(state.get("experience_owner").is_none());
    award(&melee(&mut zone, "b", 100, 400), "b", 102);
}

#[test]
fn pending_turn_undead_force_owner_flag_is_authenticated() {
    let mut zone = fixture(100);
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: sid("a"),
        object_id: MONSTER,
        direction: MirDirection::Right,
        spell: Spell::TwinDrakeBlade as u8,
        level: 0,
        attack_type: 0,
        damage: 20,
        now_ms: 200,
    });
    let mut state: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert!(!state["pending_native_hits"].as_array().unwrap().is_empty());
    state["pending_native_hits"][0]["force_experience_owner"] = json!(true);
    assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&state).unwrap()).is_err());
}

fn summoned_fixture(hp: i32) -> (ZoneRuntime, u32) {
    let mut zone = empty_zone();
    zone.handle(ZoneCommand::Leave {
        session_id: sid("a"),
    });
    zone.handle(ZoneCommand::Join(join(
        "a",
        101,
        point(8, 10),
        MirClass::Taoist,
    )));
    admit(&mut zone, "a", MirClass::Taoist);
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: sid("a"),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: point(9, 10),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 0,
        now_ms: 100,
    });
    let spawned = zone.tick(600);
    let pet = packets(&spawned, "a")
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info }
                if info.master_object_id == 101 && info.name == "BoneFamiliar" =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("real delayed owned summon must spawn");
    spawn(&mut zone, monster(hp), 700);
    (zone, pet)
}

#[test]
fn owned_summon_first_hit_keeps_master_credit_and_actual_pet_attacker_id() {
    let (mut zone, pet) = summoned_fixture(40);
    zone.tick(1_200);
    let hit = zone.tick(1_800);
    assert!(damage(&hit) > 0);
    assert!(packets(&hit, "b").iter().any(|p| matches!(p,
        ServerPacket::ObjectStruck { info } if info.object_id == MONSTER && info.attacker_id == pet)));
    let killed = melee(&mut zone, "b", 100, 2_000);
    award(&killed, "a", 101);
}

#[test]
fn pet_with_master_sixteen_tiles_away_cannot_steal_rival_claim() {
    let (mut zone, pet) = summoned_fixture(21);
    melee(&mut zone, "b", 20, 800);
    zone.tick(1_200);
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: sid("a"),
        position: point(-7, 10),
        direction: MirDirection::Right,
    });
    let killed = zone.tick(1_800);
    assert_eq!(damage(&killed), 1);
    assert!(packets(&killed, "b").iter().any(|p| matches!(p,
        ServerPacket::ObjectStruck { info } if info.object_id == MONSTER && info.attacker_id == pet)));
    award(&killed, "b", 102);
}

#[test]
fn pet_outside_sixteen_tile_master_range_clears_credit_without_losing_damage() {
    let (mut zone, pet) = summoned_fixture(21);
    melee(&mut zone, "b", 20, 800);
    zone.tick(1_200);
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: sid("a"),
        position: point(-8, 10),
        direction: MirDirection::Right,
    });
    let killed = zone.tick(1_800);
    assert_eq!(damage(&killed), 1);
    assert!(packets(&killed, "b").iter().any(|p| matches!(p,
        ServerPacket::ObjectStruck { info } if info.object_id == MONSTER && info.attacker_id == pet)));
    assert!(
        !killed
            .iter()
            .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })),
        "out-of-range pet clears EXPOwner rather than falling back to its master"
    );
    assert!(checkpoint_monster(&zone)["dead"].as_bool().unwrap());
    let cp: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert!(
        cp["ground_drops"].as_object().unwrap().is_empty(),
        "no owner means no loot"
    );
}

#[test]
fn map_quake_lethal_damage_preserves_an_existing_player_claim() {
    let mut zone = empty_zone();
    for (name, position) in [("a", point(40, 41)), ("b", point(42, 42))] {
        zone.handle(ZoneCommand::SyncPlayerTransform {
            session_id: sid(name),
            position,
            direction: MirDirection::Right,
        });
    }
    let mut lord = monster(100_000);
    lord.object_id = 90_198;
    lord.name = "Guard".into(); // Explicit nonzero DC/MC=255, not balance evidence.
    lord.ai = 98;
    lord.image = 902;
    lord.position = point(40, 40);
    lord.drops.clear();
    assert!(zone.spawn_world_event_monster(&lord, 0).0);
    zone.tick(1);
    let state: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    let quake = state["hell_world"]["quakes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| {
            q["value"].as_i64().unwrap() > 0
                && q["start"].as_u64().unwrap() > 200
                && q["position"] != json!(point(40, 40))
                && q["position"] != json!(point(40, 41))
        })
        .expect("real AI98 generates a delayed, nonzero environmental quake");
    let position: Point = serde_json::from_value(quake["position"].clone()).unwrap();
    let start = quake["start"].as_u64().unwrap();
    assert!(
        start <= 5_200,
        "the claim remains within its five-second window"
    );
    zone.despawn_world_event_monster(lord.object_id, 2);
    for (name, position) in [
        ("a", point(position.x - 1, position.y)),
        ("b", point(position.x, position.y + 1)),
    ] {
        zone.handle(ZoneCommand::SyncPlayerTransform {
            session_id: sid(name),
            position,
            direction: MirDirection::Right,
        });
    }
    let mut victim = monster(21);
    victim.position = position;
    spawn(&mut zone, victim, 3);
    assert_eq!(damage(&melee(&mut zone, "a", 20, 200)), 20);
    let killed = zone.tick(start);
    assert_eq!(damage(&killed), 1);
    assert!(
        packets(&killed, "b").iter().any(|p| matches!(p,
        ServerPacket::ObjectStruck { info } if info.object_id == MONSTER && info.attacker_id == 0))
    );
    award(&killed, "a", 101);
    assert!(!zone
        .tick(start + 500)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
}

#[test]
fn wild_lethal_hit_on_real_master_pet_never_becomes_a_wild_kill_reward() {
    let (mut zone, pet) = summoned_fixture(21);
    zone.despawn_world_event_monster(MONSTER, 701);
    let mut source = monster(100_000);
    source.object_id = 90_199;
    source.name = "ArcherGuard".into(); // Fixed nonzero DC255 kills the real pet.
    source.image = 139;
    source.ai = 0;
    source.move_speed_ms = 300;
    source.attack_speed_ms = 2_000;
    assert!(zone.spawn_world_event_monster(&source, 702).0);
    let launch = zone.tick(800);
    assert!(packets(&launch, "b").iter().any(|p| matches!(p,
        ServerPacket::ObjectAttack { info } if info.object_id == source.object_id)));
    let killed = zone.tick(1_100);
    assert!(packets(&killed, "b").iter().any(|p| matches!(p,
        ServerPacket::ObjectDied { info } if info.object_id == pet)));
    assert!(!killed
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
    let state: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert!(state["ground_drops"].as_object().unwrap().is_empty());
    assert_eq!(zone.player_vitals(&sid("a")).unwrap().0, 100_000);
}

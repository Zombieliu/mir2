//! Five-class regressions exercise the authoritative Zone command boundary.
use super::*;
use crate::config::WorldEntityDisposition;
use crate::runtime::zone::types::{ZoneMonsterDefense, ZonePlayerCombatStats};
use mir2_protocol::MirGender;

const TARGET: u32 = 9_001;
const TARGET_HP: i32 = 5_000;

fn joined(class: MirClass, mp: i32) -> (ZoneRuntime, SessionId) {
    let owner = SessionId::new("five-class-owner");
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    zone.set_journey_evidence_enabled_for_test(true);
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: owner.clone(),
        account_id: "five-class-account".into(),
        character_index: 7,
        object_id: 101,
        name: "FiveClassOwner".into(),
        class,
        gender: MirGender::Male,
        level: 50,
        hp: 100,
        max_hp: 100,
        mp,
        map_file_name: "0".into(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 100,
            max_dc: 100,
            min_mc: 50,
            max_mc: 50,
            accuracy: 100,
            luck: 100,
            attack_speed: 8, // A geared source character reaches the 550ms cap.
            ..Default::default()
        },
    }));
    zone.handle(ZoneCommand::sync_player_combat_state(
        owner.clone(),
        class,
        class == MirClass::Archer,
        false,
        true,
        false,
        false,
        false,
    ));
    (zone, owner)
}

fn spawn(zone: &mut ZoneRuntime, owner: &SessionId, distance: i32, ac: i32, mac: i32) {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: TARGET,
            name: "Scarecrow".into(),
            name_colour_argb: -1,
            image: 5,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 1,
            max_hp: TARGET_HP,
            hp: TARGET_HP,
            experience: 1,
            move_speed_ms: 60_000,
            attack_speed_ms: 60_000,
            friendly_guild: None,
            position: Point {
                x: 10 + distance,
                y: 10,
            },
            direction: MirDirection::Left,
            defense: ZoneMonsterDefense {
                min_ac: ac,
                max_ac: ac,
                min_mac: mac,
                max_mac: mac,
                ..Default::default()
            },
            respawn: None,
            drops: Vec::new(),
        },
    });
}

fn melee(owner: &SessionId, spell: Spell, level: u8, now_ms: u64) -> ZoneCommand {
    ZoneCommand::PlayerAttackObject {
        session_id: owner.clone(),
        object_id: TARGET,
        direction: MirDirection::Right,
        spell: spell as u8,
        level,
        attack_type: 0,
        damage: 99_999,
        now_ms,
    }
}

fn arrow(owner: &SessionId, spell: Spell, distance: i32, now_ms: u64) -> ZoneCommand {
    ZoneCommand::PlayerRangeAttackObject {
        session_id: owner.clone(),
        object_id: TARGET,
        direction: MirDirection::Right,
        target: Point {
            x: 10 + distance,
            y: 10,
        },
        spell,
        level: 3,
        attack_type: 0,
        damage: 99_999,
        now_ms,
    }
}

fn cast(owner: &SessionId, spell: Spell, distance: i32, now_ms: u64) -> ZoneCommand {
    let magic = crystal_magic_by_spell(&format!("{spell:?}")).unwrap();
    ZoneCommand::PlayerCastMagic {
        session_id: owner.clone(),
        object_id: TARGET,
        spell,
        direction: MirDirection::Right,
        target: Point {
            x: 10 + distance,
            y: 10,
        },
        cast: true,
        level: 3,
        damage: 99_999,
        mp_cost: i32::from(magic.base_cost) + 3 * i32::from(magic.level_cost),
        cooldown_ms: u64::from(magic.delay_base),
        now_ms,
    }
}

fn packets(out: &[ZoneOutbound]) -> impl Iterator<Item = &ServerPacket> {
    out.iter().flat_map(|outbound| match outbound {
        ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. }
        | ZoneOutbound::ToAll { packets } => packets.as_slice(),
        _ => &[],
    })
}

fn journey(out: &[ZoneOutbound]) -> Vec<&ZoneJourneyEventReceipt> {
    out.iter()
        .filter_map(|outbound| match outbound {
            ZoneOutbound::JourneyEvent { receipt } => Some(receipt),
            _ => None,
        })
        .collect()
}

fn magic_practice(out: &[ZoneOutbound]) -> Vec<&ZoneMagicPracticeReceipt> {
    out.iter()
        .filter_map(|outbound| match outbound {
            ZoneOutbound::MagicPractice { receipt } => Some(receipt),
            _ => None,
        })
        .collect()
}

#[test]
fn fiveclass_double_slash_pays_once_per_swing_and_hits_mac_then_no_armour_at_400ms() {
    let (mut zone, owner) = joined(MirClass::Assassin, 30);
    spawn(&mut zone, &owner, 1, 80, 10);
    let launch = zone.handle(melee(&owner, Spell::DoubleSlash, 3, 20));
    assert!(
        packets(&launch).any(|p| matches!(p, ServerPacket::ObjectAttack { info }
        if info.spell == Spell::DoubleSlash as u8))
    );
    assert_eq!(zone.players[&owner].mp, 25, "source cost is 2 + level 3");
    assert!(journey(&launch).is_empty(), "launch is not damage evidence");
    zone.tick(319);
    assert_eq!(zone.native_monsters[&TARGET].hp, TARGET_HP);
    let first = zone.tick(320);
    assert_eq!(
        zone.native_monsters[&TARGET].hp,
        TARGET_HP - 100,
        "level-three 1.1 * 100 subtracts MAC 10, not AC 80"
    );
    assert_eq!(journey(&first).len(), 1);
    assert_eq!(
        journey(&first)[0].kind,
        ZoneJourneyEventKind::PhysicalDamage {
            technique: ZoneJourneyPhysicalTechnique::DoubleSlash
        }
    );
    assert!(journey(&zone.tick(419)).is_empty());
    let second = zone.tick(420);
    assert_eq!(zone.native_monsters[&TARGET].hp, TARGET_HP - 210);
    assert_eq!(journey(&second).len(), 1);
    assert_eq!(journey(&second)[0].damage, Some(110));
    assert_eq!(journey(&second)[0].source_action_at_ms, 20);
    assert_eq!(journey(&second)[0].committed_at_ms, 420);
    assert_eq!(zone.players[&owner].mp, 25);
    // DoubleSlash remains a prepared toggle in the personal session. The Zone
    // must support the next accepted swing rather than consume it as Slaying.
    zone.handle(melee(&owner, Spell::DoubleSlash, 3, 620));
    assert_eq!(zone.players[&owner].mp, 20);
}

#[test]
fn fiveclass_double_slash_rejections_do_not_spend_mana_or_start_action_clocks() {
    for (mp, distance) in [(4, 1), (30, 2)] {
        let (mut zone, owner) = joined(MirClass::Assassin, mp);
        spawn(&mut zone, &owner, distance, 0, 0);
        let before = zone.players[&owner].next_attack_ready_at_ms;
        let out = zone.handle(melee(&owner, Spell::DoubleSlash, 3, 20));
        assert!(!packets(&out).any(|p| matches!(p, ServerPacket::ObjectAttack { .. })));
        assert_eq!(zone.players[&owner].mp, mp);
        assert_eq!(zone.players[&owner].next_attack_ready_at_ms, before);
        assert_eq!(zone.five_class_pvp.pending_count_for_test(), 0);
        assert!(
            zone.pending_native_hits.is_empty(),
            "new-profession attacks never enter the old queue"
        );
    }
    let (mut zone, owner) = joined(MirClass::Assassin, 30);
    spawn(&mut zone, &owner, 1, 0, 0);
    zone.handle(melee(&owner, Spell::DoubleSlash, 3, 20));
    let ready = zone.players[&owner].next_attack_ready_at_ms;
    let pending = zone.five_class_pvp.pending_count_for_test();
    let rejected = zone.handle(melee(&owner, Spell::DoubleSlash, 3, ready - 1));
    assert!(!packets(&rejected).any(|p| matches!(p, ServerPacket::ObjectAttack { .. })));
    assert_eq!(zone.players[&owner].mp, 25);
    assert_eq!(zone.five_class_pvp.pending_count_for_test(), pending);
    assert_eq!(zone.players[&owner].next_attack_ready_at_ms, ready);
}

#[test]
fn fiveclass_double_slash_second_strike_can_hit_when_first_strike_misses() {
    let mut saw_independent_second_hit = false;
    for now in 20..148 {
        let (mut zone, owner) = joined(MirClass::Assassin, 30);
        spawn(&mut zone, &owner, 1, 0, 0);
        zone.players.get_mut(&owner).unwrap().combat_stats.accuracy = 0;
        zone.native_monsters
            .get_mut(&TARGET)
            .unwrap()
            .defense
            .agility = 8;
        zone.handle(melee(&owner, Spell::DoubleSlash, 3, now));
        zone.tick(now + 300);
        if zone.native_monsters[&TARGET].hp != TARGET_HP {
            continue;
        }
        zone.tick(now + 400);
        if zone.native_monsters[&TARGET].hp < TARGET_HP {
            saw_independent_second_hit = true;
            break;
        }
    }
    assert!(
        saw_independent_second_hit,
        "Crystal queues strike two before strike one's Agility result; it rolls independently"
    );
}

#[test]
fn fiveclass_fatal_sword_is_learned_passive_bypasses_armour_and_consumes_only_a_hit() {
    let (mut zone, owner) = joined(MirClass::Assassin, 30);
    spawn(&mut zone, &owner, 1, 1_000, 1_000);
    assert!(!zone.players[&owner].fatal_sword_armed);
    assert_eq!(zone.players[&owner].combat_stats.fatal_sword_level, None);
    zone.players
        .get_mut(&owner)
        .unwrap()
        .combat_stats
        .fatal_sword_level = Some(0);
    let now = (20..2_000)
        .find(|now| crate::runtime::combat::crystal_accuracy_roll(*now, 101, TARGET ^ 91, 10) == 0)
        .expect("source ten-way proc roll");
    // Force a genuine agility miss at the actual 300ms impact. The passive
    // stays armed until a later positive strike; an animation alone cannot XP.
    zone.players.get_mut(&owner).unwrap().combat_stats.accuracy = 0;
    let agility = (1..1_000)
        .find(|agility| {
            crate::runtime::combat::crystal_accuracy_roll(
                now + 300,
                101,
                TARGET,
                (*agility + 1) as u64,
            ) > 0
        })
        .unwrap();
    zone.native_monsters
        .get_mut(&TARGET)
        .unwrap()
        .defense
        .agility = agility;
    zone.handle(melee(&owner, Spell::None, 0, now));
    assert!(zone.players[&owner].fatal_sword_armed);
    assert!(magic_practice(&zone.tick(now + 300)).is_empty());
    assert_eq!(zone.native_monsters[&TARGET].hp, TARGET_HP);
    assert!(zone.players[&owner].fatal_sword_armed);
    zone.players.get_mut(&owner).unwrap().combat_stats.accuracy = 2_000;
    zone.handle(melee(&owner, Spell::None, 0, now + 600));
    let landed = zone.tick(now + 900);
    assert_eq!(
        zone.native_monsters[&TARGET].hp,
        TARGET_HP - 100,
        "source normal damageFinal precedes passive damageBase change; the hit bypasses armour"
    );
    assert!(!zone.players[&owner].fatal_sword_armed);
    assert_eq!(magic_practice(&landed).len(), 1);
    assert_eq!(
        magic_practice(&landed)[0].spell,
        ZoneMagicPracticeSpell::FatalSword
    );
    assert_eq!(magic_practice(&landed)[0].damage, 100);
    assert_eq!(journey(&landed).len(), 1);
    assert!(
        packets(&landed).any(|p| matches!(p, ServerPacket::ObjectEffect { info }
        if info.object_id == TARGET && info.effect == 1))
    );
    assert_eq!(zone.players[&owner].mp, 30, "passive is free");
}

#[test]
fn fiveclass_unlearned_fatal_sword_cannot_proc_or_award_practice() {
    let (mut zone, owner) = joined(MirClass::Assassin, 30);
    spawn(&mut zone, &owner, 1, 1_000, 0);
    let now = (20..2_000)
        .find(|now| crate::runtime::combat::crystal_accuracy_roll(*now, 101, TARGET ^ 91, 10) == 0)
        .unwrap();
    zone.handle(melee(&owner, Spell::None, 0, now));
    let out = zone.tick(now + 300);
    assert_eq!(zone.native_monsters[&TARGET].hp, TARGET_HP);
    assert!(!zone.players[&owner].fatal_sword_armed);
    assert!(magic_practice(&out).is_empty());
    assert!(
        journey(&out).is_empty(),
        "armour-blocked ordinary hit is not quest evidence"
    );
}

#[test]
fn fiveclass_ordinary_focus_arrow_keeps_dc_damage_ac_and_distance_delay() {
    for spell in [Spell::None, Spell::Focus] {
        let (mut zone, owner) = joined(MirClass::Archer, 30);
        spawn(&mut zone, &owner, 1, 10, 40);
        let now = (20..2_000)
            .find(|now| {
                five_class_combat::ordinary_arrow_hit(
                    &zone.players[&owner],
                    &zone.native_monsters[&TARGET],
                    TARGET,
                    spell == Spell::Focus,
                    *now,
                )
            })
            .unwrap();
        let launch = zone.handle(arrow(&owner, spell, 1, now));
        assert!(
            packets(&launch).any(|p| matches!(p, ServerPacket::ObjectRangeAttack { info }
            if info.spell == spell as u8))
        );
        assert!(journey(&launch).is_empty());
        assert_eq!(zone.players[&owner].mp, 30);
        zone.tick(now + 599);
        assert_eq!(zone.native_monsters[&TARGET].hp, TARGET_HP);
        let landed = zone.tick(now + 600); // 550 + 50 * one tile.
        assert_eq!(
            zone.native_monsters[&TARGET].hp,
            TARGET_HP - 90,
            "Focus doubles hit chance, not DC damage; high MAC does not affect ordinary arrows"
        );
        assert_eq!(journey(&landed).len(), 1);
        assert_eq!(journey(&landed)[0].kind, ZoneJourneyEventKind::RangedDamage);
        assert_eq!(journey(&landed)[0].source_action_at_ms, now);
        assert_eq!(journey(&landed)[0].committed_at_ms, now + 600);
        assert!(
            magic_practice(&landed).is_empty(),
            "Focus XP follows accepted action, not hit receipt"
        );
    }
}

#[test]
fn fiveclass_archer_range_requires_real_bow_class_and_rejects_stale_coordinates() {
    for class in [
        MirClass::Warrior,
        MirClass::Wizard,
        MirClass::Taoist,
        MirClass::Assassin,
        MirClass::Archer,
    ] {
        let (mut zone, owner) = joined(class, 30);
        spawn(&mut zone, &owner, 1, 0, 0);
        if class == MirClass::Archer {
            zone.handle(ZoneCommand::sync_player_combat_state(
                owner.clone(),
                class,
                false,
                false,
                true,
                false,
                false,
                false,
            ));
        }
        let out = zone.handle(arrow(&owner, Spell::Focus, 1, 20));
        assert!(!packets(&out).any(|p| matches!(p, ServerPacket::ObjectRangeAttack { .. })));
        assert_eq!(zone.five_class_pvp.pending_count_for_test(), 0);
        assert!(
            zone.pending_native_hits.is_empty(),
            "new-profession attacks never enter the old queue"
        );
        assert_eq!(zone.players[&owner].mp, 30);
    }
    let (mut zone, owner) = joined(MirClass::Archer, 30);
    spawn(&mut zone, &owner, 1, 0, 0);
    let out = zone.handle(arrow(&owner, Spell::None, 2, 20));
    assert!(!packets(&out).any(|p| matches!(p, ServerPacket::ObjectRangeAttack { .. })));
    assert_eq!(zone.players[&owner].next_attack_ready_at_ms, 0);
}

#[test]
fn fiveclass_straight_and_double_shot_use_mc_mac_and_two_distinct_impact_receipts() {
    for (spell, damage, hit_count) in [(Spell::StraightShot, 56, 1), (Spell::DoubleShot, 53, 2)] {
        let (mut zone, owner) = joined(MirClass::Archer, 30);
        spawn(&mut zone, &owner, 9, 80, 5);
        let launch = zone.handle(cast(&owner, spell, 9, 20));
        assert!(packets(&launch).any(
            |p| matches!(p, ServerPacket::Magic { spell: s, cast: true, .. }
            if *s == spell)
        ));
        assert_eq!(
            zone.players[&owner].mp, 21,
            "3 + 2 * level three, once per cast"
        );
        assert!(journey(&launch).is_empty());
        assert!(magic_practice(&launch).is_empty());
        zone.tick(969);
        assert_eq!(zone.native_monsters[&TARGET].hp, TARGET_HP);
        let first = zone.tick(970); // 20 + 500 + 9 * 50.
        assert_eq!(zone.native_monsters[&TARGET].hp, TARGET_HP - damage);
        assert_eq!(journey(&first).len(), 1);
        assert_eq!(magic_practice(&first).len(), 1);
        assert_eq!(magic_practice(&first)[0].spell.spell(), spell);
        assert_eq!(magic_practice(&first)[0].cast_at_ms, 20);
        assert_eq!(magic_practice(&first)[0].damage, damage);
        let early = zone.tick(1019);
        assert!(journey(&early).is_empty());
        let second = zone.tick(1020);
        assert_eq!(
            zone.native_monsters[&TARGET].hp,
            TARGET_HP - damage * hit_count
        );
        assert_eq!(journey(&second).len(), (hit_count - 1) as usize);
        if spell == Spell::DoubleShot {
            assert_eq!(magic_practice(&second).len(), 1);
            assert_eq!(
                magic_practice(&second)[0].spell,
                ZoneMagicPracticeSpell::DoubleShotSecond
            );
            assert_eq!(
                journey(&second)[0].kind,
                ZoneJourneyEventKind::DoubleShotDamage
            );
        }
        assert_eq!(zone.players[&owner].mp, 21);
    }
}

#[test]
fn fiveclass_shot_cooldown_insufficient_mana_and_range_failures_do_not_charge_again() {
    for spell in [Spell::StraightShot, Spell::DoubleShot] {
        for (mp, distance) in [(8, 9), (30, 11)] {
            let (mut zone, owner) = joined(MirClass::Archer, mp);
            spawn(&mut zone, &owner, distance, 0, 0);
            let out = zone.handle(cast(&owner, spell, distance, 20));
            assert!(!packets(&out).any(|p| matches!(p, ServerPacket::Magic { cast: true, .. })));
            assert_eq!(zone.players[&owner].mp, mp);
            assert!(zone.players[&owner].magic_ready_at_ms.is_empty());
            assert_eq!(zone.five_class_pvp.pending_count_for_test(), 0);
            assert!(
                zone.pending_native_hits.is_empty(),
                "new-profession attacks never enter the old queue"
            );
        }
        let (mut zone, owner) = joined(MirClass::Archer, 30);
        spawn(&mut zone, &owner, 9, 0, 0);
        zone.handle(cast(&owner, spell, 9, 20));
        let before_ready = zone.players[&owner].magic_ready_at_ms[&(spell as u8)];
        let pending = zone.five_class_pvp.pending_count_for_test();
        let rejected = zone.handle(cast(&owner, spell, 9, before_ready - 1));
        assert!(!packets(&rejected).any(|p| matches!(p, ServerPacket::Magic { cast: true, .. })));
        assert_eq!(zone.players[&owner].mp, 21);
        assert_eq!(
            zone.players[&owner].magic_ready_at_ms[&(spell as u8)],
            before_ready
        );
        assert_eq!(zone.five_class_pvp.pending_count_for_test(), pending);
        zone.handle(cast(&owner, spell, 9, before_ready));
        assert_eq!(
            zone.players[&owner].mp, 12,
            "the source spell clock releases exactly at deadline"
        );
    }
}

#[test]
fn fiveclass_delayed_arrows_cannot_damage_a_new_owner_life_or_changed_identity() {
    for spell in [Spell::None, Spell::StraightShot, Spell::DoubleShot] {
        for boundary in 0..5 {
            let (mut zone, owner) = joined(MirClass::Archer, 30);
            spawn(&mut zone, &owner, 1, 0, 0);
            if spell == Spell::None {
                zone.handle(arrow(&owner, Spell::Focus, 1, 20));
            } else {
                zone.handle(cast(&owner, spell, 1, 20));
            }
            assert!(
                zone.five_class_pvp.pending_count_for_test() > 0,
                "a real arrow is pending before the life/identity change"
            );
            assert!(
                zone.pending_native_hits.is_empty(),
                "new-profession arrows use the typed impact queue"
            );
            match boundary {
                0 => {
                    let generation = zone.players[&owner].life_generation;
                    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                        session_id: owner.clone(),
                        hp: 0,
                        max_hp: 100,
                        mp: 21,
                        dead: true,
                    });
                    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                        session_id: owner.clone(),
                        hp: 100,
                        max_hp: 100,
                        mp: 21,
                        dead: false,
                    });
                    assert!(zone.players[&owner].life_generation > generation);
                }
                1 => {
                    zone.players.get_mut(&owner).unwrap().account_id = "replacement-account".into()
                }
                2 => zone.players.get_mut(&owner).unwrap().character_index += 1,
                3 => zone.players.get_mut(&owner).unwrap().object_id += 1,
                _ => zone.key = ZoneKey::for_map("D001"),
            }
            let out = zone.tick(1_000);
            assert_eq!(
                zone.native_monsters[&TARGET].hp, TARGET_HP,
                "{spell:?} boundary {boundary}"
            );
            assert!(journey(&out).is_empty());
            assert!(magic_practice(&out).is_empty());
        }
    }
}

#[test]
fn fiveclass_mental_state_penalties_apply_after_source_arrow_power() {
    for (state, level, expected) in [(0, 0, 100), (1, 0, 55), (1, 3, 70), (2, 3, 80)] {
        let (mut zone, owner) = joined(MirClass::Archer, 30);
        let player = zone.players.get_mut(&owner).unwrap();
        player.combat_stats.mental_state = state;
        player.combat_stats.mental_state_level = level;
        assert_eq!(
            five_class_combat::arrow_damage(
                player,
                &Point { x: 19, y: 10 },
                Spell::None,
                0,
                99_999,
                20
            ),
            expected
        );
    }
}

#[test]
fn fiveclass_old_stat_serialization_defaults_do_not_fabricate_learned_passives() {
    let mut old = serde_json::to_value(ZonePlayerCombatStats::default()).unwrap();
    for key in ["fatal_sword_level", "mental_state", "mental_state_level"] {
        old.as_object_mut().unwrap().remove(key);
    }
    let stats: ZonePlayerCombatStats = serde_json::from_value(old).unwrap();
    assert_eq!(stats.fatal_sword_level, None);
    assert_eq!((stats.mental_state, stats.mental_state_level), (0, 0));
    let mut learned = stats;
    learned.fatal_sword_level = Some(0);
    let restored: ZonePlayerCombatStats =
        serde_json::from_str(&serde_json::to_string(&learned).unwrap()).unwrap();
    assert_eq!(restored.fatal_sword_level, Some(0));
}

#[test]
fn fiveclass_double_slash_reads_current_armour_at_each_actual_impact() {
    let (mut zone, owner) = joined(MirClass::Assassin, 30);
    spawn(&mut zone, &owner, 1, 0, 0);
    zone.players
        .get_mut(&owner)
        .unwrap()
        .combat_stats
        .critical_rate = 100;
    zone.players
        .get_mut(&owner)
        .unwrap()
        .combat_stats
        .critical_damage = 10;
    zone.handle(melee(&owner, Spell::DoubleSlash, 3, 20));
    // A live defensive change after launch must block the first MAC impact.
    zone.native_monsters
        .get_mut(&TARGET)
        .unwrap()
        .defense
        .min_mac = 1000;
    zone.native_monsters
        .get_mut(&TARGET)
        .unwrap()
        .defense
        .max_mac = 1000;
    let blocked = zone.tick(320);
    assert_eq!(zone.native_monsters[&TARGET].hp, TARGET_HP);
    assert!(journey(&blocked).is_empty());
    // Source strike two uses Agility defence and an independent critical roll.
    let second = zone.tick(420);
    assert!(zone.native_monsters[&TARGET].hp < TARGET_HP);
    assert_eq!(journey(&second).len(), 1);
    assert_eq!(journey(&second)[0].committed_at_ms, 420);
}

#[test]
fn fiveclass_arrow_armour_block_after_launch_cannot_be_punched_through_by_critical() {
    let (mut zone, owner) = joined(MirClass::Archer, 30);
    spawn(&mut zone, &owner, 1, 0, 0);
    zone.players
        .get_mut(&owner)
        .unwrap()
        .combat_stats
        .critical_rate = 100;
    zone.players
        .get_mut(&owner)
        .unwrap()
        .combat_stats
        .critical_damage = 100;
    let now = (20..2000)
        .find(|now| {
            five_class_combat::ordinary_arrow_hit(
                &zone.players[&owner],
                &zone.native_monsters[&TARGET],
                TARGET,
                true,
                *now,
            )
        })
        .unwrap();
    let launch = zone.handle(arrow(&owner, Spell::Focus, 1, now));
    assert!(packets(&launch).any(|p| matches!(p, ServerPacket::ObjectRangeAttack { .. })));
    zone.native_monsters
        .get_mut(&TARGET)
        .unwrap()
        .defense
        .min_ac = 1000;
    zone.native_monsters
        .get_mut(&TARGET)
        .unwrap()
        .defense
        .max_ac = 1000;
    let blocked = zone.tick(now + 600);
    assert_eq!(zone.native_monsters[&TARGET].hp, TARGET_HP);
    assert!(journey(&blocked).is_empty());
    assert!(
        !packets(&blocked).any(|p| matches!(p, ServerPacket::ObjectEffect { info }
        if info.object_id == TARGET && info.effect == 11))
    );
}

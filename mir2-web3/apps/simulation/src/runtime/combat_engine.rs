//! Crystal-faithful combat math.
//!
//! This module is a pure, deterministic port of the damage pipeline implemented
//! in the reference Crystal server (`Server/MirObjects/MapObject.cs`,
//! `HumanObject.cs`, `MonsterObject.cs`). It intentionally has **no** access to
//! the ECS world so that every rule can be unit-tested in isolation against
//! hand-computed Crystal values.
//!
//! The Crystal server resolves combat with `Envir.Random`. The simulation is
//! deterministic, so every random draw is replaced by [`deterministic_roll`],
//! which yields a stable value in `[0, modulo)` from the current tick plus a
//! caller-supplied salt/purpose pair. Distinct `purpose` constants keep the
//! draws inside a single attack decorrelated (hit vs. armour vs. crit vs. luck).

use mir2_protocol::MirClass;

/// Deterministic, well-mixed replacement for `Envir.Random.Next(modulo)`.
///
/// Combat draws many small-modulus values (damage ranges, dodge, crit) for the
/// same attacker/target across consecutive ticks. The project-wide
/// `deterministic_roll` multiplies the tick by a constant that shares small
/// factors with common moduli, which collapses the spread (e.g. every roll mod
/// 5 is identical as the tick advances). We use a splitmix64-style finaliser
/// here so that every `(tick, salt, purpose)` triple avalanches and damage
/// varies hit-to-hit exactly as Crystal's RNG would, while staying fully
/// deterministic/replayable.
fn roll(tick: u64, salt: usize, purpose: usize, modulo: u64) -> u64 {
    if modulo == 0 {
        return 0;
    }
    let mut x = tick
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((salt as u64).wrapping_mul(0xD1B5_4A32_D192_ED03))
        .wrapping_add((purpose as u64).wrapping_mul(0xCA5A_8263_9512_1157))
        .wrapping_add(0x2545_F491_4F6C_DD1D);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x % modulo
}

/// `Settings.MaxLuck` — luck saturates the attack roll to its max/min at this
/// weight. (`Server/Settings.cs`)
pub(super) const MAX_LUCK: i64 = 10;
/// `Settings.CriticalRateWeight` — critical chance is `CriticalRate * 5` out of 100.
pub(super) const CRITICAL_RATE_WEIGHT: i64 = 5;
/// `Settings.CriticalDamageWeight` — critical bonus is `damage * (CriticalDamage / 50) * 10`.
pub(super) const CRITICAL_DAMAGE_WEIGHT: f64 = 50.0;
/// `Settings.MagicResistWeight` — magic resist is rolled out of this weight.
pub(super) const MAGIC_RESIST_WEIGHT: i64 = 10;
/// `Settings.FreezingAttackWeight` — freezing proc weight.
pub(super) const FREEZING_ATTACK_WEIGHT: i64 = 10;
/// `Settings.PoisonAttackWeight` — poison proc / poison resist weight.
pub(super) const POISON_ATTACK_WEIGHT: i64 = 10;

// Distinct roll "purposes" so the deterministic draws inside one attack do not
// correlate with each other.
const PURPOSE_ATTACK_POWER: usize = 0x01;
const PURPOSE_ATTACK_LUCK: usize = 0x02;
const PURPOSE_HIT: usize = 0x03;
const PURPOSE_MAGIC_RESIST: usize = 0x04;
const PURPOSE_CRIT: usize = 0x06;
const PURPOSE_REFLECT: usize = 0x07;
const PURPOSE_DEFENCE: usize = 0x08;

/// Crystal `DefenceType` (`Shared/Enums.cs`). Selects which armour stat is rolled
/// and whether an agility/magic-resist dodge applies.
///
/// The full set is resolved by [`get_armour`]; melee and physical ranged blows
/// construct `ACAgility`, while the magic / agility / repulsion variants are
/// constructed as the skill damage paths migrate onto this pipeline.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DefenceType {
    ACAgility,
    AC,
    MACAgility,
    MAC,
    Agility,
    Repulsion,
    None,
}

impl Default for DefenceType {
    fn default() -> Self {
        DefenceType::ACAgility
    }
}

/// The combat-relevant subset of Crystal's `Stats` map, resolved for a single
/// combatant (player or monster) at the moment of an attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct CombatStats {
    pub(super) min_dc: i32,
    pub(super) max_dc: i32,
    pub(super) min_mc: i32,
    pub(super) max_mc: i32,
    pub(super) min_sc: i32,
    pub(super) max_sc: i32,
    pub(super) min_ac: i32,
    pub(super) max_ac: i32,
    pub(super) min_mac: i32,
    pub(super) max_mac: i32,
    pub(super) accuracy: i32,
    pub(super) agility: i32,
    pub(super) luck: i32,
    pub(super) critical_rate: i32,
    pub(super) critical_damage: i32,
    pub(super) magic_resist: i32,
    pub(super) poison_resist: i32,
    pub(super) reflect: i32,
    pub(super) freezing: i32,
    pub(super) poison_attack: i32,
    pub(super) hp_drain_rate_percent: i32,
    pub(super) damage_reduction_percent: i32,
    pub(super) attack_bonus: i32,
}

/// Uniform integer in `[min, max]` (inclusive), mirroring .NET
/// `Envir.Random.Next(min, max + 1)`.
pub(super) fn deterministic_range(
    tick: u64,
    salt: usize,
    purpose: usize,
    min: i32,
    max: i32,
) -> i32 {
    let lo = min.min(max);
    let hi = max.max(min);
    let span = (i64::from(hi) - i64::from(lo) + 1).max(1) as u64;
    lo + roll(tick, salt, purpose, span) as i32
}

/// Crystal `MapObject.GetAttackPower(min, max)` — randomised offensive roll with
/// luck pulling toward the maximum (or minimum, when luck is negative).
pub(super) fn get_attack_power(luck: i32, min: i32, max: i32, tick: u64, salt: usize) -> i32 {
    let min = min.max(0);
    let max = max.max(min);

    if luck > 0 {
        if i64::from(luck) > roll(tick, salt, PURPOSE_ATTACK_LUCK, MAX_LUCK as u64) as i64
        {
            return max;
        }
    } else if luck < 0
        && i64::from(luck)
            < -(roll(tick, salt, PURPOSE_ATTACK_LUCK, MAX_LUCK as u64) as i64)
    {
        return min;
    }

    deterministic_range(tick, salt, PURPOSE_ATTACK_POWER, min, max)
}

/// Crystal `MapObject.GetDefencePower(min, max)` — randomised armour roll with no
/// luck component.
pub(super) fn get_defence_power(min: i32, max: i32, tick: u64, salt: usize) -> i32 {
    deterministic_range(tick, salt, PURPOSE_DEFENCE, min.max(0), max)
}

/// Outcome of [`get_armour`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ArmourRoll {
    pub(super) armour: i32,
    /// `false` when the blow was dodged (agility) or magic-resisted.
    pub(super) hit: bool,
}

/// Crystal `MapObject.GetArmour(type, attacker, out hit)`.
///
/// Rolls the appropriate armour stat for `defence_type` and decides whether the
/// blow connects (agility dodge and/or magic resist). The armour value is a
/// `GetDefencePower` roll over the relevant Min/Max AC or MAC.
pub(super) fn get_armour(
    defence_type: DefenceType,
    target: &CombatStats,
    attacker_accuracy: i32,
    tick: u64,
    salt: usize,
) -> ArmourRoll {
    let agility_dodges = |tick: u64, salt: usize| -> bool {
        // rand(agility + 1) > accuracy  =>  miss
        let span = i64::from(target.agility.max(0)) + 1;
        (roll(tick, salt, PURPOSE_HIT, span as u64) as i64) > i64::from(attacker_accuracy)
    };
    let magic_resisted = |tick: u64, salt: usize| -> bool {
        // rand(MagicResistWeight) < MagicResist  =>  miss
        (roll(tick, salt, PURPOSE_MAGIC_RESIST, MAGIC_RESIST_WEIGHT as u64) as i64)
            < i64::from(target.magic_resist)
    };

    let mut hit = true;
    let mut armour = 0;
    match defence_type {
        DefenceType::ACAgility => {
            if agility_dodges(tick, salt) {
                hit = false;
            }
            armour = get_defence_power(target.min_ac, target.max_ac, tick, salt);
        }
        DefenceType::AC => {
            armour = get_defence_power(target.min_ac, target.max_ac, tick, salt);
        }
        DefenceType::MACAgility => {
            if magic_resisted(tick, salt) {
                hit = false;
            }
            if agility_dodges(tick, salt) {
                hit = false;
            }
            armour = get_defence_power(target.min_mac, target.max_mac, tick, salt);
        }
        DefenceType::MAC => {
            if magic_resisted(tick, salt) {
                hit = false;
            }
            armour = get_defence_power(target.min_mac, target.max_mac, tick, salt);
        }
        DefenceType::Agility => {
            if agility_dodges(tick, salt) {
                hit = false;
            }
        }
        DefenceType::Repulsion | DefenceType::None => {}
    }

    ArmourRoll { armour, hit }
}

/// Why a blow dealt no damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NoDamageReason {
    /// Dodged (agility) or magic-resisted in [`get_armour`].
    Missed,
    /// Armour met or exceeded the incoming damage.
    Blocked,
    /// Reflected back at the attacker.
    Reflected,
}

/// Result of [`resolve_attacked`], mirroring the return of Crystal `Attacked`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AttackedOutcome {
    /// Net damage applied to the target (`damage - armour`), always `>= 0`.
    pub(super) net_damage: i32,
    /// The rolled armour that was subtracted (after `ArmourRate`).
    pub(super) armour: i32,
    /// Set when the critical-hit roll succeeded.
    pub(super) critical: bool,
    /// `Some(reason)` when no damage was dealt.
    pub(super) no_damage: Option<NoDamageReason>,
    /// Damage to apply back to the attacker when [`NoDamageReason::Reflected`].
    pub(super) reflect_damage: i32,
}

impl AttackedOutcome {
    fn no_damage(reason: NoDamageReason, armour: i32) -> Self {
        AttackedOutcome {
            net_damage: 0,
            armour,
            critical: false,
            no_damage: Some(reason),
            reflect_damage: 0,
        }
    }
}

/// Crystal `Attacked(attacker, damage, type)` damage pipeline, shared by the
/// player-victim and monster-victim overloads (the monster overload is the
/// player overload with `reflect == 0` and `damage_reduction_percent == 0`).
///
/// `raw_damage` is the attacker's already-rolled attack power
/// ([`get_attack_power`]). `armour_rate` / `damage_rate` are the target's poison
/// modifiers (`ArmourRate` / `DamageRate`), defaulting to `1.0`.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_attacked(
    attacker: &CombatStats,
    target: &CombatStats,
    raw_damage: i32,
    defence_type: DefenceType,
    armour_rate: f32,
    damage_rate: f32,
    tick: u64,
    salt: usize,
) -> AttackedOutcome {
    let armour_roll = get_armour(defence_type, target, attacker.accuracy, tick, salt);
    if !armour_roll.hit {
        return AttackedOutcome::no_damage(NoDamageReason::Missed, armour_roll.armour);
    }

    // armour *= ArmourRate; damage *= DamageRate
    let armour = ((armour_roll.armour as f32) * armour_rate) as i32;
    let mut damage = ((raw_damage.max(0) as f32) * damage_rate) as i32;

    // damage += attacker.AttackBonus
    damage = damage.saturating_add(attacker.attack_bonus);

    // Reflect: rand(100) < target.Reflect  =>  bounce the blow back, no damage.
    if target.reflect > 0
        && (roll(tick, salt, PURPOSE_REFLECT, 100) as i64) < i64::from(target.reflect)
    {
        return AttackedOutcome {
            net_damage: 0,
            armour,
            critical: false,
            no_damage: Some(NoDamageReason::Reflected),
            reflect_damage: damage.max(0),
        };
    }

    // DamageReductionPercent (MagicShield / ElementalBarrier).
    if target.damage_reduction_percent > 0 {
        damage -= damage * target.damage_reduction_percent / 100;
    }

    if armour >= damage {
        return AttackedOutcome::no_damage(NoDamageReason::Blocked, armour);
    }

    // Critical: rand(100) < CriticalRate * CriticalRateWeight.
    let mut critical = false;
    if attacker.critical_rate > 0
        && (roll(tick, salt, PURPOSE_CRIT, 100) as i64)
            < i64::from(attacker.critical_rate) * CRITICAL_RATE_WEIGHT
    {
        critical = true;
        let bonus = ((damage as f64)
            * ((attacker.critical_damage as f64) / CRITICAL_DAMAGE_WEIGHT)
            * 10.0)
            .floor() as i32;
        damage = damage.saturating_add(bonus);
    }

    let net = (damage - armour).max(0);
    AttackedOutcome {
        net_damage: net,
        armour,
        critical,
        no_damage: None,
        reflect_damage: 0,
    }
}

// Stat caps from `Settings.ClassBaseStats[..].Caps` (`Shared/BaseStats.cs`),
// applied to the equipment+buff totals in `RefreshStatCaps`.
const CAP_MAGIC_RESIST: i32 = 2;
const CAP_POISON_RESIST: i32 = 6;
const CAP_CRITICAL_RATE: i32 = 18;
const CAP_CRITICAL_DAMAGE: i32 = 10;
const CAP_FREEZING: i32 = 6;
const CAP_POISON_ATTACK: i32 = 6;

/// Crystal `StatFormula.Stat` growth: `Base` when `Gain == 0`, otherwise
/// `Base + (int)(level / Gain)` (float divide, truncated toward zero).
fn base_stat(base: i32, gain: f32, level: i32) -> i32 {
    if gain == 0.0 {
        base
    } else {
        (base as f32 + level as f32 / gain) as i32
    }
}

/// Per-class level-scaled base combat stats, ported from the default
/// `BaseStats(MirClass)` table in `Shared/BaseStats.cs`. Players derive their
/// offensive power (DC/MC/SC) almost entirely from equipment; these bases supply
/// the small level-scaled floor plus the class accuracy/agility.
pub(super) fn player_base_combat_stats(class: MirClass, level: u16) -> CombatStats {
    let lvl = i32::from(level);
    let s = |base: i32, gain: f32| base_stat(base, gain, lvl);
    let mut stats = CombatStats::default();
    match class {
        MirClass::Warrior => {
            stats.max_ac = s(0, 7.0);
            stats.min_dc = s(0, 5.0);
            stats.max_dc = s(0, 5.0);
            stats.agility = 15;
            stats.accuracy = 5;
        }
        MirClass::Wizard => {
            stats.min_dc = s(0, 7.0);
            stats.max_dc = s(0, 7.0);
            stats.min_mc = s(0, 7.0);
            stats.max_mc = s(0, 7.0);
            stats.agility = 15;
            stats.accuracy = 5;
        }
        MirClass::Taoist => {
            stats.min_mac = s(0, 12.0);
            stats.max_mac = s(0, 6.0);
            stats.min_dc = s(0, 7.0);
            stats.max_dc = s(0, 7.0);
            stats.min_sc = s(0, 7.0);
            stats.max_sc = s(0, 7.0);
            stats.agility = 18;
            stats.accuracy = 5;
        }
        MirClass::Assassin => {
            stats.min_dc = s(0, 8.0);
            stats.max_dc = s(0, 8.0);
            stats.agility = 20;
            stats.accuracy = 5;
        }
        MirClass::Archer => {
            stats.min_dc = s(0, 8.0);
            stats.max_dc = s(0, 8.0);
            stats.min_mc = s(0, 8.0);
            stats.max_mc = s(0, 8.0);
            stats.agility = 15;
            stats.accuracy = 8;
        }
    }
    stats
}

/// Crystal `HumanObject.RefreshStatCaps()` — clamp capped stats to their class
/// maximums, floor every Min/Max stat at zero, and enforce `Min <= Max`.
pub(super) fn apply_player_stat_caps(stats: &mut CombatStats) {
    stats.magic_resist = stats.magic_resist.min(CAP_MAGIC_RESIST);
    stats.poison_resist = stats.poison_resist.min(CAP_POISON_RESIST);
    stats.critical_rate = stats.critical_rate.min(CAP_CRITICAL_RATE);
    stats.critical_damage = stats.critical_damage.min(CAP_CRITICAL_DAMAGE);
    stats.freezing = stats.freezing.min(CAP_FREEZING);
    stats.poison_attack = stats.poison_attack.min(CAP_POISON_ATTACK);

    for v in [
        &mut stats.min_ac,
        &mut stats.max_ac,
        &mut stats.min_mac,
        &mut stats.max_mac,
        &mut stats.min_dc,
        &mut stats.max_dc,
        &mut stats.min_mc,
        &mut stats.max_mc,
        &mut stats.min_sc,
        &mut stats.max_sc,
    ] {
        *v = (*v).max(0);
    }

    stats.min_dc = stats.min_dc.min(stats.max_dc);
    stats.min_mc = stats.min_mc.min(stats.max_mc);
    stats.min_sc = stats.min_sc.min(stats.max_sc);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warrior(min_dc: i32, max_dc: i32) -> CombatStats {
        CombatStats {
            min_dc,
            max_dc,
            accuracy: 7,
            ..Default::default()
        }
    }

    #[test]
    fn attack_power_stays_within_range() {
        let stats = warrior(5, 10);
        for tick in 0..2_000u64 {
            let v = get_attack_power(stats.luck, stats.min_dc, stats.max_dc, tick, 42);
            assert!((5..=10).contains(&v), "tick {tick} produced {v}");
        }
    }

    #[test]
    fn attack_power_is_deterministic() {
        let a = get_attack_power(0, 3, 9, 123, 77);
        let b = get_attack_power(0, 3, 9, 123, 77);
        assert_eq!(a, b);
    }

    #[test]
    fn attack_power_covers_whole_range() {
        let mut seen_min = false;
        let mut seen_max = false;
        for tick in 0..5_000u64 {
            let v = get_attack_power(0, 4, 8, tick, 5);
            seen_min |= v == 4;
            seen_max |= v == 8;
        }
        assert!(seen_min && seen_max, "range endpoints should both occur");
    }

    #[test]
    fn high_luck_always_rolls_maximum() {
        // Luck >= MaxLuck always beats rand(MaxLuck) in [0, MaxLuck).
        for tick in 0..1_000u64 {
            let v = get_attack_power(MAX_LUCK as i32, 2, 20, tick, 1);
            assert_eq!(v, 20, "tick {tick}");
        }
    }

    #[test]
    fn strong_curse_always_rolls_minimum() {
        for tick in 0..1_000u64 {
            let v = get_attack_power(-(MAX_LUCK as i32), 2, 20, tick, 1);
            assert_eq!(v, 2, "tick {tick}");
        }
    }

    #[test]
    fn defence_power_within_range_no_luck() {
        for tick in 0..2_000u64 {
            let v = get_defence_power(2, 6, tick, 9);
            assert!((2..=6).contains(&v));
        }
    }

    #[test]
    fn zero_agility_target_never_dodges() {
        let target = CombatStats {
            agility: 0,
            ..Default::default()
        };
        for tick in 0..1_000u64 {
            let roll = get_armour(DefenceType::ACAgility, &target, 0, tick, 3);
            assert!(roll.hit, "0-agility target must always be hit, tick {tick}");
        }
    }

    #[test]
    fn high_agility_low_accuracy_sometimes_dodges() {
        let target = CombatStats {
            agility: 30,
            ..Default::default()
        };
        let mut dodges = 0;
        for tick in 0..1_000u64 {
            if !get_armour(DefenceType::ACAgility, &target, 1, tick, 3).hit {
                dodges += 1;
            }
        }
        assert!(dodges > 0, "high-agility target should dodge sometimes");
    }

    #[test]
    fn armour_subtracts_from_damage() {
        // Fixed armour (min==max) and no dodge: net == damage - armour.
        let attacker = CombatStats {
            accuracy: 100,
            ..Default::default()
        };
        let target = CombatStats {
            min_ac: 4,
            max_ac: 4,
            agility: 0,
            ..Default::default()
        };
        let out = resolve_attacked(
            &attacker,
            &target,
            10,
            DefenceType::ACAgility,
            1.0,
            1.0,
            7,
            11,
        );
        assert_eq!(out.no_damage, None);
        assert_eq!(out.armour, 4);
        assert_eq!(out.net_damage, 6);
    }

    #[test]
    fn armour_at_or_above_damage_blocks() {
        let attacker = CombatStats {
            accuracy: 100,
            ..Default::default()
        };
        let target = CombatStats {
            min_ac: 12,
            max_ac: 12,
            ..Default::default()
        };
        let out = resolve_attacked(
            &attacker,
            &target,
            10,
            DefenceType::ACAgility,
            1.0,
            1.0,
            1,
            1,
        );
        assert_eq!(out.no_damage, Some(NoDamageReason::Blocked));
        assert_eq!(out.net_damage, 0);
    }

    #[test]
    fn red_poison_armour_rate_increases_damage_taken() {
        // ArmourRate 0.9 (Crystal red poison -0.10) shrinks effective armour.
        let attacker = CombatStats {
            accuracy: 100,
            ..Default::default()
        };
        let target = CombatStats {
            min_ac: 10,
            max_ac: 10,
            ..Default::default()
        };
        let full = resolve_attacked(&attacker, &target, 20, DefenceType::AC, 1.0, 1.0, 3, 2);
        let poisoned = resolve_attacked(&attacker, &target, 20, DefenceType::AC, 0.9, 1.0, 3, 2);
        assert_eq!(full.armour, 10);
        assert_eq!(poisoned.armour, 9);
        assert!(poisoned.net_damage > full.net_damage);
    }

    #[test]
    fn damage_rate_amplifies_incoming_damage() {
        let attacker = CombatStats::default();
        let target = CombatStats {
            min_ac: 0,
            max_ac: 0,
            ..Default::default()
        };
        let base = resolve_attacked(&attacker, &target, 10, DefenceType::AC, 1.0, 1.0, 1, 1);
        let stunned = resolve_attacked(&attacker, &target, 10, DefenceType::AC, 1.0, 1.2, 1, 1);
        assert_eq!(base.net_damage, 10);
        assert_eq!(stunned.net_damage, 12);
    }

    #[test]
    fn attack_bonus_is_added_before_armour() {
        let attacker = CombatStats {
            attack_bonus: 5,
            ..Default::default()
        };
        let target = CombatStats {
            min_ac: 3,
            max_ac: 3,
            ..Default::default()
        };
        let out = resolve_attacked(&attacker, &target, 10, DefenceType::AC, 1.0, 1.0, 1, 1);
        assert_eq!(out.net_damage, 10 + 5 - 3);
    }

    #[test]
    fn damage_reduction_percent_reduces_damage() {
        let attacker = CombatStats::default();
        let target = CombatStats {
            damage_reduction_percent: 50,
            ..Default::default()
        };
        let out = resolve_attacked(&attacker, &target, 40, DefenceType::AC, 1.0, 1.0, 1, 1);
        assert_eq!(out.net_damage, 20);
    }

    #[test]
    fn reflect_bounces_damage_back() {
        let attacker = CombatStats::default();
        let target = CombatStats {
            reflect: 100, // always reflect
            min_ac: 0,
            max_ac: 0,
            ..Default::default()
        };
        let out = resolve_attacked(&attacker, &target, 15, DefenceType::AC, 1.0, 1.0, 4, 4);
        assert_eq!(out.no_damage, Some(NoDamageReason::Reflected));
        assert_eq!(out.net_damage, 0);
        assert_eq!(out.reflect_damage, 15);
    }

    #[test]
    fn critical_hit_adds_weighted_bonus() {
        // CriticalRate 100 => always crit. CriticalDamage 5 => +(dmg * (5/50) * 10) = +dmg.
        let attacker = CombatStats {
            critical_rate: 100,
            critical_damage: 5,
            ..Default::default()
        };
        let target = CombatStats::default();
        let out = resolve_attacked(&attacker, &target, 20, DefenceType::AC, 1.0, 1.0, 2, 8);
        assert!(out.critical);
        assert_eq!(out.net_damage, 40);
    }

    #[test]
    fn warrior_base_stats_scale_with_level() {
        // MaxDC base = level / 5 (truncated); Accuracy/Agility are flat.
        let l1 = player_base_combat_stats(MirClass::Warrior, 1);
        assert_eq!(l1.max_dc, 0);
        assert_eq!(l1.accuracy, 5);
        assert_eq!(l1.agility, 15);
        let l50 = player_base_combat_stats(MirClass::Warrior, 50);
        assert_eq!(l50.max_dc, 10); // 50 / 5
        assert_eq!(l50.max_ac, 7); // 50 / 7 == 7
        let assassin = player_base_combat_stats(MirClass::Assassin, 40);
        assert_eq!(assassin.max_dc, 5); // 40 / 8
        assert_eq!(assassin.agility, 20);
    }

    #[test]
    fn stat_caps_clamp_totals() {
        let mut stats = CombatStats {
            critical_rate: 50,
            critical_damage: 40,
            magic_resist: 9,
            poison_resist: 12,
            min_dc: 8,
            max_dc: 5,
            min_ac: -3,
            ..Default::default()
        };
        apply_player_stat_caps(&mut stats);
        assert_eq!(stats.critical_rate, 18);
        assert_eq!(stats.critical_damage, 10);
        assert_eq!(stats.magic_resist, 2);
        assert_eq!(stats.poison_resist, 6);
        assert_eq!(stats.min_ac, 0);
        assert_eq!(stats.min_dc, 5, "MinDC clamped to MaxDC");
    }

    #[test]
    fn zero_critical_rate_never_crits() {
        let attacker = CombatStats {
            critical_rate: 0,
            critical_damage: 50,
            ..Default::default()
        };
        let target = CombatStats::default();
        for tick in 0..1_000u64 {
            let out = resolve_attacked(&attacker, &target, 10, DefenceType::AC, 1.0, 1.0, tick, 1);
            assert!(!out.critical, "tick {tick}");
        }
    }
}

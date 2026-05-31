//! Crystal-faithful player stat engine.
//!
//! This module mirrors Crystal `PlayerObject.RefreshStats()`: it aggregates the
//! player's base class/level stats, equipment, and active buffs into a single
//! authoritative [`PlayerStats`] block keyed by the Crystal `Stat` index space
//! (see `crystal_compat::CRYSTAL_STAT_*`). Combat, defence, regeneration, and
//! weight systems read from this block instead of recomputing ad-hoc totals.
//!
//! ## Compatibility contract
//!
//! The values that already drove combat before this engine existed
//! (`MaxDC` melee, `MaxAC`/defence mitigation, accuracy) are reproduced exactly
//! so the established behaviour/tests are preserved: for seed equipment the
//! `Min*`/`Max*` ranges collapse (`min == max`), crit-rate is `0`, and the
//! resistances/recovery/weight fields are inert. The new dimensions only become
//! active once gear or buffs actually supply the relevant Crystal stat indices.

use std::collections::BTreeMap;

use bevy_ecs::prelude::{Resource, World};
use mir2_protocol::MirClass;

use super::crystal_compat::*;
use super::equipment::EquipmentState;
use super::resources::{BuffResource, InventoryResource, SessionResource, Stage5SystemsResource};

/// Crystal social experience-rate bonuses. Marriage, mentorship, and guild
/// membership each grant the player a small bonus to experience gained, mirroring
/// the Crystal lover/mentee/guild systems (previously stored but inert).
pub(super) const CRYSTAL_LOVER_EXP_RATE_PERCENT: i32 = 5;
pub(super) const CRYSTAL_MENTEE_EXP_RATE_PERCENT: i32 = 10;
pub(super) const CRYSTAL_GUILD_EXP_RATE_PERCENT: i32 = 3;

/// A computed Crystal stat block. Stat indices follow `CRYSTAL_STAT_*`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct PlayerStats {
    values: BTreeMap<u8, i32>,
}

// The accessors form the complete Crystal `Stat` surface; some bounds (e.g.
// MAC/SC ranges) are not yet consumed by the current combat paths but are part
// of the authoritative block used for display and future systems.
#[allow(dead_code)]
impl PlayerStats {
    /// Raw value for a Crystal stat index (0 when absent).
    pub(super) fn get(&self, stat: u8) -> i32 {
        self.values.get(&stat).copied().unwrap_or(0)
    }

    fn add(&mut self, stat: u8, value: i32) {
        if value == 0 {
            return;
        }
        let entry = self.values.entry(stat).or_insert(0);
        *entry = entry.saturating_add(value);
    }

    // --- Range accessors (max is always clamped to be >= min). ---
    pub(super) fn min_ac(&self) -> i32 {
        self.get(CRYSTAL_STAT_MIN_AC).max(0)
    }
    pub(super) fn max_ac(&self) -> i32 {
        self.get(CRYSTAL_STAT_MAX_AC).max(self.min_ac())
    }
    pub(super) fn min_mac(&self) -> i32 {
        self.get(CRYSTAL_STAT_MIN_MAC).max(0)
    }
    pub(super) fn max_mac(&self) -> i32 {
        self.get(CRYSTAL_STAT_MAX_MAC).max(self.min_mac())
    }
    pub(super) fn min_dc(&self) -> i32 {
        self.get(CRYSTAL_STAT_MIN_DC).max(0)
    }
    pub(super) fn max_dc(&self) -> i32 {
        self.get(CRYSTAL_STAT_MAX_DC).max(self.min_dc())
    }
    pub(super) fn min_mc(&self) -> i32 {
        self.get(CRYSTAL_STAT_MIN_MC).max(0)
    }
    pub(super) fn max_mc(&self) -> i32 {
        self.get(CRYSTAL_STAT_MAX_MC).max(self.min_mc())
    }
    pub(super) fn min_sc(&self) -> i32 {
        self.get(CRYSTAL_STAT_MIN_SC).max(0)
    }
    pub(super) fn max_sc(&self) -> i32 {
        self.get(CRYSTAL_STAT_MAX_SC).max(self.min_sc())
    }

    // --- Scalar accessors. ---
    pub(super) fn max_hp(&self) -> i32 {
        self.get(CRYSTAL_STAT_HP).max(1)
    }
    pub(super) fn max_mp(&self) -> i32 {
        self.get(CRYSTAL_STAT_MP).max(0)
    }
    pub(super) fn accuracy(&self) -> i32 {
        self.get(CRYSTAL_STAT_ACCURACY)
    }
    pub(super) fn agility(&self) -> i32 {
        self.get(CRYSTAL_STAT_AGILITY)
    }
    pub(super) fn luck(&self) -> i32 {
        self.get(CRYSTAL_STAT_LUCK)
    }
    pub(super) fn attack_speed(&self) -> i32 {
        self.get(CRYSTAL_STAT_ATTACK_SPEED)
    }
    pub(super) fn magic_resist(&self) -> i32 {
        self.get(CRYSTAL_STAT_MAGIC_RESIST).clamp(0, 10)
    }
    pub(super) fn poison_resist(&self) -> i32 {
        self.get(CRYSTAL_STAT_POISON_RESIST).clamp(0, 10)
    }
    pub(super) fn health_recovery(&self) -> i32 {
        self.get(CRYSTAL_STAT_HEALTH_RECOVERY).max(0)
    }
    pub(super) fn spell_recovery(&self) -> i32 {
        self.get(CRYSTAL_STAT_SPELL_RECOVERY).max(0)
    }
    pub(super) fn poison_recovery(&self) -> i32 {
        self.get(CRYSTAL_STAT_POISON_RECOVERY).max(0)
    }
    pub(super) fn critical_rate(&self) -> i32 {
        self.get(CRYSTAL_STAT_CRITICAL_RATE).clamp(0, 100)
    }
    pub(super) fn critical_damage(&self) -> i32 {
        self.get(CRYSTAL_STAT_CRITICAL_DAMAGE).max(0)
    }
    pub(super) fn hp_drain_rate(&self) -> i32 {
        self.get(CRYSTAL_STAT_HP_DRAIN_RATE_PERCENT).clamp(0, 100)
    }
    pub(super) fn damage_reduction_percent(&self) -> i32 {
        self.get(CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT).clamp(0, 100)
    }
    pub(super) fn bag_weight(&self) -> i32 {
        self.get(CRYSTAL_STAT_BAG_WEIGHT).max(0)
    }
    pub(super) fn hand_weight(&self) -> i32 {
        self.get(CRYSTAL_STAT_HAND_WEIGHT).max(0)
    }
    pub(super) fn wear_weight(&self) -> i32 {
        self.get(CRYSTAL_STAT_WEAR_WEIGHT).max(0)
    }
}

/// Cached authoritative player stat block. Recomputed via
/// [`refresh_player_stats`] whenever inputs (equipment, buffs, level) change.
#[derive(Resource, Debug, Clone, Default)]
pub(super) struct PlayerStatsResource {
    pub(super) stats: PlayerStats,
}

/// Crystal `StatFormula` — selects the level-scaling curve for a base stat.
#[derive(Clone, Copy)]
enum CrystalStatFormula {
    Health,
    Mana,
    Weight,
    Stat,
}

/// One per-class base-stat entry (mirrors Crystal `Shared.BaseStat`).
struct CrystalBaseStat {
    stat: u8,
    formula: CrystalStatFormula,
    base: i32,
    gain: f32,
    gain_rate: f32,
}

/// Exact port of Crystal `Shared/BaseStats.cs :: BaseStat.Calculate(job, level)`.
/// `Gain == 0` yields a flat `Base` (accuracy/agility); otherwise the curve
/// depends on the formula and class.
fn crystal_base_stat_value(entry: &CrystalBaseStat, class: MirClass, level: i32) -> i32 {
    if entry.gain == 0.0 {
        return entry.base;
    }
    let l = level.max(0) as f32;
    let base = entry.base as f32;
    let gain = entry.gain;
    let rate = entry.gain_rate;
    let value = match entry.formula {
        CrystalStatFormula::Health => match class {
            MirClass::Warrior => base + (l / gain + rate + l / 20.0) * l,
            _ => base + (l / gain + rate) * l,
        },
        CrystalStatFormula::Mana => match class {
            MirClass::Wizard => base + ((l / gain + 2.0) * 2.2 * l) + (l * rate),
            MirClass::Taoist => (base + l / gain * 2.2 * l) + (l * rate),
            _ => base + (l * gain) + (l * rate),
        },
        CrystalStatFormula::Weight => base + (l / gain) * l,
        CrystalStatFormula::Stat => base + (l / gain),
    };
    value as i32
}

/// Per-class base-stat table. Values match this repository's `BaseStats` packet
/// manifest (the contract the client computes its own display from), which is
/// in turn Crystal's `Settings.ClassBaseStats` for this build.
fn class_base_stats(class: MirClass) -> &'static [CrystalBaseStat] {
    use CrystalStatFormula::{Health, Mana, Stat, Weight};
    macro_rules! s {
        ($stat:expr, $f:expr, $b:expr, $g:expr, $r:expr) => {
            CrystalBaseStat {
                stat: $stat,
                formula: $f,
                base: $b,
                gain: $g,
                gain_rate: $r,
            }
        };
    }
    match class {
        MirClass::Warrior => &[
            s!(CRYSTAL_STAT_HP, Health, 14, 4.0, 4.5),
            s!(CRYSTAL_STAT_MP, Mana, 11, 3.5, 0.0),
            s!(CRYSTAL_STAT_BAG_WEIGHT, Weight, 50, 3.0, 0.0),
            s!(CRYSTAL_STAT_WEAR_WEIGHT, Weight, 15, 20.0, 0.0),
            s!(CRYSTAL_STAT_HAND_WEIGHT, Weight, 12, 13.0, 0.0),
            s!(CRYSTAL_STAT_MAX_AC, Stat, 0, 7.0, 0.0),
            s!(CRYSTAL_STAT_MIN_DC, Stat, 0, 5.0, 0.0),
            s!(CRYSTAL_STAT_MAX_DC, Stat, 0, 5.0, 0.0),
            s!(CRYSTAL_STAT_AGILITY, Stat, 15, 0.0, 0.0),
            s!(CRYSTAL_STAT_ACCURACY, Stat, 5, 0.0, 0.0),
        ],
        MirClass::Wizard => &[
            s!(CRYSTAL_STAT_HP, Health, 14, 15.0, 1.8),
            s!(CRYSTAL_STAT_MP, Mana, 13, 5.0, 0.0),
            s!(CRYSTAL_STAT_BAG_WEIGHT, Weight, 50, 5.0, 0.0),
            s!(CRYSTAL_STAT_WEAR_WEIGHT, Weight, 15, 100.0, 0.0),
            s!(CRYSTAL_STAT_HAND_WEIGHT, Weight, 12, 90.0, 0.0),
            s!(CRYSTAL_STAT_MIN_DC, Stat, 0, 7.0, 0.0),
            s!(CRYSTAL_STAT_MAX_DC, Stat, 0, 7.0, 0.0),
            s!(CRYSTAL_STAT_MIN_MC, Stat, 0, 7.0, 0.0),
            s!(CRYSTAL_STAT_MAX_MC, Stat, 0, 7.0, 0.0),
            s!(CRYSTAL_STAT_AGILITY, Stat, 15, 0.0, 0.0),
            s!(CRYSTAL_STAT_ACCURACY, Stat, 5, 0.0, 0.0),
        ],
        MirClass::Taoist => &[
            s!(CRYSTAL_STAT_HP, Health, 14, 6.0, 2.5),
            s!(CRYSTAL_STAT_MP, Mana, 13, 8.0, 0.0),
            s!(CRYSTAL_STAT_BAG_WEIGHT, Weight, 50, 4.0, 0.0),
            s!(CRYSTAL_STAT_WEAR_WEIGHT, Weight, 15, 50.0, 0.0),
            s!(CRYSTAL_STAT_HAND_WEIGHT, Weight, 12, 42.0, 0.0),
            s!(CRYSTAL_STAT_MIN_MAC, Stat, 0, 12.0, 0.0),
            s!(CRYSTAL_STAT_MAX_MAC, Stat, 0, 6.0, 0.0),
            s!(CRYSTAL_STAT_MIN_DC, Stat, 0, 7.0, 0.0),
            s!(CRYSTAL_STAT_MAX_DC, Stat, 0, 7.0, 0.0),
            s!(CRYSTAL_STAT_MIN_SC, Stat, 0, 7.0, 0.0),
            s!(CRYSTAL_STAT_MAX_SC, Stat, 0, 7.0, 0.0),
            s!(CRYSTAL_STAT_AGILITY, Stat, 18, 0.0, 0.0),
            s!(CRYSTAL_STAT_ACCURACY, Stat, 5, 0.0, 0.0),
        ],
        MirClass::Assassin => &[
            s!(CRYSTAL_STAT_HP, Health, 14, 4.0, 3.25),
            s!(CRYSTAL_STAT_MP, Mana, 11, 5.0, 0.0),
            s!(CRYSTAL_STAT_BAG_WEIGHT, Weight, 50, 3.5, 0.0),
            s!(CRYSTAL_STAT_WEAR_WEIGHT, Weight, 15, 33.0, 0.0),
            s!(CRYSTAL_STAT_HAND_WEIGHT, Weight, 12, 30.0, 0.0),
            s!(CRYSTAL_STAT_MIN_DC, Stat, 0, 8.0, 0.0),
            s!(CRYSTAL_STAT_MAX_DC, Stat, 0, 8.0, 0.0),
            s!(CRYSTAL_STAT_AGILITY, Stat, 18, 0.0, 0.0),
            s!(CRYSTAL_STAT_ACCURACY, Stat, 7, 0.0, 0.0),
        ],
        MirClass::Archer => &[
            s!(CRYSTAL_STAT_HP, Health, 14, 4.0, 3.25),
            s!(CRYSTAL_STAT_MP, Mana, 11, 4.0, 0.0),
            s!(CRYSTAL_STAT_BAG_WEIGHT, Weight, 50, 4.0, 0.0),
            s!(CRYSTAL_STAT_WEAR_WEIGHT, Weight, 15, 33.0, 0.0),
            s!(CRYSTAL_STAT_HAND_WEIGHT, Weight, 12, 30.0, 0.0),
            s!(CRYSTAL_STAT_MIN_DC, Stat, 0, 8.0, 0.0),
            s!(CRYSTAL_STAT_MAX_DC, Stat, 0, 8.0, 0.0),
            s!(CRYSTAL_STAT_MIN_MC, Stat, 0, 8.0, 0.0),
            s!(CRYSTAL_STAT_MAX_MC, Stat, 0, 8.0, 0.0),
            s!(CRYSTAL_STAT_AGILITY, Stat, 15, 0.0, 0.0),
            s!(CRYSTAL_STAT_ACCURACY, Stat, 8, 0.0, 0.0),
        ],
    }
}

/// Per-class stat caps (`Shared/BaseStats.cs :: Caps`). Applied after assembly,
/// mirroring Crystal `RefreshStatCaps`.
fn class_stat_caps() -> &'static [(u8, i32)] {
    &[
        (CRYSTAL_STAT_MAGIC_RESIST, 2),
        (CRYSTAL_STAT_POISON_RESIST, 6),
        (CRYSTAL_STAT_CRITICAL_RATE, 18),
        (CRYSTAL_STAT_CRITICAL_DAMAGE, 10),
        (CRYSTAL_STAT_HEALTH_RECOVERY, 8),
        (CRYSTAL_STAT_SPELL_RECOVERY, 8),
        (CRYSTAL_STAT_POISON_RECOVERY, 6),
    ]
}

/// Sum of an equipped item's explicit `added_stats` for a single Crystal stat
/// index (scalar attack/defence/luck are handled separately by the caller).
fn item_added_stat_sum(item: &EquipmentState, stat: u8) -> i32 {
    item.added_stats
        .iter()
        .filter(|entry| entry.stat == stat)
        .map(|entry| entry.value)
        .sum()
}

/// A single equipped item's contribution to a `(Min*, Max*)` combat range.
///
/// `scalar` is the item's legacy primary value for that stat (weapon attack for
/// DC, armour defence for AC, `0` otherwise). The max contribution is
/// `scalar + added_stats[max_stat]`; the min collapses to that same value
/// unless the item carries an explicit `added_stats[min_stat]`. For seed
/// equipment this yields `min == max`, so deterministic combat rolls land on
/// the historical flat number.
fn item_range_contribution(
    item: &EquipmentState,
    scalar: i32,
    min_stat: u8,
    max_stat: u8,
) -> (i32, i32) {
    let max = scalar.saturating_add(item_added_stat_sum(item, max_stat));
    let explicit_min = item_added_stat_sum(item, min_stat);
    let min = if explicit_min > 0 { explicit_min } else { max };
    (min, max)
}

/// Recompute the authoritative stat block, publish it as a snapshot resource,
/// and reconcile the derived `max_hp`/`max_mp` onto the player's vitals — the
/// Crystal `RefreshStats` behaviour where gear/level changes resize the HP/MP
/// pools (current values are clamped, never inflated).
pub(super) fn refresh_player_stats(world: &mut World) {
    let stats = compute_player_stats(world);
    let max_hp = stats.max_hp();
    let max_mp = stats.max_mp();

    if let Some(mut resource) = world.get_resource_mut::<PlayerStatsResource>() {
        resource.stats = stats;
    } else {
        world.insert_resource(PlayerStatsResource { stats });
    }

    apply_pool_caps_to_player(world, max_hp, max_mp);
}

/// Resize the player's HP/MP pools to the freshly computed maxima, clamping the
/// live values so an unequip can never leave `hp > max_hp`.
fn apply_pool_caps_to_player(world: &mut World, max_hp: i32, max_mp: i32) {
    use super::components::{player_entity, PlayerVitals};

    let Some(player) = player_entity(world) else {
        return;
    };
    let updated = {
        let mut entity = world.entity_mut(player);
        entity.get_mut::<PlayerVitals>().map(|mut vitals| {
            vitals.max_hp = max_hp.max(1);
            vitals.max_mp = max_mp.max(0);
            vitals.hp = vitals.hp.min(vitals.max_hp);
            vitals.mp = vitals.mp.min(vitals.max_mp);
            *vitals
        })
    };
    if let Some(updated) = updated {
        world
            .resource_mut::<super::resources::PlayerRuntimeResource>()
            .player_vitals = updated;
    }
}

/// Total experience-rate bonus (percent) the player earns from active social
/// relationships: marriage (lover), mentorship (mentee), and guild membership.
/// Returns `0` for an unattached player, so ordinary experience gain is
/// unchanged.
pub(super) fn crystal_player_social_exp_rate_percent(world: &World) -> i32 {
    let Some(systems) = world.get_resource::<Stage5SystemsResource>() else {
        return 0;
    };
    let state = &systems.stage5_systems;
    let mut rate = 0;
    if !state.relationship.partner_name.is_empty() {
        rate += CRYSTAL_LOVER_EXP_RATE_PERCENT;
    }
    if !state.mentor.name.is_empty() {
        rate += CRYSTAL_MENTEE_EXP_RATE_PERCENT;
    }
    if !state.guild.name.is_empty() {
        rate += CRYSTAL_GUILD_EXP_RATE_PERCENT;
    }
    rate
}

/// Apply the social experience-rate bonus to a raw experience amount.
pub(super) fn crystal_apply_social_exp_rate(world: &World, base_experience: u32) -> u32 {
    let rate = crystal_player_social_exp_rate_percent(world);
    if rate <= 0 || base_experience == 0 {
        return base_experience;
    }
    let bonus = (u64::from(base_experience) * rate as u64) / 100;
    base_experience.saturating_add(u32::try_from(bonus).unwrap_or(u32::MAX))
}

/// Read the live player stat block. Always computed fresh (read-only and cheap:
/// at most a handful of equipment slots plus active buffs) so combat never sees
/// a stale snapshot mid-tick.
pub(super) fn player_stats(world: &World) -> PlayerStats {
    compute_player_stats(world)
}

/// Compute the full Crystal stat block from base class/level + equipment +
/// buffs. Pure read-only over the world.
pub(super) fn compute_player_stats(world: &World) -> PlayerStats {
    let session = world.resource::<SessionResource>();
    let (class, level) = session
        .selected_character
        .as_ref()
        .map(|character| (character.class, i32::from(character.level)))
        .unwrap_or((MirClass::Warrior, 1));
    let mut stats = PlayerStats::default();

    // --- Base class / level stats (Crystal RefreshLevelStats via BaseStat.Calculate). ---
    // The class table provides MinDC/MaxDC (and AC/MAC/MC/SC) scaling; there is
    // no flat melee floor — melee derives entirely from base + equipment, as in
    // Crystal.
    for entry in class_base_stats(class) {
        stats.add(entry.stat, crystal_base_stat_value(entry, class, level));
    }

    // --- Equipment. ---
    let inventory = world.resource::<InventoryResource>();
    accumulate_equipment(&mut stats, inventory);

    // --- Buffs. ---
    let buffs = world.resource::<BuffResource>();
    accumulate_buffs(&mut stats, buffs);

    // --- Percent multipliers, then caps (Crystal RefreshStats tail + RefreshStatCaps). ---
    apply_rate_multipliers(&mut stats);
    apply_stat_caps(&mut stats);

    stats
}

/// Crystal `RefreshStatCaps`: clamp capped stats to their per-class maximum,
/// floor the combat bounds at 0, and keep each `Min*` no greater than its `Max*`.
fn apply_stat_caps(stats: &mut PlayerStats) {
    for (stat, cap) in class_stat_caps() {
        let capped = stats.get(*stat).min(*cap);
        stats.values.insert(*stat, capped);
    }
    for stat in [
        CRYSTAL_STAT_MIN_AC,
        CRYSTAL_STAT_MAX_AC,
        CRYSTAL_STAT_MIN_MAC,
        CRYSTAL_STAT_MAX_MAC,
        CRYSTAL_STAT_MIN_DC,
        CRYSTAL_STAT_MAX_DC,
        CRYSTAL_STAT_MIN_MC,
        CRYSTAL_STAT_MAX_MC,
        CRYSTAL_STAT_MIN_SC,
        CRYSTAL_STAT_MAX_SC,
    ] {
        let floored = stats.get(stat).max(0);
        stats.values.insert(stat, floored);
    }
    for (min_stat, max_stat) in [
        (CRYSTAL_STAT_MIN_DC, CRYSTAL_STAT_MAX_DC),
        (CRYSTAL_STAT_MIN_MC, CRYSTAL_STAT_MAX_MC),
        (CRYSTAL_STAT_MIN_SC, CRYSTAL_STAT_MAX_SC),
    ] {
        let clamped = stats.get(min_stat).min(stats.get(max_stat));
        stats.values.insert(min_stat, clamped);
    }
}

fn accumulate_equipment(stats: &mut PlayerStats, inventory: &InventoryResource) {
    for item in inventory
        .equipment_items
        .iter()
        .filter(|item| !item.is_broken())
    {
        // DC: weapon base attack + added attack (via total_attack) + explicit
        // Min/Max DC added stats.
        let (min_dc, max_dc) = item_range_contribution(
            item,
            item.total_attack(),
            CRYSTAL_STAT_MIN_DC,
            CRYSTAL_STAT_MAX_DC,
        );
        stats.add(CRYSTAL_STAT_MAX_DC, max_dc);
        stats.add(CRYSTAL_STAT_MIN_DC, min_dc);

        // AC: armour base defence + added defence (via total_defence) + explicit
        // Min/Max AC added stats.
        let (min_ac, max_ac) = item_range_contribution(
            item,
            item.total_defence(),
            CRYSTAL_STAT_MIN_AC,
            CRYSTAL_STAT_MAX_AC,
        );
        stats.add(CRYSTAL_STAT_MAX_AC, max_ac);
        stats.add(CRYSTAL_STAT_MIN_AC, min_ac);

        // MAC / MC / SC ranges (purely from explicit added stats).
        for (min_stat, max_stat) in [
            (CRYSTAL_STAT_MIN_MAC, CRYSTAL_STAT_MAX_MAC),
            (CRYSTAL_STAT_MIN_MC, CRYSTAL_STAT_MAX_MC),
            (CRYSTAL_STAT_MIN_SC, CRYSTAL_STAT_MAX_SC),
        ] {
            let (min_value, max_value) = item_range_contribution(item, 0, min_stat, max_stat);
            stats.add(max_stat, max_value);
            stats.add(min_stat, min_value);
        }

        // Scalar stats carried directly on the item's added-stat list.
        for stat in [
            CRYSTAL_STAT_ACCURACY,
            CRYSTAL_STAT_AGILITY,
            CRYSTAL_STAT_HP,
            CRYSTAL_STAT_MP,
            CRYSTAL_STAT_ATTACK_SPEED,
            CRYSTAL_STAT_LUCK,
            CRYSTAL_STAT_MAGIC_RESIST,
            CRYSTAL_STAT_POISON_RESIST,
            CRYSTAL_STAT_HEALTH_RECOVERY,
            CRYSTAL_STAT_SPELL_RECOVERY,
            CRYSTAL_STAT_POISON_RECOVERY,
            CRYSTAL_STAT_CRITICAL_RATE,
            CRYSTAL_STAT_CRITICAL_DAMAGE,
            CRYSTAL_STAT_HP_DRAIN_RATE_PERCENT,
            CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT,
            CRYSTAL_STAT_ENERGY_SHIELD_PERCENT,
            CRYSTAL_STAT_ENERGY_SHIELD_HP_GAIN,
            CRYSTAL_STAT_MANA_PENALTY_PERCENT,
            CRYSTAL_STAT_MAX_DC_RATE_PERCENT,
            CRYSTAL_STAT_MAX_MC_RATE_PERCENT,
            CRYSTAL_STAT_MAX_SC_RATE_PERCENT,
            CRYSTAL_STAT_ATTACK_SPEED_RATE_PERCENT,
        ] {
            let value: i32 = item
                .added_stats
                .iter()
                .filter(|entry| entry.stat == stat)
                .map(|entry| entry.value)
                .sum();
            stats.add(stat, value);
        }

        // Equipment luck scalar (stored separately from added_stats).
        stats.add(CRYSTAL_STAT_LUCK, item.added_luck);
    }
}

fn accumulate_buffs(stats: &mut PlayerStats, buffs: &BuffResource) {
    for buff in &buffs.buffs {
        // Buff scalar attack/defence bonuses map onto MaxDC/MaxAC, mirroring the
        // historical `buff_attack_bonus`/`buff_defence_bonus` helpers.
        let attack_from_stats = buff
            .stats
            .iter()
            .filter(|entry| entry.stat == CRYSTAL_STAT_MAX_DC)
            .map(|entry| entry.value)
            .sum::<i32>();
        let defence_from_stats = buff
            .stats
            .iter()
            .filter(|entry| entry.stat == CRYSTAL_STAT_MAX_AC)
            .map(|entry| entry.value)
            .sum::<i32>();
        stats.add(CRYSTAL_STAT_MAX_DC, buff.attack_bonus.max(attack_from_stats));
        stats.add(
            CRYSTAL_STAT_MAX_AC,
            buff.defence_bonus.max(defence_from_stats),
        );

        for entry in &buff.stats {
            // MaxDC/MaxAC already folded above to honour the `.max()` semantics.
            if entry.stat == CRYSTAL_STAT_MAX_DC || entry.stat == CRYSTAL_STAT_MAX_AC {
                continue;
            }
            stats.add(entry.stat, entry.value);
        }
    }
}

/// Apply Crystal `*RatePercent` multipliers. Per `RefreshStats`, the percent is
/// applied to the `Max`/pool stats only (`HP, MP, MaxAC, MaxMAC, MaxDC, MaxMC,
/// MaxSC, AttackSpeed`), never to the `Min*` bounds.
fn apply_rate_multipliers(stats: &mut PlayerStats) {
    for (value_stat, rate_stat) in [
        (CRYSTAL_STAT_HP, CRYSTAL_STAT_HP_RATE_PERCENT),
        (CRYSTAL_STAT_MP, CRYSTAL_STAT_MP_RATE_PERCENT),
        (CRYSTAL_STAT_MAX_AC, CRYSTAL_STAT_MAX_AC_RATE_PERCENT),
        (CRYSTAL_STAT_MAX_MAC, CRYSTAL_STAT_MAX_MAC_RATE_PERCENT),
        (CRYSTAL_STAT_MAX_DC, CRYSTAL_STAT_MAX_DC_RATE_PERCENT),
        (CRYSTAL_STAT_MAX_MC, CRYSTAL_STAT_MAX_MC_RATE_PERCENT),
        (CRYSTAL_STAT_MAX_SC, CRYSTAL_STAT_MAX_SC_RATE_PERCENT),
        (CRYSTAL_STAT_ATTACK_SPEED, CRYSTAL_STAT_ATTACK_SPEED_RATE_PERCENT),
    ] {
        let percent = stats.get(rate_stat);
        if percent == 0 {
            continue;
        }
        let value = stats.get(value_stat);
        let scaled = value.saturating_add(value.saturating_mul(percent).div_euclid(100));
        stats.values.insert(value_stat, scaled);
    }
}

/// Deterministic inclusive roll in `[min, max]`. Mirrors the existing
/// `deterministic_roll` usage so combat stays reproducible. When `min == max`
/// (the seed-equipment case) the result is exactly `max`.
pub(super) fn deterministic_range_roll(
    current_tick: u64,
    actor_id: u32,
    salt: usize,
    min: i32,
    max: i32,
) -> i32 {
    let low = min.min(max);
    let high = min.max(max);
    if low >= high {
        return high;
    }
    let span = u64::try_from(high - low + 1).unwrap_or(1).max(1);
    let roll = super::monsters::deterministic_roll(
        current_tick,
        usize::try_from(actor_id).unwrap_or_default(),
        salt,
        span,
    );
    low.saturating_add(i32::try_from(roll).unwrap_or(0))
}

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::{
    crystal_magic_by_spell, crystal_monster_by_name, crystal_monster_manifest,
    format_localized_text, localized_text_or_fallback, CrystalMagicTemplate,
    CrystalRespawnTemplate,
};
use mir2_protocol::{ChatType, MirClass, MirDirection, Point, ServerPacket};
use mir2_protocol::{ObjectAttackInfo, ObjectEffectInfo, ObjectRangeAttackInfo, Spell};

use super::buffs::{apply_or_refresh_buff, BuffState};
use super::combat_engine::{
    apply_player_stat_caps, get_attack_power, player_base_combat_stats, resolve_attacked,
    CombatStats, DefenceType, NoDamageReason, FREEZING_ATTACK_WEIGHT, POISON_ATTACK_WEIGHT,
};
use super::components::{
    current_player_is_dead, current_player_object_id, entity_by_object_id, entity_facing,
    entity_name, entity_object_id, entity_position, player_entity, Facing, GeneralMeowMeowState,
    Monster, MonsterAgent, MonsterAiState, MonsterCombatStats, MonsterPoisonState, MonsterVitals,
    ObjectId, PlayerVitals, Position, SpawnSlotRef, SummonedMonster, TrainerDamageState,
};
use super::crystal_compat::*;
use super::drops::{
    handle_monster_defeat, harvest_monster_entity, harvest_target_in_direction,
    HarvestTargetSelection,
};
use super::equipment::{damage_weapon_durability, damage_worn_durability, total_attack_bonus};
use super::items::{crystal_equipment_added_stat_total, current_player_required_stat_total};
use super::monster_ai::{
    advance_world, schedule_snow_wolf_king_death_explosion, set_guardian_rocks_active_near,
};
use super::monsters::{
    crystal_dynamic_monster_template, crystal_monster_attack_damage,
    crystal_monster_effect_for_name, crystal_monster_raw_attack_damage,
    crystal_respawn_template_from_monster, deterministic_roll, ignores_monster_damage,
    is_hidden_or_sleeping_target, monster_ignores_damage, monster_is_damageable,
    monster_is_stoned_zuma, monster_locks_player_target_on_hit, monster_melee_attack_packet,
    queue_pending_monster_spawn, schedule_monster_respawn, PendingMonsterSpawnAction,
};
use super::movement::{
    current_location, current_movement, direction_toward, directional_destination, offset_point,
    push_player_in_direction, tile_distance,
};
use super::npc::dismiss_dialog;
use super::packets::{
    object_died_info_for_entity, object_health_info_for_entity, object_mana_info_for_entity,
    object_struck_packet, player_struck_packet, system_message_key,
    visible_object_bundle_for_entity,
};
use super::quests::advance_crystal_quest_kill;
use super::resources::{
    is_in_world, runtime_tick, BuffResource, ElementalResource, InventoryResource,
    PlayerRuntimeResource, RuntimeClockResource, RuntimeQueueResource, SessionResource,
    SkillResource,
};
use super::session::SimulationSession;
use super::skills::{advance_magic_progression, crystal_magic_damage, normalize_crystal_skill_key};

#[allow(deprecated)]
pub(super) fn attack_target_in_direction(world: &World, direction: MirDirection) -> Option<u32> {
    attack_target_in_direction_at_distance(world, direction, 1)
}

#[allow(deprecated)]
fn attack_target_in_direction_at_distance(
    world: &World,
    direction: MirDirection,
    distance: i32,
) -> Option<u32> {
    let player = player_entity(world)?;
    let player_position = entity_position(world, player)?;
    let target_position = offset_point(&player_position, direction, distance);

    world
        .iter_entities()
        .filter_map(|entity| {
            let position = entity.get::<Position>()?;
            if position.0 != target_position {
                return None;
            }

            let monster = entity.get::<MonsterAgent>()?;
            let ai_state = entity.get::<MonsterAiState>().copied().unwrap_or_default();
            if monster.dead || ai_state.hidden || monster_is_stoned_zuma(monster, &ai_state) {
                return None;
            }

            Some(entity.get::<ObjectId>()?.0)
        })
        .min()
}

const CRYSTAL_SPELL_EFFECT_FATAL_SWORD: u8 = 1;
const CRYSTAL_SPELL_EFFECT_REFLECT: u8 = 10;
const CRYSTAL_SPELL_EFFECT_CRITICAL: u8 = 11;
const CRYSTAL_SPELL_EFFECT_MP_EATER: u8 = 17;
const CRYSTAL_SPELL_EFFECT_HEMORRHAGE: u8 = 18;
const CRYSTAL_POISON_GREEN: u16 = 1;
const CRYSTAL_POISON_RED: u16 = 2;
const CRYSTAL_POISON_SLOW: u16 = 4;
const CRYSTAL_POISON_STUN: u16 = 16;
const CRYSTAL_POISON_BLEEDING: u16 = 128;
// Crystal `ProcessPoison`: Red poison reduces the victim's effective armour by
// 10% (`ArmourRate -= 0.10`); Stun raises incoming damage by 20%
// (`DamageRate += 0.20`).
const CRYSTAL_RED_POISON_ARMOUR_RATE_DELTA: f32 = 0.10;
const CRYSTAL_STUN_POISON_DAMAGE_RATE_DELTA: f32 = 0.20;
// `DamageType` (`Shared/Enums.cs`): Hit = 0, Miss = 1, Critical = 2.
const CRYSTAL_DAMAGE_TYPE_MISS: u8 = 1;
const CRYSTAL_DAMAGE_TYPE_CRITICAL: u8 = 2;
const CRYSTAL_PLAYER_STATUS_DAMAGE_TICK_INTERVAL: u64 = 2;
const CRYSTAL_PLAYER_GREEN_POISON_TICK_DAMAGE: i32 = 5;
const CRYSTAL_PLAYER_BLEEDING_TICK_DAMAGE: i32 = 3;
const CRYSTAL_RED_POISON_DAMAGE_BONUS_PERCENT: i32 = 25;

fn crystal_skill_level(world: &World, spell_name: &str) -> Option<u8> {
    let key = normalize_crystal_skill_key(spell_name);
    world
        .resource::<SkillResource>()
        .skills
        .iter()
        .find(|skill| skill.key == key)
        .map(|skill| skill.level)
}

fn crystal_skill_magic(world: &World, spell_name: &str) -> Option<(CrystalMagicTemplate, u8)> {
    let level = crystal_skill_level(world, spell_name)?;
    Some((crystal_magic_by_spell(spell_name)?, level))
}

fn skill_toggle_state(world: &World, spell: Spell) -> bool {
    world
        .resource::<SkillResource>()
        .spell_toggles
        .iter()
        .find(|(candidate, _)| *candidate == spell)
        .map(|(_, enabled)| *enabled)
        .unwrap_or(false)
}

fn set_skill_toggle_state(world: &mut World, spell: Spell, enabled: bool) {
    let mut skills = world.resource_mut::<SkillResource>();
    if let Some((_, existing)) = skills
        .spell_toggles
        .iter_mut()
        .find(|(candidate, _)| *candidate == spell)
    {
        *existing = enabled;
    } else {
        skills.spell_toggles.push((spell, enabled));
    }
}

pub(super) fn crystal_player_has_active_buff(world: &World, key: &str) -> bool {
    let tick = runtime_tick(world);
    world
        .resource::<BuffResource>()
        .buffs
        .iter()
        .any(|buff| buff.key == key && buff.expires_at_tick > tick)
}

pub(super) fn crystal_player_movement_blocked_by_status(world: &World) -> bool {
    [
        CAVE_MAGGOT_PARALYSIS_BUFF_KEY,
        HELL_KEEPER_DAZED_BUFF_KEY,
        MAN_TREE_STUN_BUFF_KEY,
        ICE_GUARD_FROZEN_BUFF_KEY,
    ]
    .iter()
    .any(|key| crystal_player_has_active_buff(world, key))
}

pub(super) fn crystal_player_attack_blocked_by_status(world: &World) -> bool {
    crystal_player_movement_blocked_by_status(world)
        || crystal_player_has_active_buff(world, RESTLESS_JAR_BLINDNESS_BUFF_KEY)
}

pub(super) fn crystal_player_magic_blocked_by_status(world: &World) -> bool {
    crystal_player_attack_blocked_by_status(world)
}

pub(super) fn crystal_player_slowed_by_status(world: &World) -> bool {
    crystal_player_has_active_buff(world, ICE_GUARD_SLOW_BUFF_KEY)
}

fn crystal_player_damage_after_status(world: &World, damage: i32) -> i32 {
    if damage <= 0 || !crystal_player_has_active_buff(world, YIMOOGI_RED_POISON_BUFF_KEY) {
        return damage;
    }

    damage.saturating_add(
        damage
            .saturating_mul(CRYSTAL_RED_POISON_DAMAGE_BONUS_PERCENT)
            .div_euclid(100)
            .max(1),
    )
}

#[derive(Debug, Clone, Copy)]
struct PlayerDamageOutcome {
    applied: bool,
    died: bool,
}

fn apply_damage_to_current_player(
    world: &mut World,
    damage: i32,
    packets: &mut Vec<ServerPacket>,
) -> PlayerDamageOutcome {
    // Legacy path: apply the simplified red-poison amplification, then the loss.
    // (Pipeline hits handle poison via ArmourRate in `resolve_attacked` and call
    // `apply_player_hp_loss` directly to avoid double-counting.)
    let adjusted_damage = crystal_player_damage_after_status(world, damage);
    apply_player_hp_loss(world, adjusted_damage, packets)
}

/// Subtract an already-final amount from the player's HP and emit the health /
/// death packets. Shared by the legacy damage path and the Crystal `Attacked`
/// pipeline.
fn apply_player_hp_loss(
    world: &mut World,
    amount: i32,
    packets: &mut Vec<ServerPacket>,
) -> PlayerDamageOutcome {
    if amount <= 0 {
        return PlayerDamageOutcome {
            applied: false,
            died: false,
        };
    }

    let Some(player) = player_entity(world) else {
        return PlayerDamageOutcome {
            applied: false,
            died: false,
        };
    };

    let Some((updated_vitals, was_alive)) = ({
        let mut entity = world.entity_mut(player);
        entity.get_mut::<PlayerVitals>().map(|mut vitals| {
            let was_alive = vitals.hp > 0;
            vitals.hp = vitals.hp.saturating_sub(amount).max(0);
            (*vitals, was_alive)
        })
    }) else {
        return PlayerDamageOutcome {
            applied: false,
            died: false,
        };
    };

    world.resource_mut::<PlayerRuntimeResource>().player_vitals = updated_vitals;
    if let Some(info) = object_health_info_for_entity(world, player, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }

    let died = was_alive && updated_vitals.hp <= 0;
    if died {
        if let Some(info) = object_died_info_for_entity(world, player, 0) {
            packets.push(ServerPacket::ObjectDied { info });
        }
    }

    PlayerDamageOutcome {
        applied: was_alive,
        died,
    }
}

fn crystal_player_melee_damage(world: &World) -> i32 {
    let resources = world.resource::<InventoryResource>();
    let session = world.resource::<SessionResource>();
    let level_bonus = i32::from(
        session
            .selected_character
            .as_ref()
            .map(|c| c.level)
            .unwrap_or(1)
            / 2,
    );
    let mut damage =
        18 + level_bonus + total_attack_bonus(&resources, world.resource::<BuffResource>());
    if let Some(level) = crystal_skill_level(world, "Slaying") {
        let bonuses = [5, 6, 7, 8];
        let index = usize::from(level).min(bonuses.len() - 1);
        damage = damage.saturating_add(bonuses[index]);
    }
    damage.max(1)
}

fn crystal_player_zone_base_melee_damage(world: &World) -> i32 {
    let resources = world.resource::<InventoryResource>();
    let session = world.resource::<SessionResource>();
    let level_bonus = i32::from(
        session
            .selected_character
            .as_ref()
            .map(|c| c.level)
            .unwrap_or(1)
            / 2,
    );
    let equipment_attack = resources
        .equipment_items
        .iter()
        .filter(|item| !item.is_broken())
        .map(|item| item.total_attack())
        .sum::<i32>();
    let mut damage = 18 + level_bonus + equipment_attack;
    if let Some(level) = crystal_skill_level(world, "Slaying") {
        let bonuses = [5, 6, 7, 8];
        let index = usize::from(level).min(bonuses.len() - 1);
        damage = damage.saturating_add(bonuses[index]);
    }
    damage.max(1)
}

fn crystal_skill_accuracy_bonus(world: &World) -> i32 {
    let fencing = crystal_skill_level(world, "Fencing")
        .map(|level| i32::from(level) * 3)
        .unwrap_or(0);
    let slaying = crystal_skill_level(world, "Slaying")
        .map(i32::from)
        .unwrap_or(0);
    let spirit_sword = crystal_skill_level(world, "SpiritSword")
        .map(|level| match level {
            0 => 0,
            1 => 3,
            2 => 5,
            _ => 8,
        })
        .unwrap_or(0);

    fencing.saturating_add(slaying).saturating_add(spirit_sword)
}

fn crystal_player_accuracy(world: &World) -> i32 {
    crystal_equipment_added_stat_total(world.resource::<InventoryResource>(), CRYSTAL_STAT_ACCURACY)
        .saturating_add(crystal_skill_accuracy_bonus(world))
}

/// Stand-in for the monster `Accuracy` stat, which the generated monster
/// manifest does not export (Crystal stores it in `MonsterInfo.Stats`). Without
/// it the agility dodge would make a base-agility player evade almost every
/// monster blow, so monster attacks use this floor to land reliably while still
/// letting very-high-agility builds occasionally dodge.
const MONSTER_DEFAULT_ACCURACY: i32 = 20;

/// Resolve the player's full Crystal combat stats: class/level base
/// ([`player_base_combat_stats`]) plus equipment and buff totals, normalised by
/// the class stat caps ([`apply_player_stat_caps`]).
pub(super) fn player_combat_stats(world: &World) -> CombatStats {
    let (class, level) = {
        let session = world.resource::<SessionResource>();
        session
            .selected_character
            .as_ref()
            .map(|character| (character.class, character.level))
            .unwrap_or((MirClass::Warrior, 1))
    };
    let inventory = world.resource::<InventoryResource>();
    let buffs = world.resource::<BuffResource>();
    let add = |stat: u8| current_player_required_stat_total(inventory, buffs, stat);

    // Equipment exposes a single attack/defence figure per slot (mapped onto the
    // Max stat), so when an item carries no explicit Min stat we treat the Min as
    // equal to the Max. Real Crystal gear has Min close to Max, so this is far
    // closer than assuming a `[0, Max]` spread (which would halve effective DPS).
    let equip_pair = |stat_min: u8, stat_max: u8| -> (i32, i32) {
        let equip_max = add(stat_max);
        let equip_min = add(stat_min);
        (
            if equip_min > 0 { equip_min } else { equip_max },
            equip_max,
        )
    };

    let mut stats = player_base_combat_stats(class, level);
    let (dc_min, dc_max) = equip_pair(CRYSTAL_STAT_MIN_DC, CRYSTAL_STAT_MAX_DC);
    stats.min_dc += dc_min;
    stats.max_dc += dc_max;
    let (mc_min, mc_max) = equip_pair(CRYSTAL_STAT_MIN_MC, CRYSTAL_STAT_MAX_MC);
    stats.min_mc += mc_min;
    stats.max_mc += mc_max;
    let (sc_min, sc_max) = equip_pair(CRYSTAL_STAT_MIN_SC, CRYSTAL_STAT_MAX_SC);
    stats.min_sc += sc_min;
    stats.max_sc += sc_max;
    let (ac_min, ac_max) = equip_pair(CRYSTAL_STAT_MIN_AC, CRYSTAL_STAT_MAX_AC);
    stats.min_ac += ac_min;
    stats.max_ac += ac_max;
    let (mac_min, mac_max) = equip_pair(CRYSTAL_STAT_MIN_MAC, CRYSTAL_STAT_MAX_MAC);
    stats.min_mac += mac_min;
    stats.max_mac += mac_max;
    stats.accuracy += add(CRYSTAL_STAT_ACCURACY) + crystal_skill_accuracy_bonus(world);
    stats.agility += add(CRYSTAL_STAT_AGILITY);
    stats.luck += add(CRYSTAL_STAT_LUCK);
    stats.critical_rate += add(CRYSTAL_STAT_CRITICAL_RATE);
    stats.critical_damage += add(CRYSTAL_STAT_CRITICAL_DAMAGE);
    stats.magic_resist += add(CRYSTAL_STAT_MAGIC_RESIST);
    stats.poison_resist += add(CRYSTAL_STAT_POISON_RESIST);
    stats.reflect += add(CRYSTAL_STAT_REFLECT);
    stats.freezing += add(CRYSTAL_STAT_FREEZING);
    stats.poison_attack += add(CRYSTAL_STAT_POISON_ATTACK);
    stats.hp_drain_rate_percent += add(CRYSTAL_STAT_HP_DRAIN_RATE_PERCENT);
    stats.damage_reduction_percent += add(CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT);
    stats.attack_bonus += add(CRYSTAL_STAT_ATTACK_BONUS);
    apply_player_stat_caps(&mut stats);
    stats
}

/// Resolve a monster's combat stats from the Crystal monster manifest.
pub(super) fn monster_combat_stats(name: &str) -> CombatStats {
    let Some(monster) = crystal_monster_by_name(name) else {
        return CombatStats {
            accuracy: MONSTER_DEFAULT_ACCURACY,
            ..Default::default()
        };
    };
    CombatStats {
        min_dc: monster.min_dc,
        max_dc: monster.max_dc,
        min_mc: monster.min_mc,
        max_mc: monster.max_mc,
        min_sc: monster.min_sc,
        max_sc: monster.max_sc,
        min_ac: monster.min_ac,
        max_ac: monster.max_ac,
        min_mac: monster.min_mac,
        max_mac: monster.max_mac,
        accuracy: MONSTER_DEFAULT_ACCURACY,
        agility: monster.agility,
        ..Default::default()
    }
}

/// Resolve a monster's combat stats from its runtime entity. DC/AC/MAC come from
/// the manifest (by name); the agility used for the dodge roll is taken from the
/// per-entity [`MonsterCombatStats`] component, which carries the imported
/// per-monster value (the manifest does not export agility).
pub(super) fn monster_combat_stats_for_entity(world: &World, entity: Entity) -> CombatStats {
    let mut stats = match entity_name(world, entity) {
        Some(name) => monster_combat_stats(&name),
        None => CombatStats {
            accuracy: MONSTER_DEFAULT_ACCURACY,
            ..Default::default()
        },
    };
    if let Some(combat) = world.entity(entity).get::<MonsterCombatStats>() {
        stats.agility = combat.agility;
    }
    stats
}

/// Stable per-attack salt for the resolution-time rolls (hit, armour, crit),
/// decorrelated by attacker and target. The tick component is supplied
/// separately by the engine roll.
fn combat_salt(attacker_id: u32, target: Entity) -> usize {
    (attacker_id as usize)
        .wrapping_mul(2_654_435_761)
        .wrapping_add(target.index() as usize)
}

/// Crystal `ProcessPoison` armour/damage rate modifiers currently afflicting the
/// player (Red poison shrinks armour, Stun raises damage taken).
fn player_poison_rates(world: &World) -> (f32, f32) {
    let mut armour_rate = 1.0;
    let mut damage_rate = 1.0;
    if crystal_player_has_active_buff(world, YIMOOGI_RED_POISON_BUFF_KEY) {
        armour_rate -= CRYSTAL_RED_POISON_ARMOUR_RATE_DELTA;
    }
    if crystal_player_has_active_buff(world, MAN_TREE_STUN_BUFF_KEY) {
        damage_rate += CRYSTAL_STUN_POISON_DAMAGE_RATE_DELTA;
    }
    (armour_rate, damage_rate)
}

/// Crystal `ProcessPoison` armour/damage rate modifiers for a monster victim.
fn monster_poison_rates(world: &World, entity: Entity) -> (f32, f32) {
    let mut armour_rate = 1.0;
    let mut damage_rate = 1.0;
    if let Some(state) = world.entity(entity).get::<MonsterPoisonState>() {
        if state.poison & CRYSTAL_POISON_RED != 0 {
            armour_rate -= CRYSTAL_RED_POISON_ARMOUR_RATE_DELTA;
        }
        if state.poison & CRYSTAL_POISON_STUN != 0 {
            damage_rate += CRYSTAL_STUN_POISON_DAMAGE_RATE_DELTA;
        }
    }
    (armour_rate, damage_rate)
}

/// Crystal life steal (`HPDrainRatePercent`): a fraction of the net damage dealt
/// is accumulated and, once it crosses 2, the whole part is healed to the player.
fn apply_player_hp_drain(
    world: &mut World,
    attacker: &CombatStats,
    net_damage: i32,
    packets: &mut Vec<ServerPacket>,
) {
    if attacker.hp_drain_rate_percent <= 0 || net_damage <= 0 {
        return;
    }
    let heal = {
        let mut skills = world.resource_mut::<SkillResource>();
        skills.hp_drain +=
            (net_damage as f32 / 100.0) * attacker.hp_drain_rate_percent as f32;
        if skills.hp_drain > 2.0 {
            let gain = skills.hp_drain.floor();
            skills.hp_drain -= gain;
            gain as i32
        } else {
            0
        }
    };
    if heal <= 0 {
        return;
    }
    let Some(player) = player_entity(world) else {
        return;
    };
    let restored = {
        let mut entity = world.entity_mut(player);
        entity.get_mut::<PlayerVitals>().map(|mut vitals| {
            vitals.hp = (vitals.hp + heal).min(vitals.max_hp);
            *vitals
        })
    };
    if let Some(restored) = restored {
        world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored;
        if let Some(info) = object_health_info_for_entity(world, player, 0) {
            packets.push(ServerPacket::ObjectHealth { info });
        }
    }
}

/// Crystal `MapObject.ApplyNegativeEffects` for a player blow landing on a
/// monster: roll the attacker's Freezing / PoisonAttack gear stats against the
/// level offset and, on success, apply Slow / Green poison to the target.
///
/// The Green poison ticks damage via [`tick_monster_poisons`]; the Slow flag is
/// applied and broadcast (its movement penalty in the monster AI is a tracked
/// follow-up). Magic blows (`MAC` / `MACAgility`) never proc these effects.
fn apply_player_attack_negative_effects(
    world: &mut World,
    monster_entity: Entity,
    attacker: &CombatStats,
    defence_type: DefenceType,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    if matches!(defence_type, DefenceType::MAC | DefenceType::MACAgility) {
        return;
    }
    if attacker.poison_attack <= 0 && attacker.freezing <= 0 {
        return;
    }

    let player_level = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| i32::from(character.level))
        .unwrap_or(1);
    let monster_level = entity_name(world, monster_entity)
        .and_then(|name| crystal_monster_by_name(&name))
        .map(|monster| i32::from(monster.level))
        .unwrap_or(0);
    // Crystal levelOffset: 0 when the target out-levels the attacker, else the
    // capped difference. A 0 offset always passes the `rand(levelOffset) == 0`
    // gate (.NET `Random.Next(0)` returns 0).
    let level_offset = if monster_level > player_level {
        0
    } else {
        (player_level - monster_level).min(10)
    };
    let salt = monster_entity.index() as usize;
    let level_gate = |purpose: usize| -> bool {
        level_offset == 0
            || deterministic_roll(current_tick, salt, purpose, level_offset as u64) == 0
    };

    let mut poison_flags: u16 = 0;
    let mut green_damage = 0;
    let mut duration_ticks: u64 = 0;

    if attacker.poison_attack > 0
        && (deterministic_roll(current_tick, salt, 0xA1, POISON_ATTACK_WEIGHT as u64) as i64)
            < i64::from(attacker.poison_attack)
        && level_gate(0xA2)
    {
        poison_flags |= CRYSTAL_POISON_GREEN;
        green_damage = (3 + deterministic_roll(current_tick, salt, 0xA3, attacker.poison_attack as u64)
            as i32)
            .min(10);
        duration_ticks = duration_ticks.max(5);
    }

    if attacker.freezing > 0
        && (deterministic_roll(current_tick, salt, 0xB1, FREEZING_ATTACK_WEIGHT as u64) as i64)
            < i64::from(attacker.freezing)
        && level_gate(0xB2)
    {
        poison_flags |= CRYSTAL_POISON_SLOW;
        let slow_ticks =
            (3 + deterministic_roll(current_tick, salt, 0xB3, attacker.freezing as u64)).min(10);
        duration_ticks = duration_ticks.max(slow_ticks);
    }

    if poison_flags != 0 {
        apply_monster_poison(
            world,
            monster_entity,
            poison_flags,
            green_damage,
            current_tick,
            duration_ticks,
        );
        if let Some(object_id) = entity_object_id(world, monster_entity) {
            packets.push(ServerPacket::ObjectPoisoned {
                object_id,
                poison: poison_flags,
            });
        }
    }
}

/// Roll the player's melee attack power (`GetAttackPower(MinDC, MaxDC)` with
/// luck) and return it alongside the stat snapshot used to resolve the blow.
fn player_melee_attack_power(
    world: &World,
    current_tick: u64,
    target_object_id: u32,
) -> (CombatStats, i32) {
    let stats = player_combat_stats(world);
    let attacker_id = current_player_object_id(world).unwrap_or(0);
    let salt = (attacker_id as usize)
        .wrapping_mul(2_654_435_761)
        .wrapping_add(target_object_id as usize);
    let raw = get_attack_power(stats.luck, stats.min_dc, stats.max_dc, current_tick, salt);
    (stats, raw)
}

/// Run the Crystal `Attacked` pipeline for a blow landing on the player:
/// armour/dodge, critical, reflect, then HP loss. Returns the HP-loss outcome so
/// the caller can emit the struck/durability/counter-attack follow-ups.
fn resolve_attack_on_player(
    world: &mut World,
    player: Entity,
    action: &PendingCombatAction,
    profile: CombatAttackProfile,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> PlayerDamageOutcome {
    let player_stats = player_combat_stats(world);
    let (armour_rate, damage_rate) = player_poison_rates(world);
    let salt = combat_salt(action.attacker_id, player);
    let outcome = resolve_attacked(
        &profile.attacker,
        &player_stats,
        action.damage,
        profile.defence_type,
        armour_rate,
        damage_rate,
        current_tick,
        salt,
    );

    match outcome.no_damage {
        Some(NoDamageReason::Reflected) => {
            if outcome.reflect_damage > 0 {
                if let Some(attacker_entity) = entity_by_object_id(world, action.attacker_id) {
                    if monster_is_damageable(world, attacker_entity) {
                        let _ = damage_monster_entity(
                            world,
                            attacker_entity,
                            outcome.reflect_damage,
                            current_tick,
                            packets,
                        );
                    }
                }
            }
            if let Some(player_object_id) = current_player_object_id(world) {
                packets.push(object_effect_packet(
                    player_object_id,
                    CRYSTAL_SPELL_EFFECT_REFLECT,
                    0,
                ));
            }
            PlayerDamageOutcome {
                applied: false,
                died: false,
            }
        }
        Some(_) => {
            if let Some(player_object_id) = current_player_object_id(world) {
                packets.push(ServerPacket::DamageIndicator {
                    damage: 0,
                    damage_type: CRYSTAL_DAMAGE_TYPE_MISS,
                    object_id: player_object_id,
                });
            }
            PlayerDamageOutcome {
                applied: false,
                died: false,
            }
        }
        None => {
            if outcome.critical {
                if let Some(player_object_id) = current_player_object_id(world) {
                    packets.push(object_effect_packet(
                        player_object_id,
                        CRYSTAL_SPELL_EFFECT_CRITICAL,
                        0,
                    ));
                }
            }
            apply_player_hp_loss(world, outcome.net_damage, packets)
        }
    }
}

fn crystal_monster_agility(world: &World, monster_entity: Entity) -> i32 {
    world
        .entity(monster_entity)
        .get::<MonsterCombatStats>()
        .map(|stats| stats.agility.max(0))
        .unwrap_or(0)
}

fn crystal_player_hit_roll_succeeds(
    world: &World,
    attacker_id: u32,
    target_entity: Entity,
    current_tick: u64,
) -> bool {
    if Some(attacker_id) != current_player_object_id(world) {
        return true;
    }

    let agility = crystal_monster_agility(world, target_entity);
    if agility <= 0 {
        return true;
    }

    let roll = deterministic_roll(
        current_tick,
        usize::try_from(attacker_id).unwrap_or_default(),
        target_entity.index() as usize,
        u64::try_from(agility.saturating_add(1)).unwrap_or(1),
    );
    roll <= u64::try_from(crystal_player_accuracy(world).max(0)).unwrap_or(0)
}

fn queue_melee_passive_skill_progression(world: &mut World, due_tick: u64, tick: u64) {
    for (spell_name, spell) in [
        ("Fencing", Spell::Fencing),
        ("SpiritSword", Spell::SpiritSword),
    ] {
        let Some(magic) = crystal_magic_by_spell(spell_name) else {
            continue;
        };
        let key = normalize_crystal_skill_key(spell_name);
        let Some(index) = ({
            let skills = world.resource::<SkillResource>();
            skills.skills.iter().position(|skill| skill.key == key)
        }) else {
            continue;
        };

        for packet in advance_magic_progression(world, index, spell, &magic, tick) {
            queue_due_packet(world, due_tick, packet);
        }
    }
}

fn crystal_magic_damage_from_base(
    magic: &CrystalMagicTemplate,
    level: u8,
    base_damage: i32,
) -> i32 {
    let flat = crystal_magic_damage(magic, level);
    let level = i32::from(level) + 1;
    let multiplier = magic.multiplier_base + f32::from(level as u16 - 1) * magic.multiplier_bonus;
    ((base_damage.saturating_add(flat).max(1) as f32) * multiplier.max(0.1)).round() as i32
}

fn object_attack_packet_for_player(
    world: &World,
    player: Entity,
    location: &Point,
    direction: MirDirection,
    spell: Spell,
    level: u8,
) -> Option<ServerPacket> {
    Some(ServerPacket::ObjectAttack {
        info: ObjectAttackInfo {
            object_id: entity_object_id(world, player)?,
            location: location.clone(),
            direction,
            spell: spell as u8,
            level,
            attack_type: 0,
        },
    })
}

fn object_effect_packet(object_id: u32, effect: u8, effect_type: u32) -> ServerPacket {
    ServerPacket::ObjectEffect {
        info: ObjectEffectInfo {
            object_id,
            effect,
            effect_type,
            delay_time: 0,
            time: 0,
        },
    }
}

fn restore_player_mp(world: &mut World, amount: i32, packets: &mut Vec<ServerPacket>) {
    if amount <= 0 {
        return;
    }
    let Some(player) = player_entity(world) else {
        return;
    };
    let restored = {
        let mut entity = world.entity_mut(player);
        entity.get_mut::<PlayerVitals>().map(|mut vitals| {
            vitals.mp = (vitals.mp + amount).min(100);
            *vitals
        })
    };
    if let Some(restored) = restored {
        world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored;
        if let Some(info) = object_mana_info_for_entity(world, player) {
            packets.push(ServerPacket::ObjectMana { info });
        }
    }
}

fn gather_meditation_element_packet(world: &mut World, casted: bool) -> Option<ServerPacket> {
    let object_id = current_player_object_id(world)?;
    let mut elemental = world.resource_mut::<ElementalResource>();
    if elemental.has_elemental {
        return None;
    }
    elemental.has_elemental = true;
    elemental.elements_level = 50;
    Some(ServerPacket::SetElemental {
        object_id,
        enabled: true,
        casted,
        value: elemental.elements_level,
        element_type: 1,
        exp_last: 200,
    })
}

fn queue_counter_attack_proc(
    world: &mut World,
    current_tick: u64,
    attacker_id: u32,
    packets: &mut Vec<ServerPacket>,
) {
    if !skill_toggle_state(world, Spell::CounterAttack)
        || !world
            .resource::<BuffResource>()
            .buffs
            .iter()
            .any(|buff| buff.key == "counter-attack")
    {
        return;
    }

    let Some(player) = player_entity(world) else {
        return;
    };
    let Some(attacker) = entity_by_object_id(world, attacker_id) else {
        return;
    };
    if !monster_is_damageable(world, attacker) {
        return;
    }
    let Some(player_object_id) = entity_object_id(world, player) else {
        return;
    };
    let Some(player_position) = entity_position(world, player) else {
        return;
    };
    let Some(attacker_position) = entity_position(world, attacker) else {
        return;
    };
    if tile_distance(&player_position, &attacker_position) > 1 {
        return;
    }
    let Some((magic, level)) = crystal_skill_magic(world, "CounterAttack") else {
        return;
    };
    let threshold = u64::from(level).saturating_add(6).min(9);
    if deterministic_roll(
        current_tick,
        usize::try_from(attacker_id).unwrap_or_default(),
        usize::try_from(player_object_id).unwrap_or_default(),
        10,
    ) > threshold
    {
        return;
    }

    set_skill_toggle_state(world, Spell::CounterAttack, false);
    let direction = direction_toward(&player_position, &attacker_position)
        .unwrap_or_else(|| entity_facing(world, player).unwrap_or(MirDirection::Down));
    packets.push(ServerPacket::ObjectMagic {
        object_id: player_object_id,
        location: player_position.clone(),
        direction,
        spell: Spell::CounterAttack,
        target_id: attacker_id,
        target: attacker_position.clone(),
        cast: true,
        level,
        self_broadcast: false,
        secondary_target_ids: Vec::new(),
    });

    let damage = crystal_magic_damage_from_base(&magic, level, crystal_player_melee_damage(world));
    schedule_damage_to_monster(
        world,
        current_tick + melee_attack_delay_ticks(),
        player_object_id,
        attacker,
        damage,
        entity_name(world, attacker),
        None,
    );
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PendingCombatTarget {
    Player,
    Monster(Entity),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PendingPlayerStatusEffect {
    Paralysis {
        chance_denominator: u64,
        duration_ticks: u64,
    },
    Dazed {
        duration_ticks: u64,
    },
    StunPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    SlowAndFrozen {
        slow_chance_denominator: u64,
        slow_duration_ticks: u64,
        slow_salt: u64,
        frozen_chance_denominator: u64,
        frozen_duration_ticks: u64,
        frozen_salt: u64,
    },
    SlowAndParalysis {
        slow_chance_denominator: u64,
        slow_duration_ticks: u64,
        slow_salt: u64,
        paralysis_chance_denominator: u64,
        paralysis_duration_ticks: u64,
        paralysis_salt: u64,
    },
    FrozenPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    SlowPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    RedPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    WhiteFoxmanSlow {
        duration_ticks: u64,
    },
    BleedingPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    BlindnessPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    GreenPoison {
        chance_denominator: u64,
        duration_ticks: u64,
    },
    GreenPoisonAndParalysis {
        green_chance_denominator: u64,
        green_duration_ticks: u64,
        green_salt: u64,
        paralysis_chance_denominator: u64,
        paralysis_duration_ticks: u64,
        paralysis_salt: u64,
    },
}

#[derive(Debug, Clone)]
pub(super) struct PendingMonsterDefeatAction {
    pub(super) object_id: u32,
    pub(super) name: String,
}

#[derive(Debug, Clone)]
pub(super) struct PendingPlayerMovement {
    pub(super) direction: MirDirection,
    pub(super) distance: i32,
}

/// Snapshot of the attacker's combat stats taken at attack time, plus the
/// defence type of the blow. When present on a [`PendingCombatAction`], the
/// resolver runs the full Crystal `Attacked` pipeline (hit roll, armour
/// subtraction, critical, reflect) instead of applying `damage` verbatim.
#[derive(Debug, Clone, Copy)]
pub(super) struct CombatAttackProfile {
    pub(super) attacker: CombatStats,
    pub(super) defence_type: DefenceType,
}

#[derive(Debug, Clone)]
pub(super) struct PendingCombatAction {
    pub(super) due_tick: u64,
    pub(super) attacker_id: u32,
    pub(super) target: PendingCombatTarget,
    pub(super) damage: i32,
    pub(super) player_status_effect: Option<PendingPlayerStatusEffect>,
    pub(super) due_packet: Option<ServerPacket>,
    pub(super) player_movement: Option<PendingPlayerMovement>,
    pub(super) on_monster_defeat: Option<PendingMonsterDefeatAction>,
    /// When `Some`, `damage` is the raw rolled attack power and the resolver
    /// applies the target's armour/crit/reflect via [`resolve_attacked`].
    pub(super) attack_profile: Option<CombatAttackProfile>,
}

pub(super) fn queue_pending_combat_action(world: &mut World, action: PendingCombatAction) {
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_combat_actions
        .push(action);
}

pub(super) fn queue_due_packet(world: &mut World, due_tick: u64, packet: ServerPacket) {
    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id: 0,
            target: PendingCombatTarget::Player,
            damage: 0,
            player_status_effect: None,
            due_packet: Some(packet),
            player_movement: None,
            on_monster_defeat: None,
            attack_profile: None,
        },
    );
}

pub(super) fn schedule_heal_to_player(world: &mut World, due_tick: u64, heal: i32) {
    if heal <= 0 {
        return;
    }

    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id: 0,
            target: PendingCombatTarget::Player,
            damage: heal.saturating_neg(),
            player_status_effect: None,
            due_packet: None,
            player_movement: None,
            on_monster_defeat: None,
            attack_profile: None,
        },
    );
}

pub(super) fn schedule_damage_to_player(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    attacker_name: String,
    damage: i32,
) {
    schedule_damage_to_player_with_effect(
        world,
        due_tick,
        attacker_id,
        attacker_name,
        damage,
        None,
    );
}

pub(super) fn schedule_damage_to_player_with_effect(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    attacker_name: String,
    damage: i32,
    player_status_effect: Option<PendingPlayerStatusEffect>,
) {
    schedule_damage_to_player_with_effect_and_due_packet(
        world,
        due_tick,
        attacker_id,
        attacker_name,
        damage,
        player_status_effect,
        None,
    );
}

pub(super) fn schedule_damage_to_player_with_effect_and_due_packet(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    attacker_name: String,
    damage: i32,
    player_status_effect: Option<PendingPlayerStatusEffect>,
    due_packet: Option<ServerPacket>,
) {
    schedule_damage_to_player_with_effect_due_packet_and_movement(
        world,
        due_tick,
        attacker_id,
        attacker_name,
        damage,
        player_status_effect,
        due_packet,
        None,
    );
}

pub(super) fn schedule_damage_to_player_with_effect_due_packet_and_movement(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    _attacker_name: String,
    damage: i32,
    player_status_effect: Option<PendingPlayerStatusEffect>,
    due_packet: Option<ServerPacket>,
    player_movement: Option<PendingPlayerMovement>,
) {
    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id,
            target: PendingCombatTarget::Player,
            damage,
            player_status_effect,
            due_packet,
            player_movement,
            on_monster_defeat: None,
            attack_profile: None,
        },
    );
}

pub(super) fn schedule_player_status_effect(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    player_status_effect: PendingPlayerStatusEffect,
) {
    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id,
            target: PendingCombatTarget::Player,
            damage: 0,
            player_status_effect: Some(player_status_effect),
            due_packet: None,
            player_movement: None,
            on_monster_defeat: None,
            attack_profile: None,
        },
    );
}

pub(super) fn schedule_damage_to_monster(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    target_entity: Entity,
    damage: i32,
    target_name: Option<String>,
    defeat_action: Option<PendingMonsterDefeatAction>,
) {
    schedule_damage_to_monster_with_due_packet(
        world,
        due_tick,
        attacker_id,
        target_entity,
        damage,
        target_name,
        defeat_action,
        None,
    );
}

pub(super) fn schedule_damage_to_monster_with_due_packet(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    target_entity: Entity,
    damage: i32,
    _target_name: Option<String>,
    defeat_action: Option<PendingMonsterDefeatAction>,
    due_packet: Option<ServerPacket>,
) {
    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id,
            target: PendingCombatTarget::Monster(target_entity),
            damage,
            player_status_effect: None,
            due_packet,
            player_movement: None,
            on_monster_defeat: defeat_action,
            attack_profile: None,
        },
    );
}

/// Queue a player melee/skill blow against a monster that should resolve through
/// the full Crystal `Attacked` pipeline. `raw_damage` is the attacker's rolled
/// attack power; the target's armour, critical and reflect are applied at
/// resolution from `attacker_stats` + `defence_type`.
#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_player_attack_on_monster(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    target_entity: Entity,
    raw_damage: i32,
    defeat_action: Option<PendingMonsterDefeatAction>,
    due_packet: Option<ServerPacket>,
    attacker_stats: CombatStats,
    defence_type: DefenceType,
) {
    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id,
            target: PendingCombatTarget::Monster(target_entity),
            damage: raw_damage,
            player_status_effect: None,
            due_packet,
            player_movement: None,
            on_monster_defeat: defeat_action,
            attack_profile: Some(CombatAttackProfile {
                attacker: attacker_stats,
                defence_type,
            }),
        },
    );
}

pub(super) fn apply_monster_poison(
    world: &mut World,
    monster_entity: Entity,
    poison: u16,
    green_damage: i32,
    current_tick: u64,
    duration_ticks: u64,
) {
    if poison == 0 || duration_ticks == 0 {
        return;
    }

    world.entity_mut(monster_entity).insert(MonsterPoisonState {
        poison,
        green_damage: green_damage.max(0),
        next_damage_tick: current_tick.saturating_add(2),
        expires_at_tick: current_tick.saturating_add(duration_ticks),
    });
}

pub(super) fn tick_monster_poisons(
    world: &mut World,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    const GREEN_POISON: u16 = 1;

    let poisoned_entities = {
        #[allow(deprecated)]
        world
            .iter_entities()
            .filter_map(|entity| {
                let state = *entity.get::<MonsterPoisonState>()?;
                Some((entity.id(), state))
            })
            .collect::<Vec<_>>()
    };

    for (entity, mut state) in poisoned_entities {
        let object_id = entity_object_id(world, entity);
        let dead = world
            .entity(entity)
            .get::<MonsterAgent>()
            .map(|agent| agent.dead)
            .unwrap_or(true);
        if dead || current_tick >= state.expires_at_tick {
            world.entity_mut(entity).remove::<MonsterPoisonState>();
            if let Some(object_id) = object_id {
                packets.push(ServerPacket::ObjectPoisoned {
                    object_id,
                    poison: 0,
                });
            }
            continue;
        }

        if state.poison & GREEN_POISON != 0
            && state.green_damage > 0
            && current_tick >= state.next_damage_tick
        {
            let died =
                damage_monster_entity(world, entity, state.green_damage, current_tick, packets);
            if died {
                world.entity_mut(entity).remove::<MonsterPoisonState>();
                continue;
            }
            state.next_damage_tick = current_tick.saturating_add(2);
            world.entity_mut(entity).insert(state);
        }
    }
}

pub(super) fn tick_player_status_effects(
    world: &mut World,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    if current_tick % CRYSTAL_PLAYER_STATUS_DAMAGE_TICK_INTERVAL != 0 {
        return;
    }

    let mut damage = 0;
    if crystal_player_has_active_buff(world, TOXIC_GHOUL_GREEN_POISON_BUFF_KEY) {
        damage += CRYSTAL_PLAYER_GREEN_POISON_TICK_DAMAGE;
    }
    if crystal_player_has_active_buff(world, FROST_TIGER_BLEEDING_BUFF_KEY) {
        damage += CRYSTAL_PLAYER_BLEEDING_TICK_DAMAGE;
    }

    if damage > 0 {
        let _ = apply_damage_to_current_player(world, damage, packets);
    }
}

pub(super) fn combat_delay_ticks(delay_ms: u64) -> u64 {
    delay_ms.max(1).div_ceil(1_000)
}

pub(super) fn melee_attack_delay_ticks() -> u64 {
    combat_delay_ticks(300)
}

pub(super) fn queued_before_world_tick_due_tick(current_tick: u64, delay_ticks: u64) -> u64 {
    current_tick + delay_ticks + 1
}

pub(super) fn ranged_attack_delay_ticks(source: &Point, target: &Point) -> u64 {
    let distance =
        u64::try_from(tile_distance(source, target).max(0)).expect("tile distance should fit u64");
    combat_delay_ticks(distance * 50 + 500)
}

pub(super) fn tucson_mage_wide_line_delay_ticks(
    source: &Point,
    target: &Point,
    direction: MirDirection,
) -> u64 {
    if *target == offset_point(source, direction, 1) {
        combat_delay_ticks(300)
    } else {
        ranged_attack_delay_ticks(source, target)
    }
}

pub(super) fn manectric_king_mass_attack_delay_ticks(source: &Point, target: &Point) -> u64 {
    let distance =
        u64::try_from(tile_distance(source, target).max(0)).expect("tile distance should fit u64");
    combat_delay_ticks(distance * 50 + 750)
}

pub(super) fn deterministic_chance_roll(
    current_tick: u64,
    attacker_id: u32,
    salt: u64,
    denominator: u64,
) -> bool {
    if denominator <= 1 {
        return true;
    }

    let value = current_tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(attacker_id).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    value % denominator == 0
}

pub(super) fn apply_player_paralysis(world: &mut World, current_tick: u64, duration_ticks: u64) {
    apply_or_refresh_buff(
        world,
        BuffState {
            key: CAVE_MAGGOT_PARALYSIS_BUFF_KEY.to_string(),
            name: "Paralysis".to_string(),
            description: "Movement is stopped by paralysis poison.".to_string(),
            expires_at_tick: current_tick + duration_ticks,
            attack_bonus: 0,
            defence_bonus: 0,
            stats: Vec::new(),
        },
    );
}

pub(super) fn apply_player_red_poison(world: &mut World, current_tick: u64, duration_ticks: u64) {
    apply_or_refresh_buff(
        world,
        BuffState {
            key: YIMOOGI_RED_POISON_BUFF_KEY.to_string(),
            name: "Red Poison".to_string(),
            description: "Crystal red poison is active.".to_string(),
            expires_at_tick: current_tick + duration_ticks,
            attack_bonus: 0,
            defence_bonus: 0,
            stats: Vec::new(),
        },
    );
}

pub(super) fn apply_pending_player_status_effect(
    world: &mut World,
    current_tick: u64,
    attacker_id: u32,
    effect: PendingPlayerStatusEffect,
) {
    match effect {
        PendingPlayerStatusEffect::Paralysis {
            chance_denominator,
            duration_ticks,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, 7, chance_denominator) {
                return;
            }

            apply_player_paralysis(world, current_tick, duration_ticks);
        }
        PendingPlayerStatusEffect::Dazed { duration_ticks } => {
            apply_or_refresh_buff(
                world,
                BuffState {
                    key: HELL_KEEPER_DAZED_BUFF_KEY.to_string(),
                    name: "Dazed".to_string(),
                    description: "Crystal dazed poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::StunPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: MAN_TREE_STUN_BUFF_KEY.to_string(),
                    name: "Stun".to_string(),
                    description: "Crystal stun poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::SlowAndFrozen {
            slow_chance_denominator,
            slow_duration_ticks,
            slow_salt,
            frozen_chance_denominator,
            frozen_duration_ticks,
            frozen_salt,
        } => {
            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                slow_salt,
                slow_chance_denominator,
            ) {
                apply_or_refresh_buff(
                    world,
                    BuffState {
                        key: ICE_GUARD_SLOW_BUFF_KEY.to_string(),
                        name: "Slow".to_string(),
                        description: "Crystal slow poison is active.".to_string(),
                        expires_at_tick: current_tick + slow_duration_ticks,
                        attack_bonus: 0,
                        defence_bonus: 0,
                        stats: Vec::new(),
                    },
                );
            }

            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                frozen_salt,
                frozen_chance_denominator,
            ) {
                apply_or_refresh_buff(
                    world,
                    BuffState {
                        key: ICE_GUARD_FROZEN_BUFF_KEY.to_string(),
                        name: "Frozen".to_string(),
                        description: "Crystal frozen poison is active.".to_string(),
                        expires_at_tick: current_tick + frozen_duration_ticks,
                        attack_bonus: 0,
                        defence_bonus: 0,
                        stats: Vec::new(),
                    },
                );
            }
        }
        PendingPlayerStatusEffect::SlowAndParalysis {
            slow_chance_denominator,
            slow_duration_ticks,
            slow_salt,
            paralysis_chance_denominator,
            paralysis_duration_ticks,
            paralysis_salt,
        } => {
            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                slow_salt,
                slow_chance_denominator,
            ) {
                apply_or_refresh_buff(
                    world,
                    BuffState {
                        key: ICE_GUARD_SLOW_BUFF_KEY.to_string(),
                        name: "Slow".to_string(),
                        description: "Crystal slow poison is active.".to_string(),
                        expires_at_tick: current_tick + slow_duration_ticks,
                        attack_bonus: 0,
                        defence_bonus: 0,
                        stats: Vec::new(),
                    },
                );
            }

            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                paralysis_salt,
                paralysis_chance_denominator,
            ) {
                apply_player_paralysis(world, current_tick, paralysis_duration_ticks);
            }
        }
        PendingPlayerStatusEffect::FrozenPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: ICE_GUARD_FROZEN_BUFF_KEY.to_string(),
                    name: "Frozen".to_string(),
                    description: "Crystal frozen poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::SlowPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: ICE_GUARD_SLOW_BUFF_KEY.to_string(),
                    name: "Slow".to_string(),
                    description: "Crystal slow poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::RedPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_player_red_poison(world, current_tick, duration_ticks);
        }
        PendingPlayerStatusEffect::WhiteFoxmanSlow { duration_ticks } => {
            let level = world
                .resource::<SessionResource>()
                .selected_character
                .as_ref()
                .map(|character| i32::from(character.level))
                .unwrap_or(1);
            let threshold = (4 + (50 - level)).clamp(0, 20) as u64;
            if threshold == 0 {
                return;
            }
            if threshold < 20 {
                let value = current_tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ u64::from(attacker_id).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                    ^ 46_u64.wrapping_mul(0x94D0_49BB_1331_11EB);
                if value % 20 >= threshold {
                    return;
                }
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: ICE_GUARD_SLOW_BUFF_KEY.to_string(),
                    name: "Slow".to_string(),
                    description: "Crystal slow poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::BleedingPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: FROST_TIGER_BLEEDING_BUFF_KEY.to_string(),
                    name: "Bleeding".to_string(),
                    description: "Crystal bleeding poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::BlindnessPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: RESTLESS_JAR_BLINDNESS_BUFF_KEY.to_string(),
                    name: "Blindness".to_string(),
                    description: "Crystal blindness poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::GreenPoison {
            chance_denominator,
            duration_ticks,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, 28, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: TOXIC_GHOUL_GREEN_POISON_BUFF_KEY.to_string(),
                    name: "Green Poison".to_string(),
                    description: "Crystal green poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::GreenPoisonAndParalysis {
            green_chance_denominator,
            green_duration_ticks,
            green_salt,
            paralysis_chance_denominator,
            paralysis_duration_ticks,
            paralysis_salt,
        } => {
            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                green_salt,
                green_chance_denominator,
            ) {
                apply_or_refresh_buff(
                    world,
                    BuffState {
                        key: TOXIC_GHOUL_GREEN_POISON_BUFF_KEY.to_string(),
                        name: "Green Poison".to_string(),
                        description: "Crystal green poison is active.".to_string(),
                        expires_at_tick: current_tick + green_duration_ticks,
                        attack_bonus: 0,
                        defence_bonus: 0,
                        stats: Vec::new(),
                    },
                );
            }

            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                paralysis_salt,
                paralysis_chance_denominator,
            ) {
                apply_player_paralysis(world, current_tick, paralysis_duration_ticks);
            }
        }
    }
}

pub(super) fn resolve_pending_combat_actions(
    world: &mut World,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let due_actions = {
        let mut resources = world.resource_mut::<RuntimeQueueResource>();
        let mut due = Vec::new();
        let mut pending = Vec::with_capacity(resources.pending_combat_actions.len());

        for action in resources.pending_combat_actions.drain(..) {
            if action.due_tick <= current_tick {
                due.push(action);
            } else {
                pending.push(action);
            }
        }

        resources.pending_combat_actions = pending;
        due
    };

    for action in due_actions {
        match action.target {
            PendingCombatTarget::Player => {
                let Some(player) = player_entity(world) else {
                    continue;
                };
                if action.damage > 0 {
                    let outcome = if let Some(profile) = action.attack_profile {
                        resolve_attack_on_player(world, player, &action, profile, current_tick, packets)
                    } else {
                        apply_damage_to_current_player(world, action.damage, packets)
                    };
                    if outcome.applied {
                        packets.push(player_struck_packet(action.attacker_id));
                        packets.extend(damage_worn_durability(world, current_tick));
                        if !outcome.died {
                            queue_counter_attack_proc(
                                world,
                                current_tick,
                                action.attacker_id,
                                packets,
                            );
                        }
                    }
                } else if action.damage < 0 {
                    let restored = {
                        let mut entity = world.entity_mut(player);
                        entity.get_mut::<PlayerVitals>().map(|mut vitals| {
                            let heal = action.damage.saturating_neg();
                            vitals.hp = (vitals.hp + heal).min(vitals.max_hp);
                            *vitals
                        })
                    };
                    if let Some(restored) = restored {
                        world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored;
                        if let Some(info) = object_health_info_for_entity(world, player, 0) {
                            packets.push(ServerPacket::ObjectHealth { info });
                        }
                    }
                }
                if let Some(effect) = action.player_status_effect {
                    apply_pending_player_status_effect(
                        world,
                        current_tick,
                        action.attacker_id,
                        effect,
                    );
                }
                if let Some(packet) = action.due_packet {
                    packets.push(packet);
                }
                if let Some(movement) = action.player_movement {
                    let _ = push_player_in_direction(
                        world,
                        player,
                        movement.direction,
                        movement.distance,
                        packets,
                    );
                }
            }
            PendingCombatTarget::Monster(target_entity) => {
                if !monster_is_damageable(world, target_entity) {
                    continue;
                }
                // Resolve net damage either through the full Crystal `Attacked`
                // pipeline (when the attacker captured a profile) or the legacy
                // raw-damage + agility hit-roll path (skills/effects not yet
                // migrated).
                let (net_damage, missed, critical) = if let Some(profile) = action.attack_profile {
                    let target_stats = monster_combat_stats_for_entity(world, target_entity);
                    let (armour_rate, damage_rate) = monster_poison_rates(world, target_entity);
                    let salt = combat_salt(action.attacker_id, target_entity);
                    let outcome = resolve_attacked(
                        &profile.attacker,
                        &target_stats,
                        action.damage,
                        profile.defence_type,
                        armour_rate,
                        damage_rate,
                        current_tick,
                        salt,
                    );
                    (
                        outcome.net_damage,
                        outcome.no_damage.is_some(),
                        outcome.critical,
                    )
                } else if action.damage > 0
                    && !crystal_player_hit_roll_succeeds(
                        world,
                        action.attacker_id,
                        target_entity,
                        current_tick,
                    )
                {
                    (0, true, false)
                } else {
                    (action.damage, false, false)
                };

                if missed {
                    if let Some(object_id) = entity_object_id(world, target_entity) {
                        packets.push(ServerPacket::DamageIndicator {
                            damage: 0,
                            damage_type: CRYSTAL_DAMAGE_TYPE_MISS,
                            object_id,
                        });
                    }
                    continue;
                }
                if let Some(packet) = action.due_packet {
                    packets.push(packet);
                }
                let ignores_damage = monster_ignores_damage(world, target_entity);
                if let Some(packet) = object_struck_packet(world, target_entity, action.attacker_id)
                {
                    packets.push(packet);
                }
                if ignores_damage {
                    continue;
                }
                if critical {
                    if let Some(object_id) = entity_object_id(world, target_entity) {
                        packets.push(object_effect_packet(
                            object_id,
                            CRYSTAL_SPELL_EFFECT_CRITICAL,
                            0,
                        ));
                        packets.push(ServerPacket::DamageIndicator {
                            damage: net_damage,
                            damage_type: CRYSTAL_DAMAGE_TYPE_CRITICAL,
                            object_id,
                        });
                    }
                }
                if let Some(profile) = action.attack_profile {
                    apply_player_attack_negative_effects(
                        world,
                        target_entity,
                        &profile.attacker,
                        profile.defence_type,
                        current_tick,
                        packets,
                    );
                    apply_player_hp_drain(world, &profile.attacker, net_damage, packets);
                }
                let monster_dead = damage_monster_entity(
                    world,
                    target_entity,
                    net_damage,
                    current_tick,
                    packets,
                );
                if monster_dead {
                    if let Some(defeat_action) = action.on_monster_defeat {
                        handle_monster_defeat(
                            world,
                            defeat_action.object_id,
                            &defeat_action.name,
                            packets,
                        );
                    }
                }
            }
        }
    }
}

pub(super) fn trainer_dps(total_damage: i32, start_tick: u64, end_tick: u64) -> f64 {
    let elapsed_ticks = end_tick.saturating_sub(start_tick).max(1);
    f64::from(total_damage) / elapsed_ticks as f64
}

pub(super) fn trainer_damage_chat(
    world: &World,
    damage: i32,
    state: &TrainerDamageState,
) -> ServerPacket {
    let dps = trainer_dps(state.total_damage, state.start_tick, state.last_attack_tick);
    let language = world.resource::<SessionResource>().language;
    let actor = localized_text_or_fallback(language, "server.You", "You");
    ServerPacket::Chat {
        message: format_localized_text(
            language,
            "server.PetInflictedDamageDps",
            [
                damage.to_string(),
                "Physical Agility".to_string(),
                format!("{dps:.2}"),
                actor,
            ],
        ),
        chat_type: ChatType::Trainer,
    }
}

pub(super) fn trainer_average_chat(
    world: &World,
    state: &TrainerDamageState,
) -> Option<ServerPacket> {
    if state.hit_count == 0 {
        return None;
    }

    let average_damage = state.total_damage / i32::try_from(state.hit_count).ok()?.max(1);
    let dps = trainer_dps(state.total_damage, state.start_tick, state.last_attack_tick);
    Some(ServerPacket::Chat {
        message: format_localized_text(
            world.resource::<SessionResource>().language,
            "server.AverageDamageOnTrainer",
            [average_damage.to_string(), format!("{dps:.2}")],
        ),
        chat_type: ChatType::Trainer,
    })
}

pub(super) fn record_trainer_damage(
    world: &mut World,
    monster_entity: Entity,
    damage: i32,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    if damage <= 0 {
        return;
    }

    let mut state = world
        .entity(monster_entity)
        .get::<TrainerDamageState>()
        .copied()
        .unwrap_or_default();
    if state.hit_count == 0 {
        state.start_tick = current_tick;
        state.total_damage = 0;
    }
    state.hit_count = state.hit_count.saturating_add(1);
    state.total_damage = state.total_damage.saturating_add(damage);
    state.last_attack_tick = current_tick;

    world.entity_mut(monster_entity).insert(state);
    packets.push(trainer_damage_chat(world, damage, &state));
}

pub(super) fn emit_due_trainer_average_chats(
    world: &mut World,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let mut due = Vec::new();
    {
        let mut query =
            world.query_filtered::<(Entity, &TrainerDamageState), bevy_ecs::query::With<Monster>>();
        for (entity, state) in query.iter(world) {
            if state.hit_count > 0
                && current_tick > state.last_attack_tick + TRAINER_DAMAGE_REPORT_IDLE_TICKS
            {
                due.push((entity, *state));
            }
        }
    }

    for (entity, state) in due {
        if let Some(packet) = trainer_average_chat(world, &state) {
            packets.push(packet);
        }
        world
            .entity_mut(entity)
            .insert(TrainerDamageState::default());
    }
}

pub(super) fn damage_monster_entity(
    world: &mut World,
    monster_entity: Entity,
    damage: i32,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if damage <= 0 {
        return false;
    }

    let spawn_ref = world.entity(monster_entity).get::<SpawnSlotRef>().copied();
    let summoned_state = world
        .entity(monster_entity)
        .get::<SummonedMonster>()
        .copied();
    let position = entity_position(world, monster_entity);
    let name = entity_name(world, monster_entity);
    let (current_hp, max_hp, mut agent, ai_state) = {
        let monster = world.entity(monster_entity);
        let vitals = monster.get::<MonsterVitals>().expect("monster vitals");
        let agent = monster.get::<MonsterAgent>().expect("monster agent");
        let ai_state = monster.get::<MonsterAiState>().copied().unwrap_or_default();
        (vitals.hp, vitals.max_hp, agent.clone(), ai_state)
    };

    if agent.dead || current_hp <= 0 {
        return false;
    }

    if agent.ai == 56 {
        record_trainer_damage(world, monster_entity, damage, current_tick, packets);
        return false;
    }

    if ignores_monster_damage(&agent) || (agent.ai == 98 && ai_state.extra_byte < 4) {
        return false;
    }

    if agent.ai == 54 {
        if ai_state.mode {
            return false;
        }
        if damage >= current_hp {
            let mut sleep_state = ai_state;
            sleep_state.mode = true;
            sleep_state.next_state_tick = current_tick + DRAGON_STATUE_SLEEP_DURATION_TICKS;
            agent.tracking_player = false;
            agent.next_attack_tick = current_tick + 1;
            agent.next_move_tick = current_tick + 1;
            world.entity_mut(monster_entity).insert((
                MonsterVitals { hp: 0, max_hp },
                agent,
                sleep_state,
            ));
            if let Some(info) = object_health_info_for_entity(world, monster_entity, 0) {
                packets.push(ServerPacket::ObjectHealth { info });
            }
            return false;
        }
    }

    if agent.ai == 47 && summoned_state.is_some() {
        if let Some(summoned) = summoned_state {
            if let Some(parent) = entity_by_object_id(world, summoned.summoner_object_id) {
                if let Some(mut parent_ai_state) =
                    world.entity_mut(parent).get_mut::<MonsterAiState>()
                {
                    parent_ai_state.mode = false;
                }
            }
        }
        return false;
    }

    let trap_rock_first_hit_collapse = agent.ai == 47 && !ai_state.hidden && ai_state.mode;
    let mut next_ai_state = ai_state;
    if trap_rock_first_hit_collapse {
        next_ai_state.mode = false;
    }
    if matches!(agent.ai, 124 | 125) && next_ai_state.mode {
        if let Some(object_id) = entity_object_id(world, monster_entity) {
            if deterministic_chance_roll(current_tick, object_id, 1240, 4) {
                next_ai_state.mode = false;
            }
        }
    }

    let general_meow_meow_shield_armour = if agent.ai == 123
        && world
            .entity(monster_entity)
            .get::<GeneralMeowMeowState>()
            .map(|state| state.shield_until_tick > current_tick)
            .unwrap_or(false)
    {
        GENERAL_MEOW_MEOW_SHIELD_ARMOUR
    } else {
        0
    };

    let applied_damage = if trap_rock_first_hit_collapse {
        current_hp
    } else if matches!(agent.ai, 3 | 128) {
        1
    } else {
        damage.saturating_sub(general_meow_meow_shield_armour)
    };
    if applied_damage <= 0 {
        return false;
    }
    let next_hp = current_hp.saturating_sub(applied_damage).max(0);
    let monster_dead = next_hp == 0;
    let death_agent = agent.clone();
    agent.dead = monster_dead;

    let dead_ai = agent.ai;
    world.entity_mut(monster_entity).insert((
        MonsterVitals {
            hp: next_hp,
            max_hp,
        },
        agent,
        next_ai_state,
    ));

    let should_despawn_summoned = monster_dead
        && spawn_ref.is_none()
        && dead_ai != 60
        && summoned_state
            .map(|summoned| summoned.despawn_tick_after_death.is_none())
            .unwrap_or(false);
    if monster_dead {
        if let Some(spawn_ref) = spawn_ref {
            schedule_monster_respawn(world, spawn_ref, current_tick);
        }
        if dead_ai == 25 && ai_state.extra_byte < REVIVING_ZOMBIE_MAX_REVIVALS {
            let mut revival_state = ai_state;
            revival_state.mode = true;
            revival_state.next_state_tick = current_tick + REVIVING_ZOMBIE_REVIVE_DELAY_TICKS;
            world.entity_mut(monster_entity).insert(revival_state);
        }
    }

    if let Some(info) = object_health_info_for_entity(world, monster_entity, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    if monster_dead {
        if let Some(info) = object_died_info_for_entity(world, monster_entity, 0) {
            packets.push(ServerPacket::ObjectDied { info });
        }
        if let Some(monster_name) = name.as_deref() {
            packets.extend(advance_crystal_quest_kill(world, monster_name));
        }
    }

    if monster_dead && dead_ai == 60 {
        if let Some(position) = position.as_ref() {
            explode_vampire_spider(world, monster_entity, position, current_tick, packets);
        }
    }
    if monster_dead && dead_ai == 119 {
        if let Some(position) = position.as_ref() {
            complete_jar1_death(world, monster_entity, position, current_tick);
        }
    }
    if monster_dead && dead_ai == 128 {
        if let Some(position) = position.as_ref() {
            complete_tucson_egg_death(world, monster_entity, position, current_tick);
        }
    }
    if monster_dead && dead_ai == 97 {
        if let Some(summoned) = summoned_state {
            advance_hell_lord_stage(world, summoned.summoner_object_id, current_tick, packets);
        }
    }
    if monster_dead && dead_ai == 50 {
        if let Some(position) = position.as_ref() {
            set_guardian_rocks_active_near(world, position, false);
        }
    }
    if monster_dead && dead_ai == 180 {
        if let (Some(position), Some(name)) = (position.as_ref(), name.as_deref()) {
            schedule_snow_wolf_king_death_explosion(
                world,
                monster_entity,
                &death_agent,
                position,
                name,
                current_tick,
            );
        }
    }
    if monster_dead && dead_ai == 63 {
        if let Some(position) = position {
            explode_charmed_snake(world, monster_entity, &position, current_tick);
        }
    }
    if should_despawn_summoned {
        let _ = world.despawn(monster_entity);
    }

    let _ = name;
    monster_dead
}

pub(super) fn crystal_jar1_slave_template(
    monster_name: &str,
    monster_entity: Entity,
    current_tick: u64,
) -> Option<CrystalRespawnTemplate> {
    let parent = crystal_monster_by_name(monster_name)?;
    let minimum_level = parent.level.saturating_sub(10);
    let valid_monsters: Vec<_> = crystal_monster_manifest()
        .monsters
        .into_iter()
        .filter(|monster| monster.level <= parent.level && monster.level >= minimum_level)
        .filter(|monster| !monster.is_boss)
        .filter(|monster| !matches!(monster.ai, 72 | 73 | 80 | 81 | 82))
        .collect();

    let index = deterministic_roll(
        current_tick,
        monster_entity.index() as usize,
        usize::from(parent.ai),
        valid_monsters.len() as u64,
    ) as usize;
    valid_monsters
        .get(index)
        .cloned()
        .map(crystal_respawn_template_from_monster)
}

pub(super) fn complete_jar1_death(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
) {
    let name = entity_name(world, monster_entity).unwrap_or_else(|| "Jar1".to_string());
    let Some(template) = crystal_jar1_slave_template(&name, monster_entity, current_tick) else {
        return;
    };
    let target_entity = world
        .entity(monster_entity)
        .get::<MonsterAgent>()
        .filter(|agent| agent.tracking_player && agent.hostile_to_player)
        .and_then(|_| player_entity(world));

    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: current_tick + JAR1_DEATH_SPAWN_DELAY_TICKS,
            summoner_entity: monster_entity,
            template: CrystalRespawnTemplate {
                location: position.clone(),
                ..template
            },
            target_entity,
            summon_metadata: None,
            hostile_to_player_override: Some(true),
        },
    );
}

pub(super) fn complete_tucson_egg_death(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let name = entity_name(world, monster_entity).unwrap_or_else(|| "TucsonEgg".to_string());
    let damage = crystal_monster_raw_attack_damage(&name);
    if damage > 0 {
        if let Some(player) = player_entity(world) {
            if let Some(player_position) = entity_position(world, player) {
                if tile_distance(position, &player_position) <= 1 {
                    schedule_damage_to_player_with_effect(
                        world,
                        current_tick + TUCSON_EGG_DEATH_DELAY_TICKS,
                        attacker_id,
                        name.clone(),
                        damage,
                        Some(PendingPlayerStatusEffect::GreenPoison {
                            chance_denominator: TUCSON_EGG_GREEN_POISON_CHANCE_DENOMINATOR,
                            duration_ticks: TUCSON_EGG_GREEN_POISON_DURATION_TICKS,
                        }),
                    );
                }
            }
        }
    }

    if crystal_monster_effect_for_name(&name) != 1 {
        return;
    }
    let Some(template) = crystal_dynamic_monster_template("GeneralTucson")
        .or_else(|| crystal_dynamic_monster_template("TucsonGeneral"))
    else {
        return;
    };
    let direction = entity_facing(world, monster_entity).unwrap_or(MirDirection::Down);
    let spawn_position =
        directional_destination(world, position, direction, 1, Some(monster_entity))
            .unwrap_or_else(|| position.clone());
    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: current_tick + TUCSON_EGG_DEATH_DELAY_TICKS,
            summoner_entity: monster_entity,
            template: CrystalRespawnTemplate {
                location: spawn_position,
                ..template
            },
            target_entity: player_entity(world),
            summon_metadata: None,
            hostile_to_player_override: Some(true),
        },
    );
}

pub(super) fn advance_hell_lord_stage(
    world: &mut World,
    lord_object_id: u32,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(lord_entity) = entity_by_object_id(world, lord_object_id) else {
        return;
    };

    {
        let mut entry = world.entity_mut(lord_entity);
        let Some(agent) = entry.get::<MonsterAgent>() else {
            return;
        };
        if agent.ai != 98 || agent.dead {
            return;
        }
        let mut ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if ai_state.extra_byte >= 4 {
            return;
        }

        ai_state.extra_byte += 1;
        ai_state.mode = true;
        ai_state.next_state_tick = current_tick + HELL_LORD_RAGE_DELAY_TICKS;
        entry.insert(ai_state);
    }

    if let Some((_, bundle)) = visible_object_bundle_for_entity(
        world,
        lord_entity,
        world.resource::<SessionResource>().language,
    ) {
        if matches!(bundle.spawn_packet, ServerPacket::ObjectMonster { .. }) {
            packets.push(bundle.spawn_packet);
        }
    }
}

pub(super) fn explode_hell_bomb(
    world: &mut World,
    monster_entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let attacker_name =
        entity_name(world, monster_entity).unwrap_or_else(|| "HellBomb".to_string());
    let attacker_hostile_to_player = agent.hostile_to_player;

    if let Some(packet) =
        monster_melee_attack_packet(world, monster_entity, position, MirDirection::Up)
    {
        packets.push(packet);
    }

    agent.dead = true;
    let max_hp = world
        .entity(monster_entity)
        .get::<MonsterVitals>()
        .map(|vitals| vitals.max_hp)
        .unwrap_or(1);
    {
        let mut entry = world.entity_mut(monster_entity);
        entry.insert((
            MonsterVitals { hp: 0, max_hp },
            agent.clone(),
            Facing(MirDirection::Up),
        ));
        if let Some(mut summoned) = entry.get_mut::<SummonedMonster>() {
            summoned.despawn_tick_after_death =
                Some(current_tick + HELL_BOMB_EXPLOSION_DELAY_TICKS + 1);
        }
    }

    if let Some(info) = object_health_info_for_entity(world, monster_entity, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    if let Some(info) = object_died_info_for_entity(world, monster_entity, 0) {
        packets.push(ServerPacket::ObjectDied { info });
    }

    schedule_hell_bomb_explosion_damage(
        world,
        monster_entity,
        position,
        current_tick,
        attacker_id,
        attacker_name,
        attacker_hostile_to_player,
    );
}

pub(super) fn schedule_hell_bomb_explosion_damage(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
    attacker_id: u32,
    attacker_name: String,
    attacker_hostile_to_player: bool,
) {
    let damage = crystal_monster_attack_damage(&attacker_name);
    let due_tick = current_tick + HELL_BOMB_EXPLOSION_DELAY_TICKS;
    let player_status_effect = hell_bomb_player_status_effect(&attacker_name);

    if attacker_hostile_to_player {
        if let Some(player) = player_entity(world) {
            if let Some(player_position) = entity_position(world, player) {
                if tile_distance(position, &player_position) <= HELL_BOMB_EXPLOSION_RADIUS {
                    schedule_damage_to_player_with_effect(
                        world,
                        due_tick,
                        attacker_id,
                        attacker_name.clone(),
                        damage,
                        player_status_effect,
                    );
                }
            }
        }
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    for target_entity in monster_entities {
        if target_entity == monster_entity {
            continue;
        }
        let entry = world.entity(target_entity);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_hostile_to_player == target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if tile_distance(position, &target_position) > HELL_BOMB_EXPLOSION_RADIUS {
            continue;
        }

        schedule_damage_to_monster(
            world,
            due_tick,
            attacker_id,
            target_entity,
            damage,
            None,
            None,
        );
    }
}

pub(super) fn hell_bomb_player_status_effect(name: &str) -> Option<PendingPlayerStatusEffect> {
    match name {
        "HellBomb1" => Some(PendingPlayerStatusEffect::FrozenPoison {
            chance_denominator: 1,
            duration_ticks: HELL_BOMB_POISON_DURATION_TICKS,
            salt: 991,
        }),
        "HellBomb2" => Some(PendingPlayerStatusEffect::Dazed {
            duration_ticks: HELL_BOMB_POISON_DURATION_TICKS,
        }),
        "HellBomb3" => Some(PendingPlayerStatusEffect::BleedingPoison {
            chance_denominator: 1,
            duration_ticks: HELL_BOMB_POISON_DURATION_TICKS,
            salt: 993,
        }),
        _ => None,
    }
}

pub(super) fn explode_bomb_spider(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };

    let bomb_spider_damage = crystal_monster_by_name("BombSpider")
        .map(|monster| monster.max_dc.max(monster.min_dc).max(1))
        .unwrap_or(6);

    if tile_distance(
        position,
        &entity_position(world, player_entity(world).expect("player")).expect("player position"),
    ) <= 1
    {
        schedule_damage_to_player(
            world,
            world.resource::<RuntimeClockResource>().tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
            attacker_id,
            "BombSpider".to_string(),
            bomb_spider_damage,
        );
    }

    let current_tick = world.resource::<RuntimeClockResource>().tick;
    let _ = damage_monster_entity(world, monster_entity, i32::MAX, current_tick, packets);
}

pub(super) fn explode_vampire_spider(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
    _packets: &mut Vec<ServerPacket>,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let Some(attacker_agent) = world.entity(monster_entity).get::<MonsterAgent>().cloned() else {
        return;
    };

    let spider_damage = crystal_monster_by_name("VampireSpider")
        .map(|monster| (monster.max_dc.max(monster.min_dc) * 10).max(1))
        .unwrap_or(10);

    if let Some(player) = player_entity(world) {
        let Some(player_position) = entity_position(world, player) else {
            return;
        };
        if tile_distance(position, &player_position) <= 1 {
            schedule_damage_to_player(
                world,
                current_tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
                attacker_id,
                "VampireSpider".to_string(),
                spider_damage,
            );
        }
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    for target_entity in monster_entities {
        if target_entity == monster_entity {
            continue;
        }
        let entry = world.entity(target_entity);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_agent.hostile_to_player == target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if tile_distance(position, &target_position) > 1 {
            continue;
        }

        schedule_damage_to_monster(
            world,
            current_tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
            attacker_id,
            target_entity,
            spider_damage,
            None,
            None,
        );
    }

    let Some(summoned) = world
        .entity(monster_entity)
        .get::<SummonedMonster>()
        .copied()
    else {
        return;
    };
    if let Some(mut summoned_mut) = world
        .entity_mut(monster_entity)
        .get_mut::<SummonedMonster>()
    {
        summoned_mut.despawn_tick_after_death = Some(
            summoned
                .despawn_tick_after_death
                .unwrap_or(current_tick + 1),
        );
    }
}

pub(super) fn explode_charmed_snake(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let Some(attacker_agent) = world.entity(monster_entity).get::<MonsterAgent>().cloned() else {
        return;
    };
    let damage = 30;

    if let Some(player) = player_entity(world) {
        let Some(player_position) = entity_position(world, player) else {
            return;
        };
        if tile_distance(position, &player_position) <= 1 && attacker_agent.hostile_to_player {
            schedule_damage_to_player(
                world,
                current_tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
                attacker_id,
                "CharmedSnake".to_string(),
                damage,
            );
        }
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    for target_entity in monster_entities {
        if target_entity == monster_entity {
            continue;
        }
        let entry = world.entity(target_entity);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_agent.hostile_to_player == target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if tile_distance(position, &target_position) > 1 {
            continue;
        }

        schedule_damage_to_monster(
            world,
            current_tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
            attacker_id,
            target_entity,
            damage,
            None,
            None,
        );
    }
}

impl SimulationSession {
    pub fn zone_melee_attack_damage(&self) -> i32 {
        if !is_in_world(self.app.world()) {
            return 1;
        }
        crystal_player_zone_base_melee_damage(self.app.world())
    }

    pub fn zone_range_attack_profile(&self) -> (Spell, u8, i32) {
        if !is_in_world(self.app.world()) {
            return (Spell::None, 0, 1);
        }

        let world = self.app.world();
        let current_tick = runtime_tick(world);
        let player_object_id = current_player_object_id(world).unwrap_or_default();
        let mut damage = crystal_player_zone_base_melee_damage(world).max(1);
        let mut spell = Spell::None;
        let mut spell_level = 0;
        if let Some((magic, level)) = crystal_skill_magic(world, "Focus") {
            if level >= 3
                || deterministic_chance_roll(
                    current_tick,
                    player_object_id,
                    Spell::Focus as u64,
                    5_u64.saturating_sub(u64::from(level)).max(1),
                )
            {
                spell = Spell::Focus;
                spell_level = level;
                damage = crystal_magic_damage_from_base(&magic, level, damage).max(1);
            }
        }
        (spell, spell_level, damage)
    }

    pub fn zone_magic_attack_profile(&self, spell: Spell) -> Option<(u8, i32, i32, u64)> {
        if !is_in_world(self.app.world()) {
            return None;
        }

        let world = self.app.world();
        let base_damage = crystal_player_zone_base_melee_damage(world).max(1);
        let spell_name = format!("{spell:?}");
        let Some((magic, level)) = crystal_skill_magic(world, &spell_name) else {
            return None;
        };
        let mp_cost = i32::from(magic.base_cost) + i32::from(magic.level_cost) * i32::from(level);
        let cooldown_ms = u64::from(
            magic
                .delay_base
                .saturating_sub(magic.delay_reduction.saturating_mul(u32::from(level)))
                .max(1),
        );
        let damage = if matches!(spell, Spell::ElectricShock | Spell::Entrapment) {
            0
        } else {
            crystal_magic_damage_from_base(&magic, level, base_damage).max(1)
        };
        Some((level, damage, mp_cost.max(0), cooldown_ms))
    }

    pub fn attack(&mut self, object_id: u32) -> Vec<ServerPacket> {
        let packets = self.attack_impl(object_id);
        self.finalize_packets(packets)
    }

    pub fn harvest(&mut self, direction: MirDirection) -> Vec<ServerPacket> {
        let packets = self.harvest_impl(direction);
        self.finalize_packets(packets)
    }

    pub(super) fn attack_impl(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.attack_impl_with_spell(object_id, Spell::None)
    }

    pub(super) fn attack_impl_with_spell(
        &mut self,
        object_id: u32,
        requested_spell: Spell,
    ) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }
        if current_player_is_dead(self.app.world())
            || crystal_player_attack_blocked_by_status(self.app.world())
        {
            return vec![ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            }];
        }

        dismiss_dialog(self.app.world_mut());
        let Some(monster_entity) = entity_by_object_id(self.app.world(), object_id) else {
            return Vec::new();
        };

        let player_entity = player_entity(self.app.world()).expect("player should exist");
        let player_position =
            entity_position(self.app.world(), player_entity).expect("player position");
        let target_position =
            entity_position(self.app.world(), monster_entity).expect("monster position");
        let target_name =
            entity_name(self.app.world(), monster_entity).unwrap_or_else(|| "Target".to_string());

        let monster_entry = self.app.world().entity(monster_entity);
        let Some(monster_agent) = monster_entry.get::<MonsterAgent>() else {
            return Vec::new();
        };
        let monster_ai_state = monster_entry
            .get::<MonsterAiState>()
            .copied()
            .unwrap_or_default();
        if monster_agent.dead
            || monster_ai_state.hidden
            || monster_is_stoned_zuma(monster_agent, &monster_ai_state)
        {
            return Vec::new();
        }

        let Some(direction) = direction_toward(&player_position, &target_position) else {
            return Vec::new();
        };

        let mut packets = Vec::new();
        {
            let mut player = self.app.world_mut().entity_mut(player_entity);
            let mut facing = player.get_mut::<Facing>().expect("player facing");
            if facing.0 != direction {
                facing.0 = direction;
                packets.push(ServerPacket::UserLocation {
                    location: current_location(self.app.world()),
                });
            }
        }

        let target_distance = tile_distance(&player_position, &target_position);
        let thrusting_requested = requested_spell == Spell::Thrusting
            && crystal_skill_magic(self.app.world(), "Thrusting").is_some()
            && skill_toggle_state(self.app.world(), Spell::Thrusting);
        if target_distance > 1 && !(thrusting_requested && target_distance == 2) {
            return packets;
        }

        let current_tick = runtime_tick(self.app.world());
        let player_object_id =
            current_player_object_id(self.app.world()).expect("player object id");
        let hit_due_tick =
            queued_before_world_tick_due_tick(current_tick, melee_attack_delay_ticks());
        let target_object_id =
            entity_object_id(self.app.world(), monster_entity).unwrap_or(object_id);
        let (player_attack_stats, mut damage) =
            player_melee_attack_power(self.app.world(), current_tick, target_object_id);
        let mut attack_spell = Spell::None;
        let mut attack_spell_level = 0;
        let mut due_packet = None;

        if thrusting_requested {
            if let Some((magic, level)) = crystal_skill_magic(self.app.world(), "Thrusting") {
                attack_spell = Spell::Thrusting;
                attack_spell_level = level;
                damage = crystal_magic_damage_from_base(&magic, level, damage);
            }
        } else if requested_spell == Spell::Slaying {
            let slaying_armed = {
                let skills = self.app.world().resource::<SkillResource>();
                skills.slaying_armed || skill_toggle_state(self.app.world(), Spell::Slaying)
            };
            if slaying_armed {
                if let Some((magic, level)) = crystal_skill_magic(self.app.world(), "Slaying") {
                    attack_spell = Spell::Slaying;
                    attack_spell_level = level;
                    damage = crystal_magic_damage_from_base(&magic, level, damage);
                    self.app
                        .world_mut()
                        .resource_mut::<SkillResource>()
                        .slaying_armed = false;
                    set_skill_toggle_state(self.app.world_mut(), Spell::Slaying, false);
                }
            }
        } else if requested_spell == Spell::FlamingSword
            && skill_toggle_state(self.app.world(), Spell::FlamingSword)
        {
            if let Some((magic, level)) = crystal_skill_magic(self.app.world(), "FlamingSword") {
                attack_spell = Spell::FlamingSword;
                attack_spell_level = level;
                damage = crystal_magic_damage_from_base(&magic, level, damage);
                set_skill_toggle_state(self.app.world_mut(), Spell::FlamingSword, false);
            }
        }

        if let Some((magic, level)) = crystal_skill_magic(self.app.world(), "FatalSword") {
            let should_proc = {
                let mut skills = self.app.world_mut().resource_mut::<SkillResource>();
                if skills.fatal_sword_armed {
                    skills.fatal_sword_armed = false;
                    true
                } else if level >= 3
                    || deterministic_chance_roll(current_tick, player_object_id, 91, 10)
                {
                    skills.fatal_sword_armed = true;
                    false
                } else {
                    false
                }
            };
            if should_proc {
                damage = crystal_magic_damage_from_base(&magic, level, damage);
                due_packet = Some(object_effect_packet(
                    target_object_id,
                    CRYSTAL_SPELL_EFFECT_FATAL_SWORD,
                    0,
                ));
            }
        }

        if let Some((magic, level)) = crystal_skill_magic(self.app.world(), "MPEater") {
            let accuracy = crystal_player_accuracy(self.app.world()).max(0);
            let should_proc = {
                let mut skills = self.app.world_mut().resource_mut::<SkillResource>();
                if skills.mp_eater_armed {
                    skills.mp_eater_armed = false;
                    skills.mp_eater_count = 0;
                    true
                } else {
                    let base_count = 1_i32.saturating_add(accuracy / 2);
                    let max_count = base_count.saturating_add(i32::from(level) * 5);
                    let gain_range =
                        u64::try_from(max_count.saturating_sub(base_count).max(1)).unwrap_or(1);
                    let gain = base_count.saturating_add(
                        i32::try_from(deterministic_roll(
                            current_tick,
                            usize::try_from(player_object_id).unwrap_or_default(),
                            309,
                            gain_range,
                        ))
                        .unwrap_or(0),
                    );
                    skills.mp_eater_count = skills
                        .mp_eater_count
                        .saturating_add(u16::try_from(gain.max(0)).unwrap_or(u16::MAX));
                    if skills.mp_eater_count >= 100 {
                        skills.mp_eater_armed = true;
                    }
                    false
                }
            };
            if should_proc {
                damage = crystal_magic_damage_from_base(&magic, level, damage);
                packets.push(object_effect_packet(
                    target_object_id,
                    CRYSTAL_SPELL_EFFECT_MP_EATER,
                    player_object_id,
                ));
                restore_player_mp(
                    self.app.world_mut(),
                    5 * (i32::from(level) + accuracy / 4),
                    &mut packets,
                );
            }
        }

        if let Some((magic, level)) = crystal_skill_magic(self.app.world(), "Hemorrhage") {
            let should_proc = {
                let mut skills = self.app.world_mut().resource_mut::<SkillResource>();
                if skills.hemorrhage_armed {
                    skills.hemorrhage_armed = false;
                    skills.hemorrhage_attack_count = 0;
                    true
                } else {
                    let gain = 20_u16.saturating_add(u16::from(level).saturating_mul(20));
                    skills.hemorrhage_attack_count =
                        skills.hemorrhage_attack_count.saturating_add(gain);
                    if skills.hemorrhage_attack_count >= 55 {
                        skills.hemorrhage_armed = true;
                    }
                    false
                }
            };
            if should_proc {
                damage = crystal_magic_damage_from_base(&magic, level, damage);
                packets.push(object_effect_packet(
                    target_object_id,
                    CRYSTAL_SPELL_EFFECT_HEMORRHAGE,
                    0,
                ));
                apply_monster_poison(
                    self.app.world_mut(),
                    monster_entity,
                    CRYSTAL_POISON_BLEEDING,
                    0,
                    current_tick,
                    u64::from(level).saturating_mul(2).max(1),
                );
                packets.push(ServerPacket::ObjectPoisoned {
                    object_id: target_object_id,
                    poison: CRYSTAL_POISON_BLEEDING,
                });
            }
        }

        if let Some(level) = crystal_skill_level(self.app.world(), "Slaying") {
            let should_arm = {
                let skills = self.app.world().resource::<SkillResource>();
                !skills.slaying_armed
                    && !skill_toggle_state(self.app.world(), Spell::Slaying)
                    && (level >= 3
                        || deterministic_chance_roll(current_tick, player_object_id, 2, 12))
            };
            if should_arm {
                self.app
                    .world_mut()
                    .resource_mut::<SkillResource>()
                    .slaying_armed = true;
                set_skill_toggle_state(self.app.world_mut(), Spell::Slaying, true);
                packets.push(ServerPacket::SpellToggle {
                    object_id: player_object_id,
                    spell: Spell::Slaying,
                    can_use: true,
                });
            }
        }

        if let Some(level) = crystal_skill_level(self.app.world(), "Meditation") {
            if level >= 3
                || deterministic_chance_roll(
                    current_tick,
                    player_object_id,
                    126,
                    8_u64.saturating_sub(u64::from(level)).max(1),
                )
            {
                if let Some(packet) = gather_meditation_element_packet(self.app.world_mut(), false)
                {
                    queue_due_packet(self.app.world_mut(), hit_due_tick, packet);
                }
            }
        }

        if let Some(packet) = object_attack_packet_for_player(
            self.app.world(),
            player_entity,
            &player_position,
            direction,
            attack_spell,
            attack_spell_level,
        ) {
            packets.push(packet);
        }
        if let Some(packet) = damage_weapon_durability(self.app.world_mut(), current_tick) {
            packets.push(packet);
        }
        let mut agent = {
            let monster = self.app.world().entity(monster_entity);
            let agent = monster.get::<MonsterAgent>().expect("monster agent");
            agent.clone()
        };
        if monster_locks_player_target_on_hit(&agent) {
            agent.tracking_player = true;
        }
        self.app
            .world_mut()
            .entity_mut(monster_entity)
            .insert(agent);
        schedule_player_attack_on_monster(
            self.app.world_mut(),
            hit_due_tick,
            player_object_id,
            monster_entity,
            damage,
            Some(PendingMonsterDefeatAction {
                object_id,
                name: target_name.clone(),
            }),
            due_packet,
            player_attack_stats,
            DefenceType::ACAgility,
        );
        queue_melee_passive_skill_progression(self.app.world_mut(), hit_due_tick, current_tick);

        packets.extend(advance_world(self.app.world_mut()));
        packets
    }

    pub(super) fn attack_in_direction_with_spell(
        &mut self,
        direction: MirDirection,
        requested_spell: Spell,
    ) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }
        if current_player_is_dead(self.app.world())
            || crystal_player_attack_blocked_by_status(self.app.world())
        {
            return vec![ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            }];
        }

        if let Some(player) = player_entity(self.app.world()) {
            self.app
                .world_mut()
                .entity_mut(player)
                .insert(Facing(direction));
        }

        let object_id = attack_target_in_direction(self.app.world(), direction).or_else(|| {
            if requested_spell == Spell::Thrusting
                && skill_toggle_state(self.app.world(), Spell::Thrusting)
            {
                attack_target_in_direction_at_distance(self.app.world(), direction, 2)
            } else {
                None
            }
        });

        let Some(object_id) = object_id else {
            return vec![ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            }];
        };

        self.attack_impl_with_spell(object_id, requested_spell)
    }

    pub(super) fn range_attack_impl(
        &mut self,
        direction: MirDirection,
        target_id: u32,
        target_location: Point,
    ) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }
        if current_player_is_dead(self.app.world())
            || crystal_player_attack_blocked_by_status(self.app.world())
        {
            return vec![ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            }];
        }
        dismiss_dialog(self.app.world_mut());
        let Some(target_entity) = entity_by_object_id(self.app.world(), target_id) else {
            return Vec::new();
        };
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };
        let Some(player_position) = entity_position(self.app.world(), player) else {
            return Vec::new();
        };
        let Some(target_position) = entity_position(self.app.world(), target_entity) else {
            return Vec::new();
        };
        let target_entry = self.app.world().entity(target_entity);
        let Some(target_agent) = target_entry.get::<MonsterAgent>() else {
            return Vec::new();
        };
        let target_ai_state = target_entry
            .get::<MonsterAiState>()
            .copied()
            .unwrap_or_default();
        if target_agent.dead
            || target_ai_state.hidden
            || monster_is_stoned_zuma(target_agent, &target_ai_state)
            || !target_agent.hostile_to_player
        {
            return Vec::new();
        }

        self.app
            .world_mut()
            .entity_mut(player)
            .insert(Facing(direction));
        let current_tick = runtime_tick(self.app.world());
        let player_object_id =
            current_player_object_id(self.app.world()).expect("player object id");
        let (player_attack_stats, mut damage) =
            player_melee_attack_power(self.app.world(), current_tick, target_id);
        let mut spell = Spell::None;
        let mut spell_level = 0;
        if let Some((magic, level)) = crystal_skill_magic(self.app.world(), "Focus") {
            if level >= 3
                || deterministic_chance_roll(
                    current_tick,
                    player_object_id,
                    121,
                    5_u64.saturating_sub(u64::from(level)).max(1),
                )
            {
                spell = Spell::Focus;
                spell_level = level;
                damage = crystal_magic_damage_from_base(&magic, level, damage);
            }
        }
        let due_tick = queued_before_world_tick_due_tick(
            current_tick,
            ranged_attack_delay_ticks(&player_position, &target_position),
        );
        let target_name =
            entity_name(self.app.world(), target_entity).unwrap_or_else(|| "Target".to_string());
        schedule_player_attack_on_monster(
            self.app.world_mut(),
            due_tick,
            player_object_id,
            target_entity,
            damage,
            Some(PendingMonsterDefeatAction {
                object_id: target_id,
                name: target_name,
            }),
            None,
            player_attack_stats,
            DefenceType::ACAgility,
        );

        let mut packets = vec![
            ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            },
            ServerPacket::RangeAttack {
                target_id,
                target: target_location.clone(),
                spell,
            },
        ];
        if let Some(packet) = object_attack_packet_for_player(
            self.app.world(),
            player,
            &player_position,
            direction,
            Spell::None,
            0,
        ) {
            packets.push(packet);
        }
        packets.push(ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id: player_object_id,
                location: player_position,
                direction,
                target_id,
                target: target_location,
                attack_type: 0,
                spell: spell as u8,
                level: spell_level,
            },
        });
        packets.extend(advance_world(self.app.world_mut()));
        packets
    }

    pub(super) fn harvest_impl(&mut self, direction: MirDirection) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }
        if current_player_is_dead(self.app.world()) {
            return vec![ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            }];
        }

        dismiss_dialog(self.app.world_mut());
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };

        self.app
            .world_mut()
            .entity_mut(player)
            .insert(Facing(direction));

        let mut packets = vec![ServerPacket::UserLocation {
            location: current_location(self.app.world()),
        }];

        if let Some(selection) = harvest_target_in_direction(self.app.world(), direction) {
            packets.push(ServerPacket::ObjectHarvest {
                movement: current_movement(self.app.world()),
            });
            match selection {
                HarvestTargetSelection::Target(target) => {
                    packets.extend(harvest_monster_entity(self.app.world_mut(), target));
                }
                HarvestTargetSelection::OwnerBlocked => {
                    packets.push(system_message_key(
                        self.app.world(),
                        "server.NoNearbyOwnedCarcasses",
                    ));
                }
            }
        }

        packets.extend(advance_world(self.app.world_mut()));
        packets
    }
}

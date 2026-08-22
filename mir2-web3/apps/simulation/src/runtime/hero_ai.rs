use std::collections::BTreeMap;

use bevy_ecs::{
    entity::Entity,
    prelude::{Resource, World},
};
use mir2_game_data::{CrystalMagicTemplate, crystal_magic_by_spell, crystal_monster_by_name};
use mir2_protocol::{
    ClientBuff, MirClass, MirDirection, ObjectAttackInfo, ObjectMovement, ObjectRangeAttackInfo,
    Point, ServerPacket, Spell, UserItem, UserItemStat,
};

use crate::config::{EquipmentSlot, WorldEntityDisposition};

use super::buffs::crystal_buff_type_for_key;
use super::combat::{
    PendingMonsterDefeatAction, combat_delay_ticks, melee_attack_delay_ticks, queue_due_packet,
    ranged_attack_delay_ticks, schedule_damage_to_monster,
};
use super::components::{
    CharacterBody, Facing, Hero, Monster, MonsterAgent, MonsterAiState, MonsterVitals, ObjectId,
    PlayerVitals, Position, entity_name, entity_position, hero_entity, player_entity,
};
use super::crystal_compat::{
    CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT, CRYSTAL_STAT_MANA_PENALTY_PERCENT, CRYSTAL_STAT_MAX_AC,
    CRYSTAL_STAT_MAX_DC, CRYSTAL_STAT_MAX_MC, CRYSTAL_STAT_MAX_SC, CRYSTAL_STAT_MIN_DC,
    CRYSTAL_STAT_MIN_MC, CRYSTAL_STAT_MIN_SC,
};
use super::equipment::equipment_slot_index;
use super::items::{
    ItemState, crystal_equipment_slot_for_item_key, crystal_item_added_stat_value,
    crystal_item_stat_value, crystal_item_template_for_item_key, user_item_from_item_state,
};
use super::monsters::{is_hidden_or_sleeping_target, monster_locks_player_target_on_hit};
use super::movement::{
    can_occupy, direction_toward, offset_point, rotated_direction, tile_distance,
};
use super::packets::{
    object_health_info_for_entity, object_mana_info_for_entity, object_struck_packet,
};
use super::resources::{HeroInventoryResource, PlayerRuntimeResource, Stage5SystemsResource};
use super::skills::crystal_magic_damage;

const HERO_BEHAVIOUR_ATTACK: u8 = 0;
const HERO_BEHAVIOUR_COUNTER_ATTACK: u8 = 1;
const HERO_BEHAVIOUR_FOLLOW: u8 = 2;
const HERO_VIEW_RANGE: i32 = 8;
const HERO_MELEE_RANGE: i32 = 1;
const HERO_ARCHER_RANGE: i32 = 5;
const HERO_MOVE_INTERVAL_TICKS: u64 = 1;
const HERO_ATTACK_INTERVAL_TICKS: u64 = 2;
const HERO_SLAYING_DAMAGE_BONUSES: [i32; 4] = [5, 6, 7, 8];
const HERO_BUFF_MAGIC_BOOSTER: u8 = 21;
const HERO_BUFF_MAGIC_SHIELD: u8 = 24;

#[derive(Clone, Copy, Default)]
struct HeroAiSkillState {
    concentration_expires_at: u64,
    next_concentration_tick: u64,
    next_shot_tick: u64,
    next_heal_tick: u64,
    next_wizard_spell_tick: u64,
    magic_shield_expires_at: u64,
    magic_booster_expires_at: u64,
}

#[derive(Resource, Default)]
struct HeroAiSkillResource {
    states: BTreeMap<u32, HeroAiSkillState>,
    pending_heals: Vec<HeroPendingHeal>,
}

#[derive(Clone, Copy)]
struct HeroPendingHeal {
    due_tick: u64,
    target_entity: Entity,
    amount: i32,
}

#[derive(Clone)]
struct HeroAiTarget {
    entity: Entity,
    object_id: u32,
    name: String,
    position: Point,
}

struct HeroHealTarget {
    entity: Entity,
    object_id: u32,
    position: Point,
    hp: i32,
    max_hp: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeroWizardSpellArea {
    Single,
    CasterSquare { radius: i32 },
    TargetSquare { radius: i32 },
    Repulsion,
}

struct HeroWizardSpellChoice {
    spell: Spell,
    magic: CrystalMagicTemplate,
    skill_level: u8,
    area: HeroWizardSpellArea,
}

fn stage5_hero_behaviour(world: &World) -> Option<u8> {
    world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
        .filter(|hero| hero.spawned)
        .map(|hero| hero.behaviour)
}

fn hero_learned_magic_filter_active(world: &World) -> bool {
    !world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero_learned_magics
        .is_empty()
}

fn hero_spell_is_crystal_ranged(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::FireBall
            | Spell::ThunderBolt
            | Spell::FireBang
            | Spell::FireWall
            | Spell::FrostCrunch
            | Spell::Vampirism
            | Spell::FlameDisruptor
            | Spell::IceStorm
            | Spell::MeteorStrike
            | Spell::Blizzard
            | Spell::SoulFireBall
            | Spell::StraightShot
            | Spell::ElementalShot
            | Spell::PoisonShot
    )
}

fn hero_has_usable_learned_ranged_magic(world: &World) -> bool {
    world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero_learned_magics
        .iter()
        .any(|magic| magic.key > 0 && hero_spell_is_crystal_ranged(magic.spell))
}

fn hero_learned_magic_level(world: &World, spell: Spell, derived_level: u8) -> Option<u8> {
    let learned_magics = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero_learned_magics;
    if learned_magics.is_empty() {
        return Some(derived_level);
    }

    let learned = learned_magics
        .iter()
        .find(|magic| magic.spell == spell && magic.key > 0)?;
    Some(derived_level.min(learned.level.min(2)))
}

fn hero_attack_range(world: &World, class: Option<MirClass>) -> i32 {
    if hero_learned_magic_filter_active(world) {
        return match class {
            Some(MirClass::Archer) if hero_has_usable_learned_ranged_magic(world) => {
                HERO_ARCHER_RANGE
            }
            Some(MirClass::Wizard) if hero_has_usable_learned_ranged_magic(world) => {
                HERO_VIEW_RANGE
            }
            _ => HERO_MELEE_RANGE,
        };
    }

    match class {
        Some(MirClass::Archer) => HERO_ARCHER_RANGE,
        Some(MirClass::Wizard) => HERO_VIEW_RANGE,
        _ => HERO_MELEE_RANGE,
    }
}

fn hero_inventory_equipment_slot(item: &ItemState) -> Option<EquipmentSlot> {
    item.equip_slot
        .or_else(|| crystal_equipment_slot_for_item_key(&item.key))
}

fn hero_inventory_equipment_candidates(item: &ItemState) -> [Option<EquipmentSlot>; 2] {
    match hero_inventory_equipment_slot(item) {
        Some(EquipmentSlot::BraceletLeft) => [
            Some(EquipmentSlot::BraceletLeft),
            Some(EquipmentSlot::BraceletRight),
        ],
        Some(EquipmentSlot::RingLeft) => [
            Some(EquipmentSlot::RingLeft),
            Some(EquipmentSlot::RingRight),
        ],
        slot => [slot, None],
    }
}

fn hero_inventory_item_is_broken(item: &ItemState) -> bool {
    item.durability_max.unwrap_or_default() > 0 && item.durability_current.unwrap_or_default() == 0
}

fn hero_inventory_item_modeled_base_stat(item: &ItemState, stat: u8) -> i32 {
    match stat {
        CRYSTAL_STAT_MAX_AC => item.defence,
        CRYSTAL_STAT_MAX_DC => item.attack,
        _ => 0,
    }
}

fn hero_inventory_item_crystal_stat(item: &ItemState, stat: u8) -> i32 {
    if hero_inventory_equipment_slot(item).is_none() || hero_inventory_item_is_broken(item) {
        return 0;
    }

    let modeled_base = hero_inventory_item_modeled_base_stat(item, stat);
    let template_base = crystal_item_template_for_item_key(&item.key)
        .map(|template| crystal_item_stat_value(&template, stat))
        .unwrap_or_default();
    let base = if modeled_base != 0 {
        modeled_base
    } else {
        template_base
    };
    base.saturating_add(crystal_item_added_stat_value(item, stat))
}

pub(super) fn hero_inventory_crystal_stat_total(world: &World, stat: u8) -> i32 {
    world
        .resource::<HeroInventoryResource>()
        .items
        .iter()
        .map(|item| hero_inventory_item_crystal_stat(item, stat))
        .sum()
}

pub(super) fn hero_inventory_equipment_slots(world: &World) -> Vec<Option<UserItem>> {
    let mut slots = vec![None; 14];
    for item in &world.resource::<HeroInventoryResource>().items {
        for candidate in hero_inventory_equipment_candidates(item)
            .into_iter()
            .flatten()
        {
            let Some(index) = equipment_slot_index(candidate) else {
                continue;
            };
            let Some(slot) = slots.get_mut(index) else {
                continue;
            };
            if slot.is_none() {
                *slot = Some(user_item_from_item_state(item));
                break;
            }
        }
    }
    slots
}

fn hero_ai_skill_state(world: &mut World, hero_id: u32) -> HeroAiSkillState {
    if world.get_resource::<HeroAiSkillResource>().is_none() {
        world.insert_resource(HeroAiSkillResource::default());
    }

    world
        .resource::<HeroAiSkillResource>()
        .states
        .get(&hero_id)
        .copied()
        .unwrap_or_default()
}

fn update_hero_ai_skill_state(
    world: &mut World,
    hero_id: u32,
    update: impl FnOnce(&mut HeroAiSkillState),
) {
    if world.get_resource::<HeroAiSkillResource>().is_none() {
        world.insert_resource(HeroAiSkillResource::default());
    }

    let mut skills = world.resource_mut::<HeroAiSkillResource>();
    update(skills.states.entry(hero_id).or_default());
}

fn queue_hero_ai_pending_heal(
    world: &mut World,
    due_tick: u64,
    target_entity: Entity,
    amount: i32,
) {
    if amount <= 0 {
        return;
    }
    if world.get_resource::<HeroAiSkillResource>().is_none() {
        world.insert_resource(HeroAiSkillResource::default());
    }
    world
        .resource_mut::<HeroAiSkillResource>()
        .pending_heals
        .push(HeroPendingHeal {
            due_tick,
            target_entity,
            amount,
        });
}

fn tick_hero_ai_pending_heals(world: &mut World, tick: u64, packets: &mut Vec<ServerPacket>) {
    let due_heals = {
        let Some(mut skills) = world.get_resource_mut::<HeroAiSkillResource>() else {
            return;
        };
        let pending = std::mem::take(&mut skills.pending_heals);
        let mut due = Vec::new();
        let mut future = Vec::new();
        for heal in pending {
            if heal.due_tick <= tick {
                due.push(heal);
            } else {
                future.push(heal);
            }
        }
        skills.pending_heals = future;
        due
    };

    for heal in due_heals {
        let restored = {
            let mut entry = world.entity_mut(heal.target_entity);
            entry.get_mut::<PlayerVitals>().map(|mut vitals| {
                vitals.hp = vitals.hp.saturating_add(heal.amount).min(vitals.max_hp);
                *vitals
            })
        };
        if let Some(restored) = restored {
            if Some(heal.target_entity) == player_entity(world) {
                world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored;
            }
            if let Some(info) = object_health_info_for_entity(world, heal.target_entity, 0) {
                packets.push(ServerPacket::ObjectHealth { info });
            }
        }
    }
}

fn hero_attack_damage(
    world: &World,
    class: Option<MirClass>,
    level: Option<u16>,
    spell: Spell,
    spell_level: u8,
) -> i32 {
    let level_bonus = i32::from(level.unwrap_or(1).max(1)) / 3;
    let class_bonus = match class.unwrap_or(MirClass::Warrior) {
        MirClass::Warrior => 10,
        MirClass::Assassin => 9,
        MirClass::Archer => 8,
        MirClass::Taoist => 7,
        MirClass::Wizard => 6,
    };
    let hero_attack_bonus = hero_inventory_crystal_stat_total(world, CRYSTAL_STAT_MAX_DC)
        .max(hero_inventory_crystal_stat_total(
            world,
            CRYSTAL_STAT_MIN_DC,
        ))
        .max(0);
    let slaying_bonus = hero_slaying_skill_level(world, class, level)
        .map(|level| HERO_SLAYING_DAMAGE_BONUSES[usize::from(level).min(3)])
        .unwrap_or_default();
    let mut damage = (class_bonus + level_bonus)
        .saturating_add(hero_attack_bonus)
        .saturating_add(slaying_bonus)
        .max(1);
    if let Some(spell_name) = hero_damage_spell_name(spell) {
        if let Some(magic) = crystal_magic_by_spell(spell_name) {
            damage = hero_magic_damage_from_base(&magic, spell_level, damage);
        };
    }
    damage.max(1)
}

fn hero_damage_spell_name(spell: Spell) -> Option<&'static str> {
    match spell {
        Spell::FlamingSword => Some("FlamingSword"),
        Spell::StraightShot => Some("StraightShot"),
        Spell::FireBall => Some("FireBall"),
        Spell::GreatFireBall => Some("GreatFireBall"),
        Spell::ThunderBolt => Some("ThunderBolt"),
        _ => None,
    }
}

fn hero_crystal_magic_level_for_template(
    hero_level: Option<u16>,
    magic: &CrystalMagicTemplate,
) -> Option<u8> {
    let hero_level = u8::try_from(hero_level.unwrap_or(1)).unwrap_or(u8::MAX);
    if hero_level >= magic.level3 {
        Some(2)
    } else if hero_level >= magic.level2 {
        Some(1)
    } else if hero_level >= magic.level1 {
        Some(0)
    } else {
        None
    }
}

fn hero_crystal_magic_level(
    world: &World,
    hero_level: Option<u16>,
    spell_name: &str,
    spell: Spell,
) -> Option<u8> {
    let magic = crystal_magic_by_spell(spell_name)?;
    let derived_level = hero_crystal_magic_level_for_template(hero_level, &magic)?;
    hero_learned_magic_level(world, spell, derived_level)
}

fn hero_crystal_magic_for_class(
    world: &World,
    class: Option<MirClass>,
    level: Option<u16>,
    required_class: MirClass,
    spell_name: &str,
    spell: Spell,
) -> Option<(CrystalMagicTemplate, u8)> {
    if class != Some(required_class) {
        return None;
    }
    let magic = crystal_magic_by_spell(spell_name)?;
    let derived_level = hero_crystal_magic_level_for_template(level, &magic)?;
    let skill_level = hero_learned_magic_level(world, spell, derived_level)?;
    Some((magic, skill_level))
}

fn hero_slaying_skill_level(
    world: &World,
    class: Option<MirClass>,
    level: Option<u16>,
) -> Option<u8> {
    (class == Some(MirClass::Warrior))
        .then(|| hero_crystal_magic_level(world, level, "Slaying", Spell::Slaying))
        .flatten()
}

fn hero_flaming_sword_skill_level(
    world: &World,
    class: Option<MirClass>,
    level: Option<u16>,
) -> Option<u8> {
    (class == Some(MirClass::Warrior))
        .then(|| hero_crystal_magic_level(world, level, "FlamingSword", Spell::FlamingSword))
        .flatten()
}

fn hero_magic_damage_from_base(
    magic: &mir2_game_data::CrystalMagicTemplate,
    level: u8,
    base_damage: i32,
) -> i32 {
    let flat = crystal_magic_damage(magic, level);
    let level = i32::from(level) + 1;
    let multiplier = magic.multiplier_base + f32::from(level as u16 - 1) * magic.multiplier_bonus;
    ((base_damage.saturating_add(flat).max(1) as f32) * multiplier.max(0.1)).round() as i32
}

fn hero_magic_mana_cost(magic: &CrystalMagicTemplate, level: u8) -> i32 {
    i32::from(magic.base_cost).saturating_add(i32::from(magic.level_cost) * i32::from(level))
}

fn hero_magic_delay_ms(magic: &CrystalMagicTemplate, level: u8) -> i64 {
    i64::from(
        magic
            .delay_base
            .saturating_sub(magic.delay_reduction.saturating_mul(u32::from(level)))
            .max(1),
    )
}

fn hero_magic_cooldown_ticks(magic: &CrystalMagicTemplate, level: u8) -> u64 {
    combat_delay_ticks(u64::try_from(hero_magic_delay_ms(magic, level).max(1)).unwrap_or(1))
}

fn hero_magic_experience_gain(tick: u64, hero_id: u32, spell: Spell) -> u16 {
    1 + ((tick + u64::from(hero_id) + u64::from(spell as u8)) % 3) as u16
}

fn hero_magic_experience_threshold(
    magic: &CrystalMagicTemplate,
    level: u8,
    hero_level: u16,
) -> Option<u16> {
    match level {
        0 if hero_level >= u16::from(magic.level1) => Some(magic.need1.max(1)),
        1 if hero_level >= u16::from(magic.level2) => Some(magic.need2.max(1)),
        2 if hero_level >= u16::from(magic.level3) => Some(magic.need3.max(1)),
        _ => None,
    }
}

fn advance_hero_learned_magic(
    world: &mut World,
    tick: u64,
    hero_id: u32,
    spell: Spell,
    magic: &CrystalMagicTemplate,
    hero_level: Option<u16>,
    packets: &mut Vec<ServerPacket>,
) {
    let hero_level = hero_level.unwrap_or(1);
    let gain = hero_magic_experience_gain(tick, hero_id, spell);
    let outcome = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let Some(learned) = stage5
            .stage5_systems
            .hero_learned_magics
            .iter_mut()
            .find(|learned| learned.spell == spell && learned.key > 0)
        else {
            return;
        };
        let old_level = learned.level;
        let Some(threshold) = hero_magic_experience_threshold(magic, old_level, hero_level) else {
            return;
        };

        learned.experience = learned.experience.saturating_add(gain);
        if learned.experience >= threshold {
            learned.level = learned.level.saturating_add(1).min(3);
            learned.experience = if learned.level >= 3 {
                0
            } else {
                learned.experience.saturating_sub(threshold)
            };
        }
        Some((
            old_level != learned.level,
            learned.level,
            learned.experience,
        ))
    };

    let Some((leveled, level, experience)) = outcome else {
        return;
    };
    if leveled {
        packets.push(ServerPacket::MagicDelay {
            object_id: hero_id,
            spell,
            delay: hero_magic_delay_ms(magic, level),
        });
    }
    packets.push(ServerPacket::MagicLeveled {
        object_id: hero_id,
        spell,
        level,
        experience,
    });
}

fn hero_try_consume_magic_mp(
    world: &mut World,
    hero_entity: Entity,
    magic: &CrystalMagicTemplate,
    level: u8,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let mana_cost = hero_magic_mana_cost(magic, level);
    if mana_cost <= 0 {
        return true;
    }

    {
        let mut entry = world.entity_mut(hero_entity);
        let Some(mut vitals) = entry.get_mut::<PlayerVitals>() else {
            return false;
        };
        if vitals.mp < mana_cost {
            return false;
        }
        vitals.mp = vitals.mp.saturating_sub(mana_cost).max(0);
    }

    if let Some(info) = object_mana_info_for_entity(world, hero_entity) {
        packets.push(ServerPacket::ObjectMana { info });
    }
    true
}

fn hero_archer_concentration_duration_ticks(level: u8) -> u64 {
    combat_delay_ticks((45_u64 + u64::from(level).saturating_mul(15)).saturating_mul(1_000))
}

fn prepare_hero_archer_concentration(
    world: &mut World,
    tick: u64,
    hero_entity: Entity,
    hero_id: u32,
    class: Option<MirClass>,
    level: Option<u16>,
    packets: &mut Vec<ServerPacket>,
) {
    let Some((magic, skill_level)) = hero_crystal_magic_for_class(
        world,
        class,
        level,
        MirClass::Archer,
        "Concentration",
        Spell::Concentration,
    ) else {
        return;
    };
    let state = hero_ai_skill_state(world, hero_id);
    if tick < state.next_concentration_tick || tick < state.concentration_expires_at {
        return;
    }
    if !hero_try_consume_magic_mp(world, hero_entity, &magic, skill_level, packets) {
        return;
    }

    packets.push(ServerPacket::SetConcentration {
        object_id: hero_id,
        enabled: true,
        interrupted: false,
    });
    advance_hero_learned_magic(
        world,
        tick,
        hero_id,
        Spell::Concentration,
        &magic,
        level,
        packets,
    );
    update_hero_ai_skill_state(world, hero_id, |state| {
        state.concentration_expires_at =
            tick.saturating_add(hero_archer_concentration_duration_ticks(skill_level));
        state.next_concentration_tick =
            tick.saturating_add(hero_magic_cooldown_ticks(&magic, skill_level));
    });
}

fn hero_archer_shot_spell(
    world: &mut World,
    tick: u64,
    hero_entity: Entity,
    hero_id: u32,
    class: Option<MirClass>,
    level: Option<u16>,
    packets: &mut Vec<ServerPacket>,
) -> Option<(Spell, u8)> {
    let (magic, skill_level) = hero_crystal_magic_for_class(
        world,
        class,
        level,
        MirClass::Archer,
        "StraightShot",
        Spell::StraightShot,
    )?;
    if tick < hero_ai_skill_state(world, hero_id).next_shot_tick {
        return None;
    }
    if !hero_try_consume_magic_mp(world, hero_entity, &magic, skill_level, packets) {
        return None;
    }

    update_hero_ai_skill_state(world, hero_id, |state| {
        state.next_shot_tick = tick.saturating_add(hero_magic_cooldown_ticks(&magic, skill_level));
    });
    advance_hero_learned_magic(
        world,
        tick,
        hero_id,
        Spell::StraightShot,
        &magic,
        level,
        packets,
    );
    Some((Spell::StraightShot, skill_level))
}

fn hero_heal_amount(
    world: &World,
    hero_level: Option<u16>,
    magic: &CrystalMagicTemplate,
    skill_level: u8,
) -> i32 {
    let tao_power = hero_inventory_crystal_stat_total(world, CRYSTAL_STAT_MAX_SC)
        .max(hero_inventory_crystal_stat_total(
            world,
            CRYSTAL_STAT_MIN_SC,
        ))
        .max(0);
    crystal_magic_damage(magic, skill_level)
        .saturating_add(tao_power.saturating_mul(2))
        .saturating_add(i32::from(hero_level.unwrap_or(1).max(1)))
        .max(1)
}

fn hero_wizard_choice_for_spell(
    world: &World,
    class: Option<MirClass>,
    level: Option<u16>,
    name: &str,
    spell: Spell,
    area: HeroWizardSpellArea,
) -> Option<HeroWizardSpellChoice> {
    let (magic, skill_level) =
        hero_crystal_magic_for_class(world, class, level, MirClass::Wizard, name, spell)?;
    Some(HeroWizardSpellChoice {
        spell,
        magic,
        skill_level,
        area,
    })
}

fn hero_monster_level(world: &World, entity: Entity) -> u16 {
    entity_name(world, entity)
        .and_then(|name| crystal_monster_by_name(&name).map(|template| template.level))
        .unwrap_or_default()
}

fn hero_monster_is_undead(world: &World, entity: Entity) -> bool {
    entity_name(world, entity)
        .and_then(|name| crystal_monster_by_name(&name).map(|template| template.undead))
        .unwrap_or(false)
}

fn hero_wizard_spell_choices(
    world: &World,
    class: Option<MirClass>,
    level: Option<u16>,
    hero_position: &Point,
    target: &HeroAiTarget,
    distance: i32,
) -> Vec<HeroWizardSpellChoice> {
    if class != Some(MirClass::Wizard) {
        return Vec::new();
    }

    let mut choices = Vec::new();
    let hero_level = level.unwrap_or(1);
    if distance == 1 && hero_monster_level(world, target.entity) < hero_level {
        if let Some(choice) = hero_wizard_choice_for_spell(
            world,
            class,
            level,
            "Repulsion",
            Spell::Repulsion,
            HeroWizardSpellArea::Repulsion,
        ) {
            choices.push(choice);
        }
    }

    let surrounded_count = hero_hostile_monsters_in_square(world, hero_position, 2).len();
    if distance < 3 && surrounded_count > 1 {
        for (name, spell) in [
            ("FlameField", Spell::FlameField),
            ("ThunderStorm", Spell::ThunderStorm),
        ] {
            if let Some(choice) = hero_wizard_choice_for_spell(
                world,
                class,
                level,
                name,
                spell,
                HeroWizardSpellArea::CasterSquare { radius: 2 },
            ) {
                choices.push(choice);
            }
        }
    }

    let target_surrounded_count = hero_hostile_monsters_in_square(world, &target.position, 1).len();
    if target_surrounded_count > 1 {
        for (name, spell) in [("IceStorm", Spell::IceStorm), ("FireBang", Spell::FireBang)] {
            if let Some(choice) = hero_wizard_choice_for_spell(
                world,
                class,
                level,
                name,
                spell,
                HeroWizardSpellArea::TargetSquare { radius: 1 },
            ) {
                choices.push(choice);
            }
        }
    }

    if hero_monster_is_undead(world, target.entity)
        && hero_monster_level(world, target.entity) < hero_level
    {
        if let Some(choice) = hero_wizard_choice_for_spell(
            world,
            class,
            level,
            "TurnUndead",
            Spell::TurnUndead,
            HeroWizardSpellArea::Single,
        ) {
            choices.push(choice);
        }
    }

    for (name, spell) in [
        ("FlameDisruptor", Spell::FlameDisruptor),
        ("Vampirism", Spell::Vampirism),
        ("FrostCrunch", Spell::FrostCrunch),
    ] {
        if let Some(choice) = hero_wizard_choice_for_spell(
            world,
            class,
            level,
            name,
            spell,
            HeroWizardSpellArea::Single,
        ) {
            choices.push(choice);
        }
    }

    for (name, spell) in [
        ("ThunderBolt", Spell::ThunderBolt),
        ("GreatFireBall", Spell::GreatFireBall),
        ("FireBall", Spell::FireBall),
    ] {
        if let Some(choice) = hero_wizard_choice_for_spell(
            world,
            class,
            level,
            name,
            spell,
            HeroWizardSpellArea::Single,
        ) {
            choices.push(choice);
        }
    }

    choices
}

fn hero_wizard_magic_damage(
    world: &World,
    level: Option<u16>,
    magic: &CrystalMagicTemplate,
    skill_level: u8,
) -> i32 {
    let magic_power = hero_inventory_crystal_stat_total(world, CRYSTAL_STAT_MAX_MC)
        .max(hero_inventory_crystal_stat_total(
            world,
            CRYSTAL_STAT_MIN_MC,
        ))
        .max(0);
    let level_bonus = i32::from(level.unwrap_or(1).max(1)) / 3;
    hero_magic_damage_from_base(magic, skill_level, magic_power.saturating_add(level_bonus)).max(1)
}

fn hero_wizard_magic_shield_duration_ticks(
    world: &World,
    magic: &CrystalMagicTemplate,
    skill_level: u8,
) -> u64 {
    let magic_power = hero_inventory_crystal_stat_total(world, CRYSTAL_STAT_MAX_MC)
        .max(hero_inventory_crystal_stat_total(
            world,
            CRYSTAL_STAT_MIN_MC,
        ))
        .max(0);
    let duration_seconds = crystal_magic_damage(magic, skill_level)
        .saturating_add(magic_power)
        .saturating_add(15)
        .max(1);
    combat_delay_ticks(
        u64::try_from(duration_seconds)
            .unwrap_or(1)
            .saturating_mul(1_000),
    )
}

fn hero_wizard_magic_booster_duration_ticks() -> u64 {
    combat_delay_ticks(60_000)
}

fn hero_buff_packet(
    hero_id: u32,
    buff_type: u8,
    visible: bool,
    duration_ticks: u64,
    stats: Vec<UserItemStat>,
) -> ServerPacket {
    ServerPacket::AddBuff {
        buff: ClientBuff {
            buff_type,
            visible,
            object_id: hero_id,
            expire_time: duration_ticks.saturating_mul(1_000).min(i64::MAX as u64) as i64,
            infinite: false,
            paused: false,
            stats,
            values: Vec::new(),
        },
    }
}

fn tick_hero_wizard_support(
    world: &mut World,
    tick: u64,
    hero_entity: Entity,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let (hero_id, hero_position, hero_direction, class, level) = {
        let hero = world.entity(hero_entity);
        let Some(hero_id) = hero.get::<ObjectId>().map(|id| id.0) else {
            return false;
        };
        let Some(hero_position) = hero.get::<Position>().map(|position| position.0.clone()) else {
            return false;
        };
        let hero_direction = hero
            .get::<Facing>()
            .map(|facing| facing.0)
            .unwrap_or(MirDirection::Down);
        let body = hero.get::<CharacterBody>();
        (
            hero_id,
            hero_position,
            hero_direction,
            body.map(|body| body.class),
            body.map(|body| body.level),
        )
    };
    if class != Some(MirClass::Wizard) {
        return false;
    }

    let state = hero_ai_skill_state(world, hero_id);
    if tick < state.next_wizard_spell_tick {
        return false;
    }

    if tick >= state.magic_shield_expires_at {
        if let Some((magic, skill_level)) = hero_crystal_magic_for_class(
            world,
            class,
            level,
            MirClass::Wizard,
            "MagicShield",
            Spell::MagicShield,
        ) {
            if !hero_try_consume_magic_mp(world, hero_entity, &magic, skill_level, packets) {
                return false;
            }
            let duration_ticks =
                hero_wizard_magic_shield_duration_ticks(world, &magic, skill_level);
            packets.push(ServerPacket::ObjectMagic {
                object_id: hero_id,
                location: hero_position.clone(),
                direction: hero_direction,
                spell: Spell::MagicShield,
                target_id: hero_id,
                target: hero_position.clone(),
                cast: true,
                level: skill_level,
                self_broadcast: false,
                secondary_target_ids: Vec::new(),
            });
            packets.push(hero_buff_packet(
                hero_id,
                HERO_BUFF_MAGIC_SHIELD,
                false,
                duration_ticks,
                vec![UserItemStat {
                    stat: CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT,
                    value: (i32::from(skill_level) + 2) * 10,
                }],
            ));
            advance_hero_learned_magic(
                world,
                tick,
                hero_id,
                Spell::MagicShield,
                &magic,
                level,
                packets,
            );
            let cooldown =
                hero_magic_cooldown_ticks(&magic, skill_level).max(HERO_ATTACK_INTERVAL_TICKS);
            update_hero_ai_skill_state(world, hero_id, |state| {
                state.next_wizard_spell_tick = tick.saturating_add(cooldown);
                state.magic_shield_expires_at = tick.saturating_add(duration_ticks);
            });
            if let Some(mut hero) = world.entity_mut(hero_entity).get_mut::<Hero>() {
                hero.next_attack_tick = hero.next_attack_tick.max(tick.saturating_add(cooldown));
            }
            return true;
        }
    }

    if tick >= state.magic_booster_expires_at {
        if let Some((magic, skill_level)) = hero_crystal_magic_for_class(
            world,
            class,
            level,
            MirClass::Wizard,
            "MagicBooster",
            Spell::MagicBooster,
        ) {
            if !hero_try_consume_magic_mp(world, hero_entity, &magic, skill_level, packets) {
                return false;
            }
            let duration_ticks = hero_wizard_magic_booster_duration_ticks();
            let magic_bonus = 6 + i32::from(skill_level) * 6;
            packets.push(ServerPacket::ObjectMagic {
                object_id: hero_id,
                location: hero_position.clone(),
                direction: hero_direction,
                spell: Spell::MagicBooster,
                target_id: hero_id,
                target: hero_position.clone(),
                cast: true,
                level: skill_level,
                self_broadcast: false,
                secondary_target_ids: Vec::new(),
            });
            packets.push(hero_buff_packet(
                hero_id,
                HERO_BUFF_MAGIC_BOOSTER,
                true,
                duration_ticks,
                vec![
                    UserItemStat {
                        stat: CRYSTAL_STAT_MIN_MC,
                        value: magic_bonus,
                    },
                    UserItemStat {
                        stat: CRYSTAL_STAT_MAX_MC,
                        value: magic_bonus,
                    },
                    UserItemStat {
                        stat: CRYSTAL_STAT_MANA_PENALTY_PERCENT,
                        value: 6 + i32::from(skill_level),
                    },
                ],
            ));
            advance_hero_learned_magic(
                world,
                tick,
                hero_id,
                Spell::MagicBooster,
                &magic,
                level,
                packets,
            );
            let cooldown =
                hero_magic_cooldown_ticks(&magic, skill_level).max(HERO_ATTACK_INTERVAL_TICKS);
            update_hero_ai_skill_state(world, hero_id, |state| {
                state.next_wizard_spell_tick = tick.saturating_add(cooldown);
                state.magic_booster_expires_at = tick.saturating_add(duration_ticks);
            });
            if let Some(mut hero) = world.entity_mut(hero_entity).get_mut::<Hero>() {
                hero.next_attack_tick = hero.next_attack_tick.max(tick.saturating_add(cooldown));
            }
            return true;
        }
    }

    false
}

fn hero_heal_target(
    world: &World,
    hero_entity: Entity,
    hero_position: &Point,
    range: i32,
) -> Option<HeroHealTarget> {
    [player_entity(world), Some(hero_entity)]
        .into_iter()
        .flatten()
        .filter_map(|entity| {
            let entry = world.entity(entity);
            let vitals = entry.get::<PlayerVitals>()?;
            if vitals.hp <= 0 || vitals.hp >= vitals.max_hp {
                return None;
            }
            let position = entry.get::<Position>()?.0.clone();
            if tile_distance(hero_position, &position) > range {
                return None;
            }
            Some(HeroHealTarget {
                entity,
                object_id: entry.get::<ObjectId>()?.0,
                position,
                hp: vitals.hp,
                max_hp: vitals.max_hp,
            })
        })
        .min_by_key(|target| {
            let missing_percent = ((target.max_hp - target.hp).max(0) * 100) / target.max_hp.max(1);
            (
                if Some(target.entity) == player_entity(world) {
                    0
                } else {
                    1
                },
                -missing_percent,
                target.object_id,
            )
        })
}

fn tick_hero_taoist_heal(
    world: &mut World,
    tick: u64,
    hero_entity: Entity,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let (hero_id, hero_position, hero_direction, class, level) = {
        let hero = world.entity(hero_entity);
        let Some(hero_id) = hero.get::<ObjectId>().map(|id| id.0) else {
            return false;
        };
        let Some(hero_position) = hero.get::<Position>().map(|position| position.0.clone()) else {
            return false;
        };
        let hero_direction = hero
            .get::<Facing>()
            .map(|facing| facing.0)
            .unwrap_or(MirDirection::Down);
        let body = hero.get::<CharacterBody>();
        (
            hero_id,
            hero_position,
            hero_direction,
            body.map(|body| body.class),
            body.map(|body| body.level),
        )
    };
    let Some((magic, skill_level)) = hero_crystal_magic_for_class(
        world,
        class,
        level,
        MirClass::Taoist,
        "Healing",
        Spell::Healing,
    ) else {
        return false;
    };
    if tick < hero_ai_skill_state(world, hero_id).next_heal_tick {
        return false;
    }
    let Some(target) = hero_heal_target(world, hero_entity, &hero_position, i32::from(magic.range))
    else {
        return false;
    };
    if !hero_try_consume_magic_mp(world, hero_entity, &magic, skill_level, packets) {
        return false;
    }

    let direction = direction_toward(&hero_position, &target.position).unwrap_or(hero_direction);
    packets.push(ServerPacket::ObjectMagic {
        object_id: hero_id,
        location: hero_position,
        direction,
        spell: Spell::Healing,
        target_id: target.object_id,
        target: target.position.clone(),
        cast: true,
        level: skill_level,
        self_broadcast: false,
        secondary_target_ids: Vec::new(),
    });

    let heal = hero_heal_amount(world, level, &magic, skill_level);
    let restored = {
        let mut entry = world.entity_mut(target.entity);
        entry.get_mut::<PlayerVitals>().map(|mut vitals| {
            vitals.hp = vitals.hp.saturating_add(heal).min(vitals.max_hp);
            *vitals
        })
    };
    if let Some(restored) = restored {
        if Some(target.entity) == player_entity(world) {
            world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored;
        }
        if let Some(info) = object_health_info_for_entity(world, target.entity, 0) {
            packets.push(ServerPacket::ObjectHealth { info });
        }
    }

    advance_hero_learned_magic(world, tick, hero_id, Spell::Healing, &magic, level, packets);
    let cooldown = hero_magic_cooldown_ticks(&magic, skill_level).max(HERO_ATTACK_INTERVAL_TICKS);
    update_hero_ai_skill_state(world, hero_id, |state| {
        state.next_heal_tick = tick.saturating_add(cooldown);
    });
    if let Some(mut hero) = world.entity_mut(hero_entity).get_mut::<Hero>() {
        hero.next_attack_tick = hero.next_attack_tick.max(tick.saturating_add(cooldown));
    }
    true
}

fn hero_attack_spell(
    world: &mut World,
    tick: u64,
    hero_entity: Entity,
    hero_id: u32,
    class: Option<MirClass>,
    level: Option<u16>,
    ranged: bool,
    packets: &mut Vec<ServerPacket>,
) -> (Spell, u8) {
    if ranged {
        prepare_hero_archer_concentration(world, tick, hero_entity, hero_id, class, level, packets);
        if let Some(shot) =
            hero_archer_shot_spell(world, tick, hero_entity, hero_id, class, level, packets)
        {
            return shot;
        }
        return (Spell::None, 0);
    }
    if let Some(level) = hero_flaming_sword_skill_level(world, class, level) {
        return (Spell::FlamingSword, level);
    }
    if let Some(level) = hero_slaying_skill_level(world, class, level) {
        return (Spell::Slaying, level);
    }
    (Spell::None, 0)
}

#[allow(deprecated)]
fn nearest_hero_target(
    world: &World,
    hero_position: &Point,
    behaviour: u8,
) -> Option<HeroAiTarget> {
    let mut best: Option<(i32, i32, i32, u32, HeroAiTarget)> = None;

    for entity in world.iter_entities() {
        if entity.get::<Monster>().is_none() {
            continue;
        }
        let Some(agent) = entity.get::<MonsterAgent>() else {
            continue;
        };
        let ai_state = entity.get::<MonsterAiState>().copied().unwrap_or_default();
        let hostile =
            agent.hostile_to_player || agent.disposition == WorldEntityDisposition::Hostile;
        if !hostile || agent.dead || is_hidden_or_sleeping_target(agent, &ai_state) {
            continue;
        }
        if behaviour == HERO_BEHAVIOUR_COUNTER_ATTACK && !agent.tracking_player {
            continue;
        }
        let Some(position) = entity.get::<Position>().map(|position| position.0.clone()) else {
            continue;
        };
        let distance = tile_distance(hero_position, &position);
        if distance > HERO_VIEW_RANGE {
            continue;
        }
        let Some(object_id) = entity.get::<ObjectId>().map(|object_id| object_id.0) else {
            continue;
        };
        let target = HeroAiTarget {
            entity: entity.id(),
            object_id,
            name: entity_name(world, entity.id()).unwrap_or_default(),
            position,
        };
        let key = (
            distance,
            target.position.y,
            target.position.x,
            target.object_id,
            target.clone(),
        );
        if best.as_ref().map_or(true, |current| {
            (key.0, key.1, key.2, key.3) < (current.0, current.1, current.2, current.3)
        }) {
            best = Some(key);
        }
    }

    best.map(|(_, _, _, _, target)| target)
}

#[allow(deprecated)]
fn hero_hostile_monsters_in_square(
    world: &World,
    center: &Point,
    radius: i32,
) -> Vec<HeroAiTarget> {
    world
        .iter_entities()
        .filter_map(|entity| {
            if entity.get::<Monster>().is_none() {
                return None;
            }
            let agent = entity.get::<MonsterAgent>()?;
            let ai_state = entity.get::<MonsterAiState>().copied().unwrap_or_default();
            if agent.dead
                || !agent.hostile_to_player
                || is_hidden_or_sleeping_target(agent, &ai_state)
            {
                return None;
            }
            let position = entity.get::<Position>()?.0.clone();
            let dx = (position.x - center.x).abs();
            let dy = (position.y - center.y).abs();
            if dx > radius || dy > radius {
                return None;
            }
            Some(HeroAiTarget {
                entity: entity.id(),
                object_id: entity.get::<ObjectId>()?.0,
                name: entity_name(world, entity.id()).unwrap_or_default(),
                position,
            })
        })
        .collect()
}

fn mark_hero_magic_target_hit(world: &mut World, target_entity: Entity) {
    if let Some(mut agent) = world.entity_mut(target_entity).get_mut::<MonsterAgent>() {
        if monster_locks_player_target_on_hit(&agent) {
            agent.tracking_player = true;
        }
    }
}

fn schedule_hero_wizard_damage_to_target(
    world: &mut World,
    cast_tick: u64,
    due_tick: u64,
    hero_id: u32,
    hero_entity: Entity,
    level: Option<u16>,
    choice: &HeroWizardSpellChoice,
    target: &HeroAiTarget,
) {
    let mut damage = hero_wizard_magic_damage(world, level, &choice.magic, choice.skill_level);
    if choice.spell == Spell::ThunderStorm && !hero_monster_is_undead(world, target.entity) {
        damage = (damage / 10).max(1);
    }
    if choice.spell == Spell::FlameDisruptor && !hero_monster_is_undead(world, target.entity) {
        damage = damage.saturating_mul(3).saturating_div(2).max(1);
    }
    if choice.spell == Spell::TurnUndead {
        if !hero_monster_is_undead(world, target.entity)
            || hero_monster_level(world, target.entity) >= level.unwrap_or(1)
        {
            return;
        }
        if let Some(vitals) = world.entity(target.entity).get::<MonsterVitals>() {
            damage = vitals.hp.max(damage).max(1);
        }
    }
    schedule_damage_to_monster(
        world,
        due_tick,
        hero_id,
        target.entity,
        damage,
        Some(target.name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: target.object_id,
            name: target.name.clone(),
        }),
    );
    if choice.spell == Spell::Vampirism {
        let heal = damage
            .saturating_mul(i32::from(choice.skill_level).saturating_add(1))
            .saturating_div(3)
            .max(1);
        queue_hero_ai_pending_heal(world, due_tick, hero_entity, heal);
    }
    if choice.spell == Spell::FrostCrunch {
        let freeze_ms = (2_u64 + u64::from(choice.skill_level)).saturating_mul(1_000);
        let release_tick = cast_tick.saturating_add(combat_delay_ticks(freeze_ms));
        if let Some(mut agent) = world.entity_mut(target.entity).get_mut::<MonsterAgent>() {
            agent.next_move_tick = agent.next_move_tick.max(release_tick);
            agent.next_attack_tick = agent.next_attack_tick.max(release_tick);
        }
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::AddBuff {
                buff: ClientBuff {
                    buff_type: crystal_buff_type_for_key("magic").unwrap_or(201),
                    visible: false,
                    object_id: target.object_id,
                    expire_time: i64::try_from(freeze_ms).unwrap_or(i64::MAX),
                    infinite: false,
                    paused: false,
                    stats: Vec::new(),
                    values: Vec::new(),
                },
            },
        );
    }
    mark_hero_magic_target_hit(world, target.entity);
}

fn apply_hero_wizard_repulsion(
    world: &mut World,
    _tick: u64,
    hero_id: u32,
    hero_position: &Point,
    level: Option<u16>,
    choice: &HeroWizardSpellChoice,
    target: &HeroAiTarget,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(start) = entity_position(world, target.entity) else {
        return;
    };
    let Some(push_direction) = direction_toward(hero_position, &start) else {
        return;
    };
    let reverse = rotated_direction(push_direction, 4);
    let mut position = start;
    let push_distance = 1_i32.saturating_add(i32::from(choice.skill_level));
    let mut pushed = 0_i32;

    for _ in 0..push_distance.max(1) {
        let next = offset_point(&position, push_direction, 1);
        if !can_occupy(world, next.clone(), Some(target.entity)) {
            break;
        }
        position = next.clone();
        world
            .entity_mut(target.entity)
            .insert((Position(next.clone()), Facing(reverse)));
        packets.push(ServerPacket::ObjectPushed {
            object_id: target.object_id,
            location: next,
            direction: reverse,
        });
        pushed += 1;
    }

    if pushed == 0 {
        return;
    }

    mark_hero_magic_target_hit(world, target.entity);
    let thunder_element_repulsion = world
        .entity(target.entity)
        .get::<MonsterAgent>()
        .map(|agent| agent.ai == 49)
        .unwrap_or(false);
    if !thunder_element_repulsion {
        return;
    }

    if let Some(packet) = object_struck_packet(world, target.entity, hero_id) {
        packets.push(packet);
    }
    let damage = hero_wizard_magic_damage(world, level, &choice.magic, choice.skill_level)
        .max(50)
        .saturating_mul(pushed.max(1));
    if let Some(mut vitals) = world.entity_mut(target.entity).get_mut::<MonsterVitals>() {
        vitals.hp = (vitals.hp - damage).max(1);
    }
    if let Some(info) = object_health_info_for_entity(world, target.entity, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
}

fn cast_hero_wizard_spell_choice(
    world: &mut World,
    tick: u64,
    hero_entity: Entity,
    hero_id: u32,
    hero_position: &Point,
    direction: MirDirection,
    level: Option<u16>,
    target: &HeroAiTarget,
    choice: &HeroWizardSpellChoice,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if !hero_try_consume_magic_mp(
        world,
        hero_entity,
        &choice.magic,
        choice.skill_level,
        packets,
    ) {
        return false;
    }

    packets.push(ServerPacket::ObjectMagic {
        object_id: hero_id,
        location: hero_position.clone(),
        direction,
        spell: choice.spell,
        target_id: target.object_id,
        target: target.position.clone(),
        cast: true,
        level: choice.skill_level,
        self_broadcast: false,
        secondary_target_ids: Vec::new(),
    });

    match choice.area {
        HeroWizardSpellArea::Single => {
            schedule_hero_wizard_damage_to_target(
                world,
                tick,
                tick + ranged_attack_delay_ticks(hero_position, &target.position),
                hero_id,
                hero_entity,
                level,
                choice,
                target,
            );
        }
        HeroWizardSpellArea::CasterSquare { radius } => {
            let due_tick = tick + combat_delay_ticks(500);
            for area_target in hero_hostile_monsters_in_square(world, hero_position, radius) {
                schedule_hero_wizard_damage_to_target(
                    world,
                    tick,
                    due_tick,
                    hero_id,
                    hero_entity,
                    level,
                    choice,
                    &area_target,
                );
            }
        }
        HeroWizardSpellArea::TargetSquare { radius } => {
            let due_tick = tick + combat_delay_ticks(500);
            for area_target in hero_hostile_monsters_in_square(world, &target.position, radius) {
                schedule_hero_wizard_damage_to_target(
                    world,
                    tick,
                    due_tick,
                    hero_id,
                    hero_entity,
                    level,
                    choice,
                    &area_target,
                );
            }
        }
        HeroWizardSpellArea::Repulsion => {
            apply_hero_wizard_repulsion(
                world,
                tick,
                hero_id,
                hero_position,
                level,
                choice,
                target,
                packets,
            );
        }
    }

    advance_hero_learned_magic(
        world,
        tick,
        hero_id,
        choice.spell,
        &choice.magic,
        level,
        packets,
    );
    let cooldown = hero_magic_cooldown_ticks(&choice.magic, choice.skill_level)
        .max(HERO_ATTACK_INTERVAL_TICKS);
    update_hero_ai_skill_state(world, hero_id, |state| {
        state.next_wizard_spell_tick = tick.saturating_add(cooldown);
    });
    if let Some(mut hero) = world.entity_mut(hero_entity).get_mut::<Hero>() {
        hero.next_attack_tick = tick.saturating_add(cooldown);
    }
    true
}

fn hero_attack_packet(
    hero_id: u32,
    hero_position: &Point,
    direction: MirDirection,
    target: &HeroAiTarget,
    ranged: bool,
    spell: Spell,
    spell_level: u8,
) -> ServerPacket {
    if ranged {
        ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id: hero_id,
                location: hero_position.clone(),
                direction,
                target_id: target.object_id,
                target: target.position.clone(),
                attack_type: 0,
                spell: spell as u8,
                level: spell_level,
            },
        }
    } else {
        ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: hero_id,
                location: hero_position.clone(),
                direction,
                spell: spell as u8,
                level: spell_level,
                attack_type: 0,
            },
        }
    }
}

fn hero_progression_spell_name(spell: Spell) -> Option<&'static str> {
    match spell {
        Spell::Slaying => Some("Slaying"),
        _ => hero_damage_spell_name(spell),
    }
}

fn tick_hero_attack(
    world: &mut World,
    tick: u64,
    hero_entity: Entity,
    target: HeroAiTarget,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let (hero_id, hero_position, class, level, next_attack_tick) = {
        let hero = world.entity(hero_entity);
        let Some(hero_id) = hero.get::<ObjectId>().map(|id| id.0) else {
            return false;
        };
        let Some(hero_position) = hero.get::<Position>().map(|position| position.0.clone()) else {
            return false;
        };
        let body = hero.get::<CharacterBody>();
        let next_attack_tick = hero
            .get::<Hero>()
            .map(|hero| hero.next_attack_tick)
            .unwrap_or_default();
        (
            hero_id,
            hero_position,
            body.map(|body| body.class),
            body.map(|body| body.level),
            next_attack_tick,
        )
    };
    if tick < next_attack_tick {
        return false;
    }
    let Some(direction) = direction_toward(&hero_position, &target.position) else {
        return false;
    };

    let distance = tile_distance(&hero_position, &target.position);
    let attack_range = hero_attack_range(world, class);
    if distance == 0 || distance > attack_range {
        return false;
    }

    if class == Some(MirClass::Wizard) {
        if tick >= hero_ai_skill_state(world, hero_id).next_wizard_spell_tick {
            for choice in
                hero_wizard_spell_choices(world, class, level, &hero_position, &target, distance)
            {
                if cast_hero_wizard_spell_choice(
                    world,
                    tick,
                    hero_entity,
                    hero_id,
                    &hero_position,
                    direction,
                    level,
                    &target,
                    &choice,
                    packets,
                ) {
                    return true;
                }
            }
        }

        if distance > HERO_MELEE_RANGE {
            return false;
        }
    }

    let ranged = class == Some(MirClass::Archer) && distance > HERO_MELEE_RANGE;
    let (attack_spell, attack_spell_level) = hero_attack_spell(
        world,
        tick,
        hero_entity,
        hero_id,
        class,
        level,
        ranged,
        packets,
    );
    packets.push(hero_attack_packet(
        hero_id,
        &hero_position,
        direction,
        &target,
        ranged,
        attack_spell,
        attack_spell_level,
    ));

    let due_tick = if ranged {
        tick + ranged_attack_delay_ticks(&hero_position, &target.position)
    } else {
        tick + melee_attack_delay_ticks()
    };
    schedule_damage_to_monster(
        world,
        due_tick,
        hero_id,
        target.entity,
        hero_attack_damage(world, class, level, attack_spell, attack_spell_level),
        Some(target.name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: target.object_id,
            name: target.name,
        }),
    );
    if !ranged && attack_spell != Spell::None {
        if let Some(magic) =
            hero_progression_spell_name(attack_spell).and_then(crystal_magic_by_spell)
        {
            advance_hero_learned_magic(world, tick, hero_id, attack_spell, &magic, level, packets);
        }
    }

    if let Some(mut hero) = world.entity_mut(hero_entity).get_mut::<Hero>() {
        hero.next_attack_tick = tick + HERO_ATTACK_INTERVAL_TICKS;
    }
    if let Some(mut agent) = world.entity_mut(target.entity).get_mut::<MonsterAgent>() {
        if monster_locks_player_target_on_hit(&agent) {
            agent.tracking_player = true;
        }
    }
    true
}

fn tick_hero_move_toward(
    world: &mut World,
    tick: u64,
    hero_entity: Entity,
    target_position: &Point,
    packets: &mut Vec<ServerPacket>,
) {
    let (hero_id, hero_position, next_move_tick) = {
        let hero = world.entity(hero_entity);
        let Some(hero_id) = hero.get::<ObjectId>().map(|id| id.0) else {
            return;
        };
        let Some(hero_position) = hero.get::<Position>().map(|position| position.0.clone()) else {
            return;
        };
        let next_move_tick = hero
            .get::<Hero>()
            .map(|hero| hero.next_move_tick)
            .unwrap_or_default();
        (hero_id, hero_position, next_move_tick)
    };

    if tick < next_move_tick || tile_distance(&hero_position, target_position) <= HERO_MELEE_RANGE {
        return;
    }

    let Some(base_direction) = direction_toward(&hero_position, target_position) else {
        return;
    };
    let mut next_step = None;
    for offset in [0, 1, -1, 2, -2, 3, -3, 4] {
        let direction = rotated_direction(base_direction, offset);
        let destination = offset_point(&hero_position, direction, 1);
        if can_occupy(world, destination.clone(), Some(hero_entity)) {
            next_step = Some((destination, direction));
            break;
        }
    }
    let Some((destination, direction)) = next_step else {
        return;
    };

    world
        .entity_mut(hero_entity)
        .insert((Position(destination.clone()), Facing(direction)));
    if let Some(mut hero) = world.entity_mut(hero_entity).get_mut::<Hero>() {
        hero.next_move_tick = tick + HERO_MOVE_INTERVAL_TICKS;
    }
    packets.push(ServerPacket::ObjectWalk {
        movement: ObjectMovement {
            object_id: hero_id,
            position: destination,
            direction,
        },
    });
}

pub(super) fn tick_stage5_hero_combat_ai(
    world: &mut World,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    tick_hero_ai_pending_heals(world, tick, packets);

    let Some(behaviour) = stage5_hero_behaviour(world) else {
        return;
    };
    if !matches!(
        behaviour,
        HERO_BEHAVIOUR_ATTACK | HERO_BEHAVIOUR_COUNTER_ATTACK | HERO_BEHAVIOUR_FOLLOW
    ) {
        return;
    }

    let Some(hero_entity) = hero_entity(world) else {
        return;
    };
    if tick_hero_taoist_heal(world, tick, hero_entity, packets) {
        return;
    }
    if behaviour == HERO_BEHAVIOUR_FOLLOW {
        return;
    }
    let Some(hero_position) = entity_position(world, hero_entity) else {
        return;
    };
    let Some(target) = nearest_hero_target(world, &hero_position, behaviour) else {
        return;
    };
    if tick_hero_wizard_support(world, tick, hero_entity, packets) {
        return;
    }
    let class = world
        .entity(hero_entity)
        .get::<CharacterBody>()
        .map(|body| body.class);

    if tile_distance(&hero_position, &target.position) <= hero_attack_range(world, class)
        && tick_hero_attack(world, tick, hero_entity, target.clone(), packets)
    {
        return;
    }

    if behaviour == HERO_BEHAVIOUR_ATTACK {
        tick_hero_move_toward(world, tick, hero_entity, &target.position, packets);
    }
}

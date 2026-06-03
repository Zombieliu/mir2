use serde::{Deserialize, Serialize};

use crate::config::SkillSnapshot;
use crate::EquipmentSlot;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_magic_by_spell, crystal_magic_manifest, crystal_monster_by_name,
    localized_text_or_fallback, starter_server_data, CrystalItemTemplate, CrystalMagicTemplate,
    CrystalRespawnTemplate, LanguageCode, SkillEffectTemplate, SkillTemplate,
};
use mir2_protocol::{
    ClientBuff, ClientMagic, MirClass, MirDirection, ObjectAttackInfo, ObjectEffectInfo,
    ObjectMovement, ObjectSpellInfo, Point, ServerPacket, Spell, UserItemStat, UserLocation,
};

use super::buffs::{
    apply_or_refresh_buff, buff_metadata, client_buff_packet_for_state, crystal_buff_type_for_key,
    BuffState,
};
use super::combat::{
    apply_monster_poison, combat_delay_ticks, damage_monster_entity, queue_due_packet,
    queued_before_world_tick_due_tick, ranged_attack_delay_ticks, schedule_damage_to_monster,
    schedule_heal_to_player, set_cast_defence, CrystalDefence, PendingMonsterDefeatAction,
};
use super::components::{
    current_player_object_id, entity_by_object_id, entity_facing, entity_name, entity_object_id,
    entity_player_vitals, entity_position, hero_entity, player_entity, CharacterBody, DisplayName,
    MonsterAgent, MonsterVitals, PlayerVitals, Position, SummonedMonster,
};
use super::crystal_compat::{
    CRYSTAL_ITEM_TYPE_AMULET, CRYSTAL_STAT_ACCURACY, CRYSTAL_STAT_AGILITY,
    CRYSTAL_STAT_ATTACK_SPEED, CRYSTAL_STAT_ATTACK_SPEED_RATE_PERCENT,
    CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT, CRYSTAL_STAT_ENERGY_SHIELD_HP_GAIN,
    CRYSTAL_STAT_ENERGY_SHIELD_PERCENT, CRYSTAL_STAT_LUCK, CRYSTAL_STAT_MANA_PENALTY_PERCENT,
    CRYSTAL_STAT_MAX_AC, CRYSTAL_STAT_MAX_DC, CRYSTAL_STAT_MAX_DC_RATE_PERCENT,
    CRYSTAL_STAT_MAX_MAC, CRYSTAL_STAT_MAX_MC, CRYSTAL_STAT_MAX_MC_RATE_PERCENT,
    CRYSTAL_STAT_MAX_SC, CRYSTAL_STAT_MAX_SC_RATE_PERCENT, CRYSTAL_STAT_MIN_AC,
    CRYSTAL_STAT_MIN_DC, CRYSTAL_STAT_MIN_MC, CRYSTAL_STAT_MIN_SC, CRYSTAL_STAT_POISON_ATTACK,
    CRYSTAL_STAT_SKILL_GAIN_MULTIPLIER, CRYSTAL_STAT_TELEPORT_MANA_PENALTY_PERCENT,
};
use super::equipment::equipment_slot_unique_id;
use super::items::{
    crystal_item_template_for_dynamic_key, current_player_required_stat_total,
    merged_user_item_stats,
};
use super::map::{
    current_map_disallows_random_teleport, current_map_disallows_reincarnation, is_safe_zone_point,
};
use super::monsters::{
    active_summoned_monster_count, allocate_runtime_monster_object_id,
    crystal_dynamic_monster_template, deterministic_roll, queue_pending_monster_spawn,
    PendingMonsterSpawnAction,
};
use super::movement::{
    can_occupy, direction_toward, is_blocked_tile, offset_point, rotated_direction,
    summon_spawn_position_near, tile_distance,
};
use super::packets::{
    object_died_info_for_entity, object_health_info_for_entity, object_mana_info_for_entity,
    object_struck_packet,
};
use super::resources::{
    is_in_world, BuffResource, ElementalResource, InventoryResource, PendingGroundSpellAction,
    PlayerRuntimeResource, RuntimeConfigResource, RuntimeQueueResource, SessionResource,
    SkillResource, Stage5SystemsResource,
};
use super::session::SimulationSession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SkillState {
    pub(super) key: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) level: u8,
    #[serde(default)]
    pub(super) experience: u16,
    #[serde(default)]
    pub(super) hotkey: u8,
    pub(super) cooldown_ticks: u32,
    #[serde(default)]
    pub(super) delay_ms: i64,
    pub(super) cooldown_ends_at: u64,
    #[serde(default)]
    pub(super) cast_time_ms: i64,
}

impl SkillState {
    pub(super) fn snapshot(&self, tick: u64, language: LanguageCode) -> SkillSnapshot {
        let metadata = skill_cast_metadata_for_skill_key(&self.key);
        SkillSnapshot {
            key: self.key.clone(),
            name: localized_skill_name(language, &self.key, &self.name),
            description: localized_skill_description(language, &self.key, &self.description),
            spell: metadata.spell.map(str::to_string),
            cast_kind: metadata.cast_kind.as_str().to_string(),
            offensive: metadata.offensive,
            level: self.level,
            experience: self.experience,
            hotkey: self.hotkey,
            delay_ms: self.delay_ms,
            cast_time_ms: self.cast_time_ms,
            cooldown_remaining_ticks: self.cooldown_ends_at.saturating_sub(tick) as u32,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct SkillCastContext {
    pub(super) direction: MirDirection,
    pub(super) target_id: u32,
    pub(super) target: Point,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SkillCastKind {
    Passive,
    Toggle,
    SelfOnly,
    Target,
    Ground,
    Direction,
}

impl SkillCastKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passive => "passive",
            Self::Toggle => "toggle",
            Self::SelfOnly => "self",
            Self::Target => "target",
            Self::Ground => "ground",
            Self::Direction => "direction",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SkillCastMetadata {
    spell: Option<&'static str>,
    cast_kind: SkillCastKind,
    offensive: bool,
}

fn skill_cast_metadata_for_skill_key(key: &str) -> SkillCastMetadata {
    let spell_name = skill_definition(key)
        .and_then(|definition| definition.crystal_spell)
        .or_else(|| crystal_magic_for_skill_key(key).map(|magic| magic.spell));
    let Some(spell_name) = spell_name else {
        return SkillCastMetadata {
            spell: None,
            cast_kind: SkillCastKind::SelfOnly,
            offensive: false,
        };
    };
    let spell = Spell::from_crystal_name(&spell_name);
    let cast_kind = spell
        .map(crystal_spell_cast_kind)
        .unwrap_or(SkillCastKind::Target);
    let offensive = spell.map(crystal_spell_is_offensive).unwrap_or(false);
    SkillCastMetadata {
        spell: canonical_spell_name(spell),
        cast_kind,
        offensive,
    }
}

fn canonical_spell_name(spell: Option<Spell>) -> Option<&'static str> {
    match spell? {
        Spell::None => Some("None"),
        Spell::Fencing => Some("Fencing"),
        Spell::Slaying => Some("Slaying"),
        Spell::Thrusting => Some("Thrusting"),
        Spell::HalfMoon => Some("HalfMoon"),
        Spell::ShoulderDash => Some("ShoulderDash"),
        Spell::TwinDrakeBlade => Some("TwinDrakeBlade"),
        Spell::Entrapment => Some("Entrapment"),
        Spell::FlamingSword => Some("FlamingSword"),
        Spell::LionRoar => Some("LionRoar"),
        Spell::CrossHalfMoon => Some("CrossHalfMoon"),
        Spell::BladeAvalanche => Some("BladeAvalanche"),
        Spell::ProtectionField => Some("ProtectionField"),
        Spell::Rage => Some("Rage"),
        Spell::CounterAttack => Some("CounterAttack"),
        Spell::SlashingBurst => Some("SlashingBurst"),
        Spell::Fury => Some("Fury"),
        Spell::ImmortalSkin => Some("ImmortalSkin"),
        Spell::FireBall => Some("FireBall"),
        Spell::Repulsion => Some("Repulsion"),
        Spell::ElectricShock => Some("ElectricShock"),
        Spell::GreatFireBall => Some("GreatFireBall"),
        Spell::HellFire => Some("HellFire"),
        Spell::ThunderBolt => Some("ThunderBolt"),
        Spell::Teleport => Some("Teleport"),
        Spell::FireBang => Some("FireBang"),
        Spell::FireWall => Some("FireWall"),
        Spell::Lightning => Some("Lightning"),
        Spell::FrostCrunch => Some("FrostCrunch"),
        Spell::ThunderStorm => Some("ThunderStorm"),
        Spell::MagicShield => Some("MagicShield"),
        Spell::TurnUndead => Some("TurnUndead"),
        Spell::Vampirism => Some("Vampirism"),
        Spell::IceStorm => Some("IceStorm"),
        Spell::FlameDisruptor => Some("FlameDisruptor"),
        Spell::Mirroring => Some("Mirroring"),
        Spell::FlameField => Some("FlameField"),
        Spell::Blizzard => Some("Blizzard"),
        Spell::MagicBooster => Some("MagicBooster"),
        Spell::MeteorStrike => Some("MeteorStrike"),
        Spell::IceThrust => Some("IceThrust"),
        Spell::FastMove => Some("FastMove"),
        Spell::StormEscape => Some("StormEscape"),
        Spell::Healing => Some("Healing"),
        Spell::SpiritSword => Some("SpiritSword"),
        Spell::Poisoning => Some("Poisoning"),
        Spell::SoulFireBall => Some("SoulFireBall"),
        Spell::SummonSkeleton => Some("SummonSkeleton"),
        Spell::Hiding => Some("Hiding"),
        Spell::MassHiding => Some("MassHiding"),
        Spell::SoulShield => Some("SoulShield"),
        Spell::Revelation => Some("Revelation"),
        Spell::BlessedArmour => Some("BlessedArmour"),
        Spell::EnergyRepulsor => Some("EnergyRepulsor"),
        Spell::TrapHexagon => Some("TrapHexagon"),
        Spell::Purification => Some("Purification"),
        Spell::MassHealing => Some("MassHealing"),
        Spell::Hallucination => Some("Hallucination"),
        Spell::UltimateEnhancer => Some("UltimateEnhancer"),
        Spell::SummonShinsu => Some("SummonShinsu"),
        Spell::Reincarnation => Some("Reincarnation"),
        Spell::SummonHolyDeva => Some("SummonHolyDeva"),
        Spell::Curse => Some("Curse"),
        Spell::Plague => Some("Plague"),
        Spell::PoisonCloud => Some("PoisonCloud"),
        Spell::EnergyShield => Some("EnergyShield"),
        Spell::PetEnhancer => Some("PetEnhancer"),
        Spell::HealingCircle => Some("HealingCircle"),
        Spell::FatalSword => Some("FatalSword"),
        Spell::DoubleSlash => Some("DoubleSlash"),
        Spell::Haste => Some("Haste"),
        Spell::FlashDash => Some("FlashDash"),
        Spell::LightBody => Some("LightBody"),
        Spell::HeavenlySword => Some("HeavenlySword"),
        Spell::FireBurst => Some("FireBurst"),
        Spell::Trap => Some("Trap"),
        Spell::PoisonSword => Some("PoisonSword"),
        Spell::MoonLight => Some("MoonLight"),
        Spell::MPEater => Some("MPEater"),
        Spell::SwiftFeet => Some("SwiftFeet"),
        Spell::DarkBody => Some("DarkBody"),
        Spell::Hemorrhage => Some("Hemorrhage"),
        Spell::CrescentSlash => Some("CrescentSlash"),
        Spell::MoonMist => Some("MoonMist"),
        Spell::CatTongue => Some("CatTongue"),
        Spell::Focus => Some("Focus"),
        Spell::StraightShot => Some("StraightShot"),
        Spell::DoubleShot => Some("DoubleShot"),
        Spell::ExplosiveTrap => Some("ExplosiveTrap"),
        Spell::DelayedExplosion => Some("DelayedExplosion"),
        Spell::Meditation => Some("Meditation"),
        Spell::BackStep => Some("BackStep"),
        Spell::ElementalShot => Some("ElementalShot"),
        Spell::Concentration => Some("Concentration"),
        Spell::Stonetrap => Some("Stonetrap"),
        Spell::ElementalBarrier => Some("ElementalBarrier"),
        Spell::SummonVampire => Some("SummonVampire"),
        Spell::VampireShot => Some("VampireShot"),
        Spell::SummonToad => Some("SummonToad"),
        Spell::PoisonShot => Some("PoisonShot"),
        Spell::CrippleShot => Some("CrippleShot"),
        Spell::SummonSnakes => Some("SummonSnakes"),
        Spell::NapalmShot => Some("NapalmShot"),
        Spell::OneWithNature => Some("OneWithNature"),
        Spell::BindingShot => Some("BindingShot"),
        Spell::MentalState => Some("MentalState"),
        Spell::Blink => Some("Blink"),
        Spell::Portal => Some("Portal"),
        Spell::BattleCry => Some("BattleCry"),
        Spell::FireBounce => Some("FireBounce"),
        Spell::MeteorShower => Some("MeteorShower"),
        _ => None,
    }
}

fn crystal_spell_cast_kind(spell: Spell) -> SkillCastKind {
    match spell {
        Spell::Fencing
        | Spell::SpiritSword
        | Spell::Focus
        | Spell::Meditation
        | Spell::MentalState
        | Spell::MPEater
        | Spell::Hemorrhage => SkillCastKind::Passive,
        Spell::Slaying
        | Spell::Thrusting
        | Spell::HalfMoon
        | Spell::CrossHalfMoon
        | Spell::DoubleSlash
        | Spell::CounterAttack
        | Spell::FatalSword
        | Spell::FlamingSword
        | Spell::PoisonSword => SkillCastKind::Toggle,
        Spell::Fury
        | Spell::Haste
        | Spell::SwiftFeet
        | Spell::ProtectionField
        | Spell::Rage
        | Spell::LightBody
        | Spell::MoonLight
        | Spell::DarkBody
        | Spell::Hiding
        | Spell::MassHiding
        | Spell::ImmortalSkin
        | Spell::MagicShield
        | Spell::EnergyShield
        | Spell::MagicBooster
        | Spell::Concentration
        | Spell::ElementalBarrier
        | Spell::Mirroring
        | Spell::OneWithNature
        | Spell::Reincarnation
        | Spell::SummonSkeleton
        | Spell::SummonShinsu
        | Spell::SummonHolyDeva
        | Spell::SummonVampire
        | Spell::SummonToad
        | Spell::SummonSnakes
        | Spell::PetEnhancer
        // Taoist support buffs applied to self/allies (no hostile target).
        | Spell::SoulShield
        | Spell::BlessedArmour
        | Spell::UltimateEnhancer
        // Assassin self-centred hide that also mists nearby tiles.
        | Spell::MoonMist
        // Friendly decoy trap placed at the caster's own feet (no target/aim).
        | Spell::Stonetrap => SkillCastKind::SelfOnly,
        Spell::FireWall
        | Spell::FireBang
        | Spell::IceStorm
        | Spell::Blizzard
        | Spell::MeteorStrike
        | Spell::MeteorShower
        | Spell::PoisonCloud
        | Spell::Plague
        | Spell::TrapHexagon
        | Spell::Trap
        | Spell::ExplosiveTrap
        | Spell::DelayedExplosion
        | Spell::Portal
        | Spell::Blink
        | Spell::Teleport
        | Spell::MassHealing
        | Spell::HealingCircle
        | Spell::Curse => SkillCastKind::Ground,
        Spell::HellFire
        | Spell::Lightning
        | Spell::Repulsion
        | Spell::EnergyRepulsor
        | Spell::FireBurst
        | Spell::BackStep
        | Spell::ShoulderDash
        | Spell::FlashDash
        | Spell::StormEscape
        | Spell::BladeAvalanche
        | Spell::SlashingBurst
        | Spell::CrescentSlash
        | Spell::LionRoar
        // Forward / self-centred AoE spells aimed by facing (cast with no locked
        // target), not single-target spells.
        | Spell::IceThrust
        | Spell::HeavenlySword
        | Spell::ThunderStorm
        | Spell::BattleCry => SkillCastKind::Direction,
        _ => SkillCastKind::Target,
    }
}

fn crystal_spell_is_offensive(spell: Spell) -> bool {
    !matches!(
        crystal_spell_cast_kind(spell),
        SkillCastKind::Passive | SkillCastKind::SelfOnly
    ) && !matches!(
        spell,
        Spell::Healing
            | Spell::MassHealing
            | Spell::HealingCircle
            | Spell::Teleport
            | Spell::Blink
            | Spell::Portal
            | Spell::BackStep
            | Spell::FlashDash
            | Spell::StormEscape
            // Cleanse cast on self/allies (Crystal Purification) — Target-kind
            // but not offensive, so it must not require a hostile target.
            | Spell::Purification
    )
}

pub(super) fn localized_skill_base_key(key: &str) -> Option<&'static str> {
    match key {
        "minor-heal" => Some("content.skill.minorHeal"),
        "battle-focus" => Some("content.skill.battleFocus"),
        "summon-shinsu" => Some("content.skill.summonShinsu"),
        "summon-vampire" => Some("content.skill.summonVampire"),
        "summon-toad" => Some("content.skill.summonToad"),
        "summon-snakes" => Some("content.skill.summonSnakes"),
        "stonetrap" => Some("content.skill.stonetrap"),
        _ => None,
    }
}

pub(super) fn localized_skill_name(language: LanguageCode, key: &str, fallback: &str) -> String {
    localized_skill_base_key(key)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.name"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn localized_skill_description(
    language: LanguageCode,
    key: &str,
    fallback: &str,
) -> String {
    localized_skill_base_key(key)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.description"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn crystal_skill_state(spell_name: &str, level: u8) -> Option<SkillState> {
    if let Some(skill) = starter_server_data().skills.into_iter().find(|skill| {
        skill
            .crystal_spell
            .as_deref()
            .is_some_and(|spell| spell.eq_ignore_ascii_case(spell_name))
    }) {
        let delay_ms = skill
            .crystal_spell
            .as_deref()
            .and_then(crystal_magic_by_spell)
            .map(|magic| crystal_magic_delay_ms(&magic, level))
            .unwrap_or_else(|| i64::from(skill.cooldown_ticks) * 1_000);
        return Some(SkillState {
            key: skill.key,
            name: skill.name,
            description: skill.description,
            level,
            experience: 0,
            hotkey: 0,
            cooldown_ticks: skill.cooldown_ticks,
            delay_ms,
            cooldown_ends_at: 0,
            cast_time_ms: 0,
        });
    }

    let magic = crystal_magic_by_spell(spell_name)?;
    Some(SkillState {
        key: normalize_crystal_skill_key(&magic.spell),
        name: magic.name.clone(),
        description: format!("Crystal NPC granted skill {}.", magic.spell),
        level,
        experience: 0,
        hotkey: 0,
        cooldown_ticks: crystal_magic_cooldown_ticks(&magic, level),
        delay_ms: crystal_magic_delay_ms(&magic, level),
        cooldown_ends_at: 0,
        cast_time_ms: 0,
    })
}

pub(super) fn normalize_crystal_skill_key(spell_name: &str) -> String {
    spell_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub(super) fn crystal_book_skill_state(template: &CrystalItemTemplate) -> Option<SkillState> {
    crystal_skill_state(&template.name, 0)
}

pub(super) fn skill_definition(key: &str) -> Option<SkillTemplate> {
    starter_server_data()
        .skills
        .into_iter()
        .find(|skill| skill.key == key)
}

pub(super) fn crystal_magic_for_skill_key(key: &str) -> Option<CrystalMagicTemplate> {
    if let Some(skill) = skill_definition(key) {
        if let Some(spell) = skill.crystal_spell {
            return crystal_magic_by_spell(&spell);
        }
    }

    crystal_magic_manifest()
        .magics
        .into_iter()
        .find(|magic| normalize_crystal_skill_key(&magic.spell) == key)
}

pub(super) fn client_magic_for_skill_state(skill: &SkillState, tick: u64) -> Option<ClientMagic> {
    let magic = crystal_magic_for_skill_key(&skill.key)?;
    Some(ClientMagic {
        name: magic.name.clone(),
        spell: Spell::from_crystal_name(&magic.spell)?,
        base_cost: magic.base_cost,
        level_cost: magic.level_cost,
        icon: magic.icon,
        level1: magic.level1,
        level2: magic.level2,
        level3: magic.level3,
        need1: magic.need1,
        need2: magic.need2,
        need3: magic.need3,
        level: skill.level,
        key: skill.hotkey,
        experience: skill.experience,
        delay: crystal_magic_delay_ms(&magic, skill.level),
        range: magic.range,
        cast_time: skill
            .cast_time_ms
            .saturating_sub(i64::try_from(tick.saturating_mul(1_000)).unwrap_or(i64::MAX)),
    })
}

fn crystal_magic_cooldown_ticks(magic: &CrystalMagicTemplate, level: u8) -> u32 {
    u32::try_from(combat_delay_ticks(
        u64::try_from(crystal_magic_delay_ms(magic, level).max(1))
            .expect("positive magic delay should fit u64"),
    ))
    .expect("magic cooldown ticks should fit u32")
}

fn crystal_magic_delay_ms(magic: &CrystalMagicTemplate, level: u8) -> i64 {
    i64::from(
        magic
            .delay_base
            .saturating_sub(magic.delay_reduction.saturating_mul(u32::from(level)))
            .max(1),
    )
}

pub(super) fn seed_skills() -> Vec<SkillState> {
    starter_server_data()
        .skills
        .into_iter()
        .map(|skill| {
            let delay_ms = skill
                .crystal_spell
                .as_deref()
                .and_then(crystal_magic_by_spell)
                .map(|magic| crystal_magic_delay_ms(&magic, 3))
                .unwrap_or_else(|| i64::from(skill.cooldown_ticks) * 1_000);
            SkillState {
                key: skill.key,
                name: skill.name,
                description: skill.description,
                level: 3,
                experience: 0,
                hotkey: 0,
                cooldown_ticks: skill.cooldown_ticks,
                delay_ms,
                cooldown_ends_at: 0,
                cast_time_ms: 0,
            }
        })
        .collect()
}

pub(super) fn skill_key_for_crystal_spell(spell: Spell) -> Option<String> {
    let spell_name = format!("{spell:?}");
    if let Some(skill) = starter_server_data().skills.into_iter().find(|skill| {
        skill
            .crystal_spell
            .as_deref()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&spell_name))
    }) {
        return Some(skill.key);
    }

    crystal_magic_by_spell(&spell_name).map(|magic| normalize_crystal_skill_key(&magic.spell))
}

pub(super) fn assign_magic_key(world: &mut World, spell: Spell, key: u8, old_key: u8) {
    if key > 16 || old_key > 16 {
        assign_hero_magic_key(world, spell, key);
        return;
    }
    let Some(skill_key) = skill_key_for_crystal_spell(spell) else {
        return;
    };

    let mut skills = world.resource_mut::<SkillResource>();
    for skill in &mut skills.skills {
        if skill.key == skill_key {
            skill.hotkey = key;
        } else if key != 0 && skill.hotkey == key {
            skill.hotkey = 0;
        }
    }
}

fn assign_hero_magic_key(world: &mut World, spell: Spell, key: u8) {
    if !world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
        .is_some_and(|hero| hero.spawned)
    {
        return;
    }
    if hero_entity(world)
        .and_then(|entity| world.entity(entity).get::<PlayerVitals>().copied())
        .is_some_and(|vitals| vitals.hp <= 0)
    {
        return;
    }

    let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
    let learned_magics = &mut stage5.stage5_systems.hero_learned_magics;
    if !learned_magics.iter().any(|learned| learned.spell == spell) {
        return;
    }
    for learned in learned_magics {
        if learned.spell == spell {
            learned.key = key;
        } else if key != 0 && learned.key == key {
            learned.key = 0;
        }
    }
}

pub(super) fn cast_skill(world: &mut World, key: &str) -> Vec<ServerPacket> {
    cast_skill_with_context(world, key, None)
}

pub(super) fn skill_cast_preflight_for_key(
    world: &World,
    key: &str,
    context: Option<&SkillCastContext>,
) -> bool {
    let Some(player) = player_entity(world) else {
        return false;
    };
    if entity_player_vitals(world, player).is_none_or(|vitals| vitals.hp <= 0) {
        return false;
    }
    let metadata = skill_cast_metadata_for_skill_key(key);
    if metadata.cast_kind == SkillCastKind::Passive {
        return false;
    }
    let spell = metadata.spell.and_then(Spell::from_crystal_name);
    let magic = crystal_magic_for_skill_key(key);
    crystal_skill_context_preflight(world, player, spell, magic.as_ref(), metadata, context)
}

fn crystal_skill_context_preflight(
    world: &World,
    player: Entity,
    spell: Option<Spell>,
    magic: Option<&CrystalMagicTemplate>,
    metadata: SkillCastMetadata,
    context: Option<&SkillCastContext>,
) -> bool {
    if let Some(spell) = spell {
        if !crystal_spell_map_restrictions_allow(world, spell) {
            return false;
        }
        if !crystal_spell_required_items_available(world, spell) {
            return false;
        }
    }

    let Some(player_position) = entity_position(world, player) else {
        return false;
    };

    if metadata.offensive && crystal_magic_point_in_safe_zone(world, &player_position) {
        return false;
    }

    match metadata.cast_kind {
        SkillCastKind::Passive => false,
        SkillCastKind::SelfOnly | SkillCastKind::Toggle | SkillCastKind::Direction => true,
        SkillCastKind::Ground => {
            let Some(context) = context else {
                return !metadata.offensive;
            };
            if !crystal_magic_range_allows(magic, &player_position, &context.target) {
                return false;
            }
            if metadata.offensive && crystal_magic_point_in_safe_zone(world, &context.target) {
                return false;
            }
            crystal_magic_line_of_sight_clear(world, &player_position, &context.target)
        }
        SkillCastKind::Target => {
            if matches!(spell, Some(Spell::Healing)) {
                return crystal_healing_preflight(world, player, context, magic);
            }
            let Some(context) = context else {
                return false;
            };
            if context.target_id == 0 {
                return false;
            }
            let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
                return false;
            };
            let Some(target_position) = entity_position(world, target_entity) else {
                return false;
            };
            if !crystal_magic_target_alive(world, target_entity) {
                return false;
            }
            if metadata.offensive && !crystal_magic_target_hostile(world, target_entity) {
                return false;
            }
            if !crystal_magic_range_allows(magic, &player_position, &target_position) {
                return false;
            }
            if metadata.offensive && crystal_magic_point_in_safe_zone(world, &target_position) {
                return false;
            }
            crystal_magic_line_of_sight_clear(world, &player_position, &target_position)
        }
    }
}

fn crystal_healing_preflight(
    world: &World,
    player: Entity,
    context: Option<&SkillCastContext>,
    magic: Option<&CrystalMagicTemplate>,
) -> bool {
    let player_object_id = current_player_object_id(world).unwrap_or_default();
    let target_entity = match context {
        Some(context) if context.target_id != 0 && context.target_id != player_object_id => {
            let Some(entity) = entity_by_object_id(world, context.target_id) else {
                return false;
            };
            entity
        }
        _ => player,
    };
    if !crystal_magic_target_alive(world, target_entity) {
        return false;
    }
    let Some(player_position) = entity_position(world, player) else {
        return false;
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return false;
    };
    crystal_magic_range_allows(magic, &player_position, &target_position)
}

fn crystal_magic_target_alive(world: &World, entity: Entity) -> bool {
    if let Some(vitals) = world.entity(entity).get::<PlayerVitals>() {
        return vitals.hp > 0;
    }
    if let Some(vitals) = world.entity(entity).get::<MonsterVitals>() {
        return vitals.hp > 0
            && world
                .entity(entity)
                .get::<MonsterAgent>()
                .is_none_or(|agent| !agent.dead);
    }
    true
}

fn crystal_magic_target_hostile(world: &World, entity: Entity) -> bool {
    world
        .entity(entity)
        .get::<MonsterAgent>()
        .is_some_and(|agent| !agent.dead && agent.hostile_to_player)
}

fn crystal_magic_range_allows(
    magic: Option<&CrystalMagicTemplate>,
    origin: &Point,
    target: &Point,
) -> bool {
    let Some(magic) = magic else {
        return true;
    };
    magic.range == 0 || tile_distance(origin, target) <= i32::from(magic.range)
}

fn crystal_magic_point_in_safe_zone(world: &World, point: &Point) -> bool {
    let config = &world.resource::<RuntimeConfigResource>().config;
    let map = world.resource::<super::resources::MapRuntimeResource>();
    is_safe_zone_point(config, map, point)
}

fn crystal_spell_map_restrictions_allow(world: &World, spell: Spell) -> bool {
    match spell {
        Spell::Teleport | Spell::Blink => !current_map_disallows_random_teleport(world),
        Spell::Reincarnation => !current_map_disallows_reincarnation(world),
        _ => true,
    }
}

fn crystal_spell_required_items_available(world: &World, spell: Spell) -> bool {
    match spell {
        Spell::SoulFireBall
        | Spell::Curse
        | Spell::TrapHexagon
        | Spell::Plague
        | Spell::BindingShot
        | Spell::SummonSkeleton
        | Spell::SummonShinsu
        | Spell::SummonHolyDeva => has_equipped_crystal_amulet(world, 0),
        Spell::Poisoning | Spell::PoisonSword | Spell::PoisonShot | Spell::CrippleShot => {
            has_equipped_crystal_poison(world, None)
        }
        Spell::PoisonCloud => {
            has_equipped_crystal_amulet(world, 0) && has_equipped_crystal_poison(world, Some(1))
        }
        Spell::Trap | Spell::DelayedExplosion | Spell::ExplosiveTrap => {
            has_equipped_crystal_amulet(world, 0)
        }
        Spell::Reincarnation => has_equipped_crystal_amulet(world, 3),
        _ => true,
    }
}

fn crystal_magic_line_of_sight_clear(world: &World, origin: &Point, target: &Point) -> bool {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let step_count = dx.abs().max(dy.abs());
    if step_count <= 1 {
        return true;
    }
    let aligned = dx == 0 || dy == 0 || dx.abs() == dy.abs();
    if !aligned {
        return true;
    }
    let step_x = dx.signum();
    let step_y = dy.signum();
    let mut point = origin.clone();
    for _ in 1..step_count {
        point.x += step_x;
        point.y += step_y;
        if is_blocked_tile(world, &point) {
            return false;
        }
    }
    true
}

pub(super) fn cast_skill_with_context(
    world: &mut World,
    key: &str,
    context: Option<SkillCastContext>,
) -> Vec<ServerPacket> {
    let tick = super::session::runtime_tick(world);
    let skill_index = {
        let skills = world.resource::<SkillResource>();
        skills.skills.iter().position(|skill| skill.key == key)
    };
    let Some(index) = skill_index else {
        return Vec::new();
    };

    let skill = world.resource::<SkillResource>().skills[index].clone();
    let definition = skill_definition(skill.key.as_str());
    let crystal_magic = crystal_magic_for_skill_key(skill.key.as_str());
    let crystal_spell = definition
        .as_ref()
        .and_then(|definition| definition.crystal_spell.as_deref())
        .or_else(|| crystal_magic.as_ref().map(|magic| magic.spell.as_str()));
    let spell_packet = crystal_spell.and_then(Spell::from_crystal_name);
    let override_default_magic_packets = crystal_spell.is_some_and(|spell| {
        matches!(
            spell,
            "MeteorShower" | "BackStep" | "ShoulderDash" | "FlashDash"
        )
    });
    let manifest_handles_magic_progression = crystal_spell
        .is_some_and(|spell| matches!(spell, "BackStep" | "ShoulderDash" | "FlashDash"));
    let Some(mana_cost) = definition
        .as_ref()
        .map(|definition| definition.mana_cost)
        .or_else(|| {
            crystal_magic.as_ref().map(|magic| {
                i32::from(magic.base_cost) + i32::from(magic.level_cost) * i32::from(skill.level)
            })
        })
    else {
        return Vec::new();
    };
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    if !skill_cast_preflight_for_key(world, skill.key.as_str(), context.as_ref()) {
        return Vec::new();
    }
    if skill.cooldown_ends_at > tick {
        return Vec::new();
    }
    let current_mp = entity_player_vitals(world, player)
        .map(|vitals| vitals.mp)
        .unwrap_or_default();
    if current_mp < mana_cost {
        return Vec::new();
    }

    let mut packets = Vec::new();
    let spent_vitals = {
        let mut entity = world.entity_mut(player);
        let mut vitals = entity.get_mut::<PlayerVitals>().expect("player vitals");
        vitals.mp = (vitals.mp - mana_cost).max(0);
        *vitals
    };
    world.resource_mut::<PlayerRuntimeResource>().player_vitals = spent_vitals;
    if let Some(info) = object_mana_info_for_entity(world, player) {
        packets.push(ServerPacket::ObjectMana { info });
    }

    let crystal_self_buff_packets = crystal_magic
        .as_ref()
        .and_then(|magic| apply_crystal_self_buff_spell(world, &skill, magic, tick));
    if let Some(crystal_self_buff_packets) = crystal_self_buff_packets {
        packets.extend(crystal_self_buff_packets);
    } else if let Some(definition) = definition.as_ref() {
        match &definition.effect {
            SkillEffectTemplate::Heal { hp } => {
                if let Some(magic) = crystal_magic
                    .as_ref()
                    .filter(|magic| magic.spell == "Healing")
                {
                    packets.extend(apply_crystal_healing_spell(
                        world,
                        &skill,
                        magic,
                        context.as_ref(),
                        tick,
                    ));
                } else {
                    let healed_vitals = {
                        let mut entity = world.entity_mut(player);
                        let mut vitals = entity.get_mut::<PlayerVitals>().expect("player vitals");
                        vitals.hp = (vitals.hp + *hp).min(vitals.max_hp);
                        *vitals
                    };
                    world.resource_mut::<PlayerRuntimeResource>().player_vitals = healed_vitals;
                    if let Some(info) = object_health_info_for_entity(world, player, 0) {
                        packets.push(ServerPacket::ObjectHealth { info });
                    }
                }
            }
            SkillEffectTemplate::Buff {
                buff_key,
                buff_name,
                buff_description,
                duration_ticks,
                attack_bonus,
                defence_bonus,
            } => {
                let (resolved_buff_name, resolved_buff_description) =
                    buff_metadata(buff_key, buff_name, buff_description);
                let buff = BuffState {
                    key: buff_key.clone(),
                    name: resolved_buff_name,
                    description: resolved_buff_description,
                    expires_at_tick: tick + *duration_ticks,
                    attack_bonus: *attack_bonus,
                    defence_bonus: *defence_bonus,
                    stats: merged_user_item_stats(&[], *defence_bonus, *attack_bonus, None),
                };
                apply_or_refresh_buff(world, buff.clone());
                if let Some(packet) = client_buff_packet_for_state(world, &buff) {
                    packets.push(packet);
                }
            }
            SkillEffectTemplate::Summon { spell } => {
                packets.extend(cast_summon_skill(
                    world,
                    player,
                    &definition.key,
                    &definition.name,
                    spell,
                    skill.level,
                    tick,
                ));
            }
        }
    }

    if definition.is_none() {
        // Scope the spell's Crystal DefenceType so scheduled/immediate monster damage rolls the
        // correct target armour (AC/MAC), then restore the prior value.
        let cast_defence = crystal_magic
            .as_ref()
            .map(|magic| crystal_spell_defence(&magic.spell));
        let previous_defence = set_cast_defence(world, cast_defence);
        packets.extend(apply_manifest_spell_effect(
            world,
            player,
            &skill,
            index,
            crystal_magic.as_ref(),
            spell_packet,
            context.as_ref(),
            tick,
        ));
        set_cast_defence(world, previous_defence);
    }

    if let Some(spell) = spell_packet.filter(|_| !override_default_magic_packets) {
        let object_id = current_player_object_id(world).unwrap_or_default();
        let location = entity_position(world, player).unwrap_or(Point { x: 0, y: 0 });
        let direction = context
            .as_ref()
            .map(|context| context.direction)
            .or_else(|| entity_facing(world, player))
            .unwrap_or(MirDirection::Down);
        let mut target_id = context
            .as_ref()
            .map(|context| context.target_id)
            .unwrap_or(0);
        let mut target = context
            .as_ref()
            .map(|context| context.target.clone())
            .unwrap_or_else(|| location.clone());
        if target_id == 0
            && definition.as_ref().is_some_and(|definition| {
                matches!(&definition.effect, SkillEffectTemplate::Heal { .. })
            })
        {
            target_id = object_id;
            target = location.clone();
        }
        packets.push(ServerPacket::Magic {
            spell,
            target_id,
            target: target.clone(),
            cast: true,
            level: skill.level,
            secondary_target_ids: Vec::new(),
        });
        packets.push(ServerPacket::ObjectMagic {
            object_id,
            location,
            direction,
            spell,
            target_id,
            target,
            cast: true,
            level: skill.level,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        });
        if context
            .as_ref()
            .is_some_and(|context| context.target_id != 0)
            && matches!(
                spell,
                Spell::FireBall
                    | Spell::GreatFireBall
                    | Spell::ThunderBolt
                    | Spell::SoulFireBall
                    | Spell::FireBounce
                    | Spell::CatTongue
            )
        {
            packets.push(ServerPacket::ObjectProjectile {
                spell,
                source_id: object_id,
                destination_id: target_id,
            });
        }
    }

    let skill_level_for_timing = world
        .resource::<SkillResource>()
        .skills
        .get(index)
        .map(|skill| skill.level)
        .unwrap_or(skill.level);
    let (cooldown_ticks, delay_ms) = crystal_magic
        .as_ref()
        .map(|magic| {
            (
                crystal_magic_cooldown_ticks(magic, skill_level_for_timing),
                crystal_magic_delay_ms(magic, skill_level_for_timing),
            )
        })
        .unwrap_or((skill.cooldown_ticks, skill.delay_ms));
    {
        let mut skills = world.resource_mut::<SkillResource>();
        let skill = &mut skills.skills[index];
        skill.cooldown_ticks = cooldown_ticks;
        skill.delay_ms = delay_ms;
        skill.cooldown_ends_at = tick + u64::from(cooldown_ticks);
        skill.cast_time_ms = i64::try_from(tick.saturating_mul(1_000)).unwrap_or(i64::MAX);
    }
    if !manifest_handles_magic_progression {
        if let (Some(spell), Some(magic)) = (spell_packet, crystal_magic.as_ref()) {
            packets.extend(advance_magic_progression(world, index, spell, magic, tick));
        }
    }
    packets
}

fn apply_manifest_spell_effect(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    skill_index: usize,
    crystal_magic: Option<&CrystalMagicTemplate>,
    spell: Option<Spell>,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    let Some(magic) = crystal_magic else {
        return packets;
    };

    match magic.spell.as_str() {
        "Fury" | "Haste" | "SwiftFeet" | "ProtectionField" | "Rage" | "LightBody" | "MoonLight"
        | "DarkBody" | "Hiding" | "MassHiding" | "ImmortalSkin" => {
            if let Some(buff_packets) = apply_crystal_self_buff_spell(world, skill, magic, tick) {
                packets.extend(buff_packets);
            }
            return packets;
        }
        "SoulShield" | "BlessedArmour" | "UltimateEnhancer" => {
            packets.extend(apply_crystal_taoist_support_spell(
                world, skill, magic, context, tick,
            ));
            return packets;
        }
        "Concentration" => {
            packets.extend(apply_crystal_concentration_spell(world, skill, tick));
            return packets;
        }
        "EnergyShield" => {
            packets.extend(apply_crystal_energy_shield_spell(world, skill, magic, tick));
            return packets;
        }
        "PetEnhancer" => {
            packets.extend(apply_crystal_pet_enhancer_spell(
                world, skill, context, tick,
            ));
            return packets;
        }
        "LionRoar" | "BattleCry" => {
            packets.extend(apply_crystal_warrior_area_control_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "ElementalShot" => {
            packets.extend(apply_crystal_elemental_shot_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "ElementalBarrier" => {
            packets.extend(apply_crystal_elemental_barrier_spell(
                world, skill, magic, tick,
            ));
            return packets;
        }
        "BackStep" => {
            packets.extend(apply_crystal_back_step_spell(world, player, skill, context));
            let moved = packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::UserBackStep { .. }));
            packets.push(ServerPacket::MagicCast {
                spell: Spell::BackStep,
            });
            if moved {
                if let Some(spell) = spell {
                    packets.extend(advance_magic_progression(
                        world,
                        skill_index,
                        spell,
                        magic,
                        tick,
                    ));
                }
            }
            return packets;
        }
        "ShoulderDash" => {
            packets.extend(apply_crystal_shoulder_dash_spell(
                world, player, skill, magic, context, tick,
            ));
            if packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::UserDash { .. }))
            {
                if let Some(spell) = spell {
                    packets.extend(advance_magic_progression(
                        world,
                        skill_index,
                        spell,
                        magic,
                        tick,
                    ));
                }
            }
            return packets;
        }
        "SlashingBurst" => {
            packets.extend(apply_crystal_slashing_burst_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "FlashDash" => {
            let (flash_packets, hit_target) =
                apply_crystal_flash_dash_spell(world, player, skill, magic, context, tick);
            packets.extend(flash_packets);
            if hit_target {
                if let Some(spell) = spell {
                    packets.extend(advance_magic_progression(
                        world,
                        skill_index,
                        spell,
                        magic,
                        tick,
                    ));
                }
            }
            return packets;
        }
        "MassHealing" => {
            packets.extend(apply_crystal_area_healing_spell(
                world, player, skill, magic, context, tick, false,
            ));
            return packets;
        }
        "HealingCircle" => {
            packets.extend(apply_crystal_area_healing_spell(
                world, player, skill, magic, context, tick, true,
            ));
            return packets;
        }
        "Curse" => {
            packets.extend(apply_crystal_curse_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "TrapHexagon" => {
            packets.extend(apply_crystal_trap_hexagon_spell(
                world, player, skill, context, tick,
            ));
            return packets;
        }
        "Poisoning" => {
            packets.extend(apply_crystal_poisoning_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "PoisonCloud" => {
            packets.extend(apply_crystal_poison_cloud_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "Plague" => {
            packets.extend(apply_crystal_plague_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "Purification" => {
            packets.extend(apply_crystal_purification_spell(
                world, skill, context, tick,
            ));
            return packets;
        }
        "Revelation" => {
            packets.extend(apply_crystal_revelation_spell(
                world, skill, magic, context, tick,
            ));
            return packets;
        }
        "BindingShot" => {
            packets.extend(apply_crystal_binding_shot(
                world, player, skill, context, tick,
            ));
            return packets;
        }
        "Trap" => {
            packets.extend(apply_crystal_trap_spell(
                world, player, skill, context, tick,
            ));
            return packets;
        }
        "PoisonSword" => {
            packets.extend(apply_crystal_poison_sword_spell(
                world, player, skill, magic, tick,
            ));
            return packets;
        }
        "ExplosiveTrap" => {
            packets.extend(apply_crystal_explosive_trap_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "StraightShot" | "DoubleShot" => {
            packets.extend(apply_crystal_archer_basic_shot(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "VampireShot" | "PoisonShot" | "CrippleShot" => {
            packets.extend(apply_crystal_special_arrow_shot(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "MagicBooster" => {
            let magic_bonus = 6 + i32::from(skill.level) * 6;
            let buff = BuffState {
                key: "magic-booster".to_string(),
                name: "Magic Booster".to_string(),
                description: "Crystal magic booster buff is active.".to_string(),
                expires_at_tick: tick + combat_delay_ticks(60_000),
                attack_bonus: 0,
                defence_bonus: 0,
                stats: vec![
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
                        value: 6 + i32::from(skill.level),
                    },
                ],
            };
            apply_or_refresh_buff(world, buff.clone());
            if let Some(packet) = client_buff_packet_for_state(world, &buff) {
                packets.push(packet);
            }
            return packets;
        }
        "Repulsion" | "EnergyRepulsor" | "FireBurst" => {
            packets.extend(apply_crystal_repulsion_spell(
                world, player, skill, magic, tick,
            ));
            return packets;
        }
        "StormEscape" => {
            packets.extend(apply_crystal_storm_escape_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "FireWall" => {
            packets.extend(apply_crystal_fire_wall_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "HellFire" => {
            packets.extend(apply_crystal_hell_fire_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "FireBang" | "IceStorm" => {
            packets.extend(apply_crystal_fire_bang_family_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "Blizzard" | "MeteorStrike" => {
            packets.extend(apply_crystal_blizzard_family_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "MeteorShower" => {
            packets.extend(apply_crystal_meteor_shower_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "FireBounce" => {
            packets.extend(apply_crystal_fire_bounce_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "FireBall" | "GreatFireBall" => {
            packets.extend(apply_crystal_projectile_damage_spell(
                world, player, skill, magic, context, tick, false,
            ));
            return packets;
        }
        "SoulFireBall" => {
            packets.extend(apply_crystal_projectile_damage_spell(
                world, player, skill, magic, context, tick, true,
            ));
            return packets;
        }
        "Healing" => {
            packets.extend(apply_crystal_healing_spell(
                world, skill, magic, context, tick,
            ));
            return packets;
        }
        "Lightning" => {
            packets.extend(apply_crystal_lightning_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "ThunderBolt" => {
            packets.extend(apply_crystal_thunder_bolt_spell(
                world, skill, magic, context, tick,
            ));
            return packets;
        }
        "ElectricShock" => {
            packets.extend(apply_crystal_electric_shock_spell(
                world, player, skill, context, tick,
            ));
            return packets;
        }
        "FlameDisruptor" => {
            packets.extend(apply_crystal_flame_disruptor_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "IceThrust" => {
            packets.extend(apply_crystal_ice_thrust_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "FrostCrunch" => {
            packets.extend(apply_crystal_frost_crunch_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "Vampirism" => {
            packets.extend(apply_crystal_vampirism_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "TurnUndead" => {
            packets.extend(apply_crystal_turn_undead_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "NapalmShot" => {
            packets.extend(apply_crystal_napalm_shot_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "DelayedExplosion" => {
            packets.extend(apply_crystal_delayed_explosion_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "ThunderStorm" | "FlameField" => {
            packets.extend(apply_crystal_thunder_storm_family_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "MagicShield" => {
            packets.extend(apply_crystal_magic_shield_spell(world, skill, magic, tick));
            return packets;
        }
        "Teleport" => {
            packets.extend(apply_crystal_teleport_spell(
                world, player, skill, context, tick,
            ));
            return packets;
        }
        "Blink" => {
            packets.extend(apply_crystal_blink_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "SummonSkeleton" | "SummonHolyDeva" => {
            packets.extend(apply_crystal_taoist_summon_spell(
                world, player, skill, magic, tick,
            ));
            return packets;
        }
        "BladeAvalanche" => {
            packets.extend(apply_crystal_blade_avalanche_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "HalfMoon" | "CrossHalfMoon" => {
            packets.extend(apply_crystal_half_moon_family_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "DoubleSlash" => {
            packets.extend(apply_crystal_double_slash_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "TwinDrakeBlade" => {
            packets.extend(apply_crystal_twin_drake_blade_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "HeavenlySword" => {
            packets.extend(apply_crystal_heavenly_sword_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "Entrapment" => {
            packets.extend(apply_crystal_entrapment_spell(
                world, player, skill, context, tick,
            ));
            return packets;
        }
        "CrescentSlash" => {
            packets.extend(apply_crystal_crescent_slash_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "Mirroring" => {
            packets.extend(apply_crystal_mirroring_spell(world, player, skill, tick));
            return packets;
        }
        "CounterAttack" | "FatalSword" | "FastMove" | "Fencing" | "FlamingSword" | "Focus"
        | "Hemorrhage" | "Meditation" | "MentalState" | "MPEater" | "Slaying" | "SpiritSword"
        | "Thrusting" => {
            // Passive / always-on skills with no active server effect (FastMove is a passive
            // movement-speed skill in Crystal with no cast handler).
            return packets;
        }
        "MoonMist" => {
            packets.extend(apply_crystal_moon_mist_spell(
                world, player, skill, magic, tick,
            ));
            return packets;
        }
        "CatTongue" => {
            packets.extend(apply_crystal_cat_tongue_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        "Hallucination" => {
            packets.extend(apply_crystal_hallucination_spell(
                world, player, skill, context, tick,
            ));
            return packets;
        }
        "OneWithNature" => {
            packets.extend(apply_crystal_one_with_nature_spell(
                world, player, skill, magic, tick,
            ));
            return packets;
        }
        "Reincarnation" => {
            packets.extend(apply_crystal_reincarnation_spell(world, player, tick));
            return packets;
        }
        "Portal" => {
            packets.extend(apply_crystal_portal_spell(
                world, player, skill, magic, context, tick,
            ));
            return packets;
        }
        _ => {}
    }

    let Some(context) = context else {
        return packets;
    };
    if context.target_id == 0 || matches!(spell, Some(Spell::None | Spell::Healing)) {
        return packets;
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return packets;
    };
    let Some(player_object_id) = current_player_object_id(world) else {
        return packets;
    };
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    schedule_damage_to_monster(
        world,
        due_tick,
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: context.target_id,
            name: target_name,
        }),
    );
    packets
}

fn apply_crystal_self_buff_spell(
    world: &mut World,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    tick: u64,
) -> Option<Vec<ServerPacket>> {
    let level = i32::from(skill.level);
    let (key, name, description, duration_ms, stats) = match magic.spell.as_str() {
        "Fury" => (
            "battle-focus",
            "Fury",
            "Crystal Fury buff is active.",
            60_000_u64 + u64::from(skill.level).saturating_mul(10_000),
            vec![UserItemStat {
                stat: CRYSTAL_STAT_ATTACK_SPEED,
                value: 4,
            }],
        ),
        "Haste" => (
            "haste",
            "Haste",
            "Crystal Haste buff is active.",
            25_000_u64 + u64::from(skill.level).saturating_mul(15_000),
            vec![UserItemStat {
                stat: CRYSTAL_STAT_ATTACK_SPEED,
                value: level.saturating_mul(2).saturating_add(2),
            }],
        ),
        "SwiftFeet" => (
            "swift-feet",
            "Swift Feet",
            "Crystal Swift Feet buff is active.",
            25_000_u64 + u64::from(skill.level).saturating_mul(5_000),
            Vec::new(),
        ),
        "LightBody" => (
            "light-body",
            "Light Body",
            "Crystal Light Body buff is active.",
            (u64::from(skill.level) + 1).saturating_mul(30_000),
            vec![UserItemStat {
                stat: CRYSTAL_STAT_AGILITY,
                value: (i32::from(skill.level) + 1).saturating_mul(2),
            }],
        ),
        "MoonLight" | "DarkBody" => {
            let defence = current_player_crystal_stat(world, CRYSTAL_STAT_MAX_AC)
                .max(current_player_crystal_stat(world, CRYSTAL_STAT_MIN_AC));
            let duration_ms = u64::try_from(
                defence
                    .saturating_add((i32::from(skill.level) + 1).saturating_mul(5))
                    .max(1),
            )
            .unwrap_or(1)
            .saturating_mul(500);
            if magic.spell == "MoonLight" {
                (
                    "moon-light",
                    "Moon Light",
                    "Crystal Moon Light buff is active.",
                    duration_ms,
                    Vec::new(),
                )
            } else {
                (
                    "dark-body",
                    "Dark Body",
                    "Crystal Dark Body buff is active.",
                    duration_ms,
                    Vec::new(),
                )
            }
        }
        "Hiding" | "MassHiding" => (
            "hiding",
            if magic.spell == "MassHiding" {
                "Mass Hiding"
            } else {
                "Hiding"
            },
            "Crystal hiding buff is active.",
            (10_u64 + u64::from(skill.level).saturating_mul(10)).saturating_mul(1_000),
            Vec::new(),
        ),
        "ImmortalSkin" => {
            let attack = current_player_crystal_stat(world, CRYSTAL_STAT_MAX_DC);
            let defence = current_player_crystal_stat(world, CRYSTAL_STAT_MAX_AC);
            let attack_penalty =
                crystal_scaled_stat_bonus(attack, 0.05 + 0.01 * f64::from(level)).saturating_neg();
            let defence_bonus = crystal_scaled_stat_bonus(defence, 0.10 + 0.07 * f64::from(level));
            (
                "immortal-skin",
                "Immortal Skin",
                "Crystal Immortal Skin buff is active.",
                60_000_u64 + u64::from(skill.level).saturating_mul(1_000),
                vec![
                    UserItemStat {
                        stat: CRYSTAL_STAT_MAX_DC,
                        value: attack_penalty,
                    },
                    UserItemStat {
                        stat: CRYSTAL_STAT_MAX_AC,
                        value: defence_bonus,
                    },
                ],
            )
        }
        "ProtectionField" => {
            let defence = current_player_crystal_stat(world, CRYSTAL_STAT_MAX_AC);
            let add_value = crystal_scaled_stat_bonus(defence, 0.20 + 0.03 * f64::from(level));
            (
                "protection-field",
                "Protection Field",
                "Crystal Protection Field buff is active.",
                (45_u64 + u64::from(skill.level).saturating_mul(15)).saturating_mul(1_000),
                vec![
                    UserItemStat {
                        stat: CRYSTAL_STAT_MIN_AC,
                        value: add_value,
                    },
                    UserItemStat {
                        stat: CRYSTAL_STAT_MAX_AC,
                        value: add_value,
                    },
                ],
            )
        }
        "Rage" => {
            let attack = current_player_crystal_stat(world, CRYSTAL_STAT_MAX_DC);
            let add_value = crystal_scaled_stat_bonus(attack, 0.12 + 0.03 * f64::from(level));
            (
                "rage",
                "Rage",
                "Crystal Rage buff is active.",
                (18_u64 + u64::from(skill.level).saturating_mul(6)).saturating_mul(1_000),
                vec![
                    UserItemStat {
                        stat: CRYSTAL_STAT_MIN_DC,
                        value: add_value,
                    },
                    UserItemStat {
                        stat: CRYSTAL_STAT_MAX_DC,
                        value: add_value,
                    },
                ],
            )
        }
        _ => return None,
    };

    let buff = BuffState {
        key: key.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        expires_at_tick: tick.saturating_add(combat_delay_ticks(duration_ms)),
        attack_bonus: stats
            .iter()
            .filter(|stat| stat.stat == CRYSTAL_STAT_MAX_DC)
            .map(|stat| stat.value)
            .sum(),
        defence_bonus: stats
            .iter()
            .filter(|stat| stat.stat == CRYSTAL_STAT_MAX_AC)
            .map(|stat| stat.value)
            .sum(),
        stats,
    };
    apply_or_refresh_buff(world, buff.clone());
    let mut packets: Vec<ServerPacket> = client_buff_packet_for_state(world, &buff)
        .into_iter()
        .collect();
    if matches!(key, "hiding" | "moon-light" | "dark-body") {
        if let Some(object_id) = current_player_object_id(world) {
            packets.push(ServerPacket::ObjectHidden {
                object_id,
                hidden: true,
            });
        }
    }
    Some(packets)
}

fn current_player_crystal_stat(world: &World, stat: u8) -> i32 {
    let inventory = world.resource::<super::resources::InventoryResource>();
    let buffs = world.resource::<super::resources::BuffResource>();
    current_player_required_stat_total(inventory, buffs, stat)
}

fn crystal_scaled_stat_bonus(base: i32, multiplier: f64) -> i32 {
    ((f64::from(base) * multiplier).round() as i32).max(0)
}

fn apply_crystal_energy_shield_spell(
    world: &mut World,
    skill: &SkillState,
    _magic: &CrystalMagicTemplate,
    tick: u64,
) -> Vec<ServerPacket> {
    let level = i32::from(skill.level);
    let luck = current_player_crystal_stat(world, CRYSTAL_STAT_LUCK).max(0);
    let chance = (10 - (luck / 3 + level + 1)).max(2);
    let shield_percent = (100 + chance / 2) / chance;
    let hp_gain = current_player_crystal_stat(world, CRYSTAL_STAT_MAX_SC)
        .max(current_player_crystal_stat(world, CRYSTAL_STAT_MIN_SC))
        .saturating_add((level + 1).saturating_mul(5))
        .max(1);
    let duration_ms = (30_u64 + u64::from(skill.level).saturating_mul(50)).saturating_mul(1_000);
    let buff = BuffState {
        key: "energy-shield".to_string(),
        name: "Energy Shield".to_string(),
        description: "Crystal Energy Shield buff is active.".to_string(),
        expires_at_tick: tick.saturating_add(combat_delay_ticks(duration_ms)),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: vec![
            UserItemStat {
                stat: CRYSTAL_STAT_ENERGY_SHIELD_PERCENT,
                value: shield_percent,
            },
            UserItemStat {
                stat: CRYSTAL_STAT_ENERGY_SHIELD_HP_GAIN,
                value: hp_gain,
            },
        ],
    };
    apply_or_refresh_buff(world, buff.clone());
    client_buff_packet_for_state(world, &buff)
        .into_iter()
        .collect()
}

const CRYSTAL_SPELL_EFFECT_TELEPORT: u8 = 2;
const CRYSTAL_SPELL_EFFECT_HEALING: u8 = 3;
const CRYSTAL_SPELL_EFFECT_TWIN_DRAKE_BLADE: u8 = 5;
const CRYSTAL_SPELL_EFFECT_MAGIC_SHIELD_UP: u8 = 6;
const CRYSTAL_SPELL_EFFECT_ENTRAPMENT: u8 = 9;
const CRYSTAL_SPELL_EFFECT_MOON_MIST: u8 = 34;

fn apply_crystal_magic_shield_spell(
    world: &mut World,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    tick: u64,
) -> Vec<ServerPacket> {
    if world
        .resource::<super::resources::BuffResource>()
        .buffs
        .iter()
        .any(|buff| buff.key == "magic-shield")
    {
        return Vec::new();
    }
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let duration_seconds = crystal_magic_damage(magic, skill.level)
        .saturating_add(current_player_crystal_stat(world, CRYSTAL_STAT_MAX_MC))
        .saturating_add(15)
        .max(1);
    let duration_ms = u64::try_from(duration_seconds)
        .unwrap_or(1)
        .saturating_mul(1_000);
    let buff = BuffState {
        key: "magic-shield".to_string(),
        name: "Magic Shield".to_string(),
        description: "Crystal magic shield buff is active.".to_string(),
        expires_at_tick: tick + combat_delay_ticks(duration_ms),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: vec![UserItemStat {
            stat: CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT,
            value: (i32::from(skill.level) + 2) * 10,
        }],
    };
    apply_or_refresh_buff(world, buff.clone());
    let mut packets: Vec<ServerPacket> = client_buff_packet_for_state(world, &buff)
        .into_iter()
        .collect();
    packets.push(ServerPacket::ObjectEffect {
        info: ObjectEffectInfo {
            object_id,
            effect: CRYSTAL_SPELL_EFFECT_MAGIC_SHIELD_UP,
            effect_type: 0,
            delay_time: 0,
            time: 0,
        },
    });
    packets
}

fn apply_crystal_teleport_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    if current_map_disallows_random_teleport(world) {
        return Vec::new();
    }
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    let destination = context
        .map(|context| context.target.clone())
        .filter(|target| can_occupy(world, target.clone(), Some(player)))
        .or_else(|| deterministic_teleport_destination(world, player, &origin, skill.level));
    let Some(destination) = destination else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    world.entity_mut(player).insert((
        Position(destination.clone()),
        super::components::Facing(direction),
    ));
    let buff = BuffState {
        key: "temporal-flux".to_string(),
        name: "Temporal Flux".to_string(),
        description: "Crystal temporal flux teleport penalty is active.".to_string(),
        expires_at_tick: tick + combat_delay_ticks(30_000),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: vec![UserItemStat {
            stat: CRYSTAL_STAT_TELEPORT_MANA_PENALTY_PERCENT,
            value: 30,
        }],
    };
    apply_or_refresh_buff(world, buff.clone());
    let mut packets = vec![
        ServerPacket::UserLocation {
            location: UserLocation {
                position: destination,
                direction,
            },
        },
        ServerPacket::ObjectEffect {
            info: ObjectEffectInfo {
                object_id,
                effect: CRYSTAL_SPELL_EFFECT_TELEPORT,
                effect_type: 0,
                delay_time: 0,
                time: 0,
            },
        },
    ];
    if let Some(packet) = client_buff_packet_for_state(world, &buff) {
        packets.push(packet);
    }
    packets
}

fn apply_crystal_blink_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    if current_map_disallows_random_teleport(world) {
        return Vec::new();
    }
    let Some(context) = context else {
        return Vec::new();
    };
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    if tile_distance(&origin, &context.target) > i32::from(magic.range) {
        return Vec::new();
    }
    if !can_occupy(world, context.target.clone(), Some(player)) {
        return Vec::new();
    }
    let succeeds = skill.level >= 3
        || deterministic_roll(
            tick,
            usize::try_from(object_id).unwrap_or_default(),
            1_507 + usize::from(skill.level),
            4,
        ) < u64::from(skill.level) + 1;
    if !succeeds {
        return Vec::new();
    }

    let destination = context.target.clone();
    world.entity_mut(player).insert((
        Position(destination.clone()),
        super::components::Facing(context.direction),
    ));
    let buff = BuffState {
        key: "temporal-flux".to_string(),
        name: "Temporal Flux".to_string(),
        description: "Crystal temporal flux teleport penalty is active.".to_string(),
        expires_at_tick: tick + combat_delay_ticks(30_000),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: vec![UserItemStat {
            stat: CRYSTAL_STAT_TELEPORT_MANA_PENALTY_PERCENT,
            value: 30,
        }],
    };
    apply_or_refresh_buff(world, buff.clone());

    let mut packets = vec![
        ServerPacket::UserLocation {
            location: UserLocation {
                position: destination,
                direction: context.direction,
            },
        },
        ServerPacket::ObjectEffect {
            info: ObjectEffectInfo {
                object_id,
                effect: CRYSTAL_SPELL_EFFECT_TELEPORT,
                effect_type: 0,
                delay_time: 0,
                time: 0,
            },
        },
    ];
    if let Some(packet) = client_buff_packet_for_state(world, &buff) {
        packets.push(packet);
    }
    packets
}

fn deterministic_teleport_destination(
    world: &World,
    player: Entity,
    origin: &Point,
    skill_level: u8,
) -> Option<Point> {
    let radius = 4 + i32::from(skill_level).saturating_mul(3);
    let object_id = current_player_object_id(world).unwrap_or_default();
    let mut candidates = Vec::new();
    for y in origin.y - radius..=origin.y + radius {
        for x in origin.x - radius..=origin.x + radius {
            let point = Point { x, y };
            if point == *origin || tile_distance(origin, &point) > radius {
                continue;
            }
            if can_occupy(world, point.clone(), Some(player)) {
                candidates.push(point);
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let index = deterministic_roll(
        u64::from(skill_level).saturating_add(1),
        usize::try_from(object_id).unwrap_or_default(),
        1011 + usize::from(skill_level),
        u64::try_from(candidates.len()).unwrap_or(1),
    ) as usize;
    candidates.get(index).cloned()
}

fn apply_crystal_pet_enhancer_spell(
    world: &mut World,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if agent.dead
        || (agent.hostile_to_player
            && world
                .entity(target_entity)
                .get::<SummonedMonster>()
                .is_none())
    {
        return Vec::new();
    }

    let target_level = i32::from(monster_level(world, target_entity)).max(1);
    let dc_inc = 2 + target_level.saturating_mul(2);
    let ac_inc = 4 + target_level;
    let duration_ms = (10_u64 + u64::from(skill.level).saturating_mul(5)).saturating_mul(1_000);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: crystal_buff_type_for_key("pet-enhancer").unwrap_or(22),
                visible: true,
                object_id: context.target_id,
                expire_time: i64::try_from(duration_ms).unwrap_or(i64::MAX),
                infinite: false,
                paused: false,
                stats: vec![
                    UserItemStat {
                        stat: CRYSTAL_STAT_MIN_DC,
                        value: dc_inc,
                    },
                    UserItemStat {
                        stat: CRYSTAL_STAT_MAX_DC,
                        value: dc_inc,
                    },
                    UserItemStat {
                        stat: CRYSTAL_STAT_MIN_AC,
                        value: ac_inc,
                    },
                    UserItemStat {
                        stat: CRYSTAL_STAT_MAX_AC,
                        value: ac_inc,
                    },
                ],
                values: Vec::new(),
            },
        },
    );
    Vec::new()
}

fn apply_crystal_warrior_area_control_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    const CRYSTAL_LION_ROAR_PARALYSIS: u16 = 256;

    let center = context
        .map(|context| context.target.clone())
        .or_else(|| entity_position(world, player))
        .unwrap_or(Point { x: 0, y: 0 });
    let player_level = crystal_player_level(world, player);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let targets = hostile_monsters_in_square(world, &center, 2);

    for target_entity in targets {
        let Some(object_id) = entity_object_id(world, target_entity) else {
            continue;
        };
        if magic.spell == "LionRoar" {
            if player_level.saturating_add(3) < monster_level(world, target_entity) {
                continue;
            }
            let duration_ms = (u64::from(skill.level) + 2).saturating_mul(1_000);
            let release_tick = tick.saturating_add(combat_delay_ticks(duration_ms));
            if let Some(mut agent) = world.entity_mut(target_entity).get_mut::<MonsterAgent>() {
                agent.next_move_tick = agent.next_move_tick.max(release_tick);
                agent.next_attack_tick = agent.next_attack_tick.max(release_tick);
            }
            apply_monster_poison(
                world,
                target_entity,
                CRYSTAL_LION_ROAR_PARALYSIS,
                0,
                due_tick,
                combat_delay_ticks(duration_ms),
            );
            queue_due_packet(
                world,
                due_tick,
                ServerPacket::ObjectPoisoned {
                    object_id,
                    poison: CRYSTAL_LION_ROAR_PARALYSIS,
                },
            );
            continue;
        }

        let success_chance = match skill.level {
            0 => 10,
            1 => 30,
            2 => 50,
            _ => 70,
        };
        let success = skill.level >= 3
            || deterministic_roll(
                tick,
                usize::try_from(object_id).unwrap_or_default(),
                907 + usize::from(skill.level),
                100,
            ) < success_chance;
        if !success {
            continue;
        }
        if let Some(mut agent) = world.entity_mut(target_entity).get_mut::<MonsterAgent>() {
            agent.tracking_player = true;
            agent.next_move_tick = agent.next_move_tick.min(tick);
            agent.next_attack_tick = agent.next_attack_tick.min(tick);
        }
    }

    Vec::new()
}

fn apply_crystal_taoist_support_spell(
    world: &mut World,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();

    match magic.spell.as_str() {
        "SoulShield" | "BlessedArmour" => {
            let Some(delete_packet) = consume_equipped_crystal_amulet(world, 0, 1) else {
                return packets;
            };
            let player_level = active_player_level(world);
            let value = player_level / 7 + 4;
            let stat = if magic.spell == "SoulShield" {
                CRYSTAL_STAT_MAX_MAC
            } else {
                CRYSTAL_STAT_MAX_AC
            };
            let duration_ms = u64::try_from(taoist_support_duration_seconds(world, skill))
                .unwrap_or_default()
                .saturating_mul(1_000);
            let key = if magic.spell == "SoulShield" {
                "soul-shield"
            } else {
                "blessed-armour"
            };
            let name = if magic.spell == "SoulShield" {
                "Soul Shield"
            } else {
                "Blessed Armour"
            };
            let buff = BuffState {
                key: key.to_string(),
                name: name.to_string(),
                description: format!("Crystal {name} buff is active."),
                expires_at_tick: tick.saturating_add(combat_delay_ticks(duration_ms)),
                attack_bonus: 0,
                defence_bonus: (stat == CRYSTAL_STAT_MAX_AC).then_some(value).unwrap_or(0),
                stats: vec![UserItemStat { stat, value }],
            };
            packets.push(delete_packet);
            apply_or_refresh_buff(world, buff.clone());
            if let Some(packet) = client_buff_packet_for_state(world, &buff) {
                packets.push(packet);
            }
            packets
        }
        "UltimateEnhancer" => {
            if !context_targets_current_player(world, context) {
                return packets;
            }
            let Some(delete_packet) = consume_equipped_crystal_amulet(world, 0, 1) else {
                return packets;
            };
            let max_sc = current_player_crystal_stat(world, CRYSTAL_STAT_MAX_SC);
            let value = if max_sc >= 5 { (max_sc / 5).min(8) } else { 1 };
            let stat = match active_player_class(world) {
                Some(MirClass::Wizard | MirClass::Archer) => CRYSTAL_STAT_MAX_MC,
                Some(MirClass::Taoist) => CRYSTAL_STAT_MAX_SC,
                _ => CRYSTAL_STAT_MAX_DC,
            };
            let duration_ms = u64::try_from(taoist_support_duration_seconds(world, skill))
                .unwrap_or_default()
                .saturating_mul(1_000);
            let buff = BuffState {
                key: "ultimate-enhancer".to_string(),
                name: "Ultimate Enhancer".to_string(),
                description: "Crystal Ultimate Enhancer buff is active.".to_string(),
                expires_at_tick: tick.saturating_add(combat_delay_ticks(duration_ms)),
                attack_bonus: (stat == CRYSTAL_STAT_MAX_DC).then_some(value).unwrap_or(0),
                defence_bonus: 0,
                stats: vec![UserItemStat { stat, value }],
            };
            packets.push(delete_packet);
            apply_or_refresh_buff(world, buff.clone());
            if let Some(packet) = client_buff_packet_for_state(world, &buff) {
                packets.push(packet);
            }
            packets
        }
        _ => packets,
    }
}

fn apply_crystal_concentration_spell(
    world: &mut World,
    skill: &SkillState,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let duration_ms = (45_u64 + u64::from(skill.level).saturating_mul(15)).saturating_mul(1_000);
    let buff = BuffState {
        key: "concentration".to_string(),
        name: "Concentration".to_string(),
        description: "Crystal Concentration buff is active.".to_string(),
        expires_at_tick: tick.saturating_add(combat_delay_ticks(duration_ms)),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: Vec::new(),
    };
    apply_or_refresh_buff(world, buff.clone());

    let mut packets = Vec::new();
    if let Some(packet) = client_buff_packet_for_state(world, &buff) {
        packets.push(packet);
    }
    packets.push(ServerPacket::SetConcentration {
        object_id,
        enabled: true,
        interrupted: false,
    });
    packets
}

fn apply_crystal_elemental_shot_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    if !world.resource::<ElementalResource>().has_elemental {
        return gather_crystal_element(world, true).into_iter().collect();
    }

    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }

    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    let orb_power =
        crystal_elemental_orb_power(world.resource::<ElementalResource>().elements_level, false);
    let damage = crystal_magic_get_damage(
        magic,
        skill.level,
        crystal_spell_attack_power(world, tick, magic).saturating_add(orb_power),
    );
    schedule_damage_to_monster(
        world,
        due_tick,
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: context.target_id,
            name: target_name,
        }),
    );
    if let Some(packet) = clear_crystal_element(world, false) {
        queue_due_packet(world, due_tick, packet);
    }
    Vec::new()
}

fn apply_crystal_elemental_barrier_spell(
    world: &mut World,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    tick: u64,
) -> Vec<ServerPacket> {
    if !world.resource::<ElementalResource>().has_elemental {
        return gather_crystal_element(world, true).into_iter().collect();
    }

    let orb_power =
        crystal_elemental_orb_power(world.resource::<ElementalResource>().elements_level, true);
    let duration_seconds = crystal_magic_damage(magic, skill.level)
        .saturating_add(orb_power)
        .max(1);
    let buff = BuffState {
        key: "elemental-barrier".to_string(),
        name: "Elemental Barrier".to_string(),
        description: "Crystal elemental barrier buff is active.".to_string(),
        expires_at_tick: tick.saturating_add(combat_delay_ticks(
            u64::try_from(duration_seconds)
                .unwrap_or(1)
                .saturating_mul(1_000),
        )),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: vec![UserItemStat {
            stat: CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT,
            value: (i32::from(skill.level) + 1).saturating_mul(10),
        }],
    };
    apply_or_refresh_buff(world, buff.clone());
    let mut packets = Vec::new();
    if let Some(packet) = clear_crystal_element(world, false) {
        packets.push(packet);
    }
    if let Some(packet) = client_buff_packet_for_state(world, &buff) {
        packets.push(packet);
    }
    packets
}

fn apply_crystal_back_step_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
) -> Vec<ServerPacket> {
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    let back_direction = rotated_direction(direction, 4);
    let max_distance = if skill.level == 0 {
        1
    } else {
        i32::from(skill.level)
    };

    let mut destination = origin.clone();
    let mut distance = 0_i32;
    for _ in 0..max_distance.max(1) {
        let next = offset_point(&destination, back_direction, 1);
        if !can_occupy(world, next.clone(), Some(player)) {
            break;
        }
        destination = next;
        distance += 1;
    }

    if distance > 0 {
        world.entity_mut(player).insert((
            Position(destination.clone()),
            super::components::Facing(direction),
        ));
    }

    let movement = ObjectMovement {
        object_id,
        position: destination.clone(),
        direction,
    };
    let mut packets = Vec::new();
    if distance > 0 {
        packets.push(ServerPacket::UserBackStep {
            location: destination,
            direction,
        });
    }
    packets.push(ServerPacket::ObjectBackStep { movement, distance });
    packets
}

fn apply_crystal_shoulder_dash_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    world
        .entity_mut(player)
        .insert(super::components::Facing(direction));

    let player_level = crystal_player_level(world, player);
    let max_distance = i32::from(skill.level)
        .saturating_add(2)
        .saturating_add(deterministic_roll(
            tick,
            usize::try_from(object_id).unwrap_or_default(),
            773 + usize::from(skill.level),
            2,
        ) as i32);
    let mut packets = Vec::new();
    let mut current = origin.clone();
    let mut travelled = 0_i32;
    let mut pushed_target =
        hostile_monster_at_position(world, &offset_point(&current, direction, 1))
            .filter(|entity| monster_level(world, *entity) < player_level);

    for _ in 0..max_distance.max(1) {
        let next = offset_point(&current, direction, 1);
        if let Some(target_entity) = pushed_target {
            if push_monster_one_tile_for_dash(world, target_entity, direction, &mut packets) == 0 {
                break;
            }
        }
        if !can_occupy(world, next.clone(), Some(player)) {
            break;
        }
        current = next.clone();
        world
            .entity_mut(player)
            .insert((Position(next.clone()), super::components::Facing(direction)));
        packets.push(ServerPacket::UserDash {
            location: next.clone(),
            direction,
        });
        packets.push(ServerPacket::ObjectDash {
            object_id,
            location: next,
            direction,
        });
        travelled += 1;
    }

    if travelled == 0 {
        packets.push(ServerPacket::UserDashFail {
            location: origin.clone(),
            direction,
        });
        packets.push(ServerPacket::ObjectDashFail {
            object_id,
            location: origin,
            direction,
        });
    } else if let Some(target_entity) = pushed_target.take() {
        if let Some(packet) = object_struck_packet(world, target_entity, object_id) {
            packets.push(packet);
        }
        damage_monster_entity(
            world,
            target_entity,
            crystal_spell_damage(world, tick, magic, skill.level),
            tick,
            &mut packets,
        );
    }

    packets.push(ServerPacket::MagicCast {
        spell: Spell::ShoulderDash,
    });
    packets
}

fn apply_crystal_slashing_burst_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    let front = offset_point(&origin, direction, 1);
    if let Some(target_entity) = hostile_monster_at_position(world, &front) {
        let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
        let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
        if let Some(target_id) = entity_object_id(world, target_entity) {
            schedule_damage_to_monster(
                world,
                due_tick,
                object_id,
                target_entity,
                crystal_spell_damage(world, tick, magic, skill.level),
                Some(target_name.clone()),
                Some(PendingMonsterDefeatAction {
                    object_id: target_id,
                    name: target_name,
                }),
            );
        }
    }

    let destination = offset_point(&origin, direction, 2);
    if !can_occupy(world, destination.clone(), Some(player)) {
        return Vec::new();
    }

    world.entity_mut(player).insert((
        Position(destination.clone()),
        super::components::Facing(direction),
    ));
    vec![ServerPacket::UserAttackMove {
        location: destination,
        direction,
    }]
}

fn apply_crystal_flash_dash_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> (Vec<ServerPacket>, bool) {
    const CRYSTAL_STUN_POISON: u16 = 16;

    let Some(object_id) = current_player_object_id(world) else {
        return (Vec::new(), false);
    };
    let Some(origin) = entity_position(world, player) else {
        return (Vec::new(), false);
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    let mut location = origin.clone();
    let mut distance = 0_i32;
    if skill.level > 1 {
        let destination = offset_point(&origin, direction, 1);
        if can_occupy(world, destination.clone(), Some(player)) {
            location = destination.clone();
            distance = 1;
            world.entity_mut(player).insert((
                Position(destination.clone()),
                super::components::Facing(direction),
            ));
        }
    } else {
        world
            .entity_mut(player)
            .insert(super::components::Facing(direction));
    }

    let mut packets = Vec::new();
    if distance > 0 {
        packets.push(ServerPacket::UserDashAttack {
            location: location.clone(),
            direction,
        });
        packets.push(ServerPacket::ObjectDashAttack {
            object_id,
            location: location.clone(),
            direction,
            distance,
        });
    } else {
        packets.push(ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id,
                location: origin,
                direction,
                spell: Spell::FlashDash as u8,
                level: skill.level,
                attack_type: 0,
            },
        });
    }

    let mut hit_target = false;
    let hit_location = offset_point(&location, direction, 1);
    if let Some(target_entity) = hostile_monster_at_position(world, &hit_location) {
        let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(300));
        let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
        if let Some(target_id) = entity_object_id(world, target_entity) {
            hit_target = true;
            schedule_damage_to_monster(
                world,
                due_tick,
                object_id,
                target_entity,
                crystal_spell_damage(world, tick, magic, skill.level),
                Some(target_name.clone()),
                Some(PendingMonsterDefeatAction {
                    object_id: target_id,
                    name: target_name,
                }),
            );
            apply_monster_poison(
                world,
                target_entity,
                CRYSTAL_STUN_POISON,
                0,
                due_tick,
                combat_delay_ticks(
                    u64::from(skill.level)
                        .saturating_add(1)
                        .saturating_mul(1_000),
                ),
            );
            queue_due_packet(
                world,
                due_tick,
                ServerPacket::ObjectPoisoned {
                    object_id: target_id,
                    poison: CRYSTAL_STUN_POISON,
                },
            );
        }
    }

    packets.push(ServerPacket::MagicCast {
        spell: Spell::FlashDash,
    });
    (packets, hit_target)
}

fn push_monster_one_tile_for_dash(
    world: &mut World,
    entity: Entity,
    direction: MirDirection,
    packets: &mut Vec<ServerPacket>,
) -> i32 {
    let Some(object_id) = entity_object_id(world, entity) else {
        return 0;
    };
    let Some(position) = entity_position(world, entity) else {
        return 0;
    };
    let destination = offset_point(&position, direction, 1);
    if !can_occupy(world, destination.clone(), Some(entity)) {
        return 0;
    }
    let reverse = rotated_direction(direction, 4);
    world.entity_mut(entity).insert((
        Position(destination.clone()),
        super::components::Facing(reverse),
    ));
    packets.push(ServerPacket::ObjectPushed {
        object_id,
        location: destination,
        direction: reverse,
    });
    1
}

#[allow(deprecated)]
fn hostile_monster_at_position(world: &World, position: &Point) -> Option<Entity> {
    world.iter_entities().find_map(|entity| {
        let agent = entity.get::<MonsterAgent>()?;
        if agent.dead || !agent.hostile_to_player {
            return None;
        }
        let entity_position = entity.get::<Position>()?;
        (&entity_position.0 == position).then_some(entity.id())
    })
}

#[allow(deprecated)]
fn hostile_monsters_at_position(world: &World, position: &Point) -> Vec<Entity> {
    world
        .iter_entities()
        .filter_map(|entity| {
            let agent = entity.get::<MonsterAgent>()?;
            if agent.dead || !agent.hostile_to_player {
                return None;
            }
            let entity_position = entity.get::<Position>()?;
            (&entity_position.0 == position).then_some(entity.id())
        })
        .collect()
}

#[allow(deprecated)]
fn hostile_monsters_in_square(world: &World, center: &Point, radius: i32) -> Vec<Entity> {
    world
        .iter_entities()
        .filter_map(|entity| {
            let agent = entity.get::<MonsterAgent>()?;
            if agent.dead || !agent.hostile_to_player {
                return None;
            }
            let position = entity.get::<Position>()?;
            let dx = (position.0.x - center.x).abs();
            let dy = (position.0.y - center.y).abs();
            (dx <= radius && dy <= radius).then_some(entity.id())
        })
        .collect()
}

fn schedule_damage_to_hostile_monsters_at(
    world: &mut World,
    position: &Point,
    due_tick: u64,
    caster_object_id: u32,
    damage: i32,
) {
    for target_entity in hostile_monsters_at_position(world, position) {
        schedule_damage_to_hostile_monster_entity(
            world,
            target_entity,
            due_tick,
            caster_object_id,
            damage,
        );
    }
}

fn schedule_damage_to_hostile_monster_entity(
    world: &mut World,
    target_entity: Entity,
    due_tick: u64,
    caster_object_id: u32,
    damage: i32,
) {
    let Some(target_id) = entity_object_id(world, target_entity) else {
        return;
    };
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    schedule_damage_to_monster(
        world,
        due_tick,
        caster_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: target_id,
            name: target_name,
        }),
    );
}

fn monster_is_undead(world: &World, entity: Entity) -> bool {
    entity_name(world, entity)
        .and_then(|name| crystal_monster_by_name(&name).map(|template| template.undead))
        .unwrap_or(false)
}

fn crystal_player_level(world: &World, player: Entity) -> u16 {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.level)
        .or_else(|| {
            world
                .entity(player)
                .get::<CharacterBody>()
                .map(|body| body.level)
        })
        .unwrap_or(1)
}

fn monster_level(world: &World, entity: Entity) -> u16 {
    entity_name(world, entity)
        .and_then(|name| crystal_monster_by_name(&name).map(|template| template.level))
        .unwrap_or(0)
}

fn apply_crystal_repulsion_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let player_level = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.level)
        .or_else(|| {
            world
                .entity(player)
                .get::<CharacterBody>()
                .map(|body| body.level)
        })
        .unwrap_or_default();
    let candidates = {
        #[allow(deprecated)]
        world
            .iter_entities()
            .filter_map(|entity| {
                let agent = entity.get::<MonsterAgent>()?;
                if agent.dead || !agent.hostile_to_player {
                    return None;
                }
                let position = entity.get::<Position>()?;
                (tile_distance(&player_position, &position.0) <= 1).then_some(entity.id())
            })
            .collect::<Vec<_>>()
    };

    let mut packets = Vec::new();
    let mut success_index = 0_usize;
    for entity in candidates {
        let Some(object_id) = entity_object_id(world, entity) else {
            continue;
        };
        let Some(name) = entity_name(world, entity) else {
            continue;
        };
        if !crystal_dynamic_monster_template(&name)
            .map(|template| template.monster_can_push)
            .unwrap_or(true)
        {
            continue;
        }
        let monster_level = crystal_monster_by_name(&name)
            .map(|template| template.level)
            .unwrap_or_default();
        if monster_level >= player_level {
            continue;
        }

        let chance = 6_i32
            .saturating_add(i32::from(skill.level).saturating_mul(3))
            .saturating_add(i32::from(player_level))
            .saturating_sub(i32::from(monster_level));
        if chance <= 0 {
            continue;
        }
        if chance < 20 {
            let roll = deterministic_roll(
                tick,
                usize::try_from(object_id).unwrap_or_default(),
                701 + usize::from(skill.level),
                20,
            );
            if roll >= u64::try_from(chance).unwrap_or_default() {
                continue;
            }
        }

        let Some(start) = entity_position(world, entity) else {
            continue;
        };
        let Some(push_direction) = direction_toward(&player_position, &start) else {
            continue;
        };
        let distance = 1_i32
            .saturating_add((i32::from(skill.level) - 1).max(0))
            .saturating_add(
                i32::try_from(deterministic_roll(
                    tick,
                    usize::try_from(object_id).unwrap_or_default(),
                    719 + success_index,
                    2,
                ))
                .unwrap_or_default(),
            );
        let pushed = push_monster_for_crystal_repulsion(
            world,
            entity,
            object_id,
            push_direction,
            distance,
            &mut packets,
        );
        if pushed == 0 {
            continue;
        }
        success_index += 1;

        let thunder_element_repulsion = world
            .entity(entity)
            .get::<MonsterAgent>()
            .map(|agent| agent.ai == 49)
            .unwrap_or(false);
        if thunder_element_repulsion {
            if let Some(packet) = object_struck_packet(world, entity, player_object_id) {
                packets.push(packet);
            }
            let damage = crystal_magic_damage(magic, skill.level)
                .max(50)
                .saturating_mul(pushed.max(1));
            if let Some(mut vitals) = world.entity_mut(entity).get_mut::<MonsterVitals>() {
                vitals.hp = (vitals.hp - damage).max(1);
            }
            if let Some(info) = object_health_info_for_entity(world, entity, 0) {
                packets.push(ServerPacket::ObjectHealth { info });
            }
        }
    }

    packets
}

fn push_monster_for_crystal_repulsion(
    world: &mut World,
    entity: Entity,
    object_id: u32,
    direction: MirDirection,
    distance: i32,
    packets: &mut Vec<ServerPacket>,
) -> i32 {
    let mut pushed = 0_i32;
    let reverse = rotated_direction(direction, 4);
    let Some(mut position) = entity_position(world, entity) else {
        return pushed;
    };

    for _ in 0..distance.max(1) {
        let next = offset_point(&position, direction, 1);
        if !can_occupy(world, next.clone(), Some(entity)) {
            break;
        }
        position = next.clone();
        world
            .entity_mut(entity)
            .insert((Position(next.clone()), super::components::Facing(reverse)));
        packets.push(ServerPacket::ObjectPushed {
            object_id,
            location: next,
            direction: reverse,
        });
        pushed += 1;
    }

    pushed
}

fn apply_crystal_storm_escape_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    const CRYSTAL_STORM_ESCAPE_EFFECT: u8 = 23;

    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(context) = context else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };

    let damage_due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let targets = {
        #[allow(deprecated)]
        world
            .iter_entities()
            .filter_map(|entity| {
                let agent = entity.get::<MonsterAgent>()?;
                if agent.dead || !agent.hostile_to_player {
                    return None;
                }
                let position = entity.get::<Position>()?;
                (tile_distance(&origin, &position.0) <= 2).then_some(entity.id())
            })
            .collect::<Vec<_>>()
    };
    for target_entity in targets {
        let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
        let Some(target_id) = entity_object_id(world, target_entity) else {
            continue;
        };
        schedule_damage_to_monster(
            world,
            damage_due_tick,
            player_object_id,
            target_entity,
            damage,
            Some(target_name.clone()),
            Some(PendingMonsterDefeatAction {
                object_id: target_id,
                name: target_name,
            }),
        );
    }

    let teleport_success = skill.level >= 3
        || deterministic_roll(
            tick,
            usize::try_from(player_object_id).unwrap_or_default(),
            751,
            4,
        ) < u64::from(skill.level) + 1;
    if !teleport_success || !can_occupy(world, context.target.clone(), Some(player)) {
        return Vec::new();
    }

    let destination = context.target.clone();
    world.entity_mut(player).insert((
        Position(destination.clone()),
        super::components::Facing(context.direction),
    ));
    let buff = BuffState {
        key: "temporal-flux".to_string(),
        name: "Temporal Flux".to_string(),
        description: "Crystal temporal flux teleport penalty is active.".to_string(),
        expires_at_tick: tick + combat_delay_ticks(30_000),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: vec![UserItemStat {
            stat: CRYSTAL_STAT_TELEPORT_MANA_PENALTY_PERCENT,
            value: 30,
        }],
    };
    apply_or_refresh_buff(world, buff.clone());

    let mut packets = vec![
        ServerPacket::UserLocation {
            location: UserLocation {
                position: destination,
                direction: context.direction,
            },
        },
        ServerPacket::ObjectEffect {
            info: ObjectEffectInfo {
                object_id: player_object_id,
                effect: CRYSTAL_STORM_ESCAPE_EFFECT,
                effect_type: 0,
                delay_time: 0,
                time: 0,
            },
        },
    ];
    if let Some(packet) = client_buff_packet_for_state(world, &buff) {
        packets.push(packet);
    }
    packets
}

fn apply_crystal_fire_wall_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let target = context
        .map(|context| context.target.clone())
        .unwrap_or(player_position);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let locations = fire_wall_cross_locations(world, &target);

    for (index, location) in locations.iter().enumerate() {
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id: player_object_id
                        .saturating_add(80_000)
                        .saturating_add(u32::try_from(index).unwrap_or_default()),
                    location: location.clone(),
                    spell: Spell::FireWall,
                    direction: MirDirection::Up,
                    param: false,
                },
            },
        );
    }

    if !locations.is_empty() {
        world
            .resource_mut::<RuntimeQueueResource>()
            .pending_ground_spell_actions
            .push(PendingGroundSpellAction {
                spell: Spell::FireWall,
                caster_object_id: player_object_id,
                locations,
                damage,
                next_tick: due_tick,
                expires_at_tick: due_tick.saturating_add(combat_delay_ticks(
                    10_000_u64.saturating_add(
                        u64::try_from(damage.max(0) / 2)
                            .unwrap_or_default()
                            .saturating_mul(1_000),
                    ),
                )),
                tick_interval: combat_delay_ticks(2_000),
            });
    }

    Vec::new()
}

fn apply_crystal_lightning_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let damage = crystal_spell_damage(world, tick, magic, skill.level);

    let mut location = origin;
    for _ in 0..6 {
        location = offset_point(&location, direction, 1);
        if is_blocked_tile(world, &location) {
            continue;
        }
        let Some(target_entity) = hostile_monster_at_position(world, &location) else {
            continue;
        };
        let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
        let Some(target_id) = entity_object_id(world, target_entity) else {
            continue;
        };
        schedule_damage_to_monster(
            world,
            due_tick,
            player_object_id,
            target_entity,
            damage,
            Some(target_name.clone()),
            Some(PendingMonsterDefeatAction {
                object_id: target_id,
                name: target_name,
            }),
        );
    }

    Vec::new()
}

fn apply_crystal_hell_fire_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    let mut directions = vec![direction];
    if skill.level == 3 {
        directions.push(rotated_direction(direction, 1));
        directions.push(rotated_direction(direction, -1));
    }
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));

    for direction in directions {
        let mut location = origin.clone();
        for _ in 0..4 {
            location = offset_point(&location, direction, 1);
            if is_blocked_tile(world, &location) {
                break;
            }
            schedule_damage_to_hostile_monsters_at(
                world,
                &location,
                due_tick,
                player_object_id,
                damage,
            );
        }
    }
    Vec::new()
}

fn apply_crystal_fire_bang_family_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let target = context
        .map(|context| context.target.clone())
        .or_else(|| entity_position(world, player))
        .unwrap_or(Point { x: 0, y: 0 });
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    for target_entity in hostile_monsters_in_square(world, &target, 1) {
        schedule_damage_to_hostile_monster_entity(
            world,
            target_entity,
            due_tick,
            player_object_id,
            damage,
        );
    }
    Vec::new()
}

fn apply_crystal_blizzard_family_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let target = context
        .map(|context| context.target.clone())
        .or_else(|| entity_position(world, player))
        .unwrap_or(Point { x: 0, y: 0 });
    let spell = if magic.spell == "MeteorStrike" {
        Spell::MeteorStrike
    } else {
        Spell::Blizzard
    };
    let locations = square_locations(&target, 2);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    for location in &locations {
        let object_id = allocate_runtime_monster_object_id(world);
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id,
                    location: location.clone(),
                    spell,
                    direction: MirDirection::Up,
                    param: location == &target,
                },
            },
        );
    }
    let ground_damage = crystal_spell_damage(world, tick, magic, skill.level);
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_ground_spell_actions
        .push(PendingGroundSpellAction {
            spell,
            caster_object_id: player_object_id,
            damage: ground_damage,
            locations,
            next_tick: due_tick.saturating_add(combat_delay_ticks(800)),
            expires_at_tick: due_tick.saturating_add(combat_delay_ticks(3_000)),
            tick_interval: combat_delay_ticks(440),
        });
    Vec::new()
}

fn apply_crystal_meteor_shower_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let player_position = entity_position(world, player).unwrap_or(Point { x: 0, y: 0 });
    let direction = entity_facing(world, player).unwrap_or(MirDirection::Down);
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    schedule_damage_to_hostile_monster_entity(
        world,
        target_entity,
        due_tick,
        player_object_id,
        damage,
    );

    let mut secondary_targets = hostile_monsters_in_square(world, &target_position, 4)
        .into_iter()
        .filter(|entity| *entity != target_entity)
        .filter_map(|entity| {
            let position = entity_position(world, entity)?;
            let object_id = entity_object_id(world, entity)?;
            Some((
                entity,
                object_id,
                tile_distance(&target_position, &position),
            ))
        })
        .collect::<Vec<_>>();
    secondary_targets.sort_by_key(|(_, object_id, distance)| (*distance, *object_id));
    secondary_targets.truncate(3);

    let secondary_target_ids = secondary_targets
        .iter()
        .map(|(_, object_id, _)| *object_id)
        .collect::<Vec<_>>();
    for (entity, _, _) in secondary_targets {
        schedule_damage_to_hostile_monster_entity(
            world,
            entity,
            due_tick,
            player_object_id,
            damage.saturating_div(2).max(1),
        );
    }

    vec![
        ServerPacket::Magic {
            spell: Spell::MeteorShower,
            target_id: context.target_id,
            target: target_position.clone(),
            cast: true,
            level: skill.level,
            secondary_target_ids: secondary_target_ids.clone(),
        },
        ServerPacket::ObjectMagic {
            object_id: player_object_id,
            location: player_position,
            direction,
            spell: Spell::MeteorShower,
            target_id: context.target_id,
            target: target_position,
            cast: true,
            level: skill.level,
            self_broadcast: false,
            secondary_target_ids,
        },
    ]
}

fn apply_crystal_fire_bounce_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let player_position = entity_position(world, player).unwrap_or(Point { x: 0, y: 0 });
    let mut chain = vec![target_entity];
    let mut previous_position =
        entity_position(world, target_entity).unwrap_or_else(|| context.target.clone());
    let max_bounces = usize::from(skill.level).saturating_add(2);
    for _ in 0..max_bounces {
        let mut candidates = hostile_monsters_in_square(world, &previous_position, 4)
            .into_iter()
            .filter(|entity| !chain.contains(entity))
            .filter_map(|entity| {
                let position = entity_position(world, entity)?;
                let object_id = entity_object_id(world, entity)?;
                Some((
                    entity,
                    object_id,
                    tile_distance(&previous_position, &position),
                ))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, object_id, distance)| (*distance, *object_id));
        let Some((next_entity, _, _)) = candidates.into_iter().next() else {
            break;
        };
        previous_position =
            entity_position(world, next_entity).unwrap_or_else(|| previous_position.clone());
        chain.push(next_entity);
    }

    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let mut due_tick = tick;
    let mut source_object_id = player_object_id;
    let mut source_position = player_position;
    for (index, target_entity) in chain.into_iter().enumerate() {
        let Some(target_position) = entity_position(world, target_entity) else {
            continue;
        };
        let Some(target_object_id) = entity_object_id(world, target_entity) else {
            continue;
        };
        let delay_ticks = if index == 0 {
            ranged_attack_delay_ticks(&source_position, &target_position)
        } else {
            let distance = u64::try_from(tile_distance(&source_position, &target_position).max(0))
                .unwrap_or_default();
            combat_delay_ticks(distance.saturating_mul(50).max(1))
        };
        due_tick = queued_before_world_tick_due_tick(due_tick, delay_ticks);
        if index > 0 {
            queue_due_packet(
                world,
                due_tick,
                ServerPacket::ObjectProjectile {
                    spell: Spell::FireBounce,
                    source_id: source_object_id,
                    destination_id: target_object_id,
                },
            );
        }
        schedule_damage_to_hostile_monster_entity(
            world,
            target_entity,
            due_tick,
            player_object_id,
            damage,
        );
        source_object_id = target_object_id;
        source_position = target_position;
    }

    Vec::new()
}

fn apply_crystal_thunder_bolt_spell(
    world: &mut World,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let mut damage = crystal_spell_damage(world, tick, magic, skill.level);
    if monster_is_undead(world, target_entity) {
        damage = damage.saturating_mul(3).saturating_div(2).max(1);
    }
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    schedule_damage_to_hostile_monster_entity(
        world,
        target_entity,
        due_tick,
        player_object_id,
        damage,
    );
    Vec::new()
}

fn apply_crystal_electric_shock_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if agent.dead || !agent.hostile_to_player {
        return Vec::new();
    }
    if monster_level(world, target_entity) > crystal_player_level(world, player).saturating_add(2) {
        return Vec::new();
    }
    let release_tick = tick.saturating_add(combat_delay_ticks(
        (u64::from(skill.level).saturating_mul(5) + 10).saturating_mul(1_000),
    ));
    if let Some(mut agent) = world.entity_mut(target_entity).get_mut::<MonsterAgent>() {
        agent.tracking_player = false;
        agent.next_move_tick = agent.next_move_tick.max(release_tick);
        agent.next_attack_tick = agent.next_attack_tick.max(release_tick);
    }
    Vec::new()
}

fn apply_crystal_flame_disruptor_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };
    let mut damage = crystal_spell_damage(world, tick, magic, skill.level);
    if !monster_is_undead(world, target_entity) {
        damage = damage.saturating_mul(3).saturating_div(2).max(1);
    }
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    schedule_damage_to_hostile_monster_entity(
        world,
        target_entity,
        due_tick,
        player_object_id,
        damage,
    );
    Vec::new()
}

fn apply_crystal_ice_thrust_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    const CRYSTAL_POISON_FROZEN: u16 = 8;

    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    let front = offset_point(&origin, direction, 1);
    let starts = [
        offset_point(&front, rotated_direction(direction, -1), 1),
        offset_point(&front, direction, 1),
        offset_point(&front, rotated_direction(direction, 1), 1),
    ];
    let near_damage = crystal_spell_damage_with_crit(
        world,
        tick,
        magic,
        skill.level,
        crystal_luck_crit_chance(world),
    );
    let far_damage = near_damage.saturating_mul(3).saturating_div(5).max(1);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(1_500));

    for start in starts {
        for row in 0..3 {
            let hit_point = offset_point(&start, direction, row);
            let damage = if row <= 1 { near_damage } else { far_damage };
            for target_entity in hostile_monsters_at_position(world, &hit_point) {
                schedule_damage_to_hostile_monster_entity(
                    world,
                    target_entity,
                    due_tick,
                    player_object_id,
                    damage,
                );
                if monster_level(world, target_entity) <= crystal_player_level(world, player) + 10 {
                    if let Some(object_id) = entity_object_id(world, target_entity) {
                        apply_monster_poison(
                            world,
                            target_entity,
                            CRYSTAL_POISON_FROZEN,
                            0,
                            tick,
                            combat_delay_ticks(
                                (5_u64 + u64::from(skill.level)).saturating_mul(1_000),
                            ),
                        );
                        queue_due_packet(
                            world,
                            due_tick,
                            ServerPacket::ObjectPoisoned {
                                object_id,
                                poison: CRYSTAL_POISON_FROZEN,
                            },
                        );
                    }
                }
            }
        }
    }
    Vec::new()
}

fn apply_crystal_frost_crunch_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };

    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    schedule_damage_to_monster(
        world,
        due_tick,
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: context.target_id,
            name: target_name,
        }),
    );

    let freeze_ms = (2_u64 + u64::from(skill.level)).saturating_mul(1_000);
    let release_tick = tick.saturating_add(combat_delay_ticks(freeze_ms));
    if let Some(mut agent) = world.entity_mut(target_entity).get_mut::<MonsterAgent>() {
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
                object_id: context.target_id,
                expire_time: i64::try_from(freeze_ms).unwrap_or(i64::MAX),
                infinite: false,
                paused: false,
                stats: Vec::new(),
                values: Vec::new(),
            },
        },
    );

    Vec::new()
}

fn apply_crystal_vampirism_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };

    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    schedule_damage_to_monster(
        world,
        due_tick,
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: context.target_id,
            name: target_name,
        }),
    );
    schedule_heal_to_player(
        world,
        due_tick,
        damage
            .saturating_mul(i32::from(skill.level).saturating_add(1))
            .saturating_div(3)
            .max(1),
    );

    Vec::new()
}

fn apply_crystal_turn_undead_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead
        || !target_agent.hostile_to_player
        || !monster_is_undead(world, target_entity)
    {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };

    let player_level = crystal_player_level(world, player);
    let target_level = monster_level(world, target_entity);
    let target_hp = world
        .entity(target_entity)
        .get::<MonsterVitals>()
        .map(|vitals| vitals.hp)
        .unwrap_or_default();
    let base_damage = crystal_magic_damage(magic, skill.level)
        .saturating_add(i32::from(skill.level).saturating_mul(5));
    let damage = if player_level >= target_level {
        target_hp.max(base_damage)
    } else {
        base_damage.max(1)
    };
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    schedule_damage_to_monster(
        world,
        due_tick,
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: context.target_id,
            name: target_name,
        }),
    );

    Vec::new()
}

fn apply_crystal_thunder_storm_family_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    _context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let base_damage = crystal_spell_damage(world, tick, magic, skill.level);
    let targets = hostile_monsters_in_square(world, &player_position, 2);

    for target_entity in targets {
        let mut damage = base_damage;
        if magic.spell == "ThunderStorm" && !monster_is_undead(world, target_entity) {
            damage = (damage / 10).max(1);
        }
        let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
        let Some(target_id) = entity_object_id(world, target_entity) else {
            continue;
        };
        schedule_damage_to_monster(
            world,
            due_tick,
            player_object_id,
            target_entity,
            damage,
            Some(target_name.clone()),
            Some(PendingMonsterDefeatAction {
                object_id: target_id,
                name: target_name,
            }),
        );
    }

    Vec::new()
}

fn apply_crystal_napalm_shot_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(center_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(center_agent) = world.entity(center_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if center_agent.dead || !center_agent.hostile_to_player {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(center_position) = entity_position(world, center_entity) else {
        return Vec::new();
    };

    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &center_position),
    );
    let damage =
        crystal_archer_state_damage(world, crystal_spell_damage(world, tick, magic, skill.level));
    for target_entity in hostile_monsters_in_square(world, &center_position, 2) {
        let Some(target_id) = entity_object_id(world, target_entity) else {
            continue;
        };
        let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
        schedule_damage_to_monster(
            world,
            due_tick,
            player_object_id,
            target_entity,
            damage,
            Some(target_name.clone()),
            Some(PendingMonsterDefeatAction {
                object_id: target_id,
                name: target_name,
            }),
        );
    }
    Vec::new()
}

fn apply_crystal_delayed_explosion_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    const CRYSTAL_POISON_DELAYED_EXPLOSION: u16 = 64;
    const CRYSTAL_SPELL_EFFECT_DELAYED_EXPLOSION: u8 = 15;

    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };

    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    schedule_damage_to_monster(
        world,
        due_tick,
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: context.target_id,
            name: target_name,
        }),
    );

    let duration_seconds = damage
        .saturating_mul(2)
        .saturating_add((i32::from(skill.level) + 1).saturating_mul(7))
        .max(1);
    apply_monster_poison(
        world,
        target_entity,
        CRYSTAL_POISON_DELAYED_EXPLOSION,
        0,
        due_tick,
        combat_delay_ticks(
            u64::try_from(duration_seconds)
                .unwrap_or(1)
                .saturating_mul(1_000),
        ),
    );
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectPoisoned {
            object_id: context.target_id,
            poison: CRYSTAL_POISON_DELAYED_EXPLOSION,
        },
    );
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectEffect {
            info: ObjectEffectInfo {
                object_id: context.target_id,
                effect: CRYSTAL_SPELL_EFFECT_DELAYED_EXPLOSION,
                effect_type: 0,
                delay_time: 0,
                time: 0,
            },
        },
    );

    let explosion_tick = due_tick.saturating_add(combat_delay_ticks(4_000));
    queue_due_packet(
        world,
        explosion_tick,
        ServerPacket::ObjectEffect {
            info: ObjectEffectInfo {
                object_id: context.target_id,
                effect: CRYSTAL_SPELL_EFFECT_DELAYED_EXPLOSION,
                effect_type: 2,
                delay_time: 0,
                time: 0,
            },
        },
    );
    for affected_entity in hostile_monsters_in_square(world, &target_position, 1) {
        let Some(affected_id) = entity_object_id(world, affected_entity) else {
            continue;
        };
        let affected_name =
            entity_name(world, affected_entity).unwrap_or_else(|| "Target".to_string());
        schedule_damage_to_monster(
            world,
            explosion_tick,
            player_object_id,
            affected_entity,
            damage,
            Some(affected_name.clone()),
            Some(PendingMonsterDefeatAction {
                object_id: affected_id,
                name: affected_name,
            }),
        );
    }
    queue_due_packet(
        world,
        explosion_tick.saturating_add(1),
        ServerPacket::RemoveDelayedExplosion {
            object_id: context.target_id,
        },
    );

    Vec::new()
}

pub(super) fn tick_ground_spell_actions(
    world: &mut World,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let actions = {
        let mut queue = world.resource_mut::<RuntimeQueueResource>();
        std::mem::take(&mut queue.pending_ground_spell_actions)
    };
    let mut retained = Vec::new();

    for mut action in actions {
        if tick >= action.expires_at_tick {
            continue;
        }
        if tick >= action.next_tick {
            let mut detonated_trap = false;
            if matches!(
                action.spell,
                Spell::FireWall
                    | Spell::Blizzard
                    | Spell::MeteorStrike
                    | Spell::PoisonCloud
                    | Spell::ExplosiveTrap
            ) {
                // Ground-spell ticks are deferred past the cast scope; re-establish the magic
                // DefenceType so each hit subtracts the target's MAC armour.
                let previous_ground_defence = set_cast_defence(world, Some(CrystalDefence::Mac));
                for location in &action.locations {
                    for target_entity in hostile_monsters_at_position(world, location) {
                        if let Some(packet) =
                            object_struck_packet(world, target_entity, action.caster_object_id)
                        {
                            packets.push(packet);
                        }
                        damage_monster_entity(world, target_entity, action.damage, tick, packets);
                        if action.spell == Spell::PoisonCloud {
                            if let Some(object_id) = entity_object_id(world, target_entity) {
                                apply_monster_poison(
                                    world,
                                    target_entity,
                                    1,
                                    action.damage.saturating_div(3).max(1),
                                    tick,
                                    combat_delay_ticks(12_000),
                                );
                                packets.push(ServerPacket::ObjectPoisoned {
                                    object_id,
                                    poison: 1,
                                });
                            }
                        }
                        if action.spell == Spell::Blizzard
                            && deterministic_roll(
                                tick,
                                action.caster_object_id as usize,
                                0xB112 + target_entity.index() as usize,
                                8,
                            ) == 0
                        {
                            if let Some(object_id) = entity_object_id(world, target_entity) {
                                // Crystal Blizzard: 1/8 chance per tick to apply Slow (type 4,
                                // ~5 ticks at 2s each = 10s for the common Freezing=0 case).
                                apply_monster_poison(
                                    world,
                                    target_entity,
                                    4,
                                    0,
                                    tick,
                                    combat_delay_ticks(10_000),
                                );
                                packets.push(ServerPacket::ObjectPoisoned {
                                    object_id,
                                    poison: 4,
                                });
                            }
                        }
                        if action.spell == Spell::ExplosiveTrap {
                            detonated_trap = true;
                        }
                    }
                }
                set_cast_defence(world, previous_ground_defence);
            }
            if detonated_trap {
                continue;
            }
            action.next_tick = tick.saturating_add(action.tick_interval.max(1));
        }
        if action.next_tick < action.expires_at_tick {
            retained.push(action);
        }
    }

    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_ground_spell_actions = retained;
}

fn fire_wall_cross_locations(world: &World, target: &Point) -> Vec<Point> {
    [
        target.clone(),
        offset_point(target, MirDirection::Up, 1),
        offset_point(target, MirDirection::Right, 1),
        offset_point(target, MirDirection::Down, 1),
        offset_point(target, MirDirection::Left, 1),
    ]
    .into_iter()
    .filter(|location| !is_blocked_tile(world, location))
    .collect()
}

fn apply_crystal_area_healing_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
    healing_circle: bool,
) -> Vec<ServerPacket> {
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let target = context
        .map(|context| context.target.clone())
        .unwrap_or_else(|| player_position.clone());
    if tile_distance(&player_position, &target) > 1 {
        return Vec::new();
    }

    let delay_ms = if healing_circle { 1_700 } else { 500 };
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(delay_ms));
    let heal = if healing_circle {
        25
    } else {
        crystal_spell_damage(world, tick, magic, skill.level)
    };
    schedule_heal_to_player(world, due_tick, heal);

    if healing_circle {
        let object_id = current_player_object_id(world)
            .unwrap_or_default()
            .saturating_add(70_000)
            .saturating_add((tick as u32) % 1_000);
        let direction = context
            .map(|context| context.direction)
            .or_else(|| entity_facing(world, player))
            .unwrap_or(MirDirection::Up);
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id,
                    location: target,
                    spell: Spell::HealingCircle,
                    direction,
                    param: false,
                },
            },
        );
    }

    Vec::new()
}

fn apply_crystal_curse_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    let Some(delete_packet) = consume_equipped_crystal_amulet(world, 0, 1) else {
        return packets;
    };
    packets.push(delete_packet);

    let Some(player_object_id) = current_player_object_id(world) else {
        return packets;
    };
    let Some(player_position) = entity_position(world, player) else {
        return packets;
    };
    let target = context
        .map(|context| context.target.clone())
        .unwrap_or_else(|| player_position.clone());
    let success_window = (10_i32 - ((i32::from(skill.level) + 1) * 2)).max(1) as usize;
    let roll = deterministic_roll(
        tick,
        usize::try_from(player_object_id).unwrap_or_default(),
        491 + usize::from(skill.level),
        u64::try_from(success_window).unwrap_or(1),
    );
    if roll > 2 {
        return packets;
    }

    let delay_ms = u64::from(tile_distance(&player_position, &target) as u32)
        .saturating_mul(50)
        .saturating_add(500);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(delay_ms));
    let duration_seconds = crystal_magic_damage(magic, skill.level).max(1);
    let stat_penalty = 1 + (i32::from(skill.level) + 1) * 2;
    let duration_ms = i64::from(duration_seconds).saturating_mul(1_000);
    let curse_stats = vec![
        UserItemStat {
            stat: CRYSTAL_STAT_MAX_DC_RATE_PERCENT,
            value: stat_penalty.saturating_neg(),
        },
        UserItemStat {
            stat: CRYSTAL_STAT_MAX_MC_RATE_PERCENT,
            value: stat_penalty.saturating_neg(),
        },
        UserItemStat {
            stat: CRYSTAL_STAT_MAX_SC_RATE_PERCENT,
            value: stat_penalty.saturating_neg(),
        },
        UserItemStat {
            stat: CRYSTAL_STAT_ATTACK_SPEED_RATE_PERCENT,
            value: 0,
        },
    ];

    let affected = {
        #[allow(deprecated)]
        world
            .iter_entities()
            .filter_map(|entity| {
                let agent = entity.get::<MonsterAgent>()?;
                if agent.dead || !agent.hostile_to_player {
                    return None;
                }
                let position = entity.get::<Position>()?;
                (tile_distance(&position.0, &target) <= 3).then_some(entity.id())
            })
            .collect::<Vec<_>>()
    };
    let release_tick = tick.saturating_add(combat_delay_ticks(
        u64::try_from(duration_seconds)
            .unwrap_or_default()
            .saturating_mul(1_000),
    ));
    for entity in affected {
        if let Some(mut agent) = world.entity_mut(entity).get_mut::<MonsterAgent>() {
            agent.next_move_tick = agent.next_move_tick.max(release_tick);
            agent.next_attack_tick = agent.next_attack_tick.max(release_tick);
        }
        let Some(object_id) = entity_object_id(world, entity) else {
            continue;
        };
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::AddBuff {
                buff: ClientBuff {
                    buff_type: 12,
                    visible: false,
                    object_id,
                    expire_time: duration_ms,
                    infinite: false,
                    paused: false,
                    stats: curse_stats.clone(),
                    values: Vec::new(),
                },
            },
        );
    }

    packets
}

fn apply_crystal_trap_hexagon_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let target = context
        .map(|context| context.target.clone())
        .unwrap_or_else(|| player_position.clone());
    let affected = {
        #[allow(deprecated)]
        world
            .iter_entities()
            .filter_map(|entity| {
                let agent = entity.get::<MonsterAgent>()?;
                if agent.dead || !agent.hostile_to_player {
                    return None;
                }
                let position = entity.get::<Position>()?;
                (tile_distance(&position.0, &target) <= 1).then_some(entity.id())
            })
            .collect::<Vec<_>>()
    };
    if affected.is_empty() {
        return Vec::new();
    }

    let Some(delete_packet) = consume_equipped_crystal_amulet(world, 0, 1) else {
        return Vec::new();
    };

    let duration_ms = (u64::from(skill.level).saturating_mul(5) + 10).saturating_mul(1_000);
    let release_tick = tick.saturating_add(combat_delay_ticks(duration_ms));
    for entity in affected {
        if let Some(mut agent) = world.entity_mut(entity).get_mut::<MonsterAgent>() {
            agent.tracking_player = false;
            agent.next_move_tick = agent.next_move_tick.max(release_tick);
            agent.next_attack_tick = agent.next_attack_tick.max(release_tick);
        }
    }

    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    for spawnpoint in trap_hexagon_spell_points(&target) {
        let object_id = allocate_runtime_monster_object_id(world);
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id,
                    location: spawnpoint,
                    spell: Spell::TrapHexagon,
                    direction: MirDirection::Up,
                    param: false,
                },
            },
        );
    }

    vec![delete_packet]
}

fn trap_hexagon_spell_points(center: &Point) -> Vec<Point> {
    [
        MirDirection::Up,
        MirDirection::Right,
        MirDirection::Down,
        MirDirection::Left,
    ]
    .into_iter()
    .flat_map(|direction| {
        let start = offset_point(center, direction, 2);
        let side_direction = if matches!(direction, MirDirection::Up | MirDirection::Down) {
            MirDirection::Right
        } else {
            MirDirection::Up
        };
        [
            offset_point(&start, side_direction, 1),
            offset_point(&start, side_direction, -1),
        ]
    })
    .filter(|point| point.x > 0 && point.y > 0)
    .collect()
}

fn apply_crystal_poisoning_spell(
    world: &mut World,
    _player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    const GREEN_POISON: u16 = 1;
    const RED_POISON: u16 = 2;

    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }

    let Some((delete_packet, poison_shape)) = consume_equipped_crystal_poison(world, None, 1)
    else {
        return Vec::new();
    };

    let player_object_id = current_player_object_id(world).unwrap_or_default();
    let power = crystal_spell_damage(world, tick, magic, skill.level);
    let poison_attack = current_player_crystal_stat(world, CRYSTAL_STAT_POISON_ATTACK).max(0);
    let poison_attack_bonus = if poison_attack > 0 {
        deterministic_roll(
            tick,
            usize::try_from(player_object_id).unwrap_or_default(),
            617 + usize::from(skill.level),
            u64::try_from(poison_attack).unwrap_or_default(),
        ) as i32
    } else {
        0
    };
    let green_damage = power
        .saturating_div(15)
        .saturating_add(i32::from(skill.level) + 1)
        .saturating_add(poison_attack_bonus)
        .max(1);
    let duration = power
        .saturating_mul(2)
        .saturating_add((i32::from(skill.level) + 1).saturating_mul(7))
        .max(1);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let poison = if poison_shape == 1 {
        GREEN_POISON
    } else {
        RED_POISON
    };

    apply_monster_poison(
        world,
        target_entity,
        poison,
        if poison == GREEN_POISON {
            green_damage
        } else {
            0
        },
        due_tick,
        u64::try_from(duration)
            .unwrap_or_default()
            .saturating_mul(2),
    );
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectPoisoned {
            object_id: context.target_id,
            poison,
        },
    );

    vec![delete_packet]
}

fn apply_crystal_poison_cloud_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    if !has_equipped_crystal_amulet(world, 0) || !has_equipped_crystal_poison(world, Some(1)) {
        return Vec::new();
    }
    let Some(amulet_delete_packet) = consume_equipped_crystal_amulet(world, 0, 5) else {
        return Vec::new();
    };
    let Some((poison_delete_packet, poison_shape)) =
        consume_equipped_crystal_poison(world, Some(1), 5)
    else {
        return Vec::new();
    };
    let target = context
        .map(|context| context.target.clone())
        .or_else(|| entity_position(world, player))
        .unwrap_or(Point { x: 0, y: 0 });
    let locations = square_locations(&target, 1);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let object_id = allocate_runtime_monster_object_id(world);
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id,
                location: target,
                spell: Spell::PoisonCloud,
                direction: MirDirection::Up,
                param: false,
            },
        },
    );
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_ground_spell_actions
        .push(PendingGroundSpellAction {
            spell: Spell::PoisonCloud,
            caster_object_id: player_object_id,
            damage: damage.saturating_add(i32::from(poison_shape)),
            locations,
            next_tick: due_tick,
            expires_at_tick: due_tick.saturating_add(combat_delay_ticks(6_000)),
            tick_interval: combat_delay_ticks(1_000),
        });

    vec![amulet_delete_packet, poison_delete_packet]
}

fn apply_crystal_plague_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    const GREEN_POISON: u16 = 1;
    const RED_POISON: u16 = 2;
    const SLOW_POISON: u16 = 4;
    const FROZEN_POISON: u16 = 8;

    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(amulet_delete_packet) = consume_equipped_crystal_amulet(world, 0, 1) else {
        return Vec::new();
    };
    let poison_delete_packet = consume_equipped_crystal_poison(world, None, 1);
    let held_poison = poison_delete_packet
        .as_ref()
        .map(|(_, poison_shape)| {
            if *poison_shape == 1 {
                GREEN_POISON
            } else {
                RED_POISON
            }
        })
        .unwrap_or(0);
    let target = context
        .map(|context| context.target.clone())
        .or_else(|| entity_position(world, player))
        .unwrap_or(Point { x: 0, y: 0 });
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let spell_power = crystal_spell_damage(world, tick, magic, skill.level);
    let direct_damage = current_player_crystal_stat(world, CRYSTAL_STAT_MAX_SC)
        .saturating_mul(2)
        .max(spell_power);
    let duration_ticks = combat_delay_ticks(
        u64::try_from(
            (2_i32.saturating_mul(i32::from(skill.level) + 1))
                .saturating_add(spell_power.saturating_div(10))
                .max(1),
        )
        .unwrap_or(1)
        .saturating_mul(1_000),
    );
    let targets = hostile_monsters_in_square(world, &target, 1);
    for target_entity in targets {
        schedule_damage_to_hostile_monster_entity(
            world,
            target_entity,
            due_tick,
            player_object_id,
            direct_damage,
        );
        let Some(object_id) = entity_object_id(world, target_entity) else {
            continue;
        };
        let roll = (u64::from(object_id)
            .saturating_add(tick)
            .saturating_add(u64::from(skill.level)))
            % 15;
        let poison = match roll {
            0..=2 => SLOW_POISON,
            3..=4 => FROZEN_POISON,
            5..=9 if held_poison != 0 => held_poison,
            _ => 0,
        };
        if poison != 0 {
            let green_damage = if poison == GREEN_POISON {
                spell_power
                    .saturating_add((i32::from(skill.level) + 1).saturating_mul(2))
                    .max(1)
            } else {
                0
            };
            apply_monster_poison(
                world,
                target_entity,
                poison,
                green_damage,
                due_tick,
                duration_ticks,
            );
            queue_due_packet(
                world,
                due_tick,
                ServerPacket::ObjectPoisoned { object_id, poison },
            );
        }
    }

    let mut packets = vec![amulet_delete_packet];
    if let Some((delete_packet, _)) = poison_delete_packet {
        packets.push(delete_packet);
    }
    packets
}

fn apply_crystal_purification_spell(
    world: &mut World,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    if !context_targets_current_player(world, context) {
        return Vec::new();
    }
    let roll = deterministic_roll(
        tick,
        usize::try_from(player_object_id).unwrap_or_default(),
        503 + usize::from(skill.level),
        4,
    );
    if roll > u64::from(skill.level) {
        return Vec::new();
    }

    let removed = {
        let mut buffs = world.resource_mut::<super::resources::BuffResource>();
        let removed = buffs
            .buffs
            .iter()
            .filter(|buff| buff.key == "curse")
            .filter_map(|buff| crystal_buff_type_for_key(&buff.key))
            .collect::<Vec<_>>();
        buffs.buffs.retain(|buff| buff.key != "curse");
        removed
    };
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    for buff_type in removed {
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::RemoveBuff {
                buff_type,
                object_id: player_object_id,
            },
        );
    }

    Vec::new()
}

fn apply_crystal_revelation_spell(
    world: &mut World,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let roll = deterministic_roll(
        tick,
        usize::try_from(context.target_id).unwrap_or_default(),
        509 + usize::from(skill.level),
        4,
    );
    if roll > u64::from(skill.level) {
        return Vec::new();
    }

    let duration = crystal_magic_damage(magic, skill.level)
        .max(1)
        .min(i32::from(u8::MAX)) as u8;
    if let Some(info) = object_health_info_for_entity(world, target_entity, duration) {
        queue_due_packet(
            world,
            queued_before_world_tick_due_tick(tick, combat_delay_ticks(500)),
            ServerPacket::ObjectHealth { info },
        );
    }
    Vec::new()
}

fn apply_crystal_binding_shot(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let target = world.entity(target_entity);
    let Some(center_agent) = target.get::<MonsterAgent>() else {
        return Vec::new();
    };
    if center_agent.dead {
        return Vec::new();
    }
    let Some(center_object_id) = target.get::<super::components::ObjectId>().map(|id| id.0) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(center_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };

    let duration_ms = (u64::from(skill.level).saturating_mul(5) + 10).saturating_mul(1_000);
    let duration_ticks = combat_delay_ticks(duration_ms);
    let release_tick = tick.saturating_add(duration_ticks);
    let affected = {
        #[allow(deprecated)]
        world
            .iter_entities()
            .filter_map(|entity| {
                let agent = entity.get::<MonsterAgent>()?;
                if agent.dead {
                    return None;
                }
                let position = entity.get::<Position>()?;
                (tile_distance(&position.0, &center_position) <= 1).then_some(entity.id())
            })
            .collect::<Vec<_>>()
    };
    for entity in affected {
        if let Some(mut agent) = world.entity_mut(entity).get_mut::<MonsterAgent>() {
            agent.tracking_player = false;
            agent.next_move_tick = agent.next_move_tick.max(release_tick);
            agent.next_attack_tick = agent.next_attack_tick.max(release_tick);
        }
    }

    let visual_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &center_position),
    );
    queue_due_packet(
        world,
        visual_tick,
        ServerPacket::SetBindingShot {
            object_id: center_object_id,
            enabled: true,
            value: i64::try_from(duration_ms).unwrap_or(i64::MAX),
        },
    );
    Vec::new()
}

fn apply_crystal_trap_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if agent.dead || !agent.hostile_to_player {
        return Vec::new();
    }
    let player_level = crystal_player_level(world, player);
    if monster_level(world, target_entity) >= player_level.saturating_add(2) {
        return Vec::new();
    }
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };
    let duration_ms = 60_000_u64;
    let release_tick = tick.saturating_add(combat_delay_ticks(duration_ms));
    if let Some(mut agent) = world.entity_mut(target_entity).get_mut::<MonsterAgent>() {
        agent.tracking_player = false;
        agent.next_move_tick = agent.next_move_tick.max(release_tick);
        agent.next_attack_tick = agent.next_attack_tick.max(release_tick);
    }
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let object_id = allocate_runtime_monster_object_id(world);
    let direction = context.direction;
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id,
                location: target_position,
                spell: Spell::Trap,
                direction,
                param: skill.level > 0,
            },
        },
    );
    Vec::new()
}

fn apply_crystal_explosive_trap_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    _context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = entity_facing(world, player).unwrap_or(MirDirection::Down);
    let front = offset_point(&player_position, direction, 1);
    if is_blocked_tile(world, &front) {
        return Vec::new();
    }

    let side_direction = rotated_direction(direction, 2);
    let mut trap_locations = vec![front.clone()];
    for side_offset in [-1, 1] {
        let location = offset_point(&front, side_direction, side_offset);
        if !is_blocked_tile(world, &location) {
            trap_locations.push(location);
        }
    }

    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    for location in &trap_locations {
        let object_id = allocate_runtime_monster_object_id(world);
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id,
                    location: location.clone(),
                    spell: Spell::ExplosiveTrap,
                    direction,
                    param: false,
                },
            },
        );
    }
    let trap_damage = crystal_spell_damage(world, tick, magic, skill.level);
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_ground_spell_actions
        .push(PendingGroundSpellAction {
            spell: Spell::ExplosiveTrap,
            caster_object_id: player_object_id,
            damage: trap_damage,
            locations: trap_locations,
            next_tick: due_tick,
            expires_at_tick: due_tick.saturating_add(combat_delay_ticks(10_000)),
            tick_interval: combat_delay_ticks(500),
        });

    Vec::new()
}

fn apply_crystal_poison_sword_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    tick: u64,
) -> Vec<ServerPacket> {
    const GREEN_POISON: u16 = 1;

    let Some((delete_packet, _)) = consume_equipped_crystal_poison(world, None, 1) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = entity_facing(world, player).unwrap_or(MirDirection::Down);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let power = crystal_spell_damage(world, tick, magic, skill.level);
    let poison_attack = current_player_crystal_stat(world, CRYSTAL_STAT_POISON_ATTACK).max(0);
    let poison_attack_bonus = if poison_attack > 0 {
        deterministic_roll(
            tick,
            usize::try_from(current_player_object_id(world).unwrap_or_default())
                .unwrap_or_default(),
            907 + usize::from(skill.level),
            u64::try_from(poison_attack).unwrap_or_default(),
        ) as i32
    } else {
        0
    };
    let green_damage = power
        .saturating_div(10)
        .saturating_add(i32::from(skill.level) + 1)
        .saturating_add(poison_attack_bonus)
        .max(1);
    let duration_ticks = combat_delay_ticks(
        u64::try_from(
            3_i32
                .saturating_add(power.saturating_div(10))
                .saturating_add(i32::from(skill.level).saturating_mul(3))
                .max(1),
        )
        .unwrap_or(1)
        .saturating_mul(1_000),
    );

    for offset in -1..=3 {
        let hit_point = offset_point(&player_position, rotated_direction(direction, offset), 1);
        for target_entity in hostile_monsters_at_position(world, &hit_point) {
            let Some(object_id) = entity_object_id(world, target_entity) else {
                continue;
            };
            apply_monster_poison(
                world,
                target_entity,
                GREEN_POISON,
                green_damage,
                due_tick,
                duration_ticks,
            );
            queue_due_packet(
                world,
                due_tick,
                ServerPacket::ObjectPoisoned {
                    object_id,
                    poison: GREEN_POISON,
                },
            );
        }
    }

    vec![delete_packet]
}

fn apply_crystal_projectile_damage_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
    consume_amulet: bool,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };

    let mut packets = Vec::new();
    if consume_amulet {
        let Some(delete_packet) = consume_equipped_crystal_amulet(world, 0, 1) else {
            return Vec::new();
        };
        packets.push(delete_packet);
    }

    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    schedule_damage_to_monster(
        world,
        due_tick,
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: context.target_id,
            name: target_name,
        }),
    );
    packets
}

fn apply_crystal_healing_spell(
    world: &mut World,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    if context
        .map(|context| context.target_id != 0 && context.target_id != player_object_id)
        .unwrap_or(false)
    {
        return Vec::new();
    }
    let current_hp = world
        .entity(player)
        .get::<PlayerVitals>()
        .map(|vitals| vitals.hp)
        .unwrap_or(0);
    let max_hp = world
        .entity(player)
        .get::<PlayerVitals>()
        .map(|vitals| vitals.max_hp)
        .unwrap_or(0);
    if current_hp >= max_hp {
        return Vec::new();
    }

    let level_bonus = i32::from(crystal_player_level(world, player));
    let heal = crystal_magic_get_damage(
        magic,
        skill.level,
        crystal_spell_attack_power(world, tick, magic).saturating_mul(2),
    )
    .saturating_add(level_bonus)
    .max(1);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    schedule_heal_to_player(world, due_tick, heal);
    let effect_packet = ServerPacket::ObjectEffect {
        info: ObjectEffectInfo {
            object_id: player_object_id,
            effect: CRYSTAL_SPELL_EFFECT_HEALING,
            effect_type: 0,
            delay_time: 0,
            time: 0,
        },
    };
    queue_due_packet(world, due_tick, effect_packet.clone());
    vec![effect_packet]
}

fn apply_crystal_taoist_summon_spell(
    world: &mut World,
    player: Entity,
    _skill: &SkillState,
    magic: &CrystalMagicTemplate,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = entity_facing(world, player).unwrap_or(MirDirection::Down);
    let (monster_name, amulet_count, delay_ms, max_minions) = match magic.spell.as_str() {
        "SummonSkeleton" => ("BoneFamiliar", 1, 500, 2),
        "SummonHolyDeva" => ("HolyDeva", 2, 1_500, 2),
        _ => return Vec::new(),
    };
    if recall_existing_summon(
        world,
        player_object_id,
        monster_name,
        &player_position,
        true,
    ) {
        return Vec::new();
    }
    if active_summoned_monster_count(world, player_object_id) >= max_minions {
        return Vec::new();
    }
    let Some(delete_packet) = consume_equipped_crystal_amulet(world, 0, amulet_count) else {
        return Vec::new();
    };
    let Some(template) = crystal_dynamic_monster_template(monster_name) else {
        return Vec::new();
    };
    let spawn_position =
        summon_spawn_position_near(world, &player_position, direction, 1, Some(player));
    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: tick + combat_delay_ticks(delay_ms),
            summoner_entity: player,
            template: CrystalRespawnTemplate {
                location: spawn_position,
                ..template
            },
            target_entity: None,
            summon_metadata: Some(SummonedMonster {
                summoner_object_id: player_object_id,
                visible_extra: true,
                expire_tick: None,
                require_summoner_within: Some(15),
                despawn_tick_after_death: None,
                totem_master_object_id: None,
                max_minions: Some(max_minions),
            }),
            hostile_to_player_override: Some(false),
        },
    );
    vec![delete_packet]
}

fn apply_crystal_blade_avalanche_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    world
        .entity_mut(player)
        .insert(super::components::Facing(direction));
    let damage = crystal_spell_damage_with_crit(
        world,
        tick,
        magic,
        skill.level,
        crystal_luck_crit_chance(world),
    );
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));

    for lane in [-1, 0, 1] {
        let start = offset_point(&player_position, rotated_direction(direction, lane), 1);
        for row in 0..3 {
            let hit_point = offset_point(&start, direction, row);
            let row_damage = if row <= 1 {
                damage
            } else {
                damage.saturating_mul(3).saturating_div(5).max(1)
            };
            for target_entity in hostile_monsters_at_position(world, &hit_point) {
                schedule_damage_to_hostile_monster_entity(
                    world,
                    target_entity,
                    due_tick,
                    player_object_id,
                    row_damage,
                );
            }
        }
    }
    Vec::new()
}

fn apply_crystal_crescent_slash_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    world
        .entity_mut(player)
        .insert(super::components::Facing(direction));
    let back = rotated_direction(direction, 4);
    let pre_back = rotated_direction(back, -1);
    let next_back = rotated_direction(back, 1);
    let damage = crystal_spell_damage_with_crit(
        world,
        tick,
        magic,
        skill.level,
        current_player_crystal_stat(world, CRYSTAL_STAT_ACCURACY),
    );
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));

    for offset in -4..=3 {
        let hit_direction = rotated_direction(direction, offset);
        if hit_direction == back || hit_direction == pre_back || hit_direction == next_back {
            continue;
        }
        let hit_point = offset_point(&player_position, hit_direction, 1);
        for target_entity in hostile_monsters_at_position(world, &hit_point) {
            schedule_damage_to_hostile_monster_entity(
                world,
                target_entity,
                due_tick,
                player_object_id,
                damage,
            );
        }
    }
    Vec::new()
}

fn apply_crystal_half_moon_family_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    world
        .entity_mut(player)
        .insert(super::components::Facing(direction));
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(300));
    let full_ring = magic.spell == "CrossHalfMoon";
    let offsets: &[i32] = if full_ring {
        &[-4, -3, -2, -1, 1, 2, 3]
    } else {
        &[-1, 1, 2, 3]
    };
    for offset in offsets {
        let hit_point = offset_point(&player_position, rotated_direction(direction, *offset), 1);
        for target_entity in hostile_monsters_at_position(world, &hit_point) {
            schedule_damage_to_hostile_monster_entity(
                world,
                target_entity,
                due_tick,
                player_object_id,
                damage,
            );
        }
    }
    Vec::new()
}

fn apply_crystal_double_slash_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some((player_object_id, target_entity, target_id)) =
        crystal_hostile_target_for_context(world, context)
    else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| {
            entity_position(world, player)
                .and_then(|origin| direction_toward(&origin, &target_position))
        })
        .unwrap_or(MirDirection::Down);
    world
        .entity_mut(player)
        .insert(super::components::Facing(direction));
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    for delay_ms in [300, 400] {
        let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(delay_ms));
        let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
        schedule_damage_to_monster(
            world,
            due_tick,
            player_object_id,
            target_entity,
            damage,
            Some(target_name.clone()),
            Some(PendingMonsterDefeatAction {
                object_id: target_id,
                name: target_name,
            }),
        );
    }
    Vec::new()
}

fn apply_crystal_twin_drake_blade_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some((player_object_id, target_entity, target_id)) =
        crystal_hostile_target_for_context(world, context)
    else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| {
            entity_position(world, player)
                .and_then(|origin| direction_toward(&origin, &target_position))
        })
        .unwrap_or(MirDirection::Down);
    world
        .entity_mut(player)
        .insert(super::components::Facing(direction));
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    schedule_damage_to_monster(
        world,
        queued_before_world_tick_due_tick(tick, combat_delay_ticks(300)),
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: target_id,
            name: target_name.clone(),
        }),
    );
    schedule_damage_to_monster(
        world,
        queued_before_world_tick_due_tick(tick, combat_delay_ticks(400)),
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: target_id,
            name: target_name,
        }),
    );
    let stun_due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(400));
    let duration_ticks = combat_delay_ticks(
        u64::from(skill.level)
            .saturating_add(2)
            .saturating_mul(1_000),
    );
    apply_monster_poison(
        world,
        target_entity,
        CRYSTAL_POISON_STUN,
        0,
        stun_due_tick,
        duration_ticks,
    );
    queue_due_packet(
        world,
        stun_due_tick,
        ServerPacket::ObjectEffect {
            info: ObjectEffectInfo {
                object_id: target_id,
                effect: CRYSTAL_SPELL_EFFECT_TWIN_DRAKE_BLADE,
                effect_type: 0,
                delay_time: 0,
                time: 0,
            },
        },
    );
    queue_due_packet(
        world,
        stun_due_tick,
        ServerPacket::ObjectPoisoned {
            object_id: target_id,
            poison: CRYSTAL_POISON_STUN,
        },
    );
    Vec::new()
}

fn apply_crystal_heavenly_sword_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = context
        .map(|context| context.direction)
        .or_else(|| entity_facing(world, player))
        .unwrap_or(MirDirection::Down);
    world
        .entity_mut(player)
        .insert(super::components::Facing(direction));
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let mut hit_point = player_position;
    for _ in 0..3 {
        hit_point = offset_point(&hit_point, direction, 1);
        for target_entity in hostile_monsters_at_position(world, &hit_point) {
            schedule_damage_to_hostile_monster_entity(
                world,
                target_entity,
                due_tick,
                player_object_id,
                damage,
            );
        }
    }
    Vec::new()
}

fn apply_crystal_entrapment_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    const CRYSTAL_POISON_PARALYSIS: u16 = 256;

    let Some(context) = context else {
        return Vec::new();
    };
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };
    if tile_distance(&player_position, &target_position) > 7 {
        return Vec::new();
    }
    if monster_level(world, target_entity) >= crystal_player_level(world, player).saturating_add(5)
    {
        return Vec::new();
    }
    let pull_direction = rotated_direction(context.direction, 4);
    let pull_distance = (tile_distance(&player_position, &target_position) - 2).max(0);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let duration_ticks = combat_delay_ticks(
        u64::from(skill.level)
            .saturating_add(1)
            .saturating_mul(1_000),
    );
    apply_monster_poison(
        world,
        target_entity,
        CRYSTAL_POISON_PARALYSIS,
        0,
        due_tick,
        duration_ticks,
    );
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectEffect {
            info: ObjectEffectInfo {
                object_id: context.target_id,
                effect: CRYSTAL_SPELL_EFFECT_ENTRAPMENT,
                effect_type: 0,
                delay_time: 0,
                time: 0,
            },
        },
    );
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectPoisoned {
            object_id: context.target_id,
            poison: CRYSTAL_POISON_PARALYSIS,
        },
    );
    if pull_distance > 0 {
        let mut packets = Vec::new();
        let pushed = push_monster_for_crystal_repulsion(
            world,
            target_entity,
            context.target_id,
            pull_direction,
            pull_distance,
            &mut packets,
        );
        if pushed > 0 {
            for packet in packets {
                queue_due_packet(world, due_tick, packet);
            }
        }
    }
    Vec::new()
}

fn crystal_hostile_target_for_context(
    world: &World,
    context: Option<&SkillCastContext>,
) -> Option<(u32, Entity, u32)> {
    let context = context?;
    if context.target_id == 0 {
        return None;
    }
    let target_entity = entity_by_object_id(world, context.target_id)?;
    let agent = world.entity(target_entity).get::<MonsterAgent>()?;
    if agent.dead || !agent.hostile_to_player {
        return None;
    }
    let player_object_id = current_player_object_id(world)?;
    Some((player_object_id, target_entity, context.target_id))
}

fn apply_crystal_mirroring_spell(
    world: &mut World,
    player: Entity,
    _skill: &SkillState,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let existing = {
        #[allow(deprecated)]
        world.iter_entities().find_map(|entity| {
            let summoned = entity.get::<SummonedMonster>()?;
            if summoned.summoner_object_id != player_object_id {
                return None;
            }
            let agent = entity.get::<MonsterAgent>()?;
            if agent.dead {
                return None;
            }
            let name = entity.get::<DisplayName>()?;
            (name.value == "Clone").then_some(entity.id())
        })
    };
    if let Some(existing) = existing {
        let mut packets = Vec::new();
        if let Some(mut vitals) = world.entity_mut(existing).get_mut::<MonsterVitals>() {
            vitals.hp = 0;
        }
        if let Some(mut agent) = world.entity_mut(existing).get_mut::<MonsterAgent>() {
            agent.dead = true;
        }
        if let Some(info) = object_died_info_for_entity(world, existing, 0) {
            packets.push(ServerPacket::ObjectDied { info });
        }
        return packets;
    }

    let Some(template) = crystal_dynamic_monster_template("Clone") else {
        return Vec::new();
    };
    let direction = entity_facing(world, player).unwrap_or(MirDirection::Down);
    let spawn_position =
        summon_spawn_position_near(world, &player_position, direction, 1, Some(player));
    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: tick + combat_delay_ticks(500),
            summoner_entity: player,
            template: CrystalRespawnTemplate {
                location: spawn_position,
                ..template
            },
            target_entity: None,
            summon_metadata: Some(SummonedMonster {
                summoner_object_id: player_object_id,
                visible_extra: true,
                expire_tick: None,
                require_summoner_within: Some(15),
                despawn_tick_after_death: None,
                totem_master_object_id: None,
                max_minions: Some(1),
            }),
            hostile_to_player_override: Some(false),
        },
    );
    Vec::new()
}

fn apply_crystal_moon_mist_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    tick: u64,
) -> Vec<ServerPacket> {
    if world
        .resource::<super::resources::BuffResource>()
        .buffs
        .iter()
        .any(|buff| buff.key == "moon-light")
    {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let duration_ms = u64::try_from(
        current_player_crystal_stat(world, CRYSTAL_STAT_MAX_AC)
            .max(current_player_crystal_stat(world, CRYSTAL_STAT_MIN_AC))
            .saturating_add((i32::from(skill.level) + 1).saturating_mul(5))
            .max(1),
    )
    .unwrap_or(1)
    .saturating_mul(500);
    let buff = BuffState {
        key: "moon-light".to_string(),
        name: "Moon Light".to_string(),
        description: "Crystal moon mist stealth is active.".to_string(),
        expires_at_tick: tick + combat_delay_ticks(duration_ms),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: Vec::new(),
    };
    apply_or_refresh_buff(world, buff.clone());
    let mut packets: Vec<ServerPacket> = client_buff_packet_for_state(world, &buff)
        .into_iter()
        .collect();
    packets.push(ServerPacket::ObjectHidden {
        object_id: player_object_id,
        hidden: true,
    });
    packets.push(ServerPacket::ObjectEffect {
        info: ObjectEffectInfo {
            object_id: player_object_id,
            effect: CRYSTAL_SPELL_EFFECT_MOON_MIST,
            effect_type: 0,
            delay_time: 0,
            time: 0,
        },
    });

    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    for target_entity in hostile_monsters_in_square(world, &player_position, 2) {
        schedule_damage_to_hostile_monster_entity(
            world,
            target_entity,
            due_tick,
            player_object_id,
            damage,
        );
        if monster_is_undead(world, target_entity) {
            if let Some(object_id) = entity_object_id(world, target_entity) {
                apply_monster_poison(
                    world,
                    target_entity,
                    CRYSTAL_POISON_STUN,
                    0,
                    due_tick,
                    combat_delay_ticks((u64::from(skill.level) + 2).saturating_mul(1_000)),
                );
                queue_due_packet(
                    world,
                    due_tick,
                    ServerPacket::ObjectPoisoned {
                        object_id,
                        poison: CRYSTAL_POISON_STUN,
                    },
                );
            }
        }
    }

    packets
}

fn apply_crystal_cat_tongue_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    schedule_damage_to_hostile_monster_entity(
        world,
        target_entity,
        due_tick,
        player_object_id,
        damage,
    );
    let roll = deterministic_roll(
        tick,
        usize::try_from(context.target_id).unwrap_or_default(),
        1117 + usize::from(skill.level),
        10,
    );
    let poison = match roll {
        0..=4 => CRYSTAL_POISON_STUN,
        5..=7 => CRYSTAL_POISON_SLOW,
        _ => CRYSTAL_POISON_FROZEN,
    };
    apply_crystal_control_poison(
        world,
        target_entity,
        context.target_id,
        poison,
        skill.level,
        due_tick,
    );
    Vec::new()
}

fn apply_crystal_hallucination_spell(
    world: &mut World,
    _player: Entity,
    skill: &SkillState,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if agent.dead || !agent.hostile_to_player {
        return Vec::new();
    }
    let Some(delete_packet) = consume_equipped_crystal_amulet(world, 0, 1) else {
        return Vec::new();
    };
    let release_tick = tick.saturating_add(combat_delay_ticks(
        (10_u64 + u64::from(skill.level).saturating_mul(5)).saturating_mul(1_000),
    ));
    if let Some(mut agent) = world.entity_mut(target_entity).get_mut::<MonsterAgent>() {
        agent.tracking_player = false;
        agent.next_move_tick = tick;
        agent.next_attack_tick = release_tick;
    }
    vec![delete_packet]
}

fn apply_crystal_one_with_nature_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let (has_vampire, has_poison) = {
        let buffs = world.resource::<super::resources::BuffResource>();
        (
            buffs.buffs.iter().any(|buff| buff.key == "vampire-shot"),
            buffs.buffs.iter().any(|buff| buff.key == "poison-shot"),
        )
    };
    let damage = crystal_spell_damage(world, tick, magic, skill.level);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let targets = hostile_monsters_in_square(world, &player_position, 2);
    for target_entity in targets {
        schedule_damage_to_hostile_monster_entity(
            world,
            target_entity,
            due_tick,
            player_object_id,
            damage,
        );
        if has_poison {
            if let Some(object_id) = entity_object_id(world, target_entity) {
                apply_crystal_arrow_green_poison(
                    world,
                    target_entity,
                    object_id,
                    damage,
                    skill.level,
                    tick,
                    due_tick,
                    1211,
                );
            }
        }
    }
    if has_vampire {
        schedule_heal_to_player(
            world,
            due_tick,
            damage
                .saturating_mul(i32::from(skill.level) + 1)
                .saturating_div(4)
                .max(1),
        );
    }
    let mut packets = Vec::new();
    for (_, buff_type) in consume_crystal_arrow_buffs(world) {
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::RemoveBuff {
                buff_type,
                object_id: player_object_id,
            },
        );
        packets.push(ServerPacket::RemoveBuff {
            buff_type,
            object_id: player_object_id,
        });
    }
    packets
}

fn apply_crystal_reincarnation_spell(
    world: &mut World,
    player: Entity,
    tick: u64,
) -> Vec<ServerPacket> {
    if current_map_disallows_reincarnation(world) {
        return Vec::new();
    }
    let Some(delete_packet) = consume_equipped_crystal_amulet(world, 3, 1) else {
        return Vec::new();
    };
    let Some(player_object_id) = current_player_object_id(world) else {
        return vec![delete_packet];
    };
    let Some(player_position) = entity_position(world, player) else {
        return vec![delete_packet];
    };
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let spell_object_id = allocate_runtime_monster_object_id(world);
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id: spell_object_id,
                location: player_position,
                spell: Spell::Reincarnation,
                direction: entity_facing(world, player).unwrap_or(MirDirection::Up),
                param: true,
            },
        },
    );
    let mut packets = vec![delete_packet];
    packets.push(ServerPacket::RequestReincarnation);
    queue_due_packet(
        world,
        due_tick.saturating_add(combat_delay_ticks(6_000)),
        ServerPacket::CancelReincarnation,
    );
    let _ = player_object_id;
    packets
}

fn apply_crystal_portal_spell(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let target = context
        .map(|context| context.target.clone())
        .unwrap_or(player_position);
    if !can_occupy(world, target.clone(), Some(player)) {
        return Vec::new();
    }
    let duration = 30_u64 + u64::from(skill.level).saturating_mul(30);
    let pass_through_count = i32::from(skill.level)
        .saturating_mul(2)
        .saturating_sub(1)
        .max(1);
    let due_tick = queued_before_world_tick_due_tick(tick, combat_delay_ticks(500));
    let spell_object_id = allocate_runtime_monster_object_id(world);
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id: spell_object_id,
                location: target,
                spell: Spell::Portal,
                direction: entity_facing(world, player).unwrap_or(MirDirection::Up),
                param: pass_through_count > crystal_magic_damage(magic, skill.level),
            },
        },
    );
    let _ = duration;
    Vec::new()
}

const CRYSTAL_POISON_SLOW: u16 = 4;
const CRYSTAL_POISON_FROZEN: u16 = 8;
const CRYSTAL_POISON_STUN: u16 = 16;

fn apply_crystal_control_poison(
    world: &mut World,
    target_entity: Entity,
    object_id: u32,
    poison: u16,
    skill_level: u8,
    due_tick: u64,
) {
    let duration_ms = (u64::from(skill_level) + 1).saturating_mul(3_000);
    apply_monster_poison(
        world,
        target_entity,
        poison,
        0,
        due_tick,
        combat_delay_ticks(duration_ms),
    );
    let release_tick = due_tick.saturating_add(combat_delay_ticks(duration_ms));
    if let Some(mut agent) = world.entity_mut(target_entity).get_mut::<MonsterAgent>() {
        if poison != CRYSTAL_POISON_SLOW {
            agent.next_move_tick = agent.next_move_tick.max(release_tick);
            agent.next_attack_tick = agent.next_attack_tick.max(release_tick);
        } else {
            agent.next_move_tick = agent.next_move_tick.max(due_tick.saturating_add(1));
        }
    }
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectPoisoned { object_id, poison },
    );
}

fn apply_crystal_special_arrow_shot(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };

    let range = (player_position.x - target_position.x)
        .abs()
        .max((player_position.y - target_position.y).abs());
    let damage = crystal_archer_state_damage(
        world,
        crystal_spell_range_damage(world, tick, magic, skill.level, range),
    );
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    schedule_damage_to_monster(
        world,
        due_tick,
        player_object_id,
        target_entity,
        damage,
        Some(target_name.clone()),
        Some(PendingMonsterDefeatAction {
            object_id: context.target_id,
            name: target_name,
        }),
    );

    match magic.spell.as_str() {
        "VampireShot" => {
            schedule_heal_to_player(
                world,
                due_tick,
                damage.saturating_mul(i32::from(skill.level) + 1) / 4,
            );
            apply_crystal_arrow_marker_buff(
                world,
                player_object_id,
                skill,
                tick,
                "vampire-shot",
                "poison-shot",
                133,
                "Vampire Shot",
            )
        }
        "PoisonShot" => {
            apply_crystal_arrow_green_poison(
                world,
                target_entity,
                context.target_id,
                damage,
                skill.level,
                tick,
                due_tick,
                135,
            );
            apply_crystal_arrow_marker_buff(
                world,
                player_object_id,
                skill,
                tick,
                "poison-shot",
                "vampire-shot",
                135,
                "Poison Shot",
            )
        }
        "CrippleShot" => apply_crystal_cripple_arrow_followup(
            world,
            skill,
            &target_position,
            damage,
            tick,
            due_tick,
        ),
        _ => Vec::new(),
    }
}

fn apply_crystal_archer_basic_shot(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
    magic: &CrystalMagicTemplate,
    context: Option<&SkillCastContext>,
    tick: u64,
) -> Vec<ServerPacket> {
    let Some(context) = context else {
        return Vec::new();
    };
    if context.target_id == 0 {
        return Vec::new();
    }
    let Some(target_entity) = entity_by_object_id(world, context.target_id) else {
        return Vec::new();
    };
    let Some(target_agent) = world.entity(target_entity).get::<MonsterAgent>() else {
        return Vec::new();
    };
    if target_agent.dead || !target_agent.hostile_to_player {
        return Vec::new();
    }
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return Vec::new();
    };
    let target_name = entity_name(world, target_entity).unwrap_or_else(|| "Target".to_string());
    let due_tick = queued_before_world_tick_due_tick(
        tick,
        ranged_attack_delay_ticks(&player_position, &target_position),
    );
    let range = (player_position.x - target_position.x)
        .abs()
        .max((player_position.y - target_position.y).abs());
    let damage = crystal_archer_state_damage(
        world,
        crystal_spell_range_damage(world, tick, magic, skill.level, range),
    );
    let hit_count = if magic.spell == "DoubleShot" { 2 } else { 1 };
    for hit in 0..hit_count {
        schedule_damage_to_monster(
            world,
            due_tick.saturating_add(hit),
            player_object_id,
            target_entity,
            damage,
            Some(target_name.clone()),
            Some(PendingMonsterDefeatAction {
                object_id: context.target_id,
                name: target_name.clone(),
            }),
        );
    }
    Vec::new()
}

fn crystal_archer_state_damage(world: &World, damage: i32) -> i32 {
    let skills = world.resource::<SkillResource>();
    let mental_state_level = skill_key_for_crystal_spell(Spell::MentalState)
        .and_then(|key| {
            skills
                .skills
                .iter()
                .find(|skill| skill.key == key)
                .map(|skill| i32::from(skill.level))
        })
        .unwrap_or(0);
    let percent = match skills.mental_state {
        1 => 55 + mental_state_level.saturating_mul(5),
        2 => 80,
        _ => 100,
    };
    damage.saturating_mul(percent).saturating_div(100).max(1)
}

fn apply_crystal_arrow_marker_buff(
    world: &mut World,
    player_object_id: u32,
    skill: &SkillState,
    tick: u64,
    buff_key: &'static str,
    other_buff_key: &'static str,
    spell_salt: usize,
    name: &'static str,
) -> Vec<ServerPacket> {
    let has_arrow_buff = world
        .resource::<super::resources::BuffResource>()
        .buffs
        .iter()
        .any(|buff| buff.key == buff_key || buff.key == other_buff_key);
    if has_arrow_buff {
        return Vec::new();
    }

    let roll = deterministic_roll(
        tick,
        usize::try_from(player_object_id).unwrap_or_default(),
        usize::from(skill.level).saturating_add(spell_salt),
        20,
    );
    if roll < 8 {
        return Vec::new();
    }

    let duration_ms = (5_u64 + u64::from(skill.level).saturating_mul(5)).saturating_mul(1_000);
    let buff = BuffState {
        key: buff_key.to_string(),
        name: name.to_string(),
        description: format!("Crystal {name} buff is active."),
        expires_at_tick: tick.saturating_add(combat_delay_ticks(duration_ms)),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: Vec::new(),
    };
    apply_or_refresh_buff(world, buff.clone());
    client_buff_packet_for_state(world, &buff)
        .into_iter()
        .collect()
}

fn square_locations(center: &Point, radius: i32) -> Vec<Point> {
    let mut locations = Vec::new();
    for y in center.y - radius..=center.y + radius {
        for x in center.x - radius..=center.x + radius {
            locations.push(Point { x, y });
        }
    }
    locations
}

fn apply_crystal_cripple_arrow_followup(
    world: &mut World,
    skill: &SkillState,
    target_position: &Point,
    damage: i32,
    tick: u64,
    due_tick: u64,
) -> Vec<ServerPacket> {
    let consumed_buffs = consume_crystal_arrow_buffs(world);
    if consumed_buffs.is_empty() {
        return Vec::new();
    }

    let player_object_id = current_player_object_id(world).unwrap_or_default();
    for (_, buff_type) in &consumed_buffs {
        queue_due_packet(
            world,
            due_tick,
            ServerPacket::RemoveBuff {
                buff_type: *buff_type,
                object_id: player_object_id,
            },
        );
    }

    if consumed_buffs.iter().any(|(key, _)| key == "vampire-shot") {
        schedule_heal_to_player(
            world,
            due_tick,
            damage.saturating_mul(i32::from(skill.level) + 1) / 4,
        );
    }

    if consumed_buffs.iter().any(|(key, _)| key == "poison-shot") {
        let affected = {
            #[allow(deprecated)]
            world
                .iter_entities()
                .filter_map(|entity| {
                    let agent = entity.get::<MonsterAgent>()?;
                    if agent.dead || !agent.hostile_to_player {
                        return None;
                    }
                    let position = entity.get::<Position>()?;
                    (tile_distance(&position.0, target_position) <= 1).then_some(entity.id())
                })
                .collect::<Vec<_>>()
        };
        for (index, entity) in affected.into_iter().enumerate() {
            if let Some(object_id) = entity_object_id(world, entity) {
                apply_crystal_arrow_green_poison(
                    world,
                    entity,
                    object_id,
                    damage,
                    skill.level,
                    tick,
                    due_tick,
                    171 + index,
                );
            }
        }
    }

    Vec::new()
}

fn consume_crystal_arrow_buffs(world: &mut World) -> Vec<(String, u8)> {
    let mut consumed = Vec::new();
    let mut buffs = world.resource_mut::<super::resources::BuffResource>();
    buffs.buffs.retain(|buff| {
        if !matches!(buff.key.as_str(), "vampire-shot" | "poison-shot") {
            return true;
        }
        if let Some(buff_type) = crystal_buff_type_for_key(&buff.key) {
            consumed.push((buff.key.clone(), buff_type));
        }
        false
    });
    consumed
}

fn apply_crystal_arrow_green_poison(
    world: &mut World,
    target_entity: Entity,
    object_id: u32,
    damage: i32,
    skill_level: u8,
    tick: u64,
    due_tick: u64,
    salt: usize,
) {
    const GREEN_POISON: u16 = 1;

    let poison_attack = current_player_crystal_stat(world, CRYSTAL_STAT_POISON_ATTACK).max(0);
    let poison_attack_bonus = if poison_attack > 0 {
        deterministic_roll(
            tick,
            usize::try_from(object_id).unwrap_or_default(),
            salt.saturating_add(usize::from(skill_level)),
            u64::try_from(poison_attack).unwrap_or_default(),
        ) as i32
    } else {
        0
    };
    let green_damage = damage
        .saturating_div(25)
        .saturating_add(i32::from(skill_level) + 1)
        .saturating_add(poison_attack_bonus)
        .max(1);
    let duration = damage
        .saturating_mul(2)
        .saturating_add((i32::from(skill_level) + 1).saturating_mul(7))
        .max(1);

    apply_monster_poison(
        world,
        target_entity,
        GREEN_POISON,
        green_damage,
        due_tick,
        u64::try_from(duration)
            .unwrap_or_default()
            .saturating_mul(2),
    );
    queue_due_packet(
        world,
        due_tick,
        ServerPacket::ObjectPoisoned {
            object_id,
            poison: GREEN_POISON,
        },
    );
}

fn taoist_support_duration_seconds(world: &World, skill: &SkillState) -> i32 {
    let spell_power = current_player_crystal_stat(world, CRYSTAL_STAT_MAX_SC)
        .max(current_player_crystal_stat(world, CRYSTAL_STAT_MIN_SC));
    spell_power
        .saturating_mul(4)
        .saturating_add((i32::from(skill.level) + 1).saturating_mul(50))
        .max(1)
}

fn active_player_level(world: &World) -> i32 {
    world
        .resource::<super::resources::SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| i32::from(character.level))
        .unwrap_or(1)
}

fn active_player_class(world: &World) -> Option<MirClass> {
    world
        .resource::<super::resources::SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.class)
}

fn context_targets_current_player(world: &World, context: Option<&SkillCastContext>) -> bool {
    let Some(context) = context else {
        return true;
    };
    context.target_id == 0
        || current_player_object_id(world).is_some_and(|object_id| object_id == context.target_id)
}

fn gather_crystal_element(world: &mut World, casted: bool) -> Option<ServerPacket> {
    let object_id = current_player_object_id(world)?;
    {
        let mut elemental = world.resource_mut::<ElementalResource>();
        elemental.has_elemental = true;
        elemental.elements_level = CRYSTAL_ORB_EXP[0];
    }
    Some(ServerPacket::SetElemental {
        object_id,
        enabled: true,
        casted,
        value: CRYSTAL_ORB_EXP[0],
        element_type: 1,
        exp_last: crystal_elemental_exp_last(),
    })
}

fn clear_crystal_element(world: &mut World, casted: bool) -> Option<ServerPacket> {
    let object_id = current_player_object_id(world)?;
    {
        let mut elemental = world.resource_mut::<ElementalResource>();
        elemental.has_elemental = false;
        elemental.elements_level = 0;
    }
    Some(ServerPacket::SetElemental {
        object_id,
        enabled: false,
        casted,
        value: 0,
        element_type: 0,
        exp_last: crystal_elemental_exp_last(),
    })
}

const CRYSTAL_ORB_EXP: [u32; 4] = [50, 100, 150, 200];
const CRYSTAL_ORB_DEF: [i32; 4] = [2, 4, 6, 8];
const CRYSTAL_ORB_DMG: [i32; 4] = [4, 8, 12, 16];

fn crystal_elemental_exp_last() -> u32 {
    *CRYSTAL_ORB_EXP.last().unwrap_or(&200)
}

fn crystal_elemental_orb_count(elements_level: u32) -> usize {
    CRYSTAL_ORB_EXP
        .iter()
        .rposition(|threshold| elements_level >= *threshold)
        .map(|index| index + 1)
        .unwrap_or(1)
}

fn crystal_elemental_orb_power(elements_level: u32, defensive: bool) -> i32 {
    let index = crystal_elemental_orb_count(elements_level).saturating_sub(1);
    if defensive {
        CRYSTAL_ORB_DEF[index]
    } else {
        CRYSTAL_ORB_DMG[index]
    }
}

fn consume_equipped_crystal_amulet(
    world: &mut World,
    shape: u16,
    count: u16,
) -> Option<ServerPacket> {
    let mut inventory = world.resource_mut::<super::resources::InventoryResource>();
    let index = inventory.equipment_items.iter().position(|item| {
        item.slot == EquipmentSlot::Amulet && item.shape == Some(shape) && !item.is_broken()
    })?;
    let unique_id = equipment_slot_unique_id(inventory.equipment_items[index].slot).unwrap_or(9);
    inventory.equipment_items.remove(index);
    Some(ServerPacket::DeleteItem { unique_id, count })
}

fn has_equipped_crystal_amulet(world: &World, shape: u16) -> bool {
    let inventory = world.resource::<super::resources::InventoryResource>();
    inventory.equipment_items.iter().any(|item| {
        item.slot == EquipmentSlot::Amulet && item.shape == Some(shape) && !item.is_broken()
    })
}

fn has_equipped_crystal_poison(world: &World, shape: Option<u16>) -> bool {
    let inventory = world.resource::<super::resources::InventoryResource>();
    inventory.equipment_items.iter().any(|item| {
        if item.is_broken() {
            return false;
        }
        let item_shape = item.shape.unwrap_or_default();
        let shape_matches = shape
            .map(|shape| item_shape == shape)
            .unwrap_or_else(|| matches!(item_shape, 1 | 2));
        if !shape_matches {
            return false;
        }
        crystal_item_template_for_dynamic_key(&item.key)
            .map(|template| template.item_type == CRYSTAL_ITEM_TYPE_AMULET)
            .unwrap_or(false)
    })
}

fn consume_equipped_crystal_poison(
    world: &mut World,
    shape: Option<u16>,
    count: u16,
) -> Option<(ServerPacket, u16)> {
    let mut inventory = world.resource_mut::<super::resources::InventoryResource>();
    let index = inventory.equipment_items.iter().position(|item| {
        if item.is_broken() {
            return false;
        }
        let item_shape = item.shape.unwrap_or_default();
        let shape_matches = shape
            .map(|shape| item_shape == shape)
            .unwrap_or_else(|| matches!(item_shape, 1 | 2));
        if !shape_matches {
            return false;
        }
        crystal_item_template_for_dynamic_key(&item.key)
            .map(|template| template.item_type == CRYSTAL_ITEM_TYPE_AMULET)
            .unwrap_or(false)
    })?;
    let slot = inventory.equipment_items[index].slot;
    let poison_shape = inventory.equipment_items[index].shape.unwrap_or_default();
    let unique_id = equipment_slot_unique_id(slot).unwrap_or(9);
    inventory.equipment_items.remove(index);
    Some((ServerPacket::DeleteItem { unique_id, count }, poison_shape))
}

pub(super) fn consume_zone_magic_inventory_components(
    world: &mut World,
    spell: Spell,
) -> Option<Vec<ServerPacket>> {
    match spell {
        Spell::PoisonCloud => {
            if !has_equipped_crystal_amulet(world, 0)
                || !has_equipped_crystal_poison(world, Some(1))
            {
                return None;
            }
            let amulet_delete_packet = consume_equipped_crystal_amulet(world, 0, 5)?;
            let (poison_delete_packet, _) = consume_equipped_crystal_poison(world, Some(1), 5)?;
            Some(vec![amulet_delete_packet, poison_delete_packet])
        }
        Spell::SummonSkeleton => {
            let amulet_delete_packet = consume_equipped_crystal_amulet(world, 0, 1)?;
            Some(vec![amulet_delete_packet])
        }
        Spell::SummonShinsu => {
            // Crystal `SummonShinsu` consumes 5 amulets (GetAmulet(5)/ConsumeItem(item, 5)).
            let amulet_delete_packet = consume_equipped_crystal_amulet(world, 0, 5)?;
            Some(vec![amulet_delete_packet])
        }
        Spell::SummonHolyDeva => {
            let amulet_delete_packet = consume_equipped_crystal_amulet(world, 0, 2)?;
            Some(vec![amulet_delete_packet])
        }
        _ => Some(Vec::new()),
    }
}

/// Crystal `UserMagic.GetDamage(0)` — the spell's intrinsic power (no player
/// attack base). Identical to `crystal_magic_get_damage(magic, level, 0)` by
/// construction: `(int)(GetPower() * GetMultiplier())` (MagicInfo.cs:180,190).
///
/// Previously this truncated `GetPower` via integer division and floored the
/// multiplier at 1.0, diverging from Crystal for spells whose power has a
/// fractional `/4` term or a sub-1.0 multiplier. Delegating keeps the intrinsic
/// helper bit-identical to the canonical formula so the two never drift.
pub(super) fn crystal_magic_damage(magic: &CrystalMagicTemplate, level: u8) -> i32 {
    crystal_magic_get_damage(magic, level, 0)
}

/// Crystal `UserMagic.GetPower()` (MagicInfo.cs:190) —
/// `(int)Math.Round((MPower() / 4F) * (Level + 1) + DefPower())`. `MPower()`/`DefPower()`
/// are uniform rolls (`Random(Base, Base + Bonus)`); we collapse them to their
/// deterministic mean (matching [`crystal_magic_damage`]) so the value stays
/// reproducible across replays. The whole expression is then evaluated in f32 and
/// rounded once, mirroring `Math.Round` (the prior code truncated via integer
/// division, undercounting by up to ~1 power point).
pub(super) fn crystal_magic_get_power(magic: &CrystalMagicTemplate, level: u8) -> i32 {
    let level = f32::from(level) + 1.0;
    let defence_power = f32::from(magic.power_base) + f32::from(magic.power_bonus) / 2.0;
    let magic_power = f32::from(magic.mpower_base) + f32::from(magic.mpower_bonus) / 2.0;
    ((magic_power / 4.0) * level + defence_power).round() as i32
}

/// Crystal `UserMagic.GetMultiplier()` = `MultiplierBase + Level * MultiplierBonus`.
pub(super) fn crystal_magic_multiplier(magic: &CrystalMagicTemplate, level: u8) -> f32 {
    magic.multiplier_base + f32::from(level) * magic.multiplier_bonus
}

/// Crystal `UserMagic.GetDamage(damageBase)` (MagicInfo.cs:180) =
/// `(int)((damageBase + GetPower()) * GetMultiplier())`.
///
/// This is the canonical magic damage formula. Unlike the legacy approach of adding the
/// player's attack power *after* the multiplier, Crystal folds `damageBase` (the player's
/// MC/SC/DC attack roll) into the value *before* applying the level multiplier, so high-level
/// spells scale the player's gear contribution too.
///
/// Crystal does NOT floor the multiplier or the base: many warrior/assassin skills
/// (HalfMoon 0.3, CrossHalfMoon 0.4, Thrusting 0.25, DoubleSlash/TwinDrakeBlade 0.8,
/// Hemorrhage 0.2) carry a `MultiplierBase < 1`, so flooring the multiplier at `1.0`
/// would inflate their damage several-fold. The cast `(int)(...)` truncates toward
/// zero, so we truncate here rather than round.
pub(super) fn crystal_magic_get_damage(
    magic: &CrystalMagicTemplate,
    level: u8,
    damage_base: i32,
) -> i32 {
    let power = crystal_magic_get_power(magic, level);
    let multiplier = crystal_magic_multiplier(magic, level);
    let base = damage_base.saturating_add(power);
    ((base as f32) * multiplier).trunc() as i32
}

/// The player stat channel a spell's `damageBase` is rolled from. Wizard and archer spells
/// scale off Magic-Crystal (MC), Taoist spells off Spirit-Crystal (SC), and Warrior/Assassin
/// melee skills off Damage-Crystal (DC), mirroring Crystal's per-spell `GetAttackPower` source.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CrystalDamageChannel {
    Mc,
    Sc,
    Dc,
}

pub(super) fn crystal_spell_damage_channel(spell: &str) -> CrystalDamageChannel {
    match spell {
        // Taoist spells roll Spirit-Crystal (SC).
        "SoulFireBall" | "Healing" | "MassHealing" | "HealingCircle" | "Poisoning"
        | "PoisonCloud" | "Plague" | "Curse" | "TrapHexagon" | "Revelation" => {
            CrystalDamageChannel::Sc
        }
        // Warrior + Assassin melee skills roll Damage-Crystal (DC).
        "Slaying" | "DoubleSlash" | "Thrusting" | "HalfMoon" | "CrossHalfMoon"
        | "TwinDrakeBlade" | "FlamingSword" | "BladeAvalanche" | "SlashingBurst"
        | "CrescentSlash" | "HeavenlySword" | "PoisonSword" | "ShoulderDash" | "FlashDash"
        | "MoonMist" | "CatTongue" => CrystalDamageChannel::Dc,
        // Everything else (Wizard + Archer ranged) rolls Magic-Crystal (MC).
        _ => CrystalDamageChannel::Mc,
    }
}

fn crystal_channel_stats(channel: CrystalDamageChannel) -> (u8, u8) {
    match channel {
        CrystalDamageChannel::Mc => (CRYSTAL_STAT_MIN_MC, CRYSTAL_STAT_MAX_MC),
        CrystalDamageChannel::Sc => (CRYSTAL_STAT_MIN_SC, CRYSTAL_STAT_MAX_SC),
        CrystalDamageChannel::Dc => (CRYSTAL_STAT_MIN_DC, CRYSTAL_STAT_MAX_DC),
    }
}

/// The Crystal `DefenceType` a spell's damage rolls against, deciding which target armour stat
/// (AC vs MAC) is subtracted and whether an agility dodge applies. Wizard/Taoist/Archer offensive
/// magic and the MAC-routed warrior skills use MAC; a handful of assassin/warrior skills use AC;
/// the HalfMoon family is pure agility dodge; control/knockback/poison spells bypass armour.
pub(super) fn crystal_spell_defence(spell: &str) -> CrystalDefence {
    match spell {
        "FlamingSword" | "SlashingBurst" | "CrescentSlash" | "FlashDash" | "CatTongue"
        | "MoonMist" => CrystalDefence::Ac,
        "DoubleSlash" | "Thrusting" | "HalfMoon" | "CrossHalfMoon" | "TwinDrakeBlade" => {
            CrystalDefence::Agility
        }
        "TurnUndead" | "ElectricShock" | "Repulsion" | "EnergyRepulsor" | "FireBurst"
        | "Poisoning" | "Plague" | "Curse" | "ShoulderDash" | "PoisonSword" => CrystalDefence::None,
        // Wizard/Taoist/Archer magic + BladeAvalanche/HeavenlySword all roll against MAC.
        _ => CrystalDefence::Mac,
    }
}

fn crystal_spell_salt(spell: &str) -> u64 {
    spell
        .bytes()
        .enumerate()
        .fold(0x517C_C1B7_2722_0A95u64, |acc, (idx, byte)| {
            acc.wrapping_mul(31)
                .wrapping_add(u64::from(byte).wrapping_add(idx as u64))
        })
}

/// Crystal `MapObject.GetAttackPower(min, max)` — deterministic roll in `[min, max]` honoring
/// the Luck stat (positive Luck can force the maximum, negative Luck the minimum). Uses the
/// shared deterministic RNG so a given cast is reproducible across replays.
pub(super) fn crystal_attack_power_roll(
    world: &World,
    tick: u64,
    salt: u64,
    min: i32,
    max: i32,
) -> i32 {
    let min = min.max(0);
    let max = max.max(min);
    let object_id = current_player_object_id(world).unwrap_or_default();
    let luck = current_player_crystal_stat(world, CRYSTAL_STAT_LUCK);
    const MAX_LUCK: u64 = 10; // Crystal Settings.MaxLuck default.
    if luck > 0 {
        let roll = deterministic_roll(tick, object_id as usize, salt as usize, MAX_LUCK) as i32;
        if luck > roll {
            return max;
        }
    } else if luck < 0 {
        let roll = deterministic_roll(
            tick,
            object_id as usize,
            salt.wrapping_add(0xABCD) as usize,
            MAX_LUCK,
        ) as i32;
        if luck < -roll {
            return min;
        }
    }
    if max <= min {
        return min;
    }
    let span = (max - min + 1) as u64;
    min + deterministic_roll(tick, object_id as usize, salt as usize, span) as i32
}

/// Rolls the player's attack power for a spell's damage channel
/// (Crystal `GetAttackPower(MinXX, MaxXX)`).
pub(super) fn crystal_spell_attack_power(
    world: &World,
    tick: u64,
    magic: &CrystalMagicTemplate,
) -> i32 {
    let channel = crystal_spell_damage_channel(&magic.spell);
    let (min_stat, max_stat) = crystal_channel_stats(channel);
    let min = current_player_crystal_stat(world, min_stat);
    let max = current_player_crystal_stat(world, max_stat);
    crystal_attack_power_roll(world, tick, crystal_spell_salt(&magic.spell), min, max)
}

/// Crystal `magic.GetDamage(GetAttackPower(MinXX, MaxXX))` — the full per-cast damage for a
/// spell, including the player's gear-scaled attack power folded inside the level multiplier.
pub(super) fn crystal_spell_damage(
    world: &World,
    tick: u64,
    magic: &CrystalMagicTemplate,
    level: u8,
) -> i32 {
    let base = crystal_spell_attack_power(world, tick, magic);
    crystal_magic_get_damage(magic, level, base).max(1)
}

/// Like [`crystal_spell_damage`] but with a critical-hit chance (out of 100) that *doubles the
/// rolled attack power* before applying `GetDamage` — Crystal's melee skill crit behaviour
/// (`if Random(0,100) <= chance) damageBase += damageBase`). Only the gear-scaled attack power
/// doubles, not the whole damage, matching Crystal's intent.
pub(super) fn crystal_spell_damage_with_crit(
    world: &World,
    tick: u64,
    magic: &CrystalMagicTemplate,
    level: u8,
    crit_chance: i32,
) -> i32 {
    let mut base = crystal_spell_attack_power(world, tick, magic);
    if crit_chance > 0 {
        let object_id = current_player_object_id(world).unwrap_or_default();
        let roll = deterministic_roll(
            tick,
            object_id as usize,
            crystal_spell_salt(&magic.spell).wrapping_add(0x5C0F) as usize,
            100,
        ) as i32;
        if roll < crit_chance {
            base = base.saturating_add(base);
        }
    }
    crystal_magic_get_damage(magic, level, base).max(1)
}

/// Crystal melee crit chance for Luck-scaled skills (`1 + Luck`, e.g. BladeAvalanche, IceThrust).
fn crystal_luck_crit_chance(world: &World) -> i32 {
    1 + current_player_crystal_stat(world, CRYSTAL_STAT_LUCK)
}

/// Crystal `GetRangeAttackPower(min, max, range)` — applies the bow falloff penalty
/// (`min -= floor(min / MaxAttackRange * (MaxAttackRange - range))`) before rolling. Archer
/// shots use this so distant targets take less damage.
/// Crystal `Globals.MaxAttackRange`.
const CRYSTAL_MAX_ATTACK_RANGE: i32 = 9;

/// Crystal `GetRangeAttackPower` bow falloff applied to the minimum roll:
/// `min -= floor(min / MaxAttackRange * (MaxAttackRange - range))`. Closer targets take less
/// (bows reward shooting at maximum range).
fn crystal_range_min_after_falloff(min: i32, range: i32) -> i32 {
    let range = range.clamp(0, CRYSTAL_MAX_ATTACK_RANGE);
    let penalty = (f64::from(min) / f64::from(CRYSTAL_MAX_ATTACK_RANGE)
        * f64::from(CRYSTAL_MAX_ATTACK_RANGE - range))
    .floor() as i32;
    min - penalty
}

pub(super) fn crystal_spell_range_damage(
    world: &World,
    tick: u64,
    magic: &CrystalMagicTemplate,
    level: u8,
    range: i32,
) -> i32 {
    let channel = crystal_spell_damage_channel(&magic.spell);
    let (min_stat, max_stat) = crystal_channel_stats(channel);
    let max = current_player_crystal_stat(world, max_stat);
    let min = crystal_range_min_after_falloff(current_player_crystal_stat(world, min_stat), range);
    let base = crystal_attack_power_roll(world, tick, crystal_spell_salt(&magic.spell), min, max);
    crystal_magic_get_damage(magic, level, base).max(1)
}

pub(super) fn advance_magic_progression(
    world: &mut World,
    index: usize,
    spell: Spell,
    magic: &CrystalMagicTemplate,
    tick: u64,
) -> Vec<ServerPacket> {
    let player_level = world
        .resource::<super::resources::SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.level)
        .unwrap_or(1);
    let object_id = current_player_object_id(world).unwrap_or_default();
    let gain = magic_experience_gain(tick, object_id, spell)
        .saturating_mul(crystal_skill_gain_multiplier(world));
    let mut packets = Vec::new();
    let mut skills = world.resource_mut::<SkillResource>();
    let skill = &mut skills.skills[index];
    let old_level = skill.level;
    let Some(threshold) = magic_experience_threshold(magic, skill.level, player_level) else {
        return packets;
    };

    skill.experience = skill.experience.saturating_add(gain);
    if skill.experience >= threshold {
        skill.level = skill.level.saturating_add(1).min(3);
        skill.experience = if skill.level >= 3 {
            0
        } else {
            skill.experience.saturating_sub(threshold)
        };
        skill.delay_ms = crystal_magic_delay_ms(magic, skill.level);
        skill.cooldown_ticks = crystal_magic_cooldown_ticks(magic, skill.level);
    }

    if old_level != skill.level {
        packets.push(ServerPacket::MagicDelay {
            object_id,
            spell,
            delay: skill.delay_ms,
        });
    }
    packets.push(ServerPacket::MagicLeveled {
        object_id,
        spell,
        level: skill.level,
        experience: skill.experience,
    });
    packets
}

fn magic_experience_gain(tick: u64, object_id: u32, spell: Spell) -> u16 {
    1 + ((tick + u64::from(object_id) + u64::from(spell as u8)) % 3) as u16
}

fn crystal_skill_gain_multiplier(world: &World) -> u16 {
    let multiplier = current_player_required_stat_total(
        world.resource::<InventoryResource>(),
        world.resource::<BuffResource>(),
        CRYSTAL_STAT_SKILL_GAIN_MULTIPLIER,
    );
    u16::try_from(multiplier.clamp(1, i32::from(u8::MAX))).unwrap_or(1)
}

fn magic_experience_threshold(
    magic: &CrystalMagicTemplate,
    level: u8,
    player_level: u16,
) -> Option<u16> {
    match level {
        0 if player_level >= u16::from(magic.level1) => Some(magic.need1.max(1)),
        1 if player_level >= u16::from(magic.level2) => Some(magic.need2.max(1)),
        2 if player_level >= u16::from(magic.level3) => Some(magic.need3.max(1)),
        _ => None,
    }
}

pub(super) fn cast_summon_skill(
    world: &mut World,
    player: Entity,
    _skill_key: &str,
    _skill_name: &str,
    spell: &str,
    skill_level: u8,
    tick: u64,
) -> Vec<ServerPacket> {
    let packets = Vec::new();

    let Some(player_object_id) = current_player_object_id(world) else {
        return packets;
    };
    let Some(player_position) = entity_position(world, player) else {
        return packets;
    };
    let player_direction = entity_facing(world, player).unwrap_or(MirDirection::Down);
    let near_spawn = |distance| {
        summon_spawn_position_near(
            world,
            &player_position,
            player_direction,
            distance,
            Some(player),
        )
    };

    let (monster_name, due_tick, spawn_position, summon_metadata, max_summons, unique_recall) =
        match spell {
            "SummonShinsu" => (
                "Shinsu",
                tick + combat_delay_ticks(500),
                near_spawn(1),
                Some(SummonedMonster {
                    summoner_object_id: player_object_id,
                    visible_extra: true,
                    expire_tick: None,
                    require_summoner_within: Some(15),
                    despawn_tick_after_death: None,
                    totem_master_object_id: None,
                    max_minions: Some(2),
                }),
                2,
                true,
            ),
            "SummonVampire" => (
                "VampireSpider",
                tick + ranged_attack_delay_ticks(&player_position, &near_spawn(3)),
                near_spawn(3),
                Some(SummonedMonster {
                    summoner_object_id: player_object_id,
                    visible_extra: true,
                    expire_tick: Some(
                        tick + combat_delay_ticks(15_000 + u64::from(skill_level) * 1_500),
                    ),
                    require_summoner_within: Some(15),
                    despawn_tick_after_death: None,
                    totem_master_object_id: None,
                    max_minions: Some(2),
                }),
                2,
                true,
            ),
            "SummonToad" => (
                "SpittingToad",
                tick + ranged_attack_delay_ticks(&player_position, &near_spawn(3)),
                near_spawn(3),
                Some(SummonedMonster {
                    summoner_object_id: player_object_id,
                    visible_extra: true,
                    expire_tick: Some(
                        tick + combat_delay_ticks(25_000 + u64::from(skill_level) * 2_000),
                    ),
                    require_summoner_within: Some(15),
                    despawn_tick_after_death: None,
                    totem_master_object_id: None,
                    max_minions: Some(2),
                }),
                2,
                true,
            ),
            "SummonSnakes" => (
                "SnakeTotem",
                tick + ranged_attack_delay_ticks(&player_position, &near_spawn(3)),
                near_spawn(3),
                Some(SummonedMonster {
                    summoner_object_id: player_object_id,
                    visible_extra: true,
                    expire_tick: Some(
                        tick + combat_delay_ticks(20_000 + u64::from(skill_level) * 1_500),
                    ),
                    require_summoner_within: Some(15),
                    despawn_tick_after_death: None,
                    totem_master_object_id: None,
                    max_minions: Some(usize::from(skill_level) + 1),
                }),
                2,
                true,
            ),
            "Stonetrap" => (
                "StoneTrap",
                tick + combat_delay_ticks(500),
                near_spawn(2),
                Some(SummonedMonster {
                    summoner_object_id: player_object_id,
                    visible_extra: true,
                    expire_tick: Some(tick + combat_delay_ticks(10_000)),
                    require_summoner_within: Some(15),
                    despawn_tick_after_death: None,
                    totem_master_object_id: None,
                    max_minions: Some(1),
                }),
                1,
                false,
            ),
            _ => return Vec::new(),
        };

    if recall_existing_summon(
        world,
        player_object_id,
        monster_name,
        &player_position,
        unique_recall,
    ) {
        return packets;
    }

    if active_summoned_monster_count(world, player_object_id) >= max_summons {
        return packets;
    }

    let Some(template) = crystal_dynamic_monster_template(monster_name) else {
        return Vec::new();
    };

    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick,
            summoner_entity: player,
            template: CrystalRespawnTemplate {
                location: spawn_position,
                ..template
            },
            target_entity: None,
            summon_metadata,
            hostile_to_player_override: Some(false),
        },
    );

    packets
}

pub(super) fn recall_existing_summon(
    world: &mut World,
    summoner_object_id: u32,
    monster_name: &str,
    player_position: &Point,
    enabled: bool,
) -> bool {
    if !enabled {
        return false;
    }

    #[allow(deprecated)]
    let existing = world.iter_entities().find_map(|entity| {
        let summoned = entity.get::<SummonedMonster>()?;
        if summoned.summoner_object_id != summoner_object_id {
            return None;
        }
        let agent = entity.get::<MonsterAgent>()?;
        if agent.dead {
            return None;
        }
        let name = entity.get::<DisplayName>()?;
        (name.value == monster_name).then_some(entity.id())
    });

    let Some(existing) = existing else {
        return false;
    };
    world
        .entity_mut(existing)
        .insert(Position(player_position.clone()));
    true
}

impl SimulationSession {
    pub fn cast_skill(&mut self, key: &str) -> Vec<ServerPacket> {
        let packets = self.cast_skill_impl(key);
        self.finalize_packets(packets)
    }

    pub(super) fn cast_skill_impl(&mut self, key: &str) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        cast_skill(self.app.world_mut(), key)
    }
}

#[cfg(test)]
mod crystal_damage_formula_tests {
    use super::*;

    fn test_magic(
        spell: &str,
        power_base: u16,
        power_bonus: u16,
        mpower_base: u16,
        mpower_bonus: u16,
        multiplier_base: f32,
        multiplier_bonus: f32,
    ) -> CrystalMagicTemplate {
        CrystalMagicTemplate {
            name: spell.to_string(),
            spell: spell.to_string(),
            base_cost: 0,
            level_cost: 0,
            icon: 0,
            level1: 0,
            level2: 0,
            level3: 0,
            need1: 0,
            need2: 0,
            need3: 0,
            delay_base: 0,
            delay_reduction: 0,
            power_base,
            power_bonus,
            mpower_base,
            mpower_bonus,
            multiplier_base,
            multiplier_bonus,
            range: 0,
        }
    }

    #[test]
    fn get_power_matches_crystal_def_plus_mpower_scaling() {
        // Crystal GetPower() = Round(MPower/4 * (Level+1) + DefPower).
        let magic = test_magic("FireBall", 10, 0, 8, 0, 1.0, 0.0);
        // level 0: 10 + (8 * 1) / 4 = 12
        assert_eq!(crystal_magic_get_power(&magic, 0), 12);
        // level 2: 10 + (8 * 3) / 4 = 16
        assert_eq!(crystal_magic_get_power(&magic, 2), 16);
    }

    #[test]
    fn multiplier_scales_linearly_with_level() {
        // Crystal GetMultiplier() = MultiplierBase + Level * MultiplierBonus.
        let magic = test_magic("FireBall", 0, 0, 0, 0, 1.0, 0.5);
        assert_eq!(crystal_magic_multiplier(&magic, 0), 1.0);
        assert_eq!(crystal_magic_multiplier(&magic, 1), 1.5);
        assert_eq!(crystal_magic_multiplier(&magic, 2), 2.0);
    }

    #[test]
    fn get_damage_folds_attack_power_inside_multiplier() {
        // Crystal GetDamage(base) = (base + GetPower()) * GetMultiplier().
        // This is the key fidelity fix: the player's attack power is multiplied too.
        let magic = test_magic("FireBall", 10, 0, 8, 0, 1.0, 0.5);
        // level 2: power = 16, mult = 2.0 -> (20 + 16) * 2 = 72
        assert_eq!(crystal_magic_get_damage(&magic, 2, 20), 72);
        // A higher attack roll scales through the multiplier: (40 + 16) * 2 = 112
        assert_eq!(crystal_magic_get_damage(&magic, 2, 40), 112);
    }

    #[test]
    fn get_damage_with_zero_base_equals_legacy_intrinsic_damage() {
        // The legacy intrinsic helper `crystal_magic_damage` is now defined as
        // `GetDamage(0)`, so the two agree for every spell/level by construction
        // (Crystal MagicInfo.cs:180 with damageBase = 0).
        let magic = test_magic("Lightning", 5, 4, 12, 6, 1.2, 0.4);
        for level in 0..=3u8 {
            assert_eq!(
                crystal_magic_get_damage(&magic, level, 0),
                crystal_magic_damage(&magic, level),
                "level {level} GetDamage(0) should match legacy intrinsic damage",
            );
        }

        // And both equal the hand-computed Crystal value for a clean-integer
        // spell: DefPower 4, MPower 8, MultiplierBase 2.0.
        //   GetPower(0) = Round((8/4)*1 + 4) = 6, GetDamage(0) = (int)(6*2.0) = 12.
        //   GetPower(2) = Round((8/4)*3 + 4) = 10, GetDamage(0) = (int)(10*2.0) = 20.
        let clean = test_magic("Lightning", 4, 0, 8, 0, 2.0, 0.0);
        assert_eq!(crystal_magic_damage(&clean, 0), 12);
        assert_eq!(crystal_magic_damage(&clean, 2), 20);

        // A sub-1.0 multiplier is honored (no longer floored to 1.0):
        // GetPower 0, MultiplierBase 0.5 -> the intrinsic is (int)(0 * 0.5) = 0.
        let weak = test_magic("HalfMoon", 0, 0, 0, 0, 0.5, 0.0);
        assert_eq!(crystal_magic_damage(&weak, 0), 0);
    }

    #[test]
    fn damage_channel_matches_crystal_class_sources() {
        // Wizard + Archer ranged scale off Magic-Crystal (MC).
        for spell in [
            "FireBall",
            "GreatFireBall",
            "Lightning",
            "FrostCrunch",
            "StraightShot",
        ] {
            assert_eq!(
                crystal_spell_damage_channel(spell),
                CrystalDamageChannel::Mc,
                "{spell} should roll MC",
            );
        }
        // Taoist spells scale off Spirit-Crystal (SC).
        for spell in [
            "Healing",
            "SoulFireBall",
            "Poisoning",
            "Curse",
            "MassHealing",
        ] {
            assert_eq!(
                crystal_spell_damage_channel(spell),
                CrystalDamageChannel::Sc,
                "{spell} should roll SC",
            );
        }
        // Warrior + Assassin melee scale off Damage-Crystal (DC).
        for spell in [
            "Slaying",
            "HalfMoon",
            "TwinDrakeBlade",
            "CrescentSlash",
            "CatTongue",
        ] {
            assert_eq!(
                crystal_spell_damage_channel(spell),
                CrystalDamageChannel::Dc,
                "{spell} should roll DC",
            );
        }
    }

    #[test]
    fn range_falloff_rewards_maximum_range() {
        // Crystal bows do full min damage at max range and the least at point blank.
        assert_eq!(
            crystal_range_min_after_falloff(90, CRYSTAL_MAX_ATTACK_RANGE),
            90
        );
        assert_eq!(crystal_range_min_after_falloff(90, 0), 0);
        // Mid range: penalty = floor(90/9 * (9-4)) = 50 -> 40.
        assert_eq!(crystal_range_min_after_falloff(90, 4), 40);
        // Range above the cap is clamped (no negative penalty / over-damage).
        assert_eq!(crystal_range_min_after_falloff(90, 50), 90);
    }

    #[test]
    fn armour_subtracts_and_floors_at_zero() {
        use crate::runtime::combat::crystal_armour_reduced_damage;
        // Crystal net = max(0, damage - armour); armour >= damage is a 0-damage miss.
        assert_eq!(crystal_armour_reduced_damage(100, 30), 70);
        assert_eq!(crystal_armour_reduced_damage(30, 100), 0);
        assert_eq!(crystal_armour_reduced_damage(50, 50), 0);
        // Negative armour is clamped to 0 (never amplifies damage).
        assert_eq!(crystal_armour_reduced_damage(50, -5), 50);
    }

    #[test]
    fn spell_defence_matches_crystal_defence_types() {
        use crate::runtime::combat::CrystalDefence;
        // Wizard/Taoist/Archer magic + MAC-routed warrior skills roll against MAC.
        for spell in [
            "FireBall",
            "Lightning",
            "SoulFireBall",
            "BladeAvalanche",
            "StraightShot",
        ] {
            assert_eq!(crystal_spell_defence(spell), CrystalDefence::Mac, "{spell}");
        }
        // Physical skills roll against AC (no agility dodge).
        for spell in [
            "CrescentSlash",
            "FlashDash",
            "CatTongue",
            "MoonMist",
            "FlamingSword",
        ] {
            assert_eq!(crystal_spell_defence(spell), CrystalDefence::Ac, "{spell}");
        }
        // HalfMoon family is pure agility dodge (no armour).
        for spell in [
            "HalfMoon",
            "CrossHalfMoon",
            "DoubleSlash",
            "Thrusting",
            "TwinDrakeBlade",
        ] {
            assert_eq!(
                crystal_spell_defence(spell),
                CrystalDefence::Agility,
                "{spell}"
            );
            assert!(crystal_spell_defence(spell).uses_agility_dodge());
        }
        // Control / knockback / poison-application spells bypass armour and dodge.
        for spell in ["Curse", "Repulsion", "Poisoning", "ShoulderDash"] {
            assert_eq!(
                crystal_spell_defence(spell),
                CrystalDefence::None,
                "{spell}"
            );
        }
    }
}

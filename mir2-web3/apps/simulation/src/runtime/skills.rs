use serde::{Deserialize, Serialize};

use crate::config::SkillSnapshot;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_magic_by_spell, crystal_magic_manifest, localized_text_or_fallback,
    starter_server_data, CrystalItemTemplate, CrystalMagicTemplate, CrystalRespawnTemplate,
    LanguageCode, SkillEffectTemplate, SkillTemplate,
};
use mir2_protocol::{ClientMagic, MirDirection, Point, ServerPacket, Spell, UserItemStat};

use super::buffs::{apply_or_refresh_buff, buff_metadata, client_buff_packet_for_state, BuffState};
use super::combat::{
    combat_delay_ticks, queued_before_world_tick_due_tick, ranged_attack_delay_ticks,
    schedule_damage_to_monster, PendingMonsterDefeatAction,
};
use super::components::{
    current_player_object_id, entity_by_object_id, entity_facing, entity_name,
    entity_player_vitals, entity_position, player_entity, DisplayName, MonsterAgent, PlayerVitals,
    Position, SummonedMonster,
};
use super::crystal_compat::CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT;
use super::items::merged_user_item_stats;
use super::monsters::{
    active_summoned_monster_count, crystal_dynamic_monster_template, queue_pending_monster_spawn,
    PendingMonsterSpawnAction,
};
use super::movement::summon_spawn_position_near;
use super::packets::{object_health_info_for_entity, object_mana_info_for_entity};
use super::resources::{is_in_world, SkillResource};
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
        SkillSnapshot {
            key: self.key.clone(),
            name: localized_skill_name(language, &self.key, &self.name),
            description: localized_skill_description(language, &self.key, &self.description),
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

pub(super) fn cast_skill(world: &mut World, key: &str) -> Vec<ServerPacket> {
    cast_skill_with_context(world, key, None)
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
    if skill.cooldown_ends_at > tick {
        return Vec::new();
    }
    let definition = skill_definition(skill.key.as_str());
    let crystal_magic = crystal_magic_for_skill_key(skill.key.as_str());
    let crystal_spell = definition
        .as_ref()
        .and_then(|definition| definition.crystal_spell.as_deref())
        .or_else(|| crystal_magic.as_ref().map(|magic| magic.spell.as_str()));
    let spell_packet = crystal_spell.and_then(Spell::from_crystal_name);
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
    let current_mp = entity_player_vitals(world, player)
        .map(|vitals| vitals.mp)
        .unwrap_or_default();
    if current_mp < mana_cost {
        return Vec::new();
    }

    let mut packets = Vec::new();
    {
        let mut entity = world.entity_mut(player);
        let mut vitals = entity.get_mut::<PlayerVitals>().expect("player vitals");
        vitals.mp = (vitals.mp - mana_cost).max(0);
    }
    if let Some(info) = object_mana_info_for_entity(world, player) {
        packets.push(ServerPacket::ObjectMana { info });
    }

    if let Some(definition) = definition.as_ref() {
        match &definition.effect {
            SkillEffectTemplate::Heal { hp } => {
                {
                    let mut entity = world.entity_mut(player);
                    let mut vitals = entity.get_mut::<PlayerVitals>().expect("player vitals");
                    vitals.hp = (vitals.hp + *hp).min(vitals.max_hp);
                }
                if let Some(info) = object_health_info_for_entity(world, player, 0) {
                    packets.push(ServerPacket::ObjectHealth { info });
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
        packets.extend(apply_manifest_spell_effect(
            world,
            player,
            &skill,
            crystal_magic.as_ref(),
            spell_packet,
            context.as_ref(),
            tick,
        ));
    }

    if let Some(spell) = spell_packet {
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
    }

    let (cooldown_ticks, delay_ms) = crystal_magic
        .as_ref()
        .map(|magic| {
            (
                crystal_magic_cooldown_ticks(magic, skill.level),
                crystal_magic_delay_ms(magic, skill.level),
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
    if let (Some(spell), Some(magic)) = (spell_packet, crystal_magic.as_ref()) {
        packets.extend(advance_magic_progression(world, index, spell, magic, tick));
    }
    packets
}

fn apply_manifest_spell_effect(
    world: &mut World,
    player: Entity,
    skill: &SkillState,
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
        "MagicShield" => {
            let buff = BuffState {
                key: "magic-shield".to_string(),
                name: "Magic Shield".to_string(),
                description: "Crystal magic shield buff is active.".to_string(),
                expires_at_tick: tick
                    + combat_delay_ticks(60_000 + u64::from(skill.level).saturating_mul(10_000)),
                attack_bonus: 0,
                defence_bonus: 0,
                stats: vec![UserItemStat {
                    stat: CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT,
                    value: (i32::from(skill.level) + 2) * 10,
                }],
            };
            apply_or_refresh_buff(world, buff.clone());
            if let Some(packet) = client_buff_packet_for_state(world, &buff) {
                packets.push(packet);
            }
            return packets;
        }
        "Teleport" => {
            if let Some(target) = context.map(|context| context.target.clone()) {
                if super::movement::can_occupy(world, target.clone(), Some(player)) {
                    world.entity_mut(player).insert(Position(target));
                }
            }
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
    let damage = crystal_magic_damage(magic, skill.level);
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

fn crystal_magic_damage(magic: &CrystalMagicTemplate, level: u8) -> i32 {
    let level = i32::from(level) + 1;
    let defence_power = i32::from(magic.power_base) + i32::from(magic.power_bonus) / 2;
    let magic_power = i32::from(magic.mpower_base) + i32::from(magic.mpower_bonus) / 2;
    let base = defence_power + (magic_power * level) / 4;
    let multiplier = magic.multiplier_base + f32::from(level as u16 - 1) * magic.multiplier_bonus;
    ((base.max(1) as f32) * multiplier.max(1.0)).round() as i32
}

fn advance_magic_progression(
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
    let gain = magic_experience_gain(tick, object_id, spell);
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

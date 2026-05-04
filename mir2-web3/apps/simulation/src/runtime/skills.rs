use serde::{Deserialize, Serialize};

use crate::config::SkillSnapshot;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_magic_by_spell, localized_text_or_fallback, starter_server_data, CrystalItemTemplate,
    CrystalRespawnTemplate, LanguageCode, SkillEffectTemplate, SkillTemplate,
};
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::buffs::{apply_or_refresh_buff, buff_metadata, BuffState};
use super::combat::{combat_delay_ticks, ranged_attack_delay_ticks};
use super::components::{
    current_player_object_id, entity_facing, entity_player_vitals, entity_position, player_entity,
    DisplayName, MonsterAgent, PlayerVitals, Position, SummonedMonster,
};
use super::items::merged_user_item_stats;
use super::monsters::{
    active_summoned_monster_count, crystal_dynamic_monster_template, queue_pending_monster_spawn,
    PendingMonsterSpawnAction,
};
use super::movement::summon_spawn_position_near;
use super::resources::{is_in_world, SkillResource};
use super::session::SimulationSession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SkillState {
    pub(super) key: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) level: u8,
    pub(super) cooldown_ticks: u32,
    pub(super) cooldown_ends_at: u64,
}

impl SkillState {
    pub(super) fn snapshot(&self, tick: u64, language: LanguageCode) -> SkillSnapshot {
        SkillSnapshot {
            key: self.key.clone(),
            name: localized_skill_name(language, &self.key, &self.name),
            description: localized_skill_description(language, &self.key, &self.description),
            cooldown_remaining_ticks: self.cooldown_ends_at.saturating_sub(tick) as u32,
        }
    }
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
        return Some(SkillState {
            key: skill.key,
            name: skill.name,
            description: skill.description,
            level,
            cooldown_ticks: skill.cooldown_ticks,
            cooldown_ends_at: 0,
        });
    }

    let magic = crystal_magic_by_spell(spell_name)?;
    Some(SkillState {
        key: normalize_crystal_skill_key(&magic.spell),
        name: magic.name,
        description: format!("Crystal NPC granted skill {}.", magic.spell),
        level,
        cooldown_ticks: magic.delay_base.max(1),
        cooldown_ends_at: 0,
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

pub(super) fn seed_skills() -> Vec<SkillState> {
    starter_server_data()
        .skills
        .into_iter()
        .map(|skill| SkillState {
            key: skill.key,
            name: skill.name,
            description: skill.description,
            level: 3,
            cooldown_ticks: skill.cooldown_ticks,
            cooldown_ends_at: 0,
        })
        .collect()
}

pub(super) fn cast_skill(world: &mut World, key: &str) -> Vec<ServerPacket> {
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
    let Some(definition) = skill_definition(skill.key.as_str()) else {
        return Vec::new();
    };
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let current_mp = entity_player_vitals(world, player)
        .map(|vitals| vitals.mp)
        .unwrap_or_default();
    if current_mp < definition.mana_cost {
        return Vec::new();
    }

    let mut packets = Vec::new();
    {
        let mut entity = world.entity_mut(player);
        let mut vitals = entity.get_mut::<PlayerVitals>().expect("player vitals");
        vitals.mp = (vitals.mp - definition.mana_cost).max(0);
        match &definition.effect {
            SkillEffectTemplate::Heal { hp } => {
                vitals.hp = (vitals.hp + *hp).min(vitals.max_hp);
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
                drop(vitals);
                drop(entity);
                apply_or_refresh_buff(
                    world,
                    BuffState {
                        key: buff_key.clone(),
                        name: resolved_buff_name,
                        description: resolved_buff_description,
                        expires_at_tick: tick + *duration_ticks,
                        attack_bonus: *attack_bonus,
                        defence_bonus: *defence_bonus,
                        stats: merged_user_item_stats(&[], *defence_bonus, *attack_bonus, None),
                    },
                );
            }
            SkillEffectTemplate::Summon { spell } => {
                drop(vitals);
                drop(entity);
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

    world.resource_mut::<SkillResource>().skills[index].cooldown_ends_at =
        tick + u64::from(skill.cooldown_ticks);
    packets
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

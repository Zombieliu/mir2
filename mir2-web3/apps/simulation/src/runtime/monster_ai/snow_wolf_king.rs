// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::CrystalRespawnTemplate;
use mir2_protocol::{MirDirection, Point};

use super::super::combat::*;
use super::super::components::{
    Monster, MonsterAgent, SummonedMonster, entity_facing, entity_object_id, entity_position,
    player_entity,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn spawn_snow_wolf_king_slaves(
    world: &mut World,
    object_id: u32,
    position: &Point,
    direction: MirDirection,
    target_entity: Entity,
    target_position: &Point,
    agent: &MonsterAgent,
) {
    let Some(template) = crystal_dynamic_monster_template("SnowWolf") else {
        return;
    };
    let target_back = entity_facing(world, target_entity)
        .map(|facing| offset_point(target_position, facing, -1))
        .unwrap_or_else(|| offset_point(target_position, direction, -1));
    let summon_metadata = SummonedMonster {
        summoner_object_id: object_id,
        visible_extra: false,
        expire_tick: None,
        require_summoner_within: None,
        despawn_tick_after_death: None,
        totem_master_object_id: None,
        max_minions: Some(SNOW_WOLF_KING_SLAVE_COUNT),
    };

    for _ in 0..SNOW_WOLF_KING_SLAVE_COUNT {
        let template = CrystalRespawnTemplate {
            location: target_back.clone(),
            ..template.clone()
        };
        if spawn_runtime_monster(
            world,
            &template,
            target_back.clone(),
            direction,
            Some(target_entity),
            Some(summon_metadata),
            Some(agent.hostile_to_player),
            Some(agent.disposition),
            combat_delay_ticks(2_000),
        )
        .is_none()
        {
            let fallback_template = CrystalRespawnTemplate {
                location: position.clone(),
                ..template
            };
            let _ = spawn_runtime_monster(
                world,
                &fallback_template,
                position.clone(),
                direction,
                Some(target_entity),
                Some(summon_metadata),
                Some(agent.hostile_to_player),
                Some(agent.disposition),
                combat_delay_ticks(2_000),
            );
        }
    }
}

pub(in crate::runtime) fn schedule_snow_wolf_king_death_explosion(
    world: &mut World,
    monster_entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
    monster_name: &str,
    current_tick: u64,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let damage = crystal_monster_attack_damage(monster_name);
    if damage <= 0 {
        return;
    }
    let due_tick = current_tick + combat_delay_ticks(500);

    if agent.hostile_to_player {
        if let Some(player) = player_entity(world) {
            if entity_position(world, player)
                .map(|player_position| tile_distance(position, &player_position) <= 1)
                .unwrap_or(false)
            {
                schedule_damage_to_player(
                    world,
                    due_tick,
                    attacker_id,
                    monster_name.to_string(),
                    damage,
                );
            }
        }
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    for target in nearby_opposing_monster_targets(
        world,
        &monster_entities,
        monster_entity,
        position,
        agent,
        1,
    ) {
        schedule_damage_to_monster(world, due_tick, attacker_id, target, damage, None, None);
    }
}

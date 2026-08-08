// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::CrystalRespawnTemplate;
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    entity_object_id, entity_position, player_entity, Facing, Monster, MonsterAgent,
    MonsterAiState, Position, SummonedMonster,
};
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn update_snake_totem_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let Some(object_id) = entity_object_id(world, entity) else {
        return true;
    };
    let is_friendly_totem = !agent.hostile_to_player;
    let max_minions = world
        .entity(entity)
        .get::<SummonedMonster>()
        .and_then(|summoned| summoned.max_minions)
        .unwrap_or(2);
    let active_minions = active_summoned_monster_count(world, object_id);

    if active_minions >= max_minions || tick < agent.next_attack_tick {
        agent.tracking_player = false;
        return true;
    }

    let target_entity = if is_friendly_totem {
        let monster_entities: Vec<Entity> = world
            .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
            .iter(world)
            .collect();
        let nearby_hostiles = monster_entities
            .iter()
            .copied()
            .filter(|candidate| *candidate != entity)
            .filter(|candidate| {
                let entry = world.entity(*candidate);
                let Some(candidate_agent) = entry.get::<MonsterAgent>() else {
                    return false;
                };
                if candidate_agent.dead || !candidate_agent.hostile_to_player {
                    return false;
                }
                let candidate_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
                if is_hidden_or_sleeping_target(candidate_agent, &candidate_ai_state) {
                    return false;
                }
                entry
                    .get::<Position>()
                    .map(|candidate_position| {
                        tile_distance(position, &candidate_position.0) <= agent.view_range.max(1)
                    })
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        for hostile in nearby_hostiles {
            if let Some(mut hostile_agent) = world.entity_mut(hostile).get_mut::<MonsterAgent>() {
                hostile_agent.tracking_player = false;
            }
        }
        summoned_monster_entity_target(world, &monster_entities, entity, position, agent)
    } else {
        let Some(player) = player_entity(world) else {
            return true;
        };
        let Some(player_position) = entity_position(world, player) else {
            return true;
        };
        if tile_distance(position, &player_position) > agent.view_range.max(1) {
            agent.tracking_player = false;
            return true;
        }
        Some(player)
    };

    let Some(target_entity) = target_entity else {
        agent.tracking_player = false;
        return true;
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return true;
    };
    let Some(direction) = direction_toward(position, &target_position) else {
        return true;
    };

    agent.tracking_player = false;
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    world.entity_mut(entity).insert(Facing(direction));
    if let Some(packet) = monster_melee_attack_packet(world, entity, position, direction) {
        packets.push(packet);
    }

    let spawn_position =
        directional_destination(world, position, MirDirection::Down, 1, Some(entity))
            .unwrap_or_else(|| offset_point(position, MirDirection::Down, 1));
    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: tick + combat_delay_ticks(500),
            summoner_entity: entity,
            template: CrystalRespawnTemplate {
                location: spawn_position,
                ..crystal_dynamic_monster_template("CharmedSnake")
                    .expect("CharmedSnake template should exist")
            },
            target_entity: Some(target_entity),
            summon_metadata: Some(SummonedMonster {
                summoner_object_id: object_id,
                visible_extra: true,
                expire_tick: Some(
                    tick + combat_delay_ticks(10_000 + (max_minions as u64 - 1) * 2_000),
                ),
                require_summoner_within: Some(15),
                despawn_tick_after_death: None,
                totem_master_object_id: Some(object_id),
                max_minions: Some(max_minions),
            }),
            hostile_to_player_override: Some(!is_friendly_totem),
        },
    );

    true
}

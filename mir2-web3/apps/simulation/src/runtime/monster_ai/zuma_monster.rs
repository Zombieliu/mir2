// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::components::{
    entity_object_id, Monster, MonsterAgent, MonsterAiState, ObjectId, Position,
};
use super::super::monsters::*;
use super::super::movement::*;

pub(in crate::runtime) fn update_zuma_monster_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if !ai_state.extra {
        return false;
    }

    if tile_distance(position, player_position) > 2 {
        return true;
    }

    ai_state.extra = false;
    agent.tracking_player = true;
    agent.next_attack_tick = agent.next_attack_tick.max(tick + 1);
    if let Some(object_id) = entity_object_id(world, entity) {
        packets.push(ServerPacket::ObjectShow { object_id });
    }

    let awakened_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .filter(|candidate| *candidate != entity)
        .filter(|candidate| {
            let entry = world.entity(*candidate);
            let Some(candidate_agent) = entry.get::<MonsterAgent>() else {
                return false;
            };
            if !monster_uses_zuma_stone_state(candidate_agent.ai) {
                return false;
            }
            let candidate_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
            candidate_ai_state.extra
                && entry
                    .get::<Position>()
                    .map(|candidate_position| tile_distance(position, &candidate_position.0) <= 14)
                    .unwrap_or(false)
        })
        .collect();

    for awakened in awakened_entities {
        let mut entry = world.entity_mut(awakened);
        if let Some(mut awakened_state) = entry.get_mut::<MonsterAiState>() {
            awakened_state.extra = false;
        }
        if let Some(mut awakened_agent) = entry.get_mut::<MonsterAgent>() {
            awakened_agent.tracking_player = true;
            awakened_agent.next_attack_tick = awakened_agent.next_attack_tick.max(tick + 1);
        }
        if let Some(object_id) = entry.get::<ObjectId>().map(|value| value.0) {
            packets.push(ServerPacket::ObjectShow { object_id });
        }
    }

    true
}

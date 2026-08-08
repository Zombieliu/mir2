// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::crystal_monster_by_name;
use mir2_protocol::{Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{entity_object_id, Monster, MonsterAgent, MonsterAiState};

use super::common::*;

pub(in crate::runtime) fn update_shinsu_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    let has_target =
        summoned_monster_entity_target(world, &monster_entities, entity, position, agent).is_some();
    if has_target {
        ai_state.next_state_tick = tick + combat_delay_ticks(30_000);
        agent.tracking_player = true;
    }

    if !ai_state.mode && has_target {
        ai_state.mode = true;
        ai_state.hidden = false;
        if let Some(monster) = crystal_monster_by_name("Shinsu1") {
            agent.image = monster.image;
        }
        if let Some(object_id) = entity_object_id(world, entity) {
            packets.push(ServerPacket::ObjectShow { object_id });
        }
        return true;
    }

    if ai_state.mode && !has_target && tick >= ai_state.next_state_tick {
        ai_state.mode = false;
        ai_state.hidden = true;
        agent.tracking_player = false;
        if let Some(monster) = crystal_monster_by_name("Shinsu") {
            agent.image = monster.image;
        }
        if let Some(object_id) = entity_object_id(world, entity) {
            packets.push(ServerPacket::ObjectHide { object_id });
        }
        return true;
    }

    let _ = player_position;
    false
}

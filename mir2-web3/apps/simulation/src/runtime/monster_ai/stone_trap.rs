// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::Point;

use super::super::components::{Monster, MonsterAgent, Position, entity_object_id};
use super::super::movement::*;

pub(in crate::runtime) fn update_stone_trap_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
) -> bool {
    if agent.dead {
        return true;
    }

    let Some(object_id) = entity_object_id(world, entity) else {
        return false;
    };

    agent.tracking_player = false;
    agent.next_move_tick = tick + 1;
    agent.next_attack_tick = tick + 1;

    let target_candidates: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .filter(|candidate| *candidate != entity)
        .filter(|candidate| {
            let entry = world.entity(*candidate);
            let Some(candidate_agent) = entry.get::<MonsterAgent>() else {
                return false;
            };
            if candidate_agent.dead {
                return false;
            }
            let Some(candidate_position) = entry.get::<Position>().map(|value| value.0.clone())
            else {
                return false;
            };
            tile_distance(position, &candidate_position) <= agent.view_range.max(1)
                && candidate_agent.hostile_to_player
        })
        .collect();

    for candidate in target_candidates {
        if let Some(mut candidate_agent) = world.entity_mut(candidate).get_mut::<MonsterAgent>() {
            candidate_agent.tracking_player = false;
            candidate_agent.next_move_tick = candidate_agent.next_move_tick.min(tick + 1);
        }
    }

    let _ = object_id;
    false
}

// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{entity_object_id, Facing, MonsterAgent, MonsterAiState, Position};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn update_axe_skeleton_fear_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !agent.tracking_player {
        return false;
    }

    if tick < ai_state.next_state_tick {
        return false;
    }

    ai_state.next_state_tick = tick + FOXMAN_FEAR_DURATION_TICKS;
    if tick < agent.next_move_tick {
        return true;
    }

    let next_step = if tile_distance(position, player_position) >= monster_attack_range(agent) {
        monster_step_toward_with_fallback(world, entity, position, player_position, tick, 8)
    } else {
        let away_direction = direction_toward(player_position, position);
        let object_id = entity_object_id(world, entity).unwrap_or_default();
        let prefer_next = deterministic_chance_roll(tick, object_id, 80, 2);
        let offsets: [i32; 8] = if prefer_next {
            [0, 1, 2, 3, 4, 5, 6, 7]
        } else {
            [0, -1, -2, -3, -4, -5, -6, -7]
        };
        away_direction.and_then(|direction| {
            offsets.into_iter().find_map(|offset| {
                let direction = rotated_direction(direction, offset);
                let next = offset_point(position, direction, 1);
                can_occupy(world, next.clone(), Some(entity)).then_some((next, direction))
            })
        })
    };

    if let Some((next, direction)) = next_step {
        let object_id = entity_object_id(world, entity).unwrap_or_default();
        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        world
            .entity_mut(entity)
            .insert((Position(next.clone()), Facing(direction), agent.clone()));
        packets.push(ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: next,
                direction,
            },
        });
    }

    true
}

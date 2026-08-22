// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{Facing, MonsterAgent, MonsterAiState, Position, entity_object_id};
use super::super::movement::*;

pub(in crate::runtime) fn update_deer_run_away_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !ai_state.mode || !agent.can_wander {
        return false;
    }

    let distance = tile_distance(position, player_position);
    if !agent.tracking_player && distance > agent.view_range.max(1) {
        return false;
    }
    agent.tracking_player = true;

    if tick < agent.next_move_tick {
        return true;
    }

    let Some(away_direction) = direction_toward(player_position, position) else {
        return true;
    };
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let prefer_next = deterministic_chance_roll(tick, object_id, 20, 2);
    let offsets: [i32; 8] = if prefer_next {
        [0, 1, 2, 3, 4, 5, 6, 7]
    } else {
        [0, -1, -2, -3, -4, -5, -6, -7]
    };

    for offset in offsets {
        let direction = rotated_direction(away_direction, offset);
        let next = offset_point(position, direction, 1);
        if !can_occupy(world, next.clone(), Some(entity)) {
            continue;
        }

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
        return true;
    }

    true
}

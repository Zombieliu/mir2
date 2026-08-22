// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    Facing, MonsterAgent, MonsterAiState, Position, SummonedMonster, entity_object_id,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

pub(in crate::runtime) fn update_holy_deva_fear_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead
        || !agent.tracking_player
        || world.entity(entity).get::<SummonedMonster>().is_some()
    {
        return false;
    }

    if tick < ai_state.next_state_tick {
        return false;
    }

    ai_state.next_state_tick = tick + FOXMAN_FEAR_DURATION_TICKS;
    if tile_distance(position, player_position) >= monster_attack_range(agent) {
        return true;
    }
    if tick < agent.next_move_tick {
        return true;
    }

    let Some(away_direction) = direction_toward(player_position, position) else {
        return true;
    };
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let prefer_next = deterministic_chance_roll(tick, object_id, 380, 2);
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

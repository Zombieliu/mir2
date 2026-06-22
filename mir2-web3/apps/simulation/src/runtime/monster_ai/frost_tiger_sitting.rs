// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::components::{MonsterAgent, MonsterAiState};
use super::super::crystal_compat::*;

use super::common::*;

pub(in crate::runtime) fn update_frost_tiger_sitting_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return false;
    }

    if ai_state.extra {
        if agent.tracking_player {
            ai_state.extra = false;
            ai_state.next_state_tick = tick + FROST_TIGER_SIT_DOWN_MAX_DELAY_TICKS;
            if let Some(packet) = object_sit_down_packet(world, entity, position, false) {
                packets.push(packet);
            }
            return false;
        }
        agent.next_attack_tick = tick + 1;
        agent.next_move_tick = tick + 1;
        return true;
    }

    if !agent.tracking_player && tick >= ai_state.next_state_tick {
        ai_state.extra = true;
        agent.tracking_player = false;
        agent.next_attack_tick = tick + 1;
        agent.next_move_tick = tick + 1;
        if let Some(packet) = object_sit_down_packet(world, entity, position, true) {
            packets.push(packet);
        }
        return true;
    }

    false
}

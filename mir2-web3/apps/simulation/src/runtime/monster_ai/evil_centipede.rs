// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::components::{entity_object_id, MonsterAgent, MonsterAiState, MonsterVitals};
use super::super::movement::*;

pub(in crate::runtime) fn update_evil_centipede_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let reveal_nearby = tile_distance(position, player_position) <= 3;
    let active_nearby = tile_distance(position, player_position) <= 7;

    if ai_state.hidden {
        agent.tracking_player = false;

        if tick < ai_state.next_state_tick {
            return true;
        }

        ai_state.next_state_tick = tick + 2;
        if reveal_nearby {
            ai_state.hidden = false;
            agent.tracking_player = true;
            agent.next_attack_tick = agent.next_attack_tick.max(tick + 1);
            if let Some(object_id) = entity_object_id(world, entity) {
                packets.push(ServerPacket::ObjectShow { object_id });
            }
        }
        return true;
    }

    if tick >= ai_state.next_state_tick {
        ai_state.next_state_tick = tick + 2;
        if !active_nearby {
            ai_state.hidden = true;
            ai_state.next_state_tick = tick + 3;
            agent.tracking_player = false;
            if let Some(mut vitals) = world.entity_mut(entity).get_mut::<MonsterVitals>() {
                vitals.hp = vitals.max_hp;
            }
            if let Some(object_id) = entity_object_id(world, entity) {
                packets.push(ServerPacket::ObjectHide { object_id });
            }
            return true;
        }
    }

    false
}

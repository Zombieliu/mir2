// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::components::{entity_object_id, MonsterAgent, MonsterAiState};
use super::super::movement::*;

pub(in crate::runtime) fn update_dig_out_zombie_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !ai_state.hidden {
        return false;
    }

    agent.tracking_player = false;
    agent.next_move_tick = tick + 1;

    if tile_distance(position, player_position) > 3 {
        return true;
    }

    ai_state.hidden = false;
    agent.tracking_player = true;
    agent.next_attack_tick = agent.next_attack_tick.max(tick + 2);
    if let Some(object_id) = entity_object_id(world, entity) {
        packets.push(ServerPacket::ObjectShow { object_id });
    }

    true
}

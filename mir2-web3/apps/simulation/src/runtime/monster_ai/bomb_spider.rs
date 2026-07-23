// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{MonsterAgent, MonsterAiState};
use super::super::movement::*;

pub(in crate::runtime) fn update_bomb_spider_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return true;
    }

    let player_is_target = tile_distance(position, player_position) <= 1;
    if !agent.tracking_player || player_is_target || tick >= ai_state.next_state_tick {
        explode_bomb_spider(world, entity, position, packets);
        return true;
    }

    false
}

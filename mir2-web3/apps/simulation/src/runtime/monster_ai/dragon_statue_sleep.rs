// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::ServerPacket;

use super::super::components::{MonsterAgent, MonsterAiState, MonsterVitals};
use super::super::packets::*;

pub(in crate::runtime) fn update_dragon_statue_sleep_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !ai_state.mode {
        return false;
    }

    agent.tracking_player = false;
    agent.next_attack_tick = tick + 1;
    agent.next_move_tick = tick + 1;
    if tick < ai_state.next_state_tick {
        return true;
    }

    ai_state.mode = false;
    let max_hp = world
        .entity(entity)
        .get::<MonsterVitals>()
        .map(|vitals| vitals.max_hp)
        .unwrap_or(1);
    world
        .entity_mut(entity)
        .insert(MonsterVitals { hp: max_hp, max_hp });
    if let Some(info) = object_health_info_for_entity(world, entity, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    true
}

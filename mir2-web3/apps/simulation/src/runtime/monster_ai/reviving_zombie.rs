// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::ServerPacket;

use super::super::components::{MonsterAgent, MonsterAiState, MonsterVitals};
use super::super::crystal_compat::*;
use super::super::packets::*;

pub(in crate::runtime) fn update_reviving_zombie_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if !agent.dead {
        return false;
    }

    if !ai_state.mode
        || ai_state.extra_byte >= REVIVING_ZOMBIE_MAX_REVIVALS
        || tick < ai_state.next_state_tick
    {
        return true;
    }

    ai_state.extra_byte += 1;
    ai_state.mode = false;
    ai_state.next_state_tick = 0;
    agent.dead = false;

    let revival_count = i32::from(ai_state.extra_byte);
    let max_hp = world
        .entity(entity)
        .get::<MonsterVitals>()
        .map(|vitals| vitals.max_hp.max(1))
        .unwrap_or(1);
    let hp = (max_hp * (100 - 25 * revival_count) / 100).max(1);
    world
        .entity_mut(entity)
        .insert((MonsterVitals { hp, max_hp }, agent.clone(), *ai_state));

    if let Some(info) = object_revived_info_for_entity(world, entity, false) {
        packets.push(ServerPacket::ObjectRevived { info });
    }
    if let Some(info) = object_health_info_for_entity(world, entity, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }

    true
}

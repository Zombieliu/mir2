// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::components::{
    Facing, MonsterAgent, MonsterAiState, MonsterVitals, entity_facing, entity_object_id,
    player_entity,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn update_bone_lord_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !monster_can_attack(agent, ai_state) {
        return false;
    }
    if !agent.tracking_player
        && !(agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1))
    {
        return false;
    }

    let Some(vitals) = world.entity(entity).get::<MonsterVitals>().copied() else {
        return false;
    };
    if vitals.max_hp < i32::from(BONE_LORD_STAGE_COUNT) {
        return false;
    }

    let stage_size = (vitals.max_hp / i32::from(BONE_LORD_STAGE_COUNT)).max(1);
    let stage = (vitals.hp.max(0) / stage_size).clamp(0, i32::from(BONE_LORD_STAGE_COUNT)) as u8;
    if stage >= ai_state.extra_byte {
        return false;
    }

    let Some(object_id) = entity_object_id(world, entity) else {
        return false;
    };
    let Some(player) = player_entity(world) else {
        return false;
    };
    let direction = direction_toward(position, player_position)
        .or_else(|| entity_facing(world, entity))
        .unwrap_or(MirDirection::Up);

    ai_state.extra_byte = stage;
    agent.tracking_player = true;
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    world.entity_mut(entity).insert(Facing(direction));
    if let Some(packet) = monster_typed_attack_packet(world, entity, position, direction, 1) {
        packets.push(packet);
    }

    spawn_monster_slave_wave(
        world,
        entity,
        object_id,
        position,
        direction,
        Some(player),
        agent,
        tick,
        &BONE_LORD_SLAVE_NAMES,
        BONE_LORD_MAX_SLAVES,
        BONE_LORD_SPAWN_BATCH_SIZE,
    );
    true
}

// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point};

use super::super::components::{
    MonsterAgent, MonsterAiState, MonsterVitals, entity_facing, entity_object_id, player_entity,
};
use super::super::crystal_compat::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn update_zuma_taurus_stage_state(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
) {
    if agent.dead {
        return;
    }

    let Some(vitals) = world.entity(entity).get::<MonsterVitals>().copied() else {
        return;
    };
    if vitals.max_hp < i32::from(ZUMA_TAURUS_STAGE_COUNT) {
        return;
    }

    let stage_size = (vitals.max_hp / i32::from(ZUMA_TAURUS_STAGE_COUNT)).max(1);
    let stage = (vitals.hp.max(0) / stage_size).clamp(0, i32::from(ZUMA_TAURUS_STAGE_COUNT)) as u8;
    if stage >= ai_state.extra_byte {
        return;
    }

    let Some(object_id) = entity_object_id(world, entity) else {
        return;
    };
    let direction = entity_facing(world, entity).unwrap_or(MirDirection::DownLeft);
    let target_entity = if agent.tracking_player
        || (agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1))
    {
        player_entity(world)
    } else {
        None
    };

    ai_state.extra_byte = stage;
    spawn_monster_slave_wave(
        world,
        entity,
        object_id,
        position,
        direction,
        target_entity,
        agent,
        tick,
        &ZUMA_TAURUS_SLAVE_NAMES,
        ZUMA_TAURUS_MAX_SLAVES,
        ZUMA_TAURUS_SPAWN_BATCH_SIZE,
    );
}

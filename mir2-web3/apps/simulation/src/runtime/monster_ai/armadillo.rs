// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectMovement, ObjectSpellInfo, Point, ServerPacket, Spell};

use super::super::combat::*;
use super::super::components::{
    entity_facing, entity_object_id, Facing, MonsterAgent, MonsterAiState, Position,
};
use super::super::monsters::*;
use super::super::movement::*;
use super::dig_out_zombie::update_dig_out_zombie_state;

pub(in crate::runtime) fn update_armadillo_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let was_hidden = ai_state.hidden;
    if update_dig_out_zombie_state(
        world,
        entity,
        agent,
        ai_state,
        position,
        player_position,
        tick,
        packets,
    ) {
        if was_hidden && !ai_state.hidden {
            ai_state.next_state_tick = tick + combat_delay_ticks(500);
        }
        return true;
    }

    if !ai_state.hidden
        && !ai_state.extra
        && ai_state.next_state_tick > 0
        && tick >= ai_state.next_state_tick
    {
        ai_state.extra = true;
        packets.push(armadillo_dig_out_spell_packet(
            world,
            position.clone(),
            entity_facing(world, entity).unwrap_or(MirDirection::Down),
        ));
    }

    update_armadillo_run_away_state(
        world,
        entity,
        agent,
        ai_state,
        position,
        player_position,
        tick,
        packets,
    )
}

pub(in crate::runtime) fn armadillo_dig_out_spell_packet(
    world: &mut World,
    location: Point,
    direction: MirDirection,
) -> ServerPacket {
    ServerPacket::ObjectSpell {
        info: ObjectSpellInfo {
            object_id: allocate_runtime_monster_object_id(world),
            location,
            spell: Spell::DigOutArmadillo,
            direction,
            param: true,
        },
    }
}

pub(in crate::runtime) fn update_armadillo_run_away_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || ai_state.hidden || !ai_state.mode {
        return false;
    }

    agent.tracking_player = false;
    if tick < agent.next_move_tick {
        return true;
    }

    let Some(away_direction) = direction_toward(player_position, position) else {
        return true;
    };
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let prefer_next = deterministic_chance_roll(tick, object_id, 1250, 2);
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

// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::components::{
    entity_object_id, Facing, MonsterAgent, MonsterVitals, Position, WoomaTaurusState,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn update_wooma_taurus_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return false;
    }

    let existing_state = world.entity(entity).get::<WoomaTaurusState>().copied();
    let mut state = existing_state.unwrap_or_else(|| {
        initial_wooma_taurus_state(tick, agent.move_interval_ticks, agent.attack_interval_ticks)
    });

    if state.mad_until_tick > 0 && tick >= state.mad_until_tick {
        state.mad_until_tick = 0;
        agent.move_interval_ticks = state.base_move_interval_ticks;
        agent.attack_interval_ticks = state.base_attack_interval_ticks;
    }

    if let Some(vitals) = world.entity(entity).get::<MonsterVitals>() {
        if vitals.max_hp >= i32::from(WOOMA_TAURUS_STAGE_COUNT) {
            let stage_size = (vitals.max_hp / i32::from(WOOMA_TAURUS_STAGE_COUNT)).max(1);
            let stage =
                (vitals.hp.max(0) / stage_size).clamp(0, i32::from(WOOMA_TAURUS_STAGE_COUNT)) as u8;
            if stage < state.stage {
                state.mad_until_tick = tick + WOOMA_TAURUS_MAD_DURATION_TICKS;
                agent.move_interval_ticks = WOOMA_TAURUS_MAD_MOVE_INTERVAL_TICKS;
                agent.attack_interval_ticks = WOOMA_TAURUS_MAD_ATTACK_INTERVAL_TICKS;
                agent.next_move_tick = agent.next_move_tick.min(tick + 1);
                agent.next_attack_tick = agent.next_attack_tick.min(tick + 1);
            }
            state.stage = stage;
        }
    }

    let mut teleported = false;
    if tick >= state.next_teleport_tick {
        state.next_teleport_tick = tick + WOOMA_TAURUS_TELEPORT_DELAY_TICKS;
        if wooma_taurus_blocked_neighbor_count(world, entity, position)
            >= WOOMA_TAURUS_BLOCKED_NEIGHBOR_THRESHOLD
        {
            if let Some(destination) =
                wooma_taurus_teleport_destination(world, entity, position, tick)
            {
                agent.tracking_player = false;
                agent.next_move_tick = tick + 1;
                agent.next_attack_tick = tick + 1;
                let direction = world
                    .entity(entity)
                    .get::<Facing>()
                    .map(|facing| facing.0)
                    .unwrap_or(MirDirection::Down);
                if let Some(object_id) = entity_object_id(world, entity) {
                    world
                        .entity_mut(entity)
                        .insert(Position(destination.clone()));
                    packets.push(ServerPacket::ObjectTeleportOut {
                        object_id,
                        effect_type: 0,
                    });
                    packets.push(ServerPacket::ObjectWalk {
                        movement: ObjectMovement {
                            object_id,
                            position: destination,
                            direction,
                        },
                    });
                    packets.push(ServerPacket::ObjectTeleportIn {
                        object_id,
                        effect_type: 0,
                    });
                    teleported = true;
                }
            }
        }
    }

    if existing_state != Some(state) {
        world.entity_mut(entity).insert(state);
    }

    teleported
}

pub(in crate::runtime) fn wooma_taurus_blocked_neighbor_count(
    world: &World,
    entity: Entity,
    position: &Point,
) -> usize {
    [
        MirDirection::Up,
        MirDirection::UpRight,
        MirDirection::Right,
        MirDirection::DownRight,
        MirDirection::Down,
        MirDirection::DownLeft,
        MirDirection::Left,
        MirDirection::UpLeft,
    ]
    .into_iter()
    .filter(|direction| {
        let point = offset_point(position, *direction, 1);
        !can_occupy(world, point, Some(entity))
    })
    .count()
}

pub(in crate::runtime) fn wooma_taurus_teleport_destination(
    world: &World,
    entity: Entity,
    position: &Point,
    tick: u64,
) -> Option<Point> {
    let mut candidates = Vec::new();
    for distance in 2..=WOOMA_TAURUS_TELEPORT_RADIUS {
        for y in position.y - distance..=position.y + distance {
            for x in position.x - distance..=position.x + distance {
                let point = Point { x, y };
                if tile_distance(position, &point) != distance {
                    continue;
                }
                if can_occupy(world, point.clone(), Some(entity)) {
                    candidates.push(point);
                }
            }
        }
    }

    if candidates.is_empty() {
        return first_occupiable_point_near(
            world,
            position,
            WOOMA_TAURUS_TELEPORT_RADIUS * 2,
            Some(entity),
        );
    }

    let index = deterministic_roll(tick, entity.index() as usize, 11, candidates.len() as u64);
    candidates.get(index as usize).cloned()
}

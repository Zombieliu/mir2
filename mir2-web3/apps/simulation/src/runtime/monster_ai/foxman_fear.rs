// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{Facing, MonsterAgent, MonsterAiState, Position, entity_object_id};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

pub(in crate::runtime) fn update_foxman_fear_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !agent.tracking_player {
        return false;
    }

    let distance = tile_distance(position, player_position);
    if agent.ai == 45
        && distance <= 1
        && tick < ai_state.next_state_tick
        && tick >= agent.next_route_tick
    {
        if let Some(destination) = foxman_teleport_destination(world, entity, position, tick) {
            let direction = direction_toward(&destination, player_position).unwrap_or_else(|| {
                world
                    .entity(entity)
                    .get::<Facing>()
                    .map(|facing| facing.0)
                    .unwrap_or(MirDirection::Down)
            });
            agent.next_route_tick = tick + RED_FOXMAN_TELEPORT_COOLDOWN_TICKS;
            agent.next_move_tick = tick + 1;
            agent.next_attack_tick = tick + 1;
            world.entity_mut(entity).insert((
                Position(destination.clone()),
                Facing(direction),
                agent.clone(),
            ));
            if let Some(object_id) = entity_object_id(world, entity) {
                packets.push(ServerPacket::ObjectTeleportOut {
                    object_id,
                    effect_type: RED_FOXMAN_TELEPORT_EFFECT,
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
                    effect_type: RED_FOXMAN_TELEPORT_EFFECT,
                });
            }
            return true;
        }
    }

    if tick < ai_state.next_state_tick {
        return false;
    }

    ai_state.next_state_tick = tick + FOXMAN_FEAR_DURATION_TICKS;
    if tick < agent.next_move_tick {
        return true;
    }

    let attack_range = monster_attack_range(agent);
    let preferred_direction = if distance >= attack_range {
        direction_toward(position, player_position)
    } else {
        direction_toward(player_position, position)
    };
    let Some(preferred_direction) = preferred_direction else {
        return true;
    };
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let prefer_next = deterministic_chance_roll(tick, object_id, 450, 2);
    let offsets: [i32; 8] = if prefer_next {
        [0, 1, 2, 3, 4, 5, 6, 7]
    } else {
        [0, -1, -2, -3, -4, -5, -6, -7]
    };

    for offset in offsets {
        let direction = rotated_direction(preferred_direction, offset);
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

pub(in crate::runtime) fn foxman_teleport_destination(
    world: &World,
    entity: Entity,
    position: &Point,
    tick: u64,
) -> Option<Point> {
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let side = RED_FOXMAN_TELEPORT_RADIUS * 2 + 1;
    let area = side * side;
    let start = ((tick as i32)
        .wrapping_add((object_id as i32).wrapping_mul(31))
        .rem_euclid(area)) as i32;

    for offset in 0..area {
        let index = (start + offset).rem_euclid(area);
        let dx = index.rem_euclid(side) - RED_FOXMAN_TELEPORT_RADIUS;
        let dy = index.div_euclid(side) - RED_FOXMAN_TELEPORT_RADIUS;
        let candidate = Point {
            x: position.x + dx,
            y: position.y + dy,
        };
        if candidate == *position || tile_distance(position, &candidate) <= 1 {
            continue;
        }
        if can_occupy(world, candidate.clone(), Some(entity)) {
            return Some(candidate);
        }
    }

    None
}

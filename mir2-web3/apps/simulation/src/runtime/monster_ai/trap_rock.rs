// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use crate::config::WorldEntityDisposition;
use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    DisplayName, Facing, Monster, MonsterAgent, MonsterAiState, MonsterCombatStats, MonsterVitals,
    ObjectId, Position, SummonedMonster, WorldObject, entity_name, entity_object_id,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

pub(in crate::runtime) fn update_trap_rock_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return true;
    }
    if !ai_state.hidden {
        if tick >= ai_state.next_state_tick {
            ai_state.next_state_tick = tick + 2;
            if player_position != &agent.patrol_origin {
                if damage_monster_entity(world, entity, i32::MAX, tick, packets) {
                    agent.dead = true;
                    ai_state.mode = false;
                }
                return true;
            }
        }
        return false;
    }

    agent.tracking_player = false;
    if tick < ai_state.next_state_tick {
        return true;
    }

    ai_state.next_state_tick = tick + 2;
    if !agent.hostile_to_player
        || tile_distance(&agent.patrol_origin, player_position) > agent.view_range.max(1)
    {
        return true;
    }

    let Some(destination) = trap_rock_visible_destination(world, entity, player_position, tick)
    else {
        return true;
    };
    let direction = direction_toward(&destination, player_position).unwrap_or(MirDirection::Down);
    ai_state.hidden = false;
    // Crystal stores TargetLocation after reveal; TrapRock is static, so patrol_origin is free here.
    agent.patrol_origin = player_position.clone();
    agent.tracking_player = true;
    agent.next_attack_tick = agent.next_attack_tick.max(tick + 1);
    agent.next_move_tick = tick + 1;
    ai_state.next_state_tick = tick + 2;
    world
        .entity_mut(entity)
        .insert((Position(destination.clone()), Facing(direction)));
    if let Some(object_id) = entity_object_id(world, entity) {
        packets.push(ServerPacket::ObjectShow { object_id });
    }
    apply_player_paralysis(world, tick, TRAP_ROCK_PARALYSIS_DURATION_TICKS);
    spawn_trap_rock_child_rocks(world, entity, &destination, player_position, tick, packets);

    true
}

pub(in crate::runtime) fn spawn_trap_rock_child_rocks(
    world: &mut World,
    parent_entity: Entity,
    parent_position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(parent_object_id) = entity_object_id(world, parent_entity) else {
        return;
    };
    let parent_corner =
        direction_toward(player_position, parent_position).unwrap_or(MirDirection::Up);
    let name = entity_name(world, parent_entity).unwrap_or_else(|| "TrapRock".to_string());
    let Some(template) = crystal_dynamic_monster_template(&name) else {
        return;
    };
    for corner in [
        MirDirection::Up,
        MirDirection::Right,
        MirDirection::Down,
        MirDirection::Left,
    ] {
        if corner == parent_corner {
            continue;
        }
        let child_position = offset_point(player_position, corner, 1);
        if !can_occupy(world, child_position.clone(), Some(parent_entity)) {
            continue;
        }
        let child_direction =
            direction_toward(&child_position, player_position).unwrap_or(MirDirection::Down);
        let object_id = allocate_runtime_monster_object_id(world);
        let child = world
            .spawn((
                WorldObject,
                Monster,
                ObjectId(object_id),
                DisplayName::literal(template.monster_name.clone()),
                Position(child_position),
                Facing(child_direction),
                MonsterAgent {
                    image: template.monster_image,
                    dead: false,
                    patrol_origin: player_position.clone(),
                    ai: template.monster_ai,
                    disposition: WorldEntityDisposition::Hostile,
                    hostile_to_player: true,
                    tracking_player: true,
                    view_range: i32::from(template.monster_view_range),
                    can_wander: crystal_respawn_can_wander(template.monster_hp),
                    move_interval_ticks: crystal_speed_to_ticks(template.monster_move_speed),
                    attack_interval_ticks: crystal_speed_to_ticks(template.monster_attack_speed),
                    next_move_tick: tick + 1,
                    next_attack_tick: tick + 1,
                    route: Vec::new(),
                    route_index: 0,
                    route_waiting: false,
                    next_route_tick: tick + 1,
                },
                MonsterAiState {
                    hidden: false,
                    extra: false,
                    extra_byte: 0,
                    mode: false,
                    next_state_tick: tick + 2,
                },
                MonsterVitals {
                    hp: template.monster_hp.max(1),
                    max_hp: template.monster_hp.max(1),
                },
                MonsterCombatStats {
                    agility: template.monster_agility,
                },
                SummonedMonster {
                    summoner_object_id: parent_object_id,
                    visible_extra: false,
                    expire_tick: None,
                    require_summoner_within: None,
                    despawn_tick_after_death: None,
                    totem_master_object_id: None,
                    max_minions: Some(3),
                },
            ))
            .id();
        if let Some(object_id) = entity_object_id(world, child) {
            packets.push(ServerPacket::ObjectShow { object_id });
        }
    }
}

pub(in crate::runtime) fn trap_rock_visible_destination(
    world: &World,
    entity: Entity,
    player_position: &Point,
    tick: u64,
) -> Option<Point> {
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let directions = [
        MirDirection::Up,
        MirDirection::Right,
        MirDirection::Down,
        MirDirection::Left,
    ];
    let start = trap_rock_spawn_corner_index(tick, object_id);
    let len = directions.len();
    let fallback = offset_point(player_position, directions[start], 1);
    Some(
        directions
            .into_iter()
            .cycle()
            .skip(start)
            .take(len)
            .map(|direction| offset_point(player_position, direction, 1))
            .find(|point| can_occupy(world, point.clone(), Some(entity)))
            .unwrap_or(fallback),
    )
}

#[cfg(test)]
pub(in crate::runtime) fn trap_rock_spawn_corner_direction(
    tick: u64,
    object_id: u32,
) -> MirDirection {
    match trap_rock_spawn_corner_index(tick, object_id) {
        0 => MirDirection::Up,
        1 => MirDirection::Right,
        2 => MirDirection::Down,
        _ => MirDirection::Left,
    }
}

pub(in crate::runtime) fn trap_rock_spawn_corner_index(tick: u64, object_id: u32) -> usize {
    ((tick ^ u64::from(object_id).wrapping_mul(0x9E37_79B9)) % 4) as usize
}

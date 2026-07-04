// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::CrystalRespawnTemplate;
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    entity_object_id, Facing, MonsterAgent, MonsterAiState, ObjectId, Position, SummonedMonster,
};
use super::super::monsters::*;
use super::super::movement::*;

pub(in crate::runtime) fn monster_step_toward_with_fallback(
    world: &World,
    entity: Entity,
    position: &Point,
    target: &Point,
    tick: u64,
    salt: usize,
) -> Option<(Point, MirDirection)> {
    let base_direction = direction_toward(position, target)?;
    let prefer_next = deterministic_roll(tick, entity.index() as usize, salt, 2) == 0;

    let current_distance = tile_distance(position, target);
    let mut sidestep: Option<(Point, MirDirection)> = None;

    for step in 0..8_i32 {
        let offset = if step == 0 {
            0
        } else if prefer_next {
            step
        } else {
            -step
        };
        let direction = rotated_direction(base_direction, offset);
        let destination = offset_point(position, direction, 1);
        if can_occupy(world, destination.clone(), Some(entity)) {
            // Greedy step that actually closes the gap: keep Crystal's cheap,
            // deterministic behaviour for the common open-field case.
            if tile_distance(&destination, target) < current_distance {
                return Some((destination, direction));
            }
            // Otherwise it's a sidestep/stall (a blocker sits between us and the
            // target). Remember the first such option but prefer a real route.
            sidestep.get_or_insert((destination, direction));
        }
    }

    // Greedy could not make progress toward the target — a wall or other
    // obstacle is in the way. Route around it with A* so monsters/heroes chase
    // through doorways and around corners instead of grinding into the wall.
    if let Some(routed) = monster_pathfind_step(world, entity, position, target) {
        return Some(routed);
    }

    sidestep
}

/// A* next-step for a chasing monster/hero. Routes toward a tile *adjacent* to
/// `target` (the target tile itself is typically occupied by the player/monster
/// being chased), returning the first tile of the shortest route together with
/// the facing for that step, or `None` when no route exists within the
/// pathfinder's bounds.
fn monster_pathfind_step(
    world: &World,
    entity: Entity,
    position: &Point,
    target: &Point,
) -> Option<(Point, MirDirection)> {
    let path = super::super::pathfind::find_path_adjacent(world, position, target, Some(entity))?;
    let next = path.into_iter().next()?;
    let direction = direction_toward(position, &next)?;
    Some((next, direction))
}

pub(in crate::runtime) fn summoned_monster_entity_target(
    world: &World,
    monster_entities: &[Entity],
    attacker: Entity,
    attacker_position: &Point,
    attacker_agent: &MonsterAgent,
) -> Option<Entity> {
    let attack_range = attacker_agent
        .view_range
        .max(monster_attack_range(attacker_agent));
    let attacker_is_friendly = !attacker_agent.hostile_to_player;
    let mut best: Option<(i32, i32, i32, u32, Entity)> = None;

    for candidate in monster_entities {
        if *candidate == attacker {
            continue;
        }

        let entry = world.entity(*candidate);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }

        let target_is_friendly = !target_agent.hostile_to_player;
        if attacker_is_friendly == target_is_friendly {
            continue;
        }

        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        let distance = tile_distance(attacker_position, &target_position);
        if distance > attack_range {
            continue;
        }

        let object_id = entry
            .get::<ObjectId>()
            .map(|value| value.0)
            .unwrap_or_default();
        let candidate_key = (
            distance,
            target_position.y,
            target_position.x,
            object_id,
            *candidate,
        );

        if best.map_or(true, |current| candidate_key < current) {
            best = Some(candidate_key);
        }
    }

    best.map(|(_, _, _, _, entity)| entity)
}

pub(in crate::runtime) fn summon_attack_damage(
    attacker_name: &str,
    attacker_agent: &MonsterAgent,
    target_distance: i32,
) -> i32 {
    match attacker_agent.ai {
        63 => crystal_monster_attack_damage(attacker_name),
        60 => crystal_monster_attack_damage(attacker_name),
        61 => crystal_monster_attack_damage(attacker_name),
        18 => crystal_monster_attack_damage(attacker_name),
        _ if attacker_agent.ai == 62 => 0,
        _ => crystal_monster_attack_damage(attacker_name),
    }
    .max(if attacker_agent.ai == 61 && target_distance > 0 {
        1
    } else {
        0
    })
}

pub(in crate::runtime) fn hostile_monster_summon_target(
    world: &World,
    monster_entities: &[Entity],
    attacker: Entity,
    attacker_position: &Point,
    attacker_agent: &MonsterAgent,
) -> Option<Entity> {
    if !attacker_agent.hostile_to_player {
        return None;
    }

    let attack_range = attacker_agent
        .view_range
        .max(monster_attack_range(attacker_agent));
    let mut best: Option<(i32, i32, i32, i32, i32, u32, Entity)> = None;

    for candidate in monster_entities {
        if *candidate == attacker {
            continue;
        }

        let entry = world.entity(*candidate);
        if entry.get::<SummonedMonster>().is_none() {
            continue;
        }
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead
            || target_agent.hostile_to_player
            || is_hidden_or_sleeping_target(target_agent, &target_ai_state)
        {
            continue;
        }

        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        let distance = tile_distance(attacker_position, &target_position);
        if distance > attack_range {
            continue;
        }

        let object_id = entry
            .get::<ObjectId>()
            .map(|value| value.0)
            .unwrap_or_default();
        let trap_priority = if target_agent.ai == 255 { 0 } else { 1 };
        let is_summoned_priority = if entry.get::<SummonedMonster>().is_some() {
            0
        } else {
            1
        };
        let candidate_key = (
            trap_priority,
            is_summoned_priority,
            distance,
            target_position.y,
            target_position.x,
            object_id,
            *candidate,
        );

        if best.map_or(true, |current| candidate_key < current) {
            best = Some(candidate_key);
        }
    }

    best.map(|(_, _, _, _, _, _, entity)| entity)
}

pub(in crate::runtime) fn shinsu_line_monster_targets(
    world: &World,
    monster_entities: &[Entity],
    attacker: Entity,
    attacker_position: &Point,
    direction: MirDirection,
    attacker_agent: &MonsterAgent,
) -> Vec<Entity> {
    let attacker_is_friendly = !attacker_agent.hostile_to_player;
    let line_tiles = [
        offset_point(attacker_position, direction, 1),
        offset_point(attacker_position, direction, 2),
    ];
    let mut targets = Vec::new();

    for candidate in monster_entities {
        if *candidate == attacker {
            continue;
        }

        let entry = world.entity(*candidate);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_is_friendly == !target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if line_tiles.contains(&target_position) {
            targets.push(*candidate);
        }
    }

    targets
}

pub(in crate::runtime) fn forward_line_opposing_monster_targets(
    world: &World,
    monster_entities: &[Entity],
    attacker: Entity,
    attacker_position: &Point,
    direction: MirDirection,
    attacker_agent: &MonsterAgent,
    distance: i32,
) -> Vec<Entity> {
    let line_tiles: Vec<Point> = (1..=distance)
        .map(|amount| offset_point(attacker_position, direction, amount))
        .collect();
    let attacker_is_friendly = !attacker_agent.hostile_to_player;
    let mut targets = Vec::new();

    for candidate in monster_entities {
        if *candidate == attacker {
            continue;
        }

        let entry = world.entity(*candidate);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_is_friendly == !target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if line_tiles.contains(&target_position) {
            targets.push(*candidate);
        }
    }

    targets
}

pub(in crate::runtime) fn tucson_mage_wide_line_points(
    attacker_position: &Point,
    direction: MirDirection,
) -> Vec<Point> {
    let forward = offset_point(attacker_position, direction, 1);
    let mut points = vec![forward.clone()];

    for offset in -1..=1 {
        let line_direction = rotated_direction(direction, offset);
        for amount in 1..=2 {
            points.push(offset_point(&forward, line_direction, amount));
        }
    }

    points
}

pub(in crate::runtime) fn tucson_mage_wide_line_opposing_monster_targets(
    world: &World,
    monster_entities: &[Entity],
    attacker: Entity,
    attacker_position: &Point,
    direction: MirDirection,
    attacker_agent: &MonsterAgent,
) -> Vec<Entity> {
    let target_points = tucson_mage_wide_line_points(attacker_position, direction);
    let attacker_is_friendly = !attacker_agent.hostile_to_player;
    let mut targets = Vec::new();

    for candidate in monster_entities {
        if *candidate == attacker {
            continue;
        }

        let entry = world.entity(*candidate);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_is_friendly == !target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if target_points.contains(&target_position) {
            targets.push(*candidate);
        }
    }

    targets
}

pub(in crate::runtime) fn manectric_claw_cone_points(
    attacker_position: &Point,
    direction: MirDirection,
) -> Vec<Point> {
    let mut points = Vec::new();

    for offset in -1..=1 {
        let start = offset_point(attacker_position, rotated_direction(direction, offset), 1);
        for amount in 0..=2 {
            points.push(offset_point(&start, direction, amount));
        }
    }

    points
}

pub(in crate::runtime) fn manectric_claw_cone_opposing_monster_targets(
    world: &World,
    monster_entities: &[Entity],
    attacker: Entity,
    attacker_position: &Point,
    direction: MirDirection,
    attacker_agent: &MonsterAgent,
) -> Vec<Entity> {
    let target_points = manectric_claw_cone_points(attacker_position, direction);
    let attacker_is_friendly = !attacker_agent.hostile_to_player;
    let mut targets = Vec::new();

    for candidate in monster_entities {
        if *candidate == attacker {
            continue;
        }

        let entry = world.entity(*candidate);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_is_friendly == !target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if target_points.contains(&target_position) {
            targets.push(*candidate);
        }
    }

    targets
}

pub(in crate::runtime) fn nearby_opposing_monster_targets(
    world: &World,
    monster_entities: &[Entity],
    attacker: Entity,
    attacker_position: &Point,
    attacker_agent: &MonsterAgent,
    radius: i32,
) -> Vec<Entity> {
    let attacker_is_friendly = !attacker_agent.hostile_to_player;
    let mut targets = Vec::new();

    for candidate in monster_entities {
        if *candidate == attacker {
            continue;
        }

        let entry = world.entity(*candidate);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_is_friendly == !target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if tile_distance(attacker_position, &target_position) <= radius {
            targets.push(*candidate);
        }
    }

    targets
}

pub(in crate::runtime) fn halfmoon_opposing_monster_targets(
    world: &World,
    monster_entities: &[Entity],
    attacker: Entity,
    attacker_position: &Point,
    direction: MirDirection,
    attacker_agent: &MonsterAgent,
) -> Vec<Entity> {
    let halfmoon_points = [-1, 0, 1, 2].map(|offset| {
        let direction = rotated_direction(direction, offset);
        offset_point(attacker_position, direction, 1)
    });
    let attacker_is_friendly = !attacker_agent.hostile_to_player;
    let mut targets = Vec::new();

    for candidate in monster_entities {
        if *candidate == attacker {
            continue;
        }

        let entry = world.entity(*candidate);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_is_friendly == !target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if halfmoon_points.contains(&target_position) {
            targets.push(*candidate);
        }
    }

    targets
}

pub(in crate::runtime) fn monster_prefers_monster_target(
    world: &World,
    attacker: Entity,
    attacker_agent: &MonsterAgent,
) -> bool {
    world
        .entity(attacker)
        .get::<SummonedMonster>()
        .map(|_| !attacker_agent.hostile_to_player)
        .unwrap_or(false)
        || matches!(attacker_agent.ai, 6 | 58 | 113)
}

pub(in crate::runtime) fn spawn_monster_slave_wave(
    world: &mut World,
    entity: Entity,
    object_id: u32,
    position: &Point,
    direction: MirDirection,
    target_entity: Option<Entity>,
    agent: &MonsterAgent,
    tick: u64,
    slave_names: &[&str],
    max_slaves: usize,
    batch_size: usize,
) {
    let spawn_count = max_slaves
        .saturating_sub(active_summoned_monster_count(world, object_id))
        .min(batch_size);
    if spawn_count == 0 {
        return;
    }

    let front = offset_point(position, direction, 1);
    for index in 0..spawn_count {
        let name_index = deterministic_roll(
            tick,
            entity.index() as usize,
            index,
            slave_names.len() as u64,
        ) as usize;
        let Some(template) = crystal_dynamic_monster_template(slave_names[name_index]) else {
            continue;
        };
        let summon_metadata = SummonedMonster {
            summoner_object_id: object_id,
            visible_extra: false,
            expire_tick: None,
            require_summoner_within: None,
            despawn_tick_after_death: None,
            totem_master_object_id: None,
            max_minions: Some(max_slaves),
        };
        let template = CrystalRespawnTemplate {
            location: front.clone(),
            ..template
        };
        if spawn_runtime_monster(
            world,
            &template,
            front.clone(),
            direction,
            target_entity,
            Some(summon_metadata),
            Some(agent.hostile_to_player),
            Some(agent.disposition),
            combat_delay_ticks(2_000),
        )
        .is_none()
        {
            let fallback_template = CrystalRespawnTemplate {
                location: position.clone(),
                ..template
            };
            let _ = spawn_runtime_monster(
                world,
                &fallback_template,
                position.clone(),
                direction,
                target_entity,
                Some(summon_metadata),
                Some(agent.hostile_to_player),
                Some(agent.disposition),
                combat_delay_ticks(2_000),
            );
        }
    }
}

pub(in crate::runtime) fn first_occupiable_point_near(
    world: &World,
    origin: &Point,
    radius: i32,
    ignore_entity: Option<Entity>,
) -> Option<Point> {
    for distance in 0..=radius.max(0) {
        for y in origin.y - distance..=origin.y + distance {
            for x in origin.x - distance..=origin.x + distance {
                let point = Point { x, y };
                if tile_distance(origin, &point) != distance {
                    continue;
                }
                if can_occupy(world, point.clone(), ignore_entity) {
                    return Some(point);
                }
            }
        }
    }

    None
}

pub(in crate::runtime) fn active_monster_count_by_ai_near(
    world: &World,
    ai: u8,
    origin: &Point,
    radius: i32,
) -> usize {
    #[allow(deprecated)]
    world
        .iter_entities()
        .filter(|entity| {
            entity
                .get::<MonsterAgent>()
                .map(|agent| !agent.dead && agent.ai == ai)
                .unwrap_or(false)
                && entity
                    .get::<Position>()
                    .map(|position| tile_distance(origin, &position.0) <= radius)
                    .unwrap_or(false)
        })
        .count()
}

pub(in crate::runtime) fn object_sit_down_packet(
    world: &World,
    entity: Entity,
    position: &Point,
    sitting: bool,
) -> Option<ServerPacket> {
    let object_id = entity_object_id(world, entity)?;
    let direction = world.entity(entity).get::<Facing>()?.0;
    Some(ServerPacket::ObjectSitDown {
        movement: ObjectMovement {
            object_id,
            position: position.clone(),
            direction,
        },
        sitting,
    })
}

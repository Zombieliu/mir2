// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::CrystalRespawnTemplate;
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::components::{
    entity_facing, entity_name, entity_object_id, player_entity, DisplayName, Facing, Monster,
    MonsterAgent, MonsterCombatStats, MonsterVitals, ObjectId, Position, WorldObject, YimoogiState,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;
use super::super::resources::{runtime_tick, MapRuntimeResource};

pub(in crate::runtime) fn update_yimoogi_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return false;
    }

    let mut state = world
        .entity(entity)
        .get::<YimoogiState>()
        .copied()
        .unwrap_or_else(|| initial_yimoogi_state(tick));
    let original_state = state;
    let has_player_target = agent.tracking_player
        || (agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1));

    if !state.is_child && !state.final_teleport && yimoogi_should_final_teleport(world, entity) {
        if let Some(destination) = yimoogi_final_teleport_destination(world, entity, position, tick)
        {
            let old_position = position.clone();
            let direction = entity_facing(world, entity).unwrap_or(MirDirection::Down);
            state.final_teleport = true;
            agent.tracking_player = false;
            agent.next_move_tick = tick + 1;
            agent.next_attack_tick = tick + 1;
            world.entity_mut(entity).insert((
                Position(destination.clone()),
                Facing(direction),
                state,
            ));

            if let Some(object_id) = entity_object_id(world, entity) {
                packets.push(ServerPacket::ObjectWalk {
                    movement: ObjectMovement {
                        object_id,
                        position: destination,
                        direction,
                    },
                });
            }

            spawn_yimoogi_white_serpents(
                world,
                &old_position,
                direction,
                has_player_target.then(|| player_entity(world)).flatten(),
                agent,
                tick,
            );
            return true;
        }
    }

    if !state.is_child && !state.child_spawned && tick > state.spawn_tick {
        let direction = entity_facing(world, entity).unwrap_or(MirDirection::Down);
        if let Some(packet) = monster_typed_attack_packet(world, entity, position, direction, 2) {
            packets.push(packet);
        }
        if let Some(child_object_id) = spawn_yimoogi_child(
            world,
            entity,
            position,
            direction,
            has_player_target.then(|| player_entity(world)).flatten(),
            agent,
            tick,
        ) {
            state.child_spawned = true;
            state.sister_object_id = Some(child_object_id);
        }
        agent.next_move_tick = tick + 1;
        agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
        world.entity_mut(entity).insert(state);
        return true;
    }

    if state != original_state {
        world.entity_mut(entity).insert(state);
    }
    false
}

pub(in crate::runtime) fn yimoogi_should_final_teleport(world: &World, entity: Entity) -> bool {
    let Some(vitals) = world.entity(entity).get::<MonsterVitals>() else {
        return false;
    };
    vitals.hp <= vitals.max_hp / 10
}

pub(in crate::runtime) fn yimoogi_final_teleport_destination(
    world: &World,
    entity: Entity,
    position: &Point,
    tick: u64,
) -> Option<Point> {
    let bounds = world.resource::<MapRuntimeResource>().map_region_bounds;
    let width = u64::try_from(bounds.max_x - bounds.min_x + 1).ok()?;
    let height = u64::try_from(bounds.max_y - bounds.min_y + 1).ok()?;

    for attempt in 0..YIMOOGI_FINAL_TELEPORT_ATTEMPTS {
        let x = bounds.min_x
            + i32::try_from(deterministic_roll(
                tick + attempt as u64,
                entity.index() as usize,
                attempt * 2,
                width,
            ))
            .ok()?;
        let y = bounds.min_y
            + i32::try_from(deterministic_roll(
                tick + attempt as u64,
                entity.index() as usize,
                attempt * 2 + 1,
                height,
            ))
            .ok()?;
        let candidate = Point { x, y };
        if candidate != *position && can_occupy(world, candidate.clone(), Some(entity)) {
            return Some(candidate);
        }
    }

    None
}

pub(in crate::runtime) fn spawn_yimoogi_child(
    world: &mut World,
    parent_entity: Entity,
    position: &Point,
    direction: MirDirection,
    target_entity: Option<Entity>,
    agent: &MonsterAgent,
    tick: u64,
) -> Option<u32> {
    let parent_object_id = entity_object_id(world, parent_entity)?;
    let name = entity_name(world, parent_entity).unwrap_or_else(|| "Yimoogi".to_string());
    let template = crystal_dynamic_monster_template(&name)?;
    let front = offset_point(position, direction, 1);
    let spawn_position = if can_occupy(world, front.clone(), Some(parent_entity)) {
        front
    } else {
        position.clone()
    };
    let child = spawn_yimoogi_runtime_monster(
        world,
        &template,
        spawn_position,
        direction,
        target_entity,
        agent,
        YIMOOGI_CHILD_ACTIVATION_DELAY_TICKS,
    )?;
    let child_object_id = entity_object_id(world, child)?;
    let mut entry = world.entity_mut(child);
    entry.insert(YimoogiState {
        spawn_tick: tick + YIMOOGI_CHILD_SPAWN_DELAY_TICKS,
        child_spawned: true,
        is_child: true,
        final_teleport: false,
        sister_object_id: Some(parent_object_id),
    });
    Some(child_object_id)
}

pub(in crate::runtime) fn spawn_yimoogi_runtime_monster(
    world: &mut World,
    template: &CrystalRespawnTemplate,
    position: Point,
    direction: MirDirection,
    target_entity: Option<Entity>,
    parent_agent: &MonsterAgent,
    activation_delay_ticks: u64,
) -> Option<Entity> {
    let tick = runtime_tick(world);
    let mut agent = MonsterAgent {
        image: template.monster_image,
        dead: false,
        patrol_origin: position.clone(),
        ai: template.monster_ai,
        disposition: parent_agent.disposition,
        hostile_to_player: parent_agent.hostile_to_player,
        tracking_player: false,
        view_range: i32::from(template.monster_view_range),
        can_wander: crystal_respawn_can_wander(template.monster_hp),
        move_interval_ticks: crystal_speed_to_ticks(template.monster_move_speed),
        attack_interval_ticks: crystal_speed_to_ticks(template.monster_attack_speed),
        next_move_tick: tick + activation_delay_ticks,
        next_attack_tick: tick + activation_delay_ticks,
        route: Vec::new(),
        route_index: 0,
        route_waiting: false,
        next_route_tick: tick + activation_delay_ticks,
    };
    if target_entity.is_some() && agent.hostile_to_player {
        agent.tracking_player = true;
    }

    let object_id = allocate_runtime_monster_object_id(world);
    let mut entity = world.spawn((
        WorldObject,
        Monster,
        ObjectId(object_id),
        DisplayName::literal(template.monster_name.clone()),
        Position(position.clone()),
        Facing(direction),
        agent,
        initial_monster_ai_state_for_object(
            template.monster_ai,
            tick + activation_delay_ticks,
            object_id,
        ),
        MonsterVitals {
            hp: template.monster_hp.max(1),
            max_hp: template.monster_hp.max(1),
        },
        MonsterCombatStats {
            agility: template.monster_agility,
        },
    ));
    if template.monster_ai == 36 {
        entity.insert(initial_yimoogi_state(tick + activation_delay_ticks));
    }
    Some(entity.id())
}

pub(in crate::runtime) fn spawn_yimoogi_white_serpents(
    world: &mut World,
    position: &Point,
    direction: MirDirection,
    target_entity: Option<Entity>,
    agent: &MonsterAgent,
    _tick: u64,
) {
    let Some(template) = crystal_dynamic_monster_template(YIMOOGI_WHITE_SERPENT_NAME) else {
        return;
    };

    for _ in 0..YIMOOGI_FINAL_WHITE_SERPENT_COUNT {
        let _ = spawn_yimoogi_runtime_monster(
            world,
            &template,
            position.clone(),
            direction,
            target_entity,
            agent,
            YIMOOGI_CHILD_ACTIVATION_DELAY_TICKS,
        );
    }
}

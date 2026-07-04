// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    entity_facing, entity_name, entity_object_id, Facing, Monster, MonsterAgent, MonsterAiState,
    Position,
};
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn update_thunder_element_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !monster_can_attack(agent, ai_state) {
        return false;
    }
    if !monster_in_attack_range(agent, position, player_position) {
        return false;
    }
    if !agent.tracking_player
        && !(agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1))
    {
        return false;
    }
    if tick < agent.next_attack_tick {
        return false;
    }

    let Some(attacker_id) = entity_object_id(world, entity) else {
        return false;
    };
    let monster_name = entity_name(world, entity).unwrap_or_else(|| "ThunderElement".to_string());
    let mut attack_position = position.clone();
    if deterministic_chance_roll(tick, attacker_id, 490, 3) {
        let target_x_offset = deterministic_roll(tick, entity.index() as usize, 491, 3) as i32 - 1;
        let target_y_offset = deterministic_roll(tick, entity.index() as usize, 492, 3) as i32 - 1;
        let move_target = Point {
            x: player_position.x + target_x_offset,
            y: player_position.y + target_y_offset,
        };
        if let Some((destination, movement_direction)) =
            monster_step_toward_with_fallback(world, entity, position, &move_target, tick, 49)
        {
            attack_position = destination.clone();
            agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
            world.entity_mut(entity).insert((
                Position(destination.clone()),
                Facing(movement_direction),
                agent.clone(),
            ));
            packets.push(ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: attacker_id,
                    position: destination,
                    direction: movement_direction,
                },
            });
        }
    }

    let attack_direction = direction_toward(&attack_position, player_position)
        .or_else(|| entity_facing(world, entity))
        .unwrap_or(MirDirection::Up);
    agent.tracking_player = true;
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    world
        .entity_mut(entity)
        .insert((Facing(attack_direction), agent.clone()));

    let damage = monster_player_attack_damage(
        world,
        &monster_name,
        agent,
        &attack_position,
        player_position,
    );
    if damage <= 0 {
        return true;
    }

    let due_tick = tick + monster_attack_delay_ticks(agent, &attack_position, player_position);
    let due_packet =
        monster_typed_attack_packet(world, entity, &attack_position, attack_direction, 0);
    schedule_damage_to_player_with_effect_and_due_packet(
        world,
        due_tick,
        attacker_id,
        monster_name,
        damage,
        None,
        due_packet,
    );

    #[allow(deprecated)]
    let monster_entities: Vec<Entity> = world
        .iter_entities()
        .filter_map(|entity| entity.contains::<Monster>().then_some(entity.id()))
        .collect();
    for area_target in nearby_opposing_monster_targets(
        world,
        &monster_entities,
        entity,
        &attack_position,
        agent,
        2,
    ) {
        schedule_damage_to_monster(
            world,
            due_tick,
            attacker_id,
            area_target,
            damage,
            None,
            None,
        );
    }

    true
}

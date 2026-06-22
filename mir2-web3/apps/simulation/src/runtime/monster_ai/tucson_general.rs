// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectSpellInfo, Point, ServerPacket, Spell};

use super::super::combat::*;
use super::super::components::{
    entity_name, entity_object_id, entity_position, player_entity, Facing, Monster, MonsterAgent,
    MonsterAiState,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn update_tucson_general_state(
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
    if tick < agent.next_attack_tick || tick < ai_state.next_state_tick {
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

    let Some(player) = player_entity(world) else {
        return false;
    };
    let Some(direction) = direction_toward(position, player_position) else {
        return false;
    };
    if let Some(current_direction) = world.entity(entity).get::<Facing>().map(|facing| facing.0) {
        if current_direction != direction {
            world.entity_mut(entity).insert(Facing(direction));
        }
    }
    if let Some(packet) = monster_typed_ranged_attack_packet(
        world,
        entity,
        position,
        direction,
        player,
        player_position,
        0,
    ) {
        packets.push(packet);
    }
    schedule_tucson_general_rocks(world, entity, agent, position, tick);
    agent.tracking_player = true;
    agent.next_attack_tick = tick + TUCSON_GENERAL_RAGE_ATTACK_PAUSE_TICKS;
    ai_state.next_state_tick = tick + TUCSON_GENERAL_RAGE_COOLDOWN_TICKS;
    true
}

pub(in crate::runtime) fn schedule_tucson_general_rocks(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
    tick: u64,
) {
    let Some(attacker_id) = entity_object_id(world, entity) else {
        return;
    };
    let attacker_name = entity_name(world, entity).unwrap_or_else(|| "TucsonGeneral".to_string());
    let target_positions = tucson_general_rock_target_positions(world, entity, agent, position);
    let damage = crystal_monster_raw_attack_damage(&attacker_name);
    let view_range = agent.view_range.max(1);

    for rock_index in 0..TUCSON_GENERAL_ROCK_COUNT {
        let roll_tick = tick + u64::try_from(rock_index).expect("rock index should fit u64");
        let target_position = if !target_positions.is_empty()
            && deterministic_chance_roll(
                roll_tick,
                attacker_id,
                TUCSON_GENERAL_ROCK_TARGET_SALT_BASE
                    + u64::try_from(rock_index).expect("rock index should fit u64"),
                3,
            ) {
            let target_index = deterministic_roll(
                roll_tick,
                attacker_id as usize,
                TUCSON_GENERAL_ROCK_TARGET_INDEX_SALT_BASE + rock_index,
                target_positions.len() as u64,
            ) as usize;
            target_positions.get(target_index).cloned()
        } else {
            tucson_general_scattered_rock_position(
                world,
                position,
                view_range,
                roll_tick,
                attacker_id,
                rock_index,
            )
        };
        let Some(rock_position) = target_position else {
            continue;
        };
        if rock_position.x == position.x || rock_position.y == position.y {
            continue;
        }

        let start_ms = deterministic_roll(
            roll_tick,
            attacker_id as usize,
            TUCSON_GENERAL_ROCK_DELAY_SALT_BASE + rock_index,
            5_000,
        );
        let spawn_tick = tick + combat_delay_ticks(start_ms);
        let impact_tick = spawn_tick + combat_delay_ticks(1_000);
        let spell_packet = ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id: allocate_runtime_monster_object_id(world),
                location: rock_position.clone(),
                spell: Spell::TucsonGeneralRock,
                direction: MirDirection::Up,
                param: false,
            },
        };
        queue_due_packet(world, spawn_tick, spell_packet);

        if damage <= 0 {
            continue;
        }
        if agent.hostile_to_player {
            if let Some(player) = player_entity(world) {
                if entity_position(world, player)
                    .map(|player_position| player_position == rock_position)
                    .unwrap_or(false)
                {
                    schedule_damage_to_player(
                        world,
                        impact_tick,
                        attacker_id,
                        attacker_name.clone(),
                        damage,
                    );
                }
            }
        }

        let monster_entities: Vec<Entity> = world
            .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
            .iter(world)
            .collect();
        for target in nearby_opposing_monster_targets(
            world,
            &monster_entities,
            entity,
            &rock_position,
            agent,
            0,
        ) {
            schedule_damage_to_monster(world, impact_tick, attacker_id, target, damage, None, None);
        }
    }
}

pub(in crate::runtime) fn tucson_general_rock_target_positions(
    world: &World,
    entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
) -> Vec<Point> {
    let mut targets = Vec::new();
    if agent.hostile_to_player {
        if let Some(player) = player_entity(world) {
            if let Some(player_position) = entity_position(world, player) {
                if tile_distance(position, &player_position) <= 10 {
                    targets.push(player_position);
                }
            }
        }
    }

    #[allow(deprecated)]
    let monster_entities: Vec<Entity> = world
        .iter_entities()
        .filter_map(|entity| entity.contains::<Monster>().then_some(entity.id()))
        .collect();
    for target in
        nearby_opposing_monster_targets(world, &monster_entities, entity, position, agent, 10)
    {
        if let Some(target_position) = entity_position(world, target) {
            targets.push(target_position);
        }
    }
    targets
}

pub(in crate::runtime) fn tucson_general_scattered_rock_position(
    world: &World,
    position: &Point,
    view_range: i32,
    roll_tick: u64,
    attacker_id: u32,
    rock_index: usize,
) -> Option<Point> {
    let spread = u64::try_from(view_range * 2 + 1).expect("view range should fit u64");
    for attempt in 0..8usize {
        let x_roll = deterministic_roll(
            roll_tick + attempt as u64,
            attacker_id as usize,
            TUCSON_GENERAL_ROCK_SCATTER_X_SALT_BASE + rock_index + attempt,
            spread,
        ) as i32;
        let y_roll = deterministic_roll(
            roll_tick + attempt as u64,
            attacker_id as usize,
            TUCSON_GENERAL_ROCK_SCATTER_Y_SALT_BASE + rock_index + attempt,
            spread,
        ) as i32;
        let location = clamp_to_map_region(
            world,
            Point {
                x: position.x + x_roll - view_range,
                y: position.y + y_roll - view_range,
            },
        );
        if location.x != position.x && location.y != position.y {
            return Some(location);
        }
    }
    None
}

// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    entity_facing, entity_name, entity_object_id, entity_position, player_entity,
    GeneralMeowMeowState, Monster, MonsterAgent, MonsterAiState, MonsterVitals,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;
use super::super::packets::*;
use super::super::resources::current_language;

use super::common::*;

pub(in crate::runtime) fn update_general_meow_meow_state(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    if agent.dead {
        return;
    }

    let mut state = world
        .entity(entity)
        .get::<GeneralMeowMeowState>()
        .copied()
        .unwrap_or_else(|| initial_general_meow_meow_state(tick));
    let original_state = state;
    let was_shield_active = ai_state.mode;
    let has_player_target = agent.tracking_player
        || (agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1));

    if has_player_target && tick > state.next_slave_spawn_tick {
        if let Some(object_id) = entity_object_id(world, entity) {
            let target_entity = player_entity(world);
            let direction = direction_toward(position, player_position)
                .or_else(|| entity_facing(world, entity))
                .unwrap_or(MirDirection::Up);
            spawn_monster_slave_wave(
                world,
                entity,
                object_id,
                position,
                direction,
                target_entity,
                agent,
                tick,
                &GENERAL_MEOW_MEOW_SLAVE_NAMES,
                GENERAL_MEOW_MEOW_MAX_SLAVES,
                GENERAL_MEOW_MEOW_SLAVE_SPAWN_COUNT,
            );
        }
        state.next_slave_spawn_tick = tick + GENERAL_MEOW_MEOW_SLAVE_SPAWN_INTERVAL_TICKS;
    }

    let can_use_shield = has_player_target
        && monster_can_attack(agent, ai_state)
        && monster_in_attack_range(agent, position, player_position);
    let in_shield_phase = can_use_shield
        && world
            .entity(entity)
            .get::<MonsterVitals>()
            .map(|vitals| general_meow_meow_shield_phase(vitals.hp, vitals.max_hp))
            .unwrap_or(false);

    if in_shield_phase {
        state.shield_until_tick = tick + GENERAL_MEOW_MEOW_SHIELD_DURATION_TICKS;
        if state.next_thunder_tick == 0 || tick > state.next_thunder_tick {
            if cast_general_meow_meow_mass_thunder(
                world,
                entity,
                agent,
                position,
                player_position,
                tick,
                packets,
            ) {
                state.next_thunder_tick = tick
                    + GENERAL_MEOW_MEOW_THUNDER_MIN_COOLDOWN_TICKS
                    + deterministic_roll(
                        tick,
                        entity.index() as usize,
                        1230,
                        GENERAL_MEOW_MEOW_THUNDER_RANDOM_COOLDOWN_TICKS,
                    );
            }
        }
    }

    ai_state.mode = state.shield_until_tick > tick;
    if state != original_state {
        world.entity_mut(entity).insert(state);
    }
    if ai_state.mode != was_shield_active {
        if let Some((_, bundle)) =
            visible_object_bundle_for_entity(world, entity, current_language(world))
        {
            if matches!(bundle.spawn_packet, ServerPacket::ObjectMonster { .. }) {
                packets.push(bundle.spawn_packet);
            }
        }
    }
}

pub(in crate::runtime) fn general_meow_meow_shield_phase(hp: i32, max_hp: i32) -> bool {
    let percent = hp.max(0) * 100 / max_hp.max(1);
    (70..=80).contains(&percent) || (40..=50).contains(&percent) || percent <= 20
}

pub(in crate::runtime) fn cast_general_meow_meow_mass_thunder(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let Some(attacker_id) = entity_object_id(world, entity) else {
        return false;
    };
    let monster_name = entity_name(world, entity).unwrap_or_else(|| "GeneralMeowMeow".to_string());
    let base_damage = crystal_monster_raw_magic_damage(&monster_name);
    let damage = if base_damage > 0 {
        let mitigation = crystal_player_rolled_armour(world);
        (base_damage - mitigation).max(1)
    } else {
        0
    };
    let due_tick = tick + GENERAL_MEOW_MEOW_THUNDER_SPAWN_DELAY_TICKS;
    let mut cast_count = 0;

    if agent.hostile_to_player {
        if player_entity(world).is_some() {
            let spell_packet = general_meow_meow_thunder_spell_packet(
                world,
                player_position.clone(),
                MirDirection::Up,
            );
            schedule_damage_to_player_with_effect_and_due_packet(
                world,
                due_tick,
                attacker_id,
                monster_name.clone(),
                damage,
                None,
                Some(spell_packet),
            );
            cast_count += 1;
        }
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    let targets = nearby_opposing_monster_targets(
        world,
        &monster_entities,
        entity,
        player_position,
        agent,
        12,
    );
    for target in targets {
        let Some(target_position) = entity_position(world, target) else {
            continue;
        };
        let spell_packet =
            general_meow_meow_thunder_spell_packet(world, target_position, MirDirection::Up);
        schedule_damage_to_monster_with_due_packet(
            world,
            due_tick,
            attacker_id,
            target,
            damage,
            None,
            None,
            Some(spell_packet),
        );
        cast_count += 1;
    }

    let _ = position;
    let _ = packets;
    cast_count > 0
}

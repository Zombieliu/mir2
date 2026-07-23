// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    entity_name, entity_object_id, Facing, Monster, MonsterAgent, MonsterAiState,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn update_kirin_ice_thrust_state(
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
    let distance = tile_distance(position, player_position);
    if distance != 3 {
        return false;
    }
    if !agent.tracking_player && !(agent.hostile_to_player && distance <= agent.view_range.max(1)) {
        return false;
    }
    if tick < agent.next_attack_tick || tick % 5 != 0 {
        return false;
    }

    let Some(attacker_id) = entity_object_id(world, entity) else {
        return false;
    };
    let monster_name = entity_name(world, entity).unwrap_or_else(|| "Lamia".to_string());
    let base_damage = crystal_monster_raw_magic_damage(&monster_name);
    if base_damage <= 0 {
        return false;
    }
    let Some(attack_direction) = direction_toward(position, player_position) else {
        return false;
    };
    let mitigation = crystal_player_rolled_armour(world);
    let damage = (base_damage - mitigation).max(1);
    let due_tick = tick + combat_delay_ticks(500);

    agent.tracking_player = true;
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    world
        .entity_mut(entity)
        .insert((Facing(attack_direction), agent.clone()));
    if let Some(packet) = monster_typed_attack_packet(world, entity, position, attack_direction, 2)
    {
        packets.push(packet);
    }

    schedule_damage_to_player_with_effect(
        world,
        due_tick,
        attacker_id,
        monster_name.clone(),
        damage,
        Some(PendingPlayerStatusEffect::SlowPoison {
            chance_denominator: KIRIN_SLOW_CHANCE_DENOMINATOR,
            duration_ticks: KIRIN_SLOW_DURATION_TICKS,
            salt: 186,
        }),
    );

    #[allow(deprecated)]
    let monster_entities: Vec<Entity> = world
        .iter_entities()
        .filter_map(|entity| entity.contains::<Monster>().then_some(entity.id()))
        .collect();
    for target in manectric_claw_cone_opposing_monster_targets(
        world,
        &monster_entities,
        entity,
        position,
        attack_direction,
        agent,
    ) {
        schedule_damage_to_monster(world, due_tick, attacker_id, target, damage, None, None);
    }

    true
}

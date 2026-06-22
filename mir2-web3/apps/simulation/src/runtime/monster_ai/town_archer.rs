// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    current_player_is_dead, entity_name, entity_object_id, player_entity, Facing, MonsterAgent,
    MonsterAiState,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;
use super::super::resources::PlayerRuntimeResource;

// Crystal `TownArcher` (AI 57): ranged guard that targets ONLY red-name
// players (`PKPoints >= 200`) within view, fires `ObjectRangeAttack`, and
// schedules projectile damage. Inert against monsters and non-PK players.
// Crystal/Server/MirObjects/Monsters/TownArcher.cs — FindTarget filters
// `playerob.PKPoints < 200`, Attack broadcasts ObjectRangeAttack +
// ProjectileAttack(GetAttackPower(MinDC, MaxDC)).
pub(in crate::runtime) fn update_town_archer_state(
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
    if tick < agent.next_attack_tick {
        return false;
    }

    let pk_points = world.resource::<PlayerRuntimeResource>().pk_points;
    if pk_points < CRYSTAL_RED_NAME_PK_POINTS {
        return false;
    }

    if current_player_is_dead(world) {
        return false;
    }

    if !monster_in_attack_range(agent, position, player_position) {
        return false;
    }

    let Some(player) = player_entity(world) else {
        return false;
    };
    let Some(attacker_id) = entity_object_id(world, entity) else {
        return false;
    };
    let Some(direction) = direction_toward(position, player_position) else {
        return false;
    };

    let monster_name = entity_name(world, entity).unwrap_or_default();
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

    let damage =
        monster_player_attack_damage(world, &monster_name, agent, position, player_position);
    if damage > 0 {
        schedule_damage_to_player(
            world,
            tick + monster_attack_delay_ticks(agent, position, player_position),
            attacker_id,
            monster_name,
            damage,
        );
    }

    agent.tracking_player = true;
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    true
}

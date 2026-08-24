// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    current_player_object_id, entity_facing, entity_object_id, player_entity, Facing, MonsterAgent,
    MonsterAiState, MonsterVitals, Position,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;
use super::super::packets::*;
use super::super::resources::current_language;

pub(in crate::runtime) fn update_great_fox_spirit_state(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if !ai_state.mode {
        set_guardian_rocks_active_near(world, position, true);
        ai_state.mode = true;
        world.entity_mut(entity).insert(*ai_state);
    }

    let Some(vitals) = world.entity(entity).get::<MonsterVitals>() else {
        return false;
    };
    if vitals.max_hp < 4 {
        return false;
    }

    let stage_size = (vitals.max_hp / 4).max(1);
    let stage = (4 - (vitals.hp.max(0) / stage_size)).clamp(0, 4) as u8;
    if stage > ai_state.extra_byte {
        ai_state.extra_byte = stage;
        world.entity_mut(entity).insert(*ai_state);
        if let Some((_, bundle)) =
            visible_object_bundle_for_entity(world, entity, current_language(world))
        {
            if matches!(bundle.spawn_packet, ServerPacket::ObjectMonster { .. }) {
                packets.push(bundle.spawn_packet);
            }
        }
    }

    try_great_fox_spirit_recall(
        world,
        entity,
        agent,
        ai_state,
        position,
        player_position,
        tick,
        packets,
    )
}

pub(in crate::runtime) fn try_great_fox_spirit_recall(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !monster_can_attack(agent, ai_state) {
        return false;
    }
    if tile_distance(position, player_position) <= 3
        || tile_distance(position, player_position) > 30
        || ai_state.next_state_tick > tick
        || (!agent.tracking_player && !agent.hostile_to_player)
    {
        return false;
    }

    let Some(attacker_id) = entity_object_id(world, entity) else {
        return false;
    };
    if !deterministic_chance_roll(tick, attacker_id, 502, 10) {
        return false;
    }

    ai_state.next_state_tick = tick + GREAT_FOX_SPIRIT_RECALL_COOLDOWN_TICKS;
    world.entity_mut(entity).insert(*ai_state);
    if deterministic_chance_roll(tick, attacker_id, 503, 4) {
        return false;
    }

    let Some(player) = player_entity(world) else {
        return false;
    };
    let Some(player_object_id) = current_player_object_id(world) else {
        return false;
    };
    // Crystal skips each recall candidate when its all-or-nothing
    // MagicResist roll succeeds. The ten-second recall cooldown has already
    // started at this point, matching `GreatFoxSpirit.ProcessTarget`.
    if crystal_player_magic_mitigated(world, 1) == 0 {
        return false;
    }
    let direction = rotated_direction(
        MirDirection::Up,
        deterministic_roll(tick, entity.index() as usize, 50, 7) as i32,
    );
    let candidate = offset_point(position, direction, 1);
    let destination = if can_occupy(world, candidate.clone(), Some(player)) {
        candidate
    } else {
        position.clone()
    };
    let facing = entity_facing(world, player).unwrap_or(MirDirection::Down);

    world
        .entity_mut(player)
        .insert((Position(destination.clone()), Facing(facing)));
    packets.push(ServerPacket::ObjectTeleportOut {
        object_id: player_object_id,
        effect_type: GREAT_FOX_SPIRIT_RECALL_TELEPORT_EFFECT,
    });
    packets.push(ServerPacket::ObjectWalk {
        movement: ObjectMovement {
            object_id: player_object_id,
            position: destination,
            direction: facing,
        },
    });
    packets.push(ServerPacket::ObjectTeleportIn {
        object_id: player_object_id,
        effect_type: GREAT_FOX_SPIRIT_RECALL_TELEPORT_EFFECT,
    });
    true
}

#[allow(deprecated)]
pub(in crate::runtime) fn set_guardian_rocks_active_near(
    world: &mut World,
    origin: &Point,
    active: bool,
) -> bool {
    let rocks = world
        .iter_entities()
        .filter_map(|entity| {
            let agent = entity.get::<MonsterAgent>()?;
            let position = entity.get::<Position>()?;
            (agent.ai == 48 && tile_distance(origin, &position.0) <= 20).then_some(entity.id())
        })
        .collect::<Vec<_>>();

    let mut changed = false;
    for rock in rocks {
        let mut ai_state = world
            .entity(rock)
            .get::<MonsterAiState>()
            .copied()
            .unwrap_or_default();
        if ai_state.mode != active {
            ai_state.mode = active;
            world.entity_mut(rock).insert(ai_state);
            changed = true;
        }
    }

    changed
}

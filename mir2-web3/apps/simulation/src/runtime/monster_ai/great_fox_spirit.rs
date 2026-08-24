// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    entity_facing, entity_object_id, entity_position, player_entity, Facing, Monster, MonsterAgent,
    MonsterAiState, MonsterVitals, PlayerVitals, Position, RemotePlayer, SelfPlayer, WorldObject,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;
use super::super::packets::*;
use super::super::resources::current_language;

const GREAT_FOX_SPIRIT_RECALL_RANGE: i32 = 30;

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

    // Crystal skips each recall candidate when its all-or-nothing
    // MagicResist roll succeeds. The ten-second recall cooldown has already
    // started at this point, matching `GreatFoxSpirit.ProcessTarget`.
    for target in great_fox_spirit_recall_targets(world, entity, agent, position) {
        if great_fox_spirit_target_magic_mitigated(world, target) == 0 {
            continue;
        }
        if teleport_great_fox_spirit_target(world, entity, target, position, tick, packets) {
            return true;
        }
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GreatFoxSpiritRecallTarget {
    Player(Entity),
    Monster(Entity),
}

impl GreatFoxSpiritRecallTarget {
    fn entity(self) -> Entity {
        match self {
            Self::Player(entity) | Self::Monster(entity) => entity,
        }
    }
}

/// Bounded implementation of Crystal FindAllTargets(30, CurrentLocation).
///
/// A personal SimulationSession owns exactly one authoritative player. Its
/// RemotePlayer entities are render/snapshot mirrors and cannot be safely
/// mutated here, so they are deliberately excluded. All remaining entities
/// in this ECS are rebuilt for the active MapRuntimeResource; there is no
/// stale cross-map object to target once map transition cleanup has run.
fn great_fox_spirit_recall_targets(
    world: &World,
    attacker: Entity,
    attacker_agent: &MonsterAgent,
    origin: &Point,
) -> Vec<GreatFoxSpiritRecallTarget> {
    let mut candidates = Vec::new();

    if let Some(player) = player_entity(world) {
        if player != attacker
            && great_fox_spirit_is_attack_target(
                world,
                attacker,
                attacker_agent,
                GreatFoxSpiritRecallTarget::Player(player),
                origin,
            )
        {
            candidates.push((
                great_fox_spirit_target_sort_key(world, player, origin),
                GreatFoxSpiritRecallTarget::Player(player),
            ));
        }
    }

    for candidate in world.iter_entities() {
        let target = GreatFoxSpiritRecallTarget::Monster(candidate.id());
        if great_fox_spirit_is_attack_target(world, attacker, attacker_agent, target, origin) {
            candidates.push((
                great_fox_spirit_target_sort_key(world, candidate.id(), origin),
                target,
            ));
        }
    }

    candidates.sort_by_key(|(key, _)| *key);
    candidates.into_iter().map(|(_, target)| target).collect()
}

fn great_fox_spirit_target_sort_key(
    world: &World,
    target: Entity,
    origin: &Point,
) -> (i32, i32, i32, u32, u32) {
    let position = entity_position(world, target).unwrap_or_else(|| origin.clone());
    (
        tile_distance(origin, &position),
        position.y,
        position.x,
        entity_object_id(world, target).unwrap_or(u32::MAX),
        target.index(),
    )
}

fn great_fox_spirit_is_attack_target(
    world: &World,
    attacker: Entity,
    attacker_agent: &MonsterAgent,
    target: GreatFoxSpiritRecallTarget,
    origin: &Point,
) -> bool {
    let target_entity = target.entity();
    if target_entity == attacker
        || entity_position(world, target_entity).is_none()
        || entity_object_id(world, target_entity).is_none()
        || !world.entity(target_entity).contains::<WorldObject>()
    {
        return false;
    }

    let target_position = entity_position(world, target_entity).expect("position checked above");
    let distance = tile_distance(origin, &target_position);
    if !(4..=GREAT_FOX_SPIRIT_RECALL_RANGE).contains(&distance) {
        return false;
    }

    match target {
        GreatFoxSpiritRecallTarget::Player(player) => {
            // Only SelfPlayer has a transform that this personal session can
            // authoritatively mutate. RemotePlayer is intentionally not part
            // of this collection even when it is otherwise attackable.
            world.entity(player).contains::<SelfPlayer>()
                && !world.entity(player).contains::<RemotePlayer>()
                && world
                    .entity(player)
                    .get::<PlayerVitals>()
                    .is_some_and(|vitals| vitals.hp > 0)
                && attacker_agent.hostile_to_player
        }
        GreatFoxSpiritRecallTarget::Monster(monster) => {
            let entry = world.entity(monster);
            let Some(target_agent) = entry.get::<MonsterAgent>() else {
                return false;
            };
            let Some(vitals) = entry.get::<MonsterVitals>() else {
                return false;
            };
            let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
            entry.contains::<Monster>()
                && vitals.hp > 0
                && !target_agent.dead
                && !is_hidden_or_sleeping_target(target_agent, &target_ai_state)
                // This is the existing runtime's bounded IsAttackTarget
                // model for monster-vs-monster combat: opposite dispositions
                // only. Same-side monsters are never recall targets.
                && target_agent.hostile_to_player != attacker_agent.hostile_to_player
        }
    }
}

/// Resolve MagicResist independently for each candidate. Player resistance is
/// authoritative today. Crystal's MonsterObject also exposes Stats[MagicResist],
/// but the current personal ECS has no Monster MagicResist component or
/// imported monster stat block; its Monster candidates therefore use the
/// runtime's authoritative zero-resistance default rather than inheriting the
/// player's stat or inventing a value.
fn great_fox_spirit_target_magic_mitigated(
    world: &World,
    target: GreatFoxSpiritRecallTarget,
) -> i32 {
    match target {
        GreatFoxSpiritRecallTarget::Player(_) => crystal_player_magic_mitigated(world, 1),
        GreatFoxSpiritRecallTarget::Monster(_) => 1,
    }
}

fn teleport_great_fox_spirit_target(
    world: &mut World,
    attacker: Entity,
    target: GreatFoxSpiritRecallTarget,
    origin: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let target_entity = target.entity();
    let Some(object_id) = entity_object_id(world, target_entity) else {
        return false;
    };
    if entity_position(world, attacker).as_ref() != Some(origin)
        || !world.entity(target_entity).contains::<WorldObject>()
    {
        return false;
    }

    let direction = rotated_direction(
        MirDirection::Up,
        deterministic_roll(tick, attacker.index() as usize, 50, 7) as i32,
    );
    let candidate = offset_point(origin, direction, 1);
    let destination = if can_occupy(world, candidate.clone(), Some(target_entity)) {
        candidate
    } else {
        // Crystal Teleport validates the map point, not blocking occupancy;
        // its fallback is the Fox's current tile. The attacker position is a
        // live current-map node, so this fallback is valid and bounded here.
        origin.clone()
    };
    let facing = entity_facing(world, target_entity).unwrap_or(MirDirection::Down);

    world
        .entity_mut(target_entity)
        .insert((Position(destination.clone()), Facing(facing)));
    packets.push(ServerPacket::ObjectTeleportOut {
        object_id,
        effect_type: GREAT_FOX_SPIRIT_RECALL_TELEPORT_EFFECT,
    });
    packets.push(ServerPacket::ObjectWalk {
        movement: ObjectMovement {
            object_id,
            position: destination,
            direction: facing,
        },
    });
    packets.push(ServerPacket::ObjectTeleportIn {
        object_id,
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

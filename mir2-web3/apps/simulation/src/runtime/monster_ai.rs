use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::{crystal_monster_by_name, CrystalRespawnTemplate};
use mir2_protocol::{MirDirection, ObjectMovement, ObjectSpellInfo, Point, ServerPacket, Spell};

use crate::config::WorldEntityDisposition;

use super::buffs::{
    tick_buffs, tick_crystal_normal_hero_potion_restore, tick_crystal_normal_potion_restore,
};
use super::combat::*;
use super::components::{
    current_player_is_dead, current_player_object_id, entity_facing, entity_name, entity_object_id,
    entity_position, player_entity, DisplayName, Facing, GeneralMeowMeowState, Monster,
    MonsterAgent, MonsterAiState, MonsterCombatStats, MonsterVitals, ObjectId, Position,
    SummonedMonster, WoomaTaurusState, WorldObject, YimoogiState,
};
use super::crystal_compat::*;
use super::drops::tick_ground_drop_expiry;
use super::equipment::total_defence_bonus;
use super::fishing::tick_fishing;
use super::hero_ai::tick_stage5_hero_combat_ai;
use super::inventory::sync_expired_expanded_storage;
use super::monsters::*;
use super::movement::*;
use super::npc::process_crystal_npc_goods_expiry;
use super::packets::*;
use super::rental::{process_expired_rental_items, return_rented_items_on_player_death};
use super::resources::{
    advance_runtime_tick, current_language, is_in_world, runtime_tick, BuffResource,
    InventoryResource, MapRuntimeResource,
};
use super::session::SimulationSession;
use super::skills::tick_ground_spell_actions;

pub(super) fn monster_step_toward_with_fallback(
    world: &World,
    entity: Entity,
    position: &Point,
    target: &Point,
    tick: u64,
    salt: usize,
) -> Option<(Point, MirDirection)> {
    let base_direction = direction_toward(position, target)?;
    let prefer_next = deterministic_roll(tick, entity.index() as usize, salt, 2) == 0;

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
            return Some((destination, direction));
        }
    }

    None
}

pub(super) fn summoned_monster_entity_target(
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

pub(super) fn summon_attack_damage(
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

pub(super) fn hostile_monster_summon_target(
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

pub(super) fn shinsu_line_monster_targets(
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

pub(super) fn forward_line_opposing_monster_targets(
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

pub(super) fn tucson_mage_wide_line_points(
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

pub(super) fn tucson_mage_wide_line_opposing_monster_targets(
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

pub(super) fn manectric_claw_cone_points(
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

pub(super) fn manectric_claw_cone_opposing_monster_targets(
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

pub(super) fn nearby_opposing_monster_targets(
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

pub(super) fn halfmoon_opposing_monster_targets(
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

pub(super) fn monster_prefers_monster_target(
    world: &World,
    attacker: Entity,
    attacker_agent: &MonsterAgent,
) -> bool {
    world
        .entity(attacker)
        .get::<SummonedMonster>()
        .map(|_| !attacker_agent.hostile_to_player)
        .unwrap_or(false)
        || matches!(attacker_agent.ai, 6 | 113)
}

pub(super) fn update_special_monster_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if update_summoned_monster_state(world, entity, agent, position, tick, packets) {
        return true;
    }

    if agent.ai == 2
        && update_deer_run_away_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 50
        && update_great_fox_spirit_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 17 {
        update_zuma_taurus_stage_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
        );
    }

    if agent.ai == 49
        && update_thunder_element_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 36
        && update_yimoogi_state(
            world,
            entity,
            agent,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 123 {
        update_general_meow_meow_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        );
    }

    if agent.ai == 54
        && update_dragon_statue_sleep_state(world, entity, agent, ai_state, tick, packets)
    {
        return true;
    }

    if agent.ai == 34
        && update_frost_tiger_sitting_state(world, entity, agent, ai_state, position, tick, packets)
    {
        return true;
    }

    if agent.ai == 8
        && update_axe_skeleton_fear_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if matches!(agent.ai, 45 | 46)
        && update_foxman_fear_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 38
        && update_holy_deva_fear_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 186
        && update_kirin_ice_thrust_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    match agent.ai {
        14 => update_evil_centipede_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        5 => update_cannibal_plant_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        11 => update_wooma_taurus_state(world, entity, agent, position, tick, packets),
        40 => update_bomb_spider_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        47 => update_trap_rock_state(
            world,
            entity,
            agent,
            ai_state,
            player_position,
            tick,
            packets,
        ),
        15 | 16 | 17 | 173 | 174 => update_zuma_monster_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        18 => update_shinsu_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        24 => update_dig_out_zombie_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        124 | 125 => update_armadillo_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        25 => update_reviving_zombie_state(world, entity, agent, ai_state, tick, packets),
        30 => update_bone_lord_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        42 => update_yin_devil_node_state(agent, tick),
        60 => update_vampire_spider_state(world, entity, agent, position, tick, packets),
        61 => update_spitting_toad_state(world, entity, agent, position, tick, packets),
        62 => update_snake_totem_state(world, entity, agent, position, tick, packets),
        131 => update_tucson_general_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        98 => update_hell_lord_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        99 => update_hell_bomb_state(world, entity, agent, ai_state, position, tick, packets),
        255 => update_stone_trap_state(world, entity, agent, position, tick),
        _ => false,
    }
}

pub(super) fn update_great_fox_spirit_state(
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

pub(super) fn try_great_fox_spirit_recall(
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
pub(super) fn set_guardian_rocks_active_near(
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

pub(super) fn update_thunder_element_state(
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

pub(super) fn update_yimoogi_state(
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

pub(super) fn yimoogi_should_final_teleport(world: &World, entity: Entity) -> bool {
    let Some(vitals) = world.entity(entity).get::<MonsterVitals>() else {
        return false;
    };
    vitals.hp <= vitals.max_hp / 10
}

pub(super) fn yimoogi_final_teleport_destination(
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

pub(super) fn spawn_yimoogi_child(
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

pub(super) fn spawn_yimoogi_runtime_monster(
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

pub(super) fn spawn_yimoogi_white_serpents(
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

pub(super) fn update_dragon_statue_sleep_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !ai_state.mode {
        return false;
    }

    agent.tracking_player = false;
    agent.next_attack_tick = tick + 1;
    agent.next_move_tick = tick + 1;
    if tick < ai_state.next_state_tick {
        return true;
    }

    ai_state.mode = false;
    let max_hp = world
        .entity(entity)
        .get::<MonsterVitals>()
        .map(|vitals| vitals.max_hp)
        .unwrap_or(1);
    world
        .entity_mut(entity)
        .insert(MonsterVitals { hp: max_hp, max_hp });
    if let Some(info) = object_health_info_for_entity(world, entity, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    true
}

pub(super) fn update_frost_tiger_sitting_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return false;
    }

    if ai_state.extra {
        if agent.tracking_player {
            ai_state.extra = false;
            ai_state.next_state_tick = tick + FROST_TIGER_SIT_DOWN_MAX_DELAY_TICKS;
            if let Some(packet) = object_sit_down_packet(world, entity, position, false) {
                packets.push(packet);
            }
            return false;
        }
        agent.next_attack_tick = tick + 1;
        agent.next_move_tick = tick + 1;
        return true;
    }

    if !agent.tracking_player && tick >= ai_state.next_state_tick {
        ai_state.extra = true;
        agent.tracking_player = false;
        agent.next_attack_tick = tick + 1;
        agent.next_move_tick = tick + 1;
        if let Some(packet) = object_sit_down_packet(world, entity, position, true) {
            packets.push(packet);
        }
        return true;
    }

    false
}

pub(super) fn object_sit_down_packet(
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

pub(super) fn update_foxman_fear_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !agent.tracking_player {
        return false;
    }

    let distance = tile_distance(position, player_position);
    if agent.ai == 45
        && distance <= 1
        && tick < ai_state.next_state_tick
        && tick >= agent.next_route_tick
    {
        if let Some(destination) = foxman_teleport_destination(world, entity, position, tick) {
            let direction = direction_toward(&destination, player_position).unwrap_or_else(|| {
                world
                    .entity(entity)
                    .get::<Facing>()
                    .map(|facing| facing.0)
                    .unwrap_or(MirDirection::Down)
            });
            agent.next_route_tick = tick + RED_FOXMAN_TELEPORT_COOLDOWN_TICKS;
            agent.next_move_tick = tick + 1;
            agent.next_attack_tick = tick + 1;
            world.entity_mut(entity).insert((
                Position(destination.clone()),
                Facing(direction),
                agent.clone(),
            ));
            if let Some(object_id) = entity_object_id(world, entity) {
                packets.push(ServerPacket::ObjectTeleportOut {
                    object_id,
                    effect_type: RED_FOXMAN_TELEPORT_EFFECT,
                });
                packets.push(ServerPacket::ObjectWalk {
                    movement: ObjectMovement {
                        object_id,
                        position: destination,
                        direction,
                    },
                });
                packets.push(ServerPacket::ObjectTeleportIn {
                    object_id,
                    effect_type: RED_FOXMAN_TELEPORT_EFFECT,
                });
            }
            return true;
        }
    }

    if tick < ai_state.next_state_tick {
        return false;
    }

    ai_state.next_state_tick = tick + FOXMAN_FEAR_DURATION_TICKS;
    if tick < agent.next_move_tick {
        return true;
    }

    let attack_range = monster_attack_range(agent);
    let preferred_direction = if distance >= attack_range {
        direction_toward(position, player_position)
    } else {
        direction_toward(player_position, position)
    };
    let Some(preferred_direction) = preferred_direction else {
        return true;
    };
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let prefer_next = deterministic_chance_roll(tick, object_id, 450, 2);
    let offsets: [i32; 8] = if prefer_next {
        [0, 1, 2, 3, 4, 5, 6, 7]
    } else {
        [0, -1, -2, -3, -4, -5, -6, -7]
    };

    for offset in offsets {
        let direction = rotated_direction(preferred_direction, offset);
        let next = offset_point(position, direction, 1);
        if !can_occupy(world, next.clone(), Some(entity)) {
            continue;
        }

        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        world
            .entity_mut(entity)
            .insert((Position(next.clone()), Facing(direction), agent.clone()));
        packets.push(ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: next,
                direction,
            },
        });
        return true;
    }

    true
}

pub(super) fn foxman_teleport_destination(
    world: &World,
    entity: Entity,
    position: &Point,
    tick: u64,
) -> Option<Point> {
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let side = RED_FOXMAN_TELEPORT_RADIUS * 2 + 1;
    let area = side * side;
    let start = ((tick as i32)
        .wrapping_add((object_id as i32).wrapping_mul(31))
        .rem_euclid(area)) as i32;

    for offset in 0..area {
        let index = (start + offset).rem_euclid(area);
        let dx = index.rem_euclid(side) - RED_FOXMAN_TELEPORT_RADIUS;
        let dy = index.div_euclid(side) - RED_FOXMAN_TELEPORT_RADIUS;
        let candidate = Point {
            x: position.x + dx,
            y: position.y + dy,
        };
        if candidate == *position || tile_distance(position, &candidate) <= 1 {
            continue;
        }
        if can_occupy(world, candidate.clone(), Some(entity)) {
            return Some(candidate);
        }
    }

    None
}

pub(super) fn update_deer_run_away_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !ai_state.mode || !agent.can_wander {
        return false;
    }

    let distance = tile_distance(position, player_position);
    if !agent.tracking_player && distance > agent.view_range.max(1) {
        return false;
    }
    agent.tracking_player = true;

    if tick < agent.next_move_tick {
        return true;
    }

    let Some(away_direction) = direction_toward(player_position, position) else {
        return true;
    };
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let prefer_next = deterministic_chance_roll(tick, object_id, 20, 2);
    let offsets: [i32; 8] = if prefer_next {
        [0, 1, 2, 3, 4, 5, 6, 7]
    } else {
        [0, -1, -2, -3, -4, -5, -6, -7]
    };

    for offset in offsets {
        let direction = rotated_direction(away_direction, offset);
        let next = offset_point(position, direction, 1);
        if !can_occupy(world, next.clone(), Some(entity)) {
            continue;
        }

        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        world
            .entity_mut(entity)
            .insert((Position(next.clone()), Facing(direction), agent.clone()));
        packets.push(ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: next,
                direction,
            },
        });
        return true;
    }

    true
}

pub(super) fn update_axe_skeleton_fear_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !agent.tracking_player {
        return false;
    }

    if tick < ai_state.next_state_tick {
        return false;
    }

    ai_state.next_state_tick = tick + FOXMAN_FEAR_DURATION_TICKS;
    if tick < agent.next_move_tick {
        return true;
    }

    let next_step = if tile_distance(position, player_position) >= monster_attack_range(agent) {
        monster_step_toward_with_fallback(world, entity, position, player_position, tick, 8)
    } else {
        let away_direction = direction_toward(player_position, position);
        let object_id = entity_object_id(world, entity).unwrap_or_default();
        let prefer_next = deterministic_chance_roll(tick, object_id, 80, 2);
        let offsets: [i32; 8] = if prefer_next {
            [0, 1, 2, 3, 4, 5, 6, 7]
        } else {
            [0, -1, -2, -3, -4, -5, -6, -7]
        };
        away_direction.and_then(|direction| {
            offsets.into_iter().find_map(|offset| {
                let direction = rotated_direction(direction, offset);
                let next = offset_point(position, direction, 1);
                can_occupy(world, next.clone(), Some(entity)).then_some((next, direction))
            })
        })
    };

    if let Some((next, direction)) = next_step {
        let object_id = entity_object_id(world, entity).unwrap_or_default();
        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        world
            .entity_mut(entity)
            .insert((Position(next.clone()), Facing(direction), agent.clone()));
        packets.push(ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: next,
                direction,
            },
        });
    }

    true
}

pub(super) fn update_holy_deva_fear_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead
        || !agent.tracking_player
        || world.entity(entity).get::<SummonedMonster>().is_some()
    {
        return false;
    }

    if tick < ai_state.next_state_tick {
        return false;
    }

    ai_state.next_state_tick = tick + FOXMAN_FEAR_DURATION_TICKS;
    if tile_distance(position, player_position) >= monster_attack_range(agent) {
        return true;
    }
    if tick < agent.next_move_tick {
        return true;
    }

    let Some(away_direction) = direction_toward(player_position, position) else {
        return true;
    };
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let prefer_next = deterministic_chance_roll(tick, object_id, 380, 2);
    let offsets: [i32; 8] = if prefer_next {
        [0, 1, 2, 3, 4, 5, 6, 7]
    } else {
        [0, -1, -2, -3, -4, -5, -6, -7]
    };

    for offset in offsets {
        let direction = rotated_direction(away_direction, offset);
        let next = offset_point(position, direction, 1);
        if !can_occupy(world, next.clone(), Some(entity)) {
            continue;
        }

        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        world
            .entity_mut(entity)
            .insert((Position(next.clone()), Facing(direction), agent.clone()));
        packets.push(ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: next,
                direction,
            },
        });
        return true;
    }

    true
}

pub(super) fn update_kirin_ice_thrust_state(
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
    let mitigation = total_defence_bonus(
        world.resource::<InventoryResource>(),
        world.resource::<BuffResource>(),
    );
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

pub(super) fn update_general_meow_meow_state(
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

pub(super) fn general_meow_meow_shield_phase(hp: i32, max_hp: i32) -> bool {
    let percent = hp.max(0) * 100 / max_hp.max(1);
    (70..=80).contains(&percent) || (40..=50).contains(&percent) || percent <= 20
}

pub(super) fn cast_general_meow_meow_mass_thunder(
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
        let mitigation = total_defence_bonus(
            world.resource::<InventoryResource>(),
            world.resource::<BuffResource>(),
        );
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

pub(super) fn update_bone_lord_state(
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
    if !agent.tracking_player
        && !(agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1))
    {
        return false;
    }

    let Some(vitals) = world.entity(entity).get::<MonsterVitals>().copied() else {
        return false;
    };
    if vitals.max_hp < i32::from(BONE_LORD_STAGE_COUNT) {
        return false;
    }

    let stage_size = (vitals.max_hp / i32::from(BONE_LORD_STAGE_COUNT)).max(1);
    let stage = (vitals.hp.max(0) / stage_size).clamp(0, i32::from(BONE_LORD_STAGE_COUNT)) as u8;
    if stage >= ai_state.extra_byte {
        return false;
    }

    let Some(object_id) = entity_object_id(world, entity) else {
        return false;
    };
    let Some(player) = player_entity(world) else {
        return false;
    };
    let direction = direction_toward(position, player_position)
        .or_else(|| entity_facing(world, entity))
        .unwrap_or(MirDirection::Up);

    ai_state.extra_byte = stage;
    agent.tracking_player = true;
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    world.entity_mut(entity).insert(Facing(direction));
    if let Some(packet) = monster_typed_attack_packet(world, entity, position, direction, 1) {
        packets.push(packet);
    }

    spawn_monster_slave_wave(
        world,
        entity,
        object_id,
        position,
        direction,
        Some(player),
        agent,
        tick,
        &BONE_LORD_SLAVE_NAMES,
        BONE_LORD_MAX_SLAVES,
        BONE_LORD_SPAWN_BATCH_SIZE,
    );
    true
}

pub(super) fn update_zuma_taurus_stage_state(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
) {
    if agent.dead {
        return;
    }

    let Some(vitals) = world.entity(entity).get::<MonsterVitals>().copied() else {
        return;
    };
    if vitals.max_hp < i32::from(ZUMA_TAURUS_STAGE_COUNT) {
        return;
    }

    let stage_size = (vitals.max_hp / i32::from(ZUMA_TAURUS_STAGE_COUNT)).max(1);
    let stage = (vitals.hp.max(0) / stage_size).clamp(0, i32::from(ZUMA_TAURUS_STAGE_COUNT)) as u8;
    if stage >= ai_state.extra_byte {
        return;
    }

    let Some(object_id) = entity_object_id(world, entity) else {
        return;
    };
    let direction = entity_facing(world, entity).unwrap_or(MirDirection::DownLeft);
    let target_entity = if agent.tracking_player
        || (agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1))
    {
        player_entity(world)
    } else {
        None
    };

    ai_state.extra_byte = stage;
    spawn_monster_slave_wave(
        world,
        entity,
        object_id,
        position,
        direction,
        target_entity,
        agent,
        tick,
        &ZUMA_TAURUS_SLAVE_NAMES,
        ZUMA_TAURUS_MAX_SLAVES,
        ZUMA_TAURUS_SPAWN_BATCH_SIZE,
    );
}

pub(super) fn spawn_monster_slave_wave(
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

pub(super) fn spawn_snow_wolf_king_slaves(
    world: &mut World,
    object_id: u32,
    position: &Point,
    direction: MirDirection,
    target_entity: Entity,
    target_position: &Point,
    agent: &MonsterAgent,
) {
    let Some(template) = crystal_dynamic_monster_template("SnowWolf") else {
        return;
    };
    let target_back = entity_facing(world, target_entity)
        .map(|facing| offset_point(target_position, facing, -1))
        .unwrap_or_else(|| offset_point(target_position, direction, -1));
    let summon_metadata = SummonedMonster {
        summoner_object_id: object_id,
        visible_extra: false,
        expire_tick: None,
        require_summoner_within: None,
        despawn_tick_after_death: None,
        totem_master_object_id: None,
        max_minions: Some(SNOW_WOLF_KING_SLAVE_COUNT),
    };

    for _ in 0..SNOW_WOLF_KING_SLAVE_COUNT {
        let template = CrystalRespawnTemplate {
            location: target_back.clone(),
            ..template.clone()
        };
        if spawn_runtime_monster(
            world,
            &template,
            target_back.clone(),
            direction,
            Some(target_entity),
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
                Some(target_entity),
                Some(summon_metadata),
                Some(agent.hostile_to_player),
                Some(agent.disposition),
                combat_delay_ticks(2_000),
            );
        }
    }
}

pub(super) fn schedule_snow_wolf_king_death_explosion(
    world: &mut World,
    monster_entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
    monster_name: &str,
    current_tick: u64,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let damage = crystal_monster_attack_damage(monster_name);
    if damage <= 0 {
        return;
    }
    let due_tick = current_tick + combat_delay_ticks(500);

    if agent.hostile_to_player {
        if let Some(player) = player_entity(world) {
            if entity_position(world, player)
                .map(|player_position| tile_distance(position, &player_position) <= 1)
                .unwrap_or(false)
            {
                schedule_damage_to_player(
                    world,
                    due_tick,
                    attacker_id,
                    monster_name.to_string(),
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
        monster_entity,
        position,
        agent,
        1,
    ) {
        schedule_damage_to_monster(world, due_tick, attacker_id, target, damage, None, None);
    }
}

pub(super) fn update_tucson_general_state(
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

pub(super) fn schedule_tucson_general_rocks(
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

pub(super) fn tucson_general_rock_target_positions(
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

pub(super) fn tucson_general_scattered_rock_position(
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

pub(super) fn update_wooma_taurus_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return false;
    }

    let existing_state = world.entity(entity).get::<WoomaTaurusState>().copied();
    let mut state = existing_state.unwrap_or_else(|| {
        initial_wooma_taurus_state(tick, agent.move_interval_ticks, agent.attack_interval_ticks)
    });

    if state.mad_until_tick > 0 && tick >= state.mad_until_tick {
        state.mad_until_tick = 0;
        agent.move_interval_ticks = state.base_move_interval_ticks;
        agent.attack_interval_ticks = state.base_attack_interval_ticks;
    }

    if let Some(vitals) = world.entity(entity).get::<MonsterVitals>() {
        if vitals.max_hp >= i32::from(WOOMA_TAURUS_STAGE_COUNT) {
            let stage_size = (vitals.max_hp / i32::from(WOOMA_TAURUS_STAGE_COUNT)).max(1);
            let stage =
                (vitals.hp.max(0) / stage_size).clamp(0, i32::from(WOOMA_TAURUS_STAGE_COUNT)) as u8;
            if stage < state.stage {
                state.mad_until_tick = tick + WOOMA_TAURUS_MAD_DURATION_TICKS;
                agent.move_interval_ticks = WOOMA_TAURUS_MAD_MOVE_INTERVAL_TICKS;
                agent.attack_interval_ticks = WOOMA_TAURUS_MAD_ATTACK_INTERVAL_TICKS;
                agent.next_move_tick = agent.next_move_tick.min(tick + 1);
                agent.next_attack_tick = agent.next_attack_tick.min(tick + 1);
            }
            state.stage = stage;
        }
    }

    let mut teleported = false;
    if tick >= state.next_teleport_tick {
        state.next_teleport_tick = tick + WOOMA_TAURUS_TELEPORT_DELAY_TICKS;
        if wooma_taurus_blocked_neighbor_count(world, entity, position)
            >= WOOMA_TAURUS_BLOCKED_NEIGHBOR_THRESHOLD
        {
            if let Some(destination) =
                wooma_taurus_teleport_destination(world, entity, position, tick)
            {
                agent.tracking_player = false;
                agent.next_move_tick = tick + 1;
                agent.next_attack_tick = tick + 1;
                let direction = world
                    .entity(entity)
                    .get::<Facing>()
                    .map(|facing| facing.0)
                    .unwrap_or(MirDirection::Down);
                if let Some(object_id) = entity_object_id(world, entity) {
                    world
                        .entity_mut(entity)
                        .insert(Position(destination.clone()));
                    packets.push(ServerPacket::ObjectTeleportOut {
                        object_id,
                        effect_type: 0,
                    });
                    packets.push(ServerPacket::ObjectWalk {
                        movement: ObjectMovement {
                            object_id,
                            position: destination,
                            direction,
                        },
                    });
                    packets.push(ServerPacket::ObjectTeleportIn {
                        object_id,
                        effect_type: 0,
                    });
                    teleported = true;
                }
            }
        }
    }

    if existing_state != Some(state) {
        world.entity_mut(entity).insert(state);
    }

    teleported
}

pub(super) fn wooma_taurus_blocked_neighbor_count(
    world: &World,
    entity: Entity,
    position: &Point,
) -> usize {
    [
        MirDirection::Up,
        MirDirection::UpRight,
        MirDirection::Right,
        MirDirection::DownRight,
        MirDirection::Down,
        MirDirection::DownLeft,
        MirDirection::Left,
        MirDirection::UpLeft,
    ]
    .into_iter()
    .filter(|direction| {
        let point = offset_point(position, *direction, 1);
        !can_occupy(world, point, Some(entity))
    })
    .count()
}

pub(super) fn wooma_taurus_teleport_destination(
    world: &World,
    entity: Entity,
    position: &Point,
    tick: u64,
) -> Option<Point> {
    let mut candidates = Vec::new();
    for distance in 2..=WOOMA_TAURUS_TELEPORT_RADIUS {
        for y in position.y - distance..=position.y + distance {
            for x in position.x - distance..=position.x + distance {
                let point = Point { x, y };
                if tile_distance(position, &point) != distance {
                    continue;
                }
                if can_occupy(world, point.clone(), Some(entity)) {
                    candidates.push(point);
                }
            }
        }
    }

    if candidates.is_empty() {
        return first_occupiable_point_near(
            world,
            position,
            WOOMA_TAURUS_TELEPORT_RADIUS * 2,
            Some(entity),
        );
    }

    let index = deterministic_roll(tick, entity.index() as usize, 11, candidates.len() as u64);
    candidates.get(index as usize).cloned()
}

pub(super) fn update_shinsu_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    let has_target =
        summoned_monster_entity_target(world, &monster_entities, entity, position, agent).is_some();
    if has_target {
        ai_state.next_state_tick = tick + combat_delay_ticks(30_000);
        agent.tracking_player = true;
    }

    if !ai_state.mode && has_target {
        ai_state.mode = true;
        ai_state.hidden = false;
        if let Some(monster) = crystal_monster_by_name("Shinsu1") {
            agent.image = monster.image;
        }
        if let Some(object_id) = entity_object_id(world, entity) {
            packets.push(ServerPacket::ObjectShow { object_id });
        }
        return true;
    }

    if ai_state.mode && !has_target && tick >= ai_state.next_state_tick {
        ai_state.mode = false;
        ai_state.hidden = true;
        agent.tracking_player = false;
        if let Some(monster) = crystal_monster_by_name("Shinsu") {
            agent.image = monster.image;
        }
        if let Some(object_id) = entity_object_id(world, entity) {
            packets.push(ServerPacket::ObjectHide { object_id });
        }
        return true;
    }

    let _ = player_position;
    false
}

pub(super) fn update_vampire_spider_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if !agent.dead {
        return false;
    }

    if let Some(despawn_tick) = world
        .entity(entity)
        .get::<SummonedMonster>()
        .and_then(|summoned| summoned.despawn_tick_after_death)
    {
        if tick >= despawn_tick {
            let _ = world.despawn(entity);
        }
        return true;
    }

    explode_vampire_spider(world, entity, position, tick, packets);
    true
}

pub(super) fn update_spitting_toad_state(
    _world: &mut World,
    _entity: Entity,
    agent: &mut MonsterAgent,
    _position: &Point,
    _tick: u64,
    _packets: &mut Vec<ServerPacket>,
) -> bool {
    agent.tracking_player = false;
    false
}

pub(super) fn update_stone_trap_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
) -> bool {
    if agent.dead {
        return true;
    }

    let Some(object_id) = entity_object_id(world, entity) else {
        return false;
    };

    agent.tracking_player = false;
    agent.next_move_tick = tick + 1;
    agent.next_attack_tick = tick + 1;

    let target_candidates: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .filter(|candidate| *candidate != entity)
        .filter(|candidate| {
            let entry = world.entity(*candidate);
            let Some(candidate_agent) = entry.get::<MonsterAgent>() else {
                return false;
            };
            if candidate_agent.dead {
                return false;
            }
            let Some(candidate_position) = entry.get::<Position>().map(|value| value.0.clone())
            else {
                return false;
            };
            tile_distance(position, &candidate_position) <= agent.view_range.max(1)
                && candidate_agent.hostile_to_player
        })
        .collect();

    for candidate in target_candidates {
        if let Some(mut candidate_agent) = world.entity_mut(candidate).get_mut::<MonsterAgent>() {
            candidate_agent.tracking_player = false;
            candidate_agent.next_move_tick = candidate_agent.next_move_tick.min(tick + 1);
        }
    }

    let _ = object_id;
    false
}

pub(super) fn update_summoned_monster_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let Some(summoned) = world.entity(entity).get::<SummonedMonster>().copied() else {
        return false;
    };

    let mut should_die = false;
    let mut despawn_after_death = summoned.despawn_tick_after_death;

    if let Some(expire_tick) = summoned.expire_tick {
        if tick >= expire_tick {
            should_die = true;
        }
    }

    let mut summoner_dead = false;

    match summoned_owner_state(world, summoned.summoner_object_id) {
        Some((summoner_position, is_dead)) => {
            summoner_dead = is_dead;
            if let Some(range) = summoned.require_summoner_within {
                if tile_distance(position, &summoner_position) > range {
                    should_die = true;
                }
            }
        }
        None => {
            should_die = true;
        }
    }

    if agent.dead {
        if let Some(despawn_tick) = summoned.despawn_tick_after_death {
            if tick >= despawn_tick {
                let _ = world.despawn(entity);
            }
            return true;
        }
        return false;
    }

    if summoner_dead {
        should_die = true;
    }

    if should_die {
        if agent.ai == 62 {
            despawn_after_death = Some(tick + combat_delay_ticks(3_000));
            kill_summoned_minions(
                world,
                entity_object_id(world, entity).unwrap_or(summoned.summoner_object_id),
                tick,
                packets,
            );
        }
        if agent.ai == 99 {
            explode_hell_bomb(world, entity, agent, position, tick, packets);
            return true;
        }

        if let Some(mut summoned_mut) = world.entity_mut(entity).get_mut::<SummonedMonster>() {
            summoned_mut.despawn_tick_after_death = despawn_after_death;
        }

        let _ = damage_monster_entity(world, entity, i32::MAX, tick, packets);
        return true;
    }

    false
}

pub(super) fn kill_summoned_minions(
    world: &mut World,
    summoner_object_id: u32,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    #[allow(deprecated)]
    let summoned_entities: Vec<Entity> = world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<SummonedMonster>()
                .filter(|summoned| summoned.summoner_object_id == summoner_object_id)
                .and_then(|_| entity.get::<MonsterAgent>().filter(|agent| !agent.dead))
                .map(|_| entity.id())
        })
        .collect();

    for summoned_entity in summoned_entities {
        let _ = damage_monster_entity(world, summoned_entity, i32::MAX, tick, packets);
    }
}

pub(super) fn update_bomb_spider_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return true;
    }

    let player_is_target = tile_distance(position, player_position) <= 1;
    if !agent.tracking_player || player_is_target || tick >= ai_state.next_state_tick {
        explode_bomb_spider(world, entity, position, packets);
        return true;
    }

    false
}

pub(super) fn update_cannibal_plant_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let player_nearby = tile_distance(position, player_position) <= 3;

    if ai_state.hidden {
        agent.tracking_player = false;

        if tick < ai_state.next_state_tick {
            return true;
        }

        ai_state.next_state_tick = tick + 2;
        if player_nearby {
            ai_state.hidden = false;
            agent.next_attack_tick = agent.next_attack_tick.max(tick + 1);
            if let Some(object_id) = entity_object_id(world, entity) {
                packets.push(ServerPacket::ObjectShow { object_id });
            }
        }
        return true;
    }

    if tick >= ai_state.next_state_tick {
        ai_state.next_state_tick = tick + 2;
        if !player_nearby {
            ai_state.hidden = true;
            ai_state.next_state_tick = tick + 3;
            agent.tracking_player = false;
            if let Some(mut vitals) = world.entity_mut(entity).get_mut::<MonsterVitals>() {
                vitals.hp = vitals.max_hp;
            }
            if let Some(object_id) = entity_object_id(world, entity) {
                packets.push(ServerPacket::ObjectHide { object_id });
            }
            return true;
        }
    }

    false
}

pub(super) fn update_evil_centipede_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let reveal_nearby = tile_distance(position, player_position) <= 3;
    let active_nearby = tile_distance(position, player_position) <= 7;

    if ai_state.hidden {
        agent.tracking_player = false;

        if tick < ai_state.next_state_tick {
            return true;
        }

        ai_state.next_state_tick = tick + 2;
        if reveal_nearby {
            ai_state.hidden = false;
            agent.tracking_player = true;
            agent.next_attack_tick = agent.next_attack_tick.max(tick + 1);
            if let Some(object_id) = entity_object_id(world, entity) {
                packets.push(ServerPacket::ObjectShow { object_id });
            }
        }
        return true;
    }

    if tick >= ai_state.next_state_tick {
        ai_state.next_state_tick = tick + 2;
        if !active_nearby {
            ai_state.hidden = true;
            ai_state.next_state_tick = tick + 3;
            agent.tracking_player = false;
            if let Some(mut vitals) = world.entity_mut(entity).get_mut::<MonsterVitals>() {
                vitals.hp = vitals.max_hp;
            }
            if let Some(object_id) = entity_object_id(world, entity) {
                packets.push(ServerPacket::ObjectHide { object_id });
            }
            return true;
        }
    }

    false
}

pub(super) fn update_trap_rock_state(
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

pub(super) fn spawn_trap_rock_child_rocks(
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

pub(super) fn trap_rock_visible_destination(
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
pub(super) fn trap_rock_spawn_corner_direction(tick: u64, object_id: u32) -> MirDirection {
    match trap_rock_spawn_corner_index(tick, object_id) {
        0 => MirDirection::Up,
        1 => MirDirection::Right,
        2 => MirDirection::Down,
        _ => MirDirection::Left,
    }
}

pub(super) fn trap_rock_spawn_corner_index(tick: u64, object_id: u32) -> usize {
    ((tick ^ u64::from(object_id).wrapping_mul(0x9E37_79B9)) % 4) as usize
}

pub(super) fn update_dig_out_zombie_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || !ai_state.hidden {
        return false;
    }

    agent.tracking_player = false;
    agent.next_move_tick = tick + 1;

    if tile_distance(position, player_position) > 3 {
        return true;
    }

    ai_state.hidden = false;
    agent.tracking_player = true;
    agent.next_attack_tick = agent.next_attack_tick.max(tick + 2);
    if let Some(object_id) = entity_object_id(world, entity) {
        packets.push(ServerPacket::ObjectShow { object_id });
    }

    true
}

pub(super) fn update_armadillo_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let was_hidden = ai_state.hidden;
    if update_dig_out_zombie_state(
        world,
        entity,
        agent,
        ai_state,
        position,
        player_position,
        tick,
        packets,
    ) {
        if was_hidden && !ai_state.hidden {
            ai_state.next_state_tick = tick + combat_delay_ticks(500);
        }
        return true;
    }

    if !ai_state.hidden
        && !ai_state.extra
        && ai_state.next_state_tick > 0
        && tick >= ai_state.next_state_tick
    {
        ai_state.extra = true;
        packets.push(armadillo_dig_out_spell_packet(
            world,
            position.clone(),
            entity_facing(world, entity).unwrap_or(MirDirection::Down),
        ));
    }

    update_armadillo_run_away_state(
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

pub(super) fn armadillo_dig_out_spell_packet(
    world: &mut World,
    location: Point,
    direction: MirDirection,
) -> ServerPacket {
    ServerPacket::ObjectSpell {
        info: ObjectSpellInfo {
            object_id: allocate_runtime_monster_object_id(world),
            location,
            spell: Spell::DigOutArmadillo,
            direction,
            param: true,
        },
    }
}

pub(super) fn update_armadillo_run_away_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead || ai_state.hidden || !ai_state.mode {
        return false;
    }

    agent.tracking_player = false;
    if tick < agent.next_move_tick {
        return true;
    }

    let Some(away_direction) = direction_toward(player_position, position) else {
        return true;
    };
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    let prefer_next = deterministic_chance_roll(tick, object_id, 1250, 2);
    let offsets: [i32; 8] = if prefer_next {
        [0, 1, 2, 3, 4, 5, 6, 7]
    } else {
        [0, -1, -2, -3, -4, -5, -6, -7]
    };

    for offset in offsets {
        let direction = rotated_direction(away_direction, offset);
        let next = offset_point(position, direction, 1);
        if !can_occupy(world, next.clone(), Some(entity)) {
            continue;
        }

        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        world
            .entity_mut(entity)
            .insert((Position(next.clone()), Facing(direction), agent.clone()));
        packets.push(ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: next,
                direction,
            },
        });
        return true;
    }

    true
}

pub(super) fn update_reviving_zombie_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if !agent.dead {
        return false;
    }

    if !ai_state.mode
        || ai_state.extra_byte >= REVIVING_ZOMBIE_MAX_REVIVALS
        || tick < ai_state.next_state_tick
    {
        return true;
    }

    ai_state.extra_byte += 1;
    ai_state.mode = false;
    ai_state.next_state_tick = 0;
    agent.dead = false;

    let revival_count = i32::from(ai_state.extra_byte);
    let max_hp = world
        .entity(entity)
        .get::<MonsterVitals>()
        .map(|vitals| vitals.max_hp.max(1))
        .unwrap_or(1);
    let hp = (max_hp * (100 - 25 * revival_count) / 100).max(1);
    world
        .entity_mut(entity)
        .insert((MonsterVitals { hp, max_hp }, agent.clone(), *ai_state));

    if let Some(info) = object_revived_info_for_entity(world, entity, false) {
        packets.push(ServerPacket::ObjectRevived { info });
    }
    if let Some(info) = object_health_info_for_entity(world, entity, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }

    true
}

pub(super) fn update_yin_devil_node_state(agent: &mut MonsterAgent, tick: u64) -> bool {
    if agent.dead {
        return true;
    }

    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;
    agent.next_attack_tick = tick + 1;
    true
}

pub(super) fn update_zuma_monster_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if !ai_state.extra {
        return false;
    }

    if tile_distance(position, player_position) > 2 {
        return true;
    }

    ai_state.extra = false;
    agent.tracking_player = true;
    agent.next_attack_tick = agent.next_attack_tick.max(tick + 1);
    if let Some(object_id) = entity_object_id(world, entity) {
        packets.push(ServerPacket::ObjectShow { object_id });
    }

    let awakened_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .filter(|candidate| *candidate != entity)
        .filter(|candidate| {
            let entry = world.entity(*candidate);
            let Some(candidate_agent) = entry.get::<MonsterAgent>() else {
                return false;
            };
            if !monster_uses_zuma_stone_state(candidate_agent.ai) {
                return false;
            }
            let candidate_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
            candidate_ai_state.extra
                && entry
                    .get::<Position>()
                    .map(|candidate_position| tile_distance(position, &candidate_position.0) <= 14)
                    .unwrap_or(false)
        })
        .collect();

    for awakened in awakened_entities {
        let mut entry = world.entity_mut(awakened);
        if let Some(mut awakened_state) = entry.get_mut::<MonsterAiState>() {
            awakened_state.extra = false;
        }
        if let Some(mut awakened_agent) = entry.get_mut::<MonsterAgent>() {
            awakened_agent.tracking_player = true;
            awakened_agent.next_attack_tick = awakened_agent.next_attack_tick.max(tick + 1);
        }
        if let Some(object_id) = entry.get::<ObjectId>().map(|value| value.0) {
            packets.push(ServerPacket::ObjectShow { object_id });
        }
    }

    true
}

pub(super) fn update_snake_totem_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let Some(object_id) = entity_object_id(world, entity) else {
        return true;
    };
    let is_friendly_totem = !agent.hostile_to_player;
    let max_minions = world
        .entity(entity)
        .get::<SummonedMonster>()
        .and_then(|summoned| summoned.max_minions)
        .unwrap_or(2);
    let active_minions = active_summoned_monster_count(world, object_id);

    if active_minions >= max_minions || tick < agent.next_attack_tick {
        agent.tracking_player = false;
        return true;
    }

    let target_entity = if is_friendly_totem {
        let monster_entities: Vec<Entity> = world
            .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
            .iter(world)
            .collect();
        let nearby_hostiles = monster_entities
            .iter()
            .copied()
            .filter(|candidate| *candidate != entity)
            .filter(|candidate| {
                let entry = world.entity(*candidate);
                let Some(candidate_agent) = entry.get::<MonsterAgent>() else {
                    return false;
                };
                if candidate_agent.dead || !candidate_agent.hostile_to_player {
                    return false;
                }
                let candidate_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
                if is_hidden_or_sleeping_target(candidate_agent, &candidate_ai_state) {
                    return false;
                }
                entry
                    .get::<Position>()
                    .map(|candidate_position| {
                        tile_distance(position, &candidate_position.0) <= agent.view_range.max(1)
                    })
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        for hostile in nearby_hostiles {
            if let Some(mut hostile_agent) = world.entity_mut(hostile).get_mut::<MonsterAgent>() {
                hostile_agent.tracking_player = false;
            }
        }
        summoned_monster_entity_target(world, &monster_entities, entity, position, agent)
    } else {
        let Some(player) = player_entity(world) else {
            return true;
        };
        let Some(player_position) = entity_position(world, player) else {
            return true;
        };
        if tile_distance(position, &player_position) > agent.view_range.max(1) {
            agent.tracking_player = false;
            return true;
        }
        Some(player)
    };

    let Some(target_entity) = target_entity else {
        agent.tracking_player = false;
        return true;
    };
    let Some(target_position) = entity_position(world, target_entity) else {
        return true;
    };
    let Some(direction) = direction_toward(position, &target_position) else {
        return true;
    };

    agent.tracking_player = false;
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    world.entity_mut(entity).insert(Facing(direction));
    if let Some(packet) = monster_melee_attack_packet(world, entity, position, direction) {
        packets.push(packet);
    }

    let spawn_position =
        directional_destination(world, position, MirDirection::Down, 1, Some(entity))
            .unwrap_or_else(|| offset_point(position, MirDirection::Down, 1));
    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: tick + combat_delay_ticks(500),
            summoner_entity: entity,
            template: CrystalRespawnTemplate {
                location: spawn_position,
                ..crystal_dynamic_monster_template("CharmedSnake")
                    .expect("CharmedSnake template should exist")
            },
            target_entity: Some(target_entity),
            summon_metadata: Some(SummonedMonster {
                summoner_object_id: object_id,
                visible_extra: true,
                expire_tick: Some(
                    tick + combat_delay_ticks(10_000 + (max_minions as u64 - 1) * 2_000),
                ),
                require_summoner_within: Some(15),
                despawn_tick_after_death: None,
                totem_master_object_id: Some(object_id),
                max_minions: Some(max_minions),
            }),
            hostile_to_player_override: Some(!is_friendly_totem),
        },
    );

    true
}

pub(super) fn update_hell_bomb_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;
    agent.next_attack_tick = tick + 1;

    if agent.dead {
        return true;
    }

    if ai_state.next_state_tick == 0 {
        ai_state.next_state_tick = tick + HELL_BOMB_EXPLOSION_LIFETIME_TICKS;
        return true;
    }

    if tick >= ai_state.next_state_tick {
        explode_hell_bomb(world, entity, agent, position, tick, packets);
    }

    true
}

pub(super) fn update_hell_lord_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;

    if agent.dead {
        return true;
    }

    let Some(lord_object_id) = entity_object_id(world, entity) else {
        return true;
    };

    if tick < agent.next_attack_tick {
        return true;
    }
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);

    let target_entity = player_entity(world);
    let mut spawned_knight = false;
    let rage_delay_elapsed = ai_state.next_state_tick == 0 || tick >= ai_state.next_state_tick;
    if ai_state.extra_byte < 4
        && active_summoned_monster_count_by_ai(world, lord_object_id, 97) == 0
        && (!ai_state.mode || rage_delay_elapsed)
    {
        if let Some(knight_name) = hell_lord_knight_name(ai_state.extra_byte) {
            let spawn_position = hell_lord_knight_spawn_position(world, position, entity);
            if queue_hell_lord_summon(
                world,
                entity,
                knight_name,
                spawn_position,
                tick,
                target_entity,
                Some(SummonedMonster {
                    summoner_object_id: lord_object_id,
                    visible_extra: false,
                    expire_tick: None,
                    require_summoner_within: None,
                    despawn_tick_after_death: None,
                    totem_master_object_id: None,
                    max_minions: None,
                }),
                Some(true),
            ) {
                ai_state.mode = true;
                spawned_knight = true;
            }
        }
    }

    if spawned_knight {
        world.entity_mut(entity).insert(Facing(MirDirection::Up));
        if let Some(packet) = monster_melee_attack_packet(world, entity, position, MirDirection::Up)
        {
            packets.push(packet);
        }
    }

    let bomb_count = active_monster_count_by_ai_near(world, 99, position, 25);
    let should_spawn_bomb = bomb_count < 5
        && (spawned_knight
            || deterministic_roll(
                tick,
                entity.index() as usize,
                usize::from(ai_state.extra_byte),
                3,
            ) == 0);
    if should_spawn_bomb {
        let bomb_position = hell_bomb_spawn_position(world, player_position, tick, entity);
        let _ = queue_hell_lord_summon(
            world,
            entity,
            hell_bomb_name_for_tick(tick),
            bomb_position,
            tick,
            target_entity,
            None,
            Some(true),
        );
    }

    true
}

pub(super) fn hell_lord_knight_name(stage: u8) -> Option<&'static str> {
    match stage {
        0 => Some("HellKnight1"),
        1 => Some("HellKnight2"),
        2 => Some("HellKnight3"),
        3 => Some("HellKnight4"),
        _ => None,
    }
}

pub(super) fn hell_bomb_name_for_tick(tick: u64) -> &'static str {
    match tick % 3 {
        0 => "HellBomb1",
        1 => "HellBomb2",
        _ => "HellBomb3",
    }
}

pub(super) fn hell_lord_knight_spawn_position(
    world: &World,
    lord_position: &Point,
    entity: Entity,
) -> Point {
    let front = offset_point(lord_position, MirDirection::DownLeft, 12);
    first_occupiable_point_near(world, &front, 10, Some(entity)).unwrap_or_else(|| {
        summon_spawn_position_near(
            world,
            lord_position,
            MirDirection::DownLeft,
            1,
            Some(entity),
        )
    })
}

pub(super) fn hell_bomb_spawn_position(
    world: &World,
    player_position: &Point,
    tick: u64,
    summoner_entity: Entity,
) -> Point {
    let directions = [
        MirDirection::Up,
        MirDirection::UpRight,
        MirDirection::Right,
        MirDirection::DownRight,
        MirDirection::Down,
        MirDirection::DownLeft,
        MirDirection::Left,
        MirDirection::UpLeft,
    ];
    let direction = directions[(tick as usize) % directions.len()];
    let distance = 5 + i32::try_from(tick % 4).expect("tick modulo fits i32");
    summon_spawn_position_near(
        world,
        player_position,
        direction,
        distance,
        Some(summoner_entity),
    )
}

pub(super) fn first_occupiable_point_near(
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

pub(super) fn queue_hell_lord_summon(
    world: &mut World,
    summoner_entity: Entity,
    monster_name: &str,
    position: Point,
    tick: u64,
    target_entity: Option<Entity>,
    summon_metadata: Option<SummonedMonster>,
    hostile_to_player_override: Option<bool>,
) -> bool {
    let Some(template) = crystal_dynamic_monster_template(monster_name) else {
        return false;
    };

    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: tick + combat_delay_ticks(500),
            summoner_entity,
            template: CrystalRespawnTemplate {
                location: position,
                ..template
            },
            target_entity,
            summon_metadata,
            hostile_to_player_override,
        },
    );
    true
}

pub(super) fn active_monster_count_by_ai_near(
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

pub(super) fn advance_world(world: &mut World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }

    let tick = advance_runtime_tick(world);
    process_crystal_npc_goods_expiry(world);
    let mut packets = Vec::new();
    process_expired_rental_items(world, &mut packets);
    if current_player_is_dead(world) {
        return_rented_items_on_player_death(world, &mut packets);
    }
    tick_buffs(world, &mut packets);
    sync_expired_expanded_storage(world, &mut packets);
    tick_crystal_normal_potion_restore(world, &mut packets);
    tick_crystal_normal_hero_potion_restore(world, &mut packets);
    tick_stage5_hero_auto_pot(world, tick, &mut packets);
    tick_stage5_hero_combat_ai(world, tick, &mut packets);
    tick_ground_drop_expiry(world, tick);
    tick_stage5_intelligent_creatures(world, tick, &mut packets);
    tick_fishing(world, &mut packets);
    resolve_pending_combat_actions(world, tick, &mut packets);
    tick_ground_spell_actions(world, tick, &mut packets);
    tick_monster_poisons(world, tick, &mut packets);
    emit_due_trainer_average_chats(world, tick, &mut packets);
    resolve_pending_monster_spawns(world, tick);
    let revived_entities = tick_respawns(world);
    let player = player_entity(world).expect("player should exist");
    let player_position = entity_position(world, player).expect("player position");

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();

    for &entity in &monster_entities {
        let (original_agent, original_ai_state, monster_name, position, current_direction) = {
            let view = world.entity(entity);
            let agent = view.get::<MonsterAgent>().expect("monster agent").clone();
            (
                agent,
                view.get::<MonsterAiState>().copied().unwrap_or_default(),
                view.get::<DisplayName>()
                    .expect("monster name")
                    .resolve(current_language(world)),
                view.get::<Position>().expect("monster position").0.clone(),
                view.get::<Facing>().expect("monster facing").0,
            )
        };
        let mut agent = original_agent.clone();
        let mut ai_state = original_ai_state;

        if update_special_monster_state(
            world,
            entity,
            &mut agent,
            &mut ai_state,
            &position,
            &player_position,
            tick,
            &mut packets,
        ) {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        if agent.dead {
            continue;
        }

        let monster_target = if monster_prefers_monster_target(world, entity, &agent) {
            summoned_monster_entity_target(world, &monster_entities, entity, &position, &agent)
        } else {
            hostile_monster_summon_target(world, &monster_entities, entity, &position, &agent)
        };

        if let Some(target_entity) = monster_target.or_else(|| {
            find_guard_target_monster(world, &monster_entities, entity, &position, &agent)
        }) {
            let target_position =
                entity_position(world, target_entity).expect("guard target position");

            if monster_can_attack(&agent, &ai_state)
                && monster_in_attack_range(&agent, &position, &target_position)
            {
                if let Some(direction) = direction_toward(&position, &target_position) {
                    if tick >= agent.next_attack_tick {
                        agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
                        let attack_direction =
                            monster_attack_packet_direction(&agent, current_direction, direction);
                        if agent.ai != 6 && current_direction != attack_direction {
                            world.entity_mut(entity).insert(Facing(attack_direction));
                        }
                        world.entity_mut(entity).insert(agent.clone());

                        if agent.ai == 6 {
                            if let Some(attack_packets) =
                                guard_melee_attack_packets(world, entity, target_entity)
                            {
                                packets.extend(attack_packets);
                            }
                        } else if !monster_prefers_ranged_when_not_adjacent(
                            &agent,
                            &position,
                            &target_position,
                        ) {
                            if let Some(packet) = monster_typed_attack_packet(
                                world,
                                entity,
                                &position,
                                attack_direction,
                                monster_object_attack_type(&agent, &position, &target_position),
                            ) {
                                packets.push(packet);
                            }
                        } else if let Some(packet) = monster_typed_ranged_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            target_entity,
                            &target_position,
                            monster_object_range_attack_type(&agent, &position, &target_position),
                        ) {
                            packets.push(packet);
                        }
                        if let Some(attacker_id) = entity_object_id(world, entity) {
                            let due_tick = tick
                                + monster_attack_delay_ticks(&agent, &position, &target_position);
                            let damage = summon_attack_damage(
                                &monster_name,
                                &agent,
                                tile_distance(&position, &target_position),
                            );
                            if damage > 0 {
                                schedule_damage_to_monster(
                                    world,
                                    due_tick,
                                    attacker_id,
                                    target_entity,
                                    damage,
                                    None,
                                    None,
                                );
                                if agent.ai == 18 {
                                    for line_target in shinsu_line_monster_targets(
                                        world,
                                        &monster_entities,
                                        entity,
                                        &position,
                                        direction,
                                        &agent,
                                    ) {
                                        if line_target == target_entity {
                                            continue;
                                        }
                                        schedule_damage_to_monster(
                                            world,
                                            due_tick,
                                            attacker_id,
                                            line_target,
                                            damage,
                                            None,
                                            None,
                                        );
                                    }
                                }
                            }
                        }
                    }

                    persist_monster_runtime_state(
                        world,
                        entity,
                        &original_agent,
                        &agent,
                        original_ai_state,
                        ai_state,
                    );

                    continue;
                }
            }

            if tick < agent.next_move_tick {
                persist_monster_runtime_state(
                    world,
                    entity,
                    &original_agent,
                    &agent,
                    original_ai_state,
                    ai_state,
                );
                continue;
            }

            let next = step_point_toward(&position, &target_position);
            if next != position && can_occupy(world, next.clone(), Some(entity)) {
                agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
                if let Some(direction) = direction_toward(&position, &next) {
                    world.entity_mut(entity).insert((
                        Position(next.clone()),
                        Facing(direction),
                        agent.clone(),
                    ));
                    packets.push(ServerPacket::ObjectWalk {
                        movement: ObjectMovement {
                            object_id: entity_object_id(world, entity)
                                .expect("moving monster object id"),
                            position: next,
                            direction,
                        },
                    });
                } else {
                    world
                        .entity_mut(entity)
                        .insert((Position(next), agent.clone()));
                }
                continue;
            }
        }

        let distance = tile_distance(&position, &player_position);
        if agent.tracking_player && distance > MONSTER_PLAYER_TARGET_RANGE {
            agent.tracking_player = false;
        }
        if agent.hostile_to_player && distance <= agent.view_range.max(1) {
            agent.tracking_player = true;
        }
        let player_in_aggro_range = agent.tracking_player;

        if monster_can_attack(&agent, &ai_state)
            && monster_in_attack_range(&agent, &position, &player_position)
            && player_in_aggro_range
            && !(agent.ai == 27 && distance > 1 && ai_state.next_state_tick > tick)
            && !(agent.ai == 20 && distance > 1 && ai_state.next_state_tick > tick)
            && !(agent.ai == 192 && distance > 1 && ai_state.next_state_tick > tick)
        {
            if tick >= agent.next_attack_tick {
                let Some(direction) = direction_toward(&position, &player_position) else {
                    persist_monster_runtime_state(
                        world,
                        entity,
                        &original_agent,
                        &agent,
                        original_ai_state,
                        ai_state,
                    );
                    continue;
                };

                let attack_direction =
                    monster_attack_packet_direction(&agent, current_direction, direction);
                if current_direction != attack_direction {
                    world.entity_mut(entity).insert(Facing(attack_direction));
                }

                let Some(attacker_id) = entity_object_id(world, entity) else {
                    persist_monster_runtime_state(
                        world,
                        entity,
                        &original_agent,
                        &agent,
                        original_ai_state,
                        ai_state,
                    );
                    continue;
                };

                if matches!(agent.ai, 12 | 39 | 62) {
                    let template = match agent.ai {
                        12 => bug_bat_template(),
                        39 => bomb_spider_template(),
                        62 => crystal_dynamic_monster_template("CharmedSnake"),
                        _ => None,
                    };
                    let Some(template) = template else {
                        persist_monster_runtime_state(
                            world,
                            entity,
                            &original_agent,
                            &agent,
                            original_ai_state,
                            ai_state,
                        );
                        continue;
                    };

                    let summon_cap = if agent.ai == 62 { 2 } else { 20 };
                    if active_summoned_monster_count(world, attacker_id) >= summon_cap {
                        persist_monster_runtime_state(
                            world,
                            entity,
                            &original_agent,
                            &agent,
                            original_ai_state,
                            ai_state,
                        );
                        continue;
                    }

                    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
                    world.entity_mut(entity).insert(agent.clone());
                    if let Some(packet) =
                        monster_melee_attack_packet(world, entity, &position, direction)
                    {
                        packets.push(packet);
                    }
                    let spawn_position = match agent.ai {
                        39 => match current_direction {
                            MirDirection::Up => offset_point(&position, MirDirection::Down, 1),
                            MirDirection::UpRight => {
                                offset_point(&position, MirDirection::DownRight, 1)
                            }
                            MirDirection::Right => {
                                offset_point(&position, MirDirection::DownLeft, 1)
                            }
                            _ => position.clone(),
                        },
                        62 => {
                            let fallback = offset_point(&position, MirDirection::Down, 1);
                            directional_destination(
                                world,
                                &position,
                                MirDirection::Down,
                                1,
                                Some(entity),
                            )
                            .unwrap_or(fallback)
                        }
                        _ => position.clone(),
                    };
                    let summon_metadata = if agent.ai == 62 {
                        Some(SummonedMonster {
                            summoner_object_id: attacker_id,
                            visible_extra: true,
                            expire_tick: Some(tick + combat_delay_ticks(10_000)),
                            require_summoner_within: Some(15),
                            despawn_tick_after_death: None,
                            totem_master_object_id: Some(attacker_id),
                            max_minions: Some(summon_cap),
                        })
                    } else {
                        None
                    };
                    queue_pending_monster_spawn(
                        world,
                        PendingMonsterSpawnAction {
                            due_tick: tick + combat_delay_ticks(500),
                            summoner_entity: entity,
                            template: CrystalRespawnTemplate {
                                location: spawn_position,
                                ..template
                            },
                            target_entity: Some(player),
                            summon_metadata,
                            hostile_to_player_override: Some(true),
                        },
                    );
                } else {
                    let armadillo_type_one_branch = matches!(agent.ai, 124 | 125) && tick % 6 == 1;
                    let armadillo_retreat_branch = matches!(agent.ai, 124 | 125) && tick % 6 == 0;
                    let hell_keeper_type_one_branch = agent.ai == 79 && tick % 3 == 1;
                    let snow_wolf_type_one_branch = agent.ai == 179 && tick % 5 == 1;
                    let frozen_miner_type_one_branch = agent.ai == 187 && tick % 8 == 1;
                    let frozen_magician_type_one_branch = agent.ai == 189
                        && tile_distance(&position, &player_position) > 1
                        && tick % 3 == 1;
                    let frozen_warewolf_hp_percent = if agent.ai == 180 {
                        world
                            .entity(entity)
                            .get::<MonsterVitals>()
                            .map(|vitals| vitals.hp * 100 / vitals.max_hp.max(1))
                            .unwrap_or(100)
                    } else {
                        100
                    };
                    let frozen_warewolf_variant_branch =
                        agent.ai == 180 && deterministic_chance_roll(tick, attacker_id, 1800, 3);
                    let frozen_warewolf_attack_type = if frozen_warewolf_variant_branch {
                        if frozen_warewolf_hp_percent >= 60 {
                            1
                        } else if frozen_warewolf_hp_percent >= 30 {
                            2
                        } else {
                            3
                        }
                    } else {
                        0
                    };
                    let lamia_kirin_type_one_branch = agent.ai == 186 && tick % 5 == 0;
                    let dark_beast_secondary_branch = agent.ai == 112 && tick % 5 == 0;
                    let seedings_general_stomp_branch =
                        agent.ai == 121 && distance > 1 && tick % 5 == 0;
                    let seedings_general_close_splash_branch =
                        agent.ai == 121 && distance == 1 && tick % 5 == 0;
                    let dark_devil_range_branch = agent.ai == 20 && distance > 1;
                    let minotaur_king_range_branch = agent.ai == 33 && distance > 1;
                    let general_meow_meow_range_branch = agent.ai == 123 && distance > 2;
                    let general_meow_meow_slam_branch =
                        agent.ai == 123 && distance <= 2 && tick % 9 == 0;
                    let manectric_claw_thrust_branch = agent.ai == 86 && distance > 1;
                    let guardian_rock_pull_movement = if agent.ai == 48 && distance > 1 {
                        direction_toward(&player_position, &position).map(|direction| {
                            PendingPlayerMovement {
                                direction,
                                distance: (distance - 1).clamp(1, 4),
                            }
                        })
                    } else {
                        None
                    };
                    let tucson_general_type_two_range_branch =
                        agent.ai == 131 && distance > 2 && tick % 4 == 0;
                    let tucson_general_stomp_branch =
                        agent.ai == 131 && distance <= 2 && tick % 4 == 0;
                    let cat_shaman_red_poison_branch =
                        agent.ai == 118 && distance > 1 && tick % 5 == 0;
                    let black_tortoise_halfmoon_branch =
                        agent.ai == 182 && distance == 1 && tick % 5 == 0;
                    let red_foxman_type_one_range_branch =
                        agent.ai == 45 && distance > 1 && tick % 2 == 0;
                    let white_foxman_slow_branch = agent.ai == 46 && distance > 1 && tick % 8 == 0;
                    let stray_cat_push_branch = agent.ai == 117 && distance == 1 && tick % 10 == 0;
                    let manectric_king_hp_percent = if agent.ai == 88 {
                        world
                            .entity(entity)
                            .get::<MonsterVitals>()
                            .map(|vitals| vitals.hp * 100 / vitals.max_hp.max(1))
                            .unwrap_or(100)
                    } else {
                        100
                    };
                    let manectric_king_mass_attack_branch = agent.ai == 88
                        && manectric_king_hp_percent < 20
                        && (ai_state.next_state_tick == 0 || tick >= ai_state.next_state_tick);
                    let manectric_king_push_line_branch = agent.ai == 88
                        && distance <= 2
                        && tick % 3 == 0
                        && !manectric_king_mass_attack_branch;
                    let great_fox_spirit_area_branch = agent.ai == 50;
                    let yimoogi_poison_branch = agent.ai == 36 && distance <= 4 && tick % 6 == 0;
                    let trap_rock_parent_paralysis_branch = agent.ai == 47
                        && world.entity(entity).get::<SummonedMonster>().is_none()
                        && deterministic_chance_roll(
                            tick,
                            attacker_id,
                            47,
                            TRAP_ROCK_ATTACK_PARALYSIS_CHANCE_DENOMINATOR,
                        );
                    let dark_wraith_line_branch = agent.ai == 192 && distance > 1;
                    let spitting_spider_line_branch = matches!(agent.ai, 4 | 35);
                    let crystal_spider_line_branch = agent.ai == 37 && distance > 1;
                    let king_scorpion_line_targets = if agent.ai == 19 {
                        forward_line_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                            2,
                        )
                    } else {
                        Vec::new()
                    };
                    let king_scorpion_second_tile = offset_point(&position, attack_direction, 2);
                    let king_scorpion_forced_range_branch = agent.ai == 19
                        && king_scorpion_line_targets.iter().any(|target| {
                            entity_position(world, *target)
                                .map(|target_position| target_position == king_scorpion_second_tile)
                                .unwrap_or(false)
                        });
                    let king_scorpion_range_branch = agent.ai == 19
                        && (distance > 1 || king_scorpion_forced_range_branch || tick % 5 == 0);
                    let red_moon_evil_area_branch = agent.ai == 13;
                    let evil_centipede_area_branch = agent.ai == 14;
                    let oma_king_close_line_branch =
                        agent.ai == 43 && distance <= 2 && tick % 3 != 0;
                    let oma_king_type_one_magic_branch =
                        agent.ai == 43 && !oma_king_close_line_branch;
                    let ice_guard_fire_range_branch =
                        agent.ai == 102 && distance > 1 && tick % 3 == 0;
                    let ice_guard_ice_range_branch =
                        agent.ai == 102 && distance > 1 && !ice_guard_fire_range_branch;
                    let khazard_pull_branch = agent.ai == 27 && distance > 1;
                    let frozen_axeman_pull_branch = agent.ai == 188
                        && distance == 1
                        && tick % 3 != 0
                        && (ai_state.next_state_tick == 0 || tick >= ai_state.next_state_tick);
                    let dark_wraith_adjacent_area_targets = if agent.ai == 192 && distance == 1 {
                        nearby_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            &agent,
                            1,
                        )
                    } else {
                        Vec::new()
                    };
                    let dark_wraith_adjacent_area_branch =
                        !dark_wraith_adjacent_area_targets.is_empty() && tick % 2 == 1;
                    let snow_yeti_adjacent_double_hit_branch = agent.ai == 190 && distance == 1;
                    let turtle_grass_single_push_branch = agent.ai == 173 && tick % 4 == 0;
                    let cannibal_tentacles_halfmoon_branch =
                        agent.ai == 130 && distance == 1 && tick % 5 == 0;
                    let jar2_adjacent_melee_branch =
                        agent.ai == 120 && distance == 1 && tick % 3 == 0;
                    let jar2_adjacent_range_branch =
                        agent.ai == 120 && distance == 1 && !jar2_adjacent_melee_branch;
                    let sand_snail_halfmoon_branch = agent.ai == 115 && tick % 7 == 0;
                    let sand_snail_green_area_branch =
                        agent.ai == 115 && !sand_snail_halfmoon_branch && tick % 2 == 0;
                    let man_tree_boulder_branch = agent.ai == 174 && tick % 8 == 0;
                    let man_tree_halfmoon_branch =
                        agent.ai == 174 && !man_tree_boulder_branch && tick % 4 == 0;
                    let restless_jar_hp_percent = if agent.ai == 122 {
                        world
                            .entity(entity)
                            .get::<MonsterVitals>()
                            .map(|vitals| vitals.hp * 100 / vitals.max_hp.max(1))
                            .unwrap_or(100)
                    } else {
                        100
                    };
                    let restless_jar_special_adjacent_branch =
                        agent.ai == 122 && distance == 1 && tick % 3 == 2;
                    let restless_jar_tornado_branch =
                        restless_jar_special_adjacent_branch && restless_jar_hp_percent >= 50;
                    let restless_jar_stomp_branch =
                        restless_jar_special_adjacent_branch && restless_jar_hp_percent < 50;
                    let restless_jar_spin_branch = agent.ai == 122
                        && distance == 1
                        && !restless_jar_tornado_branch
                        && !restless_jar_stomp_branch;
                    let tucson_warrior_adjacent_smash_branch =
                        agent.ai == 127 && distance == 1 && tick % 5 == 0;
                    let tucson_warrior_adjacent_halfmoon_branch =
                        agent.ai == 127 && distance == 1 && !tucson_warrior_adjacent_smash_branch;
                    let tucson_warrior_smash_branch =
                        agent.ai == 127 && (distance > 1 || tucson_warrior_adjacent_smash_branch);
                    let tucson_mage_wide_line_branch =
                        agent.ai == 126 && (distance > 1 || tick % 3 == 0);
                    let cannibal_tentacles_halfmoon_targets = if cannibal_tentacles_halfmoon_branch
                    {
                        halfmoon_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        )
                    } else {
                        Vec::new()
                    };
                    let sand_snail_halfmoon_targets = if sand_snail_halfmoon_branch {
                        halfmoon_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        )
                    } else {
                        Vec::new()
                    };
                    let man_tree_halfmoon_targets = if man_tree_halfmoon_branch {
                        halfmoon_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        )
                    } else {
                        Vec::new()
                    };
                    let tucson_warrior_halfmoon_targets = if tucson_warrior_adjacent_halfmoon_branch
                    {
                        halfmoon_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        )
                    } else {
                        Vec::new()
                    };
                    let manectric_claw_thrust_move_branch = manectric_claw_thrust_branch
                        && deterministic_chance_roll(tick, attacker_id, 862, 2);
                    if manectric_claw_thrust_move_branch {
                        agent.next_attack_tick = tick + combat_delay_ticks(300);
                        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
                        if let Some((destination, direction)) = monster_step_toward_with_fallback(
                            world,
                            entity,
                            &position,
                            &player_position,
                            tick,
                            86,
                        ) {
                            world.entity_mut(entity).insert((
                                Position(destination.clone()),
                                Facing(direction),
                                agent.clone(),
                            ));
                            packets.push(ServerPacket::ObjectWalk {
                                movement: ObjectMovement {
                                    object_id: attacker_id,
                                    position: destination,
                                    direction,
                                },
                            });
                        } else {
                            world.entity_mut(entity).insert(agent.clone());
                        }
                        persist_monster_runtime_state(
                            world,
                            entity,
                            &original_agent,
                            &agent,
                            original_ai_state,
                            ai_state,
                        );
                        continue;
                    }
                    let final_damage = if hell_keeper_type_one_branch {
                        let base_damage = crystal_monster_raw_magic_damage(&monster_name);
                        if base_damage <= 0 {
                            0
                        } else {
                            let mitigation = total_defence_bonus(
                                world.resource::<InventoryResource>(),
                                world.resource::<BuffResource>(),
                            );
                            (base_damage - mitigation).max(1)
                        }
                    } else if yimoogi_poison_branch {
                        0
                    } else if dark_beast_secondary_branch {
                        crystal_monster_raw_magic_damage(&monster_name)
                    } else if snow_wolf_type_one_branch {
                        let base_damage = crystal_monster_raw_magic_damage(&monster_name);
                        if base_damage <= 0 {
                            0
                        } else {
                            let mitigation = total_defence_bonus(
                                world.resource::<InventoryResource>(),
                                world.resource::<BuffResource>(),
                            );
                            (base_damage - mitigation).max(1)
                        }
                    } else if frozen_miner_type_one_branch {
                        let damage = crystal_monster_raw_attack_damage(&monster_name);
                        if damage > 0 {
                            (damage * 8 / 10).max(1)
                        } else {
                            0
                        }
                    } else if frozen_magician_type_one_branch {
                        crystal_monster_raw_magic_damage(&monster_name) * 3 / 2
                    } else if armadillo_type_one_branch && agent.ai == 124 {
                        (crystal_monster_attack_damage(&monster_name) / 2).max(1)
                    } else if armadillo_type_one_branch && agent.ai == 125 {
                        0
                    } else if cannibal_tentacles_halfmoon_branch {
                        let mitigation = total_defence_bonus(
                            world.resource::<InventoryResource>(),
                            world.resource::<BuffResource>(),
                        );
                        (CANNIBAL_TENTACLES_HALFMOON_DAMAGE - mitigation).max(1)
                    } else if king_scorpion_range_branch {
                        crystal_monster_magic_damage(&monster_name)
                    } else if jar2_adjacent_range_branch {
                        crystal_monster_raw_magic_damage(&monster_name)
                    } else if sand_snail_green_area_branch {
                        crystal_monster_magic_damage(&monster_name)
                    } else if seedings_general_close_splash_branch {
                        let mitigation = total_defence_bonus(
                            world.resource::<InventoryResource>(),
                            world.resource::<BuffResource>(),
                        );
                        (crystal_monster_magic_damage(&monster_name) - mitigation).max(1)
                    } else if man_tree_boulder_branch {
                        crystal_monster_raw_magic_damage(&monster_name)
                    } else if restless_jar_stomp_branch {
                        crystal_monster_raw_attack_damage(&monster_name)
                    } else if tucson_warrior_adjacent_smash_branch {
                        crystal_monster_magic_damage(&monster_name)
                    } else if general_meow_meow_slam_branch {
                        let mitigation = total_defence_bonus(
                            world.resource::<InventoryResource>(),
                            world.resource::<BuffResource>(),
                        );
                        (crystal_monster_attack_damage(&monster_name) * 3 - mitigation).max(1)
                    } else if tucson_general_type_two_range_branch {
                        let mitigation = total_defence_bonus(
                            world.resource::<InventoryResource>(),
                            world.resource::<BuffResource>(),
                        );
                        (crystal_monster_spell_damage(&monster_name) * 2 - mitigation).max(1)
                    } else if tucson_general_stomp_branch {
                        let mitigation = total_defence_bonus(
                            world.resource::<InventoryResource>(),
                            world.resource::<BuffResource>(),
                        );
                        (crystal_monster_magic_damage(&monster_name) - mitigation).max(1)
                    } else if white_foxman_slow_branch {
                        0
                    } else if stray_cat_push_branch {
                        crystal_monster_raw_magic_damage(&monster_name)
                    } else if tucson_mage_wide_line_branch {
                        let base_damage = crystal_monster_raw_magic_damage(&monster_name);
                        if base_damage <= 0 {
                            0
                        } else {
                            let mitigation = total_defence_bonus(
                                world.resource::<InventoryResource>(),
                                world.resource::<BuffResource>(),
                            );
                            (base_damage - mitigation).max(1)
                        }
                    } else if manectric_king_mass_attack_branch {
                        let mitigation = total_defence_bonus(
                            world.resource::<InventoryResource>(),
                            world.resource::<BuffResource>(),
                        );
                        (crystal_monster_magic_damage(&monster_name) - mitigation).max(1)
                    } else if manectric_king_push_line_branch {
                        let mitigation = total_defence_bonus(
                            world.resource::<InventoryResource>(),
                            world.resource::<BuffResource>(),
                        );
                        (crystal_monster_attack_damage(&monster_name) - mitigation).max(1)
                    } else if oma_king_type_one_magic_branch {
                        crystal_monster_magic_damage(&monster_name)
                    } else {
                        monster_player_attack_damage(
                            world,
                            &monster_name,
                            &agent,
                            &position,
                            &player_position,
                        )
                    };
                    let attack_type = if armadillo_type_one_branch
                        || hell_keeper_type_one_branch
                        || snow_wolf_type_one_branch
                        || frozen_miner_type_one_branch
                        || frozen_magician_type_one_branch
                        || lamia_kirin_type_one_branch
                        || dark_beast_secondary_branch
                        || yimoogi_poison_branch
                        || seedings_general_close_splash_branch
                        || turtle_grass_single_push_branch
                        || cannibal_tentacles_halfmoon_branch
                        || sand_snail_halfmoon_branch
                        || man_tree_halfmoon_branch
                        || restless_jar_tornado_branch
                        || tucson_warrior_adjacent_smash_branch
                        || general_meow_meow_slam_branch
                        || tucson_general_stomp_branch
                        || black_tortoise_halfmoon_branch
                        || stray_cat_push_branch
                        || tucson_mage_wide_line_branch
                        || manectric_king_push_line_branch
                        || oma_king_type_one_magic_branch
                    {
                        1
                    } else if dark_wraith_adjacent_area_branch {
                        1
                    } else if frozen_axeman_pull_branch
                        || sand_snail_green_area_branch
                        || man_tree_boulder_branch
                        || restless_jar_stomp_branch
                    {
                        2
                    } else if agent.ai == 180 {
                        frozen_warewolf_attack_type
                    } else {
                        monster_object_attack_type(&agent, &position, &player_position)
                    };
                    let yimoogi_ranged_attack_pause =
                        if agent.ai == 36 && (distance > 2 || yimoogi_poison_branch) {
                            combat_delay_ticks(500)
                        } else {
                            0
                        };
                    agent.next_attack_tick =
                        tick + agent.attack_interval_ticks.max(1) + yimoogi_ranged_attack_pause;
                    if frozen_axeman_pull_branch {
                        ai_state.next_state_tick = tick + combat_delay_ticks(10_000);
                    }
                    if khazard_pull_branch {
                        ai_state.next_state_tick = tick + combat_delay_ticks(5_000);
                    }
                    if dark_devil_range_branch {
                        ai_state.next_state_tick =
                            tick + combat_delay_ticks(2_000 + (tick % 3) * 1_000);
                    }
                    if dark_wraith_line_branch {
                        ai_state.next_state_tick =
                            tick + combat_delay_ticks(3_000 + (tick % 5) * 1_000);
                    }
                    if manectric_king_mass_attack_branch {
                        ai_state.next_state_tick =
                            tick + combat_delay_ticks(2_000 + (tick % 5) * 1_000);
                    }
                    if agent.ai == 180 && frozen_warewolf_hp_percent < 70 && !ai_state.mode {
                        ai_state.mode = true;
                        spawn_snow_wolf_king_slaves(
                            world,
                            attacker_id,
                            &position,
                            attack_direction,
                            player,
                            &player_position,
                            &agent,
                        );
                    }
                    if armadillo_retreat_branch {
                        let retreat_position = offset_point(&position, attack_direction, -2);
                        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
                        if agent.ai == 125 {
                            ai_state.mode = true;
                        }
                        world.entity_mut(entity).insert((
                            Position(retreat_position.clone()),
                            Facing(attack_direction),
                            agent.clone(),
                        ));
                        packets.push(ServerPacket::ObjectBackStep {
                            movement: ObjectMovement {
                                object_id: attacker_id,
                                position: retreat_position.clone(),
                                direction: attack_direction,
                            },
                            distance: 2,
                        });
                        if agent.ai == 124 {
                            let retreat_damage = crystal_monster_raw_attack_damage(&monster_name);
                            if retreat_damage > 0 {
                                let due_tick = tick + combat_delay_ticks(900);
                                if tile_distance(&retreat_position, &player_position) <= 2 {
                                    schedule_damage_to_player(
                                        world,
                                        due_tick,
                                        attacker_id,
                                        monster_name.clone(),
                                        retreat_damage,
                                    );
                                }
                                for area_target in nearby_opposing_monster_targets(
                                    world,
                                    &monster_entities,
                                    entity,
                                    &retreat_position,
                                    &agent,
                                    2,
                                ) {
                                    if monster_ignores_damage(world, area_target) {
                                        ai_state.mode = true;
                                    }
                                    schedule_damage_to_monster(
                                        world,
                                        due_tick,
                                        attacker_id,
                                        area_target,
                                        retreat_damage,
                                        None,
                                        None,
                                    );
                                }
                            }
                        }
                        persist_monster_runtime_state(
                            world,
                            entity,
                            &original_agent,
                            &agent,
                            original_ai_state,
                            ai_state,
                        );
                        continue;
                    }
                    world.entity_mut(entity).insert(agent.clone());
                    let trap_rock_child =
                        agent.ai == 47 && world.entity(entity).get::<SummonedMonster>().is_some();
                    let attack_packet = if snow_yeti_adjacent_double_hit_branch {
                        if let Some(packet) = monster_typed_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            0,
                        ) {
                            packets.push(packet);
                        }
                        if let Some(packet) = monster_typed_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            1,
                        ) {
                            packets.push(packet);
                        }
                        None
                    } else if !trap_rock_child
                        && !yimoogi_poison_branch
                        && (king_scorpion_range_branch
                            || restless_jar_tornado_branch
                            || jar2_adjacent_range_branch
                            || manectric_king_mass_attack_branch
                            || monster_prefers_ranged_when_not_adjacent(
                                &agent,
                                &position,
                                &player_position,
                            ))
                    {
                        monster_typed_ranged_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            player,
                            &player_position,
                            if frozen_magician_type_one_branch
                                || ice_guard_fire_range_branch
                                || seedings_general_stomp_branch
                                || restless_jar_tornado_branch
                            {
                                1
                            } else if cat_shaman_red_poison_branch {
                                1
                            } else if tucson_general_type_two_range_branch {
                                2
                            } else if red_foxman_type_one_range_branch {
                                1
                            } else if white_foxman_slow_branch {
                                1
                            } else {
                                monster_object_range_attack_type(
                                    &agent,
                                    &position,
                                    &player_position,
                                )
                            },
                        )
                    } else {
                        monster_typed_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            attack_type,
                        )
                    };
                    let due_packet = if monster_broadcasts_attack_on_damage_due(&agent) {
                        attack_packet
                    } else {
                        if let Some(packet) = attack_packet {
                            packets.push(packet);
                        }
                        None
                    };
                    if white_foxman_slow_branch {
                        schedule_player_status_effect(
                            world,
                            tick + combat_delay_ticks(300),
                            attacker_id,
                            PendingPlayerStatusEffect::WhiteFoxmanSlow {
                                duration_ticks: WHITE_FOXMAN_SLOW_DURATION_TICKS,
                            },
                        );
                    }
                    if stray_cat_push_branch {
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            1,
                            &mut packets,
                        );
                    }
                    if manectric_king_push_line_branch {
                        let line_distance = (4 - distance).max(1);
                        let push_distance = (line_distance - 1).max(1);
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            push_distance,
                            &mut packets,
                        );
                    }
                    if khazard_pull_branch {
                        if let Some(pull_direction) = direction_toward(&player_position, &position)
                        {
                            let _ = push_player_in_direction(
                                world,
                                player,
                                pull_direction,
                                distance,
                                &mut packets,
                            );
                        }
                    }
                    if turtle_grass_single_push_branch {
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            3,
                            &mut packets,
                        );
                    }
                    if armadillo_type_one_branch && agent.ai == 125 {
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            2,
                            &mut packets,
                        );
                    }
                    if yimoogi_poison_branch {
                        apply_player_red_poison(world, tick, YIMOOGI_RED_POISON_DURATION_TICKS);
                    }
                    if trap_rock_parent_paralysis_branch {
                        apply_player_paralysis(world, tick, TRAP_ROCK_PARALYSIS_DURATION_TICKS);
                    }
                    if restless_jar_stomp_branch {
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            1,
                            &mut packets,
                        );
                    }
                    if snow_yeti_adjacent_double_hit_branch && final_damage > 0 {
                        for delay_ms in [500, 1_500] {
                            schedule_damage_to_player(
                                world,
                                tick + combat_delay_ticks(delay_ms),
                                attacker_id,
                                monster_name.clone(),
                                final_damage,
                            );
                        }
                    } else if dark_wraith_adjacent_area_branch && final_damage > 0 {
                        let due_tick = tick + combat_delay_ticks(600);
                        schedule_damage_to_player(
                            world,
                            due_tick,
                            attacker_id,
                            monster_name.clone(),
                            final_damage,
                        );
                        for area_target in dark_wraith_adjacent_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                    } else if frozen_axeman_pull_branch && final_damage > 0 {
                        let push_distance = 2 + i32::try_from(tick % 3).unwrap_or(0);
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            push_distance,
                            &mut packets,
                        );
                        schedule_damage_to_player(
                            world,
                            tick + combat_delay_ticks(500),
                            attacker_id,
                            monster_name.clone(),
                            final_damage,
                        );
                    } else if armadillo_type_one_branch && agent.ai == 124 && final_damage > 0 {
                        for delay_ms in [400, 600, 800] {
                            schedule_damage_to_player(
                                world,
                                tick + combat_delay_ticks(delay_ms),
                                attacker_id,
                                monster_name.clone(),
                                final_damage,
                            );
                        }
                    } else if oma_king_close_line_branch && final_damage > 0 {
                        let pushed_player = if distance <= 1 {
                            push_player_in_direction(
                                world,
                                player,
                                attack_direction,
                                3 + i32::try_from(tick % 3).unwrap_or(0),
                                &mut packets,
                            )
                            .is_some()
                        } else {
                            false
                        };
                        if pushed_player && tick % 8 == 0 {
                            apply_player_paralysis(
                                world,
                                tick,
                                CAVE_MAGGOT_PARALYSIS_DURATION_TICKS,
                            );
                        }

                        let due_tick = tick + combat_delay_ticks(300);
                        let line_tiles = [
                            offset_point(&position, attack_direction, 1),
                            offset_point(&position, attack_direction, 2),
                        ];
                        if entity_position(world, player)
                            .map(|point| line_tiles.contains(&point))
                            .unwrap_or(false)
                        {
                            schedule_damage_to_player(
                                world,
                                due_tick,
                                attacker_id,
                                monster_name.clone(),
                                final_damage,
                            );
                        }
                        for line_target in shinsu_line_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        ) {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                line_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                    } else if agent.ai == 48 {
                        schedule_damage_to_player_with_effect_due_packet_and_movement(
                            world,
                            tick + monster_attack_delay_ticks(&agent, &position, &player_position),
                            attacker_id,
                            monster_name.clone(),
                            0,
                            None,
                            due_packet,
                            guardian_rock_pull_movement,
                        );
                    } else if final_damage > 0 {
                        let due_tick = if frozen_miner_type_one_branch {
                            tick + combat_delay_ticks(1_000)
                        } else if frozen_magician_type_one_branch {
                            let distance =
                                u64::try_from(tile_distance(&position, &player_position).max(0))
                                    .expect("tile distance should fit u64");
                            tick + combat_delay_ticks(distance * 50 + 750)
                        } else if lamia_kirin_type_one_branch {
                            tick + combat_delay_ticks(500)
                        } else if snow_wolf_type_one_branch {
                            tick + combat_delay_ticks(450)
                        } else if turtle_grass_single_push_branch {
                            tick + combat_delay_ticks(500)
                        } else if cannibal_tentacles_halfmoon_branch {
                            tick + combat_delay_ticks(500)
                        } else if jar2_adjacent_range_branch {
                            tick + combat_delay_ticks(500)
                        } else if oma_king_type_one_magic_branch {
                            tick + combat_delay_ticks(500)
                        } else if manectric_king_mass_attack_branch {
                            tick + manectric_king_mass_attack_delay_ticks(
                                &position,
                                &player_position,
                            )
                        } else if tucson_general_type_two_range_branch {
                            tick + combat_delay_ticks(500)
                        } else {
                            tick + monster_attack_delay_ticks(&agent, &position, &player_position)
                        };
                        let area_monster_targets = if frozen_miner_type_one_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let seedings_general_area_targets = if seedings_general_stomp_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                2,
                            )
                        } else {
                            Vec::new()
                        };
                        let sand_snail_green_area_targets = if sand_snail_green_area_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let man_tree_boulder_area_targets = if man_tree_boulder_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let tucson_warrior_smash_targets = if tucson_warrior_smash_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let tucson_mage_wide_line_targets =
                            if tucson_mage_wide_line_branch && final_damage > 0 {
                                tucson_mage_wide_line_opposing_monster_targets(
                                    world,
                                    &monster_entities,
                                    entity,
                                    &position,
                                    attack_direction,
                                    &agent,
                                )
                            } else {
                                Vec::new()
                            };
                        let hell_keeper_area_targets = if agent.ai == 79 && final_damage > 0 {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                agent.view_range.max(1),
                            )
                        } else {
                            Vec::new()
                        };
                        let snow_wolf_area_targets = if agent.ai == 179 && final_damage > 0 {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                2,
                            )
                        } else {
                            Vec::new()
                        };
                        let tucson_general_stomp_targets = if tucson_general_stomp_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                3,
                            )
                        } else {
                            Vec::new()
                        };
                        let black_tortoise_halfmoon_targets = if black_tortoise_halfmoon_branch {
                            halfmoon_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                attack_direction,
                                &agent,
                            )
                        } else {
                            Vec::new()
                        };
                        let restless_jar_area_targets =
                            if restless_jar_spin_branch || restless_jar_stomp_branch {
                                nearby_opposing_monster_targets(
                                    world,
                                    &monster_entities,
                                    entity,
                                    &position,
                                    &agent,
                                    1,
                                )
                            } else {
                                Vec::new()
                            };
                        let minotaur_king_area_targets = if minotaur_king_range_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                3,
                            )
                        } else {
                            Vec::new()
                        };
                        let general_meow_meow_area_targets = if general_meow_meow_range_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                2,
                            )
                        } else {
                            Vec::new()
                        };
                        let mir_statue_area_targets = if agent.ai == 54 {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                2,
                            )
                        } else {
                            Vec::new()
                        };
                        let manectric_king_mass_targets = if manectric_king_mass_attack_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                7,
                            )
                        } else {
                            Vec::new()
                        };
                        let manectric_claw_cone_targets = if manectric_claw_thrust_branch {
                            manectric_claw_cone_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                attack_direction,
                                &agent,
                            )
                        } else {
                            Vec::new()
                        };
                        let dark_devil_area_targets = if dark_devil_range_branch {
                            let area_center = offset_point(&position, attack_direction, 2);
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &area_center,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let dark_wraith_line_targets = if dark_wraith_line_branch {
                            forward_line_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                attack_direction,
                                &agent,
                                4,
                            )
                        } else {
                            Vec::new()
                        };
                        let spider_line_targets =
                            if spitting_spider_line_branch || crystal_spider_line_branch {
                                forward_line_opposing_monster_targets(
                                    world,
                                    &monster_entities,
                                    entity,
                                    &position,
                                    attack_direction,
                                    &agent,
                                    if crystal_spider_line_branch { 3 } else { 2 },
                                )
                            } else {
                                Vec::new()
                            };
                        let red_moon_evil_area_targets = if red_moon_evil_area_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                agent.view_range.max(1),
                            )
                        } else {
                            Vec::new()
                        };
                        let great_fox_spirit_area_targets = if great_fox_spirit_area_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                if distance > 2 { 7 } else { 2 },
                            )
                        } else {
                            Vec::new()
                        };
                        let evil_centipede_area_targets = if evil_centipede_area_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                7,
                            )
                        } else {
                            Vec::new()
                        };
                        if red_moon_evil_area_branch {
                            if let Some(packet) =
                                object_effect_packet(world, player, SPELL_EFFECT_RED_MOON_EVIL)
                            {
                                packets.push(packet);
                            }
                            for area_target in &red_moon_evil_area_targets {
                                if let Some(packet) = object_effect_packet(
                                    world,
                                    *area_target,
                                    SPELL_EFFECT_RED_MOON_EVIL,
                                ) {
                                    packets.push(packet);
                                }
                            }
                        }
                        if great_fox_spirit_area_branch && distance > 2 {
                            if let Some(packet) =
                                object_effect_packet(world, player, SPELL_EFFECT_GREAT_FOX_SPIRIT)
                            {
                                packets.push(packet);
                            }
                            for area_target in &great_fox_spirit_area_targets {
                                if let Some(packet) = object_effect_packet(
                                    world,
                                    *area_target,
                                    SPELL_EFFECT_GREAT_FOX_SPIRIT,
                                ) {
                                    packets.push(packet);
                                }
                            }
                        }
                        schedule_damage_to_player_with_effect_due_packet_and_movement(
                            world,
                            due_tick,
                            attacker_id,
                            monster_name.clone(),
                            final_damage,
                            if hell_keeper_type_one_branch && final_damage > 0 {
                                Some(PendingPlayerStatusEffect::Dazed {
                                    duration_ticks: HELL_KEEPER_DAZED_DURATION_TICKS,
                                })
                            } else if dark_beast_secondary_branch
                                && crystal_monster_effect_for_name(&monster_name) == 1
                            {
                                Some(PendingPlayerStatusEffect::BleedingPoison {
                                    chance_denominator: 5,
                                    duration_ticks: 5,
                                    salt: 112,
                                })
                            } else if ice_guard_ice_range_branch {
                                Some(PendingPlayerStatusEffect::SlowAndFrozen {
                                    slow_chance_denominator: ICE_GUARD_SLOW_CHANCE_DENOMINATOR,
                                    slow_duration_ticks: ICE_GUARD_SLOW_DURATION_TICKS,
                                    slow_salt: 102,
                                    frozen_chance_denominator: ICE_GUARD_FROZEN_CHANCE_DENOMINATOR,
                                    frozen_duration_ticks: ICE_GUARD_FROZEN_DURATION_TICKS,
                                    frozen_salt: 103,
                                })
                            } else if agent.ai == 86 && distance > 1 {
                                Some(PendingPlayerStatusEffect::SlowAndFrozen {
                                    slow_chance_denominator: 5,
                                    slow_duration_ticks: 4,
                                    slow_salt: 860,
                                    frozen_chance_denominator: 5,
                                    frozen_duration_ticks: 2,
                                    frozen_salt: 861,
                                })
                            } else if snow_wolf_type_one_branch && final_damage > 0 {
                                Some(PendingPlayerStatusEffect::SlowAndFrozen {
                                    slow_chance_denominator: SNOW_WOLF_SLOW_CHANCE_DENOMINATOR,
                                    slow_duration_ticks: SNOW_WOLF_SLOW_DURATION_TICKS,
                                    slow_salt: 179,
                                    frozen_chance_denominator: SNOW_WOLF_FROZEN_CHANCE_DENOMINATOR,
                                    frozen_duration_ticks: SNOW_WOLF_FROZEN_DURATION_TICKS,
                                    frozen_salt: 179,
                                })
                            } else if agent.ai == 190 && distance > 1 {
                                Some(PendingPlayerStatusEffect::FrozenPoison {
                                    chance_denominator: SNOW_YETI_FROZEN_CHANCE_DENOMINATOR,
                                    duration_ticks: SNOW_YETI_FROZEN_DURATION_TICKS,
                                    salt: 190,
                                })
                            } else if seedings_general_stomp_branch {
                                Some(PendingPlayerStatusEffect::FrozenPoison {
                                    chance_denominator: SEEDINGS_GENERAL_POISON_CHANCE_DENOMINATOR,
                                    duration_ticks: SEEDINGS_GENERAL_POISON_DURATION_TICKS,
                                    salt: 1211,
                                })
                            } else if cannibal_tentacles_halfmoon_branch {
                                Some(PendingPlayerStatusEffect::GreenPoison {
                                    chance_denominator: 1,
                                    duration_ticks: CANNIBAL_TENTACLES_GREEN_POISON_DURATION_TICKS,
                                })
                            } else if agent.ai == 120
                                && (distance > 1 || jar2_adjacent_range_branch)
                            {
                                Some(PendingPlayerStatusEffect::FrozenPoison {
                                    chance_denominator: JAR2_FROZEN_CHANCE_DENOMINATOR,
                                    duration_ticks: JAR2_FROZEN_DURATION_TICKS,
                                    salt: 120,
                                })
                            } else if sand_snail_green_area_branch {
                                Some(PendingPlayerStatusEffect::GreenPoison {
                                    chance_denominator: 1,
                                    duration_ticks: SAND_SNAIL_GREEN_POISON_DURATION_TICKS,
                                })
                            } else if man_tree_boulder_branch {
                                Some(PendingPlayerStatusEffect::StunPoison {
                                    chance_denominator: MAN_TREE_STUN_CHANCE_DENOMINATOR,
                                    duration_ticks: MAN_TREE_STUN_DURATION_TICKS,
                                    salt: 174,
                                })
                            } else if agent.ai == 50 {
                                Some(PendingPlayerStatusEffect::SlowAndParalysis {
                                    slow_chance_denominator:
                                        GREAT_FOX_SPIRIT_SLOW_CHANCE_DENOMINATOR,
                                    slow_duration_ticks: GREAT_FOX_SPIRIT_SLOW_DURATION_TICKS,
                                    slow_salt: 500,
                                    paralysis_chance_denominator:
                                        GREAT_FOX_SPIRIT_PARALYSIS_CHANCE_DENOMINATOR,
                                    paralysis_duration_ticks:
                                        GREAT_FOX_SPIRIT_PARALYSIS_DURATION_TICKS,
                                    paralysis_salt: 501,
                                })
                            } else if restless_jar_tornado_branch {
                                Some(PendingPlayerStatusEffect::BlindnessPoison {
                                    chance_denominator: RESTLESS_JAR_BLINDNESS_CHANCE_DENOMINATOR,
                                    duration_ticks: RESTLESS_JAR_BLINDNESS_DURATION_TICKS,
                                    salt: 1221,
                                })
                            } else if tucson_general_stomp_branch {
                                Some(PendingPlayerStatusEffect::Paralysis {
                                    chance_denominator: TUCSON_GENERAL_PARALYSIS_CHANCE_DENOMINATOR,
                                    duration_ticks: TUCSON_GENERAL_PARALYSIS_DURATION_TICKS,
                                })
                            } else if cat_shaman_red_poison_branch {
                                Some(PendingPlayerStatusEffect::RedPoison {
                                    chance_denominator: CAT_SHAMAN_RED_POISON_CHANCE_DENOMINATOR,
                                    duration_ticks: CAT_SHAMAN_RED_POISON_DURATION_TICKS,
                                    salt: 118,
                                })
                            } else if matches!(agent.ai, 181 | 182) && distance > 1 {
                                Some(PendingPlayerStatusEffect::GreenPoison {
                                    chance_denominator:
                                        WATER_DRAGON_GREEN_POISON_CHANCE_DENOMINATOR,
                                    duration_ticks: WATER_DRAGON_GREEN_POISON_DURATION_TICKS,
                                })
                            } else if evil_centipede_area_branch {
                                Some(PendingPlayerStatusEffect::GreenPoisonAndParalysis {
                                    green_chance_denominator:
                                        EVIL_CENTIPEDE_GREEN_POISON_CHANCE_DENOMINATOR,
                                    green_duration_ticks:
                                        EVIL_CENTIPEDE_GREEN_POISON_DURATION_TICKS,
                                    green_salt: 140,
                                    paralysis_chance_denominator:
                                        EVIL_CENTIPEDE_PARALYSIS_CHANCE_DENOMINATOR,
                                    paralysis_duration_ticks:
                                        EVIL_CENTIPEDE_PARALYSIS_DURATION_TICKS,
                                    paralysis_salt: 141,
                                })
                            } else if agent.ai == 121 && distance > 1 {
                                Some(PendingPlayerStatusEffect::SlowPoison {
                                    chance_denominator: SEEDINGS_GENERAL_POISON_CHANCE_DENOMINATOR,
                                    duration_ticks: SEEDINGS_GENERAL_POISON_DURATION_TICKS,
                                    salt: 1210,
                                })
                            } else if agent.ai == 34 && distance > 1 {
                                if crystal_monster_effect_for_name(&monster_name) == 1 {
                                    Some(PendingPlayerStatusEffect::SlowPoison {
                                        chance_denominator: FROST_TIGER_POISON_CHANCE_DENOMINATOR,
                                        duration_ticks: FROST_TIGER_POISON_DURATION_TICKS,
                                        salt: 341,
                                    })
                                } else {
                                    Some(PendingPlayerStatusEffect::BleedingPoison {
                                        chance_denominator: FROST_TIGER_POISON_CHANCE_DENOMINATOR,
                                        duration_ticks: FROST_TIGER_POISON_DURATION_TICKS,
                                        salt: 340,
                                    })
                                }
                            } else {
                                monster_player_status_effect(&agent)
                            },
                            due_packet,
                            guardian_rock_pull_movement,
                        );
                        for area_target in area_monster_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in seedings_general_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in cannibal_tentacles_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in sand_snail_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in sand_snail_green_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in man_tree_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in man_tree_boulder_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in tucson_warrior_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in tucson_warrior_smash_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for line_target in tucson_mage_wide_line_targets {
                            if let Some(target_position) = entity_position(world, line_target) {
                                schedule_damage_to_monster(
                                    world,
                                    tick + tucson_mage_wide_line_delay_ticks(
                                        &position,
                                        &target_position,
                                        attack_direction,
                                    ),
                                    attacker_id,
                                    line_target,
                                    final_damage,
                                    None,
                                    None,
                                );
                            }
                        }
                        for area_target in hell_keeper_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in snow_wolf_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in tucson_general_stomp_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in black_tortoise_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for line_target in king_scorpion_line_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                line_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in restless_jar_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in minotaur_king_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in general_meow_meow_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in mir_statue_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in manectric_king_mass_targets {
                            if let Some(target_position) = entity_position(world, area_target) {
                                schedule_damage_to_monster(
                                    world,
                                    tick + manectric_king_mass_attack_delay_ticks(
                                        &position,
                                        &target_position,
                                    ),
                                    attacker_id,
                                    area_target,
                                    final_damage,
                                    None,
                                    None,
                                );
                            }
                        }
                        for area_target in manectric_claw_cone_targets {
                            if let Some(target_position) = entity_position(world, area_target) {
                                let base_damage = if tile_distance(&position, &target_position) > 2
                                {
                                    crystal_monster_magic_damage(&monster_name)
                                } else {
                                    crystal_monster_attack_damage(&monster_name)
                                };
                                let mitigation = total_defence_bonus(
                                    world.resource::<InventoryResource>(),
                                    world.resource::<BuffResource>(),
                                );
                                schedule_damage_to_monster(
                                    world,
                                    due_tick,
                                    attacker_id,
                                    area_target,
                                    (base_damage - mitigation).max(1),
                                    None,
                                    None,
                                );
                            }
                        }
                        for area_target in dark_devil_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for line_target in dark_wraith_line_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                line_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for line_target in spider_line_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                line_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in red_moon_evil_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in great_fox_spirit_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in evil_centipede_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                    }
                }
            }
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        let route_target = if monster_can_follow_route(&agent) {
            monster_route_target(&mut agent, &position, tick)
        } else {
            None
        };
        let desired_point = if player_in_aggro_range && monster_can_chase_player(&agent) {
            player_position.clone()
        } else if let Some(route_target) = route_target {
            route_target
        } else if monster_can_patrol_origin(&agent) {
            patrol_target(&agent.patrol_origin, tick, entity.index())
        } else {
            position.clone()
        };

        if desired_point == position {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        if tick < agent.next_move_tick {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        let next = step_point_toward(&position, &desired_point);
        if next == position {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        if !can_occupy(world, next.clone(), Some(entity)) {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        if let Some(direction) = direction_toward(&position, &next) {
            world.entity_mut(entity).insert((
                Position(next.clone()),
                Facing(direction),
                agent.clone(),
            ));
        } else {
            world
                .entity_mut(entity)
                .insert((Position(next.clone()), agent.clone()));
        }

        packets.push(ServerPacket::ObjectWalk {
            movement: object_movement(world, entity).expect("monster movement"),
        });
    }

    for entity in revived_entities {
        if let Some(info) = object_revived_info_for_entity(world, entity, false) {
            packets.push(ServerPacket::ObjectRevived { info });
        }
        if let Some(info) = object_health_info_for_entity(world, entity, 0) {
            packets.push(ServerPacket::ObjectHealth { info });
        }
    }

    packets
}

impl SimulationSession {
    pub fn tick(&mut self) -> Vec<ServerPacket> {
        let packets = advance_world(self.app.world_mut());
        self.finalize_packets(packets)
    }
}

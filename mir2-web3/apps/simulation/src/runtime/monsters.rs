use std::collections::BTreeSet;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::{
    crystal_map_respawns_by_file_name, crystal_monster_ai_is_data_only, crystal_monster_by_name,
    crystal_monster_combat_profile, crystal_starter_region_respawns, starter_server_data,
    CrystalMonsterTemplate, CrystalRespawnTemplate, CrystalRoutePoint,
};
use mir2_protocol::{
    MirDirection, MonsterInfo, ObjectAttackInfo, ObjectEffectInfo, ObjectRangeAttackInfo,
    ObjectSpellInfo, Point, ServerPacket, Spell,
};

use crate::config::{MonsterSpawnSource, SimulationConfig, WorldEntityDisposition};

use super::combat::{
    combat_delay_ticks, crystal_attack_power_roll, deterministic_chance_roll,
    melee_attack_delay_ticks, ranged_attack_delay_ticks, PendingPlayerStatusEffect,
};
use super::components::{
    entity_facing, entity_object_id, entity_position, DisplayName, Facing, GeneralMeowMeowState,
    HarvestMonsterState, HarvestOwnership, Monster, MonsterAgent, MonsterAiState,
    MonsterCombatStats, MonsterVitals, ObjectId, PlayerVitals, Position, RouteStep, SpawnSlotRef,
    SummonedMonster, WoomaTaurusState, WorldObject, YimoogiState,
};
use super::crystal_compat::*;
use super::drops::PendingHarvestDrops;
use super::equipment::total_defence_bonus;
use super::items::current_player_required_stat_total;
use super::map::{
    collision_data_for_map_or_config, is_static_spawnable_point_with_collision,
    runtime_full_map_collision_data, walkable_point_count_in_rect, walkable_points_in_rect,
};
use super::movement::{direction_toward, offset_point, runtime_position_exists, tile_distance};
use super::packets::object_movement;
use super::resources::{
    BuffResource, InventoryResource, MapRuntimeResource, ObjectIdAllocatorResource,
    RuntimeConfigResource, RuntimeQueueResource,
};

#[derive(Debug, Clone)]
pub(super) struct PendingMonsterSpawnAction {
    pub(super) due_tick: u64,
    pub(super) summoner_entity: Entity,
    pub(super) template: CrystalRespawnTemplate,
    pub(super) target_entity: Option<Entity>,
    pub(super) summon_metadata: Option<SummonedMonster>,
    pub(super) hostile_to_player_override: Option<bool>,
}

#[derive(Debug, Clone)]
pub(super) struct MonsterSpawnSlot {
    pub(super) entity: Option<Entity>,
    pub(super) object_id: u32,
    pub(super) spawn_position: Point,
    pub(super) next_respawn_tick: Option<u64>,
}

#[derive(Debug, Clone)]
pub(super) enum MonsterRespawnSchedule {
    FixedTicks {
        delay_ticks: u64,
        random_delay_ticks: u64,
    },
    CrystalMinutes {
        delay_minutes: u16,
        random_delay_minutes: u16,
    },
}

#[derive(Debug, Clone)]
pub(super) struct MonsterSpawnRule {
    pub(super) name: String,
    pub(super) image: u16,
    pub(super) direction: MirDirection,
    pub(super) respawn_schedule: MonsterRespawnSchedule,
    pub(super) ai: u8,
    pub(super) disposition: WorldEntityDisposition,
    pub(super) hostile_to_player: bool,
    pub(super) view_range: i32,
    pub(super) can_wander: bool,
    pub(super) move_interval_ticks: u64,
    pub(super) attack_interval_ticks: u64,
    pub(super) max_hp: i32,
    pub(super) agility: i32,
    pub(super) route: Vec<RouteStep>,
    pub(super) slots: Vec<MonsterSpawnSlot>,
}

#[derive(Resource, Debug, Clone)]
pub(super) struct MonsterSpawnTable {
    pub(super) rules: Vec<MonsterSpawnRule>,
}

pub(super) fn build_spawn_table(config: &SimulationConfig) -> MonsterSpawnTable {
    match config.monster_spawn_source {
        MonsterSpawnSource::StarterScenario => build_starter_spawn_table(config),
        MonsterSpawnSource::CrystalStarterRegion => {
            build_crystal_starter_region_spawn_table(config)
        }
    }
}

#[cfg(test)]
pub(super) fn build_crystal_current_map_spawn_table(
    config: &SimulationConfig,
    map_file_name: &str,
) -> MonsterSpawnTable {
    let mut next_object_id = 60_000_u32;
    let rules = crystal_map_respawns_by_file_name(map_file_name)
        .map(|map| map.respawns)
        .unwrap_or_default()
        .into_iter()
        .take(96)
        .enumerate()
        .map(|(rule_index, respawn)| {
            let route = normalized_route_steps_for_map(config, map_file_name, &respawn.route);
            let slot_positions = spawn_positions_for_rule_on_map(
                config,
                map_file_name,
                &respawn.location,
                respawn.spread.min(16),
                usize::from(respawn.count.min(4)),
                rule_index,
            );
            let slots = slot_positions
                .into_iter()
                .map(|spawn_position| {
                    let object_id = next_object_id;
                    next_object_id += 1;
                    MonsterSpawnSlot {
                        entity: None,
                        object_id,
                        spawn_position,
                        next_respawn_tick: None,
                    }
                })
                .collect();

            MonsterSpawnRule {
                name: respawn.monster_name,
                image: respawn.monster_image,
                direction: respawn.direction,
                respawn_schedule: MonsterRespawnSchedule::CrystalMinutes {
                    delay_minutes: respawn.delay_minutes,
                    random_delay_minutes: respawn.random_delay_minutes,
                },
                ai: respawn.monster_ai,
                disposition: monster_disposition_for_ai(respawn.monster_ai),
                hostile_to_player: monster_targets_players(respawn.monster_ai),
                view_range: i32::from(respawn.monster_view_range),
                can_wander: crystal_respawn_can_wander(respawn.monster_hp),
                move_interval_ticks: crystal_speed_to_ticks(respawn.monster_move_speed),
                attack_interval_ticks: crystal_speed_to_ticks(respawn.monster_attack_speed),
                max_hp: respawn.monster_hp.max(1),
                agility: respawn.monster_agility,
                route,
                slots,
            }
        })
        .collect();

    MonsterSpawnTable { rules }
}

pub(super) fn build_crystal_current_map_visible_spawn_table(
    config: &SimulationConfig,
    map_file_name: &str,
    player_position: &Point,
) -> MonsterSpawnTable {
    let mut rules = Vec::new();

    if let Some(map) = crystal_map_respawns_by_file_name(map_file_name) {
        for respawn in map.respawns {
            let visible_spawns =
                start_game_visible_respawn_spawns(map_file_name, &respawn, player_position);
            if visible_spawns.is_empty() {
                continue;
            }

            for (slot_index, spawn_position, direction) in visible_spawns {
                let route = normalized_route_steps_for_map(config, map_file_name, &respawn.route);
                let object_id = crystal_respawn_object_id(&respawn, slot_index);
                rules.push(MonsterSpawnRule {
                    name: respawn.monster_name.clone(),
                    image: respawn.monster_image,
                    direction,
                    respawn_schedule: MonsterRespawnSchedule::CrystalMinutes {
                        delay_minutes: respawn.delay_minutes,
                        random_delay_minutes: respawn.random_delay_minutes,
                    },
                    ai: respawn.monster_ai,
                    disposition: monster_disposition_for_ai(respawn.monster_ai),
                    hostile_to_player: monster_targets_players(respawn.monster_ai),
                    view_range: i32::from(respawn.monster_view_range),
                    can_wander: crystal_respawn_can_wander(respawn.monster_hp),
                    move_interval_ticks: crystal_speed_to_ticks(respawn.monster_move_speed),
                    attack_interval_ticks: crystal_speed_to_ticks(respawn.monster_attack_speed),
                    max_hp: respawn.monster_hp.max(1),
                    agility: respawn.monster_agility,
                    route,
                    slots: vec![MonsterSpawnSlot {
                        entity: None,
                        object_id,
                        spawn_position,
                        next_respawn_tick: None,
                    }],
                });
            }
        }
    }

    MonsterSpawnTable { rules }
}

pub(super) fn build_starter_spawn_table(config: &SimulationConfig) -> MonsterSpawnTable {
    let rules = starter_server_data()
        .monster_spawns
        .iter()
        .enumerate()
        .map(|(rule_index, spawn)| {
            let slot_positions = spawn_positions_for_rule(
                config,
                &spawn.position,
                spawn.spread,
                usize::from(spawn.count),
                rule_index,
            );
            let slots = slot_positions
                .into_iter()
                .enumerate()
                .map(|(slot_index, spawn_position)| MonsterSpawnSlot {
                    entity: None,
                    object_id: spawn.object_id + u32::try_from(slot_index).expect("slot id fits"),
                    spawn_position,
                    next_respawn_tick: None,
                })
                .collect();

            MonsterSpawnRule {
                name: spawn.name.clone(),
                image: spawn.image,
                direction: spawn.direction,
                respawn_schedule: MonsterRespawnSchedule::FixedTicks {
                    delay_ticks: spawn.respawn_delay_ticks,
                    random_delay_ticks: spawn.random_delay_ticks,
                },
                ai: 0,
                disposition: if spawn.can_wander {
                    WorldEntityDisposition::Hostile
                } else {
                    WorldEntityDisposition::Neutral
                },
                hostile_to_player: spawn.can_wander,
                view_range: if spawn.can_wander { 4 } else { 0 },
                can_wander: spawn.can_wander,
                move_interval_ticks: 1,
                attack_interval_ticks: 1,
                max_hp: spawn.max_hp,
                agility: 0,
                route: Vec::new(),
                slots,
            }
        })
        .collect();

    MonsterSpawnTable { rules }
}

pub(super) fn build_crystal_starter_region_spawn_table(
    config: &SimulationConfig,
) -> MonsterSpawnTable {
    let mut next_object_id = 60_000_u32;
    let rules = crystal_starter_region_respawns()
        .into_iter()
        .enumerate()
        .map(|(rule_index, respawn)| {
            let route = normalized_route_steps(config, &respawn.route);
            let slot_positions = spawn_positions_for_rule(
                config,
                &respawn.location,
                respawn.spread,
                usize::from(respawn.count),
                rule_index,
            );
            let slots = slot_positions
                .into_iter()
                .map(|spawn_position| {
                    let object_id = next_object_id;
                    next_object_id += 1;
                    MonsterSpawnSlot {
                        entity: None,
                        object_id,
                        spawn_position,
                        next_respawn_tick: None,
                    }
                })
                .collect();

            MonsterSpawnRule {
                name: respawn.monster_name,
                image: respawn.monster_image,
                direction: respawn.direction,
                respawn_schedule: MonsterRespawnSchedule::CrystalMinutes {
                    delay_minutes: respawn.delay_minutes,
                    random_delay_minutes: respawn.random_delay_minutes,
                },
                ai: respawn.monster_ai,
                disposition: monster_disposition_for_ai(respawn.monster_ai),
                hostile_to_player: monster_targets_players(respawn.monster_ai),
                view_range: i32::from(respawn.monster_view_range),
                can_wander: crystal_respawn_can_wander(respawn.monster_hp),
                move_interval_ticks: crystal_speed_to_ticks(respawn.monster_move_speed),
                attack_interval_ticks: crystal_speed_to_ticks(respawn.monster_attack_speed),
                max_hp: respawn.monster_hp.max(1),
                agility: respawn.monster_agility,
                route,
                slots,
            }
        })
        .collect();

    MonsterSpawnTable { rules }
}

pub(super) fn spawn_positions_for_rule(
    config: &SimulationConfig,
    origin: &Point,
    spread: u16,
    slot_count: usize,
    rule_index: usize,
) -> Vec<Point> {
    spawn_positions_for_rule_on_map(
        config,
        &config.map.file_name,
        origin,
        spread,
        slot_count,
        rule_index,
    )
}

pub(super) fn spawn_positions_for_rule_on_map(
    config: &SimulationConfig,
    map_file_name: &str,
    origin: &Point,
    spread: u16,
    slot_count: usize,
    rule_index: usize,
) -> Vec<Point> {
    if slot_count == 0 {
        return Vec::new();
    }

    let candidates = crystal_spawn_candidates_on_map(config, map_file_name, origin, spread);
    if candidates.is_empty() {
        return Vec::new();
    }

    let seed = rule_index % candidates.len();
    let origin_index = candidates.iter().position(|candidate| candidate == origin);

    (0..slot_count)
        .map(|slot_index| {
            if slot_index == 0 {
                return origin_index
                    .map(|index| candidates[index].clone())
                    .unwrap_or_else(|| candidates[seed].clone());
            }

            let mut index = (seed + slot_index) % candidates.len();
            if candidates.len() > 1 && origin_index.is_some() && Some(index) == origin_index {
                index = (index + 1) % candidates.len();
            }

            candidates[index].clone()
        })
        .collect()
}

pub(super) fn crystal_spawn_candidates_on_map(
    config: &SimulationConfig,
    map_file_name: &str,
    origin: &Point,
    spread: u16,
) -> Vec<Point> {
    let collision = collision_data_for_map_or_config(config, map_file_name);
    let bounds = collision.collision.region_bounds;
    let spread = i32::from(spread);
    let min_x = (origin.x - spread).max(bounds.min_x);
    let max_x = (origin.x + spread).min(bounds.max_x);
    let min_y = (origin.y - spread).max(bounds.min_y);
    let max_y = (origin.y + spread).min(bounds.max_y);
    let mut candidates = Vec::new();

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let point = Point { x, y };
            if is_static_spawnable_point_with_collision(config, map_file_name, &collision, &point) {
                candidates.push(point);
            }
        }
    }

    candidates
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn is_static_spawnable_point(config: &SimulationConfig, point: &Point) -> bool {
    is_static_spawnable_point_on_map(config, &config.map.file_name, point)
}

pub(super) fn is_static_spawnable_point_on_map(
    config: &SimulationConfig,
    map_file_name: &str,
    point: &Point,
) -> bool {
    let collision = collision_data_for_map_or_config(config, map_file_name);
    is_static_spawnable_point_with_collision(config, map_file_name, &collision, point)
}

pub(super) fn normalized_route_steps(
    config: &SimulationConfig,
    route: &[CrystalRoutePoint],
) -> Vec<RouteStep> {
    normalized_route_steps_for_map(config, &config.map.file_name, route)
}

pub(super) fn normalized_route_steps_for_map(
    config: &SimulationConfig,
    map_file_name: &str,
    route: &[CrystalRoutePoint],
) -> Vec<RouteStep> {
    route
        .iter()
        .filter_map(|step| {
            let location = Point {
                x: step.x,
                y: step.y,
            };
            is_static_spawnable_point_on_map(config, map_file_name, &location).then_some(
                RouteStep {
                    location,
                    delay_ticks: crystal_route_delay_ticks(step.delay),
                },
            )
        })
        .collect()
}

pub(super) fn crystal_route_delay_ticks(delay_ms: i32) -> u64 {
    let delay_ms = u64::try_from(delay_ms.max(0)).expect("route delay should fit u64");
    if delay_ms == 0 {
        0
    } else {
        1 + delay_ms.div_ceil(1_000)
    }
}

pub(super) fn crystal_respawn_can_wander(monster_hp: i32) -> bool {
    monster_hp < 100_000
}

pub(super) fn crystal_respawn_template_from_monster(
    monster: CrystalMonsterTemplate,
) -> CrystalRespawnTemplate {
    CrystalRespawnTemplate {
        monster_index: monster.monster_index,
        location: Point { x: 0, y: 0 },
        count: 1,
        spread: 0,
        delay_minutes: 0,
        direction: MirDirection::Up,
        route_path: None,
        random_delay_minutes: 0,
        respawn_index: 0,
        save_respawn_time: false,
        respawn_ticks: 0,
        monster_name: monster.name,
        monster_image: monster.image,
        monster_ai: monster.ai,
        monster_view_range: monster.view_range,
        monster_hp: monster.hp,
        monster_attack_speed: monster.attack_speed,
        monster_move_speed: monster.move_speed,
        monster_can_push: monster.can_push,
        monster_can_tame: monster.can_tame,
        monster_auto_rev: monster.auto_rev,
        monster_undead: monster.undead,
        monster_agility: monster.agility,
        route: Vec::new(),
    }
}

pub(super) fn crystal_dynamic_monster_template(name: &str) -> Option<CrystalRespawnTemplate> {
    crystal_monster_by_name(name).map(crystal_respawn_template_from_monster)
}

pub(super) fn bug_bat_template() -> Option<CrystalRespawnTemplate> {
    Some(
        crystal_dynamic_monster_template("BugBat").unwrap_or_else(|| CrystalRespawnTemplate {
            // Crystal does not export BugBat into the respawn manifest because
            // BugBagMaggot summons it dynamically.
            monster_index: 0,
            location: Point { x: 0, y: 0 },
            count: 1,
            spread: 0,
            delay_minutes: 0,
            direction: MirDirection::Up,
            route_path: None,
            random_delay_minutes: 0,
            respawn_index: 0,
            save_respawn_time: false,
            respawn_ticks: 0,
            monster_name: "BugBat".to_string(),
            monster_image: BUG_BAT_IMAGE,
            monster_ai: 0,
            monster_view_range: BUG_BAT_FALLBACK_VIEW_RANGE,
            monster_hp: BUG_BAT_FALLBACK_HP,
            monster_attack_speed: BUG_BAT_FALLBACK_ATTACK_SPEED,
            monster_move_speed: BUG_BAT_FALLBACK_MOVE_SPEED,
            monster_can_push: true,
            monster_can_tame: true,
            monster_auto_rev: true,
            monster_undead: false,
            monster_agility: 0,
            route: Vec::new(),
        }),
    )
}

pub(super) fn bomb_spider_template() -> Option<CrystalRespawnTemplate> {
    Some(
        crystal_dynamic_monster_template("BombSpider").unwrap_or_else(|| CrystalRespawnTemplate {
            monster_index: 0,
            location: Point { x: 0, y: 0 },
            count: 1,
            spread: 0,
            delay_minutes: 0,
            direction: MirDirection::Up,
            route_path: None,
            random_delay_minutes: 0,
            respawn_index: 0,
            save_respawn_time: false,
            respawn_ticks: 0,
            monster_name: "BombSpider".to_string(),
            monster_image: BOMB_SPIDER_IMAGE,
            monster_ai: 40,
            monster_view_range: BOMB_SPIDER_FALLBACK_VIEW_RANGE,
            monster_hp: BOMB_SPIDER_FALLBACK_HP,
            monster_attack_speed: 0,
            monster_move_speed: BOMB_SPIDER_FALLBACK_MOVE_SPEED,
            monster_can_push: true,
            monster_can_tame: false,
            monster_auto_rev: true,
            monster_undead: false,
            monster_agility: 0,
            route: Vec::new(),
        }),
    )
}

pub(super) fn monster_ai_state_for_ai(ai: u8) -> MonsterAiState {
    match ai {
        5 | 14 => MonsterAiState {
            hidden: true,
            extra: false,
            extra_byte: 0,
            mode: false,
            next_state_tick: 0,
        },
        47 => MonsterAiState {
            hidden: true,
            extra: false,
            extra_byte: 0,
            mode: true,
            next_state_tick: 0,
        },
        48 => MonsterAiState {
            hidden: false,
            extra: false,
            extra_byte: 0,
            mode: true,
            next_state_tick: 0,
        },
        15 | 16 | 173 | 174 => MonsterAiState {
            hidden: false,
            extra: true,
            extra_byte: 0,
            mode: false,
            next_state_tick: 0,
        },
        17 => MonsterAiState {
            hidden: false,
            extra: true,
            extra_byte: ZUMA_TAURUS_STAGE_COUNT,
            mode: false,
            next_state_tick: 0,
        },
        18 => MonsterAiState {
            hidden: true,
            extra: false,
            extra_byte: 0,
            mode: false,
            next_state_tick: 0,
        },
        24 | 124 | 125 => MonsterAiState {
            hidden: true,
            extra: false,
            extra_byte: 0,
            mode: false,
            next_state_tick: 0,
        },
        30 => MonsterAiState {
            hidden: false,
            extra: false,
            extra_byte: BONE_LORD_STAGE_COUNT,
            mode: false,
            next_state_tick: 0,
        },
        38 => MonsterAiState {
            hidden: false,
            extra: true,
            extra_byte: 0,
            mode: false,
            next_state_tick: 0,
        },
        97 => MonsterAiState {
            hidden: false,
            extra: true,
            extra_byte: 0,
            mode: false,
            next_state_tick: 0,
        },
        _ => MonsterAiState::default(),
    }
}

pub(super) fn initial_monster_ai_state(ai: u8, tick: u64) -> MonsterAiState {
    let mut state = monster_ai_state_for_ai(ai);
    if ai == 40 {
        state.next_state_tick = tick + BOMB_SPIDER_EXPLOSION_LIFETIME_TICKS;
    }
    if ai == 47 {
        state.next_state_tick = tick + 2;
    }
    if ai == 34 {
        state.next_state_tick = tick + FROST_TIGER_SIT_DOWN_MAX_DELAY_TICKS;
    }
    if ai == 99 {
        state.next_state_tick = tick + HELL_BOMB_EXPLOSION_LIFETIME_TICKS;
    }
    state
}

pub(super) fn initial_monster_ai_state_for_object(
    ai: u8,
    tick: u64,
    object_id: u32,
) -> MonsterAiState {
    let mut state = initial_monster_ai_state(ai, tick);
    if ai == 2 {
        state.mode = deterministic_chance_roll(tick, object_id, 2, 7);
    }
    state
}

pub(super) fn initial_harvest_monster_state(ai: u8) -> Option<HarvestMonsterState> {
    let remaining_skin_count = match ai {
        2 => DEER_SKIN_COUNT,
        1 | 7 | 9 | 28 | 35 => HARVEST_MONSTER_SKIN_COUNT,
        _ => return None,
    };
    Some(HarvestMonsterState {
        remaining_skin_count,
        drops_ready: false,
        harvested: false,
    })
}

pub(super) fn initial_wooma_taurus_state(
    tick: u64,
    base_move_interval_ticks: u64,
    base_attack_interval_ticks: u64,
) -> WoomaTaurusState {
    WoomaTaurusState {
        stage: WOOMA_TAURUS_STAGE_COUNT,
        mad_until_tick: 0,
        next_teleport_tick: tick + WOOMA_TAURUS_TELEPORT_DELAY_TICKS,
        base_move_interval_ticks: base_move_interval_ticks.max(1),
        base_attack_interval_ticks: base_attack_interval_ticks.max(1),
    }
}

pub(super) fn initial_general_meow_meow_state(tick: u64) -> GeneralMeowMeowState {
    GeneralMeowMeowState {
        shield_until_tick: 0,
        next_thunder_tick: 0,
        next_slave_spawn_tick: tick + GENERAL_MEOW_MEOW_SLAVE_SPAWN_INTERVAL_TICKS,
    }
}

pub(super) fn initial_yimoogi_state(tick: u64) -> YimoogiState {
    YimoogiState {
        spawn_tick: tick + YIMOOGI_CHILD_SPAWN_DELAY_TICKS,
        child_spawned: false,
        is_child: false,
        final_teleport: false,
        sister_object_id: None,
    }
}

pub(super) fn monster_disposition_for_ai(ai: u8) -> WorldEntityDisposition {
    match ai {
        1 | 2 | 3 | 6 | 34 | 56 | 57 | 58 | 113 => WorldEntityDisposition::Neutral,
        _ => WorldEntityDisposition::Hostile,
    }
}

pub(super) fn monster_targets_players(ai: u8) -> bool {
    !matches!(ai, 1 | 2 | 3 | 6 | 34 | 56 | 57 | 58 | 113)
}

pub(super) fn monster_uses_zuma_stone_state(ai: u8) -> bool {
    matches!(ai, 15 | 16 | 17 | 173 | 174)
}

pub(super) fn monster_is_stoned_zuma(agent: &MonsterAgent, ai_state: &MonsterAiState) -> bool {
    monster_uses_zuma_stone_state(agent.ai) && ai_state.extra
}

pub(super) fn is_hidden_or_sleeping_target(
    target_agent: &MonsterAgent,
    target_ai_state: &MonsterAiState,
) -> bool {
    target_ai_state.hidden || monster_is_stoned_zuma(target_agent, target_ai_state)
}

pub(super) fn monster_is_damageable(world: &World, monster_entity: Entity) -> bool {
    let entry = world.entity(monster_entity);
    let Some(agent) = entry.get::<MonsterAgent>() else {
        return false;
    };
    let ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
    let Some(vitals) = entry.get::<MonsterVitals>() else {
        return false;
    };

    !agent.dead && !ai_state.hidden && !monster_is_stoned_zuma(agent, &ai_state) && vitals.hp > 0
}

pub(super) fn monster_can_attack(agent: &MonsterAgent, ai_state: &MonsterAiState) -> bool {
    if agent.dead || ai_state.hidden || monster_is_stoned_zuma(agent, ai_state) {
        return false;
    }

    match agent.ai {
        18 => ai_state.mode,
        48 => ai_state.mode,
        // Crystal `CanAttack { return false; }` non-combatants: gates, training dummies, props, the
        // EvilMir body parts (53), Wall (82), Football (68), CaveStatue (151), BoulderSpirit (170).
        1 | 2 | 3 | 53 | 56 | 68 | 82 | 98 | 99 | 128 | 151 | 170 | 255 => false,
        _ => true,
    }
}

pub(super) fn ignores_monster_damage(agent: &MonsterAgent) -> bool {
    matches!(agent.ai, 48 | 49 | 99 | 255)
}

pub(super) fn monster_ignores_damage(world: &World, monster_entity: Entity) -> bool {
    let entry = world.entity(monster_entity);
    let Some(agent) = entry.get::<MonsterAgent>() else {
        return false;
    };
    let ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();

    ignores_monster_damage(agent) || (agent.ai == 98 && ai_state.extra_byte < 4)
}

pub(super) fn queue_pending_monster_spawn(world: &mut World, action: PendingMonsterSpawnAction) {
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_monster_spawns
        .push(action);
}

pub(super) fn allocate_runtime_monster_object_id(world: &mut World) -> u32 {
    world
        .resource_mut::<ObjectIdAllocatorResource>()
        .next_runtime_monster_id()
}

pub(super) fn active_summoned_monster_count(world: &World, summoner_object_id: u32) -> usize {
    #[allow(deprecated)]
    world
        .iter_entities()
        .filter(|entity| {
            entity
                .get::<SummonedMonster>()
                .map(|summoned| summoned.summoner_object_id == summoner_object_id)
                .unwrap_or(false)
                && entity
                    .get::<MonsterAgent>()
                    .map(|agent| !agent.dead)
                    .unwrap_or(false)
        })
        .count()
}

pub(super) fn active_summoned_monster_count_by_ai(
    world: &World,
    summoner_object_id: u32,
    ai: u8,
) -> usize {
    #[allow(deprecated)]
    world
        .iter_entities()
        .filter(|entity| {
            entity
                .get::<SummonedMonster>()
                .map(|summoned| summoned.summoner_object_id == summoner_object_id)
                .unwrap_or(false)
                && entity
                    .get::<MonsterAgent>()
                    .map(|agent| !agent.dead && agent.ai == ai)
                    .unwrap_or(false)
        })
        .count()
}

#[allow(deprecated)]
pub(super) fn summoned_owner_state(world: &World, owner_object_id: u32) -> Option<(Point, bool)> {
    world.iter_entities().find_map(|entity| {
        let object_id = entity.get::<ObjectId>()?;
        if object_id.0 != owner_object_id {
            return None;
        }

        let position = entity.get::<Position>()?.0.clone();
        let dead = entity
            .get::<MonsterAgent>()
            .map(|agent| agent.dead)
            .unwrap_or(false);
        Some((position, dead))
    })
}

pub(super) fn summoned_monster_defaults(
    summoner_ai: u8,
    template: &CrystalRespawnTemplate,
) -> SummonedMonster {
    let mut summoned = SummonedMonster {
        summoner_object_id: 0,
        visible_extra: false,
        expire_tick: None,
        require_summoner_within: None,
        despawn_tick_after_death: None,
        totem_master_object_id: None,
        max_minions: None,
    };

    match (summoner_ai, template.monster_ai) {
        (12, _) | (39, _) => {}
        (62, 63) => {
            summoned.visible_extra = true;
            summoned.require_summoner_within = Some(15);
            summoned.totem_master_object_id = Some(0);
        }
        _ => {}
    }

    summoned
}

pub(super) fn spawn_runtime_monster(
    world: &mut World,
    template: &CrystalRespawnTemplate,
    position: Point,
    direction: MirDirection,
    target_entity: Option<Entity>,
    summoned: Option<SummonedMonster>,
    hostile_to_player_override: Option<bool>,
    disposition_override: Option<WorldEntityDisposition>,
    activation_delay_ticks: u64,
) -> Option<Entity> {
    let (config, current_map_file_name) = {
        let map = world.resource::<MapRuntimeResource>();
        (
            world.resource::<RuntimeConfigResource>().config.clone(),
            map.current_map.file_name.clone(),
        )
    };
    let valid_spawn_point =
        is_static_spawnable_point_on_map(&config, &current_map_file_name, &position)
            || (summoned.is_some() && runtime_position_exists(world, &position));
    if !valid_spawn_point {
        return None;
    }

    let tick = super::session::runtime_tick(world);
    let route = normalized_route_steps(&config, &template.route);
    let mut agent = MonsterAgent {
        image: template.monster_image,
        dead: false,
        patrol_origin: position.clone(),
        ai: template.monster_ai,
        disposition: disposition_override
            .unwrap_or_else(|| monster_disposition_for_ai(template.monster_ai)),
        hostile_to_player: hostile_to_player_override
            .unwrap_or_else(|| monster_targets_players(template.monster_ai)),
        tracking_player: false,
        view_range: i32::from(template.monster_view_range),
        can_wander: crystal_respawn_can_wander(template.monster_hp),
        move_interval_ticks: crystal_speed_to_ticks(template.monster_move_speed),
        attack_interval_ticks: crystal_speed_to_ticks(template.monster_attack_speed),
        next_move_tick: tick + activation_delay_ticks,
        next_attack_tick: tick + activation_delay_ticks,
        route,
        route_index: 0,
        route_waiting: false,
        next_route_tick: tick + activation_delay_ticks,
    };
    if target_entity.is_some() && agent.hostile_to_player {
        agent.tracking_player = true;
    }

    let object_id = allocate_runtime_monster_object_id(world);
    let facing = if template.monster_ai == 3 {
        MirDirection::Up
    } else {
        direction
    };
    let mut entity = world.spawn((
        WorldObject,
        Monster,
        ObjectId(object_id),
        DisplayName::literal(template.monster_name.clone()),
        Position(position.clone()),
        Facing(facing),
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
    if let Some(summoned) = summoned {
        entity.insert(summoned);
    }
    if template.monster_ai == 123 {
        entity.insert(initial_general_meow_meow_state(
            tick + activation_delay_ticks,
        ));
    }
    if template.monster_ai == 36 {
        entity.insert(initial_yimoogi_state(tick + activation_delay_ticks));
    }
    Some(entity.id())
}

pub(super) fn resolve_pending_monster_spawns(world: &mut World, current_tick: u64) {
    let due_actions = {
        let mut resources = world.resource_mut::<RuntimeQueueResource>();
        let mut due = Vec::new();
        let mut pending = Vec::with_capacity(resources.pending_monster_spawns.len());

        for action in resources.pending_monster_spawns.drain(..) {
            if action.due_tick <= current_tick {
                due.push(action);
            } else {
                pending.push(action);
            }
        }

        resources.pending_monster_spawns = pending;
        due
    };

    for action in due_actions {
        let Ok(summoner_entry) = world.get_entity(action.summoner_entity) else {
            continue;
        };
        let Some(summoner_position) = summoner_entry
            .get::<Position>()
            .map(|value| value.0.clone())
        else {
            continue;
        };
        let Some(summoner_object_id) = summoner_entry.get::<ObjectId>().map(|value| value.0) else {
            continue;
        };
        let summoner_ai = summoner_entry
            .get::<MonsterAgent>()
            .map(|agent| agent.ai)
            .unwrap_or_default();
        let summoner_dead = summoner_entry
            .get::<MonsterAgent>()
            .map(|agent| agent.dead)
            .unwrap_or(false);

        let spawns_regular_monster =
            matches!(summoner_ai, 119 | 128) && action.summon_metadata.is_none();
        let summon_cap = action
            .summon_metadata
            .and_then(|summoned| summoned.max_minions)
            .unwrap_or(20);
        if (!spawns_regular_monster && summoner_dead)
            || (!spawns_regular_monster
                && active_summoned_monster_count(world, summoner_object_id) >= summon_cap)
        {
            continue;
        }

        let target_entity = action.target_entity.filter(|entity| {
            world
                .get_entity(*entity)
                .map(|entry| {
                    entry.get::<PlayerVitals>().is_some()
                        || entry
                            .get::<MonsterAgent>()
                            .map(|agent| !agent.dead)
                            .unwrap_or(false)
                })
                .unwrap_or(false)
        });

        let spawn_position = if action.template.location == (Point { x: 0, y: 0 }) {
            summoner_position
        } else {
            action.template.location.clone()
        };

        let summon_metadata = if spawns_regular_monster {
            None
        } else {
            let mut summon_metadata = action
                .summon_metadata
                .unwrap_or_else(|| summoned_monster_defaults(summoner_ai, &action.template));
            summon_metadata.summoner_object_id = summoner_object_id;
            if summon_metadata.totem_master_object_id.is_some() {
                summon_metadata.totem_master_object_id = Some(summoner_object_id);
            }
            Some(summon_metadata)
        };

        let _ = spawn_runtime_monster(
            world,
            &action.template,
            spawn_position,
            MirDirection::Up,
            target_entity,
            summon_metadata,
            action.hostile_to_player_override,
            action.hostile_to_player_override.map(|hostile| {
                if hostile {
                    WorldEntityDisposition::Hostile
                } else {
                    WorldEntityDisposition::Friendly
                }
            }),
            1,
        );
    }
}

pub(super) fn crystal_speed_to_ticks(speed_ms: u16) -> u64 {
    u64::from(speed_ms.max(400)).div_ceil(1_000)
}

pub(super) fn respawn_tick_for_schedule(
    schedule: &MonsterRespawnSchedule,
    current_tick: u64,
    rule_index: usize,
    slot_index: usize,
) -> u64 {
    match schedule {
        MonsterRespawnSchedule::FixedTicks {
            delay_ticks,
            random_delay_ticks,
        } => {
            let extra_delay = if *random_delay_ticks == 0 {
                0
            } else {
                deterministic_roll(current_tick, rule_index, slot_index, random_delay_ticks + 1)
            };
            current_tick + delay_ticks + extra_delay
        }
        MonsterRespawnSchedule::CrystalMinutes {
            delay_minutes,
            random_delay_minutes,
        } => {
            const SIMULATION_TICKS_PER_CRYSTAL_MINUTE: u64 = 60;
            let delay_minutes = i64::from(*delay_minutes);
            let random_delay_minutes = i64::from(*random_delay_minutes);
            let roll = if random_delay_minutes == 0 {
                0
            } else {
                deterministic_roll(
                    current_tick,
                    rule_index,
                    slot_index,
                    u64::try_from(random_delay_minutes * 2)
                        .expect("random delay minutes window should fit u64"),
                ) as i64
            };
            let scheduled_minutes = (delay_minutes - random_delay_minutes + roll).max(1) as u64;
            current_tick + scheduled_minutes * SIMULATION_TICKS_PER_CRYSTAL_MINUTE
        }
    }
}

pub(super) fn deterministic_roll(
    current_tick: u64,
    rule_index: usize,
    slot_index: usize,
    modulo: u64,
) -> u64 {
    if modulo == 0 {
        return 0;
    }

    let salt = current_tick
        .wrapping_mul(1_103_515_245)
        .wrapping_add((rule_index as u64).wrapping_mul(97_651))
        .wrapping_add((slot_index as u64).wrapping_mul(12_347))
        .wrapping_add(0x9E37_79B9);
    salt % modulo
}

/// Crystal `RevivingZombie` rolls `LifeCount = Random.Next(3)` in its constructor — every zombie
/// gets 0, 1 or 2 revivals, not a guaranteed 2. Keyed by object id so the value is stable for a
/// given zombie slot.
pub(super) fn reviving_zombie_life_count(object_id: u32) -> u8 {
    deterministic_roll(0, object_id as usize, REVIVING_ZOMBIE_LIFECOUNT_SALT, 3) as u8
}

/// Crystal `RevivingZombie.Die` sets `RevivalTime = (4 + Random.Next(20)) * 1000`, i.e. a 4–23
/// second wait before the zombie claws back up.
pub(super) fn reviving_zombie_revive_delay_ticks(object_id: u32, current_tick: u64) -> u64 {
    REVIVING_ZOMBIE_REVIVE_DELAY_TICKS
        + deterministic_roll(current_tick, object_id as usize, REVIVING_ZOMBIE_DELAY_SALT, 20)
}

pub(super) fn crystal_monster_attack_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_dc.max(monster.min_dc).max(1))
        .unwrap_or(7)
}

pub(super) fn crystal_monster_magic_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_mc.max(monster.min_mc).max(1))
        .unwrap_or(7)
}

pub(super) fn crystal_monster_spell_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_sc.max(monster.min_sc).max(1))
        .unwrap_or(7)
}

pub(super) fn crystal_monster_raw_magic_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_mc.max(monster.min_mc))
        .unwrap_or(0)
}

pub(super) fn crystal_monster_raw_attack_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_dc.max(monster.min_dc))
        .unwrap_or(0)
}

// Crystal's `GetAttackPower(MinDC, MaxDC)` rolls a fresh value in the range for every swing; these
// are the rolled counterparts of the `*_damage` leaves above, seeded by (tick, attacker object id).
pub(super) fn crystal_monster_attack_damage_rolled(name: &str, tick: u64, attacker_id: u32) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| {
            crystal_attack_power_roll(
                monster.min_dc,
                monster.max_dc,
                tick,
                attacker_id,
                CRYSTAL_DAMAGE_ROLL_SALT_DC,
            )
            .max(1)
        })
        .unwrap_or(7)
}

pub(super) fn crystal_monster_magic_damage_rolled(name: &str, tick: u64, attacker_id: u32) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| {
            crystal_attack_power_roll(
                monster.min_mc,
                monster.max_mc,
                tick,
                attacker_id,
                CRYSTAL_DAMAGE_ROLL_SALT_MC,
            )
            .max(1)
        })
        .unwrap_or(7)
}

pub(super) fn crystal_monster_raw_magic_damage_rolled(name: &str, tick: u64, attacker_id: u32) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| {
            crystal_attack_power_roll(
                monster.min_mc,
                monster.max_mc,
                tick,
                attacker_id,
                CRYSTAL_DAMAGE_ROLL_SALT_MC,
            )
        })
        .unwrap_or(0)
}

pub(super) fn crystal_monster_raw_attack_damage_rolled(
    name: &str,
    tick: u64,
    attacker_id: u32,
) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| {
            crystal_attack_power_roll(
                monster.min_dc,
                monster.max_dc,
                tick,
                attacker_id,
                CRYSTAL_DAMAGE_ROLL_SALT_DC,
            )
        })
        .unwrap_or(0)
}

pub(super) fn crystal_monster_spell_damage_rolled(name: &str, tick: u64, attacker_id: u32) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| {
            crystal_attack_power_roll(
                monster.min_sc,
                monster.max_sc,
                tick,
                attacker_id,
                CRYSTAL_DAMAGE_ROLL_SALT_SC,
            )
            .max(1)
        })
        .unwrap_or(7)
}

// Lower bounds of the damage classes, used by tests to assert that a rolled special attack lands
// inside `[min_class, max_class]` rather than pinning the (now-variant) maximum.
pub(super) fn crystal_monster_min_attack_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.min_dc.max(0))
        .unwrap_or(0)
}

pub(super) fn crystal_monster_min_magic_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.min_mc.max(0))
        .unwrap_or(0)
}

pub(super) fn crystal_monster_min_spell_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.min_sc.max(0))
        .unwrap_or(0)
}

pub(super) fn crystal_monster_effect_for_name(name: &str) -> u8 {
    crystal_monster_by_name(name)
        .map(|monster| monster.effect)
        .unwrap_or(0)
}

#[allow(deprecated)]
pub(super) fn tick_respawns(world: &mut World) -> Vec<Entity> {
    let tick = super::session::runtime_tick(world);
    let mut due_respawns = Vec::new();
    {
        let mut spawn_table = world.resource_mut::<MonsterSpawnTable>();
        for (rule_index, rule) in spawn_table.rules.iter_mut().enumerate() {
            for (slot_index, slot) in rule.slots.iter_mut().enumerate() {
                let Some(respawn_tick) = slot.next_respawn_tick else {
                    continue;
                };
                if tick < respawn_tick {
                    continue;
                }
                let Some(entity) = slot.entity else {
                    continue;
                };
                slot.next_respawn_tick = None;
                due_respawns.push((
                    entity,
                    rule_index,
                    slot_index,
                    slot.object_id,
                    slot.spawn_position.clone(),
                    rule.direction,
                    rule.ai,
                    rule.disposition,
                    rule.hostile_to_player,
                    rule.view_range,
                    rule.max_hp,
                    rule.image,
                    rule.can_wander,
                    rule.move_interval_ticks,
                    rule.attack_interval_ticks,
                    rule.agility,
                    rule.route.clone(),
                ));
            }
        }
    }

    let mut revived_entities = Vec::with_capacity(due_respawns.len());

    for (
        entity,
        rule_index,
        slot_index,
        object_id,
        spawn_position,
        direction,
        ai,
        disposition,
        hostile_to_player,
        view_range,
        max_hp,
        image,
        can_wander,
        move_interval_ticks,
        attack_interval_ticks,
        agility,
        route,
    ) in due_respawns
    {
        world.entity_mut(entity).insert((
            Position(spawn_position.clone()),
            Facing(direction),
            MonsterVitals { hp: max_hp, max_hp },
            MonsterAgent {
                image,
                dead: false,
                patrol_origin: spawn_position,
                ai,
                disposition,
                hostile_to_player,
                tracking_player: false,
                view_range,
                can_wander,
                move_interval_ticks,
                attack_interval_ticks,
                next_move_tick: tick,
                next_attack_tick: tick,
                route,
                route_index: 0,
                route_waiting: false,
                next_route_tick: tick,
            },
            initial_monster_ai_state_for_object(ai, tick, object_id),
            MonsterCombatStats { agility },
            SpawnSlotRef {
                rule_index,
                slot_index,
            },
        ));
        if ai == 11 {
            world.entity_mut(entity).insert(initial_wooma_taurus_state(
                tick,
                move_interval_ticks,
                attack_interval_ticks,
            ));
        }
        if ai == 123 {
            world
                .entity_mut(entity)
                .insert(initial_general_meow_meow_state(tick));
        }
        if ai == 36 {
            world.entity_mut(entity).insert(initial_yimoogi_state(tick));
        }
        world.entity_mut(entity).remove::<PendingHarvestDrops>();
        world.entity_mut(entity).remove::<HarvestOwnership>();
        if let Some(state) = initial_harvest_monster_state(ai) {
            world.entity_mut(entity).insert(state);
        }
        revived_entities.push(entity);
    }

    revived_entities
}

pub(super) fn schedule_monster_respawn(
    world: &mut World,
    spawn_ref: SpawnSlotRef,
    current_tick: u64,
) {
    let mut spawn_table = world.resource_mut::<MonsterSpawnTable>();
    let Some(rule) = spawn_table.rules.get_mut(spawn_ref.rule_index) else {
        return;
    };
    let Some(slot) = rule.slots.get_mut(spawn_ref.slot_index) else {
        return;
    };
    slot.next_respawn_tick = Some(respawn_tick_for_schedule(
        &rule.respawn_schedule,
        current_tick,
        spawn_ref.rule_index,
        spawn_ref.slot_index,
    ));
}

pub(super) fn monster_route_target(
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
) -> Option<Point> {
    if agent.route.is_empty() || tick < agent.next_route_tick {
        return None;
    }

    loop {
        let route_index = agent.route_index % agent.route.len();
        let step = &agent.route[route_index];

        if *position != step.location {
            agent.route_waiting = false;
            return Some(step.location.clone());
        }

        if step.delay_ticks > 0 && !agent.route_waiting {
            agent.route_waiting = true;
            agent.next_route_tick = tick + step.delay_ticks;
            return None;
        }

        agent.route_waiting = false;
        agent.route_index = (route_index + 1) % agent.route.len();

        if agent.route.len() == 1 {
            return Some(step.location.clone());
        }
    }
}

pub(super) fn patrol_target(origin: &Point, tick: u64, salt: u32) -> Point {
    let pattern = [
        Point {
            x: origin.x + 1,
            y: origin.y,
        },
        Point {
            x: origin.x,
            y: origin.y - 1,
        },
        Point {
            x: origin.x - 1,
            y: origin.y,
        },
        Point {
            x: origin.x,
            y: origin.y + 1,
        },
    ];
    pattern[((tick + u64::from(salt)) as usize / 2) % pattern.len()].clone()
}

pub(super) fn start_game_visible_respawn_spawns(
    map_file_name: &str,
    respawn: &CrystalRespawnTemplate,
    player_position: &Point,
) -> Vec<(usize, Point, MirDirection)> {
    if respawn.count == 0 {
        return Vec::new();
    }

    if respawn.count == 1 && respawn.spread == 0 {
        if point_in_data_range(&respawn.location, player_position) {
            return vec![(0, respawn.location.clone(), respawn.direction)];
        }
        return Vec::new();
    }

    let spread = i32::from(respawn.spread);
    if spread <= 0 {
        return Vec::new();
    }

    let data_min_x = player_position.x - CRYSTAL_DATA_RANGE;
    let data_max_x = player_position.x + CRYSTAL_DATA_RANGE;
    let data_min_y = player_position.y - CRYSTAL_DATA_RANGE;
    let data_max_y = player_position.y + CRYSTAL_DATA_RANGE;
    let spawn_min_x = respawn.location.x - spread;
    let spawn_max_x = respawn.location.x + spread;
    let spawn_min_y = respawn.location.y - spread;
    let spawn_max_y = respawn.location.y + spread;

    let min_x = data_min_x.max(spawn_min_x);
    let max_x = data_max_x.min(spawn_max_x);
    let min_y = data_min_y.max(spawn_min_y);
    let max_y = data_max_y.min(spawn_max_y);
    if min_x > max_x || min_y > max_y {
        return Vec::new();
    }

    let full_collision = runtime_full_map_collision_data(map_file_name);
    let visible_candidates = full_collision
        .as_ref()
        .map(|collision| walkable_points_in_rect(collision, min_x, max_x, min_y, max_y));
    let spawn_candidate_count = full_collision.as_ref().map(|collision| {
        walkable_point_count_in_rect(
            collision,
            spawn_min_x,
            spawn_max_x,
            spawn_min_y,
            spawn_max_y,
        )
    });
    let (visible_count, visible_cells, width) =
        if let (Some(visible_candidates), Some(spawn_candidate_count)) =
            (&visible_candidates, spawn_candidate_count)
        {
            if spawn_candidate_count == 0 || visible_candidates.is_empty() {
                return Vec::new();
            }
            let visible_count = ((usize::from(respawn.count) * visible_candidates.len())
                / spawn_candidate_count)
                .min(usize::from(respawn.count));
            (
                visible_count,
                u64::try_from(visible_candidates.len()).expect("visible cell count should fit u64"),
                0,
            )
        } else {
            let visible_width = i64::from(max_x - min_x + 1);
            let visible_height = i64::from(max_y - min_y + 1);
            let spawn_side = i64::from(spread * 2 + 1);
            let visible_area = visible_width * visible_height;
            let spawn_area = spawn_side * spawn_side;
            let visible_count = ((i64::from(respawn.count) * visible_area) / spawn_area)
                .clamp(0, i64::from(respawn.count)) as usize;
            (
                visible_count,
                u64::try_from(visible_area).expect("visible area should fit u64"),
                i32::try_from(visible_width).expect("visible width should fit i32"),
            )
        };
    let representative_visible_count =
        if visible_count == 0 && point_in_data_range(&respawn.location, player_position) {
            1
        } else {
            visible_count
        };
    if representative_visible_count == 0 {
        return Vec::new();
    }

    let mut used = BTreeSet::new();
    let mut spawns = Vec::new();

    for slot_index in 0..representative_visible_count {
        let base = deterministic_roll(
            0,
            respawn.respawn_index.max(0) as usize,
            slot_index,
            visible_cells,
        );
        let mut cell_index = i32::try_from(base).expect("visible cell index should fit i32");
        for _ in 0..visible_cells {
            let point = if let Some(visible_candidates) = &visible_candidates {
                visible_candidates[cell_index as usize].clone()
            } else {
                Point {
                    x: min_x + (cell_index % width),
                    y: min_y + (cell_index / width),
                }
            };
            let x = point.x;
            let y = point.y;
            if used.insert((x, y)) {
                let direction = crystal_respawn_dynamic_direction(respawn, slot_index);
                spawns.push((slot_index, point, direction));
                break;
            }
            cell_index = (cell_index + 1) % i32::try_from(visible_cells).unwrap_or(i32::MAX);
        }
    }

    spawns
}

pub(super) fn crystal_respawn_object_id(
    respawn: &CrystalRespawnTemplate,
    slot_index: usize,
) -> u32 {
    if respawn.count == 1 && respawn.spread == 0 {
        return CRYSTAL_RESPAWN_OBJECT_ID_BASE + respawn.respawn_index as u32;
    }

    CRYSTAL_RESPAWN_OBJECT_ID_BASE
        + respawn.respawn_index.max(0) as u32 * 100
        + u32::try_from(slot_index).unwrap_or(u32::MAX)
}

pub(super) fn crystal_respawn_dynamic_direction(
    respawn: &CrystalRespawnTemplate,
    slot_index: usize,
) -> MirDirection {
    match deterministic_roll(0, respawn.respawn_index.max(0) as usize, slot_index + 17, 8) {
        0 => MirDirection::Up,
        1 => MirDirection::UpRight,
        2 => MirDirection::Right,
        3 => MirDirection::DownRight,
        4 => MirDirection::Down,
        5 => MirDirection::DownLeft,
        6 => MirDirection::Left,
        _ => MirDirection::UpLeft,
    }
}

pub(super) fn crystal_respawn_object_monster_packet(
    respawn: &CrystalRespawnTemplate,
    object_id: u32,
    location: Point,
    direction: MirDirection,
) -> ServerPacket {
    let monster = crystal_monster_by_name(&respawn.monster_name);
    ServerPacket::ObjectMonster {
        info: MonsterInfo {
            object_id,
            name: respawn.monster_name.clone(),
            name_colour_argb: -1,
            location,
            image: respawn.monster_image,
            direction,
            effect: monster
                .as_ref()
                .map(|template| template.effect)
                .unwrap_or(0),
            ai: respawn.monster_ai,
            light: monster.as_ref().map(|template| template.light).unwrap_or(0),
            dead: false,
            skeleton: false,
            poison: 0,
            hidden: false,
            shock_time: 0,
            binding_shot_center: false,
            extra: false,
            extra_byte: 0,
            master_object_id: 0,
            rarity: 0,
            buffs: Vec::new(),
        },
    }
}

pub(super) fn point_in_data_range(point: &Point, center: &Point) -> bool {
    point.x >= center.x - CRYSTAL_DATA_RANGE
        && point.x <= center.x + CRYSTAL_DATA_RANGE
        && point.y >= center.y - CRYSTAL_DATA_RANGE
        && point.y <= center.y + CRYSTAL_DATA_RANGE
}

pub(super) fn persist_monster_runtime_state(
    world: &mut World,
    entity: Entity,
    original_agent: &MonsterAgent,
    agent: &MonsterAgent,
    original_ai_state: MonsterAiState,
    ai_state: MonsterAiState,
) {
    if agent == original_agent && ai_state == original_ai_state {
        return;
    }

    let mut entry = world.entity_mut(entity);
    if agent != original_agent {
        entry.insert(agent.clone());
    }
    if ai_state != original_ai_state {
        entry.insert(ai_state);
    }
}

pub(super) fn monster_attack_range(agent: &MonsterAgent) -> i32 {
    match agent.ai {
        4 | 29 | 35 => 2,
        18 => 2,
        27 => 4,
        26 => 6,
        30 => 7,
        33 => 6,
        20 => 3,
        44 => 2,
        43 => 7,
        47 => 1,
        49 => 2,
        34 => 6,
        37 => 3,
        45 | 46 => 6,
        38 => 6,
        13 => agent.view_range.max(1),
        14 => 7,
        50 => 7,
        79 => agent.view_range.max(1),
        16 => 9,
        19 => 2,
        36 => 7,
        126 => 3,
        116 | 117 | 127 | 186 | 188 => 2,
        173 => 2,
        118 => 6,
        181 => agent.view_range.max(1),
        182 => 5,
        12 | 39 => CRYSTAL_DATA_RANGE,
        40 => 1,
        48 => agent.view_range.max(1),
        54 => agent.view_range.max(1),
        60 => 1,
        61 => 12,
        86 => 3,
        88 => 3,
        51 => 8,
        120 => 6,
        121 => 2,
        122 => 6,
        123 => 12,
        102 => 8,
        189 => 9,
        190 => 9,
        192 => 4,
        130 => agent.view_range.max(1),
        131 => agent.view_range.max(1),
        31 | 32 => 8,
        6 | 113 => agent.view_range.max(1),
        57 => 10,
        8 => 6,
        // Families without a bespoke reach fall back to the generated profile's InAttackRange
        // distance (0 means "out to view range"), so data-only ranged casters/archers engage from
        // distance instead of only when adjacent.
        _ => crystal_monster_combat_profile(agent.ai)
            .map(|profile| {
                if profile.attack_range == 0 {
                    agent.view_range.max(1)
                } else {
                    profile.attack_range.max(1)
                }
            })
            .unwrap_or(1),
    }
}

pub(super) fn monster_in_attack_range(
    agent: &MonsterAgent,
    source: &Point,
    target: &Point,
) -> bool {
    match agent.ai {
        16 => tile_distance(source, target) <= 9,
        126 => {
            let dx = (target.x - source.x).abs();
            let dy = (target.y - source.y).abs();
            dx <= 3 && dy <= 3 && (dx != 0 || dy != 0)
        }
        37 => {
            let dx = (target.x - source.x).abs();
            let dy = (target.y - source.y).abs();
            dx <= 3 && dy <= 3 && (dx != 0 || dy != 0) && (dx == 0 || dy == 0 || dx == dy)
        }
        79 => target != source && tile_distance(source, target) <= agent.view_range.max(1),
        88 => {
            let dx = (target.x - source.x).abs();
            let dy = (target.y - source.y).abs();
            dx <= 3 && dy <= 3 && (dx != 0 || dy != 0) && (dx == 0 || dy == 0 || dx == dy)
        }
        192 => {
            let dx = (target.x - source.x).abs();
            let dy = (target.y - source.y).abs();
            dx <= 4 && dy <= 4 && (dx != 0 || dy != 0) && (dx == 0 || dy == 0 || dx == dy)
        }
        26 => {
            let dx = (target.x - source.x).abs();
            let dy = (target.y - source.y).abs();
            dx <= 6 && dy <= 6 && (dx != 0 || dy != 0) && (dx == 0 || dy == 0 || dx == dy)
        }
        27 => {
            let dx = (target.x - source.x).abs();
            let dy = (target.y - source.y).abs();
            dx <= 4 && dy <= 4 && (dx != 0 || dy != 0) && (dx == 0 || dy == 0 || dx == dy)
        }
        19 | 44 | 116 | 117 | 127 | 186 | 188 => {
            let dx = (target.x - source.x).abs();
            let dy = (target.y - source.y).abs();
            dx <= 2
                && dy <= 2
                && (dx != 0 || dy != 0)
                && ((dx <= 1 && dy <= 1) || dx == dy || dx % 2 == dy % 2)
        }
        35 | 173 => {
            let dx = (target.x - source.x).abs();
            let dy = (target.y - source.y).abs();
            dx <= 2
                && dy <= 2
                && (dx != 0 || dy != 0)
                && ((dx <= 1 && dy <= 1) || dx == dy || dx % 2 == dy % 2)
        }
        4 | 18 | 29 | 61 => {
            let dx = (target.x - source.x).abs();
            let dy = (target.y - source.y).abs();
            if dx == 0 && dy == 0 {
                return false;
            }
            let max_range = match agent.ai {
                61 => 12,
                _ => 2,
            };
            if dx > max_range || dy > max_range {
                return false;
            }

            (dx <= max_range && dy <= max_range) || dx == dy || dx % 2 == dy % 2
        }
        _ => tile_distance(source, target) <= monster_attack_range(agent),
    }
}

pub(super) fn monster_uses_ranged_attack(agent: &MonsterAgent) -> bool {
    matches!(
        agent.ai,
        8 | 26 | 32 | 38 | 45 | 46 | 47 | 48 | 54 | 57 | 61 | 113 | 118 | 181 | 182
    )
}

pub(super) fn monster_prefers_ranged_when_not_adjacent(
    agent: &MonsterAgent,
    source: &Point,
    target: &Point,
) -> bool {
    match agent.ai {
        16 => tile_distance(source, target) != 1,
        34 => tile_distance(source, target) != 1,
        27 => tile_distance(source, target) > 1,
        36 => tile_distance(source, target) > 2,
        50 => tile_distance(source, target) > 2,
        123 => tile_distance(source, target) > 2,
        131 => tile_distance(source, target) > 2,
        19 | 20 | 30 | 31 | 32 | 33 | 51 | 86 | 102 | 118 | 120 | 121 | 122 | 130 | 181 | 182
        | 189 | 190 => tile_distance(source, target) != 1,
        _ => monster_uses_ranged_attack(agent),
    }
}

pub(super) fn monster_attack_packet_direction(
    agent: &MonsterAgent,
    current_direction: MirDirection,
    target_direction: MirDirection,
) -> MirDirection {
    match agent.ai {
        79 => current_direction,
        _ => target_direction,
    }
}

pub(super) fn monster_broadcasts_attack_on_damage_due(agent: &MonsterAgent) -> bool {
    matches!(agent.ai, 48 | 49 | 54)
}

pub(super) fn monster_can_follow_route(agent: &MonsterAgent) -> bool {
    !matches!(
        agent.ai,
        3 | 5
            | 6
            | 12
            | 13
            | 14
            | 18
            | 39
            | 40
            | 47
            | 48
            | 50
            | 54
            | 56
            | 57
            | 58
            | 61
            | 79
            | 98
            | 99
            | 113
            | 119
            | 120
            | 122
            | 128
            | 255
    )
}

pub(super) fn monster_can_chase_player(agent: &MonsterAgent) -> bool {
    agent.can_wander
        && !matches!(
            agent.ai,
            3 | 5
                | 6
                | 12
                | 13
                | 14
                | 18
                | 39
                | 47
                | 48
                | 50
                | 52
                | 53
                | 54
                | 56
                | 57
                | 58
                | 61
                | 79
                | 82
                | 89
                | 98
                | 99
                | 113
                | 119
                | 120
                | 122
                | 128
                | 142
                | 149
                | 151
                | 158
                | 166
                | 170
                | 212
                | 255
        )
}

pub(super) fn monster_can_patrol_origin(agent: &MonsterAgent) -> bool {
    agent.can_wander
        && !matches!(
            agent.ai,
            3 | 5
                | 6
                | 12
                | 13
                | 14
                | 18
                | 39
                | 40
                | 47
                | 48
                | 50
                | 52
                | 53
                | 54
                | 56
                | 57
                | 58
                | 61
                | 79
                | 82
                | 89
                | 98
                | 99
                | 113
                | 119
                | 120
                | 122
                | 128
                | 142
                | 149
                | 151
                | 158
                | 166
                | 170
                | 212
                | 255
        )
}

pub(super) fn monster_attack_delay_ticks(
    agent: &MonsterAgent,
    source: &Point,
    target: &Point,
) -> u64 {
    match agent.ai {
        7 | 10 | 11 | 13 | 17 | 28 | 35 | 49 | 79 | 112 | 115 | 173 => combat_delay_ticks(300),
        38 => combat_delay_ticks(500),
        14 => combat_delay_ticks(500),
        174 => combat_delay_ticks(600),
        16 if tile_distance(source, target) > 1 => combat_delay_ticks(500),
        16 => combat_delay_ticks(300),
        34 if tile_distance(source, target) > 1 => ranged_attack_delay_ticks(source, target),
        34 => combat_delay_ticks(300),
        22 => combat_delay_ticks(300),
        36 if tile_distance(source, target) > 2 => combat_delay_ticks(500),
        36 => combat_delay_ticks(300),
        37 if tile_distance(source, target) > 1 => {
            let distance = u64::try_from(tile_distance(source, target).max(0))
                .expect("tile distance should fit u64");
            combat_delay_ticks(distance * 50 + 300)
        }
        37 => combat_delay_ticks(300),
        179 => combat_delay_ticks(350),
        180 => combat_delay_ticks(500),
        26 => combat_delay_ticks(300),
        4 => {
            let distance = u64::try_from(tile_distance(source, target).max(0))
                .expect("tile distance should fit u64");
            combat_delay_ticks(distance * 50 + 300)
        }
        29 => {
            let distance = u64::try_from(tile_distance(source, target).max(0))
                .expect("tile distance should fit u64");
            combat_delay_ticks(distance * 50 + 250)
        }
        27 => combat_delay_ticks(300),
        19 if tile_distance(source, target) > 1 => {
            let distance = u64::try_from(tile_distance(source, target).max(0))
                .expect("tile distance should fit u64");
            combat_delay_ticks(distance * 50 + 300)
        }
        19 => combat_delay_ticks(300),
        20 if tile_distance(source, target) > 1 => combat_delay_ticks(500),
        20 => combat_delay_ticks(300),
        43 if tile_distance(source, target) > 2 => combat_delay_ticks(500),
        43 => combat_delay_ticks(300),
        30 if tile_distance(source, target) > 1 => ranged_attack_delay_ticks(source, target),
        30 => combat_delay_ticks(300),
        61 => ranged_attack_delay_ticks(source, target),
        86 if tile_distance(source, target) > 1 => combat_delay_ticks(500),
        86 => combat_delay_ticks(300),
        88 => combat_delay_ticks(500),
        44 if tile_distance(source, target) > 1 => combat_delay_ticks(250),
        45 => combat_delay_ticks(500),
        48 => combat_delay_ticks(500),
        50 => combat_delay_ticks(300),
        54 => combat_delay_ticks(500),
        51 if tile_distance(source, target) > 1 => ranged_attack_delay_ticks(source, target),
        51 => combat_delay_ticks(300),
        102 if tile_distance(source, target) > 1 => combat_delay_ticks(500),
        102 => combat_delay_ticks(300),
        126 if tile_distance(source, target) > 1 => {
            let distance = u64::try_from(tile_distance(source, target).max(0))
                .expect("tile distance should fit u64");
            combat_delay_ticks(distance * 50 + 500)
        }
        126 => combat_delay_ticks(300),
        127 => combat_delay_ticks(300),
        187 => combat_delay_ticks(600),
        188 if tile_distance(source, target) > 1 => combat_delay_ticks(500),
        186 => combat_delay_ticks(300),
        189 if tile_distance(source, target) > 1 => {
            let distance = u64::try_from(tile_distance(source, target).max(0))
                .expect("tile distance should fit u64");
            combat_delay_ticks(distance * 50 + 600)
        }
        189 => combat_delay_ticks(600),
        190 if tile_distance(source, target) > 1 => ranged_attack_delay_ticks(source, target),
        190 => combat_delay_ticks(500),
        192 if tile_distance(source, target) > 1 => combat_delay_ticks(1),
        192 => melee_attack_delay_ticks(),
        130 if tile_distance(source, target) > 1 => ranged_attack_delay_ticks(source, target),
        130 => combat_delay_ticks(300),
        131 if tile_distance(source, target) > 2 => ranged_attack_delay_ticks(source, target),
        131 => combat_delay_ticks(500),
        116 if tile_distance(source, target) > 1 => combat_delay_ticks(300),
        117 if tile_distance(source, target) > 1 => combat_delay_ticks(1_000),
        118 if tile_distance(source, target) > 1 => combat_delay_ticks(500),
        120 if tile_distance(source, target) > 1 => combat_delay_ticks(500),
        120 => combat_delay_ticks(300),
        121 => combat_delay_ticks(300),
        122 if tile_distance(source, target) > 1 => ranged_attack_delay_ticks(source, target),
        122 => combat_delay_ticks(300),
        123 => combat_delay_ticks(500),
        124 | 125 => combat_delay_ticks(400),
        181 if tile_distance(source, target) <= 1 => combat_delay_ticks(600),
        33 if tile_distance(source, target) > 1 => combat_delay_ticks(500),
        33 => combat_delay_ticks(300),
        31 => combat_delay_ticks(500),
        32 => ranged_attack_delay_ticks(source, target),
        _ if monster_uses_ranged_attack(agent) => ranged_attack_delay_ticks(source, target),
        _ => melee_attack_delay_ticks(),
    }
}

/// Damage-class fallback driven by the generated per-AI combat profile: casters use magic/spell
/// power, distance-gated families use DC in melee and MC at range. Families with a bespoke case in
/// `monster_player_attack_damage` never reach this.
fn crystal_monster_profile_attack_damage(
    name: &str,
    ai: u8,
    source: &Point,
    target: &Point,
    tick: u64,
    attacker_id: u32,
) -> i32 {
    match crystal_monster_combat_profile(ai).map(|profile| profile.damage) {
        Some(damage) if damage == "mc" => {
            crystal_monster_magic_damage_rolled(name, tick, attacker_id)
        }
        Some(damage) if damage == "sc" => {
            crystal_monster_spell_damage_rolled(name, tick, attacker_id)
        }
        Some(damage) if damage == "dc_mc" && tile_distance(source, target) > 1 => {
            crystal_monster_magic_damage_rolled(name, tick, attacker_id)
        }
        _ => crystal_monster_attack_damage_rolled(name, tick, attacker_id),
    }
}

/// Crystal tags each monster attack with a `DefenceType`; `MAC`/`MACAgility` attacks are reduced by
/// the target's magic armour rather than physical AC. This captures, per AI family, whether the
/// attack at the given range resolves against the player's magic defence (extracted from each
/// subclass's `Attack()`/range-attack `DefenceType`).
pub(super) fn monster_attack_uses_magic_defence(ai: u8, source: &Point, target: &Point) -> bool {
    match ai {
        // Normal attack always carries a MAC* defence type (full Crystal roster, extracted from each
        // subclass's Attack() override — covers spawned and data-only families alike).
        7 | 10 | 14 | 16 | 17 | 22 | 26 | 28 | 30 | 31 | 32 | 36 | 38 | 45 | 46 | 50 | 52 | 55 | 61
        | 63 | 73 | 75 | 87 | 92 | 94 | 100 | 102 | 103 | 107 | 108 | 110 | 120 | 122 | 135 | 136
        | 140 | 142 | 152 | 160 | 210 | 215 | 216 => true,
        // Physical in melee, magic at range (the Attack() mixes ACAgility and MAC*).
        19 | 34 | 37 | 43 | 49 | 67 | 72 | 85 | 91 | 105 | 115 | 118 | 121 | 123 | 129 | 130 | 131
        | 137 | 141 | 143 | 144 | 147 | 148 | 150 | 153 | 155 | 156 | 159 | 162 | 163 | 167 | 178
        | 181 | 182 | 196 | 200 | 212 | 223 => tile_distance(source, target) > 1,
        _ => false,
    }
}

/// The player armour that mitigates a given monster attack: magic defence (`Stat.MaxMAC`) for
/// MAC-typed attacks, physical AC (`total_defence_bonus`) otherwise.
pub(super) fn monster_player_damage_mitigation(
    world: &World,
    ai: u8,
    source: &Point,
    target: &Point,
) -> i32 {
    let resources = world.resource::<InventoryResource>();
    let buffs = world.resource::<BuffResource>();
    if monster_attack_uses_magic_defence(ai, source, target) {
        current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MAX_MAC)
    } else {
        total_defence_bonus(resources, buffs)
    }
}

pub(super) fn monster_player_attack_damage(
    world: &World,
    monster_name: &str,
    agent: &MonsterAgent,
    source: &Point,
    target: &Point,
    attacker_id: u32,
) -> i32 {
    let tick = super::session::runtime_tick(world);
    let base_damage = match agent.ai {
        7 | 10 | 11 | 13 | 14 | 17 | 22 | 28 | 30 | 33 | 35 | 36 | 37 | 38 | 49 | 51 | 54 | 79
        | 112 | 115 | 173 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        48 => 0,
        16 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        19 if tile_distance(source, target) > 1 => crystal_monster_magic_damage_rolled(monster_name, tick, attacker_id),
        19 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        20 if tile_distance(source, target) > 1 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id) * 3,
        20 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        43 if tile_distance(source, target) > 2 => crystal_monster_magic_damage_rolled(monster_name, tick, attacker_id),
        43 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        47 => 0,
        50 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        27 if tile_distance(source, target) > 1 => 0,
        27 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        34 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        174 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        179 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        180 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        186 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        120 if tile_distance(source, target) > 1 => crystal_monster_raw_magic_damage_rolled(monster_name, tick, attacker_id),
        119 | 120 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        121 if tile_distance(source, target) > 1 => crystal_monster_magic_damage_rolled(monster_name, tick, attacker_id),
        121 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        122 if tile_distance(source, target) > 1 => crystal_monster_raw_magic_damage_rolled(monster_name, tick, attacker_id),
        122 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        123 if tile_distance(source, target) > 2 => crystal_monster_magic_damage_rolled(monster_name, tick, attacker_id),
        123 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        124 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        125 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id) * 2,
        102 if tile_distance(source, target) > 1 => crystal_monster_raw_magic_damage_rolled(monster_name, tick, attacker_id),
        102 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        86 if tile_distance(source, target) > 2 => crystal_monster_magic_damage_rolled(monster_name, tick, attacker_id),
        86 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        88 => crystal_monster_magic_damage_rolled(monster_name, tick, attacker_id),
        126 if tile_distance(source, target) > 1 => crystal_monster_raw_magic_damage_rolled(monster_name, tick, attacker_id),
        126 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        127 if tile_distance(source, target) > 1 => crystal_monster_magic_damage_rolled(monster_name, tick, attacker_id),
        127 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        187 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        188 if tile_distance(source, target) > 1 => {
            crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id) * 2
        }
        188 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        189 if tile_distance(source, target) > 1 => crystal_monster_raw_magic_damage_rolled(monster_name, tick, attacker_id),
        189 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        190 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        192 if tile_distance(source, target) > 1 => {
            crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id) * 3
        }
        192 => crystal_monster_raw_attack_damage_rolled(monster_name, tick, attacker_id),
        130 if tile_distance(source, target) > 1 => crystal_monster_magic_damage_rolled(monster_name, tick, attacker_id),
        130 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        131 if tile_distance(source, target) > 2 => crystal_monster_spell_damage_rolled(monster_name, tick, attacker_id),
        131 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        118 if tile_distance(source, target) > 1 => crystal_monster_raw_magic_damage_rolled(monster_name, tick, attacker_id),
        118 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        181 if tile_distance(source, target) > 1 => crystal_monster_magic_damage_rolled(monster_name, tick, attacker_id),
        181 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        182 if tile_distance(source, target) > 1 => crystal_monster_raw_magic_damage_rolled(monster_name, tick, attacker_id),
        182 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        45 | 46 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        117 if tile_distance(source, target) > 1 => crystal_monster_raw_magic_damage_rolled(monster_name, tick, attacker_id),
        117 => crystal_monster_attack_damage_rolled(monster_name, tick, attacker_id),
        // Crystal `MonsterObject.Attack` deals `GetAttackPower(MinDC, MaxDC)` for every family that
        // does not override it (SpittingSpider, ShamanZombie, BoneSpearman, VampireSpider,
        // SpittingToad, the plain MonsterObject, …). The previous flat `7` placeholder made those
        // monsters deal a fixed 7 to players regardless of their stats; fall back to the monster's
        // real damage-class instead. Families that never strike players (guards, training dummy,
        // egg, devil node) never reach this path.
        _ => crystal_monster_profile_attack_damage(
            monster_name,
            agent.ai,
            source,
            target,
            tick,
            attacker_id,
        ),
    };
    let mitigation = monster_player_damage_mitigation(world, agent.ai, source, target);
    if base_damage <= 0 {
        return 0;
    }
    (base_damage - mitigation).max(1)
}

pub(super) fn monster_player_status_effect(
    agent: &MonsterAgent,
) -> Option<PendingPlayerStatusEffect> {
    match agent.ai {
        // SpittingSpider.CompleteAttack: PoisonTarget(target, 8, 5, PoisonType.Green, 2000).
        4 => Some(PendingPlayerStatusEffect::GreenPoison {
            chance_denominator: SPITTING_SPIDER_GREEN_POISON_CHANCE_DENOMINATOR,
            duration_ticks: SPITTING_SPIDER_GREEN_POISON_DURATION_TICKS,
        }),
        7 => Some(PendingPlayerStatusEffect::Paralysis {
            chance_denominator: CAVE_MAGGOT_PARALYSIS_CHANCE_DENOMINATOR,
            duration_ticks: CAVE_MAGGOT_PARALYSIS_DURATION_TICKS,
        }),
        22 => Some(PendingPlayerStatusEffect::Paralysis {
            chance_denominator: INCARNATED_ZT_PARALYSIS_CHANCE_DENOMINATOR,
            duration_ticks: INCARNATED_ZT_PARALYSIS_DURATION_TICKS,
        }),
        28 => Some(PendingPlayerStatusEffect::GreenPoison {
            chance_denominator: TOXIC_GHOUL_GREEN_POISON_CHANCE_DENOMINATOR,
            duration_ticks: TOXIC_GHOUL_GREEN_POISON_DURATION_TICKS,
        }),
        37 => Some(PendingPlayerStatusEffect::GreenPoison {
            chance_denominator: TOXIC_GHOUL_GREEN_POISON_CHANCE_DENOMINATOR,
            duration_ticks: TOXIC_GHOUL_GREEN_POISON_DURATION_TICKS,
        }),
        // EvilMir (ai 52): green DOT + paralysis on every landed hit (the profile only carries green).
        52 => Some(PendingPlayerStatusEffect::GreenPoisonAndParalysis {
            green_chance_denominator: EVIL_MIR_GREEN_POISON_CHANCE_DENOMINATOR,
            green_duration_ticks: EVIL_MIR_GREEN_POISON_DURATION_TICKS,
            green_salt: 152,
            paralysis_chance_denominator: EVIL_MIR_PARALYSIS_CHANCE_DENOMINATOR,
            paralysis_duration_ticks: EVIL_MIR_PARALYSIS_DURATION_TICKS,
            paralysis_salt: 153,
        }),
        // Data-only families that apply TWO poisons per hit, 1:1 with each subclass's Crystal
        // `PoisonTarget(target, chance, duration, type)` calls. The generated profile only carries one
        // of the two, so these bespoke cases surface the pair (a rare tertiary poison is omitted —
        // no triple-poison variant exists).
        // FinialTurtle.Attack: Slow(8,15) + Frozen(15,5).
        72 => Some(PendingPlayerStatusEffect::SlowAndFrozen {
            slow_chance_denominator: 8,
            slow_duration_ticks: 15,
            slow_salt: 1720,
            frozen_chance_denominator: 15,
            frozen_duration_ticks: 5,
            frozen_salt: 1721,
        }),
        // OmaMage.Attack: Slow(6,5) + Frozen(9,5).
        147 => Some(PendingPlayerStatusEffect::SlowAndFrozen {
            slow_chance_denominator: 6,
            slow_duration_ticks: 5,
            slow_salt: 1470,
            frozen_chance_denominator: 9,
            frozen_duration_ticks: 5,
            frozen_salt: 1471,
        }),
        // RhinoPriest.Attack: Slow(2,5) + Frozen(4,5).
        137 => Some(PendingPlayerStatusEffect::SlowAndFrozen {
            slow_chance_denominator: 2,
            slow_duration_ticks: 5,
            slow_salt: 1370,
            frozen_chance_denominator: 4,
            frozen_duration_ticks: 5,
            frozen_salt: 1371,
        }),
        // KingGuard.Attack: Slow(5,10) + Paralysis(5,10) (a tertiary green is not modelled).
        105 => Some(PendingPlayerStatusEffect::SlowAndParalysis {
            slow_chance_denominator: 5,
            slow_duration_ticks: 10,
            slow_salt: 1050,
            paralysis_chance_denominator: 5,
            paralysis_duration_ticks: 10,
            paralysis_salt: 1051,
        }),
        // TurtleKing.Attack: Slow(5,15) + Paralysis(5,5) (a tertiary dazed is not modelled).
        73 => Some(PendingPlayerStatusEffect::SlowAndParalysis {
            slow_chance_denominator: 5,
            slow_duration_ticks: 15,
            slow_salt: 730,
            paralysis_chance_denominator: 5,
            paralysis_duration_ticks: 5,
            paralysis_salt: 731,
        }),
        // GasToad.Attack: Green(1,5) + Paralysis(1,5) — both land on every hit.
        132 => Some(PendingPlayerStatusEffect::GreenPoisonAndParalysis {
            green_chance_denominator: 1,
            green_duration_ticks: 5,
            green_salt: 1320,
            paralysis_chance_denominator: 1,
            paralysis_duration_ticks: 5,
            paralysis_salt: 1321,
        }),
        // On-hit poisons the generated profile missed (the PoisonTarget lives in CompleteRangeAttack
        // or a non-standard branch), restored 1:1 with each subclass's Crystal call.
        // On-hit poisons the generated profile missed (the PoisonTarget lives in CompleteAttack with
        // a stat-rolled duration), restored 1:1 with each subclass's Crystal `PoisonTarget(target,
        // chance, duration, type)`. The chance is faithful; a stat-derived duration (GetAttackPower)
        // is approximated as a documented fixed tick count.
        // LightTurtle.CompleteAttack: PoisonTarget(_, 4, GetAttackPower(MinSC,MaxSC), Green).
        74 => Some(PendingPlayerStatusEffect::GreenPoison {
            chance_denominator: 4,
            duration_ticks: 5,
        }),
        // HellCannibal.Attack: PoisonTarget(_, 1, Random.Next(GetAttackPower(MinSC,MaxSC)/2), Red).
        78 => Some(PendingPlayerStatusEffect::RedPoison {
            chance_denominator: 1,
            duration_ticks: 5,
            salt: 780,
        }),
        // FlameAssassin.Attack: PoisonTarget(_, 1, GetAttackPower(MinMC,MaxMC), Slow).
        95 => Some(PendingPlayerStatusEffect::SlowPoison {
            chance_denominator: 1,
            duration_ticks: 5,
            salt: 950,
        }),
        // Dazed on-hit families (gate the player's next attack), 1:1 with their Crystal PoisonTarget;
        // each Crystal duration is a `Random.Next` roll, approximated as a fixed tick count.
        // HellSlasher.CompleteAttack: PoisonTarget(_, 5, Random.Next(1,4), Dazed).
        76 => Some(PendingPlayerStatusEffect::DazedPoison {
            chance_denominator: 5,
            duration_ticks: 3,
            salt: 760,
        }),
        // TrollKing.Attack: PoisonTarget(_, 1, Random.Next(MaxMC), Dazed).
        91 => Some(PendingPlayerStatusEffect::DazedPoison {
            chance_denominator: 1,
            duration_ticks: 5,
            salt: 910,
        }),
        // StoningStatue.Attack: PoisonTarget(_, 2, Random.Next(5,10), Dazed).
        135 => Some(PendingPlayerStatusEffect::DazedPoison {
            chance_denominator: 2,
            duration_ticks: 5,
            salt: 1350,
        }),
        // Generated-profile poison fallback: families without a bespoke poison case apply the
        // on-hit poison extracted from their Crystal `PoisonTarget` call (data-only families etc.).
        ai => crystal_monster_profile_poison(ai),
    }
}

fn crystal_monster_profile_poison(ai: u8) -> Option<PendingPlayerStatusEffect> {
    // Spawned families already have curated poison (explicit cases plus the advance_world cascade,
    // including non-poison special attacks like SandSnail's halfmoon); only fill the gap for the
    // never-spawned data-only classes.
    if !crystal_monster_ai_is_data_only(ai) {
        return None;
    }
    let poison = crystal_monster_combat_profile(ai)?.poison?;
    let chance_denominator = poison.chance.max(1);
    let duration_ticks = poison.duration;
    let salt = 0x9_0000 + u64::from(ai);
    Some(match poison.kind.as_str() {
        "green" => PendingPlayerStatusEffect::GreenPoison {
            chance_denominator,
            duration_ticks,
        },
        "red" => PendingPlayerStatusEffect::RedPoison {
            chance_denominator,
            duration_ticks,
            salt,
        },
        "paralysis" => PendingPlayerStatusEffect::Paralysis {
            chance_denominator,
            duration_ticks,
        },
        "slow" => PendingPlayerStatusEffect::SlowPoison {
            chance_denominator,
            duration_ticks,
            salt,
        },
        "frozen" => PendingPlayerStatusEffect::FrozenPoison {
            chance_denominator,
            duration_ticks,
            salt,
        },
        "bleeding" => PendingPlayerStatusEffect::BleedingPoison {
            chance_denominator,
            duration_ticks,
            salt,
        },
        "stun" => PendingPlayerStatusEffect::StunPoison {
            chance_denominator,
            duration_ticks,
            salt,
        },
        "blindness" => PendingPlayerStatusEffect::BlindnessPoison {
            chance_denominator,
            duration_ticks,
            salt,
        },
        _ => return None,
    })
}

pub(super) fn monster_locks_player_target_on_hit(agent: &MonsterAgent) -> bool {
    monster_can_chase_player(agent) && !matches!(agent.ai, 1 | 2 | 3)
}

pub(super) fn guard_can_target_monster(attacker: &MonsterAgent, target: &MonsterAgent) -> bool {
    matches!(attacker.ai, 6 | 113)
        && !target.dead
        && !matches!(target.ai, 1 | 2 | 3 | 6 | 57 | 58 | 113)
}

pub(super) fn find_guard_target_monster(
    world: &World,
    monster_entities: &[Entity],
    attacker: Entity,
    attacker_position: &Point,
    attacker_agent: &MonsterAgent,
) -> Option<Entity> {
    let attack_range = monster_attack_range(attacker_agent);
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
        if !guard_can_target_monster(attacker_agent, target_agent) {
            continue;
        }
        if target_ai_state.hidden || monster_is_stoned_zuma(target_agent, &target_ai_state) {
            continue;
        }

        let Some(target_position) = entry.get::<Position>().map(|position| position.0.clone())
        else {
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

pub(super) fn monster_melee_attack_packet(
    world: &World,
    entity: Entity,
    location: &Point,
    direction: MirDirection,
) -> Option<ServerPacket> {
    monster_typed_attack_packet(world, entity, location, direction, 0)
}

pub(super) fn monster_typed_attack_packet(
    world: &World,
    entity: Entity,
    location: &Point,
    direction: MirDirection,
    attack_type: u8,
) -> Option<ServerPacket> {
    Some(ServerPacket::ObjectAttack {
        info: ObjectAttackInfo {
            object_id: entity_object_id(world, entity)?,
            location: location.clone(),
            direction,
            spell: 0,
            level: 0,
            attack_type,
        },
    })
}

pub(super) fn monster_object_attack_type(
    agent: &MonsterAgent,
    source: &Point,
    target: &Point,
) -> u8 {
    if tile_distance(source, target) <= 1 {
        return 0;
    }

    match agent.ai {
        37 | 44 | 116 | 126 | 127 | 188 => 1,
        43 if tile_distance(source, target) > 2 => 1,
        192 => 2,
        117 => 2,
        _ => 0,
    }
}

pub(super) fn monster_object_range_attack_type(
    agent: &MonsterAgent,
    source: &Point,
    target: &Point,
) -> u8 {
    match agent.ai {
        131 if tile_distance(source, target) > 2 => 1,
        _ => 0,
    }
}

pub(super) fn guard_melee_attack_packets(
    world: &World,
    entity: Entity,
    target_entity: Entity,
) -> Option<[ServerPacket; 2]> {
    let target_position = entity_position(world, target_entity)?;
    let target_facing = entity_facing(world, target_entity)?;
    let target_back = offset_point(&target_position, target_facing, -1);
    let attack_direction = direction_toward(&target_back, &target_position)?;

    Some([
        ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: entity_object_id(world, entity)?,
                location: target_back,
                direction: attack_direction,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        },
        ServerPacket::ObjectTurn {
            movement: object_movement(world, entity)?,
        },
    ])
}

pub(super) fn monster_typed_ranged_attack_packet(
    world: &World,
    entity: Entity,
    location: &Point,
    direction: MirDirection,
    target_entity: Entity,
    target: &Point,
    attack_type: u8,
) -> Option<ServerPacket> {
    Some(ServerPacket::ObjectRangeAttack {
        info: ObjectRangeAttackInfo {
            object_id: entity_object_id(world, entity)?,
            location: location.clone(),
            direction,
            target_id: entity_object_id(world, target_entity)?,
            target: target.clone(),
            attack_type,
            spell: 0,
            level: 0,
        },
    })
}

pub(super) fn object_effect_packet(
    world: &World,
    entity: Entity,
    effect: u8,
) -> Option<ServerPacket> {
    Some(ServerPacket::ObjectEffect {
        info: ObjectEffectInfo {
            object_id: entity_object_id(world, entity)?,
            effect,
            effect_type: 0,
            delay_time: 0,
            time: 0,
        },
    })
}

pub(super) fn general_meow_meow_thunder_spell_packet(
    world: &mut World,
    location: Point,
    direction: MirDirection,
) -> ServerPacket {
    ServerPacket::ObjectSpell {
        info: ObjectSpellInfo {
            object_id: allocate_runtime_monster_object_id(world),
            location,
            spell: Spell::GeneralMeowMeowThunder,
            direction,
            param: false,
        },
    }
}

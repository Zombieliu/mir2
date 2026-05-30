use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{With, Without, World};
use mir2_game_data::{
    crystal_map_respawns_by_file_name, crystal_map_respawns_by_index, crystal_npc_info_manifest,
    localized_text_or_fallback, starter_map_collision, BlockedMapCellTemplate, DecorKind,
    DecorObjectTemplate, DoorMapCellTemplate, MapBounds, MapCellAttribute, SceneView,
    StarterMapCollision, TerrainKind, TerrainPatchTemplate,
};
use mir2_protocol::{ChatType, MapInformation, MirDirection, Point, ServerPacket};

use super::components::{
    entity_position, player_entity, CharacterBody, DisplayName, Facing, Hero, Monster,
    MonsterAgent, MonsterCombatStats, MonsterVitals, Npc, NpcAgent, ObjectId, PlayerVitals,
    Position, RemotePlayer, SelfPlayer, SpawnSlotRef, WorldObject,
};
use super::crystal_compat::DEFAULT_CRYSTAL_CLIENT_ROOT;
use super::monsters::{
    build_crystal_current_map_visible_spawn_table, build_spawn_table,
    initial_general_meow_meow_state, initial_monster_ai_state_for_object, initial_yimoogi_state,
};
use super::movement::{current_location, point_in_bounds, summon_spawn_position_near, tile_key};
use super::packets::{
    localized_monster_name_key, localized_npc_name_key, localized_visible_player_name_key,
};
use super::resources::{
    current_language, is_in_world, reset_crystal_player_movement_timing, MapRuntimeResource,
    NpcStateResource, PlayerRuntimeResource, RuntimeConfigResource, RuntimeQueueResource,
    SessionResource, Stage5SystemsResource,
};
use super::save::{active_character_runtime_state, ActiveCharacterRuntimeState};
use super::session::{system_message, SimulationSession};
use crate::config::{MapDropRuleRecord, MonsterSpawnSource, SimulationConfig};
use crate::MapTransferRecord;

#[derive(Debug, Clone)]
pub(super) struct RuntimeMapCollisionData {
    pub(super) collision: StarterMapCollision,
    pub(super) blocked_set: BTreeSet<(i32, i32)>,
    pub(super) closed_door_set: BTreeSet<(i32, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZoneMapCollisionData {
    pub bounds: MapBounds,
    pub blocked_cells: BTreeSet<(i32, i32)>,
    pub transfer_source_cells: BTreeSet<(i32, i32)>,
}

pub(super) fn normalize_map_file_name(file_name: &str) -> String {
    file_name
        .trim()
        .trim_end_matches(".map")
        .trim_end_matches(".MAP")
        .to_ascii_lowercase()
}

pub(crate) fn zone_map_collision_data(map_file_name: &str) -> Option<ZoneMapCollisionData> {
    let collision = runtime_full_map_collision_data(map_file_name)
        .or_else(|| runtime_map_collision_data(map_file_name))?;
    let mut blocked_cells = collision.blocked_set;
    blocked_cells.extend(collision.closed_door_set);
    let transfer_source_cells = crystal_direct_movement_transfer_source_cells(map_file_name);
    Some(ZoneMapCollisionData {
        bounds: collision.collision.region_bounds,
        blocked_cells,
        transfer_source_cells,
    })
}

pub(super) fn is_safe_zone_point(
    config: &SimulationConfig,
    map: &MapRuntimeResource,
    point: &Point,
) -> bool {
    if config.safe_zones.iter().any(|zone| {
        normalize_map_file_name(&zone.map_file_name)
            == normalize_map_file_name(&map.current_map.file_name)
            && point_in_bounds(&zone.bounds, point)
    }) {
        return true;
    }

    crystal_map_respawns_by_file_name(&map.current_map.file_name)
        .map(|map| {
            map.safe_zones.iter().any(|zone| {
                let size = i32::from(zone.size);
                point_in_bounds(
                    &MapBounds {
                        min_x: zone.location.x - size,
                        max_x: zone.location.x + size,
                        min_y: zone.location.y - size,
                        max_y: zone.location.y + size,
                    },
                    point,
                )
            })
        })
        .unwrap_or(false)
}

pub(super) fn current_map_drop_rule<'a>(
    config: &'a SimulationConfig,
    map: &MapRuntimeResource,
) -> Option<&'a MapDropRuleRecord> {
    let current_map = normalize_map_file_name(&map.current_map.file_name);
    config
        .map_drop_rules
        .iter()
        .find(|rule| normalize_map_file_name(&rule.map_file_name) == current_map)
}

pub(super) fn current_map_disallows_town_teleport(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_drop_rule(config, map)
        .map(|rule| rule.no_town_teleport)
        .unwrap_or(false)
}

pub(super) fn current_map_disallows_escape(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_drop_rule(config, map)
        .map(|rule| rule.no_escape)
        .unwrap_or(false)
}

pub(super) fn current_map_disallows_random_teleport(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_drop_rule(config, map)
        .map(|rule| rule.no_random)
        .unwrap_or(false)
}

pub(super) fn current_map_disallows_drug(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_drop_rule(config, map)
        .map(|rule| rule.no_drug)
        .unwrap_or(false)
}

pub(super) fn current_map_disallows_reincarnation(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_drop_rule(config, map)
        .map(|rule| rule.no_reincarnation)
        .unwrap_or(false)
}

pub(super) fn current_map_manifest_disallows_throw_item(map: &MapRuntimeResource) -> bool {
    crystal_map_respawns_by_file_name(&map.current_map.file_name)
        .map(|map| map.no_throw_item)
        .unwrap_or(false)
}

pub(super) fn current_map_disallows_throw_item(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_manifest_disallows_throw_item(map)
        || current_map_drop_rule(config, map)
            .map(|rule| rule.no_throw_item)
            .unwrap_or(false)
}

pub(super) fn current_map_manifest_disallows_monster_drop(map: &MapRuntimeResource) -> bool {
    crystal_map_respawns_by_file_name(&map.current_map.file_name)
        .map(|map| map.no_drop_monster)
        .unwrap_or(false)
}

pub(super) fn current_map_disallows_monster_drop(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_manifest_disallows_monster_drop(map)
        || current_map_drop_rule(config, map)
            .map(|rule| rule.no_drop_monster)
            .unwrap_or(false)
}

pub(super) fn current_map_manifest_disallows_mount(map: &MapRuntimeResource) -> bool {
    crystal_map_respawns_by_file_name(&map.current_map.file_name)
        .map(|map| map.no_mount)
        .unwrap_or(false)
}

pub(super) fn current_map_disallows_mount(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_manifest_disallows_mount(map)
        || current_map_drop_rule(config, map)
            .map(|rule| rule.no_mount)
            .unwrap_or(false)
}

pub(super) fn current_map_manifest_disallows_hero(map: &MapRuntimeResource) -> bool {
    crystal_map_respawns_by_file_name(&map.current_map.file_name)
        .map(|map| map.no_hero)
        .unwrap_or(false)
}

pub(super) fn current_map_disallows_hero(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_manifest_disallows_hero(map)
        || current_map_drop_rule(config, map)
            .map(|rule| rule.no_hero)
            .unwrap_or(false)
}

pub(super) fn current_map_manifest_requires_bridle(map: &MapRuntimeResource) -> bool {
    crystal_map_respawns_by_file_name(&map.current_map.file_name)
        .map(|map| map.need_bridle)
        .unwrap_or(false)
}

pub(super) fn current_map_requires_bridle(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    current_map_manifest_requires_bridle(map)
        || current_map_drop_rule(config, map)
            .map(|rule| rule.need_bridle)
            .unwrap_or(false)
}

pub(super) fn transfer_for_key(
    config: &SimulationConfig,
    map: &MapRuntimeResource,
    key: &str,
) -> Option<MapTransferRecord> {
    config
        .map_transfers
        .iter()
        .find(|transfer| transfer.key == key)
        .cloned()
        .or_else(|| {
            crystal_movement_transfer_records_for_map(&map.current_map.file_name)
                .into_iter()
                .find(|transfer| transfer.key == key)
        })
}

pub(super) fn transfer_for_current_player_position(world: &World) -> Option<MapTransferRecord> {
    let player = player_entity(world)?;
    let position = entity_position(world, player)?;
    let map = world.resource::<MapRuntimeResource>();
    let current_map = normalize_map_file_name(&map.current_map.file_name);
    let config = &world.resource::<RuntimeConfigResource>().config;

    config
        .map_transfers
        .iter()
        .filter(|transfer| normalize_map_file_name(&transfer.from_map_file_name) == current_map)
        .cloned()
        .chain(crystal_movement_transfer_records_for_map(
            &map.current_map.file_name,
        ))
        .find(|transfer| point_in_bounds(&transfer.from_bounds, &position))
}

pub(super) fn is_current_map_transfer_source(world: &World, point: &Point) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let current_map = normalize_map_file_name(&map.current_map.file_name);
    let config = &world.resource::<RuntimeConfigResource>().config;

    config
        .map_transfers
        .iter()
        .filter(|transfer| normalize_map_file_name(&transfer.from_map_file_name) == current_map)
        .any(|transfer| point_in_bounds(&transfer.from_bounds, point))
        || crystal_movement_transfer_records_for_map(&map.current_map.file_name)
            .iter()
            .any(|transfer| point_in_bounds(&transfer.from_bounds, point))
}

pub(super) fn apply_current_player_position_map_transfer(world: &mut World) -> Vec<ServerPacket> {
    let Some(transfer) = transfer_for_current_player_position(world) else {
        return Vec::new();
    };

    apply_map_transfer(world, &transfer.key)
}

pub(super) fn crystal_movement_transfer_records_for_map(
    map_file_name: &str,
) -> Vec<MapTransferRecord> {
    let Some(map) = crystal_map_respawns_by_file_name(map_file_name) else {
        return Vec::new();
    };

    map.movements
        .iter()
        .filter_map(|movement| {
            if movement.need_hole || movement.need_move || movement.conquest_index > 0 {
                return None;
            }
            let target = crystal_map_respawns_by_index(movement.map_index)?;
            if !crystal_manifest_movement_destination_is_valid(
                &target.map_file_name,
                &movement.destination,
            ) {
                return None;
            }
            Some(MapTransferRecord {
                key: crystal_movement_transfer_key(
                    &map.map_file_name,
                    movement.source.x,
                    movement.source.y,
                    movement.map_index,
                    movement.destination.x,
                    movement.destination.y,
                ),
                from_map_file_name: map.map_file_name.clone(),
                from_bounds: MapBounds {
                    min_x: movement.source.x,
                    max_x: movement.source.x,
                    min_y: movement.source.y,
                    max_y: movement.source.y,
                },
                to_map_file_name: target.map_file_name,
                to_map_title: target.map_title,
                to_position: movement.destination.clone(),
                to_direction: MirDirection::Down,
            })
        })
        .collect()
}

pub(crate) fn crystal_direct_movement_transfer_source_cells(
    map_file_name: &str,
) -> BTreeSet<(i32, i32)> {
    crystal_movement_transfer_records_for_map(map_file_name)
        .into_iter()
        .flat_map(|transfer| {
            (transfer.from_bounds.min_y..=transfer.from_bounds.max_y).flat_map(move |y| {
                (transfer.from_bounds.min_x..=transfer.from_bounds.max_x).map(move |x| (x, y))
            })
        })
        .collect()
}

fn crystal_manifest_movement_destination_is_valid(
    map_file_name: &str,
    destination: &Point,
) -> bool {
    runtime_full_map_collision_data(map_file_name)
        .map(|collision| full_map_collision_walkable(&collision, destination))
        .unwrap_or(true)
}

pub(super) fn crystal_movement_transfer_key(
    from_map_file_name: &str,
    source_x: i32,
    source_y: i32,
    target_map_index: i32,
    destination_x: i32,
    destination_y: i32,
) -> String {
    format!(
        "crystal-move:{}:{source_x}:{source_y}:{target_map_index}:{destination_x}:{destination_y}",
        normalize_map_file_name(from_map_file_name)
    )
}

pub(super) fn active_scene_view(world: &World) -> Option<SceneView> {
    let config = &world.resource::<RuntimeConfigResource>().config;
    let Some(player) = player_entity(world) else {
        return Some(config.scene_view.clone());
    };
    let position = entity_position(world, player)?;
    Some(SceneView {
        center: position,
        width: config.scene_view.width,
        height: config.scene_view.height,
    })
}

pub(super) fn point_visible(scene_view: Option<&SceneView>, point: &Point) -> bool {
    let Some(scene_view) = scene_view else {
        return true;
    };
    let half_width = i32::from(scene_view.width / 2);
    let half_height = i32::from(scene_view.height / 2);
    point.x >= scene_view.center.x - half_width
        && point.x <= scene_view.center.x + half_width
        && point.y >= scene_view.center.y - half_height
        && point.y <= scene_view.center.y + half_height
}

pub(super) fn rect_visible(
    scene_view: Option<&SceneView>,
    x: i32,
    y: i32,
    width: u16,
    height: u16,
) -> bool {
    let Some(scene_view) = scene_view else {
        return true;
    };
    let half_width = i32::from(scene_view.width / 2);
    let half_height = i32::from(scene_view.height / 2);
    let view_min_x = scene_view.center.x - half_width;
    let view_max_x = scene_view.center.x + half_width;
    let view_min_y = scene_view.center.y - half_height;
    let view_max_y = scene_view.center.y + half_height;
    let rect_max_x = x + i32::from(width) - 1;
    let rect_max_y = y + i32::from(height) - 1;

    x <= view_max_x && rect_max_x >= view_min_x && y <= view_max_y && rect_max_y >= view_min_y
}

pub(super) fn filter_terrain_patches(
    world: &World,
    scene_view: Option<&SceneView>,
) -> Vec<TerrainPatchTemplate> {
    world
        .resource::<RuntimeConfigResource>()
        .config
        .terrain_patches
        .iter()
        .filter(|patch| rect_visible(scene_view, patch.x, patch.y, patch.width, patch.height))
        .cloned()
        .collect()
}

pub(super) fn filter_decor_objects(
    world: &World,
    scene_view: Option<&SceneView>,
) -> Vec<DecorObjectTemplate> {
    world
        .resource::<RuntimeConfigResource>()
        .config
        .decor_objects
        .iter()
        .filter(|decor| {
            point_visible(
                scene_view,
                &Point {
                    x: decor.x,
                    y: decor.y,
                },
            )
        })
        .cloned()
        .collect()
}

pub(super) fn apply_map_transfer(world: &mut World, key: &str) -> Vec<ServerPacket> {
    if let Some((map_file_name, position)) = parse_debug_crystal_transfer_key(key) {
        let map_info = super::npc_script::crystal_npc_move_map_information(world, &map_file_name);
        return relocate_player_to_map(world, map_info, position, MirDirection::Down, None);
    }

    let Some(transfer) = transfer_for_key(
        &world.resource::<RuntimeConfigResource>().config,
        world.resource::<MapRuntimeResource>(),
        key,
    ) else {
        let language = super::session::current_language(world);
        return vec![super::session::system_message(&localized_text_or_fallback(
            language,
            "server.NotFound",
            "server.NotFound",
        ))];
    };

    let Some(player) = player_entity(world) else {
        let language = super::session::current_language(world);
        return vec![super::session::system_message(&localized_text_or_fallback(
            language,
            "server.NotFound",
            "server.NotFound",
        ))];
    };
    let current_position = entity_position(world, player).expect("player position");
    let current_map_file_name = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    if normalize_map_file_name(&current_map_file_name)
        != normalize_map_file_name(&transfer.from_map_file_name)
        || !point_in_bounds(&transfer.from_bounds, &current_position)
    {
        let language = super::session::current_language(world);
        return vec![super::session::system_message(&localized_text_or_fallback(
            language,
            "server.CannotPositionMoveOnMap",
            "server.CannotPositionMoveOnMap",
        ))];
    }

    let current_map = world.resource::<MapRuntimeResource>().current_map.clone();
    relocate_player_to_map(
        world,
        MapInformation {
            file_name: transfer.to_map_file_name.clone(),
            title: transfer.to_map_title.clone(),
            ..current_map
        },
        transfer.to_position,
        transfer.to_direction,
        None,
    )
}

pub(super) fn relocate_player_to_map(
    world: &mut World,
    map_info: MapInformation,
    position: Point,
    direction: MirDirection,
    system_message_text: Option<String>,
) -> Vec<ServerPacket> {
    let Some(player) = player_entity(world) else {
        let language = super::session::current_language(world);
        return vec![super::session::system_message(&localized_text_or_fallback(
            language,
            "server.NotFound",
            "server.NotFound",
        ))];
    };

    {
        world.resource_mut::<MapRuntimeResource>().current_map = map_info;
        let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
        player_runtime.player_position = position.clone();
        player_runtime.player_direction = direction;
    }
    world.resource_mut::<NpcStateResource>().active_npc_dialog = None;
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_combat_actions = Vec::new();
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_monster_spawns = Vec::new();
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_ground_spell_actions = Vec::new();
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_movement_command = None;
    reset_crystal_player_movement_timing(world);
    world
        .entity_mut(player)
        .insert((Position(position), Facing(direction)));

    refresh_runtime_map_collision(world);
    clear_non_player_world_entities(world);
    spawn_visible_world_for_current_map(world);

    let language = super::session::current_language(world);
    let map_info = {
        let map = world.resource::<MapRuntimeResource>();
        let mut info = map.current_map.clone();
        info.title = super::session::localized_map_title(language, &info.title);
        info
    };

    let mut packets = vec![
        ServerPacket::MapInformation { info: map_info },
        ServerPacket::UserLocation {
            location: current_location(world),
        },
    ];
    if let Some(message) = system_message_text {
        packets.push(ServerPacket::Chat {
            message,
            chat_type: ChatType::System,
        });
    }
    despawn_stage5_hero_for_no_hero_map(world, &mut packets);
    packets
}

pub(super) fn parse_debug_crystal_transfer_key(key: &str) -> Option<(String, Point)> {
    let mut parts = key.split(':');
    if parts.next()? != "crystal" {
        return None;
    }
    let map_file_name = parts.next()?.trim().trim_end_matches(".map").to_string();
    let x = parts.next()?.parse::<i32>().ok()?;
    let y = parts.next()?.parse::<i32>().ok()?;
    Some((map_file_name, Point { x, y }))
}

pub(super) fn clear_non_player_world_entities(world: &mut World) {
    let mut query = world.query_filtered::<Entity, (With<WorldObject>, Without<SelfPlayer>)>();
    let entities: Vec<Entity> = query.iter(world).collect();

    for entity in entities {
        let _ = world.despawn(entity);
    }
}

pub(super) fn should_use_crystal_current_map_world(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    if config.monster_spawn_source == MonsterSpawnSource::CrystalStarterRegion
        && crystal_map_respawns_by_file_name(&map.current_map.file_name).is_some()
    {
        return true;
    }
    map.current_map.title != config.map.title
        || normalize_map_file_name(&map.current_map.file_name)
            != normalize_map_file_name(&config.map.file_name)
}

pub(super) fn spawn_visible_world_for_current_map(world: &mut World) {
    let mut spawn_table = {
        let map = world.resource::<MapRuntimeResource>();
        let player_runtime = world.resource::<PlayerRuntimeResource>();
        let config = &world.resource::<RuntimeConfigResource>().config;
        build_crystal_current_map_visible_spawn_table(
            config,
            &map.current_map.file_name,
            &player_runtime.player_position,
        )
    };

    for (rule_index, rule) in spawn_table.rules.iter_mut().enumerate() {
        for (slot_index, slot) in rule.slots.iter_mut().enumerate() {
            let entity = world
                .spawn((
                    WorldObject,
                    Monster,
                    ObjectId(slot.object_id),
                    DisplayName::literal(rule.name.clone()),
                    Position(slot.spawn_position.clone()),
                    Facing(rule.direction),
                    SpawnSlotRef {
                        rule_index,
                        slot_index,
                    },
                    MonsterAgent {
                        image: rule.image,
                        dead: false,
                        patrol_origin: slot.spawn_position.clone(),
                        ai: rule.ai,
                        disposition: rule.disposition,
                        hostile_to_player: rule.hostile_to_player,
                        tracking_player: false,
                        view_range: rule.view_range,
                        can_wander: rule.can_wander,
                        move_interval_ticks: rule.move_interval_ticks,
                        attack_interval_ticks: rule.attack_interval_ticks,
                        next_move_tick: 0,
                        next_attack_tick: 0,
                        route: rule.route.clone(),
                        route_index: 0,
                        route_waiting: false,
                        next_route_tick: 0,
                    },
                    initial_monster_ai_state_for_object(rule.ai, 0, slot.object_id),
                    MonsterVitals {
                        hp: rule.max_hp,
                        max_hp: rule.max_hp,
                    },
                    MonsterCombatStats {
                        agility: rule.agility,
                    },
                ))
                .id();
            if rule.ai == 36 {
                world.entity_mut(entity).insert(initial_yimoogi_state(0));
            }
            slot.entity = Some(entity);
        }
    }

    spawn_crystal_current_map_npcs(world);
    spawn_stage5_hero(world);
    world.insert_resource(spawn_table);
}

pub(super) fn spawn_stage5_hero(world: &mut World) -> Option<Entity> {
    let existing: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<Hero>>();
        query.iter(world).collect()
    };
    for entity in existing {
        let _ = world.despawn(entity);
    }

    let hero_allowed = !current_map_disallows_hero(world);
    let hero = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
        .filter(|hero| hero.spawned && hero_allowed)
        .cloned()?;
    let owner_name = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())?;
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let player = player_entity(world)?;
    let player_position = entity_position(world, player)?;
    let player_direction = world
        .entity(player)
        .get::<Facing>()
        .map(|facing| facing.0)?;
    let spawn_position =
        summon_spawn_position_near(world, &player_position, player_direction, 1, Some(player));
    let max_hp = 60 + i32::from(hero.level).saturating_mul(6);
    let max_mp = 30 + i32::from(hero.level).saturating_mul(4);

    Some(
        world
            .spawn((
                WorldObject,
                Hero {
                    owner_name: owner_name.clone(),
                    next_attack_tick: 0,
                    next_move_tick: 0,
                },
                ObjectId(config.object_id.saturating_add(1)),
                DisplayName::literal(hero.name),
                Position(spawn_position),
                Facing(player_direction),
                CharacterBody {
                    class: hero.class,
                    gender: hero.gender,
                    level: hero.level,
                    armour_shape: None,
                    weapon_shape: None,
                },
                PlayerVitals {
                    hp: max_hp,
                    max_hp,
                    mp: max_mp,
                },
            ))
            .id(),
    )
}

fn despawn_stage5_hero_for_no_hero_map(world: &mut World, packets: &mut Vec<ServerPacket>) {
    if !current_map_disallows_hero(world) {
        return;
    }

    let was_spawned = world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_mut()
        .map(|hero| {
            let was_spawned = hero.spawned;
            hero.spawned = false;
            was_spawned
        })
        .unwrap_or(false);
    if !was_spawned {
        return;
    }

    packets.push(ServerPacket::UpdateHeroSpawnState { state: 1 });
    let language = super::session::current_language(world);
    packets.push(ServerPacket::Chat {
        message: localized_text_or_fallback(
            language,
            "server.HeroesNotAllowedOnMap",
            "Heroes are not allowed on this map. Your Hero has been unsummoned.",
        ),
        chat_type: ChatType::System,
    });
}

pub(super) fn spawn_crystal_current_map_npcs(world: &mut World) {
    let (map_file_name, character) = {
        let map = world.resource::<MapRuntimeResource>();
        let Some(character) = world
            .resource::<SessionResource>()
            .selected_character
            .clone()
        else {
            return;
        };
        (
            normalize_map_file_name(&map.current_map.file_name),
            character,
        )
    };
    let quest_ids_by_npc = super::npc::crystal_quest_ids_by_npc();

    for npc in crystal_npc_info_manifest().npcs {
        let npc_map_file_name = npc.map_file_name.as_deref().map(normalize_map_file_name);
        if npc_map_file_name.as_deref() != Some(map_file_name.as_str()) {
            continue;
        }
        if !super::npc::crystal_npc_visible_to_character(&npc, &character) {
            continue;
        }
        let Some(object_id) = npc.loaded_object_id else {
            continue;
        };
        let quest_ids = quest_ids_by_npc
            .get(&object_id)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default();

        world.spawn((
            WorldObject,
            Npc,
            ObjectId(object_id),
            DisplayName::literal(npc.name),
            Position(npc.location),
            Facing(MirDirection::Up),
            NpcAgent {
                image: npc.image,
                colour_argb: 0,
                quest_ids,
                script_key: Some(npc.script_key),
            },
        ));
    }
}

pub(super) fn runtime_map_collision_data(map_file_name: &str) -> Option<RuntimeMapCollisionData> {
    let normalized = normalize_map_file_name(map_file_name);
    static RUNTIME_MAP_COLLISION_CACHE: OnceLock<
        Mutex<BTreeMap<String, Option<RuntimeMapCollisionData>>>,
    > = OnceLock::new();
    let cache = RUNTIME_MAP_COLLISION_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(cached) = cache
        .lock()
        .expect("runtime map collision cache should not be poisoned")
        .get(&normalized)
        .cloned()
    {
        return cached;
    }

    let parsed = runtime_map_collision_data_uncached(&normalized);
    cache
        .lock()
        .expect("runtime map collision cache should not be poisoned")
        .insert(normalized, parsed.clone());
    parsed
}

pub(super) fn runtime_map_collision_data_uncached(
    normalized: &str,
) -> Option<RuntimeMapCollisionData> {
    if normalized == normalize_map_file_name("0") {
        return Some(runtime_map_collision_from_template(starter_map_collision()));
    }

    let map_path = crystal_map_path(normalized)?;
    let bytes = fs::read(map_path).ok()?;
    let collision = parse_runtime_map_collision(normalized, &bytes)?;
    Some(runtime_map_collision_from_template(collision))
}

pub(super) fn runtime_map_collision_from_template(
    collision: StarterMapCollision,
) -> RuntimeMapCollisionData {
    let blocked_set = collision
        .blocked_cells
        .iter()
        .map(|cell| (cell.x, cell.y))
        .collect();
    let closed_door_set = collision
        .doors
        .iter()
        .filter(|door| door.closed)
        .map(|door| (door.x, door.y))
        .collect();

    RuntimeMapCollisionData {
        collision,
        blocked_set,
        closed_door_set,
    }
}

pub(super) fn crystal_map_path(map_file_name: &str) -> Option<PathBuf> {
    let normalized = normalize_map_file_name(map_file_name);
    crystal_client_root_candidates()
        .into_iter()
        .map(|client_root| client_root.join("Map").join(format!("{normalized}.map")))
        .find(|candidate| candidate.exists())
}

fn crystal_client_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(client_root) = std::env::var("CRYSTAL_CLIENT_ROOT") {
        candidates.push(PathBuf::from(client_root));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("downloads")
            .join("crystal-client-full"),
    );
    candidates.push(PathBuf::from(DEFAULT_CRYSTAL_CLIENT_ROOT));
    candidates
}

pub(super) fn parse_runtime_map_collision(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    let map_type = detect_runtime_map_type(bytes)?;
    match map_type {
        0 => parse_runtime_map_collision_v0(map_file_name, bytes),
        1 => parse_runtime_map_collision_v1(map_file_name, bytes),
        2 => parse_runtime_map_collision_v2(map_file_name, bytes),
        3 => parse_runtime_map_collision_v3(map_file_name, bytes),
        4 => parse_runtime_map_collision_v4(map_file_name, bytes),
        5 => parse_runtime_map_collision_v5(map_file_name, bytes),
        6 => parse_runtime_map_collision_v6(map_file_name, bytes),
        7 => parse_runtime_map_collision_v7(map_file_name, bytes),
        100 => parse_runtime_map_collision_v100(map_file_name, bytes),
        _ => None,
    }
}

pub(super) fn detect_runtime_map_type(bytes: &[u8]) -> Option<u8> {
    if bytes.len() < 8 {
        return None;
    }
    if bytes.get(2) == Some(&0x43) && bytes.get(3) == Some(&0x23) {
        return Some(100);
    }
    if bytes.first() == Some(&0) {
        return Some(5);
    }
    if bytes.first() == Some(&0x0F) && bytes.get(5) == Some(&0x53) && bytes.get(14) == Some(&0x33) {
        return Some(6);
    }
    if bytes.first() == Some(&0x15)
        && bytes.get(4) == Some(&0x32)
        && bytes.get(6) == Some(&0x41)
        && bytes.get(19) == Some(&0x31)
    {
        return Some(4);
    }
    if bytes.first() == Some(&0x10)
        && bytes.get(2) == Some(&0x61)
        && bytes.get(7) == Some(&0x31)
        && bytes.get(14) == Some(&0x31)
    {
        return Some(1);
    }
    if (bytes.get(4) == Some(&0x0F) || bytes.get(4) == Some(&0x03))
        && bytes.get(18) == Some(&0x0D)
        && bytes.get(19) == Some(&0x0A)
    {
        let width = i32::from(read_i16_le(bytes, 0)?);
        let height = i32::from(read_i16_le(bytes, 2)?);
        let expected_v2 = 52 + width * height * 14;
        return Some(if i32::try_from(bytes.len()).ok()? > expected_v2 {
            3
        } else {
            2
        });
    }
    if bytes.first() == Some(&0x0D)
        && bytes.get(1) == Some(&0x4C)
        && bytes.get(7) == Some(&0x20)
        && bytes.get(11) == Some(&0x6D)
    {
        return Some(7);
    }
    Some(0)
}

pub(super) fn parse_runtime_map_collision_v100(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    if bytes.first() != Some(&1) || bytes.get(1) != Some(&0) {
        return None;
    }
    let width = u16::try_from(read_i16_le(bytes, 4)?).ok()?;
    let height = u16::try_from(read_i16_le(bytes, 6)?).ok()?;
    let mut offset = 8usize;
    let mut blocked_cells = Vec::new();
    let mut doors = Vec::new();

    for x in 0..i32::from(width) {
        for y in 0..i32::from(height) {
            offset += 2;
            let high_wall = (read_i32_le(bytes, offset)? & 0x2000_0000) != 0;
            offset += 10;
            let low_wall = (read_i16_le(bytes, offset)? & i16::MIN) != 0;
            offset += 2;
            let door = *bytes.get(offset)?;
            if high_wall || low_wall {
                blocked_cells.push(BlockedMapCellTemplate {
                    x,
                    y,
                    attribute: if low_wall {
                        MapCellAttribute::LowWall
                    } else {
                        MapCellAttribute::HighWall
                    },
                });
            }
            if door > 0 {
                doors.push(DoorMapCellTemplate {
                    x,
                    y,
                    index: door & 0x7F,
                    closed: true,
                });
            }
            offset += 12;
        }
    }

    Some(build_runtime_collision_output(
        map_file_name,
        width,
        height,
        blocked_cells,
        doors,
    ))
}

pub(super) fn parse_runtime_map_collision_v0(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    parse_runtime_map_collision_fixed(map_file_name, bytes, 52, 12, 0)
}

pub(super) fn parse_runtime_map_collision_v2(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    parse_runtime_map_collision_fixed(map_file_name, bytes, 52, 14, 2)
}

pub(super) fn parse_runtime_map_collision_v3(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    parse_runtime_map_collision_fixed(map_file_name, bytes, 52, 36, 3)
}

pub(super) fn parse_runtime_map_collision_v4(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    let xor = read_i16_le(bytes, 33)?;
    let width = u16::try_from(read_i16_le(bytes, 31)? ^ xor).ok()?;
    let height = u16::try_from(read_i16_le(bytes, 35)? ^ xor).ok()?;
    let mut offset = 64usize;
    let mut blocked_cells = Vec::new();
    let mut doors = Vec::new();

    for x in 0..i32::from(width) {
        for y in 0..i32::from(height) {
            let high_wall = (read_i16_le(bytes, offset)? & i16::MIN) != 0;
            offset += 2;
            let low_wall = (read_i16_le(bytes, offset)? & i16::MIN) != 0;
            offset += 4;
            let door = *bytes.get(offset)?;
            if high_wall || low_wall {
                blocked_cells.push(BlockedMapCellTemplate {
                    x,
                    y,
                    attribute: if low_wall {
                        MapCellAttribute::LowWall
                    } else {
                        MapCellAttribute::HighWall
                    },
                });
            }
            if door > 0 {
                doors.push(DoorMapCellTemplate {
                    x,
                    y,
                    index: door & 0x7F,
                    closed: true,
                });
            }
            offset += 7;
        }
    }

    Some(build_runtime_collision_output(
        map_file_name,
        width,
        height,
        blocked_cells,
        doors,
    ))
}

pub(super) fn parse_runtime_map_collision_v5(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    let width = u16::try_from(read_i16_le(bytes, 22)?).ok()?;
    let height = u16::try_from(read_i16_le(bytes, 24)?).ok()?;
    let mut offset =
        28usize + (3usize * usize::from((width / 2) + (width % 2)) * usize::from(height / 2));
    let mut blocked_cells = Vec::new();

    for x in 0..i32::from(width) {
        for y in 0..i32::from(height) {
            let flags = *bytes.get(offset)?;
            let high_wall = (flags & 0x01) != 1;
            let low_wall = !high_wall && (flags & 0x02) != 2;
            if high_wall || low_wall {
                blocked_cells.push(BlockedMapCellTemplate {
                    x,
                    y,
                    attribute: if low_wall {
                        MapCellAttribute::LowWall
                    } else {
                        MapCellAttribute::HighWall
                    },
                });
            }
            offset += 14;
        }
    }

    Some(build_runtime_collision_output(
        map_file_name,
        width,
        height,
        blocked_cells,
        Vec::new(),
    ))
}

pub(super) fn parse_runtime_map_collision_v6(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    let width = u16::try_from(read_i16_le(bytes, 16)?).ok()?;
    let height = u16::try_from(read_i16_le(bytes, 18)?).ok()?;
    let mut offset = 40usize;
    let mut blocked_cells = Vec::new();

    for x in 0..i32::from(width) {
        for y in 0..i32::from(height) {
            let flags = *bytes.get(offset)?;
            let high_wall = (flags & 0x01) != 1;
            let low_wall = !high_wall && (flags & 0x02) != 2;
            if high_wall || low_wall {
                blocked_cells.push(BlockedMapCellTemplate {
                    x,
                    y,
                    attribute: if low_wall {
                        MapCellAttribute::LowWall
                    } else {
                        MapCellAttribute::HighWall
                    },
                });
            }
            offset += 20;
        }
    }

    Some(build_runtime_collision_output(
        map_file_name,
        width,
        height,
        blocked_cells,
        Vec::new(),
    ))
}

pub(super) fn parse_runtime_map_collision_v7(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    let width = u16::try_from(read_i16_le(bytes, 21)?).ok()?;
    let height = u16::try_from(read_i16_le(bytes, 25)?).ok()?;
    let mut offset = 54usize;
    let mut blocked_cells = Vec::new();
    let mut doors = Vec::new();

    for x in 0..i32::from(width) {
        for y in 0..i32::from(height) {
            let high_wall = (read_i16_le(bytes, offset)? & i16::MIN) != 0;
            offset += 6;
            let low_wall = (read_i16_le(bytes, offset)? & i16::MIN) != 0;
            offset += 2;
            let door = *bytes.get(offset)?;
            if high_wall || low_wall {
                blocked_cells.push(BlockedMapCellTemplate {
                    x,
                    y,
                    attribute: if low_wall {
                        MapCellAttribute::LowWall
                    } else {
                        MapCellAttribute::HighWall
                    },
                });
            }
            if door > 0 {
                doors.push(DoorMapCellTemplate {
                    x,
                    y,
                    index: door & 0x7F,
                    closed: true,
                });
            }
            offset += 7;
        }
    }

    Some(build_runtime_collision_output(
        map_file_name,
        width,
        height,
        blocked_cells,
        doors,
    ))
}

pub(super) fn parse_runtime_map_collision_v1(
    map_file_name: &str,
    bytes: &[u8],
) -> Option<StarterMapCollision> {
    let xor = read_i16_le(bytes, 23)?;
    let width = u16::try_from(read_i16_le(bytes, 21)? ^ xor).ok()?;
    let height = u16::try_from(read_i16_le(bytes, 25)? ^ xor).ok()?;
    let mut offset = 54usize;
    let mut blocked_cells = Vec::new();
    let mut doors = Vec::new();

    for x in 0..i32::from(width) {
        for y in 0..i32::from(height) {
            let high_wall =
                ((read_i32_le(bytes, offset)? ^ 0xAA38_AA38_u32 as i32) & 0x2000_0000) != 0;
            offset += 6;
            let low_wall = ((read_i16_le(bytes, offset)? ^ xor) & i16::MIN) != 0;
            offset += 2;
            let door = *bytes.get(offset)?;
            if high_wall || low_wall {
                blocked_cells.push(BlockedMapCellTemplate {
                    x,
                    y,
                    attribute: if low_wall {
                        MapCellAttribute::LowWall
                    } else {
                        MapCellAttribute::HighWall
                    },
                });
            }
            if door > 0 {
                doors.push(DoorMapCellTemplate {
                    x,
                    y,
                    index: door & 0x7F,
                    closed: true,
                });
            }
            offset += 7;
        }
    }

    Some(build_runtime_collision_output(
        map_file_name,
        width,
        height,
        blocked_cells,
        doors,
    ))
}

pub(super) fn parse_runtime_map_collision_fixed(
    map_file_name: &str,
    bytes: &[u8],
    start_offset: usize,
    stride: usize,
    _map_type: u8,
) -> Option<StarterMapCollision> {
    let width = u16::try_from(read_i16_le(bytes, 0)?).ok()?;
    let height = u16::try_from(read_i16_le(bytes, 2)?).ok()?;
    let mut offset = start_offset;
    let mut blocked_cells = Vec::new();
    let mut doors = Vec::new();

    for x in 0..i32::from(width) {
        for y in 0..i32::from(height) {
            let high_wall = (read_i16_le(bytes, offset)? & i16::MIN) != 0;
            let low_wall = (read_i16_le(bytes, offset + 2)? & i16::MIN) != 0;
            let no_floor = (read_i16_le(bytes, offset + 4)? & i16::MIN) != 0;
            let door_offset = match stride {
                12 => offset + 8,
                14 => offset + 6,
                36 => offset + 6,
                _ => offset + 6,
            };
            let door = *bytes.get(door_offset)?;
            if high_wall || low_wall || no_floor {
                blocked_cells.push(BlockedMapCellTemplate {
                    x,
                    y,
                    attribute: if low_wall {
                        MapCellAttribute::LowWall
                    } else {
                        MapCellAttribute::HighWall
                    },
                });
            }
            if door > 0 {
                doors.push(DoorMapCellTemplate {
                    x,
                    y,
                    index: door & 0x7F,
                    closed: true,
                });
            }
            offset += stride;
        }
    }

    Some(build_runtime_collision_output(
        map_file_name,
        width,
        height,
        blocked_cells,
        doors,
    ))
}

pub(super) fn build_runtime_collision_output(
    map_file_name: &str,
    map_width: u16,
    map_height: u16,
    blocked_cells: Vec<BlockedMapCellTemplate>,
    doors: Vec<DoorMapCellTemplate>,
) -> StarterMapCollision {
    StarterMapCollision {
        map_file_name: format!("{}.map", normalize_map_file_name(map_file_name)),
        map_width,
        map_height,
        region_bounds: MapBounds {
            min_x: 0,
            max_x: i32::from(map_width.saturating_sub(1)),
            min_y: 0,
            max_y: i32::from(map_height.saturating_sub(1)),
        },
        play_bounds: MapBounds {
            min_x: 0,
            max_x: i32::from(map_width.saturating_sub(1)),
            min_y: 0,
            max_y: i32::from(map_height.saturating_sub(1)),
        },
        blocked_cells,
        doors,
    }
}

pub(super) fn read_i16_le(bytes: &[u8], offset: usize) -> Option<i16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(i16::from_le_bytes([slice[0], slice[1]]))
}

pub(super) fn read_i32_le(bytes: &[u8], offset: usize) -> Option<i32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

pub(super) fn runtime_full_map_collision_data(
    map_file_name: &str,
) -> Option<RuntimeMapCollisionData> {
    let normalized = normalize_map_file_name(map_file_name);
    static RUNTIME_FULL_MAP_COLLISION_CACHE: OnceLock<
        Mutex<BTreeMap<String, Option<RuntimeMapCollisionData>>>,
    > = OnceLock::new();
    let cache = RUNTIME_FULL_MAP_COLLISION_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(cached) = cache
        .lock()
        .expect("runtime full map collision cache should not be poisoned")
        .get(&normalized)
        .cloned()
    {
        return cached;
    }

    let parsed = crystal_map_path(&normalized)
        .and_then(|map_path| fs::read(map_path).ok())
        .and_then(|bytes| parse_runtime_map_collision(&normalized, &bytes))
        .map(runtime_map_collision_from_template);
    cache
        .lock()
        .expect("runtime full map collision cache should not be poisoned")
        .insert(normalized, parsed.clone());
    parsed
}

pub(super) fn refresh_runtime_map_collision(world: &mut World) {
    let current_map = world.resource::<MapRuntimeResource>().current_map.clone();
    let collision = runtime_active_map_collision_data(&current_map).unwrap_or_else(|| {
        let fallback = world
            .resource::<RuntimeConfigResource>()
            .config
            .map_collision
            .clone();
        runtime_map_collision_from_template(fallback)
    });

    let doors = super::resources::DoorRegistry::from_templates(&collision.collision.doors);
    {
        let mut map = world.resource_mut::<MapRuntimeResource>();
        map.map_region_bounds = collision.collision.region_bounds;
        map.blocked_cells = collision.blocked_set;
        map.closed_door_cells = collision.closed_door_set;
        map.doors = doors;
    }
    super::mining::rebuild_mine_spots(world);
}

pub(super) fn runtime_active_map_collision_data(
    map: &MapInformation,
) -> Option<RuntimeMapCollisionData> {
    if normalize_map_file_name(&map.file_name) == normalize_map_file_name("0")
        && map.title != "Starter Field"
    {
        return runtime_full_map_collision_data(&map.file_name)
            .or_else(|| runtime_map_collision_data(&map.file_name));
    }
    runtime_map_collision_data(&map.file_name)
}

pub(super) fn collision_data_for_map_or_config(
    config: &SimulationConfig,
    map_file_name: &str,
) -> RuntimeMapCollisionData {
    runtime_map_collision_data(map_file_name)
        .unwrap_or_else(|| runtime_map_collision_from_template(config.map_collision.clone()))
}

pub(super) fn is_static_spawnable_point_with_collision(
    config: &SimulationConfig,
    map_file_name: &str,
    collision: &RuntimeMapCollisionData,
    point: &Point,
) -> bool {
    if !point_in_bounds(&collision.collision.region_bounds, point) {
        return false;
    }

    let map_blocked = collision.blocked_set.contains(&tile_key(point));
    let door_blocked = collision.closed_door_set.contains(&tile_key(point));
    let use_config_overlays =
        normalize_map_file_name(map_file_name) == normalize_map_file_name(&config.map.file_name);

    let terrain_blocked = use_config_overlays
        && config.terrain_patches.iter().any(|patch| {
            let within = point.x >= patch.x
                && point.x < patch.x + i32::from(patch.width)
                && point.y >= patch.y
                && point.y < patch.y + i32::from(patch.height);

            within && matches!(patch.kind, TerrainKind::Water)
        });

    let decor_blocked = use_config_overlays
        && config.decor_objects.iter().any(|decor| {
            decor.x == point.x
                && decor.y == point.y
                && matches!(
                    decor.kind,
                    DecorKind::Tree
                        | DecorKind::Rock
                        | DecorKind::Banner
                        | DecorKind::Campfire
                        | DecorKind::Stump
                )
        });

    !(map_blocked || door_blocked || terrain_blocked || decor_blocked)
}

pub(super) fn walkable_points_in_rect(
    collision: &RuntimeMapCollisionData,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
) -> Vec<Point> {
    let mut points = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let point = Point { x, y };
            if full_map_collision_walkable(collision, &point) {
                points.push(point);
            }
        }
    }
    points
}

pub(super) fn walkable_point_count_in_rect(
    collision: &RuntimeMapCollisionData,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
) -> usize {
    let mut count = 0;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if full_map_collision_walkable(collision, &Point { x, y }) {
                count += 1;
            }
        }
    }
    count
}

pub(super) fn full_map_collision_walkable(
    collision: &RuntimeMapCollisionData,
    point: &Point,
) -> bool {
    super::movement::point_in_bounds(&collision.collision.region_bounds, point)
        && !collision
            .blocked_set
            .contains(&super::movement::tile_key(point))
        && !collision
            .closed_door_set
            .contains(&super::movement::tile_key(point))
}

pub(super) fn rebuild_world(world: &mut World) {
    clear_world_entities(world);

    let player_runtime = world.resource::<PlayerRuntimeResource>().clone();
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let mut spawn_table = build_spawn_table(&config);
    let selected_character = world
        .resource::<SessionResource>()
        .selected_character
        .clone();

    if let Some(character) = selected_character {
        let runtime_state =
            active_character_runtime_state(world).unwrap_or_else(|| ActiveCharacterRuntimeState {
                position: player_runtime.player_position.clone(),
                direction: player_runtime.player_direction,
                vitals: player_runtime.player_vitals,
            });
        world.spawn((
            WorldObject,
            SelfPlayer,
            ObjectId(config.object_id),
            DisplayName::literal(character.name.clone()),
            Position(runtime_state.position),
            Facing(runtime_state.direction),
            CharacterBody {
                class: character.class,
                gender: character.gender,
                level: character.level,
                armour_shape: None,
                weapon_shape: None,
            },
            runtime_state.vitals,
        ));
    }

    for record in &config.visible_players {
        world.spawn((
            WorldObject,
            RemotePlayer,
            ObjectId(record.object_id),
            localized_visible_player_name_key(record.object_id)
                .map(|key| DisplayName::localized(key, record.name.clone()))
                .unwrap_or_else(|| DisplayName::literal(record.name.clone())),
            Position(record.position.clone()),
            Facing(record.direction),
            CharacterBody {
                class: record.class,
                gender: record.gender,
                level: record.level,
                armour_shape: record.armour_shape,
                weapon_shape: record.weapon_shape,
            },
        ));
    }

    for (rule_index, rule) in spawn_table.rules.iter_mut().enumerate() {
        for (slot_index, slot) in rule.slots.iter_mut().enumerate() {
            let entity = world
                .spawn((
                    WorldObject,
                    Monster,
                    ObjectId(slot.object_id),
                    localized_monster_name_key(slot.object_id)
                        .map(|key| DisplayName::localized(key, rule.name.clone()))
                        .unwrap_or_else(|| DisplayName::literal(rule.name.clone())),
                    Position(slot.spawn_position.clone()),
                    Facing(rule.direction),
                    SpawnSlotRef {
                        rule_index,
                        slot_index,
                    },
                    MonsterAgent {
                        image: rule.image,
                        dead: false,
                        patrol_origin: slot.spawn_position.clone(),
                        ai: rule.ai,
                        disposition: rule.disposition,
                        hostile_to_player: rule.hostile_to_player,
                        tracking_player: false,
                        view_range: rule.view_range,
                        can_wander: rule.can_wander,
                        move_interval_ticks: rule.move_interval_ticks,
                        attack_interval_ticks: rule.attack_interval_ticks,
                        next_move_tick: 0,
                        next_attack_tick: 0,
                        route: rule.route.clone(),
                        route_index: 0,
                        route_waiting: false,
                        next_route_tick: 0,
                    },
                    initial_monster_ai_state_for_object(rule.ai, 0, slot.object_id),
                    MonsterVitals {
                        hp: rule.max_hp,
                        max_hp: rule.max_hp,
                    },
                    MonsterCombatStats {
                        agility: rule.agility,
                    },
                ))
                .id();
            if rule.ai == 123 {
                world
                    .entity_mut(entity)
                    .insert(initial_general_meow_meow_state(0));
            }
            slot.entity = Some(entity);
        }
    }

    for record in &config.visible_npcs {
        world.spawn((
            WorldObject,
            Npc,
            ObjectId(record.object_id),
            localized_npc_name_key(record.object_id)
                .map(|key| DisplayName::localized(key, record.name.clone()))
                .unwrap_or_else(|| DisplayName::literal(record.name.clone())),
            Position(record.position.clone()),
            Facing(record.direction),
            NpcAgent {
                image: record.image,
                colour_argb: record.colour_argb,
                quest_ids: record.quest_ids.clone(),
                script_key: record.script_key.clone(),
            },
        ));
    }

    spawn_stage5_hero(world);
    world.insert_resource(spawn_table);
}

#[allow(deprecated)]
pub(super) fn clear_world_entities(world: &mut World) {
    let entities: Vec<Entity> = world
        .iter_entities()
        .filter_map(|entity| entity.contains::<WorldObject>().then_some(entity.id()))
        .collect();

    for entity in entities {
        let _ = world.despawn(entity);
    }
}

impl SimulationSession {
    pub fn transfer_map(&mut self, key: &str) -> Vec<ServerPacket> {
        let packets = self.transfer_map_impl(key);
        self.finalize_packets(packets)
    }

    pub(super) fn transfer_map_impl(&mut self, key: &str) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            let language = current_language(self.app.world());
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        }

        apply_map_transfer(self.app.world_mut(), key)
    }
}

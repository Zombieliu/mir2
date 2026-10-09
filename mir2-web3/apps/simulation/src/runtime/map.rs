use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{With, Without, World};
use mir2_game_data::{
    crystal_map_respawns_by_index, crystal_map_respawns_ref, crystal_npc_info_manifest,
    localized_text_or_fallback, starter_map_collision, DecorKind, DecorObjectTemplate, MapBounds,
    SceneView, StarterMapCollision, TerrainKind, TerrainPatchTemplate,
};
use mir2_protocol::{ChatType, MapInformation, MirDirection, Point, ServerPacket};

use super::components::{
    current_player_object_id, entity_position, player_entity, CharacterBody, DisplayName, Facing,
    Hero, Monster, MonsterAgent, MonsterCombatStats, MonsterVitals, Npc, NpcAgent, ObjectId,
    PlayerVitals, Position, RemotePlayer, SelfPlayer, SpawnSlotRef, WorldObject,
};
use super::crystal_compat::DEFAULT_CRYSTAL_CLIENT_ROOT;
use super::map_events::{authorized_map_coordinate_transfers, crystal_map_coordinate_source_cells};
use super::monsters::{
    build_crystal_current_map_full_spawn_table, build_crystal_current_map_visible_spawn_table,
    build_spawn_table, initial_general_meow_meow_state, initial_monster_ai_state_for_object,
    initial_yimoogi_state, MonsterSpawnRule, MonsterSpawnSlot, MonsterSpawnTable,
};
use super::movement::{current_location, point_in_bounds, summon_spawn_position_near, tile_key};
use super::packets::{
    localized_monster_name_key, localized_npc_name_key, localized_visible_player_name_key,
};
use super::resources::{
    current_language, is_in_world, reset_crystal_player_movement_timing, MapRuntimeResource,
    MountResource, NpcStateResource, PlayerRuntimeResource, RuntimeConfigResource,
    RuntimeQueueResource, SessionResource, Stage5SystemsResource,
};
use super::save::{active_character_runtime_state, ActiveCharacterRuntimeState};
use super::session::{system_message, SimulationSession};
use crate::config::{MapDropRuleRecord, MonsterSpawnSource, SimulationConfig};
use crate::MapTransferRecord;

#[derive(Debug, Clone)]
pub(super) struct RuntimeMapCollisionData {
    // Parsed terrain is immutable and reused by every personal world on this map.
    // Closed doors remain owned because their runtime state changes independently.
    pub(super) collision: Arc<StarterMapCollision>,
    pub(super) blocked_set: Arc<BTreeSet<(i32, i32)>>,
    pub(super) closed_door_set: BTreeSet<(i32, i32)>,
    pub(super) fishing_cells: Arc<BTreeMap<(i32, i32), i8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZoneMapCollisionData {
    pub bounds: MapBounds,
    pub blocked_cells: BTreeSet<(i32, i32)>,
    pub transfer_source_cells: BTreeSet<(i32, i32)>,
    /// Door index → its cells (deduped, `& 0x7F`). `blocked_cells` already
    /// includes these (doors start closed); the zone uses this to open/close
    /// shared doors dynamically.
    pub doors: BTreeMap<u8, Vec<(i32, i32)>>,
}

pub(super) fn normalize_map_file_name(file_name: &str) -> String {
    let normalized = file_name.trim().to_ascii_lowercase();
    normalized
        .strip_suffix(".map")
        .unwrap_or(&normalized)
        .to_owned()
}

/// When set, the shared multiplayer zone hosts every map at its *full* size
/// (the gz map-pack collision), so Bichon "0" is the whole 700×700 province
/// instead of the collision-bounded starter slice and players can roam all of
/// it and reach every transfer. Off by default — the starter demo/tests keep
/// the slice. Enabled once at gateway startup for the activated Crystal world
/// ([`set_crystal_full_world_zone_collision`]); it is process-wide because the
/// shared zone is a process-wide singleton created without per-session config.
static CRYSTAL_FULL_WORLD_ZONE_COLLISION: AtomicBool = AtomicBool::new(false);

/// Enable/disable full-map collision for the shared zone world. Called from the
/// gateway entry points when `MonsterSpawnSource::CrystalWorld` is active.
pub fn set_crystal_full_world_zone_collision(enabled: bool) {
    CRYSTAL_FULL_WORLD_ZONE_COLLISION.store(enabled, Ordering::Relaxed);
}

fn crystal_full_world_zone_collision_enabled() -> bool {
    CRYSTAL_FULL_WORLD_ZONE_COLLISION.load(Ordering::Relaxed)
}

pub(crate) fn zone_map_collision_data(map_file_name: &str) -> Option<ZoneMapCollisionData> {
    // The activated world hosts every map full-size (Bichon "0" included); other
    // modes keep the prior behaviour where "0" is the starter slice.
    let collision = if crystal_full_world_zone_collision_enabled() {
        runtime_world_map_collision_data(map_file_name)
            .map(|collision| (*collision).clone())
            .or_else(|| runtime_map_collision_data(map_file_name))?
    } else {
        runtime_map_collision_data(map_file_name)?
    };
    // The Zone merges doors into this set and mutates it when they open/close.
    // Keep its owned projection separate from the cached immutable terrain.
    let mut blocked_cells = collision.blocked_set.as_ref().clone();
    blocked_cells.extend(collision.closed_door_set);
    let mut transfer_source_cells = crystal_direct_movement_transfer_source_cells(map_file_name);
    transfer_source_cells.extend(crystal_map_coordinate_source_cells(map_file_name));
    let mut doors: BTreeMap<u8, Vec<(i32, i32)>> = BTreeMap::new();
    for door in &collision.collision.doors {
        let cell = (door.x, door.y);
        let entry = doors.entry(door.index & 0x7F).or_default();
        if !entry.contains(&cell) {
            entry.push(cell);
        }
    }
    Some(ZoneMapCollisionData {
        bounds: collision.collision.region_bounds,
        blocked_cells,
        transfer_source_cells,
        doors,
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

    crystal_map_respawns_ref(&map.current_map.file_name)
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

/// Crystal `HumanObject.SetBindSafeZone`: walking, running, or teleporting
/// into an imported safe area binds to that area's center, not the tile the
/// player happened to occupy. The Zone calls this through its authoritative
/// transform projection after an accepted step.
pub(super) fn refresh_player_bind_at_position(world: &mut World, position: &Point) {
    let _=super::buffs::refresh_player_safe_duration_buffs(world,position);
    let map_file_name = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    let Some(safe_zone) = crystal_map_respawns_ref(&map_file_name).and_then(|map| {
        map.safe_zones.iter().find(|safe_zone| {
            let size = i32::from(safe_zone.size);
            position.x >= safe_zone.location.x - size
                && position.x <= safe_zone.location.x + size
                && position.y >= safe_zone.location.y - size
                && position.y <= safe_zone.location.y + size
        })
    }) else {
        return;
    };
    let new_bind = crate::config::CharacterBindPoint {
        map_file_name,
        position: safe_zone.location.clone(),
    };
    let mut player = world.resource_mut::<PlayerRuntimeResource>();
    if player.bind_point.as_ref() != Some(&new_bind) {
        player.bind_point = Some(new_bind);
    }
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
    crystal_map_respawns_ref(&map.current_map.file_name)
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
    crystal_map_respawns_ref(&map.current_map.file_name)
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
    crystal_map_respawns_ref(&map.current_map.file_name)
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
    crystal_map_respawns_ref(&map.current_map.file_name)
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
    crystal_map_respawns_ref(&map.current_map.file_name)
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
        .chain(authorized_map_coordinate_transfers(world))
        .filter(|transfer| conquest_movement_allowed(world, transfer.conquest_index))
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
        .filter(|transfer| conquest_movement_allowed(world, transfer.conquest_index))
        .any(|transfer| point_in_bounds(&transfer.from_bounds, point))
        || crystal_movement_transfer_records_for_map(&map.current_map.file_name)
            .iter()
            .filter(|transfer| conquest_movement_allowed(world, transfer.conquest_index))
            .any(|transfer| point_in_bounds(&transfer.from_bounds, point))
        || crystal_map_coordinate_source_cells(&map.current_map.file_name)
            .any(|(x, y)| point.x == x && point.y == y)
        || super::default_npc_events::is_active_map_coord(&map.current_map.file_name,point)
}

pub(super) fn apply_current_player_position_map_transfer(world: &mut World) -> Vec<ServerPacket> {
    let map_file_name = world.resource::<MapRuntimeResource>().current_map.file_name.clone();
    let position = player_entity(world).and_then(|player| entity_position(world,player));
    if let Some(position) = position.filter(|position| super::default_npc_events::is_active_map_coord(&map_file_name,position)) {
        let source_need_move = crystal_map_respawns_ref(&map_file_name).is_some_and(|map|
            map.movements.iter().any(|movement| movement.source == position && movement.need_move && !movement.need_hole));
        if source_need_move {
            if super::default_npc_events::has_coordinate_ticket(world,&map_file_name,&position) {
                // Only a newly accepted entry owns a pending coordinate hook.
                // A failed script condition must let the next step leave the
                // entrance rather than locking the character on that cell.
                return vec![ServerPacket::NPCUpdate { npc_id:0 }];
            }
            return Vec::new();
        }
    }
    let Some(transfer) = transfer_for_current_player_position(world) else {
        return Vec::new();
    };

    apply_map_transfer(world, &transfer.key)
}

pub(super) fn crystal_movement_transfer_records_for_map(
    map_file_name: &str,
) -> Vec<MapTransferRecord> {
    let Some(map) = crystal_map_respawns_ref(map_file_name) else {
        return Vec::new();
    };

    map.movements
        .iter()
        .filter_map(|movement| {
            // `need_hole`/`need_move` movements require items/quest state we do
            // not model yet, so they stay excluded. Conquest movements are now
            // retained and carry their index; whether they actually fire is
            // gated on live war state at the call sites (see
            // `conquest_transfer_allowed`).
            if movement.need_hole || movement.need_move {
                return None;
            }
            // A destination of (0,0) is the manifest's "no explicit destination"
            // sentinel (the C# `Point` default; ~11% of movements). Crystal
            // never lands a player on the map's (0,0) corner — such direct
            // transfers are skipped. This also matches the collision-based
            // validity check below on machines that have the client `.map`
            // files (where (0,0) reads as non-walkable), but does not depend on
            // those binaries being present.
            if movement.destination.x == 0 && movement.destination.y == 0 {
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
                conquest_index: movement.conquest_index,
            })
        })
        .collect()
}

/// Whether the current player may use a conquest movement of the given index
/// (Crystal: `MyGuild.Conquest.Info.Index == ConquestIndex`). Ordinary
/// movements (`conquest_index <= 0`) are always allowed.
pub(super) fn conquest_movement_allowed(world: &World, conquest_index: i32) -> bool {
    if conquest_index <= 0 {
        return true;
    }
    let config=&world.resource::<RuntimeConfigResource>().config;
    if config.conquest_policies.iter().any(|p|p.index==conquest_index) {
        let Ok(Some(record))=config.shared_conquest_snapshot_checked(conquest_index) else{return false};
        let session=world.resource::<SessionResource>();
        let Some((account_id,character))=session.account_id.as_ref().zip(session.selected_character.as_ref()) else{return false};
        let identity=crate::config::Stage5FriendIdentity{account_id:account_id.clone(),character_index:character.index};
        let Ok(store)=config.account_store.lock() else{return false};
        return record.owner_guild_id.as_ref()
            .and_then(|id|store.shared_guilds.get(id)).is_some_and(|g|g.member(&identity).is_some());
    }
    let guild = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .name
        .trim()
        .to_string();
    if guild.is_empty() {
        return false;
    }
    world
        .resource::<MapRuntimeResource>()
        .conquest_owners
        .get(&conquest_index)
        .map(|owner| owner.eq_ignore_ascii_case(&guild))
        .unwrap_or(false)
}

pub(crate) fn crystal_direct_movement_transfer_source_cells(
    map_file_name: &str,
) -> BTreeSet<(i32, i32)> {
    let mut cells: BTreeSet<(i32,i32)> = crystal_movement_transfer_records_for_map(map_file_name)
        .into_iter()
        .flat_map(|transfer| {
            (transfer.from_bounds.min_y..=transfer.from_bounds.max_y).flat_map(move |y| {
                (transfer.from_bounds.min_x..=transfer.from_bounds.max_x).map(move |x| (x, y))
            })
        })
        .collect();
    if let Some(map) = crystal_map_respawns_ref(map_file_name) {
        cells.extend(map.movements.iter().filter(|movement| movement.need_move && !movement.need_hole
            && super::default_npc_events::is_active_map_coord(map_file_name,&movement.source))
            .map(|movement| (movement.source.x,movement.source.y)));
    }
    cells
}

fn crystal_manifest_movement_destination_is_valid(
    map_file_name: &str,
    destination: &Point,
) -> bool {
    runtime_full_map_collision_data(map_file_name)
        .map(|collision| {
            // Crystal's `Map.ValidPoint` checks the map cell's terrain validity
            // before a movement completes. A closed door is a separate dynamic
            // obstruction and does not invalidate an explicitly configured
            // movement destination. Several real shop exits intentionally land
            // beside/on a closed door cell (for example 0120 -> map 2 at
            // 517,492); rejecting those destinations removes the only exit and
            // strands the player inside the service map.
            point_in_bounds(&collision.collision.region_bounds, destination)
                && !collision.blocked_set.contains(&tile_key(destination))
        })
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
    match crate::world_runtime::parse_debug_crystal_transfer_key(key) {
        Ok(Some((map_file_name, position))) => {
            let map_info =
                super::npc_script::crystal_npc_move_map_information(world, &map_file_name);
            return relocate_player_to_map(world, map_info, position, MirDirection::Down, None);
        }
        Err(_) => {
            let language = super::session::current_language(world);
            return vec![super::session::system_message(&localized_text_or_fallback(
                language,
                "server.InvalidPacketReceived",
                "server.InvalidPacketReceived",
            ))];
        }
        Ok(None) => {}
    }

    let transfer = transfer_for_key(
        &world.resource::<RuntimeConfigResource>().config,
        world.resource::<MapRuntimeResource>(),
        key,
    )
    .or_else(|| {
        authorized_map_coordinate_transfers(world)
            .into_iter()
            .find(|transfer| transfer.key == key)
    });
    let Some(transfer) = transfer else {
        let language = super::session::current_language(world);
        return vec![super::session::system_message(&localized_text_or_fallback(
            language,
            "server.NotFound",
            "server.NotFound",
        ))];
    };

    // A conquest movement only fires for a member of the owning guild.
    if !conquest_movement_allowed(world, transfer.conquest_index) {
        let language = super::session::current_language(world);
        return vec![super::session::system_message(&localized_text_or_fallback(
            language,
            "server.CannotPositionMoveOnMap",
            "server.CannotPositionMoveOnMap",
        ))];
    }

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
    let mut destination_map = MapInformation {
        file_name: transfer.to_map_file_name.clone(),
        title: transfer.to_map_title.clone(),
        ..current_map
    };
    // Ordinary entrance transfers must carry the destination's identity and
    // presentation metadata, just as login does. Keeping the source map's
    // index/minimap/light makes a real map change look like same-map movement.
    // Custom maps absent from the Crystal manifest retain configured values.
    crate::config::apply_crystal_map_metadata(&mut destination_map);
    relocate_player_to_map(
        world,
        destination_map,
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
    if !world
        .resource::<RuntimeConfigResource>()
        .config
        .map_is_allowed(&map_info.file_name)
    {
        return vec![system_message(
            "This map is unavailable in the active content profile.",
        )];
    }

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
        .insert((Position(position.clone()), Facing(direction)));
    refresh_player_bind_at_position(world, &position);

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
    dismount_for_no_mount_map(world, &mut packets);
    // On-chain veins re-render on map transfer too (M4): unlike P0 spots — which
    // re-broadcast their stage on every swing — chain veins only update on a chain
    // settlement, so entering their map mid-session must seed the last known tier.
    packets.extend(super::onchain::onchain_mine_node_state_packets(world));
    if super::resources::is_in_world(world) {
        let map_file_name = world.resource::<MapRuntimeResource>().current_map.file_name.clone();
        let _ = super::default_npc_events::enqueue(world,super::default_npc_events::DefaultNpcEvent::MapEnter { map_file_name });
    }
    packets
}

/// Crystal force-dismounts a player when they enter a map flagged `NoMount`.
/// Mirror that on map transfer: if the destination map disallows mounts and the
/// player is currently riding, clear the ride and emit `MountUpdate` so the
/// client (and any AOI observers, once the gateway rebroadcasts it) drops the
/// mount visual.
fn dismount_for_no_mount_map(world: &mut World, packets: &mut Vec<ServerPacket>) {
    if !current_map_disallows_mount(world) {
        return;
    }
    let was_riding = {
        let mut mount = world.resource_mut::<MountResource>();
        let was_riding = mount.riding_mount;
        mount.riding_mount = false;
        was_riding
    };
    if !was_riding {
        return;
    }
    let mount_type = world.resource::<MountResource>().mount_type;
    packets.push(ServerPacket::MountUpdate {
        object_id: current_player_object_id(world).unwrap_or_default(),
        mount_type,
        riding_mount: false,
    });
    let language = super::session::current_language(world);
    packets.push(ServerPacket::Chat {
        message: localized_text_or_fallback(
            language,
            "server.MountNotAllowedOnMap",
            "Mounts are not allowed on this map. You have dismounted.",
        ),
        chat_type: ChatType::System,
    });
}

pub(super) fn clear_non_player_world_entities(world: &mut World) {
    let mut query = world.query_filtered::<Entity, (With<WorldObject>, Without<SelfPlayer>)>();
    let entities: Vec<Entity> = query.iter(world).collect();

    for entity in entities {
        let _ = world.despawn(entity);
    }
}

/// Spawn the operator-configured `visible_npcs` into the world. Used by
/// `rebuild_world` and again after the crystal world rebuild (which clears all
/// non-player objects), so configured NPCs also appear on crystal maps.
pub(super) fn spawn_config_visible_npcs(world: &mut World) {
    let records = world
        .resource::<RuntimeConfigResource>()
        .config
        .visible_npcs
        .clone();
    for record in &records {
        if record.script_key.as_deref().is_none_or(|script_key| {
            !world
                .resource::<RuntimeConfigResource>()
                .config
                .npc_script_is_allowed(script_key)
        }) && world
            .resource::<RuntimeConfigResource>()
            .config
            .content_profile
            .is_some()
        {
            continue;
        }
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
}

pub(super) fn should_use_crystal_current_map_world(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    if config.monster_spawn_source.uses_crystal_current_map()
        && crystal_map_respawns_ref(&map.current_map.file_name).is_some()
    {
        return true;
    }
    map.current_map.title != config.map.title
        || normalize_map_file_name(&map.current_map.file_name)
            != normalize_map_file_name(&config.map.file_name)
}

/// Activation / deactivation radii (Chebyshev tiles from a spawn point) for the
/// on-demand monster pool. Activation must exceed the client's view/data range
/// so a monster always exists before it can be seen; deactivation is larger for
/// hysteresis so a monster pacing the boundary does not flicker in and out.
const MONSTER_ACTIVATION_RANGE: i32 = 20;
const MONSTER_DEACTIVATION_RANGE: i32 = 30;

fn chebyshev_tile_distance(a: &Point, b: &Point) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

/// Spawn one spawn-table slot's ECS entity (the standard Crystal monster
/// bundle). Shared by map-entry spawning and the on-demand activation pass so
/// every materialised monster is identical regardless of how it entered the
/// world.
fn materialize_monster_slot(
    world: &mut World,
    rule_index: usize,
    rule: &MonsterSpawnRule,
    slot_index: usize,
    slot: &MonsterSpawnSlot,
) -> Entity {
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
    if rule.ai == 123 {
        world
            .entity_mut(entity)
            .insert(initial_general_meow_meow_state(0));
    }
    entity
}

/// Disclosed test preparation: activate dormant original source slots without
/// changing their IDs, spawn positions, templates, policies or runtime ticks.
/// No normal client command calls this; respawning/dead pools are rejected.
#[cfg(feature = "test-support")]
pub(super) fn materialize_cold_source_pool_for_test(world: &mut World) -> Result<usize, String> {
    if !is_in_world(world)
        || world
            .resource::<RuntimeConfigResource>()
            .config
            .monster_spawn_source
            != MonsterSpawnSource::CrystalWorld
    {
        return Err("cold source pool requires an entered original world".into());
    }
    let mut table = world
        .remove_resource::<MonsterSpawnTable>()
        .ok_or("missing original spawn pool")?;
    let unavailable = table
        .rules
        .iter()
        .flat_map(|rule| &rule.slots)
        .any(|slot| {
            slot.next_respawn_tick.is_some()
                || slot.entity.is_some_and(|entity| {
                    world.get::<MonsterAgent>(entity).is_none_or(|agent| agent.dead)
                        || world
                            .get::<MonsterVitals>(entity)
                            .is_none_or(|vitals| vitals.hp <= 0)
                })
        });
    if unavailable {
        world.insert_resource(table);
        return Err("cold source pool cannot resurrect or replace existing actors".into());
    }
    let mut activated = 0;
    for rule_index in 0..table.rules.len() {
        for slot_index in 0..table.rules[rule_index].slots.len() {
            if table.rules[rule_index].slots[slot_index].entity.is_some() {
                continue;
            }
            let rule = &table.rules[rule_index];
            let entity =
                materialize_monster_slot(world, rule_index, rule, slot_index, &rule.slots[slot_index]);
            table.rules[rule_index].slots[slot_index].entity = Some(entity);
            activated += 1;
        }
    }
    world.insert_resource(table);
    Ok(activated)
}

/// On-demand monster pool reconciliation for the fully-activated world: keep the
/// ECS holding only the monsters around the player. Slots within activation
/// range are materialised (fresh and alive); slots that have drifted beyond the
/// deactivation range are despawned and returned to the dormant pool. This is
/// what lets a 1,922-monster map cost ~tens of live entities instead of the full
/// roster, and it is client-transparent: the activation ring sits outside the
/// view range, so it only ever adds/removes entities the player cannot see — the
/// visible-object diff (scene view) is untouched. No-op outside `CrystalWorld`.
pub(super) fn reconcile_monster_activation(world: &mut World) {
    if world
        .resource::<RuntimeConfigResource>()
        .config
        .monster_spawn_source
        != MonsterSpawnSource::CrystalWorld
    {
        return;
    }
    let Some(player) = player_entity(world) else {
        return;
    };
    let Some(player_position) = entity_position(world, player) else {
        return;
    };
    let tick = super::session::runtime_tick(world);
    let Some(mut spawn_table) = world.remove_resource::<MonsterSpawnTable>() else {
        return;
    };

    // Index iteration keeps the borrow of `spawn_table` (for reading a slot's
    // rule) scoped tightly so it never overlaps the `&mut World` spawn/despawn.
    for rule_index in 0..spawn_table.rules.len() {
        for slot_index in 0..spawn_table.rules[rule_index].slots.len() {
            let slot = &spawn_table.rules[rule_index].slots[slot_index];
            let distance = chebyshev_tile_distance(&slot.spawn_position, &player_position);
            // Preserve respawn timing: a slot still cooling down stays dormant
            // (no monster) until its respawn tick elapses, so leaving and
            // returning to a spot can't skip the kill's respawn delay.
            let respawn_due = slot
                .next_respawn_tick
                .map_or(true, |respawn_tick| tick >= respawn_tick);
            match slot.entity {
                // Dormant slot: materialise it once the player is near, but only
                // if it is alive (no timer) or its respawn delay has elapsed.
                None => {
                    if distance <= MONSTER_ACTIVATION_RANGE && respawn_due {
                        let entity = {
                            let rule = &spawn_table.rules[rule_index];
                            materialize_monster_slot(
                                world,
                                rule_index,
                                rule,
                                slot_index,
                                &rule.slots[slot_index],
                            )
                        };
                        let slot = &mut spawn_table.rules[rule_index].slots[slot_index];
                        slot.entity = Some(entity);
                        slot.next_respawn_tick = None;
                    }
                }
                // Slot that has drifted far from the player: despawn its entity
                // but keep `next_respawn_tick`, so a mid-respawn slot doesn't pop
                // back alive the instant the player returns.
                Some(entity) => {
                    if distance > MONSTER_DEACTIVATION_RANGE {
                        let _ = world.despawn(entity);
                        spawn_table.rules[rule_index].slots[slot_index].entity = None;
                    }
                }
            }
        }
    }

    world.insert_resource(spawn_table);
}

pub(super) fn spawn_visible_world_for_current_map(world: &mut World) {
    let is_full_world = world
        .resource::<RuntimeConfigResource>()
        .config
        .monster_spawn_source
        == MonsterSpawnSource::CrystalWorld;
    let mut spawn_table = {
        let map = world.resource::<MapRuntimeResource>();
        let player_runtime = world.resource::<PlayerRuntimeResource>();
        let config = &world.resource::<RuntimeConfigResource>().config;
        // `CrystalWorld` keeps the whole map's roster as a dormant pool and only
        // materialises monsters near the player (see below); the starter region
        // materialises its on-screen slice eagerly.
        if is_full_world {
            build_crystal_current_map_full_spawn_table(config, &map.current_map.file_name)
        } else {
            build_crystal_current_map_visible_spawn_table(
                config,
                &map.current_map.file_name,
                &player_runtime.player_position,
            )
        }
    };

    // Non-pooled sources spawn every slot up front. `CrystalWorld` leaves slots
    // dormant and lets the activation pass materialise only the nearby ones.
    if !is_full_world {
        for rule_index in 0..spawn_table.rules.len() {
            for slot_index in 0..spawn_table.rules[rule_index].slots.len() {
                let entity = {
                    let rule = &spawn_table.rules[rule_index];
                    materialize_monster_slot(
                        world,
                        rule_index,
                        rule,
                        slot_index,
                        &rule.slots[slot_index],
                    )
                };
                spawn_table.rules[rule_index].slots[slot_index].entity = Some(entity);
            }
        }
    }

    spawn_crystal_current_map_npcs(world);
    spawn_stage5_hero(world);
    world.insert_resource(spawn_table);

    if is_full_world {
        reconcile_monster_activation(world);
    }
}

pub(super) fn spawn_stage5_hero(world: &mut World) -> Option<Entity> {
    let existing: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<Hero>>();
        query.iter(world).collect()
    };
    for entity in existing {
        if let Some(vitals) = world.get::<PlayerVitals>(entity) {
            let saved=crate::config::HeroVitalsState {hp:vitals.hp,mp:vitals.mp};
            world.resource_mut::<super::resources::HeroInventoryResource>().saved_vitals=Some(saved);
        }
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
    let max_hp=super::hero_ai::hero_authoritative_stat(world,super::crystal_compat::CRYSTAL_STAT_HP).max(0);
    let max_mp=super::hero_ai::hero_authoritative_stat(world,super::crystal_compat::CRYSTAL_STAT_MP).max(0);
    let saved=world.resource::<super::resources::HeroInventoryResource>().saved_vitals;
    let hp=saved.map_or(max_hp,|v|v.hp.min(max_hp));
    let mp=saved.map_or(max_mp,|v|v.mp.min(max_mp));

    let looks=super::hero_inventory::appearance(world);
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
                    armour_shape: u16::try_from(looks.armour).ok(),
                    weapon_shape: u16::try_from(looks.weapon).ok(),
                },
                PlayerVitals {
                    hp,
                    max_hp,
                    mp,
                    max_mp,
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
    let map_file_name = {
        let map = world.resource::<MapRuntimeResource>();
        if world
            .resource::<SessionResource>()
            .selected_character
            .is_none()
        {
            return;
        }
        normalize_map_file_name(&map.current_map.file_name)
    };
    let quest_ids_by_npc = super::npc::crystal_quest_ids_by_npc();
    let config = world.resource::<RuntimeConfigResource>().config.clone();

    for npc in crystal_npc_info_manifest().npcs.into_iter().chain(mir2_game_data::periodic_quests::npc_templates()) {
        let npc_map_file_name = npc.map_file_name.as_deref().map(normalize_map_file_name);
        if npc_map_file_name.as_deref() != Some(map_file_name.as_str()) {
            continue;
        }
        if !npc.loaded_object_id.is_some_and(|id|mir2_game_data::periodic_quests::npc(id).is_some())
            && !config.npc_script_is_allowed(&npc.script_key) {
            continue;
        }
        let Some(object_id) = npc.loaded_object_id else {
            continue;
        };
        let quest_ids = super::quests::effective_quest_ids_for_npc(world,object_id,&quest_ids_by_npc
            .get(&object_id)
            .map(|ids| ids.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default());

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
                script_key: (!npc.script_key.is_empty()).then_some(npc.script_key),
            },
        ));
    }
}

fn publish_map_collision<T: Clone>(
    cache: &Mutex<BTreeMap<String, Option<T>>>,
    normalized: String,
    parsed: Option<T>,
) -> Option<T> {
    // Parsing stays outside the lock. A concurrent loader may have already
    // published this map; return that same allocation instead of overwriting it.
    // None is still a cached result, including when it is the first publisher.
    cache
        .lock()
        .expect("runtime map collision cache should not be poisoned")
        .entry(normalized)
        .or_insert(parsed)
        .clone()
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
    publish_map_collision(cache, normalized, parsed)
}

pub(super) fn runtime_map_collision_data_uncached(
    normalized: &str,
) -> Option<RuntimeMapCollisionData> {
    if normalized == normalize_map_file_name("0") {
        return Some(runtime_map_collision_from_template(starter_map_collision()));
    }

    let bytes = crystal_map_path(normalized)
        .and_then(|path| fs::read(path).ok())
        .or_else(|| read_crystal_map_pack_bytes(normalized))?;
    let collision = parse_runtime_map_collision(normalized, &bytes)?;
    Some(runtime_map_collision_from_template(collision))
}

/// Locate the bundled `crystal-map-pack` directory (gzipped Crystal `.map`
/// files), so the sim can host maps with no Crystal client install. Honors
/// `MIR2_CRYSTAL_MAP_PACK`, else falls back to the in-repo pack path.
fn crystal_map_pack_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("MIR2_CRYSTAL_MAP_PACK") {
        let dir = PathBuf::from(dir);
        return dir.is_dir().then_some(dir);
    }
    let repo_default =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/lib/generated/crystal-map-pack");
    repo_default.is_dir().then_some(repo_default)
}

/// Decompress a map's raw `.map` bytes from `{pack}/{name}.map.gz`.
fn read_crystal_map_pack_bytes(normalized: &str) -> Option<Vec<u8>> {
    use std::io::Read;
    let gz_path = crystal_map_pack_dir()?.join(format!("{normalized}.map.gz"));
    let gz = fs::read(gz_path).ok()?;
    let mut decoder = flate2::read::GzDecoder::new(gz.as_slice());
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).ok()?;
    Some(bytes)
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
    let fishing_cells = collision
        .fishing_cells
        .iter()
        .map(|cell| ((cell.x, cell.y), cell.attribute))
        .collect();

    RuntimeMapCollisionData {
        collision: Arc::new(collision),
        blocked_set: Arc::new(blocked_set),
        closed_door_set,
        fishing_cells: Arc::new(fishing_cells),
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
    let normalized = normalize_map_file_name(map_file_name);
    // Collision availability is not a successful Source Map.Load receipt.
    // Share the bounded Source cell decoder; preserve the existing cache key.
    super::source_map_loading::decode_runtime_map_collision(&normalized, bytes).ok()
}

pub(super) fn runtime_full_map_collision_data(
    map_file_name: &str,
) -> Option<Arc<RuntimeMapCollisionData>> {
    let normalized = normalize_map_file_name(map_file_name);
    static RUNTIME_FULL_MAP_COLLISION_CACHE: OnceLock<
        Mutex<BTreeMap<String, Option<Arc<RuntimeMapCollisionData>>>>,
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
        .map(runtime_map_collision_from_template)
        .map(Arc::new);
    publish_map_collision(cache, normalized, parsed)
}

/// Full per-map collision for the activated Crystal world. Prefers an
/// uncompressed Crystal client install, then falls back to the bundled gzipped
/// map-pack so it works with no client present. Unlike
/// [`runtime_map_collision_data`], it returns the *full* Bichon map for "0"
/// (the pack's `0.map`) instead of the starter slice, so a fully-activated map
/// spawns across its real walkable cells. Cached per map; concurrent cold loads
/// may parse independently but all return the same published allocation.
pub(super) fn runtime_world_map_collision_data(
    map_file_name: &str,
) -> Option<Arc<RuntimeMapCollisionData>> {
    let normalized = normalize_map_file_name(map_file_name);
    static RUNTIME_WORLD_MAP_COLLISION_CACHE: OnceLock<
        Mutex<BTreeMap<String, Option<Arc<RuntimeMapCollisionData>>>>,
    > = OnceLock::new();
    let cache = RUNTIME_WORLD_MAP_COLLISION_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(cached) = cache
        .lock()
        .expect("runtime world map collision cache should not be poisoned")
        .get(&normalized)
        .cloned()
    {
        return cached;
    }

    let parsed = runtime_full_map_collision_data(&normalized).or_else(|| {
        read_crystal_map_pack_bytes(&normalized)
            .and_then(|bytes| parse_runtime_map_collision(&normalized, &bytes))
            .map(runtime_map_collision_from_template)
            .map(Arc::new)
    });
    publish_map_collision(cache, normalized, parsed)
}

pub(super) fn refresh_runtime_map_collision(world: &mut World) {
    let current_map = world.resource::<MapRuntimeResource>().current_map.clone();
    let config = &world.resource::<RuntimeConfigResource>().config;
    // The activated world walks the full map everywhere (gz map-pack backed), so
    // players are never fenced into the Bichon starter slice — they can reach
    // every transfer and roam all maps. Other sources keep their prior collision.
    let collision = if config.monster_spawn_source == MonsterSpawnSource::CrystalWorld {
        crystal_world_collision_data_or_config(config, &current_map.file_name)
    } else {
        runtime_active_map_collision_data(&current_map)
            .unwrap_or_else(|| runtime_map_collision_from_template(config.map_collision.clone()))
    };

    let doors = super::resources::DoorRegistry::from_templates(&collision.collision.doors);
    {
        let mut map = world.resource_mut::<MapRuntimeResource>();
        map.map_region_bounds = collision.collision.region_bounds;
        map.blocked_cells = collision.blocked_set;
        map.closed_door_cells = collision.closed_door_set;
        map.doors = doors;
        map.fishing_cells = collision.fishing_cells;
    }
    super::mining::rebuild_mine_spots(world);
    if let Some(mut hazards) = world.get_resource_mut::<super::hazard::MapHazardResource>() {
        hazards.reset();
    }
}

pub(super) fn runtime_active_map_collision_data(
    map: &MapInformation,
) -> Option<RuntimeMapCollisionData> {
    if normalize_map_file_name(&map.file_name) == normalize_map_file_name("0")
        && map.title != "Starter Field"
    {
        return runtime_world_map_collision_data(&map.file_name)
            .map(|collision| (*collision).clone())
            .or_else(|| runtime_map_collision_data(&map.file_name));
    }
    runtime_map_collision_data(&map.file_name)
}

pub(super) fn collision_data_for_map_or_config(
    config: &SimulationConfig,
    map_file_name: &str,
) -> RuntimeMapCollisionData {
    if config.monster_spawn_source == MonsterSpawnSource::CrystalWorld {
        crystal_world_collision_data_or_config(config, map_file_name)
    } else {
        runtime_map_collision_data(map_file_name)
            .unwrap_or_else(|| runtime_map_collision_from_template(config.map_collision.clone()))
    }
}

fn crystal_world_collision_data_or_config(
    config: &SimulationConfig,
    map_file_name: &str,
) -> RuntimeMapCollisionData {
    runtime_world_map_collision_data(map_file_name)
        .map(|collision| (*collision).clone())
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
        // Reconcile the freshly spawned player's pools/stat block with the
        // current class/level/equipment (Crystal `RefreshStats`).
        super::stats::refresh_player_stats(world);
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

    spawn_config_visible_npcs(world);

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

#[cfg(test)]
#[path = "map_collision_source_tests.rs"]
mod map_collision_source_tests;

#[cfg(test)]
#[path = "map_transfer_metadata_tests.rs"]
mod map_transfer_metadata_tests;

pub(super) fn current_map_disallows_intelligent_creatures(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    crystal_map_respawns_ref(&map.current_map.file_name)
        .is_some_and(|map| map.no_intelligent_creatures)
        || current_map_drop_rule(config, map).is_some_and(|rule| rule.no_intelligent_creatures)
}

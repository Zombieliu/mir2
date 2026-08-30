use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{With, World};
use mir2_game_data::{DecorKind, MapBounds, TerrainKind};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket, UserLocation};

use super::combat::crystal_player_movement_blocked_by_status;
use super::components::{
    current_player_is_dead, current_player_object_id, entity_facing, entity_position,
    player_entity, Facing, GroundDrop, Hero, MonsterAgent, MonsterAiState, Npc, ObjectId,
    PlayerVitals, Position, RemotePlayer, SelfPlayer,
};
use super::map::{
    apply_current_player_position_map_transfer, is_current_map_transfer_source,
    normalize_map_file_name, relocate_player_to_map,
};
use super::map_events::map_coordinate_hint_packets_for_path;
use super::monster_ai::advance_world;
use super::npc::dismiss_dialog;
use super::packets::{object_health_info_for_entity, object_revived_info_for_entity};
use super::pathfind;
use super::resources::{
    crystal_player_can_run, is_in_world, mark_crystal_player_move, MapRuntimeResource,
    PlayerRuntimeResource, RuntimeConfigResource,
};
use super::session::SimulationSession;

pub(super) fn direction_toward(source: &Point, target: &Point) -> Option<MirDirection> {
    let dx = (target.x - source.x).signum();
    let dy = (target.y - source.y).signum();

    match (dx, dy) {
        (0, 0) => None,
        (0, -1) => Some(MirDirection::Up),
        (1, -1) => Some(MirDirection::UpRight),
        (1, 0) => Some(MirDirection::Right),
        (1, 1) => Some(MirDirection::DownRight),
        (0, 1) => Some(MirDirection::Down),
        (-1, 1) => Some(MirDirection::DownLeft),
        (-1, 0) => Some(MirDirection::Left),
        (-1, -1) => Some(MirDirection::UpLeft),
        _ => None,
    }
}

pub(super) fn step_point_toward(source: &Point, target: &Point) -> Point {
    Point {
        x: source.x + (target.x - source.x).signum(),
        y: source.y + (target.y - source.y).signum(),
    }
}

pub(super) fn step_point_toward_by(source: &Point, target: &Point, amount: i32) -> Point {
    let mut position = source.clone();
    for _ in 0..amount.max(1) {
        let next = step_point_toward(&position, target);
        if next == position {
            break;
        }
        position = next;
        if position == *target {
            break;
        }
    }
    position
}

pub(super) fn offset_point(source: &Point, direction: MirDirection, amount: i32) -> Point {
    let (dx, dy) = match direction {
        MirDirection::Up => (0, -1),
        MirDirection::UpRight => (1, -1),
        MirDirection::Right => (1, 0),
        MirDirection::DownRight => (1, 1),
        MirDirection::Down => (0, 1),
        MirDirection::DownLeft => (-1, 1),
        MirDirection::Left => (-1, 0),
        MirDirection::UpLeft => (-1, -1),
    };

    Point {
        x: source.x + dx * amount,
        y: source.y + dy * amount,
    }
}

pub(super) fn rotated_direction(direction: MirDirection, offset: i32) -> MirDirection {
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
    let index = directions
        .iter()
        .position(|candidate| *candidate == direction)
        .unwrap_or(0) as i32;
    directions[((index + offset).rem_euclid(8)) as usize]
}

pub(super) fn tile_distance(source: &Point, target: &Point) -> i32 {
    (source.x - target.x).abs().max((source.y - target.y).abs())
}

pub(super) fn tile_key(point: &Point) -> (i32, i32) {
    (point.x, point.y)
}

pub(super) fn point_in_bounds(bounds: &MapBounds, point: &Point) -> bool {
    point.x >= bounds.min_x
        && point.x <= bounds.max_x
        && point.y >= bounds.min_y
        && point.y <= bounds.max_y
}

pub(super) fn directional_destination(
    world: &World,
    source: &Point,
    direction: MirDirection,
    amount: i32,
    ignore_entity: Option<Entity>,
) -> Option<Point> {
    let mut current = source.clone();
    for _ in 0..amount.max(1) {
        let next = offset_point(&current, direction, 1);
        if !can_occupy(world, next.clone(), ignore_entity) {
            return None;
        }
        current = next;
    }
    Some(current)
}

pub(super) fn summon_spawn_position_near(
    world: &World,
    origin: &Point,
    preferred_direction: MirDirection,
    distance: i32,
    ignore_entity: Option<Entity>,
) -> Point {
    let candidates = [
        preferred_direction,
        MirDirection::Up,
        MirDirection::UpRight,
        MirDirection::Right,
        MirDirection::DownRight,
        MirDirection::Down,
        MirDirection::DownLeft,
        MirDirection::Left,
        MirDirection::UpLeft,
    ];

    for direction in candidates {
        if let Some(point) =
            directional_destination(world, origin, direction, distance, ignore_entity)
        {
            return point;
        }
    }

    offset_point(origin, preferred_direction, distance)
}

pub(super) fn can_traverse_between(
    world: &World,
    source: &Point,
    target: &Point,
    ignore_entity: Option<Entity>,
) -> bool {
    let mut current = source.clone();
    while current != *target {
        let next = step_point_toward(&current, target);
        if next == current || !can_player_movement_occupy(world, next.clone(), ignore_entity) {
            return false;
        }
        current = next;
    }
    true
}

pub(super) fn can_occupy(world: &World, point: Point, ignore_entity: Option<Entity>) -> bool {
    !is_blocked_tile(world, &point) && !has_blocking_entity(world, &point, ignore_entity)
}

fn can_player_movement_occupy(world: &World, point: Point, ignore_entity: Option<Entity>) -> bool {
    !is_player_movement_blocked_tile(world, &point)
        && !has_blocking_entity(world, &point, ignore_entity)
}

pub(super) fn current_location(world: &World) -> UserLocation {
    let player = player_entity(world).expect("player should exist when current_location is used");
    UserLocation {
        position: entity_position(world, player).expect("player position"),
        direction: entity_facing(world, player).expect("player facing"),
    }
}

pub(super) fn town_teleport_packets(world: &mut World) -> Vec<ServerPacket> {
    if let Some(player) = player_entity(world) {
        let spawn = world
            .resource::<RuntimeConfigResource>()
            .config
            .spawn
            .clone();
        world
            .entity_mut(player)
            .insert((Position(spawn), Facing(MirDirection::Down)));
    }

    vec![ServerPacket::UserLocation {
        location: current_location(world),
    }]
}

/// Crystal `PlayerObject.TownRevive` (PlayerObject.cs:1392): a dead player
/// requests respawn at their bind point. The client sends this after the death
/// prompt (`C.TownRevive`, dispatched from `MirConnection.cs:505`). Crystal moves
/// the player to `BindMapIndex`/`BindLocation`, restores HP/MP, then enqueues
/// `S.Revived` to self and broadcasts `S.ObjectRevived { Effect = true }`.
///
/// The single-session world binds to the configured map + spawn (its town/safe
/// zone), so a field death must change both authorities. A same-map revive only
/// needs `UserLocation`; a cross-map revive goes through the ordinary map
/// relocation path and emits `MapInformation` before the revive packets. No-op
/// when the player is not dead (mirrors `if (!Dead) return;`).
pub(super) fn town_revive_packets(world: &mut World) -> Vec<ServerPacket> {
    if !current_player_is_dead(world) {
        return Vec::new();
    }
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };

    // Crystal restores `Stats[HP]`/`Stats[MP]`; HP floors at 1 so the revived
    // player is never re-flagged dead by a zero-HP read.
    let revived_vitals = {
        let mut entry = world.entity_mut(player);
        let Some(mut vitals) = entry.get_mut::<PlayerVitals>() else {
            return Vec::new();
        };
        vitals.hp = vitals.max_hp.max(1);
        vitals.mp = vitals.max_mp;
        *vitals
    };

    let (bind_map, bind_position, current_map_file_name) = {
        let config = &world.resource::<RuntimeConfigResource>().config;
        let current_map = &world.resource::<MapRuntimeResource>().current_map;
        (
            config.map.clone(),
            config.spawn.clone(),
            current_map.file_name.clone(),
        )
    };
    world.resource_mut::<PlayerRuntimeResource>().player_vitals = revived_vitals;

    let changes_map = normalize_map_file_name(&current_map_file_name)
        != normalize_map_file_name(&bind_map.file_name);
    let mut packets = if changes_map {
        relocate_player_to_map(world, bind_map, bind_position, MirDirection::Down, None)
    } else {
        world
            .entity_mut(player)
            .insert((Position(bind_position.clone()), Facing(MirDirection::Down)));
        {
            let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
            runtime.player_position = bind_position;
            runtime.player_direction = MirDirection::Down;
        }
        vec![ServerPacket::UserLocation {
            location: current_location(world),
        }]
    };

    // Snap the player to the bind location, clear the dead flag (S.Revived), and
    // play the revive effect (broadcast S.ObjectRevived). ObjectHealth refreshes
    // the restored HP/MP bar.
    packets.push(ServerPacket::Revived);
    if let Some(info) = object_revived_info_for_entity(world, player, true) {
        packets.push(ServerPacket::ObjectRevived { info });
    }
    if let Some(info) = object_health_info_for_entity(world, player, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    packets
}

pub(super) fn crystal_random_same_map_teleport_packets(
    world: &mut World,
    max_radius: i32,
) -> Option<Vec<ServerPacket>> {
    let player = player_entity(world)?;
    let start = entity_position(world, player)?;
    let direction = world.resource::<PlayerRuntimeResource>().player_direction;
    let map_info = world.resource::<MapRuntimeResource>().current_map.clone();

    for radius in 1..=max_radius.max(1) {
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let candidate = Point {
                    x: start.x.saturating_add(dx),
                    y: start.y.saturating_add(dy),
                };
                if candidate == start {
                    continue;
                }
                if can_occupy(world, candidate.clone(), Some(player)) {
                    return Some(relocate_player_to_map(
                        world, map_info, candidate, direction, None,
                    ));
                }
            }
        }
    }

    None
}

pub(super) fn current_movement(world: &World) -> ObjectMovement {
    let player = player_entity(world).expect("player should exist when current_movement is used");
    ObjectMovement {
        object_id: current_player_object_id(world).expect("player object id"),
        position: entity_position(world, player).expect("player position"),
        direction: entity_facing(world, player).expect("player facing"),
    }
}

pub(super) fn player_motion_packet(world: &World, running: bool) -> ServerPacket {
    let movement = current_movement(world);
    if running {
        ServerPacket::ObjectRun { movement }
    } else {
        ServerPacket::ObjectWalk { movement }
    }
}

pub(super) fn push_player_in_direction(
    world: &mut World,
    player: Entity,
    direction: MirDirection,
    amount: i32,
    packets: &mut Vec<ServerPacket>,
) -> Option<Point> {
    let object_id = current_player_object_id(world)?;
    let origin = entity_position(world, player)?;
    let mut destination = origin.clone();

    for _ in 0..amount.max(1) {
        let next = offset_point(&destination, direction, 1);
        if !can_occupy(world, next.clone(), Some(player)) {
            break;
        }
        destination = next;
    }

    if destination == origin {
        return None;
    }

    world
        .entity_mut(player)
        .insert((Position(destination.clone()), Facing(direction)));
    packets.push(ServerPacket::ObjectWalk {
        movement: ObjectMovement {
            object_id,
            position: destination.clone(),
            direction,
        },
    });
    Some(destination)
}

pub(super) fn follow_player_with_stage5_hero(
    world: &mut World,
    previous_player_position: Point,
    previous_player_direction: MirDirection,
    running: bool,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(player) = player_entity(world) else {
        return;
    };
    let Some(player_position) = entity_position(world, player) else {
        return;
    };
    let Some(hero_entity) = ({
        let mut query = world.query_filtered::<Entity, With<Hero>>();
        query.iter(world).next()
    }) else {
        return;
    };
    let Some(hero_object_id) = world.entity(hero_entity).get::<ObjectId>().map(|id| id.0) else {
        return;
    };
    let Some(hero_position) = entity_position(world, hero_entity) else {
        return;
    };

    let desired_position = if can_occupy(world, previous_player_position.clone(), Some(hero_entity))
    {
        previous_player_position
    } else {
        summon_spawn_position_near(
            world,
            &player_position,
            previous_player_direction,
            1,
            Some(hero_entity),
        )
    };
    if hero_position == desired_position {
        return;
    }

    let direction =
        direction_toward(&hero_position, &desired_position).unwrap_or(previous_player_direction);
    world
        .entity_mut(hero_entity)
        .insert((Position(desired_position.clone()), Facing(direction)));
    let movement = ObjectMovement {
        object_id: hero_object_id,
        position: desired_position,
        direction,
    };
    if running {
        packets.push(ServerPacket::ObjectRun { movement });
    } else {
        packets.push(ServerPacket::ObjectWalk { movement });
    }
}

pub(super) fn move_distance_for_mode(running: bool) -> i32 {
    if running {
        2
    } else {
        1
    }
}

/// Choose the next tile to step onto when routing from `from` toward `to`,
/// using the server-side A* search so click-to-move paths around static
/// blockers and other occupants instead of stalling against them.
///
/// Tries an exact-tile route first; if `to` is unreachable because it is itself
/// occupied (the common "click a monster to approach it" case), falls back to a
/// route that stops on a tile *adjacent* to `to`. Returns `None` only when not
/// even an adjacent tile is reachable, letting the caller fall back to
/// straight-line stepping.
fn pathfind_next_step(
    world: &World,
    from: &Point,
    to: &Point,
    max_step: i32,
    ignore_entity: Entity,
) -> Option<Point> {
    let path = pathfind::find_path(world, from, to, Some(ignore_entity))
        .filter(|path| !path.is_empty())
        .or_else(|| {
            pathfind::find_path_adjacent(world, from, to, Some(ignore_entity))
                .filter(|path| !path.is_empty())
        })?;
    let first = path.first()?.clone();
    if max_step <= 1 {
        return Some(first);
    }
    // Running covers two tiles, but only extend to the second tile when the
    // route continues in the same direction. If the path bends at the first
    // tile, degrade the run to a single-tile walk for this step rather than
    // cutting the corner (Crystal allows a run to degrade to a walk).
    if let (Some(second), Some(direction)) = (path.get(1), direction_toward(from, &first)) {
        if direction_toward(&first, second) == Some(direction) {
            return Some(second.clone());
        }
    }
    Some(first)
}

fn movement_path_between(source: &Point, destination: &Point) -> Vec<Point> {
    let mut path = Vec::new();
    let mut current = source.clone();
    while current != *destination {
        let next = step_point_toward(&current, destination);
        if next == current {
            break;
        }
        path.push(next.clone());
        current = next;
    }
    path
}

pub(super) fn clamp_to_map_region(world: &World, point: Point) -> Point {
    let bounds = world.resource::<MapRuntimeResource>().map_region_bounds;

    Point {
        x: point.x.clamp(bounds.min_x, bounds.max_x),
        y: point.y.clamp(bounds.min_y, bounds.max_y),
    }
}

pub(super) fn step_player(world: &mut World, amount: i32) -> bool {
    if player_is_paralyzed(world) {
        return false;
    }

    if let Some(player) = player_entity(world) {
        let (direction, position) = {
            let entity = world.entity(player);
            (
                entity.get::<Facing>().expect("player facing").0,
                entity.get::<Position>().expect("player position").0.clone(),
            )
        };
        if let Some(destination) =
            player_directional_destination(world, &position, direction, amount, Some(player))
        {
            world.entity_mut(player).insert(Position(destination));
            return true;
        }
    }

    false
}

fn player_directional_destination(
    world: &World,
    source: &Point,
    direction: MirDirection,
    amount: i32,
    ignore_entity: Option<Entity>,
) -> Option<Point> {
    let mut current = source.clone();
    for _ in 0..amount.max(1) {
        let next = offset_point(&current, direction, 1);
        if !can_player_movement_occupy(world, next.clone(), ignore_entity) {
            return None;
        }
        current = next;
    }
    Some(current)
}

pub(super) fn player_is_paralyzed(world: &World) -> bool {
    crystal_player_movement_blocked_by_status(world)
}

#[allow(deprecated)]
pub(super) fn runtime_position_exists(world: &World, point: &Point) -> bool {
    world.iter_entities().any(|entity| {
        entity
            .get::<Position>()
            .map(|position| position.0 == *point)
            .unwrap_or(false)
    })
}

pub(super) fn is_blocked_tile(world: &World, point: &Point) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    if !point_in_bounds(&map.map_region_bounds, point) {
        return true;
    }

    let map_blocked = map.blocked_cells.contains(&tile_key(point));
    let door_blocked = map.closed_door_cells.contains(&tile_key(point));

    let terrain_blocked = config.terrain_patches.iter().any(|patch| {
        let within = point.x >= patch.x
            && point.x < patch.x + i32::from(patch.width)
            && point.y >= patch.y
            && point.y < patch.y + i32::from(patch.height);

        within && matches!(patch.kind, TerrainKind::Water)
    });

    let decor_blocked = config.decor_objects.iter().any(|decor| {
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

    map_blocked || door_blocked || terrain_blocked || decor_blocked
}

fn is_player_movement_blocked_tile(world: &World, point: &Point) -> bool {
    if !point_in_bounds(
        &world.resource::<MapRuntimeResource>().map_region_bounds,
        point,
    ) {
        return true;
    }

    let blocked = is_blocked_tile(world, point);
    blocked && !is_current_map_transfer_source(world, point)
}

#[allow(deprecated)]
pub(super) fn has_blocking_entity(
    world: &World,
    point: &Point,
    ignore_entity: Option<Entity>,
) -> bool {
    world.iter_entities().any(|entity| {
        if Some(entity.id()) == ignore_entity || entity.get::<GroundDrop>().is_some() {
            return false;
        }

        let position = entity.get::<Position>();
        let is_blocker = entity.get::<SelfPlayer>().is_some()
            || entity.get::<Hero>().is_some()
            || entity.get::<RemotePlayer>().is_some()
            || entity.get::<Npc>().is_some()
            || entity
                .get::<MonsterAgent>()
                .map(|monster| {
                    let ai_state = entity.get::<MonsterAiState>().copied().unwrap_or_default();
                    !monster.dead && !ai_state.hidden
                })
                .unwrap_or(false);

        is_blocker
            && position
                .map(|position| &position.0 == point)
                .unwrap_or(false)
    })
}

impl SimulationSession {
    pub fn move_to(&mut self, destination: Point) -> Vec<ServerPacket> {
        self.move_to_with_mode(destination, false)
    }

    pub fn move_to_with_mode(&mut self, destination: Point, running: bool) -> Vec<ServerPacket> {
        let packets = self.move_to_with_mode_impl(destination, running);
        self.finalize_packets(packets)
    }

    pub(super) fn move_to_with_mode_impl(
        &mut self,
        destination: Point,
        running: bool,
    ) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        if current_player_is_dead(self.app.world())
            || crystal_player_movement_blocked_by_status(self.app.world())
        {
            let mut packets = vec![ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            }];
            packets.extend(advance_world(self.app.world_mut()));
            return packets;
        }

        // A disconnect can persist the authoritative transform after the
        // player stepped onto a one-cell movement source but before the client
        // observed the ensuing map change. The next ordinary movement intent
        // must finish that already-earned transfer instead of trying to walk
        // out through the surrounding building collision.
        let mut standing_transfer_packets =
            apply_current_player_position_map_transfer(self.app.world_mut());
        if !standing_transfer_packets.is_empty() {
            standing_transfer_packets.extend(advance_world(self.app.world_mut()));
            return standing_transfer_packets;
        }

        dismiss_dialog(self.app.world_mut());
        let target = clamp_to_map_region(self.app.world(), destination);
        let player_entity = player_entity(self.app.world()).expect("player should exist");
        let next_position = {
            let entity = self.app.world().entity(player_entity);
            entity.get::<Position>().expect("player position").0.clone()
        };
        let previous_player_direction = self
            .app
            .world()
            .entity(player_entity)
            .get::<Facing>()
            .expect("player facing")
            .0;
        let mut packets = Vec::new();

        if next_position != target {
            let move_distance = move_distance_for_mode(running);
            let candidate = step_point_toward_by(&next_position, &target, move_distance);

            if candidate != next_position {
                // Prefer an A*-routed step so click-to-move walks around walls,
                // trees and other occupants. Fall back to the original
                // straight-line stepping when no full route exists (e.g. the
                // destination tile is occupied), which still advances toward the
                // target and preserves the legacy "approach then stop" feel.
                let routed_step = pathfind_next_step(
                    self.app.world(),
                    &next_position,
                    &target,
                    move_distance,
                    player_entity,
                );

                let next_step = if let Some(routed_step) = routed_step {
                    Some(routed_step)
                } else if can_traverse_between(
                    self.app.world(),
                    &next_position,
                    &candidate,
                    Some(player_entity),
                ) {
                    Some(candidate)
                } else if running {
                    let fallback = step_point_toward(&next_position, &target);
                    if fallback != next_position
                        && can_traverse_between(
                            self.app.world(),
                            &next_position,
                            &fallback,
                            Some(player_entity),
                        )
                    {
                        Some(fallback)
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(next_step) = next_step {
                    let movement_path = movement_path_between(&next_position, &next_step);
                    if let Some(direction) = direction_toward(&next_position, &next_step) {
                        self.app
                            .world_mut()
                            .entity_mut(player_entity)
                            .insert((Position(next_step), Facing(direction)));

                        packets.push(player_motion_packet(self.app.world(), running));
                        follow_player_with_stage5_hero(
                            self.app.world_mut(),
                            next_position.clone(),
                            previous_player_direction,
                            running,
                            &mut packets,
                        );
                        packets.extend(map_coordinate_hint_packets_for_path(
                            self.app.world(),
                            &movement_path,
                        ));
                        packets.extend(apply_current_player_position_map_transfer(
                            self.app.world_mut(),
                        ));
                    }
                }
            }
        }

        packets.extend(advance_world(self.app.world_mut()));
        packets
    }
    pub(super) fn move_player_by_direction(
        &mut self,
        direction: MirDirection,
        running: bool,
    ) -> Vec<ServerPacket> {
        if current_player_is_dead(self.app.world())
            || crystal_player_movement_blocked_by_status(self.app.world())
        {
            let mut packets = vec![ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            }];
            packets.extend(advance_world(self.app.world_mut()));
            return packets;
        }

        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };
        let previous_position = entity_position(self.app.world(), player).expect("player position");
        let previous_direction = entity_facing(self.app.world(), player).unwrap_or(direction);

        self.app
            .world_mut()
            .entity_mut(player)
            .insert(Facing(direction));
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .player_direction = direction;

        // See move_to_with_mode_impl: a normal direction key also reactivates
        // a transfer source restored as the persisted player position. Crystal
        // applies the new facing before CheckMovement, so ENTERMAP receives
        // the requested direction as well.
        let mut standing_transfer_packets =
            apply_current_player_position_map_transfer(self.app.world_mut());
        if !standing_transfer_packets.is_empty() {
            standing_transfer_packets.extend(advance_world(self.app.world_mut()));
            return standing_transfer_packets;
        }

        if running && !crystal_player_can_run(self.app.world_mut()) {
            let mut packets = vec![ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            }];
            packets.extend(advance_world(self.app.world_mut()));
            return packets;
        }

        let moved = step_player(self.app.world_mut(), move_distance_for_mode(running));
        if moved {
            mark_crystal_player_move(self.app.world_mut(), running);
        }

        let mut packets = vec![ServerPacket::UserLocation {
            location: current_location(self.app.world()),
        }];
        let current_position = entity_position(self.app.world(), player).expect("player position");
        if current_position != previous_position {
            follow_player_with_stage5_hero(
                self.app.world_mut(),
                previous_position.clone(),
                previous_direction,
                running,
                &mut packets,
            );
            let movement_path = movement_path_between(&previous_position, &current_position);
            packets.extend(map_coordinate_hint_packets_for_path(
                self.app.world(),
                &movement_path,
            ));
            packets.extend(apply_current_player_position_map_transfer(
                self.app.world_mut(),
            ));
        }

        packets.extend(advance_world(self.app.world_mut()));
        packets
    }
}

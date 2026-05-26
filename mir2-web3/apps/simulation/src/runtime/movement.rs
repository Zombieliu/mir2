use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{With, World};
use mir2_game_data::{DecorKind, MapBounds, TerrainKind};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket, UserLocation};

use super::components::{
    current_player_object_id, entity_facing, entity_position, player_entity, Facing, GroundDrop,
    Hero, MonsterAgent, MonsterAiState, Npc, ObjectId, Position, RemotePlayer, SelfPlayer,
};
use super::crystal_compat::CAVE_MAGGOT_PARALYSIS_BUFF_KEY;
use super::map::{
    apply_current_player_position_map_transfer, is_current_map_transfer_source,
    relocate_player_to_map,
};
use super::monster_ai::advance_world;
use super::npc::dismiss_dialog;
use super::resources::{
    crystal_player_can_run, is_in_world, mark_crystal_player_move, BuffResource,
    MapRuntimeResource, PlayerRuntimeResource, RuntimeConfigResource,
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
    let tick = super::session::runtime_tick(world);
    world
        .resource::<BuffResource>()
        .buffs
        .iter()
        .any(|buff| buff.key == CAVE_MAGGOT_PARALYSIS_BUFF_KEY && buff.expires_at_tick > tick)
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
            let candidate =
                step_point_toward_by(&next_position, &target, move_distance_for_mode(running));

            if candidate != next_position {
                let next_step = if can_traverse_between(
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
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };
        let previous_position = entity_position(self.app.world(), player).expect("player position");
        let previous_direction = entity_facing(self.app.world(), player).unwrap_or(direction);

        self.app
            .world_mut()
            .entity_mut(player)
            .insert(Facing(direction));

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
                previous_position,
                previous_direction,
                running,
                &mut packets,
            );
            packets.extend(apply_current_player_position_map_transfer(
                self.app.world_mut(),
            ));
        }

        packets.extend(advance_world(self.app.world_mut()));
        packets
    }
}

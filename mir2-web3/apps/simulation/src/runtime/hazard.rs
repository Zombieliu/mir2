//! Environmental map hazards — Crystal `Map.Process` lightning/lava strikes.
//!
//! On maps flagged `Lightning`/`Fire`, the server periodically (every 3–15 s)
//! strikes a cell: a 1-in-4 chance lands directly on a player, otherwise a
//! random cell within ±10 tiles. A strike on a player's cell deals
//! `Random(LightningDamage)` / `Random(FireDamage)` damage. Hazard placement
//! comes from `SimulationConfig.map_hazards` (Crystal stores the flags in the
//! Map DB, which is absent here).

use bevy_ecs::prelude::{Resource, World};
use mir2_protocol::{MirDirection, ObjectSpellInfo, Point, ServerPacket, Spell};

use super::combat::apply_damage_to_current_player;
use super::components::{current_player_object_id, entity_position, player_entity};
use super::monsters::{allocate_runtime_monster_object_id, deterministic_roll};
use super::movement::point_in_bounds;
use super::resources::{is_in_world, MapRuntimeResource, RuntimeConfigResource};
use super::session::SimulationSession;

impl SimulationSession {
    /// The active map's shared hazard flags `(lightning, fire, lightning_damage,
    /// fire_damage)`, for the zone to drive authoritative lightning/fire across
    /// every co-located player. `None` when the map has no hazards.
    pub fn current_map_hazard_config(&self) -> Option<(bool, bool, i32, i32)> {
        if !is_in_world(self.app.world()) {
            return None;
        }
        current_map_hazard(self.app.world()).map(|hazard| {
            (
                hazard.lightning,
                hazard.fire,
                hazard.lightning_damage,
                hazard.fire_damage,
            )
        })
    }
}

/// Crystal reschedules each hazard `Random(3000, 15000)` ms ahead; one tick is
/// 1000 ms so the interval is 3–15 ticks.
const HAZARD_MIN_INTERVAL_TICKS: u64 = 3;
const HAZARD_INTERVAL_SPAN_TICKS: u64 = 12;

#[derive(Resource, Debug, Clone, Default)]
pub(super) struct MapHazardResource {
    pub next_lightning_tick: u64,
    pub next_fire_tick: u64,
    pub lightning_strikes: u64,
    pub fire_strikes: u64,
}

impl MapHazardResource {
    /// Reset hazard state when the active map changes so a new hazard map does
    /// not inherit a stale schedule or strike counter.
    pub(super) fn reset(&mut self) {
        self.next_lightning_tick = 0;
        self.next_fire_tick = 0;
        self.lightning_strikes = 0;
        self.fire_strikes = 0;
    }
}

struct HazardConfig {
    lightning: bool,
    fire: bool,
    lightning_damage: i32,
    fire_damage: i32,
}

fn current_map_hazard(world: &World) -> Option<HazardConfig> {
    let current_map = super::map::normalize_map_file_name(
        &world.resource::<MapRuntimeResource>().current_map.file_name,
    );
    world
        .resource::<RuntimeConfigResource>()
        .config
        .map_hazards
        .iter()
        .find(|hazard| super::map::normalize_map_file_name(&hazard.map_file_name) == current_map)
        .map(|hazard| HazardConfig {
            lightning: hazard.lightning,
            fire: hazard.fire,
            lightning_damage: hazard.lightning_damage,
            fire_damage: hazard.fire_damage,
        })
}

pub(super) fn tick_map_hazards(world: &mut World, tick: u64, packets: &mut Vec<ServerPacket>) {
    let Some(hazard) = current_map_hazard(world) else {
        return;
    };

    if hazard.lightning && tick >= world.resource::<MapHazardResource>().next_lightning_tick {
        let interval = HAZARD_MIN_INTERVAL_TICKS
            + deterministic_roll(tick, 0xA1, 0, HAZARD_INTERVAL_SPAN_TICKS);
        let strike_index = {
            let mut hazards = world.resource_mut::<MapHazardResource>();
            hazards.next_lightning_tick = tick.saturating_add(interval);
            let index = hazards.lightning_strikes;
            hazards.lightning_strikes = index.wrapping_add(1);
            index
        };
        strike(
            world,
            tick,
            Spell::MapLightning,
            hazard.lightning_damage,
            strike_index,
            0x11,
            packets,
        );
    }

    if hazard.fire && tick >= world.resource::<MapHazardResource>().next_fire_tick {
        let interval = HAZARD_MIN_INTERVAL_TICKS
            + deterministic_roll(tick, 0xF1, 0, HAZARD_INTERVAL_SPAN_TICKS);
        let strike_index = {
            let mut hazards = world.resource_mut::<MapHazardResource>();
            hazards.next_fire_tick = tick.saturating_add(interval);
            let index = hazards.fire_strikes;
            hazards.fire_strikes = index.wrapping_add(1);
            index
        };
        strike(
            world,
            tick,
            Spell::MapLava,
            hazard.fire_damage,
            strike_index,
            0x22,
            packets,
        );
    }
}

fn strike(
    world: &mut World,
    tick: u64,
    spell: Spell,
    max_damage: i32,
    strike_index: u64,
    salt: usize,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(player) = player_entity(world) else {
        return;
    };
    let Some(player_position) = entity_position(world, player) else {
        return;
    };
    let object_id = current_player_object_id(world).unwrap_or_default();

    // 1-in-4 strikes land directly on a player (Crystal `Random(4) == 0`);
    // otherwise a random cell within ±10 tiles. A per-strike counter keeps the
    // 25% rate deterministic and decoupled from the strike interval.
    let on_player = strike_index % 4 == 0;
    let location = if on_player {
        player_position.clone()
    } else {
        let dx = deterministic_roll(tick, object_id as usize, salt + 1, 20) as i32 - 10;
        let dy = deterministic_roll(tick, object_id as usize, salt + 2, 20) as i32 - 10;
        Point {
            x: player_position.x + dx,
            y: player_position.y + dy,
        }
    };

    if !hazard_point_valid(world, &location) {
        return;
    }

    packets.push(ServerPacket::ObjectSpell {
        info: ObjectSpellInfo {
            object_id: allocate_runtime_monster_object_id(world),
            location: location.clone(),
            spell,
            direction: MirDirection::Up,
            param: false,
        },
    });

    if location == player_position {
        let damage = deterministic_roll(
            tick,
            object_id as usize,
            salt + 3,
            i64::from(max_damage.max(1)) as u64,
        ) as i32;
        apply_damage_to_current_player(world, damage, packets);
    }
}

fn hazard_point_valid(world: &World, point: &Point) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    point_in_bounds(&map.map_region_bounds, point)
        && !map.blocked_cells.contains(&(point.x, point.y))
        && !map.closed_door_cells.contains(&(point.x, point.y))
}

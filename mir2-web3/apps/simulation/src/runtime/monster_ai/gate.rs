//! Crystal `Gate` (AI 81) — a passive structural castle door, NOT a combatant.
//!
//! Port of `Server/MirObjects/Monsters/Gate.cs` (+ its `CastleGate` base).
//! The Gate is immobile, never attacks, never chases, and — outside an active
//! conquest war — is invulnerable (`Attacked` forces `damage = 0` when
//! `!Conquest.WarIsOn`, `Gate.cs:81,90`). While Closed it lays down a
//! `BlockArray` of cells around itself that block movement (`Gate.cs:11-72`,
//! `CastleGate.ActiveDoorWall(true)`).
//!
//! What this module reproduces faithfully now (the achievable core):
//!   * immobile + non-attacking (pinned `next_move_tick` / `next_attack_tick`,
//!     `can_wander = false`, `tracking_player = false`);
//!   * the Sabuk-Door (`info.Effect == 1`) `BlockArray` placed once on first
//!     tick into the map's collision-blocking set while the gate is Closed —
//!     the sim analog of `ActiveDoorWall(true)`;
//!   * `CheckDirection`: facing recomputed from
//!     `GetDamageLevel = round(3 * HP / MaxHP)` clamped `>= 1`,
//!     `Direction = (MirDirection)(3 - level)`, broadcasting `ObjectTurn` on a
//!     change (`Gate.cs:107-148`);
//!   * HP regen disabled (`CanRegen => false`, `Gate.cs:150-156`) — the AI pass
//!     never regenerates a gate because this handler always intercepts the tick.
//!
//! Invulnerability is enforced one layer up: AI 81 is added to
//! `ignores_monster_damage` (see `otherTableEdits`), which is the faithful stub
//! for `!Conquest.WarIsOn => damage = 0` — the sim has no conquest war state, so
//! `!WarIsOn` is always true and the gate always takes 0 damage.
//!
//! Deferred (conquest-subsystem-gated, no Crystal packet in the sim): the full
//! open/close (`OpenDoor`/`CloseDoor`, `Gate.cs:95-129`), `RepairGate`
//! (`Gate.cs:131-139`), tax, and the guild-owner damage exception
//! (`attacker.MyGuild == GuildInfo.Owner`, `Gate.cs:81`).

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::components::{
    Facing, MonsterAgent, MonsterAiState, MonsterVitals, entity_object_id,
};
use super::super::resources::MapRuntimeResource;

/// Sabuk Door (`info.Effect == 1`) `BlockArray` — the cells, relative to the
/// gate's own tile, that block movement while the gate is Closed
/// (`Gate.cs:13-25`). SabukGate (image 950) is the only AI-81 monster in the
/// Crystal manifest and it is `Effect == 1`, so this is the only block layout
/// the sim ever needs; the other effects (`Gate.cs:26-68`) are West/South/East
/// castle `Gi` which never reach AI 81 here.
const SABUK_DOOR_BLOCK_OFFSETS: [(i32, i32); 8] = [
    (0, -1),
    (0, -2),
    (1, -1),
    (1, -2),
    (-1, 0),
    (-2, 0),
    (-1, -1),
    (-1, 1),
];

/// Crystal `Gate.GetDamageLevel` (`Gate.cs:141-148`):
/// `round(3 * HP / MaxHP)`, clamped to a minimum of 1.
fn gate_damage_level(hp: i32, max_hp: i32) -> i32 {
    if max_hp <= 0 {
        return 1;
    }
    let level = ((3.0 * f64::from(hp.max(0))) / f64::from(max_hp)).round() as i32;
    level.max(1)
}

/// Crystal `Gate.CheckDirection` (`Gate.cs:107-116`): the gate "leans / cracks"
/// as it loses HP. `Direction = (MirDirection)(3 - GetDamageLevel())`; at full
/// HP `level == 3` → `Up`, mid → `UpRight`, near death → `Right`.
fn gate_closed_facing(hp: i32, max_hp: i32) -> MirDirection {
    let raw = (3 - gate_damage_level(hp, max_hp)).clamp(0, 7) as u8;
    MirDirection::try_from(raw).unwrap_or(MirDirection::Up)
}

pub(in crate::runtime) fn update_gate_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    // A dead gate is handled (no default melee/chase): the wall stays whatever
    // the map collision currently holds; full revive/repair is conquest-gated.
    if agent.dead {
        return true;
    }

    // Immobile + non-combatant: pin the movement/attack clocks forward and
    // never roam or track, mirroring the passive-structure handlers
    // (`yin_devil_node`, `stone_trap`). `monster_can_attack`/`monster_can_chase`
    // already exclude AI 81 (see `otherTableEdits`), this is belt-and-braces.
    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;
    agent.next_attack_tick = tick + 1;

    // On first tick, lay down the Closed gate's BlockArray as collision. The
    // sim's `closed_door_cells` set is exactly what the movement code treats as
    // blocked (`movement.rs:481`), so it is the faithful analog of Crystal's
    // `ActiveDoorWall(true)`. `ai_state.extra` records "wall placed / Closed".
    if !ai_state.extra {
        let cells: Vec<(i32, i32)> = SABUK_DOOR_BLOCK_OFFSETS
            .iter()
            .map(|(dx, dy)| (position.x + dx, position.y + dy))
            .collect();
        let mut map = world.resource_mut::<MapRuntimeResource>();
        for cell in cells {
            map.closed_door_cells.insert(cell);
        }
        ai_state.extra = true;
    }

    // CheckDirection: recompute the Closed facing from current HP and broadcast
    // ObjectTurn if it changed (`Gate.cs:107-116`). HP regen is never applied —
    // this handler always intercepts, so the default vital-regen path can never
    // run for a gate (`CanRegen => false`).
    let (hp, max_hp) = world
        .entity(entity)
        .get::<MonsterVitals>()
        .map(|vitals| (vitals.hp, vitals.max_hp))
        .unwrap_or((1, 1));
    let new_direction = gate_closed_facing(hp, max_hp);
    let current_direction = world.entity(entity).get::<Facing>().map(|facing| facing.0);
    if current_direction != Some(new_direction) {
        world.entity_mut(entity).insert(Facing(new_direction));
        if let Some(object_id) = entity_object_id(world, entity) {
            packets.push(ServerPacket::ObjectTurn {
                movement: ObjectMovement {
                    object_id,
                    position: position.clone(),
                    direction: new_direction,
                },
            });
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::super::super::components::{Monster, ObjectId, Position, WorldObject};
    use super::super::super::monsters::crystal_dynamic_monster_template;
    use super::super::super::resources::DoorRegistry;
    use super::*;
    use crate::{SimulationConfig, WorldEntityDisposition};
    use mir2_game_data::MapBounds;
    use std::collections::{BTreeMap, BTreeSet};

    /// Minimal world carrying only what `update_gate_state` touches: the entity's
    /// monster components and a `MapRuntimeResource` (for the BlockArray cells).
    fn world_with_map() -> World {
        let mut world = World::new();
        let config = SimulationConfig::default();
        let bounds = MapBounds {
            min_x: 0,
            max_x: 1000,
            min_y: 0,
            max_y: 1000,
        };
        world.insert_resource(MapRuntimeResource::new(
            &config,
            bounds,
            BTreeSet::new(),
            BTreeSet::new(),
            DoorRegistry::default(),
            BTreeMap::new(),
        ));
        world
    }

    /// Spawn the gate from the "SabukGate" Crystal template (manifest AI 81),
    /// then pin `ai == 81` so the dispatch contract routes to this module even
    /// if the manifest drifts (HARD RULE 5). Returns the entity + its position.
    fn spawn_sabuk_gate(world: &mut World, position: Point) -> Entity {
        let template =
            crystal_dynamic_monster_template("SabukGate").expect("SabukGate template should exist");
        world
            .spawn((
                WorldObject,
                Monster,
                ObjectId(9_100),
                Position(position.clone()),
                Facing(MirDirection::Down),
                MonsterAgent {
                    image: template.monster_image,
                    dead: false,
                    patrol_origin: position,
                    ai: 81,
                    disposition: WorldEntityDisposition::Neutral,
                    hostile_to_player: false,
                    tracking_player: true,
                    view_range: i32::from(template.monster_view_range),
                    can_wander: true,
                    move_interval_ticks: 1,
                    attack_interval_ticks: 1,
                    next_move_tick: 0,
                    next_attack_tick: 0,
                    route: Vec::new(),
                    route_index: 0,
                    route_waiting: false,
                    next_route_tick: 0,
                },
                MonsterAiState::default(),
                MonsterVitals {
                    hp: template.monster_hp.max(1),
                    max_hp: template.monster_hp.max(1),
                },
            ))
            .id()
    }

    #[test]
    fn gate_is_immobile_blocks_cells_and_faces_up_at_full_hp() {
        let mut world = world_with_map();
        let gate_position = Point { x: 60, y: 60 };
        let entity = spawn_sabuk_gate(&mut world, gate_position.clone());

        let mut agent = world.entity(entity).get::<MonsterAgent>().unwrap().clone();
        let mut ai_state = world
            .entity(entity)
            .get::<MonsterAiState>()
            .copied()
            .unwrap();
        let mut packets = Vec::new();

        let tick = 10;
        let handled = update_gate_state(
            &mut world,
            entity,
            &mut agent,
            &mut ai_state,
            &gate_position,
            tick,
            &mut packets,
        );

        // Handled this tick (no fall-through to default melee/chase) + immobile.
        assert!(handled, "gate must intercept its own AI tick");
        assert!(!agent.tracking_player, "gate never tracks the player");
        assert!(!agent.can_wander, "gate never wanders");
        assert!(
            agent.next_move_tick > tick && agent.next_attack_tick > tick,
            "gate pins its move/attack clocks forward"
        );

        // The Sabuk-Door BlockArray is now collision (ActiveDoorWall analog):
        // (0, -1) relative to the gate must be blocked.
        let map = world.resource::<MapRuntimeResource>();
        assert!(
            map.closed_door_cells
                .contains(&(gate_position.x, gate_position.y - 1)),
            "gate must block its BlockArray cells while closed"
        );
        assert!(
            map.closed_door_cells
                .contains(&(gate_position.x - 2, gate_position.y)),
            "gate must block the full BlockArray, not just one cell"
        );

        // At full HP (5000/5000) GetDamageLevel == 3 → Direction = Up, and an
        // ObjectTurn is broadcast because the spawn faced Down.
        assert_eq!(
            world.entity(entity).get::<Facing>().unwrap().0,
            MirDirection::Up,
            "full-HP gate faces Up"
        );
        assert!(
            packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::ObjectTurn { .. })),
            "facing change must broadcast ObjectTurn"
        );
        // ai_state.extra latches the wall-placed/Closed state.
        assert!(ai_state.extra, "gate marks its wall as placed");
    }

    #[test]
    fn gate_facing_tracks_damage_level() {
        // round(3 * HP / MaxHP), clamped >= 1; Direction = (MirDirection)(3 - level).
        assert_eq!(gate_closed_facing(5000, 5000), MirDirection::Up); // level 3
        assert_eq!(gate_closed_facing(2500, 5000), MirDirection::UpRight); // round(1.5) = 2
        assert_eq!(gate_closed_facing(1, 5000), MirDirection::Right); // clamps to level 1
        assert_eq!(gate_closed_facing(0, 5000), MirDirection::Right); // clamps to level 1
    }
}

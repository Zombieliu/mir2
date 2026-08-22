//! Crystal `TreeQueen` (AI 142) — an immobile boss that bombards the whole map
//! with root ground-fields whenever a player is present, and only melees a
//! player who steps within 2 tiles.
//!
//! Port of `Crystal/Server/MirObjects/Monsters/TreeQueen.cs`:
//!   * `CanMove == false`, `Direction = Up`, `ApplyPoison` is a no-op, `Walk`
//!     and `Turn` are no-ops (immobility + poison immunity — see the
//!     `monsters.rs` table edits the coordinator applies for AI 142).
//!   * `Spawned()` arms two timers: the root timer at `Time + 5s`
//!     (`_rootSpawnTime`) and the ground-root timer at `Time + 15s`
//!     (`_groundRootSpawnTime`).
//!   * `ProcessTarget()` (TreeQueen.cs:298) runs both timers whenever
//!     `CurrentMap.Players.Count > 0` (no melee target required), then melees
//!     when a target is in range.
//!
//! Crystal randomness is reproduced with the deterministic roll/chance helpers
//! salted by the AI id (142) so replays stay stable.
//!
//! Field damage plumbing: Crystal spawns `SpellObject`s (TreeQueenRoot /
//! TreeQueenMassRoots / TreeQueenGroundRoots) that tick and damage whatever
//! shares their cell. This port emits the matching `ObjectSpell` visuals for the
//! client and schedules the field's damage directly against the player when the
//! player occupies a struck tile (the same approach `tucson_general` uses for
//! its rage rocks). See `deferredParts` in the port notes for the persistent
//! field-vs-player tick that the shared `tick_ground_spell_actions` does not yet
//! drive for monster casters.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{
    MirDirection, ObjectRangeAttackInfo, ObjectSpellInfo, Point, ServerPacket, Spell,
};

use super::super::combat::*;
use super::super::components::{
    Facing, MonsterAgent, MonsterAiState, entity_name, entity_object_id, entity_position,
    player_entity,
};
use super::super::monsters::*;
use super::super::movement::*;

/// Crystal `_rootSpreadMin` / `_rootSpreadMax` (TreeQueen.cs:13-14).
const ROOT_SPREAD_MIN: i32 = 5;
const ROOT_SPREAD_MAX: i32 = 15;
/// Crystal `_rootCount` (TreeQueen.cs:15) — used as `Random(1, _rootCount)`.
const ROOT_COUNT: u64 = 5;
/// Crystal `_groundrootSpread` (TreeQueen.cs:18).
const GROUND_ROOT_SPREAD: i32 = 5;
/// Crystal `_nearMultiplier` (TreeQueen.cs:21).
const NEAR_MULTIPLIER: u64 = 4;

/// Salt bases (all xored with the AI id by the deterministic helpers) keeping
/// each random draw independent and replay-stable.
const SALT_ROOT_BRANCH: u64 = 0x7142_0001;
const SALT_ROOT_COUNT: u64 = 0x7142_0002;
const SALT_ROOT_DISTANCE: u64 = 0x7142_0003;
const SALT_ROOT_OFFSET_X: u64 = 0x7142_0100;
const SALT_ROOT_OFFSET_Y: u64 = 0x7142_0200;
const SALT_ROOT_DIRECT_HIT: u64 = 0x7142_0300;
const SALT_ROOT_START: u64 = 0x7142_0400;
const SALT_ROOT_RESCHEDULE: u64 = 0x7142_0005;
const SALT_GROUND_ROOT_OFFSET_X: u64 = 0x7142_1000;
const SALT_GROUND_ROOT_OFFSET_Y: u64 = 0x7142_2000;
const SALT_GROUND_ROOT_START: u64 = 0x7142_3000;
const SALT_GROUND_ROOT_RESCHEDULE: u64 = 0x7142_0006;
const SALT_MELEE_BRANCH: u64 = 0x7142_0007;

/// Crystal `Spawned()` timer offsets (TreeQueen.cs:292-293): root at +5s,
/// ground-root at +15s.
fn root_timer_initial(tick: u64) -> u64 {
    tick + combat_delay_ticks(5_000)
}

fn ground_root_timer_initial(tick: u64) -> u64 {
    tick + combat_delay_ticks(15_000)
}

/// `update_special_monster_state` dispatch entry for AI 142.
///
/// Returns `true` whenever the TreeQueen owns the tick (it always does while a
/// player is in the world), so the default melee/chase path is skipped — the
/// boss is immobile and runs its own bombardment + proximity-melee logic.
pub(in crate::runtime) fn update_tree_queen_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    // Immobile + faces Up regardless of state (Crystal `Direction = Up`,
    // `CanMove == false`).
    agent.can_wander = false;
    agent.tracking_player = false;
    agent.next_move_tick = agent.next_move_tick.max(tick);
    if world.entity(entity).get::<Facing>().map(|facing| facing.0) != Some(MirDirection::Up) {
        world.entity_mut(entity).insert(Facing(MirDirection::Up));
    }

    if agent.dead {
        return true;
    }

    // Crystal `Spawned()` arms the two timers once.
    if !ai_state.extra {
        ai_state.extra = true;
        ai_state.mode = true; // `_notNear = true`
        ai_state.next_state_tick = root_timer_initial(tick);
        agent.next_move_tick = ground_root_timer_initial(tick);
        return true;
    }

    // Crystal `ProcessTarget()`: bail entirely while no player is on the map.
    let Some(player) = player_entity(world) else {
        return true;
    };
    let Some(attacker_id) = entity_object_id(world, entity) else {
        return true;
    };
    let monster_name = entity_name(world, entity).unwrap_or_else(|| "TreeQueen".to_string());

    // --- Root timer (TreeQueen.cs:302-316) ---
    if tick > ai_state.next_state_tick {
        if deterministic_chance_roll_nonzero(tick, attacker_id, SALT_ROOT_BRANCH, 4) {
            // `Random(4) > 0` (3/4): per-player scattered roots.
            spawn_roots(
                world,
                attacker_id,
                &monster_name,
                player_position,
                tick,
                packets,
            );
        } else {
            // `Random(4) == 0` (1/4): a 7x7 mass-root field on a player.
            spawn_mass_roots(
                world,
                entity,
                attacker_id,
                &monster_name,
                position,
                player,
                player_position,
                tick,
                packets,
            );
        }

        // `Random(1, 4)` => {1,2,3} seconds, x4 when the player is near.
        let next_seconds =
            1 + deterministic_roll(tick, attacker_id as usize, SALT_ROOT_RESCHEDULE as usize, 3);
        let multiplier = if ai_state.mode { 1 } else { NEAR_MULTIPLIER };
        ai_state.next_state_tick = tick + combat_delay_ticks(next_seconds * multiplier * 1_000);
    }

    // --- Ground-root timer (TreeQueen.cs:318-325) ---
    if tick > agent.next_move_tick {
        spawn_ground_roots(
            world,
            attacker_id,
            &monster_name,
            position,
            player,
            tick,
            packets,
        );

        // `Random(2, 3)` => {2} seconds, x4 when the player is near.
        let next_seconds = 2 + deterministic_roll(
            tick,
            attacker_id as usize,
            SALT_GROUND_ROOT_RESCHEDULE as usize,
            1,
        );
        let multiplier = if ai_state.mode { 1 } else { NEAR_MULTIPLIER };
        agent.next_move_tick = tick + combat_delay_ticks(next_seconds * multiplier * 1_000);
    }

    // --- Direct melee (TreeQueen.cs:327-333 + Attack()) ---
    // `ranged = same cell || !InRange(2)`; the direct attack only fires when the
    // target is "not ranged" (within 2 tiles and not on the same cell).
    let distance = tile_distance(position, player_position);
    let not_ranged = distance != 0 && distance <= 2;
    if not_ranged && tick >= agent.next_attack_tick {
        ai_state.mode = false; // `_notNear = false`
        agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
        tree_queen_melee(
            world,
            entity,
            attacker_id,
            &monster_name,
            position,
            player,
            player_position,
            tick,
            packets,
        );
    } else if distance == 0 || distance > 2 {
        // Crystal sets `_notNear = true` on the ranged branch of Attack().
        ai_state.mode = true;
    }

    true
}

/// Crystal `SpawnRoots()` (TreeQueen.cs:135-184): for the (single) player, drop
/// `Random(1,_rootCount)` `TreeQueenRoot` fields scattered within
/// `Random(min,max)` tiles; each field has a 1/3 chance to land directly on the
/// player (which also stops further roots for that player). Damage is
/// `Random(Random(MinMC,MaxMC))` (MC). MinMC/MaxMC are 0 for the DB row, so the
/// fields are visual unless the table grows MC.
fn spawn_roots(
    world: &mut World,
    attacker_id: u32,
    monster_name: &str,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    // `Random(1, _rootCount)` => {1,2,3,4}
    let count = 1 + deterministic_roll(
        tick,
        attacker_id as usize,
        SALT_ROOT_COUNT as usize,
        ROOT_COUNT - 1,
    );
    // `Random(_rootSpreadMin, _rootSpreadMax)` => {5..14}
    let distance = ROOT_SPREAD_MIN
        + deterministic_roll(
            tick,
            attacker_id as usize,
            SALT_ROOT_DISTANCE as usize,
            u64::try_from(ROOT_SPREAD_MAX - ROOT_SPREAD_MIN).unwrap_or(1),
        ) as i32;
    let span = u64::try_from(distance * 2 + 1).unwrap_or(1);
    let mc_damage = crystal_monster_magic_damage(monster_name);

    for i in 0..count {
        let slot = i as usize;
        // `Random(-distance, distance + 1)` => [-distance, distance].
        let mut location = Point {
            x: player_position.x
                + deterministic_roll(
                    tick,
                    attacker_id as usize,
                    SALT_ROOT_OFFSET_X as usize + slot,
                    span,
                ) as i32
                - distance,
            y: player_position.y
                + deterministic_roll(
                    tick,
                    attacker_id as usize,
                    SALT_ROOT_OFFSET_Y as usize + slot,
                    span,
                ) as i32
                - distance,
        };

        // `Random(3) == 0` (1/3): land on the player and stop spawning roots.
        let direct_hit =
            !deterministic_chance_roll_nonzero(tick, attacker_id, SALT_ROOT_DIRECT_HIT + i, 3);
        if direct_hit {
            location = player_position.clone();
        }

        let location = clamp_to_map_region(world, location);

        // `Random(2000)` stagger before the field appears.
        let start_ms = deterministic_roll(
            tick,
            attacker_id as usize,
            SALT_ROOT_START as usize + slot,
            2_000,
        );
        let spawn_tick = tick + combat_delay_ticks(start_ms);

        emit_root_field(
            world,
            Spell::TreeQueenRoot,
            location.clone(),
            spawn_tick,
            attacker_id,
            mc_damage,
            monster_name,
            player_position,
            packets,
        );

        if direct_hit {
            break;
        }
    }
}

/// Crystal `SpawnMassRoots()` (TreeQueen.cs:186-237): broadcast an
/// `ObjectRangeAttack Type 1` at a random player, then carpet a 7x7
/// `TreeQueenMassRoots` field centred on that player. Damage is
/// `GetAttackPower(MinMC, MaxMC)` (MC).
#[allow(clippy::too_many_arguments)]
fn spawn_mass_roots(
    world: &mut World,
    entity: Entity,
    attacker_id: u32,
    monster_name: &str,
    position: &Point,
    player: Entity,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(target_id) = entity_object_id(world, player) else {
        return;
    };
    if let Some(packet) =
        mass_roots_range_packet(world, entity, position, target_id, player_position)
    {
        packets.push(packet);
    }

    let mc_damage = crystal_monster_magic_damage(monster_name);
    // Crystal uses a fixed `start = 500` for every cell of the mass field.
    let spawn_tick = tick + combat_delay_ticks(500);

    for dy in -3..=3 {
        for dx in -3..=3 {
            let cell = Point {
                x: player_position.x + dx,
                y: player_position.y + dy,
            };
            if cell == *position {
                continue; // Crystal skips the caster's own cell.
            }
            let cell = clamp_to_map_region(world, cell);
            emit_root_field(
                world,
                Spell::TreeQueenMassRoots,
                cell,
                spawn_tick,
                attacker_id,
                mc_damage,
                monster_name,
                player_position,
                packets,
            );
        }
    }
}

/// Crystal `SpawnGroundRoots()` (TreeQueen.cs:239-286): pick a 5x5 region near
/// self (`Random(-spread, spread+1)`) and carpet a `TreeQueenGroundRoots` field,
/// each cell staggered up to 4000ms. Damage is `GetAttackPower(MinDC, MaxDC)`
/// (DC) — this is the field that actually hurts for the DB row.
fn spawn_ground_roots(
    world: &mut World,
    attacker_id: u32,
    monster_name: &str,
    position: &Point,
    player: Entity,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let span = u64::try_from(GROUND_ROOT_SPREAD * 2 + 1).unwrap_or(1);
    // `Random(-_groundrootSpread, _groundrootSpread + 1)` => [-spread, spread].
    let center = Point {
        x: position.x
            + deterministic_roll(
                tick,
                attacker_id as usize,
                SALT_GROUND_ROOT_OFFSET_X as usize,
                span,
            ) as i32
            - GROUND_ROOT_SPREAD,
        y: position.y
            + deterministic_roll(
                tick,
                attacker_id as usize,
                SALT_GROUND_ROOT_OFFSET_Y as usize,
                span,
            ) as i32
            - GROUND_ROOT_SPREAD,
    };

    let dc_damage = crystal_monster_attack_damage(monster_name);
    let player_position = entity_position(world, player);

    for dy in -2..=2 {
        for dx in -2..=2 {
            let cell = Point {
                x: center.x + dx,
                y: center.y + dy,
            };
            if cell == *position {
                continue; // Crystal skips the caster's own cell.
            }
            let cell = clamp_to_map_region(world, cell);

            // `Random(4000)` per-cell stagger (TreeQueen.cs:266).
            let slot = usize::try_from((dy + 2) * 5 + (dx + 2)).unwrap_or(0);
            let start_ms = deterministic_roll(
                tick,
                attacker_id as usize,
                SALT_GROUND_ROOT_START as usize + slot,
                4_000,
            );
            let spawn_tick = tick + combat_delay_ticks(start_ms);

            if let Some(packet) =
                root_spell_packet(world, Spell::TreeQueenGroundRoots, cell.clone(), spawn_tick)
            {
                packets.push(packet);
            }
            schedule_field_damage_if_on_cell(
                world,
                &cell,
                player_position.as_ref(),
                spawn_tick,
                attacker_id,
                monster_name,
                dc_damage,
            );
        }
    }
}

/// Crystal `Attack()` direct branch (TreeQueen.cs:54-100) — fires only when the
/// player is within 2 (and not on the caster's cell):
///   * `Random(2) > 0` (1/2): Type 0 "fire bombardment", DC vs MACAgility to
///     `FindAllTargets(3)` after 500ms.
///   * else: Type 1 "push", DC vs MACAgility to `FindAllTargets(1)` after 500ms,
///     pushing each target 5 tiles away.
#[allow(clippy::too_many_arguments)]
fn tree_queen_melee(
    world: &mut World,
    entity: Entity,
    attacker_id: u32,
    monster_name: &str,
    position: &Point,
    player: Entity,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let dc_damage = crystal_monster_attack_damage(monster_name);
    let due_tick = tick + combat_delay_ticks(500);
    let fire_bombardment =
        deterministic_chance_roll_nonzero(tick, attacker_id, SALT_MELEE_BRANCH, 2);

    if fire_bombardment {
        // Type 0 broadcast (Crystal `ObjectAttack { Type = 0 }`).
        if let Some(packet) =
            monster_typed_attack_packet(world, entity, position, MirDirection::Up, 0)
        {
            packets.push(packet);
        }
        if dc_damage > 0 && tile_distance(position, player_position) <= 3 {
            schedule_damage_to_player(
                world,
                due_tick,
                attacker_id,
                monster_name.to_string(),
                dc_damage,
            );
        }
    } else {
        // Type 1 broadcast (Crystal `ObjectAttack { Type = 1 }`).
        if let Some(packet) =
            monster_typed_attack_packet(world, entity, position, MirDirection::Up, 1)
        {
            packets.push(packet);
        }
        if tile_distance(position, player_position) <= 1 {
            if dc_damage > 0 {
                schedule_damage_to_player(
                    world,
                    due_tick,
                    attacker_id,
                    monster_name.to_string(),
                    dc_damage,
                );
            }
            // Crystal pushes each target away from the caster, distance 5.
            if let Some(push_direction) = direction_toward(position, player_position) {
                let _ = push_player_in_direction(world, player, push_direction, 5, packets);
            }
        }
    }
}

/// Emit a single root field: the client-visible `ObjectSpell` plus the scheduled
/// player damage if the player shares the struck cell at impact time.
#[allow(clippy::too_many_arguments)]
fn emit_root_field(
    world: &mut World,
    spell: Spell,
    location: Point,
    spawn_tick: u64,
    attacker_id: u32,
    damage: i32,
    monster_name: &str,
    player_position: &Point,
    packets: &mut Vec<ServerPacket>,
) {
    if let Some(packet) = root_spell_packet(world, spell, location.clone(), spawn_tick) {
        packets.push(packet);
    }
    schedule_field_damage_if_on_cell(
        world,
        &location,
        Some(player_position),
        spawn_tick,
        attacker_id,
        monster_name,
        damage,
    );
}

/// Build the staggered `ObjectSpell` visual for a root field tile.
fn root_spell_packet(
    world: &mut World,
    spell: Spell,
    location: Point,
    spawn_tick: u64,
) -> Option<ServerPacket> {
    let packet = ServerPacket::ObjectSpell {
        info: ObjectSpellInfo {
            object_id: allocate_runtime_monster_object_id(world),
            location,
            spell,
            direction: MirDirection::Up,
            param: false,
        },
    };
    queue_due_packet(world, spawn_tick, packet);
    None
}

/// Crystal's `SpellObject` damages whatever shares its cell when it ticks; this
/// port schedules that damage against the player when the player occupies the
/// struck tile at impact time (mirrors `tucson_general`'s rock impacts).
fn schedule_field_damage_if_on_cell(
    world: &mut World,
    cell: &Point,
    player_position: Option<&Point>,
    spawn_tick: u64,
    attacker_id: u32,
    monster_name: &str,
    damage: i32,
) {
    if damage <= 0 {
        return;
    }
    let Some(player_position) = player_position else {
        return;
    };
    if player_position == cell {
        schedule_damage_to_player(
            world,
            spawn_tick,
            attacker_id,
            monster_name.to_string(),
            damage,
        );
    }
}

/// Crystal `SpawnMassRoots` broadcast `ObjectRangeAttack { Type = 1 }`.
fn mass_roots_range_packet(
    world: &World,
    entity: Entity,
    position: &Point,
    target_id: u32,
    target: &Point,
) -> Option<ServerPacket> {
    Some(ServerPacket::ObjectRangeAttack {
        info: ObjectRangeAttackInfo {
            object_id: entity_object_id(world, entity)?,
            location: position.clone(),
            direction: MirDirection::Up,
            target_id,
            target: target.clone(),
            attack_type: 1,
            spell: 0,
            level: 0,
        },
    })
}

/// `Envir.Random.Next(n) > 0` — true with probability `(n-1)/n`. Implemented via
/// the deterministic chance helper, which returns true on the `== 0` slot, so we
/// invert it.
fn deterministic_chance_roll_nonzero(
    current_tick: u64,
    attacker_id: u32,
    salt: u64,
    denominator: u64,
) -> bool {
    !deterministic_chance_roll(current_tick, attacker_id, salt, denominator)
}

#[cfg(test)]
mod tests {
    use super::super::super::components::{
        DisplayName, Monster, MonsterCombatStats, MonsterVitals, ObjectId, Position, WorldObject,
        entity_object_id,
    };
    use super::super::super::monsters::{
        crystal_dynamic_monster_template, crystal_respawn_can_wander,
    };
    use super::super::super::session::SimulationSession;
    use super::*;
    use crate::{SimulationConfig, WorldEntityDisposition};
    use mir2_protocol::ClientPacket;

    /// Spawn a real "TreeQueen" template (manifest AI == 142) the same way
    /// `spawn_crystal_monster_for_test` does, so the agent that reaches the AI
    /// tick is genuinely AI 142 and routes to `update_tree_queen_state`.
    ///
    /// Asserts the key Crystal contract: the boss intercepts its own tick
    /// (`true`), is immobile (`CanMove == false` → never wanders/chases), faces
    /// Up (`Direction = Up`), and `Spawned()` arms BOTH bombardment timers (root
    /// at +5s, ground-root at +15s — TreeQueen.cs:289-296).
    #[test]
    fn tree_queen_is_immobile_faces_up_and_arms_timers() {
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        let player = player_entity(session.app.world()).expect("player entity");
        let player_origin = Point { x: 900, y: 900 };
        session
            .app
            .world_mut()
            .entity_mut(player)
            .insert(Position(player_origin.clone()));

        let template = crystal_dynamic_monster_template("TreeQueen").expect("TreeQueen template");
        assert_eq!(
            template.monster_ai, 142,
            "TreeQueen manifest AI should be 142"
        );

        let queen_object_id = 91_420_u32;
        // Far from the player so no melee branch fires on the first tick.
        let queen_position = Point {
            x: player_origin.x + 10,
            y: player_origin.y + 10,
        };
        let tick = 100_u64;
        let mut agent = MonsterAgent {
            image: template.monster_image,
            dead: false,
            patrol_origin: queen_position.clone(),
            ai: template.monster_ai,
            disposition: WorldEntityDisposition::Hostile,
            hostile_to_player: true,
            tracking_player: true,
            view_range: i32::from(template.monster_view_range),
            can_wander: crystal_respawn_can_wander(template.monster_hp),
            move_interval_ticks: 1,
            attack_interval_ticks: 1,
            next_move_tick: tick,
            next_attack_tick: tick,
            route: Vec::new(),
            route_index: 0,
            route_waiting: false,
            next_route_tick: tick,
        };

        let entity = session
            .app
            .world_mut()
            .spawn((
                WorldObject,
                Monster,
                ObjectId(queen_object_id),
                DisplayName::literal(template.monster_name.clone()),
                Position(queen_position.clone()),
                Facing(MirDirection::Down),
                agent.clone(),
                MonsterAiState::default(),
                MonsterVitals {
                    hp: template.monster_hp.max(1),
                    max_hp: template.monster_hp.max(1),
                },
                MonsterCombatStats {
                    agility: template.monster_agility,
                },
            ))
            .id();
        assert_eq!(
            entity_object_id(session.app.world(), entity),
            Some(queen_object_id)
        );

        let mut ai_state = MonsterAiState::default();
        let mut packets = Vec::new();
        let handled = update_tree_queen_state(
            session.app.world_mut(),
            entity,
            &mut agent,
            &mut ai_state,
            &queen_position,
            &player_origin,
            tick,
            &mut packets,
        );

        assert!(handled, "TreeQueen must intercept its own AI tick");
        assert!(
            !agent.can_wander,
            "TreeQueen never wanders (CanMove == false)"
        );
        assert!(!agent.tracking_player, "TreeQueen never chases");
        assert!(ai_state.extra, "Spawned() must mark the timers as armed");
        assert_eq!(
            ai_state.next_state_tick,
            root_timer_initial(tick),
            "root timer arms at +5s (Settings.Second * 5)"
        );
        assert_eq!(
            agent.next_move_tick,
            ground_root_timer_initial(tick),
            "ground-root timer arms at +15s (Settings.Second * 15)"
        );

        let facing = session
            .app
            .world()
            .entity(entity)
            .get::<Facing>()
            .expect("queen facing")
            .0;
        assert_eq!(
            facing,
            MirDirection::Up,
            "TreeQueen faces Up (Direction = Up)"
        );

        // A dead TreeQueen is still "handled" (no default path) and inert.
        let mut dead_agent = agent.clone();
        dead_agent.dead = true;
        let mut dead_state = ai_state;
        assert!(update_tree_queen_state(
            session.app.world_mut(),
            entity,
            &mut dead_agent,
            &mut dead_state,
            &queen_position,
            &player_origin,
            tick + 50,
            &mut packets,
        ));
    }

    #[test]
    fn timer_offsets_match_crystal_spawned() {
        assert_eq!(root_timer_initial(0), combat_delay_ticks(5_000));
        assert_eq!(ground_root_timer_initial(0), combat_delay_ticks(15_000));
    }

    #[test]
    fn chance_roll_nonzero_inverts_zero_slot() {
        // `Random(n) > 0` is true unless the deterministic helper hits the 0 slot.
        for tick in 0..64u64 {
            assert_eq!(
                deterministic_chance_roll_nonzero(tick, 142, SALT_ROOT_BRANCH, 4),
                !deterministic_chance_roll(tick, 142, SALT_ROOT_BRANCH, 4)
            );
        }
    }
}

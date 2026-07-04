//! Monster AI: MudZombie (Crystal AI id 108).
//!
//! Faithful port of `Crystal/Server/MirObjects/Monsters/MudZombie.cs` (base
//! chain `MonsterObject.cs`). A `ViewRange`-aggro poison caster, structurally a
//! SpittingSpider/ShamanZombie hybrid:
//!
//!   * `InAttackRange` is `Functions.InRange(loc, target, Info.ViewRange)`
//!     (`MudZombie.cs:14-17`) — a Chebyshev box of radius `ViewRange`, so it can
//!     strike anywhere it can see and only chases when the target leaves view.
//!   * `Attack` (`MudZombie.cs:19-56`) splits on
//!     `ranged = loc == target || !InRange(loc, target, 2)`:
//!       - **line branch** (`!ranged`, i.e. Chebyshev 1..=2): broadcast
//!         `ObjectAttack { Type = 0 }` + `LineAttack(GetAttackPower(MinDC,MaxDC), 2)`
//!         — a 2-tile DC line (`ACAgility`, `+500ms` base delay) splashing the two
//!         tiles ahead.
//!       - **ranged branch** (same-tile or Chebyshev > 2): `AttackTime` gets an
//!         extra `+500` pause, broadcast `ObjectRangeAttack`, then a single
//!         `GetAttackPower(MinMC,MaxMC)` `RangeDamage` (`DefenceType.MAC`) at
//!         `+500ms`.
//!   * `CompleteAttack` / `CompleteRangeAttack` (`MudZombie.cs:58-82`): every hit
//!     that lands (`Attacked(...) > 0`) calls
//!     `PoisonTarget(target, 5, 8, PoisonType.Green, 2000)` — `Random.Next(5) == 0`
//!     to apply Crystal green poison for `8` duration.
//!
//! Deterministic-RNG only: the green-poison `Random.Next(5)` is realised through
//! `PendingPlayerStatusEffect::GreenPoison { chance_denominator: 5, .. }`, whose
//! resolution rolls `deterministic_chance_roll` (see
//! `super::super::combat::apply_pending_player_status_effect`). This module adds
//! no `rand`/`Math` randomness of its own.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    current_player_is_dead, entity_facing, entity_name, entity_object_id, entity_position,
    player_entity, Facing, Monster, MonsterAgent, MonsterAiState,
};
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

/// Crystal `MudZombie` AI id (`crystal_monster_manifest.json` → `"ai": 108`).
#[cfg(test)]
pub(in crate::runtime) const MUD_ZOMBIE_AI: u8 = 108;

/// Crystal `Attack`: line branch fires `LineAttack(damage, 2)` — a 2-tile line
/// (`MudZombie.cs:42`).
const MUD_ZOMBIE_LINE_DISTANCE: i32 = 2;

/// Crystal `LineAttack(damage, 2)` default `additionalDelay` (`MonsterObject.cs:3611`):
/// per-target delay is `MaxDistance(self, target) * 50 + 500` ms (`:3634`).
const MUD_ZOMBIE_LINE_BASE_DELAY_MS: u64 = 500;

/// Crystal ranged branch: extra `AttackTime = Envir.Time + AttackSpeed + 500`
/// pause and `DelayedAction(RangeDamage, Envir.Time + 500, ...)` (`MudZombie.cs:46,53`).
const MUD_ZOMBIE_RANGED_EXTRA_PAUSE_MS: u64 = 500;
const MUD_ZOMBIE_RANGED_DAMAGE_DELAY_MS: u64 = 500;

/// Crystal `PoisonTarget(target, 5, 8, PoisonType.Green, 2000)` (`MudZombie.cs:68,81`).
/// `chanceToPoison = 5` → `Random.Next(5) == 0`; `poisonDuration = 8`.
const MUD_ZOMBIE_GREEN_POISON_CHANCE_DENOMINATOR: u64 = 5;
const MUD_ZOMBIE_GREEN_POISON_DURATION_TICKS: u64 = 8;

/// Crystal `MudZombie.ProcessTarget` / `Attack` (`MudZombie.cs:19-101`).
///
/// Returns `true` when this monster's custom AI handled the tick (the dispatch
/// then skips the default melee/chase). Returns `false` only when there is no
/// valid attack target *or* the target is outside `ViewRange`, so the shared
/// chase path runs — Crystal's `MoveTo(Target.CurrentLocation)` (`MudZombie.cs:100`).
pub(in crate::runtime) fn update_mud_zombie_state(
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

    // Crystal `ProcessTarget` requires a valid attack target. Mirror the shared
    // aggro gate the other view-range casters use (tracking, or hostile + within
    // view range). A dead player is never a target (`IsAttackTarget`).
    let has_player_target = agent.tracking_player
        || (agent.hostile_to_player && mud_zombie_in_view_range(agent, position, player_position));
    if !has_player_target || current_player_is_dead(world) {
        return false;
    }

    // Crystal `InAttackRange` (`MudZombie.cs:14-17`):
    // `Functions.InRange(CurrentLocation, Target.CurrentLocation, Info.ViewRange)`.
    // Out of view → `ProcessTarget` falls to `MoveTo` → let the shared chase run.
    if !mud_zombie_in_view_range(agent, position, player_position) {
        return false;
    }

    // Crystal `Attack`: `AttackTime = Envir.Time + AttackSpeed` (`MudZombie.cs:30`).
    // Honour AttackSpeed via `next_attack_tick`; in range but still cooling down,
    // hold position (we own the tick — Crystal never re-enters MoveTo here).
    if tick < agent.next_attack_tick {
        return true;
    }

    let Some(attacker_id) = entity_object_id(world, entity) else {
        return false;
    };
    let Some(direction) = direction_toward(position, player_position) else {
        return false;
    };

    // Crystal `Attack`: `Direction = Functions.DirectionFromPoint(...)`
    // (`MudZombie.cs:27`); `ShockTime = 0` (the monster commits to the attack).
    agent.tracking_player = true;
    if entity_facing(world, entity) != Some(direction) {
        world.entity_mut(entity).insert(Facing(direction));
    }

    let monster_name = entity_name(world, entity).unwrap_or_else(|| "MudZombie".to_string());

    // Crystal: `ranged = CurrentLocation == Target.CurrentLocation
    //                     || !Functions.InRange(CurrentLocation, Target.CurrentLocation, 2)`
    // (`MudZombie.cs:33`). `InRange(a,b,2)` is Chebyshev <= 2, so the line
    // branch (`!ranged`) is Chebyshev 1..=2 and the ranged branch is same-tile
    // or Chebyshev > 2.
    let ranged = mud_zombie_is_ranged(position, player_position);

    if !ranged {
        mud_zombie_line_attack(
            world,
            entity,
            agent,
            attacker_id,
            &monster_name,
            position,
            player_position,
            direction,
            tick,
            packets,
        );
    } else {
        mud_zombie_range_attack(
            world,
            entity,
            agent,
            attacker_id,
            &monster_name,
            position,
            player_position,
            direction,
            tick,
            packets,
        );
    }

    true
}

/// Crystal `InAttackRange` (`MudZombie.cs:14-17`):
/// `Functions.InRange(loc, target, ViewRange)` == Chebyshev distance
/// `<= ViewRange`. Mir's `Functions.InRange` is inclusive on both axes and does
/// *not* exclude the same tile (the same-tile case is handled by the `ranged`
/// split inside `Attack`).
fn mud_zombie_in_view_range(agent: &MonsterAgent, source: &Point, target: &Point) -> bool {
    tile_distance(source, target) <= agent.view_range.max(1)
}

/// Crystal `Attack` split (`MudZombie.cs:33`):
/// `ranged = loc == target || !Functions.InRange(loc, target, 2)`. Returns `true`
/// for the ranged (MC) branch, `false` for the 2-tile DC line branch.
fn mud_zombie_is_ranged(source: &Point, target: &Point) -> bool {
    let distance = tile_distance(source, target);
    distance == 0 || distance > MUD_ZOMBIE_LINE_DISTANCE
}

/// Crystal `Attack` line branch (`MudZombie.cs:35-43`): broadcast
/// `ObjectAttack { Type = 0 }` then `LineAttack(GetAttackPower(MinDC,MaxDC), 2)`.
/// The line splashes the two tiles ahead (`MonsterObject.cs:3611-3643`); each
/// caught attack target takes the DC blow (`ACAgility`) after
/// `MaxDistance(self, target) * 50 + 500` ms, and on a landed hit is poisoned.
#[allow(clippy::too_many_arguments)]
fn mud_zombie_line_attack(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    attacker_id: u32,
    monster_name: &str,
    position: &Point,
    player_position: &Point,
    direction: MirDirection,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    // Crystal `AttackTime = Envir.Time + AttackSpeed` (`MudZombie.cs:30`).
    agent_set_next_attack(agent, tick, 0);

    // Crystal: `Broadcast(new S.ObjectAttack { ... })` (`MudZombie.cs:37`).
    if let Some(packet) = monster_typed_attack_packet(world, entity, position, direction, 0) {
        packets.push(packet);
    }

    // Crystal: `int damage = GetAttackPower(MinDC, MaxDC); if (damage == 0) return;`
    // (`MudZombie.cs:39-40`). LineAttack uses `DefenceType.ACAgility` (physical),
    // so even the 2-tile reach is mitigated by AC only — compute it physically
    // rather than via `monster_player_attack_damage`, whose distance>1 path would
    // wrongly apply magic mitigation.
    let damage = mud_zombie_line_damage(world, monster_name);
    if damage <= 0 {
        return;
    }

    // Crystal `LineAttack`: tiles `PointMove(loc, dir, i)` for `i in 1..=2`
    // (`MonsterObject.cs:3613-3615`).
    let line_tiles = [
        offset_point(position, direction, 1),
        offset_point(position, direction, 2),
    ];

    // The player, if hostile (`IsAttackTarget`) and standing in a line tile.
    if agent.hostile_to_player && line_tiles.contains(player_position) {
        let due_tick =
            tick + combat_delay_ticks(mud_zombie_line_delay_ms(position, player_position));
        schedule_damage_to_player_with_effect(
            world,
            due_tick,
            attacker_id,
            monster_name.to_string(),
            damage,
            // Crystal `CompleteAttack` poisons on a landed hit (`MudZombie.cs:68`).
            Some(mud_zombie_green_poison()),
        );
    }

    // Opposing monsters caught in the two forward tiles (Crystal hits any attack
    // target on those cells). `shinsu_line_monster_targets` computes exactly the
    // `dir`+1 / `dir`+2 tiles for opposing monsters.
    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    for target in
        shinsu_line_monster_targets(world, &monster_entities, entity, position, direction, agent)
    {
        let Some(target_position) = entity_position(world, target) else {
            continue;
        };
        let due_tick =
            tick + combat_delay_ticks(mud_zombie_line_delay_ms(position, &target_position));
        schedule_damage_to_monster(world, due_tick, attacker_id, target, damage, None, None);
    }
}

/// Crystal `Attack` ranged branch (`MudZombie.cs:45-55`): an extra `+500` pause
/// on `AttackTime`, broadcast `ObjectRangeAttack`, then a single
/// `GetAttackPower(MinMC,MaxMC)` `RangeDamage` (`DefenceType.MAC`) at `+500ms`,
/// poisoning the target on a landed hit (`CompleteRangeAttack`, `MudZombie.cs:71-82`).
#[allow(clippy::too_many_arguments)]
fn mud_zombie_range_attack(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    attacker_id: u32,
    monster_name: &str,
    position: &Point,
    player_position: &Point,
    direction: MirDirection,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    // Crystal: `AttackTime = Envir.Time + AttackSpeed + 500` (`MudZombie.cs:46`).
    agent_set_next_attack(
        agent,
        tick,
        combat_delay_ticks(MUD_ZOMBIE_RANGED_EXTRA_PAUSE_MS),
    );

    let Some(player) = player_entity(world) else {
        return;
    };

    // Crystal: `Broadcast(new S.ObjectRangeAttack { ... })` (`MudZombie.cs:48`).
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

    // Crystal: `int damage = GetAttackPower(MinMC, MaxMC); if (damage == 0) return;`
    // (`MudZombie.cs:50-51`). `DelayedAction(RangeDamage, +500, target, damage,
    // DefenceType.MAC)` (`MudZombie.cs:53`) → MC blow mitigated by the player's
    // magic armour (MAC).
    let damage = mud_zombie_range_damage(world, monster_name);
    if damage <= 0 {
        return;
    }

    schedule_damage_to_player_with_effect(
        world,
        tick + combat_delay_ticks(MUD_ZOMBIE_RANGED_DAMAGE_DELAY_MS),
        attacker_id,
        monster_name.to_string(),
        damage,
        // Crystal `CompleteRangeAttack` poisons on a landed hit (`MudZombie.cs:81`).
        Some(mud_zombie_green_poison()),
    );
}

/// Crystal `PoisonTarget(target, 5, 8, PoisonType.Green, 2000)` (`MudZombie.cs:68,81`).
fn mud_zombie_green_poison() -> PendingPlayerStatusEffect {
    PendingPlayerStatusEffect::GreenPoison {
        chance_denominator: MUD_ZOMBIE_GREEN_POISON_CHANCE_DENOMINATOR,
        duration_ticks: MUD_ZOMBIE_GREEN_POISON_DURATION_TICKS,
    }
}

/// Crystal line damage `GetAttackPower(MinDC, MaxDC)` mitigated by `ACAgility`
/// (player AC). Mirrors `monster_player_attack_damage`'s melee arm but skips its
/// distance>1 magic-mitigation, because `LineAttack` is physical (`ACAgility`).
fn mud_zombie_line_damage(world: &World, monster_name: &str) -> i32 {
    let base = crystal_monster_attack_damage(monster_name);
    if base <= 0 {
        return 0;
    }
    (base - crystal_player_rolled_armour(world)).max(1)
}

/// Crystal ranged damage `GetAttackPower(MinMC, MaxMC)` mitigated by
/// `DefenceType.MAC` (player magic armour). We fold AC then the magic-mitigation
/// pass the existing ranged arms use, keeping numerics consistent with the
/// zone-authoritative pipeline.
fn mud_zombie_range_damage(world: &World, monster_name: &str) -> i32 {
    let base = crystal_monster_magic_damage(monster_name);
    if base <= 0 {
        return 0;
    }
    let mitigated = (base - crystal_player_rolled_armour(world)).max(1);
    crystal_player_magic_mitigated(world, mitigated)
}

/// Crystal `LineAttack` per-target delay: `MaxDistance(self, ob) * 50 + 500`
/// (`MonsterObject.cs:3634`).
fn mud_zombie_line_delay_ms(source: &Point, target: &Point) -> u64 {
    let distance = u64::try_from(tile_distance(source, target).max(0)).unwrap_or(0);
    distance * 50 + MUD_ZOMBIE_LINE_BASE_DELAY_MS
}

/// Crystal `AttackTime = Envir.Time + AttackSpeed (+ extra)` (`MudZombie.cs:30,46`).
fn agent_set_next_attack(agent: &mut MonsterAgent, tick: u64, extra_ticks: u64) {
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1) + extra_ticks;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::World;

    use crate::config::WorldEntityDisposition;
    use crate::runtime::components::{
        DisplayName, Facing, Monster, MonsterAgent, MonsterAiState, MonsterVitals, ObjectId,
        Position, WorldObject,
    };

    /// Build a MudZombie agent mirroring the Crystal manifest entry
    /// (`crystal_monster_manifest.json` → name "MudZombie": image 263, ai 108,
    /// view_range 7). `spawn_crystal_monster_for_test`'s session-based harness
    /// lives in a sibling private test module that isn't reachable here, so —
    /// following the established per-monster module-test convention
    /// (`horned_warrior.rs`, `trap_rock.rs`) — we spawn on a bare `World` with
    /// `ai` set explicitly to 108 so the dispatch would route to this module.
    fn mud_zombie_agent(position: Point) -> MonsterAgent {
        MonsterAgent {
            image: 263,
            dead: false,
            patrol_origin: position,
            ai: MUD_ZOMBIE_AI,
            disposition: WorldEntityDisposition::Hostile,
            hostile_to_player: true,
            tracking_player: true,
            view_range: 7,
            can_wander: false,
            move_interval_ticks: 1,
            attack_interval_ticks: 3,
            next_move_tick: 0,
            next_attack_tick: 0,
            route: Vec::new(),
            route_index: 0,
            route_waiting: false,
            next_route_tick: 0,
        }
    }

    fn spawn_mud_zombie(world: &mut World, agent: &MonsterAgent, position: Point) -> Entity {
        world
            .spawn((
                WorldObject,
                Monster,
                ObjectId(108_001),
                DisplayName::literal("MudZombie".to_string()),
                Position(position.clone()),
                Facing(MirDirection::Down),
                agent.clone(),
                MonsterAiState::default(),
                MonsterVitals {
                    hp: 100,
                    max_hp: 100,
                },
            ))
            .id()
    }

    /// Crystal `InAttackRange` uses `Functions.InRange(loc, target, ViewRange)`
    /// — Chebyshev `<= ViewRange`, not the default melee box.
    #[test]
    fn in_view_range_is_chebyshev_view_range() {
        let agent = mud_zombie_agent(Point { x: 10, y: 10 });
        let origin = Point { x: 10, y: 10 };
        // (7,0) and (5,5) are within the radius-7 box.
        assert!(mud_zombie_in_view_range(
            &agent,
            &origin,
            &Point { x: 17, y: 10 }
        ));
        assert!(mud_zombie_in_view_range(
            &agent,
            &origin,
            &Point { x: 15, y: 15 }
        ));
        // (8,0) leaves the box.
        assert!(!mud_zombie_in_view_range(
            &agent,
            &origin,
            &Point { x: 18, y: 10 }
        ));
    }

    /// Crystal `Attack` split (`MudZombie.cs:33`): same-tile or Chebyshev > 2 is
    /// the ranged (MC) branch; Chebyshev 1..=2 is the 2-tile DC line branch.
    #[test]
    fn branch_split_is_line_within_two_else_ranged() {
        let origin = Point { x: 0, y: 0 };
        // Same tile -> ranged.
        assert!(mud_zombie_is_ranged(&origin, &origin));
        // Adjacent and 2-tile -> line branch (not ranged).
        assert!(!mud_zombie_is_ranged(&origin, &Point { x: 1, y: 0 }));
        assert!(!mud_zombie_is_ranged(&origin, &Point { x: 2, y: 2 }));
        // 3 tiles -> ranged.
        assert!(mud_zombie_is_ranged(&origin, &Point { x: 3, y: 0 }));
    }

    /// Crystal `PoisonTarget(target, 5, 8, Green, 2000)` (`MudZombie.cs:68,81`):
    /// chance denominator 5, duration 8.
    #[test]
    fn green_poison_matches_crystal_args() {
        assert!(matches!(
            mud_zombie_green_poison(),
            PendingPlayerStatusEffect::GreenPoison {
                chance_denominator: 5,
                duration_ticks: 8,
            }
        ));
    }

    /// Out of view range with no aggro lock → the AI must NOT own the tick (it
    /// falls through to the shared chase, mirroring Crystal's `MoveTo`).
    /// Resource-free: the dispatch returns before any scheduling.
    #[test]
    fn out_of_view_range_falls_through_to_chase() {
        let mut world = World::new();
        let zombie_position = Point { x: 20, y: 20 };
        let mut agent = mud_zombie_agent(zombie_position.clone());
        // No aggro lock so the gate depends purely on view range.
        agent.tracking_player = false;
        let entity = spawn_mud_zombie(&mut world, &agent, zombie_position.clone());

        let mut ai_state = MonsterAiState::default();
        // 10 tiles away (> ViewRange 7).
        let player_position = Point { x: 30, y: 20 };
        let mut packets = Vec::new();

        let handled = update_mud_zombie_state(
            &mut world,
            entity,
            &mut agent,
            &mut ai_state,
            &zombie_position,
            &player_position,
            1_000,
            &mut packets,
        );

        assert!(
            !handled,
            "an out-of-view MudZombie must fall through to the shared chase"
        );
        assert!(packets.is_empty(), "no packets when falling through");
    }

    /// Key behaviour end-to-end through the real dispatch entry point: an in-view
    /// MudZombie that is still on AttackSpeed cooldown OWNS the tick (Crystal
    /// `ProcessTarget` enters `Attack` once `InAttackRange`, and never re-enters
    /// `MoveTo` while a valid in-range target exists). Asserts the ViewRange
    /// attack-range gate routes correctly — resource-free, since the cooldown
    /// branch returns before any damage scheduling.
    #[test]
    fn in_view_but_cooling_down_owns_tick() {
        let mut world = World::new();
        let zombie_position = Point { x: 20, y: 20 };
        let mut agent = mud_zombie_agent(zombie_position.clone());
        // On attack cooldown so the dispatch returns at the "in range, cooling
        // down" branch before touching combat resources.
        agent.next_attack_tick = 5_000;
        let entity = spawn_mud_zombie(&mut world, &agent, zombie_position.clone());

        let mut ai_state = MonsterAiState::default();
        // 4 tiles east: inside ViewRange (7), so InAttackRange is satisfied.
        let player_position = Point { x: 24, y: 20 };
        let mut packets = Vec::new();

        let handled = update_mud_zombie_state(
            &mut world,
            entity,
            &mut agent,
            &mut ai_state,
            &zombie_position,
            &player_position,
            1_000,
            &mut packets,
        );

        assert!(
            handled,
            "an in-view MudZombie on attack cooldown must own the tick (not chase)"
        );
        assert!(
            packets.is_empty(),
            "cooling-down MudZombie emits no attack packets"
        );
    }
}

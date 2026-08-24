//! Crystal `EvilMir` (AI id 52): an immobile, sleep-gated boss that wakes after
//! a timer, then alternates a 7/8 single-target ranged strike with a 1/8 mass
//! AoE, both dual-poisoning (Green + Paralysis) and dealing MAC-school damage.
//!
//! Crystal authority: `Crystal/Server/MirObjects/Monsters/EvilMir.cs`.
//!   - `Direction = MirDirection.Up` in the ctor (line 47); `Turn`/`Walk` are
//!     no-ops and `CanMove => false` (lines 42, 50-54) — EvilMir never moves,
//!     turns, roams, or patrols.
//!   - `CanAttack => !Sleeping && base.CanAttack` (lines 35-41); `ProcessSearch`
//!     is skipped while sleeping (lines 71-75); `IsAttackTarget(...) => !Sleeping
//!     && base` (lines 56-63) and `Attacked(...) => Sleeping ? 0 : base` (lines
//!     152-160) — while asleep it is invisible to searches, not a valid target,
//!     and takes zero damage. `Struck(...) => 0` (lines 162-165) — monster /
//!     reflect damage is always shrugged.
//!   - `ProcessAI` (lines 77-87): when `!Dead && Sleeping && Time > WakeUpTime`,
//!     clears `Sleeping` and restores `HP = Stats[HP]` (full heal on wake).
//!   - `ProcessTarget` (lines 104-137): fires only when `CanAttack &&
//!     InAttackRange` (`InRange(.., Info.ViewRange == 14)`). `Random(8) > 0`
//!     (7/8) → targeted `ObjectRangeAttack` toward the target, damage scheduled
//!     after `MaxDistance * 50 + 620` ms. Otherwise (1/8) → mass `ObjectAttack`,
//!     damage scheduled after 500 ms.
//!   - `Attack`/`CompleteAttack` (lines 89-150): damage is
//!     `GetAttackPower(MinDC, MaxDC)` vs `DefenceType.MAC`; the targeted branch
//!     scales it by `0.75`. On a landed hit, `PoisonTarget(Green, 5/15, 2000ms)`
//!     + `PoisonTarget(Paralysis, 5/5, 1000ms)`. Mass hits every opposing target
//!     within 17 of self; targeted hits a 2-tile cluster around the victim.
//!
//! Deferred (infra the sim lacks): the `DragonLink`/`DragonSystem` coupling —
//! sleep-on-death revive, de-level timers, and dragon-scaled `MaxDC` (lines
//! 19-33, 110-118, 167-188). There is no dragon system in the sim, so EvilMir is
//! ported as the always-`DragonLink == false` core: a plain `Random(8)` attack
//! roll, plain `MaxDC`, normal `Die()`. The sleep/immobile/poison combat core is
//! implemented faithfully. See `deferredParts` in the port notes.
//!
//! Sleep state mapping: Crystal `Sleeping` ⇒ `MonsterAiState.hidden` (the sim's
//! existing "not damageable / not targetable / excluded from `monster_can_attack`
//! + `is_hidden_or_sleeping_target`" flag — exactly Crystal's `Attacked => 0` /
//! `IsAttackTarget => false` / `CanAttack => !Sleeping` semantics). `mode` is a
//! one-shot "spawn initialised" sentinel; `next_state_tick` is `WakeUpTime`.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    current_player_is_dead, entity_object_id, player_entity, Facing, Monster, MonsterAgent,
    MonsterAiState, MonsterVitals,
};
use super::super::monsters::*;
use super::super::movement::*;
use super::super::packets::*;

use super::common::*;

/// Crystal `EvilMir.WakeUpTime = Time + 5 * 60 * 1000` (the DragonLink revive
/// delay, line 186) reused as the spawn-sleep timer for the dragon-less core.
pub(in crate::runtime) const EVIL_MIR_WAKE_DELAY_TICKS: u64 = 5 * 60;

/// Crystal `Info.ViewRange` for EvilMir (manifest `view_range == 14`); used for
/// both the search/aggro and the `InAttackRange` gate (`ProcessTarget`).
const EVIL_MIR_VIEW_RANGE: i32 = 14;

/// Crystal `CompleteAttack` mass branch: `FindAllTargets(17, CurrentLocation)` —
/// hit every opposing target within 17 tiles of EvilMir itself.
const EVIL_MIR_MASS_RADIUS: i32 = 17;

/// Crystal `CompleteAttack` targeted branch: `FindAllTargets(2, Target.Location)`
/// — the 2-tile cluster around the victim also takes the hit.
const EVIL_MIR_TARGETED_CLUSTER_RADIUS: i32 = 2;

/// Crystal `Attack`: targeted (non-mass) damage is scaled `damage * 0.75`.
fn evil_mir_targeted_damage(base_damage: i32) -> i32 {
    base_damage * 3 / 4
}

/// Crystal `PoisonTarget(Target, 5, 15, PoisonType.Green, 2000)` +
/// `PoisonTarget(Target, 5, 5, PoisonType.Paralysis, 1000)`. The poison `value`
/// (5) is Crystal's apply chance/strength; `2000`/`1000` ms become the durations.
/// Modeled with the shared `GreenPoisonAndParalysis` player effect (same shape
/// `EvilCentipede` uses), so the player takes ticking green damage + a paralysis
/// lock when the rolls land.
fn evil_mir_player_poison() -> PendingPlayerStatusEffect {
    PendingPlayerStatusEffect::GreenPoisonAndParalysis {
        // PoisonType.Green: value 5 → 1/5 apply chance, 2000 ms duration.
        green_chance_denominator: 5,
        green_duration_ticks: combat_delay_ticks(2_000),
        green_salt: 520,
        // PoisonType.Paralysis: value 5 → 1/5 apply chance, 1000 ms duration.
        paralysis_chance_denominator: 5,
        paralysis_duration_ticks: combat_delay_ticks(1_000),
        paralysis_salt: 521,
    }
}

/// Crystal `EvilMir`: immobile sleep-gated boss. Intercepts the AI tick entirely
/// (returns `true` = handled) so the shared melee/chase/patrol path never moves
/// it. Returns `false` only as a defensive fall-through when prerequisites are
/// missing (no object id / no direction), never to let the default mover run it.
pub(in crate::runtime) fn update_evil_mir_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    // Crystal `CanMove => false`, `Walk`/`Turn`/`ProcessRoam` no-ops: EvilMir is
    // pinned in place. Clear wander and hold the move cadence so the shared
    // dispatch can never advance it into a step, regardless of branch.
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;

    // Crystal `Die()` removes it from play; a dead EvilMir is still "handled"
    // (never falls to the default mover).
    if agent.dead {
        agent.tracking_player = false;
        agent.next_attack_tick = tick + 1;
        return true;
    }

    // Spawn/respawn initialisation: come up hidden + sleeping, facing Up, with a
    // wake timer. `mode` is the one-shot sentinel so this runs exactly once.
    if !ai_state.mode {
        ai_state.mode = true;
        ai_state.hidden = true;
        ai_state.next_state_tick = tick + EVIL_MIR_WAKE_DELAY_TICKS;
        agent.tracking_player = false;
        agent.next_attack_tick = tick + 1;
        // Crystal ctor: `Direction = MirDirection.Up`.
        if world
            .entity(entity)
            .get::<Facing>()
            .map(|facing| facing.0 != MirDirection::Up)
            .unwrap_or(false)
        {
            world.entity_mut(entity).insert(Facing(MirDirection::Up));
        }
        if let Some(object_id) = entity_object_id(world, entity) {
            packets.push(ServerPacket::ObjectHide { object_id });
        }
        return true;
    }

    // Crystal `ProcessAI`: `if (!Dead && Sleeping && Time > WakeUpTime)` →
    // wake, full-heal, and return (no attack on the wake tick).
    if ai_state.hidden {
        agent.tracking_player = false;
        agent.next_attack_tick = tick + 1;
        if tick < ai_state.next_state_tick {
            // Still sleeping: inert + invulnerable (the `hidden` flag drives
            // `monster_is_damageable`/`monster_can_attack`/`Attacked => 0`).
            return true;
        }

        // Wake: clear Sleeping, restore HP = Stats[HP] (full heal).
        ai_state.hidden = false;
        let max_hp = world
            .entity(entity)
            .get::<MonsterVitals>()
            .map(|vitals| vitals.max_hp)
            .unwrap_or(1);
        world
            .entity_mut(entity)
            .insert(MonsterVitals { hp: max_hp, max_hp });
        if let Some(object_id) = entity_object_id(world, entity) {
            packets.push(ServerPacket::ObjectShow { object_id });
        }
        if let Some(info) = object_health_info_for_entity(world, entity, 0) {
            packets.push(ServerPacket::ObjectHealth { info });
        }
        return true;
    }

    // Awake. Crystal `ProcessTarget`: only act when `CanAttack && InAttackRange`
    // and the attack cooldown has elapsed.
    if tick < agent.next_attack_tick {
        return true;
    }
    if current_player_is_dead(world) {
        agent.tracking_player = false;
        return true;
    }

    // Crystal `InAttackRange`: same map + `InRange(.., Info.ViewRange == 14)`.
    let in_range = tile_distance(position, player_position) <= EVIL_MIR_VIEW_RANGE;
    if !in_range {
        agent.tracking_player = false;
        return true;
    }

    let Some(player) = player_entity(world) else {
        return false;
    };
    let Some(attacker_id) = entity_object_id(world, entity) else {
        return false;
    };

    // Crystal: `byte random = 8; if (Random.Next(random) > 0) { targeted } else
    // { mass }`. `deterministic_chance_roll(.., 8)` is true 1/8 of the time
    // (== Crystal's `Random(8) == 0`), so the MASS branch is the `roll == true`
    // case and the TARGETED branch is the 7/8 `roll == false` case.
    let mass_attack = deterministic_chance_roll(tick, attacker_id, 52, 8);

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();

    // Crystal `GetAttackPower(MinDC, MaxDC)` vs `DefenceType.MAC`. We reuse the
    // shared player-damage roller (DC spread + player armour/magic mitigation)
    // for the player-facing number, and the raw DC for opposing monsters.
    let base_player_damage = monster_player_attack_damage(
        world,
        attacker_id,
        "EvilMir",
        agent,
        position,
        player_position,
    );
    let base_monster_damage = crystal_monster_attack_damage("EvilMir");

    if mass_attack {
        // Crystal mass: `Broadcast(ObjectAttack)`, damage after 500 ms,
        // `FindAllTargets(17, CurrentLocation)`.
        if let Some(packet) =
            monster_typed_attack_packet(world, entity, position, MirDirection::Up, 0)
        {
            packets.push(packet);
        }
        let due_tick = tick + combat_delay_ticks(500);

        if base_player_damage > 0
            && tile_distance(position, player_position) <= EVIL_MIR_MASS_RADIUS
        {
            schedule_damage_to_player_with_effect(
                world,
                due_tick,
                attacker_id,
                "EvilMir".to_string(),
                base_player_damage,
                Some(evil_mir_player_poison()),
            );
        }
        if base_monster_damage > 0 {
            for target in nearby_opposing_monster_targets(
                world,
                &monster_entities,
                entity,
                position,
                agent,
                EVIL_MIR_MASS_RADIUS,
            ) {
                schedule_damage_to_monster(
                    world,
                    due_tick,
                    attacker_id,
                    target,
                    base_monster_damage,
                    None,
                    None,
                );
            }
        }
    } else {
        // Crystal targeted: face the target (via `SetDirection`, which only ever
        // yields Up/Right/UpRight), broadcast `ObjectRangeAttack`, damage after
        // `MaxDistance * 50 + 620` ms, scaled `* 0.75`, hitting the 2-tile
        // cluster around the victim.
        let Some(face) = direction_toward(position, player_position) else {
            return false;
        };
        let attack_direction = evil_mir_set_direction(face);
        if world
            .entity(entity)
            .get::<Facing>()
            .map(|facing| facing.0 != attack_direction)
            .unwrap_or(false)
        {
            world.entity_mut(entity).insert(Facing(attack_direction));
        }

        if let Some(packet) = monster_typed_ranged_attack_packet(
            world,
            entity,
            position,
            attack_direction,
            player,
            player_position,
            0,
        ) {
            packets.push(packet);
        }

        let distance = u64::try_from(tile_distance(position, player_position).max(0)).unwrap_or(0);
        let due_tick = tick + combat_delay_ticks(distance * 50 + 620);

        let player_damage = evil_mir_targeted_damage(base_player_damage);
        let cluster_damage = evil_mir_targeted_damage(base_monster_damage);

        if player_damage > 0 {
            schedule_damage_to_player_with_effect(
                world,
                due_tick,
                attacker_id,
                "EvilMir".to_string(),
                player_damage,
                Some(evil_mir_player_poison()),
            );
        }
        if cluster_damage > 0 {
            for target in nearby_opposing_monster_targets(
                world,
                &monster_entities,
                entity,
                player_position,
                agent,
                EVIL_MIR_TARGETED_CLUSTER_RADIUS,
            ) {
                schedule_damage_to_monster(
                    world,
                    due_tick,
                    attacker_id,
                    target,
                    cluster_damage,
                    None,
                    None,
                );
            }
        }
    }

    // Crystal: `ActionTime = Time + 300; AttackTime = Time + AttackSpeed`.
    agent.tracking_player = true;
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    true
}

/// Crystal `EvilMir.SetDirection` (lines 190-203): collapses an arbitrary facing
/// into one of the three sprite-supported directions (Up / Right / UpRight).
fn evil_mir_set_direction(dir: MirDirection) -> MirDirection {
    match dir {
        MirDirection::DownRight | MirDirection::Right => MirDirection::Up,
        MirDirection::Left | MirDirection::UpLeft => MirDirection::Right,
        _ => MirDirection::UpRight,
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::components::{
        entity_object_id, entity_position, player_entity, DisplayName, Facing, Monster,
        MonsterAiState, MonsterCombatStats, MonsterVitals, ObjectId, Position, WorldObject,
    };
    use super::super::super::monsters::{
        crystal_dynamic_monster_template, crystal_respawn_can_wander, is_hidden_or_sleeping_target,
        monster_can_attack, monster_is_damageable,
    };
    use super::super::super::session::SimulationSession;
    use super::*;
    use crate::{SimulationConfig, WorldEntityDisposition};
    use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};

    /// Spawn a real "EvilMir" template (manifest AI == 52) the same way
    /// `spawn_crystal_monster_for_test` does, drive the AI through spawn → sleep
    /// → wake, and assert the Crystal-faithful behaviour:
    ///   - spawns hidden/sleeping + immobile and is NOT damageable while asleep
    ///     (Crystal `Attacked => 0`, `IsAttackTarget => false`, `CanMove =>
    ///     false`);
    ///   - never tracks/moves on its own (`CanMove => false`);
    ///   - wakes after `WakeUpTime`, full-heals, becomes damageable, and then
    ///     fires an attack at an in-view target (`ProcessTarget`).
    #[test]
    fn evil_mir_sleeps_immobile_then_wakes_and_attacks() {
        let mut session = SimulationSession::new(SimulationConfig::default());
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        let player = player_entity(session.app.world()).expect("player entity");
        let boss_origin = Point { x: 900, y: 900 };
        // Place the player well within EvilMir's ViewRange (14) but not adjacent.
        let player_origin = Point {
            x: boss_origin.x + 5,
            y: boss_origin.y,
        };
        session
            .app
            .world_mut()
            .entity_mut(player)
            .insert(Position(player_origin.clone()));

        let template = crystal_dynamic_monster_template("EvilMir").expect("EvilMir template");
        assert_eq!(template.monster_ai, 52, "EvilMir manifest AI should be 52");

        let boss_object_id = 98_052_u32;
        let tick = 100_u64;
        let mut agent = MonsterAgent {
            image: template.monster_image,
            dead: false,
            patrol_origin: boss_origin.clone(),
            ai: template.monster_ai,
            disposition: WorldEntityDisposition::Hostile,
            hostile_to_player: true,
            tracking_player: true,
            view_range: i32::from(template.monster_view_range),
            can_wander: crystal_respawn_can_wander(template.monster_hp),
            move_interval_ticks: 1,
            attack_interval_ticks: 3,
            next_move_tick: tick,
            next_attack_tick: tick,
            route: Vec::new(),
            route_index: 0,
            route_waiting: false,
            next_route_tick: tick,
        };
        // The dispatch routes to this module when agent.ai == 52; the manifest
        // already yields 52, so no override is needed.
        assert_eq!(agent.ai, 52);

        let boss_entity = session
            .app
            .world_mut()
            .spawn((
                WorldObject,
                Monster,
                ObjectId(boss_object_id),
                DisplayName::literal(template.monster_name.clone()),
                Position(boss_origin.clone()),
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
            entity_object_id(session.app.world(), boss_entity),
            Some(boss_object_id)
        );

        // --- Tick 1: spawn initialisation → hidden/sleeping, faces Up. ---
        let mut ai_state = MonsterAiState::default();
        let mut packets = Vec::new();
        let handled = update_evil_mir_state(
            session.app.world_mut(),
            boss_entity,
            &mut agent,
            &mut ai_state,
            &boss_origin,
            &player_origin,
            tick,
            &mut packets,
        );
        assert!(handled, "EvilMir AI must report handled (immobile boss)");
        assert!(ai_state.hidden, "EvilMir must spawn asleep (hidden)");
        assert!(ai_state.mode, "spawn-initialised sentinel must be set");
        assert!(!agent.tracking_player, "asleep EvilMir does not search");
        assert!(!agent.can_wander, "EvilMir CanMove => false");
        assert!(
            ai_state.next_state_tick > tick,
            "wake timer must be in the future"
        );
        // Persist like the runtime does so damageability checks see the state.
        session.app.world_mut().entity_mut(boss_entity).insert((
            agent.clone(),
            ai_state,
            Facing(MirDirection::Up),
        ));

        // While asleep: not damageable, not a valid target, excluded from attack
        // (Crystal `Attacked => 0` / `IsAttackTarget => false` / `CanAttack`).
        assert!(
            !monster_is_damageable(session.app.world(), boss_entity),
            "asleep EvilMir must take zero damage (Attacked => 0)"
        );
        assert!(
            is_hidden_or_sleeping_target(&agent, &ai_state),
            "asleep EvilMir must not be a search/attack target"
        );
        assert!(
            !monster_can_attack(&agent, &ai_state),
            "asleep EvilMir must not attack (CanAttack => !Sleeping)"
        );

        // --- Tick 2: still before WakeUpTime → stays inert + immobile. ---
        let before_wake = ai_state.next_state_tick - 1;
        let mut packets = Vec::new();
        update_evil_mir_state(
            session.app.world_mut(),
            boss_entity,
            &mut agent,
            &mut ai_state,
            &boss_origin,
            &player_origin,
            before_wake,
            &mut packets,
        );
        assert!(ai_state.hidden, "EvilMir must stay asleep until WakeUpTime");
        assert_eq!(
            entity_position(session.app.world(), boss_entity).expect("boss position"),
            boss_origin,
            "EvilMir must never move on its own"
        );

        // --- Tick 3: at/after WakeUpTime → wake + full heal, no attack yet. ---
        // Pre-damage HP so we can observe the wake full-heal.
        let max_hp = template.monster_hp.max(1);
        session
            .app
            .world_mut()
            .entity_mut(boss_entity)
            .insert(MonsterVitals {
                hp: max_hp / 2,
                max_hp,
            });
        let wake_tick = ai_state.next_state_tick;
        let mut packets = Vec::new();
        update_evil_mir_state(
            session.app.world_mut(),
            boss_entity,
            &mut agent,
            &mut ai_state,
            &boss_origin,
            &player_origin,
            wake_tick,
            &mut packets,
        );
        // The real dispatch persists the mutated ai_state to the world via
        // persist_monster_runtime_state; mirror that here so the world-reading
        // monster_is_damageable() below observes the cleared `hidden` flag.
        session
            .app
            .world_mut()
            .entity_mut(boss_entity)
            .insert(ai_state);
        assert!(!ai_state.hidden, "EvilMir must wake after WakeUpTime");
        let woke_vitals = *session
            .app
            .world()
            .entity(boss_entity)
            .get::<MonsterVitals>()
            .expect("boss vitals");
        assert_eq!(
            woke_vitals.hp, woke_vitals.max_hp,
            "wake must restore HP = Stats[HP] (full heal)"
        );
        assert!(
            monster_is_damageable(session.app.world(), boss_entity),
            "awake EvilMir must be damageable"
        );

        // --- Tick 4: awake + in view + cooldown elapsed → fires an attack. ---
        // (Wake returns early without attacking; advance past the cooldown.)
        let attack_tick = agent.next_attack_tick;
        let mut packets = Vec::new();
        let handled = update_evil_mir_state(
            session.app.world_mut(),
            boss_entity,
            &mut agent,
            &mut ai_state,
            &boss_origin,
            &player_origin,
            attack_tick,
            &mut packets,
        );
        assert!(
            handled,
            "awake EvilMir AI is still handled (never the mover)"
        );
        assert!(
            agent.tracking_player,
            "EvilMir locks onto an in-view target on attack"
        );
        assert!(
            agent.next_attack_tick > attack_tick,
            "attack must push the AttackSpeed cooldown forward"
        );
        // It emitted either an ObjectAttack (mass) or ObjectRangeAttack
        // (targeted) toward the player — both are valid Crystal branches.
        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectAttack { .. } | ServerPacket::ObjectRangeAttack { .. }
            )),
            "awake in-range EvilMir must broadcast an attack packet"
        );
        // Still immobile after attacking.
        assert_eq!(
            entity_position(session.app.world(), boss_entity).expect("boss position"),
            boss_origin,
            "EvilMir must never move, even while attacking"
        );
    }
}

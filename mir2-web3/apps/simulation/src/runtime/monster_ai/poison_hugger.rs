//! Monster AI 69 — `PoisonHugger` (Crystal `Server/MirObjects/Monsters/PoisonHugger.cs`).
//!
//! `PoisonHugger : MonsterObject`. A short-lived proximity bomb that hugs its
//! target and detonates on contact, spraying green poison. Faithful port of the
//! Crystal `ProcessTarget`/`Die`/`CompleteDeath` trio:
//!
//! * **Lifetime** (`PoisonHugger.cs:15`): the constructor arms
//!   `ExplosionTime = Envir.Time + 1000 * 60 * 5` (5 minutes). When that time is
//!   exceeded the monster auto-dies (and explodes). Stored here in
//!   `ai_state.next_state_tick`, seeded lazily on the first tick (the shared
//!   `initial_monster_ai_state` does not special-case AI 69), exactly like
//!   `hell_bomb.rs` seeds its lifetime.
//! * **Per tick** (`PoisonHugger.cs:23` `ProcessTarget`, only meaningful while a
//!   target exists — Crystal's `ProcessSearch` runs immediately before it):
//!   * No target / `Time > ExplosionTime` / target no longer attackable →
//!     `Die()` (suicide detonation, below).
//!   * `ranged = !InRange(loc, target, 1)` i.e. target further than 1 tile.
//!   * `InAttackRange()` is `InRange(loc, target, AttackRange=5)` (Chebyshev ≤ 5):
//!     * adjacent (`!ranged`, dist ≤ 1) → `Die()` (suicide detonation).
//!     * ranged with `Random.Next(5) == 0` (1/5) → broadcast `ObjectRangeAttack`,
//!       then a `DelayedType.RangeDamage` at `Time + MaxDistance*50 + 500`,
//!       single-target, `GetAttackPower(MinDC, MaxDC)`, `DefenceType.ACAgility`.
//!     * ranged otherwise (4/5) → `MoveTo(target)`.
//!   * dist 2..=5 fall-through and dist > 5 both end in `MoveTo(target)` (chase).
//! * **Die** (`PoisonHugger.cs:76`): `FindAllTargets(1, CurrentLocation)` — every
//!   attackable, opposing object inside the 3x3 around self — each gets a
//!   `DelayedType.Die` action at `Time + 500` for `GetAttackPower(MinDC, MaxDC)`
//!   with `DefenceType.ACAgility`. `CompleteDeath` (`:88`) deals that damage and,
//!   on a surviving hit, `PoisonTarget(target, 5, 5, PoisonType.Green, 2000)`
//!   (1/5 chance, 5-tick Green poison whose value is `GetAttackPower(MinSC,MaxSC)`).
//!
//! Damage/poison plumbing mirrors `explode_bomb_spider`/`explode_vampire_spider`
//! (radius-1 opposing-only AoE) and the Green-poison helpers used by the other
//! ported poison monsters. PoisonHugger's manifest DC/SC are all zero, so the
//! `crystal_monster_*_damage` helpers (which floor at 1, matching the BombSpider
//! port) supply the explosion's nominal DC and the poison's nominal SC.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    current_player_is_dead, entity_name, entity_object_id, entity_position, player_entity, Facing,
    Monster, MonsterAgent, MonsterAiState, Position,
};
use super::super::monsters::*;
use super::super::movement::*;

/// `PoisonHugger.AttackRange = 5` (`PoisonHugger.cs:9`).
const POISON_HUGGER_ATTACK_RANGE: i32 = 5;

/// `ExplosionTime = Envir.Time + 1000 * 60 * 5` (`PoisonHugger.cs:15`). At one
/// runtime tick per 1000ms (see `combat_delay_ticks`) that is 300 ticks — the
/// same value `BombSpider` uses for its identical 5-minute fuse.
const POISON_HUGGER_EXPLOSION_LIFETIME_TICKS: u64 = 300;

/// `DelayedAction(DelayedType.Die, Envir.Time + 500, ...)` (`PoisonHugger.cs:82`):
/// the suicide-blast damage lands 500ms after detonation.
const POISON_HUGGER_EXPLOSION_DELAY_MS: u64 = 500;

/// `FindAllTargets(1, CurrentLocation)` (`PoisonHugger.cs:78`): the detonation
/// hits everything attackable inside the 3x3 (Chebyshev radius 1) around self.
const POISON_HUGGER_EXPLOSION_RADIUS: i32 = 1;

/// `PoisonTarget(target, chanceToPoison = 5, ...)` (`PoisonHugger.cs:96`):
/// `Random.Next(5) == 0` gates the Green poison on a struck target.
const POISON_HUGGER_POISON_CHANCE_DENOMINATOR: u64 = 5;

/// `PoisonTarget(target, ..., poisonDuration = 5, ...)` (`PoisonHugger.cs:96`).
const POISON_HUGGER_GREEN_POISON_DURATION_TICKS: u64 = 5;

/// `Envir.Random.Next(5) == 0` ranged-attack gate (`PoisonHugger.cs:38`).
const POISON_HUGGER_RANGE_ATTACK_DENOMINATOR: u64 = 5;

/// Green poison bit used by `MonsterPoisonState` (`combat.rs` `GREEN_POISON = 1`).
const POISON_HUGGER_GREEN_POISON_BIT: u16 = 1;

/// Deterministic-RNG salt for this AI, mirroring the `agent.ai` salt convention
/// used by the other ported monster modules (replaces Crystal `Envir.Random`).
const POISON_HUGGER_AI_SALT: u64 = 69;

pub(in crate::runtime) fn update_poison_hugger_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return true;
    }

    // Crystal arms `ExplosionTime` in the constructor; the shared AI-state seeder
    // does not special-case AI 69, so arm the 5-minute fuse on the first tick
    // (mirrors `hell_bomb.rs`). Sentinel 0 = unset.
    if ai_state.next_state_tick == 0 {
        ai_state.next_state_tick = tick + POISON_HUGGER_EXPLOSION_LIFETIME_TICKS;
    }
    let explosion_time = ai_state.next_state_tick;

    // `if (!CanAttack) return;` (`PoisonHugger.cs:25`). A monster that may not
    // attack also takes no ProcessTarget action this tick; fall through so the
    // default engine can still tick movement/state.
    if !monster_can_attack(agent, ai_state) {
        return false;
    }

    // Crystal `ProcessSearch` (run immediately before `ProcessTarget`) is what
    // produces `Target`. In the sim we mirror it directly: the player is a valid
    // target when alive, hostile to us, and within view range — equivalent to
    // `IsAttackTarget(this)` + sight. The engine's own aggro bookkeeping runs
    // only on the default (fall-through) path, so we must resolve the target
    // ourselves here.
    let player_alive = !current_player_is_dead(world);
    let distance = tile_distance(position, player_position);
    let has_target = player_alive && agent.hostile_to_player && distance <= agent.view_range.max(1);

    // `if (Target == null || Envir.Time > ExplosionTime || !IsAttackTarget) Die();`
    // (`PoisonHugger.cs:27`).
    if !has_target || tick > explosion_time {
        detonate_poison_hugger(world, entity, agent, position, tick, packets);
        return true;
    }

    // `bool ranged = !Functions.InRange(CurrentLocation, Target.CurrentLocation, 1);`
    let ranged = distance > 1;

    // `if (InAttackRange())` — `InRange(loc, target, AttackRange=5)`.
    if distance <= POISON_HUGGER_ATTACK_RANGE {
        if !ranged {
            // Adjacent: `Die();` (suicide detonation) (`PoisonHugger.cs:63`).
            detonate_poison_hugger(world, entity, agent, position, tick, packets);
            return true;
        }

        // Ranged: `if (Envir.Random.Next(5) == 0)` fire a ranged poison bolt.
        if deterministic_chance_roll(
            tick,
            entity.index() as u32,
            POISON_HUGGER_AI_SALT,
            POISON_HUGGER_RANGE_ATTACK_DENOMINATOR,
        ) {
            poison_hugger_range_attack(
                world,
                entity,
                agent,
                position,
                player_position,
                tick,
                packets,
            );
            return true;
        }

        // `else { MoveTo(Target.CurrentLocation); }` — chase one step.
    }

    // Fall-through `MoveTo(Target.CurrentLocation)` (covers the 4/5 ranged case
    // and dist > AttackRange). One greedy/A* step toward the player.
    poison_hugger_step_toward_player(
        world,
        entity,
        agent,
        position,
        player_position,
        tick,
        packets,
    );
    true
}

/// Crystal `Die()` (`PoisonHugger.cs:76`) + `CompleteDeath` (`:88`): schedule the
/// 500ms suicide blast against every opposing object in the 3x3, then die. The
/// blast deals `GetAttackPower(MinDC, MaxDC)` (`DefenceType.ACAgility` — no armour
/// reduction) and Green-poisons survivors. Mirrors `explode_vampire_spider`'s
/// radius-1 opposing-only AoE plumbing.
fn detonate_poison_hugger(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(attacker_id) = entity_object_id(world, entity) else {
        // Cannot identify the bomb; still remove it so it does not linger.
        let _ = damage_monster_entity(world, entity, i32::MAX, tick, packets);
        return;
    };
    let monster_name = entity_name(world, entity).unwrap_or_else(|| "PoisonHugger".to_string());

    // `GetAttackPower(MinDC, MaxDC)` — PoisonHugger's DC is 0 in the manifest, so
    // this floors at 1 (the same convention `explode_bomb_spider` uses).
    let blast_damage = crystal_monster_attack_damage(&monster_name);
    // `PoisonTarget` value is `GetAttackPower(MinSC, MaxSC)` — also 0 in the
    // manifest, floored at 1.
    let poison_damage = crystal_monster_spell_damage(&monster_name);
    let due_tick = tick + combat_delay_ticks(POISON_HUGGER_EXPLOSION_DELAY_MS);
    let attacker_hostile_to_player = agent.hostile_to_player;

    // Player branch: a hostile bomb hits the player if inside the 3x3.
    if attacker_hostile_to_player {
        if let Some(player) = player_entity(world) {
            if let Some(player_position) = entity_position(world, player) {
                if !current_player_is_dead(world)
                    && tile_distance(position, &player_position) <= POISON_HUGGER_EXPLOSION_RADIUS
                {
                    schedule_damage_to_player_with_effect(
                        world,
                        due_tick,
                        attacker_id,
                        monster_name.clone(),
                        blast_damage,
                        Some(PendingPlayerStatusEffect::GreenPoison {
                            chance_denominator: POISON_HUGGER_POISON_CHANCE_DENOMINATOR,
                            duration_ticks: POISON_HUGGER_GREEN_POISON_DURATION_TICKS,
                        }),
                    );
                }
            }
        }
    }

    // Opposing-monster branch: schedule the blast damage and apply the Green
    // poison to every opposing monster inside the 3x3 (FindAllTargets opposing
    // filter == the `attacker_hostile_to_player != target.hostile` test used by
    // the other explosion helpers).
    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    for target_entity in monster_entities {
        if target_entity == entity {
            continue;
        }
        let entry = world.entity(target_entity);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_hostile_to_player == target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if tile_distance(position, &target_position) > POISON_HUGGER_EXPLOSION_RADIUS {
            continue;
        }

        schedule_damage_to_monster(
            world,
            due_tick,
            attacker_id,
            target_entity,
            blast_damage,
            None,
            None,
        );
        // Crystal poisons on a surviving hit (1/5). The sim has no
        // post-resolution poison hook, so gate the poison with the same
        // deterministic 1/5 roll and apply it alongside the scheduled damage —
        // the closest faithful core with existing primitives.
        if poison_damage > 0
            && deterministic_chance_roll(
                tick,
                attacker_id,
                POISON_HUGGER_AI_SALT,
                POISON_HUGGER_POISON_CHANCE_DENOMINATOR,
            )
        {
            apply_monster_poison(
                world,
                target_entity,
                POISON_HUGGER_GREEN_POISON_BIT,
                poison_damage,
                tick,
                POISON_HUGGER_GREEN_POISON_DURATION_TICKS,
            );
        }
    }

    // `base.Die()` — remove the bomb. `damage_monster_entity(i32::MAX)` is the
    // canonical self-kill (emits ObjectHealth/ObjectDied + handles despawn), the
    // same primitive `explode_bomb_spider` uses.
    agent.dead = true;
    let _ = damage_monster_entity(world, entity, i32::MAX, tick, packets);
}

/// Crystal ranged branch (`PoisonHugger.cs:40`): face the target, broadcast
/// `ObjectRangeAttack`, and schedule single-target `RangeDamage` after
/// `MaxDistance*50 + 500` ms (`DefenceType.ACAgility`).
fn poison_hugger_range_attack(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(player) = player_entity(world) else {
        return;
    };
    let Some(attacker_id) = entity_object_id(world, entity) else {
        return;
    };
    let Some(direction) = direction_toward(position, player_position) else {
        return;
    };
    let monster_name = entity_name(world, entity).unwrap_or_else(|| "PoisonHugger".to_string());

    if world
        .entity(entity)
        .get::<Facing>()
        .map(|facing| facing.0 != direction)
        .unwrap_or(false)
    {
        world.entity_mut(entity).insert(Facing(direction));
    }

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

    // `int delay = MaxDistance * 50 + 500;` — `ranged_attack_delay_ticks` is
    // exactly `combat_delay_ticks(MaxDistance * 50 + 500)`.
    let damage = crystal_monster_attack_damage(&monster_name);
    if damage > 0 {
        schedule_damage_to_player(
            world,
            tick + ranged_attack_delay_ticks(position, player_position),
            attacker_id,
            monster_name,
            damage,
        );
    }

    agent.tracking_player = true;
    // `AttackTime = Envir.Time + AttackSpeed;`
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
}

/// Crystal `MoveTo(Target.CurrentLocation)` (`PoisonHugger.cs:58`/`:73`): one
/// step toward the player, routing around blockers like the other chase modules.
fn poison_hugger_step_toward_player(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    agent.tracking_player = true;
    if tick < agent.next_move_tick {
        return;
    }

    if let Some((destination, direction)) = super::common::monster_step_toward_with_fallback(
        world,
        entity,
        position,
        player_position,
        tick,
        POISON_HUGGER_AI_SALT as usize,
    ) {
        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        world
            .entity_mut(entity)
            .insert((Position(destination.clone()), Facing(direction)));
        if let Some(object_id) = entity_object_id(world, entity) {
            packets.push(ServerPacket::ObjectWalk {
                movement: mir2_protocol::ObjectMovement {
                    object_id,
                    position: destination,
                    direction,
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::components::{
        DisplayName, MonsterCombatStats, MonsterVitals, ObjectId, WorldObject,
    };
    use super::super::super::monsters::crystal_dynamic_monster_template;
    use super::super::super::session::SimulationSession;
    use super::*;
    use crate::{SimulationConfig, WorldEntityDisposition};
    use mir2_protocol::{ClientPacket, MirDirection, ServerPacket};

    fn spawn_poison_hugger(
        session: &mut SimulationSession,
        object_id: u32,
        position: Point,
        tick: u64,
    ) -> (Entity, MonsterAgent) {
        let template =
            crystal_dynamic_monster_template("PoisonHugger").expect("PoisonHugger template");
        assert_eq!(
            template.monster_ai, 69,
            "PoisonHugger manifest AI should be 69"
        );

        let agent = MonsterAgent {
            image: template.monster_image,
            dead: false,
            patrol_origin: position.clone(),
            ai: template.monster_ai,
            disposition: WorldEntityDisposition::Hostile,
            hostile_to_player: true,
            tracking_player: true,
            view_range: i32::from(template.monster_view_range).max(7),
            can_wander: true,
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
                ObjectId(object_id),
                DisplayName::literal("PoisonHugger".to_string()),
                Position(position.clone()),
                Facing(MirDirection::Up),
                agent.clone(),
                MonsterAiState::default(),
                MonsterVitals { hp: 1, max_hp: 1 },
                MonsterCombatStats { agility: 0 },
            ))
            .id();
        (entity, agent)
    }

    /// Adjacent to a live, hostile target → Crystal `Die()` suicide detonation:
    /// the bomb dies this tick and a delayed (500ms) blast lands on the player.
    /// Spawning a real "PoisonHugger" template yields `ai == 69`, so this is the
    /// exact AI path the dispatch routes to.
    #[test]
    fn poison_hugger_detonates_when_adjacent() {
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

        // Find an open tile pair: player at P, bomb at P+1 (adjacent), both open.
        let (player_origin, bomb_position) = (240..460)
            .find_map(|y| {
                (240..460).find_map(|x| {
                    let player_position = Point { x, y };
                    let bomb_position = Point { x: x + 1, y };
                    (can_occupy(session.app.world(), player_position.clone(), Some(player))
                        && can_occupy(session.app.world(), bomb_position.clone(), None))
                    .then_some((player_position, bomb_position))
                })
            })
            .expect("test should find an open adjacent tile pair");

        session
            .app
            .world_mut()
            .entity_mut(player)
            .insert(Position(player_origin.clone()));

        let tick = 100_u64;
        let (entity, mut agent) =
            spawn_poison_hugger(&mut session, 779_069, bomb_position.clone(), tick);

        let mut ai_state = MonsterAiState::default();
        let mut packets = Vec::new();
        let handled = update_poison_hugger_state(
            session.app.world_mut(),
            entity,
            &mut agent,
            &mut ai_state,
            &bomb_position,
            &player_origin,
            tick,
            &mut packets,
        );

        assert!(
            handled,
            "PoisonHugger AI should handle the tick (return true)"
        );
        assert!(
            agent.dead,
            "adjacent PoisonHugger should suicide-detonate (Die)"
        );
        // The fuse was armed on this first tick.
        assert_eq!(
            ai_state.next_state_tick,
            tick + POISON_HUGGER_EXPLOSION_LIFETIME_TICKS,
        );
        // A death packet was emitted for the detonating bomb.
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectDied { info } if info.object_id == 779_069
        )));
    }
}

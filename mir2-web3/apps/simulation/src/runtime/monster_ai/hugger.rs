// Crystal `Hugger` (AI 70) — a mobile proximity bomber.
//
// Port of `Crystal/Server/MirObjects/Monsters/Hugger.cs`. The Hugger chases its
// target and melees with a generic DC swing like an ordinary monster, but it is
// fitted with a death-timer: it self-destructs when it loses its target, when
// its melee kills its target, or when its 5-minute fuse runs out. On death it
// explodes — every target within range 1 takes a DC hit (ACAgility) and the
// player is additionally hit with a 1-in-5 Green poison.
//
// `Hugger.cs:16-47` `ProcessTarget`:
//   if (!CanAttack) return;
//   if (Target == null)            { Die(); return; }
//   if (Envir.Time > ExplosionTime){ Die(); return; }
//   if (InAttackRange())           { Attack(); if (Target.Dead) Die(); return; }
//   if (Envir.Time < ShockTime)    { Target = null; return; }
//   MoveTo(Target.CurrentLocation);
// `Hugger.cs:49-69` `Die`/`CompleteDeath`:
//   ActionList.Add(DelayedAction(Die, Envir.Time + 500)); base.Die();
//   targets = FindAllTargets(1, CurrentLocation, false);
//   foreach target: damage = GetAttackPower(MinDC,MaxDC); if (damage==0) return;
//                   if (target.Attacked(this, damage, ACAgility) <= 0) return;
//                   PoisonTarget(target, 5, 5, Green, 2000);

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    entity_name, entity_object_id, entity_position, player_entity, Monster, MonsterAgent,
    MonsterAiState,
};
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

/// `Hugger` ctor: `ExplosionTime = Envir.Time + 1000 * 60 * 5` (`Hugger.cs:13`).
/// `combat_delay_ticks(300_000)` == 300 ticks at one tick per second.
pub(in crate::runtime) const HUGGER_EXPLOSION_LIFETIME_TICKS: u64 = 300;

/// Crystal `Die()` defers `CompleteDeath` by 500ms (`Hugger.cs:51`); the AoE lands then.
const HUGGER_EXPLOSION_DELAY_MS: u64 = 500;

/// `FindAllTargets(1, ...)` — explosion reach is Chebyshev distance 1 (`Hugger.cs:57`).
const HUGGER_EXPLOSION_RADIUS: i32 = 1;

/// `PoisonTarget(target, 5, 5, Green, 2000)` (`Hugger.cs:67`): 1-in-5 chance,
/// 5-second Green poison.
const HUGGER_GREEN_POISON_CHANCE_DENOMINATOR: u64 = 5;
const HUGGER_GREEN_POISON_DURATION_TICKS: u64 = 5;

pub(in crate::runtime) fn update_hugger_state(
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

    // `ExplosionTime` is normally seeded at spawn (see `initial_monster_ai_state`
    // for AI 70). Seed lazily as a fallback so a Hugger that reaches the AI pass
    // with an un-initialised fuse still detonates after the full 5-minute window
    // rather than immediately.
    if ai_state.next_state_tick == 0 {
        ai_state.next_state_tick = tick + HUGGER_EXPLOSION_LIFETIME_TICKS;
    }

    // `Target == null` (`Hugger.cs:20`) and `Envir.Time > ExplosionTime`
    // (`Hugger.cs:25`) both route straight to `Die()` — i.e. detonate.
    if !hugger_has_target(world, entity, agent, position, player_position)
        || tick >= ai_state.next_state_tick
    {
        explode_hugger(world, entity, agent, position, tick, packets);
        return true;
    }

    // Otherwise the Hugger behaves like an ordinary melee chaser: when adjacent
    // it swings for generic DC (`Attack()`), otherwise it walks toward the target
    // (`MoveTo`). Both are exactly the shared default monster path, so fall
    // through to it.
    //
    // NOTE: Crystal additionally `Die()`s when that melee kills the target
    // (`Hugger.cs:34-35`). The default path resolves melee damage asynchronously
    // (scheduled into `resolve_pending_combat_actions`), so there is no
    // synchronous "did my swing kill it?" signal to hook here; this melee-kill
    // detonation is deferred. For the shipped Hugger (MinDC = MaxDC = 0) the
    // branch is unreachable anyway, and the fuse / lost-target detonations above
    // remain faithful.
    false
}

/// True when the Hugger still has a Crystal `Target`: the player within view
/// range (or already being tracked), or any nearby opposing monster.
fn hugger_has_target(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
    player_position: &Point,
) -> bool {
    let view_range = agent.view_range.max(1);

    if agent.tracking_player
        || (agent.hostile_to_player && tile_distance(position, player_position) <= view_range)
    {
        return true;
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    !nearby_opposing_monster_targets(
        world,
        &monster_entities,
        entity,
        position,
        agent,
        view_range,
    )
    .is_empty()
}

/// Crystal `Hugger.Die()` + `CompleteDeath` (`Hugger.cs:49-69`): the monster dies
/// and, 500ms later, every target within range 1 takes a DC hit (ACAgility) and
/// the player is additionally Green-poisoned.
pub(in crate::runtime) fn explode_hugger(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(attacker_id) = entity_object_id(world, entity) else {
        return;
    };
    let attacker_name = entity_name(world, entity).unwrap_or_else(|| "Hugger".to_string());

    // Visual swing for the detonation, mirroring the other bombers.
    if let Some(packet) = monster_melee_attack_packet(world, entity, position, MirDirection::Up) {
        packets.push(packet);
    }

    // `damage = GetAttackPower(MinDC, MaxDC)` then `if (damage == 0) return;`
    // (`Hugger.cs:62-63`) — a zero roll aborts the whole AoE, so no target takes
    // damage and none is poisoned.
    let damage = crystal_monster_raw_attack_damage(&attacker_name);
    if damage > 0 {
        let due_tick = tick + combat_delay_ticks(HUGGER_EXPLOSION_DELAY_MS);

        if agent.hostile_to_player {
            if let Some(player) = player_entity(world) {
                if let Some(player_position) = entity_position(world, player) {
                    if tile_distance(position, &player_position) <= HUGGER_EXPLOSION_RADIUS {
                        schedule_damage_to_player_with_effect(
                            world,
                            due_tick,
                            attacker_id,
                            attacker_name.clone(),
                            damage,
                            Some(PendingPlayerStatusEffect::GreenPoison {
                                chance_denominator: HUGGER_GREEN_POISON_CHANCE_DENOMINATOR,
                                duration_ticks: HUGGER_GREEN_POISON_DURATION_TICKS,
                            }),
                        );
                    }
                }
            }
        }

        let monster_entities: Vec<Entity> = world
            .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
            .iter(world)
            .collect();
        for target in nearby_opposing_monster_targets(
            world,
            &monster_entities,
            entity,
            position,
            agent,
            HUGGER_EXPLOSION_RADIUS,
        ) {
            // NOTE: Crystal also Green-poisons opposing monster targets
            // (`PoisonTarget` in the same loop). The scheduled monster-damage
            // path carries no poison payload, so monster-side Green poison is
            // deferred; the DC hit itself is faithful.
            schedule_damage_to_monster(world, due_tick, attacker_id, target, damage, None, None);
        }
    }

    // `base.Die()` — run the normal death/respawn path and emit the death packets.
    // Mark the cloned agent dead first so persistence cannot resurrect it.
    agent.dead = true;
    let _ = damage_monster_entity(world, entity, i32::MAX, tick, packets);
}

#[cfg(test)]
mod tests {
    use super::super::super::components::{
        DisplayName, Facing, MonsterCombatStats, MonsterVitals, ObjectId, Position, WorldObject,
    };
    use super::super::super::monsters::crystal_dynamic_monster_template;
    use super::super::super::session::SimulationSession;
    use super::*;
    use crate::{SimulationConfig, WorldEntityDisposition};
    use mir2_protocol::{ClientPacket, MirDirection, ServerPacket};

    fn spawn_hugger(
        session: &mut SimulationSession,
        object_id: u32,
        position: Point,
        tick: u64,
    ) -> (Entity, MonsterAgent) {
        let template = crystal_dynamic_monster_template("Hugger").expect("Hugger template");
        // The manifest AI is 70; the dispatch routes "Hugger" straight here.
        assert_eq!(template.monster_ai, 70, "Hugger manifest AI should be 70");

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
                DisplayName::literal("Hugger".to_string()),
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

    /// Crystal `Hugger.ProcessTarget`: once `Envir.Time > ExplosionTime` the
    /// Hugger `Die()`s — it detonates and is removed. We arm an already-expired
    /// fuse and assert the AI handles the tick, marks the bomb dead, and emits its
    /// death packet. Spawning a real "Hugger" template yields `ai == 70`, the AI
    /// path the dispatch routes to.
    #[test]
    fn hugger_detonates_when_fuse_expires() {
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

        // Open tile pair: player at P, Hugger at P+1 (adjacent), both occupiable.
        // Adjacency keeps a live target present so the lost-target detonation does
        // not pre-empt the fuse path under test.
        let (player_origin, hugger_position) = (240..460)
            .find_map(|y| {
                (240..460).find_map(|x| {
                    let player_position = Point { x, y };
                    let hugger_position = Point { x: x + 1, y };
                    (can_occupy(session.app.world(), player_position.clone(), Some(player))
                        && can_occupy(session.app.world(), hugger_position.clone(), None))
                    .then_some((player_position, hugger_position))
                })
            })
            .expect("test should find an open adjacent tile pair");

        session
            .app
            .world_mut()
            .entity_mut(player)
            .insert(Position(player_origin.clone()));

        let tick = 1_000_u64;
        let (entity, mut agent) =
            spawn_hugger(&mut session, 779_070, hugger_position.clone(), tick);

        // Arm a fuse that has already elapsed: `Envir.Time > ExplosionTime`.
        let mut ai_state = MonsterAiState {
            next_state_tick: tick - 1,
            ..Default::default()
        };
        let mut packets = Vec::new();
        let handled = update_hugger_state(
            session.app.world_mut(),
            entity,
            &mut agent,
            &mut ai_state,
            &hugger_position,
            &player_origin,
            tick,
            &mut packets,
        );

        assert!(handled, "expired-fuse Hugger AI should handle the tick");
        assert!(agent.dead, "expired-fuse Hugger should detonate (Die)");
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectDied { info } if info.object_id == 779_070
        )));

        // Hugger's MinDC == MaxDC == 0, so `damage == 0` aborts CompleteDeath: no
        // delayed player damage is queued and the player is left unharmed. (No
        // delayed-damage / Struck packets here, only the bomb's own death.)
        assert!(
            !packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::Struck { .. })),
            "zero-DC Hugger detonation should not damage anything"
        );
    }
}

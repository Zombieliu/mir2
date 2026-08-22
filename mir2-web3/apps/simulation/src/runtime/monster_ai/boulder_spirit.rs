//! Crystal `BoulderSpirit` (AI 170): an immobile, non-attacking proximity mine.
//!
//! Crystal/Server/MirObjects/Monsters/BoulderSpirit.cs:
//! - `CanMove`/`CanAttack`/`CanRegen` are all `false`; `Walk(dir)` returns
//!   `false` (it never moves, chases, attacks, or regenerates).
//! - `ProcessAI` (line 21): `FindAllTargets(Info.ViewRange, CurrentLocation,
//!   needSight: true)` — if any hostile (player or opposing monster) is within
//!   view and visible (not Hidden), call `Die()`.
//! - `Die()` (line 33): queues `DelayedAction(DelayedType.Die, Time + 300)` then
//!   `base.Die()`, so the kill resolves 300ms later.
//! - `CompleteDeath` (line 39): `FindAllTargets(Info.ViewRange, CurrentLocation,
//!   needSight: false)` and `Attacked(this, GetAttackPower(MinDC, MaxDC),
//!   DefenceType.ACAgility)` against every opposing target in view; bails the
//!   per-target loop when `damage == 0`.
//!
//! Modelled on `bomb_spider` / `hell_bomb`: it intercepts the per-tick AI so the
//! default melee/chase path never runs. `ai_state.mode` is the "armed/dying"
//! flag and `ai_state.next_state_tick` is the 300ms detonation deadline (both
//! start clean: `monster_ai_state_for_ai(170)` yields `mode=false`,
//! `next_state_tick=0`).

// NOTE: new per-monster module; follows the post-refactor monster_ai layout.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    Monster, MonsterAgent, MonsterAiState, current_player_is_dead, entity_name, entity_object_id,
    entity_position, player_entity,
};
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

pub(in crate::runtime) fn update_boulder_spirit_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    // Crystal: CanMove=false, CanAttack=false, CanRegen=false, Walk()=>false.
    // Pin the movement/attack clocks forward so the (skipped) default path can
    // never act, mirroring `hell_bomb`'s immobilisation.
    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;
    agent.next_attack_tick = tick + 1;

    if agent.dead {
        return true;
    }

    let view_range = agent.view_range.max(1);

    // Phase 2 — armed by a prior ProcessAI. Crystal's `Die()` delayed the kill by
    // 300ms; once that deadline passes, run `CompleteDeath` (the AoE) and die.
    if ai_state.mode {
        if tick >= ai_state.next_state_tick {
            detonate_boulder_spirit(world, entity, agent, position, view_range, tick, packets);
        }
        return true;
    }

    // Phase 1 — ProcessAI: FindAllTargets(ViewRange, needSight: true). Any hostile
    // within view that is visible (not hidden) trips the mine.
    if boulder_spirit_has_visible_target(
        world,
        entity,
        agent,
        position,
        player_position,
        view_range,
    ) {
        ai_state.mode = true;
        ai_state.next_state_tick = tick + combat_delay_ticks(300);
    }

    true
}

/// Crystal `FindAllTargets(ViewRange, location, needSight: true)` reduced to a
/// boolean "is anything I can attack within view and currently visible?".
///
/// `needSight: true` in Crystal only excludes `Hidden` targets (it is not a
/// geometric wall raycast) — `nearby_opposing_monster_targets` already filters
/// hidden/sleeping monsters, so it is the faithful monster-side query. The
/// single sim player is never `Hidden`, so a live, hostile, in-range player
/// always counts.
fn boulder_spirit_has_visible_target(
    world: &World,
    entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
    player_position: &Point,
    view_range: i32,
) -> bool {
    if agent.hostile_to_player
        && !current_player_is_dead(world)
        && tile_distance(position, player_position) <= view_range
    {
        return true;
    }

    #[allow(deprecated)]
    let monster_entities: Vec<Entity> = world
        .iter_entities()
        .filter_map(|entry| entry.contains::<Monster>().then_some(entry.id()))
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

/// Crystal `CompleteDeath`: deal `GetAttackPower(MinDC, MaxDC)` (DefenceType
/// `ACAgility`) to every opposing target within view (`needSight: false`), then
/// finalise the death. `crystal_monster_attack_damage` is the canonical port of
/// `GetAttackPower(MinDC, MaxDC)`; like Crystal's `if (damage == 0) return;` we
/// skip the AoE when it is non-positive, but the structure still dies.
fn detonate_boulder_spirit(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    view_range: i32,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    if let Some(attacker_id) = entity_object_id(world, entity) {
        let attacker_name =
            entity_name(world, entity).unwrap_or_else(|| "BoulderSpirit".to_string());
        let damage = crystal_monster_attack_damage(&attacker_name);

        if damage > 0 {
            let due_tick = tick + combat_delay_ticks(300);

            // Player target — Crystal includes the player via FindAllTargets when
            // the spirit is hostile to it.
            if agent.hostile_to_player {
                if let Some(player) = player_entity(world) {
                    if !current_player_is_dead(world) {
                        if let Some(player_position) = entity_position(world, player) {
                            if tile_distance(position, &player_position) <= view_range {
                                schedule_damage_to_player(
                                    world,
                                    due_tick,
                                    attacker_id,
                                    attacker_name.clone(),
                                    damage,
                                );
                            }
                        }
                    }
                }
            }

            // Opposing monsters within view (needSight: false). The mine itself
            // is excluded by `nearby_opposing_monster_targets` (attacker skip).
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
                view_range,
            ) {
                schedule_damage_to_monster(
                    world,
                    due_tick,
                    attacker_id,
                    target,
                    damage,
                    None,
                    None,
                );
            }
        }
    }

    // Finalise the kill (HP 0, ObjectDied/drops/respawn + quest-kill plumbing),
    // matching `base.Die()`/`CompleteDeath`. Mirror it into our local copy so the
    // caller persists a dead agent.
    let _ = damage_monster_entity(world, entity, i32::MAX, tick, packets);
    agent.dead = true;
}

#[cfg(test)]
mod tests {
    use super::super::super::components::{
        DisplayName, Facing, MonsterCombatStats, MonsterVitals, ObjectId, Position, WorldObject,
        entity_object_id, player_entity,
    };
    use super::super::super::monsters::{
        crystal_dynamic_monster_template, initial_monster_ai_state_for_object,
    };
    use super::super::super::resources::runtime_tick;
    use super::super::super::session::SimulationSession;
    use super::*;
    use crate::{SimulationConfig, WorldEntityDisposition};
    use mir2_protocol::{ClientPacket, MirDirection};

    /// Spawn a real "BoulderSpirit" template (manifest AI == 170) the same way
    /// `spawn_crystal_monster_for_test` does, so the agent reaching the AI is
    /// genuinely AI 170 and routes to `update_boulder_spirit_state`.
    ///
    /// Crystal `BoulderSpirit` is a proximity mine: `ProcessAI` arms `Die()` when
    /// a hostile is in `ViewRange`, and `CompleteDeath` deals
    /// `GetAttackPower(MinDC, MaxDC)` (ACAgility) to everything opposing in view
    /// 300ms later. We drive the AI directly (no shared-dispatch wiring needed,
    /// mirroring `football`'s in-module test) and confirm: (1) a nearby hostile
    /// player arms the mine, (2) once the 300ms deadline passes it dies and
    /// broadcasts `ObjectDied`, and (3) the scheduled detonation damage lands on
    /// the in-view player after a real world tick resolves it.
    #[test]
    fn boulder_spirit_arms_then_detonates_and_damages_player() {
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        let player_origin = Point { x: 322, y: 277 };
        let player = player_entity(session.app.world()).expect("player entity");
        session
            .app
            .world_mut()
            .entity_mut(player)
            .insert(Position(player_origin.clone()));
        let before_hp = session
            .world_snapshot()
            .player_hp
            .expect("player hp present");

        let template =
            crystal_dynamic_monster_template("BoulderSpirit").expect("BoulderSpirit template");
        assert_eq!(
            template.monster_ai, 170,
            "BoulderSpirit manifest AI should be 170"
        );

        // A few tiles away but inside ViewRange (manifest == 7).
        let spirit_object_id = 98_170_u32;
        let spirit_position = Point {
            x: player_origin.x + 3,
            y: player_origin.y,
        };
        let current_tick = runtime_tick(session.app.world());
        let mut agent = MonsterAgent {
            image: template.monster_image,
            dead: false,
            patrol_origin: spirit_position.clone(),
            ai: template.monster_ai,
            disposition: WorldEntityDisposition::Hostile,
            hostile_to_player: true,
            tracking_player: true,
            view_range: i32::from(template.monster_view_range),
            can_wander: true,
            move_interval_ticks: 1,
            attack_interval_ticks: 1,
            next_move_tick: current_tick,
            next_attack_tick: current_tick,
            route: Vec::new(),
            route_index: 0,
            route_waiting: false,
            next_route_tick: current_tick,
        };
        assert_eq!(agent.ai, 170);
        assert!(
            agent.view_range >= 7,
            "ViewRange should cover the 3-tile gap"
        );

        let mut ai_state = initial_monster_ai_state_for_object(
            template.monster_ai,
            current_tick,
            spirit_object_id,
        );
        assert!(
            !ai_state.mode && ai_state.next_state_tick == 0,
            "fresh BoulderSpirit must start un-armed"
        );

        let spirit_entity = session
            .app
            .world_mut()
            .spawn((
                WorldObject,
                Monster,
                ObjectId(spirit_object_id),
                DisplayName::literal(template.monster_name.clone()),
                Position(spirit_position.clone()),
                Facing(MirDirection::Left),
                agent.clone(),
                ai_state,
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
            entity_object_id(session.app.world(), spirit_entity),
            Some(spirit_object_id)
        );

        // Phase 1 — ProcessAI sees the in-view hostile player and arms Die().
        let mut packets = Vec::new();
        let handled = update_boulder_spirit_state(
            session.app.world_mut(),
            spirit_entity,
            &mut agent,
            &mut ai_state,
            &spirit_position,
            &player_origin,
            current_tick,
            &mut packets,
        );
        assert!(
            handled,
            "BoulderSpirit AI must report handled (skip default)"
        );
        assert!(
            ai_state.mode,
            "an in-view hostile player must arm the BoulderSpirit"
        );
        assert!(
            !agent.dead && !agent.can_wander && !agent.tracking_player,
            "armed BoulderSpirit stays inert, not yet dead"
        );
        let detonation_tick = ai_state.next_state_tick;
        assert!(detonation_tick > current_tick, "Die() is delayed ~300ms");

        // Phase 2 — at the deadline it detonates: dies + broadcasts ObjectDied,
        // and schedules the ACAgility damage to the in-view player.
        let mut detonate_packets = Vec::new();
        let handled = update_boulder_spirit_state(
            session.app.world_mut(),
            spirit_entity,
            &mut agent,
            &mut ai_state,
            &spirit_position,
            &player_origin,
            detonation_tick,
            &mut detonate_packets,
        );
        assert!(handled);
        assert!(agent.dead, "BoulderSpirit must be dead after detonation");
        assert!(
            detonate_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectDied { info } if info.object_id == spirit_object_id
            )),
            "detonation must broadcast ObjectDied for the BoulderSpirit"
        );

        // The detonation damage was scheduled into the real world; a normal world
        // tick resolves it and the in-view player loses HP.
        let mut hp_after = before_hp;
        for _ in 0..4 {
            let _ = session.tick();
            hp_after = session
                .world_snapshot()
                .player_hp
                .expect("player hp present");
        }
        assert!(
            hp_after < before_hp,
            "BoulderSpirit detonation should damage the in-view player"
        );
    }
}

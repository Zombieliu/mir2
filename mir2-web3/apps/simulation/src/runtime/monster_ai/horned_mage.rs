//! Monster AI 163 — `HornedMage` (Crystal `Server/MirObjects/Monsters/HornedMage.cs`).
//!
//! `HornedMage : AxeSkeleton`. It inherits AxeSkeleton's fear/kite `ProcessTarget`
//! (`AxeSkeleton.cs:50` — FearTime 5000ms, AttackRange 6, MoveTo when
//! `dist >= AttackRange` else retreat-walk away from the target) but overrides
//! `Attack()` (`HornedMage.cs:15`):
//!
//! * Melee (`!ranged`, target within 3 tiles): broadcast `ObjectAttack`, then a
//!   `DelayedType.Damage` at `Time + 500` dealt with `DefenceType.MACAgility` to
//!   the target AND — via `CompleteAttack` → `FindAllTargets(3, CurrentLocation)`
//!   (`HornedMage.cs:104`) — to every opposing object within radius 3 (splash).
//!   Damage = `GetAttackPower(MinMC, MaxMC)`.
//! * Ranged (`ranged`, target further than 3 tiles): `Envir.Random.Next(5)`:
//!   * `> 0` (4/5): broadcast `ObjectRangeAttack { Type = 0 }` + a
//!     `DelayedType.RangeDamage` at `Time + 800`, single-target,
//!     `DefenceType.AC`. Damage = `GetAttackPower(MinDC, MaxDC)`.
//!   * `== 0` (1/5): broadcast `ObjectRangeAttack { Type = 1 }`, then
//!     `TeleportTarget(4, 4)` (`HornedMage.cs:59`, `:67`) — teleport the target
//!     to a walkable tile within +/-4 of the monster (up to 4 attempts) — deals
//!     NO damage, then re-faces the (now-moved) target.
//!
//! Behaviour-preserving port that uses only existing sim primitives. The fear/kite
//! half mirrors `axe_skeleton_fear.rs` (AI 8) but is reimplemented locally with a
//! hardcoded `AttackRange = 6` so the module is self-contained and does not depend
//! on AI 163 being present in the `monster_attack_range` table.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    current_player_object_id, entity_name, entity_object_id, entity_position, player_entity,
    Facing, Monster, MonsterAgent, MonsterAiState, Position,
};
use super::super::crystal_compat::*;
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

/// `AxeSkeleton.AttackRange` (`AxeSkeleton.cs:10`). HornedMage does not override it.
const HORNED_MAGE_ATTACK_RANGE: i32 = 6;

/// Crystal `Functions.InRange(CurrentLocation, Target.CurrentLocation, 3)` threshold
/// that switches HornedMage between melee and ranged attacks (`HornedMage.cs:23`).
const HORNED_MAGE_MELEE_RANGE: i32 = 3;

/// `FindAllTargets(3, ...)` splash radius for the melee strike (`HornedMage.cs:104`).
const HORNED_MAGE_SPLASH_RADIUS: i32 = 3;

/// `TeleportTarget(attempts = 4, distance = 4)` (`HornedMage.cs:59`).
const HORNED_MAGE_TELEPORT_ATTEMPTS: u64 = 4;
const HORNED_MAGE_TELEPORT_DISTANCE: i32 = 4;

/// Deterministic-RNG salt for this AI (Crystal `Envir.Random`). Mirrors the
/// `agent.ai` salt convention used across the other ported monster modules.
const HORNED_MAGE_AI_SALT: u64 = 163;

pub(in crate::runtime) fn update_horned_mage_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    // Crystal `ProcessTarget`: `if (Target == null || !CanAttack) return;`
    if agent.dead || !monster_can_attack(agent, ai_state) {
        return false;
    }
    if !agent.tracking_player
        && !(agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1))
    {
        return false;
    }

    let distance = tile_distance(position, player_position);
    let in_attack_range = distance <= HORNED_MAGE_ATTACK_RANGE;

    // `if (InAttackRange() && Envir.Time < FearTime) { Attack(); return; }`
    // The fear window is active while `tick < ai_state.next_state_tick`.
    if in_attack_range && tick < ai_state.next_state_tick {
        horned_mage_attack(
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

    // `FearTime = Envir.Time + 5000;`
    ai_state.next_state_tick = tick + FOXMAN_FEAR_DURATION_TICKS;

    // Movement (`MoveTo` / retreat-walk) is rate-limited by the monster move clock.
    if tick < agent.next_move_tick {
        return true;
    }

    // `int dist = MaxDistance(...); if (dist >= AttackRange) MoveTo(target); else <retreat>`.
    let next_step = if distance >= HORNED_MAGE_ATTACK_RANGE {
        monster_step_toward_with_fallback(
            world,
            entity,
            position,
            player_position,
            tick,
            HORNED_MAGE_AI_SALT as usize,
        )
    } else {
        // Retreat: `dir = DirectionFromPoint(Target.CurrentLocation, CurrentLocation)`
        // (away from the player), Walk(dir); on failure rotate Next/Previous (no
        // favour, `Envir.Random.Next(2)`) up to 7 steps. Mirrors axe_skeleton_fear.
        let away_direction = direction_toward(player_position, position);
        let object_id = entity_object_id(world, entity).unwrap_or_default();
        let prefer_next = deterministic_chance_roll(tick, object_id, HORNED_MAGE_AI_SALT, 2);
        let offsets: [i32; 8] = if prefer_next {
            [0, 1, 2, 3, 4, 5, 6, 7]
        } else {
            [0, -1, -2, -3, -4, -5, -6, -7]
        };
        away_direction.and_then(|direction| {
            offsets.into_iter().find_map(|offset| {
                let direction = rotated_direction(direction, offset);
                let next = offset_point(position, direction, 1);
                can_occupy(world, next.clone(), Some(entity)).then_some((next, direction))
            })
        })
    };

    if let Some((next, direction)) = next_step {
        let object_id = entity_object_id(world, entity).unwrap_or_default();
        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        world
            .entity_mut(entity)
            .insert((Position(next.clone()), Facing(direction), agent.clone()));
        packets.push(ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: next,
                direction,
            },
        });
    }

    true
}

/// Crystal `HornedMage.Attack()` (`HornedMage.cs:15`).
fn horned_mage_attack(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    // `AttackTime = Envir.Time + AttackSpeed;` — rate-limit the strike. (Crystal
    // schedules the cooldown unconditionally inside Attack(); we gate on the same
    // clock so the per-tick AI does not fire every tick within the fear window.)
    if tick < agent.next_attack_tick {
        return;
    }

    let Some(attacker_id) = entity_object_id(world, entity) else {
        return;
    };
    let Some(player) = player_entity(world) else {
        return;
    };

    // `ranged = CurrentLocation != Target.CurrentLocation && !InRange(.., .., 3)`.
    let ranged = position != player_position
        && tile_distance(position, player_position) > HORNED_MAGE_MELEE_RANGE;

    // `Direction = DirectionFromPoint(CurrentLocation, Target.CurrentLocation);`
    let Some(direction) = direction_toward(position, player_position) else {
        return;
    };
    world.entity_mut(entity).insert(Facing(direction));

    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    agent.tracking_player = true;
    world.entity_mut(entity).insert(agent.clone());

    let monster_name = entity_name(world, entity).unwrap_or_else(|| "HornedMage".to_string());

    if !ranged {
        // Melee: MC damage, MACAgility, splash radius 3 at Time + 500.
        let base_damage = crystal_monster_raw_magic_damage(&monster_name);
        // Crystal `if (damage == 0) return;` (after broadcasting nothing).
        if base_damage <= 0 {
            return;
        }

        // `Broadcast(new S.ObjectAttack { ... })` — Type 0 melee swing.
        if let Some(packet) = monster_typed_attack_packet(world, entity, position, direction, 0) {
            packets.push(packet);
        }

        let due_tick = tick + combat_delay_ticks(500);
        // Player armour is pre-rolled and subtracted here (the established sim
        // convention for monster-AI scheduled damage); the MACAgility defence-type
        // selection itself is not separately modelled on the player path.
        let mitigation = crystal_player_rolled_armour(world);
        let player_damage = (base_damage - mitigation).max(1);
        schedule_damage_to_player(
            world,
            due_tick,
            attacker_id,
            monster_name.clone(),
            player_damage,
        );

        // `CompleteAttack` → `FindAllTargets(3, CurrentLocation)` splash to every
        // opposing monster within radius 3.
        for target in nearby_monster_splash_targets(world, entity, position, agent) {
            schedule_damage_to_monster(
                world,
                due_tick,
                attacker_id,
                target,
                base_damage,
                None,
                None,
            );
        }
        return;
    }

    // Ranged: `Envir.Random.Next(5)`. `deterministic_chance_roll(.., 5) == true`
    // is the `== 0` case (1/5 → teleport); `false` is `> 0` (4/5 → DC bolt).
    let teleport_branch = deterministic_chance_roll(tick, attacker_id, HORNED_MAGE_AI_SALT, 5);

    if !teleport_branch {
        // 4/5: `ObjectRangeAttack { Type = 0 }` + RangeDamage(Time + 800, DC, AC), single-target.
        let base_damage = crystal_monster_raw_attack_damage(&monster_name);
        if base_damage <= 0 {
            return;
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

        let due_tick = tick + combat_delay_ticks(800);
        let mitigation = crystal_player_rolled_armour(world);
        let player_damage = (base_damage - mitigation).max(1);
        schedule_damage_to_player(world, due_tick, attacker_id, monster_name, player_damage);
    } else {
        // 1/5: `ObjectRangeAttack { Type = 1 }` + TeleportTarget(4, 4), NO damage, re-face.
        if let Some(packet) = monster_typed_ranged_attack_packet(
            world,
            entity,
            position,
            direction,
            player,
            player_position,
            1,
        ) {
            packets.push(packet);
        }

        if horned_mage_teleport_target(world, entity, player, position, tick, packets) {
            // `Direction = DirectionFromPoint(CurrentLocation, Target.CurrentLocation);`
            // re-face the player at its new tile.
            if let Some(new_player_position) = entity_position(world, player) {
                if let Some(new_direction) = direction_toward(position, &new_player_position) {
                    world.entity_mut(entity).insert(Facing(new_direction));
                }
            }
        }
    }
}

/// Crystal `FindAllTargets(3, CurrentLocation)` restricted to opposing monsters
/// (the player is handled separately above). Mirrors the `nearby_opposing_*`
/// splash used by the other AoE bosses.
fn nearby_monster_splash_targets(
    world: &World,
    entity: Entity,
    position: &Point,
    agent: &MonsterAgent,
) -> Vec<Entity> {
    #[allow(deprecated)]
    let monster_entities: Vec<Entity> = world
        .iter_entities()
        .filter_map(|candidate| candidate.contains::<Monster>().then_some(candidate.id()))
        .collect();
    nearby_opposing_monster_targets(
        world,
        &monster_entities,
        entity,
        position,
        agent,
        HORNED_MAGE_SPLASH_RADIUS,
    )
}

/// Crystal `HornedMage.TeleportTarget(attempts, distance)` (`HornedMage.cs:67`)
/// with `distance > 0`: pick a tile at `CurrentLocation + Random(-distance, distance)`
/// on each axis, up to `attempts` tries, teleport the player there if walkable.
/// Returns `true` if the player was moved. Uses deterministic RNG.
fn horned_mage_teleport_target(
    world: &mut World,
    entity: Entity,
    player: Entity,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let Some(player_object_id) = current_player_object_id(world) else {
        return false;
    };
    let object_id = entity_object_id(world, entity).unwrap_or_default();
    // `Random(-distance, distance + 1)` spans `2 * distance + 1` values.
    let span = u64::try_from(HORNED_MAGE_TELEPORT_DISTANCE * 2 + 1).unwrap_or(1);

    for attempt in 0..HORNED_MAGE_TELEPORT_ATTEMPTS {
        let roll_tick = tick + attempt;
        let x_roll = i32::try_from(deterministic_roll(
            roll_tick,
            object_id as usize,
            (HORNED_MAGE_AI_SALT as usize).wrapping_add(attempt as usize * 2),
            span,
        ))
        .unwrap_or(0);
        let y_roll = i32::try_from(deterministic_roll(
            roll_tick,
            object_id as usize,
            (HORNED_MAGE_AI_SALT as usize).wrapping_add(attempt as usize * 2 + 1),
            span,
        ))
        .unwrap_or(0);
        let candidate = Point {
            x: position.x + x_roll - HORNED_MAGE_TELEPORT_DISTANCE,
            y: position.y + y_roll - HORNED_MAGE_TELEPORT_DISTANCE,
        };

        // `Target.Teleport(CurrentMap, location, ...)` — only succeeds on a free,
        // walkable tile. Ignore the player's own tile when testing occupancy.
        if can_occupy(world, candidate.clone(), Some(player)) {
            let direction = direction_toward(position, &candidate).unwrap_or(MirDirection::Up);
            world
                .entity_mut(player)
                .insert((Position(candidate.clone()), Facing(direction)));
            packets.push(ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: player_object_id,
                    position: candidate,
                    direction,
                },
            });
            return true;
        }
    }

    false
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
    use mir2_protocol::{ClientPacket, ServerPacket};

    /// HornedMage inherits AxeSkeleton's fear/kite `ProcessTarget`: with the fear
    /// window not yet active and the target at `dist >= AttackRange (6)`, it sets
    /// FearTime and `MoveTo`s one tile closer (`AxeSkeleton.cs:70-71`). Spawning a
    /// real "HornedMage" template yields `ai == 163`, so this drives the exact AI
    /// path the dispatch routes to. Stat-independent (HornedMage has zero DC/MC).
    #[test]
    fn horned_mage_moves_closer_before_fear_window() {
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

        // Find an open same-row lane: player at P, monster at P+6, P+5 walkable.
        let (player_origin, monster_position, expected_step) = (240..460)
            .find_map(|y| {
                (240..460).find_map(|x| {
                    let player_position = Point { x, y };
                    let monster_position = Point { x: x + 6, y };
                    let expected_step = Point { x: x + 5, y };
                    (can_occupy(session.app.world(), player_position.clone(), Some(player))
                        && can_occupy(session.app.world(), monster_position.clone(), None)
                        && can_occupy(session.app.world(), expected_step.clone(), None))
                    .then_some((player_position, monster_position, expected_step))
                })
            })
            .expect("test should find an open HornedMage approach lane");

        session
            .app
            .world_mut()
            .entity_mut(player)
            .insert(Position(player_origin.clone()));

        let template = crystal_dynamic_monster_template("HornedMage").expect("HornedMage template");
        assert_eq!(
            template.monster_ai, 163,
            "HornedMage manifest AI should be 163"
        );

        let monster_object_id = 778_163_u32;
        let tick = 100_u64;
        let mut agent = MonsterAgent {
            image: template.monster_image,
            dead: false,
            patrol_origin: monster_position.clone(),
            ai: template.monster_ai,
            disposition: WorldEntityDisposition::Hostile,
            hostile_to_player: true,
            tracking_player: true,
            view_range: i32::from(template.monster_view_range),
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
        let monster_entity = session
            .app
            .world_mut()
            .spawn((
                WorldObject,
                Monster,
                ObjectId(monster_object_id),
                DisplayName::literal("HornedMage".to_string()),
                Position(monster_position.clone()),
                Facing(MirDirection::Left),
                agent.clone(),
                MonsterAiState::default(),
                MonsterVitals { hp: 50, max_hp: 50 },
                MonsterCombatStats { agility: 0 },
            ))
            .id();

        let player_position = player_origin.clone();
        let mut ai_state = MonsterAiState::default();
        let mut packets = Vec::new();
        let handled = update_horned_mage_state(
            session.app.world_mut(),
            monster_entity,
            &mut agent,
            &mut ai_state,
            &monster_position,
            &player_position,
            tick,
            &mut packets,
        );

        assert!(
            handled,
            "HornedMage AI should handle the tick (return true)"
        );
        // FearTime was (re)armed for the kite window.
        assert_eq!(ai_state.next_state_tick, tick + FOXMAN_FEAR_DURATION_TICKS);
        // It stepped one tile closer to the player.
        assert_eq!(
            entity_position(session.app.world(), monster_entity).expect("monster position"),
            expected_step,
        );
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement }
                if movement.object_id == monster_object_id
        )));
    }
}

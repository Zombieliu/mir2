//! Monster AI: HornedSorceror (Crystal AI id 169).
//!
//! Faithful port of `Crystal/Server/MirObjects/Monsters/HornedSorceror.cs`
//! `Attack()` / `ProcessTarget()` (and the `Thrust` / `Tornado` / `LineAttack`
//! sub-behaviours it drives via `MonsterObject.cs`). This is a bespoke
//! `ProcessAI` boss: per attack tick it dispatches on HP%, two long cooldown
//! timers (`_ChargedStompTime`, `_TornadoTime`), and an `_Immune` window during
//! the charged stomp.
//!
//! Structure mirrors `kirin_ice_thrust` (dash) and `tucson_general`
//! (line/area + scheduled impacts). The two independent cooldowns + the immune
//! deadline don't fit the shared `MonsterAiState` (one `next_state_tick`), so
//! per-instance timers live in a local `HornedSorcerorTimers` component —
//! exactly the pattern `GeneralMeowMeowState` uses, kept in this module so no
//! shared table is touched. Deterministic rolls only
//! (`deterministic_chance_roll` / `deterministic_roll`), salted by the AI id.

use bevy_ecs::{component::Component, entity::Entity, prelude::World};
use mir2_game_data::crystal_monster_by_name;
use mir2_protocol::{
    MirDirection, ObjectAttackInfo, ObjectMovement, ObjectSpellInfo, Point, ServerPacket, Spell,
};

use super::super::combat::*;
use super::super::components::{
    Facing, Monster, MonsterAgent, MonsterAiState, MonsterVitals, Position, entity_name,
    entity_object_id, entity_position, player_entity,
};
use super::super::monsters::*;
use super::super::movement::*;
use super::super::packets::*;
use super::super::resources::{PendingGroundSpellAction, RuntimeQueueResource, current_language};

use super::common::*;

/// `HornedSorceror.AttackRange` (HornedSorceror.cs:14-20).
const HORNED_SORCEROR_ATTACK_RANGE: i32 = 5;
/// Crystal AI id, used as the deterministic-roll salt base so HornedSorceror's
/// rolls never alias another monster sharing a tick + object id.
const HORNED_SORCEROR_AI: u64 = 169;

/// Per-instance cooldown/immune timers for the HornedSorceror boss. Local to
/// this module (mirrors `GeneralMeowMeowState`) because the two Crystal timers
/// plus the immune deadline exceed the single `MonsterAiState::next_state_tick`.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(in crate::runtime) struct HornedSorcerorTimers {
    /// Earliest tick a fresh charged stomp may start (`_ChargedStompTime`).
    pub(in crate::runtime) stomp_ready_tick: u64,
    /// Earliest tick the dust tornado may fire again (`_TornadoTime`).
    pub(in crate::runtime) tornado_ready_tick: u64,
    /// Tick the active charged-stomp immune window resolves (`_Immune` clear +
    /// delayed AoE). 0 when not stomping.
    pub(in crate::runtime) immune_until_tick: u64,
}

pub(in crate::runtime) fn update_horned_sorceror_state(
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
        return false;
    }

    let mut timers = world
        .entity(entity)
        .get::<HornedSorcerorTimers>()
        .copied()
        .unwrap_or_default();

    // --- Immune (charged-stomp) window upkeep -------------------------------
    // Crystal sets `_Immune = true` for the whole charged stomp and clears it in
    // the delayed `CompleteAttack` (HornedSorceror.cs:68, 262). While immune the
    // boss is untargetable (`IsAttackTarget` returns false, lines 32-40) and
    // skips taking damage. The sim's `ai_state.hidden` flag gives both for free:
    // `is_hidden_or_sleeping_target` (target selection) and
    // `monster_is_damageable` (scheduled-hit resolution) both honour it. We lift
    // it here once the stomp resolves, re-broadcasting the spawn so the client
    // re-shows us.
    if ai_state.hidden {
        if tick >= timers.immune_until_tick {
            ai_state.hidden = false;
            timers.immune_until_tick = 0;
            world.entity_mut(entity).insert(timers);
            rebroadcast_self_spawn(world, entity, packets);
        } else {
            // Mid-stomp: hold position, attack/move suppressed until completion.
            return true;
        }
    }

    if !monster_can_attack(agent, ai_state) {
        return false;
    }

    let distance = tile_distance(position, player_position);
    let has_player_target =
        agent.tracking_player || (agent.hostile_to_player && distance <= agent.view_range.max(1));
    if !has_player_target {
        return false;
    }

    // Crystal `ProcessTarget` (HornedSorceror.cs:135-152): when out of attack
    // range, walk toward `Target.Front`; only `Attack()` when InAttackRange.
    if distance > HORNED_SORCEROR_ATTACK_RANGE {
        return step_toward_target(
            world,
            entity,
            agent,
            position,
            player_position,
            tick,
            packets,
        );
    }

    if tick < agent.next_attack_tick {
        return true;
    }

    let Some(attacker_id) = entity_object_id(world, entity) else {
        return false;
    };
    let Some(direction) = direction_toward(position, player_position) else {
        return false;
    };
    let monster_name = entity_name(world, entity).unwrap_or_else(|| "HornedSorceror".to_string());

    // Crystal `Attack()`: `Direction = DirectionFromPoint(...)` then
    // `AttackTime = now + AttackSpeed` (HornedSorceror.cs:52, 55).
    face_direction(world, entity, direction);
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    agent.tracking_player = true;

    // Crystal `ranged = self == target || !InRange(self, target, 1)` — i.e.
    // anything farther than one tile (HornedSorceror.cs:53).
    let ranged = distance > 1;
    let hp_percent = monster_hp_percent(world, entity);

    // --- Charged Stomp (HornedSorceror.cs:58-78) ----------------------------
    // `now > _ChargedStompTime && HP% < 90 && Random(4) == 0`.
    if hp_percent < 90
        && tick >= timers.stomp_ready_tick
        && deterministic_chance_roll(tick, attacker_id, HORNED_SORCEROR_AI + 1, 4)
    {
        return charged_stomp(
            world,
            entity,
            agent,
            ai_state,
            &mut timers,
            position,
            direction,
            attacker_id,
            &monster_name,
            tick,
            packets,
        );
    }

    // --- Dust Tornado (HornedSorceror.cs:81-87) -----------------------------
    // `now > _TornadoTime && HP% < 90 && Random(4) == 0`.
    if hp_percent < 90
        && tick >= timers.tornado_ready_tick
        && deterministic_chance_roll(tick, attacker_id, HORNED_SORCEROR_AI + 2, 4)
    {
        return dust_tornado(
            world,
            entity,
            agent,
            &mut timers,
            position,
            direction,
            attacker_id,
            &monster_name,
            tick,
            packets,
        );
    }

    if !ranged {
        // Crystal adjacent branch (HornedSorceror.cs:89-119).
        if deterministic_roll(
            tick,
            attacker_id as usize,
            HORNED_SORCEROR_AI as usize + 3,
            5,
        ) > 2
        {
            // Thrust hit: Type 0, DC 2-tile LineAttack (lines 91-101).
            push_attack_packet(world, entity, position, direction, 0, 0, packets);
            let damage = crystal_monster_raw_attack_damage(&monster_name);
            line_attack(
                world,
                entity,
                agent,
                position,
                direction,
                2,
                damage,
                attacker_id,
                &monster_name,
                tick,
            );
            return true;
        }
        if deterministic_roll(
            tick,
            attacker_id as usize,
            HORNED_SORCEROR_AI as usize + 4,
            5,
        ) > 2
        {
            // Dust hit: Type 1, MC 3-tile LineAttack (lines 102-112).
            push_attack_packet(world, entity, position, direction, 1, 0, packets);
            let damage = crystal_monster_raw_magic_damage(&monster_name);
            line_attack(
                world,
                entity,
                agent,
                position,
                direction,
                3,
                damage,
                attacker_id,
                &monster_name,
                tick,
            );
            return true;
        }
        // else: Thrust dash (lines 113-118).
        thrust(
            world,
            entity,
            agent,
            position,
            player_position,
            attacker_id,
            &monster_name,
            tick,
            packets,
        );
        return true;
    }

    // Crystal ranged branch (HornedSorceror.cs:120-132): 1/3 Thrust, else step
    // toward the target's current location.
    if deterministic_chance_roll(tick, attacker_id, HORNED_SORCEROR_AI + 5, 3) {
        thrust(
            world,
            entity,
            agent,
            position,
            player_position,
            attacker_id,
            &monster_name,
            tick,
            packets,
        );
    } else {
        // `MoveTo(Target.CurrentLocation)` — chase one step.
        step_toward_target(
            world,
            entity,
            agent,
            position,
            player_position,
            tick,
            packets,
        );
    }
    true
}

/// Crystal `Charged Stomp` (HornedSorceror.cs:58-78 + `CompleteAttack`
/// 255-279). Broadcasts `ObjectAttack Type=2 Level=stompLoops`, goes immune for
/// the wind-up, and schedules `SC * loops` AC damage to everything within
/// radius 2 at completion (Crystal `FindAllTargets(2)` AoE).
#[allow(clippy::too_many_arguments)]
fn charged_stomp(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    timers: &mut HornedSorcerorTimers,
    position: &Point,
    direction: MirDirection,
    attacker_id: u32,
    monster_name: &str,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    // `stompLoops = Random(5, 10)` — C# `Next(5, 10)` is 5..=9 inclusive.
    let roll = deterministic_roll(
        tick,
        attacker_id as usize,
        HORNED_SORCEROR_AI as usize + 6,
        5,
    );
    let stomp_loops = (5 + roll).clamp(5, 9) as u8;
    let stomp_duration_ms = u64::from(stomp_loops) * 500;
    let completion_ms = stomp_duration_ms + 500;
    let completion_tick = tick + combat_delay_ticks(completion_ms);

    // Crystal: `ActionTime = now + duration + 500`, `AttackTime = ... + AttackSpeed`.
    agent.next_attack_tick = completion_tick + agent.attack_interval_ticks.max(1);
    agent.next_move_tick = completion_tick;

    // `_Immune = true` + `_ChargedStompTime = now + 20000`.
    ai_state.hidden = true;
    timers.immune_until_tick = completion_tick;
    timers.stomp_ready_tick = tick + combat_delay_ticks(20_000);
    world.entity_mut(entity).insert(*timers);

    face_direction(world, entity, direction);
    push_attack_packet(world, entity, position, direction, 2, stomp_loops, packets);

    // `damage = GetAttackPower(MinSC, MaxSC) * stompLoops`; if 0, Crystal returns
    // (no DelayedAction) — but `_Immune`/`_ChargedStompTime` are already set, so
    // the boss still goes immune. We mirror that: schedule damage only when > 0.
    let base = horned_sorceror_raw_spell_damage(monster_name);
    let damage = base * i32::from(stomp_loops);
    if damage > 0 {
        // Crystal `CompleteAttack` with aoe=true: `FindAllTargets(2)` then
        // `Attacked` on each (radius-2 Chebyshev around the boss).
        if agent.hostile_to_player {
            if let Some(player) = player_entity(world) {
                if entity_position(world, player)
                    .map(|p| tile_distance(position, &p) <= 2)
                    .unwrap_or(false)
                {
                    schedule_damage_to_player(
                        world,
                        completion_tick,
                        attacker_id,
                        monster_name.to_string(),
                        damage,
                    );
                }
            }
        }
        let monster_entities: Vec<Entity> = world
            .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
            .iter(world)
            .collect();
        for target in
            nearby_opposing_monster_targets(world, &monster_entities, entity, position, agent, 2)
        {
            schedule_damage_to_monster(
                world,
                completion_tick,
                attacker_id,
                target,
                damage,
                None,
                None,
            );
        }
    }

    world.entity_mut(entity).insert(agent.clone());
    true
}

/// Crystal `Dust Tornado` / `Tornado()` (HornedSorceror.cs:81-87, 154-198).
/// Broadcasts `ObjectAttack Type=3`, then spawns a 5x5 grid of
/// `HornedSorcererDustTornado` SpellObjects centred on the boss — each ticks `MC`
/// damage every second for 15 s, starting 1 s after the cast. We register one
/// persistent ground-spell field over the whole box; `tick_ground_spell_actions`
/// then drives the per-second `MC` ticks against the player + opposing monsters
/// (replacing the previous single-impact approximation). Like Crystal we lay the
/// field unconditionally (the visual always shows); the tick damage is `MC`, which
/// is 0 in the stock manifest and scales up for future bosses.
#[allow(clippy::too_many_arguments)]
fn dust_tornado(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    timers: &mut HornedSorcerorTimers,
    position: &Point,
    direction: MirDirection,
    attacker_id: u32,
    monster_name: &str,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    // `_TornadoTime = now + 15000`.
    timers.tornado_ready_tick = tick + combat_delay_ticks(15_000);
    world.entity_mut(entity).insert(*timers);

    face_direction(world, entity, direction);
    push_attack_packet(world, entity, position, direction, 3, 0, packets);

    // 5x5 box centred on the boss (Crystal `location.X/Y - 2 ..= + 2`).
    let damage = crystal_monster_raw_magic_damage(monster_name);
    let locations: Vec<Point> = (-2..=2)
        .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| Point {
            x: position.x + dx,
            y: position.y + dy,
        })
        .collect();
    // Crystal `start = 1000`, `time = Settings.Second * 15`, `TickSpeed = 1000`,
    // `ExpireTime = now + time + start` (HornedSorceror.cs:176-183).
    let start_delay = combat_delay_ticks(1_000);
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_ground_spell_actions
        .push(PendingGroundSpellAction {
            spell: Spell::HornedSorcererDustTornado,
            caster_object_id: attacker_id,
            locations,
            damage,
            next_tick: tick + start_delay,
            expires_at_tick: tick + combat_delay_ticks(16_000),
            tick_interval: combat_delay_ticks(1_000),
        });
    // Crystal shows only the centre cell's tornado (`Show = location == cell`).
    let visual_object_id = allocate_runtime_monster_object_id(world);
    queue_due_packet(
        world,
        tick + start_delay,
        ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id: visual_object_id,
                location: position.clone(),
                spell: Spell::HornedSorcererDustTornado,
                direction,
                param: false,
            },
        },
    );

    world.entity_mut(entity).insert(agent.clone());
    true
}

/// Crystal `Thrust(target)` (HornedSorceror.cs:200-232). Dashes up to 3 tiles
/// toward the target, scheduling `MaxDC` AC `RangeDamage` at every landed tile
/// at +500 ms, then broadcasts `ObjectDashAttack Distance = 3`. If the 3 tiles
/// ahead are not all valid, Crystal returns without moving (no dash).
#[allow(clippy::too_many_arguments)]
fn thrust(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    target_position: &Point,
    attacker_id: u32,
    monster_name: &str,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(jump_dir) = direction_toward(position, target_position) else {
        return;
    };

    // Crystal first validates all three tiles ahead, bailing entirely if any is
    // invalid (lines 206-210). Mirror with `can_occupy` for the dash path.
    for step in 1..=3 {
        let cell = offset_point(position, jump_dir, step);
        if !can_occupy(world, cell.clone(), Some(entity)) {
            return;
        }
    }

    // Dash lands 3 tiles ahead; ActionTime/move pacing set on the boss.
    let landing = offset_point(position, jump_dir, 3);
    let damage = crystal_monster_raw_attack_damage(monster_name);
    let due_tick = tick + combat_delay_ticks(500);

    // Crystal schedules a RangeDamage at each of the three landed tiles.
    // Replicate by hitting any opposing occupant on tiles 1..=3 along the path.
    if damage > 0 {
        let monster_entities: Vec<Entity> = world
            .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
            .iter(world)
            .collect();
        let player = player_entity(world);
        for step in 1..=3 {
            let cell = offset_point(position, jump_dir, step);
            if agent.hostile_to_player {
                if let Some(player) = player {
                    if entity_position(world, player)
                        .map(|p| p == cell)
                        .unwrap_or(false)
                    {
                        schedule_damage_to_player(
                            world,
                            due_tick,
                            attacker_id,
                            monster_name.to_string(),
                            damage,
                        );
                    }
                }
            }
            for target in
                nearby_opposing_monster_targets(world, &monster_entities, entity, &cell, agent, 0)
            {
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

    agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
    world
        .entity_mut(entity)
        .insert((Position(landing.clone()), Facing(jump_dir), agent.clone()));
    packets.push(ServerPacket::ObjectDashAttack {
        object_id: attacker_id,
        location: landing,
        direction: jump_dir,
        distance: 3,
    });
}

/// Crystal `LineAttack(damage, distance, 300)` (MonsterObject.cs:3611-3643).
/// For each tile 1..=distance along `direction`, schedule `damage` to the first
/// attackable occupant at `MaxDistance*50 + 300` ms. Crystal early-returns on a
/// 0 GetAttackPower roll before calling LineAttack, so a 0 here is a no-op.
#[allow(clippy::too_many_arguments)]
fn line_attack(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
    direction: MirDirection,
    distance: i32,
    damage: i32,
    attacker_id: u32,
    monster_name: &str,
    tick: u64,
) {
    if damage <= 0 {
        return;
    }
    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    let player = player_entity(world);

    for amount in 1..=distance {
        let cell = offset_point(position, direction, amount);
        // Crystal delay: `MaxDistance(self, cell) * 50 + additionalDelay`.
        let step_ms = u64::try_from(amount.max(0)).unwrap_or(0) * 50 + 300;
        let due_tick = tick + combat_delay_ticks(step_ms);

        if agent.hostile_to_player {
            if let Some(player) = player {
                if entity_position(world, player)
                    .map(|p| p == cell)
                    .unwrap_or(false)
                {
                    schedule_damage_to_player(
                        world,
                        due_tick,
                        attacker_id,
                        monster_name.to_string(),
                        damage,
                    );
                }
            }
        }
        for target in
            nearby_opposing_monster_targets(world, &monster_entities, entity, &cell, agent, 0)
        {
            schedule_damage_to_monster(world, due_tick, attacker_id, target, damage, None, None);
        }
    }
}

/// Crystal `MoveTo(target)` chase step (HornedSorceror.cs:130, 151). One
/// deterministic greedy/path-routed step toward `target`, emitting `ObjectWalk`.
fn step_toward_target(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    target: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if tick < agent.next_move_tick {
        return true;
    }
    if let Some((next, direction)) = monster_step_toward_with_fallback(
        world,
        entity,
        position,
        target,
        tick,
        HORNED_SORCEROR_AI as usize,
    ) {
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
    } else {
        world.entity_mut(entity).insert(agent.clone());
    }
    true
}

fn face_direction(world: &mut World, entity: Entity, direction: MirDirection) {
    if world
        .entity(entity)
        .get::<Facing>()
        .map(|facing| facing.0 != direction)
        .unwrap_or(true)
    {
        world.entity_mut(entity).insert(Facing(direction));
    }
}

/// Build + push an `ObjectAttack` carrying a non-zero `level` (the stomp loop
/// count). `monster_typed_attack_packet` hardcodes `level = 0`, so the charged
/// stomp (`Type=2 Level=stompLoops`) needs the struct directly.
fn push_attack_packet(
    world: &World,
    entity: Entity,
    location: &Point,
    direction: MirDirection,
    attack_type: u8,
    level: u8,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(object_id) = entity_object_id(world, entity) else {
        return;
    };
    packets.push(ServerPacket::ObjectAttack {
        info: ObjectAttackInfo {
            object_id,
            location: location.clone(),
            direction,
            spell: 0,
            level,
            attack_type,
        },
    });
}

/// Re-broadcast our spawn packet so the client re-shows us when the immune
/// (hidden) window lifts — mirrors `general_meow_meow`'s shield-toggle respawn.
fn rebroadcast_self_spawn(world: &mut World, entity: Entity, packets: &mut Vec<ServerPacket>) {
    if let Some((_, bundle)) =
        visible_object_bundle_for_entity(world, entity, current_language(world))
    {
        if matches!(bundle.spawn_packet, ServerPacket::ObjectMonster { .. }) {
            packets.push(bundle.spawn_packet);
        }
    }
}

fn monster_hp_percent(world: &World, entity: Entity) -> i32 {
    world
        .entity(entity)
        .get::<MonsterVitals>()
        .map(|vitals| vitals.hp.max(0) * 100 / vitals.max_hp.max(1))
        .unwrap_or(100)
}

/// Raw `GetAttackPower(MinSC, MaxSC)` base (no `.max(1)` floor) for the charged
/// stomp. The shared `crystal_monster_spell_damage` clamps to >= 1, which would
/// break Crystal's `if (damage == 0) return;` guard (HornedSorceror.cs:73);
/// HornedSorceror ships with SC = 0 in the manifest, so we read the raw range
/// directly (matching `crystal_monster_raw_attack/magic_damage`).
fn horned_sorceror_raw_spell_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_sc.max(monster.min_sc))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::super::super::components::{DisplayName, ObjectId, WorldObject};
    use super::super::super::session::SimulationSession;
    use super::*;
    use crate::{SimulationConfig, WorldEntityDisposition};
    use mir2_protocol::ClientPacket;
    // `monster_is_damageable`, `player_entity`, `entity_position`, `can_occupy`,
    // `Monster`, `MonsterAgent`, `MonsterAiState`, `Facing`, `Position` are brought
    // in via the module-level globs (`use super::super::{monsters,components,movement}::*`
    // etc.), re-exported into this test by `use super::*`.

    /// A fully-resourced session world (the bare `World::new()` used by
    /// `horned_commander`'s dead-boss test panics the moment any AI branch reads
    /// `MapRuntimeResource` / `RuntimeQueueResource` / `SessionResource`, which
    /// HornedSorceror's combat + immune-recovery paths do). We spawn the boss
    /// directly into the session world and drive `update_horned_sorceror_state`,
    /// so the test needs none of the `tests.rs` fixtures (out of reach here).
    fn session_with_player() -> SimulationSession {
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session
    }

    fn place_player(session: &mut SimulationSession, position: Point) {
        let player = player_entity(session.app.world()).expect("player entity");
        session
            .app
            .world_mut()
            .entity_mut(player)
            .insert(Position(position));
    }

    fn spawn_boss(
        session: &mut SimulationSession,
        object_id: u32,
        position: Point,
        hp: i32,
        max_hp: i32,
    ) -> Entity {
        session
            .app
            .world_mut()
            .spawn((
                WorldObject,
                Monster,
                ObjectId(object_id),
                DisplayName::literal("HornedSorceror".to_string()),
                Position(position.clone()),
                Facing(MirDirection::Left),
                MonsterAgent {
                    image: 346,
                    dead: false,
                    patrol_origin: position,
                    ai: HORNED_SORCEROR_AI as u8,
                    disposition: WorldEntityDisposition::Hostile,
                    hostile_to_player: true,
                    tracking_player: true,
                    view_range: 7,
                    can_wander: false,
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
                MonsterVitals { hp, max_hp },
            ))
            .id()
    }

    fn run_tick(
        session: &mut SimulationSession,
        entity: Entity,
        tick: u64,
    ) -> (MonsterAiState, Vec<ServerPacket>) {
        let mut agent = session
            .app
            .world()
            .entity(entity)
            .get::<MonsterAgent>()
            .cloned()
            .expect("boss agent");
        let mut ai_state = *session
            .app
            .world()
            .entity(entity)
            .get::<MonsterAiState>()
            .expect("boss ai state");
        let position = session
            .app
            .world()
            .entity(entity)
            .get::<Position>()
            .expect("boss position")
            .0
            .clone();
        let player = player_entity(session.app.world()).expect("player entity");
        let player_position =
            entity_position(session.app.world(), player).expect("player position");
        let mut packets = Vec::new();
        update_horned_sorceror_state(
            session.app.world_mut(),
            entity,
            &mut agent,
            &mut ai_state,
            &position,
            &player_position,
            tick,
            &mut packets,
        );
        // Persist like the real dispatch site does.
        session.app.world_mut().entity_mut(entity).insert(ai_state);
        session.app.world_mut().entity_mut(entity).insert(agent);
        (ai_state, packets)
    }

    /// A dead boss must defer entirely to the default pass (returns false) and
    /// emit nothing — exercises the real dispatch entry point with the exact
    /// component set a spawned HornedSorceror carries.
    #[test]
    fn dead_boss_falls_through_to_default() {
        let mut session = session_with_player();
        let object_id = 99_169_u32;
        let boss_position = Point { x: 900, y: 900 };
        place_player(&mut session, Point { x: 901, y: 900 });
        let entity = spawn_boss(&mut session, object_id, boss_position.clone(), 50, 100);
        session
            .app
            .world_mut()
            .entity_mut(entity)
            .get_mut::<MonsterAgent>()
            .expect("agent")
            .dead = true;

        let mut agent = session
            .app
            .world()
            .entity(entity)
            .get::<MonsterAgent>()
            .cloned()
            .unwrap();
        let mut ai_state = MonsterAiState::default();
        let player_position = Point { x: 901, y: 900 };
        let mut packets = Vec::new();
        let handled = update_horned_sorceror_state(
            session.app.world_mut(),
            entity,
            &mut agent,
            &mut ai_state,
            &boss_position,
            &player_position,
            100,
            &mut packets,
        );
        assert!(!handled, "dead boss should fall through to default AI");
        assert!(packets.is_empty(), "dead boss should emit no packets");
    }

    /// In attack range (<=5) and tracking, the boss must run its bespoke
    /// `Attack()` and broadcast some attack packet (`ObjectAttack` for the
    /// line/stomp/tornado branches, or `ObjectDashAttack` for the Thrust dash) —
    /// proving the AI-169 ProcessAI dispatch is wired. The 0-stat manifest means
    /// no damage is scheduled, so any of the adjacent branches is a safe no-op
    /// beyond its broadcast packet.
    #[test]
    fn in_range_boss_emits_an_attack_packet() {
        let mut session = session_with_player();
        let object_id = 99_170_u32;
        // Adjacent target → the `!ranged` branch (line attack or thrust); all of
        // them broadcast an attack packet.
        let boss_position = Point { x: 900, y: 900 };
        place_player(&mut session, Point { x: 901, y: 900 });
        let entity = spawn_boss(&mut session, object_id, boss_position, 100, 100);

        // Sweep a few ticks so we don't depend on a single tick's branch roll;
        // HP is full, so neither stomp nor tornado fires and we stay in the
        // adjacent line/thrust branches, each of which emits a packet.
        let mut saw_attack = false;
        for tick in 5..15 {
            // Reset the attack cooldown each tick so Attack() runs.
            session
                .app
                .world_mut()
                .entity_mut(entity)
                .get_mut::<MonsterAgent>()
                .expect("agent")
                .next_attack_tick = 0;
            let (_, packets) = run_tick(&mut session, entity, tick);
            if packets.iter().any(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectAttack { info } if info.object_id == object_id
                ) || matches!(
                    packet,
                    ServerPacket::ObjectDashAttack { object_id: id, .. } if *id == object_id
                )
            }) {
                saw_attack = true;
                break;
            }
        }
        assert!(
            saw_attack,
            "in-range HornedSorceror should broadcast an ObjectAttack or ObjectDashAttack"
        );
    }

    /// The charged-stomp immune window must make the boss untargetable: while
    /// `ai_state.hidden` is set (the sim's `_Immune` analog) `monster_is_damageable`
    /// returns false, so scheduled hits are skipped; the boss then lifts immunity
    /// at the parked completion tick and is damageable again. Drives the immune
    /// upkeep at the top of `update_horned_sorceror_state` directly.
    #[test]
    fn charged_stomp_immune_window_blocks_damage_then_recovers() {
        let mut session = session_with_player();
        let object_id = 99_171_u32;
        let boss_position = Point { x: 905, y: 905 };
        place_player(&mut session, Point { x: 906, y: 905 });
        let entity = spawn_boss(&mut session, object_id, boss_position, 50, 100);

        // Enter the immune window exactly as `charged_stomp` does.
        session
            .app
            .world_mut()
            .entity_mut(entity)
            .insert(HornedSorcerorTimers {
                stomp_ready_tick: 100,
                tornado_ready_tick: 0,
                immune_until_tick: 50,
            });
        session
            .app
            .world_mut()
            .entity_mut(entity)
            .get_mut::<MonsterAiState>()
            .expect("ai state")
            .hidden = true;

        // While hidden, the boss must be undamageable (the `_Immune` contract).
        assert!(
            !monster_is_damageable(session.app.world(), entity),
            "HornedSorceror must be undamageable while the immune (hidden) window is active"
        );

        // Tick BEFORE the deadline: the boss holds the tick and stays hidden.
        let (ai_state, _) = run_tick(&mut session, entity, 40);
        assert!(ai_state.hidden, "boss must stay immune before the deadline");

        // Tick AT the deadline: immunity lifts, boss becomes targetable again.
        let (ai_state, _) = run_tick(&mut session, entity, 50);
        assert!(
            !ai_state.hidden,
            "HornedSorceror must lift immunity once the stomp window elapses"
        );
        assert!(
            monster_is_damageable(session.app.world(), entity),
            "HornedSorceror must be damageable again after the immune window resolves"
        );
    }

    /// Task 3: the dust tornado registers a PERSISTENT MC ground field (Crystal
    /// `Tornado()` lays a 5x5 grid of `HornedSorcererDustTornado` SpellObjects that
    /// tick every second for 15 s) instead of a single impact. Drive the real AI
    /// until the 1/4 tornado roll fires — the charged stomp is gated off so the only
    /// reachable field-laying branch is the tornado — then assert the queued field's
    /// 5x5 shape, caster, and Crystal timing.
    #[test]
    fn dust_tornado_registers_a_persistent_mc_field() {
        let mut session = session_with_player();
        let object_id = 99_172_u32;
        let entity = spawn_boss(&mut session, object_id, Point { x: 920, y: 920 }, 50, 100);
        // Gate OFF the charged stomp (cooldown far in the future) so the only
        // field-laying branch the AI can reach is the dust tornado (ready now).
        session
            .app
            .world_mut()
            .entity_mut(entity)
            .insert(HornedSorcerorTimers {
                stomp_ready_tick: u64::MAX,
                tornado_ready_tick: 0,
                immune_until_tick: 0,
            });

        let mut fired = None;
        for tick in 1..400u64 {
            // Keep the player adjacent to the boss (which may dash/step) so it stays
            // in attack range and rolls the tornado each tick; reset the attack
            // cooldown so `Attack()` runs.
            let boss_position =
                entity_position(session.app.world(), entity).expect("boss position");
            place_player(
                &mut session,
                Point {
                    x: boss_position.x + 1,
                    y: boss_position.y,
                },
            );
            session
                .app
                .world_mut()
                .entity_mut(entity)
                .get_mut::<MonsterAgent>()
                .expect("agent")
                .next_attack_tick = 0;
            let _ = run_tick(&mut session, entity, tick);
            let field = session
                .app
                .world()
                .resource::<RuntimeQueueResource>()
                .pending_ground_spell_actions
                .iter()
                .find(|action| action.spell == Spell::HornedSorcererDustTornado)
                .cloned();
            if let Some(field) = field {
                fired = Some((tick, field));
                break;
            }
        }

        let (tick, field) = fired.expect("the dust tornado must register a persistent field");
        let boss_position = entity_position(session.app.world(), entity).expect("boss position");
        assert_eq!(field.caster_object_id, object_id, "field owned by the boss");
        assert_eq!(field.locations.len(), 25, "Crystal Tornado lays a 5x5 grid");
        assert!(
            field.locations.contains(&boss_position),
            "the field must be centred on the boss"
        );
        // Crystal: first tick +1000ms, TickSpeed 1000ms, ExpireTime now + 15s + 1s.
        assert_eq!(field.next_tick, tick + combat_delay_ticks(1_000));
        assert_eq!(field.tick_interval, combat_delay_ticks(1_000));
        assert_eq!(field.expires_at_tick, tick + combat_delay_ticks(16_000));
    }
}

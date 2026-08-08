//! Monster AI: HornedWarrior (Crystal AI id 165).
//!
//! Faithful port of `Crystal/Server/MirObjects/Monsters/HornedWarrior.cs`
//! (base chain `MonsterObject.cs`). A range-4 boss that:
//!   * grants itself a damage-reduction "shield" once it drops below 50% HP,
//!   * while shielded, refuses to attack and kites *away* from its target,
//!   * unshielded + adjacent, 2/3 of the time lands a single-target DC strike,
//!   * otherwise unleashes a `WideLineAttack` (length 4, width 3) DC splash that
//!     hits the player and any opposing monsters caught in the three lanes.
//!
//! Deterministic-RNG only: Crystal `Envir.Random.Next(n)` /
//! `Random.Next(0, 5000)` map to `deterministic_chance_roll` /
//! `deterministic_roll` seeded by `tick`, the attacker object id, and the AI id
//! (165) as the salt (see `super::super::combat`).

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    entity_facing, entity_name, entity_object_id, entity_position, Facing, Monster, MonsterAgent,
    MonsterAiState, MonsterVitals, Position,
};
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

/// Crystal `HornedWarrior.AttackRange = 4` (`HornedWarrior.cs:9`).
pub(in crate::runtime) const HORNED_WARRIOR_AI: u8 = 165;
const HORNED_WARRIOR_ATTACK_RANGE: i32 = 4;
/// Crystal shield cooldown: `Envir.Time + 15000 + Envir.Random.Next(0, 5000)`
/// (`HornedWarrior.cs:53`).
const HORNED_WARRIOR_SHIELD_BASE_MS: u64 = 15_000;
const HORNED_WARRIOR_SHIELD_RANDOM_MS: u64 = 5_000;
/// Crystal `WideLineAttack(damage, 4, 500, ACAgility, false, 3)`
/// (`HornedWarrior.cs:82`): line length 4, width 3, +500ms base delay.
const HORNED_WARRIOR_WIDE_LINE_LENGTH: i32 = 4;
const HORNED_WARRIOR_WIDE_LINE_BASE_DELAY_MS: u64 = 500;

/// Crystal `HornedWarrior.ProcessTarget` / `Attack` (`HornedWarrior.cs:29-134`).
///
/// Returns `true` when this monster's custom AI handled the tick (the dispatch
/// then skips the default melee/chase). Returns `false` only when the monster is
/// unshielded and out of attack range, so the shared chase path runs — Crystal's
/// `MoveTo(Target.CurrentLocation)` (`HornedWarrior.cs:105`).
pub(in crate::runtime) fn update_horned_warrior_state(
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

    // Crystal `ProcessTarget`: requires a valid attack target. Mirror the shared
    // aggro gate other bosses use (tracking, or hostile + within view range).
    let has_player_target = agent.tracking_player
        || (agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1));
    if !has_player_target {
        return false;
    }

    // Crystal: `var buff = HasBuff(BuffType.HornedWarriorShield, out _);`
    // (`HornedWarrior.cs:90`). The shield window lives in `next_state_tick`; the
    // spec collapses Crystal's separate 10s buff + 15-20s cooldown into a single
    // 15-20s shielded window (see module-level `deferredParts` note).
    let shielded = tick < ai_state.next_state_tick;
    ai_state.mode = shielded;

    if shielded {
        // Crystal `ProcessTarget` buff branch (`HornedWarrior.cs:107-133`):
        // refuse to attack and walk *away* from the target.
        return horned_warrior_move_away(
            world,
            entity,
            agent,
            position,
            player_position,
            tick,
            packets,
        );
    }

    // Crystal: `if (InAttackRange() && !buff) { Attack(); return; }`
    // (`HornedWarrior.cs:92-96`). Out of range + unshielded → fall through to the
    // shared chase (Crystal `MoveTo`).
    if !horned_warrior_in_attack_range(position, player_position) {
        return false;
    }

    // Attack cadence: Crystal sets `AttackTime = Envir.Time + AttackSpeed`
    // (`HornedWarrior.cs:41`). Gate on `next_attack_tick` so we honour AttackSpeed.
    if tick < agent.next_attack_tick {
        // In range but still on attack cooldown: hold position (we own the tick).
        return true;
    }

    let Some(attacker_id) = entity_object_id(world, entity) else {
        return false;
    };
    let Some(direction) = direction_toward(position, player_position) else {
        return false;
    };

    // Crystal `Attack`: `Direction = Functions.DirectionFromPoint(...)`
    // (`HornedWarrior.cs:37`).
    agent.tracking_player = true;
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    if entity_facing(world, entity) != Some(direction) {
        world.entity_mut(entity).insert(Facing(direction));
    }

    let vitals = world.entity(entity).get::<MonsterVitals>().copied();
    let hp_percent = vitals
        .map(|vitals| vitals.hp.max(0) * 100 / vitals.max_hp.max(1))
        .unwrap_or(100);

    // Crystal `Attack`: `if (Envir.Time > _ShieldTime && hpPercent < 50)` → grant
    // the shield, broadcast `ObjectAttack { Type = 2 }`, and deal no damage this
    // tick (`HornedWarrior.cs:51-66`). Reaching here implies unshielded already,
    // so `tick >= next_state_tick` holds.
    if hp_percent < 50 {
        let extra = deterministic_roll(
            tick,
            attacker_id as usize,
            HORNED_WARRIOR_AI as usize,
            HORNED_WARRIOR_SHIELD_RANDOM_MS,
        );
        ai_state.next_state_tick = tick + combat_delay_ticks(HORNED_WARRIOR_SHIELD_BASE_MS + extra);
        ai_state.mode = true;
        if let Some(packet) = monster_typed_attack_packet(world, entity, position, direction, 2) {
            packets.push(packet);
        }
        return true;
    }

    // Crystal `Attack`: `int damage = GetAttackPower(MinDC, MaxDC);`
    // (`HornedWarrior.cs:68`). `monster_player_attack_damage` rolls DC for AI 165
    // (the default arm) and applies the player's AC mitigation = `ACAgility`.
    let monster_name = entity_name(world, entity).unwrap_or_else(|| "HornedWarrior".to_string());
    let damage = monster_player_attack_damage(
        world,
        attacker_id,
        &monster_name,
        agent,
        position,
        player_position,
    );
    if damage <= 0 {
        // Crystal `if (damage == 0) return;` (`HornedWarrior.cs:69`).
        return true;
    }

    // Crystal `ranged` (`HornedWarrior.cs:38`):
    //   `CurrentLocation == Target.CurrentLocation || !InRange(loc, target, 1)`.
    // `InRange(a,b,1)` is Chebyshev <= 1, so `!ranged` means strictly adjacent
    // (Chebyshev == 1; same-tile is the `ranged` case).
    let adjacent = tile_distance(position, player_position) == 1;

    // Crystal: `if (!ranged && Envir.Random.Next(3) > 0)` (`HornedWarrior.cs:71`).
    // `Random.Next(3) > 0` is the 2/3 branch (result 1 or 2). Our
    // `deterministic_chance_roll(.., 3)` returns true on the 1/3 case
    // (`value % 3 == 0` == Crystal's `== 0`), so the 2/3 branch is its negation.
    let single_target_roll =
        !deterministic_chance_roll(tick, attacker_id, HORNED_WARRIOR_AI as u64, 3);

    if adjacent && single_target_roll {
        // Crystal: broadcast `ObjectAttack { Type = 0 }` + a single DC hit at
        // `Envir.Time + 300` (`HornedWarrior.cs:73-76`).
        if let Some(packet) = monster_typed_attack_packet(world, entity, position, direction, 0) {
            packets.push(packet);
        }
        schedule_damage_to_player(
            world,
            tick + combat_delay_ticks(300),
            attacker_id,
            monster_name,
            damage,
        );
        return true;
    }

    // Crystal else branch: broadcast `ObjectAttack { Type = 1 }` +
    // `WideLineAttack(damage, 4, 500, ACAgility, false, 3)`
    // (`HornedWarrior.cs:80-82`).
    if let Some(packet) = monster_typed_attack_packet(world, entity, position, direction, 1) {
        packets.push(packet);
    }
    horned_warrior_wide_line_attack(
        world,
        entity,
        agent,
        attacker_id,
        &monster_name,
        position,
        player_position,
        direction,
        damage,
        tick,
    );
    true
}

/// Crystal `HornedWarrior.InAttackRange` (`HornedWarrior.cs:16-27`):
/// `x,y` are absolute deltas; reject if either exceeds `AttackRange` (4) or both
/// are zero, then accept when `(x<=1 && y<=1) || (x==y || x%2==y%2)`.
fn horned_warrior_in_attack_range(source: &Point, target: &Point) -> bool {
    let x = (target.x - source.x).abs();
    let y = (target.y - source.y).abs();
    if x == 0 && y == 0 {
        return false;
    }
    if x > HORNED_WARRIOR_ATTACK_RANGE || y > HORNED_WARRIOR_ATTACK_RANGE {
        return false;
    }
    (x <= 1 && y <= 1) || (x == y || x % 2 == y % 2)
}

/// Crystal `ProcessTarget` buff branch (`HornedWarrior.cs:107-133`): face *away*
/// from the target (`DirectionFromPoint(Target, Current)`), try to `Walk(dir)`,
/// and on failure rotate alternately clockwise / counter-clockwise looking for an
/// open tile. We own the tick regardless (Crystal never falls through to chase
/// while shielded).
fn horned_warrior_move_away(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if tick < agent.next_move_tick {
        return true;
    }

    let Some(away_direction) = direction_toward(player_position, position) else {
        return true;
    };

    // Crystal `switch (Envir.Random.Next(2))`: case 0 rotates with `NextDir`
    // (clockwise, +1), default rotates with `PreviousDir` (counter-clockwise, -1).
    let prefer_next = deterministic_chance_roll(
        tick,
        entity_object_id(world, entity).unwrap_or_default(),
        HORNED_WARRIOR_AI as u64,
        2,
    );
    // Crystal first tries `Walk(dir)` (offset 0), then 7 further rotations.
    let offsets: [i32; 8] = if prefer_next {
        [0, 1, 2, 3, 4, 5, 6, 7]
    } else {
        [0, -1, -2, -3, -4, -5, -6, -7]
    };

    let step = offsets.into_iter().find_map(|offset| {
        let direction = rotated_direction(away_direction, offset);
        let next = offset_point(position, direction, 1);
        can_occupy(world, next.clone(), Some(entity)).then_some((next, direction))
    });

    if let Some((next, direction)) = step {
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

/// Crystal `WideLineAttack(damage, 4, 500, ACAgility, false, 3)`
/// (`MonsterObject.cs:3645-3713`). width 3 → three parallel lanes (centre,
/// `Functions.Left`, `Functions.Right`), each extending `1..=4` tiles along
/// `direction`. Each caught target takes `damage` after
/// `MaxDistance(self, target) * 50 + 500` ms. The player and opposing monsters
/// in the lane tiles are scheduled; `push = false` so nothing is shoved.
#[allow(clippy::too_many_arguments)]
fn horned_warrior_wide_line_attack(
    world: &mut World,
    entity: Entity,
    agent: &MonsterAgent,
    attacker_id: u32,
    monster_name: &str,
    position: &Point,
    player_position: &Point,
    direction: MirDirection,
    damage: i32,
    tick: u64,
) {
    let lane_points = horned_warrior_wide_line_points(position, direction);

    // Player: only if hostile (Crystal `IsAttackTarget`) and standing in a lane.
    if agent.hostile_to_player && lane_points.contains(player_position) {
        let delay_ms = wide_line_target_delay_ms(position, player_position);
        schedule_damage_to_player(
            world,
            tick + combat_delay_ticks(delay_ms),
            attacker_id,
            monster_name.to_string(),
            damage,
        );
    }

    // Opposing monsters caught in the lanes (Crystal hits any attack target).
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
        HORNED_WARRIOR_WIDE_LINE_LENGTH,
    ) {
        let Some(target_position) = entity_position(world, target) else {
            continue;
        };
        if !lane_points.contains(&target_position) {
            continue;
        }
        let delay_ms = wide_line_target_delay_ms(position, &target_position);
        schedule_damage_to_monster(
            world,
            tick + combat_delay_ticks(delay_ms),
            attacker_id,
            target,
            damage,
            None,
            None,
        );
    }
}

/// Crystal per-target delay: `MaxDistance(CurrentLocation, ob) * 50 + 500`
/// (`MonsterObject.cs:3703`).
fn wide_line_target_delay_ms(source: &Point, target: &Point) -> u64 {
    let distance = u64::try_from(tile_distance(source, target).max(0)).unwrap_or(0);
    distance * 50 + HORNED_WARRIOR_WIDE_LINE_BASE_DELAY_MS
}

/// The tile set for Crystal's `WideLineAttack` start-point fan-out at width 3.
/// `Functions.Left(loc, dir)` == `rotated_direction(dir, -2)` one tile; `Right`
/// == `rotated_direction(dir, +2)` (verified against `Functions.cs:248-310`).
/// Centre / left / right lanes each run `1..=length` along `direction`.
pub(in crate::runtime) fn horned_warrior_wide_line_points(
    position: &Point,
    direction: MirDirection,
) -> Vec<Point> {
    let lane_origins = [
        position.clone(),
        offset_point(position, rotated_direction(direction, -2), 1),
        offset_point(position, rotated_direction(direction, 2), 1),
    ];

    let mut points = Vec::new();
    for origin in lane_origins {
        for amount in 1..=HORNED_WARRIOR_WIDE_LINE_LENGTH {
            points.push(offset_point(&origin, direction, amount));
        }
    }
    points
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

    fn warrior_agent(position: Point) -> MonsterAgent {
        MonsterAgent {
            image: 343,
            dead: false,
            patrol_origin: position,
            ai: HORNED_WARRIOR_AI,
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
        }
    }

    /// Crystal `InAttackRange` (HornedWarrior.cs:16-27): range cap 4, then
    /// `(x<=1 && y<=1) || (x==y || x%2==y%2)`.
    #[test]
    fn in_attack_range_matches_crystal_pattern() {
        let origin = Point { x: 10, y: 10 };
        // Same tile -> false.
        assert!(!horned_warrior_in_attack_range(&origin, &origin));
        // Adjacent (1,0) and diagonal (1,1) -> true.
        assert!(horned_warrior_in_attack_range(
            &origin,
            &Point { x: 11, y: 10 }
        ));
        assert!(horned_warrior_in_attack_range(
            &origin,
            &Point { x: 11, y: 11 }
        ));
        // (4,4) diagonal within cap -> true.
        assert!(horned_warrior_in_attack_range(
            &origin,
            &Point { x: 14, y: 14 }
        ));
        // (5,0) beyond range cap -> false.
        assert!(!horned_warrior_in_attack_range(
            &origin,
            &Point { x: 15, y: 10 }
        ));
        // (3,1): x%2 == y%2 (both odd) -> true (Crystal's parity clause).
        assert!(horned_warrior_in_attack_range(
            &origin,
            &Point { x: 13, y: 11 }
        ));
        // (2,1): x even, y odd, x!=y -> false.
        assert!(!horned_warrior_in_attack_range(
            &origin,
            &Point { x: 12, y: 11 }
        ));
    }

    /// Crystal `WideLineAttack` width-3 fan-out facing `Right`
    /// (MonsterObject.cs:3645-3713): centre row at y, Left lane (y-1) and Right
    /// lane (y+1), each running x+1..=x+4.
    #[test]
    fn wide_line_points_form_three_lanes_of_length_four() {
        let origin = Point { x: 0, y: 0 };
        let points = horned_warrior_wide_line_points(&origin, MirDirection::Right);
        assert_eq!(points.len(), 12);
        // Centre lane.
        for x in 1..=4 {
            assert!(
                points.contains(&Point { x, y: 0 }),
                "missing centre ({x},0)"
            );
        }
        // Left(Right) == rotated -2 == Up == y-1.
        for x in 1..=4 {
            assert!(
                points.contains(&Point { x, y: -1 }),
                "missing left ({x},-1)"
            );
        }
        // Right(Right) == rotated +2 == Down == y+1.
        for x in 1..=4 {
            assert!(points.contains(&Point { x, y: 1 }), "missing right ({x},1)");
        }
    }

    /// Key behaviour: below 50% HP and off shield cooldown, the HornedWarrior
    /// raises its shield — sets the shield window into the future, flags
    /// `mode`, broadcasts an `ObjectAttack { attack_type: 2 }`, deals no damage,
    /// and reports that it handled the tick. Mirrors Crystal `Attack`
    /// (HornedWarrior.cs:51-66). Resource-free (the shield branch returns before
    /// any damage scheduling), so it runs against a bare `World`.
    #[test]
    fn low_health_raises_shield_and_broadcasts_type_two() {
        let mut world = World::new();
        let object_id = 99_165_u32;
        let warrior_position = Point { x: 20, y: 20 };
        let agent = warrior_agent(warrior_position.clone());
        let entity = world
            .spawn((
                WorldObject,
                Monster,
                ObjectId(object_id),
                DisplayName::literal("HornedWarrior".to_string()),
                Position(warrior_position.clone()),
                Facing(MirDirection::Left),
                agent.clone(),
                MonsterAiState::default(),
                // 40% HP -> below the 50% shield threshold.
                MonsterVitals {
                    hp: 40,
                    max_hp: 100,
                },
            ))
            .id();

        let mut agent = agent;
        let mut ai_state = MonsterAiState::default();
        // Adjacent player so we enter Attack() rather than the chase fall-through.
        let player_position = Point { x: 21, y: 20 };
        let tick = 1_000_u64;
        let mut packets = Vec::new();

        let handled = update_horned_warrior_state(
            &mut world,
            entity,
            &mut agent,
            &mut ai_state,
            &warrior_position,
            &player_position,
            tick,
            &mut packets,
        );

        assert!(handled, "shield grant should own the tick");
        assert!(ai_state.mode, "shield should be flagged active via mode");
        assert!(
            ai_state.next_state_tick > tick,
            "shield window must extend into the future"
        );
        // Crystal: 15000 + Random(0,5000) ms -> 50..=67 ticks at 300ms/tick.
        let shield_ticks = ai_state.next_state_tick - tick;
        assert!(
            (50..=67).contains(&shield_ticks),
            "shield window {shield_ticks} ticks should be in 50..=67"
        );
        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectAttack { info }
                    if info.object_id == object_id && info.attack_type == 2
            )),
            "shield grant should broadcast ObjectAttack type 2"
        );
    }

    /// While the shield window is active the HornedWarrior must NOT attack: it
    /// owns the tick (to kite away) and emits no `ObjectAttack`. Crystal
    /// `ProcessTarget` buff branch (HornedWarrior.cs:90-133). We put movement on
    /// cooldown (`next_move_tick` in the future) so the kite step short-circuits
    /// before touching map resources — the no-attack contract is what we assert.
    #[test]
    fn shielded_warrior_does_not_attack() {
        let mut world = World::new();
        let object_id = 99_166_u32;
        let warrior_position = Point { x: 30, y: 30 };
        let mut agent = warrior_agent(warrior_position.clone());
        // Movement on cooldown -> kite branch returns before `can_occupy`.
        agent.next_move_tick = 5_000;
        let entity = world
            .spawn((
                WorldObject,
                Monster,
                ObjectId(object_id),
                DisplayName::literal("HornedWarrior".to_string()),
                Position(warrior_position.clone()),
                Facing(MirDirection::Left),
                agent.clone(),
                MonsterAiState::default(),
                MonsterVitals {
                    hp: 80,
                    max_hp: 100,
                },
            ))
            .id();

        let tick = 2_000_u64;
        // Shield active: next_state_tick in the future.
        let mut ai_state = MonsterAiState {
            next_state_tick: tick + 10,
            ..MonsterAiState::default()
        };
        let player_position = Point { x: 31, y: 30 };
        let mut packets = Vec::new();

        let handled = update_horned_warrior_state(
            &mut world,
            entity,
            &mut agent,
            &mut ai_state,
            &warrior_position,
            &player_position,
            tick,
            &mut packets,
        );

        assert!(
            handled,
            "shielded warrior should own the tick (kite, not chase)"
        );
        assert!(
            !packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })),
            "a shielded HornedWarrior must not attack"
        );
    }
}

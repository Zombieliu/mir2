// New per-monster module following the post-refactor pattern of one
// `<name>.rs` submodule per monster behaviour.
//
// Monster AI: `HornedArcher` (Crystal AI id 164).
//
// Crystal `Server/MirObjects/Monsters/HornedArcher.cs`. `HornedArcher`
// extends `AxeSkeleton` (AI 8): its `Attack()` (HornedArcher.cs:48-72) is the
// same ranged-DC strike AxeSkeleton uses (`S.ObjectRangeAttack` Type 0 +
// `GetAttackPower(MinDC, MaxDC)` on a `MaxDistance*50+500` ms delay), with
// AttackRange 6 (AxeSkeleton.cs:10-16). That ranged-attack half is delivered
// by the shared default monster path once AI 164 is wired into the
// `monster_uses_ranged_attack` / `monster_attack_range` /
// `monster_player_attack_damage` tables (see the coordinator table rows
// returned for this port); this module only models HornedArcher's bespoke
// `ProcessTarget` ally-buff override (HornedArcher.cs:15-46).
//
// `ProcessTarget` (HornedArcher.cs:15-46): once `Envir.Time > BuffTime`, the
// archer scans `FindAllFriends(Info.ViewRange, CurrentLocation)`; if any
// friend exists it picks a random one, faces it, broadcasts
// `S.ObjectRangeAttack { ..., TargetID = friend, Type = 1 }`, schedules a
// `DelayedType.Damage` action `MaxDistance*50+500` ms out that — in
// `CompleteAttack` (HornedArcher.cs:74-111) — applies a 10s combat buff
// (`Settings.Second * 10`) to every friend within 4 of the chosen ally
// (Effect 0 => `HornedArcherBuff` +Min/MaxDC +Min/MaxMC; Effect 1 =>
// `ColdArcherBuff` +Min/MaxAC +Min/MaxMAC), then sets `BuffTime = +20000`,
// `ActionTime = +300`, `AttackTime = +AttackSpeed`, `ShockTime = 0` and
// returns. If no friend is found it sets `BuffTime = +10000` and falls
// through to `base.ProcessTarget()` (the AxeSkeleton chase / ranged attack).

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::components::{
    Facing, Monster, MonsterAgent, MonsterAiState, ObjectId, Position, entity_facing,
    entity_object_id, entity_position,
};
use super::super::monsters::*;
use super::super::movement::*;

/// Crystal `HornedArcher.BuffTime` base re-arm after an *empty* friend scan
/// (HornedArcher.cs:42 — `BuffTime = Envir.Time + 10000`). 10000 ms →
/// `combat_delay_ticks(10_000)` = `10_000.div_ceil(1_000)` = 10 ticks.
const HORNED_ARCHER_BUFF_COOLDOWN_TICKS: u64 = 10;
/// Crystal `HornedArcher.BuffTime` re-arm after a *successful* buff cast
/// (HornedArcher.cs:35 — `BuffTime = Envir.Time + 20000`). 20000 ms → 20 ticks.
const HORNED_ARCHER_BUFF_SUCCESS_COOLDOWN_TICKS: u64 = 20;
/// Crystal `HornedArcher.ProcessTarget` `ActionTime = Envir.Time + 300`
/// (HornedArcher.cs:36) — the post-cast action pause, re-used as the attack
/// gate so the archer does not immediately also fire its ranged DC strike.
/// 300 ms → `combat_delay_ticks(300)` = `300.div_ceil(1_000)` = 1 tick.
const HORNED_ARCHER_BUFF_ACTION_PAUSE_TICKS: u64 = 1;

/// HornedArcher (AI 164) bespoke `ProcessTarget` ally-buff (HornedArcher.cs:15-46).
///
/// Returns `true` when a friendly monster was found and the buff cast was
/// emitted (Crystal `return`s here, skipping the base ranged attack this
/// tick). Returns `false` otherwise so the shared default path runs the
/// inherited AxeSkeleton chase + ranged-DC attack.
pub(in crate::runtime) fn update_horned_archer_state(
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

    // Crystal `ProcessTarget` only runs with a live target. Mirror the shared
    // aggro gate the other modules use: a tracked player, or a hostile archer
    // with the player inside its view range.
    if !agent.tracking_player
        && !(agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1))
    {
        return false;
    }

    // Crystal `if (Envir.Time > BuffTime)` (HornedArcher.cs:17). We park the
    // per-monster `BuffTime` in `ai_state.next_state_tick`; while it is still in
    // the future the archer just falls through to its base AxeSkeleton behaviour.
    if tick < ai_state.next_state_tick {
        return false;
    }

    let Some(friend) = horned_archer_pick_friend(world, entity, agent, position, tick) else {
        // No friends in range → Crystal sets `BuffTime = Envir.Time + 10000`
        // and falls through to `base.ProcessTarget()` (HornedArcher.cs:42-45).
        ai_state.next_state_tick = tick + HORNED_ARCHER_BUFF_COOLDOWN_TICKS;
        return false;
    };

    let Some(friend_position) = entity_position(world, friend) else {
        ai_state.next_state_tick = tick + HORNED_ARCHER_BUFF_COOLDOWN_TICKS;
        return false;
    };
    let Some(friend_object_id) = entity_object_id(world, friend) else {
        ai_state.next_state_tick = tick + HORNED_ARCHER_BUFF_COOLDOWN_TICKS;
        return false;
    };

    // `Direction = Functions.DirectionFromPoint(CurrentLocation, friend...)`
    // (HornedArcher.cs:27) — face from us toward the friend; if they share our
    // tile keep the current facing.
    let direction = direction_toward(position, &friend_position)
        .or_else(|| entity_facing(world, entity))
        .unwrap_or(MirDirection::Up);

    // Crystal cooldowns / pauses after a successful cast (HornedArcher.cs:35-38).
    ai_state.next_state_tick = tick + HORNED_ARCHER_BUFF_SUCCESS_COOLDOWN_TICKS;
    agent.next_attack_tick = agent
        .next_attack_tick
        .max(tick + HORNED_ARCHER_BUFF_ACTION_PAUSE_TICKS);

    world
        .entity_mut(entity)
        .insert((Facing(direction), agent.clone()));

    // `Broadcast(new S.ObjectRangeAttack { ..., TargetID = friend, Type = 1 })`
    // (HornedArcher.cs:29). This buff-cast animation packet exists in the
    // protocol, so it is emitted faithfully even though the stat side-effect
    // below is deferred.
    if let Some(packet) = monster_typed_ranged_attack_packet(
        world,
        entity,
        position,
        direction,
        friend,
        &friend_position,
        1,
    ) {
        packets.push(packet);
    }

    // DEFERRED — the monster-buff stat application (HornedArcher.cs:31-33,
    // 82-106). Crystal queues a `DelayedType.Damage` action `MaxDistance*50+500`
    // ms out whose `CompleteAttack` calls
    // `friends[i].AddBuff(BuffType.HornedArcherBuff | ColdArcherBuff, ...)` on
    // every friend within 4 of `friend`, granting +DC/+MC (Effect 0) or
    // +AC/+MAC (Effect 1) for 10s. This sim has no monster-side buff system:
    // buffs live in the player-only `BuffResource` (runtime/buffs.rs) keyed by
    // string, and `MonsterCombatStats` carries only `agility` — there is no path
    // to add a temporary DC/MC/AC/MAC modifier to a `MonsterAgent`. Faking it
    // (e.g. mutating base monster stats) would never expire and would corrupt
    // the shared template-derived damage tables, so per the port's safe-path
    // rule the stat side-effect is skipped; the control flow, target selection,
    // cooldowns and the cast packet are all faithful. `friend_object_id` is
    // bound to document the buff target a monster-buff system would consume.
    let _ = friend_object_id;

    true
}

/// Crystal `FindAllFriends(Info.ViewRange, CurrentLocation)` followed by
/// `friends[Envir.Random.Next(friends.Count)]` (HornedArcher.cs:19-23).
///
/// "Friends" are same-side monsters (Crystal `IsFriendlyTarget`): for a
/// hostile-to-player archer, other hostile-to-player monsters; for a tamed
/// archer, other player-friendly monsters. Self / dead / hidden / sleeping
/// monsters are excluded. Selection is the deterministic-RNG analogue of
/// `Envir.Random.Next(friends.Count)` (salt = AI id 164) over a stable
/// (object-id-sorted) candidate list, so replays are reproducible.
fn horned_archer_pick_friend(
    world: &World,
    entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
    tick: u64,
) -> Option<Entity> {
    let view_range = agent.view_range.max(1);
    let archer_is_friendly = !agent.hostile_to_player;

    #[allow(deprecated)]
    let mut friends: Vec<(u32, Entity)> = world
        .iter_entities()
        .filter_map(|candidate| {
            let candidate_entity = candidate.id();
            if candidate_entity == entity || !candidate.contains::<Monster>() {
                return None;
            }
            let target_agent = candidate.get::<MonsterAgent>()?;
            let target_ai_state = candidate
                .get::<MonsterAiState>()
                .copied()
                .unwrap_or_default();
            if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
                return None;
            }
            // Same side only (Crystal `IsFriendlyTarget`).
            if (!target_agent.hostile_to_player) != archer_is_friendly {
                return None;
            }
            let target_position = candidate.get::<Position>().map(|value| value.0.clone())?;
            if tile_distance(position, &target_position) > view_range {
                return None;
            }
            let object_id = candidate
                .get::<ObjectId>()
                .map(|value| value.0)
                .unwrap_or_default();
            Some((object_id, candidate_entity))
        })
        .collect();

    if friends.is_empty() {
        return None;
    }
    // Stable order so the deterministic index below is reproducible.
    friends.sort_by_key(|(object_id, _)| *object_id);

    let archer_object_id = entity_object_id(world, entity).unwrap_or_default();
    let index =
        deterministic_roll(tick, archer_object_id as usize, 164, friends.len() as u64) as usize;
    friends.get(index).map(|(_, friend)| *friend)
}

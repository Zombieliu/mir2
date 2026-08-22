//! Monster AI: HornedCommander (Crystal AI id 171).
//!
//! Faithful port of `Crystal/Server/MirObjects/Monsters/HornedCommander.cs`
//! (the boss base chain lives in `MonsterObject.cs`). HornedCommander is a
//! bespoke multi-phase boss with summons, an immunity/shield window, an
//! AoE rock-fall, a charge-up spin, a hammer smash, a teleport, and a
//! ground-spell rock-spike grid.
//!
//! State is packed into the shared `MonsterAiState` (no new component is
//! introduced — coordinator owns `components.rs`):
//!   * `mode`          -> Crystal `_Immune` (also the shield-active flag).
//!   * `extra`         -> Crystal `_CalledBoulders` / `_StartedBoulderWalk`
//!                        (the <80% boulder phase has fired once).
//!   * `extra_byte`    -> bit0 `_CalledRockSpikes`, bit1 `_CalledShield`,
//!                        bit2 `_StartAdvanced`.
//!   * `next_state_tick` -> Crystal `_RockSpikeTime` during the rock-spike
//!                        phase, reused as the shield-expiry tick while
//!                        immune (the two phases never overlap in HP band).
//!
//! Determinism: every Crystal `Envir.Random(n)` / `RandomChance` becomes a
//! `deterministic_roll` / `deterministic_chance_roll` seeded by tick +
//! object id + a per-call salt (the AI id 171 plus an offset). No `rand`.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::CrystalRespawnTemplate;
use mir2_protocol::{MirDirection, ObjectAttackInfo, ObjectSpellInfo, Point, ServerPacket, Spell};

use super::super::combat::*;
use super::super::components::{
    Facing, Monster, MonsterAgent, MonsterAiState, MonsterVitals, SummonedMonster, entity_facing,
    entity_name, entity_object_id, player_entity,
};
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;

/// Crystal AI id of HornedCommander (used by the module's tests to spawn/assert).
#[cfg(test)]
const HORNED_COMMANDER_AI: u8 = 171;
/// Deterministic-roll salt base for HornedCommander (AI 171) so its rolls never
/// collide with another monster sharing tick + object id.
const SALT_BASE: u64 = 171_000;

/// Crystal `_MapCentre` (HornedCommander.cs:25-34). Hard-coded to the intended
/// spawn map `HY01_morae_chon` centre `(26, 32)`.
const MAP_CENTRE: Point = Point { x: 26, y: 32 };

// HP gate percents (HornedCommander.cs:37/41/48).
const BOULDER_HEALTH_PERCENT: i32 = 80;
const ROCK_HEALTH_PERCENT: i32 = 50;
const SHIELD_HEALTH_PERCENT: i32 = 10;
/// `_ShieldSeconds` (HornedCommander.cs:50) — immunity/shield window length.
const SHIELD_SECONDS_MS: u64 = 20_000;

// extra_byte phase bits.
const FLAG_CALLED_ROCK_SPIKES: u8 = 0b001;
const FLAG_CALLED_SHIELD: u8 = 0b010;
const FLAG_STARTED_ADVANCED: u8 = 0b100;

/// Crystal `Settings.HornedCommanderBombMob` (Settings.cs:208) — the 8-ring
/// boulder-phase slave.
const BOMB_MOB: &str = "BoulderSpirit";
/// Crystal `Settings.HornedCommanderMob` (Settings.cs:207) — the shield-phase
/// single slave.
const HORNED_MOB: &str = "HornedSorceror";

pub(in crate::runtime) fn update_horned_commander_state(
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

    let Some(object_id) = entity_object_id(world, entity) else {
        return false;
    };
    let monster_name = entity_name(world, entity).unwrap_or_else(|| "HornedCommander".to_string());
    let Some(vitals) = world.entity(entity).get::<MonsterVitals>().copied() else {
        return false;
    };
    let health_percent = vitals.hp.max(0) * 100 / vitals.max_hp.max(1);

    // ----- ProcessAI ordering (HornedCommander.cs:67-124) -----

    // Phase 0: boulder summon, fires once when HP first dips below 80%.
    // (HornedCommander.cs:69-73)
    if (health_percent < BOULDER_HEALTH_PERCENT || ai_state.extra) && !ai_state.extra {
        ai_state.extra = true; // _StartedBoulderWalk + _CalledBoulders
        spawn_boulder(
            world,
            entity,
            object_id,
            &monster_name,
            position,
            player_position,
            tick,
            packets,
        );
    }

    // `if (_Immune) return;` (HornedCommander.cs:75) — but expire the shield
    // first so we don't stay immune forever (Crystal does this via the buff
    // system / ProcessBuffEnd, which the sim lacks; emulate with the timer).
    if ai_state.mode {
        if tick >= ai_state.next_state_tick {
            // ProcessBuffEnd (HornedCommander.cs:139-147): drop immunity and
            // gate the next attack/action behind the Crystal cooldowns.
            ai_state.mode = false;
            agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
            ai_state.next_state_tick = 0;
        } else {
            // Still immune: untargetable + takes no damage. We hold the tick so
            // default melee/chase never runs while shielded.
            persist_facing(world, entity, position, player_position);
            return true;
        }
    }

    // Phase 2: shield + immunity, fires once under 10%.
    // (HornedCommander.cs:77-90)
    if health_percent < SHIELD_HEALTH_PERCENT && ai_state.extra_byte & FLAG_CALLED_SHIELD == 0 {
        // KillRockSpikes(): stop the grid (HornedCommander.cs:79 / 572-582).
        ai_state.extra_byte |= FLAG_CALLED_SHIELD;
        ai_state.mode = true; // _Immune
        ai_state.next_state_tick = tick + combat_delay_ticks(SHIELD_SECONDS_MS);

        let direction = facing_direction(world, entity, position, player_position);
        world.entity_mut(entity).insert(Facing(direction));
        // Broadcast ObjectAttack Type 4 Level = _ShieldSeconds.
        packets.push(typed_attack_packet_with_level(
            object_id, position, direction, 4, 20,
        ));

        // SpawnSlave() — single HornedCommanderMob (HornedCommander.cs:88/555).
        spawn_slave(world, entity, object_id, position, direction, tick);
        return true;
    }

    // Phase 1: rock-spike grid, HP in [10, 50). (HornedCommander.cs:92-112)
    if health_percent < ROCK_HEALTH_PERCENT && health_percent >= SHIELD_HEALTH_PERCENT {
        if ai_state.extra_byte & FLAG_CALLED_ROCK_SPIKES == 0 {
            // SetupRockSpike() is deterministic geometry around the map centre;
            // we compute the grid lazily in spawn_rock_spikes instead of caching.
            ai_state.extra_byte |= FLAG_CALLED_ROCK_SPIKES;
            ai_state.next_state_tick = tick; // _RockSpikeTime = now -> spawn ASAP

            let direction = facing_direction(world, entity, position, player_position);
            // Broadcast ObjectRangeAttack Type 2 (HornedCommander.cs:99).
            if let Some(player) = player_entity(world) {
                if let Some(packet) = monster_typed_ranged_attack_packet(
                    world,
                    entity,
                    position,
                    direction,
                    player,
                    player_position,
                    2,
                ) {
                    packets.push(packet);
                }
            }
        }
    }

    // `if (_CalledRockSpikes && Envir.Time > _RockSpikeTime)` then spawn the
    // grid. (HornedCommander.cs:103-112). Crystal drips ONE grid node per 5s and
    // dedups against live `_RockSpikeEffects` CastLocations; the sim exposes no
    // queryable spell-field registry to the AI, so instead of repeating the same
    // node we lay the whole 7x7 grid in one pass (the faithful END STATE — a
    // rock-spike carpet over the arena) and then stop the timer. See module
    // notes for this deliberate deviation.
    if ai_state.extra_byte & FLAG_CALLED_ROCK_SPIKES != 0
        && ai_state.extra_byte & FLAG_CALLED_SHIELD == 0
        && ai_state.next_state_tick != u64::MAX
        && tick > ai_state.next_state_tick
    {
        spawn_rock_spike_grid(world, entity, object_id, &monster_name, tick);
        ai_state.next_state_tick = u64::MAX; // grid laid; halt further spawns
    }

    // _StartAdvanced toggling + Reset on full heal. (HornedCommander.cs:114-121)
    if health_percent < 100 && ai_state.extra_byte & FLAG_STARTED_ADVANCED == 0 {
        ai_state.extra_byte |= FLAG_STARTED_ADVANCED;
    } else if health_percent == 100 && ai_state.extra_byte & FLAG_STARTED_ADVANCED != 0 {
        // Reset() (HornedCommander.cs:126-137): clear every phase flag. We can't
        // despawn already-spawned slaves without their handles, so KillSlaves is
        // deferred (see module notes); the phase machine itself resets.
        ai_state.extra = false;
        ai_state.mode = false;
        ai_state.extra_byte = 0;
        ai_state.next_state_tick = 0;
    }

    // ----- ProcessTarget / Attack (HornedCommander.cs:149-279) -----
    // Crystal then runs `base.ProcessAI()` -> ProcessTarget -> Attack. We only
    // model the bespoke Attack() branches; if none apply we fall through to the
    // shared default melee/chase by returning false.
    let start_advanced = ai_state.extra_byte & FLAG_STARTED_ADVANCED != 0;
    if !start_advanced {
        return false;
    }
    if !monster_can_attack(agent, ai_state) {
        return false;
    }
    if tick < agent.next_attack_tick {
        return false;
    }
    if !monster_in_attack_range(agent, position, player_position) {
        return false;
    }
    if !agent.tracking_player
        && !(agent.hostile_to_player
            && tile_distance(position, player_position) <= agent.view_range.max(1))
    {
        return false;
    }
    let Some(player) = player_entity(world) else {
        return false;
    };
    let Some(direction) = direction_toward(position, player_position) else {
        return false;
    };

    // Crystal sets Direction toward the target before every branch.
    if entity_facing(world, entity) != Some(direction) {
        world.entity_mut(entity).insert(Facing(direction));
    }
    agent.tracking_player = true;
    let dc = crystal_monster_raw_attack_damage(&monster_name);

    // Charge-Up AOE Rock Fall: Random(20)==0. (HornedCommander.cs:188-209)
    if deterministic_chance_roll(tick, object_id, SALT_BASE + 1, 20) {
        let rock_fall_loops = 5 + deterministic_roll(tick, object_id as usize, 11, 5); // Random.Next(5,10)
        let rock_fall_duration = rock_fall_loops * 500;

        ai_state.mode = true; // _Immune for the wind-up
        ai_state.next_state_tick = tick + combat_delay_ticks(rock_fall_duration + 500);
        agent.next_attack_tick = tick
            + combat_delay_ticks(rock_fall_duration + 500)
            + agent.attack_interval_ticks.max(1);

        // Broadcast ObjectAttack Type 3 Level = rockFallLoops.
        packets.push(typed_attack_packet_with_level(
            object_id,
            position,
            direction,
            3,
            rock_fall_loops as u8,
        ));

        let front = offset_point(position, direction, 2);
        mass_spawn_rock_fall(world, entity, object_id, &monster_name, &front, tick);

        // DelayedAction RangeDamage DC*loops, AC, radius 5 at +duration+500.
        let damage = dc * rock_fall_loops as i32;
        if damage > 0 {
            schedule_area_damage(
                world,
                entity,
                object_id,
                &monster_name,
                player,
                position,
                tick + combat_delay_ticks(rock_fall_duration + 500),
                damage,
                5,
            );
        }
        return true;
    }

    // Charge-Up Spin Hit: Random(15)==0, two AoE hits. (HornedCommander.cs:212-232)
    if deterministic_chance_roll(tick, object_id, SALT_BASE + 2, 15) {
        let spin_loops = 5 + deterministic_roll(tick, object_id as usize, 12, 5);
        let spin_duration = spin_loops * 700;

        ai_state.mode = true; // _Immune
        ai_state.next_state_tick = tick + combat_delay_ticks(spin_duration + 1500);
        agent.next_attack_tick =
            tick + combat_delay_ticks(spin_duration + 1500) + agent.attack_interval_ticks.max(1);

        // Broadcast ObjectAttack Type 2 Level = spinLoops.
        packets.push(typed_attack_packet_with_level(
            object_id,
            position,
            direction,
            2,
            spin_loops as u8,
        ));

        let damage = dc * spin_loops as i32;
        if damage > 0 {
            // Two DelayedAction Damage aoe=true (radius 3 around self).
            for delay_ms in [spin_duration + 500, spin_duration + 1000] {
                schedule_area_damage(
                    world,
                    entity,
                    object_id,
                    &monster_name,
                    player,
                    position,
                    tick + combat_delay_ticks(delay_ms),
                    damage,
                    3,
                );
            }
        }
        return true;
    }

    // Hammer Smash: Random(10)==0. (HornedCommander.cs:235-245)
    if deterministic_chance_roll(tick, object_id, SALT_BASE + 3, 10) {
        agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
        if dc == 0 {
            return true;
        }
        // Broadcast ObjectRangeAttack Type 1.
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
        // DelayedAction RangeDamage DC, AC, radius 3 at +300, front+2.
        let front = offset_point(position, direction, 2);
        schedule_area_damage(
            world,
            entity,
            object_id,
            &monster_name,
            player,
            &front,
            tick + combat_delay_ticks(300),
            dc,
            3,
        );
        return true;
    }

    // Teleport: Random(10)==0. (HornedCommander.cs:248-256)
    if deterministic_chance_roll(tick, object_id, SALT_BASE + 4, 10) {
        agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
        // Broadcast ObjectRangeAttack Type 0.
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
        // CompleteRangeAttack with empty data -> TeleportRandom(10,10) + clear
        // target. Mirror with a deterministic short-range reposition.
        teleport_random(world, entity, agent, object_id, position, tick, packets);
        agent.tracking_player = false;
        return true;
    }

    // Normal Attacks: Random(2)==0 -> Type 0, else Type 1; both DC ACAgility,
    // single target, at +500. (HornedCommander.cs:259-278)
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
    if dc == 0 {
        return true;
    }
    let attack_type = if deterministic_chance_roll(tick, object_id, SALT_BASE + 5, 2) {
        0
    } else {
        1
    };
    packets.push(typed_attack_packet_with_level(
        object_id,
        position,
        direction,
        attack_type,
        0,
    ));
    let mitigation = crystal_player_rolled_armour(world);
    let damage = (dc - mitigation).max(1);
    if agent.hostile_to_player {
        schedule_damage_to_player(
            world,
            tick + combat_delay_ticks(500),
            object_id,
            monster_name.clone(),
            damage,
        );
    }
    true
}

/// Crystal `Attack` always faces the target before acting; while idle/immune we
/// still keep facing the player so the broadcast direction matches Crystal.
fn persist_facing(world: &mut World, entity: Entity, position: &Point, player_position: &Point) {
    let direction = facing_direction(world, entity, position, player_position);
    if entity_facing(world, entity) != Some(direction) {
        world.entity_mut(entity).insert(Facing(direction));
    }
}

fn facing_direction(
    world: &World,
    entity: Entity,
    position: &Point,
    player_position: &Point,
) -> MirDirection {
    direction_toward(position, player_position)
        .or_else(|| entity_facing(world, entity))
        .unwrap_or(MirDirection::Up)
}

/// ObjectAttack with an explicit `level` (the shared helper hard-codes level 0,
/// but Crystal encodes shield-seconds / spin-loops / rock-fall-loops in Level).
fn typed_attack_packet_with_level(
    object_id: u32,
    location: &Point,
    direction: MirDirection,
    attack_type: u8,
    level: u8,
) -> ServerPacket {
    ServerPacket::ObjectAttack {
        info: ObjectAttackInfo {
            object_id,
            location: location.clone(),
            direction,
            spell: 0,
            level,
            attack_type,
        },
    }
}

/// SpawnBoulder (HornedCommander.cs:525-553): teleport toward the map centre,
/// then spawn 8 BombMob slaves in an 8-direction ring (alt radii 9/7).
#[allow(clippy::too_many_arguments)]
fn spawn_boulder(
    world: &mut World,
    entity: Entity,
    object_id: u32,
    monster_name: &str,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let _ = monster_name;
    // Crystal: `if InRange(CurrentLocation, _MapCentre, 20) Teleport(_MapCentre)`.
    // The sim has no boss-arena teleport primitive that relocates a live monster
    // mid-tick cleanly, so we keep HornedCommander in place (see module notes)
    // and spawn the ring around its current location, which still matches the
    // 8-direction boulder pattern relative to the boss.
    let target_entity = player_entity(world);
    for i in 0u8..8 {
        let direction = mir_direction_from_index(i);
        let odd = i % 2 != 0;
        let radius = if odd { 7 } else { 9 };
        let point = offset_point(position, direction, radius);
        // Crystal faces the slave back toward the commander.
        let slave_facing = direction_toward(&point, position).unwrap_or(direction);
        queue_horned_commander_summon(
            world,
            entity,
            object_id,
            BOMB_MOB,
            point,
            slave_facing,
            target_entity,
            tick,
        );
    }
    let _ = player_position;
    let _ = packets;
}

/// SpawnSlave (HornedCommander.cs:555-570): a single HornedCommanderMob in front
/// (fallback to the boss tile).
fn spawn_slave(
    world: &mut World,
    entity: Entity,
    object_id: u32,
    position: &Point,
    direction: MirDirection,
    tick: u64,
) {
    let target_entity = player_entity(world);
    let front = offset_point(position, direction, 1);
    let spawn_point = if can_occupy(world, front.clone(), Some(entity)) {
        front
    } else {
        position.clone()
    };
    queue_horned_commander_summon(
        world,
        entity,
        object_id,
        HORNED_MOB,
        spawn_point,
        direction,
        target_entity,
        tick,
    );
}

/// Shared summon path mirroring `hell_lord::queue_hell_lord_summon`: looks up the
/// Crystal template and queues a pending spawn at +500ms, marked as a slave of
/// this boss and hostile to the player.
#[allow(clippy::too_many_arguments)]
fn queue_horned_commander_summon(
    world: &mut World,
    summoner_entity: Entity,
    object_id: u32,
    monster_name: &str,
    position: Point,
    _direction: MirDirection,
    target_entity: Option<Entity>,
    tick: u64,
) {
    let Some(template) = crystal_dynamic_monster_template(monster_name) else {
        return;
    };
    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: tick + combat_delay_ticks(500),
            summoner_entity,
            template: CrystalRespawnTemplate {
                location: position,
                ..template
            },
            target_entity,
            summon_metadata: Some(SummonedMonster {
                summoner_object_id: object_id,
                visible_extra: false,
                expire_tick: None,
                require_summoner_within: None,
                despawn_tick_after_death: None,
                totem_master_object_id: None,
                max_minions: None,
            }),
            hostile_to_player_override: Some(true),
        },
    );
}

/// MassSpawnRockFall (HornedCommander.cs:356-369): scatter rock-fall ground
/// fields on a coarse grid around `front`, each ticking MC damage. We collapse
/// Crystal's per-cell SpawnRockFall flood into one visible `ObjectSpell` per grid
/// node plus a scheduled MC hit, which is the sim's ground-field idiom
/// (cf. `tucson_general::schedule_tucson_general_rocks`).
fn mass_spawn_rock_fall(
    world: &mut World,
    entity: Entity,
    object_id: u32,
    monster_name: &str,
    front: &Point,
    tick: u64,
) {
    let range = 15;
    let mc = crystal_monster_raw_magic_damage(monster_name);
    let mut node_index = 0u64;
    let mut x = -range;
    while x < range {
        let mut y = -range;
        while y < range {
            let node = Point {
                x: front.x + x,
                y: front.y + y,
            };
            // Crystal `SpawnRockFall(location, Random(0,200), duration)`: a
            // per-node start offset within [0,200)ms, then start = 500 + offset.
            let offset =
                deterministic_roll(tick, object_id as usize, (40 + node_index) as usize, 200);
            let start_ms = 500 + offset;
            let spawn_tick = tick + combat_delay_ticks(start_ms);

            let spell_packet = ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id: allocate_runtime_monster_object_id(world),
                    location: clamp_to_map_region(world, node.clone()),
                    spell: Spell::HornedCommanderRockFall,
                    direction: MirDirection::Up,
                    param: false,
                },
            };
            queue_due_packet(world, spawn_tick, spell_packet);

            if mc > 0 {
                // Impact one tick after the field appears (TickSpeed analogue).
                let impact_tick = spawn_tick + combat_delay_ticks(1_000);
                schedule_point_damage(
                    world,
                    entity,
                    object_id,
                    monster_name,
                    &clamp_to_map_region(world, node),
                    impact_tick,
                    mc,
                );
            }
            node_index += 1;
            y += 10;
        }
        x += 10;
    }
}

/// SpawnRockSpikes / SpawnRockSpike (HornedCommander.cs:441-523): the 7x7 grid
/// (5-tile spacing) centred on the map centre. Crystal spawns one node per 5s and
/// dedups against live SpellObjects; lacking a queryable field registry we lay the
/// whole grid in one pass (49 nodes), each a visible `HornedCommanderRockSpike`
/// ground field plus a scheduled MC hit. This matches Crystal's eventual carpet.
fn spawn_rock_spike_grid(
    world: &mut World,
    entity: Entity,
    object_id: u32,
    monster_name: &str,
    tick: u64,
) {
    let mc = crystal_monster_raw_magic_damage(monster_name);
    for gx in 0..7i32 {
        for gy in 0..7i32 {
            let node = clamp_to_map_region(
                world,
                Point {
                    x: MAP_CENTRE.x + (gx - 3) * 5,
                    y: MAP_CENTRE.y + (gy - 3) * 5,
                },
            );
            let spawn_tick = tick + combat_delay_ticks(500);
            let spell_packet = ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id: allocate_runtime_monster_object_id(world),
                    location: node.clone(),
                    spell: Spell::HornedCommanderRockSpike,
                    direction: MirDirection::Up,
                    param: false,
                },
            };
            queue_due_packet(world, spawn_tick, spell_packet);

            if mc > 0 {
                let impact_tick = spawn_tick + combat_delay_ticks(1_000);
                schedule_point_damage(
                    world,
                    entity,
                    object_id,
                    monster_name,
                    &node,
                    impact_tick,
                    mc,
                );
            }
        }
    }
}

/// Schedule a single-point hit: damages the player if standing on `point`, plus
/// any opposing monsters on the tile (radius 0).
#[allow(clippy::too_many_arguments)]
fn schedule_point_damage(
    world: &mut World,
    entity: Entity,
    object_id: u32,
    monster_name: &str,
    point: &Point,
    due_tick: u64,
    damage: i32,
) {
    schedule_area_damage_internal(
        world,
        entity,
        object_id,
        monster_name,
        None,
        point,
        due_tick,
        damage,
        0,
    );
}

/// Schedule AoE damage centred on `centre` with Chebyshev `radius`, hitting the
/// player (if it's the bound target and in range) and all opposing monsters.
#[allow(clippy::too_many_arguments)]
fn schedule_area_damage(
    world: &mut World,
    entity: Entity,
    object_id: u32,
    monster_name: &str,
    player: Entity,
    centre: &Point,
    due_tick: u64,
    damage: i32,
    radius: i32,
) {
    schedule_area_damage_internal(
        world,
        entity,
        object_id,
        monster_name,
        Some(player),
        centre,
        due_tick,
        damage,
        radius,
    );
}

#[allow(clippy::too_many_arguments)]
fn schedule_area_damage_internal(
    world: &mut World,
    entity: Entity,
    object_id: u32,
    monster_name: &str,
    player: Option<Entity>,
    centre: &Point,
    due_tick: u64,
    damage: i32,
    radius: i32,
) {
    if damage <= 0 {
        return;
    }
    let agent_hostile = world
        .entity(entity)
        .get::<MonsterAgent>()
        .map(|agent| agent.hostile_to_player)
        .unwrap_or(true);
    if agent_hostile {
        if let Some(player) = player.or_else(|| player_entity(world)) {
            if let Some(player_position) = super::super::components::entity_position(world, player)
            {
                if tile_distance(centre, &player_position) <= radius {
                    // Crystal applies the target's AC inside `Attacked`; the sim's
                    // scheduled-player path takes pre-mitigated damage, so subtract
                    // the player's rolled AC here (cf. general_meow_meow / tucson).
                    let mitigation = crystal_player_rolled_armour(world);
                    let player_damage = (damage - mitigation).max(1);
                    schedule_damage_to_player(
                        world,
                        due_tick,
                        object_id,
                        monster_name.to_string(),
                        player_damage,
                    );
                }
            }
        }
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    let Some(agent) = world.entity(entity).get::<MonsterAgent>().cloned() else {
        return;
    };
    for target in
        nearby_opposing_monster_targets(world, &monster_entities, entity, centre, &agent, radius)
    {
        schedule_damage_to_monster(world, due_tick, object_id, target, damage, None, None);
    }
}

/// TeleportRandom(10, 10) (HornedCommander.cs:338-354): up to 10 attempts to land
/// within +/-10 tiles. Deterministic reposition to the first occupiable jitter.
fn teleport_random(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    object_id: u32,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    for attempt in 0..10u64 {
        let dx = deterministic_roll(
            tick + attempt,
            object_id as usize,
            60 + attempt as usize,
            21,
        ) as i32
            - 10;
        let dy = deterministic_roll(
            tick + attempt,
            object_id as usize,
            80 + attempt as usize,
            21,
        ) as i32
            - 10;
        let candidate = clamp_to_map_region(
            world,
            Point {
                x: position.x + dx,
                y: position.y + dy,
            },
        );
        if candidate != *position && can_occupy(world, candidate.clone(), Some(entity)) {
            let direction = direction_toward(position, &candidate)
                .or_else(|| entity_facing(world, entity))
                .unwrap_or(MirDirection::Up);
            world.entity_mut(entity).insert((
                super::super::components::Position(candidate.clone()),
                Facing(direction),
            ));
            agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
            // Mirror the yimoogi teleport idiom: relocate + a single ObjectWalk
            // so clients re-anchor the boss (the sim has no boss teleport-VFX
            // primitive exposed to monster AI).
            packets.push(ServerPacket::ObjectWalk {
                movement: mir2_protocol::ObjectMovement {
                    object_id,
                    position: candidate,
                    direction,
                },
            });
            return;
        }
    }
}

/// Map an 8-direction index to a MirDirection (Crystal `(MirDirection)i`).
fn mir_direction_from_index(index: u8) -> MirDirection {
    match index % 8 {
        0 => MirDirection::Up,
        1 => MirDirection::UpRight,
        2 => MirDirection::Right,
        3 => MirDirection::DownRight,
        4 => MirDirection::Down,
        5 => MirDirection::DownLeft,
        6 => MirDirection::Left,
        _ => MirDirection::UpLeft,
    }
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

    fn boss_agent(_object_id: u32, position: Point) -> (MonsterAgent, MonsterAiState) {
        (
            MonsterAgent {
                image: 348,
                dead: true,
                patrol_origin: position,
                ai: HORNED_COMMANDER_AI,
                disposition: WorldEntityDisposition::Hostile,
                hostile_to_player: true,
                tracking_player: true,
                view_range: 12,
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
        )
    }

    /// 8-direction ring mapping matches Crystal `(MirDirection)i`.
    #[test]
    fn direction_ring_matches_crystal_indices() {
        assert_eq!(mir_direction_from_index(0), MirDirection::Up);
        assert_eq!(mir_direction_from_index(1), MirDirection::UpRight);
        assert_eq!(mir_direction_from_index(2), MirDirection::Right);
        assert_eq!(mir_direction_from_index(3), MirDirection::DownRight);
        assert_eq!(mir_direction_from_index(4), MirDirection::Down);
        assert_eq!(mir_direction_from_index(5), MirDirection::DownLeft);
        assert_eq!(mir_direction_from_index(6), MirDirection::Left);
        assert_eq!(mir_direction_from_index(7), MirDirection::UpLeft);
    }

    /// A dead boss must defer entirely to the default pass (returns false) and
    /// never touch world resources — this exercises the real dispatch entry
    /// point with the components a spawned HornedCommander carries.
    #[test]
    fn dead_boss_falls_through_to_default() {
        let mut world = World::new();
        let object_id = 99_171_u32;
        let boss_position = Point { x: 26, y: 32 };
        let (agent, ai_state) = boss_agent(object_id, boss_position.clone());
        let entity = world
            .spawn((
                WorldObject,
                Monster,
                ObjectId(object_id),
                DisplayName::literal("HornedCommander".to_string()),
                Position(boss_position.clone()),
                Facing(MirDirection::Left),
                agent.clone(),
                ai_state,
                MonsterVitals {
                    hp: 10,
                    max_hp: 1000,
                },
            ))
            .id();

        let mut agent = agent;
        let mut ai_state = ai_state;
        let player_position = Point { x: 27, y: 32 };
        let mut packets = Vec::new();
        let handled = update_horned_commander_state(
            &mut world,
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
}

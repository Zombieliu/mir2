// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{MonsterAgent, MonsterAiState};
use super::super::crystal_compat::*;
use super::super::movement::*;

pub(in crate::runtime) fn update_hell_bomb_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;
    agent.next_attack_tick = tick + 1;

    if agent.dead {
        return true;
    }

    if ai_state.next_state_tick == 0 {
        ai_state.next_state_tick = tick + HELL_BOMB_EXPLOSION_LIFETIME_TICKS;
        return true;
    }

    if tick >= ai_state.next_state_tick {
        explode_hell_bomb(world, entity, agent, position, tick, packets);
    }

    true
}

pub(in crate::runtime) fn hell_bomb_name_for_tick(tick: u64) -> &'static str {
    match tick % 3 {
        0 => "HellBomb1",
        1 => "HellBomb2",
        _ => "HellBomb3",
    }
}

pub(in crate::runtime) fn hell_bomb_spawn_position(
    world: &World,
    player_position: &Point,
    tick: u64,
    summoner_entity: Entity,
) -> Point {
    let directions = [
        MirDirection::Up,
        MirDirection::UpRight,
        MirDirection::Right,
        MirDirection::DownRight,
        MirDirection::Down,
        MirDirection::DownLeft,
        MirDirection::Left,
        MirDirection::UpLeft,
    ];
    let direction = directions[(tick as usize) % directions.len()];
    let distance = 5 + i32::try_from(tick % 4).expect("tick modulo fits i32");
    summon_spawn_position_near(
        world,
        player_position,
        direction,
        distance,
        Some(summoner_entity),
    )
}

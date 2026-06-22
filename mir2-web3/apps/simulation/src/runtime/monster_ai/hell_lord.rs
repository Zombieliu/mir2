// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::CrystalRespawnTemplate;
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{
    entity_object_id, player_entity, Facing, MonsterAgent, MonsterAiState, SummonedMonster,
};
use super::super::monsters::*;
use super::super::movement::*;

use super::common::*;
use super::hell_bomb::{hell_bomb_name_for_tick, hell_bomb_spawn_position};

pub(in crate::runtime) fn update_hell_lord_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;

    if agent.dead {
        return true;
    }

    let Some(lord_object_id) = entity_object_id(world, entity) else {
        return true;
    };

    if tick < agent.next_attack_tick {
        return true;
    }
    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);

    let target_entity = player_entity(world);
    let mut spawned_knight = false;
    let rage_delay_elapsed = ai_state.next_state_tick == 0 || tick >= ai_state.next_state_tick;
    if ai_state.extra_byte < 4
        && active_summoned_monster_count_by_ai(world, lord_object_id, 97) == 0
        && (!ai_state.mode || rage_delay_elapsed)
    {
        if let Some(knight_name) = hell_lord_knight_name(ai_state.extra_byte) {
            let spawn_position = hell_lord_knight_spawn_position(world, position, entity);
            if queue_hell_lord_summon(
                world,
                entity,
                knight_name,
                spawn_position,
                tick,
                target_entity,
                Some(SummonedMonster {
                    summoner_object_id: lord_object_id,
                    visible_extra: false,
                    expire_tick: None,
                    require_summoner_within: None,
                    despawn_tick_after_death: None,
                    totem_master_object_id: None,
                    max_minions: None,
                }),
                Some(true),
            ) {
                ai_state.mode = true;
                spawned_knight = true;
            }
        }
    }

    if spawned_knight {
        world.entity_mut(entity).insert(Facing(MirDirection::Up));
        if let Some(packet) = monster_melee_attack_packet(world, entity, position, MirDirection::Up)
        {
            packets.push(packet);
        }
    }

    let bomb_count = active_monster_count_by_ai_near(world, 99, position, 25);
    let should_spawn_bomb = bomb_count < 5
        && (spawned_knight
            || deterministic_roll(
                tick,
                entity.index() as usize,
                usize::from(ai_state.extra_byte),
                3,
            ) == 0);
    if should_spawn_bomb {
        let bomb_position = hell_bomb_spawn_position(world, player_position, tick, entity);
        let _ = queue_hell_lord_summon(
            world,
            entity,
            hell_bomb_name_for_tick(tick),
            bomb_position,
            tick,
            target_entity,
            None,
            Some(true),
        );
    }

    true
}

pub(in crate::runtime) fn hell_lord_knight_name(stage: u8) -> Option<&'static str> {
    match stage {
        0 => Some("HellKnight1"),
        1 => Some("HellKnight2"),
        2 => Some("HellKnight3"),
        3 => Some("HellKnight4"),
        _ => None,
    }
}

pub(in crate::runtime) fn hell_lord_knight_spawn_position(
    world: &World,
    lord_position: &Point,
    entity: Entity,
) -> Point {
    let front = offset_point(lord_position, MirDirection::DownLeft, 12);
    first_occupiable_point_near(world, &front, 10, Some(entity)).unwrap_or_else(|| {
        summon_spawn_position_near(
            world,
            lord_position,
            MirDirection::DownLeft,
            1,
            Some(entity),
        )
    })
}

pub(in crate::runtime) fn queue_hell_lord_summon(
    world: &mut World,
    summoner_entity: Entity,
    monster_name: &str,
    position: Point,
    tick: u64,
    target_entity: Option<Entity>,
    summon_metadata: Option<SummonedMonster>,
    hostile_to_player_override: Option<bool>,
) -> bool {
    let Some(template) = crystal_dynamic_monster_template(monster_name) else {
        return false;
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
            summon_metadata,
            hostile_to_player_override,
        },
    );
    true
}

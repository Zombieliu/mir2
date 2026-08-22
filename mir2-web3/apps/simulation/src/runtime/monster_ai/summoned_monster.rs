// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{MonsterAgent, SummonedMonster, entity_object_id};
use super::super::monsters::*;
use super::super::movement::*;

pub(in crate::runtime) fn update_summoned_monster_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let Some(summoned) = world.entity(entity).get::<SummonedMonster>().copied() else {
        return false;
    };

    let mut should_die = false;
    let mut despawn_after_death = summoned.despawn_tick_after_death;

    if let Some(expire_tick) = summoned.expire_tick {
        if tick >= expire_tick {
            should_die = true;
        }
    }

    let mut summoner_dead = false;

    match summoned_owner_state(world, summoned.summoner_object_id) {
        Some((summoner_position, is_dead)) => {
            summoner_dead = is_dead;
            if let Some(range) = summoned.require_summoner_within {
                if tile_distance(position, &summoner_position) > range {
                    should_die = true;
                }
            }
        }
        None => {
            should_die = true;
        }
    }

    if agent.dead {
        if let Some(despawn_tick) = summoned.despawn_tick_after_death {
            if tick >= despawn_tick {
                let _ = world.despawn(entity);
            }
            return true;
        }
        return false;
    }

    if summoner_dead {
        should_die = true;
    }

    if should_die {
        if agent.ai == 62 {
            despawn_after_death = Some(tick + combat_delay_ticks(3_000));
            kill_summoned_minions(
                world,
                entity_object_id(world, entity).unwrap_or(summoned.summoner_object_id),
                tick,
                packets,
            );
        }
        if agent.ai == 99 {
            explode_hell_bomb(world, entity, agent, position, tick, packets);
            return true;
        }

        if let Some(mut summoned_mut) = world.entity_mut(entity).get_mut::<SummonedMonster>() {
            summoned_mut.despawn_tick_after_death = despawn_after_death;
        }

        let _ = damage_monster_entity(world, entity, i32::MAX, tick, packets);
        return true;
    }

    false
}

pub(in crate::runtime) fn kill_summoned_minions(
    world: &mut World,
    summoner_object_id: u32,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    #[allow(deprecated)]
    let summoned_entities: Vec<Entity> = world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<SummonedMonster>()
                .filter(|summoned| summoned.summoner_object_id == summoner_object_id)
                .and_then(|_| entity.get::<MonsterAgent>().filter(|agent| !agent.dead))
                .map(|_| entity.id())
        })
        .collect();

    for summoned_entity in summoned_entities {
        let _ = damage_monster_entity(world, summoned_entity, i32::MAX, tick, packets);
    }
}

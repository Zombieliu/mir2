// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::combat::*;
use super::super::components::{MonsterAgent, SummonedMonster};

pub(in crate::runtime) fn update_vampire_spider_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if !agent.dead {
        return false;
    }

    if let Some(despawn_tick) = world
        .entity(entity)
        .get::<SummonedMonster>()
        .and_then(|summoned| summoned.despawn_tick_after_death)
    {
        if tick >= despawn_tick {
            let _ = world.despawn(entity);
        }
        return true;
    }

    explode_vampire_spider(world, entity, position, tick, packets);
    true
}

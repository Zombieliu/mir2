// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{Point, ServerPacket};

use super::super::components::MonsterAgent;

pub(in crate::runtime) fn update_spitting_toad_state(
    _world: &mut World,
    _entity: Entity,
    agent: &mut MonsterAgent,
    _position: &Point,
    _tick: u64,
    _packets: &mut Vec<ServerPacket>,
) -> bool {
    agent.tracking_player = false;
    false
}

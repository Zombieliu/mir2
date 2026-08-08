// NOTE: split out of the former monolithic `monster_ai.rs` by a mechanical,
// behavior-preserving refactor. Logic is unchanged.

use super::super::components::MonsterAgent;

pub(in crate::runtime) fn update_yin_devil_node_state(agent: &mut MonsterAgent, tick: u64) -> bool {
    if agent.dead {
        return true;
    }

    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;
    agent.next_attack_tick = tick + 1;
    true
}

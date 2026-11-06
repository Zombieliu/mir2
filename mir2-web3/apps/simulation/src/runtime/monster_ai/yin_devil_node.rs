// Crystal `YinDevilNode` (AI 41/42) is an immobile support node. Its attack
// animation is a delayed ally-buff cast, never an attack against the player.

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_protocol::{MirDirection, Point, ServerPacket, UserItemStat};

use super::super::buffs::{apply_or_refresh_buff, buff_metadata, client_buff_packet_for_state};
use super::super::combat::combat_delay_ticks;
use super::super::components::{
    entity_facing, entity_position, player_entity, CharacterBody, Monster, MonsterAgent,
    MonsterAiState, Position,
};
use super::super::monsters::{
    is_hidden_or_sleeping_target, monster_can_attack, monster_typed_attack_packet,
};
use super::super::movement::tile_distance;

const YIN_DEVIL_NODE_FRIEND_RANGE: i32 = 7;
const YIN_DEVIL_NODE_BUFF_DURATION_TICKS: u64 = 5;

/// Crystal `YinDevilNode.ProcessTarget` / `CompleteAttack` (AI 41/42).
///
/// The personal simulation has a real player BuffResource, but it has no
/// target-owned, expiring monster buff component. Therefore this keeps the
/// ObjectAttack cast and its 500ms completion timing, applies the real player
/// buff only when the node is explicitly friendly to the player, and
/// fail-closes for monster targets instead of forging an AddBuff packet that
/// would not survive a monster snapshot or affect combat.
pub(in crate::runtime) fn update_yin_devil_node_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if agent.dead {
        return true;
    }

    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick.saturating_add(1);

    if ai_state.extra {
        if tick < ai_state.next_state_tick {
            return true;
        }

        ai_state.extra = false;
        ai_state.next_state_tick = 0;
        yin_devil_node_complete_attack(world, agent, position, tick, packets);
        return true;
    }

    if !monster_can_attack(agent, ai_state) || tick < agent.next_attack_tick {
        return true;
    }

    if yin_devil_node_friendly_targets(world, entity, agent, position).is_empty() {
        return true;
    }

    let direction = entity_facing(world, entity).unwrap_or(MirDirection::Up);
    let Some(packet) = monster_typed_attack_packet(world, entity, position, direction, 0) else {
        return true;
    };

    packets.push(packet);
    agent.next_attack_tick = tick.saturating_add(agent.attack_interval_ticks.max(1));
    agent.next_move_tick = tick.saturating_add(1);
    ai_state.extra = true;
    ai_state.next_state_tick = tick.saturating_add(combat_delay_ticks(500));
    true
}

fn yin_devil_node_complete_attack(
    world: &mut World,
    agent: &MonsterAgent,
    position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    if agent.hostile_to_player {
        return;
    }

    let Some(player) = player_entity(world) else {
        return;
    };
    let Some(player_position) = entity_position(world, player) else {
        return;
    };
    if tile_distance(position, &player_position) > YIN_DEVIL_NODE_FRIEND_RANGE {
        return;
    }

    let Some(level) = world
        .entity(player)
        .get::<CharacterBody>()
        .map(|body| i32::from(body.level))
    else {
        return;
    };
    let (key, stat) = yin_devil_node_buff(agent.ai);
    let value = level.saturating_div(7).saturating_add(4);
    let (name, description) = match key {
        "blessed-armour" => buff_metadata(key, "Blessed Armour", "Increases physical defence."),
        _ => buff_metadata(key, "Ultimate Enhancer", "Increases physical attack."),
    };
    let buff = super::super::buffs::BuffState {
        key: key.to_string(),
        name,
        description,
        expires_at_tick: tick.saturating_add(YIN_DEVIL_NODE_BUFF_DURATION_TICKS),
        attack_bonus: 0,
        defence_bonus: 0,
        stats: vec![UserItemStat { stat, value }],
    };
    apply_or_refresh_buff(world, buff.clone());
    if let Some(packet) = client_buff_packet_for_state(world, &buff) {
        packets.push(packet);
    }
}

fn yin_devil_node_buff(ai: u8) -> (&'static str, u8) {
    match ai {
        41 => (
            "blessed-armour",
            super::super::crystal_compat::CRYSTAL_STAT_MAX_AC,
        ),
        42 => (
            "ultimate-enhancer",
            super::super::crystal_compat::CRYSTAL_STAT_MAX_DC,
        ),
        _ => (
            "blessed-armour",
            super::super::crystal_compat::CRYSTAL_STAT_MAX_AC,
        ),
    }
}

fn yin_devil_node_friendly_targets(
    world: &World,
    entity: Entity,
    agent: &MonsterAgent,
    position: &Point,
) -> Vec<Entity> {
    let node_is_friendly_to_player = !agent.hostile_to_player;
    let mut targets = Vec::new();

    #[allow(deprecated)]
    for candidate in world.iter_entities() {
        let candidate_entity = candidate.id();
        if candidate_entity == entity || !candidate.contains::<Monster>() {
            continue;
        }
        let Some(target_agent) = candidate.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = candidate
            .get::<MonsterAiState>()
            .copied()
            .unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if (!target_agent.hostile_to_player) != node_is_friendly_to_player {
            continue;
        }
        let Some(target_position) = candidate.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if tile_distance(position, &target_position) <= YIN_DEVIL_NODE_FRIEND_RANGE {
            targets.push(candidate_entity);
        }
    }

    if node_is_friendly_to_player {
        if let Some(player) = player_entity(world) {
            if entity_position(world, player).is_some_and(|target_position| {
                tile_distance(position, &target_position) <= YIN_DEVIL_NODE_FRIEND_RANGE
            }) {
                targets.push(player);
            }
        }
    }

    targets
}

#[cfg(test)]
mod tests {
    use super::yin_devil_node_buff;

    #[test]
    fn crystal_ai41_and_ai42_select_their_source_buff_stat() {
        assert_eq!(yin_devil_node_buff(41), ("blessed-armour", 1));
        assert_eq!(yin_devil_node_buff(42), ("ultimate-enhancer", 5));
    }
}

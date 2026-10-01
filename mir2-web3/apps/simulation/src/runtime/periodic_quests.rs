use bevy_ecs::prelude::World;
use mir2_game_data::periodic_quests::{self as data, PeriodicCadence};
use mir2_protocol::{ClientPacket, ClientQuestInfo, ServerPacket};
use serde::{Deserialize, Serialize};

use super::super::resources::{QuestResource, SessionResource};
use super::quest_recurrence::{self, QuestCadence};
use crate::config::{CharacterSaveRecord, QuestStage};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::runtime) struct AcceptedPeriodicReward {
    pub level: u16,
    pub experience: u32,
    pub gold: u32,
}

pub(super) fn cadence(id: i32) -> Option<QuestCadence> {
    data::quest(id).map(|quest| match quest.cadence {
        PeriodicCadence::Daily => QuestCadence::Daily,
        PeriodicCadence::Weekly => QuestCadence::Weekly,
    })
}

fn reward_at_acceptance(world: &World, id: i32) -> Option<AcceptedPeriodicReward> {
    let quest = data::quest(id)?;
    let level = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()?
        .level;
    let threshold = super::super::leveling::crystal_max_experience_for_level(level).max(0) as u64;
    let base = u32::try_from(threshold.saturating_mul(u64::from(quest.reward_exp_percent)) / 100)
        .unwrap_or(u32::MAX);
    Some(AcceptedPeriodicReward {
        level,
        experience: super::super::stats::crystal_apply_social_exp_rate(world, base),
        gold: quest.gold,
    })
}

pub(super) fn info(world: &World, id: i32) -> Option<ClientQuestInfo> {
    let definition = data::quest(id)?;
    let reward = world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|state| state.quest_id == id)
        .and_then(|state| state.accepted_periodic_reward.clone())
        .or_else(|| reward_at_acceptance(world, id))?;
    Some(definition.info(reward.experience, reward.gold))
}

pub(super) fn infos(world: &World) -> Vec<ClientQuestInfo> {
    data::catalog()
        .quests
        .iter()
        .filter_map(|quest| info(world, quest.id))
        .collect()
}

pub(in crate::runtime) fn ids_for_npc(id: u32) -> Vec<i32> {
    if data::npc(id).is_none() {
        return Vec::new();
    }
    data::catalog()
        .quests
        .iter()
        .map(|quest| quest.id)
        .collect()
}

fn slot_occupied(world: &World, id: i32, for_finish: bool) -> bool {
    slot_occupied_at(world, id, for_finish, quest_recurrence::server_now_millis())
}

pub(super) fn slot_occupied_at(world: &World, id: i32, for_finish: bool, now_ms: u64) -> bool {
    let Some(definition) = data::quest(id) else {
        return false;
    };
    let cadence = cadence(id).expect("configured cadence");
    let period = quest_recurrence::effective_period(world, cadence, now_ms);
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .any(|state| {
            let Some(other) = data::quest(state.quest_id) else {
                return false;
            };
            other.cadence == definition.cadence
                && other.slot == definition.slot
                && ((state.quest_id != id
                    && matches!(
                        state.stage,
                        QuestStage::InProgress | QuestStage::ReadyToTurnIn
                    ))
                    || (state
                        .cadence_last_claimed_period
                        .is_some_and(|claimed| claimed >= period)
                        && (!for_finish
                            || state.quest_id != id
                            || state.stage == QuestStage::Completed)))
        })
}

pub(super) fn can_accept(world: &World, id: i32) -> bool {
    !slot_occupied(world, id, false)
}

pub(super) fn can_finish(world: &World, id: i32) -> bool {
    !data::is_periodic(id)
        || (!slot_occupied(world, id, true)
            && world
                .resource::<QuestResource>()
                .quests
                .iter()
                .find(|state| state.quest_id == id)
                .and_then(|state| state.accepted_periodic_reward.as_ref())
                .is_some_and(|reward| {
                    reward.experience > 0 && reward.gold > 0 && (10..=50).contains(&reward.level)
                }))
}

pub(super) fn lock_reward(world: &mut World, id: i32) -> bool {
    let Some(reward) = reward_at_acceptance(world, id) else {
        return false;
    };
    let mut quests = world.resource_mut::<QuestResource>();
    let Some(state) = quests.quests.iter_mut().find(|state| state.quest_id == id) else {
        return false;
    };
    state.accepted_periodic_reward = Some(reward);
    true
}

pub(super) fn objective_map_matches(id: i32, monster_index: i32, map: &str) -> bool {
    data::quest(id).is_none_or(|quest| {
        quest.kills.iter().any(|kill| {
            kill.monster_index == monster_index
                && kill
                    .maps
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(map.trim_end_matches(".map")))
        })
    })
}

/// Native Crystal hides claimed IDs. Project the same slot's alternate level
/// bands as unavailable, without manufacturing durable completions for them.
pub(super) fn unavailable_ids(world: &World) -> Vec<i32> {
    data::catalog()
        .quests
        .iter()
        .filter(|quest| slot_occupied(world, quest.id, false))
        .filter(|quest| {
            !matches!(
                super::quest_stage(world, quest.id),
                Some(QuestStage::InProgress | QuestStage::ReadyToTurnIn)
            )
        })
        .map(|quest| quest.id)
        .collect()
}

pub(in crate::runtime) fn packet_requires_checkpoint(packet: &ClientPacket) -> bool {
    match packet {
        ClientPacket::FinishQuest { quest_index, .. } => data::is_periodic(*quest_index),
        ClientPacket::AcceptQuest { quest_index, .. }
        | ClientPacket::AbandonQuest { quest_index } => data::is_periodic(*quest_index),
        ClientPacket::CallNpc { object_id, .. } => data::npc(*object_id).is_some(),
        ClientPacket::NpcConfirmInput { npc_id, .. } => data::npc(*npc_id).is_some(),
        _ => false,
    }
}

pub(crate) fn target_is_periodic(target: &str) -> bool {
    let mut parts = target.trim().trim_start_matches('@').split(':');
    let Some(first) = parts.next() else {
        return false;
    };
    if first.eq_ignore_ascii_case("FinishQuest") || first.eq_ignore_ascii_case("AcceptQuest") {
        return parts
            .next()
            .and_then(|part| part.parse().ok())
            .is_some_and(data::is_periodic);
    }
    first.eq_ignore_ascii_case("quest")
        && parts.next().is_some_and(|part| {
            part.eq_ignore_ascii_case("finish") || part.eq_ignore_ascii_case("accept")
        })
        && parts
            .next()
            .and_then(|part| part.parse().ok())
            .is_some_and(data::is_periodic)
}

/// A full-character CAS must commit before exposing a successful claim. The
/// owner of the outer checkpoint handles nested NPC/FinishQuest calls too.
pub(in crate::runtime) fn successful_claim_since(
    world: &World,
    before: &CharacterSaveRecord,
    packets: &[ServerPacket],
) -> bool {
    let old = before
        .quest_states_json
        .iter()
        .filter_map(|json| serde_json::from_str::<super::QuestState>(json).ok())
        .map(|state| (state.quest_id, state.stage))
        .collect::<std::collections::BTreeMap<_, _>>();
    world.resource::<QuestResource>().quests.iter().filter(|state|data::is_periodic(state.quest_id))
        .any(|state| {
            let previous=old.get(&state.quest_id).copied().unwrap_or(QuestStage::Available);
            (previous==QuestStage::ReadyToTurnIn && state.stage==QuestStage::Completed
                && state.cadence_last_claimed_period.is_some()
                && packets.iter().any(|packet|matches!(packet,ServerPacket::ChangeQuest{quest_id,quest_state:2,taken:false,..} if *quest_id==state.quest_id)))
            || (previous==QuestStage::Available && matches!(state.stage,QuestStage::InProgress|QuestStage::ReadyToTurnIn)
                && state.accepted_periodic_reward.is_some()
                && packets.iter().any(|packet|matches!(packet,ServerPacket::ChangeQuest{quest_id,quest_state:0,taken:true,..} if *quest_id==state.quest_id)))
            || (previous==QuestStage::InProgress && state.stage==QuestStage::Available
                && packets.iter().any(|packet|matches!(packet,ServerPacket::ChangeQuest{quest_id,quest_state:2,taken:false,..} if *quest_id==state.quest_id)))
        })
}

pub(in crate::runtime) fn info_packet(world: &World, id: i32) -> Option<ServerPacket> {
    info(world, id).map(|info| ServerPacket::NewQuestInfo { info })
}

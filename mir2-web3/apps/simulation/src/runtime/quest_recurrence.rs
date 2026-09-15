use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy_ecs::prelude::World;
use mir2_protocol::{ClientQuestInfo, ServerPacket};

use crate::config::QuestStage;

use super::super::resources::{is_in_world, QuestResource, SessionResource};

const MILLIS_PER_DAY: u64 = 86_400_000;
const UTC_PLUS_EIGHT_MILLIS: u64 = 8 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QuestCadence {
    Daily,
    Weekly,
    Repeatable,
}

pub(super) fn server_now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(in crate::runtime) fn server_newcomer_v1_enabled() -> bool {
    std::env::var("MIR2_QUEST_CADENCE")
        .ok()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("newcomer-v1"))
}

pub(super) fn cadence_for_quest(world: &World, quest_id: i32) -> Option<QuestCadence> {
    let newcomer_v1 = world
        .get_resource::<QuestResource>()
        .is_some_and(|quests| quests.newcomer_v1_cadence);
    cadence_for_quest_id(quest_id, newcomer_v1)
}

fn cadence_for_quest_id(quest_id: i32, newcomer_v1: bool) -> Option<QuestCadence> {
    if newcomer_v1 && super::newcomer_progression::is_daily_quest(quest_id) {
        return Some(QuestCadence::Daily);
    }
    if newcomer_v1 && quest_id == 140 {
        return Some(QuestCadence::Weekly);
    }
    static QUEST_TYPES: OnceLock<BTreeMap<i32, u8>> = OnceLock::new();
    let quest_type = QUEST_TYPES
        .get_or_init(|| {
            super::crystal_client_quest_infos()
                .into_iter()
                .map(|info| (info.index, info.quest_type))
                .collect()
        })
        .get(&quest_id)
        .copied()?;
    match quest_type {
        super::CRYSTAL_QUEST_TYPE_DAILY => Some(QuestCadence::Daily),
        super::CRYSTAL_QUEST_TYPE_REPEATABLE => Some(QuestCadence::Repeatable),
        _ => None,
    }
}

pub(super) fn apply_cadence_quest_info(world: &World, info: &mut ClientQuestInfo) {
    if !world
        .get_resource::<QuestResource>()
        .is_some_and(|quests| quests.newcomer_v1_cadence)
    {
        return;
    }
    let Some(cadence) = cadence_for_quest(world, info.index) else {
        return;
    };
    let (label, detail) = match cadence {
        QuestCadence::Daily => ("Daily", "Daily reset: 00:00 server time (UTC+8)."),
        QuestCadence::Weekly => ("Weekly", "Weekly reset: Monday 00:00 server time (UTC+8)."),
        QuestCadence::Repeatable => (
            "Repeatable",
            "Repeatable: available again after successful completion.",
        ),
    };
    info.group = label.to_string();
    if !info.description.iter().any(|line| line == detail) {
        info.description.insert(0, detail.to_string());
    }
}

pub(in crate::runtime) fn quest_completion_is_permanent(world: &World, quest_id: i32) -> bool {
    !matches!(
        cadence_for_quest(world, quest_id),
        Some(QuestCadence::Repeatable)
    )
}

pub(super) fn record_quest_completion(world: &mut World, quest_id: i32) {
    record_quest_completion_at(world, quest_id, server_now_millis());
}

pub(super) fn record_quest_completion_at(world: &mut World, quest_id: i32, now_ms: u64) {
    let Some(cadence) = cadence_for_quest(world, quest_id) else {
        return;
    };
    if cadence == QuestCadence::Repeatable {
        return;
    }
    let newcomer_v1 = world.resource::<QuestResource>().newcomer_v1_cadence;
    let raw_period = cadence_period(cadence, now_ms);
    let effective_period = world
        .resource::<QuestResource>()
        .quests
        .iter()
        .filter(|quest| cadence_for_quest_id(quest.quest_id, newcomer_v1) == Some(cadence))
        .filter_map(|quest| quest.cadence_high_watermark_period)
        .fold(raw_period, u64::max);
    let mut quests = world.resource_mut::<QuestResource>();
    let Some(quest) = quests
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == quest_id)
    else {
        return;
    };
    quest.cadence_high_watermark_period = Some(effective_period);
    quest.cadence_last_claimed_period = Some(effective_period);
}

pub(in crate::runtime) fn refresh_quest_recurrence(world: &mut World) -> Vec<ServerPacket> {
    refresh_quest_recurrence_at(world, server_now_millis())
}

pub(super) fn refresh_quest_recurrence_at(world: &mut World, now_ms: u64) -> Vec<ServerPacket> {
    if !is_in_world(world)
        || world
            .get_resource::<SessionResource>()
            .and_then(|session| session.selected_character.as_ref())
            .is_none()
    {
        return Vec::new();
    }

    let newcomer_v1 = world.resource::<QuestResource>().newcomer_v1_cadence;
    let changed = refresh_quest_states_at(
        &mut world.resource_mut::<QuestResource>().quests,
        newcomer_v1,
        now_ms,
    );
    if !changed {
        return Vec::new();
    }
    let mut packets = vec![ServerPacket::CompleteQuest {
        completed_quests: super::completed_quest_ids(world),
    }];
    if matches!(
        super::quest_stage(world, super::newcomer_progression::DAILY_BONUS_ID),
        Some(QuestStage::InProgress | QuestStage::ReadyToTurnIn)
    ) {
        if let Some(packet) =
            super::crystal_quest_update_packet(world, super::newcomer_progression::DAILY_BONUS_ID)
        {
            packets.push(packet);
        }
    }
    packets
}

fn refresh_quest_states_at(
    quests: &mut [super::QuestState],
    newcomer_v1: bool,
    now_ms: u64,
) -> bool {
    let mut changed = false;
    let raw_daily = cadence_period(QuestCadence::Daily, now_ms);
    let raw_weekly = cadence_period(QuestCadence::Weekly, now_ms);
    let daily_period = quests
        .iter()
        .filter(|quest| {
            cadence_for_quest_id(quest.quest_id, newcomer_v1) == Some(QuestCadence::Daily)
        })
        .filter_map(|quest| quest.cadence_high_watermark_period)
        .fold(raw_daily, u64::max);
    let weekly_period = quests
        .iter()
        .filter(|quest| {
            cadence_for_quest_id(quest.quest_id, newcomer_v1) == Some(QuestCadence::Weekly)
        })
        .filter_map(|quest| quest.cadence_high_watermark_period)
        .fold(raw_weekly, u64::max);
    for quest in quests.iter_mut() {
        let Some(cadence) = cadence_for_quest_id(quest.quest_id, newcomer_v1) else {
            continue;
        };
        if cadence == QuestCadence::Repeatable {
            if quest.stage == QuestStage::Completed {
                reset_completed_quest(quest);
                changed = true;
            }
            continue;
        }

        let effective_period = match cadence {
            QuestCadence::Daily => daily_period,
            QuestCadence::Weekly => weekly_period,
            QuestCadence::Repeatable => 0,
        };
        quest.cadence_high_watermark_period = Some(effective_period);
        if quest.stage != QuestStage::Completed {
            // Active and ready quests cross reset boundaries intact. Their
            // eventual hand-in is charged to the period in which it succeeds.
            continue;
        }
        let Some(claimed_period) = quest.cadence_last_claimed_period else {
            // Old saves did not record cadence. Treat a completed row as a
            // claim in the current effective period rather than reopening it.
            quest.cadence_last_claimed_period = Some(effective_period);
            continue;
        };
        if effective_period > claimed_period {
            reset_completed_quest(quest);
            changed = true;
        }
    }
    changed |= super::newcomer_progression::sync_daily_bonus_states(quests, newcomer_v1);
    changed
}

fn reset_completed_quest(quest: &mut super::QuestState) {
    quest.stage = QuestStage::Available;
    quest.current = 0;
    quest.task_progress.clear();
}

fn cadence_period(cadence: QuestCadence, now_ms: u64) -> u64 {
    let local_day = now_ms.saturating_add(UTC_PLUS_EIGHT_MILLIS) / MILLIS_PER_DAY;
    match cadence {
        QuestCadence::Daily => local_day,
        // 1970-01-01 was Thursday. Adding three makes Monday the first day
        // of each seven-day bucket in server-local (UTC+8) civil time.
        QuestCadence::Weekly => local_day.saturating_add(3) / 7,
        QuestCadence::Repeatable => 0,
    }
}

#[cfg(test)]
pub(super) fn daily_period_at(now_ms: u64) -> u64 {
    cadence_period(QuestCadence::Daily, now_ms)
}

#[cfg(test)]
pub(super) fn weekly_period_at(now_ms: u64) -> u64 {
    cadence_period(QuestCadence::Weekly, now_ms)
}

#[cfg(test)]
pub(super) fn refresh_states_for_test(
    quests: &mut [super::QuestState],
    newcomer_v1: bool,
    now_ms: u64,
) -> bool {
    refresh_quest_states_at(quests, newcomer_v1, now_ms)
}

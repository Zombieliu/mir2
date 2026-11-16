use mir2_protocol::{ClientPacket, ServerPacket};

use super::*;
use crate::{SimulationConfig, SimulationSession};

const DAY_MS: u64 = 86_400_000;
const FIRST_LOCAL_MIDNIGHT_UTC_MS: u64 = 16 * 60 * 60 * 1_000;
const FIRST_LOCAL_MONDAY_UTC_MS: u64 = 4 * DAY_MS - 8 * 60 * 60 * 1_000;

fn state(quest_id: i32, stage: QuestStage) -> QuestState {
    let info = crystal_quest_info_by_id(quest_id).expect("Crystal quest fixture");
    QuestState::from_crystal_info(&info, stage)
}

fn authenticated_session() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    let character_index = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::LoginSuccess { characters } => characters.first().map(|item| item.index),
            _ => None,
        })
        .expect("demo character");
    session.handle_packet(ClientPacket::StartGame { character_index });
    session
}

#[test]
fn utc_plus_eight_daily_and_monday_weekly_boundaries_are_exact() {
    assert_eq!(
        quest_recurrence::daily_period_at(FIRST_LOCAL_MIDNIGHT_UTC_MS - 1) + 1,
        quest_recurrence::daily_period_at(FIRST_LOCAL_MIDNIGHT_UTC_MS)
    );
    assert_eq!(
        quest_recurrence::weekly_period_at(FIRST_LOCAL_MONDAY_UTC_MS - 1) + 1,
        quest_recurrence::weekly_period_at(FIRST_LOCAL_MONDAY_UTC_MS)
    );
}

#[test]
fn daily_claim_reopens_once_after_boundary_and_clock_rollback_does_not_reopen() {
    let claim_time = FIRST_LOCAL_MIDNIGHT_UTC_MS + 10 * DAY_MS;
    let claim_period = quest_recurrence::daily_period_at(claim_time);
    let mut quest = state(141, QuestStage::Completed);
    quest.cadence_last_claimed_period = Some(claim_period);
    quest.cadence_high_watermark_period = Some(claim_period);

    assert!(!quest_recurrence::refresh_states_for_test(
        std::slice::from_mut(&mut quest),
        false,
        claim_time - DAY_MS,
    ));
    assert_eq!(quest.stage, QuestStage::Completed);
    assert!(quest_recurrence::refresh_states_for_test(
        std::slice::from_mut(&mut quest),
        false,
        claim_time + DAY_MS,
    ));
    assert_eq!(quest.stage, QuestStage::Available);
    assert_eq!(quest.current, 0);
    assert!(quest.task_progress.is_empty());
}

#[test]
fn active_and_ready_daily_progress_survive_boundary_without_losing_items() {
    let before = FIRST_LOCAL_MIDNIGHT_UTC_MS - 1;
    let after = FIRST_LOCAL_MIDNIGHT_UTC_MS;
    for stage in [QuestStage::InProgress, QuestStage::ReadyToTurnIn] {
        let mut quest = state(141, stage);
        quest.current = 7;
        quest.task_progress.insert("item:1169".to_string(), 7);
        quest.cadence_high_watermark_period = Some(quest_recurrence::daily_period_at(before));
        assert!(!quest_recurrence::refresh_states_for_test(
            std::slice::from_mut(&mut quest),
            false,
            after,
        ));
        assert_eq!(quest.stage, stage);
        assert_eq!(quest.current, 7);
        assert_eq!(quest.task_progress.get("item:1169"), Some(&7));
    }
}

#[test]
fn legacy_completed_daily_save_is_claimed_in_current_period_then_reopens_next_day() {
    let now = FIRST_LOCAL_MIDNIGHT_UTC_MS + 20 * DAY_MS;
    let mut quest: QuestState = serde_json::from_str(
        r#"{"quest_id":141,"title":"legacy","summary":"","reward_preview":"","required":15,"current":15,"stage":"completed","task_progress":{}}"#,
    )
    .expect("legacy quest state");
    assert!(!quest_recurrence::refresh_states_for_test(
        std::slice::from_mut(&mut quest),
        false,
        now,
    ));
    assert_eq!(quest.stage, QuestStage::Completed);
    assert_eq!(
        quest.cadence_last_claimed_period,
        Some(quest_recurrence::daily_period_at(now))
    );

    let restored: QuestState =
        serde_json::from_str(&serde_json::to_string(&quest).unwrap()).unwrap();
    let mut restored = restored;
    assert!(quest_recurrence::refresh_states_for_test(
        std::slice::from_mut(&mut restored),
        false,
        now + DAY_MS,
    ));
    assert_eq!(restored.stage, QuestStage::Available);
}

#[test]
fn repeatable_is_never_a_completed_quest_and_reopens_on_next_lifecycle_refresh() {
    let mut session = authenticated_session();
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_capacity = 0;
    ensure_runtime_quest(session.app.world_mut(), 142);
    set_quest_stage(session.app.world_mut(), 142, QuestStage::ReadyToTurnIn);
    let before_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let reward_gold = crystal_quest_info_by_id(142).unwrap().reward_gold;

    assert!(complete_quest_with_selection(
        session.app.world_mut(),
        142,
        None,
    ));
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        before_gold.saturating_add(reward_gold)
    );
    assert!(!completed_quest_ids(session.app.world()).contains(&142));
    let after_first_claim = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(!complete_quest_with_selection(
        session.app.world_mut(),
        142,
        None,
    ));
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        after_first_claim,
        "a repeated Finish must not duplicate rewards"
    );

    let packets = quest_recurrence::refresh_quest_recurrence_at(
        session.app.world_mut(),
        FIRST_LOCAL_MIDNIGHT_UTC_MS,
    );
    assert_eq!(
        quest_stage(session.app.world(), 142),
        Some(QuestStage::Available)
    );
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if !completed_quests.contains(&142)
    )));
}

#[test]
fn ordinary_server_accept_and_finish_packets_settle_daily_once() {
    let mut session = authenticated_session();
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 15;
    let accept = super::super::packets::stage5_accept_quest_packet(session.app.world_mut(), 141);
    assert!(accept.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 141,
            taken: true,
            ..
        }
    )));
    set_quest_stage(session.app.world_mut(), 141, QuestStage::ReadyToTurnIn);
    let before_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let finish =
        super::super::packets::stage5_finish_quest_packet(session.app.world_mut(), 141, None);
    assert!(finish.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&141)
    )));
    let after_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert_eq!(
        after_gold,
        before_gold.saturating_add(crystal_quest_info_by_id(141).unwrap().reward_gold)
    );
    let retry =
        super::super::packets::stage5_finish_quest_packet(session.app.world_mut(), 141, None);
    assert!(retry.is_empty());
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        after_gold
    );
}

#[test]
fn weekly_q140_exists_only_in_newcomer_profile_and_keeps_source_content() {
    let now = FIRST_LOCAL_MONDAY_UTC_MS + 7 * DAY_MS;
    let prior_period = quest_recurrence::weekly_period_at(now) - 1;
    let mut default_quest = state(140, QuestStage::Completed);
    default_quest.cadence_last_claimed_period = Some(prior_period);
    default_quest.cadence_high_watermark_period = Some(prior_period);
    assert!(!quest_recurrence::refresh_states_for_test(
        std::slice::from_mut(&mut default_quest),
        false,
        now,
    ));
    assert_eq!(default_quest.stage, QuestStage::Completed);

    let mut newcomer_quest = default_quest.clone();
    assert!(quest_recurrence::refresh_states_for_test(
        std::slice::from_mut(&mut newcomer_quest),
        true,
        now,
    ));
    assert_eq!(newcomer_quest.stage, QuestStage::Available);

    let mut session = authenticated_session();
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = true;
    let info = effective_crystal_quest_info_by_id(session.app.world(), 140).unwrap();
    assert_eq!(info.group, "Weekly");
    assert_eq!((info.min_level_needed, info.max_level_needed), (10, 20));
    assert_eq!(info.quest_needed, 0);
    let template = crystal_quest_template_by_id(140).unwrap();
    assert!(template
        .item_tasks
        .iter()
        .any(|task| { task.item_name == "GoldChestnut" && task.count == 5 }));
}

#[test]
fn newcomer_profile_labels_native_daily_and_repeatable_without_changing_rewards() {
    let mut session = authenticated_session();
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v1_cadence = true;
    for (quest_id, expected_group, expected_gold, expected_exp) in [
        (141, "Daily", 5_000, 6_500),
        (142, "Repeatable", 25_000, 10_000),
    ] {
        let info = effective_crystal_quest_info_by_id(session.app.world(), quest_id).unwrap();
        assert_eq!(info.group, expected_group);
        assert_eq!(info.quest_needed, 0);
        assert_eq!(info.reward_gold, expected_gold);
        assert_eq!(info.reward_exp, expected_exp);
        assert!(!info.description.is_empty());
    }
}

#[test]
fn abandoning_daily_progress_preserves_claim_and_rollback_metadata() {
    let mut session = authenticated_session();
    ensure_runtime_quest(session.app.world_mut(), 141);
    {
        let mut quests = session.app.world_mut().resource_mut::<QuestResource>();
        let quest = quests
            .quests
            .iter_mut()
            .find(|quest| quest.quest_id == 141)
            .unwrap();
        quest.stage = QuestStage::InProgress;
        quest.current = 3;
        quest.task_progress.insert("item:1169".to_string(), 3);
        quest.cadence_last_claimed_period = Some(50);
        quest.cadence_high_watermark_period = Some(55);
    }
    assert!(abandon_quest(session.app.world_mut(), 141));
    let quest = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == 141)
        .unwrap();
    assert_eq!(quest.stage, QuestStage::Available);
    assert_eq!(quest.cadence_last_claimed_period, Some(50));
    assert_eq!(quest.cadence_high_watermark_period, Some(55));
}

use super::*;
use crate::{SimulationConfig, SimulationSession};
use mir2_protocol::{ClientPacket, MirClass};

const DAY: u64 = 86_400_000;

fn session(level: u16, class: MirClass) -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    let index = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::LoginSuccess { characters } => {
                characters.first().map(|character| character.index)
            }
            _ => None,
        })
        .unwrap();
    session.handle_packet(ClientPacket::StartGame {
        character_index: index,
    });
    {
        let mut resource = session.app.world_mut().resource_mut::<SessionResource>();
        let character = resource.selected_character.as_mut().unwrap();
        character.level = level;
        character.class = class;
    }
    session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .max_experience = super::super::leveling::crystal_max_experience_for_level(level);
    session
}

fn current(session: &SimulationSession, id: i32) -> u32 {
    quest_progress(session.app.world(), id).unwrap().0
}

#[test]
fn periodic_level_bands_offer_exactly_three_daily_and_two_weekly_for_all_classes() {
    for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
        for level in [9, 10, 14, 15, 24, 25, 34, 35, 50, 51] {
            let session = session(level, class);
            let available = periodic_quests::infos(session.app.world())
                .into_iter()
                .filter(|info| can_accept_crystal_quest(session.app.world(), info))
                .collect::<Vec<_>>();
            let expected = if (10..=50).contains(&level) { 5 } else { 0 };
            assert_eq!(available.len(), expected, "{class:?} level {level}");
            if expected > 0 {
                assert_eq!(
                    available
                        .iter()
                        .filter(|info| info.group == "Daily Tasks")
                        .count(),
                    3
                );
                assert_eq!(
                    available
                        .iter()
                        .filter(|info| info.group == "Weekly Tasks")
                        .count(),
                    2
                );
            }
            assert!(available
                .iter()
                .all(|info| info.rewards_fixed_item.is_empty()
                    && info.rewards_select_item.is_empty()
                    && info.reward_credit == 0));
        }
    }
}

#[test]
fn legal_kill_updates_daily_and_weekly_but_same_monster_on_wrong_map_does_not() {
    let mut session = session(10, MirClass::Taoist);
    for id in [92001, 92004] {
        assert_eq!(
            begin_quest(session.app.world_mut(), id),
            QuestStage::InProgress
        );
    }
    advance_crystal_quest_kill(session.app.world_mut(), "Scarecrow");
    assert_eq!((current(&session, 92001), current(&session, 92004)), (1, 1));
    session
        .app
        .world_mut()
        .resource_mut::<super::super::resources::MapRuntimeResource>()
        .current_map
        .file_name = "D001".into();
    assert!(advance_crystal_quest_kill(session.app.world_mut(), "Scarecrow").is_empty());
    assert_eq!((current(&session, 92001), current(&session, 92004)), (1, 1));
    session
        .app
        .world_mut()
        .resource_mut::<super::super::resources::MapRuntimeResource>()
        .current_map
        .file_name = "0".into();
    assert!(advance_crystal_quest_kill(session.app.world_mut(), "Scarecrow0").is_empty());
    let target = mir2_game_data::periodic_quests::quest(92001).unwrap().kills[0].count;
    for _ in 1..target {
        advance_crystal_quest_kill(session.app.world_mut(), "Scarecrow");
    }
    assert_eq!(
        quest_stage(session.app.world(), 92001),
        Some(QuestStage::ReadyToTurnIn)
    );
    assert_eq!(
        quest_stage(session.app.world(), 92004),
        Some(QuestStage::InProgress)
    );
    assert_eq!(
        (current(&session, 92001), current(&session, 92004)),
        (target, target)
    );
}

#[test]
fn reaccepted_legacy_completed_periodic_row_uses_the_reduced_target_total() {
    let mut session = session(20, MirClass::Warrior);
    let definition = mir2_game_data::periodic_quests::quest(92006).unwrap();
    let now = quest_recurrence::server_now_millis();
    let previous_period = quest_recurrence::daily_period_at(now - DAY);
    let mut legacy =
        QuestState::from_crystal_info(&definition.info(14_000, 8_000), QuestStage::Completed);
    legacy.required = 70;
    legacy.current = 70;
    legacy.summary =
        "Defeat 70 Omas on the Bichon road, then return to either Task Steward.".into();
    legacy.task_progress.insert("kill:48".into(), 70);
    legacy.cadence_last_claimed_period = Some(previous_period);
    legacy.cadence_high_watermark_period = Some(previous_period);
    legacy.accepted_periodic_reward = Some(periodic_quests::AcceptedPeriodicReward {
        level: 20,
        experience: 14_000,
        gold: 8_000,
    });
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .quests
        .push(legacy);
    reconcile_effective_quest_states(session.app.world_mut());
    quest_recurrence::refresh_quest_recurrence_at(session.app.world_mut(), now);
    assert_eq!(
        quest_progress(session.app.world(), definition.id),
        Some((0, 70))
    );
    let available = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|row| row.quest_id == definition.id)
        .unwrap();
    let snapshot = effective_quest_snapshot(session.app.world(), available, LanguageCode::English);
    assert_eq!(snapshot.summary, definition.summary);
    assert_eq!((snapshot.current, snapshot.required), (0, 7));
    assert_eq!(
        begin_quest(session.app.world_mut(), definition.id),
        QuestStage::InProgress
    );
    assert_eq!(
        quest_progress(session.app.world(), definition.id),
        Some((0, 7))
    );
    for _ in 0..7 {
        advance_crystal_quest_kill(session.app.world_mut(), "Oma");
    }
    assert_eq!(
        quest_stage(session.app.world(), definition.id),
        Some(QuestStage::ReadyToTurnIn)
    );
    let row = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|row| row.quest_id == definition.id)
        .unwrap();
    assert_eq!(row.cadence_last_claimed_period, Some(previous_period));
    assert_eq!(row.summary, definition.summary);
    assert_eq!(
        row.accepted_periodic_reward.as_ref().unwrap().experience,
        480_000
    );
}

#[test]
fn every_periodic_objective_accepts_only_its_configured_maps() {
    for definition in &mir2_game_data::periodic_quests::catalog().quests {
        let mut session = session(definition.min_level as u16, MirClass::Wizard);
        assert_eq!(
            begin_quest(session.app.world_mut(), definition.id),
            QuestStage::InProgress
        );
        for task in &definition.kills {
            session
                .app
                .world_mut()
                .resource_mut::<super::super::resources::MapRuntimeResource>()
                .current_map
                .file_name = "WrongMap".into();
            let before = current(&session, definition.id);
            assert!(advance_crystal_quest_kill(session.app.world_mut(), &task.monster).is_empty());
            assert_eq!(current(&session, definition.id), before);
            for map in &task.maps {
                session
                    .app
                    .world_mut()
                    .resource_mut::<super::super::resources::MapRuntimeResource>()
                    .current_map
                    .file_name = map.clone();
                assert!(
                    !advance_crystal_quest_kill(session.app.world_mut(), &task.monster).is_empty()
                );
            }
        }
    }
}

#[test]
fn active_lower_band_blocks_same_slot_after_level_up_without_losing_progress_or_reward() {
    let mut session = session(14, MirClass::Warrior);
    begin_quest(session.app.world_mut(), 92001);
    advance_crystal_quest_kill(session.app.world_mut(), "Scarecrow");
    let locked = periodic_quests::info(session.app.world(), 92001)
        .unwrap()
        .reward_exp;
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 15;
    assert!(!can_accept_quest(session.app.world(), 92006));
    assert!(can_accept_quest(session.app.world(), 92007));
    assert_eq!(current(&session, 92001), 1);
    assert_eq!(
        periodic_quests::info(session.app.world(), 92001)
            .unwrap()
            .reward_exp,
        locked
    );
    assert!(completed_quest_ids(session.app.world()).contains(&92006));
    assert!(!completed_quest_ids(session.app.world()).contains(&92001));
}

#[test]
fn daily_and_weekly_reset_clear_claimed_rewards_but_preserve_unfinished_tasks() {
    let monday = 4 * DAY - 8 * 60 * 60 * 1000;
    for (id, boundary) in [(92001, monday), (92004, monday)] {
        let definition = mir2_game_data::periodic_quests::quest(id).unwrap();
        let mut completed =
            QuestState::from_crystal_info(&definition.info(600, 4000), QuestStage::Completed);
        let before = boundary - 1;
        let period = if id == 92001 {
            quest_recurrence::daily_period_at(before)
        } else {
            quest_recurrence::weekly_period_at(before)
        };
        completed.cadence_last_claimed_period = Some(period);
        completed.cadence_high_watermark_period = Some(period);
        completed.current = completed.required;
        completed.task_progress.insert("kill:39".into(), 80);
        completed.accepted_periodic_reward = Some(periodic_quests::AcceptedPeriodicReward {
            level: 10,
            experience: 600,
            gold: 4000,
        });
        for stage in [QuestStage::InProgress, QuestStage::ReadyToTurnIn] {
            let mut active = completed.clone();
            active.stage = stage;
            active.cadence_last_claimed_period = None;
            let reward = active.accepted_periodic_reward.clone();
            assert!(!quest_recurrence::refresh_states_for_test(
                std::slice::from_mut(&mut active),
                false,
                boundary
            ));
            assert_eq!(active.stage, stage);
            assert_eq!(active.current, completed.current);
            assert_eq!(active.task_progress, completed.task_progress);
            assert_eq!(active.accepted_periodic_reward, reward);
        }
        assert!(quest_recurrence::refresh_states_for_test(
            std::slice::from_mut(&mut completed),
            false,
            boundary
        ));
        assert_eq!(completed.stage, QuestStage::Available);
        assert_eq!(completed.current, 0);
        assert!(completed.task_progress.is_empty());
        assert_eq!(completed.accepted_periodic_reward, None);
    }
}

#[test]
fn same_slot_claim_is_global_across_bands_and_backward_clock_cannot_reopen_it() {
    let mut session = session(10, MirClass::Wizard);
    begin_quest(session.app.world_mut(), 92001);
    set_quest_stage(session.app.world_mut(), 92001, QuestStage::Completed);
    let now = quest_recurrence::server_now_millis();
    quest_recurrence::record_quest_completion_at(session.app.world_mut(), 92001, now);
    for id in [92001, 92006, 92011, 92016] {
        assert!(periodic_quests::slot_occupied_at(
            session.app.world(),
            id,
            false,
            now
        ));
        assert!(periodic_quests::slot_occupied_at(
            session.app.world(),
            id,
            false,
            now - DAY
        ));
        assert!(!periodic_quests::slot_occupied_at(
            session.app.world(),
            id,
            false,
            now + DAY
        ));
    }
    assert!(!periodic_quests::slot_occupied_at(
        session.app.world(),
        92002,
        false,
        now
    ));
    assert!(!periodic_quests::slot_occupied_at(
        session.app.world(),
        92004,
        false,
        now
    ));
    quest_recurrence::refresh_quest_recurrence_at(session.app.world_mut(), now + DAY);
    assert!(!periodic_quests::slot_occupied_at(
        session.app.world(),
        92006,
        false,
        now + DAY
    ));
    assert!(!periodic_quests::slot_occupied_at(
        session.app.world(),
        92006,
        false,
        now
    ));
}

#[test]
fn unfinished_task_hand_in_is_charged_to_current_period_and_legacy_high_watermark_is_shared() {
    let mut session = session(10, MirClass::Taoist);
    begin_quest(session.app.world_mut(), 92001);
    let now = quest_recurrence::server_now_millis();
    quest_recurrence::refresh_quest_recurrence_at(session.app.world_mut(), now);
    set_quest_stage(session.app.world_mut(), 92001, QuestStage::ReadyToTurnIn);
    quest_recurrence::record_quest_completion_at(session.app.world_mut(), 92001, now + DAY);
    let state = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|state| state.quest_id == 92001)
        .unwrap();
    assert_eq!(
        state.cadence_last_claimed_period,
        Some(quest_recurrence::daily_period_at(now + DAY))
    );
    // The high watermark of a different daily task also fences the family.
    assert!(periodic_quests::slot_occupied_at(
        session.app.world(),
        92006,
        false,
        now
    ));
}

#[test]
fn abandoning_a_periodic_hunt_clears_accepted_reward_and_releases_its_slot() {
    let mut session = session(14, MirClass::Warrior);
    begin_quest(session.app.world_mut(), 92001);
    advance_crystal_quest_kill(session.app.world_mut(), "Scarecrow");
    assert!(abandon_quest(session.app.world_mut(), 92001));
    let state = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|state| state.quest_id == 92001)
        .unwrap();
    assert_eq!(state.accepted_periodic_reward, None);
    assert!(state.task_progress.is_empty());
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 15;
    assert!(can_accept_quest(session.app.world(), 92006));
}

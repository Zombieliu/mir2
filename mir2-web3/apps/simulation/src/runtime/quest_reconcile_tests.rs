use mir2_protocol::{ClientPacket, ServerPacket};

use super::*;
use crate::{SimulationConfig, SimulationSession};

fn authenticated_session(config: SimulationConfig) -> (SimulationSession, i32) {
    let mut session = SimulationSession::new(config);
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
    (session, character_index)
}

fn assert_progress(session: &SimulationSession, quest_id: i32, stage: QuestStage, current: u32) {
    let quest = session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == quest_id)
        .expect("quest state");
    assert_eq!(quest.stage, stage, "quest {quest_id} stage");
    assert_eq!(quest.current, current, "quest {quest_id} progress");
}

#[test]
fn carry_and_dialogue_routes_remain_ready_while_kill_routes_remain_incomplete() {
    let (mut session, _) = authenticated_session(SimulationConfig::default());

    assert_eq!(
        begin_quest(session.app.world_mut(), 1),
        QuestStage::ReadyToTurnIn
    );
    assert_eq!(
        begin_quest(session.app.world_mut(), 7),
        QuestStage::ReadyToTurnIn
    );
    assert_eq!(
        begin_quest(session.app.world_mut(), 10),
        QuestStage::ReadyToTurnIn
    );
    assert_eq!(
        begin_quest(session.app.world_mut(), 11),
        QuestStage::InProgress
    );

    reconcile_effective_quest_states(session.app.world_mut());

    assert_progress(&session, 1, QuestStage::ReadyToTurnIn, 1);
    assert_progress(&session, 7, QuestStage::ReadyToTurnIn, 1);
    assert_progress(&session, 10, QuestStage::ReadyToTurnIn, 1);
    assert_progress(&session, 11, QuestStage::InProgress, 0);
}

#[test]
fn dialogue_only_q10_remains_ready_after_real_save_and_reload() {
    let config = SimulationConfig::default();
    let (mut session, character_index) = authenticated_session(config.clone());

    // Unit setup: isolate persistence reconciliation without walking the live
    // Jane-to-Don route. The saved and restored state uses the production path.
    assert_eq!(
        begin_quest(session.app.world_mut(), 10),
        QuestStage::ReadyToTurnIn
    );
    session.save_active_character().expect("save q10 state");
    drop(session);

    let mut restored = SimulationSession::new(config);
    assert!(restored
        .handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    restored.handle_packet(ClientPacket::StartGame { character_index });

    assert_progress(&restored, 10, QuestStage::ReadyToTurnIn, 1);
    let don_links = quest_dialog_links_for_npc(restored.app.world(), 451, &[10, 11, 12]);
    assert!(don_links
        .iter()
        .any(|link| link.target.eq_ignore_ascii_case("@quest:finish:10")));
}

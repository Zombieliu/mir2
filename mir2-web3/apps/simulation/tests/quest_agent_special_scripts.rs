use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SimulationConfig, SimulationSession,
};
use serde_json::json;

#[test]
fn q135_visible_turn_in_exposes_and_grants_the_missing_stoneheart_access_reward() {
    let character = CharacterRecord {
        index: 0,
        name: "QuestAgentGate".to_string(),
        level: 40,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.map_file_name = "3".to_string();
    save.map_title = "MongchonProvince".to_string();
    save.position = Point { x: 330, y: 333 };
    save.direction = MirDirection::Up;
    save.quest_states_json = vec![json!({
        "quest_id": 135,
        "title": "Find Passage to Ruins",
        "summary": "Find the Stone within Stone Temple",
        "reward_preview": "StoneHeart x1",
        "required": 1,
        "current": 1,
        "stage": "readyToTurnIn",
        "task_progress": { "flag:525": 1 }
    })
    .to_string()];

    let config = SimulationConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile();
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .expect("account store mutex")
        .accounts
        .insert("quest-agent-special-script".to_string(), account);

    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "quest-agent-special-script".to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start.iter().any(|packet| matches!(
        packet,
        ServerPacket::NewQuestInfo { info }
            if info.index == 135
                && info.rewards_fixed_item.iter().any(|reward|
                    reward.item.name == "StoneHeart" && reward.count == 1)
    )));

    session.force_authoritative_player_transform(Point { x: 330, y: 333 }, MirDirection::Up);
    session.interact(926);
    let dialog = session
        .world_snapshot()
        .active_npc_dialog
        .expect("Mongchon delegate dialog");
    assert!(dialog
        .links
        .iter()
        .any(|link| link.target == "@quest:finish:135"));
    let finish = session.select_npc_dialog_target("@quest:finish:135");
    assert!(finish.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&135)
    )));
    assert!(session
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.name == "StoneHeart" && item.quantity == 1));
}

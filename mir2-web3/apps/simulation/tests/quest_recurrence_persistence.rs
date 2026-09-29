use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, QuestStage, SimulationConfig,
    SimulationSession,
};
use serde_json::{json, Value};

fn fixture(account_id: &str, period: Option<u64>) -> SimulationConfig {
    let config = SimulationConfig::default();
    let character = CharacterRecord {
        index: 0,
        name: "CadenceCheck".to_owned(),
        level: 20,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    let mut quest = json!({
        "quest_id": 141,
        "title": "Gathering of Bones",
        "summary": "BichonProvince",
        "reward_preview": "Gold and experience",
        "required": 15,
        "current": 15,
        "stage": "completed",
        "task_progress": {}
    });
    if let Some(period) = period {
        quest["cadence_last_claimed_period"] = json!(period);
        quest["cadence_high_watermark_period"] = json!(period);
    }
    save.quest_states_json = vec![quest.to_string()];
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert(account_id.to_owned(), account);
    config
}

fn login(config: &SimulationConfig, account_id: &str) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: account_id.to_owned(),
        password: "demo".to_owned(),
    });
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
    let completed = packets
        .iter()
        .rev()
        .find_map(|packet| match packet {
            ServerPacket::CompleteQuest { completed_quests } => Some(completed_quests),
            _ => None,
        })
        .expect("login must send authoritative completed quest IDs");
    assert_eq!(
        completed.contains(&141),
        stage(&session) == QuestStage::Completed
    );
    session
}

fn stage(session: &SimulationSession) -> QuestStage {
    session
        .world_snapshot()
        .quest_log
        .iter()
        .find(|q| q.quest_id == 141)
        .unwrap()
        .stage
}

fn saved_quest(config: &SimulationConfig, account_id: &str) -> Value {
    let store = config.account_store.lock().unwrap();
    store.accounts[account_id].saves[&0]
        .quest_states_json
        .iter()
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .find(|row| row["quest_id"] == 141)
        .unwrap()
}

#[test]
fn daily_legacy_completed_save_stays_claimed_and_persists_migration() {
    let account = "cadence-legacy";
    let config = fixture(account, None);
    let mut session = login(&config, account);
    assert_eq!(stage(&session), QuestStage::Completed);
    session.handle_packet(ClientPacket::KeepAlive { time: i64::MAX });
    assert_eq!(
        stage(&session),
        QuestStage::Completed,
        "client time cannot advance the server calendar"
    );
    session.save_active_character().unwrap();
    let saved = saved_quest(&config, account);
    assert!(saved["cadence_last_claimed_period"].as_u64().unwrap() > 0);
    assert_eq!(
        saved["cadence_last_claimed_period"],
        saved["cadence_high_watermark_period"]
    );
    drop(session);
    assert_eq!(stage(&login(&config, account)), QuestStage::Completed);
}

#[test]
fn expired_daily_reopens_after_load_and_survives_normal_save_reload() {
    let account = "cadence-expired";
    let config = fixture(account, Some(0));
    let mut session = login(&config, account);
    assert_eq!(stage(&session), QuestStage::Available);
    session.handle_packet(ClientPacket::KeepAlive { time: 1 });
    session.save_active_character().unwrap();
    let saved = saved_quest(&config, account);
    assert_eq!(saved["cadence_last_claimed_period"], 0);
    assert!(saved["cadence_high_watermark_period"].as_u64().unwrap() > 0);
    assert_eq!(saved["current"], 0);
    drop(session);
    assert_eq!(stage(&login(&config, account)), QuestStage::Available);
}

#[test]
fn future_claim_cannot_reopen_or_change_another_accounts_expired_daily() {
    let config = fixture("cadence-future", Some(u64::MAX));
    let expired_config = fixture("cadence-other", Some(0));
    let other_account = expired_config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .remove("cadence-other")
        .unwrap();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert("cadence-other".to_owned(), other_account);
    let mut future = login(&config, "cadence-future");
    let other = login(&config, "cadence-other");
    future.handle_packet(ClientPacket::KeepAlive { time: i64::MAX });
    assert_eq!(stage(&future), QuestStage::Completed);
    assert_eq!(stage(&other), QuestStage::Available);
    future.save_active_character().unwrap();
    assert_eq!(
        saved_quest(&config, "cadence-future")["cadence_last_claimed_period"],
        json!(u64::MAX)
    );
}

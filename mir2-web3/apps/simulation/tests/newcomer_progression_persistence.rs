use std::sync::Mutex;

use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, QuestStage, SimulationConfig,
    SimulationSession, VisibleNpcRecord,
};
use serde_json::{json, Value};

static PROFILE_LOCK: Mutex<()> = Mutex::new(());

struct ProfileGuard(Option<std::ffi::OsString>);
impl ProfileGuard {
    fn set(enabled: bool) -> Self {
        let previous = std::env::var_os("MIR2_QUEST_CADENCE");
        std::env::set_var(
            "MIR2_QUEST_CADENCE",
            if enabled { "newcomer-v1" } else { "" },
        );
        Self(previous)
    }
}
impl Drop for ProfileGuard {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            std::env::set_var("MIR2_QUEST_CADENCE", value);
        } else {
            std::env::remove_var("MIR2_QUEST_CADENCE");
        }
    }
}

fn quest_row(id: i32, stage: &str, period: u64) -> Value {
    json!({
        "quest_id": id, "title": "Stored newcomer quest", "summary": "Daily",
        "reward_preview": "Gold", "required": 2, "current": 2,
        "stage": stage, "task_progress": {},
        "cadence_last_claimed_period": period, "cadence_high_watermark_period": period
    })
}

fn fixture(rows: Vec<Value>) -> SimulationConfig {
    let mut config = SimulationConfig::default();
    let character = CharacterRecord {
        index: 0,
        name: "NewcomerStore".to_owned(),
        level: 20,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.quest_states_json = rows.into_iter().map(|row| row.to_string()).collect();
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert("newcomer-store".to_owned(), account);
    config.visible_npcs.retain(|npc| npc.object_id != 24);
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: 24,
        name: "Bichon Wall Board".to_owned(),
        image: 5,
        colour_argb: -1,
        position: config.spawn.clone(),
        direction: MirDirection::Down,
        quest_ids: vec![],
        script_key: Some("BichonProvince/BichonWall/Board".to_owned()),
    });
    config.visible_npcs.last_mut().unwrap().position.x += 1;
    config
}

fn login(config: &SimulationConfig) -> (SimulationSession, Vec<ServerPacket>) {
    let mut session = SimulationSession::new(config.clone());
    let replies = session.handle_packet(ClientPacket::Login {
        account_id: "newcomer-store".to_owned(),
        password: "demo".to_owned(),
    });
    assert!(replies
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let replies = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(replies
        .iter()
        .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
    (session, replies)
}

fn removed(packets: &[ServerPacket], id: i32) -> bool {
    packets.iter().any(|packet| {
        matches!(packet,
        ServerPacket::ChangeQuest { quest_id, taken: false, quest_state: 2, .. } if *quest_id == id)
    })
}

#[test]
fn newcomer_milestone_normal_packets_save_reload_and_duplicate_finish() {
    let _lock = PROFILE_LOCK.lock().unwrap();
    let _profile = ProfileGuard::set(true);
    let config = fixture(vec![]);
    let (mut session, _) = login(&config);
    session.interact(24);
    let accepted = session.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 24,
        quest_index: 2_100_015,
    });
    assert!(
        accepted.iter().any(|p| matches!(
            p,
            ServerPacket::ChangeQuest {
                quest_id: 2_100_015,
                taken: true,
                ..
            }
        )),
        "{accepted:?}; dialog={:?}",
        session.world_snapshot().active_npc_dialog
    );
    session.interact(24);
    let finished = session.handle_packet(ClientPacket::FinishQuest {
        quest_index: 2_100_015,
        selected_item_index: -1,
    });
    assert!(removed(&finished, 2_100_015), "{finished:?}");
    session.save_active_character().unwrap();
    let gold = config.account_store.lock().unwrap().accounts["newcomer-store"].saves[&0].gold;
    assert_eq!(gold, 1280 + 5000);
    drop(session);
    let (mut session, _) = login(&config);
    session.interact(24);
    assert!(!removed(
        &session.handle_packet(ClientPacket::FinishQuest {
            quest_index: 2_100_015,
            selected_item_index: -1
        }),
        2_100_015
    ));
    session.save_active_character().unwrap();
    assert_eq!(
        config.account_store.lock().unwrap().accounts["newcomer-store"].saves[&0].gold,
        gold
    );
}

#[test]
fn newcomer_previous_day_ready_bonus_reloads_without_yesterdays_credit() {
    let _lock = PROFILE_LOCK.lock().unwrap();
    let _profile = ProfileGuard::set(true);
    let config = fixture(vec![
        quest_row(2_100_001, "completed", 0),
        quest_row(2_100_002, "completed", 0),
        quest_row(2_100_004, "readyToTurnIn", 0),
    ]);
    let (mut session, _) = login(&config);
    let snapshot = session.world_snapshot();
    let bonus = snapshot
        .quest_log
        .iter()
        .find(|quest| quest.quest_id == 2_100_004)
        .unwrap();
    assert_eq!(bonus.stage, QuestStage::InProgress);
    assert_eq!(bonus.current, 0);
    session.interact(24);
    assert!(!removed(
        &session.handle_packet(ClientPacket::FinishQuest {
            quest_index: 2_100_004,
            selected_item_index: -1
        }),
        2_100_004
    ));
    session.save_active_character().unwrap();
    drop(session);
    let (session, _) = login(&config);
    assert_eq!(
        session
            .world_snapshot()
            .quest_log
            .iter()
            .find(|quest| quest.quest_id == 2_100_004)
            .unwrap()
            .stage,
        QuestStage::InProgress
    );
}

#[test]
fn newcomer_disabled_profile_keeps_stored_rewards_inaccessible() {
    let _lock = PROFILE_LOCK.lock().unwrap();
    let _profile = ProfileGuard::set(false);
    let config = fixture(vec![quest_row(2_100_015, "readyToTurnIn", 0)]);
    let (mut session, _) = login(&config);
    assert!(!session
        .world_snapshot()
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 2_100_015));
    session.interact(24);
    assert!(!removed(
        &session.handle_packet(ClientPacket::FinishQuest {
            quest_index: 2_100_015,
            selected_item_index: -1
        }),
        2_100_015
    ));
    session.save_active_character().unwrap();
    let store = config.account_store.lock().unwrap();
    let save = &store.accounts["newcomer-store"].saves[&0];
    assert_eq!(save.gold, 1280);
    assert!(
        save.quest_states_json
            .iter()
            .any(|row| row.contains("2100015")),
        "disabling the profile must preserve history"
    );
}

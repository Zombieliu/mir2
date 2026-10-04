//! Prepared LOCAL fixtures exercising ordinary ClientPacket NPC paths.
//! These seeded levels, quests and items are not a natural player journey,
//! source-item acquisition, public Gateway or native acceptance receipt.
use mir2_game_data::{crystal_item_by_name, crystal_map_respawns_by_file_name};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point, ServerPacket};
use mir2_simulation::{AccountRecord, CharacterRecord, CharacterSaveRecord, SimulationConfig, SimulationSession};
use serde_json::{json, Value};

const ACCOUNT: &str = "classic-late-route-local-fixture";
const STONE: u32 = 1142;
const BIG_TAOIST: u32 = 843;

fn prepared_session(map: &str, position: Point, level: u16, quests: &[(i32, &str)], hearts: u32) -> SimulationSession {
    prepared_session_with(MirClass::Warrior, map, position, level, quests, hearts, |_| {})
}

fn prepared_session_with(class: MirClass, map: &str, position: Point, level: u16, quests: &[(i32, &str)], hearts: u32,
    customize: impl FnOnce(&mut CharacterSaveRecord)) -> SimulationSession {
    let character = CharacterRecord { index: 0, name: "PreparedClassicGate".into(), level,
        class, gender: MirGender::Male };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.map_file_name = map.into();
    save.map_title = map.into();
    save.position = position;
    save.direction = MirDirection::Right;
    save.quest_states_json = quests.iter().map(|(id, stage)| json!({
        "quest_id": id, "title": "Prepared gate fixture", "summary": "Not a natural quest journey",
        "reward_preview": "", "required": 1, "current": if *stage == "completed" {1} else {0},
        "stage": stage, "task_progress": {}
    }).to_string()).collect();
    // BigTaoist's source first-visit flag is prepared explicitly. The tested
    // quest predicate and visible eight-page sequence remain normal packets.
    save.npc_flag_states_json = vec![json!({"index": 530, "value": true}).to_string()];
    if hearts > 0 {
        save.inventory_items_json = (0..hearts).map(|slot| json!({
            "key": "crystal-item-1080", "name": "StoneHeart", "icon": 448, "slot": slot,
            "unique_id": 1080001 + slot, "container": "bag1", "quantity": 1,
            "description": "Prepared local gate item", "durability_current": null,
            "durability_max": null, "weight": 1, "equip_slot": null,
            "attack": 0, "defence": 0, "heal_hp": 0, "heal_mp": 0,
            "user_item_metadata": {"item_index": 1080}
        }).to_string()).collect();
    }
    customize(&mut save);
    let config = SimulationConfig::default().with_crystal_world_runtime().with_platinum_176_profile();
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config.account_store.lock().unwrap().accounts.insert(ACCOUNT.into(), account);
    let mut session = SimulationSession::new(config);
    assert!(session.handle_packet(ClientPacket::Login { account_id: ACCOUNT.into(), password: "demo".into() })
        .iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start.iter().any(|packet| matches!(packet, ServerPacket::MapInformation { info } if info.file_name == map)), "prepared StartGame {start:?}");
    session
}

fn stone(quests: &[(i32, &str)], level: u16, hearts: u32) -> SimulationSession {
    prepared_session("D715", Point { x: 271, y: 85 }, level, quests, hearts)
}

fn call(session: &mut SimulationSession, id: u32, key: &str) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::CallNpc { object_id: id, key: key.into() })
}

fn has_link(session: &SimulationSession, target: &str) -> bool {
    session.world_snapshot().active_npc_dialog.as_ref()
        .is_some_and(|dialog| dialog.links.iter().any(|link| link.target.eq_ignore_ascii_case(target)))
}

fn saved_flag(session: &SimulationSession, flag: u32) -> bool {
    session.active_character_checkpoint().unwrap().npc_flag_states_json.iter().any(|encoded| {
        let value: Value = serde_json::from_str(encoded).unwrap();
        value["index"] == flag && value["value"] == true
    })
}

fn hearts(session: &SimulationSession) -> u32 {
    session.world_snapshot().inventory_items.iter().filter(|item| item.name == "StoneHeart")
        .map(|item| item.quantity).sum()
}

#[test]
fn stone_q135_only_current_accepted_states_find_the_stone() {
    for (stage, expected) in [("available", false), ("inProgress", true), ("readyToTurnIn", true), ("completed", false)] {
        let mut session = stone(&[(135, stage)], 40, 0);
        call(&mut session, STONE, "@Main");
        assert_eq!(saved_flag(&session, 525), expected, "q135 {stage}: Available/Completed are not CurrentQuests");
    }
    let mut absent = stone(&[], 40, 0);
    call(&mut absent, STONE, "@Main");
    assert!(!saved_flag(&absent, 525));
}

#[test]
fn completed_q153_exposes_a_real_stone_link_and_committed_map_transfer() {
    let mut session = stone(&[(153, "completed")], 40, 2);
    call(&mut session, STONE, "@Main");
    let dialog = session.world_snapshot().active_npc_dialog.expect("actual source NPC dialog");
    assert_eq!(dialog.npc_object_id, STONE);
    assert!(dialog.body.iter().any(|line| line.contains("Mysterious Stone")));
    assert!(has_link(&session, "@stonetomba"), "completed q153 must expose the actual source link");
    let packets = call(&mut session, STONE, "@stonetomba");
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::MapInformation { info } if info.file_name == "D710A")), "normal NPC action needs a committed MapInformation receipt");
    let saved = session.active_character_checkpoint().unwrap();
    assert_eq!(saved.map_file_name, "D710A");
    assert_eq!(saved.position, Point { x: 29, y: 17 });
    assert_eq!(hearts(&session), 1);
    // The old NPC and link cannot debit a second item after leaving its map.
    let replay = call(&mut session, STONE, "@stonetomba");
    assert!(!replay.iter().any(|packet| matches!(packet, ServerPacket::MapInformation { .. })));
    assert_eq!(hearts(&session), 1);
    assert!(session.handle_packet(ClientPacket::LogOut).iter().any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(session.active_character_checkpoint().unwrap().map_file_name, "D710A");
    assert_eq!(hearts(&session), 1, "normal logout/relogin must retain exactly-one debit");
}

#[test]
fn q153_incomplete_or_absent_never_grants_the_ancient_entrance() {
    for quests in [vec![], vec![(153, "available")], vec![(153, "inProgress")], vec![(153, "readyToTurnIn")]] {
        let mut session = stone(&quests, 40, 1);
        call(&mut session, STONE, "@Main");
        assert!(!has_link(&session, "@stonetomba"), "q153 {quests:?} must not be completed");
        call(&mut session, STONE, "@stonetomba");
        assert_eq!(session.active_character_checkpoint().unwrap().map_file_name, "D715");
        assert_eq!(hearts(&session), 1);
    }
}

#[test]
fn stone_source_numeric_level_window_is_22_through_42() {
    for (level, eligible) in [(21, false), (22, true), (42, true), (43, false)] {
        let mut session = stone(&[(153, "completed")], level, 1);
        call(&mut session, STONE, "@Main");
        assert_eq!(has_link(&session, "@stonetomba"), eligible, "source LEVEL >21 AND <43 at level{level}");
    }
}

#[test]
fn stone_item_count_zero_one_two_is_authoritative() {
    for count in [0, 1, 2] {
        let mut session = stone(&[(153, "completed")], 40, count);
        call(&mut session, STONE, "@Main");
        assert!(has_link(&session, "@stonetomba"));
        let packets = call(&mut session, STONE, "@stonetomba");
        let transferred = packets.iter().any(|packet| matches!(packet, ServerPacket::MapInformation { info } if info.file_name == "D710A"));
        assert_eq!(transferred, count > 0);
        assert_eq!(hearts(&session), count.saturating_sub(1));
        assert_eq!(session.active_character_checkpoint().unwrap().map_file_name, if count > 0 {"D710A"} else {"D715"});
    }
}

#[test]
fn stone_wrong_object_and_out_of_range_do_not_grant_or_debit() {
    let mut session = stone(&[(135, "inProgress"), (153, "completed")], 40, 1);
    call(&mut session, BIG_TAOIST, "@Main");
    call(&mut session, BIG_TAOIST, "@stonetomba");
    assert!(!saved_flag(&session, 525));
    assert_eq!(hearts(&session), 1);
    let mut distant = prepared_session("D715", Point { x: 250, y: 85 }, 40, &[(135, "inProgress"), (153, "completed")], 1);
    call(&mut distant, STONE, "@Main");
    call(&mut distant, STONE, "@stonetomba");
    assert!(!saved_flag(&distant, 525));
    assert_eq!(distant.active_character_checkpoint().unwrap().map_file_name, "D715");
    assert_eq!(hearts(&distant), 1);
}

#[test]
fn big_taoist_completed_q146_exposes_and_consumes_only_current_dialog_links() {
    // Prepared inside D10061: this does not validate its missing ordinary inbound entry.
    let mut session = prepared_session("D10061", Point { x: 11, y: 17 }, 40, &[(146, "completed")], 0);
    call(&mut session, BIG_TAOIST, "@Main");
    assert!(has_link(&session, "@Quest1"), "completed q146 must expose the source live first page");
    for index in 1..=8 {
        let target = format!("@Quest{index}");
        assert!(has_link(&session, &target), "only observed {target} may be selected");
        call(&mut session, BIG_TAOIST, &target);
    }
    assert!(saved_flag(&session, 531));
    assert!(session.handle_packet(ClientPacket::LogOut).iter().any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(saved_flag(&session, 531));
    assert_eq!(session.active_character_checkpoint().unwrap().map_file_name, "D10061");
}

#[test]
fn big_taoist_incomplete_q146_never_exposes_or_executes_the_completion_sequence() {
    for quests in [vec![], vec![(146, "available")], vec![(146, "inProgress")], vec![(146, "readyToTurnIn")]] {
        let mut session = prepared_session("D10061", Point { x: 11, y: 17 }, 40, &quests, 0);
        call(&mut session, BIG_TAOIST, "@Main");
        assert!(!has_link(&session, "@Quest1"), "incomplete q146 {quests:?}");
        call(&mut session, BIG_TAOIST, "@Quest8");
        assert!(!saved_flag(&session, 531));
    }
}

#[test]
fn normal_q135_reward_is_a_valid_carried_stoneheart_after_logout() {
    let mut session = prepared_session_with(MirClass::Warrior, "3", Point { x: 330, y: 333 }, 40, &[(135, "readyToTurnIn")], 0, |save| {
        let mut quest: Value = serde_json::from_str(&save.quest_states_json[0]).unwrap();
        quest["current"] = json!(1); quest["task_progress"] = json!({"flag:525":1});
        save.quest_states_json = vec![quest.to_string()];
        save.npc_flag_states_json.push(json!({"index":525,"value":true}).to_string());
    });
    call(&mut session, 926, "@Main");
    assert!(has_link(&session, "@quest:finish:135"));
    let finish = call(&mut session, 926, "@quest:finish:135");
    assert!(finish.iter().any(|packet| matches!(packet, ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&135))));
    let item = session.world_snapshot().inventory_items.into_iter().find(|item| item.name == "StoneHeart").expect("normal q135 reward, not a GiveItem command");
    assert_eq!(item.key, "crystal-item-1080");
    assert_eq!(item.quantity, 1);
    let unique_id = item.unique_id;
    assert_ne!(unique_id, 0);
    assert!(session.handle_packet(ClientPacket::LogOut).iter().any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })), "real reward must pass saved-carrier validation");
    assert!(session.world_snapshot().inventory_items.iter().any(|item| item.unique_id == unique_id && item.quantity == 1));
}

#[test]
fn another_valid_item_cannot_substitute_for_stoneheart() {
    let mut session = prepared_session_with(MirClass::Warrior, "D715", Point {x:271,y:85}, 40, &[(153,"completed")], 1, |save| {
        let mut item: Value = serde_json::from_str(&save.inventory_items_json[0]).unwrap();
        let potion = crystal_item_by_name("TownTeleport").unwrap();
        item["key"] = json!(format!("crystal-item-{}", potion.item_index));
        item["name"] = json!(potion.name);
        item["user_item_metadata"]["item_index"] = json!(potion.item_index);
        save.inventory_items_json = vec![item.to_string()];
    });
    call(&mut session, STONE, "@Main");
    assert!(has_link(&session, "@stonetomba"));
    let packets = call(&mut session, STONE, "@stonetomba");
    assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::MapInformation { .. })));
    assert_eq!(session.active_character_checkpoint().unwrap().map_file_name, "D715");
    assert!(session.world_snapshot().inventory_items.iter().any(|item| item.unique_id == 1080001));
}

fn book_session(class: MirClass, level: u16, name: &str) -> SimulationSession {
    let info = crystal_item_by_name(name).expect("actual imported book");
    prepared_session_with(class, "0", Point {x:288,y:616}, level, &[], 0, |save| {
        save.inventory_items_json = vec![json!({
            "key": format!("crystal-item-{}", info.item_index), "name": info.name,
            "icon": info.image, "slot": 0, "unique_id": 71080, "container": "bag1", "quantity": 1,
            "description": "Prepared ordinary UseItem gate only", "durability_current": null,
            "durability_max": null, "weight": info.weight, "equip_slot": null,
            "attack": 0, "defence": 0, "heal_hp": 0, "heal_mp": 0,
            "user_item_metadata": {"item_index": info.item_index}
        }).to_string()];
    })
}

#[test]
fn late_book_use_rejects_underlevel_wrong_class_and_nonowned_identity_without_consumption() {
    for (class, level, uid) in [(MirClass::Warrior,46,71080), (MirClass::Wizard,50,71080), (MirClass::Warrior,50,99999)] {
        let mut session = book_session(class,level,"CounterAttack");
        let before = session.world_snapshot().inventory_items;
        let packets = session.handle_packet(ClientPacket::UseItem { unique_id:uid, grid:MirGridType::Inventory });
        assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::UseItem { success:false, .. })));
        assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::NewMagic { .. })));
        assert_eq!(session.world_snapshot().inventory_items.len(), before.len());
        assert!(session.world_snapshot().inventory_items.iter().any(|item| item.unique_id == 71080 && item.quantity == 1));
    }
}

#[test]
fn prepared_late_book_learning_has_real_newmagic_one_debit_and_normal_save_but_is_not_a_drop() {
    for (class, name) in [(MirClass::Warrior,"CounterAttack"), (MirClass::Wizard,"Blink"), (MirClass::Taoist,"Plague")] {
        let mut session = book_session(class,50,name);
        let packets = session.handle_packet(ClientPacket::UseItem { unique_id:71080, grid:MirGridType::Inventory });
        assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::UseItem { success:true, .. })), "{name} UseItem");
        assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::NewMagic { .. })), "{name} NewMagic");
        assert!(!session.world_snapshot().inventory_items.iter().any(|item| item.unique_id == 71080));
        let replay = session.handle_packet(ClientPacket::UseItem { unique_id:71080, grid:MirGridType::Inventory });
        assert!(!replay.iter().any(|packet| matches!(packet, ServerPacket::NewMagic { .. })));
        let skills = session.active_character_checkpoint().unwrap().skill_states_json;
        assert!(skills.iter().any(|encoded| encoded.to_ascii_lowercase().contains(&name.to_ascii_lowercase())));
        assert!(session.handle_packet(ClientPacket::LogOut).iter().any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
        session.handle_packet(ClientPacket::StartGame { character_index:0 });
        assert!(session.active_character_checkpoint().unwrap().skill_states_json.iter().any(|encoded| encoded.to_ascii_lowercase().contains(&name.to_ascii_lowercase())));
    }
}

#[test]
fn supplied_great_tao_needmove_is_not_bypassed_by_an_ordinary_walk() {
    let source = crystal_map_respawns_by_file_name("D10051").unwrap();
    let target = crystal_map_respawns_by_file_name("D10061").unwrap();
    assert!(source.movements.iter().any(|movement| movement.map_index == target.map_index
        && movement.source == Point { x:178, y:53 } && movement.need_move && !movement.need_hole));
    let mut session = prepared_session("D10051", Point { x:177, y:53 }, 50, &[], 0);
    let packets = session.handle_packet(ClientPacket::Walk { direction:MirDirection::Right });
    assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::MapInformation { info } if info.file_name == "D10061")));
    let saved = session.active_character_checkpoint().unwrap();
    assert_eq!(saved.map_file_name, "D10051", "supplied unbound NeedMove remains a defect");
    assert_eq!(saved.position, Point {x:178,y:53}, "prepared adjacent ordinary walk reaches the actual source cell; this is not a full Red Moon journey");
}

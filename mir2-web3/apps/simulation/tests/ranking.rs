use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, PlayerInspectInfo, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, RankingInspectIdentity,
    RankingInspectPresence, RankingInspectRequest, SimulationConfig, SimulationSession,
};
use serde_json::json;

fn ranked_character(index: i32, name: &str, level: u16, class: MirClass) -> CharacterRecord {
    CharacterRecord {
        index,
        name: name.to_string(),
        level,
        class,
        gender: MirGender::Male,
    }
}

fn save_for_character(
    config: &SimulationConfig,
    character: CharacterRecord,
    experience: i64,
) -> CharacterSaveRecord {
    let mut save = CharacterSaveRecord::new(character);
    save.map_file_name = config.map.file_name.clone();
    save.map_title = config.map.title.clone();
    save.position = config.spawn.clone();
    save.direction = MirDirection::Down;
    save.experience = experience;
    save
}

fn account_for_character(
    config: &SimulationConfig,
    character: CharacterRecord,
    experience: i64,
) -> AccountRecord {
    let mut account = AccountRecord::empty();
    account.characters.push(character.clone());
    account.saves.insert(
        character.index,
        save_for_character(config, character, experience),
    );
    account
}

fn ranking_packet(packets: &[ServerPacket]) -> &ServerPacket {
    packets
        .iter()
        .find(|packet| matches!(packet, ServerPacket::Rankings { .. }))
        .expect("ranking request should emit Rankings")
}

#[test]
fn get_ranking_returns_crystal_rankings_from_character_saves() {
    let config = SimulationConfig::default();
    let scout = ranked_character(0, "Scout", 12, MirClass::Warrior);
    let blade = ranked_character(1, "Blade", 40, MirClass::Warrior);
    let mage = ranked_character(2, "Mage", 40, MirClass::Wizard);
    let arrow = ranked_character(3, "Arrow", 18, MirClass::Archer);

    {
        let mut store = config.account_store.lock().expect("account store lock");
        store.accounts.clear();
        store.next_character_index = 4;
        store.accounts.insert(
            "demo".to_string(),
            account_for_character(&config, scout.clone(), 50),
        );
        store.accounts.insert(
            "blade".to_string(),
            account_for_character(&config, blade.clone(), 500),
        );
        store.accounts.insert(
            "mage".to_string(),
            account_for_character(&config, mage.clone(), 900),
        );
        store.accounts.insert(
            "arrow".to_string(),
            account_for_character(&config, arrow.clone(), 1),
        );
    }

    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    let overall = session.handle_packet(ClientPacket::GetRanking {
        rank_type: 0,
        rank_index: 0,
        online_only: false,
    });
    match ranking_packet(&overall) {
        ServerPacket::Rankings {
            rank_type,
            my_rank,
            listing_details,
            listings,
            count,
        } => {
            assert_eq!(*rank_type, 0);
            assert_eq!(*count, 4);
            assert_eq!(*my_rank, 4);
            assert_eq!(
                listing_details
                    .iter()
                    .map(|entry| entry.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["Mage", "Blade", "Arrow", "Scout"]
            );
            assert_eq!(*listings, vec![2, 1, 3, 0]);
        }
        _ => unreachable!("ranking_packet only returns Rankings"),
    }

    let warriors = session.handle_packet(ClientPacket::GetRanking {
        rank_type: 1,
        rank_index: 0,
        online_only: false,
    });
    match ranking_packet(&warriors) {
        ServerPacket::Rankings {
            my_rank,
            listing_details,
            count,
            ..
        } => {
            assert_eq!(*count, 2);
            assert_eq!(*my_rank, 2);
            assert_eq!(
                listing_details
                    .iter()
                    .map(|entry| entry.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["Blade", "Scout"]
            );
        }
        _ => unreachable!("ranking_packet only returns Rankings"),
    }

    let online = session.handle_packet(ClientPacket::GetRanking {
        rank_type: 0,
        rank_index: 0,
        online_only: true,
    });
    match ranking_packet(&online) {
        ServerPacket::Rankings {
            my_rank,
            listing_details,
            count,
            ..
        } => {
            assert_eq!(*count, 1);
            assert_eq!(*my_rank, 4);
            assert_eq!(listing_details[0].name, "Scout");
        }
        _ => unreachable!("ranking_packet only returns Rankings"),
    }
}

const INSPECT_TARGET: i32 = 7;
const INSPECT_TARGET_ACCOUNT: &str = "ranking-inspect-target";

fn inspect_equipment(slot: &str, item_type: u8, uid: u64) -> String {
    let item = mir2_game_data::crystal_item_manifest_ref()
        .items.iter().find(|item| item.item_type == item_type)
        .expect("actual Crystal equipment template");
    json!({
        "key": format!("crystal-item-{}", item.item_index),
        "slot": slot, "name": item.name, "icon": item.image,
        "shape": u16::try_from(item.shape).ok(), "description": "Ranking inspect carrier fixture",
        "quantity": 1, "durability_current": item.durability,
        "durability_max": item.durability, "attack": 0, "defence": 0,
        "user_item_unique_id": uid, "user_item_metadata": { "item_index": item.item_index }
    }).to_string()
}

fn inspect_config() -> SimulationConfig {
    // SimulationConfig::default supplies an independent memory store: no file
    // repository or source database is involved in these public API fixtures.
    let config = SimulationConfig::default();
    let mut viewer = account_for_character(
        &config, ranked_character(0, "InspectViewer", 12, MirClass::Warrior), 0,
    );
    let mut target = account_for_character(
        &config, ranked_character(INSPECT_TARGET, "RankTarget", 48, MirClass::Taoist), 100,
    );
    for (account, index, hair) in [(&mut viewer, 0, 3), (&mut target, INSPECT_TARGET, 9)] {
        let save = account.saves.get_mut(&index).unwrap();
        save.equipment_items_explicit_empty = true;
        save.equipment_items_json.clear();
        save.stage5_systems_json = Some(json!({
            "appearance": { "hair": hair },
            "relationship": { "partnerIdentity": null }
        }).to_string());
    }
    target.saves.get_mut(&INSPECT_TARGET).unwrap().equipment_items_json =
        vec![inspect_equipment("weapon", 1, 0)];
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        store.next_character_index = INSPECT_TARGET + 1;
        store.accounts.insert("demo".into(), viewer);
        store.accounts.insert(INSPECT_TARGET_ACCOUNT.into(), target);
    }
    config
}

fn start_inspect_viewer(config: SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    });
    assert!(login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
}

fn prepare_target_inspect(session: &SimulationSession) -> RankingInspectRequest {
    let request = session.prepare_ranking_inspect(INSPECT_TARGET as u32)
        .expect("active requester may query another ranking character");
    assert_eq!(request.target_identity(), &RankingInspectIdentity {
        account_id: INSPECT_TARGET_ACCOUNT.into(), character_index: INSPECT_TARGET,
        name: "RankTarget".into(),
    });
    request
}

fn inspected_player(packets: Vec<ServerPacket>) -> PlayerInspectInfo {
    match packets.as_slice() {
        [ServerPacket::PlayerInspect { info }] => info.clone(),
        _ => panic!("one complete PlayerInspect expected: {packets:?}"),
    }
}

#[test]
fn ranking_inspect_public_session_requires_login_and_start_game_before_a_ticket() {
    let mut session = SimulationSession::new(inspect_config());
    assert!(session.prepare_ranking_inspect(INSPECT_TARGET as u32).is_none());
    assert!(session.active_ranking_inspect_projection().is_none());
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.prepare_ranking_inspect(INSPECT_TARGET as u32).is_none());
    session.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "incorrect".into() });
    assert!(session.prepare_ranking_inspect(INSPECT_TARGET as u32).is_none());
    let login = session.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() });
    assert!(login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session.prepare_ranking_inspect(INSPECT_TARGET as u32).is_none());
    assert!(session.active_ranking_inspect_projection().is_none());
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let info = inspected_player(session.complete_ranking_inspect(
        prepare_target_inspect(&session), RankingInspectPresence::Offline,
    ));
    assert_eq!(info.name, "RankTarget");
    assert_eq!(session.active_ranking_inspect_projection().unwrap().identity.character_index, 0);
}

#[test]
fn ranking_inspect_public_offline_projects_target_and_all_fourteen_actual_slots() {
    let config = inspect_config();
    let actual_slots = [
        ("weapon", 1), ("armour", 2), ("helmet", 4), ("torch", 12),
        ("necklace", 5), ("braceletLeft", 6), ("braceletRight", 8),
        ("ringLeft", 7), ("ringRight", 7), ("amulet", 8),
        ("belt", 9), ("boots", 10), ("stone", 11), ("mount", 19),
    ];
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.get_mut(INSPECT_TARGET_ACCOUNT).unwrap().saves.get_mut(&INSPECT_TARGET).unwrap()
            .equipment_items_json = actual_slots.iter().enumerate()
                .map(|(slot, (name, item_type))| inspect_equipment(name, *item_type, slot as u64))
                .collect();
    }
    let session = start_inspect_viewer(config.clone());
    let info = inspected_player(session.complete_ranking_inspect(
        prepare_target_inspect(&session), RankingInspectPresence::Offline,
    ));
    assert_eq!((info.name.as_str(), info.level, info.hair), ("RankTarget", 48, 9));
    assert_eq!((info.class, info.gender), (MirClass::Taoist, MirGender::Male));
    assert!(info.guild_name.is_empty() && info.guild_rank.is_empty() && info.lover_name.is_empty());
    assert!(!info.allow_observe && !info.is_hero);
    assert_eq!(info.equipment.len(), 14);
    for (slot, (_, item_type)) in actual_slots.iter().enumerate() {
        let equipped = info.equipment[slot].as_ref().expect("actual equipment root");
        assert_eq!(equipped.unique_id, slot as u64);
        assert_eq!(mir2_game_data::crystal_item_by_index(equipped.item_index).unwrap().item_type, *item_type);
    }
    // Persisted absence produces a hole at the same full protocol width.
    let _ = config.account_store.lock().unwrap().accounts.get_mut(INSPECT_TARGET_ACCOUNT).unwrap()
        .saves.get_mut(&INSPECT_TARGET).unwrap().equipment_items_json.pop();
    let with_hole = inspected_player(session.complete_ranking_inspect(
        prepare_target_inspect(&session), RankingInspectPresence::Offline,
    ));
    assert_eq!(with_hole.equipment.len(), 14);
    assert!(with_hole.equipment[13].is_none());
    assert_eq!(with_hole.equipment[0].as_ref().unwrap().unique_id, 0);
}

#[test]
fn ranking_inspect_public_offline_rejects_unknown_raw_facts_instead_of_defaults() {
    let config = inspect_config();
    let session = start_inspect_viewer(config.clone());
    assert_eq!(inspected_player(session.complete_ranking_inspect(
        prepare_target_inspect(&session), RankingInspectPresence::Offline,
    )).hair, 9);
    for raw in [
        None,
        Some("{".to_owned()),
        Some(json!({"relationship": {"partnerIdentity": null}}).to_string()),
        Some(json!({"appearance": {"hair": 256}, "relationship": {"partnerIdentity": null}}).to_string()),
        Some(json!({"appearance": {"hair": 9}, "relationship": {"partnerName": "CachedOnly"}}).to_string()),
    ] {
        config.account_store.lock().unwrap().accounts.get_mut(INSPECT_TARGET_ACCOUNT).unwrap()
            .saves.get_mut(&INSPECT_TARGET).unwrap().stage5_systems_json = raw;
        assert!(session.complete_ranking_inspect(
            prepare_target_inspect(&session), RankingInspectPresence::Offline,
        ).is_empty());
    }
}

#[test]
fn ranking_inspect_public_target_wire_index_stays_globally_unique() {
    let config = inspect_config();
    let session = start_inspect_viewer(config.clone());
    assert!(session.prepare_ranking_inspect(u32::MAX).is_none());
    assert!(session.prepare_ranking_inspect(99).is_none());
    config.account_store.lock().unwrap().accounts.insert("duplicate-target".into(),
        AccountRecord::new(ranked_character(INSPECT_TARGET, "OtherTarget", 20, MirClass::Wizard)));
    assert!(session.prepare_ranking_inspect(INSPECT_TARGET as u32).is_none());
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.remove("duplicate-target");
        store.accounts.get_mut(INSPECT_TARGET_ACCOUNT).unwrap().characters.push(
            ranked_character(INSPECT_TARGET, "DuplicateWithinAccount", 20, MirClass::Wizard),
        );
    }
    assert!(session.prepare_ranking_inspect(INSPECT_TARGET as u32).is_none());
}

#[test]
fn ranking_inspect_public_partner_uses_current_name_and_full_account_index() {
    let config = inspect_config();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.insert("partner".into(),
            AccountRecord::new(ranked_character(31, "CachedPartner", 30, MirClass::Wizard)));
        store.accounts.get_mut("partner").unwrap().characters[0].name = "CurrentPartner".into();
        store.accounts.insert("unrelated-partner-index".into(),
            AccountRecord::new(ranked_character(31, "UnrelatedName", 30, MirClass::Warrior)));
        store.accounts.get_mut(INSPECT_TARGET_ACCOUNT).unwrap().saves.get_mut(&INSPECT_TARGET).unwrap()
            .stage5_systems_json = Some(json!({
                "appearance": {"hair": 9},
                "relationship": {"partnerIdentity": {"accountId": "partner", "characterIndex": 31},
                    "partnerName": "CachedPartner"}
            }).to_string());
    }
    let session = start_inspect_viewer(config.clone());
    let info = inspected_player(session.complete_ranking_inspect(
        prepare_target_inspect(&session), RankingInspectPresence::Offline,
    ));
    assert_eq!(info.name, "RankTarget");
    assert_eq!(info.lover_name, "CurrentPartner");
    config.account_store.lock().unwrap().accounts.get_mut("partner").unwrap().characters.push(
        ranked_character(31, "AmbiguousPartner", 30, MirClass::Wizard),
    );
    assert!(session.complete_ranking_inspect(
        prepare_target_inspect(&session), RankingInspectPresence::Offline,
    ).is_empty());
}

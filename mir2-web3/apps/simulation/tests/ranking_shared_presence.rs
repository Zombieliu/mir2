use std::collections::BTreeSet;

use mir2_protocol::{ClientPacket, MirClass, MirGender, PlayerInspectInfo, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, InProcessWorldRuntime,
    RankingInspectIdentity, RankingInspectOnlineProjection, RankingInspectPresence,
    RankingInspectRequest, SimulationConfig, SimulationSession, Stage5FriendIdentity,
    WorldCommand, WorldRuntime,
};
use serde_json::json;

fn fixture() -> SimulationSession {
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for index in 0..25 {
            let mut account = AccountRecord::empty();
            account.characters.push(CharacterRecord {
                index,
                name: format!("Rank{index:02}"),
                level: 30 - index as u16,
                class: if index == 1 {
                    MirClass::Wizard
                } else {
                    MirClass::Warrior
                },
                gender: MirGender::Male,
            });
            let mut save = CharacterSaveRecord::new(account.characters[0].clone());
            save.map_file_name = config.map.file_name.clone();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            account.saves.insert(index, save);
            store.accounts.insert(
                if index == 0 {
                    "demo".into()
                } else {
                    format!("account{index}")
                },
                account,
            );
        }
    }
    SimulationSession::new(config)
}

fn online() -> BTreeSet<(String, i32)> {
    [
        ("demo".into(), 0),
        ("account1".into(), 1),
        ("account24".into(), 24),
        // A character index under a different account is not the same identity.
        ("wrong-account".into(), 2),
    ]
    .into_iter()
    .collect()
}

fn ranking(packets: Vec<ServerPacket>) -> (i32, i32, Vec<i64>) {
    packets
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::Rankings {
                my_rank,
                count,
                listings,
                ..
            } => Some((my_rank, count, listings)),
            _ => None,
        })
        .expect("ranking response")
}

#[test]
fn shared_online_filter_uses_exact_identity_before_pagination_and_preserves_global_rank() {
    let mut session = fixture();
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let (rank, count, ids) = ranking(session.ranking_with_online_characters(0, 1, true, &online()));
    assert_eq!((rank, count), (1, 3));
    assert_eq!(ids, [1, 24]);
    let (_, count, ids) = ranking(session.ranking_with_online_characters(1, 0, true, &online()));
    assert_eq!(count, 2);
    assert_eq!(ids, [0, 24]);
    let (_, count, ids) =
        ranking(session.ranking_with_online_characters(0, 20, false, &BTreeSet::new()));
    assert_eq!(count, 25);
    assert_eq!(ids, [20, 21, 22, 23, 24]);
    let (_, count, ids) =
        ranking(session.ranking_with_online_characters(0, 0, true, &BTreeSet::new()));
    assert_eq!(count, 0);
    assert!(ids.is_empty());
}

#[test]
fn shared_ranking_does_not_bypass_login_or_start_game() {
    let mut session = fixture();
    assert!(session
        .ranking_with_online_characters(0, 0, true, &online())
        .is_empty());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert!(session
        .ranking_with_online_characters(0, 0, false, &online())
        .is_empty());
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session
        .ranking_with_online_characters(6, 0, false, &online())
        .is_empty());
    assert!(session
        .ranking_with_online_characters(0, -1, false, &online())
        .is_empty());
    session.handle_packet(ClientPacket::LogOut);
    assert!(session
        .ranking_with_online_characters(0, 0, false, &online())
        .is_empty());
}

const LIVE_TARGET: i32 = 7;
const LIVE_TARGET_ACCOUNT: &str = "ranking-live-target";
const LIVE_WEAPON_UID: u64 = 70_701;

fn runtime_equipment(slot: &str, item_type: u8, uid: u64) -> String {
    let item = mir2_game_data::crystal_item_manifest_ref().items.iter()
        .find(|item| item.item_type == item_type).expect("actual Crystal equipment template");
    json!({
        "key": format!("crystal-item-{}", item.item_index),
        "slot": slot, "name": item.name, "icon": item.image,
        "shape": u16::try_from(item.shape).ok(), "description": "Current online inspect fixture",
        "quantity": 1, "durability_current": item.durability,
        "durability_max": item.durability, "attack": 0, "defence": 0,
        "user_item_unique_id": uid, "user_item_metadata": {"item_index": item.item_index}
    }).to_string()
}

fn inspect_runtime_config() -> SimulationConfig {
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        store.next_character_index = LIVE_TARGET + 1;
        for (account_id, index, name, level, hair) in [
            ("demo", 0, "RuntimeViewer", 12, 3),
            (LIVE_TARGET_ACCOUNT, LIVE_TARGET, "LiveRankTarget", 52, 11),
        ] {
            let character = CharacterRecord {
                index, name: name.into(), level, class: MirClass::Warrior, gender: MirGender::Male,
            };
            let mut account = AccountRecord::new(character);
            let save = account.saves.get_mut(&index).unwrap();
            save.map_file_name = config.map.file_name.clone();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            save.equipment_items_explicit_empty = true;
            save.equipment_items_json.clear();
            save.stage5_systems_json = Some(json!({
                "appearance": {"hair": hair}, "relationship": {"partnerIdentity": null}
            }).to_string());
            if index == LIVE_TARGET {
                save.equipment_items_json.push(runtime_equipment("weapon", 1, LIVE_WEAPON_UID));
            }
            store.accounts.insert(account_id.into(), account);
        }
    }
    config
}

fn runtime_packet(runtime: &mut InProcessWorldRuntime, packet: ClientPacket) -> Vec<ServerPacket> {
    runtime.execute(WorldCommand::ClientPacket(packet)).expect("memory runtime packet")
}

fn runtime_login_start(runtime: &mut InProcessWorldRuntime, account_id: &str, index: i32) {
    let login = runtime_packet(runtime, ClientPacket::Login {
        account_id: account_id.into(), password: "demo".into(),
    });
    assert!(login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = runtime_packet(runtime, ClientPacket::StartGame { character_index: index });
    assert!(start.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
}

fn prepare_runtime_inspect(runtime: &InProcessWorldRuntime) -> RankingInspectRequest {
    let request = runtime.prepare_ranking_inspect(LIVE_TARGET as u32).expect("active runtime inspect ticket");
    assert_eq!(request.target_identity(), &RankingInspectIdentity {
        account_id: LIVE_TARGET_ACCOUNT.into(), character_index: LIVE_TARGET, name: "LiveRankTarget".into(),
    });
    request
}

fn runtime_inspected(packets: Vec<ServerPacket>) -> PlayerInspectInfo {
    match packets.as_slice() {
        [ServerPacket::PlayerInspect { info }] => info.clone(),
        _ => panic!("exact complete runtime PlayerInspect expected: {packets:?}"),
    }
}

#[test]
fn ranking_inspect_runtime_online_uses_current_target_export_instead_of_saved_fields() {
    let config = inspect_runtime_config();
    let mut viewer = InProcessWorldRuntime::new(config.clone());
    assert!(viewer.prepare_ranking_inspect(LIVE_TARGET as u32).is_none());
    assert!(viewer.active_ranking_inspect_projection().is_none());
    runtime_login_start(&mut viewer, "demo", 0);
    let mut target = InProcessWorldRuntime::new(config.clone());
    runtime_login_start(&mut target, LIVE_TARGET_ACCOUNT, LIVE_TARGET);
    {
        // The online owner's loaded state remains current while this saved image
        // represents an older level and a different, valid worn item.
        let mut store = config.account_store.lock().unwrap();
        let save = store.accounts.get_mut(LIVE_TARGET_ACCOUNT).unwrap().saves.get_mut(&LIVE_TARGET).unwrap();
        save.character.level = 17;
        save.equipment_items_json = vec![runtime_equipment("helmet", 4, LIVE_WEAPON_UID + 1)];
        save.stage5_systems_json = Some(json!({
            "appearance": {"hair": 4}, "relationship": {"partnerIdentity": null}
        }).to_string());
    }
    let current: RankingInspectOnlineProjection = target.active_ranking_inspect_projection()
        .expect("matching active target owner exports its live equipment");
    assert_eq!(current.identity.account_id, LIVE_TARGET_ACCOUNT);
    assert_eq!(current.info.level, 52);
    assert_eq!(current.info.hair, 11);
    assert_eq!(current.info.equipment[0].as_ref().unwrap().unique_id, LIVE_WEAPON_UID);
    assert!(current.info.equipment[2].is_none());
    let online = runtime_inspected(viewer.complete_ranking_inspect(
        prepare_runtime_inspect(&viewer), RankingInspectPresence::Online(Some(current.clone())),
    ));
    assert_eq!(online, current.info);
    let saved = runtime_inspected(viewer.complete_ranking_inspect(
        prepare_runtime_inspect(&viewer), RankingInspectPresence::Offline,
    ));
    assert_eq!((saved.level, saved.hair), (17, 4));
    assert!(saved.equipment[0].is_none());
    assert_eq!(saved.equipment[2].as_ref().unwrap().unique_id, LIVE_WEAPON_UID + 1);
    assert_ne!(online.equipment, saved.equipment);
}

#[test]
fn ranking_inspect_runtime_online_none_and_unknown_never_fall_back_to_valid_save() {
    let mut viewer = InProcessWorldRuntime::new(inspect_runtime_config());
    runtime_login_start(&mut viewer, "demo", 0);
    let saved = runtime_inspected(viewer.complete_ranking_inspect(
        prepare_runtime_inspect(&viewer), RankingInspectPresence::Offline,
    ));
    assert_eq!(saved.name, "LiveRankTarget");
    assert_eq!(saved.equipment[0].as_ref().unwrap().unique_id, LIVE_WEAPON_UID);
    for presence in [RankingInspectPresence::Online(None), RankingInspectPresence::Unknown] {
        assert!(viewer.complete_ranking_inspect(prepare_runtime_inspect(&viewer), presence).is_empty());
    }
}

#[test]
fn ranking_inspect_runtime_rejects_foreign_hero_and_malformed_online_projections() {
    let config = inspect_runtime_config();
    let mut viewer = InProcessWorldRuntime::new(config.clone());
    runtime_login_start(&mut viewer, "demo", 0);
    let mut target = InProcessWorldRuntime::new(config);
    runtime_login_start(&mut target, LIVE_TARGET_ACCOUNT, LIVE_TARGET);
    let current = target.active_ranking_inspect_projection().unwrap();
    let accepted = runtime_inspected(viewer.complete_ranking_inspect(
        prepare_runtime_inspect(&viewer), RankingInspectPresence::Online(Some(current.clone())),
    ));
    assert_eq!(accepted.equipment.len(), 14);
    for violation in ["account", "index", "name", "hero", "width13", "width15", "zeroCount", "wrongSlot"] {
        let mut projection = current.clone();
        match violation {
            "account" => projection.identity.account_id = "demo".into(),
            "index" => projection.identity.character_index = 0,
            "name" => projection.info.name = "RuntimeViewer".into(),
            "hero" => projection.info.is_hero = true,
            "width13" => { let _ = projection.info.equipment.pop(); },
            "width15" => projection.info.equipment.push(None),
            "zeroCount" => projection.info.equipment[0].as_mut().unwrap().count = 0,
            "wrongSlot" => {
                let mut wrong_slot = projection.info.equipment[0].as_ref().unwrap().clone();
                wrong_slot.unique_id = LIVE_WEAPON_UID + 1;
                projection.info.equipment[1] = Some(wrong_slot);
            },
            _ => unreachable!(),
        }
        assert!(viewer.complete_ranking_inspect(
            prepare_runtime_inspect(&viewer), RankingInspectPresence::Online(Some(projection)),
        ).is_empty(), "online violation must reject without offline fallback: {violation}");
    }
}

#[test]
fn ranking_inspect_runtime_single_use_ticket_rejects_requester_logout_relogin_aba() {
    let config = inspect_runtime_config();
    let mut viewer = InProcessWorldRuntime::new(config.clone());
    runtime_login_start(&mut viewer, "demo", 0);
    let identity_before = viewer.active_ranking_inspect_projection().unwrap().identity;
    let revision_before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision;
    let old_ticket = prepare_runtime_inspect(&viewer);
    let target_identity = old_ticket.target_identity().clone();
    let logout = runtime_packet(&mut viewer, ClientPacket::LogOut);
    assert!(logout.iter().any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    assert!(viewer.prepare_ranking_inspect(LIVE_TARGET as u32).is_none());
    assert!(viewer.active_ranking_inspect_projection().is_none());
    runtime_login_start(&mut viewer, "demo", 0);
    assert_eq!(viewer.active_ranking_inspect_projection().unwrap().identity, identity_before);
    assert!(config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision > revision_before);
    let fresh_ticket = prepare_runtime_inspect(&viewer);
    assert_eq!(fresh_ticket.target_identity(), &target_identity);
    // Requests are non-Clone and completion consumes them. This actual stale
    // owned ticket must fail even after the same account/index/name reappears.
    assert!(viewer.complete_ranking_inspect(old_ticket, RankingInspectPresence::Offline).is_empty());
    let fresh = runtime_inspected(viewer.complete_ranking_inspect(fresh_ticket, RankingInspectPresence::Offline));
    assert_eq!(fresh.name, "LiveRankTarget");
}

#[test]
fn ranking_inspect_runtime_online_partner_uses_exact_account_and_current_name() {
    let config = inspect_runtime_config();
    {
        let mut store = config.account_store.lock().unwrap();
        for (account_id, name) in [("partner", "OldPartner"), ("unrelated", "WrongPartner")] {
            store.accounts.insert(account_id.into(), AccountRecord::new(CharacterRecord {
                index: 31, name: name.into(), level: 30, class: MirClass::Wizard, gender: MirGender::Female,
            }));
        }
        store.accounts.get_mut("partner").unwrap().characters[0].name = "CurrentPartner".into();
        store.accounts.get_mut(LIVE_TARGET_ACCOUNT).unwrap().saves.get_mut(&LIVE_TARGET).unwrap()
            .stage5_systems_json = Some(json!({
                "appearance": {"hair": 11},
                "relationship": {"partnerIdentity": {"accountId": "partner", "characterIndex": 31},
                    "partnerName": "OldPartner", "authorityRevision": 1}
            }).to_string());
        store.accounts.get_mut("partner").unwrap().saves.get_mut(&31).unwrap()
            .stage5_systems_json = Some(json!({
                "appearance": {"hair": 0},
                "relationship": {"partnerIdentity": {"accountId": LIVE_TARGET_ACCOUNT, "characterIndex": LIVE_TARGET},
                    "partnerName": "LiveRankTarget", "authorityRevision": 1}
            }).to_string());
    }
    let mut viewer = InProcessWorldRuntime::new(config.clone());
    runtime_login_start(&mut viewer, "demo", 0);
    let mut target = InProcessWorldRuntime::new(config);
    runtime_login_start(&mut target, LIVE_TARGET_ACCOUNT, LIVE_TARGET);
    let mut current = target.active_ranking_inspect_projection().unwrap();
    assert_eq!(current.partner_identity,
        Some(Stage5FriendIdentity { account_id: "partner".into(), character_index: 31 }));
    assert_eq!(current.info.lover_name, "CurrentPartner");
    current.info.lover_name = "UntrustedCachedPartnerName".into();
    let info = runtime_inspected(viewer.complete_ranking_inspect(
        prepare_runtime_inspect(&viewer), RankingInspectPresence::Online(Some(current)),
    ));
    assert_eq!(info.name, "LiveRankTarget");
    assert_eq!(info.lover_name, "CurrentPartner");
}

#[test]
fn ranking_inspect_disconnect_retires_requester_and_old_ticket_even_after_exact_checkpoint_restore() {
    let mut viewer = InProcessWorldRuntime::new(inspect_runtime_config());
    runtime_login_start(&mut viewer, "demo", 0);
    let before = viewer.active_ranking_inspect_projection().unwrap().identity;
    let checkpoint = viewer.active_character_checkpoint().unwrap();
    let old_ticket = prepare_runtime_inspect(&viewer);
    let packets = runtime_packet(&mut viewer, ClientPacket::Disconnect);
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::Disconnect { .. })));
    // Retained personal state exists only for persistence recovery.
    assert!(viewer.prepare_ranking_inspect(LIVE_TARGET as u32).is_none());
    assert!(viewer.active_ranking_inspect_projection().is_none());
    viewer.restore_active_character_checkpoint(&checkpoint).unwrap();
    assert_eq!(viewer.active_ranking_inspect_projection().unwrap().identity, before);
    assert_eq!(viewer.active_character_checkpoint().unwrap().revision, checkpoint.revision);
    assert!(viewer.complete_ranking_inspect(old_ticket, RankingInspectPresence::Offline).is_empty());
    let fresh = prepare_runtime_inspect(&viewer);
    assert_eq!(runtime_inspected(viewer.complete_ranking_inspect(fresh, RankingInspectPresence::Offline)).name, "LiveRankTarget");
}

#[test]
fn ranking_inspect_failed_disconnect_save_retires_queries_while_recovery_state_is_retained() {
    let config = inspect_runtime_config();
    let mut viewer = InProcessWorldRuntime::new(config.clone());
    runtime_login_start(&mut viewer, "demo", 0);
    let old_ticket = prepare_runtime_inspect(&viewer);
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap().revision += 1;
    }
    assert!(viewer.execute(WorldCommand::ClientPacket(ClientPacket::Disconnect)).is_err());
    assert!(viewer.active_identity().is_some(), "failed saving preserves recovery identity");
    assert!(viewer.prepare_ranking_inspect(LIVE_TARGET as u32).is_none());
    assert!(viewer.active_ranking_inspect_projection().is_none());
    assert!(viewer.complete_ranking_inspect(old_ticket, RankingInspectPresence::Offline).is_empty());
}

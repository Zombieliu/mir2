use mir2_gateway::{
    GatewayConfig, GatewaySession, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRegistry,
    ZoneRuntimeFactory,
};
use mir2_protocol::{ClientPacket, MirClass, MirGender, PlayerInspectInfo, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, WorldCommand, ZoneRuntimeHandle,
};
use serde_json::json;
use std::sync::Arc;

fn fixture() -> GatewayConfig {
    let config = GatewayConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for (index, account_id, name) in [
            (0, "demo", "Scout"),
            (1, "peer", "Peer"),
            (2, "offline", "Offline"),
        ] {
            let mut account = AccountRecord::empty();
            account.characters.push(CharacterRecord {
                index,
                name: name.into(),
                level: 10 + index as u16,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            });
            let mut save = CharacterSaveRecord::new(account.characters[0].clone());
            save.map_file_name = config.map.file_name.clone();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            account.saves.insert(index, save);
            store.accounts.insert(account_id.into(), account);
        }
    }
    config
}

fn login(runtime: &mut ZoneRuntimeHandle, account: &str, index: i32) {
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: account.into(),
            password: "demo".into(),
        }))
        .unwrap();
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap();
    assert!(runtime.active_identity().is_some());
}

fn names(runtime: &mut ZoneRuntimeHandle, online_only: bool) -> Vec<String> {
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::GetRanking {
            rank_type: 0,
            rank_index: 0,
            online_only,
        }))
        .unwrap()
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::Rankings {
                listing_details, ..
            } => Some(
                listing_details
                    .into_iter()
                    .map(|entry| entry.name)
                    .collect(),
            ),
            _ => None,
        })
        .expect("ranking reply")
}

#[test]
fn ranking_tracks_other_zone_join_logout_and_disconnect_without_hiding_offline_rankings() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut owner = factory.create_runtime(config.clone(), &ZoneId::new("ranking-zone-a"));
    let mut peer = factory.create_runtime(config.clone(), &ZoneId::new("ranking-zone-b"));
    login(&mut owner, "demo", 0);
    assert_eq!(names(&mut owner, true), ["Scout"]);
    login(&mut peer, "peer", 1);
    assert_eq!(names(&mut owner, true), ["Peer", "Scout"]);
    assert_eq!(names(&mut owner, false), ["Offline", "Peer", "Scout"]);
    peer.execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    assert_eq!(names(&mut owner, true), ["Scout"]);
    peer.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
        character_index: 1,
    }))
    .unwrap();
    assert_eq!(names(&mut owner, true), ["Peer", "Scout"]);
    drop(peer);
    assert_eq!(names(&mut owner, true), ["Scout"]);
    assert_eq!(names(&mut owner, false), ["Offline", "Peer", "Scout"]);
}

const PEER_WEAPON_UID: u64 = 88_001;

fn inspect_equipment(slot: &str, item_type: u8, uid: u64) -> String {
    let item = mir2_game_data::crystal_item_manifest_ref()
        .items
        .iter()
        .find(|item| item.item_type == item_type)
        .expect("actual Crystal equipment template");
    json!({
        "key": format!("crystal-item-{}", item.item_index),
        "slot": slot,
        "name": item.name,
        "icon": item.image,
        "shape": u16::try_from(item.shape).ok(),
        "description": "Ranking inspect memory fixture",
        "quantity": 1,
        "durability_current": item.durability,
        "durability_max": item.durability,
        "attack": 0,
        "defence": 0,
        "user_item_unique_id": uid,
        "user_item_metadata": {"item_index": item.item_index}
    })
    .to_string()
}

fn inspect_fixture() -> GatewayConfig {
    let config = fixture();
    {
        let mut store = config.account_store.lock().unwrap();
        store.shared_guilds.clear();
        store.next_character_index = 3;
        for (account_id, index, level, hair) in
            [("demo", 0, 10, 3), ("peer", 1, 51, 11), ("offline", 2, 37, 9)]
        {
            let account = store.accounts.get_mut(account_id).unwrap();
            account.characters[0].level = level;
            let save = account.saves.get_mut(&index).unwrap();
            save.character.level = level;
            save.equipment_items_explicit_empty = true;
            save.equipment_items_json.clear();
            save.stage5_systems_json = Some(
                json!({
                    "appearance": {"hair": hair},
                    "relationship": {"partnerIdentity": null}
                })
                .to_string(),
            );
            if index == 1 {
                save.equipment_items_json
                    .push(inspect_equipment("weapon", 1, PEER_WEAPON_UID));
            } else if index == 2 {
                save.equipment_items_json = vec![
                    inspect_equipment("weapon", 1, 0),
                    inspect_equipment("braceletRight", 8, PEER_WEAPON_UID + 4),
                ];
            }
        }
    }
    config
}

fn replace_inspect_save(
    config: &GatewayConfig,
    account_id: &str,
    index: i32,
    level: u16,
    hair: u8,
    equipment: Vec<String>,
) {
    let mut store = config.account_store.lock().unwrap();
    let save = store
        .accounts
        .get_mut(account_id)
        .unwrap()
        .saves
        .get_mut(&index)
        .unwrap();
    save.character.level = level;
    save.stage5_systems_json = Some(
        json!({
            "appearance": {"hair": hair},
            "relationship": {"partnerIdentity": null}
        })
        .to_string(),
    );
    save.equipment_items_explicit_empty = true;
    save.equipment_items_json = equipment;
}

fn inspect_packet(character_index: u32, hero: bool) -> ClientPacket {
    ClientPacket::Inspect {
        object_id: character_index,
        ranking: true,
        hero,
    }
}

fn factory_inspect(runtime: &mut ZoneRuntimeHandle, character_index: u32) -> Vec<ServerPacket> {
    runtime
        .execute(WorldCommand::ClientPacket(inspect_packet(character_index, false)))
        .expect("memory factory ranking inspect")
}

fn inspected(packets: Vec<ServerPacket>) -> PlayerInspectInfo {
    let mut replies = packets.into_iter().filter_map(|packet| match packet {
        ServerPacket::PlayerInspect { info } => Some(info),
        _ => None,
    });
    let info = replies.next().expect("PlayerInspect reply");
    assert!(replies.next().is_none(), "exactly one inspect reply");
    info
}

fn assert_no_inspect(packets: &[ServerPacket]) {
    assert!(
        !packets.iter().any(|packet| matches!(packet, ServerPacket::PlayerInspect { .. })),
        "an unproved inspect target must not produce PlayerInspect: {packets:?}"
    );
}

fn gateway_login(session: &mut GatewaySession, start: bool) {
    let packets = session
        .try_handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .expect("memory gateway login");
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    if start {
        let packets = session
            .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
            .expect("memory gateway StartGame");
        assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
        assert!(session.active_identity().is_some());
    }
}

#[test]
fn ranking_inspect_factory_reads_current_target_from_another_local_zone() {
    let config = inspect_fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut owner = factory.create_runtime(config.clone(), &ZoneId::new("inspect-local-owner"));
    let mut target = factory.create_runtime(config.clone(), &ZoneId::new("inspect-local-target"));
    login(&mut owner, "demo", 0);
    login(&mut target, "peer", 1);
    replace_inspect_save(
        &config,
        "peer",
        1,
        17,
        4,
        vec![inspect_equipment("helmet", 4, PEER_WEAPON_UID + 1)],
    );
    // A normal target tick publishes the actual loaded owner state after the
    // saved image has become stale. The requester issues only one Inspect.
    target.execute(WorldCommand::Tick).expect("publish current target projection");
    let info = inspected(factory_inspect(&mut owner, 1));
    assert_eq!((info.name.as_str(), info.level, info.hair), ("Peer", 51, 11));
    assert_eq!(info.equipment.len(), 14);
    assert_eq!(info.equipment[0].as_ref().unwrap().unique_id, PEER_WEAPON_UID);
    assert!(info.equipment[2].is_none());
    assert!(!info.allow_observe);
    assert!(!info.is_hero);
}

#[test]
fn ranking_inspect_factory_offline_uses_explicit_other_character_save() {
    let config = inspect_fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut owner = factory.create_runtime(config, &ZoneId::new("inspect-offline-owner"));
    login(&mut owner, "demo", 0);
    let info = inspected(factory_inspect(&mut owner, 2));
    assert_eq!((info.name.as_str(), info.level, info.hair), ("Offline", 37, 9));
    assert_eq!((info.class, info.gender), (MirClass::Warrior, MirGender::Male));
    assert_eq!(info.equipment.len(), 14);
    assert_eq!(info.equipment[0].as_ref().unwrap().unique_id, 0);
    assert_eq!(info.equipment[6].as_ref().unwrap().unique_id, PEER_WEAPON_UID + 4);
    for slot in [1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13] {
        assert!(info.equipment[slot].is_none(), "actual empty slot {slot}");
    }
    assert!(info.guild_name.is_empty());
    assert!(info.guild_rank.is_empty());
    assert!(info.lover_name.is_empty());
    assert!(!info.allow_observe);
    assert!(!info.is_hero);
}

#[test]
fn ranking_inspect_factory_offline_unknown_raw_facts_rejects_defaults() {
    for raw in [
        r#"{"appearance":{},"relationship":{"partnerIdentity":null}}"#,
        r#"{"appearance":{"hair":9},"relationship":{}}"#,
        r#"{"appearance":{"hair":256},"relationship":{"partnerIdentity":null}}"#,
        r#"{"appearance":{"hair":9},"relationship":{"partnerIdentity":"unknown"}}"#,
    ] {
        let config = inspect_fixture();
        config.account_store.lock().unwrap().accounts.get_mut("offline").unwrap()
            .saves.get_mut(&2).unwrap().stage5_systems_json = Some(raw.into());
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let mut owner = factory.create_runtime(config, &ZoneId::new("inspect-unknown-owner"));
        login(&mut owner, "demo", 0);
        assert_no_inspect(&factory_inspect(&mut owner, 2));
    }
}

fn inspect_retired_target(departure: &str) {
    let config = inspect_fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut owner = factory.create_runtime(config.clone(), &ZoneId::new("inspect-retired-owner"));
    let mut target = factory.create_runtime(config.clone(), &ZoneId::new("inspect-retired-target"));
    login(&mut owner, "demo", 0);
    login(&mut target, "peer", 1);
    match departure {
        "logout" => {
            let packets = target.execute(WorldCommand::ClientPacket(ClientPacket::LogOut)).unwrap();
            assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
            assert!(target.active_identity().is_none());
        }
        "disconnect" => {
            let packets = target.execute(WorldCommand::ClientPacket(ClientPacket::Disconnect)).unwrap();
            assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::Disconnect { reason: 0 })));
        }
        "drop" => drop(target),
        _ => panic!("unknown retirement fixture"),
    }
    assert!(
        !names(&mut owner, true).iter().any(|name| name == "Peer"),
        "retired target must leave authoritative factory presence"
    );
    // This current offline image is intentionally different from the retired
    // owner's cached level, hair, and weapon. It is written after retirement.
    replace_inspect_save(
        &config,
        "peer",
        1,
        63,
        6,
        vec![inspect_equipment("helmet", 4, PEER_WEAPON_UID + 3)],
    );
    let info = inspected(factory_inspect(&mut owner, 1));
    assert_eq!((info.name.as_str(), info.level, info.hair), ("Peer", 63, 6));
    assert_eq!(info.equipment.len(), 14);
    assert!(info.equipment[0].is_none());
    assert_eq!(info.equipment[2].as_ref().unwrap().unique_id, PEER_WEAPON_UID + 3);
}

#[test]
fn ranking_inspect_factory_logout_discards_online_projection_before_query() {
    inspect_retired_target("logout");
}

#[test]
fn ranking_inspect_factory_disconnect_discards_online_projection_before_query() {
    inspect_retired_target("disconnect");
}

#[test]
fn ranking_inspect_factory_failed_disconnect_save_retires_projection_but_retains_recovery_presence() {
    let config = inspect_fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut owner = factory.create_runtime(config.clone(), &ZoneId::new("inspect-cas-owner"));
    let mut target = factory.create_runtime(config.clone(), &ZoneId::new("inspect-cas-target"));
    login(&mut owner, "demo", 0);
    login(&mut target, "peer", 1);
    target.execute(WorldCommand::Tick).expect("publish target live projection before Disconnect");
    let recovery_identity = target.active_identity().expect("active inspected target");
    let canonical_revision = {
        let mut store = config.account_store.lock().unwrap();
        let save = store.accounts.get_mut("peer").unwrap().saves.get_mut(&1).unwrap();
        save.revision = save.revision.checked_add(1).expect("fixture save revision");
        save.revision
    };

    let error = target.execute(WorldCommand::ClientPacket(ClientPacket::Disconnect))
        .err().expect("Disconnect full-save CAS must reject the stale target owner");
    assert!(error.contains("stale full character save rejected"), "actual persist CAS failure: {error}");
    assert_eq!(target.active_identity(), Some(recovery_identity));
    assert_eq!(config.account_store.lock().unwrap().accounts["peer"].saves[&1].revision, canonical_revision);
    assert!(
        names(&mut owner, true).iter().any(|name| name == "Peer"),
        "failed save retains target presence for recovery"
    );
    // The retained online presence has no current Inspect exporter after the
    // failed departure. Its old projection and valid saved image cannot reply.
    assert_no_inspect(&factory_inspect(&mut owner, 1));
}

#[test]
fn ranking_inspect_factory_drop_discards_online_projection_before_query() {
    inspect_retired_target("drop");
}

#[test]
fn ranking_inspect_factory_checkpoint_presence_without_live_projection_rejects_saved_fallback() {
    let config = inspect_fixture();
    let source = SharedInProcessZoneRuntimeFactory::new();
    let target_zone = ZoneId::new("inspect-checkpoint-target");
    let mut target = source.create_runtime(config.clone(), &target_zone);
    login(&mut target, "peer", 1);
    let checkpoint = source.zone_checkpoint_bytes(&target_zone).expect("memory Zone checkpoint");
    let restored = SharedInProcessZoneRuntimeFactory::new();
    assert_eq!(restored.install_checkpoint_bytes(&checkpoint).unwrap(), 1);
    let mut owner = restored.create_runtime(config.clone(), &ZoneId::new("inspect-checkpoint-owner"));
    login(&mut owner, "demo", 0);
    assert!(names(&mut owner, true).iter().any(|name| name == "Peer"), "checkpoint retains target presence");
    assert_no_inspect(&factory_inspect(&mut owner, 1));

    // A separate local factory proves the same authoritative saved target can
    // be inspected when offline. Restored presence without a current exporter
    // must therefore reject rather than treating the target as offline.
    let offline_factory = SharedInProcessZoneRuntimeFactory::new();
    let mut offline_owner = offline_factory.create_runtime(config, &ZoneId::new("inspect-checkpoint-control"));
    login(&mut offline_owner, "demo", 0);
    let saved = inspected(factory_inspect(&mut offline_owner, 1));
    assert_eq!((saved.name.as_str(), saved.level, saved.hair), ("Peer", 51, 11));
    assert_eq!(saved.equipment[0].as_ref().unwrap().unique_id, PEER_WEAPON_UID);
}

#[test]
fn ranking_inspect_gateway_production_entry_requires_auth_for_all_ranking_hero_flags() {
    for (ranking, hero) in [(false, false), (false, true), (true, false), (true, true)] {
        let config = inspect_fixture();
        let zone_id = ZoneId::new("inspect-production-auth");
        let registry = ZoneRegistry::new(zone_id, Arc::new(SharedInProcessZoneRuntimeFactory::new()));
        let mut session = GatewaySession::new_with_zone_registry(config, &registry);
        gateway_login(&mut session, true);
        let error = session.execute_production_player_command(
            false,
            WorldCommand::ClientPacket(ClientPacket::Inspect { object_id: 2, ranking, hero }),
        ).err().expect("Inspect requires an authenticated production caller");
        assert!(error.contains("authenticated"), "strict production auth rejection: {error}");
        assert!(session.active_identity().is_some());
    }
}

#[test]
fn ranking_inspect_gateway_claimed_auth_cannot_replace_login_and_start_game() {
    for phase in ["unauthenticated", "preStart", "loggedOut"] {
        for (ranking, hero) in [(false, false), (false, true), (true, false), (true, true)] {
            let config = inspect_fixture();
            let zone_id = ZoneId::new("inspect-production-local-owner");
            let registry = ZoneRegistry::new(zone_id, Arc::new(SharedInProcessZoneRuntimeFactory::new()));
            let mut session = GatewaySession::new_with_zone_registry(config, &registry);
            if phase != "unauthenticated" {
                gateway_login(&mut session, phase == "loggedOut");
            }
            if phase == "loggedOut" {
                session.try_handle_packet(ClientPacket::LogOut).unwrap();
            }
            assert!(session.active_identity().is_none());
            let execution = session.execute_production_player_command(
                true,
                WorldCommand::ClientPacket(ClientPacket::Inspect { object_id: 2, ranking, hero }),
            ).expect("memory query with no active Game owner");
            assert_no_inspect(&execution.packets);
        }
    }
}

#[test]
fn ranking_inspect_gateway_ranking_flag_precedes_hero_at_actual_query_entry() {
    let config = inspect_fixture();
    let zone_id = ZoneId::new("inspect-ranking-before-hero");
    let registry = ZoneRegistry::new(zone_id, Arc::new(SharedInProcessZoneRuntimeFactory::new()));
    let mut session = GatewaySession::new_with_zone_registry(config, &registry);
    gateway_login(&mut session, true);
    let execution = session.execute_production_player_command(
        true,
        WorldCommand::ClientPacket(inspect_packet(2, true)),
    ).expect("authenticated ranking and Hero query");
    let info = inspected(execution.packets);
    assert_eq!((info.name.as_str(), info.level, info.hair), ("Offline", 37, 9));
    assert_eq!(info.equipment.len(), 14);
    assert_eq!(info.equipment[0].as_ref().unwrap().unique_id, 0);
    assert!(!info.is_hero, "ranking takes priority over Hero when both flags are set");
    assert!(!info.allow_observe);
}

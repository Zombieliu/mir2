//! Prepared isolated D604/Ashes fixtures using the actual imported @MAIN.
//! Normal protocol and Gateway fallback wrappers; no public account or source
//! page modification. These do not certify natural quest or native UI entry.
use super::npc_script::crystal_npc_flag_value;
use crate::{
    AccountStoreTransactionFault, SimulationConfig, SimulationSession, VisibleNpcRecord,
    WorldEntitySnapshot,
};
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};

const NPC: u32 = 1140;
const FLAG: u32 = 524;

fn fixture() -> (SimulationConfig, SimulationSession, WorldEntitySnapshot) {
    fixture_with_store(None)
}

fn fixture_with_store(
    file: Option<&std::path::Path>,
) -> (SimulationConfig, SimulationSession, WorldEntitySnapshot) {
    let info = mir2_game_data::crystal_npc_info_manifest_ref()
        .npcs
        .iter()
        .find(|info| info.loaded_object_id == Some(NPC))
        .unwrap();
    assert_eq!(info.script_key, "MongchonProvince/Ashes");
    assert_eq!(info.map_file_name.as_deref(), Some("D604"));
    let script = mir2_game_data::crystal_npc_script_by_key(&info.script_key).unwrap();
    assert!(script
        .sections
        .iter()
        .any(|section| section.label.eq_ignore_ascii_case("@MAIN")
            && section.lines.iter().any(|line| line == "SET [524] 1")));
    let mut config = SimulationConfig::default();
    config.map.file_name = "D604".into();
    config.map.title = "Lost Soul Cave".into();
    config.spawn = Point {
        x: info.location.x + 1,
        y: info.location.y,
    };
    config.visible_npcs = vec![VisibleNpcRecord {
        object_id: NPC,
        name: info.name.clone(),
        image: info.image,
        colour_argb: -1,
        position: info.location.clone(),
        direction: MirDirection::Down,
        quest_ids: vec![],
        script_key: Some(info.script_key.clone()),
    }];
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        save.map_file_name = config.map.file_name.clone();
        save.map_title = config.map.title.clone();
        save.position = config.spawn.clone();
        save.quest_states_json.clear();
        save.npc_flag_states_json.clear();
    }
    if let Some(file) = file {
        let initial = config.account_store.lock().unwrap().clone();
        config = config.with_account_store_path(file);
        *config.account_store.lock().unwrap() = initial;
        config.save_account_store().unwrap();
    }
    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserInformation { .. })));
    assert_eq!(session.current_map_file_name().as_deref(), Some("D604"));
    let npc = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.object_id == NPC)
        .unwrap();
    session.save_active_character().unwrap();
    assert!(!crystal_npc_flag_value(session.app.world(), FLAG));
    (config, session, npc)
}

fn open(session: &mut SimulationSession, npc: &WorldEntitySnapshot, path: u8) -> Vec<ServerPacket> {
    match path {
        0 => session.interact(NPC),
        1 => session.interact_shared_npc_snapshot(npc),
        2 => session.call_npc(NPC, "@MAIN"),
        3 => session.call_shared_npc_snapshot(npc, "@MAIN"),
        4 => session.handle_packet(ClientPacket::CallNpc {
            object_id: NPC,
            key: "@MAIN".into(),
        }),
        _ => unreachable!(),
    }
}

fn succeeded(packets: &[ServerPacket]) -> bool {
    packets.iter().any(|packet| {
        matches!(packet, ServerPacket::Chat { message, .. }
        if message.contains("Return to the LostSoul"))
    })
}

fn prepare_live_oven(session: &mut SimulationSession) {
    use super::resources::{InventoryResource, Stage5SystemsResource};
    let held = {
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        let index = inventory
            .inventory_items
            .iter()
            .position(|item| item.key == "dagger")
            .unwrap();
        inventory.inventory_items.remove(index)
    };
    let mut wire = super::items::try_user_item_from_item_state(&held).unwrap();
    wire.refine_added = 1;
    let held = super::items::try_item_state_from_user_item(held, &wire).unwrap();
    let mut oven = crate::Stage5RefineState {
        oven_item_state_json: Some(serde_json::to_string(&held).unwrap()),
        current_item: Some(held.key.clone()),
        remaining_ms: 120_000,
        refining: true,
        ..Default::default()
    };
    super::refine_oven::restore_timer(&mut oven).unwrap();
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .refine = oven;
    session.save_active_character().unwrap();
}

#[test]
fn original_main_flag_is_durable_before_ack_on_all_npc_entry_wrappers() {
    for path in 0..5 {
        let (config, mut session, npc) = fixture();
        let before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
        let packets = open(&mut session, &npc, path);
        assert!(succeeded(&packets), "Source @MAIN path {path}: {packets:?}");
        assert!(
            crystal_npc_flag_value(session.app.world(), FLAG),
            "live path {path}"
        );
        let durable = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
        assert_eq!(durable.revision, before.revision + 1, "durable path {path}");
        assert!(
            durable
                .npc_flag_states_json
                .iter()
                .any(
                    |entry| serde_json::from_str::<serde_json::Value>(entry).unwrap()
                        == serde_json::json!({"index":FLAG,"value":true})
                ),
            "Source flag missing from durable path {path}"
        );
    }
}

#[test]
fn original_main_known_pre_persist_failure_rolls_back_flag_and_suppresses_source_ack() {
    for path in 0..5 {
        let (config, mut session, npc) = fixture();
        let before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        let packets = open(&mut session, &npc, path);
        assert!(
            !succeeded(&packets),
            "Uncommitted Source success ACK path {path}"
        );
        assert!(
            !crystal_npc_flag_value(session.app.world(), FLAG),
            "Uncommitted flag path {path}"
        );
        let durable = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
        assert_eq!(
            serde_json::to_value(durable).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert!(
            succeeded(&open(&mut session, &npc, path)),
            "normal retry path {path}"
        );
        assert!(crystal_npc_flag_value(session.app.world(), FLAG));
    }
}

#[test]
fn repeated_unchanged_main_does_not_consume_account_store_write_fault() {
    let (config, mut session, npc) = fixture();
    assert!(succeeded(&open(&mut session, &npc, 4)));
    let before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision;
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(succeeded(&open(&mut session, &npc, 4)));
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision,
        before
    );
    assert!(
        session.save_active_character().is_err(),
        "view must leave the next real write fault untouched"
    );
}

#[test]
fn unchanged_main_with_live_oven_does_not_save_elapsed_only_timer_projection() {
    use super::resources::Stage5SystemsResource;
    let (config, mut session, npc) = fixture();
    assert!(succeeded(&open(&mut session, &npc, 4)));
    prepare_live_oven(&mut session);
    let raw_before = session
        .app
        .world()
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .refine
        .clone();
    let durable_before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    let before = session.begin_npc_dialog_source_command(false).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(4));
    let packets = open(&mut session, &npc, 4);
    let result = session.finish_guild_experience_command(before, packets);
    assert!(
        result.is_ok(),
        "Unchanged Source @MAIN attempted a real storage write: {result:?}"
    );
    assert!(succeeded(&result.unwrap()));
    assert_eq!(
        session
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .refine,
        raw_before
    );
    let durable_after = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(
        serde_json::to_value(durable_after).unwrap(),
        serde_json::to_value(durable_before).unwrap()
    );
    assert!(
        session.save_active_character().is_err(),
        "unchanged view must leave the next real write fault untouched"
    );
}

#[test]
fn actual_main_flag_mutation_with_live_oven_still_saves_before_ack() {
    use super::resources::Stage5SystemsResource;
    let (config, mut session, npc) = fixture();
    prepare_live_oven(&mut session);
    let raw_before = session
        .app
        .world()
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .refine
        .clone();
    let revision = config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision;
    assert!(succeeded(&open(&mut session, &npc, 4)));
    assert!(crystal_npc_flag_value(session.app.world(), FLAG));
    assert_eq!(
        session
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .refine,
        raw_before
    );
    let stored = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(stored.revision, revision + 1);
    let systems: crate::Stage5SystemsState =
        serde_json::from_str(stored.stage5_systems_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        systems.refine.oven_item_state_json,
        raw_before.oven_item_state_json
    );
    assert_eq!(systems.refine.clock_epoch, raw_before.clock_epoch);
    assert_eq!(
        systems.refine.collect_deadline_ms,
        raw_before.collect_deadline_ms
    );
    assert!(systems.refine.remaining_ms <= raw_before.remaining_ms);
    assert_eq!(systems.refine.item_states, raw_before.item_states);
}

#[test]
fn actual_main_oven_failure_restores_exact_source_clock_and_retry_commits_once() {
    use super::resources::Stage5SystemsResource;
    let (config, mut session, npc) = fixture();
    prepare_live_oven(&mut session);
    let raw_before = session
        .app
        .world()
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .refine
        .clone();
    let durable_before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(!succeeded(&open(&mut session, &npc, 4)));
    assert!(!crystal_npc_flag_value(session.app.world(), FLAG));
    assert_eq!(
        session
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .refine,
        raw_before
    );
    let durable_after = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(
        serde_json::to_value(durable_after).unwrap(),
        serde_json::to_value(&durable_before).unwrap()
    );
    assert!(succeeded(&open(&mut session, &npc, 4)));
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision,
        durable_before.revision + 1
    );
    assert_eq!(
        session
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .refine,
        raw_before
    );
}

fn isolated_file_path(case: &str, path: u8) -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!(
            "mir2-npc-source-{case}-{path}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("accounts.json")
}

fn writer_marker(file: &std::path::Path) -> std::path::PathBuf {
    let mut name = file.as_os_str().to_os_string();
    name.push(".writer.lock");
    name.into()
}

#[test]
fn original_main_known_file_rename_failure_retains_image_and_retry_saves_once() {
    for path in 0..5 {
        let file = isolated_file_path("known", path);
        let (config, mut session, npc) = fixture_with_store(Some(&file));
        let before = std::fs::read(&file).unwrap();
        let revision = config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision;
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
        assert!(!succeeded(&open(&mut session, &npc, path)));
        assert!(!crystal_npc_flag_value(session.app.world(), FLAG));
        assert_eq!(std::fs::read(&file).unwrap(), before);
        assert_eq!(std::fs::metadata(writer_marker(&file)).unwrap().len(), 0);
        config.ensure_account_store_writable().unwrap();
        assert!(succeeded(&open(&mut session, &npc, path)));
        let stored: crate::AccountStore = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(stored.accounts["demo"].saves[&0].revision, revision + 1);
        assert_eq!(config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision, revision + 1);
        assert!(crystal_npc_flag_value(session.app.world(), FLAG));
        assert_eq!(std::fs::metadata(writer_marker(&file)).unwrap().len(), 0);
        drop(session);
        drop(config);
        assert!(std::fs::read(writer_marker(&file)).unwrap().is_empty());
        // Isolated images are intentionally retained; no human save is removed.
    }
}

#[test]
fn original_main_unknown_file_publication_suppresses_ack_and_survives_restart_frozen() {
    for path in 0..5 {
        let file = isolated_file_path("unknown", path);
        let (config, mut session, npc) = fixture_with_store(Some(&file));
        let before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
        config.inject_account_store_transaction_fault(
            AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
        );
        assert!(!succeeded(&open(&mut session, &npc, path)));
        assert!(crystal_npc_flag_value(session.app.world(), FLAG), "unknown cannot guess an old live image");
        assert_eq!(
            serde_json::to_value(&config.account_store.lock().unwrap().accounts["demo"].saves[&0]).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
        assert!(config.ensure_account_store_writable().is_err());
        let visible_bytes = std::fs::read(&file).unwrap();
        let visible: crate::AccountStore = serde_json::from_slice(&visible_bytes).unwrap();
        let visible_save = &visible.accounts["demo"].saves[&0];
        assert_eq!(visible_save.revision, before.revision + 1);
        assert!(visible_save.npc_flag_states_json.iter().any(|entry|
            serde_json::from_str::<serde_json::Value>(entry).unwrap()
                == serde_json::json!({"index":FLAG,"value":true})));
        // Windows enforces the authority's OS lock even for another read-only
        // handle. Observe length now, and exact persistent bytes only after
        // every live holder is dropped; never bypass the authority lock.
        let marker_length = std::fs::metadata(writer_marker(&file)).unwrap().len();
        assert!(marker_length > b"PENDING PUBLICATION\n".len() as u64);
        assert!(!succeeded(&open(&mut session, &npc, path)));
        assert!(session.save_active_character().is_err());
        let clone = config.clone();
        assert!(clone.save_account_store().is_err());
        assert_eq!(std::fs::read(&file).unwrap(), visible_bytes);
        assert_eq!(std::fs::metadata(writer_marker(&file)).unwrap().len(), marker_length);
        drop(clone);
        drop(session);
        drop(config);
        let marker_before = std::fs::read(writer_marker(&file)).unwrap();
        assert!(marker_before.starts_with(b"PENDING PUBLICATION\n"));
        assert!(String::from_utf8_lossy(&marker_before).contains("FROZEN:"));
        assert_eq!(marker_before.len() as u64, marker_length);
        let restarted = SimulationConfig::default().with_account_store_path(&file);
        assert!(restarted.ensure_account_store_writable().is_err());
        assert!(restarted.save_account_store().is_err());
        assert_eq!(std::fs::read(&file).unwrap(), visible_bytes);
        drop(restarted);
        assert_eq!(std::fs::read(writer_marker(&file)).unwrap(), marker_before);
        // Visible bytes are not confirmed durability. Do not clear the fence,
        // acknowledge success or claim operator reconciliation has happened.
    }
}

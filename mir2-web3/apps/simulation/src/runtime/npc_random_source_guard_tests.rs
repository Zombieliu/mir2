//! Prepared isolated actors use genuine imported MAIN/displayed Tmove pages.
//! This is personal Source rollback evidence, not shared Zone/outbox acceptance.
use crate::{
    AccountStoreTransactionFault, CharacterSaveRecord, SimulationConfig, SimulationSession,
    VisibleNpcRecord, WorldEntitySnapshot,
};
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};

const VILLAGE_NPC: u32 = 1250;

fn fixture(
    script_key: &str,
    file: Option<&std::path::Path>,
) -> (SimulationConfig, SimulationSession, WorldEntitySnapshot) {
    let info = mir2_game_data::crystal_npc_info_manifest_ref()
        .npcs
        .iter()
        .find(|info| info.script_key == script_key && info.loaded_object_id.is_some())
        .unwrap();
    let id = info.loaded_object_id.unwrap();
    if script_key == "PrajnaIsland/VillageChief" {
        assert_eq!(id, VILLAGE_NPC);
        assert_eq!(info.map_file_name.as_deref(), Some("B354"));
        let original = mir2_game_data::crystal_npc_script_by_key(script_key).unwrap();
        let page = original
            .sections
            .iter()
            .find(|section| section.label.eq_ignore_ascii_case("@Tmove"))
            .unwrap();
        assert!(page.lines.iter().any(|line| line == "MOVE 5 125 324"));
        assert!(page.lines.iter().any(|line| line == "MOVE 5 126 325"));
    }
    let mut config = SimulationConfig::default();
    config.map.file_name = info.map_file_name.clone().unwrap();
    config.map.title = config.map.file_name.clone();
    config.map.map_index = info.map_index;
    config.spawn = Point {
        x: info.location.x + 1,
        y: info.location.y,
    };
    config.visible_npcs = vec![VisibleNpcRecord {
        object_id: id,
        name: info.name.clone(),
        image: info.image,
        colour_argb: -1,
        position: info.location.clone(),
        direction: MirDirection::Down,
        quest_ids: vec![],
        script_key: Some(script_key.to_owned()),
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
        let image = config.account_store.lock().unwrap().clone();
        config = config.with_account_store_path(file);
        *config.account_store.lock().unwrap() = image;
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
    let npc = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.object_id == id)
        .unwrap();
    session.save_active_character().unwrap();
    (config, session, npc)
}

fn stored(config: &SimulationConfig) -> CharacterSaveRecord {
    config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone()
}

fn open_main(session: &mut SimulationSession, npc: &WorldEntitySnapshot, path: u8) {
    let _ = match path {
        0 => session.interact(npc.object_id),
        1 => session.interact_shared_npc_snapshot(npc),
        2 => session
            .try_handle_packet(ClientPacket::CallNpc {
                object_id: npc.object_id,
                key: "@MAIN".into(),
            })
            .unwrap(),
        _ => unreachable!(),
    };
    assert!(
        super::npc_input::has_menu_target(session.app.world(), "@Tmove"),
        "genuine displayed Tmove missing on ordinary path {path}"
    );
}

fn choose(
    session: &mut SimulationSession,
    npc: &WorldEntitySnapshot,
    path: u8,
) -> Vec<ServerPacket> {
    match path {
        0 => session.call_npc(npc.object_id, "@Tmove"),
        1 => session.call_shared_npc_snapshot(npc, "@Tmove"),
        2 => session.handle_packet(ClientPacket::CallNpc {
            object_id: npc.object_id,
            key: "@Tmove".into(),
        }),
        _ => unreachable!(),
    }
}

fn moved(packets: &[ServerPacket]) -> bool {
    packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MapInformation { .. }))
}

fn isolated_file(case: &str, path: u8) -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!(
            "mir2-npc-rng-{case}-{path}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("accounts.json")
}

#[test]
fn genuine_random_move_commits_both_branches_before_ack_on_three_selection_paths() {
    for path in 0..3 {
        for (seed, expected_x, expected_y) in [(0, 125, 324), (115, 126, 325)] {
            let (config, mut session, npc) = fixture("PrajnaIsland/VillageChief", None);
            session.apply_shared_npc_random_seed(seed);
            open_main(&mut session, &npc, path);
            let before = stored(&config);
            assert!(moved(&choose(&mut session, &npc, path)));
            assert_ne!(session.shared_npc_random_seed(), seed);
            let after = stored(&config);
            assert_eq!(after.revision, before.revision + 1);
            assert_eq!(after.map_file_name, "5");
            assert_eq!(
                after.position,
                Point {
                    x: expected_x,
                    y: expected_y
                }
            );
        }
    }
}

#[test]
fn genuine_random_move_known_memory_failure_restores_seed_position_and_retry_branch() {
    for path in 0..3 {
        for (seed, expected_x, expected_y) in [(0, 125, 324), (115, 126, 325)] {
            let (config, mut session, npc) = fixture("PrajnaIsland/VillageChief", None);
            session.apply_shared_npc_random_seed(seed);
            open_main(&mut session, &npc, path);
            let before = stored(&config);
            config.inject_account_store_transaction_fault(
                AccountStoreTransactionFault::BeforePersist,
            );
            assert!(!moved(&choose(&mut session, &npc, path)));
            assert_eq!(
                session.shared_npc_random_seed(),
                seed,
                "failed draw leaked on path {path}"
            );
            assert_eq!(session.current_map_file_name().as_deref(), Some("B354"));
            assert_eq!(
                serde_json::to_value(stored(&config)).unwrap(),
                serde_json::to_value(&before).unwrap()
            );
            assert!(!super::npc_input::has_menu_target(
                session.app.world(),
                "@Tmove"
            ));
            open_main(&mut session, &npc, path);
            assert!(moved(&choose(&mut session, &npc, path)));
            let after = stored(&config);
            assert_eq!(after.revision, before.revision + 1);
            assert_eq!(
                after.position,
                Point {
                    x: expected_x,
                    y: expected_y
                }
            );
        }
    }
}

#[test]
fn genuine_random_move_known_file_rename_failure_retains_seed_and_file_bytes() {
    for path in 0..3 {
        let file = isolated_file("known", path);
        let (config, mut session, npc) = fixture("PrajnaIsland/VillageChief", Some(&file));
        session.apply_shared_npc_random_seed(115);
        open_main(&mut session, &npc, path);
        let before = std::fs::read(&file).unwrap();
        let revision = stored(&config).revision;
        config
            .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
        assert!(!moved(&choose(&mut session, &npc, path)));
        assert_eq!(session.shared_npc_random_seed(), 115);
        assert_eq!(std::fs::read(&file).unwrap(), before);
        config.ensure_account_store_writable().unwrap();
        open_main(&mut session, &npc, path);
        assert!(moved(&choose(&mut session, &npc, path)));
        let disk: crate::AccountStore =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(disk.accounts["demo"].saves[&0].revision, revision + 1);
        assert_eq!(
            disk.accounts["demo"].saves[&0].position,
            Point { x: 126, y: 325 }
        );
        // Retain isolated evidence; never delete a human save or bypass its lock.
    }
}

#[test]
fn genuine_random_move_unknown_publication_keeps_advanced_seed_and_persistent_fence() {
    let file = isolated_file("unknown", 2);
    let (config, mut session, npc) = fixture("PrajnaIsland/VillageChief", Some(&file));
    session.apply_shared_npc_random_seed(0);
    open_main(&mut session, &npc, 2);
    let before = stored(&config);
    config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(!moved(&choose(&mut session, &npc, 2)));
    assert_ne!(
        session.shared_npc_random_seed(),
        0,
        "unknown publication must not guess old state"
    );
    let seed = session.shared_npc_random_seed();
    assert!(config.ensure_account_store_writable().is_err());
    let visible_bytes = std::fs::read(&file).unwrap();
    let visible: crate::AccountStore = serde_json::from_slice(&visible_bytes).unwrap();
    assert_eq!(
        visible.accounts["demo"].saves[&0].revision,
        before.revision + 1
    );
    assert_eq!(visible.accounts["demo"].saves[&0].map_file_name, "5");
    assert!(session.save_active_character().is_err());
    assert_eq!(session.shared_npc_random_seed(), seed);
    drop(session);
    drop(config);
    let restarted = SimulationConfig::default().with_account_store_path(&file);
    assert!(restarted.ensure_account_store_writable().is_err());
    assert!(restarted.save_account_store().is_err());
    assert_eq!(std::fs::read(&file).unwrap(), visible_bytes);
}

#[test]
fn genuine_lottery_random_view_keeps_transient_variable_without_forcing_a_save() {
    let (config, mut session, npc) = fixture("BichonProvince/BichonWall/Lottery", None);
    assert_eq!(npc.object_id, 23, "NPC index34 is not loaded object34");
    session.apply_shared_npc_random_seed(0);
    let before = stored(&config);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    let _ = session
        .try_handle_packet(ClientPacket::CallNpc {
            object_id: npc.object_id,
            key: "@MAIN".into(),
        })
        .unwrap();
    assert_ne!(session.shared_npc_random_seed(), 0);
    assert_eq!(
        serde_json::to_value(stored(&config)).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert!(
        session.save_active_character().is_err(),
        "pure view consumed the next real write fault"
    );
}

#[test]
fn prepared_newer_source_cas_rejection_preserves_newer_character_and_restores_rng() {
    let (config, mut session, npc) = fixture("PrajnaIsland/VillageChief", None);
    session.apply_shared_npc_random_seed(115);
    open_main(&mut session, &npc, 2);
    // An isolated newer authoritative image, not a public duplicate-session
    // admission. The stale actor must not replace the newer personal source.
    let mut newer = stored(&config);
    newer.revision += 1;
    newer.gold += 50;
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .insert(0, newer.clone());
    assert!(!moved(&choose(&mut session, &npc, 2)));
    assert_eq!(session.shared_npc_random_seed(), 115);
    assert_eq!(
        serde_json::to_value(stored(&config)).unwrap(),
        serde_json::to_value(&newer).unwrap()
    );
    assert_eq!(
        session.active_character_checkpoint().unwrap().gold,
        newer.gold
    );
}

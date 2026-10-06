//! Opt-in real loopback WebSocket cohort, with declared position preparation.
//! No production entrypoint, QA endpoint, player installation or live realm.
use crate::routing::{self, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRegistry};
use crate::{tcp::chat_broadcast::ChatBroadcastHub, GatewayConfig};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{InProcessWorldRuntime, WorldCommand, WorldRuntime, ZoneKey, ZoneRuntime};
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::{io::AsyncWriteExt, net::TcpListener};

const MAP_SHA: &str = "5e283984b31ee993c8d1157402f50d7e9d4984d8be92db49388c1e024e257ca2";
const RAW_MAP_SHA: &str = "ef8d4d9499d64bc16161abebc2548a3c9a98e99a7f634205ba45a3532db44de3";

struct Environment(Vec<(OsString, OsString)>);
impl Environment {
    fn isolate(root: &Path, frozen: &Path) -> Self {
        let inherited: Vec<_> = std::env::vars_os()
            .filter(|(key, _)| {
                let key = key.to_string_lossy().to_ascii_uppercase();
                key.starts_with("MIR2_") || key.starts_with("CRYSTAL_")
            })
            .collect();
        for (key, _) in &inherited {
            std::env::remove_var(key);
        }
        std::env::set_var("MIR2_RUNTIME_ENV", "test");
        std::env::set_var(
            "MIR2_IDENTITY_SESSION_SECRET",
            "cold-fixture-owned-session-key-32bytes",
        );
        std::env::set_var(
            "MIR2_IDENTITY_RECOVERY_PEPPER",
            "cold-fixture-owned-recovery-key-32bytes",
        );
        std::env::set_var("MIR2_ACCOUNT_STORE_BACKEND", "file");
        std::env::set_var("MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS", "0");
        std::env::set_var("MIR2_AI_LIVE_DATA_DIR", root.join("ai-owned"));
        std::env::set_var("MIR2_SAVE_RECOVERY_DIR", root.join("save-recovery-owned"));
        std::env::set_var("MIR2_CRYSTAL_MAP_PACK", frozen.join("maps"));
        // The ordinary collision loader prefers a client Map directory. Give
        // it an owned exact copy of the pinned map, ahead of developer installs.
        std::env::set_var("CRYSTAL_CLIENT_ROOT", root.join("source-client"));
        Self(inherited)
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        for (key, _) in std::env::vars_os()
            .filter(|(key, _)| {
                let key = key.to_string_lossy().to_ascii_uppercase();
                key.starts_with("MIR2_") || key.starts_with("CRYSTAL_")
            })
            .collect::<Vec<_>>()
        {
            std::env::remove_var(key);
        }
        for (key, value) in &self.0 {
            std::env::set_var(key, value);
        }
    }
}

fn original_map(root: &Path, frozen: &Path) {
    let compressed = std::fs::read(frozen.join("maps/d021.map.gz")).unwrap();
    let compressed_hash: String = Sha256::digest(&compressed)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(compressed_hash, MAP_SHA, "pin the original compressed map");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(compressed.as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        hash, RAW_MAP_SHA,
        "pin the original decompressed collision bytes"
    );
    let directory = root.join("source-client/Map");
    std::fs::create_dir_all(&directory).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("D021.map"))
        .unwrap();
    std::io::Write::write_all(&mut file, &bytes).unwrap();
}

fn command(runtime: &mut InProcessWorldRuntime, packet: ClientPacket) -> Vec<ServerPacket> {
    runtime.execute(WorldCommand::ClientPacket(packet)).unwrap()
}

fn prepared_original_world(
    class: MirClass,
    directory: &Path,
) -> (GatewayConfig, ZoneRuntime, Value, String) {
    let account = format!("dense_owned_{class:?}").to_ascii_lowercase();
    let name = format!("Dense{class:?}");
    let password = "owned-fixture-password-only".to_string();
    let mut recovery_key = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut recovery_key);
    let config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile()
        .with_account_store_path(directory.join("accounts-private.json"))
        .with_save_recovery_dir(directory.join("recovery-private"))
        .with_save_recovery_mac_key(recovery_key)
        .unwrap();
    config.account_store.lock().unwrap().accounts.clear();
    let mut personal = InProcessWorldRuntime::new(config.clone());
    assert!(command(
        &mut personal,
        ClientPacket::NewAccount {
            account_id: account.clone(),
            password: password.clone(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        }
    )
    .iter()
    .any(|p| matches!(p, ServerPacket::NewAccount { result: 8 })));
    assert!(command(
        &mut personal,
        ClientPacket::Login {
            account_id: account.clone(),
            password: password.clone()
        }
    )
    .iter()
    .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let index = command(
        &mut personal,
        ClientPacket::NewCharacter {
            name: name.clone(),
            class,
            gender: MirGender::Male,
        },
    )
    .into_iter()
    .find_map(|p| {
        if let ServerPacket::NewCharacterSuccess { char_info } = p {
            Some(char_info.index)
        } else {
            None
        }
    })
    .unwrap();
    command(
        &mut personal,
        ClientPacket::StartGame {
            character_index: index,
        },
    );
    assert!(command(&mut personal, ClientPacket::LogOut)
        .iter()
        .any(|p| matches!(p, ServerPacket::LogOutSuccess { .. })));
    drop(personal);
    let (hp, mp) = match class {
        MirClass::Warrior => (419, 116),
        MirClass::Wizard => (128, 541),
        MirClass::Taoist => (239, 260),
        _ => unreachable!(),
    };
    {
        let mut store = config.account_store.lock().unwrap();
        let record = store.accounts.get_mut(&account).unwrap();
        assert_eq!(record.gm_level, 0);
        record
            .characters
            .iter_mut()
            .find(|c| c.index == index)
            .unwrap()
            .level = 30;
        let save = record.saves.get_mut(&index).unwrap();
        assert!(
            save.skill_states_json.is_empty() && save.buff_states_json.is_empty() && save.hp > 0
        );
        save.character.level = 30;
        save.hp = hp;
        save.max_hp = hp;
        save.mp = mp;
        save.max_mp = mp;
        save.experience = 0;
        save.max_experience = config.experience_required_for_level(30);
        save.gold = 20_000;
        save.map_file_name = "D021".into();
        save.map_title = "WoomaTempleEntrance".into();
        save.position = Point { x: 59, y: 56 };
        save.direction = MirDirection::Down;
    }
    let mut source = InProcessWorldRuntime::new(config.clone());
    command(
        &mut source,
        ClientPacket::Login {
            account_id: account.clone(),
            password: password.clone(),
        },
    );
    command(
        &mut source,
        ClientPacket::StartGame {
            character_index: index,
        },
    );
    assert_eq!(
        source
            .active_zone_join_snapshot("cold-source-bootstrap")
            .unwrap()
            .level,
        30
    );
    let initial_active = source
        .current_map_shared_entity_snapshots()
        .iter()
        .filter(|entity| entity.kind == mir2_simulation::WorldEntityKind::Monster)
        .count();
    let activated = source.materialize_cold_source_pool_for_test().unwrap();
    let entities = source.current_map_shared_entity_snapshots();
    let spawns: Vec<_> = entities
        .iter()
        .filter_map(|e| source.zone_monster_spawn_snapshot(e.object_id))
        .collect();
    assert_eq!(spawns.len(), 151, "retain every original D021 actor");
    assert_eq!(
        initial_active + activated,
        spawns.len(),
        "only original dormant slots added"
    );
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for spawn in &spawns {
        *counts.entry(spawn.name.clone()).or_default() += 1;
    }
    assert_eq!(counts.get("CaveBat"), Some(&120));
    assert_eq!(counts.get("CaveMaggot"), Some(&30));
    assert_eq!(counts.get("CaveBat0"), Some(&1));
    let now = routing::shared_gateway_now_ms();
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("D021"));
    for spawn in &spawns {
        assert!(zone.spawn_world_event_monster(spawn, now).0);
    }
    command(&mut source, ClientPacket::LogOut);
    let mut preparation = zone.prepare_crowded_cave_bat_fixture_for_test().unwrap();
    preparation["sourceCounts"] = json!(counts);
    preparation["sourcePoolActivation"] = json!({ "initiallyActive": initial_active,
        "originalDormantSlotsActivated": activated, "totalOriginalSlots": spawns.len(),
        "sourceMaterializerUsed": true, "extraSpawnSlotsInvented": false });
    preparation["mapSha256"] = json!(MAP_SHA);
    preparation["decompressedMapSha256"] = json!(RAW_MAP_SHA);
    preparation["personalPreparation"] = json!({ "level": 30, "hp": hp, "mp": mp, "gold": 20000,
        "normalRegistrationAndTownBind": true, "skillsOrItemsInjected": false });
    let request = json!({ "accountId": account, "password": password, "name": name, "className": format!("{class:?}"), "characterIndex": index });
    (config, zone, preparation, request.to_string())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicit owned output and frozen input paths; bounded real network cohort"]
async fn crowded_fixture_normal_websocket_three_class_attack_escape_cohort() {
    let frozen = PathBuf::from(
        std::env::var_os("MIR2_DENSE_FROZEN_INPUTS").expect("explicit frozen input path"),
    );
    let output = PathBuf::from(
        std::env::var_os("MIR2_DENSE_OWNED_OUTPUT").expect("explicit fresh owned output path"),
    );
    assert!(frozen.is_absolute() && output.is_absolute() && !output.exists());
    std::fs::create_dir(&output).unwrap();
    original_map(&output, &frozen);
    let _environment = Environment::isolate(&output, &frozen);
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/qa/run-cold-crowded-fixture.mjs");
    let mut runner_failures = Vec::new();
    for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
        let directory = output.join(format!("{class:?}"));
        std::fs::create_dir(&directory).unwrap();
        let (config, zone, preparation, request) = prepared_original_world(class, &directory);
        std::fs::write(
            directory.join("preparation.json"),
            serde_json::to_vec_pretty(&preparation).unwrap(),
        )
        .unwrap();
        let mut owned_factory = SharedInProcessZoneRuntimeFactory::new();
        owned_factory.autonomous_ticks_by_default = false;
        let factory = Arc::new(owned_factory);
        let resources = factory.resources_for_zone(&ZoneId::primary());
        {
            let mut state = resources.zone_state.lock().unwrap();
            assert!(state.zone_sessions.is_empty() && state.players.is_empty());
            assert_eq!(resources.tick_count.load(Ordering::Acquire), 0);
            assert!(state.zone_manager.install_empty_zone(zone));
        }
        let registry = Arc::new(ZoneRegistry::new(ZoneId::primary(), factory.clone()));
        let hub = ChatBroadcastHub::from_env().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        std::env::set_var("MIR2_ALLOWED_WEB_ORIGINS", format!("http://{address}"));
        let server = tokio::spawn(crate::web::serve_web_gateway_with_zone_registry(
            listener,
            config.clone(),
            hub,
            registry,
        ));
        let mut child = tokio::process::Command::new("node")
            .arg(&script)
            .arg(&frozen)
            .arg(&directory)
            .arg(format!("ws://{address}/ws"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        input.write_all(request.as_bytes()).await.unwrap();
        input.shutdown().await.unwrap();
        drop(input);
        let admission_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while !directory.join("admitted.json").exists() {
            assert!(
                tokio::time::Instant::now() < admission_deadline,
                "first admission deadline"
            );
            assert!(
                child.try_wait().unwrap().is_none(),
                "protocol child exited before admission"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        // Ordinary wall clock/deadlines remain unchanged. Enable the original
        // autonomous 300ms cadence after admission and ordinary starter equip.
        resources
            .autonomous_ticks_enabled
            .store(true, Ordering::Release);
        let status = tokio::time::timeout(Duration::from_secs(65), child.wait())
            .await
            .unwrap()
            .unwrap();
        if !status.success() {
            runner_failures.push(format!("{class:?}: protocol runner returned {status}"));
        }
        let report: Value =
            serde_json::from_slice(&std::fs::read(directory.join("report.json")).unwrap()).unwrap();
        println!("{class:?}: qualification={}, attackEscape={}, quiet={} (prepared geometry; native acceptance false)",
            report["qualification"]["status"], report["escape"]["status"], report["escape"]["stop"]["status"]);
        let drain_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let empty = {
                let state = resources.zone_state.lock().unwrap();
                state.players.is_empty()
                    && state.zone_sessions.is_empty()
                    && state.live_zone_outbounds.is_empty()
            };
            if empty {
                break;
            }
            assert!(
                tokio::time::Instant::now() < drain_deadline,
                "owned sessions must drain"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        server.abort();
        let server_result = server.await;
        assert!(server_result.is_err_and(|error| error.is_cancelled()));
        std::fs::write(directory.join("cleanup.json"), serde_json::to_vec_pretty(&json!({
            "ordinaryLogoutAndSocketClose": report["normalLogoutAndClose"], "players": 0, "zoneSessions": 0,
            "liveOutbounds": 0, "ownedTokioListenerStopped": true, "productionProcessUsed": false,
            "autonomousCadenceTicks": resources.tick_count.load(Ordering::Acquire),
            "fullP1Accepted": false, "nativeInputAccepted": false, "nativeVisualAccepted": false
        })).unwrap()).unwrap();
        drop(factory);
        drop(resources);
    }
    assert!(
        runner_failures.is_empty(),
        "retain all three class results: {runner_failures:?}"
    );
}

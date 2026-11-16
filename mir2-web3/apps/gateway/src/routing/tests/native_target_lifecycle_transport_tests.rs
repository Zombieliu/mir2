//! A transparent recorder joins the real Gateway to the independently built
//! native client test. Frames are forwarded unchanged; no packet is synthesized.
use crate::routing::{SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRegistry};
use crate::{tcp::chat_broadcast::ChatBroadcastHub, GatewayConfig};
use futures_util::{SinkExt, StreamExt};
use rand::RngCore;
use serde_json::{json, Value};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, net::TcpListener};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue, Message};

struct OwnedEnvironment(Vec<(OsString, OsString)>);
impl OwnedEnvironment {
    fn enter(root: &Path, maps: &Path) -> Self {
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
            "owned-target-lifecycle-session-32bytes",
        );
        std::env::set_var(
            "MIR2_IDENTITY_RECOVERY_PEPPER",
            "owned-target-lifecycle-recovery-32bytes",
        );
        std::env::set_var("MIR2_ACCOUNT_STORE_BACKEND", "file");
        std::env::set_var("MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS", "0");
        std::env::set_var("MIR2_AI_LIVE_DATA_DIR", root.join("ai-private"));
        std::env::set_var("MIR2_SAVE_RECOVERY_DIR", root.join("save-recovery-private"));
        std::env::set_var("MIR2_CRYSTAL_MAP_PACK", maps);
        Self(inherited)
    }
}
impl Drop for OwnedEnvironment {
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

fn record(
    trace: &Arc<Mutex<Vec<Value>>>,
    started: Instant,
    from: &str,
    message: &Message,
) -> Option<usize> {
    let Message::Text(text) = message else {
        return None;
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return None;
    };
    // Whitelist only causal movement/lifecycle fields. Never record passwords,
    // access tokens, resume credentials or whole account/world payloads.
    let mut row = json!({"atMs":started.elapsed().as_millis(),"from":from,
        "type":value.get("type"),"packet":value.get("packet")});
    if from == "native"
        && matches!(
            value["type"].as_str(),
            Some("walk" | "run" | "turn" | "attack")
        )
    {
        for key in ["direction", "objectId"] {
            if let Some(v) = value.get(key) {
                row[key] = v.clone();
            }
        }
    }
    if from == "gateway" && value["packet"] == "UserLocation" {
        for key in ["x", "y", "direction"] {
            if let Some(v) = value["payload"].get(key) {
                row[key] = v.clone();
            }
        }
    }
    let mut trace = trace.lock().unwrap();
    let sequence = trace.len() + 1;
    row["sequence"] = json!(sequence);
    trace.push(row);
    Some(sequence)
}

async fn transparent_recorder(
    listener: TcpListener,
    upstream: String,
    origin: String,
    trace: Arc<Mutex<Vec<Value>>>,
    attack_ack: PathBuf,
) -> Result<(), String> {
    let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
    let mut native = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(|e| e.to_string())?;
    let mut request = upstream.into_client_request().map_err(|e| e.to_string())?;
    request.headers_mut().insert(
        "Origin",
        HeaderValue::from_str(&origin).map_err(|e| e.to_string())?,
    );
    let (mut gateway, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut attack_written = None;
    loop {
        tokio::select! {
            message=native.next()=>match message {
                Some(Ok(message))=>{
                    let sequence=record(&trace,started,"native",&message);
                    let closing=matches!(message,Message::Close(_));
                    let is_attack=match &message {Message::Text(text)=>serde_json::from_str::<Value>(text)
                        .is_ok_and(|v|v["type"]=="attack" && v["objectId"]==260_917),_=>false};
                    gateway.send(message).await.map_err(|e|e.to_string())?;
                    if is_attack { attack_written=sequence; }
                    if closing { return Ok(()); }
                },
                Some(Err(error))=>return Err(error.to_string()), None=>return Ok(()),
            },
            message=gateway.next()=>match message {
                Some(Ok(message))=>{
                    let sequence=record(&trace,started,"gateway",&message);
                    if let (Some(command),Some(ack))=(attack_written,sequence) {
                        if !attack_ack.exists() {
                            if let Message::Text(text)=&message {
                                if let Ok(value)=serde_json::from_str::<Value>(text) {
                                    if value["packet"]=="UserLocation" {
                                        let witness=json!({"commandSequence":command,"ackSequence":ack,
                                            "x":value["payload"]["x"],"y":value["payload"]["y"],
                                            "direction":value["payload"]["direction"]});
                                        let mut file=std::fs::OpenOptions::new().write(true).create_new(true)
                                            .open(&attack_ack).map_err(|e|e.to_string())?;
                                        std::io::Write::write_all(&mut file,&serde_json::to_vec_pretty(&witness).unwrap())
                                            .map_err(|e|e.to_string())?;
                                    }
                                }
                            }
                        }
                    }
                    let closing=matches!(message,Message::Close(_));
                    native.send(message).await.map_err(|e|e.to_string())?;
                    if closing { return Ok(()); }
                },
                Some(Err(error))=>return Err(error.to_string()), None=>return Ok(()),
            },
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires independently compiled native test executable and fresh owned output"]
async fn native_target_lifecycle_real_gateway_three_class_cohort() {
    let native = PathBuf::from(
        std::env::var_os("MIR2_TARGET_NATIVE_TEST_EXE").expect("native test executable"),
    );
    let output =
        PathBuf::from(std::env::var_os("MIR2_TARGET_OWNED_OUTPUT").expect("fresh owned output"));
    assert!(native.is_absolute() && native.is_file() && output.is_absolute() && !output.exists());
    std::fs::create_dir(&output).unwrap();
    let maps =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/routing/tests/fixtures/mining-map-pack");
    assert!(maps.join("0.map.gz").is_file());
    let _environment = OwnedEnvironment::enter(&output, &maps);
    let mut failures = Vec::new();
    for (index, class) in ["Warrior", "Wizard", "Taoist"].into_iter().enumerate() {
        let directory = output.join(class);
        std::fs::create_dir(&directory).unwrap();
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
        let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
        let resources = factory.resources_for_zone(&ZoneId::primary());
        let registry = Arc::new(ZoneRegistry::new(ZoneId::primary(), factory));
        let server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gateway_address = server_listener.local_addr().unwrap();
        let record_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let record_address = record_listener.local_addr().unwrap();
        let origin = format!("http://{record_address}");
        std::env::set_var("MIR2_ALLOWED_WEB_ORIGINS", &origin);
        let server = tokio::spawn(crate::web::serve_web_gateway_with_zone_registry(
            server_listener,
            config,
            ChatBroadcastHub::from_env().unwrap(),
            registry,
        ));
        let trace = Arc::new(Mutex::new(Vec::new()));
        let attack_ack = directory.join("attack-ack.json");
        let recorder = tokio::spawn(transparent_recorder(
            record_listener,
            format!("ws://{gateway_address}/ws"),
            origin,
            trace.clone(),
            attack_ack.clone(),
        ));
        let report_path = directory.join("native-report.json");
        let request = json!({"url":format!("ws://{record_address}/ws"),"account":format!("TargetNet{index}"),
            "password":"OwnedTarget321","name":format!("Target{index}"),"class":class,
            "map_gzip":maps.join("0.map.gz"),"report":report_path,"attack_ack":attack_ack});
        let stdout = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("native-stdout.log"))
            .unwrap();
        let stderr = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("native-stderr.log"))
            .unwrap();
        let mut child=tokio::process::Command::new(&native)
            .args(["--exact","gateway::target_lifecycle_network_tests::native_rejected_target_actual_gateway_round_trip",
                "--ignored","--nocapture","--test-threads=1"])
            .stdin(std::process::Stdio::piped()).stdout(stdout).stderr(stderr).kill_on_drop(true).spawn().unwrap();
        let mut input = child.stdin.take().unwrap();
        input
            .write_all(request.to_string().as_bytes())
            .await
            .unwrap();
        input.shutdown().await.unwrap();
        drop(input);
        let status = tokio::time::timeout(Duration::from_secs(90), child.wait()).await;
        let succeeded = matches!(&status,Ok(Ok(status)) if status.success());
        if !succeeded {
            failures.push(format!("{class}: native child failed or timed out"));
        }
        if status.is_err() {
            child.start_kill().unwrap();
            let _ = child.wait().await;
        }
        let report: Value = std::fs::read(&report_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null);
        let frames = trace.lock().unwrap().clone();
        std::fs::write(
            directory.join("wire-trace.json"),
            serde_json::to_vec_pretty(&frames).unwrap(),
        )
        .unwrap();
        let sent: Vec<_> = frames
            .iter()
            .filter(|frame| {
                frame["from"] == "native"
                    && matches!(frame["type"].as_str(), Some("walk" | "run" | "attack"))
            })
            .collect();
        let wire_correct = sent.len() == 2
            && sent[0]["type"] == "attack"
            && sent[0]["objectId"] == 260_917
            && sent[1]["type"] == "walk"
            && sent[1]["direction"] == report["freshDirection"];
        let causal_ack = wire_correct
            && frames.iter().any(|frame| {
                frame["from"] == "gateway"
                    && frame["packet"] == "UserLocation"
                    && frame["sequence"] == report["wireAckSequence"]
                    && frame["sequence"].as_u64() > sent[0]["sequence"].as_u64()
                    && frame["sequence"].as_u64() < sent[1]["sequence"].as_u64()
                    && sent[0]["sequence"] == report["wireCommandSequence"]
            });
        let logout = frames
            .iter()
            .any(|frame| frame["from"] == "gateway" && frame["packet"] == "LogOutSuccess");
        let drain_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let drained = loop {
            let empty = {
                let state = resources.zone_state.lock().unwrap();
                state.players.is_empty()
                    && state.zone_sessions.is_empty()
                    && state.live_zone_outbounds.is_empty()
            };
            if empty {
                break true;
            }
            if tokio::time::Instant::now() >= drain_deadline {
                break false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        let checks = json!({"class":class,"nativePassed":succeeded && report["status"]=="passed",
            "oldMovementNeverWritten":wire_correct,"normalLogoutWireAck":logout,
            "causalAttackCorrection":causal_ack,
            "nativeLogoutAck":report["normalLogoutAcknowledged"],"sharedZoneDrained":drained,
            "humanAcceptance":false,"rendererStarted":false});
        std::fs::write(
            directory.join("gateway-checks.json"),
            serde_json::to_vec_pretty(&checks).unwrap(),
        )
        .unwrap();
        if !wire_correct
            || !causal_ack
            || !logout
            || !drained
            || report["normalLogoutAcknowledged"] != true
        {
            failures.push(format!(
                "{class}: full native movement/logout/drain proof incomplete"
            ));
        }
        println!("{class}: native={succeeded}, wire={wire_correct}, logout={logout}, drained={drained}; human=false");
        if !recorder.is_finished() {
            recorder.abort();
        }
        let _ = recorder.await;
        server.abort();
        let _ = server.await;
    }
    std::fs::write(
        output.join("cohort.json"),
        serde_json::to_vec_pretty(&json!({"failures":failures,
        "classes":3,"normalRegistration":true,"humanAcceptance":false}))
        .unwrap(),
    )
    .unwrap();
    assert!(
        failures.is_empty(),
        "real native three-class cohort failed: {failures:?}"
    );
}

//! Prepared ordinary accounts/levels and native encounters; the relationship,
//! bank, source receipt and payout are produced only by normal TCP/WS packets.
//! File authority is released before cold reload. This is isolated socket
//! evidence, not native-client/public-play or independent OS-process acceptance.
use futures_util::{SinkExt, StreamExt};
use mir2_gateway::routing::SharedAccountInventoryCommitOutcome;
use mir2_gateway::tcp::chat_broadcast::ChatBroadcastHub;
use mir2_gateway::{
    GatewayConfig, InProcessAccountInventoryService, SharedAccountInventoryCommand,
    SharedAccountInventoryCommandEnvelope, SharedAccountInventoryExecutionContext,
    SharedAccountInventoryService, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRegistry,
};
use mir2_protocol::{
    decode_server_packet, encode_client_packet, ClientPacket, MirClass, MirDirection, MirGender,
    Point, ServerPacket, Spell,
};
use mir2_simulation::{
    AccountRecord, AccountStore, AccountStoreTransactionFault, CharacterRecord,
    InProcessWorldRuntime, MonsterSpawnSource, SharedAccountInventoryTransactionKind,
    SharedAccountInventoryTransactionReceipt, Stage5FriendIdentity, Stage5MentorState,
    WorldEntityDisposition, ZoneMonsterSpawn,
};
use serde_json::{json, Value};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

const MAP: &str = "mentor-source-transport";
const PUPIL: &str = "mentor-source-pupil";
const TEACHER: &str = "mentor-source-teacher";
const PUPIL_NAME: &str = "SourcePupil";
const TEACHER_NAME: &str = "SourceTeacher";
const PASSWORD: &str = "isolated-mentor-source-password";
type WebClient = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Environment {
    inherited: Vec<(OsString, OsString)>,
    directory: PathBuf,
}
impl Environment {
    fn new(label: &str) -> Self {
        let inherited: Vec<_> = std::env::vars_os()
            .filter(|(name, _)| name.to_string_lossy().starts_with("MIR2_"))
            .collect();
        for (name, _) in &inherited {
            std::env::remove_var(name);
        }
        let directory = std::env::temp_dir().join(format!(
            "mir2-mentor-source-{}-{}-{label}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::env::set_var("MIR2_RUNTIME_ENV", "test");
        std::env::set_var(
            "MIR2_IDENTITY_SESSION_SECRET",
            "isolated-mentor-source-session-key-32",
        );
        std::env::set_var(
            "MIR2_IDENTITY_RECOVERY_PEPPER",
            "isolated-mentor-source-recovery-key-32",
        );
        std::env::set_var("MIR2_AI_LIVE_DATA_DIR", &directory);
        Self {
            inherited,
            directory,
        }
    }
    fn path(&self) -> PathBuf {
        self.directory.join("accounts.json")
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        for (name, _) in std::env::vars_os()
            .filter(|(name, _)| name.to_string_lossy().starts_with("MIR2_"))
            .collect::<Vec<_>>()
        {
            std::env::remove_var(name);
        }
        for (name, value) in &self.inherited {
            std::env::set_var(name, value);
        }
        if self.directory.is_absolute()
            && self.directory.parent() == Some(std::env::temp_dir().as_path())
        {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
}
struct Servers(Vec<tokio::task::JoinHandle<std::io::Result<()>>>);
impl Servers {
    async fn stop(mut self) {
        for handle in self.0.drain(..) {
            handle.abort();
            let _ = handle.await;
        }
    }
}
impl Drop for Servers {
    fn drop(&mut self) {
        for handle in &self.0 {
            handle.abort();
        }
    }
}
fn identity(account: &str) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: account.into(),
        character_index: if account == PUPIL { 1 } else { 2 },
    }
}
fn mentor(config: &GatewayConfig, account: &str) -> Stage5MentorState {
    config
        .shared_mentor_profile_for(&identity(account))
        .unwrap()
        .mentor
}
fn progress(config: &GatewayConfig, account: &str) -> (u16, i64) {
    let id = identity(account);
    let store = config.account_store.lock().unwrap();
    let save = &store.accounts[account].saves[&id.character_index];
    (save.character.level, save.experience)
}
fn scene(environment: &Environment) -> GatewayConfig {
    let mut config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_path(environment.path())
        .with_save_recovery_dir(environment.directory.join("recovery"))
        .with_save_recovery_mac_key(std::array::from_fn::<u8, 32, _>(|index| index as u8 + 71))
        .unwrap();
    config.monster_spawn_source = MonsterSpawnSource::StarterScenario;
    config.map.file_name = MAP.into();
    config.map.title = "Mentor source transport fixture".into();
    config.spawn = Point { x: 10, y: 10 };
    config.map_collision.map_file_name = MAP.into();
    config.map_collision.map_width = 40;
    config.map_collision.map_height = 40;
    config.map_collision.play_bounds = mir2_game_data::MapBounds {
        min_x: 0,
        max_x: 39,
        min_y: 0,
        max_y: 39,
    };
    config.map_collision.region_bounds = config.map_collision.play_bounds.clone();
    config.map_collision.blocked_cells.clear();
    config.map_collision.doors.clear();
    config.visible_players.clear();
    config.visible_monsters.clear();
    config.visible_npcs.clear();
    config.map_transfers.clear();
    config.safe_zones.clear();
    config
}
fn prepared(environment: &Environment) -> GatewayConfig {
    let config = scene(environment);
    let mut store = config.account_store.lock().unwrap();
    store.accounts.clear();
    for (account, index, name, level, x, threshold) in [
        (PUPIL, 1, PUPIL_NAME, 10, 10, 6_000),
        (TEACHER, 2, TEACHER_NAME, 20, 14, 140_000),
    ] {
        let mut record = AccountRecord::new(CharacterRecord {
            index,
            name: name.into(),
            level,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        });
        record.password = PASSWORD.into();
        let save = record.saves.get_mut(&index).unwrap();
        save.map_file_name = MAP.into();
        save.map_title = config.map.title.clone();
        save.position = Point { x, y: 10 };
        // Original ExpList rows; the custom fixture has no content profile.
        save.max_experience = threshold;
        store.accounts.insert(account.into(), record);
    }
    drop(store);
    for account in [PUPIL, TEACHER] {
        let state = mentor(&config, account);
        assert!(state.partner_identity.is_none() && state.ledger.bank_events.is_empty());
        assert_eq!(state.ledger.bank_earned, 0);
        assert_eq!(state.ledger.leveling_credit, 0);
    }
    config
}

#[derive(Debug)]
struct ObservedSource {
    config: GatewayConfig,
    path: PathBuf,
    inner: InProcessAccountInventoryService,
    fault_done: AtomicBool,
    allow_retry: AtomicBool,
    confirmed: AtomicUsize,
    final_amount: u32,
}
impl ObservedSource {
    fn new(config: GatewayConfig, path: PathBuf, final_amount: u32) -> Self {
        Self {
            config,
            path,
            inner: InProcessAccountInventoryService::new(),
            fault_done: AtomicBool::new(false),
            allow_retry: AtomicBool::new(false),
            confirmed: AtomicUsize::new(0),
            final_amount,
        }
    }
}
impl SharedAccountInventoryService for ObservedSource {
    fn commit(
        &self,
        runtime: &mut InProcessWorldRuntime,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        (*self.commit_fenced(runtime, None, envelope)).clone()
    }
    fn commit_fenced(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        let is_kill = matches!(
            &envelope.command,
            SharedAccountInventoryCommand::MonsterKillAward(_)
        );
        if is_kill
            && self.fault_done.load(Ordering::Acquire)
            && !self.allow_retry.load(Ordering::Acquire)
        {
            return SharedAccountInventoryCommitOutcome::Deferred {
                receipt: SharedAccountInventoryTransactionReceipt {
                    kind: SharedAccountInventoryTransactionKind::MonsterKillAward,
                    committed: false,
                    packets: vec![],
                },
            };
        }
        if let SharedAccountInventoryCommand::MonsterKillAward(award) = &envelope.command {
            assert_eq!(envelope.identity.account_id, PUPIL);
            assert_eq!(
                award.experience_selection.as_ref().unwrap().final_amount,
                self.final_amount
            );
        }
        let before = is_kill.then(|| {
            let store = self.config.account_store.lock().unwrap();
            serde_json::to_value(&store.accounts[PUPIL].saves[&1]).unwrap()
        });
        let inject = is_kill && !self.fault_done.load(Ordering::Acquire);
        if inject {
            self.config.inject_account_store_transaction_fault(
                AccountStoreTransactionFault::BeforePersist,
            );
        }
        let outcome = self.inner.commit_fenced(runtime, context, envelope);
        if inject {
            assert!(
                !matches!(&outcome, SharedAccountInventoryCommitOutcome::Confirmed(receipt) if receipt.committed)
            );
            let after = {
                let store = self.config.account_store.lock().unwrap();
                serde_json::to_value(&store.accounts[PUPIL].saves[&1]).unwrap()
            };
            assert!(
                before.as_ref() == Some(&after),
                "known Source failure restores XP, level, complete mentor ledger and revision"
            );
            self.fault_done.store(true, Ordering::Release);
        } else if is_kill
            && matches!(&outcome, SharedAccountInventoryCommitOutcome::Confirmed(receipt) if receipt.committed)
        {
            let file: AccountStore =
                serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap();
            let stored = {
                let store = self.config.account_store.lock().unwrap();
                serde_json::to_value(&store.accounts[PUPIL].saves[&1]).unwrap()
            };
            assert!(
                stored == serde_json::to_value(&file.accounts[PUPIL].saves[&1]).unwrap(),
                "real XP and bank reach disk before confirmed ACK"
            );
            assert_eq!(
                mentor(&self.config, PUPIL).ledger.bank_earned,
                u64::from(self.final_amount / 100)
            );
            self.confirmed.fetch_add(1, Ordering::Release);
        }
        outcome
    }
}

async fn tcp_send(client: &mut TcpStream, packet: ClientPacket) {
    client
        .write_all(&encode_client_packet(&packet).unwrap())
        .await
        .unwrap();
}
async fn tcp_until(
    client: &mut TcpStream,
    expected: impl Fn(&ServerPacket) -> bool,
) -> Vec<ServerPacket> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut packets = Vec::new();
    loop {
        let frame = tokio::time::timeout_at(deadline, async {
            let mut header = [0u8; 2];
            client.read_exact(&mut header).await?;
            let length = u16::from_le_bytes(header) as usize;
            assert!(length >= 4);
            let mut frame = vec![0u8; length];
            frame[..2].copy_from_slice(&header);
            client.read_exact(&mut frame[2..]).await?;
            Ok::<_, std::io::Error>(frame)
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "missing TCP response; kinds {:?}",
                packets
                    .iter()
                    .map(mir2_protocol::server_packet_name)
                    .collect::<Vec<_>>()
            )
        })
        .unwrap();
        let packet = decode_server_packet(&frame).unwrap();
        let done = expected(&packet);
        packets.push(packet);
        if done {
            return packets;
        }
    }
}
async fn web_send(client: &mut WebClient, event: Value) {
    client
        .send(Message::Text(event.to_string().into()))
        .await
        .unwrap();
}
async fn web_until(client: &mut WebClient, expected: impl Fn(&Value) -> bool) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut events = Vec::new();
    loop {
        let message = tokio::time::timeout_at(deadline, client.next())
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "missing WS response; kinds {:?}",
                    events
                        .iter()
                        .map(|event: &Value| event["packet"].as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                )
            })
            .unwrap()
            .unwrap();
        let Message::Text(message) = message else {
            continue;
        };
        let event: Value = serde_json::from_str(&message).unwrap();
        assert!(event["type"] != "error", "ordinary mentor command failed");
        let done = expected(&event);
        events.push(event);
        if done {
            return events;
        }
    }
}
fn packet(event: &Value, kind: &str) -> bool {
    event["type"] == "packet" && event["packet"] == kind
}
async fn tcp_barrier(client: &mut TcpStream, marker: u8) -> Vec<ServerPacket> {
    tcp_send(client, ClientPacket::ChangeAMode { mode: marker }).await;
    tcp_until(
        client,
        |packet| matches!(packet, ServerPacket::ChangeAMode { mode } if *mode == marker),
    )
    .await
}
async fn web_barrier(client: &mut WebClient, marker: u8) -> Vec<Value> {
    web_send(client, json!({"type":"changeAMode","mode":marker})).await;
    web_until(client, |event| {
        packet(event, "ChangeAMode") && event["payload"]["mode"] == marker
    })
    .await
}
fn tcp_gains(packets: &[ServerPacket]) -> Vec<u32> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::GainExperience { amount } => Some(*amount),
            _ => None,
        })
        .collect()
}
fn web_gains(events: &[Value]) -> Vec<u32> {
    events
        .iter()
        .filter(|event| packet(event, "GainExperience"))
        .map(|event| event["payload"]["amount"].as_u64().unwrap() as u32)
        .collect()
}
async fn login_tcp(client: &mut TcpStream) -> Vec<ServerPacket> {
    tcp_send(
        client,
        ClientPacket::Login {
            account_id: PUPIL.into(),
            password: PASSWORD.into(),
        },
    )
    .await;
    tcp_until(client, |packet| {
        matches!(packet, ServerPacket::LoginSuccess { .. })
    })
    .await;
    tcp_send(client, ClientPacket::StartGame { character_index: 1 }).await;
    tcp_until(client, |packet| {
        matches!(packet, ServerPacket::UserInformation { .. })
    })
    .await
}
async fn login_web(client: &mut WebClient) -> Vec<Value> {
    web_send(
        client,
        json!({"type":"login","accountId":TEACHER,"password":PASSWORD}),
    )
    .await;
    web_until(client, |event| packet(event, "LoginSuccess")).await;
    web_send(client, json!({"type":"startGame","characterIndex":2})).await;
    web_until(client, |event| packet(event, "UserInformation")).await
}
async fn connect_web(address: std::net::SocketAddr) -> WebClient {
    let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("origin", format!("http://{address}").parse().unwrap());
    connect_async(request).await.unwrap().0
}
async fn start_servers(
    config: GatewayConfig,
    factory: Arc<SharedInProcessZoneRuntimeFactory>,
) -> (Servers, std::net::SocketAddr, std::net::SocketAddr) {
    let registry = Arc::new(ZoneRegistry::new(ZoneId::primary(), factory));
    let hub = ChatBroadcastHub::from_env().unwrap();
    let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let web = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tcp_address = tcp.local_addr().unwrap();
    let web_address = web.local_addr().unwrap();
    (
        Servers(vec![
            tokio::spawn(mir2_gateway::tcp::serve_tcp_gateway_with_zone_registry(
                tcp,
                config.clone(),
                hub.clone(),
                registry.clone(),
            )),
            tokio::spawn(mir2_gateway::web::serve_web_gateway_with_zone_registry(
                web, config, hub, registry,
            )),
        ]),
        tcp_address,
        web_address,
    )
}
fn spawn(factory: &SharedInProcessZoneRuntimeFactory, amount: u32) {
    assert_eq!(
        factory
            .apply_world_event_monsters(
                &ZoneId::primary(),
                MAP,
                &[ZoneMonsterSpawn {
                    crystal_drop_seed: None,
                    object_id: 940_001,
                    name: "Scarecrow".into(),
                    name_colour_argb: -1,
                    image: 0,
                    ai: 0,
                    disposition: Some(WorldEntityDisposition::Hostile),
                    level: 10,
                    hp: 1,
                    max_hp: 1,
                    experience: amount,
                    move_speed_ms: 60_000,
                    attack_speed_ms: 60_000,
                    friendly_guild: None,
                    position: Point { x: 10, y: 9 },
                    direction: MirDirection::Down,
                    defense: Default::default(),
                    respawn: None,
                    drops: vec![],
                }],
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
            )
            .unwrap(),
        1
    );
}
async fn logout_both(tcp: &mut TcpStream, web: &mut WebClient) {
    tcp_send(tcp, ClientPacket::LogOut).await;
    tcp_until(tcp, |packet| {
        matches!(packet, ServerPacket::LogOutSuccess { .. })
    })
    .await;
    web_send(web, json!({"type":"logOut"})).await;
    web_until(web, |event| packet(event, "LogOutSuccess")).await;
    tcp.shutdown().await.unwrap();
    web.close(None).await.unwrap();
}

async fn story(graduates: bool) {
    let environment = Environment::new(if graduates { "graduation" } else { "manual" });
    let amount = if graduates { 6_000 } else { 1_000 };
    let reward = amount / 100;
    let (weak_store, pupil_state, teacher_state) = {
        let config = prepared(&environment);
        let weak_store = Arc::downgrade(&config.account_store);
        let source = Arc::new(ObservedSource::new(
            config.clone(),
            environment.path(),
            amount,
        ));
        let factory = Arc::new(
            SharedInProcessZoneRuntimeFactory::with_account_inventory_service(source.clone()),
        );
        let (servers, tcp_address, web_address) =
            start_servers(config.clone(), factory.clone()).await;
        let mut tcp = TcpStream::connect(tcp_address).await.unwrap();
        assert!(tcp_gains(&login_tcp(&mut tcp).await).is_empty());
        let mut web = connect_web(web_address).await;
        assert!(web_gains(&login_web(&mut web).await).is_empty());
        web_send(&mut web, json!({"type":"allowMentor"})).await;
        // Bootstrap emits attack mode0. Each normal command uses a fresh mode
        // marker, so no prior bootstrap/projection ACK qualifies this barrier.
        assert!(web_gains(&web_barrier(&mut web, 1).await).is_empty());
        assert!(mentor(&config, TEACHER).allow_mentor);
        tcp_send(
            &mut tcp,
            ClientPacket::AddMentor {
                name: TEACHER_NAME.into(),
            },
        )
        .await;
        web_until(&mut web, |event| {
            packet(event, "MentorRequest")
                && event["payload"]["name"] == PUPIL_NAME
                && event["payload"]["level"] == 10
        })
        .await;
        web_send(&mut web, json!({"type":"mentorReply","acceptInvite":true})).await;
        web_until(&mut web, |event| {
            packet(event, "MentorUpdate")
                && event["payload"]["name"] == PUPIL_NAME
                && event["payload"]["online"] == true
        })
        .await;
        tcp_send(&mut tcp, ClientPacket::ChangeAMode { mode: 1 }).await;
        tcp_until(&mut tcp, |packet| matches!(packet, ServerPacket::MentorUpdate { name, level: 20, online: true, .. } if name == TEACHER_NAME)).await;
        assert_eq!(
            mentor(&config, PUPIL).partner_identity,
            Some(identity(TEACHER))
        );
        assert_eq!(
            mentor(&config, TEACHER).partner_identity,
            Some(identity(PUPIL))
        );
        assert!(mentor(&config, TEACHER).is_mentor);
        let epoch = mentor(&config, PUPIL).ledger.relationship_epoch;
        assert!(epoch > 0);
        assert_eq!(epoch, mentor(&config, TEACHER).ledger.relationship_epoch);

        spawn(&factory, amount);
        tcp_until(&mut tcp, |packet| matches!(packet, ServerPacket::ObjectMonster { info } if info.object_id == 940_001)).await;
        tcp_send(
            &mut tcp,
            ClientPacket::Attack {
                direction: MirDirection::Up,
                spell: Spell::None,
            },
        )
        .await;
        let failed = tcp_until(&mut tcp, |packet| matches!(packet, ServerPacket::ObjectDied { info } if info.object_id == 940_001)).await;
        assert!(tcp_gains(&failed).is_empty());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !source.fault_done.load(Ordering::Acquire) {
            assert!(
                tokio::time::Instant::now() < deadline,
                "actual mentor source reaches publication fault"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // Consume the acceptance command's terminal as well before issuing a
        // fresh barrier; no earlier command ACK can stand in for this one.
        let failed_barrier = tcp_barrier(&mut tcp, 2).await;
        assert!(tcp_gains(&failed_barrier).is_empty());
        assert_eq!(progress(&config, PUPIL), (10, 0));
        assert_eq!(mentor(&config, PUPIL).ledger.bank_earned, 0);
        assert!(mentor(&config, PUPIL).ledger.bank_events.is_empty());
        source.allow_retry.store(true, Ordering::Release);
        let gain = tcp_until(&mut tcp, |packet| matches!(packet, ServerPacket::GainExperience { amount: gained } if *gained == amount)).await;
        assert_eq!(tcp_gains(&gain), vec![amount]);
        assert_eq!(source.confirmed.load(Ordering::Acquire), 1);
        assert_eq!(mentor(&config, PUPIL).ledger.bank_events.len(), 1);
        assert_eq!(mentor(&config, PUPIL).ledger.bank_earned, u64::from(reward));
        if !graduates {
            assert_eq!(progress(&config, TEACHER), (20, 0));
        }

        if graduates {
            tcp_until(&mut tcp, |packet| matches!(packet, ServerPacket::MentorUpdate { name, .. } if name.is_empty())).await;
            assert_eq!(progress(&config, PUPIL), (11, 0));
            assert_eq!(mentor(&config, PUPIL).cooldown_until_ms, 0);
        } else {
            assert_eq!(progress(&config, PUPIL), (10, 1_000));
            tcp_send(&mut tcp, ClientPacket::LogOut).await;
            tcp_until(&mut tcp, |packet| {
                matches!(packet, ServerPacket::LogOutSuccess { .. })
            })
            .await;
            assert_eq!(
                mentor(&config, PUPIL).ledger.bank_settled,
                u64::from(reward)
            );
            assert_eq!(mentor(&config, TEACHER).mentee_exp, i64::from(reward));
            assert_eq!(mentor(&config, TEACHER).ledger.leveling_credit, 0);
            assert_eq!(progress(&config, TEACHER), (20, 0));
            assert!(web_gains(&web_barrier(&mut web, 2).await).is_empty());
            // The transport retires authentication on ordinary LogOut. A
            // returning character must perform fresh Login before StartGame.
            let started = login_tcp(&mut tcp).await;
            assert!(tcp_gains(&started).is_empty());
            let barrier = tcp_barrier(&mut tcp, 3).await;
            assert!(tcp_gains(&barrier).is_empty());
            assert_eq!(progress(&config, PUPIL), (10, 1_000));
            tcp_send(&mut tcp, ClientPacket::CancelMentor).await;
            let canceled = tcp_barrier(&mut tcp, 4).await;
            assert!(tcp_gains(&canceled).is_empty());
            assert!(mentor(&config, PUPIL).cooldown_until_ms > 0);
        }
        for account in [PUPIL, TEACHER] {
            assert!(mentor(&config, account).partner_identity.is_none());
        }
        assert_eq!(
            mentor(&config, PUPIL).ledger.bank_settled,
            u64::from(reward)
        );
        assert_eq!(
            mentor(&config, TEACHER).ledger.leveling_credit,
            u64::from(reward)
        );
        assert_eq!(mentor(&config, TEACHER).ledger.balance_credit, 0);
        // The recipient executes its own ordinary command; no peer private
        // runtime is obtained by this test or the shared settlement reducer.
        let payout = web_barrier(&mut web, if graduates { 2 } else { 3 }).await;
        assert_eq!(web_gains(&payout), vec![reward]);
        assert_eq!(progress(&config, TEACHER), (20, i64::from(reward)));
        let settled = mentor(&config, TEACHER);
        assert_eq!(settled.ledger.leveling_applied, u64::from(reward));
        assert_eq!(settled.mentee_exp, 0);
        assert!(web_gains(&web_barrier(&mut web, if graduates { 3 } else { 4 }).await).is_empty());
        assert!(tcp_gains(&tcp_barrier(&mut tcp, if graduates { 3 } else { 5 }).await).is_empty());
        assert_eq!(source.confirmed.load(Ordering::Acquire), 1);
        logout_both(&mut tcp, &mut web).await;
        let pupil_state = mentor(&config, PUPIL);
        let teacher_state = mentor(&config, TEACHER);
        drop(tcp);
        drop(web);
        servers.stop().await;
        drop(factory);
        drop(source);
        drop(config);
        (weak_store, pupil_state, teacher_state)
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while weak_store.upgrade().is_some() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "cold phase releases all old file/Source authorities"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let config = scene(&environment);
    assert_eq!(mentor(&config, PUPIL), pupil_state);
    assert_eq!(mentor(&config, TEACHER), teacher_state);
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
    let (servers, tcp_address, web_address) = start_servers(config.clone(), factory).await;
    let mut tcp = TcpStream::connect(tcp_address).await.unwrap();
    let pupil_started = login_tcp(&mut tcp).await;
    let mut web = connect_web(web_address).await;
    let teacher_started = login_web(&mut web).await;
    assert!(tcp_gains(&pupil_started).is_empty() && web_gains(&teacher_started).is_empty());
    assert!(tcp_gains(&tcp_barrier(&mut tcp, 1).await).is_empty());
    assert!(web_gains(&web_barrier(&mut web, 1).await).is_empty());
    assert_eq!(
        progress(&config, PUPIL),
        if graduates { (11, 0) } else { (10, 1_000) }
    );
    assert_eq!(progress(&config, TEACHER), (20, i64::from(reward)));
    assert_eq!(mentor(&config, PUPIL).ledger.bank_events.len(), 1);
    assert_eq!(
        mentor(&config, TEACHER).ledger.leveling_applied,
        u64::from(reward)
    );
    assert!(
        mentor(&config, PUPIL).partner_identity.is_none()
            && mentor(&config, TEACHER).partner_identity.is_none()
    );
    logout_both(&mut tcp, &mut web).await;
    servers.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn normal_mentor_source_manual_and_graduation_settle_once_across_logout_and_cold_login() {
    // Keep environment ownership serialized inside this one test process.
    story(false).await;
    story(true).await;
}

//! Real public TCP and WebSocket listeners use one registry. Seeded state is
//! server-owned; clients only authenticate, enter the world and click an NPC.
use futures_util::{SinkExt, StreamExt};
use mir2_gateway::tcp::chat_broadcast::ChatBroadcastHub;
use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRegistry};
use mir2_protocol::{
    decode_server_packet, encode_client_packet, ChatType, ClientPacket, MirClass, MirDirection,
    MirGender, Point, ServerPacket,
};
use mir2_simulation::conquest::{
    default_sabuk_defenses, sabuk_policy, ConquestEventKind, SharedConquestRecord,
};
use mir2_simulation::{
    AccountRecord, CharacterRecord, SharedGuildMember, SharedGuildRank, SharedGuildRecord,
    Stage5FriendIdentity, VisibleNpcRecord,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const GUILD: &str = "11111111111111111111111111111111";
const CLOCK: &str = "cccccccccccccccccccccccccccccccc";
type WebClient = WebSocketStream<MaybeTlsStream<TcpStream>>;

// This integration binary has one test. Isolate production initializers from
// inherited DB/Redis/AI/save paths without altering the parent's environment.
struct IsolatedEnvironment {
    inherited: Vec<(OsString, OsString)>,
    data_dir: PathBuf,
}
impl IsolatedEnvironment {
    fn new() -> Self {
        let inherited: Vec<_> = std::env::vars_os()
            .filter(|(name, _)| name.to_string_lossy().starts_with("MIR2_"))
            .collect();
        for (name, _) in &inherited {
            std::env::remove_var(name);
        }
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!(
            "mir2-siege-transport-{}-{unique}",
            std::process::id()
        ));
        std::env::set_var("MIR2_RUNTIME_ENV", "test");
        std::env::set_var(
            "MIR2_IDENTITY_SESSION_SECRET",
            "isolated-siege-identity-session-secret-32",
        );
        std::env::set_var(
            "MIR2_IDENTITY_RECOVERY_PEPPER",
            "isolated-siege-identity-recovery-pepper-32",
        );
        std::env::set_var("MIR2_AI_LIVE_DATA_DIR", &data_dir);
        Self {
            inherited,
            data_dir,
        }
    }
}
impl Drop for IsolatedEnvironment {
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
        // Only the unique absolute test directory directly below temp is owned.
        if self.data_dir.is_absolute()
            && self.data_dir.parent() == Some(std::env::temp_dir().as_path())
        {
            let _ = std::fs::remove_dir_all(&self.data_dir);
        }
    }
}
struct ServerTasks(Vec<tokio::task::JoinHandle<std::io::Result<()>>>);
impl Drop for ServerTasks {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}

fn server_fixture() -> (GatewayConfig, u64) {
    let mut config = GatewayConfig::default();
    config.map.file_name = "0150".into();
    config.map.title = "Sabuk palace transport fixture".into();
    config.spawn = Point { x: 10, y: 13 };
    config.map_collision.map_file_name = "0150".into();
    config.map_collision.map_width = 50;
    config.map_collision.map_height = 40;
    config.map_collision.play_bounds = mir2_game_data::MapBounds {
        min_x: 0,
        max_x: 49,
        min_y: 0,
        max_y: 39,
    };
    config.map_collision.region_bounds = config.map_collision.play_bounds.clone();
    config.map_collision.blocked_cells.clear();
    config.map_collision.doors.clear();
    config.visible_players.clear();
    config.visible_monsters.clear();
    config.map_transfers.clear();
    config.visible_npcs = vec![VisibleNpcRecord {
        object_id: 1146,
        name: "Conquest officer".into(),
        image: 5,
        colour_argb: -1,
        position: Point { x: 9, y: 13 },
        direction: MirDirection::Down,
        quest_ids: Vec::new(),
        script_key: Some("MongchonProvince/SabukWall/Conquest".into()),
    }];
    let mut policy = sabuk_policy();
    policy.utc_offset_minutes = 480;
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    // Socket joins bootstrap the Zone at the real epoch. Replaying today's
    // 18:00 after that hour is correctly rejected as a stale projection.
    // Keep the source calendar and its monotonic guard; exercise the next day.
    let transport_day = epoch.checked_add(86_400_000).unwrap();
    let start = policy.window(transport_day).unwrap().start_ms;
    assert!(start > epoch, "transport clock must follow the real bootstrap");
    config.conquest_policies = vec![policy];
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for (account, index, name, x, class) in [
            ("tcp_siege", 1, "TcpKnight", 10, MirClass::Warrior),
            ("web_siege", 2, "WebKnight", 11, MirClass::Taoist),
        ] {
            let mut record = AccountRecord::new(CharacterRecord {
                index,
                name: name.into(),
                level: 30,
                class,
                gender: MirGender::Male,
            });
            record.password = "transport-test-password".into();
            let save = record.saves.get_mut(&index).unwrap();
            save.map_file_name = "0150".into();
            save.map_title = config.map.title.clone();
            save.position = Point { x, y: 13 };
            store.accounts.insert(account.into(), record);
        }
        store.shared_guilds.insert(
            GUILD.into(),
            SharedGuildRecord {
        active_wars: Default::default(),
                id: GUILD.into(),
                name: "Knights".into(),
                revision: 1,
                level: 0,
                experience: 0,
                spare_points: 0,
                gold: 1_000_000,
                ranks: vec![
                    SharedGuildRank {
                        index: 0,
                        name: "Leader".into(),
                        options: 255,
                    },
                    SharedGuildRank {
                        index: 1,
                        name: "Member".into(),
                        options: 0,
                    },
                ],
                members: vec![
                    SharedGuildMember {
                        membership_epoch: 0,
                        identity: Stage5FriendIdentity {
                            account_id: "tcp_siege".into(),
                            character_index: 1,
                        },
                        name: "TcpKnight".into(),
                        rank_index: 1,
                    },
                    SharedGuildMember {
                        membership_epoch: 0,
                        identity: Stage5FriendIdentity {
                            account_id: "web_siege".into(),
                            character_index: 2,
                        },
                        name: "WebKnight".into(),
                        rank_index: 0,
                    },
                ],
                notice: Vec::new(),
                storage: BTreeMap::new(),
                buffs: BTreeMap::new(),
                last_buff_tick_ms: 0,
                experience_receipts: BTreeSet::new(),
                experience_receipt_payloads: BTreeMap::new(),
            },
        );
        let mut siege = SharedConquestRecord::new(1);
        siege.defenses = default_sabuk_defenses();
        siege.request(GUILD, start - 1).unwrap();
        store.shared_conquests.insert(1, siege);
    }
    (config, start)
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
            let mut header = [0_u8; 2];
            client.read_exact(&mut header).await?;
            let length = u16::from_le_bytes(header) as usize;
            assert!(length >= 4, "valid Crystal frame length");
            let mut frame = vec![0_u8; length];
            frame[..2].copy_from_slice(&header);
            client.read_exact(&mut frame[2..]).await?;
            Ok::<_, std::io::Error>(frame)
        })
        .await
        .expect("expected TCP response before deadline")
        .expect("public TCP connection stays open");
        let packet = decode_server_packet(&frame).expect("valid server Crystal packet");
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
            .expect("expected WebSocket response before deadline")
            .expect("public WebSocket stays open")
            .expect("valid WebSocket message");
        let event = match message {
            Message::Text(text) => {
                serde_json::from_str::<Value>(&text).expect("valid public JSON event")
            }
            Message::Ping(bytes) => {
                client.send(Message::Pong(bytes)).await.unwrap();
                continue;
            }
            Message::Pong(_) => continue,
            other => panic!("unexpected WebSocket frame {other:?}"),
        };
        assert_ne!(event["type"], "error", "public command failed: {event}");
        let done = expected(&event);
        events.push(event);
        if done {
            return events;
        }
    }
}
fn packet(event: &Value, name: &str) -> bool {
    event["type"] == "packet" && event["packet"] == name
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_conquest_tcp_and_web_join_one_palace_capture_and_publish_owner_npc_status() {
    let _environment = IsolatedEnvironment::new();
    let (config, start) = server_fixture();
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
    let registry = Arc::new(ZoneRegistry::new(ZoneId::primary(), factory.clone()));
    let hub = ChatBroadcastHub::from_env().unwrap();
    let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let web_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tcp_address = tcp_listener.local_addr().unwrap();
    let web_address = web_listener.local_addr().unwrap();
    let servers = ServerTasks(vec![
        tokio::spawn(mir2_gateway::tcp::serve_tcp_gateway_with_zone_registry(
            tcp_listener,
            config.clone(),
            hub.clone(),
            registry.clone(),
        )),
        tokio::spawn(mir2_gateway::web::serve_web_gateway_with_zone_registry(
            web_listener,
            config.clone(),
            hub.clone(),
            registry,
        )),
    ]);
    let mut tcp = TcpStream::connect(tcp_address).await.unwrap();
    tcp_send(
        &mut tcp,
        ClientPacket::Login {
            account_id: "tcp_siege".into(),
            password: "transport-test-password".into(),
        },
    )
    .await;
    tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::LoginSuccess { .. })
    })
    .await;
    tcp_send(&mut tcp, ClientPacket::StartGame { character_index: 1 }).await;
    let entered = tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::UserInformation { .. })
    })
    .await;
    assert!(entered
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let mut request = format!("ws://{web_address}/ws")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", format!("http://{web_address}").parse().unwrap());
    let (mut web, response) = tokio_tungstenite::connect_async(request)
        .await
        .expect("real public Axum upgrade");
    assert_eq!(response.status(), 101);
    web_send(
        &mut web,
        json!({"type":"login", "accountId":"web_siege", "password":"transport-test-password"}),
    )
    .await;
    web_until(&mut web, |event| packet(event, "LoginSuccess")).await;
    web_send(&mut web, json!({"type":"setLanguage", "language":"en"})).await;
    web_send(&mut web, json!({"type":"startGame", "characterIndex":2})).await;
    let entered = web_until(&mut web, |event| {
        packet(event, "ObjectPlayer") && event["payload"]["name"] == "TcpKnight"
    })
    .await;
    assert!(entered
        .iter()
        .any(|event| packet(event, "StartGame") && event["payload"]["result"] == 4));
    tcp_until(
        &mut tcp,
        |packet| matches!(packet, ServerPacket::ObjectPlayer { info } if info.name == "WebKnight"),
    )
    .await;
    assert_eq!(
        factory.active_zone_count(),
        1,
        "one shared runtime across public transports"
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while hub.online_count() != 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("both real sessions acquire public chat presence");
    assert_eq!(hub.online_count(), 2);

    // Invoke the same server clock entry production owns, without any client
    // debug command. Its presence comes from the two actual socket joins.
    let committed = factory
        .advance_shared_conquest(&config, start, CLOCK)
        .unwrap();
    assert!(committed
        .iter()
        .any(|event| event.kind == ConquestEventKind::WarStarted));
    assert!(committed
        .iter()
        .any(|event| event.kind == ConquestEventKind::Captured));
    let siege = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
    assert_eq!(siege.owner_guild_id.as_deref(), Some(GUILD));
    tcp_until(&mut tcp, |packet| matches!(packet, ServerPacket::Chat { message, chat_type: ChatType::Announcement } if message == "Castle captured by Knights.")).await;
    web_until(&mut web, |event| {
        packet(event, "Chat")
            && event["payload"]["message"] == "Castle captured by Knights."
            && event["payload"]["chatType"] == "Announcement"
    })
    .await;

    web_send(&mut web, json!({"type":"interact", "objectId":1146})).await;
    let shown = web_until(&mut web, |event| {
        event["type"] == "worldSnapshot"
            && event["payload"]["activeNpcDialog"]["npcObjectId"] == 1146
    })
    .await;
    let page = &shown.last().unwrap()["payload"]["activeNpcDialog"];
    assert!(page["body"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "Castle owner: Knights"));
    assert!(page["links"]
        .as_array()
        .unwrap()
        .iter()
        .any(|link| link["target"] == "@sabuk:manage"));
    assert!(
        factory
            .advance_shared_conquest(&config, start + 10_000, CLOCK)
            .unwrap()
            .is_empty(),
        "a committed capture cannot replay on the next clock tick"
    );
    web.close(None).await.unwrap();
    tcp.shutdown().await.unwrap();
    drop(tcp);
    drop(web);
    tokio::time::timeout(Duration::from_secs(10), async {
        while hub.online_count() != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("socket cleanup completes before isolated environment is restored");
    drop(servers);
}

//! Prepared account/Guild/map authority with actual source monster HP/XP.
//! Real TCP and WebSocket clients form a party, kill through ordinary Attack,
//! leave their Guild and relogin. No client QA command or public save/token is
//! used; this isolated transport check does not accept native/public gameplay.
use futures_util::{SinkExt, StreamExt};
use mir2_gateway::tcp::chat_broadcast::ChatBroadcastHub;
use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRegistry};
use mir2_protocol::{
    decode_server_packet, encode_client_packet, ClientPacket, MirClass, MirDirection, MirGender,
    Point, ServerPacket, Spell,
};
use mir2_simulation::{
    AccountRecord, CharacterRecord, MonsterSpawnSource, SharedGuildMember, SharedGuildRank,
    SharedGuildRecord, Stage5FriendIdentity, WorldEntityDisposition, ZoneMonsterDefense,
    ZoneMonsterSpawn,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

const GUILD: &str = "1123456789abcdef0123456789abcdef";
const MAP: &str = "xp-transport";
const PASSWORD: &str = "transport-test-password";
// Crystal Build/Server/Debug/Configs/ExpList.ini, Level20=140000.
const SOURCE_LEVEL_20_MAX_EXPERIENCE: i64 = 140_000;
const TEST_RECOVERY_MAC_KEY: [u8; 32] = [
    0x70, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe, 0x0f,
    0x71, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xf1, 0x02,
];
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
            "mir2-experience-transport-{}-{unique}",
            std::process::id()
        ));
        std::env::set_var("MIR2_RUNTIME_ENV", "test");
        std::env::set_var(
            "MIR2_IDENTITY_SESSION_SECRET",
            "isolated-experience-identity-session-secret-32",
        );
        std::env::set_var(
            "MIR2_IDENTITY_RECOVERY_PEPPER",
            "isolated-experience-identity-recovery-pepper-32",
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

fn server_fixture(environment: &IsolatedEnvironment) -> GatewayConfig {
    let mut config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_path(environment.data_dir.join("accounts.json"))
        .with_save_recovery_dir(environment.data_dir.join("save-recovery"))
        .with_save_recovery_mac_key(TEST_RECOVERY_MAC_KEY)
        .expect("isolated file store requires a dedicated test recovery key");
    config.monster_spawn_source = MonsterSpawnSource::StarterScenario;
    config.map.file_name = MAP.into();
    config.map.title = "Original experience transport fixture".into();
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
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for (account, index, name, x) in [
            ("xp_tcp", 1, "TcpMember", 10),
            ("xp_web", 2, "WebLeader", 12),
        ] {
            let mut record = AccountRecord::new(CharacterRecord {
                index,
                name: name.into(),
                level: 20,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            });
            record.password = PASSWORD.into();
            let save = record.saves.get_mut(&index).unwrap();
            save.max_experience = SOURCE_LEVEL_20_MAX_EXPERIENCE;
            save.map_file_name = MAP.into();
            save.map_title = config.map.title.clone();
            save.position = Point { x, y: 10 };
            store.accounts.insert(account.into(), record);
        }
        store.shared_guilds.insert(
            GUILD.into(),
            SharedGuildRecord {
                id: GUILD.into(),
                name: "Earned Guild".into(),
                revision: 1,
                level: 0,
                experience: 0,
                spare_points: 0,
                gold: 0,
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
                        identity: Stage5FriendIdentity {
                            account_id: "xp_tcp".into(),
                            character_index: 1,
                        },
                        name: "TcpMember".into(),
                        rank_index: 1,
                        membership_epoch: 7,
                    },
                    SharedGuildMember {
                        identity: Stage5FriendIdentity {
                            account_id: "xp_web".into(),
                            character_index: 2,
                        },
                        name: "WebLeader".into(),
                        rank_index: 0,
                        membership_epoch: 8,
                    },
                ],
                notice: Vec::new(),
                storage: BTreeMap::new(),
                buffs: BTreeMap::new(),
                last_buff_tick_ms: 0,
                experience_receipts: Default::default(),
                experience_receipt_payloads: Default::default(),
                active_wars: Default::default(),
            },
        );
    }
    config.save_account_store().unwrap();
    config
}

fn spawn_original_monster(factory: &SharedInProcessZoneRuntimeFactory, object_id: u32) -> u32 {
    spawn_original_monster_at(factory, object_id, Point { x: 11, y: 10 })
}

fn spawn_original_monster_at(
    factory: &SharedInProcessZoneRuntimeFactory,
    object_id: u32,
    position: Point,
) -> u32 {
    let source = mir2_game_data::crystal_monster_by_name("BoneWarrior").unwrap();
    assert_eq!((source.level, source.hp, source.experience), (19, 105, 180));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert_eq!(
        factory
            .apply_world_event_monsters(
                &ZoneId::primary(),
                MAP,
                &[ZoneMonsterSpawn {
                    crystal_drop_seed: None,
                    object_id,
                    name: source.name.clone(),
                    name_colour_argb: -1,
                    image: source.image,
                    ai: source.ai,
                    disposition: Some(WorldEntityDisposition::Hostile),
                    level: source.level,
                    max_hp: source.hp,
                    hp: source.hp,
                    experience: source.experience,
                    // A prepared static encounter isolates transport/XP from wandering AI.
                    move_speed_ms: 60_000,
                    attack_speed_ms: 60_000,
                    friendly_guild: None,
                    position,
                    direction: MirDirection::Left,
                    defense: ZoneMonsterDefense::from_crystal_template(&source),
                    respawn: None,
                    drops: Vec::new(),
                }],
                now
            )
            .unwrap(),
        1
    );
    source.experience
}

async fn normal_tcp_kill(
    client: &mut TcpStream,
    factory: &SharedInProcessZoneRuntimeFactory,
    object_id: u32,
    expected: u32,
    direction: MirDirection,
) -> Vec<ServerPacket> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    while factory
        .world_event_monster_snapshots(&ZoneId::primary(), MAP)
        .unwrap()
        .iter()
        .any(|monster| monster.object_id == object_id && !monster.dead)
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "ordinary source-stat attacks must kill original HP"
        );
        tcp_send(
            client,
            ClientPacket::Attack {
                direction,
                spell: Spell::None,
            },
        )
        .await;
        tokio::time::sleep(Duration::from_millis(650)).await;
    }
    // Death display and the durable personal receipt can be scheduled on
    // separate gateway lanes. Require both without assuming which is first.
    let received_experience = std::cell::Cell::new(false);
    let received_death = std::cell::Cell::new(false);
    let packets = tcp_until(client, |packet| {
        if matches!(packet, ServerPacket::GainExperience { amount } if *amount == expected) {
            received_experience.set(true);
        }
        if matches!(packet, ServerPacket::ObjectDied { info } if info.object_id == object_id) {
            received_death.set(true);
        }
        received_experience.get() && received_death.get()
    })
    .await;
    packets
}

fn durable_experience(config: &GatewayConfig, account: &str, index: i32) -> u32 {
    config.account_store.lock().unwrap().accounts[account].saves[&index].experience as u32
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
        .unwrap_or_else(|_| {
            let kinds = packets
                .iter()
                .map(mir2_protocol::server_packet_name)
                .collect::<Vec<_>>();
            panic!("expected TCP response before deadline; received kinds: {kinds:?}")
        })
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
        let received = tokio::time::timeout_at(deadline, client.next()).await;
        let message = received
            .unwrap_or_else(|_| {
                let kinds = events
                    .iter()
                    .map(|event: &Value| {
                        (
                            event["type"].as_str().unwrap_or(""),
                            event["packet"].as_str().unwrap_or(""),
                        )
                    })
                    .collect::<Vec<_>>();
                panic!("expected WebSocket response before deadline; received kinds: {kinds:?}")
            })
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
async fn normal_tcp_web_party_kills_capture_guild_membership_and_survive_leave_relogin() {
    let environment = IsolatedEnvironment::new();
    let config = server_fixture(&environment);
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
            account_id: "xp_tcp".into(),
            password: PASSWORD.into(),
        },
    )
    .await;
    tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::LoginSuccess { .. })
    })
    .await;
    tcp_send(&mut tcp, ClientPacket::StartGame { character_index: 1 }).await;
    let started = tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::UserInformation { .. })
    })
    .await;
    assert!(started.iter().any(|packet| {
        matches!(packet, ServerPacket::UserInformation { info }
            if info.level == 20 && info.max_experience == SOURCE_LEVEL_20_MAX_EXPERIENCE)
    }));
    let mut request = format!("ws://{web_address}/ws")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", format!("http://{web_address}").parse().unwrap());
    let (mut web, upgrade) = connect_async(request).await.unwrap();
    assert_eq!(upgrade.status(), 101);
    web_send(
        &mut web,
        json!({"type":"login","accountId":"xp_web","password":PASSWORD}),
    )
    .await;
    web_until(&mut web, |event| packet(event, "LoginSuccess")).await;
    web_send(&mut web, json!({"type":"startGame","characterIndex":2})).await;
    web_until(&mut web, |event| {
        packet(event, "ObjectPlayer") && event["payload"]["name"] == "TcpMember"
    })
    .await;
    tcp_until(
        &mut tcp,
        |packet| matches!(packet, ServerPacket::ObjectPlayer { info } if info.name == "WebLeader"),
    )
    .await;
    assert_eq!(factory.active_zone_count(), 1);

    // StartGame also sends this packet. Alternating the values gives each
    // permission command a real ordered acknowledgement across transports.
    tcp_send(&mut tcp, ClientPacket::SwitchGroup { allow_group: false }).await;
    tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::SwitchGroup { allow_group: false })
    })
    .await;
    tcp_send(&mut tcp, ClientPacket::SwitchGroup { allow_group: true }).await;
    tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::SwitchGroup { allow_group: true })
    })
    .await;
    web_send(&mut web, json!({"type":"switchGroup","allowGroup":false})).await;
    web_until(&mut web, |event| {
        packet(event, "SwitchGroup") && event["payload"]["allowGroup"] == false
    })
    .await;
    web_send(&mut web, json!({"type":"switchGroup","allowGroup":true})).await;
    web_until(&mut web, |event| {
        packet(event, "SwitchGroup") && event["payload"]["allowGroup"] == true
    })
    .await;
    tcp_send(
        &mut tcp,
        ClientPacket::AddMember {
            name: "WebLeader".into(),
        },
    )
    .await;
    let invitation_result = tcp_until(&mut tcp, |packet| {
        matches!(
            packet,
            ServerPacket::SwitchGroup { allow_group: true } | ServerPacket::Chat { .. }
        )
    })
    .await;
    let rejections = invitation_result
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::Chat { message, .. } => Some(message.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        rejections.is_empty(),
        "normal AddMember refused: {rejections:?}"
    );
    web_until(&mut web, |event| {
        packet(event, "GroupInvite") && event["payload"]["name"] == "TcpMember"
    })
    .await;
    web_send(&mut web, json!({"type":"groupInvite","acceptInvite":true})).await;
    web_until(&mut web, |event| {
        packet(event, "AddMember") && event["payload"]["name"] == "TcpMember"
    })
    .await;
    tcp_until(
        &mut tcp,
        |packet| matches!(packet, ServerPacket::AddMember { name } if name == "WebLeader"),
    )
    .await;

    let raw = spawn_original_monster(&factory, 7100);
    let owner_raw = raw * config.monster_experience_multiplier(20);
    let share = (owner_raw as f32 * 1.3 * 20.0 / 40.0) as u32;
    let guild_amount =
        (share as f32 * mir2_game_data::crystal_guild_settings().experience_rate) as u32;
    assert!(guild_amount > 0 && SOURCE_LEVEL_20_MAX_EXPERIENCE > i64::from(owner_raw + 2 * share));
    tcp_until(
        &mut tcp,
        |packet| matches!(packet, ServerPacket::ObjectMonster { info } if info.object_id == 7100),
    )
    .await;
    web_until(&mut web, |event| {
        packet(event, "ObjectMonster") && event["payload"]["objectId"] == 7100
    })
    .await;
    normal_tcp_kill(&mut tcp, &factory, 7100, share, MirDirection::Right).await;
    web_until(&mut web, |event| {
        packet(event, "GainExperience") && event["payload"]["amount"] == share
    })
    .await;
    assert_eq!(durable_experience(&config, "xp_tcp", 1), share);
    assert_eq!(durable_experience(&config, "xp_web", 2), share);
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        2 * u64::from(guild_amount)
    );

    // Original party and AOI ranges are sixteen in both axes. Put the peer
    // sixteen below its party owner, and the next monster one above the owner:
    // the peer is seventeen from that monster and sees no death/AOI wake-up.
    for y in 11..=26 {
        web_send(&mut web, json!({"type":"walk","direction":"Down"})).await;
        web_until(&mut web, |event| {
            packet(event, "UserLocation")
                && event["payload"]["x"] == 12
                && event["payload"]["y"] == y
        })
        .await;
    }

    tcp_send(
        &mut tcp,
        ClientPacket::Chat {
            message: "@LEAVEGUILD".into(),
            linked_items: Vec::new(),
        },
    )
    .await;
    tcp_until(&mut tcp, |packet| matches!(packet, ServerPacket::GuildStatus { guild_name, .. } if guild_name.is_empty())).await;
    assert!(config
        .shared_guild_for_identity(&Stage5FriendIdentity {
            account_id: "xp_tcp".into(),
            character_index: 1
        })
        .unwrap()
        .is_none());
    spawn_original_monster_at(&factory, 7101, Point { x: 10, y: 9 });
    assert_eq!(
        factory
            .world_event_monster_snapshots(&ZoneId::primary(), MAP)
            .unwrap()
            .iter()
            .find(|monster| monster.object_id == 7101)
            .unwrap()
            .position,
        Point { x: 10, y: 9 }
    );
    tcp_until(
        &mut tcp,
        |packet| matches!(packet, ServerPacket::ObjectMonster { info } if info.object_id == 7101),
    )
    .await;
    normal_tcp_kill(&mut tcp, &factory, 7101, share, MirDirection::Up).await;
    let distant_award = web_until(&mut web, |event| {
        packet(event, "GainExperience") && event["payload"]["amount"] == share
    })
    .await;
    let leaked_kinds: Vec<_> = distant_award
        .iter()
        .filter(|event| event["payload"]["objectId"] == 7101)
        .map(|event| event["packet"].as_str().unwrap_or(""))
        .collect();
    assert!(
        leaked_kinds.is_empty(),
        "distant party recipient saw outside-AOI monster packets: {leaked_kinds:?}"
    );
    assert_eq!(durable_experience(&config, "xp_tcp", 1), 2 * share);
    assert_eq!(durable_experience(&config, "xp_web", 2), 2 * share);
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        3 * u64::from(guild_amount)
    );

    tcp_send(&mut tcp, ClientPacket::LogOut).await;
    tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::LogOutSuccess { .. })
    })
    .await;
    web_until(&mut web, |event| packet(event, "DeleteGroup")).await;
    tcp.shutdown().await.unwrap();
    drop(tcp);
    tokio::time::timeout(Duration::from_secs(10), async {
        while hub.online_count() != 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let before = config.account_store.lock().unwrap().shared_guilds[GUILD].experience;
    let mut tcp = TcpStream::connect(tcp_address).await.unwrap();
    tcp_send(
        &mut tcp,
        ClientPacket::Login {
            account_id: "xp_tcp".into(),
            password: PASSWORD.into(),
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
    assert!(!entered
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainExperience { .. })));
    assert_eq!(durable_experience(&config, "xp_tcp", 1), 2 * share);
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        before
    );
    // GroupMembers are transient Crystal player references. A cold relogin
    // receives no old party award even while the previous peer remains online.
    tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::DeleteGroup)
    })
    .await;
    spawn_original_monster(&factory, 7102);
    tcp_until(
        &mut tcp,
        |packet| matches!(packet, ServerPacket::ObjectMonster { info } if info.object_id == 7102),
    )
    .await;
    normal_tcp_kill(&mut tcp, &factory, 7102, owner_raw, MirDirection::Right).await;
    assert_eq!(
        durable_experience(&config, "xp_tcp", 1),
        2 * share + owner_raw
    );
    assert_eq!(durable_experience(&config, "xp_web", 2), 2 * share);
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[GUILD].experience,
        before
    );
    tcp_send(&mut tcp, ClientPacket::LogOut).await;
    tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::LogOutSuccess { .. })
    })
    .await;
    web_send(&mut web, json!({"type":"logOut"})).await;
    web_until(&mut web, |event| packet(event, "LogOutSuccess")).await;
    tcp.shutdown().await.unwrap();
    web.close(None).await.unwrap();
    drop(tcp);
    drop(web);
    tokio::time::timeout(Duration::from_secs(10), async {
        while hub.online_count() != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    drop(servers);
}

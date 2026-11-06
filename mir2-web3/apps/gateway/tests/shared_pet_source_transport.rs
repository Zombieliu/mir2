//! Ordinary TCP summon/attack/logout and a WebSocket observer. Accounts,
//! learned magic, equipped Amulet and HP1/XP20000 encounters are prepared;
//! every pet record is produced by the real summon and final GainExp source.
//! This isolated socket test is not native-client/public-play acceptance.
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
    SharedAccountInventoryTransactionReceipt, WorldEntityDisposition, ZoneMonsterSpawn,
    ZoneSavedPetSnapshot,
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

const MAP: &str = "pet-source-transport";
const OWNER: &str = "pet-source-owner";
const WATCHER: &str = "pet-source-observer";
const PASSWORD: &str = "isolated-pet-source-password";
type WebClient = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Environment {
    inherited: Vec<(OsString, OsString)>,
    directory: PathBuf,
}
impl Environment {
    fn new() -> Self {
        let inherited: Vec<_> = std::env::vars_os()
            .filter(|(name, _)| name.to_string_lossy().starts_with("MIR2_"))
            .collect();
        for (name, _) in &inherited {
            std::env::remove_var(name);
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("mir2-pet-source-{}-{nonce}", std::process::id()));
        std::env::set_var("MIR2_RUNTIME_ENV", "test");
        std::env::set_var(
            "MIR2_IDENTITY_SESSION_SECRET",
            "isolated-pet-source-session-secret-32",
        );
        std::env::set_var(
            "MIR2_IDENTITY_RECOVERY_PEPPER",
            "isolated-pet-source-recovery-pepper-32",
        );
        std::env::set_var("MIR2_AI_LIVE_DATA_DIR", &directory);
        Self {
            inherited,
            directory,
        }
    }
    fn store_path(&self) -> PathBuf {
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

fn scene(environment: &Environment) -> GatewayConfig {
    let mut config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_path(environment.store_path())
        .with_save_recovery_dir(environment.directory.join("recovery"))
        .with_save_recovery_mac_key(std::array::from_fn::<u8, 32, _>(|index| index as u8 + 101))
        .unwrap();
    config.default_character.class = MirClass::Taoist;
    config.default_character.level = 40;
    config.monster_spawn_source = MonsterSpawnSource::StarterScenario;
    config.map.file_name = MAP.into();
    config.map.title = "Pet source transport fixture".into();
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
fn fixture(environment: &Environment) -> GatewayConfig {
    let config = scene(environment);
    let amulet = mir2_game_data::crystal_item_by_name("Amulet").unwrap();
    let mut store = config.account_store.lock().unwrap();
    store.accounts.clear();
    for (account, index, name, class, x) in [
        (OWNER, 1, "PetSourceTaoist", MirClass::Taoist, 10),
        (WATCHER, 2, "PetSourceWitness", MirClass::Warrior, 14),
    ] {
        let mut record = AccountRecord::new(CharacterRecord {
            index,
            name: name.into(),
            level: 40,
            class,
            gender: MirGender::Male,
        });
        record.password = PASSWORD.into();
        let save = record.saves.get_mut(&index).unwrap();
        save.map_file_name = MAP.into();
        save.map_title = config.map.title.clone();
        save.position = Point { x, y: 10 };
        save.mp = 200;
        save.max_mp = 200;
        // CharacterSaveRecord::new has the legacy level-one default100.
        // This prepared level40 account uses the real Crystal ExpList row.
        save.max_experience = 12_000_000;
        assert!(
            save.saved_pets.is_empty(),
            "no prepared durable pet is admitted"
        );
        if account == OWNER {
            save.skill_states_json = vec![
                json!({"key":"summonskeleton","name":"SummonSkeleton","description":"",
                "level":0,"experience":0,"cooldown_ticks":0,"cooldown_ends_at":0})
                .to_string(),
            ];
            save.equipment_items_json=vec![json!({"key":format!("crystal-item-{}",amulet.item_index),
                "slot":mir2_simulation::EquipmentSlot::Amulet,"quantity":20,"name":amulet.name,
                "icon":amulet.image,"shape":amulet.shape,"description":"",
                "durability_current":amulet.durability.max(1),"durability_max":amulet.durability.max(1),
                "attack":0,"defence":0}).to_string()];
        }
        store.accounts.insert(account.into(), record);
    }
    drop(store);
    config
}

// Only test-support injects the fault. This service delegates the actual
// native source to the production commit implementation, with its real fence.
#[derive(Debug)]
struct FaultSource {
    config: GatewayConfig,
    path: PathBuf,
    inner: InProcessAccountInventoryService,
    fault_done: AtomicBool,
    allow_retry: AtomicBool,
    confirmed: AtomicUsize,
}
impl FaultSource {
    fn new(config: GatewayConfig, path: PathBuf) -> Self {
        Self {
            config,
            path,
            inner: InProcessAccountInventoryService::new(),
            fault_done: AtomicBool::new(false),
            allow_retry: AtomicBool::new(false),
            confirmed: AtomicUsize::new(0),
        }
    }
}
impl SharedAccountInventoryService for FaultSource {
    fn commit(
        &self,
        runtime: &mut InProcessWorldRuntime,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        let outcome = self.commit_fenced(runtime, None, envelope);
        (*outcome).clone()
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
        let identity = envelope.identity.clone();
        let before = is_kill.then(|| {
            serde_json::to_value(
                &self.config.account_store.lock().unwrap().accounts[&identity.account_id].saves
                    [&identity.character_index],
            )
            .unwrap()
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
                !matches!(&outcome,SharedAccountInventoryCommitOutcome::Confirmed(receipt) if receipt.committed)
            );
            let after = serde_json::to_value(
                &self.config.account_store.lock().unwrap().accounts[&identity.account_id].saves
                    [&identity.character_index],
            )
            .unwrap();
            assert!(
                before.as_ref() == Some(&after),
                "known source failure restores the complete durable character"
            );
            self.fault_done.store(true, Ordering::Release);
        } else if is_kill
            && matches!(&outcome,SharedAccountInventoryCommitOutcome::Confirmed(receipt) if receipt.committed)
        {
            let durable: AccountStore =
                serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap();
            let live = self.config.account_store.lock().unwrap();
            let saved = &live.accounts[&identity.account_id].saves[&identity.character_index];
            let file = &durable.accounts[&identity.account_id].saves[&identity.character_index];
            assert!(
                saved.saved_pets == file.saved_pets && saved.experience == file.experience,
                "actual pet growth and Human XP must reach disk before a confirmed ACK is returned"
            );
            assert!(
                !saved.saved_pets.is_empty(),
                "normal source produces the summoned pet record"
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
                "missing TCP response; packet kinds: {:?}",
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
                    "missing observer response; packet kinds: {:?}",
                    events
                        .iter()
                        .map(|event: &Value| event["packet"].as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                )
            })
            .unwrap()
            .unwrap();
        let event = match message {
            Message::Text(text) => serde_json::from_str::<Value>(&text).unwrap(),
            Message::Ping(bytes) => {
                client.send(Message::Pong(bytes)).await.unwrap();
                continue;
            }
            Message::Pong(_) => continue,
            _ => panic!("observer connection closed unexpectedly"),
        };
        assert!(event["type"] != "error", "ordinary observer command failed");
        let done = expected(&event);
        events.push(event);
        if done {
            return events;
        }
    }
}
fn is_packet(event: &Value, name: &str) -> bool {
    event["type"] == "packet" && event["packet"] == name
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn spawn(factory: &SharedInProcessZoneRuntimeFactory, id: u32, experience: u32) -> MirDirection {
    // Explicit prepared encounter, matching the P4 actual-source fault test.
    // No pet authority, growth or saved record is prepared here.
    // The owner remains at the ordinary StartGame spawn throughout this story.
    // Choose a real free adjacent cell after the delayed pet has appeared.
    let occupied = factory
        .world_event_monster_snapshots(&ZoneId::primary(), MAP)
        .unwrap();
    let (position, direction) = [
        (MirDirection::Up, Point { x: 10, y: 9 }),
        (MirDirection::Right, Point { x: 11, y: 10 }),
        (MirDirection::Left, Point { x: 9, y: 10 }),
        (MirDirection::Down, Point { x: 10, y: 11 }),
    ]
    .into_iter()
    .find_map(|(direction, point)| {
        (!occupied
            .iter()
            .any(|monster| !monster.dead && monster.position == point))
        .then_some((point, direction))
    })
    .expect("the actual summoned pet leaves an adjacent encounter cell free");
    assert_eq!(
        factory
            .apply_world_event_monsters(
                &ZoneId::primary(),
                MAP,
                &[ZoneMonsterSpawn {
                    crystal_drop_seed: None,
                    object_id: id,
                    name: "Scarecrow".into(),
                    name_colour_argb: -1,
                    image: 0,
                    ai: 0,
                    disposition: Some(WorldEntityDisposition::Hostile),
                    level: 40,
                    hp: 1,
                    max_hp: 1,
                    experience,
                    move_speed_ms: 60_000,
                    attack_speed_ms: 60_000,
                    friendly_guild: None,
                    position,
                    direction: MirDirection::Down,
                    defense: Default::default(),
                    respawn: None,
                    drops: vec![],
                }],
                now_ms()
            )
            .unwrap(),
        1
    );
    direction
}
fn saved(config: &GatewayConfig) -> ZoneSavedPetSnapshot {
    config.account_store.lock().unwrap().accounts[OWNER].saves[&1]
        .saved_pets
        .clone()
}
fn human_progress(config: &GatewayConfig) -> (u16, i64) {
    let store = config.account_store.lock().unwrap();
    let save = &store.accounts[OWNER].saves[&1];
    (save.character.level, save.experience)
}
fn pet_fields(snapshot: &ZoneSavedPetSnapshot) -> (i32, i32, u32, u8, u8) {
    snapshot.validate_canonical().unwrap();
    let value = serde_json::to_value(snapshot.for_durable_storage()).unwrap();
    let pets = value["pets"].as_array().unwrap();
    assert_eq!(pets.len(), 1);
    let pet = pets[0].as_object().unwrap();
    assert_eq!(pet.len(), 5, "only canonical PetInfo fields are durable");
    (
        pet["monster_index"].as_i64().unwrap() as i32,
        pet["hp"].as_i64().unwrap() as i32,
        pet["experience"].as_u64().unwrap() as u32,
        pet["level"].as_u64().unwrap() as u8,
        pet["max_pet_level"].as_u64().unwrap() as u8,
    )
}
async fn login_tcp(client: &mut TcpStream) -> (u32, Vec<ServerPacket>) {
    tcp_send(
        client,
        ClientPacket::Login {
            account_id: OWNER.into(),
            password: PASSWORD.into(),
        },
    )
    .await;
    tcp_until(client, |packet| {
        matches!(packet, ServerPacket::LoginSuccess { .. })
    })
    .await;
    tcp_send(client, ClientPacket::StartGame { character_index: 1 }).await;
    let packets = tcp_until(client, |packet| {
        matches!(packet, ServerPacket::UserInformation { .. })
    })
    .await;
    let owner = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::UserInformation { info } => Some(info.object_id),
            _ => None,
        })
        .unwrap();
    (owner, packets)
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
    let servers = Servers(vec![
        tokio::spawn(mir2_gateway::tcp::serve_tcp_gateway_with_zone_registry(
            tcp,
            config.clone(),
            hub.clone(),
            registry.clone(),
        )),
        tokio::spawn(mir2_gateway::web::serve_web_gateway_with_zone_registry(
            web, config, hub, registry,
        )),
    ]);
    (servers, tcp_address, web_address)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn normal_summon_pet_gain_is_durable_before_ack_known_fault_retries_and_cold_login_restores()
{
    let environment = Environment::new();
    let (snapshot, weak_store) = {
        let config = fixture(&environment);
        let weak_store = Arc::downgrade(&config.account_store);
        let service = Arc::new(FaultSource::new(config.clone(), environment.store_path()));
        let factory = Arc::new(
            SharedInProcessZoneRuntimeFactory::with_account_inventory_service(service.clone()),
        );
        let (servers, tcp_address, web_address) =
            start_servers(config.clone(), factory.clone()).await;
        let mut tcp = TcpStream::connect(tcp_address).await.unwrap();
        let (owner, _) = login_tcp(&mut tcp).await;
        let mut request = format!("ws://{web_address}/ws")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", format!("http://{web_address}").parse().unwrap());
        let (mut web, _) = connect_async(request).await.unwrap();
        web_send(
            &mut web,
            json!({"type":"login","accountId":WATCHER,"password":PASSWORD}),
        )
        .await;
        web_until(&mut web, |event| is_packet(event, "LoginSuccess")).await;
        web_send(&mut web, json!({"type":"startGame","characterIndex":2})).await;
        let observer_players = web_until(&mut web, |event| {
            is_packet(event, "ObjectPlayer") && event["payload"]["name"] == "PetSourceTaoist"
        })
        .await;
        let observed_owner = observer_players
            .iter()
            .find(|event| {
                is_packet(event, "ObjectPlayer") && event["payload"]["name"] == "PetSourceTaoist"
            })
            .unwrap()["payload"]["objectId"]
            .as_u64()
            .unwrap();
        tcp_send(
            &mut tcp,
            ClientPacket::Magic {
                object_id: owner,
                spell: Spell::SummonSkeleton,
                direction: MirDirection::Right,
                target_id: 0,
                location: Point { x: 11, y: 10 },
                spell_target_lock: false,
            },
        )
        .await;
        let cast = tcp_until(&mut tcp, |packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    spell: Spell::SummonSkeleton,
                    cast: true,
                    ..
                }
            )
        })
        .await;
        assert!(cast
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainExperience { .. })));
        let quantity = config.account_store.lock().unwrap().accounts[OWNER].saves[&1]
            .equipment_items_json
            .iter()
            .map(|item| serde_json::from_str::<Value>(item).unwrap())
            .find(|item| item["name"] == "Amulet")
            .unwrap()["quantity"]
            .as_u64()
            .unwrap();
        assert_eq!(
            quantity, 19,
            "normal summon durably consumes its actual equipped Amulet"
        );
        let appeared=tcp_until(&mut tcp,|packet|matches!(packet,ServerPacket::ObjectMonster{info} if info.name=="BoneFamiliar")).await;
        let master = appeared
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::ObjectMonster { info } if info.name == "BoneFamiliar" => {
                    Some(info.master_object_id)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(
            master, owner,
            "ordinary owner pet packet addresses the current rendered Human"
        );
        let pet = appeared
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::ObjectMonster { info } if info.name == "BoneFamiliar" => {
                    Some(info.object_id)
                }
                _ => None,
            })
            .unwrap();
        let observer_pet = web_until(&mut web, |event| {
            is_packet(event, "ObjectMonster") && event["payload"]["name"] == "BoneFamiliar"
        })
        .await;
        let observed_master = observer_pet
            .iter()
            .find(|event| {
                is_packet(event, "ObjectMonster") && event["payload"]["name"] == "BoneFamiliar"
            })
            .unwrap()["payload"]["masterObjectId"]
            .as_u64()
            .unwrap();
        assert_eq!(
            observed_master, observed_owner,
            "observer retains the same authoritative shared Human identity"
        );
        assert_ne!(observed_master, u64::from(owner));
        let canonical = mir2_game_data::crystal_monster_by_name("BoneFamiliar").unwrap();
        let initial = factory
            .world_event_monster_snapshots(&ZoneId::primary(), MAP)
            .unwrap()
            .into_iter()
            .find(|monster| monster.object_id == pet)
            .unwrap();
        assert_eq!(initial.max_hp, canonical.hp);
        tokio::time::sleep(Duration::from_millis(700)).await;
        let direction = spawn(&factory, 91_001, 20_000);
        tcp_send(
            &mut tcp,
            ClientPacket::Attack {
                direction,
                spell: Spell::None,
            },
        )
        .await;
        let failed = tcp_until(
            &mut tcp,
            |packet| matches!(packet,ServerPacket::ObjectDied{info} if info.object_id==91_001),
        )
        .await;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !service.fault_done.load(Ordering::Acquire) {
            assert!(
                tokio::time::Instant::now() < deadline,
                "real source reaches the injected publication fault"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(failed
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainExperience { .. })));
        tcp_send(&mut tcp, ClientPacket::SwitchGroup { allow_group: false }).await;
        let barrier = tcp_until(&mut tcp, |packet| {
            matches!(packet, ServerPacket::SwitchGroup { allow_group: false })
        })
        .await;
        assert!(
            barrier
                .iter()
                .all(|packet| !matches!(packet, ServerPacket::GainExperience { .. })),
            "pending source cannot acknowledge before successful persistence"
        );
        assert_eq!(human_progress(&config), (40, 0));
        let failed_pet = factory
            .world_event_monster_snapshots(&ZoneId::primary(), MAP)
            .unwrap()
            .into_iter()
            .find(|monster| monster.object_id == pet)
            .unwrap();
        assert_eq!(
            failed_pet.max_hp, initial.max_hp,
            "failed source cannot mirror pet growth"
        );
        service.allow_retry.store(true, Ordering::Release);
        let gained = tcp_until(&mut tcp, |packet| {
            matches!(packet, ServerPacket::GainExperience { amount: 20_000 })
        })
        .await;
        assert_eq!(
            gained
                .iter()
                .filter(|packet| matches!(packet, ServerPacket::GainExperience { .. }))
                .count(),
            1
        );
        let first = pet_fields(&saved(&config));
        assert_eq!(human_progress(&config), (40, 20_000));
        assert_eq!(
            (first.0, first.2, first.3, first.4),
            (canonical.monster_index, 40_000, 1, 4)
        );
        let grown = factory
            .world_event_monster_snapshots(&ZoneId::primary(), MAP)
            .unwrap()
            .into_iter()
            .find(|monster| monster.object_id == pet)
            .unwrap();
        assert_eq!((grown.hp, grown.max_hp), (first.1, canonical.hp + 20));
        web_until(&mut web, |event| {
            is_packet(event, "ObjectColourChanged") && event["payload"]["objectId"] == pet
        })
        .await;
        // A second real GainExp invocation of one advances only the next level
        // from the backlog: batching would incorrectly skip this source step.
        tokio::time::sleep(Duration::from_millis(700)).await;
        let direction = spawn(&factory, 91_002, 1);
        tcp_send(
            &mut tcp,
            ClientPacket::Attack {
                direction,
                spell: Spell::None,
            },
        )
        .await;
        tcp_until(&mut tcp, |packet| {
            matches!(packet, ServerPacket::GainExperience { amount: 1 })
        })
        .await;
        let second = pet_fields(&saved(&config));
        assert_eq!(human_progress(&config), (40, 20_001));
        assert_eq!((second.2, second.3, second.4), (3, 2, 4));
        assert_eq!(service.confirmed.load(Ordering::Acquire), 2);
        tcp_send(&mut tcp, ClientPacket::SwitchGroup { allow_group: true }).await;
        let replay = tcp_until(&mut tcp, |packet| {
            matches!(packet, ServerPacket::SwitchGroup { allow_group: true })
        })
        .await;
        assert!(
            replay
                .iter()
                .all(|packet| !matches!(packet, ServerPacket::GainExperience { .. })),
            "ordinary later drain does not replay the confirmed source"
        );
        tcp_send(&mut tcp, ClientPacket::LogOut).await;
        tcp_until(&mut tcp, |packet| {
            matches!(packet, ServerPacket::LogOutSuccess { .. })
        })
        .await;
        let snapshot = saved(&config);
        assert_eq!((pet_fields(&snapshot).2, pet_fields(&snapshot).3), (3, 2));
        assert!(
            !factory
                .world_event_monster_snapshots(&ZoneId::primary(), MAP)
                .unwrap()
                .iter()
                .any(|monster| monster.name == "BoneFamiliar"),
            "normal logout removes live owned nodes"
        );
        web_send(&mut web, json!({"type":"logOut"})).await;
        web_until(&mut web, |event| is_packet(event, "LogOutSuccess")).await;
        tcp.shutdown().await.unwrap();
        web.close(None).await.unwrap();
        drop(tcp);
        drop(web);
        servers.stop().await;
        drop(factory);
        drop(service);
        drop(config);
        (snapshot, weak_store)
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while weak_store.upgrade().is_some() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "cold phase releases every old authority/store owner"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let config = scene(&environment); // Real file load; no warm authority or prepared pet JSON.
    assert!(
        saved(&config) == snapshot,
        "cold load retains only canonical committed pet data"
    );
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
    let (servers, address, _) = start_servers(config.clone(), factory.clone()).await;
    let mut tcp = TcpStream::connect(address).await.unwrap();
    let (owner, started) = login_tcp(&mut tcp).await;
    assert!(started
        .iter()
        .all(|packet| !matches!(packet, ServerPacket::GainExperience { .. })));
    let restored=tcp_until(&mut tcp,|packet|matches!(packet,ServerPacket::ObjectMonster{info} if info.name=="BoneFamiliar" && info.master_object_id==owner)).await;
    let pet = restored
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "BoneFamiliar" => {
                Some(info.object_id)
            }
            _ => None,
        })
        .unwrap();
    let live = factory
        .world_event_monster_snapshots(&ZoneId::primary(), MAP)
        .unwrap()
        .into_iter()
        .find(|monster| monster.object_id == pet)
        .unwrap();
    let fields = pet_fields(&snapshot);
    let canonical = mir2_game_data::crystal_monster_by_index(fields.0).unwrap();
    assert_eq!(
        (live.hp, live.max_hp),
        (fields.1, canonical.hp + i32::from(fields.3) * 20)
    );
    assert!(
        saved(&config) == snapshot,
        "ordinary cold StartGame does not duplicate pet growth"
    );
    assert_eq!(human_progress(&config), (40, 20_001));
    tcp_send(&mut tcp, ClientPacket::LogOut).await;
    tcp_until(&mut tcp, |packet| {
        matches!(packet, ServerPacket::LogOutSuccess { .. })
    })
    .await;
    tcp.shutdown().await.unwrap();
    servers.stop().await;
}

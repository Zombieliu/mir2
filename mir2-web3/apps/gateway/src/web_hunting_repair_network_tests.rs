//! Physical loopback WebSocket + PostgreSQL acceptance. These ignored tests do
//! not start PostgreSQL, Docker, a deployed Gateway, Next or a browser. Account,
//! character and NPC actions use the ordinary authenticated socket route. Only
//! the damaged catalogue item and bound funding are trusted offline fixtures.
//!
//! Run one ignored case per process with an explicitly isolated loopback
//! MIR2_ECONOMY_TEST_DATABASE_URL (dbname mir2_hunting_acceptance_*), test identity
//! secrets, MIR2_HUNTING_ECONOMY_ENABLED=1, MIR2_REALM_ID and an explicit local
//! MIR2_ALLOWED_WEB_ORIGINS. No environment is changed by this module.
use super::{hunting_economy, ws_upgrade, GatewayCapacityState, ReconnectSessionStore, WebState};
use crate::cache::{GatewaySessionCache, GatewaySessionCacheKey, InMemoryGatewaySessionCache};
use crate::routing::{SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRegistry};
use futures_util::{SinkExt, StreamExt};
use mir2_client_wire::{hunting_repair as wire, U64};
use mir2_economy_policy::Policy;
use mir2_economy_service as economy;
use mir2_protocol::{MirDirection, Point, UserItem};
use mir2_simulation::{CharacterSaveRecord, SimulationConfig, VisibleNpcRecord};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{Seek, SeekFrom, Write},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};

const ACCOUNT: &str = "repairnet_user";
const PASSWORD: &str = "RepairNetwork9!";
const NPC_ID: u32 = 4990;
const FUNDING: u64 = 1_000_000;
const LEGACY_GOLD: u32 = 1234;
const BAG_UID: u64 = 900_001;
// Separate synthetic fixture key; satisfy the same diversity guard as the
// deployed recovery store rather than weakening that guard for acceptance.
const RECOVERY_KEY: [u8; 32] = [
    0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe, 0x0f,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xf1, 0x02,
];
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

// postgres::Client drives a private runtime. Leave the Tokio context for every
// fixture call, including configuration loading; block_in_place alone does not.
fn outside_runtime<T: Send>(action: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        scope
            .spawn(action)
            .join()
            .unwrap_or_else(|_| panic!("isolated fixture worker failed"))
    })
}

fn policy(enabled: bool) -> Policy {
    serde_json::from_value(json!({"schema_version":1,
        "policy_version":if enabled {"repair-network-enabled-v1"} else {"repair-network-disabled-v1"},
        "enabled":enabled,"k":{"numerator":1,"denominator":10},
        "conversion_fee":{"numerator":2,"denominator":100},
        "daily_decay":{"numerator":1,"denominator":150},
        "baseline_daily_mint_atoms":720000,
        "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},
        "business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap()
}

struct Database {
    url: String,
    realm: String,
    schema: String,
    root: PathBuf,
}
impl Database {
    fn new() -> Self {
        let base = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
            .expect("explicit isolated PostgreSQL URL required for ignored network acceptance");
        assert!(base.starts_with("postgres://") || base.starts_with("postgresql://"));
        let parsed: postgres::Config = base
            .parse()
            .unwrap_or_else(|_| panic!("invalid isolated PostgreSQL configuration"));
        assert!(
            parsed
                .get_dbname()
                .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")),
            "acceptance database prefix required"
        );
        assert!(!parsed.get_hosts().is_empty() && parsed.get_hosts().iter().all(|host|
            matches!(host,postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost")
                || name.parse::<IpAddr>().is_ok_and(|ip|ip.is_loopback()))), "loopback PostgreSQL required");
        assert!(parsed.get_hostaddrs().iter().all(IpAddr::is_loopback));
        assert!(
            matches!(
                std::env::var("MIR2_HUNTING_ECONOMY_ENABLED").as_deref(),
                Ok("1" | "true")
            ),
            "explicit test hunting HTTP runtime required"
        );
        let realm = std::env::var("MIR2_REALM_ID").expect("explicit test realm required");
        assert!(!realm.is_empty() && realm.trim() == realm && realm.len() <= 128);
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = format!(
            "{}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let schema = format!("repair_network_{nonce}");
        let root = std::env::temp_dir().join(format!("mir2-hunting-repair-network-{nonce}"));
        let separator = if base.contains('?') { '&' } else { '?' };
        let url = format!("{base}{separator}options=-csearch_path%3D{schema}");
        outside_runtime(|| {
            let mut client = postgres::Client::connect(&base, postgres::NoTls)
                .unwrap_or_else(|_| panic!("isolated PostgreSQL connection failed"));
            client
                .batch_execute(&format!("CREATE SCHEMA {schema}"))
                .unwrap();
            let mut scoped = postgres::Client::connect(&url, postgres::NoTls)
                .unwrap_or_else(|_| panic!("isolated schema connection failed"));
            assert_eq!(
                scoped
                    .query_one("SELECT current_schema()", &[])
                    .unwrap()
                    .get::<_, String>(0),
                schema
            );
            mir2_simulation::apply_migrations(&mut scoped).unwrap();
            economy::initialize_realm(&mut scoped, &realm, &policy(true)).unwrap();
        });
        Self {
            url,
            realm,
            schema,
            root,
        }
    }

    fn run<T: Send>(&self, action: impl FnOnce(&mut postgres::Client) -> T + Send) -> T {
        outside_runtime(|| {
            let mut client = postgres::Client::connect(&self.url, postgres::NoTls)
                .unwrap_or_else(|_| panic!("isolated database probe connection failed"));
            action(&mut client)
        })
    }

    fn config(&self) -> SimulationConfig {
        outside_runtime(|| {
            let mut config = SimulationConfig::default();
            config.visible_monsters.clear();
            config.visible_players.clear();
            config.visible_npcs.push(VisibleNpcRecord {
                object_id: NPC_ID,
                name: "Synthetic network repair NPC".into(),
                image: 5,
                colour_argb: -1,
                position: Point { x: 331, y: 271 },
                direction: MirDirection::Left,
                quest_ids: Vec::new(),
                script_key: Some("BichonProvince/NaturalCave/WickedTrader".into()),
            });
            config
                .with_postgres_account_store(&self.url)
                .unwrap_or_else(|_| panic!("isolated account source configuration failed"))
                .with_hunting_economy_realm(&self.realm)
                .unwrap()
                .with_save_recovery_dir(self.root.join("recovery"))
                .with_save_recovery_mac_key(RECOVERY_KEY)
                .unwrap()
        })
    }

    fn checkpoint(&self, index: i32) -> CharacterSaveRecord {
        self.run(|client| {
            serde_json::from_value(client.query_one(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2",
            &[&ACCOUNT, &index]).unwrap().get(0)).unwrap()
        })
    }

    fn prepare_offline_target(&self, config: &SimulationConfig, index: i32) {
        // The ordinary socket has created this account and character, but has
        // not entered a World. Persist through the existing scoped CAS writer;
        // this is not a client reward route or a live ECS/resource mutation.
        outside_runtime(|| {
            {
                let mut store = config.account_store.lock().unwrap();
                let save = store
                    .accounts
                    .get_mut(ACCOUNT)
                    .unwrap()
                    .saves
                    .get_mut(&index)
                    .unwrap();
                assert_eq!(save.gold, 0, "ordinary new-character starting balance");
                let target = json!({"key":"crystal-item-221","name":"WoodenSword","icon":0,
                    "slot":20,"unique_id":BAG_UID,"container":"bag1","quantity":1,
                    "description":"synthetic damaged catalogue item","durability_current":100,"durability_max":1000,
                    "weight":1,"equip_slot":"weapon","attack":1,"defence":0,"heal_hp":0,"heal_mp":0,
                    "identified":true,"soul_bound_id":-1,"added_stats":[],"socketed":[],
                    "user_item_metadata":{"item_index":221,"awake_type":0,"awake_values":[],
                        "refined_value":0,"refine_added":0,"refine_success_chance":0,"wedding_ring":-1,
                        "expire_info":null,"rental_information":null,"sealed_info":null,"slots":[],
                        "is_shop_item":false,"gm_made":false,"live_socketed_at_capture":false,
                        "socket_layout_hydrated":true,"captured_socket_positions":[]}});
                assert!(!save.inventory_items_json.iter().any(|item| {
                    let item: Value = serde_json::from_str(item).unwrap();
                    item["container"] == "bag1" && item["slot"] == 20
                }));
                save.inventory_items_json.push(target.to_string());
                save.gold = LEGACY_GOLD;
                save.revision = save.revision.checked_add(1).unwrap();
            }
            config
                .save_account_store_account(ACCOUNT)
                .unwrap_or_else(|_| panic!("offline scoped checkpoint CAS failed"));
        });
        self.run(|client| {
            let incarnation: String = client.query_one(
                "SELECT hunting_incarnation::text FROM characters WHERE account_id=$1 AND character_index=$2",
                &[&ACCOUNT, &index]).unwrap().get(0);
            let owner = economy::WalletOwner { account_id: ACCOUNT.into(), character_index: index,
                incarnation: format!("pg-character-v1:{incarnation}") };
            economy::credit_bound_authorized_with_source(client, &self.realm,
                "synthetic-network-funding", "synthetic-network-funding-v1", FUNDING,
                |_, _| Ok(owner), |_, _| Ok(())).unwrap();
        });
    }

    fn facts(&self, index: i32) -> (u64, i64, i64, u64) {
        self.run(|client| {
            let bound: i64 = client.query_one("SELECT bound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2 AND character_index=$3",
                &[&self.realm, &ACCOUNT, &index]).unwrap().get(0);
            let results: i64 = client.query_one("SELECT count(*) FROM hunting_repair_results WHERE realm_id=$1", &[&self.realm]).unwrap().get(0);
            let burns: i64 = client.query_one("SELECT count(*) FROM hunting_economy_burn_events WHERE realm_id=$1", &[&self.realm]).unwrap().get(0);
            let amount: i64 = client.query_one("SELECT COALESCE(sum(gold_equivalent),0)::bigint FROM hunting_economy_burn_events WHERE realm_id=$1", &[&self.realm]).unwrap().get(0);
            (u64::try_from(bound).unwrap(), results, burns, u64::try_from(amount).unwrap())
        })
    }

    fn quotes(&self) -> i64 {
        self.run(|client| {
            client
                .query_one(
                    "SELECT count(*) FROM hunting_repair_quotes WHERE realm_id=$1",
                    &[&self.realm],
                )
                .unwrap()
                .get(0)
        })
    }

    fn disable_policy(&self) {
        self.run(|client| {
            let disabled = policy(false);
            economy::register_policy(client, &self.realm, &disabled).unwrap();
            economy::activate_policy(client, &self.realm, &disabled.policy_version, false).unwrap();
        });
    }
}

fn local_origin() -> String {
    let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
        .ok()
        .and_then(|value| {
            value
                .split(',')
                .map(str::trim)
                .find(|v| !v.is_empty())
                .map(str::to_owned)
        })
        .expect("explicit local WebSocket/HTTP Origin allowlist required");
    let parsed = reqwest::Url::parse(&origin).expect("valid local test Origin");
    assert_eq!(parsed.scheme(), "http");
    assert!(
        parsed
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("localhost")
                || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())),
        "loopback test Origin required"
    );
    origin
}

struct Server {
    address: SocketAddr,
    config: Arc<SimulationConfig>,
    capacity: Arc<GatewayCapacityState>,
    reconnect: Arc<ReconnectSessionStore>,
    cache: Arc<InMemoryGatewaySessionCache>,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn start(db: &Database) -> Self {
        Self::start_on(db, "127.0.0.1:0".parse().unwrap()).await
    }

    async fn start_on(db: &Database, bind: SocketAddr) -> Self {
        assert!(bind.ip().is_loopback(), "fixture listener must be loopback");
        let config = Arc::new(db.config());
        let identity = outside_runtime(|| {
            crate::identity::IdentityService::from_env(Some(db.url.clone())).unwrap_or_else(|_| {
                panic!("test PostgreSQL identity requires configured identity secrets")
            })
        });
        assert_eq!(identity.backend_label(), "postgres");
        let capacity = Arc::new(GatewayCapacityState::unlimited());
        let reconnect = Arc::new(ReconnectSessionStore::default());
        let cache = Arc::new(InMemoryGatewaySessionCache::default());
        let spectator = crate::spectator::SpectatorHub::new(crate::spectator::SpectatorConfig {
            enabled: false,
            recording_enabled: false,
            public_enabled: false,
            public_maps: Vec::new(),
            director_token: None,
            capture_interval_ms: 250,
            public_delay_ms: 30_000,
            max_delay_ms: 120_000,
            ring_frames: 40,
            max_entities: 128,
            replay_limit: 100,
            retention_hours: 1,
            entity_stale_ms: 15_000,
            data_dir: db.root.join("spectator-disabled"),
        });
        let state = WebState {
            config: Arc::clone(&config),
            deploy_revision: None,
            zone_registry: Arc::new(ZoneRegistry::new(
                ZoneId::primary(),
                Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
                    Duration::from_secs(3600),
                    BTreeMap::new(),
                )),
            )),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: cache.clone(),
            reconnect_sessions: Arc::clone(&reconnect),
            capacity: Arc::clone(&capacity),
            gameplay_event_sink: None,
            identity: Arc::new(identity),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator,
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(db.root.join("ai-disabled")),
            )
            .unwrap(),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        };
        let app = axum::Router::new()
            .route("/ws", axum::routing::get(ws_upgrade))
            .route(
                "/v1/economy/hunting-wallet",
                axum::routing::get(hunting_economy::hunting_wallet),
            )
            .with_state(state);
        let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .expect("isolated loopback WebSocket server failed");
        });
        Self {
            address,
            config,
            capacity,
            reconnect,
            cache,
            task,
        }
    }

    async fn connect(&self) -> Socket {
        let mut request = format!("ws://{}/ws", self.address)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", local_origin().parse().unwrap());
        let (mut socket, response) = tokio_tungstenite::connect_async(request)
            .await
            .unwrap_or_else(|_| panic!("isolated WebSocket upgrade failed"));
        assert_eq!(response.status(), 101);
        let (realm, _) = until(&mut socket, "realm mode", |value| {
            value["type"] == "realmInfo"
        })
        .await;
        assert_eq!(realm.value["payload"]["npcRepairMode"], "hunting-v1");
        socket
    }

    async fn wait_empty(&self, index: i32) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let status = self.capacity.status();
                if status.current_ws_connections == 0
                    && status.current_active_sessions == 0
                    && status.current_reconnect_leases == 0
                    && self.reconnect.len() == 0
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("ordinary Disconnect did not complete real teardown");
        let key = GatewaySessionCacheKey {
            account_id: ACCOUNT.into(),
            character_index: index,
        };
        assert!(self.cache.get(&key).is_none());
        assert_eq!(self.cache.route_lease_count(), 0);
    }

    async fn retire_saved_raw_close(&self, index: i32) {
        // ws==0 is observed only after handle_socket has awaited actual
        // persistence and stored the retained session. A close frame or an
        // aborted listener alone would not establish this boundary.
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if self.capacity.status().current_ws_connections == 0 && self.reconnect.len() == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("raw close did not finish real save and retained-session handoff");
        let key = GatewaySessionCacheKey {
            account_id: ACCOUNT.into(),
            character_index: index,
        };
        let retired = self
            .reconnect
            .take(&key)
            .expect("exact saved old Session must be retained");
        let cache = Arc::clone(&self.cache);
        outside_runtime(move || {
            // Controlled test cleanup of the original owner, not a Login or a
            // client command. Remove only its owned route, then drop its real
            // runtime/presence and capacity permits before the cold instance.
            assert_eq!(
                crate::cache::remove_owned_session_cache(cache.as_ref(), &retired.session),
                Some(key)
            );
            drop(retired);
        });
        self.wait_empty(index).await;
    }

    async fn stop(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Observed {
    raw: String,
    value: Value,
}
async fn send(socket: &mut Socket, value: Value) {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .unwrap_or_else(|_| panic!("isolated ordinary socket command failed"));
}
async fn until(
    socket: &mut Socket,
    label: &str,
    predicate: impl Fn(&Value) -> bool,
) -> (Observed, Vec<Observed>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut events = Vec::new();
    // Diagnostics retain fixed frame names only. Never print identity grants,
    // credentials, passwords, full payloads or database connection strings.
    let mut kinds = BTreeMap::<String, usize>::new();
    let mut observed_bytes = 0usize;
    loop {
        let message = tokio::time::timeout_at(deadline, socket.next())
            .await
            .unwrap_or_else(|_| panic!("timeout waiting for {label}; frame kinds={kinds:?}"))
            .unwrap_or_else(|| panic!("socket closed waiting for {label}"))
            .unwrap_or_else(|_| panic!("socket transport failed waiting for {label}"));
        match message {
            Message::Text(raw) => {
                observed_bytes = observed_bytes.checked_add(raw.len()).unwrap();
                assert!(observed_bytes <= 32 * 1024 * 1024 && events.len() < 16_384,
                    "bounded network fixture exceeded bootstrap budget waiting for {label}; frame kinds={kinds:?}");
                let value: Value = serde_json::from_str(raw.as_ref())
                    .unwrap_or_else(|_| panic!("invalid JSON waiting for {label}"));
                *kinds
                    .entry(format!(
                        "{}:{}",
                        value["type"].as_str().unwrap_or("?"),
                        value["packet"].as_str().unwrap_or("")
                    ))
                    .or_default() += 1;
                let matches = predicate(&value);
                let observed = Observed {
                    raw: raw.to_string(),
                    value,
                };
                if matches {
                    return (observed, events);
                }
                events.push(observed);
            }
            Message::Close(_) => panic!("socket closed waiting for {label}"),
            _ => {}
        }
    }
}
fn packet(value: &Value, name: &str) -> bool {
    value["type"] == "packet" && value["packet"] == name
}

async fn register_and_login(socket: &mut Socket) -> String {
    send(socket,json!({"type":"newAccount","accountId":ACCOUNT,"password":PASSWORD,"birthDateBinary":0,
        "userName":"Synthetic Repair","secretQuestion":"q","secretAnswer":"a","emailAddress":"repair@example.test"})).await;
    let (created, _) = until(socket, "ordinary registration", |v| packet(v, "NewAccount")).await;
    assert_eq!(created.value["payload"]["result"], 8);
    login(socket).await
}
async fn login(socket: &mut Socket) -> String {
    send(
        socket,
        json!({"type":"login","accountId":ACCOUNT,"password":PASSWORD}),
    )
    .await;
    let (_, events) = until(socket, "ordinary authenticated login", |v| {
        packet(v, "LoginSuccess")
    })
    .await;
    let grant = events
        .iter()
        .find(|event| event.value["type"] == "identitySession")
        .expect("real identity grant before LoginSuccess");
    assert_eq!(grant.value["session"]["accountId"], ACCOUNT);
    grant.value["token"]
        .as_str()
        .expect("real identity bearer grant")
        .to_string()
}
async fn new_character(socket: &mut Socket) -> i32 {
    send(
        socket,
        json!({"type":"newCharacter","name":"RepairNetHero","gender":"Male","class":"Warrior"}),
    )
    .await;
    let (created, _) = until(socket, "ordinary character creation", |v| {
        packet(v, "NewCharacterSuccess")
    })
    .await;
    i32::try_from(
        created.value["payload"]["character"]["index"]
            .as_i64()
            .unwrap(),
    )
    .unwrap()
}
async fn enter(socket: &mut Socket, index: i32) -> Value {
    send(socket, json!({"type":"startGame","characterIndex":index})).await;
    let (started, _) = until(socket, "ordinary StartGame", |v| packet(v, "StartGame")).await;
    assert_eq!(started.value["payload"]["result"], 4);
    let (snapshot, _) = until(socket, "complete post-StartGame snapshot", |v| {
        v["type"] == "worldSnapshot"
    })
    .await;
    snapshot.value["payload"].clone()
}
fn bag_target(snapshot: &Value) -> &Value {
    snapshot["inventoryItems"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            item["key"] == "crystal-item-221" && item["container"] == "bag1" && item["slot"] == 20
        })
        .expect("actual complete Bag repair target")
}
fn observed_uid(snapshot: &Value) -> u64 {
    let item = bag_target(snapshot);
    let uid = item["uniqueId"].as_u64().expect("actual safe Bag UID");
    assert!(uid > 0 && uid <= 9_007_199_254_740_991);
    assert_eq!(uid, BAG_UID);
    let user: UserItem = serde_json::from_value(item["tooltipSource"]["userItem"].clone()).unwrap();
    assert_eq!(user.unique_id, uid);
    assert_eq!(user.item_index, 221);
    assert_eq!(user.count, 1);
    assert_eq!(user.current_dura, 100);
    assert_eq!(user.max_dura, 1000);
    assert_eq!(item["tooltipSource"]["info"]["item_index"], 221);
    assert!(item["tooltipSource"]["info"]["price"]
        .as_u64()
        .is_some_and(|value| value > 0));
    assert_eq!(
        item["tooltipSource"]["info"]["durability"],
        mir2_game_data::crystal_item_by_index(221)
            .unwrap()
            .durability
    );
    assert_eq!(item["quantity"], 1);
    assert_eq!(item["durabilityCurrent"], 100);
    assert_eq!(item["durabilityMax"], 1000);
    uid
}
async fn open_repair(socket: &mut Socket) {
    send(socket, json!({"type":"interact","objectId":NPC_ID})).await;
    // The ordinary high-level Interact path publishes ObjectChat plus its
    // authoritative dialog in worldSnapshot; it does not emit NPCResponse.
    let (dialog, _) = until(socket, "ordinary NPC dialog snapshot", |v| {
        v["type"] == "worldSnapshot" && v["payload"]["activeNpcDialog"]["npcObjectId"] == NPC_ID
    })
    .await;
    let target = dialog.value["payload"]["activeNpcDialog"]["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|link| link["target"].as_str())
        .find(|target| target.eq_ignore_ascii_case("@repair"))
        .expect("actual repair link in the server dialog");
    send(socket, json!({"type":"selectNpcDialog","target":target})).await;
    let (repair, _) = until(socket, "ordinary NPC repair service", |v| {
        packet(v, "NPCRepair")
    })
    .await;
    assert!(repair.value["payload"]["rate"]
        .as_f64()
        .is_some_and(|rate| rate.is_finite() && rate > 0.0));
}
async fn owner(
    socket: &mut Socket,
    id: u64,
    action: wire::Action,
) -> (wire::ServerFrame, Vec<Observed>) {
    let request = wire::ClientRequest::new(U64::new(id), action).unwrap();
    let encoded = wire::encode_client_request(&request).unwrap();
    socket.send(Message::Text(encoded.into())).await.unwrap();
    let (frame, events) = until(socket, "original repair owner response", |value| {
        value["type"] == "huntingRepairOwner" && value["requestId"] == id.to_string()
    })
    .await;
    let parsed = wire::parse_server_frame(&frame.raw).unwrap();
    parsed.validate_for_request(&request).unwrap();
    (parsed, events)
}
async fn disconnect(socket: &mut Socket) {
    send(socket, json!({"type":"disconnect"})).await;
    let _ = until(socket, "ordinary explicit Disconnect", |v| {
        packet(v, "Disconnect")
    })
    .await;
}
fn assert_repaired_checkpoint(db: &Database, index: i32) -> CharacterSaveRecord {
    let saved = db.checkpoint(index);
    assert_eq!(saved.gold, LEGACY_GOLD);
    let target: Value = saved
        .inventory_items_json
        .iter()
        .map(|item| serde_json::from_str::<Value>(item).unwrap())
        .find(|item| item["unique_id"] == BAG_UID)
        .expect("durable original Bag item");
    assert_eq!(target["durability_current"], 970);
    assert_eq!(target["durability_max"], 970);
    saved
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicitly isolated loopback PostgreSQL and local test identity/runtime/Origin configuration"]
async fn hunting_repair_network_postgres_paid_bag_confirm_query_disconnect_cold_login_and_wallet_origin(
) {
    let db = Database::new();
    let server = Server::start(&db).await;
    let old_store = Arc::clone(&server.config.account_store);
    let mut socket = server.connect().await;
    let (unauthenticated, _) = owner(
        &mut socket,
        1,
        wire::Action::Quote {
            unique_id: U64::new(BAG_UID),
            special: false,
        },
    )
    .await;
    assert!(matches!(
        unauthenticated.reply,
        wire::Reply::Rejected {
            code: wire::Rejection::Unauthenticated,
            ..
        }
    ));
    assert!(unauthenticated.snapshot.is_none());
    assert_eq!(db.quotes(), 0);
    let bearer = register_and_login(&mut socket).await;
    let index = new_character(&mut socket).await;
    db.prepare_offline_target(&server.config, index);
    let snapshot = enter(&mut socket, index).await;
    let uid = observed_uid(&snapshot);
    assert_eq!(db.facts(index), (FUNDING, 0, 0, 0));
    let (no_dialog, _) = owner(
        &mut socket,
        2,
        wire::Action::Quote {
            unique_id: U64::new(uid),
            special: false,
        },
    )
    .await;
    assert!(matches!(
        no_dialog.reply,
        wire::Reply::Rejected {
            code: wire::Rejection::Unsupported,
            ..
        }
    ));
    assert!(no_dialog.snapshot.is_none());
    assert_eq!(db.quotes(), 0);

    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let wallet_url = format!(
        "http://{}/v1/economy/hunting-wallet?characterIndex={index}",
        server.address
    );
    let denied = http
        .get(&wallet_url)
        .bearer_auth(&bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), reqwest::StatusCode::FORBIDDEN);
    assert_eq!(
        denied.json::<Value>().await.unwrap()["code"],
        "originNotAllowed"
    );
    let invalid = http
        .get(&wallet_url)
        .header("Origin", local_origin())
        .bearer_auth("invalid-test-token")
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), reqwest::StatusCode::UNAUTHORIZED);
    let balance = http
        .get(&wallet_url)
        .header("Origin", local_origin())
        .bearer_auth(&bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(balance.status(), reqwest::StatusCode::OK);
    assert_eq!(balance.headers().get("cache-control").unwrap(), "no-store");
    let balance: Value = balance.json().await.unwrap();
    assert_eq!(balance["boundGold"], FUNDING.to_string());
    assert_eq!(balance["unboundGold"], "0");

    open_repair(&mut socket).await;
    // Refuse the old packet against a damaged item before obtaining a new quote.
    send(&mut socket, json!({"type":"repairItem","uniqueId":uid})).await;
    // Quote follows the same serial input queue. KeepAlive uses the reader's
    // fast lane and cannot establish that the earlier repair was handled.
    let (quoted, refused) = owner(
        &mut socket,
        3,
        wire::Action::Quote {
            unique_id: U64::new(uid),
            special: false,
        },
    )
    .await;
    assert!(!refused
        .iter()
        .any(|event| packet(&event.value, "ItemRepaired") || packet(&event.value, "LoseGold")));
    assert!(refused.iter().any(|event| packet(&event.value, "Chat")
        && event.value["payload"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Repair requires a server quote"))));
    assert_eq!(db.facts(index), (FUNDING, 0, 0, 0));

    let quote = match quoted.reply {
        wire::Reply::Quoted { quote } => quote,
        _ => panic!("real Bag quote required"),
    };
    assert_eq!(quote.unique_id.get(), uid);
    assert!(quote.cost_bound_gold.get() > 0);
    assert_eq!(
        db.facts(index),
        (FUNDING, 0, 0, 0),
        "Quote has no asset effect"
    );
    let before = db.checkpoint(index);
    let (committed, events) = owner(
        &mut socket,
        4,
        wire::Action::Confirm {
            quote_id: quote.quote_id.clone(),
        },
    )
    .await;
    let receipt = match committed.reply {
        wire::Reply::Committed {
            receipt,
            replayed: false,
            reload_required: false,
        } => receipt,
        _ => panic!("real paid repair must commit with current projection"),
    };
    assert!(
        events
            .iter()
            .any(|event| packet(&event.value, "ItemRepaired")),
        "real item delta precedes full owner snapshot"
    );
    assert!(!events.iter().any(|event| packet(&event.value, "LoseGold")));
    let projection = committed
        .snapshot
        .as_ref()
        .expect("same owner turn complete snapshot");
    assert_eq!(projection["gold"], LEGACY_GOLD);
    assert_eq!(bag_target(projection)["durabilityCurrent"], 970);
    let saved = assert_repaired_checkpoint(&db, index);
    assert_eq!(saved.revision, before.revision + 1);
    let expected = (
        FUNDING - quote.cost_bound_gold.get(),
        1,
        1,
        quote.cost_bound_gold.get(),
    );
    assert_eq!(db.facts(index), expected);
    let (replay, events) = owner(
        &mut socket,
        5,
        wire::Action::Query {
            quote_id: quote.quote_id.clone(),
        },
    )
    .await;
    assert!(
        matches!(replay.reply,wire::Reply::Committed {receipt:actual,replayed:true,reload_required:false} if actual==receipt)
    );
    assert!(replay.snapshot.is_some());
    assert!(!events
        .iter()
        .any(|event| packet(&event.value, "ItemRepaired") || packet(&event.value, "LoseGold")));
    assert_eq!(db.facts(index), expected);
    assert_eq!(db.checkpoint(index).revision, saved.revision);

    // Policy withdrawal does not change the required protocol mode or admit a
    // legacy asset route. Original committed receipts remain queryable.
    db.disable_policy();
    open_repair(&mut socket).await;
    let (disabled, _) = owner(
        &mut socket,
        6,
        wire::Action::Quote {
            unique_id: U64::new(uid),
            special: false,
        },
    )
    .await;
    assert!(matches!(
        disabled.reply,
        wire::Reply::Rejected {
            // Registration failures use the runtime's bounded unavailable
            // response; policy-off must reject even this zero-fee item.
            code: wire::Rejection::Unavailable,
            ..
        }
    ));
    assert!(disabled.snapshot.is_none());
    assert_eq!(db.quotes(), 1);
    let (replay, _) = owner(
        &mut socket,
        7,
        wire::Action::Query {
            quote_id: quote.quote_id.clone(),
        },
    )
    .await;
    assert!(
        matches!(replay.reply,wire::Reply::Committed {receipt:actual,replayed:true,reload_required:false} if actual==receipt)
    );
    assert_eq!(db.facts(index), expected);
    disconnect(&mut socket).await;
    drop(socket);
    server.wait_empty(index).await;
    assert_repaired_checkpoint(&db, index);
    server.stop().await;

    // A new config, registry, cache, identity service and physical connection:
    // this cannot consume a retained Session or reuse the old account mirror.
    let cold = Server::start(&db).await;
    assert!(!Arc::ptr_eq(&old_store, &cold.config.account_store));
    let mut socket = cold.connect().await;
    let _ = login(&mut socket).await;
    let loaded = enter(&mut socket, index).await;
    assert_eq!(loaded["gold"], LEGACY_GOLD);
    assert_eq!(bag_target(&loaded)["durabilityCurrent"], 970);
    assert_eq!(bag_target(&loaded)["durabilityMax"], 970);
    assert_eq!(db.facts(index), expected);
    disconnect(&mut socket).await;
    drop(socket);
    cold.wait_empty(index).await;
    cold.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicitly isolated loopback PostgreSQL and local test identity/runtime/Origin configuration"]
async fn hunting_repair_network_postgres_unread_commit_raw_close_saves_then_cold_queries_original_once(
) {
    let db = Database::new();
    let server = Server::start(&db).await;
    let old_store = Arc::clone(&server.config.account_store);
    let mut socket = server.connect().await;
    let _ = register_and_login(&mut socket).await;
    let index = new_character(&mut socket).await;
    db.prepare_offline_target(&server.config, index);
    let snapshot = enter(&mut socket, index).await;
    let uid = observed_uid(&snapshot);
    open_repair(&mut socket).await;
    let (quoted, _) = owner(
        &mut socket,
        10,
        wire::Action::Quote {
            unique_id: U64::new(uid),
            special: false,
        },
    )
    .await;
    let quote = match quoted.reply {
        wire::Reply::Quoted { quote } => quote,
        _ => panic!("real Bag quote required"),
    };
    assert_eq!(quote.unique_id.get(), uid);
    assert!(quote.cost_bound_gold.get() > 0);
    let before = db.checkpoint(index);
    let original = wire::ClientRequest::new(
        U64::new(11),
        wire::Action::Confirm {
            quote_id: quote.quote_id.clone(),
        },
    )
    .unwrap();
    socket
        .send(Message::Text(
            wire::encode_client_request(&original).unwrap().into(),
        ))
        .await
        .unwrap();

    // From this point until drop, never consume a socket frame. An independent
    // PG connection proves the real source committed; it supplies no response
    // or projection to the client. This models an unread result, not a fake
    // rollback, a test fault or a retry with a replacement operation identity.
    let expected = (
        FUNDING - quote.cost_bound_gold.get(),
        1,
        1,
        quote.cost_bound_gold.get(),
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if db.facts(index) == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("actual PG repair did not commit before the unread connection was dropped");
    let committed = assert_repaired_checkpoint(&db, index);
    assert_eq!(committed.revision, before.revision + 1);
    drop(socket);
    server.retire_saved_raw_close(index).await;
    let drained = assert_repaired_checkpoint(&db, index);
    assert!(drained.revision >= committed.revision);
    assert_eq!(drained.inventory_items_json, committed.inventory_items_json);
    assert_eq!(db.facts(index), expected);
    server.stop().await;

    let cold = Server::start(&db).await;
    assert!(!Arc::ptr_eq(&old_store, &cold.config.account_store));
    let mut socket = cold.connect().await;
    let _ = login(&mut socket).await;
    let loaded = enter(&mut socket, index).await;
    assert_eq!(loaded["gold"], LEGACY_GOLD);
    assert_eq!(bag_target(&loaded)["durabilityCurrent"], 970);
    assert_eq!(bag_target(&loaded)["durabilityMax"], 970);
    let before_query = db.checkpoint(index);
    // No NPC interaction or fresh quote is required for a readonly original
    // result query by this newly authenticated canonical character incarnation.
    let (queried, events) = owner(
        &mut socket,
        12,
        wire::Action::Query {
            quote_id: quote.quote_id.clone(),
        },
    )
    .await;
    let receipt = match queried.reply {
        wire::Reply::Committed {
            receipt,
            replayed: true,
            reload_required: false,
        } => receipt,
        _ => panic!("cold canonical Login must query the original committed repair"),
    };
    assert_eq!(receipt.quote_id, quote.quote_id);
    assert_eq!(receipt.cost_bound_gold, quote.cost_bound_gold);
    assert!(!receipt.free);
    assert!(queried.snapshot.is_some());
    assert!(!events
        .iter()
        .any(|event| packet(&event.value, "ItemRepaired") || packet(&event.value, "LoseGold")));
    assert_eq!(db.facts(index), expected);
    let after_query = db.checkpoint(index);
    assert_eq!(after_query.revision, before_query.revision);
    assert_eq!(
        after_query.inventory_items_json,
        before_query.inventory_items_json
    );
    disconnect(&mut socket).await;
    drop(socket);
    cold.wait_empty(index).await;
    cold.stop().await;
}

// Only direct, new control files in the explicitly designated private build
// directory are allowed. No credential/DSN is written and no directory deleted.
fn browser_control_file(variable: &str) -> PathBuf {
    let configured =
        PathBuf::from(std::env::var_os(variable).expect("explicit fixture control file required"));
    assert!(
        configured.is_absolute(),
        "fixture control path must be absolute"
    );
    let configured_root = PathBuf::from(
        std::env::var_os("MIR2_HUNTING_BROWSER_CONTROL_DIR")
            .expect("explicit private fixture control directory required"),
    );
    assert!(
        configured_root.is_absolute(),
        "absolute control directory required"
    );
    let root = configured_root
        .canonicalize()
        .expect("private build directory must exist");
    let parent = configured
        .parent()
        .unwrap()
        .canonicalize()
        .expect("fixture control parent must exist");
    assert_eq!(
        parent, root,
        "fixture control file must be directly inside private build directory"
    );
    let name = configured
        .file_name()
        .and_then(|name| name.to_str())
        .expect("ASCII fixture filename required");
    assert!(
        name.starts_with("hunting-repair-browser-")
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)),
        "bounded fixture filename required"
    );
    let resolved = parent.join(name);
    assert!(
        !resolved
            .try_exists()
            .expect("fixture control path check failed"),
        "fixture control file must be new"
    );
    resolved
}

fn write_browser_status(file: &mut std::fs::File, value: &Value) {
    let encoded = serde_json::to_vec_pretty(value).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file.set_len(0).unwrap();
    file.write_all(&encoded).unwrap();
    file.sync_all().unwrap();
}

/// A temporary test-only Gateway for ordinary browser Login/StartGame. The
/// synthetic password is the existing test constant, never a ready-file field
/// or console output. A real browser may inspect and mutate only this schema.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "holds an isolated loopback browser fixture until its stop file or bounded deadline"]
async fn hunting_repair_network_postgres_browser_fixture() {
    let bind: SocketAddr = std::env::var("MIR2_HUNTING_BROWSER_ADDR")
        .expect("explicit browser fixture listener required")
        .parse()
        .expect("valid fixture listener required");
    assert!(
        bind.ip().is_loopback() && bind.port() != 0,
        "fixed loopback fixture port required"
    );
    let ready_path = browser_control_file("MIR2_HUNTING_BROWSER_READY_FILE");
    let stop_path = browser_control_file("MIR2_HUNTING_BROWSER_STOP_FILE");
    assert_ne!(ready_path, stop_path);
    let seconds = std::env::var("MIR2_HUNTING_BROWSER_SECONDS")
        .map(|value| {
            value
                .parse::<u64>()
                .expect("fixture lifetime must be seconds")
        })
        .unwrap_or(20 * 60);
    assert!(
        (1..=2 * 60 * 60).contains(&seconds),
        "fixture lifetime must be between 1 and 7200 seconds"
    );
    let origin = local_origin();
    let db = Database::new();
    let server = Server::start_on(&db, bind).await;
    let mut preparation = server.connect().await;
    let _ = register_and_login(&mut preparation).await;
    let index = new_character(&mut preparation).await;
    db.prepare_offline_target(&server.config, index);
    // No StartGame: the browser must establish its own normal Login, actor and
    // canonical snapshot. Disconnect completes without retaining a character.
    disconnect(&mut preparation).await;
    drop(preparation);
    server.wait_empty(index).await;
    let initial = db.checkpoint(index);
    assert_eq!(initial.gold, LEGACY_GOLD);
    assert_eq!(db.facts(index), (FUNDING, 0, 0, 0));
    let expires_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .checked_add(u128::from(seconds) * 1000)
        .unwrap();
    let mut ready = json!({"version":"hunting-repair-browser-fixture-v1","status":"ready",
        "processId":std::process::id(),"schema":db.schema,"realm":db.realm,
        "gatewayHttpUrl":format!("http://{}",server.address),"gatewayWsUrl":format!("ws://{}/ws",server.address),
        "publicOrigin":origin,"accountId":ACCOUNT,"character":{"index":index,"name":"RepairNetHero"},
        "target":{"uniqueId":BAG_UID.to_string(),"container":"bag1","slot":20,"npcObjectId":NPC_ID,
            "durabilityCurrent":100,"durabilityMax":1000},
        "initialBoundGold":FUNDING.to_string(),"initialLegacyGold":LEGACY_GOLD,
        "expiresAtMs":u64::try_from(expires_at_ms).unwrap(),"stopFile":stop_path.to_string_lossy()});
    let mut ready_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ready_path)
        .expect("new private browser ready file required");
    write_browser_status(&mut ready_file, &ready);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    let reason = loop {
        match std::fs::symlink_metadata(&stop_path) {
            Ok(metadata) => {
                assert!(
                    metadata.is_file() && !metadata.file_type().is_symlink(),
                    "stop marker must be a plain file"
                );
                break "stopFile";
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => panic!("browser fixture stop marker check failed"),
        }
        if tokio::time::Instant::now() >= deadline {
            break "deadline";
        }
        assert!(
            !server.task.is_finished(),
            "browser fixture listener ended before stop"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    ready["status"] = json!("stopping");
    ready["stopReason"] = json!(reason);
    write_browser_status(&mut ready_file, &ready);
    // Stop accepting first. Normally the browser has already used its ordinary
    // logout/Disconnect before touching stop. Only this isolated synthetic
    // account is revoked if a tab remains; the production loop then saves it.
    server.task.abort();
    let revocation = server.reconnect.begin_account_identity_revocation(ACCOUNT);
    if server.capacity.status().current_ws_connections != 0 {
        outside_runtime(|| {
            let identity = crate::identity::IdentityService::from_env(Some(db.url.clone()))
                .unwrap_or_else(|_| panic!("isolated fixture shutdown identity unavailable"));
            let sessions = identity
                .list_account_session_ids(ACCOUNT)
                .unwrap_or_else(|_| panic!("isolated fixture shutdown sessions unavailable"));
            identity
                .revoke_all_account_sessions(ACCOUNT, "browser_fixture_shutdown")
                .unwrap_or_else(|_| panic!("isolated fixture shutdown revocation failed"));
            for session in sessions {
                server
                    .cache
                    .revoke_identity_session(&session, 120)
                    .unwrap_or_else(|_| {
                        panic!("isolated fixture shutdown cache revocation failed")
                    });
            }
        });
    }
    let closed = tokio::time::timeout(Duration::from_secs(30), async {
        while server.capacity.status().current_ws_connections != 0 {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    if closed.is_err() {
        ready["status"] = json!("shutdownUnproven");
        write_browser_status(&mut ready_file, &ready);
        panic!("browser fixture did not prove real socket teardown before shutdown deadline");
    }
    let key = GatewaySessionCacheKey {
        account_id: ACCOUNT.into(),
        character_index: index,
    };
    if let Some(retired) = server.reconnect.take(&key) {
        outside_runtime(|| {
            assert_eq!(
                crate::cache::remove_owned_session_cache(server.cache.as_ref(), &retired.session),
                Some(key)
            );
            drop(retired);
        });
    }
    server.wait_empty(index).await;
    let saved = db.checkpoint(index);
    ready["status"] = json!("stopped");
    ready["savedRevision"] = json!(saved.revision);
    ready["finalLegacyGold"] = json!(saved.gold);
    write_browser_status(&mut ready_file, &ready);
    drop(revocation);
    server.stop().await;
}

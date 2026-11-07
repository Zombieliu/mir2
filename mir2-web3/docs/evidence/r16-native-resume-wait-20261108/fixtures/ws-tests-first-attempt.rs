//! Deterministic real-WS pending-save cases. Compile/run status is recorded separately.
//! Only the final File-save I/O is paused; no production hook or environment delay is used.
use super::*;
use crate::routing::{SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRegistry, ZoneRuntimeFactory};
use mir2_simulation::{
    CharacterHandoffCheckpoint, CharacterHandoffCommitment, NativeAttackError,
    NativeAttackExecution, WorldCommandExecution, WorldRuntime, ZoneRuntimeHandle,
};
use std::any::Any;
use std::path::PathBuf;
use std::sync::Condvar;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as ClientMessage};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

struct SaveGate {
    entered: Mutex<Option<oneshot::Sender<()>>>,
    release: (Mutex<bool>, Condvar),
}
impl SaveGate {
    fn pause(&self) {
        if let Some(tx) = self.entered.lock().unwrap().take() {
            let _ = tx.send(());
        }
        let mut released = self.release.0.lock().unwrap();
        while !*released {
            released = self.release.1.wait(released).unwrap();
        }
    }
    fn release(&self) {
        *self.release.0.lock().unwrap() = true;
        self.release.1.notify_all();
    }
}
struct ReleaseOnDrop(Arc<SaveGate>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
struct ControlledFactory {
    inner: SharedInProcessZoneRuntimeFactory,
    gate: Arc<SaveGate>,
}
impl ZoneRuntimeFactory for ControlledFactory {
    fn create_runtime(&self, config: crate::GatewayConfig, zone: &ZoneId) -> ZoneRuntimeHandle {
        Box::new(ControlledRuntime {
            inner: self.inner.create_runtime(config, zone),
            gate: Arc::clone(&self.gate),
        })
    }
    fn supports_shared_groups(&self) -> bool {
        self.inner.supports_shared_groups()
    }
    fn owner_lease(&self, zone: &ZoneId) -> crate::routing::ZoneOwnerLease {
        self.inner.owner_lease(zone)
    }
    fn configure_companion_owner_fences(
        &self,
        policy: &crate::companion_relay::CompanionPolicy,
    ) -> Result<(), String> {
        self.inner.configure_companion_owner_fences(policy)
    }
    fn configure_companion_life_projection(
        &self,
        policy: &crate::companion_relay::CompanionPolicy,
        handle: crate::companion_supply::LifeProjectionHandle,
    ) -> Result<(), String> {
        self.inner
            .configure_companion_life_projection(policy, handle)
    }
}
struct ControlledRuntime {
    inner: ZoneRuntimeHandle,
    gate: Arc<SaveGate>,
}
// Every current WorldRuntime method forwards, including as_any, so real shared
// prepare/thaw downcasts retain their authority and only frozen-save I/O pauses.
use mir2_protocol::{
    client_packet_name, ChatItem, ClientIntelligentCreature, ClientPacket, MirDirection, Point,
    ServerPacket, Spell,
};
use mir2_simulation::{
    ActiveSessionIdentity, CharacterSaveRecord, ChatPacketPreparation, GroundDropSnapshot,
    SharedGroundDropPickupCommit, SimulationConfig, SimulationSession, WorldEntitySnapshot,
    WorldSnapshot,
};
use mir2_simulation::{
    GameShopPurchaseOutcome, LocalPlayerVitalsSnapshot, NpcGoldBuyBeforeExecution,
    NpcGoldBuyProcessingError, NpcGoldBuyProcessingExecution, NpcGoldBuyRequest,
    NpcPurchaseBeforeExecution, NpcPurchaseProcessingError, NpcPurchaseProcessingExecution,
    NpcPurchaseRequest, SharedAccountInventoryTransactionReceipt, SharedItemRentalDelivery,
    SharedItemRentalFeeOffer, SharedItemRentalItemOffer, SharedNpcSavedValue,
    SharedSkillItemConsumptionComponent, SharedTradeOffer, ZoneMonsterSpawn, ZonePlayerCombatStats,
};

impl WorldRuntime for ControlledRuntime {
    fn execute_procurement_owner(
        &mut self,
        _authenticated: bool,
        _request: &mir2_protocol::procurement::ProcurementRequest,
        _now_ms: u64,
    ) -> Result<mir2_simulation::procurement::ProcurementExecution, String> {
        self.inner
            .execute_procurement_owner(_authenticated, _request, _now_ms)
    }
    fn as_any(&self) -> &dyn Any {
        self.inner.as_any()
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.inner.as_any_mut()
    }
    fn supports_zone_save_fence(&self) -> bool {
        self.inner.supports_zone_save_fence()
    }
    fn bind_zone_save_fence(
        &mut self,
        _fence: mir2_simulation::ZoneSaveFence,
    ) -> Result<(), String> {
        self.inner.bind_zone_save_fence(_fence)
    }
    fn bound_zone_save_fence(&self) -> Option<mir2_simulation::ZoneSaveFence> {
        self.inner.bound_zone_save_fence()
    }
    fn prepare_zone_save_operation(
        &mut self,
        _binding: mir2_simulation::ZoneSaveOperationBinding,
        _frozen_checkpoint: &CharacterSaveRecord,
    ) -> Result<mir2_simulation::ZoneSaveOperationReceipt, String> {
        self.inner
            .prepare_zone_save_operation(_binding, _frozen_checkpoint)
    }
    fn persist_zone_save_operation(
        &mut self,
        _operation: &mir2_simulation::ZoneSaveOperation,
    ) -> Result<mir2_simulation::ZoneSaveOperationReceipt, String> {
        self.inner.persist_zone_save_operation(_operation)
    }
    fn query_zone_save_operation(
        &self,
        _binding: &mir2_simulation::ZoneSaveOperationBinding,
    ) -> Result<Option<mir2_simulation::ZoneSaveOperationReceipt>, String> {
        self.inner.query_zone_save_operation(_binding)
    }
    fn release_zone_save_operation(
        &mut self,
        _binding: &mir2_simulation::ZoneSaveOperationBinding,
    ) -> Result<mir2_simulation::ZoneSaveOperationReceipt, String> {
        self.inner.release_zone_save_operation(_binding)
    }
    fn on_connect(&self) -> Vec<ServerPacket> {
        self.inner.on_connect()
    }
    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        self.inner.execute(command)
    }
    fn execute_production_attack_requiring_typed_outcome(
        &mut self,
        _authenticated: bool,
        _object_id: u32,
    ) -> Result<NativeAttackExecution, NativeAttackError> {
        self.inner
            .execute_production_attack_requiring_typed_outcome(_authenticated, _object_id)
    }
    fn execute_with_outcome(
        &mut self,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        self.inner.execute_with_outcome(command)
    }
    fn supports_typed_game_shop_purchase_outcome(&self) -> bool {
        self.inner.supports_typed_game_shop_purchase_outcome()
    }
    fn supports_typed_npc_gold_buy_outcome(&self) -> bool {
        self.inner.supports_typed_npc_gold_buy_outcome()
    }
    fn execute_production_npc_gold_buy_requiring_typed_outcome(
        &mut self,
        _authenticated: bool,
        _request: NpcGoldBuyRequest,
    ) -> Result<NpcGoldBuyProcessingExecution, NpcGoldBuyProcessingError> {
        self.inner
            .execute_production_npc_gold_buy_requiring_typed_outcome(_authenticated, _request)
    }
    fn supports_durable_npc_purchase_owner(&self) -> bool {
        self.inner.supports_durable_npc_purchase_owner()
    }
    fn execute_npc_purchase_owner(
        &mut self,
        _authenticated: bool,
        _owner_epoch: [u8; 32],
        _action: mir2_simulation::NpcPurchaseOwnerAction,
    ) -> Result<mir2_simulation::NpcPurchaseOwnerExecution, mir2_simulation::NpcPurchaseDurableError>
    {
        self.inner
            .execute_npc_purchase_owner(_authenticated, _owner_epoch, _action)
    }
    fn npc_purchase_owner_authority(
        &self,
        _owner_epoch: [u8; 32],
    ) -> Option<mir2_simulation::NpcPurchaseProducer> {
        self.inner.npc_purchase_owner_authority(_owner_epoch)
    }
    fn npc_purchase_owner_checkpoint(&self, _owner_epoch: [u8; 32]) -> Option<CharacterSaveRecord> {
        self.inner.npc_purchase_owner_checkpoint(_owner_epoch)
    }
    fn apply_npc_purchase_owner_replica_checkpoint(
        &mut self,
        _checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        self.inner
            .apply_npc_purchase_owner_replica_checkpoint(_checkpoint)
    }
    fn supports_typed_npc_purchase_outcome(&self) -> bool {
        self.inner.supports_typed_npc_purchase_outcome()
    }
    fn execute_production_npc_purchase_requiring_typed_outcome(
        &mut self,
        _authenticated: bool,
        _request: NpcPurchaseRequest,
    ) -> Result<NpcPurchaseProcessingExecution, NpcPurchaseProcessingError> {
        self.inner
            .execute_production_npc_purchase_requiring_typed_outcome(_authenticated, _request)
    }
    fn supports_native_game_shop_gifts(&self) -> bool {
        self.inner.supports_native_game_shop_gifts()
    }
    fn execute_production_player_command(
        &mut self,
        authenticated: bool,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        self.inner
            .execute_production_player_command(authenticated, command)
    }
    fn world_snapshot(&self) -> WorldSnapshot {
        self.inner.world_snapshot()
    }
    fn current_tick(&self) -> u64 {
        self.inner.current_tick()
    }
    fn supports_magic_key_assignment(
        &self,
        _spell: mir2_protocol::Spell,
        _key: u8,
        _old_key: u8,
    ) -> bool {
        self.inner
            .supports_magic_key_assignment(_spell, _key, _old_key)
    }
    fn current_map_shared_entity_snapshots(&self) -> Vec<WorldEntitySnapshot> {
        self.inner.current_map_shared_entity_snapshots()
    }
    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.inner.active_identity()
    }
    fn active_character_checkpoint(&self) -> Option<CharacterSaveRecord> {
        self.inner.active_character_checkpoint()
    }
    fn active_character_teardown_checkpoint(&self) -> Option<CharacterSaveRecord> {
        self.inner.active_character_teardown_checkpoint()
    }
    fn take_player_drop_journal_result(
        &mut self,
    ) -> Option<mir2_simulation::PlayerDropJournalResult> {
        self.inner.take_player_drop_journal_result()
    }
    fn acknowledge_player_drop_journal_result(
        &mut self,
        _operation_key: &str,
        _intent_digest: &str,
    ) -> Result<(), String> {
        self.inner
            .acknowledge_player_drop_journal_result(_operation_key, _intent_digest)
    }
    fn apply_player_drop_journal_result(
        &mut self,
        _result: &mir2_simulation::PlayerDropJournalResult,
    ) -> Result<Vec<ServerPacket>, String> {
        self.inner.apply_player_drop_journal_result(_result)
    }
    fn restore_active_character_checkpoint(
        &mut self,
        _checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        self.inner.restore_active_character_checkpoint(_checkpoint)
    }
    fn restore_replica_session_checkpoint(
        &mut self,
        _identity: &ActiveSessionIdentity,
        _checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        self.inner
            .restore_replica_session_checkpoint(_identity, _checkpoint)
    }
    fn prepare_character_handoff(&mut self) -> Result<CharacterHandoffCheckpoint, String> {
        self.inner.prepare_character_handoff()
    }
    fn resume_character_handoff(
        &mut self,
        _checkpoint: &CharacterHandoffCheckpoint,
    ) -> Result<CharacterHandoffCommitment, String> {
        self.inner.resume_character_handoff(_checkpoint)
    }
    fn apply_character_handoff_replica(
        &mut self,
        _checkpoint: &CharacterHandoffCheckpoint,
    ) -> Result<(), String> {
        self.inner.apply_character_handoff_replica(_checkpoint)
    }
    fn save_active_character(&mut self) -> Result<(), String> {
        self.inner.save_active_character()
    }
    fn save_active_character_for_logout(&mut self) -> Result<(), String> {
        self.inner.save_active_character_for_logout()
    }
    fn save_frozen_character_checkpoint_for_logout(
        &mut self,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        self.gate.pause();
        self.inner
            .save_frozen_character_checkpoint_for_logout(checkpoint)
    }
    fn refresh_active_external_mail(&mut self) -> bool {
        self.inner.refresh_active_external_mail()
    }
}

struct Fixture {
    root: PathBuf,
    config: crate::GatewayConfig,
    store: Arc<ReconnectSessionStore>,
    capacity: Arc<GatewayCapacityState>,
    gate: Arc<SaveGate>,
    entered: Option<oneshot::Receiver<()>>,
    server: Option<tokio::task::JoinHandle<()>>,
    address: SocketAddr,
    account: String,
    character: String,
    _release: ReleaseOnDrop,
}
struct Started {
    socket: Socket,
    key: GatewaySessionCacheKey,
    credential: String,
    generation: u64,
    protected: Value,
}
impl Fixture {
    async fn new() -> Self {
        let n = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mir2-native-pending-ws-{}-{stamp}-{n}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let config = crate::GatewayConfig::default()
            .with_account_store_path(root.join("accounts.json"))
            .with_save_recovery_dir(root.join("recovery"))
            .with_save_recovery_mac_key(std::array::from_fn::<u8, 32, _>(|index| {
                0x67u8.wrapping_add(index as u8)
            }))
            .unwrap();
        let (tx, rx) = oneshot::channel();
        let gate = Arc::new(SaveGate {
            entered: Mutex::new(Some(tx)),
            release: (Mutex::new(false), Condvar::new()),
        });
        let release = ReleaseOnDrop(Arc::clone(&gate));
        let registry = ZoneRegistry::new(
            ZoneId::primary(),
            Arc::new(ControlledFactory {
                inner: SharedInProcessZoneRuntimeFactory::new(),
                gate: Arc::clone(&gate),
            }),
        );
        let store = Arc::new(ReconnectSessionStore::default());
        let capacity = Arc::new(GatewayCapacityState::with_limits(Some(4), Some(1), Some(1)));
        let spectator = crate::spectator::SpectatorHub::new(crate::spectator::SpectatorConfig {
            enabled: false,
            recording_enabled: false,
            public_enabled: false,
            public_maps: vec![],
            director_token: None,
            capture_interval_ms: 1000,
            public_delay_ms: 0,
            max_delay_ms: 1000,
            ring_frames: 1,
            max_entities: 16,
            replay_limit: 1,
            retention_hours: 1,
            entity_stale_ms: 1000,
            data_dir: root.join("spectator"),
        });
        let state = WebState {
            config: Arc::new(config.clone()),
            deploy_revision: None,
            zone_registry: Arc::new(registry),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::clone(&store),
            capacity: Arc::clone(&capacity),
            gameplay_event_sink: None,
            identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
            auth_rate_limit_policy: AuthRateLimitPolicy::default(),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator,
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(root.join("ai")),
            )
            .unwrap(),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
            companions: None,
        };
        let app = axum::Router::new()
            .route("/ws", axum::routing::get(ws_upgrade))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        Self {
            root,
            config,
            store,
            capacity,
            gate,
            entered: Some(rx),
            server: Some(server),
            address,
            account: format!("nrwait{n}"),
            character: format!("WaitHero{n}"),
            _release: release,
        }
    }
    async fn connect(&self) -> Socket {
        let mut request = format!("ws://{}/ws", self.address)
            .into_client_request()
            .unwrap();
        let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
            .ok()
            .and_then(|v| {
                v.split(',')
                    .map(str::trim)
                    .find(|s| !s.is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| format!("http://{}", self.address));
        request
            .headers_mut()
            .insert("origin", origin.parse().unwrap());
        let (mut socket, response) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(response.status(), 101);
        send(
            &mut socket,
            json!({"type":"clientCapabilities","capabilities":[NATIVE_RESUME_PROTOCOL]}),
        )
        .await;
        socket
    }
    async fn start_character(&self) -> Started {
        let mut socket = self.connect().await;
        send(
            &mut socket,
            json!({"type":"newAccount","accountId":self.account,"password":"FixtureResume73",
            "birthDateBinary":0,"userName":"Pending Resume","secretQuestion":"q","secretAnswer":"a",
            "emailAddress":"pending@example.test"}),
        )
        .await;
        let (registered, _) = read_until(&mut socket, |e| packet(e, "NewAccount")).await;
        assert_eq!(registered["payload"]["result"], 8);
        send(
            &mut socket,
            json!({"type":"login","accountId":self.account,"password":"FixtureResume73"}),
        )
        .await;
        let (_, login) = read_until(&mut socket, |e| packet(e, "LoginSuccess")).await;
        assert!(login.iter().any(|e| e["type"] == "identitySession"));
        send(
            &mut socket,
            json!({"type":"newCharacter","name":self.character,"gender":"Male","class":"Warrior"}),
        )
        .await;
        let (created, _) = read_until(&mut socket, |e| packet(e, "NewCharacterSuccess")).await;
        let index =
            i32::try_from(created["payload"]["character"]["index"].as_i64().unwrap()).unwrap();
        send(
            &mut socket,
            json!({"type":"startGame","characterIndex":index}),
        )
        .await;
        let (credential, mut events) =
            read_until(&mut socket, |e| e["type"] == "resumeCredential").await;
        assert!(events
            .iter()
            .any(|e| packet(e, "StartGame") && e["payload"]["result"] == 4));
        if !events.iter().any(|e| e["type"] == "worldSnapshot") {
            events.extend(
                read_until(&mut socket, |e| e["type"] == "worldSnapshot")
                    .await
                    .1,
            );
        }
        let snapshot = events
            .iter()
            .find(|e| e["type"] == "worldSnapshot")
            .unwrap();
        Started {
            socket,
            key: GatewaySessionCacheKey {
                account_id: self.account.clone(),
                character_index: index,
            },
            credential: credential["credential"].as_str().unwrap().to_owned(),
            generation: credential["generation"].as_u64().unwrap(),
            protected: protected(&snapshot["payload"]),
        }
    }
    async fn block_old_save(&mut self, started: &mut Started) {
        started.socket.close(None).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(10), self.entered.take().unwrap())
                .await
                .unwrap()
                .is_ok(),
            "the actual old WS reached frozen File-save I/O"
        );
        assert!(!*self.gate.release.0.lock().unwrap());
        assert_eq!(
            self.store.len(),
            0,
            "persistence has not published a resumable lease"
        );
    }
    async fn request_resume(&self, credential: &str) -> Socket {
        let mut socket = self.connect().await;
        send(
            &mut socket,
            json!({"type":"resumeSession","credential":credential}),
        )
        .await;
        socket
    }
    async fn waiters(&self, key: &GatewaySessionCacheKey, count: usize) {
        wait_for(
            || self.store.handoffs.waiter_count_for_test(key) == Some(count),
            "handler waiter count",
        )
        .await;
    }
    async fn finish(&mut self, socket: &mut Socket) {
        send(socket, json!({"type":"logOut"})).await;
        read_until(socket, |e| packet(e, "LogOutSuccess")).await;
        socket.close(None).await.unwrap();
        wait_for(
            || {
                let s = self.capacity.status();
                s.current_ws_connections == 0
                    && s.current_active_sessions == 0
                    && s.current_reconnect_leases == 0
            },
            "all isolated socket/session/lease permits released",
        )
        .await;
        assert_eq!(self.store.len(), 0);
        self.gate.release();
        if let Some(server) = self.server.take() {
            server.abort();
            let _ = server.await;
        }
        crate::session::save_recovery::release_directory_lease_for_tests(
            &self.root.join("recovery"),
        );
        let _ = &self.config;
        let temp = std::env::temp_dir().canonicalize().unwrap();
        let target = self.root.canonicalize().unwrap();
        assert_eq!(target.parent(), Some(temp.as_path()));
        assert!(target
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("mir2-native-pending-ws-"));
        std::fs::remove_dir_all(target).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.gate.release();
        if let Some(server) = self.server.take() {
            server.abort();
        }
        // A failed run keeps its isolated File/recovery data; never delete beneath a still-draining task.
    }
}
async fn send(socket: &mut Socket, value: Value) {
    socket
        .send(ClientMessage::Text(
            serde_json::to_string(&value).unwrap().into(),
        ))
        .await
        .unwrap();
}
fn packet(event: &Value, name: &str) -> bool {
    event["type"] == "packet" && event["name"] == name
}
async fn read_until(
    socket: &mut Socket,
    predicate: impl Fn(&Value) -> bool,
) -> (Value, Vec<Value>) {
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut events = Vec::new();
        loop {
            match socket
                .next()
                .await
                .expect("test socket remains connected")
                .expect("read test WS frame")
            {
                ClientMessage::Text(text) => {
                    let event: Value = serde_json::from_str(&text).expect("JSON event");
                    let matches = predicate(&event);
                    assert!(
                        event["type"] != "resumeRejected" || matches,
                        "valid pending-save resume must not be rejected"
                    );
                    events.push(event.clone());
                    if matches {
                        return (event, events);
                    }
                }
                ClientMessage::Ping(bytes) => {
                    socket.send(ClientMessage::Pong(bytes)).await.unwrap()
                }
                ClientMessage::Close(_) => panic!("test WS closed before the expected event"),
                _ => {}
            }
        }
    })
    .await
    .expect("bounded test WS event deadline")
}
async fn wait_for(mut predicate: impl FnMut() -> bool, label: &str) {
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await;
    assert!(result.is_ok(), "{label}");
}
fn protected(snapshot: &Value) -> Value {
    let mut projection = serde_json::Map::new();
    for key in [
        "mapFileName",
        "playerObjectId",
        "gold",
        "credit",
        "inventoryItems",
        "equipmentItems",
        "questLog",
        "knownSkills",
        "playerExperience",
        "playerMaxExperience",
        "beltItems",
        "storageItems",
        "heroInventoryItems",
        "heroEquipmentItems",
    ] {
        projection.insert(
            key.into(),
            snapshot
                .get(key)
                .expect("authoritative snapshot protected field")
                .clone(),
        );
    }
    let self_player = snapshot["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entity| entity["objectId"] == snapshot["playerObjectId"])
        .expect("authoritative self player entity");
    for key in ["x", "y", "direction", "class", "gender", "level"] {
        projection.insert(
            format!("self:{key}"),
            self_player.get(key).expect("self identity or transform").clone(),
        );
    }
    Value::Object(projection)
}
async fn assert_resumed(f: &Fixture, started: &Started, socket: &mut Socket) {
    let (rotated, mut events) = read_until(socket, |e| e["type"] == "resumeCredential").await;
    if !events.iter().any(|e| e["type"] == "worldSnapshot") {
        events.extend(read_until(socket, |e| e["type"] == "worldSnapshot").await.1);
    }
    let resumed = events
        .iter()
        .position(|e| e["type"] == "sessionResumed")
        .expect("resume acknowledgment");
    let snapshot = events
        .iter()
        .position(|e| e["type"] == "worldSnapshot")
        .expect("authoritative resume snapshot");
    assert!(resumed < snapshot);
    assert!(
        rotated["credential"].as_str().unwrap() != started.credential,
        "credential rotates without printing tokens"
    );
    assert_eq!(rotated["generation"].as_u64(), Some(started.generation + 1));
    assert!(
        protected(&events[snapshot]["payload"]) == started.protected,
        "personal authoritative state is preserved"
    );
    assert!(!events
        .iter()
        .skip(resumed + 1)
        .any(|e| packet(e, "ObjectChat") && e["payload"]["text"] == "pending-resume-no-replay"));
    assert_eq!(f.store.len(), 0);
    assert_eq!(f.capacity.status().current_active_sessions, 1);
    assert_eq!(f.capacity.status().current_reconnect_leases, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_resume_waits_inside_real_handler_for_pending_file_save_then_rejects_old_credential()
{
    let mut f = Fixture::new().await;
    let mut started = f.start_character().await;
    send(
        &mut started.socket,
        json!({"type":"chat","message":"pending-resume-no-replay"}),
    )
    .await;
    f.block_old_save(&mut started).await;
    let mut resumed = f.request_resume(&started.credential).await;
    f.waiters(&started.key, 1).await;
    assert!(
        !*f.gate.release.0.lock().unwrap(),
        "handler subscription is observed while save remains blocked"
    );
    assert_eq!(f.store.len(), 0);
    f.gate.release();
    assert_resumed(&f, &started, &mut resumed).await;
    let mut replay = f.request_resume(&started.credential).await;
    read_until(&mut replay, |e| e["type"] == "resumeRejected").await;
    let _ = replay.close(None).await;
    drop(replay);
    f.finish(&mut resumed).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelled_real_ws_waiter_does_not_consume_the_credential_or_another_resume_request() {
    let mut f = Fixture::new().await;
    let mut started = f.start_character().await;
    f.block_old_save(&mut started).await;
    let mut cancelled = f.request_resume(&started.credential).await;
    f.waiters(&started.key, 1).await;
    let _ = cancelled.close(None).await;
    drop(cancelled);
    f.waiters(&started.key, 0).await;
    assert!(!*f.gate.release.0.lock().unwrap());
    assert_eq!(f.store.len(), 0);
    let mut successor = f.request_resume(&started.credential).await;
    f.waiters(&started.key, 1).await;
    assert!(
        !*f.gate.release.0.lock().unwrap(),
        "successor has reached the handler before save completes"
    );
    f.gate.release();
    assert_resumed(&f, &started, &mut successor).await;
    f.finish(&mut successor).await;
}

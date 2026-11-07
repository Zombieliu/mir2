use super::*;
use crate::cache::{GatewaySessionCache, InMemoryGatewaySessionCache};
use crate::routing::{
    InProcessZoneOwnerCommandClient, PreparedZoneTeardown, ZoneOwnerCommandClient,
    ZoneOwnerCommandRequest, ZoneRegistry,
};
use mir2_protocol::{MirClass, MirGender};
use mir2_simulation::{WorldCommandExecution, ZoneRuntimeHandle};
use std::path::PathBuf;
use std::sync::Condvar;
use tokio::sync::oneshot;

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
    config: crate::GatewayConfig,
    session: Option<GatewaySession>,
    store: Arc<ReconnectSessionStore>,
    capacity: Arc<GatewayCapacityState>,
    cache: InMemoryGatewaySessionCache,
    terminal: transport::TransportSignal,
    binding: SocketReconnectBinding,
    key: GatewaySessionCacheKey,
}

impl Fixture {
    fn new() -> Self {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "mir2-reconnect-wait-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let config = crate::GatewayConfig::default()
            .with_account_store_path(root.join("accounts.json"))
            .with_save_recovery_dir(root.join("recovery"))
            .with_save_recovery_mac_key(std::array::from_fn::<u8, 32, _>(|index| {
                0x73u8.wrapping_add(index as u8)
            }))
            .unwrap();
        let registry = ZoneRegistry::in_process();
        let mut session = GatewaySession::new_with_zone_registry(config.clone(), &registry);
        let account = format!("rwfixture{sequence}");
        session
            .try_handle_packet(ClientPacket::NewAccount {
                account_id: account.clone(),
                password: "fixturePassword73".into(),
                birth_date_binary: 0,
                user_name: String::new(),
                secret_question: String::new(),
                secret_answer: String::new(),
                email_address: String::new(),
            })
            .unwrap();
        session
            .try_handle_packet(ClientPacket::Login {
                account_id: account,
                password: "fixturePassword73".into(),
            })
            .unwrap();
        let packets = session
            .try_handle_packet(ClientPacket::NewCharacter {
                name: format!("Handoff{sequence}"),
                class: MirClass::Warrior,
                gender: MirGender::Male,
            })
            .unwrap();
        let index = packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .unwrap();
        session
            .try_handle_packet(ClientPacket::StartGame {
                character_index: index,
            })
            .unwrap();
        let key = session_cache_key(&session).unwrap();
        let cache = InMemoryGatewaySessionCache::default();
        refresh_session_cache_with_route_lease(&cache, &session, 60).unwrap();
        let store = Arc::new(ReconnectSessionStore::default());
        let terminal = transport::TransportSignal::new();
        let mut binding = None;
        synchronize_socket_reconnect_binding(&session, &store, terminal.clone(), &mut binding)
            .unwrap();
        Self {
            root,
            config,
            session: Some(session),
            store,
            capacity: Arc::new(GatewayCapacityState::with_limits(None, Some(4), Some(4))),
            cache,
            terminal,
            binding,
            key,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.binding.take();
        self.session.take();
        self.store.state.lock().unwrap().sessions.clear();
        crate::session::save_recovery::release_directory_lease_for_tests(
            &self.root.join("recovery"),
        );
        let temp = std::env::temp_dir().canonicalize().unwrap();
        if let Ok(target) = self.root.canonicalize() {
            assert_eq!(target.parent(), Some(temp.as_path()));
            assert!(target
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("mir2-reconnect-wait-"));
            std::fs::remove_dir_all(target).unwrap();
        }
    }
}

#[derive(Debug)]
struct ControlledSave {
    inner: InProcessZoneOwnerCommandClient,
    entered: Mutex<Option<oneshot::Sender<()>>>,
    release: (Mutex<bool>, Condvar),
    fail: bool,
}

struct ReleaseOnDrop(Arc<ControlledSave>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

impl ControlledSave {
    fn new(fail: bool) -> (Arc<Self>, oneshot::Receiver<()>) {
        let (tx, rx) = oneshot::channel();
        (
            Arc::new(Self {
                inner: InProcessZoneOwnerCommandClient::new(),
                entered: Mutex::new(Some(tx)),
                release: (Mutex::new(false), Condvar::new()),
                fail,
            }),
            rx,
        )
    }
    fn release(&self) {
        *self.release.0.lock().unwrap() = true;
        self.release.1.notify_all();
    }
}

impl ZoneOwnerCommandClient for ControlledSave {
    fn execute(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.inner.execute(runtime, request)
    }
    fn persist_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        prepared: &PreparedZoneTeardown,
    ) -> Result<(), String> {
        if let Some(entered) = self.entered.lock().unwrap().take() {
            let _ = entered.send(());
        }
        let mut released = self.release.0.lock().unwrap();
        while !*released {
            released = self.release.1.wait(released).unwrap();
        }
        drop(released);
        if self.fail {
            return Err("controlled checkpoint failure".into());
        }
        self.inner.persist_teardown_checkpoint(runtime, prepared)
    }
}

// This is the actual persistence and thaw sequence, using the same production
// methods and store publication. Only the owner I/O completion is controlled.
async fn save_and_publish(f: &mut Fixture) -> WebTeardownPersistenceOutcome {
    let session = f.session.as_mut().unwrap();
    let mut queue = WebSessionSaveQueue::new(GatewaySaveQueueConfig::from_env());
    let outcome = persist_web_session_before_teardown(session, &mut queue).await;
    let mut resume = NativeResumeConnectionState::new();
    if !apply_teardown_persistence_to_resume(
        outcome,
        &mut resume,
        &f.store,
        Some(&f.key.account_id),
    ) {
        f.binding.take();
        return outcome;
    }
    session.release_teardown_for_resume().unwrap();
    let session = f.session.take().unwrap();
    f.store.store_with_handoff(
        f.key.clone(),
        session,
        Some(f.capacity.try_acquire_active_session().unwrap()),
        f.capacity.try_acquire_reconnect_lease().unwrap(),
        None,
        Duration::from_secs(30),
        f.binding.take().map(|(_, binding)| binding),
    );
    outcome
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reconnect_handoff_waits_for_real_checkpoint_persistence_and_replays_without_world_mutation(
) {
    let mut f = Fixture::new();
    let before = serde_json::to_value(f.session.as_ref().unwrap().world_snapshot()).unwrap();
    assert!(before.get("gold").is_some());
    assert!(before.get("inventoryItems").is_some());
    assert!(before["entities"].is_array());
    let owner = f.session.as_ref().unwrap().session_id().to_owned();
    let (controlled, entered) = ControlledSave::new(false);
    let _release_on_drop = ReleaseOnDrop(Arc::clone(&controlled));
    f.session
        .as_mut()
        .unwrap()
        .replace_teardown_owner_for_test(controlled.clone());
    f.terminal.end(transport::TransportEnd::Closed);
    let waiter = f.store.handoffs.waiter(&f.key, &owner).unwrap();
    let token = waiter.token();
    let mut waiting = Box::pin(waiter.wait());
    let store = Arc::clone(&f.store);
    let key = f.key.clone();
    let saving = tokio::spawn(async move {
        let outcome = save_and_publish(&mut f).await;
        (f, outcome)
    });
    entered.await.unwrap();
    assert!(matches!(
        futures_util::poll!(waiting.as_mut()),
        std::task::Poll::Pending
    ));
    assert!(
        store.take(&key).is_none(),
        "a checkpoint still blocked in the owner has no restorable session"
    );
    assert!(
        store.state.try_lock().is_ok(),
        "waiting does not hold the store lock"
    );
    controlled.release();
    let (f, outcome) = saving.await.unwrap();
    assert!(matches!(outcome, WebTeardownPersistenceOutcome::Saved));
    assert!(waiting.await.unwrap().same(&token));
    let verified = VerifiedIdentitySession {
        account_id: key.account_id.clone(),
        session_id: "fixture-identity".into(),
        expires_at_ms: u64::MAX,
    };
    let authorization = store
        .capture_handoff_authorization(&verified, gateway_unix_ms())
        .unwrap();
    let mut restored = store
        .take_after_handoff(&key, &token, &authorization, gateway_unix_ms())
        .unwrap();
    let packets = restored
        .session
        .replay_retained_start_game_bootstrap(&key.account_id, key.character_index, true)
        .unwrap();
    assert!(matches!(
        packets.first(),
        Some(ServerPacket::StartGame { result: 4, .. })
    ));
    let after = serde_json::to_value(restored.session.world_snapshot()).unwrap();
    assert_eq!(after["gold"], before["gold"]);
    assert_eq!(after["inventoryItems"], before["inventoryItems"]);
    assert_eq!(after["entities"], before["entities"]);
    assert!(store
        .take_after_handoff(&key, &token, &authorization, gateway_unix_ms())
        .is_none());
    drop(restored);
    drop(f);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reconnect_handoff_journaled_checkpoint_never_announces_ready_or_restores() {
    let mut f = Fixture::new();
    let owner = f.session.as_ref().unwrap().session_id().to_owned();
    let (controlled, entered) = ControlledSave::new(true);
    let _release_on_drop = ReleaseOnDrop(Arc::clone(&controlled));
    f.session
        .as_mut()
        .unwrap()
        .replace_teardown_owner_for_test(controlled.clone());
    f.terminal.end(transport::TransportEnd::Closed);
    let waiter = f.store.handoffs.waiter(&f.key, &owner).unwrap();
    let store = Arc::clone(&f.store);
    let key = f.key.clone();
    let saving = tokio::spawn(async move {
        let outcome = save_and_publish(&mut f).await;
        (f, outcome)
    });
    entered.await.unwrap();
    controlled.release();
    let (f, outcome) = saving.await.unwrap();
    assert!(matches!(outcome, WebTeardownPersistenceOutcome::Journaled));
    assert!(waiter.wait().await.is_err());
    assert!(store.take(&key).is_none());
    drop(f);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reconnect_handoff_is_visible_while_committed_movement_drain_is_still_blocked() {
    let mut f = Fixture::new();
    let owner = f.session.as_ref().unwrap().session_id().to_owned();
    let drain = Arc::new(transport::MovementDrain::default());
    let call = drain.begin().unwrap();
    f.terminal.end(transport::TransportEnd::Closed);
    let waiter = f.store.handoffs.waiter(&f.key, &owner).unwrap();
    let mut waiting = Box::pin(waiter.wait());
    let draining = tokio::spawn(async move {
        drain.close_and_wait().await;
        let outcome = save_and_publish(&mut f).await;
        (f, outcome)
    });
    assert!(matches!(
        futures_util::poll!(waiting.as_mut()),
        std::task::Poll::Pending
    ));
    drop(call);
    let (f, outcome) = draining.await.unwrap();
    assert!(matches!(outcome, WebTeardownPersistenceOutcome::Saved));
    assert!(waiting.await.is_ok());
    drop(f);
}

#[test]
fn reconnect_handoff_live_role_still_denies_a_second_route_without_waiting() {
    let f = Fixture::new();
    let owner = f.session.as_ref().unwrap().session_id();
    assert!(f.store.handoffs.waiter(&f.key, owner).is_none());
    let registry = ZoneRegistry::in_process();
    let newcomer = GatewaySession::new_with_zone_registry(f.config.clone(), &registry);
    assert!(try_acquire_start_game_route_lease(
        &f.cache,
        &newcomer,
        true,
        Some(&f.key.account_id),
        Some(f.key.character_index),
    )
    .is_err());
    assert_eq!(
        f.cache.get(&f.key).unwrap().route_lease_owner.as_deref(),
        Some(owner)
    );
}

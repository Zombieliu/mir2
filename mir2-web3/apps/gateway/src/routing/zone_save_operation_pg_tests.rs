//! Actual Hosted -> shared Zone -> simulation -> PostgreSQL lifecycle checks.
//! No transport mock or fabricated committed receipt is used in this leaf.
use super::*;
use crate::zone_lease::PostgresZoneOwnerLeaseAuthority;
use crate::zone_rpc::{TcpZoneOwnerRpcTransport, ZoneHostServer, ZoneRpcLimits};
use mir2_simulation::{AccountStoreDatabaseMode, SimulationConfig, ZoneSaveFence};
use postgres::{Client, Config as PostgresConfig, NoTls};
use serde_json::Value;
use std::net::{SocketAddr, TcpListener};

struct RunningTcpHost {
    address: SocketAddr,
    server: Arc<ZoneHostServer>,
    stop: Arc<AtomicBool>,
    completed: std::sync::mpsc::Receiver<()>,
    handle: Option<thread::JoinHandle<std::io::Result<()>>>,
    limits: ZoneRpcLimits,
}
impl RunningTcpHost {
    fn start(
        db: &Database,
        config: SimulationConfig,
        authority: SharedZoneOwnerLeaseAuthority,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let limits = ZoneRpcLimits {
            max_connections: 16,
            max_sessions: 4,
            max_sessions_per_zone: 4,
            io_timeout: Duration::from_secs(5),
            ..ZoneRpcLimits::default()
        };
        let mut factory = SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_secs(3600),
            BTreeMap::new(),
        );
        factory.autonomous_ticks_by_default = false;
        let server = Arc::new(ZoneHostServer::with_identity_and_factory(
            "hosted-owner-a",
            1,
            config,
            authority,
            Some("owned-save-pg-loopback".into()),
            limits.clone(),
            Arc::new(factory),
        ));
        server.configure_zone_map_catalog(
            BTreeMap::new(),
            BTreeSet::from([db.zone.as_str().to_string()]),
        );
        let stop = Arc::new(AtomicBool::new(false));
        let running = server.clone();
        let running_stop = stop.clone();
        let (sender, completed) = std::sync::mpsc::sync_channel(1);
        let handle = thread::spawn(move || {
            let result = running.serve_until(listener, running_stop);
            let _ = sender.send(());
            result
        });
        Self {
            address,
            server,
            stop,
            completed,
            handle: Some(handle),
            limits,
        }
    }
    fn transport(&self, db: &Database, session_id: &str) -> TcpZoneOwnerRpcTransport {
        TcpZoneOwnerRpcTransport::with_explicit_options(
            self.address,
            db.zone.clone(),
            session_id.into(),
            Some("owned-save-pg-loopback".into()),
            self.limits.clone(),
        )
    }
    fn shutdown(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.stop.store(true, Ordering::Release);
            self.completed
                .recv_timeout(Duration::from_secs(2))
                .expect("owned TCP host stop must complete within two seconds");
            handle
                .join()
                .expect("owned TCP host thread must not panic")
                .expect("owned TCP host must stop cleanly");
            let deadline = Instant::now() + Duration::from_secs(2);
            while self.server.health().active_connections != 0 && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(
                self.server.health().active_connections,
                0,
                "explicit non-reused RPC sockets must finish before fixture cleanup"
            );
        }
    }
}
impl Drop for RunningTcpHost {
    fn drop(&mut self) {
        self.shutdown();
    }
}

static PG_LOCK: Mutex<()> = Mutex::new(());

fn checked_fixture_url(value: &str) -> Result<PostgresConfig, ()> {
    if !value.starts_with("postgres://") && !value.starts_with("postgresql://") {
        return Err(());
    }
    let parsed: PostgresConfig = value.parse().map_err(|_| ())?;
    if parsed.get_hosts().len() != 1
        || parsed.get_ports().len() != 1
        || matches!(parsed.get_ports()[0], 0 | 5432)
        || parsed.get_options().is_some()
        || !parsed
            .get_dbname()
            .and_then(|name| name.strip_prefix("mir2_zone_lease_test_"))
            .is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_')
            })
    {
        return Err(());
    }
    match &parsed.get_hosts()[0] {
        postgres::config::Host::Tcp(host)
            if host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback()) =>
        {
            Ok(parsed)
        }
        _ => Err(()),
    }
}

struct Database {
    url: String,
    source_url: String,
    schema: String,
    zone: ZoneId,
}
impl Database {
    fn from_env() -> Option<Self> {
        let url = match std::env::var("MIR2_TEST_ZONE_SAVE_OPERATION_URL") {
            Ok(url) => url,
            Err(std::env::VarError::NotPresent) => return None,
            Err(_) => panic!("Hosted operation fixture URL must be Unicode"),
        };
        checked_fixture_url(&url)
            .unwrap_or_else(|_| panic!("unsafe configured Hosted operation fixture URL"));
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("hosted_zone_save_{}_{stamp}", std::process::id());
        let separator = if url.contains('?') { '&' } else { '?' };
        let source_url = format!("{url}{separator}options=-c%20search_path%3D{schema}");
        let db = Self {
            url,
            source_url,
            schema: schema.clone(),
            zone: ZoneId::new(format!("{schema}:zone")),
        };
        let mut admin = db.admin();
        // Fixture provisioning precedes every Hosted mutation. Names are owned
        // by this test and cleanup never touches another test's public lease.
        mir2_simulation::apply_migrations(&mut admin).unwrap();
        admin
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let mut source = db.connect();
        for (version, sql) in mir2_simulation::MIGRATIONS {
            if *version != "0003_zone_owner_leases" {
                source.batch_execute(sql).unwrap();
            }
        }
        Some(db)
    }
    fn admin(&self) -> Client {
        connect(&self.url)
    }
    fn connect(&self) -> Client {
        connect(&self.source_url)
    }
    fn source_config(&self) -> SimulationConfig {
        let mut config = SimulationConfig::default();
        config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
        config.account_store_database_url = Some(self.source_url.clone());
        config
    }
    fn operation_id(&self, name: &str) -> String {
        format!("{}:{name}", self.schema)
    }
    fn image(&self) -> (Value, i64, Value, i64) {
        let row = self
            .connect()
            .query_one(
                "SELECT a.raw_json,a.store_version,s.snapshot_json,s.save_version \
            FROM accounts a JOIN character_saves s USING(account_id) WHERE a.account_id='demo'",
                &[],
            )
            .unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }
    fn take_over(&self, old: &ZoneOwnerLease) {
        let token = i64::try_from(old.fencing_token() + 1).unwrap();
        assert_eq!(self.admin().execute("UPDATE public.zone_owner_leases SET owner_id='hosted-owner-b', \
            fencing_token=$2,expires_at_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint+120000, \
            heartbeat_at_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint WHERE zone_id=$1",
            &[&self.zone.as_str(), &token]).unwrap(), 1);
    }
    fn reject_commit(&self) {
        // The deferred trigger runs only when the real account-store tx tries
        // COMMIT. The G2 classifier deliberately treats even this known rollback
        // as Unknown. This is not a lost-acknowledgement fault claim.
        self.connect().batch_execute("CREATE FUNCTION hosted_reject_commit() RETURNS TRIGGER LANGUAGE plpgsql AS $$ \
            BEGIN RAISE EXCEPTION 'owned Hosted COMMIT rollback fixture' USING ERRCODE='23505'; END; $$; \
            CREATE CONSTRAINT TRIGGER hosted_reject_commit AFTER UPDATE ON accounts \
            DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION hosted_reject_commit()")
            .unwrap();
    }
    fn allow_commit(&self) {
        self.connect().batch_execute("DROP TRIGGER hosted_reject_commit ON accounts; DROP FUNCTION hosted_reject_commit()").unwrap();
    }
}
fn connect(url: &str) -> Client {
    let mut config: PostgresConfig = url.parse().unwrap();
    config
        .connect_timeout(Duration::from_secs(2))
        .notice_callback(|_| {});
    config
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("configured Hosted operation PostgreSQL fixture unavailable"))
}
impl Drop for Database {
    fn drop(&mut self) {
        let mut admin = self.admin();
        admin
            .execute(
                "DELETE FROM public.zone_character_save_operations WHERE zone_id=$1",
                &[&self.zone.as_str()],
            )
            .unwrap();
        admin
            .execute(
                "DELETE FROM public.zone_owner_leases WHERE zone_id=$1",
                &[&self.zone.as_str()],
            )
            .unwrap();
        admin
            .batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .unwrap();
    }
}

struct HostedFixture {
    config: SimulationConfig,
    authority: SharedZoneOwnerLeaseAuthority,
    lease: ZoneOwnerLease,
    factory: SharedInProcessZoneRuntimeFactory,
    hosted: HostedZoneOwnerCommandClient,
}
impl HostedFixture {
    fn new(db: &Database) -> Self {
        let authority: SharedZoneOwnerLeaseAuthority = Arc::new(
            PostgresZoneOwnerLeaseAuthority::new(db.url.clone(), "hosted-owner-a", 120_000),
        );
        // Connect/provision the real authority before starting mutations.
        let lease = authority.try_owner_lease(&db.zone).unwrap();
        let config = db.source_config();
        config.save_account_store().unwrap();
        let mut factory = SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_secs(3600),
            BTreeMap::new(),
        );
        factory.autonomous_ticks_by_default = false;
        let runtime = factory.create_runtime(config.clone(), &db.zone);
        let hosted =
            HostedZoneOwnerCommandClient::with_owner_lease_authority(runtime, authority.clone());
        for packet in [
            ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into(),
            },
            ClientPacket::StartGame { character_index: 0 },
        ] {
            hosted
                .execute_request(ZoneOwnerCommandRequest::direct(
                    lease.clone(),
                    WorldCommand::ClientPacket(packet),
                ))
                .unwrap();
        }
        assert!(
            hosted.active_identity().unwrap().is_some(),
            "real login/StartGame must produce an actor"
        );
        Self {
            config,
            authority,
            lease,
            factory,
            hosted,
        }
    }
    fn fenced(&self) -> bool {
        let slot = self.hosted.runtime.lock().unwrap();
        zone_teardown_is_fenced(slot.as_ref().unwrap())
    }
    fn ordinary_turn(&self) -> Result<WorldCommandExecution, String> {
        self.hosted
            .execute_request(ZoneOwnerCommandRequest::production_player(
                self.lease.clone(),
                true,
                WorldCommand::ClientPacket(ClientPacket::Turn {
                    direction: MirDirection::Left,
                }),
            ))
    }
    fn restart_query_client(&self, db: &Database) -> HostedZoneOwnerCommandClient {
        let fence = ZoneSaveFence::new(
            db.zone.as_str().into(),
            self.lease.owner_id().into(),
            self.lease.fencing_token(),
        )
        .unwrap();
        let config = db.source_config().with_zone_save_fence(fence).unwrap();
        // A new actual runtime has no selected actor or pending-operation map.
        // Its Query must reconstruct the exact complete receipt from PostgreSQL.
        HostedZoneOwnerCommandClient::with_owner_lease_authority(
            Box::new(InProcessWorldRuntime::new(config)),
            self.authority.clone(),
        )
    }
}

#[test]
fn hosted_zone_save_postgres_real_tcp_lifecycle_and_new_host_query_admits_no_session() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let authority: SharedZoneOwnerLeaseAuthority = Arc::new(PostgresZoneOwnerLeaseAuthority::new(
        db.url.clone(),
        "hosted-owner-a",
        120_000,
    ));
    let lease = authority.try_owner_lease(&db.zone).unwrap();
    let config = db.source_config();
    config.save_account_store().unwrap();
    let mut running = RunningTcpHost::start(&db, config, authority.clone());
    let session_id = "rpc-actual-tcp-pg-session";
    let transport = running.transport(&db, session_id);
    assert!(transport.supports_zone_save_operations());
    assert_eq!(running.server.session_count(), 0);
    transport.on_connect().unwrap();
    for packet in [
        ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        },
        ClientPacket::StartGame { character_index: 0 },
    ] {
        transport
            .execute(ZoneOwnerCommandRequest::direct(
                lease.clone(),
                WorldCommand::ClientPacket(packet),
            ))
            .unwrap();
    }
    assert!(transport.active_identity().unwrap().is_some());
    assert_eq!(running.server.session_count(), 1);
    let prepared = transport
        .prepare_zone_save_operation(&lease, &db.operation_id("actual-tcp"))
        .unwrap()
        .unwrap();
    let binding = prepared.remote_operation_binding().unwrap().clone();
    let queried = transport
        .query_zone_save_operation(&binding)
        .unwrap()
        .unwrap();
    assert_eq!(queried.status(), ZoneSaveOperationStatus::Prepared);
    assert_eq!(
        serde_json::to_value(queried.operation().frozen_checkpoint()).unwrap(),
        serde_json::to_value(prepared.checkpoint()).unwrap()
    );
    let duplicate_prepared = transport
        .prepare_zone_save_operation(&lease, binding.operation_id())
        .unwrap()
        .unwrap();
    assert_eq!(
        duplicate_prepared.remote_operation_binding(),
        Some(&binding)
    );
    let committed = transport.persist_zone_save_operation(&binding).unwrap();
    assert_eq!(committed.status(), ZoneSaveOperationStatus::Committed);
    let durable = db.image();
    assert_eq!(
        serde_json::to_value(committed.saved_checkpoint().unwrap()).unwrap(),
        durable.2
    );
    assert_eq!(committed.account_store_version(), Some(durable.1));
    assert_eq!(committed.character_save_version(), Some(durable.3));
    assert_eq!(
        transport
            .persist_zone_save_operation(&binding)
            .unwrap()
            .to_json()
            .unwrap(),
        committed.to_json().unwrap()
    );
    assert_eq!(
        db.image(),
        durable,
        "real TCP duplicate Persist cannot repeat source CAS"
    );
    let released = transport.release_zone_save_operation(&binding).unwrap();
    assert_eq!(released.status(), ZoneSaveOperationStatus::Released);
    assert_eq!(db.image(), durable);
    drop(transport);
    running.shutdown();
    drop(running);
    // Old active Session destruction may perform its ordinary final save. The
    // following baseline is deliberately after all old server state is gone.
    let before_query = db.image();
    let restarted = RunningTcpHost::start(&db, db.source_config(), authority);
    assert_eq!(restarted.server.session_count(), 0);
    let reconciliation = restarted.transport(&db, session_id);
    let receipt = reconciliation
        .query_zone_save_operation(&binding)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.to_json().unwrap(), released.to_json().unwrap());
    assert_eq!(
        restarted.server.session_count(),
        0,
        "Query must not admit a Session after Host reconstruction"
    );
    assert_eq!(
        db.image(),
        before_query,
        "new Host receipt reconciliation is read-only"
    );
}

#[test]
fn hosted_zone_save_postgres_full_shared_lifecycle_duplicate_and_fresh_runtime_query() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let fixture = HostedFixture::new(&db);
    let identity = fixture.hosted.active_identity().unwrap().unwrap();
    let key = ZonePresenceKey::from_identity(&identity);
    let before = fixture
        .hosted
        .active_character_checkpoint()
        .unwrap()
        .unwrap();
    let final_position = Point {
        x: before.position.x + 1,
        y: before.position.y,
    };
    {
        let resources = fixture.factory.resources_for_zone(&db.zone);
        let mut state = resources.zone_state.lock().unwrap();
        let session_id = state.zone_sessions[&key].clone();
        state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
            session_id,
            position: final_position.clone(),
            direction: MirDirection::Right,
        });
    }
    let prepared = fixture
        .hosted
        .prepare_remote_zone_teardown(
            &fixture.lease,
            &db.operation_id("normal"),
            "rpc-hosted-normal",
        )
        .unwrap()
        .unwrap();
    assert_eq!(prepared.status(), ZoneSaveOperationStatus::Prepared);
    assert_eq!(
        prepared.operation().frozen_checkpoint().position,
        final_position
    );
    assert!(fixture.fenced());
    assert!(fixture.ordinary_turn().is_err());
    let duplicate = fixture
        .hosted
        .prepare_remote_zone_teardown(
            &fixture.lease,
            prepared.operation().operation_id(),
            prepared.operation().rpc_session_id(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(duplicate.operation(), prepared.operation());
    let committed = fixture
        .hosted
        .persist_remote_zone_teardown(prepared.operation().binding())
        .unwrap();
    assert_eq!(committed.status(), ZoneSaveOperationStatus::Committed);
    let durable = db.image();
    assert_eq!(
        serde_json::to_value(committed.saved_checkpoint().unwrap()).unwrap(),
        durable.2
    );
    assert_eq!(committed.account_store_version(), Some(durable.1));
    assert_eq!(committed.character_save_version(), Some(durable.3));
    assert_eq!(
        fixture
            .hosted
            .persist_remote_zone_teardown(prepared.operation().binding())
            .unwrap()
            .status(),
        ZoneSaveOperationStatus::Committed
    );
    assert_eq!(
        db.image(),
        durable,
        "duplicate Persist cannot apply a second CAS"
    );
    let restarted = fixture.restart_query_client(&db);
    assert_eq!(
        restarted
            .query_remote_zone_teardown(prepared.operation().binding())
            .unwrap()
            .unwrap()
            .operation(),
        prepared.operation()
    );
    assert_eq!(
        db.image(),
        durable,
        "fresh-runtime Query must not save anything"
    );
    assert!(
        restarted
            .release_remote_zone_teardown(prepared.operation().binding())
            .is_err(),
        "receipt read alone grants no local thaw authority"
    );
    let released = fixture
        .hosted
        .release_remote_zone_teardown(prepared.operation().binding())
        .unwrap();
    assert_eq!(released.status(), ZoneSaveOperationStatus::Released);
    assert!(!fixture.fenced());
    assert!(fixture.ordinary_turn().is_ok());
    assert_eq!(
        fixture
            .hosted
            .release_remote_zone_teardown(prepared.operation().binding())
            .unwrap()
            .status(),
        ZoneSaveOperationStatus::Released
    );
    assert_eq!(db.image(), durable);
}

#[test]
fn hosted_zone_save_postgres_takeover_blocks_prepared_publication_and_retains_presence_fence() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let fixture = HostedFixture::new(&db);
    let prepared = fixture
        .hosted
        .prepare_remote_zone_teardown(
            &fixture.lease,
            &db.operation_id("takeover"),
            "rpc-hosted-takeover",
        )
        .unwrap()
        .unwrap();
    let before = db.image();
    db.take_over(&fixture.lease);
    assert!(fixture
        .hosted
        .persist_remote_zone_teardown(prepared.operation().binding())
        .is_err());
    assert!(fixture
        .hosted
        .release_remote_zone_teardown(prepared.operation().binding())
        .is_err());
    assert!(fixture.ordinary_turn().is_err());
    assert!(fixture.fenced());
    let restarted = fixture.restart_query_client(&db);
    let receipt = restarted
        .query_remote_zone_teardown(prepared.operation().binding())
        .unwrap()
        .unwrap();
    assert_eq!(receipt.status(), ZoneSaveOperationStatus::Prepared);
    assert_eq!(receipt.operation(), prepared.operation());
    assert_eq!(db.image(), before);
}

#[test]
fn hosted_zone_save_postgres_committed_receipt_survives_takeover_as_readonly_evidence() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let fixture = HostedFixture::new(&db);
    let prepared = fixture
        .hosted
        .prepare_remote_zone_teardown(
            &fixture.lease,
            &db.operation_id("committed-takeover"),
            "rpc-hosted-committed-takeover",
        )
        .unwrap()
        .unwrap();
    let committed = fixture
        .hosted
        .persist_remote_zone_teardown(prepared.operation().binding())
        .unwrap();
    let before = db.image();
    db.take_over(&fixture.lease);
    let restarted = fixture.restart_query_client(&db);
    let queried = restarted
        .query_remote_zone_teardown(prepared.operation().binding())
        .unwrap()
        .unwrap();
    assert_eq!(queried.status(), ZoneSaveOperationStatus::Committed);
    assert_eq!(queried.to_json().unwrap(), committed.to_json().unwrap());
    // Persist can return known evidence, but cannot repeat any old source CAS.
    assert_eq!(
        fixture
            .hosted
            .persist_remote_zone_teardown(prepared.operation().binding())
            .unwrap()
            .status(),
        ZoneSaveOperationStatus::Committed
    );
    assert!(fixture
        .hosted
        .release_remote_zone_teardown(prepared.operation().binding())
        .is_err());
    assert!(fixture.ordinary_turn().is_err());
    assert!(fixture.fenced());
    assert_eq!(db.image(), before);
}

#[test]
fn hosted_zone_save_postgres_deferred_commit_rollback_unknown_stays_fenced_without_replay() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let fixture = HostedFixture::new(&db);
    let prepared = fixture
        .hosted
        .prepare_remote_zone_teardown(
            &fixture.lease,
            &db.operation_id("rollback-unknown"),
            "rpc-hosted-unknown",
        )
        .unwrap()
        .unwrap();
    let before = db.image();
    let live_before = serde_json::to_value(&*fixture.config.account_store.lock().unwrap()).unwrap();
    let active_revision = fixture
        .hosted
        .active_character_checkpoint()
        .unwrap()
        .unwrap()
        .revision;
    db.reject_commit();
    let error = fixture
        .hosted
        .persist_remote_zone_teardown(prepared.operation().binding())
        .unwrap_err();
    assert!(error.contains("ZONE_SAVE_COMMIT_OUTCOME_UNKNOWN"));
    db.allow_commit();
    assert_eq!(
        db.image(),
        before,
        "real failed COMMIT must roll back account/save/receipt together"
    );
    assert_eq!(
        serde_json::to_value(&*fixture.config.account_store.lock().unwrap()).unwrap(),
        live_before
    );
    assert_eq!(
        fixture
            .hosted
            .active_character_checkpoint()
            .unwrap()
            .unwrap()
            .revision,
        active_revision
    );
    assert_eq!(
        fixture
            .hosted
            .query_remote_zone_teardown(prepared.operation().binding())
            .unwrap()
            .unwrap()
            .status(),
        ZoneSaveOperationStatus::Prepared
    );
    assert!(fixture
        .hosted
        .persist_remote_zone_teardown(prepared.operation().binding())
        .unwrap_err()
        .contains("writes are frozen"));
    assert!(fixture
        .config
        .save_account_store()
        .unwrap_err()
        .contains("writes are frozen"));
    assert!(fixture
        .hosted
        .release_remote_zone_teardown(prepared.operation().binding())
        .is_err());
    assert!(fixture.ordinary_turn().is_err());
    assert!(fixture.fenced());
    assert_eq!(
        db.image(),
        before,
        "visible Prepared alone cannot authorize replay after Unknown"
    );
}

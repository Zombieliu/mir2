use super::*;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn limits_reject_zero_unbounded_and_inverted_timeouts() {
    let defaults = PostgresObserverLimits::default();
    assert_eq!(defaults.validate(), Ok(defaults));
    for limits in [
        PostgresObserverLimits {
            connect_timeout: Duration::ZERO,
            ..defaults
        },
        PostgresObserverLimits {
            query_timeout: Duration::from_secs(31),
            ..defaults
        },
        PostgresObserverLimits {
            request_timeout: Duration::MAX,
            ..defaults
        },
        PostgresObserverLimits {
            lock_timeout: defaults.query_timeout + Duration::from_millis(1),
            ..defaults
        },
        PostgresObserverLimits {
            queue_capacity: 0,
            ..defaults
        },
        PostgresObserverLimits {
            queue_capacity: MAX_QUEUE_CAPACITY + 1,
            ..defaults
        },
    ] {
        assert_eq!(limits.validate(), Err(ResolveError::ObserverUnavailable));
    }
}

#[test]
fn invalid_dsn_and_defaults_fail_without_connecting_or_secret_output() {
    let limits = PostgresObserverLimits::default();
    for dsn in [
        "",
        "invalid secret password=private-password",
        "host=127.0.0.1 dbname=fixture",
        "host=127.0.0.1 user=reader",
        "user=reader dbname=fixture",
        "host=localhost user=reader dbname=fixture",
        "host=127.0.0.1,127.0.0.2 user=reader dbname=fixture",
    ] {
        let error = match reader_config(dsn, limits) {
            Ok(_) => panic!("unsafe or incomplete DSN was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.to_string(), "zone_owner_observer_unavailable");
        assert!(!format!("{error:?}").contains("private-password"));
    }
}

#[test]
fn configuration_overrides_dsn_options_and_sets_finite_network_server_limits() {
    let limits = PostgresObserverLimits::default();
    let config = reader_config(
        "host=127.0.0.1 user=reader dbname=fixture options='-c role=writer -c default_transaction_read_only=off -c statement_timeout=0'",
        limits,
    ).unwrap_or_else(|_| panic!("test reader configuration failed"));
    let options = config.get_options().unwrap();
    assert!(options.contains("default_transaction_read_only=on"));
    assert!(options.contains("search_path=pg_catalog"));
    assert!(options.contains("statement_timeout=500"));
    assert!(options.contains("lock_timeout=100"));
    assert!(options.contains("idle_in_transaction_session_timeout=2000"));
    assert!(!options.contains("role=writer"));
    assert_eq!(config.get_connect_timeout(), Some(&limits.connect_timeout));
    assert_eq!(config.get_tcp_user_timeout(), Some(&limits.request_timeout));
}

fn request(deadline: Instant) -> (ObserveRequest, Receiver<Observation>) {
    let (reply, response) = mpsc::sync_channel(1);
    (
        ObserveRequest {
            zone_id: "test-zone".to_owned(),
            deadline,
            reply,
        },
        response,
    )
}

#[test]
fn bounded_queue_rejects_saturation_without_waiting() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let deadline = Instant::now() + Duration::from_secs(1);
    let (first, _first_reply) = request(deadline);
    enqueue(&sender, first).unwrap();
    let start = Instant::now();
    let (second, _second_reply) = request(deadline);
    assert_eq!(
        enqueue(&sender, second),
        Err(ResolveError::ObserverUnavailable)
    );
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(receiver.try_iter().count(), 1);
}

#[test]
fn expired_queue_work_is_never_admitted() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let (expired, _reply) = request(Instant::now());
    assert_eq!(
        enqueue(&sender, expired),
        Err(ResolveError::ObserverUnavailable)
    );
    assert!(receiver.try_recv().is_err());
}

#[test]
fn disconnected_worker_and_reply_fail_closed() {
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(receiver);
    let (pending, _reply) = request(Instant::now() + Duration::from_secs(1));
    assert_eq!(
        enqueue(&sender, pending),
        Err(ResolveError::ObserverUnavailable)
    );
    let (reply, response) = mpsc::sync_channel::<Observation>(1);
    drop(reply);
    assert_eq!(
        receive_before(&response, Instant::now() + Duration::from_secs(1)),
        Err(ResolveError::ObserverUnavailable)
    );
}

#[test]
fn caller_deadline_bounds_a_stalled_worker() {
    let (_reply, response) = mpsc::sync_channel::<Observation>(1);
    let start = Instant::now();
    assert_eq!(
        receive_before(&response, start + Duration::from_millis(20)),
        Err(ResolveError::ObserverUnavailable)
    );
    assert!(start.elapsed() < Duration::from_millis(500));
}

#[test]
fn already_expired_reply_is_rejected_even_when_success_is_queued() {
    let (reply, response) = mpsc::sync_channel::<Observation>(1);
    reply.send(Ok(None)).unwrap();
    assert_eq!(
        receive_before(&response, Instant::now()),
        Err(ResolveError::ObserverUnavailable)
    );
}

#[test]
fn reconnect_cooldown_and_expired_deadlines_fail_without_waiting() {
    let now = Instant::now();
    assert!(!reconnect_permitted(now, now));
    assert!(!reconnect_permitted(
        now + Duration::from_secs(1),
        now + RECONNECT_COOLDOWN
    ));
    assert!(reconnect_permitted(now + Duration::from_secs(1), now));
}

#[test]
fn clones_share_worker_queue_and_do_not_disclose_zone_or_dsn() {
    let (requests, receiver) = mpsc::sync_channel(1);
    let observer = PostgresZoneLeaseObserver {
        shared: Arc::new(SharedObserver {
            requests,
            available: Arc::new(AtomicBool::new(true)),
            limits: PostgresObserverLimits::default(),
            application_name: "test-observer".to_owned(),
        }),
    };
    let clone = observer.clone();
    assert!(Arc::ptr_eq(&observer.shared, &clone.shared));
    let debug = format!("{observer:?}");
    assert!(debug.contains("PostgresZoneLeaseObserver"));
    assert!(!debug.contains("zone_id"));
    assert!(!debug.contains("requests"));
    let (pending, _reply) = request(Instant::now() + Duration::from_secs(1));
    enqueue(&observer.shared.requests, pending).unwrap();
    let (pending, _reply) = request(Instant::now() + Duration::from_secs(1));
    assert_eq!(
        enqueue(&clone.shared.requests, pending),
        Err(ResolveError::ObserverUnavailable)
    );
    drop(observer);
    drop(clone);
    drop(receiver.recv().unwrap());
    assert!(
        receiver.recv().is_err(),
        "last clone must close the worker queue"
    );
}

static PG_TEST_LOCK: Mutex<()> = Mutex::new(());
static NEXT_ZONE: AtomicU64 = AtomicU64::new(1);

struct PgFixture {
    reader_dsn: String,
    writer_dsn: String,
    writer: Client,
    zone: ZoneId,
}

fn db_ok<T>(result: Result<T, ::postgres::Error>, message: &'static str) -> T {
    result.unwrap_or_else(|_| panic!("{message}"))
}

fn fixture_client(dsn: &str) -> Client {
    let mut config: Config = dsn
        .parse()
        .unwrap_or_else(|_| panic!("configured observer fixture DSN is invalid"));
    config
        .connect_timeout(Duration::from_secs(2))
        .notice_callback(|_| {});
    db_ok(
        config.connect(NoTls),
        "configured observer fixture cannot connect",
    )
}

impl PgFixture {
    // Only the dedicated explicit fixture variable enables DB tests. A
    // configured but missing/broken writer or reader is a failure, never skip.
    fn from_env() -> Option<Self> {
        let reader_dsn = match std::env::var("MIR2_TEST_ZONE_OBSERVER_URL") {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => return None,
            Err(_) => panic!("observer reader fixture URL is not valid Unicode"),
        };
        assert!(
            !reader_dsn.trim().is_empty(),
            "observer reader fixture URL is empty"
        );
        let writer_dsn = std::env::var("MIR2_TEST_ZONE_OBSERVER_WRITER_URL")
            .unwrap_or_else(|_| panic!("explicit observer writer fixture URL is required"));
        assert!(
            !writer_dsn.trim().is_empty(),
            "observer writer fixture URL is empty"
        );
        assert_ne!(
            reader_dsn, writer_dsn,
            "observer fixtures must use separate reader and writer credentials"
        );
        let writer = fixture_client(&writer_dsn);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_ZONE.fetch_add(1, Ordering::Relaxed);
        let zone = ZoneId::new(format!(
            "observer-test-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        Some(Self {
            reader_dsn,
            writer_dsn,
            writer,
            zone,
        })
    }

    fn observer(&self) -> PostgresZoneLeaseObserver {
        PostgresZoneLeaseObserver::connect(&self.reader_dsn, PostgresObserverLimits::default())
            .unwrap_or_else(|_| panic!("configured reader must pass observer privilege audit"))
    }

    fn put(&mut self, owner: &str, token: i64, expires: i64, heartbeat: i64) {
        db_ok(self.writer.execute(
            "INSERT INTO public.zone_owner_leases(zone_id,owner_id,fencing_token,expires_at_ms,heartbeat_at_ms) \
             VALUES($1,$2,$3,$4,$5) ON CONFLICT(zone_id) DO UPDATE SET owner_id=$2,fencing_token=$3,expires_at_ms=$4,heartbeat_at_ms=$5",
            &[&self.zone.as_str(), &owner, &token, &expires, &heartbeat],
        ), "observer fixture row write failed");
    }

    fn reader_user(&self) -> String {
        let config: Config = self
            .reader_dsn
            .parse()
            .unwrap_or_else(|_| panic!("observer fixture reader DSN is invalid"));
        config
            .get_user()
            .unwrap_or_else(|| panic!("observer fixture reader user is missing"))
            .to_owned()
    }

    fn observer_backend_pid(&mut self, observer: &PostgresZoneLeaseObserver) -> i32 {
        let reader_user = self.reader_user();
        let rows = db_ok(
            self.writer.query(
                "SELECT pid FROM pg_catalog.pg_stat_activity WHERE application_name=$1 \
             AND usename=$2 AND datname=pg_catalog.current_database()",
                &[&observer.shared.application_name, &reader_user],
            ),
            "observer fixture backend lookup failed",
        );
        assert_eq!(
            rows.len(),
            1,
            "observer must own exactly one identifiable reader connection"
        );
        rows[0].get(0)
    }

    fn terminate_own_observer_backend(&mut self, observer: &PostgresZoneLeaseObserver, pid: i32) {
        let reader_user = self.reader_user();
        // Match both the observed PID and this observer's unique application
        // identity/user/database in the terminating statement. Never select or
        // terminate an unrelated process that merely uses the reader role.
        let rows = db_ok(
            self.writer.query(
                "SELECT pg_catalog.pg_terminate_backend(pid) FROM pg_catalog.pg_stat_activity \
             WHERE pid=$1 AND application_name=$2 AND usename=$3 \
             AND datname=pg_catalog.current_database() AND pid <> pg_catalog.pg_backend_pid()",
                &[&pid, &observer.shared.application_name, &reader_user],
            ),
            "observer fixture backend termination failed",
        );
        assert_eq!(
            rows.len(),
            1,
            "only the owned observer reader may be terminated"
        );
        assert!(
            rows[0].get::<_, bool>(0),
            "owned observer backend termination was rejected"
        );
    }
}

impl Drop for PgFixture {
    fn drop(&mut self) {
        let _ = self.writer.execute(
            "DELETE FROM public.zone_owner_leases WHERE zone_id=$1",
            &[&self.zone.as_str()],
        );
    }
}

#[test]
fn postgres_normal_reader_with_default_pg_settings_session_update_privilege_is_accepted() {
    let _guard = PG_TEST_LOCK.lock().unwrap();
    let Some(fixture) = PgFixture::from_env() else {
        return;
    };
    let mut reader = fixture_client(&fixture.reader_dsn);
    let privileges = db_ok(reader.query_one(
        "SELECT pg_catalog.has_table_privilege(CURRENT_USER, 'pg_catalog.pg_settings', 'UPDATE') AS view_update, \
         pg_catalog.has_any_column_privilege(CURRENT_USER, 'pg_catalog.pg_settings', 'UPDATE') AS column_update, \
         pg_catalog.has_table_privilege(CURRENT_USER, 'public.zone_owner_leases', \
             'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER') AS lease_write",
        &[],
    ), "observer fixture default session-GUC privileges could not be inspected");
    assert!(
        privileges.get::<_, bool>("view_update"),
        "normal fixture must retain PostgreSQL's default PUBLIC pg_settings UPDATE ACL"
    );
    assert!(
        privileges.get::<_, bool>("column_update"),
        "normal fixture must expose the default pg_settings column UPDATE capability"
    );
    assert!(
        !privileges.get::<_, bool>("lease_write"),
        "session-GUC capability must not imply persistent lease write permission"
    );
    let observer = fixture.observer();
    assert_eq!(observer.observe(&fixture.zone), Ok(None));
}

#[test]
fn postgres_reader_observes_live_changes_without_cache_or_migrations() {
    let _guard = PG_TEST_LOCK.lock().unwrap();
    let Some(mut fixture) = PgFixture::from_env() else {
        return;
    };
    let before: String = db_ok(fixture.writer.query_one(
        "SELECT COALESCE(pg_catalog.json_agg(t ORDER BY t.oid)::text, '[]') FROM \
         (SELECT oid,relname,relowner,relacl FROM pg_catalog.pg_class WHERE relnamespace='public'::pg_catalog.regnamespace) t", &[]),
        "observer fixture catalog fingerprint failed").get(0);
    let observer = fixture.observer();
    assert_eq!(observer.observe(&fixture.zone), Ok(None));
    fixture.put("host-a", 7, 9000, 8000);
    assert_eq!(
        observer.observe(&fixture.zone),
        Ok(Some(ObservedZoneLease {
            owner_id: "host-a".to_owned(),
            fencing_token: 7,
            expires_at_ms: 9000,
            heartbeat_at_ms: 8000,
        }))
    );
    // Observation returns expired rows too: the injected resolver clock decides
    // liveness, while every observation sees the current owner and epoch.
    fixture.put("host-b", 8, 1, 0);
    let row = observer.observe(&fixture.zone).unwrap().unwrap();
    assert_eq!(row.owner_id, "host-b");
    assert_eq!(row.fencing_token, 8);
    assert!(!row.is_live(1));
    let after: String = db_ok(fixture.writer.query_one(
        "SELECT COALESCE(pg_catalog.json_agg(t ORDER BY t.oid)::text, '[]') FROM \
         (SELECT oid,relname,relowner,relacl FROM pg_catalog.pg_class WHERE relnamespace='public'::pg_catalog.regnamespace) t", &[]),
        "observer fixture catalog fingerprint failed").get(0);
    assert_eq!(
        before, after,
        "observer must not migrate or change schema/permissions"
    );
}

#[test]
fn postgres_reader_rejects_writer_credentials_and_actual_reader_cannot_write() {
    let _guard = PG_TEST_LOCK.lock().unwrap();
    let Some(mut fixture) = PgFixture::from_env() else {
        return;
    };
    assert!(matches!(
        PostgresZoneLeaseObserver::connect(&fixture.writer_dsn, PostgresObserverLimits::default()),
        Err(ResolveError::ObserverUnavailable)
    ));
    let observer = fixture.observer();
    fixture.put("host-a", 7, 9000, 8000);
    let mut reader = fixture_client(&fixture.reader_dsn);
    db_ok(
        reader.batch_execute("SET default_transaction_read_only=off"),
        "reader fixture must permit testing ACL denial independently of transaction defaults",
    );
    for statement in [
        "UPDATE public.zone_owner_leases SET owner_id='forbidden' WHERE zone_id=$1",
        "DELETE FROM public.zone_owner_leases WHERE zone_id=$1",
        "INSERT INTO public.zone_owner_leases(zone_id,owner_id,fencing_token,expires_at_ms,heartbeat_at_ms) VALUES($1,'forbidden',1,1,1)",
    ] {
        let error = reader.execute(statement, &[&fixture.zone.as_str()])
            .err().unwrap_or_else(|| panic!("reader fixture unexpectedly accepted DML"));
        assert_eq!(error.code(), Some(&::postgres::error::SqlState::INSUFFICIENT_PRIVILEGE));
    }
    assert_eq!(
        observer.observe(&fixture.zone).unwrap().unwrap().owner_id,
        "host-a"
    );
}

#[test]
fn postgres_explicit_forbidden_role_fixtures_fail_privilege_audit() {
    let _guard = PG_TEST_LOCK.lock().unwrap();
    let Some(_fixture) = PgFixture::from_env() else {
        return;
    };
    let configured = match std::env::var("MIR2_TEST_ZONE_OBSERVER_FORBIDDEN_URLS_JSON") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return,
        Err(_) => panic!("forbidden observer fixture URLs are not valid Unicode"),
    };
    let dsns: Vec<String> = serde_json::from_str(&configured)
        .unwrap_or_else(|_| panic!("forbidden observer fixture URLs must be a JSON string array"));
    assert!(
        !dsns.is_empty(),
        "configured forbidden observer fixture URLs are empty"
    );
    for dsn in dsns {
        // Establish first that these are working credentials. Rejection due to
        // an invalid password or missing server would not test the role audit.
        assert!(
            reader_config(&dsn, PostgresObserverLimits::default()).is_ok(),
            "forbidden fixture DSN must satisfy observer transport configuration"
        );
        drop(fixture_client(&dsn));
        assert!(matches!(
            PostgresZoneLeaseObserver::connect(&dsn, PostgresObserverLimits::default()),
            Err(ResolveError::ObserverUnavailable)
        ));
    }
}

#[test]
fn postgres_invalid_stored_values_fail_closed() {
    let _guard = PG_TEST_LOCK.lock().unwrap();
    let Some(mut fixture) = PgFixture::from_env() else {
        return;
    };
    let observer = fixture.observer();
    for (owner, token, expires, heartbeat) in [
        ("host-a", 0, 1, 1),
        ("host-a", -1, 1, 1),
        ("host-a", 1, -1, 1),
        ("host-a", 1, 1, -1),
        ("bad owner", 1, 1, 1),
    ] {
        fixture.put(owner, token, expires, heartbeat);
        assert_eq!(
            observer.observe(&fixture.zone),
            Err(ResolveError::InvalidLease)
        );
    }
    fixture.put("host-a", 1, i64::MAX, 0);
    assert!(observer.observe(&fixture.zone).is_ok());
}

#[test]
fn postgres_lock_wait_has_finite_caller_budget_and_later_request_recovers() {
    let _guard = PG_TEST_LOCK.lock().unwrap();
    let Some(mut fixture) = PgFixture::from_env() else {
        return;
    };
    fixture.put("host-a", 1, 9000, 8000);
    let observer = fixture.observer();
    let mut lock = db_ok(
        fixture.writer.transaction(),
        "observer fixture lock transaction failed",
    );
    db_ok(
        lock.batch_execute("LOCK TABLE public.zone_owner_leases IN ACCESS EXCLUSIVE MODE"),
        "observer fixture table lock failed",
    );
    let started = Instant::now();
    assert_eq!(
        observer.observe(&fixture.zone),
        Err(ResolveError::ObserverUnavailable)
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    db_ok(lock.rollback(), "observer fixture lock rollback failed");
    std::thread::sleep(RECONNECT_COOLDOWN + Duration::from_millis(20));
    assert_eq!(
        observer.observe(&fixture.zone).unwrap().unwrap().owner_id,
        "host-a"
    );
}

#[test]
fn postgres_terminated_owned_reader_recovers_on_a_later_explicit_request() {
    let _guard = PG_TEST_LOCK.lock().unwrap();
    let Some(mut fixture) = PgFixture::from_env() else {
        return;
    };
    fixture.put("host-a", 7, 9000, 8000);
    let observer = fixture.observer();
    let shared_worker = observer.shared.clone();
    let first_pid = fixture.observer_backend_pid(&observer);
    fixture.terminate_own_observer_backend(&observer, first_pid);

    let started = Instant::now();
    assert_eq!(
        observer.observe(&fixture.zone),
        Err(ResolveError::ObserverUnavailable)
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    fixture.put("host-b", 8, 10000, 9000);
    std::thread::sleep(RECONNECT_COOLDOWN + Duration::from_millis(20));

    let recovered = observer.observe(&fixture.zone).unwrap().unwrap();
    assert_eq!(recovered.owner_id, "host-b");
    assert_eq!(recovered.fencing_token, 8);
    assert!(
        Arc::ptr_eq(&shared_worker, &observer.shared),
        "recovery must share the original worker queue"
    );
    assert_ne!(
        fixture.observer_backend_pid(&observer),
        first_pid,
        "recovery must use a fresh audited connection"
    );
}

#[test]
fn postgres_last_observer_drop_on_current_thread_async_runtime_is_safe() {
    let _guard = PG_TEST_LOCK.lock().unwrap();
    let Some(fixture) = PgFixture::from_env() else {
        return;
    };
    let observer = fixture.observer();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async move {
        drop(observer);
    });
}

use super::super::{
    build_account_store_mutation_plan, write_account_store_mutation_plan_in_transaction,
    AccountStoreMutationScope, PostgresClientConfig, ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN,
};
use super::*;
use postgres::{Client, NoTls};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn fence() -> ZoneSaveFence {
    ZoneSaveFence::new("fence-test-zone".into(), "owner-a".into(), 7).unwrap()
}

fn source_config(url: &str) -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config.account_store_database_url = Some(url.to_string());
    config
}

#[test]
fn zone_save_fence_constructor_rejects_invalid_server_capabilities() {
    for (zone, owner, token) in [
        ("", "owner", 1),
        ("zone", "", 1),
        (" zone", "owner", 1),
        ("zone", "owner\n", 1),
        ("zone", "owner", 0),
        ("zone", "owner", u64::MAX),
    ] {
        assert_eq!(
            ZoneSaveFence::new(zone.into(), owner.into(), token),
            Err(REJECTED.into())
        );
    }
    assert!(ZoneSaveFence::new("x".repeat(1025), "owner".into(), 1).is_err());
    let valid = fence();
    assert_eq!(valid.zone_id(), "fence-test-zone");
    assert_eq!(valid.owner_id(), "owner-a");
    assert_eq!(valid.fencing_token(), 7);
}

#[test]
fn zone_save_fence_binding_requires_explicit_postgres_source_and_rechecks_on_write() {
    let url = "postgres://fixture:private-secret@127.0.0.1:55491/mir2_zone_lease_test_fake";
    let mut missing = SimulationConfig::default();
    missing.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    for invalid in [
        SimulationConfig::default(),
        missing,
        SimulationConfig::default().with_account_store_database_url(url),
        source_config(" "),
        source_config("file://private-secret"),
        source_config("postgres://"),
        source_config("postgres://fixture@127.0.0.1:55491"),
    ] {
        let error = invalid.with_zone_save_fence(fence()).unwrap_err();
        assert_eq!(error, REJECTED);
        assert!(!error.contains("private-secret"));
    }
    let mut file = source_config(url);
    file.account_store_path = Some(std::path::PathBuf::from("must-not-open.json"));
    assert!(file.with_zone_save_fence(fence()).is_err());
    let config = source_config(url).with_zone_save_fence(fence()).unwrap();
    assert_eq!(config.zone_save_fence(), Some(&fence()));
    let mut changed = config.clone();
    changed.account_store_database_mode = AccountStoreDatabaseMode::Mirror;
    let ran = AtomicBool::new(false);
    assert_eq!(
        changed
            .commit_account_store_transaction(&["demo".into()], |_| {
                ran.store(true, Ordering::SeqCst);
                Ok(())
            })
            .unwrap_err(),
        REJECTED
    );
    assert!(!ran.load(Ordering::SeqCst));
    assert_eq!(changed.save_account_store().unwrap_err(), REJECTED);
    let shadow = config.fork_for_replica_apply().unwrap();
    assert!(shadow.zone_save_fence().is_none());
    assert!(shadow.account_store_database_url.is_none());
    assert!(shadow.account_store_path.is_none());
    assert!(shadow.has_isolated_replica_bootstrap_authority());
    let mut receiver = config.clone();
    receiver.rebind_account_store_from(&source_config(url));
    assert_eq!(receiver.zone_save_fence(), Some(&fence()));
}

#[test]
fn zone_save_fence_snapshot_plans_carry_fence_and_unknown_freezes_shared_writers() {
    for scoped in [false, true] {
        let config = source_config("postgres://fixture@127.0.0.1:55491/mir2_zone_lease_test_fake")
            .with_zone_save_fence(fence())
            .unwrap();
        let clone = config.clone();
        let before = config
            .account_store
            .lock()
            .unwrap()
            .source_account_versions
            .clone();
        let error = config
            .save_account_store_with_plan_writer(scoped.then_some("demo"), |_, mode, plan| {
                assert_eq!(mode, AccountStoreDatabaseMode::SourceOfTruth);
                assert_eq!(plan.zone_save_fence.as_ref(), Some(&fence()));
                Err(commit_outcome_unknown("bounded writer callback fixture"))
            })
            .unwrap_err();
        assert!(error.starts_with(COMMIT_OUTCOME_UNKNOWN));
        assert!(error.contains(ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN));
        assert!(clone
            .save_account_store_account("demo")
            .unwrap_err()
            .contains("writes are frozen"));
        assert_eq!(
            config.account_store.lock().unwrap().source_account_versions,
            before
        );
    }
}

#[test]
fn zone_save_fence_mixed_domain_unknown_freezes_independently_of_domain_prefix() {
    let config = source_config("postgres://fixture@127.0.0.1:55491/mir2_zone_lease_test_fake")
        .with_zone_save_fence(fence())
        .unwrap();
    let before = config.account_store.lock().unwrap().clone();
    let mut after = before.clone();
    after
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .values_mut()
        .next()
        .unwrap()
        .credit += 1;
    let ids = ["demo".to_string()];
    let mut plan = build_account_store_mutation_plan(
        &before,
        &after,
        AccountStoreMutationScope::Accounts(&ids),
        false,
    );
    config.bind_zone_save_fence_to_plan(&mut plan);
    assert!(plan.carries_billing_source());
    plan.guilds.insert(
        "mixed-guild".into(),
        super::super::shared_guild_store::GuildMutation {
            expected_version: Some(1),
            desired: None,
        },
    );
    assert!(!plan.guilds.is_empty());
    let error =
        config.map_clock_repository_error(&plan, commit_outcome_unknown("mixed-domain fixture"));
    assert!(error.starts_with(COMMIT_OUTCOME_UNKNOWN));
    assert!(config.ensure_account_store_writable().is_err());
}

#[test]
fn zone_save_fence_prepared_source_cannot_cross_fence_bindings() {
    let base = source_config("postgres://fixture@127.0.0.1:55491/mir2_zone_lease_test_fake");
    let receiver = base.clone().with_zone_save_fence(fence()).unwrap();
    let other = ZoneSaveFence::new("fence-test-zone".into(), "owner-b".into(), 8).unwrap();
    for producer in [
        base.clone(),
        base.clone().with_zone_save_fence(other).unwrap(),
    ] {
        let index = base.default_character.index;
        let before = base.account_store.lock().unwrap().accounts["demo"].saves[&index].clone();
        let permit =
            super::super::guild_experience::GuildExperienceCommitPermit::from_verified_shared_kill(
                super::super::Stage5FriendIdentity {
                    account_id: "demo".into(),
                    character_index: index,
                },
                "zone-fence-prepared-test".into(),
                "a".repeat(64),
                None,
            )
            .unwrap();
        let source = producer
            .prepare_kill_account_source(&permit, before, |store| {
                let save = store
                    .accounts
                    .get_mut("demo")
                    .unwrap()
                    .saves
                    .get_mut(&index)
                    .unwrap();
                save.revision = save.revision.checked_add(1).unwrap();
                save.guild_experience_journal
                    .applied_kill_receipts
                    .insert(permit.kill_key.clone(), permit.kill_payload_hash.clone());
                Ok(())
            })
            .unwrap();
        assert_eq!(source.checkpoint().revision, source.before().revision + 1);
        let ran = AtomicBool::new(false);
        let outcome = receiver.publish_prepared_kill_source(&source, |_| {
            ran.store(true, Ordering::SeqCst);
            Ok(super::super::PreparedKillPublication::Written(()))
        });
        assert!(
            matches!(outcome, Err(super::super::PreparedKillPublicationFailure::Rejected(ref error)) if error == REJECTED)
        );
        assert!(!ran.load(Ordering::SeqCst));
    }
}

// Absence skips. Every configured-but-invalid or unavailable fixture fails.
// Never consult a production/default URL and never allow default PostgreSQL port.
fn fixture_url(value: &str) -> Result<PostgresClientConfig, ()> {
    if !value.starts_with("postgres://") && !value.starts_with("postgresql://") {
        return Err(());
    }
    let parsed: PostgresClientConfig = value.parse().map_err(|_| ())?;
    let database = parsed.get_dbname().ok_or(())?;
    if parsed.get_hosts().len() != 1
        || parsed.get_ports().len() != 1
        || parsed.get_ports()[0] == 5432
        || parsed.get_ports()[0] == 0
        || parsed.get_options().is_some()
        || !database
            .strip_prefix("mir2_zone_lease_test_")
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
                .is_ok_and(|ip| ip.is_loopback()) => {}
        _ => return Err(()),
    }
    Ok(parsed)
}

#[test]
fn zone_save_fence_pg_fixture_url_cannot_target_default_or_shared_database() {
    for url in [
        "",
        "postgres://u@127.0.0.1/mir2_zone_lease_test_a",
        "postgres://u@127.0.0.1:5432/mir2_zone_lease_test_a",
        "postgres://u@localhost:55491/mir2_zone_lease_test_a",
        "postgres://u@192.0.2.1:55491/mir2_zone_lease_test_a",
        "postgres://u@127.0.0.1:55491/mir2",
        "postgres://u@127.0.0.1:55491/mir2_zone_lease_test_",
    ] {
        assert!(fixture_url(url).is_err());
    }
    assert!(fixture_url("postgres://u@127.0.0.1:55491/mir2_zone_lease_test_a").is_ok());
}

static PG_LOCK: Mutex<()> = Mutex::new(());
struct Database {
    url: String,
    source_url: String,
    schema: String,
    zone: String,
}
impl Database {
    fn from_env() -> Option<Self> {
        let url = match std::env::var("MIR2_TEST_ZONE_SAVE_FENCE_URL") {
            Ok(url) => url,
            Err(std::env::VarError::NotPresent) => return None,
            Err(_) => panic!("zone save fixture URL must be Unicode"),
        };
        fixture_url(&url).unwrap_or_else(|_| panic!("unsafe configured zone save fixture URL"));
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("zone_save_fence_{}_{stamp}", std::process::id());
        let separator = if url.contains('?') { '&' } else { '?' };
        let source_url = format!("{url}{separator}options=-c%20search_path%3D{schema}");
        let db = Self {
            url,
            source_url,
            schema: schema.clone(),
            zone: format!("{schema}:zone"),
        };
        let mut client = db.admin();
        client
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap_or_else(|_| panic!("fixture schema creation failed"));
        let mut client = db.connect();
        // The public lease relation must already be provisioned by the test
        // harness. Account fixtures deliberately never run its migration.
        for (version, sql) in crate::db_projection::MIGRATIONS {
            if *version != "0003_zone_owner_leases" {
                client
                    .batch_execute(sql)
                    .unwrap_or_else(|_| panic!("fixture account schema migration failed"));
            }
        }
        db.put("owner-a", 7, 120_000, 1);
        Some(db)
    }
    fn admin(&self) -> Client {
        connect_fixture(&self.url)
    }
    fn connect(&self) -> Client {
        connect_fixture(&self.source_url)
    }
    fn put(&self, owner: &str, token: i64, ttl: i64, heartbeat: i64) {
        self.admin().execute("INSERT INTO public.zone_owner_leases(zone_id,owner_id,fencing_token,expires_at_ms,heartbeat_at_ms) VALUES($1,$2,$3,floor(extract(epoch FROM clock_timestamp())*1000)::bigint+$4,$5) ON CONFLICT(zone_id) DO UPDATE SET owner_id=$2,fencing_token=$3,expires_at_ms=EXCLUDED.expires_at_ms,heartbeat_at_ms=$5", &[&self.zone,&owner,&token,&ttl,&heartbeat])
            .unwrap_or_else(|_| panic!("fixture public lease write failed"));
    }
    fn fence(&self) -> ZoneSaveFence {
        ZoneSaveFence::new(self.zone.clone(), "owner-a".into(), 7).unwrap()
    }
    fn config(&self) -> SimulationConfig {
        let config = source_config(&self.source_url)
            .with_zone_save_fence(self.fence())
            .unwrap();
        config.save_account_store().unwrap();
        config
    }
    fn password(&self) -> String {
        self.connect()
            .query_one(
                "SELECT password_snapshot FROM accounts WHERE account_id='demo'",
                &[],
            )
            .unwrap()
            .get(0)
    }
    fn image(&self) -> (serde_json::Value, i64, serde_json::Value, i64) {
        let row = self.connect().query_one("SELECT a.raw_json,a.store_version,s.snapshot_json,s.save_version FROM accounts a JOIN character_saves s USING(account_id) WHERE a.account_id='demo'", &[]).unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }
}
fn connect_fixture(url: &str) -> Client {
    let mut config: PostgresClientConfig = url
        .parse()
        .unwrap_or_else(|_| panic!("fixture URL parse failed"));
    config
        .connect_timeout(Duration::from_secs(2))
        .notice_callback(|_| {});
    config
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("configured zone save fixture connection failed"))
}
impl Drop for Database {
    fn drop(&mut self) {
        let mut client = self.admin();
        client
            .execute(
                "DELETE FROM public.zone_owner_leases WHERE zone_id=$1",
                &[&self.zone],
            )
            .unwrap();
        client
            .batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .unwrap();
    }
}
fn mutation(config: &SimulationConfig, password: &str) -> AccountStoreMutationPlan {
    let original = config.account_store.lock().unwrap().clone();
    let mut desired = original.clone();
    let account = desired.accounts.get_mut("demo").unwrap();
    account.password = password.into();
    account.saves.values_mut().next().unwrap().gold += 1;
    let ids = ["demo".to_string()];
    let mut plan = build_account_store_mutation_plan(
        &original,
        &desired,
        AccountStoreMutationScope::Accounts(&ids),
        false,
    );
    config.bind_zone_save_fence_to_plan(&mut plan);
    plan
}

#[test]
fn zone_save_fence_postgres_matching_stale_owner_token_expiry_missing_and_malformed() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    config
        .commit_account_store_transaction(&["demo".into()], |store| {
            store.accounts.get_mut("demo").unwrap().password = "matching-committed".into();
            Ok(())
        })
        .unwrap();
    for (owner, token, ttl, heartbeat) in [
        ("owner-b", 7, 120000, 1),
        ("owner-a", 8, 120000, 1),
        ("owner-a", 7, -1, 1),
        ("owner-a", -7, 120000, 1),
        ("owner-a", 7, 120000, 0),
        ("owner-a", 7, 120000, i64::MAX),
    ] {
        db.put(owner, token, ttl, heartbeat);
        let before = config
            .account_store
            .lock()
            .unwrap()
            .source_account_versions
            .clone();
        let durable_before = db.image();
        assert_eq!(
            config
                .commit_account_store_transaction(&["demo".into()], |store| {
                    let account = store.accounts.get_mut("demo").unwrap();
                    account.password = "must-rollback".into();
                    account.saves.values_mut().next().unwrap().gold += 1;
                    Ok(())
                })
                .unwrap_err(),
            REJECTED
        );
        assert_eq!(db.password(), "matching-committed");
        assert_eq!(db.image(), durable_before);
        assert_eq!(
            config.account_store.lock().unwrap().source_account_versions,
            before
        );
        assert!(config.ensure_account_store_writable().is_ok());
    }
    db.admin()
        .execute(
            "DELETE FROM public.zone_owner_leases WHERE zone_id=$1",
            &[&db.zone],
        )
        .unwrap();
    assert_eq!(
        config.save_account_store_account("demo").unwrap_err(),
        REJECTED
    );
}

#[test]
fn zone_save_fence_postgres_takeover_waits_for_account_transaction_commit() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let plan = mutation(&config, "committed-before-takeover");
    let mut client = db.connect();
    let mut tx = client.transaction().unwrap();
    write_account_store_mutation_plan_in_transaction(
        &mut tx,
        &plan,
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    assert_ne!(db.password(), "committed-before-takeover");
    let mut takeover = db.admin();
    let pid: i32 = takeover
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let zone = db.zone.clone();
    let (send, receive) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        takeover
            .batch_execute("SET lock_timeout='3s'; SET statement_timeout='4s'")
            .unwrap();
        let result = takeover.execute("UPDATE public.zone_owner_leases SET owner_id='owner-b',fencing_token=8 WHERE zone_id=$1", &[&zone]);
        send.send(result.is_ok()).unwrap();
    });
    wait_for_lock(&mut db.admin(), pid);
    assert!(receive.try_recv().is_err());
    tx.commit().unwrap();
    assert!(receive.recv_timeout(Duration::from_secs(3)).unwrap());
    worker.join().unwrap();
    assert_eq!(db.password(), "committed-before-takeover");
    assert_eq!(
        config.save_account_store_account("demo").unwrap_err(),
        REJECTED
    );
}

fn wait_for_lock(observer: &mut Client, pid: i32) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        let waiting: bool = observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND wait_event_type='Lock')", &[&pid]).unwrap().get(0);
        if waiting {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("fixture backend never reached a real database lock wait");
}

#[test]
fn zone_save_fence_postgres_missing_table_and_bounded_lock_wait_fail_closed() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let before = db.image();
    let plan = mutation(&config, "must-not-run-without-lease-table");
    let mut client = db.connect();
    let lock_before: String = client.query_one("SHOW lock_timeout", &[]).unwrap().get(0);
    let statement_before: String = client
        .query_one("SHOW statement_timeout", &[])
        .unwrap()
        .get(0);
    // Transactional rename makes the required public relation absent only for
    // this transaction, then rollback restores it without migration/creation.
    let mut tx = client.transaction().unwrap();
    tx.batch_execute(&format!(
        "ALTER TABLE public.zone_owner_leases RENAME TO {}_hidden",
        db.schema
    ))
    .unwrap();
    assert_eq!(
        write_account_store_mutation_plan_in_transaction(
            &mut tx,
            &plan,
            AccountStoreDatabaseMode::SourceOfTruth
        )
        .unwrap_err(),
        UNAVAILABLE
    );
    tx.rollback().unwrap();
    assert_eq!(db.image(), before);
    let mut blocker = db.admin();
    let mut lock = blocker.transaction().unwrap();
    lock.query_one(
        "SELECT zone_id FROM public.zone_owner_leases WHERE zone_id=$1 FOR UPDATE",
        &[&db.zone],
    )
    .unwrap();
    let start = Instant::now();
    assert_eq!(
        config
            .commit_account_store_transaction(&["demo".into()], |store| {
                store.accounts.get_mut("demo").unwrap().password =
                    "must-not-run-on-lock-timeout".into();
                Ok(())
            })
            .unwrap_err(),
        UNAVAILABLE
    );
    assert!(start.elapsed() < Duration::from_secs(3));
    lock.rollback().unwrap();
    assert_eq!(db.image(), before);
    assert_eq!(
        client
            .query_one("SHOW lock_timeout", &[])
            .unwrap()
            .get::<_, String>(0),
        lock_before
    );
    assert_eq!(
        client
            .query_one("SHOW statement_timeout", &[])
            .unwrap()
            .get::<_, String>(0),
        statement_before
    );
    assert!(config.ensure_account_store_writable().is_ok());
}

#[test]
fn zone_save_fence_postgres_expiry_is_read_after_waiting_for_the_lease_lock() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let before = db.image();
    let plan = mutation(&config, "expired-during-lock-wait");
    let mut blocker = db.admin();
    let mut lock = blocker.transaction().unwrap();
    lock.execute("UPDATE public.zone_owner_leases SET expires_at_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint+40 WHERE zone_id=$1", &[&db.zone]).unwrap();
    let mut writer = db.connect();
    let pid: i32 = writer
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let worker = std::thread::spawn(move || {
        let mut tx = writer.transaction().unwrap();
        write_account_store_mutation_plan_in_transaction(
            &mut tx,
            &plan,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
    });
    wait_for_lock(&mut db.admin(), pid);
    std::thread::sleep(Duration::from_millis(60));
    lock.commit().unwrap();
    assert_eq!(worker.join().unwrap().unwrap_err(), REJECTED);
    assert_eq!(db.image(), before);
}

#[test]
fn zone_save_fence_postgres_killed_lock_and_save_connections_rollback_every_write() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let baseline = db.password();
    let before = db.image();
    let mut blocker = db.admin();
    let mut lease = blocker.transaction().unwrap();
    lease
        .query_one(
            "SELECT zone_id FROM public.zone_owner_leases WHERE zone_id=$1 FOR UPDATE",
            &[&db.zone],
        )
        .unwrap();
    let mut writer = db.connect();
    let pid: i32 = writer
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let plan = mutation(&config, "killed-while-locking");
    let worker = std::thread::spawn(move || {
        let mut tx = writer.transaction().unwrap();
        write_account_store_mutation_plan_in_transaction(
            &mut tx,
            &plan,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
    });
    let mut admin = db.admin();
    wait_for_lock(&mut admin, pid);
    assert!(admin
        .query_one("SELECT pg_terminate_backend($1)", &[&pid])
        .unwrap()
        .get::<_, bool>(0));
    assert!(worker.join().unwrap().is_err());
    lease.rollback().unwrap();
    assert_eq!(db.password(), baseline);
    assert_eq!(db.image(), before);
    let mut writer = db.connect();
    let pid: i32 = writer
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let mut tx = writer.transaction().unwrap();
    let plan = mutation(&config, "killed-after-source-writes");
    write_account_store_mutation_plan_in_transaction(
        &mut tx,
        &plan,
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    assert_eq!(
        tx.query_one(
            "SELECT password_snapshot FROM accounts WHERE account_id='demo'",
            &[]
        )
        .unwrap()
        .get::<_, String>(0),
        "killed-after-source-writes"
    );
    assert_eq!(db.password(), baseline);
    assert_eq!(db.image(), before);
    assert!(admin
        .query_one("SELECT pg_terminate_backend($1)", &[&pid])
        .unwrap()
        .get::<_, bool>(0));
    assert!(tx.commit().is_err());
    assert_eq!(db.password(), baseline);
    assert!(config.ensure_account_store_writable().is_ok());
}

#[test]
fn zone_save_fence_postgres_after_commit_faults_freeze_no_replay_and_no_live_publish() {
    let _guard = PG_LOCK.lock().unwrap();
    for fault in [
        TestFault::PanicAfterCommit,
        TestFault::LoseResponseAfterCommit,
    ] {
        let Some(db) = Database::from_env() else {
            return;
        };
        let config = db.config();
        let clone = config.clone();
        let before = config.account_store.lock().unwrap().clone();
        *config.zone_save_fence_test_fault.lock().unwrap() = Some(fault);
        let error = config
            .commit_account_store_transaction(&["demo".into()], |store| {
                store.accounts.get_mut("demo").unwrap().password =
                    "durable-outcome-unacknowledged".into();
                Ok(())
            })
            .unwrap_err();
        assert!(error.starts_with(COMMIT_OUTCOME_UNKNOWN));
        assert!(error.contains(ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN));
        assert_eq!(db.password(), "durable-outcome-unacknowledged");
        let live = config.account_store.lock().unwrap();
        assert_eq!(
            live.accounts["demo"].password,
            before.accounts["demo"].password
        );
        assert_eq!(live.source_account_versions, before.source_account_versions);
        drop(live);
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();
        assert!(clone
            .commit_account_store_transaction(&["demo".into()], move |_| {
                flag.store(true, Ordering::SeqCst);
                Ok(())
            })
            .unwrap_err()
            .contains("writes are frozen"));
        assert!(!ran.load(Ordering::SeqCst));
        assert!(clone
            .save_account_store()
            .unwrap_err()
            .contains("writes are frozen"));
    }

    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let clone = config.clone();
    let durable_before = db.image();
    let plan = mutation(&config, "must-rollback-on-deferred-constraint");
    let mut client = db.connect();
    client
        .batch_execute(
            "CREATE TABLE zone_save_commit_deferred_fixture \
         (value BIGINT UNIQUE DEFERRABLE INITIALLY DEFERRED)",
        )
        .unwrap();
    let mut transaction = client.transaction().unwrap();
    write_account_store_mutation_plan_in_transaction(
        &mut transaction,
        &plan,
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    transaction
        .execute(
            "INSERT INTO zone_save_commit_deferred_fixture(value) VALUES(1),(1)",
            &[],
        )
        .unwrap();
    // The INSERT succeeds, and the real server rejects only COMMIT. Even
    // this proven rollback is conservatively classified as an unknown result.
    let driver_error = transaction.commit().unwrap_err();
    assert_eq!(driver_error.as_db_error().unwrap().code().code(), "23505");
    let classified = classify_commit_error(driver_error);
    assert!(classified.starts_with(COMMIT_OUTCOME_UNKNOWN));
    assert!(classified.contains(ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN));
    assert!(!classified.contains("zone_save_commit_deferred_fixture"));
    assert!(!classified.contains("23505"));
    let error = config.map_clock_repository_error(&plan, classified);
    assert!(error.starts_with(COMMIT_OUTCOME_UNKNOWN));
    assert_eq!(db.image(), durable_before);
    let ran = AtomicBool::new(false);
    assert!(clone
        .commit_account_store_transaction(&["demo".into()], |_| {
            ran.store(true, Ordering::SeqCst);
            Ok(())
        })
        .unwrap_err()
        .contains("writes are frozen"));
    assert!(!ran.load(Ordering::SeqCst));
}

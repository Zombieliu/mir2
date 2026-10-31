use super::super::{
    build_account_store_mutation_plan, write_account_store_mutation_plan_in_transaction,
    write_account_store_mutation_plan_to_postgres, AccountStoreMutationScope, PostgresClientConfig,
};
use super::*;
use postgres::{Client, NoTls};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn fence() -> ZoneSaveFence {
    ZoneSaveFence::new("operation-zone".into(), "owner-a".into(), 7).unwrap()
}
fn binding(id: &str) -> ZoneSaveOperationBinding {
    ZoneSaveOperationBinding::new(id.into(), "rpc-session-a".into(), fence()).unwrap()
}
fn source_config(url: &str) -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config.account_store_database_url = Some(url.into());
    config
}
fn operation(config: &SimulationConfig, binding: ZoneSaveOperationBinding) -> ZoneSaveOperation {
    let index = config.default_character.index;
    let mut frozen = config.account_store.lock().unwrap().accounts["demo"].saves[&index].clone();
    frozen.gold += 17;
    let mut expected = frozen.clone();
    expected.revision += 1;
    ZoneSaveOperation::new(binding, "demo".into(), frozen, expected, Some(42)).unwrap()
}
fn plan(config: &SimulationConfig, operation: &ZoneSaveOperation) -> AccountStoreMutationPlan {
    let before = config.account_store.lock().unwrap().clone();
    let mut after = before.clone();
    stage(&mut after, operation);
    let ids = [operation.account_id().to_string()];
    let mut plan = build_account_store_mutation_plan(
        &before,
        &after,
        AccountStoreMutationScope::Accounts(&ids),
        false,
    );
    config.bind_zone_save_fence_to_plan(&mut plan);
    plan
}
fn stage(store: &mut super::super::AccountStore, operation: &ZoneSaveOperation) {
    let account = store.accounts.get_mut(operation.account_id()).unwrap();
    let saved = operation.expected_saved_checkpoint().clone();
    *account
        .characters
        .iter_mut()
        .find(|c| c.index == operation.character_index())
        .unwrap() = saved.character.clone();
    account.saves.insert(operation.character_index(), saved);
    if let Some(access) = operation.last_access_binary_datetime() {
        account
            .character_last_access_binary_datetimes
            .insert(operation.character_index(), access);
    }
}

#[test]
fn zone_save_operation_binding_constructor_and_exact_expected_snapshot_are_bounded() {
    for bad in [
        "".into(),
        " leading".into(),
        "session\n".into(),
        "x".repeat(1025),
    ] {
        assert!(ZoneSaveOperationBinding::new(bad.clone(), "rpc".into(), fence()).is_err());
        assert!(ZoneSaveOperationBinding::new("op".into(), bad, fence()).is_err());
    }
    let config = SimulationConfig::default();
    let op = operation(&config, binding("constructor"));
    assert_eq!(op.binding(), &binding("constructor"));
    assert_eq!(op.checkpoint_digest().len(), 64);
    assert_eq!(
        op.expected_saved_checkpoint().revision,
        op.frozen_checkpoint().revision + 1
    );
    let mut wrong = op.expected_saved_checkpoint().clone();
    wrong.revision += 1;
    assert!(ZoneSaveOperation::new(
        op.binding().clone(),
        "demo".into(),
        op.frozen_checkpoint().clone(),
        wrong,
        Some(42)
    )
    .is_err());
    let mut wrong = op.expected_saved_checkpoint().clone();
    wrong.character.name.push('x');
    assert!(ZoneSaveOperation::new(
        op.binding().clone(),
        "demo".into(),
        op.frozen_checkpoint().clone(),
        wrong,
        Some(42)
    )
    .is_err());
    let mut too_large = op.frozen_checkpoint().clone();
    too_large.stage5_systems_json = Some("x".repeat(8 * 1024 * 1024));
    assert!(ZoneSaveOperation::new(
        op.binding().clone(),
        "demo".into(),
        too_large,
        op.expected_saved_checkpoint().clone(),
        Some(42)
    )
    .is_err());
}

#[test]
fn zone_save_operation_config_and_plan_reject_fence_revision_payload_and_last_access_changes() {
    let url = "postgres://fixture@127.0.0.1:55491/mir2_zone_lease_test_operations_fake";
    let config = source_config(url);
    let op = operation(&config, binding("plan"));
    assert!(config.clone().with_zone_save_operation(op.clone()).is_err());
    let config = config
        .with_zone_save_fence(fence())
        .unwrap()
        .with_zone_save_operation(op.clone())
        .unwrap();
    {
        let mut store = config.account_store.lock().unwrap();
        store.source_account_versions.insert("demo".into(), 1);
        store
            .source_save_versions
            .entry("demo".into())
            .or_default()
            .insert(op.character_index(), 1);
    }
    let exact = plan(&config, &op);
    op.validate_plan(&exact, AccountStoreDatabaseMode::SourceOfTruth)
        .unwrap();
    for alteration in 0..4 {
        let mut changed = exact.clone();
        let account = changed.accounts.get_mut("demo").unwrap();
        match alteration {
            0 => {
                account
                    .desired_account
                    .as_mut()
                    .unwrap()
                    .saves
                    .get_mut(&op.character_index())
                    .unwrap()
                    .gold += 1
            }
            1 => {
                account
                    .desired_account
                    .as_mut()
                    .unwrap()
                    .saves
                    .get_mut(&op.character_index())
                    .unwrap()
                    .revision += 1
            }
            2 => {
                account
                    .desired_account
                    .as_mut()
                    .unwrap()
                    .character_last_access_binary_datetimes
                    .insert(op.character_index(), 43);
            }
            _ => {
                changed.zone_save_fence =
                    Some(ZoneSaveFence::new("operation-zone".into(), "owner-b".into(), 8).unwrap())
            }
        }
        assert_eq!(
            op.validate_plan(&changed, AccountStoreDatabaseMode::SourceOfTruth),
            Err(REJECTED.into())
        );
    }
    assert!(config
        .clone()
        .with_zone_save_fence(ZoneSaveFence::new("other".into(), "owner-a".into(), 7).unwrap())
        .is_err());
    assert!(config
        .fork_for_replica_apply()
        .unwrap()
        .zone_save_operation()
        .is_none());
}

#[test]
fn zone_save_operation_readonly_wire_roundtrip_rejects_tampered_digest_revision_status_and_cas() {
    let config = SimulationConfig::default();
    let op = operation(&config, binding("wire"));
    assert_eq!(
        ZoneSaveOperationBinding::from_json_checked(op.binding().to_json().unwrap()).unwrap(),
        *op.binding()
    );
    let prepared = ZoneSaveOperationReceipt {
        operation: op.clone(),
        status: OperationStatus::Prepared,
        saved_checkpoint: None,
        account_store_version: None,
        character_save_version: None,
    };
    let committed = ZoneSaveOperationReceipt {
        operation: op.clone(),
        status: OperationStatus::Committed,
        saved_checkpoint: Some(op.expected_saved_checkpoint().clone()),
        account_store_version: Some(2),
        character_save_version: Some(3),
    };
    for receipt in [prepared, committed] {
        let value = receipt.to_json().unwrap();
        let parsed = ZoneSaveOperationReceipt::from_json_checked(value.clone()).unwrap();
        assert_eq!(parsed.operation(), &op);
        assert_eq!(parsed.status(), receipt.status());
        for alteration in 0..7 {
            let mut changed = value.clone();
            match alteration {
                0 => changed["operation"]["checkpointDigest"] = Value::String("0".repeat(64)),
                1 => changed["operation"]["expectedDigest"] = Value::String("0".repeat(64)),
                2 => changed["operation"]["frozenCheckpoint"]["gold"] = Value::from(999),
                3 => changed["operation"]["binding"]["fencingToken"] = Value::from(0),
                4 => changed["accountStoreVersion"] = Value::from(0),
                5 => changed["status"] = Value::String("Unknown".into()),
                _ => changed["operation"]["unexpectedCapability"] = Value::Bool(true),
            }
            assert!(ZoneSaveOperationReceipt::from_json_checked(changed).is_err());
        }
    }
}

// Absence skips; every configured unsafe/unavailable fixture fails. Only an
// explicit disposable loopback mir2_zone_lease_test_* DB on a non-default port.
fn fixture_url(value: &str) -> Result<PostgresClientConfig, ()> {
    if !value.starts_with("postgres://") && !value.starts_with("postgresql://") {
        return Err(());
    }
    let parsed: PostgresClientConfig = value.parse().map_err(|_| ())?;
    if parsed.get_hosts().len() != 1
        || parsed.get_ports().len() != 1
        || matches!(parsed.get_ports()[0], 0 | 5432)
        || parsed.get_options().is_some()
        || !parsed
            .get_dbname()
            .and_then(|db| db.strip_prefix("mir2_zone_lease_test_"))
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
#[test]
fn zone_save_operation_fixture_rejects_shared_default_or_nonlocal_database() {
    for url in [
        "",
        "postgres://u@127.0.0.1/mir2_zone_lease_test_op",
        "postgres://u@127.0.0.1:5432/mir2_zone_lease_test_op",
        "postgres://u@localhost:55491/mir2_zone_lease_test_op",
        "postgres://u@192.0.2.1:55491/mir2_zone_lease_test_op",
        "postgres://u@127.0.0.1:55491/production",
        "postgres://u@127.0.0.1:55491/mir2_zone_lease_test_",
    ] {
        assert!(fixture_url(url).is_err());
    }
    assert!(fixture_url("postgres://u@127.0.0.1:55491/mir2_zone_lease_test_op").is_ok());
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
        let url = match std::env::var("MIR2_TEST_ZONE_SAVE_OPERATION_URL") {
            Ok(url) => url,
            Err(std::env::VarError::NotPresent) => return None,
            Err(_) => panic!("zone save operation fixture must be Unicode"),
        };
        fixture_url(&url)
            .unwrap_or_else(|_| panic!("unsafe configured zone save operation fixture"));
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("zone_save_op_{}_{stamp}", std::process::id());
        let separator = if url.contains('?') { '&' } else { '?' };
        let source_url = format!("{url}{separator}options=-c%20search_path%3D{schema}");
        let db = Self {
            url,
            source_url,
            schema: schema.clone(),
            zone: format!("{schema}:zone"),
        };
        let mut admin = db.admin();
        admin
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        // Test provisioning, never production mutation. The public lease and
        // ledger share this explicitly isolated fixture database.
        admin
            .batch_execute(include_str!(
                "../../../../infra/postgres/migrations/0003_zone_owner_leases.sql"
            ))
            .unwrap();
        let mut client = db.connect();
        for (version, sql) in crate::db_projection::MIGRATIONS {
            if *version != "0003_zone_owner_leases" {
                client.batch_execute(sql).unwrap();
            }
        }
        db.put("owner-a", 7, 120_000);
        Some(db)
    }
    fn admin(&self) -> Client {
        connect(&self.url)
    }
    fn connect(&self) -> Client {
        connect(&self.source_url)
    }
    fn put(&self, owner: &str, token: i64, ttl: i64) {
        self.admin().execute("INSERT INTO public.zone_owner_leases(zone_id,owner_id,fencing_token,expires_at_ms,heartbeat_at_ms) \
            VALUES($1,$2,$3,floor(extract(epoch FROM clock_timestamp())*1000)::bigint+$4,1) \
            ON CONFLICT(zone_id) DO UPDATE SET owner_id=$2,fencing_token=$3,expires_at_ms=EXCLUDED.expires_at_ms,heartbeat_at_ms=1",
            &[&self.zone,&owner,&token,&ttl]).unwrap();
    }
    fn fence(&self, owner: &str, token: u64) -> ZoneSaveFence {
        ZoneSaveFence::new(self.zone.clone(), owner.into(), token).unwrap()
    }
    fn config(&self) -> SimulationConfig {
        let config = source_config(&self.source_url)
            .with_zone_save_fence(self.fence("owner-a", 7))
            .unwrap();
        config.save_account_store().unwrap();
        config
    }
    fn op(&self, config: &SimulationConfig, name: &str) -> ZoneSaveOperation {
        operation(
            config,
            ZoneSaveOperationBinding::new(
                format!("{}:{name}", self.schema),
                "rpc-session-a".into(),
                self.fence("owner-a", 7),
            )
            .unwrap(),
        )
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
}
fn connect(url: &str) -> Client {
    let mut config: PostgresClientConfig = url.parse().unwrap();
    config
        .connect_timeout(Duration::from_secs(2))
        .notice_callback(|_| {});
    config
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("configured zone save operation fixture unavailable"))
}
impl Drop for Database {
    fn drop(&mut self) {
        let mut client = self.admin();
        client
            .execute(
                "DELETE FROM public.zone_character_save_operations WHERE zone_id=$1",
                &[&self.zone],
            )
            .unwrap();
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

#[test]
fn zone_save_operation_postgres_prepare_is_idempotent_and_rejects_all_binding_or_payload_mismatch()
{
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let op = db.op(&config, "prepare");
    let first = config.prepare_zone_save_operation(&op).unwrap();
    assert_eq!(first.status(), OperationStatus::Prepared);
    assert_eq!(
        config.prepare_zone_save_operation(&op).unwrap().operation(),
        &op
    );
    let wrong_session = ZoneSaveOperationBinding::new(
        op.operation_id().into(),
        "rpc-session-b".into(),
        op.fence().clone(),
    )
    .unwrap();
    assert_eq!(
        config
            .query_zone_save_operation(&wrong_session)
            .unwrap_err(),
        REJECTED
    );
    for change in 0..3 {
        let mut altered = op.clone();
        match change {
            0 => altered.binding = wrong_session.clone(),
            1 => altered.account_id.push('x'),
            _ => {
                let mut checkpoint = op.frozen_checkpoint().clone();
                checkpoint.gold += 1;
                altered = ZoneSaveOperation::new(
                    op.binding().clone(),
                    op.account_id().into(),
                    checkpoint,
                    op.expected_saved_checkpoint().clone(),
                    Some(42),
                )
                .unwrap();
            }
        }
        assert_eq!(
            config.prepare_zone_save_operation(&altered).unwrap_err(),
            REJECTED
        );
    }
    assert_eq!(
        config
            .release_zone_save_operation(op.binding())
            .unwrap_err(),
        REJECTED
    );
    assert_eq!(
        config
            .query_zone_save_operation(op.binding())
            .unwrap()
            .unwrap()
            .status(),
        OperationStatus::Prepared
    );
}

#[test]
fn zone_save_operation_postgres_receipt_rolls_back_and_commits_atomically_and_duplicate_never_republishes(
) {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let op = db.op(&config, "atomic");
    config.prepare_zone_save_operation(&op).unwrap();
    let bound = config.clone().with_zone_save_operation(op.clone()).unwrap();
    let plan = plan(&bound, &op);
    let before = db.image();
    let mut client = db.connect();
    let mut tx = client.transaction().unwrap();
    write_account_store_mutation_plan_in_transaction(
        &mut tx,
        &plan,
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    assert_eq!(
        read_receipt(&mut tx, op.binding(), false)
            .unwrap()
            .unwrap()
            .status(),
        OperationStatus::Committed
    );
    assert_eq!(
        config
            .query_zone_save_operation(op.binding())
            .unwrap()
            .unwrap()
            .status(),
        OperationStatus::Prepared
    );
    tx.rollback().unwrap();
    assert_eq!(db.image(), before);
    write_account_store_mutation_plan_to_postgres(
        &mut client,
        &plan,
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    let receipt = config
        .query_zone_save_operation(op.binding())
        .unwrap()
        .unwrap();
    assert_eq!(receipt.status(), OperationStatus::Committed);
    assert_eq!(
        serde_json::to_value(receipt.saved_checkpoint().unwrap()).unwrap(),
        db.image().2
    );
    assert_eq!(receipt.account_store_version(), Some(db.image().1));
    assert_eq!(receipt.character_save_version(), Some(db.image().3));
    let committed = db.image();
    assert_eq!(
        write_account_store_mutation_plan_to_postgres(
            &mut client,
            &plan,
            AccountStoreDatabaseMode::SourceOfTruth
        )
        .unwrap_err(),
        ALREADY_COMMITTED
    );
    assert_eq!(db.image(), committed);
    assert_eq!(
        config.account_store.lock().unwrap().source_account_versions["demo"],
        before.1
    );
}

#[test]
fn zone_save_operation_postgres_old_owner_receipt_is_readable_after_takeover_without_write_authority(
) {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let op = db.op(&config, "takeover");
    config.prepare_zone_save_operation(&op).unwrap();
    let bound = config.clone().with_zone_save_operation(op.clone()).unwrap();
    write_account_store_mutation_plan_to_postgres(
        &mut db.connect(),
        &plan(&bound, &op),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    db.put("owner-b", 8, 120000);
    // A restarted server needs only the exact binding; frozen payload comes
    // from the durable ledger, never an in-memory map or client snapshot.
    let current = source_config(&db.source_url)
        .with_zone_save_fence(db.fence("owner-b", 8))
        .unwrap();
    assert_eq!(
        current
            .query_zone_save_operation(op.binding())
            .unwrap()
            .unwrap()
            .operation(),
        &op
    );
    assert_eq!(
        config
            .release_zone_save_operation(op.binding())
            .unwrap_err(),
        zone_save_fence::REJECTED
    );
    assert_eq!(
        config.prepare_zone_save_operation(&op).unwrap_err(),
        zone_save_fence::REJECTED
    );
    assert!(current
        .clone()
        .with_zone_save_operation(op.clone())
        .is_err());
    assert_eq!(
        current
            .release_zone_save_operation(op.binding())
            .unwrap()
            .status(),
        OperationStatus::Released
    );
    assert_eq!(
        current
            .release_zone_save_operation(op.binding())
            .unwrap()
            .status(),
        OperationStatus::Released
    );
    assert_eq!(
        config
            .query_zone_save_operation(op.binding())
            .unwrap()
            .unwrap()
            .status(),
        OperationStatus::Released
    );
}

#[test]
fn zone_save_operation_postgres_unknown_ack_freezes_live_cas_but_receipt_reconciles_without_replay()
{
    let _guard = PG_LOCK.lock().unwrap();
    for fault in [
        zone_save_fence::TestFault::LoseResponseAfterCommit,
        zone_save_fence::TestFault::PanicAfterCommit,
    ] {
        let Some(db) = Database::from_env() else {
            return;
        };
        let config = db.config();
        let op = db.op(&config, "unknown");
        config.prepare_zone_save_operation(&op).unwrap();
        let bound = config.clone().with_zone_save_operation(op.clone()).unwrap();
        let live_before = config.account_store.lock().unwrap().clone();
        *bound.zone_save_fence_test_fault.lock().unwrap() = Some(fault);
        let error = bound
            .commit_account_store_transaction(&["demo".into()], |store| {
                stage(store, &op);
                Ok(())
            })
            .unwrap_err();
        assert!(error.starts_with(zone_save_fence::COMMIT_OUTCOME_UNKNOWN));
        let receipt = config
            .query_zone_save_operation(op.binding())
            .unwrap()
            .unwrap();
        assert_eq!(receipt.status(), OperationStatus::Committed);
        assert_eq!(
            serde_json::to_value(receipt.saved_checkpoint().unwrap()).unwrap(),
            db.image().2
        );
        let live = config.account_store.lock().unwrap();
        assert_eq!(
            live.source_account_versions,
            live_before.source_account_versions
        );
        assert_eq!(
            serde_json::to_value(&live.accounts["demo"]).unwrap(),
            serde_json::to_value(&live_before.accounts["demo"]).unwrap()
        );
        drop(live);
        assert!(config.ensure_account_store_writable().is_err());
        assert!(bound
            .release_zone_save_operation(op.binding())
            .unwrap_err()
            .contains("writes are frozen"));
        let ran = std::sync::atomic::AtomicBool::new(false);
        assert!(bound
            .commit_account_store_transaction(&["demo".into()], |_| {
                ran.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
            .is_err());
        assert!(!ran.load(std::sync::atomic::Ordering::SeqCst));
    }
}

#[test]
fn zone_save_operation_postgres_missing_ledger_fails_closed_without_migration() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let op = db.op(&config, "missing");
    config.prepare_zone_save_operation(&op).unwrap();
    let bound = config.clone().with_zone_save_operation(op.clone()).unwrap();
    let before = db.image();
    let mut client = db.connect();
    let mut tx = client.transaction().unwrap();
    tx.batch_execute(&format!(
        "ALTER TABLE public.zone_character_save_operations RENAME TO {}_hidden",
        db.schema
    ))
    .unwrap();
    assert_eq!(
        write_account_store_mutation_plan_in_transaction(
            &mut tx,
            &plan(&bound, &op),
            AccountStoreDatabaseMode::SourceOfTruth
        )
        .unwrap_err(),
        UNAVAILABLE
    );
    tx.rollback().unwrap();
    assert_eq!(db.image(), before);
    assert_eq!(
        config
            .query_zone_save_operation(op.binding())
            .unwrap()
            .unwrap()
            .status(),
        OperationStatus::Prepared
    );
}

#[test]
fn zone_save_operation_postgres_failed_commit_preserves_prepared_receipt_and_all_account_cas() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let op = db.op(&config, "commit-rollback");
    config.prepare_zone_save_operation(&op).unwrap();
    let bound = config.clone().with_zone_save_operation(op.clone()).unwrap();
    let plan = plan(&bound, &op);
    let before = db.image();
    let mut client = db.connect();
    client.batch_execute("CREATE TABLE operation_deferred_failure(value BIGINT UNIQUE DEFERRABLE INITIALLY DEFERRED)").unwrap();
    let mut tx = client.transaction().unwrap();
    write_account_store_mutation_plan_in_transaction(
        &mut tx,
        &plan,
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    tx.execute(
        "INSERT INTO operation_deferred_failure(value) VALUES(1),(1)",
        &[],
    )
    .unwrap();
    let error = zone_save_fence::classify_commit_error(tx.commit().unwrap_err());
    assert!(bound
        .map_clock_repository_error(&plan, error)
        .starts_with(zone_save_fence::COMMIT_OUTCOME_UNKNOWN));
    assert_eq!(db.image(), before);
    assert_eq!(
        bound
            .query_zone_save_operation(op.binding())
            .unwrap()
            .unwrap()
            .status(),
        OperationStatus::Prepared
    );
    assert!(config.ensure_account_store_writable().is_err());
}

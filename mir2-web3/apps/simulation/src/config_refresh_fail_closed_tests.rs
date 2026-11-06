use std::collections::BTreeMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_protocol::{MirClass, MirGender};

use super::*;

fn test_character(index: i32, name: &str) -> CharacterRecord {
    CharacterRecord {
        index,
        name: name.to_string(),
        level: 1,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    }
}

#[test]
fn source_refresh_missing_removes_cached_account_and_versions() {
    let mut config = SimulationConfig::default();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    {
        let mut store = config.account_store.lock().unwrap();
        store.source_account_versions.insert("demo".to_string(), 7);
        store
            .source_save_versions
            .insert("demo".to_string(), BTreeMap::from([(0, 9)]));
    }

    let outcome = config
        .refresh_account_store_account_with_loader("demo", || Ok(None))
        .unwrap();

    assert_eq!(outcome, AccountSourceRefreshOutcome::Missing);
    let store = config.account_store.lock().unwrap();
    assert!(!store.accounts.contains_key("demo"));
    assert!(!store.source_account_versions.contains_key("demo"));
    assert!(!store.source_save_versions.contains_key("demo"));
}

#[test]
fn source_refresh_unavailable_preserves_cache_but_returns_error() {
    let mut config = SimulationConfig::default();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;

    let error = config
        .refresh_account_store_account_with_loader("demo", || {
            Err("injected authoritative source outage".to_string())
        })
        .unwrap_err();

    assert!(error.contains("injected authoritative source outage"));
    assert!(config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key("demo"));
}

#[test]
fn source_refresh_holds_persist_lock_until_cache_replacement_finishes() {
    let mut config = SimulationConfig::default();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    let stale_account = config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get("demo")
        .unwrap()
        .clone();
    let mut versions = AccountStoreSourceVersions::default();
    versions.accounts.insert("demo".to_string(), 1);

    let (load_started_tx, load_started_rx) = mpsc::channel();
    let (release_load_tx, release_load_rx) = mpsc::channel();
    let refresh_config = config.clone();
    let refresh = std::thread::spawn(move || {
        refresh_config.refresh_account_store_account_with_loader("demo", || {
            load_started_tx.send(()).unwrap();
            release_load_rx.recv().unwrap();
            Ok(Some((stale_account, versions)))
        })
    });
    load_started_rx.recv().unwrap();

    let (commit_done_tx, commit_done_rx) = mpsc::channel();
    let commit_config = config.clone();
    let commit = std::thread::spawn(move || {
        let result =
            commit_config.commit_account_store_transaction(&["demo".to_string()], |store| {
                let account = store.accounts.get_mut("demo").unwrap();
                account.password = "newer-committed-password".to_string();
                account.is_banned = true;
                account.ban_reason = "newer committed ban".to_string();
                Ok(())
            });
        commit_done_tx.send(()).unwrap();
        result
    });

    assert!(commit_done_rx
        .recv_timeout(Duration::from_millis(100))
        .is_err());
    release_load_tx.send(()).unwrap();
    assert_eq!(
        refresh.join().unwrap().unwrap(),
        AccountSourceRefreshOutcome::Refreshed
    );
    commit_done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    commit.join().unwrap().unwrap();

    let store = config.account_store.lock().unwrap();
    let account = store.accounts.get("demo").unwrap();
    assert_eq!(account.password, "newer-committed-password");
    assert!(account.is_banned);
    assert_eq!(account.ban_reason, "newer committed ban");
}

#[test]
fn source_account_cas_allows_only_matching_update_or_absent_new_insert() {
    assert!(validate_source_account_write(
        AccountStoreDatabaseMode::SourceOfTruth,
        Some(7),
        Some(7)
    )
    .is_ok());
    assert!(
        validate_source_account_write(AccountStoreDatabaseMode::SourceOfTruth, None, None).is_ok()
    );
    assert!(
        validate_source_account_write(AccountStoreDatabaseMode::SourceOfTruth, Some(7), None)
            .unwrap_err()
            .contains("account deleted")
    );
    assert!(
        validate_source_account_write(AccountStoreDatabaseMode::SourceOfTruth, None, Some(1))
            .unwrap_err()
            .contains("expected no row")
    );
}

#[test]
fn postgres_deleted_account_cannot_be_recreated_by_stale_save() {
    let database_url = std::env::var("MIR2_TEST_POSTGRES_URL")
        .unwrap_or_else(|_| "postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2".to_string());
    let Ok(mut cleanup_client) = Client::connect(&database_url, NoTls) else {
        eprintln!("skipping deleted-account CAS test because PostgreSQL is unavailable");
        return;
    };
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let account_id = format!("recovery-delete-{}-{unique}", std::process::id());
    let character = test_character(0, "DeletedRecoveryAccount");
    let mut accounts = BTreeMap::new();
    accounts.insert(account_id.clone(), AccountRecord::new(character.clone()));
    let store = AccountStore {
        schema_version: ACCOUNT_STORE_SCHEMA_VERSION,
        next_character_index: 1,
        game_shop_global_purchases: BTreeMap::new(),
        accounts,
        source_account_versions: BTreeMap::new(),
        source_save_versions: BTreeMap::new(),
        source_game_shop_global_version: None,
        source_game_shop_global_purchases: BTreeMap::new(),
    };
    let versions = save_account_store_to_postgres(
        database_url.clone(),
        store.clone(),
        AccountStoreDatabaseMode::Mirror,
    )
    .unwrap();
    let mut config = SimulationConfig::default();
    config.account_store = Arc::new(Mutex::new(store.with_source_versions(versions)));
    config.account_store_path = None;
    config.account_store_database_url = Some(database_url.clone());
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;

    cleanup_client
        .execute("DELETE FROM accounts WHERE account_id = $1", &[&account_id])
        .unwrap();
    let error = config.save_account_store_account(&account_id).unwrap_err();
    assert!(error.contains("found no row") || error.contains("account deleted"));
    let remaining: i64 = cleanup_client
        .query_one(
            "SELECT COUNT(*) FROM accounts WHERE account_id = $1",
            &[&account_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(remaining, 0);
}

const VALID_POSTGRES_ACCOUNT_STORE_URL: &str =
    "postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2";

fn authoritative_test_store(accounts: BTreeMap<String, AccountRecord>) -> AccountStore {
    AccountStore {
        schema_version: ACCOUNT_STORE_SCHEMA_VERSION,
        next_character_index: 0,
        game_shop_global_purchases: BTreeMap::new(),
        accounts,
        source_account_versions: BTreeMap::new(),
        source_save_versions: BTreeMap::new(),
        source_game_shop_global_version: None,
        source_game_shop_global_purchases: BTreeMap::new(),
    }
    .migrate_to_current_schema()
    .with_source_versions(AccountStoreSourceVersions::default())
}

#[test]
fn postgres_startup_loader_failure_is_propagated_without_default_fallback() {
    let loader_called = std::cell::Cell::new(false);
    let result = SimulationConfig::default().with_postgres_account_store_with_loader(
        VALID_POSTGRES_ACCOUNT_STORE_URL.to_string(),
        |database_url, _default_character| {
            loader_called.set(true);
            assert_eq!(database_url, VALID_POSTGRES_ACCOUNT_STORE_URL);
            Err("injected authoritative startup outage".to_string())
        },
    );

    let error = result
        .err()
        .expect("authoritative loader failure must reject config construction");
    assert!(loader_called.get());
    assert!(error.contains("injected authoritative startup outage"));
}

#[test]
fn postgres_startup_empty_authoritative_store_remains_empty() {
    let loader_calls = std::cell::Cell::new(0_u8);
    let config = SimulationConfig::default()
        .with_postgres_account_store_with_loader(
            VALID_POSTGRES_ACCOUNT_STORE_URL.to_string(),
            |_database_url, _default_character| {
                loader_calls.set(loader_calls.get() + 1);
                Ok(authoritative_test_store(BTreeMap::new()))
            },
        )
        .expect("empty authoritative database is a valid startup state");

    assert_eq!(loader_calls.get(), 1);
    assert!(config.account_store.lock().unwrap().accounts.is_empty());
    assert_eq!(
        config.account_store_database_mode,
        AccountStoreDatabaseMode::SourceOfTruth
    );
    assert_eq!(
        config.account_store_database_url.as_deref(),
        Some(VALID_POSTGRES_ACCOUNT_STORE_URL)
    );
    assert!(config.account_store_path.is_none());
}

#[test]
fn postgres_startup_preserves_real_authoritative_demo_account() {
    let mut demo = AccountRecord::new(test_character(42, "AuthoritativeDemo"));
    demo.password = "database-owned-password".to_string();
    let store = authoritative_test_store(BTreeMap::from([("demo".to_string(), demo)]));

    let config = SimulationConfig::default()
        .with_postgres_account_store_with_loader(
            VALID_POSTGRES_ACCOUNT_STORE_URL.to_string(),
            |_database_url, _default_character| Ok(store),
        )
        .expect("authoritative demo account should load");

    let account_store = config.account_store.lock().unwrap();
    assert_eq!(account_store.accounts.len(), 1);
    let demo = account_store.accounts.get("demo").unwrap();
    assert_eq!(demo.password, "database-owned-password");
    assert_eq!(demo.characters[0].name, "AuthoritativeDemo");
    assert!(!account_store.accounts.contains_key("Demo"));
}

#[test]
fn postgres_url_matrix_accepts_supported_forms_and_never_loads_invalid_inputs() {
    for database_url in [
        "postgres://user:password@127.0.0.1:5432/mir2",
        "postgresql://user:password@[2001:db8::1]:5432/mir2",
        "postgres://user:p%40ss@localhost/mir2%2Dprod",
        "postgresql://user:password@localhost/mir2?sslmode=disable&connect_timeout=5&application_name=mir2",
    ] {
        let loader_calls = std::cell::Cell::new(0_u8);
        let config = SimulationConfig::default()
            .with_postgres_account_store_with_loader(
                database_url.to_string(),
                |_database_url, _default_character| {
                    loader_calls.set(loader_calls.get() + 1);
                    Ok(authoritative_test_store(BTreeMap::new()))
                },
            )
            .expect("supported PostgreSQL URL must reach the injected loader");
        assert_eq!(loader_calls.get(), 1, "{database_url}");
        assert_eq!(
            config.account_store_database_url.as_deref(),
            Some(database_url)
        );
    }

    for database_url in [
        "",
        "   ",
        "http://127.0.0.1/not-postgres",
        "postgres://user:pass@localhost:abc/mir2",
        "postgres://user\n:pass@localhost/mir2",
        "postgres://user:\0pass@localhost/mir2",
        "postgresql://user:pass@[2001:db8::1/mir2",
    ] {
        let loader_called = std::cell::Cell::new(false);
        let result = SimulationConfig::default().with_postgres_account_store_with_loader(
            database_url.to_string(),
            |_database_url, _default_character| {
                loader_called.set(true);
                Ok(authoritative_test_store(BTreeMap::new()))
            },
        );
        let error = result
            .err()
            .expect("invalid PostgreSQL URL must fail closed");
        assert!(!loader_called.get(), "loader called for {database_url:?}");
        assert!(
            error.contains("empty")
                || error.contains("whitespace")
                || error.contains("control")
                || error.contains("scheme")
                || error.contains("invalid postgres"),
            "unexpected validation error for {database_url:?}: {error}"
        );
    }
}

#[test]
fn development_file_fixture_still_supplies_demo_account() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "mir2-config-file-fixture-{}-{unique}.json",
        std::process::id()
    ));
    assert!(!path.exists());

    let config = SimulationConfig::default().with_account_store_path(path.clone());

    assert!(config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key("demo"));
    assert_eq!(
        config.account_store_database_mode,
        AccountStoreDatabaseMode::Mirror
    );
    assert_eq!(config.account_store_path.as_deref(), Some(path.as_path()));
}
fn source_of_truth_test_config(accounts: BTreeMap<String, AccountRecord>) -> SimulationConfig {
    source_of_truth_test_config_with_store(authoritative_test_store(accounts))
}

fn source_of_truth_test_config_with_store(store: AccountStore) -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.account_store = Arc::new(Mutex::new(store));
    config.account_store_path = None;
    config.account_store_database_url = None;
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountStoreAuditSnapshot {
    json: Vec<u8>,
    source_account_versions: BTreeMap<String, i64>,
    source_save_versions: BTreeMap<String, BTreeMap<i32, i64>>,
    source_game_shop_global_version: Option<i64>,
    source_game_shop_global_purchases: BTreeMap<i32, u64>,
}

fn account_store_audit_snapshot(config: &SimulationConfig) -> AccountStoreAuditSnapshot {
    let store = config.account_store.lock().unwrap();
    AccountStoreAuditSnapshot {
        json: serde_json::to_vec(&*store).unwrap(),
        source_account_versions: store.source_account_versions.clone(),
        source_save_versions: store.source_save_versions.clone(),
        source_game_shop_global_version: store.source_game_shop_global_version,
        source_game_shop_global_purchases: store.source_game_shop_global_purchases.clone(),
    }
}

fn stage5_test_delivery(
    target_kind: Stage5MailTargetKind,
    target_id: impl Into<String>,
) -> Stage5MailDelivery {
    Stage5MailDelivery {
        target_kind,
        target_id: target_id.into(),
        from: "System".to_string(),
        subject: "Fail-closed probe".to_string(),
        body: "Must publish atomically.".to_string(),
        gold: 7,
        items: vec!["red-potion".to_string()],
    }
}

fn unique_test_path(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "mir2-config-refresh-{label}-{}-{unique}.json",
        std::process::id()
    ))
}

#[test]
fn postgres_row_assembler_keeps_zero_rows_empty() {
    let store = assemble_account_store_from_postgres_rows(
        Vec::new(),
        Vec::new(),
        BTreeMap::from([(17, 3)]),
        Some(9),
    )
    .expect("zero authoritative rows should assemble");

    assert!(store.accounts.is_empty());
    assert_eq!(store.game_shop_global_purchases.get(&17), Some(&3));
    assert_eq!(store.source_game_shop_global_version, Some(9));
    assert!(store.source_account_versions.is_empty());
    assert!(store.source_save_versions.is_empty());
}

#[test]
fn postgres_row_assembler_preserves_real_demo_and_versions() {
    let mut demo = AccountRecord::new(test_character(7, "DatabaseDemo"));
    demo.password = "authoritative-password".to_string();
    let store = assemble_account_store_from_postgres_rows(
        vec![("demo".to_string(), serde_json::to_value(demo).unwrap(), 41)],
        vec![("demo".to_string(), 7, 43)],
        BTreeMap::new(),
        None,
    )
    .expect("real demo row should assemble");

    assert_eq!(store.accounts.len(), 1);
    let demo = store.accounts.get("demo").unwrap();
    assert_eq!(demo.password, "authoritative-password");
    assert_eq!(demo.characters[0].name, "DatabaseDemo");
    assert_eq!(store.source_account_versions.get("demo"), Some(&41));
    assert_eq!(
        store
            .source_save_versions
            .get("demo")
            .and_then(|versions| versions.get(&7)),
        Some(&43)
    );
}

#[test]
fn postgres_row_assembler_rejects_bad_account_raw_json() {
    let error = assemble_account_store_from_postgres_rows(
        vec![(
            "broken-account".to_string(),
            serde_json::json!("not an account record"),
            1,
        )],
        Vec::new(),
        BTreeMap::new(),
        None,
    )
    .unwrap_err();

    assert!(error.contains("broken-account"));
    assert!(error.contains("raw_json decode failed"));
}

#[test]
fn environment_entry_propagates_injected_loader_errors_without_global_env() {
    let loader_called = std::cell::Cell::new(false);
    let result = SimulationConfig::default().with_account_store_environment_with_loader(
        std::path::PathBuf::from("unused-file-fixture.json"),
        AccountStoreRuntimeBackend::Postgres,
        Some(VALID_POSTGRES_ACCOUNT_STORE_URL.to_string()),
        |_database_url, _default_character| {
            loader_called.set(true);
            Err("injected environment loader outage".to_string())
        },
    );
    let error = result
        .err()
        .expect("environment loader failure must reject construction");
    assert!(loader_called.get());
    assert!(error.contains("injected environment loader outage"));

    let loader_called = std::cell::Cell::new(false);
    let missing_url = SimulationConfig::default().with_account_store_environment_with_loader(
        std::path::PathBuf::from("unused-file-fixture.json"),
        AccountStoreRuntimeBackend::Postgres,
        None,
        |_database_url, _default_character| {
            loader_called.set(true);
            Ok(authoritative_test_store(BTreeMap::new()))
        },
    );
    assert!(missing_url
        .err()
        .expect("missing environment URL must fail closed")
        .contains("MIR2_ACCOUNT_STORE_DATABASE_URL is required"));
    assert!(!loader_called.get());
}

#[test]
fn source_of_truth_mail_on_empty_store_never_creates_demo() {
    let config = source_of_truth_test_config(BTreeMap::new());
    let before = account_store_audit_snapshot(&config);

    let receipt = deliver_stage5_system_mail(
        &config,
        stage5_test_delivery(Stage5MailTargetKind::Global, ""),
    )
    .expect("global delivery to no real characters is a no-op");
    assert_eq!(receipt.delivered_count, 0);
    assert!(receipt.mail_ids.is_empty());

    for delivery in [
        stage5_test_delivery(Stage5MailTargetKind::Account, "demo"),
        stage5_test_delivery(
            Stage5MailTargetKind::Character,
            config.default_character.name.clone(),
        ),
    ] {
        assert!(deliver_stage5_system_mail(&config, delivery).is_err());
    }

    assert_eq!(account_store_audit_snapshot(&config), before);
    assert!(!config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key("demo"));
}

#[test]
fn system_mail_persist_fault_keeps_live_store_bytes_unchanged() {
    let config = SimulationConfig::default();
    let before = account_store_audit_snapshot(&config);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);

    let error = deliver_stage5_system_mail(
        &config,
        stage5_test_delivery(
            Stage5MailTargetKind::Character,
            config.default_character.name.clone(),
        ),
    )
    .unwrap_err();

    assert!(error.contains("injected account-store persistence failure"));
    assert_eq!(account_store_audit_snapshot(&config), before);
}

#[test]
fn system_mail_unknown_file_publication_freezes_writes_without_live_publish() {
    let path = unique_test_path("mail-unknown-publication");
    let config = SimulationConfig::default().with_account_store_path(path.clone());
    let before = account_store_audit_snapshot(&config);
    config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );

    let error = deliver_stage5_system_mail(
        &config,
        stage5_test_delivery(
            Stage5MailTargetKind::Character,
            config.default_character.name.clone(),
        ),
    )
    .unwrap_err();

    assert!(error.contains(ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN));
    assert_eq!(account_store_audit_snapshot(&config), before);
    assert!(matches!(
        &*config.account_store_write_state.lock().unwrap(),
        AccountStoreWriteState::Frozen { .. }
    ));

    let retry = ban_account_in_store(&config, "demo", Some(60), "frozen retry");
    assert!(retry.unwrap_err().contains("writes are frozen"));
    assert_eq!(account_store_audit_snapshot(&config), before);
    let _ = std::fs::remove_file(path);
}

#[test]
fn ban_persist_fault_missing_account_and_alias_never_mutate_or_create() {
    let config = SimulationConfig::default();
    let before = account_store_audit_snapshot(&config);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);

    let persist_error =
        ban_account_in_store(&config, "demo", Some(60), "persist fault").unwrap_err();
    assert!(persist_error.contains("injected account-store persistence failure"));
    assert_eq!(account_store_audit_snapshot(&config), before);

    let missing_error =
        ban_account_in_store(&config, "missing-account", None, "must not create").unwrap_err();
    assert!(missing_error.contains("account not found"));
    assert_eq!(account_store_audit_snapshot(&config), before);
    assert!(!config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key("missing-account"));

    let alias_error = ban_account_in_store(&config, " demo ", None, "alias").unwrap_err();
    assert!(alias_error.contains("leading or trailing whitespace"));
    assert_eq!(account_store_audit_snapshot(&config), before);
}

#[test]
fn backup_restore_respects_authoritative_and_file_fixture_seed_rules() {
    let empty_backup = unique_test_path("empty-backup");
    authoritative_test_store(BTreeMap::new())
        .save_to_path(&empty_backup)
        .unwrap();

    let source_config = source_of_truth_test_config(BTreeMap::new());
    source_config
        .restore_account_store_from_backup(&empty_backup)
        .expect("empty authoritative backup should restore");
    assert!(source_config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .is_empty());

    let primary = unique_test_path("file-primary");
    let file_config = SimulationConfig::default().with_account_store_path(primary.clone());
    file_config
        .restore_account_store_from_backup(&empty_backup)
        .expect("file fixture restore should retain development seed");
    assert!(file_config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key("demo"));

    let _ = std::fs::remove_file(empty_backup);
    let _ = std::fs::remove_file(primary);
}

#[test]
fn source_backup_restore_preserves_real_demo_and_faults_before_live_publish() {
    let backup = unique_test_path("real-demo-backup");
    let mut demo = AccountRecord::new(test_character(9, "RestoredDatabaseDemo"));
    demo.password = "restored-authoritative-password".to_string();
    authoritative_test_store(BTreeMap::from([("demo".to_string(), demo)]))
        .save_to_path(&backup)
        .unwrap();

    let source_config = source_of_truth_test_config(BTreeMap::new());
    source_config
        .restore_account_store_from_backup(&backup)
        .expect("real demo backup should restore without replacement");
    {
        let store = source_config.account_store.lock().unwrap();
        assert_eq!(store.accounts.len(), 1);
        let demo = store.accounts.get("demo").unwrap();
        assert_eq!(demo.password, "restored-authoritative-password");
        assert_eq!(demo.characters[0].name, "RestoredDatabaseDemo");
    }

    let empty_backup = unique_test_path("fault-empty-backup");
    authoritative_test_store(BTreeMap::new())
        .save_to_path(&empty_backup)
        .unwrap();
    let before = account_store_audit_snapshot(&source_config);
    source_config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    let error = source_config
        .restore_account_store_from_backup(&empty_backup)
        .unwrap_err();
    assert!(error.contains("injected account-store persistence failure"));
    assert_eq!(account_store_audit_snapshot(&source_config), before);

    let _ = std::fs::remove_file(backup);
    let _ = std::fs::remove_file(empty_backup);
}

#[derive(Debug, Clone, Default)]
struct FakeMutationRepository {
    accounts: BTreeMap<String, AccountRecord>,
    account_versions: BTreeMap<String, i64>,
    save_versions: BTreeMap<String, BTreeMap<i32, i64>>,
    global_version: Option<i64>,
    global_purchases: BTreeMap<i32, u64>,
}

fn fake_mutation_repository_from_store(store: &AccountStore) -> FakeMutationRepository {
    FakeMutationRepository {
        accounts: store.accounts.clone(),
        account_versions: store.source_account_versions.clone(),
        save_versions: store.source_save_versions.clone(),
        global_version: store.source_game_shop_global_version,
        global_purchases: store.source_game_shop_global_purchases.clone(),
    }
}

fn apply_plan_to_fake_repository_atomically(
    repository: &mut FakeMutationRepository,
    plan: &AccountStoreMutationPlan,
    fail_at_account_ordinal: Option<usize>,
) -> Result<AccountStoreSourceVersions, String> {
    let mut transaction = repository.clone();
    let mut account_ordinal = 0_usize;
    let versions = execute_account_store_mutation_plan(plan, |operation| match operation {
        AccountStoreMutationOperation::GlobalStock(mutation) => {
            let current_purchases = transaction
                .global_version
                .map(|_| transaction.global_purchases.clone());
            validate_source_game_shop_global_write(
                mutation.expected_version,
                transaction.global_version,
                &mutation.expected_purchases,
                current_purchases.as_ref(),
            )?;
            let next_version = transaction
                .global_version
                .map(|version| version.saturating_add(1))
                .unwrap_or(1);
            transaction.global_version = Some(next_version);
            transaction.global_purchases = mutation.desired_purchases.clone();
            Ok(AccountStoreMutationOperationResult::GlobalStock(Some(
                next_version,
            )))
        }
        AccountStoreMutationOperation::Account {
            account_id,
            mutation,
        } => {
            account_ordinal = account_ordinal.saturating_add(1);
            let current_account_version = transaction.account_versions.get(account_id).copied();
            let current_save_versions = transaction
                .save_versions
                .get(account_id)
                .cloned()
                .unwrap_or_default();
            validate_account_store_account_mutation(
                account_id,
                mutation,
                current_account_version,
                &current_save_versions,
                AccountStoreDatabaseMode::SourceOfTruth,
            )?;
            if fail_at_account_ordinal == Some(account_ordinal) {
                return Err(format!(
                    "injected fake repository fault at account {account_id}"
                ));
            }

            if mutation.desired_account.is_none() {
                transaction.accounts.remove(account_id);
                transaction.account_versions.remove(account_id);
                transaction.save_versions.remove(account_id);
                return Ok(AccountStoreMutationOperationResult::Account {
                    account_version: None,
                    save_versions: BTreeMap::new(),
                });
            }

            let desired_account = mutation
                .desired_account
                .as_ref()
                .expect("retained account mutation must have desired contents");
            transaction
                .accounts
                .insert(account_id.to_string(), desired_account.clone());
            let next_account_version = current_account_version
                .map(|version| version.saturating_add(1))
                .unwrap_or(1);
            transaction
                .account_versions
                .insert(account_id.to_string(), next_account_version);
            let mut next_saves = transaction
                .save_versions
                .remove(account_id)
                .unwrap_or_default();
            next_saves.retain(|character_index, _| {
                mutation
                    .saves
                    .get(character_index)
                    .is_some_and(|save| save.desired_save.is_some())
            });
            let mut returned_save_versions = BTreeMap::new();
            for (character_index, save_mutation) in &mutation.saves {
                if save_mutation.desired_save.is_none() {
                    next_saves.remove(character_index);
                    continue;
                }
                let next_version = current_save_versions
                    .get(character_index)
                    .copied()
                    .map(|version| version.saturating_add(1))
                    .unwrap_or(1);
                next_saves.insert(*character_index, next_version);
                returned_save_versions.insert(*character_index, next_version);
            }
            if !next_saves.is_empty() {
                transaction
                    .save_versions
                    .insert(account_id.to_string(), next_saves);
            }
            Ok(AccountStoreMutationOperationResult::Account {
                account_version: Some(next_account_version),
                save_versions: returned_save_versions,
            })
        }
    })?;
    *repository = transaction;
    Ok(versions)
}

fn versioned_two_account_store() -> AccountStore {
    let alpha = AccountRecord::new(test_character(1, "Alpha"));
    let beta = AccountRecord::new(test_character(2, "Beta"));
    let mut store = authoritative_test_store(BTreeMap::from([
        ("alpha".to_string(), alpha),
        ("beta".to_string(), beta),
    ]));
    store.source_account_versions =
        BTreeMap::from([("alpha".to_string(), 11), ("beta".to_string(), 12)]);
    store.source_save_versions = BTreeMap::from([
        ("alpha".to_string(), BTreeMap::from([(1, 101)])),
        ("beta".to_string(), BTreeMap::from([(2, 102)])),
    ]);
    store.game_shop_global_purchases = BTreeMap::from([(7, 4)]);
    store.source_game_shop_global_version = Some(21);
    store.source_game_shop_global_purchases = store.game_shop_global_purchases.clone();
    store
}

fn alpha_repository_probe_outcome(
    account_version: i64,
    save_version: i64,
    global_version: Option<i64>,
) -> AccountStoreRepositorySave {
    AccountStoreRepositorySave {
        account_versions: BTreeMap::from([("alpha".to_string(), account_version)]),
        save_versions: BTreeMap::from([("alpha".to_string(), BTreeMap::from([(1, save_version)]))]),
        game_shop_global_version: global_version,
    }
}

#[test]
fn account_scope_guard_skips_repository_probe_and_canonical_commit_writes_once() {
    let mut config = source_of_truth_test_config_with_store(versioned_two_account_store());
    config.account_store_database_url = Some(VALID_POSTGRES_ACCOUNT_STORE_URL.to_string());
    config.inject_account_store_repository_writer_probe(Ok(alpha_repository_probe_outcome(
        12, 102, None,
    )));
    let before = account_store_audit_snapshot(&config);
    let account_ids = vec!["alpha".to_string()];

    let error = config
        .commit_account_store_transaction(&account_ids, |store| {
            store.game_shop_global_purchases.insert(8, 2);
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("account-only closure changed global stock"));
    assert_eq!(
        config.account_store_repository_writer_probe_invocations(),
        0
    );
    assert_eq!(
        config.account_store_repository_writer_probe_last_plan_includes_global(),
        None
    );
    assert_eq!(account_store_audit_snapshot(&config), before);

    config
        .commit_account_store_transaction(&account_ids, |store| {
            store.accounts.get_mut("alpha").unwrap().password = "probe account commit".to_string();
            Ok(())
        })
        .expect("canonical account transaction should use the injected repository writer");
    assert_eq!(
        config.account_store_repository_writer_probe_invocations(),
        1
    );
    assert_eq!(
        config.account_store_repository_writer_probe_last_plan_includes_global(),
        Some(false)
    );
    let store = config.account_store.lock().unwrap();
    assert_eq!(
        store.accounts.get("alpha").unwrap().password,
        "probe account commit"
    );
    assert_eq!(store.source_account_versions.get("alpha"), Some(&12));
    assert_eq!(
        store
            .source_save_versions
            .get("alpha")
            .and_then(|versions| versions.get(&1)),
        Some(&102)
    );
    assert_eq!(store.source_game_shop_global_version, Some(21));
    assert_eq!(
        store.source_game_shop_global_purchases,
        BTreeMap::from([(7, 4)])
    );
}

#[test]
fn global_scope_guard_skips_repository_probe_and_canonical_commit_writes_once() {
    let mut config = source_of_truth_test_config_with_store(versioned_two_account_store());
    config.account_store_database_url = Some(VALID_POSTGRES_ACCOUNT_STORE_URL.to_string());
    config.inject_account_store_repository_writer_probe(Ok(alpha_repository_probe_outcome(
        12,
        102,
        Some(22),
    )));
    let before = account_store_audit_snapshot(&config);
    let account_ids = vec!["alpha".to_string()];

    let error = config
        .commit_account_store_transaction_with_global(&account_ids, |store| {
            store.accounts.get_mut("beta").unwrap().password = "scope escape".to_string();
            store.game_shop_global_purchases.insert(8, 2);
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("account beta changed outside the authorized scope"));
    assert_eq!(
        config.account_store_repository_writer_probe_invocations(),
        0
    );
    assert_eq!(
        config.account_store_repository_writer_probe_last_plan_includes_global(),
        None
    );
    assert_eq!(account_store_audit_snapshot(&config), before);

    let error = config
        .commit_account_store_transaction_with_global(&account_ids, |store| {
            store.source_game_shop_global_version = Some(999);
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("protected source metadata source_game_shop_global_version"));
    assert_eq!(
        config.account_store_repository_writer_probe_invocations(),
        0
    );
    assert_eq!(
        config.account_store_repository_writer_probe_last_plan_includes_global(),
        None
    );
    assert_eq!(account_store_audit_snapshot(&config), before);

    config
        .commit_account_store_transaction_with_global(&account_ids, |store| {
            store.accounts.get_mut("alpha").unwrap().password = "probe global commit".to_string();
            store.game_shop_global_purchases.insert(8, 2);
            Ok(())
        })
        .expect("canonical global transaction should use the injected repository writer");
    assert_eq!(
        config.account_store_repository_writer_probe_invocations(),
        1
    );
    assert_eq!(
        config.account_store_repository_writer_probe_last_plan_includes_global(),
        Some(true)
    );
    let store = config.account_store.lock().unwrap();
    assert_eq!(
        store.accounts.get("alpha").unwrap().password,
        "probe global commit"
    );
    assert_eq!(store.source_account_versions.get("alpha"), Some(&12));
    assert_eq!(
        store
            .source_save_versions
            .get("alpha")
            .and_then(|versions| versions.get(&1)),
        Some(&102)
    );
    assert_eq!(store.source_game_shop_global_version, Some(22));
    assert_eq!(
        store.source_game_shop_global_purchases,
        BTreeMap::from([(7, 4), (8, 2)])
    );
}

#[test]
fn account_scoped_commit_rejects_scope_escape_before_any_file_write() {
    let path = unique_test_path("account-scope-boundary");
    let original = versioned_two_account_store();
    let beta_password = original.accounts.get("beta").unwrap().password.clone();
    let mut config = SimulationConfig::default().with_account_store_path(path.clone());
    config.account_store = Arc::new(Mutex::new(original));
    config.account_store_database_mode = AccountStoreDatabaseMode::Mirror;
    config
        .save_account_store()
        .expect("baseline account store should persist");
    let before = account_store_audit_snapshot(&config);
    let file_before = std::fs::read(&path).unwrap();
    let account_ids = vec!["alpha".to_string()];

    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);

    let error = config
        .commit_account_store_transaction(&account_ids, |store| {
            store.accounts.get_mut("beta").unwrap().password = "scope escape".to_string();
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("account beta changed outside the authorized scope"));
    assert_eq!(account_store_audit_snapshot(&config), before);
    assert_eq!(std::fs::read(&path).unwrap(), file_before);

    let error = config
        .commit_account_store_transaction(&account_ids, |store| {
            store.accounts.remove("beta");
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("account beta changed outside the authorized scope"));
    assert_eq!(account_store_audit_snapshot(&config), before);
    assert_eq!(std::fs::read(&path).unwrap(), file_before);

    let error = config
        .commit_account_store_transaction(&account_ids, |store| {
            store
                .source_account_versions
                .insert("alpha".to_string(), 999);
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("protected source metadata source_account_versions"));
    assert_eq!(account_store_audit_snapshot(&config), before);
    assert_eq!(std::fs::read(&path).unwrap(), file_before);

    let error = config
        .commit_account_store_transaction(&account_ids, |store| {
            store.game_shop_global_purchases.insert(7, 5);
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("account-only closure changed global stock"));
    assert_eq!(account_store_audit_snapshot(&config), before);
    assert_eq!(std::fs::read(&path).unwrap(), file_before);

    let error = config
        .commit_account_store_transaction(&account_ids, |store| {
            store.accounts.get_mut("alpha").unwrap().password = "valid alpha".to_string();
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("injected failure before account-store rename"));
    assert_eq!(account_store_audit_snapshot(&config), before);
    assert_eq!(std::fs::read(&path).unwrap(), file_before);

    config
        .commit_account_store_transaction(&account_ids, |store| {
            store.accounts.get_mut("alpha").unwrap().password = "valid alpha".to_string();
            Ok(())
        })
        .expect("canonical account-only mutation should commit after the retained fault fires");
    let live = config.account_store.lock().unwrap();
    assert_eq!(live.accounts.get("alpha").unwrap().password, "valid alpha");
    assert_eq!(live.accounts.get("beta").unwrap().password, beta_password);
    drop(live);
    let persisted: AccountStore = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        persisted.accounts.get("alpha").unwrap().password,
        "valid alpha"
    );
    assert_eq!(
        persisted.accounts.get("beta").unwrap().password,
        beta_password
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn explicit_global_commit_allows_global_but_rejects_other_accounts_and_metadata() {
    let original = versioned_two_account_store();
    let original_source_accounts = original.source_account_versions.clone();
    let original_source_saves = original.source_save_versions.clone();
    let original_source_global_version = original.source_game_shop_global_version;
    let original_source_global_purchases = original.source_game_shop_global_purchases.clone();
    let beta_password = original.accounts.get("beta").unwrap().password.clone();
    let config = source_of_truth_test_config_with_store(original);
    let before = account_store_audit_snapshot(&config);
    let account_ids = vec!["alpha".to_string()];

    let error = config
        .commit_account_store_transaction_with_global(&account_ids, |store| {
            store.accounts.get_mut("beta").unwrap().password = "scope escape".to_string();
            store.game_shop_global_purchases.insert(8, 2);
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("account beta changed outside the authorized scope"));
    assert_eq!(account_store_audit_snapshot(&config), before);

    let error = config
        .commit_account_store_transaction_with_global(&account_ids, |store| {
            store.source_game_shop_global_version = Some(999);
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("protected source metadata source_game_shop_global_version"));
    assert_eq!(account_store_audit_snapshot(&config), before);

    config
        .commit_account_store_transaction_with_global(&account_ids, |store| {
            store.accounts.get_mut("alpha").unwrap().password = "global alpha".to_string();
            store.game_shop_global_purchases.insert(8, 2);
            Ok(())
        })
        .expect("explicit global transaction should accept its canonical stock mutation");
    let store = config.account_store.lock().unwrap();
    assert_eq!(
        store.accounts.get("alpha").unwrap().password,
        "global alpha"
    );
    assert_eq!(store.accounts.get("beta").unwrap().password, beta_password);
    assert_eq!(
        store.game_shop_global_purchases,
        BTreeMap::from([(7, 4), (8, 2)])
    );
    assert_eq!(store.source_account_versions, original_source_accounts);
    assert_eq!(store.source_save_versions, original_source_saves);
    assert_eq!(
        store.source_game_shop_global_version,
        original_source_global_version
    );
    assert_eq!(
        store.source_game_shop_global_purchases,
        original_source_global_purchases
    );
}

#[test]
fn legacy_full_save_publishes_account_tombstone_then_recreates_with_expected_none() {
    let mut original = versioned_two_account_store();
    original.accounts.remove("beta");
    original.source_account_versions.remove("beta");
    original.source_save_versions.remove("beta");
    let mut durable = fake_mutation_repository_from_store(&original);
    let mut config = source_of_truth_test_config_with_store(original);
    config.account_store_database_url = Some(VALID_POSTGRES_ACCOUNT_STORE_URL.to_string());
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .remove("alpha");

    config
        .save_account_store_with_plan_writer(None, |_database_url, mode, plan| {
            assert_eq!(mode, AccountStoreDatabaseMode::SourceOfTruth);
            assert!(plan.global_stock.is_none());
            let alpha = plan.accounts.get("alpha").unwrap();
            assert_eq!(alpha.expected_version, Some(11));
            assert!(alpha.desired_account.is_none());
            assert_eq!(alpha.saves.get(&1).unwrap().expected_version, Some(101));
            assert!(alpha.saves.get(&1).unwrap().desired_save.is_none());
            apply_plan_to_fake_repository_atomically(&mut durable, plan, None)
        })
        .expect("legacy full save should publish an account tombstone");
    {
        let store = config.account_store.lock().unwrap();
        assert!(!store.accounts.contains_key("alpha"));
        assert!(!store.source_account_versions.contains_key("alpha"));
        assert!(!store.source_save_versions.contains_key("alpha"));
        assert_eq!(store.source_game_shop_global_version, Some(21));
        assert_eq!(
            store.source_game_shop_global_purchases,
            BTreeMap::from([(7, 4)])
        );
    }
    assert!(!durable.accounts.contains_key("alpha"));
    assert!(!durable.account_versions.contains_key("alpha"));
    assert!(!durable.save_versions.contains_key("alpha"));

    config.account_store.lock().unwrap().accounts.insert(
        "alpha".to_string(),
        AccountRecord::new(test_character(1, "AlphaRecreated")),
    );
    config
        .save_account_store_with_plan_writer(None, |_database_url, mode, plan| {
            assert_eq!(mode, AccountStoreDatabaseMode::SourceOfTruth);
            let alpha = plan.accounts.get("alpha").unwrap();
            assert_eq!(alpha.expected_version, None);
            assert_eq!(alpha.saves.get(&1).unwrap().expected_version, None);
            assert!(alpha.desired_account.is_some());
            assert!(alpha.saves.get(&1).unwrap().desired_save.is_some());
            apply_plan_to_fake_repository_atomically(&mut durable, plan, None)
        })
        .expect("legacy full save should recreate a tombstoned account without refresh");
    let store = config.account_store.lock().unwrap();
    assert_eq!(store.source_account_versions.get("alpha"), Some(&1));
    assert_eq!(
        store
            .source_save_versions
            .get("alpha")
            .and_then(|versions| versions.get(&1)),
        Some(&1)
    );
    assert_eq!(
        durable.accounts.get("alpha").unwrap().characters[0].name,
        "AlphaRecreated"
    );
}

#[test]
fn legacy_account_save_publishes_save_tombstone_and_preserves_untouched_metadata() {
    let original = versioned_two_account_store();
    let mut durable = fake_mutation_repository_from_store(&original);
    let mut config = source_of_truth_test_config_with_store(original);
    config.account_store_database_url = Some(VALID_POSTGRES_ACCOUNT_STORE_URL.to_string());
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("alpha")
        .unwrap()
        .saves
        .remove(&1);

    config
        .save_account_store_with_plan_writer(Some("alpha"), |_database_url, mode, plan| {
            assert_eq!(mode, AccountStoreDatabaseMode::SourceOfTruth);
            assert!(plan.global_stock.is_none());
            assert_eq!(plan.accounts.len(), 1);
            assert!(!plan.accounts.contains_key("beta"));
            let alpha = plan.accounts.get("alpha").unwrap();
            assert_eq!(alpha.expected_version, Some(11));
            assert_eq!(alpha.saves.get(&1).unwrap().expected_version, Some(101));
            assert!(alpha.saves.get(&1).unwrap().desired_save.is_none());
            apply_plan_to_fake_repository_atomically(&mut durable, plan, None)
        })
        .expect("legacy account save should publish a retained-account save tombstone");
    {
        let store = config.account_store.lock().unwrap();
        assert_eq!(store.source_account_versions.get("alpha"), Some(&12));
        assert!(!store.source_save_versions.contains_key("alpha"));
        assert_eq!(store.source_account_versions.get("beta"), Some(&12));
        assert_eq!(
            store
                .source_save_versions
                .get("beta")
                .and_then(|versions| versions.get(&2)),
            Some(&102)
        );
        assert_eq!(store.source_game_shop_global_version, Some(21));
        assert_eq!(
            store.source_game_shop_global_purchases,
            BTreeMap::from([(7, 4)])
        );
    }
    assert!(!durable
        .accounts
        .get("alpha")
        .unwrap()
        .saves
        .contains_key(&1));
    assert!(!durable.save_versions.contains_key("alpha"));

    {
        let mut store = config.account_store.lock().unwrap();
        let character = store.accounts.get("alpha").unwrap().characters[0].clone();
        store
            .accounts
            .get_mut("alpha")
            .unwrap()
            .saves
            .insert(1, CharacterSaveRecord::new(character));
    }
    config
        .save_account_store_with_plan_writer(Some("alpha"), |_database_url, mode, plan| {
            assert_eq!(mode, AccountStoreDatabaseMode::SourceOfTruth);
            let alpha = plan.accounts.get("alpha").unwrap();
            assert_eq!(alpha.expected_version, Some(12));
            assert_eq!(alpha.saves.get(&1).unwrap().expected_version, None);
            assert!(alpha.saves.get(&1).unwrap().desired_save.is_some());
            apply_plan_to_fake_repository_atomically(&mut durable, plan, None)
        })
        .expect("legacy account save should recreate a deleted save without refresh");
    let store = config.account_store.lock().unwrap();
    assert_eq!(store.source_account_versions.get("alpha"), Some(&13));
    assert_eq!(
        store
            .source_save_versions
            .get("alpha")
            .and_then(|versions| versions.get(&1)),
        Some(&1)
    );
    assert_eq!(store.source_account_versions.get("beta"), Some(&12));
    assert_eq!(
        store
            .source_save_versions
            .get("beta")
            .and_then(|versions| versions.get(&2)),
        Some(&102)
    );
    assert_eq!(store.source_game_shop_global_version, Some(21));
    assert!(durable
        .accounts
        .get("alpha")
        .unwrap()
        .saves
        .contains_key(&1));
    assert_eq!(
        durable
            .save_versions
            .get("alpha")
            .and_then(|versions| versions.get(&1)),
        Some(&1)
    );
}

#[test]
fn full_restore_plan_emits_account_save_tombstones_and_global_reset() {
    let original = versioned_two_account_store();
    let mut desired = authoritative_test_store(BTreeMap::new());
    let plan = build_account_store_mutation_plan(
        &original,
        &desired,
        AccountStoreMutationScope::FullRestore,
        false,
    );

    assert_eq!(plan.accounts.len(), 2);
    let alpha = plan.accounts.get("alpha").unwrap();
    assert_eq!(alpha.expected_version, Some(11));
    assert!(alpha.desired_account.is_none());
    assert_eq!(alpha.saves.get(&1).unwrap().expected_version, Some(101));
    assert!(alpha.saves.get(&1).unwrap().desired_save.is_none());
    let beta = plan.accounts.get("beta").unwrap();
    assert_eq!(beta.expected_version, Some(12));
    assert_eq!(beta.saves.get(&2).unwrap().expected_version, Some(102));
    assert!(beta.desired_account.is_none());

    let global = plan.global_stock.as_ref().unwrap();
    assert_eq!(global.expected_version, Some(21));
    assert_eq!(global.expected_purchases, BTreeMap::from([(7, 4)]));
    assert!(global.desired_purchases.is_empty());

    let mut durable = fake_mutation_repository_from_store(&original);
    let versions = apply_plan_to_fake_repository_atomically(&mut durable, &plan, None).unwrap();
    apply_account_store_mutation_source_versions(&mut desired, &plan, versions);
    assert!(durable.accounts.is_empty());
    assert!(durable.account_versions.is_empty());
    assert!(durable.save_versions.is_empty());
    assert!(durable.global_purchases.is_empty());
    assert_eq!(durable.global_version, Some(22));
    assert!(desired.source_account_versions.is_empty());
    assert!(desired.source_save_versions.is_empty());
    assert_eq!(desired.source_game_shop_global_version, Some(22));
    assert!(desired.source_game_shop_global_purchases.is_empty());
}

#[test]
fn full_restore_uses_live_versions_for_replacement_and_ignores_backup_metadata() {
    let original = versioned_two_account_store();
    let mut replacement = AccountRecord::new(test_character(1, "AlphaRestored"));
    replacement.password = "replacement-password".to_string();
    let mut desired = authoritative_test_store(BTreeMap::from([
        ("alpha".to_string(), replacement),
        (
            "gamma".to_string(),
            AccountRecord::new(test_character(3, "Gamma")),
        ),
    ]));
    desired.source_account_versions =
        BTreeMap::from([("alpha".to_string(), 9_001), ("gamma".to_string(), 9_002)]);
    desired.source_save_versions = BTreeMap::from([
        ("alpha".to_string(), BTreeMap::from([(1, 9_101)])),
        ("gamma".to_string(), BTreeMap::from([(3, 9_103)])),
    ]);
    desired.game_shop_global_purchases = BTreeMap::from([(8, 1)]);
    desired.source_game_shop_global_version = Some(9_201);
    desired.source_game_shop_global_purchases = BTreeMap::from([(99, 99)]);

    let plan = build_account_store_mutation_plan(
        &original,
        &desired,
        AccountStoreMutationScope::FullRestore,
        false,
    );
    let alpha = plan.accounts.get("alpha").unwrap();
    assert_eq!(alpha.expected_version, Some(11));
    assert_eq!(alpha.saves.get(&1).unwrap().expected_version, Some(101));
    assert_eq!(
        alpha.desired_account.as_ref().unwrap().password,
        "replacement-password"
    );
    let beta = plan.accounts.get("beta").unwrap();
    assert_eq!(beta.expected_version, Some(12));
    assert!(beta.desired_account.is_none());
    let gamma = plan.accounts.get("gamma").unwrap();
    assert_eq!(gamma.expected_version, None);
    assert_eq!(gamma.saves.get(&3).unwrap().expected_version, None);
    let global = plan.global_stock.as_ref().unwrap();
    assert_eq!(global.expected_version, Some(21));
    assert_eq!(global.expected_purchases, BTreeMap::from([(7, 4)]));
    let mut durable = fake_mutation_repository_from_store(&original);
    let versions = apply_plan_to_fake_repository_atomically(&mut durable, &plan, None).unwrap();
    apply_account_store_mutation_source_versions(&mut desired, &plan, versions);
    assert_eq!(durable.accounts.len(), 2);
    assert_eq!(
        durable.accounts.get("alpha").unwrap().password,
        "replacement-password"
    );
    assert_eq!(
        durable.accounts.get("gamma").unwrap().characters[0].name,
        "Gamma"
    );
    assert!(!durable.accounts.contains_key("beta"));
    assert_eq!(
        desired.source_account_versions,
        BTreeMap::from([("alpha".to_string(), 12), ("gamma".to_string(), 1)])
    );
    assert_eq!(
        desired.source_save_versions,
        BTreeMap::from([
            ("alpha".to_string(), BTreeMap::from([(1, 102)])),
            ("gamma".to_string(), BTreeMap::from([(3, 1)])),
        ])
    );
    assert_eq!(desired.source_game_shop_global_version, Some(22));
    assert_eq!(
        desired.source_game_shop_global_purchases,
        BTreeMap::from([(8, 1)])
    );
    assert_eq!(global.desired_purchases, BTreeMap::from([(8, 1)]));
}

#[test]
fn account_delete_cas_rejects_account_and_save_conflicts() {
    let original = versioned_two_account_store();
    let desired = authoritative_test_store(BTreeMap::new());
    let plan = build_account_store_mutation_plan(
        &original,
        &desired,
        AccountStoreMutationScope::FullRestore,
        false,
    );
    let alpha = plan.accounts.get("alpha").unwrap();
    assert!(validate_account_store_account_mutation(
        "alpha",
        alpha,
        Some(11),
        &BTreeMap::from([(1, 101)]),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .is_ok());
    assert!(validate_account_store_account_mutation(
        "alpha",
        alpha,
        Some(99),
        &BTreeMap::from([(1, 101)]),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap_err()
    .contains("expected store_version 11"));
    assert!(validate_account_store_account_mutation(
        "alpha",
        alpha,
        Some(11),
        &BTreeMap::from([(1, 999)]),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap_err()
    .contains("expected save_version 101"));
    assert!(validate_account_store_account_mutation(
        "alpha",
        alpha,
        Some(11),
        &BTreeMap::new(),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap_err()
    .contains("found no row"));
    assert!(validate_account_store_account_mutation(
        "alpha",
        alpha,
        Some(11),
        &BTreeMap::from([(1, 101), (77, 1)]),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap_err()
    .contains("expected no row for new save"));
}

#[test]
fn character_save_cas_has_all_four_presence_states() {
    assert!(validate_source_save_write(None, None).is_ok());
    assert!(validate_source_save_write(Some(7), Some(7)).is_ok());
    assert!(validate_source_save_write(Some(7), Some(8)).is_err());
    assert!(validate_source_save_write(Some(7), None)
        .unwrap_err()
        .contains("found no row"));
    assert!(validate_source_save_write(None, Some(8))
        .unwrap_err()
        .contains("expected no row"));
}

#[test]
fn global_stock_restore_cas_supports_some_none_and_explicit_reset() {
    let expected = BTreeMap::from([(7, 4)]);
    assert!(
        validate_source_game_shop_global_write(Some(21), Some(21), &expected, Some(&expected),)
            .is_ok()
    );
    assert!(validate_source_game_shop_global_write(Some(21), None, &expected, None).is_err());
    assert!(validate_source_game_shop_global_write(
        None,
        Some(21),
        &BTreeMap::new(),
        Some(&expected),
    )
    .is_err());
    assert!(validate_source_game_shop_global_write(None, None, &BTreeMap::new(), None,).is_ok());
    assert!(validate_source_game_shop_global_write(
        Some(21),
        Some(21),
        &expected,
        Some(&BTreeMap::from([(7, 5)])),
    )
    .unwrap_err()
    .contains("contents"));
}

#[test]
fn normal_scoped_create_keeps_expected_none_insert_semantics() {
    let original = authoritative_test_store(BTreeMap::new());
    let desired = authoritative_test_store(BTreeMap::from([(
        "new-account".to_string(),
        AccountRecord::new(test_character(5, "NewAccount")),
    )]));
    let account_ids = vec!["new-account".to_string()];
    let plan = build_account_store_mutation_plan(
        &original,
        &desired,
        AccountStoreMutationScope::Accounts(&account_ids),
        false,
    );
    let mutation = plan.accounts.get("new-account").unwrap();
    assert_eq!(mutation.expected_version, None);
    assert_eq!(mutation.saves.get(&5).unwrap().expected_version, None);
    assert!(validate_account_store_account_mutation(
        "new-account",
        mutation,
        None,
        &BTreeMap::new(),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .is_ok());
}

#[test]
fn multi_account_mid_fault_rolls_back_fake_repository_and_live_store() {
    let original = versioned_two_account_store();
    let mut desired = original.clone();
    desired.accounts.get_mut("alpha").unwrap().password = "alpha-new".to_string();
    desired.accounts.get_mut("beta").unwrap().password = "beta-new".to_string();
    let account_ids = vec!["alpha".to_string(), "beta".to_string()];
    let plan = build_account_store_mutation_plan(
        &original,
        &desired,
        AccountStoreMutationScope::Accounts(&account_ids),
        false,
    );
    let mut durable = fake_mutation_repository_from_store(&original);
    let durable_before = durable.clone();
    let durable_accounts_before = serde_json::to_value(&durable_before.accounts).unwrap();
    let error = apply_plan_to_fake_repository_atomically(&mut durable, &plan, Some(2)).unwrap_err();
    assert!(error.contains("beta"));
    assert_eq!(
        serde_json::to_value(&durable.accounts).unwrap(),
        durable_accounts_before
    );
    assert_eq!(&durable.account_versions, &durable_before.account_versions);
    assert_eq!(&durable.save_versions, &durable_before.save_versions);
    assert_eq!(durable.global_version, durable_before.global_version);
    assert_eq!(&durable.global_purchases, &durable_before.global_purchases);
    assert_eq!(
        durable.accounts.get("alpha").unwrap().password,
        original.accounts.get("alpha").unwrap().password
    );
    assert_eq!(
        durable.accounts.get("beta").unwrap().password,
        original.accounts.get("beta").unwrap().password
    );

    let config = source_of_truth_test_config_with_store(original);
    let live_before = account_store_audit_snapshot(&config);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    let error = config
        .commit_account_store_transaction(&account_ids, |store| {
            store.accounts.get_mut("alpha").unwrap().password = "alpha-new".to_string();
            store.accounts.get_mut("beta").unwrap().password = "beta-new".to_string();
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("persistence failure"));
    assert_eq!(account_store_audit_snapshot(&config), live_before);
}

#[test]
fn source_full_restore_nonempty_to_empty_clears_live_and_all_source_metadata() {
    let empty_backup = unique_test_path("third-round-empty-restore");
    authoritative_test_store(BTreeMap::new())
        .save_to_path(&empty_backup)
        .unwrap();
    let config = source_of_truth_test_config_with_store(versioned_two_account_store());

    config
        .restore_account_store_from_backup(&empty_backup)
        .expect("full source restore should accept an empty authoritative backup");
    let snapshot = account_store_audit_snapshot(&config);
    let decoded: AccountStore = serde_json::from_slice(&snapshot.json).unwrap();
    assert!(decoded.accounts.is_empty());
    assert!(decoded.game_shop_global_purchases.is_empty());
    assert!(snapshot.source_account_versions.is_empty());
    assert!(snapshot.source_save_versions.is_empty());
    assert_eq!(snapshot.source_game_shop_global_version, None);
    assert!(snapshot.source_game_shop_global_purchases.is_empty());

    let _ = std::fs::remove_file(empty_backup);
}

#[test]
fn retained_account_save_delete_is_a_versioned_tombstone() {
    let original = versioned_two_account_store();
    let mut desired = original.clone();
    desired.accounts.get_mut("alpha").unwrap().saves.remove(&1);
    let account_ids = vec!["alpha".to_string()];
    let plan = build_account_store_mutation_plan(
        &original,
        &desired,
        AccountStoreMutationScope::Accounts(&account_ids),
        false,
    );
    let alpha = plan.accounts.get("alpha").unwrap();
    let save = alpha.saves.get(&1).unwrap();
    assert_eq!(save.expected_version, Some(101));
    assert!(save.desired_save.is_none());
    assert!(validate_account_store_account_mutation(
        "alpha",
        alpha,
        Some(11),
        &BTreeMap::from([(1, 101)]),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .is_ok());
    assert!(validate_account_store_account_mutation(
        "alpha",
        alpha,
        Some(11),
        &BTreeMap::new(),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .is_err());

    let mut durable = fake_mutation_repository_from_store(&original);
    let versions = apply_plan_to_fake_repository_atomically(&mut durable, &plan, None).unwrap();
    apply_account_store_mutation_source_versions(&mut desired, &plan, versions);
    assert_eq!(durable.account_versions.get("alpha"), Some(&12));
    assert!(durable.accounts.get("alpha").unwrap().saves.is_empty());
    assert!(!durable.save_versions.contains_key("alpha"));
    assert!(!desired.source_save_versions.contains_key("alpha"));
}

#[test]
fn ordinary_account_scope_never_turns_into_full_store_deletion() {
    let original = versioned_two_account_store();
    let mut desired = original.clone();
    desired.accounts.remove("beta");
    desired.source_account_versions.remove("beta");
    desired.source_save_versions.remove("beta");
    let account_ids = vec!["alpha".to_string()];
    let plan = build_account_store_mutation_plan(
        &original,
        &desired,
        AccountStoreMutationScope::Accounts(&account_ids),
        false,
    );

    assert_eq!(plan.accounts.len(), 1);
    assert!(plan.accounts.contains_key("alpha"));
    assert!(!plan.accounts.contains_key("beta"));
}

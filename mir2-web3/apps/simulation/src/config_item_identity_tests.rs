use super::*;

struct Fixture {
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        Self {
            path: std::env::temp_dir()
                .join(format!(
                    "mir2-item-identity-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ))
                .join("accounts.json"),
        }
    }
    fn config(&self) -> SimulationConfig {
        SimulationConfig::default().with_account_store_path(&self.path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}

#[test]
fn item_identity_file_missing_corrupt_and_rebootstrap_preserve_source() {
    let fixture = Fixture::new();
    let config = fixture.config();
    assert!(load(&config).is_err());
    let blocked = IdentitySourceState {
        high_watermark: 0,
        version: 1,
        bootstrapped: false,
        census_sha256: None,
    };
    assert!(reserve_file(&config, &blocked, 1).is_err());
    bootstrap_file(&config, CompleteIdentityCensus::isolated_fixture(800)).unwrap();
    assert!(bootstrap_file(&config, CompleteIdentityCensus::isolated_fixture(0)).is_err());
    let identity_path = path(&config).unwrap();
    fs::write(&identity_path, b"{bad old authority").unwrap();
    assert!(load(&config).is_err());
    assert!(reserve_file(&config, &blocked, 1).is_err());
    assert_eq!(fs::read(identity_path).unwrap(), b"{bad old authority");
}

#[test]
fn item_identity_file_reopen_burns_unused_and_stale_snapshot_cannot_reuse() {
    let fixture = Fixture::new();
    let old;
    {
        let config = fixture.config();
        bootstrap_file(&config, CompleteIdentityCensus::isolated_fixture(100)).unwrap();
        old = load(&config).unwrap();
        let mut lease = reserve_file(&config, &old, 4).unwrap();
        assert_eq!(lease.take(1).unwrap(), [101]);
        assert!(reserve_file(&config, &old, 1).is_err());
    }
    let config = fixture.config();
    let current = load(&config).unwrap();
    assert_eq!(current.high_watermark, 104);
    assert!(reserve_file(&config, &old, 1).is_err());
    let mut lease = reserve_file(&config, &current, 2).unwrap();
    assert_eq!(lease.take(2).unwrap(), [105, 106]);
}

#[test]
fn item_identity_file_publication_failure_keeps_marker_after_release() {
    let fixture = Fixture::new();
    let state;
    {
        let config = fixture.config();
        bootstrap_file(&config, CompleteIdentityCensus::isolated_fixture(300)).unwrap();
        state = load(&config).unwrap();
        assert!(reserve(
            &config,
            &state,
            4,
            None,
            Some(AccountStoreFileCommitFault::BeforeRename)
        )
        .is_err());
        assert!(config.ensure_account_store_writable().is_err());
    }
    let reopened = fixture.config();
    assert!(reopened.ensure_account_store_writable().is_err());
    assert!(reserve_file(&reopened, &state, 1).is_err());
}

#[test]
fn item_identity_file_rejects_detached_authority_and_database_bypass() {
    let fixture = Fixture::new();
    let config = fixture.config();
    bootstrap_file(&config, CompleteIdentityCensus::isolated_fixture(400)).unwrap();
    let expected = load(&config).unwrap();
    let mut detached = config.clone();
    detached.account_store = Arc::new(Mutex::new(config.account_store.lock().unwrap().clone()));
    assert!(load(&detached).is_err());
    assert!(reserve_file(&detached, &expected, 1).is_err());
    let mut bypass = config.clone();
    bypass.account_store_database_url = Some("postgresql://must-not-connect/authority".into());
    assert!(reserve_file(&bypass, &expected, 1).is_err());
    assert_eq!(load(&config).unwrap(), expected);
}

#[test]
#[ignore = "requires explicit isolated PostgreSQL database"]
fn item_identity_mirror_pg_commit_file_failure_never_compensates_high() {
    let url = std::env::var("MIR2_ITEM_IDENTITY_TEST_DATABASE_URL")
        .expect("explicit test database required");
    let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    let schema = format!(
        "item_mirror_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    client
        .batch_execute(&format!(
            "CREATE SCHEMA {schema}; SET search_path TO {schema}"
        ))
        .unwrap();
    struct SchemaCleanup(String, String);
    impl Drop for SchemaCleanup {
        fn drop(&mut self) {
            if let Ok(mut c) = postgres::Client::connect(&self.0, postgres::NoTls) {
                let _ = c.batch_execute(&format!("DROP SCHEMA {} CASCADE", self.1));
            }
        }
    }
    let _cleanup = SchemaCleanup(url, schema);
    client
        .batch_execute(include_str!(
            "../../../infra/postgres/migrations/0015_shared_item_identity.sql"
        ))
        .unwrap();
    let pg_before = pg::load(&mut client).unwrap();
    pg::bootstrap(
        &mut client,
        &pg_before,
        CompleteIdentityCensus::isolated_fixture(600),
    )
    .unwrap();
    let fixture = Fixture::new();
    let before;
    {
        let config = fixture.config();
        bootstrap_mirror_with_client(
            &config,
            CompleteIdentityCensus::isolated_fixture(600),
            &mut client,
        )
        .unwrap();
        let initial = load(&config).unwrap();
        let mut lease = reserve(&config, &initial, 2, Some(&mut client), None).unwrap();
        assert_eq!(lease.take(1).unwrap(), [601]);
        drop(lease); // 602 remains burned
        before = load(&config).unwrap();
        assert_eq!(before.high_watermark, 602);
        assert!(reserve(
            &config,
            &before,
            4,
            Some(&mut client),
            Some(AccountStoreFileCommitFault::BeforeRename)
        )
        .is_err());
        assert!(config.ensure_account_store_writable().is_err());
    }
    assert_eq!(pg::load(&mut client).unwrap().high_watermark, 606);
    let reopened = fixture.config();
    assert!(load(&reopened).is_err());
    assert!(reserve(&reopened, &before, 1, Some(&mut client), None).is_err());
    assert_eq!(pg::load(&mut client).unwrap().high_watermark, 606);
}

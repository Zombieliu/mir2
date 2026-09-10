use super::*;
use postgres::NoTls;

const MIGRATION: &str =
    include_str!("../../../infra/postgres/migrations/0015_shared_item_identity.sql");

struct Database {
    client: Client,
    schema: String,
    url: String,
}
impl Database {
    fn new() -> Self {
        let url = std::env::var("MIR2_ITEM_IDENTITY_TEST_DATABASE_URL")
            .expect("explicit isolated item identity test database URL required");
        let schema = format!(
            "item_identity_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut client = Client::connect(&url, NoTls).unwrap();
        client
            .batch_execute(&format!(
                "CREATE SCHEMA {schema}; SET search_path TO {schema}"
            ))
            .unwrap();
        client.batch_execute(MIGRATION).unwrap();
        Self {
            client,
            schema,
            url,
        }
    }
    fn other(&self) -> Client {
        let mut c = Client::connect(&self.url, NoTls).unwrap();
        c.batch_execute(&format!("SET search_path TO {}", self.schema))
            .unwrap();
        c
    }
    fn initialize(&mut self, high: u64) -> IdentitySourceState {
        let expected = load(&mut self.client).unwrap();
        bootstrap(
            &mut self.client,
            &expected,
            CompleteIdentityCensus {
                high_watermark: high,
                sha256: "a".repeat(64),
            },
        )
        .unwrap()
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        if self.schema.starts_with("item_identity_")
            && self
                .schema
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            let _ = self
                .client
                .batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema));
        }
    }
}

#[test]
#[ignore = "requires explicit isolated PostgreSQL database"]
fn item_identity_postgres_bootstrap_gate_and_exact_source() {
    let mut db = Database::new();
    let blocked = load(&mut db.client).unwrap();
    assert!(!blocked.bootstrapped);
    assert!(matches!(
        reserve(&mut db.client, &blocked, 1),
        Err(IdentitySourceError::NotBootstrapped)
    ));
    let state = db.initialize(9_007_199_254_740_993);
    assert_eq!(state.high_watermark, 9_007_199_254_740_993);
    assert!(matches!(
        bootstrap(
            &mut db.client,
            &blocked,
            CompleteIdentityCensus {
                high_watermark: 1,
                sha256: "b".repeat(64),
            }
        ),
        Err(IdentitySourceError::Stale)
    ));
    assert_eq!(load(&mut db.client).unwrap(), state);
    let mut lease = reserve(&mut db.client, &state, 2).unwrap();
    assert_eq!(
        lease.take(2).unwrap(),
        [9_007_199_254_740_994, 9_007_199_254_740_995]
    );
    assert!(db
        .client
        .execute(
            "UPDATE shared_item_identity_allocator SET census_sha256=NULL",
            &[]
        )
        .is_err());
}

#[test]
#[ignore = "requires explicit isolated PostgreSQL database"]
fn item_identity_postgres_two_connections_race_and_stale_do_not_overlap() {
    let mut db = Database::new();
    db.initialize(100);
    let mut first = db.other();
    let mut second = db.other();
    let left = load(&mut first).unwrap();
    let right = load(&mut second).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let b = barrier.clone();
    let a = std::thread::spawn(move || {
        b.wait();
        reserve(&mut first, &left, 4)
    });
    let b = std::thread::spawn(move || {
        barrier.wait();
        reserve(&mut second, &right, 4)
    });
    let outcomes = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(outcomes.iter().filter(|x| x.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|x| matches!(x, Err(IdentitySourceError::Stale)))
            .count(),
        1
    );
    let mut winner = outcomes.into_iter().find_map(Result::ok).unwrap();
    assert_eq!(winner.take(4).unwrap(), [101, 102, 103, 104]);
    let state = load(&mut db.client).unwrap();
    let mut next = reserve(&mut db.client, &state, 2).unwrap();
    assert_eq!(next.take(2).unwrap(), [105, 106]);
}

#[test]
#[ignore = "requires explicit isolated PostgreSQL database"]
fn item_identity_postgres_rollback_and_lost_response_never_return_cursor() {
    let mut db = Database::new();
    let before = db.initialize(200);
    assert!(matches!(
        reserve_inner(&mut db.client, &before, 4, CommitFault::Rollback),
        Err(IdentitySourceError::Source(_))
    ));
    assert_eq!(load(&mut db.client).unwrap(), before);
    // Real database commits; the adapter's deterministic transport fault drops
    // its success result. This is not a claim of a real TCP disconnect test.
    assert!(matches!(
        reserve_inner(
            &mut db.client,
            &before,
            4,
            CommitFault::LoseCommittedResponse
        ),
        Err(IdentitySourceError::CommitOutcomeUnknown)
    ));
    assert!(matches!(
        reserve(&mut db.client, &before, 4),
        Err(IdentitySourceError::Stale)
    ));
    let after = load(&mut db.other()).unwrap();
    assert_eq!(after.high_watermark, 204);
    let mut new = reserve(&mut db.client, &after, 2).unwrap();
    assert_eq!(new.take(2).unwrap(), [205, 206]); // uncertain range permanently burned
}

#[test]
#[ignore = "requires explicit isolated PostgreSQL database"]
fn item_identity_postgres_unsigned_and_version_limits_fail_without_changes() {
    let mut db = Database::new();
    let state = db.initialize(u64::MAX - 1);
    let mut last = reserve(&mut db.client, &state, 1).unwrap();
    assert_eq!(last.take(1).unwrap(), [u64::MAX]);
    let exhausted = load(&mut db.client).unwrap();
    assert!(matches!(
        reserve(&mut db.client, &exhausted, 1),
        Err(IdentitySourceError::InvalidReservation)
    ));
    assert_eq!(load(&mut db.client).unwrap(), exhausted);
    assert!(db
        .client
        .execute(
            "UPDATE shared_item_identity_allocator SET high_watermark=18446744073709551616",
            &[]
        )
        .is_err());
    assert!(db
        .client
        .execute(
            "UPDATE shared_item_identity_allocator SET high_watermark=-1",
            &[]
        )
        .is_err());
    db.client
        .execute(
            "UPDATE shared_item_identity_allocator SET store_version=9223372036854775807",
            &[],
        )
        .unwrap();
    let version_end = load(&mut db.client).unwrap();
    assert!(matches!(
        reserve(&mut db.client, &version_end, 1),
        Err(IdentitySourceError::InvalidReservation)
    ));
    assert_eq!(load(&mut db.client).unwrap(), version_end);
}

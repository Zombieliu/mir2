//! Real PostgreSQL source/checkpoint + operation/outbox acceptance. Explicitly
//! ignored without a private loopback acceptance database; no service is started.
#![cfg(feature = "test-support")]

use mir2_economy_policy::{Policy, Rational};
use mir2_economy_service::*;
use postgres::{Client, NoTls, Transaction};
use serde_json::{json, Value};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const KIND: &str = "npc_repair";
const PLAN: &str = "server-original-plan";

fn pg(error: postgres::Error) -> EconomyError {
    EconomyError::Database {
        sqlstate: error.code().map(|code| code.code().to_owned()),
    }
}
fn policy() -> Policy {
    Policy {
        schema_version: 1,
        policy_version: "zero-source-acceptance-v1".into(),
        enabled: true,
        k: Rational {
            numerator: 1,
            denominator: 10,
        },
        conversion_fee: Rational {
            numerator: 1,
            denominator: 50,
        },
        daily_decay: Rational {
            numerator: 1,
            denominator: 150,
        },
        baseline_daily_mint_atoms: 720_000,
        daily_mint_cap_multiplier: Rational {
            numerator: 3,
            denominator: 2,
        },
        business_timezone: "Asia/Shanghai".into(),
        settlement_hour: 4,
    }
}
fn who(account: &str, uuid: &str) -> WalletOwner {
    WalletOwner {
        account_id: account.into(),
        character_index: 0,
        incarnation: format!("pg-character-v1:{uuid}"),
    }
}
fn burn(key: &str) -> TrustedBurn {
    TrustedBurn {
        source_kind: KIND.into(),
        source_key: key.into(),
        layer: BurnLayer::Daily,
        gold_equivalent: 3,
        valuation_version: BOUND_GOLD_FACE_VALUE_VALUATION_VERSION.into(),
    }
}
fn authorize<'a>(
    who: &'a WalletOwner,
) -> impl FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner> + 'a {
    move |tx, _| {
        let row = tx.query_opt("SELECT allowed,banned FROM zero_test_actors WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 FOR UPDATE",
            &[&who.account_id,&who.character_index,&who.incarnation]).map_err(pg)?
            .ok_or(EconomyError::CharacterNotFound)?;
        if !row.get::<_, bool>("allowed") {
            return Err(EconomyError::Unauthenticated);
        }
        if row.get::<_, bool>("banned") {
            return Err(EconomyError::AccountUnavailable);
        }
        tx.query("SELECT source_id FROM zero_test_sources WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 ORDER BY source_id FOR UPDATE",
            &[&who.account_id,&who.character_index,&who.incarnation]).map_err(pg)?;
        Ok(who.clone())
    }
}
fn source<'a>(
    who: &'a WalletOwner,
    asset: &'a str,
) -> impl FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()> + 'a {
    move |tx, clock| {
        let checkpoint = json!({"item":asset,"durability_current":100,"durability_max":100,"legacy_gold":42,"other_inventory":["unchanged"]});
        let changed = tx.execute("UPDATE zero_test_sources SET revision=1,checkpoint=$5,business_day=$6,committed_at_ms=$7 WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 AND source_id=$4 AND revision=0",
            &[&who.account_id,&who.character_index,&who.incarnation,&asset,&checkpoint,&clock.business_day,&clock.database_time_ms]).map_err(pg)?;
        if changed != 1 {
            return Err(EconomyError::SourceRejected);
        }
        Ok(())
    }
}

struct Fixture {
    client: Client,
    connection: postgres::Config,
    realm: String,
    owner: WalletOwner,
}
impl Fixture {
    fn new(label: &str) -> Self {
        let raw = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
            .expect("ignored zero source acceptance requires MIR2_ECONOMY_TEST_DATABASE_URL");
        let mut connection: postgres::Config = raw
            .parse()
            .unwrap_or_else(|_| panic!("invalid isolated acceptance configuration"));
        assert!(connection
            .get_dbname()
            .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
        assert!(!connection.get_hosts().is_empty() && connection.get_hosts().iter().all(|host|
            matches!(host,postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()))));
        assert!(connection
            .get_hostaddrs()
            .iter()
            .all(std::net::IpAddr::is_loopback));
        let mut bootstrap = connection
            .connect(NoTls)
            .unwrap_or_else(|_| panic!("isolated acceptance connection failed"));
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
        let schema = format!("zero_test_{nonce}");
        bootstrap
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        connection.options(&format!("-c search_path={schema}"));
        let mut client = connection
            .connect(NoTls)
            .unwrap_or_else(|_| panic!("isolated schema connection failed"));
        assert_eq!(
            client
                .query_one("SELECT current_schema()", &[])
                .unwrap()
                .get::<_, String>(0),
            schema
        );
        client.batch_execute(MIGRATION_SQL).unwrap();
        client
            .batch_execute(BOUND_DEBIT_BURN_MIGRATION_SQL)
            .unwrap();
        client.batch_execute(ZERO_FEE_SOURCE_MIGRATION_SQL).unwrap();
        client.batch_execute(ZERO_FEE_SOURCE_MIGRATION_SQL).unwrap();
        client.batch_execute("CREATE TABLE zero_test_actors(account_id TEXT NOT NULL,character_index INTEGER NOT NULL,incarnation TEXT NOT NULL,allowed BOOLEAN NOT NULL DEFAULT true,banned BOOLEAN NOT NULL DEFAULT false,PRIMARY KEY(account_id,character_index,incarnation));
            CREATE TABLE zero_test_sources(account_id TEXT NOT NULL,character_index INTEGER NOT NULL,incarnation TEXT NOT NULL,source_id TEXT NOT NULL,revision BIGINT NOT NULL DEFAULT 0,checkpoint JSONB NOT NULL,business_day BIGINT,committed_at_ms BIGINT,PRIMARY KEY(account_id,character_index,incarnation,source_id));").unwrap();
        let realm = format!("zero-source-test-{label}-{nonce}");
        initialize_realm(&mut client, &realm, &policy()).unwrap();
        let owner = who("zero-player", "00000000-0000-4000-8000-000000000001");
        let mut result = Self {
            client,
            connection,
            realm,
            owner: owner.clone(),
        };
        result.add_owner(&owner);
        result
    }
    fn connect(&self) -> Client {
        self.connection
            .connect(NoTls)
            .unwrap_or_else(|_| panic!("isolated worker connection failed"))
    }
    fn add_owner(&mut self, who: &WalletOwner) {
        self.client.execute("INSERT INTO zero_test_actors(account_id,character_index,incarnation) VALUES($1,$2,$3)",
            &[&who.account_id,&who.character_index,&who.incarnation]).unwrap();
        for asset in ["asset-a", "asset-b", "asset-c"] {
            self.client.execute("INSERT INTO zero_test_sources(account_id,character_index,incarnation,source_id,checkpoint) VALUES($1,$2,$3,$4,$5)",
                &[&who.account_id,&who.character_index,&who.incarnation,&asset,&json!({"item":asset,"durability_current":50,"durability_max":100,"legacy_gold":42,"other_inventory":["unchanged"]})]).unwrap();
        }
    }
    fn record(
        &mut self,
        key: &str,
        asset: &str,
    ) -> EconomyResult<Idempotent<ZeroFeeSourceReceipt>> {
        record_zero_fee_source_authorized_with_source_with_timeouts(
            &mut self.client,
            &self.realm,
            KIND,
            key,
            PLAN,
            2000,
            10000,
            authorize(&self.owner),
            source(&self.owner, asset),
        )
    }
    fn lookup(&mut self, key: &str) -> EconomyResult<Option<ZeroFeeSourceReceipt>> {
        lookup_zero_fee_source_authorized_with_timeouts(
            &mut self.client,
            &self.realm,
            KIND,
            key,
            PLAN,
            2000,
            10000,
            authorize(&self.owner),
        )
    }
    // Complete source changes, durable wallets, burns, zero operations, zero outbox.
    fn state(&mut self) -> (i64, i64, i64, i64, i64) {
        let row = self.client.query_one("SELECT
            (SELECT count(*) FROM zero_test_sources WHERE revision=1),
            (SELECT count(*) FROM hunting_economy_wallets WHERE realm_id=$1),
            (SELECT count(*) FROM hunting_economy_burn_events WHERE realm_id=$1),
            (SELECT count(*) FROM hunting_economy_operations WHERE realm_id=$1 AND operation_kind='zero_fee_source'),
            (SELECT count(*) FROM hunting_economy_outbox WHERE realm_id=$1 AND event_kind='zero_fee_source')",&[&self.realm]).unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4))
    }
    fn session_timeouts(&mut self) -> (String, String) {
        let row = self
            .client
            .query_one(
                "SELECT current_setting('lock_timeout'),current_setting('statement_timeout')",
                &[],
            )
            .unwrap();
        (row.get(0), row.get(1))
    }
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_zero_fee_source_atomic_without_wallet_or_burn_and_immutable_replay() {
    let mut f = Fixture::new("atomic");
    let pool = read_realm(&mut f.client, &f.realm).unwrap();
    // Holding exclusive relation locks proves this path does not even read the
    // wallet/burn tables. A hidden read or provisioning attempt would time out.
    let mut blocker = f.connect();
    let mut held = blocker.transaction().unwrap();
    held.batch_execute(
        "LOCK TABLE hunting_economy_wallets,hunting_economy_burn_events IN ACCESS EXCLUSIVE MODE",
    )
    .unwrap();
    let receipt = record_zero_fee_source_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        KIND,
        "quote-a",
        PLAN,
        50,
        1000,
        authorize(&f.owner),
        source(&f.owner, "asset-a"),
    )
    .unwrap();
    held.rollback().unwrap();
    assert!(!receipt.replayed);
    assert_eq!(receipt.receipt.owner, f.owner);
    assert_eq!(receipt.receipt.source_key, "quote-a");
    assert_eq!(receipt.receipt.source_kind, KIND);
    assert_eq!(receipt.receipt.policy_version, policy().policy_version);
    assert_eq!(f.state(), (1, 0, 0, 1, 1));
    assert_eq!(read_realm(&mut f.client, &f.realm).unwrap(), pool);
    let row = f.client.query_one("SELECT checkpoint,business_day,committed_at_ms FROM zero_test_sources WHERE source_id='asset-a'",&[]).unwrap();
    assert_eq!(
        row.get::<_, Value>("checkpoint"),
        json!({"item":"asset-a","durability_current":100,"durability_max":100,"legacy_gold":42,"other_inventory":["unchanged"]})
    );
    assert_eq!(
        row.get::<_, i64>("business_day"),
        receipt.receipt.business_day
    );
    assert_eq!(
        row.get::<_, i64>("committed_at_ms"),
        receipt.receipt.committed_at_ms
    );
    let json = serde_json::to_value(&receipt.receipt).unwrap();
    assert!(json.get("wallet_after").is_none() && json.get("burn_event_id").is_none());
    let row = f.client.query_one("SELECT payload_json FROM hunting_economy_outbox WHERE realm_id=$1 AND event_kind='zero_fee_source'",&[&f.realm]).unwrap();
    assert_eq!(row.get::<_, Value>(0), json);
    let replay = record_zero_fee_source_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        KIND,
        "quote-a",
        PLAN,
        2000,
        10000,
        authorize(&f.owner),
        |_, _| panic!("replay cannot repair twice"),
    )
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt, receipt.receipt);
    assert_eq!(
        lookup_zero_fee_source_authorized_with_timeouts(
            &mut f.client,
            &f.realm,
            KIND,
            "quote-a",
            "changed-plan",
            2000,
            10000,
            authorize(&f.owner)
        ),
        Err(EconomyError::RequestConflict)
    );
    assert_eq!(
        record_zero_fee_source_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            KIND,
            "quote-a",
            "changed-plan",
            2000,
            10000,
            authorize(&f.owner),
            |_, _| panic!("conflict cannot mutate source")
        ),
        Err(EconomyError::RequestConflict)
    );
    // 0023 can be reapplied over existing receipts without changing history.
    f.client
        .batch_execute(ZERO_FEE_SOURCE_MIGRATION_SQL)
        .unwrap();
    assert_eq!(f.lookup("quote-a").unwrap(), Some(receipt.receipt));
    let mut changed = policy();
    changed.policy_version = "zero-numeric-change".into();
    changed.baseline_daily_mint_atoms += 1;
    register_policy(&mut f.client, &f.realm, &changed).unwrap();
    assert_eq!(
        activate_policy(&mut f.client, &f.realm, &changed.policy_version, true),
        Err(EconomyError::InvalidInput(
            "policy_change_after_daily_activity"
        ))
    );
    assert_eq!(f.state(), (1, 0, 0, 1, 1));
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_zero_fee_source_rollbacks_and_unknown_recover_original_slot() {
    let mut f = Fixture::new("faults");
    assert_eq!(
        record_zero_fee_source_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            KIND,
            "quote-a",
            PLAN,
            2000,
            10000,
            authorize(&f.owner),
            |tx, clock| {
                source(&f.owner, "asset-a")(tx, clock)?;
                Err(EconomyError::SourceRejected)
            }
        ),
        Err(EconomyError::SourceRejected)
    );
    assert_eq!(f.state(), (0, 0, 0, 0, 0));
    assert_eq!(
        record_zero_fee_source_authorized_with_source_with_test_fault(
            &mut f.client,
            &f.realm,
            KIND,
            "quote-a",
            PLAN,
            2000,
            10000,
            TestFault::BeforeCommit,
            authorize(&f.owner),
            source(&f.owner, "asset-a")
        ),
        Err(EconomyError::InjectedFailure)
    );
    assert_eq!(f.state(), (0, 0, 0, 0, 0));
    assert!(f.lookup("quote-a").unwrap().is_none());
    assert_eq!(
        record_zero_fee_source_authorized_with_source_with_test_fault(
            &mut f.client,
            &f.realm,
            KIND,
            "quote-a",
            PLAN,
            2000,
            10000,
            TestFault::CommitAcknowledgementLost,
            authorize(&f.owner),
            source(&f.owner, "asset-a")
        ),
        Err(EconomyError::OutcomeUnknown)
    );
    assert_eq!(f.state(), (1, 0, 0, 1, 1));
    let original = f
        .lookup("quote-a")
        .unwrap()
        .expect("lost acknowledgement still committed");
    let replay = record_zero_fee_source_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        KIND,
        "quote-a",
        PLAN,
        2000,
        10000,
        authorize(&f.owner),
        |_, _| panic!("unknown recovery cannot rerun source"),
    )
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt, original);
    assert_eq!(f.state(), (1, 0, 0, 1, 1));
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_zero_fee_source_shares_slot_with_positive_debit_in_both_directions() {
    let mut f = Fixture::new("shared-slot");
    let zero = f.record("quote-zero", "asset-a").unwrap();
    let positive_id =
        bound_debit_burn_operation_identity(&f.realm, &f.owner, &burn("quote-zero"), PLAN).unwrap();
    assert_eq!(positive_id.0, zero.receipt.operation_key);
    assert_eq!(
        debit_bound_and_burn_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            &burn("quote-zero"),
            PLAN,
            2000,
            10000,
            authorize(&f.owner),
            |_, _| panic!("cannot charge an already free quote")
        ),
        Err(EconomyError::RequestConflict)
    );
    assert_eq!(
        lookup_bound_debit_burn_authorized_with_timeouts(
            &mut f.client,
            &f.realm,
            &burn("quote-zero"),
            PLAN,
            2000,
            10000,
            authorize(&f.owner)
        ),
        Err(EconomyError::RequestConflict)
    );
    assert_eq!(
        f.state(),
        (1, 0, 0, 1, 1),
        "conflict must precede missing wallet/balance handling"
    );
    credit_bound_with_source(
        &mut f.client,
        &f.realm,
        &f.owner,
        "fixture-trusted-production",
        "initial",
        10,
        |_, _| Ok(()),
    )
    .unwrap();
    let positive = debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn("quote-positive"),
        PLAN,
        2000,
        10000,
        authorize(&f.owner),
        source(&f.owner, "asset-b"),
    )
    .unwrap();
    assert_eq!(positive.receipt.wallet_after.bound_gold, 7);
    assert_eq!(
        record_zero_fee_source_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            KIND,
            "quote-positive",
            PLAN,
            2000,
            10000,
            authorize(&f.owner),
            |_, _| panic!("cannot free an already charged quote")
        ),
        Err(EconomyError::RequestConflict)
    );
    assert_eq!(
        f.lookup("quote-positive"),
        Err(EconomyError::RequestConflict)
    );
    f.client
        .batch_execute(ZERO_FEE_SOURCE_MIGRATION_SQL)
        .unwrap();
    assert_eq!(
        lookup_bound_debit_burn_authorized_with_timeouts(
            &mut f.client,
            &f.realm,
            &burn("quote-positive"),
            PLAN,
            2000,
            10000,
            authorize(&f.owner)
        )
        .unwrap(),
        Some(positive.receipt)
    );
    assert_eq!(f.lookup("quote-zero").unwrap(), Some(zero.receipt));
    assert_eq!(f.state(), (2, 1, 1, 1, 1));
    assert_eq!(
        read_wallet(&mut f.client, &f.realm, &f.owner)
            .unwrap()
            .unwrap()
            .bound_gold,
        7
    );
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_zero_fee_source_authorization_precedes_receipts_and_disabled_gap_recovery() {
    let mut f = Fixture::new("authorization");
    let first = f.record("quote-a", "asset-a").unwrap();
    for (column, value, expected) in [
        ("allowed", false, EconomyError::Unauthenticated),
        ("banned", true, EconomyError::AccountUnavailable),
    ] {
        f.client
            .execute(
                &format!("UPDATE zero_test_actors SET {column}=$1"),
                &[&value],
            )
            .unwrap();
        assert_eq!(
            record_zero_fee_source_authorized_with_source_with_timeouts(
                &mut f.client,
                &f.realm,
                KIND,
                "quote-a",
                PLAN,
                2000,
                10000,
                authorize(&f.owner),
                |_, _| panic!("unauthorized replay cannot reach source")
            ),
            Err(expected.clone())
        );
        assert_eq!(f.lookup("quote-a"), Err(expected));
        f.client
            .execute(
                &format!("UPDATE zero_test_actors SET {column}=$1"),
                &[&!value],
            )
            .unwrap();
    }
    let mut disabled = policy();
    disabled.policy_version = "zero-source-disabled".into();
    disabled.enabled = false;
    register_policy(&mut f.client, &f.realm, &disabled).unwrap();
    activate_policy(&mut f.client, &f.realm, &disabled.policy_version, true).unwrap();
    assert_eq!(
        f.record("fresh-disabled", "asset-b"),
        Err(EconomyError::Disabled)
    );
    let replay = record_zero_fee_source_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        KIND,
        "quote-a",
        PLAN,
        2000,
        10000,
        authorize(&f.owner),
        |_, _| panic!("disabled replay cannot mutate source"),
    )
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt, first.receipt);
    assert_eq!(f.lookup("quote-a").unwrap(), Some(first.receipt.clone()));
    activate_policy(&mut f.client, &f.realm, &policy().policy_version, true).unwrap();
    let today = read_realm(&mut f.client, &f.realm).unwrap().genesis_day;
    f.client.execute("UPDATE hunting_economy_pools SET genesis_day=$2,settled_through_day=$3 WHERE realm_id=$1",&[&f.realm,&(today-1),&(today-2)]).unwrap();
    assert!(matches!(
        f.record("fresh-gap", "asset-b"),
        Err(EconomyError::SettlementRequired { .. })
    ));
    assert_eq!(f.lookup("quote-a").unwrap(), Some(first.receipt.clone()));
    let replay = record_zero_fee_source_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        KIND,
        "quote-a",
        PLAN,
        2000,
        10000,
        authorize(&f.owner),
        |_, _| panic!("gap replay cannot mutate source"),
    )
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt, first.receipt);
    assert!(f.lookup("never-committed").unwrap().is_none());
    assert_eq!(f.state(), (1, 0, 0, 1, 1));
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_zero_fee_source_owner_and_recreated_character_are_isolated() {
    let mut f = Fixture::new("owners");
    let original = f.record("same-quote-id", "asset-a").unwrap();
    for other in [
        who("other-player", "00000000-0000-4000-8000-000000000002"),
        who("zero-player", "00000000-0000-4000-8000-000000000003"),
        WalletOwner {
            character_index: 1,
            ..f.owner.clone()
        },
    ] {
        f.add_owner(&other);
        assert!(lookup_zero_fee_source_authorized_with_timeouts(
            &mut f.client,
            &f.realm,
            KIND,
            "same-quote-id",
            PLAN,
            2000,
            10000,
            authorize(&other)
        )
        .unwrap()
        .is_none());
        let receipt = record_zero_fee_source_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            KIND,
            "same-quote-id",
            PLAN,
            2000,
            10000,
            authorize(&other),
            source(&other, "asset-a"),
        )
        .unwrap();
        assert!(!receipt.replayed);
        assert_ne!(
            receipt.receipt.operation_key,
            original.receipt.operation_key
        );
        assert_eq!(receipt.receipt.owner, other);
    }
    let missing = who("zero-player", "00000000-0000-4000-8000-000000000099");
    assert_eq!(
        lookup_zero_fee_source_authorized_with_timeouts(
            &mut f.client,
            &f.realm,
            KIND,
            "same-quote-id",
            PLAN,
            2000,
            10000,
            authorize(&missing)
        ),
        Err(EconomyError::CharacterNotFound)
    );
    assert_eq!(f.state(), (4, 0, 0, 4, 4));
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_zero_fee_source_timeouts_are_local_and_allow_original_retry() {
    let mut f = Fixture::new("timeouts");
    let before = f.session_timeouts();
    for (lock, statement) in [
        (0, 100),
        (100, 0),
        (101, 100),
        (1, 30_001),
        (30_001, 30_001),
    ] {
        assert_eq!(
            record_zero_fee_source_authorized_with_source_with_timeouts(
                &mut f.client,
                &f.realm,
                KIND,
                "quote-a",
                PLAN,
                lock,
                statement,
                |_, _| panic!("invalid timeout cannot authorize"),
                |_, _| panic!("invalid timeout cannot mutate source")
            ),
            Err(EconomyError::InvalidInput("transaction_timeouts"))
        );
        assert_eq!(
            lookup_zero_fee_source_authorized_with_timeouts(
                &mut f.client,
                &f.realm,
                KIND,
                "quote-a",
                PLAN,
                lock,
                statement,
                |_, _| panic!("invalid lookup timeout cannot authorize")
            ),
            Err(EconomyError::InvalidInput("transaction_timeouts"))
        );
    }
    let mut blocker = f.connect();
    let mut held = blocker.transaction().unwrap();
    lock_realm_and_read_database_clock(&mut held, &f.realm).unwrap();
    let result = record_zero_fee_source_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        KIND,
        "quote-a",
        PLAN,
        30,
        200,
        authorize(&f.owner),
        |_, _| panic!("realm timeout cannot reach source"),
    );
    assert!(
        matches!(result,Err(EconomyError::Database{sqlstate:Some(ref state)}) if state=="55P03" || state=="57014")
    );
    let result = lookup_zero_fee_source_authorized_with_timeouts(
        &mut f.client,
        &f.realm,
        KIND,
        "quote-a",
        PLAN,
        30,
        200,
        authorize(&f.owner),
    );
    assert!(
        matches!(result,Err(EconomyError::Database{sqlstate:Some(ref state)}) if state=="55P03" || state=="57014")
    );
    held.rollback().unwrap();
    assert_eq!(f.session_timeouts(), before);
    let result = record_zero_fee_source_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        KIND,
        "quote-a",
        PLAN,
        20,
        50,
        authorize(&f.owner),
        |tx, clock| {
            source(&f.owner, "asset-a")(tx, clock)?;
            tx.query_one("SELECT pg_sleep(0.2)", &[]).map_err(pg)?;
            Ok(())
        },
    );
    assert!(
        matches!(result,Err(EconomyError::Database{sqlstate:Some(ref state)}) if state=="57014")
    );
    assert_eq!(f.session_timeouts(), before);
    assert_eq!(f.state(), (0, 0, 0, 0, 0));
    assert!(f.lookup("quote-a").unwrap().is_none());
    f.record("quote-a", "asset-a").unwrap();
    assert_eq!(f.state(), (1, 0, 0, 1, 1));
    assert_eq!(f.session_timeouts(), before);
}

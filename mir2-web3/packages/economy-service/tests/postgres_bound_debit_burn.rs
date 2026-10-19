//! Actual source checkpoint + wallet + burn acceptance. No external service is
//! started; every test requires an explicit loopback acceptance DB and owns a
//! private schema. Histories remain available after success or failure.
#![cfg(feature = "test-support")]

use mir2_economy_policy::{Policy, Rational};
use mir2_economy_service::*;
use postgres::{Client, NoTls, Transaction};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Barrier,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

fn pg(error: postgres::Error) -> EconomyError {
    EconomyError::Database {
        sqlstate: error.code().map(|code| code.code().to_owned()),
    }
}
fn policy() -> Policy {
    Policy {
        schema_version: 1,
        policy_version: "debit-burn-acceptance-v1".into(),
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
        baseline_daily_mint_atoms: 72_000_000,
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
fn burn(key: &str, amount: u64) -> TrustedBurn {
    TrustedBurn {
        source_kind: "npc_repair".into(),
        source_key: key.into(),
        layer: BurnLayer::Daily,
        gold_equivalent: amount,
        valuation_version: BOUND_GOLD_FACE_VALUE_VALUATION_VERSION.into(),
    }
}
fn authorize<'a>(
    who: &'a WalletOwner,
) -> impl FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner> + 'a {
    move |tx, _| {
        let row = tx.query_opt("SELECT allowed,banned FROM debit_test_actors WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 FOR UPDATE",
            &[&who.account_id, &who.character_index, &who.incarnation]).map_err(pg)?
            .ok_or(EconomyError::CharacterNotFound)?;
        if !row.get::<_, bool>("allowed") {
            return Err(EconomyError::Unauthenticated);
        }
        if row.get::<_, bool>("banned") {
            return Err(EconomyError::AccountUnavailable);
        }
        // Source rows are locked before the adapter locks this owner's wallet.
        tx.query("SELECT source_id FROM debit_test_sources WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 ORDER BY source_id FOR UPDATE",
            &[&who.account_id, &who.character_index, &who.incarnation]).map_err(pg)?;
        Ok(who.clone())
    }
}
fn source<'a>(
    who: &'a WalletOwner,
    asset: &'a str,
) -> impl FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()> + 'a {
    move |tx, clock| {
        let checkpoint =
            json!({"item":asset,"durability_current":100,"durability_max":100,"legacy_gold":42});
        let changed = tx.execute("UPDATE debit_test_sources SET revision=1,checkpoint=$5,business_day=$6,committed_at_ms=$7 WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 AND source_id=$4 AND revision=0",
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
    fn new(label: &str, balance: u64) -> Self {
        let raw = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
            .expect("ignored debit acceptance requires MIR2_ECONOMY_TEST_DATABASE_URL");
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
        let schema = format!("debit_test_{nonce}");
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
        client
            .batch_execute(BOUND_DEBIT_BURN_MIGRATION_SQL)
            .unwrap();
        client.batch_execute("CREATE TABLE debit_test_actors(account_id TEXT NOT NULL,character_index INTEGER NOT NULL,incarnation TEXT NOT NULL,allowed BOOLEAN NOT NULL DEFAULT true,banned BOOLEAN NOT NULL DEFAULT false,PRIMARY KEY(account_id,character_index,incarnation));
            CREATE TABLE debit_test_sources(account_id TEXT NOT NULL,character_index INTEGER NOT NULL,incarnation TEXT NOT NULL,source_id TEXT NOT NULL,revision BIGINT NOT NULL DEFAULT 0,checkpoint JSONB NOT NULL,business_day BIGINT,committed_at_ms BIGINT,PRIMARY KEY(account_id,character_index,incarnation,source_id));").unwrap();
        let realm = format!("bound-debit-test-{label}-{nonce}");
        initialize_realm(&mut client, &realm, &policy()).unwrap();
        let owner = who("debit-player", "00000000-0000-4000-8000-000000000001");
        let mut result = Self {
            client,
            connection,
            realm,
            owner: owner.clone(),
        };
        result.add_owner(&owner, balance);
        result
    }
    fn connect(&self) -> Client {
        self.connection
            .connect(NoTls)
            .unwrap_or_else(|_| panic!("isolated worker connection failed"))
    }
    fn add_owner(&mut self, who: &WalletOwner, balance: u64) {
        self.client.execute("INSERT INTO debit_test_actors(account_id,character_index,incarnation) VALUES($1,$2,$3)",
            &[&who.account_id,&who.character_index,&who.incarnation]).unwrap();
        for asset in ["asset-a", "asset-b", "asset-c"] {
            self.client.execute("INSERT INTO debit_test_sources(account_id,character_index,incarnation,source_id,checkpoint) VALUES($1,$2,$3,$4,$5)",
                &[&who.account_id,&who.character_index,&who.incarnation,&asset,&json!({"item":asset,"durability_current":50,"durability_max":100,"legacy_gold":42})]).unwrap();
        }
        if balance > 0 {
            credit_bound_with_source(
                &mut self.client,
                &self.realm,
                who,
                "fixture-trusted-production",
                "original-production",
                balance,
                |_, _| Ok(()),
            )
            .unwrap();
        }
    }
    fn state(&mut self) -> (i64, i64, i64, i64, i64) {
        let row = self.client.query_one("SELECT
            (SELECT count(*) FROM debit_test_sources WHERE revision=1),
            (SELECT count(*) FROM hunting_economy_burn_events WHERE realm_id=$1),
            (SELECT COALESCE(sum(gold_equivalent),0)::bigint FROM hunting_economy_burn_events WHERE realm_id=$1),
            (SELECT count(*) FROM hunting_economy_operations WHERE realm_id=$1 AND operation_kind='bound_debit_burn'),
            (SELECT count(*) FROM hunting_economy_outbox WHERE realm_id=$1 AND event_kind='bound_debit_burn')", &[&self.realm]).unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4))
    }
    fn bound(&mut self) -> u64 {
        read_wallet(&mut self.client, &self.realm, &self.owner)
            .unwrap()
            .unwrap()
            .bound_gold
    }
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_bound_debit_burn_atomic_source_replay_and_digest_conflict() {
    let mut f = Fixture::new("atomic", 100);
    let burn = burn("original-operation", 30);
    let receipt = debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "original-plan",
        2000,
        10000,
        authorize(&f.owner),
        source(&f.owner, "asset-a"),
    )
    .unwrap();
    assert!(!receipt.replayed);
    assert_eq!(receipt.receipt.bound_gold_debit, 30);
    assert_eq!(receipt.receipt.source_key, burn.source_key);
    assert_eq!(receipt.receipt.source_kind, burn.source_kind);
    assert_eq!(receipt.receipt.wallet_after.bound_gold, 70);
    assert_eq!(receipt.receipt.wallet_after.unbound_gold, 0);
    assert_eq!(f.state(), (1, 1, 30, 1, 1));
    let row = f.client.query_one("SELECT checkpoint,business_day,committed_at_ms FROM debit_test_sources WHERE source_id='asset-a'", &[]).unwrap();
    assert_eq!(
        row.get::<_, Value>("checkpoint"),
        json!({"item":"asset-a","durability_current":100,"durability_max":100,"legacy_gold":42})
    );
    assert_eq!(
        row.get::<_, i64>("business_day"),
        receipt.receipt.business_day
    );
    assert_eq!(
        row.get::<_, i64>("committed_at_ms"),
        receipt.receipt.committed_at_ms
    );
    let row = f.client.query_one("SELECT source_kind,source_key,layer,gold_equivalent,valuation_version,business_day,committed_at_ms FROM hunting_economy_burn_events WHERE realm_id=$1 AND event_id=$2",
        &[&f.realm,&receipt.receipt.burn_event_id]).unwrap();
    assert_eq!(row.get::<_, String>("source_kind"), burn.source_kind);
    assert_ne!(row.get::<_, String>("source_key"), burn.source_key);
    assert_eq!(row.get::<_, String>("layer"), "daily");
    assert_eq!(row.get::<_, i64>("gold_equivalent"), 30);
    assert_eq!(
        row.get::<_, String>("valuation_version"),
        BOUND_GOLD_FACE_VALUE_VALUATION_VERSION
    );
    assert_eq!(
        row.get::<_, i64>("business_day"),
        receipt.receipt.business_day
    );
    assert_eq!(
        row.get::<_, i64>("committed_at_ms"),
        receipt.receipt.committed_at_ms
    );
    let replay = debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "original-plan",
        2000,
        10000,
        authorize(&f.owner),
        |_, _| panic!("a replay cannot repair twice"),
    )
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt, receipt.receipt);
    for (changed, digest) in [
        (burn.clone(), "changed-plan"),
        (
            TrustedBurn {
                gold_equivalent: 31,
                ..burn.clone()
            },
            "original-plan",
        ),
    ] {
        assert_eq!(
            debit_bound_and_burn_authorized_with_source_with_timeouts(
                &mut f.client,
                &f.realm,
                &changed,
                digest,
                2000,
                10000,
                authorize(&f.owner),
                |_, _| panic!("conflict must not mutate source")
            ),
            Err(EconomyError::RequestConflict)
        );
    }
    assert_eq!(f.bound(), 70);
    assert_eq!(f.state(), (1, 1, 30, 1, 1));
    assert_eq!(
        read_realm(&mut f.client, &f.realm)
            .unwrap()
            .pool
            .stock_atoms,
        0,
        "repair burns enter daily accounting; no immediate permits are injected"
    );
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_bound_debit_burn_source_failure_commit_fault_and_original_lookup() {
    let mut f = Fixture::new("faults", 100);
    let burn = burn("original-operation", 30);
    let result = debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
        2000,
        10000,
        authorize(&f.owner),
        |tx, clock| {
            source(&f.owner, "asset-a")(tx, clock)?;
            Err(EconomyError::SourceRejected)
        },
    );
    assert_eq!(result, Err(EconomyError::SourceRejected));
    assert_eq!(f.state(), (0, 0, 0, 0, 0));
    assert_eq!(f.bound(), 100);
    assert_eq!(
        debit_bound_and_burn_authorized_with_source_with_test_fault(
            &mut f.client,
            &f.realm,
            &burn,
            "plan",
            2000,
            10000,
            TestFault::BeforeCommit,
            authorize(&f.owner),
            source(&f.owner, "asset-a")
        ),
        Err(EconomyError::InjectedFailure)
    );
    assert_eq!(f.state(), (0, 0, 0, 0, 0));
    assert_eq!(f.bound(), 100);
    assert!(lookup_bound_debit_burn_authorized_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
        2000,
        10000,
        authorize(&f.owner)
    )
    .unwrap()
    .is_none());
    assert_eq!(
        debit_bound_and_burn_authorized_with_source_with_test_fault(
            &mut f.client,
            &f.realm,
            &burn,
            "plan",
            2000,
            10000,
            TestFault::CommitAcknowledgementLost,
            authorize(&f.owner),
            source(&f.owner, "asset-a")
        ),
        Err(EconomyError::OutcomeUnknown)
    );
    assert_eq!(f.state(), (1, 1, 30, 1, 1));
    assert_eq!(f.bound(), 70);
    let original = lookup_bound_debit_burn_authorized_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
        2000,
        10000,
        authorize(&f.owner),
    )
    .unwrap()
    .unwrap();
    let replay = debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
        2000,
        10000,
        authorize(&f.owner),
        |_, _| panic!("unknown recovery cannot repeat its source"),
    )
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt, original);
    assert_eq!(f.state(), (1, 1, 30, 1, 1));
    assert_eq!(f.bound(), 70);
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_bound_debit_burn_competing_sources_cannot_overspend() {
    let mut f = Fixture::new("competition", 100);
    let gate = Arc::new(Barrier::new(3));
    let workers = (0..2)
        .map(|index| {
            let mut client = f.connect();
            let realm = f.realm.clone();
            let who = f.owner.clone();
            let gate = gate.clone();
            thread::spawn(move || {
                gate.wait();
                debit_bound_and_burn_authorized_with_source_with_timeouts(
                    &mut client,
                    &realm,
                    &burn(&format!("operation-{index}"), 80),
                    "plan",
                    2000,
                    10000,
                    authorize(&who),
                    source(&who, if index == 0 { "asset-a" } else { "asset-b" }),
                )
            })
        })
        .collect::<Vec<_>>();
    gate.wait();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(EconomyError::InsufficientBoundGold)))
            .count(),
        1
    );
    assert_eq!(f.bound(), 20);
    assert_eq!(f.state(), (1, 1, 80, 1, 1));
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_bound_debit_burn_same_source_key_is_isolated_by_owner_and_incarnation() {
    let mut f = Fixture::new("namespace", 100);
    let other = who("second-player", "00000000-0000-4000-8000-000000000002");
    let reborn = who(&f.owner.account_id, "00000000-0000-4000-8000-000000000003");
    f.add_owner(&other, 100);
    f.add_owner(&reborn, 100);
    let mut keys = std::collections::BTreeSet::new();
    let mut events = std::collections::BTreeSet::new();
    for who in [f.owner.clone(), other, reborn] {
        let receipt = debit_bound_and_burn_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            &burn("same-raw-id", 30),
            "plan",
            2000,
            10000,
            authorize(&who),
            source(&who, "asset-a"),
        )
        .unwrap();
        assert!(keys.insert(receipt.receipt.operation_key));
        assert!(events.insert(receipt.receipt.burn_event_id));
        assert_eq!(
            read_wallet(&mut f.client, &f.realm, &who)
                .unwrap()
                .unwrap()
                .bound_gold,
            70
        );
    }
    assert_eq!(f.state(), (3, 3, 90, 3, 3));
    let absent = who("unfunded-player", "00000000-0000-4000-8000-000000000004");
    f.add_owner(&absent, 0);
    assert_eq!(
        debit_bound_and_burn_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            &burn("same-raw-id", 30),
            "plan",
            2000,
            10000,
            authorize(&absent),
            |_, _| panic!("insufficient balance checked before source")
        ),
        Err(EconomyError::InsufficientBoundGold)
    );
    assert!(
        read_wallet(&mut f.client, &f.realm, &absent)
            .unwrap()
            .is_none(),
        "failed debit cannot provision a durable wallet"
    );
    assert_eq!(f.state(), (3, 3, 90, 3, 3));
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_bound_debit_burn_authorization_precedes_receipts_and_recovery_ignores_disabled_gap() {
    let mut f = Fixture::new("authorization", 100);
    let burn = burn("original-operation", 30);
    let first = debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
        2000,
        10000,
        authorize(&f.owner),
        source(&f.owner, "asset-a"),
    )
    .unwrap();
    for (column, expected) in [
        ("allowed", EconomyError::Unauthenticated),
        ("banned", EconomyError::AccountUnavailable),
    ] {
        let unavailable = column == "banned";
        f.client
            .execute(
                &format!("UPDATE debit_test_actors SET {column}=$1"),
                &[&unavailable],
            )
            .unwrap();
        assert_eq!(
            debit_bound_and_burn_authorized_with_source_with_timeouts(
                &mut f.client,
                &f.realm,
                &burn,
                "plan",
                2000,
                10000,
                authorize(&f.owner),
                |_, _| panic!("unauthorized replay cannot reach source")
            ),
            Err(expected.clone())
        );
        assert_eq!(
            lookup_bound_debit_burn_authorized_with_timeouts(
                &mut f.client,
                &f.realm,
                &burn,
                "plan",
                2000,
                10000,
                authorize(&f.owner)
            ),
            Err(expected)
        );
        f.client
            .execute(
                &format!("UPDATE debit_test_actors SET {column}=$1"),
                &[&!unavailable],
            )
            .unwrap();
    }
    let mut disabled = policy();
    disabled.policy_version = "debit-disabled".into();
    disabled.enabled = false;
    register_policy(&mut f.client, &f.realm, &disabled).unwrap();
    activate_policy(&mut f.client, &f.realm, &disabled.policy_version, true).unwrap();
    assert_eq!(
        debit_bound_and_burn_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            &self::burn("fresh-disabled", 10),
            "plan",
            2000,
            10000,
            authorize(&f.owner),
            |_, _| panic!("disabled cannot mutate source")
        ),
        Err(EconomyError::Disabled)
    );
    let replay = debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
        2000,
        10000,
        authorize(&f.owner),
        |_, _| panic!("disabled replay cannot mutate source"),
    )
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt, first.receipt);
    assert_eq!(
        lookup_bound_debit_burn_authorized_with_timeouts(
            &mut f.client,
            &f.realm,
            &burn,
            "plan",
            2000,
            10000,
            authorize(&f.owner)
        )
        .unwrap(),
        Some(first.receipt.clone())
    );
    activate_policy(&mut f.client, &f.realm, &policy().policy_version, true).unwrap();
    let today = read_realm(&mut f.client, &f.realm).unwrap().genesis_day;
    f.client.execute("UPDATE hunting_economy_pools SET genesis_day=$2,settled_through_day=$3 WHERE realm_id=$1",&[&f.realm,&(today-1),&(today-2)]).unwrap();
    assert!(matches!(
        debit_bound_and_burn_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            &self::burn("fresh-gap", 10),
            "plan",
            2000,
            10000,
            authorize(&f.owner),
            |_, _| panic!("unsettled day cannot mutate source")
        ),
        Err(EconomyError::SettlementRequired { .. })
    ));
    assert_eq!(
        lookup_bound_debit_burn_authorized_with_timeouts(
            &mut f.client,
            &f.realm,
            &burn,
            "plan",
            2000,
            10000,
            authorize(&f.owner)
        )
        .unwrap(),
        Some(first.receipt)
    );
    assert_eq!(f.bound(), 70);
    assert_eq!(f.state(), (1, 1, 30, 1, 1));
}

#[test]
#[ignore = "requires explicit isolated loopback PostgreSQL and test-support"]
fn postgres_bound_debit_burn_timeouts_are_bounded_local_and_recoverable() {
    let mut f = Fixture::new("timeouts", 100);
    let burn = burn("original-operation", 30);
    let before: (String, String) = {
        let row = f
            .client
            .query_one(
                "SELECT current_setting('lock_timeout'),current_setting('statement_timeout')",
                &[],
            )
            .unwrap();
        (row.get(0), row.get(1))
    };
    let mut blocker = f.connect();
    let mut held = blocker.transaction().unwrap();
    lock_realm_and_read_database_clock(&mut held, &f.realm).unwrap();
    let result = debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
        30,
        200,
        authorize(&f.owner),
        |_, _| panic!("lock timeout cannot reach source"),
    );
    assert!(
        matches!(result,Err(EconomyError::Database{sqlstate:Some(ref state)}) if state=="55P03"||state=="57014")
    );
    held.rollback().unwrap();
    let result = debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
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
    let after: (String, String) = {
        let row = f
            .client
            .query_one(
                "SELECT current_setting('lock_timeout'),current_setting('statement_timeout')",
                &[],
            )
            .unwrap();
        (row.get(0), row.get(1))
    };
    assert_eq!(after, before);
    assert_eq!(f.bound(), 100);
    assert_eq!(f.state(), (0, 0, 0, 0, 0));
    assert!(lookup_bound_debit_burn_authorized_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
        2000,
        10000,
        authorize(&f.owner)
    )
    .unwrap()
    .is_none());
    debit_bound_and_burn_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &burn,
        "plan",
        2000,
        10000,
        authorize(&f.owner),
        source(&f.owner, "asset-a"),
    )
    .unwrap();
    assert_eq!(f.bound(), 70);
    assert_eq!(f.state(), (1, 1, 30, 1, 1));
    let after_success: (String, String) = {
        let row = f
            .client
            .query_one(
                "SELECT current_setting('lock_timeout'),current_setting('statement_timeout')",
                &[],
            )
            .unwrap();
        (row.get(0), row.get(1))
    };
    assert_eq!(after_success, before);
}

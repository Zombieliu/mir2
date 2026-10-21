//! Actual PostgreSQL acceptance, deliberately ignored in normal test runs.
//! Run with an explicitly isolated MIR2_ECONOMY_TEST_DATABASE_URL and
//! `--features test-support --test postgres_authority -- --ignored`.
//! These fixtures directly seed a past closed day in a unique test realm, because
//! the production API correctly has no client timestamp, permit injection, or
//! backdated burn function. Histories are retained; no shared data is deleted.
#![cfg(feature = "test-support")]

use mir2_economy_policy::{Policy, Rational};
use mir2_economy_service::*;
use postgres::{Client, NoTls};
use serde_json::json;
use std::{
    sync::{Arc, Barrier},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

fn connect() -> Client {
    let url = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL").expect(
        "set an explicitly isolated MIR2_ECONOMY_TEST_DATABASE_URL to run ignored acceptance",
    );
    let config: postgres::Config = url
        .parse()
        .expect("valid isolated test PostgreSQL configuration");
    assert!(config
        .get_hostaddrs()
        .iter()
        .all(std::net::IpAddr::is_loopback));
    assert!(config
        .get_dbname()
        .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
    assert!(!config.get_hosts().is_empty() && config.get_hosts().iter().all(|host| matches!(host,
        postgres::config::Host::Tcp(name) if matches!(name.as_str(), "localhost" | "127.0.0.1" | "::1"))),
        "acceptance fixtures require a loopback host");
    config
        .connect(NoTls)
        .expect("isolated PostgreSQL connection must succeed")
}
fn policy() -> Policy {
    Policy {
        schema_version: 1,
        policy_version: "acceptance-v1".into(),
        enabled: true,
        k: Rational {
            numerator: 1,
            denominator: 10,
        },
        conversion_fee: Rational {
            numerator: 2,
            denominator: 100,
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
fn owner(incarnation: &str) -> WalletOwner {
    WalletOwner {
        account_id: "isolated-player".into(),
        character_index: 0,
        incarnation: incarnation.into(),
    }
}
fn fixture(label: &str) -> (Client, String, WalletOwner) {
    let mut client = connect();
    client
        .batch_execute(MIGRATION_SQL)
        .expect("isolated test migration");
    client.batch_execute(MATERIAL_CONVERSION_MIGRATION_SQL).expect("material ledger migration");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let realm = format!("hunting-test-{label}-{}-{nonce}", std::process::id());
    initialize_realm(&mut client, &realm, &policy()).unwrap();
    let who = owner("first-creation");
    assert_eq!(
        initialize_wallet(&mut client, &realm, &who).unwrap(),
        WalletBalance::default()
    );
    (client, realm, who)
}
/// Fixture-only SQL: one previously completed destruction before today's 04:00.
fn seed_closed_burn(client: &mut Client, realm: &str, gold: i64, days_ago: i64) -> i64 {
    let current = read_realm(client, realm).unwrap();
    let today = current.genesis_day;
    let day = today - days_ago;
    client.execute("UPDATE hunting_economy_pools SET genesis_day=$2,settled_through_day=$3 WHERE realm_id=$1",
        &[&realm,&day,&(day-1)]).unwrap();
    let mut tx = client.transaction().unwrap();
    let clock = lock_realm_and_read_database_clock(&mut tx, realm).unwrap();
    let fixture_key = format!("fixture-closed-{day}");
    tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,$2,'burn','fixture',$3,'fixture',$4,$5)",
        &[&realm,&fixture_key,&json!({"fixture_only":true}),&day,&clock.database_time_ms]).unwrap();
    tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms) VALUES($1,$2,$2,'siege','fixture',$2,$3,'fixture','acceptance-v1',$4,$5)",
        &[&realm,&fixture_key,&gold,&day,&clock.database_time_ms]).unwrap();
    tx.commit().unwrap();
    day
}
fn credit(client: &mut Client, realm: &str, who: &WalletOwner, amount: u64) {
    credit_bound_with_source(
        client,
        realm,
        who,
        "test-production",
        "source-digest",
        amount,
        |_, _| Ok(()),
    )
    .unwrap();
}
fn count(client: &mut Client, table: &str, realm: &str) -> i64 {
    // Table names are hard-coded in this test, never network input.
    client
        .query_one(
            &format!("SELECT count(*)::bigint FROM {table} WHERE realm_id=$1"),
            &[&realm],
        )
        .unwrap()
        .get(0)
}

#[test]
#[ignore = "requires explicitly isolated PostgreSQL; run with --ignored and test-support"]
fn postgres_conversion_is_atomic_idempotent_and_incarnation_bound() {
    let (mut client, realm, who) = fixture("atomic");
    credit(&mut client, &realm, &who, 2_000_000);
    assert!(matches!(
        convert(&mut client, &realm, &who, "empty-pool", 1_000_000),
        Err(EconomyError::InsufficientPermits)
    ));
    assert_eq!(
        read_wallet(&mut client, &realm, &who)
            .unwrap()
            .unwrap()
            .bound_gold,
        2_000_000
    );
    seed_closed_burn(&mut client, &realm, 100_000_000, 1);
    assert!(matches!(
        convert(&mut client, &realm, &who, "catch-up", 1_000_000),
        Err(EconomyError::SettlementRequired { .. })
    ));
    assert_eq!(settle_due(&mut client, &realm, 30).unwrap().len(), 1);
    let before_pool = read_realm(&mut client, &realm).unwrap();
    let before_wallet = read_wallet(&mut client, &realm, &who).unwrap();
    let before_burns = count(&mut client, "hunting_economy_burn_events", &realm);
    let before_operations = count(&mut client, "hunting_economy_operations", &realm);
    let before_outbox = count(&mut client, "hunting_economy_outbox", &realm);
    assert!(matches!(
        convert_with_test_fault(
            &mut client,
            &realm,
            &who,
            "rollback",
            1_000_000,
            TestFault::BeforeCommit
        ),
        Err(EconomyError::InjectedFailure)
    ));
    assert_eq!(read_realm(&mut client, &realm).unwrap(), before_pool);
    assert_eq!(
        read_wallet(&mut client, &realm, &who).unwrap(),
        before_wallet
    );
    assert_eq!(
        count(&mut client, "hunting_economy_burn_events", &realm),
        before_burns
    );
    assert_eq!(
        count(&mut client, "hunting_economy_operations", &realm),
        before_operations
    );
    assert_eq!(
        count(&mut client, "hunting_economy_outbox", &realm),
        before_outbox
    );
    assert!(matches!(
        convert_with_test_fault(
            &mut client,
            &realm,
            &who,
            "unknown",
            1_000_000,
            TestFault::CommitAcknowledgementLost
        ),
        Err(EconomyError::OutcomeUnknown)
    ));
    let replay = convert(&mut client, &realm, &who, "unknown", 1_000_000).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt.quote.fee_unbound_gold, 20_000);
    assert_eq!(
        replay.receipt.wallet_after,
        WalletBalance {
            bound_gold: 1_000_000,
            unbound_gold: 980_000,
            revision: 2
        }
    );
    assert_eq!(
        replay.receipt.pool_after_atoms,
        before_pool.pool.stock_atoms - 1_000_000
    );
    assert!(matches!(
        convert(&mut client, &realm, &who, "unknown", 999_999),
        Err(EconomyError::RequestConflict)
    ));
    assert_eq!(
        count(&mut client, "hunting_economy_burn_events", &realm),
        before_burns + 1
    );
    assert_eq!(count(&mut client, "hunting_economy_conversions", &realm), 1);
    let reborn = owner("second-creation");
    assert_eq!(
        initialize_wallet(&mut client, &realm, &reborn).unwrap(),
        WalletBalance::default()
    );
    assert!(matches!(
        convert(&mut client, &realm, &reborn, "unknown", 1_000_000),
        Err(EconomyError::InsufficientBoundGold)
    ));
}

#[test]
#[ignore = "requires explicitly isolated PostgreSQL; run with --ignored and test-support"]
fn postgres_competing_conversions_never_overspend_one_global_pool() {
    let (mut client, realm, who) = fixture("concurrent-convert");
    credit(&mut client, &realm, &who, 4_000_000);
    seed_closed_burn(&mut client, &realm, 20_000_000, 1);
    settle_due(&mut client, &realm, 30).unwrap();
    let stock = read_realm(&mut client, &realm).unwrap().pool.stock_atoms;
    assert_eq!(stock, 1_986_667);
    let gate = Arc::new(Barrier::new(3));
    let handles = (0..2)
        .map(|n| {
            let realm = realm.clone();
            let who = who.clone();
            let gate = gate.clone();
            thread::spawn(move || {
                let mut other = connect();
                gate.wait();
                convert(
                    &mut other,
                    &realm,
                    &who,
                    &format!("parallel-{n}"),
                    1_000_000,
                )
            })
        })
        .collect::<Vec<_>>();
    gate.wait();
    let results = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(EconomyError::InsufficientPermits)))
            .count(),
        1
    );
    assert_eq!(
        read_realm(&mut client, &realm).unwrap().pool.stock_atoms,
        stock - 1_000_000
    );
    assert_eq!(
        read_wallet(&mut client, &realm, &who)
            .unwrap()
            .unwrap()
            .bound_gold,
        3_000_000
    );
    assert_eq!(count(&mut client, "hunting_economy_conversions", &realm), 1);
}

#[test]
#[ignore = "requires explicitly isolated PostgreSQL; run with --ignored and test-support"]
fn postgres_multiday_settlement_cap_decay_and_duplicate_workers() {
    let (mut client, realm, _) = fixture("daily-workers");
    let day = seed_closed_burn(&mut client, &realm, 10_000_000_000, 3);
    let original = read_realm(&mut client, &realm).unwrap();
    assert!(matches!(
        settle_due_with_test_fault(&mut client, &realm, 30, TestFault::BeforeCommit),
        Err(EconomyError::InjectedFailure)
    ));
    assert_eq!(read_realm(&mut client, &realm).unwrap(), original);
    assert_eq!(count(&mut client, "hunting_economy_settlements", &realm), 0);
    let gate = Arc::new(Barrier::new(3));
    let workers = (0..2)
        .map(|_| {
            let realm = realm.clone();
            let gate = gate.clone();
            thread::spawn(move || {
                let mut other = connect();
                gate.wait();
                settle_due(&mut other, &realm, 30)
            })
        })
        .collect::<Vec<_>>();
    gate.wait();
    let rows = workers
        .into_iter()
        .map(|h| h.join().unwrap().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.iter().map(Vec::len).sum::<usize>(), 3);
    assert_eq!(count(&mut client, "hunting_economy_settlements", &realm), 3);
    let first = read_settlement(&mut client, &realm, day).unwrap().unwrap();
    assert_eq!(first.plan.minted_atoms, 108_000_000);
    assert!(first.plan.capped_atoms > 0);
    assert_eq!(first.plan.decayed_atoms, 720_000);
    assert_eq!(first.plan.next_pool.stock_atoms, 107_280_000);
    let next = read_settlement(&mut client, &realm, day + 1)
        .unwrap()
        .unwrap();
    assert_eq!(next.plan.minted_atoms, 0);
    assert_eq!(next.plan.decayed_atoms, 715_200);
    assert!(settle_due(&mut client, &realm, 30).unwrap().is_empty());
}

#[test]
#[ignore = "requires explicitly isolated PostgreSQL; run with --ignored and test-support"]
fn postgres_trusted_source_rollback_replay_and_history_are_enforced() {
    let (mut client, realm, who) = fixture("trusted-source");
    let failed = credit_bound_with_source(
        &mut client,
        &realm,
        &who,
        "failed-source",
        "payload",
        100,
        |_, _| Err(EconomyError::SourceRejected),
    );
    assert!(matches!(failed, Err(EconomyError::SourceRejected)));
    assert_eq!(
        read_wallet(&mut client, &realm, &who).unwrap(),
        Some(WalletBalance::default())
    );
    let first =
        credit_bound_with_source(&mut client, &realm, &who, "once", "payload", 100, |_, _| {
            Ok(())
        })
        .unwrap();
    let replay =
        credit_bound_with_source(&mut client, &realm, &who, "once", "payload", 100, |_, _| {
            panic!("replay must not execute trusted source")
        })
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt, first.receipt);
    assert!(matches!(
        credit_bound_with_source(
            &mut client,
            &realm,
            &who,
            "once",
            "different-payload",
            100,
            |_, _| Ok(())
        ),
        Err(EconomyError::RequestConflict)
    ));
    let burn = TrustedBurn {
        source_kind: "equipment_permanent_loss".into(),
        source_key: "battle-loss-1".into(),
        layer: BurnLayer::LightPvp,
        gold_equivalent: 252,
        valuation_version: "fixed-v1".into(),
    };
    assert!(matches!(
        burn_with_source(&mut client, &realm, &burn, "source", |_, _| Err(
            EconomyError::SourceRejected
        )),
        Err(EconomyError::SourceRejected)
    ));
    assert_eq!(count(&mut client, "hunting_economy_burn_events", &realm), 0);
    let burned = burn_with_source(&mut client, &realm, &burn, "source", |_, _| Ok(())).unwrap();
    assert!(!burned.replayed);
    assert!(
        burn_with_source(&mut client, &realm, &burn, "source", |_, _| panic!(
            "duplicate destruction"
        ))
        .unwrap()
        .replayed
    );
    assert!(client
        .execute(
            "UPDATE hunting_economy_burn_events SET gold_equivalent=999 WHERE realm_id=$1",
            &[&realm]
        )
        .is_err());
    let mut changed = policy();
    changed.k.numerator = 2;
    assert!(matches!(
        register_policy(&mut client, &realm, &changed),
        Err(EconomyError::PolicyConflict)
    ));
    assert!(client
        .execute(
            "DELETE FROM hunting_economy_policies WHERE realm_id=$1",
            &[&realm]
        )
        .is_err());
    changed.policy_version = "changed-k-after-daily-activity".into();
    register_policy(&mut client, &realm, &changed).unwrap();
    assert_eq!(
        activate_policy(&mut client, &realm, &changed.policy_version, true),
        Err(EconomyError::InvalidInput(
            "policy_change_after_daily_activity"
        ))
    );
    assert_eq!(
        read_realm(&mut client, &realm).unwrap().policy_version,
        policy().policy_version
    );
    let mut disabled = policy();
    disabled.policy_version = "disabled-v2".into();
    disabled.enabled = false;
    register_policy(&mut client, &realm, &disabled).unwrap();
    activate_policy(&mut client, &realm, &disabled.policy_version, true).unwrap();
    assert!(matches!(
        convert(&mut client, &realm, &who, "disabled", 1),
        Err(EconomyError::Disabled)
    ));
    assert!(
        credit_bound_with_source(
            &mut client,
            &realm,
            &who,
            "once",
            "payload",
            100,
            |_, _| panic!("disabled replay source")
        )
        .unwrap()
        .replayed
    );
}

#[test]
#[ignore = "requires explicitly isolated PostgreSQL; run with --ignored and test-support"]
fn postgres_bounded_maintenance_and_authorization_reset_pool_session_timeouts() {
    let (mut client, realm, who) = fixture("lock-timeout");
    let mut other = connect();
    let original_lock: String = other.query_one("SHOW lock_timeout", &[]).unwrap().get(0);
    let original_statement: String = other
        .query_one("SHOW statement_timeout", &[])
        .unwrap()
        .get(0);
    let mut held = client.transaction().unwrap();
    lock_realm_and_read_database_clock(&mut held, &realm).unwrap();
    let started = std::time::Instant::now();
    let failures = [
        settle_due_with_timeouts(&mut other, &realm, 31, 50, 500).map(|_| ()),
        read_wallet_authorized_with_timeouts(&mut other, &realm, 50, 500, |_, _| {
            panic!("authorization cannot run before realm lock")
        })
        .map(|_| ()),
        convert_authorized_with_timeouts(&mut other, &realm, "blocked", 1, 50, 500, |_, _| {
            panic!("authorization cannot run before realm lock")
        })
        .map(|_| ()),
    ];
    for failure in failures {
        assert!(
            matches!(failure,Err(EconomyError::Database{sqlstate:Some(ref code)}) if code=="55P03" || code=="57014")
        );
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert_eq!(
        other
            .query_one("SHOW lock_timeout", &[])
            .unwrap()
            .get::<_, String>(0),
        original_lock
    );
    assert_eq!(
        other
            .query_one("SHOW statement_timeout", &[])
            .unwrap()
            .get::<_, String>(0),
        original_statement
    );
    held.rollback().unwrap();
    assert!(settle_due_with_timeouts(&mut other, &realm, 31, 50, 500)
        .unwrap()
        .is_empty());
    assert!(
        read_wallet_authorized_with_timeouts(&mut other, &realm, 50, 500, |_, _| Ok(who.clone()))
            .is_ok()
    );
    assert_eq!(
        other
            .query_one("SHOW lock_timeout", &[])
            .unwrap()
            .get::<_, String>(0),
        original_lock
    );
    assert_eq!(
        other
            .query_one("SHOW statement_timeout", &[])
            .unwrap()
            .get::<_, String>(0),
        original_statement
    );
}

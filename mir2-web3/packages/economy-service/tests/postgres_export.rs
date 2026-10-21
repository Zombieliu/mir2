//! Real PostgreSQL acceptance for durable partial metric export. All fixtures
//! append unique hunting-export-test-* realms; no shared history is deleted.
//! Explicitly ignored unless test-support and isolated acceptance credentials.
#![cfg(feature = "test-support")]

use mir2_economy_policy::{Policy, Rational};
use mir2_economy_service::*;
use postgres::{config::Host, Client, NoTls};
use serde_json::json;
use std::{
    sync::{Arc, Barrier, Once, OnceLock},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

fn connect() -> Client {
    let url = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
        .unwrap_or_else(|_| panic!("explicit isolated economy acceptance database is required"));
    let config: postgres::Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid isolated acceptance database configuration"));
    let allowed_hosts = !config.get_hosts().is_empty()
        && config.get_hosts().iter().all(|host| match host {
            Host::Tcp(name) => matches!(
                name.to_ascii_lowercase().as_str(),
                "localhost" | "127.0.0.1" | "::1"
            ),
            #[allow(unreachable_patterns)]
            _ => false,
        });
    let allowed_database = config
        .get_dbname()
        .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_"));
    assert!(
        allowed_hosts
            && allowed_database
            && config
                .get_hostaddrs()
                .iter()
                .all(std::net::IpAddr::is_loopback),
        "refusing fixture SQL outside local hunting acceptance database"
    );
    let mut client = config
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("isolated acceptance connection failed"));
    // Each test process and all of its worker connections share one fresh
    // schema. Reapplying an old view definition to a newer shared public schema
    // would try to remove appended observation columns before testing export.
    static TEST_SCHEMA: OnceLock<String> = OnceLock::new();
    let schema = TEST_SCHEMA.get_or_init(|| format!("hunting_export_p_{}_{}",
        std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    client.batch_execute(&format!(
        "CREATE SCHEMA IF NOT EXISTS {schema}; SET search_path TO {schema},public"
    )).expect("isolated export schema");
    client
}

fn policy() -> Policy {
    Policy {
        schema_version: 1,
        policy_version: "export-acceptance-v1".into(),
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

fn fixture(label: &str) -> (Client, String, WalletOwner) {
    static MIGRATIONS: Once = Once::new();
    let mut client = connect();
    MIGRATIONS.call_once(|| {
        client.batch_execute(MIGRATION_SQL).unwrap();
        client.batch_execute(OBSERVATION_MIGRATION_SQL).unwrap();
        client.batch_execute(METRIC_EXPORT_MIGRATION_SQL).unwrap();
        client.batch_execute(MATERIAL_CONVERSION_MIGRATION_SQL).unwrap();
        client.batch_execute(MATERIAL_OBSERVATION_MIGRATION_SQL).unwrap();
    });
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let realm = format!("hunting-export-test-{label}-{}-{nonce}", std::process::id());
    initialize_realm(&mut client, &realm, &policy()).unwrap();
    let owner = WalletOwner {
        account_id: "synthetic-export-player".into(),
        character_index: 0,
        incarnation: "fixture-creation-v1".into(),
    };
    initialize_wallet(&mut client, &realm, &owner).unwrap();
    (client, realm, owner)
}

fn settle_fixture(client: &mut Client, realm: &str) -> SettlementReceipt {
    let day = read_realm(client, realm).unwrap().genesis_day - 1;
    client.execute("UPDATE hunting_economy_pools SET genesis_day=$2::bigint,settled_through_day=$2::bigint-1 WHERE realm_id=$1",&[&realm,&day]).unwrap();
    let mut tx = client.transaction().unwrap();
    let clock = lock_realm_and_read_database_clock(&mut tx, realm).unwrap();
    let key = format!("synthetic-closed-destruction-{day}");
    tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,$2,'burn','synthetic',$3,'synthetic',$4,$5)",&[&realm,&key,&json!({"classification":"synthetic-test-only"}),&day,&clock.database_time_ms]).unwrap();
    tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms) VALUES($1,$2,$2,'siege','synthetic-fixture',$2,100000000,'synthetic-v1','export-acceptance-v1',$3,$4)",&[&realm,&key,&day,&clock.database_time_ms]).unwrap();
    tx.commit().unwrap();
    settle_due(client, realm, 31).unwrap().remove(0)
}

fn scalar(client: &mut Client, sql: &str, realm: &str) -> i64 {
    client.query_one(sql, &[&realm]).unwrap().get(0)
}
fn assert_pending_without_metrics(client: &mut Client, realm: &str) {
    assert_eq!(
        scalar(
            client,
            "SELECT count(*)::bigint FROM hunting_economy_daily_metric_versions WHERE realm_id=$1",
            realm
        ),
        0
    );
    assert_eq!(
        scalar(
            client,
            "SELECT count(*)::bigint FROM hunting_economy_metric_exports WHERE realm_id=$1",
            realm
        ),
        0
    );
    assert_eq!(scalar(client,"SELECT count(*)::bigint FROM hunting_economy_outbox WHERE realm_id=$1 AND event_kind='daily_settlement' AND delivered_at_ms IS NULL",realm),1);
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_is_partial_exact_and_restart_idempotent() {
    let (mut client, realm, owner) = fixture("partial");
    for index in 0..3 {
        let mut who = owner.clone();
        who.character_index = index;
        initialize_wallet(&mut client, &realm, &who).unwrap();
        credit_bound_with_source(
            &mut client,
            &realm,
            &who,
            &format!("wide-source-{index}"),
            "synthetic-source",
            i64::MAX as u64,
            |_, _| Ok(()),
        )
        .unwrap();
    }
    let source = settle_fixture(&mut client, &realm);
    assert_eq!(source.bound_gold_stock, "27670116110564327421");
    // A newer operation must not make the immutable older receipt look stale.
    convert(&mut client, &realm, &owner, "new-open-day-conversion", 1000).unwrap();
    let exported = export_daily_metrics_with_timeouts(&mut client, &realm, 31, 1000, 5000).unwrap();
    assert_eq!(exported.receipts.len(), 1);
    assert_eq!(
        exported.receipts[0].disposition,
        MetricExportDisposition::InsertedPartial
    );
    let row=client.query_one("SELECT is_complete,bound_gold_stock::text,unbound_gold_stock::text,new_wallet_bound_gold_stock::text,monthly_card_gold_per_card::text,active_account_total,heavy_producer_monthly_usd_equivalent::text FROM hunting_permit_daily_observations WHERE realm_id=$1",&[&realm]).unwrap();
    assert!(!row.get::<_, bool>("is_complete"));
    for field in [
        "bound_gold_stock",
        "unbound_gold_stock",
        "monthly_card_gold_per_card",
        "heavy_producer_monthly_usd_equivalent",
    ] {
        assert!(row.get::<_, Option<String>>(field).is_none());
    }
    assert!(row.get::<_, Option<i64>>("active_account_total").is_none());
    assert_eq!(
        row.get::<_, String>("new_wallet_bound_gold_stock"),
        source.bound_gold_stock
    );
    assert_eq!(
        read_settlement(&mut client, &realm, source.business_day).unwrap(),
        Some(source)
    );
    let mut restarted = connect();
    assert!(export_daily_metrics(&mut restarted, &realm, 31)
        .unwrap()
        .receipts
        .is_empty());
    assert_eq!(
        scalar(
            &mut restarted,
            "SELECT count(*)::bigint FROM hunting_economy_metric_exports WHERE realm_id=$1",
            &realm
        ),
        1
    );
    assert_eq!(scalar(&mut restarted,"SELECT count(*)::bigint FROM hunting_economy_outbox WHERE realm_id=$1 AND event_kind='daily_settlement' AND delivered_at_ms IS NOT NULL",&realm),1);
    let rejected=restarted.execute("UPDATE hunting_economy_metric_exports SET consumer_version='should-never-change' WHERE realm_id=$1",&[&realm]).unwrap_err();
    assert_eq!(rejected.code().map(|c| c.code()), Some("P0001"));
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_failure_rolls_back_target_receipt_and_ack() {
    let (mut client, realm, _) = fixture("rollback");
    settle_fixture(&mut client, &realm);
    for fault in [
        MetricExportTestFault::AfterMetricWrite,
        MetricExportTestFault::BeforeCommit,
    ] {
        assert_eq!(
            export_daily_metrics_with_test_fault(&mut client, &realm, 31, fault),
            Err(EconomyError::InjectedFailure)
        );
        assert_pending_without_metrics(&mut client, &realm);
    }
    assert_eq!(
        export_daily_metrics(&mut connect(), &realm, 31)
            .unwrap()
            .receipts
            .len(),
        1
    );
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_unknown_commit_recovers_without_duplicate_revision() {
    let (mut client, realm, _) = fixture("unknown");
    settle_fixture(&mut client, &realm);
    assert_eq!(
        export_daily_metrics_with_test_fault(
            &mut client,
            &realm,
            31,
            MetricExportTestFault::CommitAcknowledgementLost
        ),
        Err(EconomyError::OutcomeUnknown)
    );
    let mut restarted = connect();
    assert!(export_daily_metrics(&mut restarted, &realm, 31)
        .unwrap()
        .receipts
        .is_empty());
    assert_eq!(
        scalar(
            &mut restarted,
            "SELECT count(*)::bigint FROM hunting_economy_daily_metric_versions WHERE realm_id=$1",
            &realm
        ),
        1
    );
    assert_eq!(
        scalar(
            &mut restarted,
            "SELECT count(*)::bigint FROM hunting_economy_metric_exports WHERE realm_id=$1",
            &realm
        ),
        1
    );
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_preserves_a_traceable_richer_revision() {
    let (mut client, realm, _) = fixture("richer");
    let source = settle_fixture(&mut client, &realm);
    client.execute("INSERT INTO hunting_economy_observation_settings(realm_id,settings_revision,policy_version,effective_from_day,full_realm_coverage_qualified,coverage_qualification_evidence,coverage_qualified_by,coverage_qualified_at_ms,created_at_ms) VALUES($1,1,'export-acceptance-v1',$2,true,'synthetic-test-only','test-only-qualification',0,0)",&[&realm,&source.business_day]).unwrap();
    client.execute("INSERT INTO hunting_economy_daily_metric_versions(realm_id,business_day,metric_revision,settings_revision,metric_source,metric_evidence,created_at_ms,financial_period_closed,source_period_end,source_ledger_watermark,coverage_complete,coverage_scope,coverage_evidence,bound_gold_stock,unbound_gold_stock) VALUES($1,$2::bigint,7,1,'synthetic-richer-aggregator',$3,0,true,((date '1970-01-01'+($2::bigint)::integer+1)+time '04:00') AT TIME ZONE 'Asia/Shanghai','synthetic-complete-watermark',true,'all_realm_assets','synthetic-test-only',1000,2000)",&[&realm,&source.business_day,&json!({"classification":"synthetic-test-only"})]).unwrap();
    let batch = export_daily_metrics(&mut client, &realm, 31).unwrap();
    assert_eq!(batch.receipts[0].metric_revision, 7);
    assert_eq!(
        batch.receipts[0].disposition,
        MetricExportDisposition::ReusedExisting
    );
    let row=client.query_one("SELECT is_complete,metric_revision,unbound_gold_stock::text FROM hunting_permit_daily_observations WHERE realm_id=$1",&[&realm]).unwrap();
    assert!(row.get::<_, bool>("is_complete"));
    assert_eq!(row.get::<_, i64>("metric_revision"), 7);
    assert_eq!(row.get::<_, String>("unbound_gold_stock"), "2000");
    assert_eq!(
        scalar(
            &mut client,
            "SELECT count(*)::bigint FROM hunting_economy_daily_metric_versions WHERE realm_id=$1",
            &realm
        ),
        1
    );
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_cursor_corruption_refuses_ack_and_can_retry_after_repair() {
    let (mut client, realm, _) = fixture("cursor");
    let source = settle_fixture(&mut client, &realm);
    client
        .execute(
            "UPDATE hunting_economy_pools SET settled_through_day=$2::bigint-1 WHERE realm_id=$1",
            &[&realm, &source.business_day],
        )
        .unwrap();
    assert!(matches!(
        export_daily_metrics(&mut client, &realm, 31),
        Err(EconomyError::CorruptHistory("metric_settlement_cursor"))
    ));
    assert_pending_without_metrics(&mut client, &realm);
    client
        .execute(
            "UPDATE hunting_economy_pools SET settled_through_day=$2 WHERE realm_id=$1",
            &[&realm, &source.business_day],
        )
        .unwrap();
    assert_eq!(
        export_daily_metrics(&mut client, &realm, 31)
            .unwrap()
            .receipts
            .len(),
        1
    );
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_live_permit_stock_corruption_refuses_ack() {
    let (mut client, realm, _) = fixture("stock");
    settle_fixture(&mut client, &realm);
    client
        .execute(
            "UPDATE hunting_economy_pools SET stock_atoms=stock_atoms+1 WHERE realm_id=$1",
            &[&realm],
        )
        .unwrap();
    assert!(matches!(
        export_daily_metrics(&mut client, &realm, 31),
        Err(EconomyError::CorruptHistory("metric_live_pool_stock"))
    ));
    assert_pending_without_metrics(&mut client, &realm);
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_changed_closed_day_burn_aggregate_refuses_ack() {
    let (mut client, realm, _) = fixture("burn-check");
    let source = settle_fixture(&mut client, &realm);
    let mut tx = client.transaction().unwrap();
    let clock = lock_realm_and_read_database_clock(&mut tx, &realm).unwrap();
    tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,'synthetic-late-ledger','burn','test',$2,'test',$3,$4)",&[&realm,&json!({"classification":"deliberate-synthetic-corruption"}),&source.business_day,&clock.database_time_ms]).unwrap();
    tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms) VALUES($1,'synthetic-late-ledger','synthetic-late-ledger','siege','test-corruption','test-corruption',1,'test','export-acceptance-v1',$2,$3)",&[&realm,&source.business_day,&clock.database_time_ms]).unwrap();
    tx.commit().unwrap();
    assert!(matches!(
        export_daily_metrics(&mut client, &realm, 31),
        Err(EconomyError::CorruptHistory("metric_burn_aggregate"))
    ));
    assert_pending_without_metrics(&mut client, &realm);
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_multiple_workers_create_one_version_and_one_ack() {
    let (mut client, realm, _) = fixture("workers");
    settle_fixture(&mut client, &realm);
    let barrier = Arc::new(Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let realm = realm.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                let mut c = connect();
                barrier.wait();
                export_daily_metrics(&mut c, &realm, 31)
                    .unwrap()
                    .receipts
                    .len()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .sum::<usize>(),
        1
    );
    assert_eq!(
        scalar(
            &mut client,
            "SELECT count(*)::bigint FROM hunting_economy_metric_exports WHERE realm_id=$1",
            &realm
        ),
        1
    );
    assert_eq!(
        scalar(
            &mut client,
            "SELECT count(*)::bigint FROM hunting_economy_daily_metric_versions WHERE realm_id=$1",
            &realm
        ),
        1
    );
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_recovers_previously_acked_event_without_own_consumer_receipt() {
    let (mut client, realm, _) = fixture("old-ack");
    settle_fixture(&mut client, &realm);
    let event:String=client.query_one("SELECT event_id FROM hunting_economy_outbox WHERE realm_id=$1 AND event_kind='daily_settlement'",&[&realm]).unwrap().get(0);
    acknowledge_outbox(&mut client, &realm, &event).unwrap();
    let old_ack: i64 = client
        .query_one(
            "SELECT delivered_at_ms FROM hunting_economy_outbox WHERE realm_id=$1 AND event_id=$2",
            &[&realm, &event],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        export_daily_metrics(&mut client, &realm, 31)
            .unwrap()
            .receipts
            .len(),
        1
    );
    let ack: i64 = client
        .query_one(
            "SELECT delivered_at_ms FROM hunting_economy_outbox WHERE realm_id=$1 AND event_id=$2",
            &[&realm, &event],
        )
        .unwrap()
        .get(0);
    assert_eq!(old_ack, ack);
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_export_lock_timeout_is_bounded_and_preserves_pending_work() {
    let (mut client, realm, _) = fixture("lock");
    settle_fixture(&mut client, &realm);
    let mut held = client.transaction().unwrap();
    lock_realm_and_read_database_clock(&mut held, &realm).unwrap();
    let mut other = connect();
    assert!(
        matches!(export_daily_metrics_with_timeouts(&mut other,&realm,31,50,200),Err(EconomyError::Database{sqlstate:Some(code)}) if code=="55P03")
    );
    held.rollback().unwrap();
    assert_pending_without_metrics(&mut client, &realm);
    assert_eq!(
        export_daily_metrics_with_timeouts(&mut other, &realm, 31, 1000, 5000)
            .unwrap()
            .receipts
            .len(),
        1
    );
}

//! Real local PostgreSQL acceptance for material settlement/export accounting.
//! Each test owns a new schema. The current-day case uses the public Quote and
//! Confirm APIs with an explicitly synthetic canonical source adapter. Closed
//! days are append-only historical fixtures, not game confirmations or a clock
//! crossing: only mutable fixture pool cursors/stock and wallets are seeded.
//! No clock, immutable row, trigger, existing schema or game save is modified.
#![cfg(feature = "test-support")]

use mir2_economy_policy::{MaterialValuationEntry, MaterialValuationSet, Policy, Rational};
use mir2_economy_service::*;
use postgres::{config::Host, Client, NoTls, Transaction};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const ITEM_INDEX: i32 = 50_101;
const INPUT_UID: u64 = 77;
const QUANTITY: u64 = 2;
const UNIT_GOLD: u64 = 125;
const EQUIVALENT: u64 = 250;
const FEE: u64 = 5;
const INITIAL_UNBOUND: u64 = 10_000;
const OLD_CONSUMER: &str = "hunting-daily-new-wallet-diagnostics-v1";

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
    assert!(
        allowed_hosts
            && config
                .get_dbname()
                .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_"))
            && config
                .get_hostaddrs()
                .iter()
                .all(std::net::IpAddr::is_loopback),
        "refusing fixture SQL outside local hunting acceptance database"
    );
    config
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("isolated acceptance connection failed"))
}

fn policy() -> Policy {
    Policy {
        schema_version: 1,
        policy_version: "synthetic-material-export-v1".into(),
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

fn fingerprint(value: &Value) -> String {
    fn canonical(value: Value) -> Value {
        match value {
            Value::Object(fields) => {
                let mut fields = fields.into_iter().collect::<Vec<_>>();
                fields.sort_by(|a, b| a.0.cmp(&b.0));
                Value::Object(fields.into_iter().map(|(k, v)| (k, canonical(v))).collect())
            }
            Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
            other => other,
        }
    }
    let bytes = serde_json::to_vec(&canonical(value.clone())).unwrap();
    let hex = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn source_error(_: postgres::Error) -> EconomyError {
    EconomyError::SourceRejected
}

fn authorize_source(tx: &mut Transaction<'_>, owner: &WalletOwner) -> EconomyResult<WalletOwner> {
    if tx
        .query_opt(
            "SELECT 1 FROM synthetic_material_export_sources
         WHERE account_id=$1 AND character_index=$2 AND incarnation=$3",
            &[
                &owner.account_id,
                &owner.character_index,
                &owner.incarnation,
            ],
        )
        .map_err(source_error)?
        .is_none()
    {
        return Err(EconomyError::CharacterNotFound);
    }
    Ok(owner.clone())
}

fn read_source(
    tx: &mut Transaction<'_>,
    owner: &WalletOwner,
    quantity: u64,
) -> EconomyResult<(u64, Value)> {
    let row = tx
        .query_opt(
            "SELECT revision,checkpoint FROM synthetic_material_export_sources
         WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 FOR UPDATE",
            &[
                &owner.account_id,
                &owner.character_index,
                &owner.incarnation,
            ],
        )
        .map_err(source_error)?
        .ok_or(EconomyError::SourceRejected)?;
    let revision =
        u64::try_from(row.get::<_, i64>("revision")).map_err(|_| EconomyError::SourceRejected)?;
    let checkpoint: Value = row.get("checkpoint");
    if checkpoint["revision"].as_u64() != Some(revision)
        || checkpoint["item"]["economic_state"] != "bound"
        || checkpoint["item"]["item_index"].as_i64() != Some(i64::from(ITEM_INDEX))
        || checkpoint["item"]["unique_id"].as_u64() != Some(INPUT_UID)
        || quantity == 0
        || checkpoint["item"]["count"].as_u64() != Some(quantity)
    {
        return Err(EconomyError::SourceRejected);
    }
    Ok((revision, checkpoint))
}

fn quote_source(
    tx: &mut Transaction<'_>,
    owner: &WalletOwner,
    quantity: u64,
) -> EconomyResult<MaterialConversionQuoteSource> {
    let (revision, checkpoint) = read_source(tx, owner, quantity)?;
    Ok(MaterialConversionQuoteSource {
        item_index: ITEM_INDEX,
        input_unique_id: INPUT_UID,
        input_count: quantity,
        quantity,
        input_save_revision: revision,
        input_item_digest: fingerprint(&checkpoint),
        source_binding_digest: "a".repeat(64),
    })
}

fn persist_source(
    tx: &mut Transaction<'_>,
    quote: &MaterialConversionQuoteReceipt,
    operation: &str,
) -> EconomyResult<MaterialConversionSourceResult> {
    let (revision, mut checkpoint) = read_source(tx, &quote.owner, quote.input_count)?;
    if revision != quote.input_save_revision
        || fingerprint(&checkpoint) != quote.input_item_digest
        || quote.quantity != quote.input_count
    {
        return Err(EconomyError::SourceRejected);
    }
    let after = revision
        .checked_add(1)
        .ok_or(EconomyError::SourceRejected)?;
    checkpoint["revision"] = json!(after);
    checkpoint["item"]["economic_state"] = json!("converted");
    checkpoint["item"]["operation"] = json!(operation);
    let changed = tx
        .execute(
            "UPDATE synthetic_material_export_sources SET revision=$4,checkpoint=$5
         WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 AND revision=$6",
            &[
                &quote.owner.account_id,
                &quote.owner.character_index,
                &quote.owner.incarnation,
                &(after as i64),
                &checkpoint,
                &(revision as i64),
            ],
        )
        .map_err(source_error)?;
    if changed != 1 {
        return Err(EconomyError::SourceRejected);
    }
    Ok(MaterialConversionSourceResult {
        before_revision: revision,
        after_revision: after,
        input_unique_id: INPUT_UID,
        output_unique_id: INPUT_UID,
        checkpoint,
    })
}

struct Fixture {
    client: Client,
    realm: String,
    owner: WalletOwner,
}

impl Fixture {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!(
            "hme_{}_{}_{}",
            std::process::id(),
            nonce,
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        assert!(
            schema.len() <= 63
                && schema
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        );
        let mut client = connect();
        client
            .batch_execute(&format!("CREATE SCHEMA \"{schema}\";"))
            .unwrap();
        client
            .query_one("SELECT set_config('search_path',$1,false)", &[&schema])
            .unwrap();
        for migration in [
            MIGRATION_SQL,
            OBSERVATION_MIGRATION_SQL,
            METRIC_EXPORT_MIGRATION_SQL,
            BOUND_DEBIT_BURN_MIGRATION_SQL,
            ZERO_FEE_SOURCE_MIGRATION_SQL,
            MATERIAL_CONVERSION_MIGRATION_SQL,
            MATERIAL_OBSERVATION_MIGRATION_SQL,
        ] {
            client.batch_execute(migration).unwrap();
        }
        client
            .batch_execute(
                "CREATE TABLE synthetic_material_export_sources (
                account_id text NOT NULL,character_index integer NOT NULL,incarnation text NOT NULL,
                revision bigint NOT NULL,checkpoint jsonb NOT NULL,
                PRIMARY KEY(account_id,character_index,incarnation));",
            )
            .unwrap();
        let realm = format!("synthetic-material-export-{label}-{schema}");
        initialize_realm(&mut client, &realm, &policy()).unwrap();
        let owner = WalletOwner {
            account_id: "synthetic-material-export-owner".into(),
            character_index: 0,
            incarnation: "synthetic-material-export-incarnation".into(),
        };
        initialize_wallet(&mut client, &realm, &owner).unwrap();
        // Explicit fixture funding, not a material conversion or game reward.
        client
            .execute(
                "UPDATE hunting_economy_wallets SET unbound_gold=$2 WHERE realm_id=$1",
                &[&realm, &(INITIAL_UNBOUND as i64)],
            )
            .unwrap();
        let checkpoint = json!({"classification":"synthetic-canonical-material-export-adapter",
            "revision":0,"item":{"item_index":ITEM_INDEX,"unique_id":INPUT_UID,
                "count":QUANTITY,"economic_state":"bound","server_note":"complete-source-preserved"}});
        client.execute("INSERT INTO synthetic_material_export_sources(account_id,character_index,incarnation,revision,checkpoint) VALUES($1,$2,$3,0,$4)",
            &[&owner.account_id,&owner.character_index,&owner.incarnation,&checkpoint]).unwrap();
        let valuations = MaterialValuationSet::new(
            "synthetic-material-export-prices-v1",
            vec![MaterialValuationEntry {
                item_index: ITEM_INDEX,
                unit_gold: UNIT_GOLD,
            }],
        )
        .unwrap();
        register_material_valuation(&mut client, &realm, &valuations).unwrap();
        activate_material_valuation(&mut client, &realm, valuations.valuation_version()).unwrap();
        Self {
            client,
            realm,
            owner,
        }
    }

    fn quote(&mut self) -> MaterialConversionQuoteReceipt {
        self.quote_quantity("synthetic-material-export-quote", QUANTITY)
    }

    fn quote_quantity(&mut self, request: &str, quantity: u64) -> MaterialConversionQuoteReceipt {
        let owner = self.owner.clone();
        let result = quote_material_conversion_authorized_with_source_with_timeouts(
            &mut self.client,
            &self.realm,
            request,
            INPUT_UID,
            quantity,
            1000,
            5000,
            |tx, _| authorize_source(tx, &owner),
            |tx, _, who| quote_source(tx, who, quantity),
        )
        .unwrap();
        assert!(!result.replayed);
        result.receipt
    }

    fn mint_history(&mut self, closed_days: i64) -> SettlementReceipt {
        assert!((1..=2).contains(&closed_days));
        let genesis = read_realm(&mut self.client, &self.realm)
            .unwrap()
            .genesis_day
            - closed_days;
        self.client.execute("UPDATE hunting_economy_pools SET genesis_day=$2::bigint,settled_through_day=$2::bigint-1 WHERE realm_id=$1",
            &[&self.realm, &genesis]).unwrap();
        let mut tx = self.client.transaction().unwrap();
        let clock = lock_realm_and_read_database_clock(&mut tx, &self.realm).unwrap();
        append_burn(
            &mut tx,
            &self.realm,
            genesis,
            clock.database_time_ms,
            "synthetic-seed-destruction",
            "synthetic-history",
            "siege",
            100_000_000,
        );
        tx.commit().unwrap();
        let mut settled = settle_due(&mut self.client, &self.realm, 1).unwrap();
        assert_eq!(settled.len(), 1);
        settled.remove(0)
    }
}

fn append_burn(
    tx: &mut Transaction<'_>,
    realm: &str,
    day: i64,
    now: i64,
    key: &str,
    source_kind: &str,
    layer: &str,
    amount: i64,
) {
    let payload = json!({"classification":"append-only-synthetic-historical-burn"});
    let policy_version: String = tx
        .query_one(
            "SELECT policy_version FROM hunting_economy_pools WHERE realm_id=$1",
            &[&realm],
        )
        .unwrap()
        .get(0);
    tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,$2,'burn','synthetic',$3,$4,$5,$6)",
        &[&realm,&key,&payload,&fingerprint(&payload),&day,&now]).unwrap();
    tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms) VALUES($1,$2,$2,$3,$4,$2,$5,'gold-face-value-v1',$6,$7,$8)",
        &[&realm,&key,&layer,&source_kind,&amount,&policy_version,&day,&now]).unwrap();
}

#[derive(Clone, Copy)]
enum HistoryCase {
    Valid,
    MissingFee,
    WrongFee,
    OrphanFee,
}

fn append_material_history(
    f: &mut Fixture,
    quote: &MaterialConversionQuoteReceipt,
    day: i64,
    case: HistoryCase,
) {
    let mut tx = f.client.transaction().unwrap();
    let clock = lock_realm_and_read_database_clock(&mut tx, &f.realm).unwrap();
    let equivalent = quote.quote.gold_equivalent;
    let fee = quote.quote.fee_unbound_gold;
    if matches!(case, HistoryCase::OrphanFee) {
        append_burn(
            &mut tx,
            &f.realm,
            day,
            clock.database_time_ms,
            "synthetic-orphan-fee",
            "material_conversion_fee",
            "daily",
            fee as i64,
        );
        tx.commit().unwrap();
        return;
    }
    let key = material_conversion_operation_key(&f.realm, &f.owner, &quote.quote_id).unwrap();
    let source = persist_source(&mut tx, quote, &key).unwrap();
    tx.execute(
        "UPDATE hunting_economy_pools SET stock_atoms=stock_atoms-$2 WHERE realm_id=$1",
        &[&f.realm, &(equivalent as i64)],
    )
    .unwrap();
    tx.execute("UPDATE hunting_economy_wallets SET unbound_gold=unbound_gold-$2,revision=revision+1 WHERE realm_id=$1",
        &[&f.realm,&(fee as i64)]).unwrap();
    let pool: i64 = tx
        .query_one(
            "SELECT stock_atoms FROM hunting_economy_pools WHERE realm_id=$1",
            &[&f.realm],
        )
        .unwrap()
        .get(0);
    let wallet = tx.query_one(
        "SELECT bound_gold,unbound_gold,revision FROM hunting_economy_wallets WHERE realm_id=$1",
        &[&f.realm],
    ).unwrap();
    let request_digest = fingerprint(&json!(["hunting_material_confirmation_v1", quote]));
    let burn_event = format!("synthetic-material-fee-{key}");
    let receipt = MaterialConversionReceipt {
        realm_id: f.realm.clone(),
        operation_key: key.clone(),
        request_digest: request_digest.clone(),
        owner: f.owner.clone(),
        quote: quote.clone(),
        source,
        policy_version: quote.policy_version.clone(),
        business_day: day,
        committed_at_ms: clock.database_time_ms,
        wallet_after: WalletBalance {
            bound_gold: wallet.get::<_, i64>("bound_gold") as u64,
            unbound_gold: wallet.get::<_, i64>("unbound_gold") as u64,
            revision: wallet.get::<_, i64>("revision") as u64,
        },
        pool_after_atoms: pool as u64,
        burn_event_id: burn_event.clone(),
    };
    let data = serde_json::to_value(&receipt).unwrap();
    let digest = fingerprint(&data);
    tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,$2,'material_conversion',$3,$4,$5,$6,$7)",
        &[&f.realm,&key,&request_digest,&data,&digest,&day,&clock.database_time_ms]).unwrap();
    tx.execute("INSERT INTO hunting_economy_material_conversions(realm_id,operation_key,quote_id,account_id,character_index,character_incarnation,item_index,quantity,valuation_version,valuation_digest,gold_equivalent,fee_gold,consumed_atoms,policy_version,business_day,committed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$11,$13,$14,$15)",
        &[&f.realm,&key,&quote.quote_id,&f.owner.account_id,&f.owner.character_index,&f.owner.incarnation,
          &ITEM_INDEX,&(quote.quantity as i64),&quote.valuation_version,&quote.valuation_digest,
          &(equivalent as i64),&(fee as i64),&quote.policy_version,&day,&clock.database_time_ms]).unwrap();
    tx.execute("INSERT INTO hunting_material_conversion_results(realm_id,quote_id,operation_key,receipt_json,receipt_digest,before_revision,after_revision,committed_at_ms) VALUES($1,$2,$3,$4,$5,0,1,$6)",
        &[&f.realm,&quote.quote_id,&key,&data,&digest,&clock.database_time_ms]).unwrap();
    if !matches!(case, HistoryCase::MissingFee) {
        let amount = if matches!(case, HistoryCase::WrongFee) {
            fee - 1
        } else {
            fee
        };
        tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms) VALUES($1,$2,$3,'daily','material_conversion_fee',$3,$4,'gold-face-value-v1',$5,$6,$7)",
            &[&f.realm,&burn_event,&key,&(amount as i64),&quote.policy_version,&day,&clock.database_time_ms]).unwrap();
    }
    // Keep the total burn equal to the correct fee so corruption reaches the
    // fee/ledger identity checks instead of failing the earlier aggregate gate.
    let compensation = match case {
        HistoryCase::MissingFee => fee,
        HistoryCase::WrongFee => 1,
        _ => 0,
    };
    if compensation != 0 {
        append_burn(
            &mut tx,
            &f.realm,
            day,
            clock.database_time_ms,
            "synthetic-other-daily-burn",
            "synthetic-history",
            "daily",
            compensation as i64,
        );
    }
    tx.commit().unwrap();
}

fn count(client: &mut Client, realm: &str, table: &str) -> i64 {
    assert!(matches!(
        table,
        "hunting_economy_daily_metric_versions" | "hunting_economy_metric_exports"
    ));
    client
        .query_one(
            &format!("SELECT count(*)::bigint FROM {table} WHERE realm_id=$1"),
            &[&realm],
        )
        .unwrap()
        .get(0)
}

fn assert_coverage_null(client: &mut Client, realm: &str) {
    for row in client.query("SELECT is_complete,bound_gold_stock::text,unbound_gold_stock::text,
        raw_material_price_index::text,monthly_card_gold_per_card::text,active_account_total,
        heavy_producer_monthly_usd_equivalent::text FROM hunting_permit_daily_observations WHERE realm_id=$1", &[&realm]).unwrap() {
        assert!(!row.get::<_, bool>("is_complete"));
        for field in ["bound_gold_stock", "unbound_gold_stock", "raw_material_price_index",
            "monthly_card_gold_per_card", "heavy_producer_monthly_usd_equivalent"] {
            assert!(row.get::<_, Option<String>>(field).is_none(), "{field}");
        }
        assert!(row.get::<_, Option<i64>>("active_account_total").is_none());
    }
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_material_export_closed_history_accounts_permits_fee_without_coin_inflow() {
    let mut f = Fixture::new("closed");
    let quote = f.quote();
    assert_eq!(quote.quote.gold_equivalent, EQUIVALENT);
    assert_eq!(quote.quote.fee_unbound_gold, FEE);
    let first = f.mint_history(2);
    append_material_history(&mut f, &quote, first.business_day + 1, HistoryCase::Valid);
    let settled = settle_due(&mut f.client, &f.realm, 31).unwrap();
    assert_eq!(settled.len(), 1);
    let receipt = &settled[0];
    assert_eq!(receipt.consumed_atoms, EQUIVALENT);
    assert_eq!(receipt.material_conversion_gold_equivalent, EQUIVALENT);
    assert_eq!(receipt.material_conversion_fee_unbound_gold, FEE);
    assert_eq!(receipt.gross_conversion_gold, 0);
    assert_eq!(receipt.net_unbound_gold_inflow, 0);
    assert_eq!(receipt.burns.daily, FEE);
    assert_eq!(receipt.plan.destruction_gold_equivalent, FEE);
    assert_eq!(
        receipt.plan.opening_stock_atoms,
        first.plan.next_pool.stock_atoms - EQUIVALENT
    );
    assert_coverage_null(&mut f.client, &f.realm);
    let batch = export_daily_metrics(&mut f.client, &f.realm, 31).unwrap();
    assert_eq!(batch.receipts.len(), 2);
    let row = f.client.query_one("SELECT permit_consumed_atoms::text,material_conversion_gold_equivalent::text,
        material_conversion_fee_unbound_gold::text,gold_conversion_fee_gold::text,conversion_fee_gold::text,
        gross_converted_gold::text,net_unbound_inflow_gold::text,burn_daily_gold::text
        FROM hunting_permit_daily_observations WHERE realm_id=$1 AND economy_date=date '1970-01-01'+$2::integer",
        &[&f.realm, &(receipt.business_day as i32)]).unwrap();
    for field in [
        "permit_consumed_atoms",
        "material_conversion_gold_equivalent",
    ] {
        assert_eq!(row.get::<_, String>(field), "250");
    }
    for field in [
        "material_conversion_fee_unbound_gold",
        "conversion_fee_gold",
        "burn_daily_gold",
    ] {
        assert_eq!(row.get::<_, String>(field), "5");
    }
    for field in [
        "gold_conversion_fee_gold",
        "gross_converted_gold",
        "net_unbound_inflow_gold",
    ] {
        assert_eq!(row.get::<_, String>(field), "0");
    }
    assert_eq!(
        read_wallet(&mut f.client, &f.realm, &f.owner)
            .unwrap()
            .unwrap()
            .unbound_gold,
        INITIAL_UNBOUND - FEE
    );
    assert_coverage_null(&mut f.client, &f.realm);
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_material_export_current_api_consumption_reconciles_live_pool() {
    let mut f = Fixture::new("current");
    let closed = f.mint_history(1);
    let quote = f.quote();
    let owner = f.owner.clone();
    let confirmed = confirm_material_conversion_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &quote.quote_id,
        1000,
        5000,
        |tx, _| authorize_source(tx, &owner),
        |tx, _, quote, key| persist_source(tx, quote, key),
    )
    .unwrap();
    assert!(!confirmed.replayed);
    assert_eq!(
        confirmed.receipt.wallet_after.unbound_gold,
        INITIAL_UNBOUND - FEE
    );
    assert_eq!(confirmed.receipt.wallet_after.bound_gold, 0);
    assert_eq!(
        confirmed.receipt.pool_after_atoms,
        closed.plan.next_pool.stock_atoms - EQUIVALENT
    );
    assert_eq!(
        confirmed.receipt.source.checkpoint["item"]["server_note"],
        "complete-source-preserved"
    );
    let checkpoint: Value = f
        .client
        .query_one(
            "SELECT checkpoint FROM synthetic_material_export_sources",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(checkpoint, confirmed.receipt.source.checkpoint);
    let burn = f
        .client
        .query_one(
            "SELECT gold_equivalent,valuation_version FROM hunting_economy_burn_events
        WHERE realm_id=$1 AND source_kind='material_conversion_fee'",
            &[&f.realm],
        )
        .unwrap();
    assert_eq!(burn.get::<_, i64>("gold_equivalent"), FEE as i64);
    assert_eq!(
        burn.get::<_, String>("valuation_version"),
        "gold-face-value-v1"
    );
    let batch = export_daily_metrics(&mut f.client, &f.realm, 31).unwrap();
    assert_eq!(batch.receipts.len(), 1);
    assert_eq!(batch.receipts[0].business_day, closed.business_day);
    assert_eq!(closed.gross_conversion_gold, 0);
    assert_eq!(closed.net_unbound_gold_inflow, 0);
    let coin_conversions: i64 = f
        .client
        .query_one(
            "SELECT count(*)::bigint FROM hunting_economy_conversions WHERE realm_id=$1",
            &[&f.realm],
        )
        .unwrap()
        .get(0);
    assert_eq!(coin_conversions, 0);
    assert_eq!(
        read_settlement(&mut f.client, &f.realm, closed.business_day).unwrap(),
        Some(closed)
    );
    // Only the current open day's material debit explains this stock difference.
    assert_eq!(
        read_realm(&mut f.client, &f.realm)
            .unwrap()
            .pool
            .stock_atoms,
        confirmed.receipt.pool_after_atoms
    );
    assert_coverage_null(&mut f.client, &f.realm);
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_material_export_rejects_missing_orphan_and_wrong_fee_without_partial_publish() {
    for (label, case) in [
        ("missing", HistoryCase::MissingFee),
        ("orphan", HistoryCase::OrphanFee),
        ("wrong", HistoryCase::WrongFee),
    ] {
        let mut f = Fixture::new(label);
        let quote = f.quote();
        let first = f.mint_history(2);
        append_material_history(&mut f, &quote, first.business_day + 1, case);
        assert_eq!(settle_due(&mut f.client, &f.realm, 31).unwrap().len(), 1);
        assert_coverage_null(&mut f.client, &f.realm);
        assert_eq!(
            export_daily_metrics(&mut f.client, &f.realm, 31),
            Err(EconomyError::CorruptHistory("metric_material_fee_burn")),
            "{label}"
        );
        assert_eq!(
            count(
                &mut f.client,
                &f.realm,
                "hunting_economy_daily_metric_versions"
            ),
            0
        );
        assert_eq!(
            count(&mut f.client, &f.realm, "hunting_economy_metric_exports"),
            0
        );
        let pending: i64 = f
            .client
            .query_one(
                "SELECT count(*)::bigint FROM hunting_economy_outbox
            WHERE realm_id=$1 AND event_kind='daily_settlement' AND delivered_at_ms IS NULL",
                &[&f.realm],
            )
            .unwrap()
            .get(0);
        assert_eq!(pending, 2, "{label}");
        assert_coverage_null(&mut f.client, &f.realm);
    }
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_material_export_legacy_payload_zero_semantics_and_v2_ack_preserve_v1_history() {
    let mut f = Fixture::new("legacy");
    let settled = f.mint_history(1);
    let original = f.client.query_one("SELECT receipt_json,receipt_json::text AS bytes FROM hunting_economy_settlements WHERE realm_id=$1", &[&f.realm]).unwrap();
    let original_json: Value = original.get("receipt_json");
    let original_bytes: String = original.get("bytes");
    assert!(!original_json
        .as_object()
        .unwrap()
        .contains_key("material_conversion_gold_equivalent"));
    assert!(!original_json
        .as_object()
        .unwrap()
        .contains_key("material_conversion_fee_unbound_gold"));
    assert_eq!(serde_json::to_value(&settled).unwrap(), original_json);
    let original_digest = fingerprint(&original_json);
    let unknown = f
        .client
        .query_one(
            "SELECT material_conversion_gold_equivalent::text,
        material_conversion_fee_unbound_gold::text,conversion_fee_gold::text
        FROM hunting_permit_daily_observations WHERE realm_id=$1",
            &[&f.realm],
        )
        .unwrap();
    for field in [
        "material_conversion_gold_equivalent",
        "material_conversion_fee_unbound_gold",
        "conversion_fee_gold",
    ] {
        assert!(unknown.get::<_, Option<String>>(field).is_none());
    }
    let event = f
        .client
        .query_one(
            "SELECT event_id,payload_json FROM hunting_economy_outbox
        WHERE realm_id=$1 AND event_kind='daily_settlement'",
            &[&f.realm],
        )
        .unwrap();
    let event_id: String = event.get("event_id");
    assert_eq!(event.get::<_, Value>("payload_json"), original_json);
    // Append a v1-compatible exporter fixture; this is not execution of an old
    // binary. The current v2 exporter must independently validate it and append
    // its own immutable ACK, retaining the exact source and first delivery time.
    f.client.execute("INSERT INTO hunting_economy_daily_metric_versions(realm_id,business_day,metric_revision,
        metric_source,metric_evidence,created_at_ms) VALUES($1,$2,1,$3,$4,$5)",
        &[&f.realm,&settled.business_day,&OLD_CONSUMER,&json!({"classification":"synthetic-v1-exporter-fixture"}),&settled.committed_at_ms]).unwrap();
    f.client.execute("INSERT INTO hunting_economy_metric_exports(realm_id,event_id,consumer_version,business_day,
        metric_revision,source_payload_digest,disposition,exported_at_ms) VALUES($1,$2,$3,$4,1,$5,'inserted_partial',$6)",
        &[&f.realm,&event_id,&OLD_CONSUMER,&settled.business_day,&original_digest,&settled.committed_at_ms]).unwrap();
    acknowledge_outbox(&mut f.client, &f.realm, &event_id).unwrap();
    let old_ack: i64 = f
        .client
        .query_one(
            "SELECT delivered_at_ms FROM hunting_economy_outbox WHERE realm_id=$1 AND event_id=$2",
            &[&f.realm, &event_id],
        )
        .unwrap()
        .get(0);
    let v1_zeros = f.client.query_one("SELECT material_conversion_gold_equivalent::text,
        material_conversion_fee_unbound_gold::text FROM hunting_permit_daily_observations WHERE realm_id=$1",
        &[&f.realm]).unwrap();
    assert_eq!(
        v1_zeros.get::<_, String>("material_conversion_gold_equivalent"),
        "0"
    );
    assert_eq!(
        v1_zeros.get::<_, String>("material_conversion_fee_unbound_gold"),
        "0"
    );
    let batch = export_daily_metrics(&mut f.client, &f.realm, 31).unwrap();
    assert_eq!(batch.receipts.len(), 1);
    assert_eq!(
        batch.receipts[0].consumer_version,
        DAILY_METRIC_CONSUMER_VERSION
    );
    assert_eq!(batch.receipts[0].source_payload_digest, original_digest);
    assert_eq!(
        batch.receipts[0].disposition,
        MetricExportDisposition::ReusedExisting
    );
    assert_eq!(
        count(&mut f.client, &f.realm, "hunting_economy_metric_exports"),
        2
    );
    let v1_digest: String = f
        .client
        .query_one(
            "SELECT source_payload_digest FROM hunting_economy_metric_exports
        WHERE realm_id=$1 AND event_id=$2 AND consumer_version=$3",
            &[&f.realm, &event_id, &OLD_CONSUMER],
        )
        .unwrap()
        .get(0);
    assert_eq!(v1_digest, original_digest);
    assert_eq!(
        count(
            &mut f.client,
            &f.realm,
            "hunting_economy_daily_metric_versions"
        ),
        1
    );
    let row = f
        .client
        .query_one(
            "SELECT s.receipt_json,s.receipt_json::text AS bytes,e.payload_json,e.delivered_at_ms
        FROM hunting_economy_settlements s JOIN hunting_economy_outbox e
        ON e.realm_id=s.realm_id AND e.event_id=$2 WHERE s.realm_id=$1",
            &[&f.realm, &event_id],
        )
        .unwrap();
    assert_eq!(row.get::<_, String>("bytes"), original_bytes);
    assert_eq!(row.get::<_, Value>("receipt_json"), original_json);
    assert_eq!(
        fingerprint(&row.get::<_, Value>("payload_json")),
        original_digest
    );
    assert_eq!(row.get::<_, i64>("delivered_at_ms"), old_ack);
    let zeros = f.client.query_one("SELECT material_conversion_gold_equivalent::text,
        material_conversion_fee_unbound_gold::text,conversion_fee_gold::text,gold_conversion_fee_gold::text
        FROM hunting_permit_daily_observations WHERE realm_id=$1", &[&f.realm]).unwrap();
    for field in [
        "material_conversion_gold_equivalent",
        "material_conversion_fee_unbound_gold",
        "conversion_fee_gold",
        "gold_conversion_fee_gold",
    ] {
        assert_eq!(zeros.get::<_, String>(field), "0");
    }
    let statement = f
        .client
        .prepare("SELECT * FROM hunting_permit_daily_observations LIMIT 0")
        .unwrap();
    assert_eq!(statement.columns().len(), 44);
    assert_eq!(
        statement.columns()[41].name(),
        "material_conversion_gold_equivalent"
    );
    assert_eq!(
        statement.columns()[42].name(),
        "material_conversion_fee_unbound_gold"
    );
    assert_eq!(statement.columns()[43].name(), "gold_conversion_fee_gold");
    assert!(export_daily_metrics(&mut f.client, &f.realm, 31)
        .unwrap()
        .receipts
        .is_empty());
    assert_coverage_null(&mut f.client, &f.realm);
}

fn exact_ratio_fixture(label: &str) -> (Fixture, Policy) {
    let mut f = Fixture::new(label);
    let mut exact = policy();
    exact.policy_version = "synthetic-exact-fee-v1".into();
    exact.conversion_fee = Rational {
        numerator: 12_000_000_000_000_000_001,
        denominator: 18_000_000_000_000_000_001,
    };
    register_policy(&mut f.client, &f.realm, &exact).unwrap();
    activate_policy(&mut f.client, &f.realm, &exact.policy_version, false).unwrap();
    let prices = MaterialValuationSet::new(
        "synthetic-exact-prices-v1",
        vec![MaterialValuationEntry {
            item_index: ITEM_INDEX,
            unit_gold: 1,
        }],
    )
    .unwrap();
    register_material_valuation(&mut f.client, &f.realm, &prices).unwrap();
    activate_material_valuation(&mut f.client, &f.realm, prices.valuation_version()).unwrap();
    // Before either quote, seed a different synthetic canonical source count.
    f.client
        .execute(
            "UPDATE synthetic_material_export_sources SET checkpoint=
        jsonb_set(checkpoint,'{item,count}','3'::jsonb)",
            &[],
        )
        .unwrap();
    credit_bound_with_source(
        &mut f.client,
        &f.realm,
        &f.owner,
        "synthetic-exact-gold-funding",
        "explicit-synthetic-adapter",
        3,
        |_, _| Ok(()),
    )
    .unwrap();
    (f, exact)
}

fn append_exact_gold_history(f: &mut Fixture, exact: &Policy, day: i64) {
    let quote = mir2_economy_policy::quote_conversion(exact, 3).unwrap();
    assert_eq!(quote.fee_unbound_gold, 3);
    assert_eq!(quote.net_unbound_gold_credit, 0);
    let mut tx = f.client.transaction().unwrap();
    let clock = lock_realm_and_read_database_clock(&mut tx, &f.realm).unwrap();
    tx.execute(
        "UPDATE hunting_economy_pools SET stock_atoms=stock_atoms-3 WHERE realm_id=$1",
        &[&f.realm],
    )
    .unwrap();
    tx.execute("UPDATE hunting_economy_wallets SET bound_gold=bound_gold-3,revision=revision+1 WHERE realm_id=$1",
        &[&f.realm]).unwrap();
    let pool: i64 = tx
        .query_one(
            "SELECT stock_atoms FROM hunting_economy_pools WHERE realm_id=$1",
            &[&f.realm],
        )
        .unwrap()
        .get(0);
    let wallet = tx.query_one("SELECT bound_gold,unbound_gold,revision FROM hunting_economy_wallets WHERE realm_id=$1",
        &[&f.realm]).unwrap();
    let key = "synthetic-exact-gold-history";
    let request = fingerprint(&json!(["append-only-synthetic-exact-gold", day]));
    let receipt = ConversionReceipt {
        realm_id: f.realm.clone(),
        operation_key: key.into(),
        request_digest: request.clone(),
        owner: f.owner.clone(),
        request_id: key.into(),
        policy_version: exact.policy_version.clone(),
        business_day: day,
        committed_at_ms: clock.database_time_ms,
        quote,
        wallet_after: WalletBalance {
            bound_gold: wallet.get::<_, i64>("bound_gold") as u64,
            unbound_gold: wallet.get::<_, i64>("unbound_gold") as u64,
            revision: wallet.get::<_, i64>("revision") as u64,
        },
        pool_after_atoms: pool as u64,
    };
    let data = serde_json::to_value(&receipt).unwrap();
    tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,
        receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,$2,'conversion',$3,$4,$5,$6,$7)",
        &[&f.realm,&key,&request,&data,&fingerprint(&data),&day,&clock.database_time_ms]).unwrap();
    tx.execute("INSERT INTO hunting_economy_conversions(realm_id,operation_key,account_id,character_index,
        character_incarnation,gross_gold,fee_gold,net_gold,consumed_atoms,business_day,committed_at_ms)
        VALUES($1,$2,$3,$4,$5,3,3,0,3,$6,$7)",
        &[&f.realm,&key,&f.owner.account_id,&f.owner.character_index,&f.owner.incarnation,&day,&clock.database_time_ms]).unwrap();
    tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,
        gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms)
        VALUES($1,'synthetic-exact-gold-fee',$2,'daily','conversion_fee',$2,3,'gold-face-value-v1',$3,$4,$5)",
        &[&f.realm,&key,&exact.policy_version,&day,&clock.database_time_ms]).unwrap();
    tx.commit().unwrap();
}

#[test]
#[ignore = "requires explicit local PostgreSQL acceptance; run test-support --ignored"]
fn postgres_material_export_exact_ceiling_preserves_small_remainders_with_large_valid_ratio() {
    // 3*n = 2*d+1, so both genuine public quotes must charge 3, not 2.
    // Ordinary PG numeric / rounds the positive remainder away before ceil.
    // The first schema proves real current-day API behavior and pool deduction.
    let (mut current, exact) = exact_ratio_fixture("exact-current");
    let closed = current.mint_history(1);
    let gold = convert(
        &mut current.client,
        &current.realm,
        &current.owner,
        "synthetic-exact-current-gold",
        3,
    )
    .unwrap();
    assert_eq!(gold.receipt.quote.fee_unbound_gold, 3);
    let quote = current.quote_quantity("synthetic-exact-current-material", 3);
    assert_eq!(quote.quote.gold_equivalent, 3);
    assert_eq!(quote.quote.fee_unbound_gold, 3);
    let owner = current.owner.clone();
    let confirmed = confirm_material_conversion_authorized_with_source_with_timeouts(
        &mut current.client,
        &current.realm,
        &quote.quote_id,
        1000,
        5000,
        |tx, _| authorize_source(tx, &owner),
        |tx, _, quote, key| persist_source(tx, quote, key),
    )
    .unwrap();
    assert_eq!(
        confirmed.receipt.wallet_after.unbound_gold,
        INITIAL_UNBOUND - 3
    );
    assert_eq!(
        confirmed.receipt.pool_after_atoms,
        closed.plan.next_pool.stock_atoms - 6
    );
    assert_eq!(
        export_daily_metrics(&mut current.client, &current.realm, 31)
            .unwrap()
            .receipts
            .len(),
        1
    );

    // A separate schema appends explicitly synthetic CLOSED-day counterparts;
    // no clock is moved and no actual confirmation is relabelled or rewritten.
    // The real daily exporter must accept both exact fee SQL predicates.
    let (mut historical, historical_policy) = exact_ratio_fixture("exact-history");
    assert_eq!(historical_policy, exact);
    let quote = historical.quote_quantity("synthetic-exact-history-material", 3);
    let first = historical.mint_history(2);
    let day = first.business_day + 1;
    append_material_history(&mut historical, &quote, day, HistoryCase::Valid);
    append_exact_gold_history(&mut historical, &historical_policy, day);
    let settled = settle_due(&mut historical.client, &historical.realm, 31).unwrap();
    assert_eq!(settled.len(), 1);
    let receipt = &settled[0];
    assert_eq!(receipt.consumed_atoms, 6);
    assert_eq!(receipt.gross_conversion_gold, 3);
    assert_eq!(receipt.net_unbound_gold_inflow, 0);
    assert_eq!(receipt.material_conversion_gold_equivalent, 3);
    assert_eq!(receipt.material_conversion_fee_unbound_gold, 3);
    assert_eq!(receipt.burns.daily, 6);
    assert_eq!(
        export_daily_metrics(&mut historical.client, &historical.realm, 31)
            .unwrap()
            .receipts
            .len(),
        2
    );
    let row = historical.client.query_one("SELECT conversion_fee_gold::text,gold_conversion_fee_gold::text,
        material_conversion_fee_unbound_gold::text,net_unbound_inflow_gold::text
        FROM hunting_permit_daily_observations WHERE realm_id=$1 AND economy_date=date '1970-01-01'+$2::integer",
        &[&historical.realm,&(day as i32)]).unwrap();
    assert_eq!(row.get::<_, String>("conversion_fee_gold"), "6");
    assert_eq!(row.get::<_, String>("gold_conversion_fee_gold"), "3");
    assert_eq!(
        row.get::<_, String>("material_conversion_fee_unbound_gold"),
        "3"
    );
    assert_eq!(row.get::<_, String>("net_unbound_inflow_gold"), "0");
    assert_coverage_null(&mut current.client, &current.realm);
    assert_coverage_null(&mut historical.client, &historical.realm);
}

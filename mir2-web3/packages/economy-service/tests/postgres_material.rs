//! Ignored real-PostgreSQL service acceptance in a fresh retained schema per test.
//! The source adapter below is deliberately synthetic: it locks its own actor
//! table and CASes a complete JSON checkpoint. It is NOT a game ItemState,
//! actual player/quest source, gateway, Windows UI, or multiplayer acceptance.
//! Historical fixture destruction initializes permits via production settlement;
//! current material appraisal is never entered as destruction. Existing unbound
//! fee funds come only from a separate source credit and real coin conversion.
#![cfg(feature = "test-support")]

use mir2_economy_policy::{MaterialValuationEntry, MaterialValuationSet, Policy, Rational};
use mir2_economy_service::*;
use postgres::{Client, NoTls, Transaction};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Barrier,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

const ITEM: i32 = 700_001;
const LOCK_MS: u32 = 10_000;
const STATEMENT_MS: u32 = 25_000;
const FUND_GROSS: u64 = 100_000;

fn pg(error: postgres::Error) -> EconomyError {
    EconomyError::Database {
        sqlstate: error.code().map(|code| code.code().into()),
    }
}
fn policy() -> Policy {
    Policy {
        schema_version: 1,
        policy_version: "material-acceptance-v1".into(),
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
        baseline_daily_mint_atoms: 720_000,
        daily_mint_cap_multiplier: Rational {
            numerator: 3,
            denominator: 2,
        },
        business_timezone: "Asia/Shanghai".into(),
        settlement_hour: 4,
    }
}
fn raw_hash(value: &Value) -> String {
    Sha256::digest(serde_json::to_vec(value).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn item_digest(value: &Value) -> String {
    format!("sha256:{}", raw_hash(value))
}
fn binding_digest(value: &Value) -> String {
    raw_hash(&json!(["synthetic-material-binding-v1", value]))
}
fn valuations(version: &str, unit_gold: u64) -> MaterialValuationSet {
    MaterialValuationSet::new(
        version,
        vec![MaterialValuationEntry {
            item_index: ITEM,
            unit_gold,
        }],
    )
    .unwrap()
}

struct Fixture {
    client: Client,
    connection: postgres::Config,
    realm: String,
}
impl Fixture {
    fn new(label: &str) -> Self {
        let raw = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
            .expect("ignored material acceptance requires MIR2_ECONOMY_TEST_DATABASE_URL");
        let mut connection: postgres::Config = raw
            .parse()
            .unwrap_or_else(|_| panic!("invalid isolated acceptance configuration"));
        assert!(connection
            .get_dbname()
            .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
        assert!(!connection.get_hosts().is_empty() && connection.get_hosts().iter().all(|host|
            matches!(host, postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost")
                || name.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()))));
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
        let schema = format!("hmat_{nonce}");
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
            .batch_execute(MATERIAL_CONVERSION_MIGRATION_SQL)
            .unwrap();
        client.batch_execute("CREATE TABLE material_fixture_actors (
            actor_id INTEGER PRIMARY KEY, account_id TEXT NOT NULL, character_index INTEGER NOT NULL,
            incarnation TEXT NOT NULL, allowed BOOLEAN NOT NULL DEFAULT true,
            UNIQUE(account_id,character_index,incarnation));
            CREATE TABLE material_fixture_sources (
            account_id TEXT NOT NULL, character_index INTEGER NOT NULL, incarnation TEXT NOT NULL,
            revision BIGINT NOT NULL CHECK(revision>=0), capacity INTEGER NOT NULL CHECK(capacity>0), checkpoint JSONB NOT NULL,
            source_calls BIGINT NOT NULL DEFAULT 0, saved_at_ms BIGINT, business_day BIGINT,
            PRIMARY KEY(account_id,character_index,incarnation));
            CREATE TABLE material_fixture_funding (
            actor_id INTEGER PRIMARY KEY REFERENCES material_fixture_actors(actor_id),
            amount BIGINT NOT NULL, consumed BOOLEAN NOT NULL DEFAULT false);").unwrap();
        let realm = format!("material-fixture-{label}-{nonce}");
        initialize_realm(&mut client, &realm, &policy()).unwrap();
        seed_historical_destruction(&mut client, &realm);
        let mut fixture = Self {
            client,
            connection,
            realm,
        };
        fixture.add_actor(0, true);
        fixture.set_prices("fixed-v1", 500);
        fixture
    }
    fn owner(&mut self, actor: i32) -> WalletOwner {
        let row = self.client.query_one("SELECT account_id,character_index,incarnation FROM material_fixture_actors WHERE actor_id=$1", &[&actor]).unwrap();
        WalletOwner {
            account_id: row.get(0),
            character_index: row.get(1),
            incarnation: row.get(2),
        }
    }
    fn add_actor(&mut self, actor: i32, funded: bool) {
        let owner = WalletOwner {
            account_id: format!("material-player-{actor}"),
            character_index: 0,
            incarnation: format!("pg-character-v1:11111111-1111-4111-8111-{actor:012}"),
        };
        self.client.execute("INSERT INTO material_fixture_actors(actor_id,account_id,character_index,incarnation) VALUES($1,$2,$3,$4)",
            &[&actor,&owner.account_id,&owner.character_index,&owner.incarnation]).unwrap();
        let uid = 10_001_u64 + actor as u64 * 100;
        let checkpoint = json!({"syntheticAdapterOnly":true,"revision":1,"legacyGold":1234,
            "items":[{"uniqueId":uid,"itemIndex":ITEM,"quantity":10,
                "bound":{"realm":self.realm,"account":owner.account_id,"index":owner.character_index,
                    "incarnation":owner.incarnation,"source":"fixture-existing-production-v1"}}]});
        self.client.execute("INSERT INTO material_fixture_sources(account_id,character_index,incarnation,revision,capacity,checkpoint) VALUES($1,$2,$3,1,20,$4)",
            &[&owner.account_id,&owner.character_index,&owner.incarnation,&checkpoint]).unwrap();
        initialize_wallet(&mut self.client, &self.realm, &owner).unwrap();
        if funded {
            self.client
                .execute(
                    "INSERT INTO material_fixture_funding(actor_id,amount) VALUES($1,200000)",
                    &[&actor],
                )
                .unwrap();
            credit_bound_authorized_with_source_with_timeouts(&mut self.client, &self.realm,
                "fixture-existing-credit", "fixture-funding-v1", 200_000, LOCK_MS, STATEMENT_MS,
                |tx,_| authorize(tx,actor), |tx,_| {
                    if tx.execute("UPDATE material_fixture_funding SET consumed=true WHERE actor_id=$1 AND amount=200000 AND NOT consumed", &[&actor]).map_err(pg)? != 1 {
                        return Err(EconomyError::SourceRejected);
                    }
                    Ok(())
                }).unwrap();
            let converted = convert_authorized_with_timeouts(
                &mut self.client,
                &self.realm,
                "fixture-fee-funding",
                FUND_GROSS,
                LOCK_MS,
                STATEMENT_MS,
                |tx, _| authorize(tx, actor),
            )
            .unwrap();
            assert_eq!(converted.receipt.wallet_after.bound_gold, 100_000);
            assert_eq!(converted.receipt.wallet_after.unbound_gold, 98_000);
        }
    }
    fn set_prices(&mut self, version: &str, unit_gold: u64) {
        let set = valuations(version, unit_gold);
        register_material_valuation(&mut self.client, &self.realm, &set).unwrap();
        activate_material_valuation(&mut self.client, &self.realm, version).unwrap();
    }
    fn quote(
        &mut self,
        actor: i32,
        request: &str,
        quantity: u64,
    ) -> EconomyResult<Idempotent<MaterialConversionQuoteReceipt>> {
        let uid = 10_001_u64 + actor as u64 * 100;
        let realm = self.realm.clone();
        quote_material_conversion_authorized_with_source_with_timeouts(
            &mut self.client,
            &self.realm,
            request,
            uid,
            quantity,
            LOCK_MS,
            STATEMENT_MS,
            |tx, _| authorize(tx, actor),
            |tx, _, owner| quote_source(tx, owner, &realm, uid, quantity),
        )
    }
    fn confirm(
        &mut self,
        actor: i32,
        id: &str,
    ) -> EconomyResult<Idempotent<MaterialConversionReceipt>> {
        confirm_on(&mut self.client, &self.realm, actor, id)
    }
    fn snapshot(&mut self) -> Value {
        snapshot(&mut self.client)
    }
}

/// Test-only history bootstrap before any wallet funding/material quote. No
/// permit stock, OS/DB clock or immutable trigger is modified. Only the mutable
/// fresh realm cursor is initialized to expose one real closed-day settlement.
fn seed_historical_destruction(client: &mut Client, realm: &str) {
    let mut tx = client.transaction().unwrap();
    let clock = lock_realm_and_read_database_clock(&mut tx, realm).unwrap();
    let day = clock.business_day - 1;
    tx.execute(
        "UPDATE hunting_economy_pools SET genesis_day=$2,settled_through_day=$3 WHERE realm_id=$1",
        &[&realm, &day, &(day - 1)],
    )
    .unwrap();
    let key = "synthetic-closed-destruction-only";
    tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,$2,'burn','fixture',$3,'fixture',$4,$5)",
        &[&realm,&key,&json!({"historicalFixtureOnly":true}),&day,&clock.database_time_ms]).unwrap();
    tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms) VALUES($1,$2,$2,'siege','fixture-historical',$2,10800000,'fixture-history','material-acceptance-v1',$3,$4)",
        &[&realm,&key,&day,&clock.database_time_ms]).unwrap();
    tx.commit().unwrap();
    let settled = settle_due(client, realm, 30).unwrap();
    let historical = settled
        .iter()
        .find(|receipt| receipt.business_day == day)
        .expect("the seeded closed day settles normally");
    assert_eq!(historical.plan.minted_atoms, 1_080_000);
    assert_eq!(historical.plan.next_pool.stock_atoms, 1_072_800);
}

fn authorize(tx: &mut Transaction<'_>, actor: i32) -> EconomyResult<WalletOwner> {
    let row = tx.query_opt("SELECT account_id,character_index,incarnation,allowed FROM material_fixture_actors WHERE actor_id=$1 FOR UPDATE", &[&actor]).map_err(pg)?
        .ok_or(EconomyError::Unauthenticated)?;
    if !row.get::<_, bool>("allowed") {
        return Err(EconomyError::AccountUnavailable);
    }
    Ok(WalletOwner {
        account_id: row.get("account_id"),
        character_index: row.get("character_index"),
        incarnation: row.get("incarnation"),
    })
}
fn source_row(tx: &mut Transaction<'_>, owner: &WalletOwner) -> EconomyResult<postgres::Row> {
    tx.query_opt("SELECT revision,capacity,checkpoint FROM material_fixture_sources WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 FOR UPDATE",
        &[&owner.account_id,&owner.character_index,&owner.incarnation]).map_err(pg)?.ok_or(EconomyError::SourceRejected)
}
fn source_item<'a>(
    checkpoint: &'a Value,
    owner: &WalletOwner,
    uid: u64,
) -> EconomyResult<&'a Value> {
    let items = checkpoint["items"]
        .as_array()
        .ok_or(EconomyError::SourceRejected)?;
    let found: Vec<_> = items
        .iter()
        .filter(|item| item["uniqueId"].as_u64() == Some(uid))
        .collect();
    if found.len() != 1 {
        return Err(EconomyError::SourceRejected);
    }
    let item = found[0];
    if item.get("converted").is_some()
        || !item["bound"].is_object()
        || item["bound"]["account"] != owner.account_id
        || item["bound"]["index"] != owner.character_index
        || item["bound"]["incarnation"] != owner.incarnation
    {
        return Err(EconomyError::SourceRejected);
    }
    Ok(item)
}
fn quote_source(
    tx: &mut Transaction<'_>,
    owner: &WalletOwner,
    realm: &str,
    uid: u64,
    quantity: u64,
) -> EconomyResult<MaterialConversionQuoteSource> {
    let row = source_row(tx, owner)?;
    let checkpoint: Value = row.get("checkpoint");
    let item = source_item(&checkpoint, owner, uid)?;
    if item["bound"]["realm"] != realm
        || checkpoint["revision"].as_u64() != Some(row.get::<_, i64>("revision") as u64)
    {
        return Err(EconomyError::SourceRejected);
    }
    let count = item["quantity"]
        .as_u64()
        .ok_or(EconomyError::SourceRejected)?;
    if quantity == 0 || quantity > count {
        return Err(EconomyError::SourceRejected);
    }
    Ok(MaterialConversionQuoteSource {
        item_index: item["itemIndex"]
            .as_i64()
            .and_then(|index| i32::try_from(index).ok())
            .ok_or(EconomyError::SourceRejected)?,
        input_unique_id: uid,
        input_count: count,
        quantity,
        input_save_revision: row.get::<_, i64>("revision") as u64,
        input_item_digest: item_digest(item),
        source_binding_digest: binding_digest(&item["bound"]),
    })
}
fn apply_source(
    tx: &mut Transaction<'_>,
    clock: SourceContext,
    quote: &MaterialConversionQuoteReceipt,
    operation: &str,
) -> EconomyResult<MaterialConversionSourceResult> {
    let row = source_row(tx, &quote.owner)?;
    let before = row.get::<_, i64>("revision") as u64;
    let mut checkpoint: Value = row.get("checkpoint");
    let original = source_item(&checkpoint, &quote.owner, quote.input_unique_id)?.clone();
    if before != quote.input_save_revision
        || checkpoint["revision"].as_u64() != Some(before)
        || item_digest(&original) != quote.input_item_digest
        || binding_digest(&original["bound"]) != quote.source_binding_digest
        || original["bound"]["realm"] != quote.realm_id
        || original["quantity"].as_u64() != Some(quote.input_count)
    {
        return Err(EconomyError::SourceRejected);
    }
    let partial = quote.quantity < quote.input_count;
    let items = checkpoint["items"]
        .as_array_mut()
        .ok_or(EconomyError::SourceRejected)?;
    if partial && items.len() + 1 > row.get::<_, i32>("capacity") as usize {
        return Err(EconomyError::SourceRejected);
    }
    let index = items
        .iter()
        .position(|item| item["uniqueId"].as_u64() == Some(quote.input_unique_id))
        .ok_or(EconomyError::SourceRejected)?;
    let output_uid = if partial {
        quote
            .input_unique_id
            .checked_add(1_000_000)
            .ok_or(EconomyError::SourceRejected)?
    } else {
        quote.input_unique_id
    };
    if partial
        && items
            .iter()
            .any(|item| item["uniqueId"].as_u64() == Some(output_uid))
    {
        return Err(EconomyError::SourceRejected);
    }
    let mut output = original;
    output["uniqueId"] = json!(output_uid);
    output["quantity"] = json!(quote.quantity);
    output.as_object_mut().unwrap().remove("bound");
    output["converted"] = json!({"operation":operation,"quoteId":quote.quote_id,"valuationVersion":quote.valuation_version,
        "valuationDigest":quote.valuation_digest,"sourceBindingDigest":quote.source_binding_digest,"inputUniqueId":quote.input_unique_id});
    if partial {
        items[index]["quantity"] = json!(quote.input_count - quote.quantity);
        items.push(output);
    } else {
        items[index] = output;
    }
    let after = before.checked_add(1).ok_or(EconomyError::SourceRejected)?;
    checkpoint["revision"] = json!(after);
    if tx.execute("UPDATE material_fixture_sources SET revision=$4,checkpoint=$5,source_calls=source_calls+1,saved_at_ms=$6,business_day=$7 WHERE account_id=$1 AND character_index=$2 AND incarnation=$3 AND revision=$8",
        &[&quote.owner.account_id,&quote.owner.character_index,&quote.owner.incarnation,&(after as i64),&checkpoint,&clock.database_time_ms,&clock.business_day,&(before as i64)]).map_err(pg)? != 1 {
        return Err(EconomyError::SourceRejected);
    }
    Ok(MaterialConversionSourceResult {
        before_revision: before,
        after_revision: after,
        input_unique_id: quote.input_unique_id,
        output_unique_id: output_uid,
        checkpoint,
    })
}
fn confirm_on(
    client: &mut Client,
    realm: &str,
    actor: i32,
    id: &str,
) -> EconomyResult<Idempotent<MaterialConversionReceipt>> {
    confirm_material_conversion_authorized_with_source_with_timeouts(
        client,
        realm,
        id,
        LOCK_MS,
        STATEMENT_MS,
        |tx, _| authorize(tx, actor),
        apply_source,
    )
}
fn query(
    fixture: &mut Fixture,
    actor: i32,
    id: &str,
) -> EconomyResult<Option<MaterialConversionReceipt>> {
    query_material_conversion_authorized_with_timeouts(
        &mut fixture.client,
        &fixture.realm,
        id,
        LOCK_MS,
        STATEMENT_MS,
        |tx, _| authorize(tx, actor),
    )
}
fn snapshot(client: &mut Client) -> Value {
    let mut all = serde_json::Map::new();
    for table in [
        "material_fixture_actors",
        "material_fixture_sources",
        "material_fixture_funding",
        "hunting_economy_policies",
        "hunting_economy_pools",
        "hunting_economy_wallets",
        "hunting_economy_operations",
        "hunting_economy_burn_events",
        "hunting_economy_conversions",
        "hunting_economy_material_conversions",
        "hunting_economy_outbox",
        "hunting_economy_settlements",
        "hunting_material_valuations",
        "hunting_material_active_valuations",
        "hunting_material_conversion_quotes",
        "hunting_material_conversion_results",
    ] {
        let value: Value = client.query_one(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]'::jsonb) FROM {table} t"),&[]).unwrap().get(0);
        all.insert(table.into(), value);
    }
    Value::Object(all)
}
fn count(client: &mut Client, table: &str) -> i64 {
    client
        .query_one(&format!("SELECT count(*)::bigint FROM {table}"), &[])
        .unwrap()
        .get(0)
}
fn source_calls(client: &mut Client) -> i64 {
    client
        .query_one(
            "SELECT COALESCE(sum(source_calls),0)::bigint FROM material_fixture_sources",
            &[],
        )
        .unwrap()
        .get(0)
}

#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_valuation_registration_empty_and_missing_are_fail_closed() {
    let mut f = Fixture::new("valuation");
    let set = valuations("fixed-v1", 500);
    let before = f.snapshot();
    let hash = register_material_valuation(&mut f.client, &f.realm, &set).unwrap();
    assert!(hash.starts_with("sha256:"));
    assert_eq!(f.snapshot(), before);
    assert!(matches!(
        register_material_valuation(&mut f.client, &f.realm, &valuations("fixed-v1", 501)),
        Err(EconomyError::PolicyConflict)
    ));
    assert_eq!(f.snapshot(), before);
    let empty = MaterialValuationSet::new("empty-v1", vec![]).unwrap();
    register_material_valuation(&mut f.client, &f.realm, &empty).unwrap();
    activate_material_valuation(&mut f.client, &f.realm, "empty-v1").unwrap();
    let before = f.snapshot();
    assert!(matches!(
        f.quote(0, "empty", 1),
        Err(EconomyError::MaterialValuationUnavailable)
    ));
    assert!(matches!(
        activate_material_valuation(&mut f.client, &f.realm, "missing"),
        Err(EconomyError::MaterialValuationUnavailable)
    ));
    assert_eq!(f.snapshot(), before);
    let missing = MaterialValuationSet::new(
        "other-item-v1",
        vec![MaterialValuationEntry {
            item_index: ITEM + 1,
            unit_gold: 500,
        }],
    )
    .unwrap();
    register_material_valuation(&mut f.client, &f.realm, &missing).unwrap();
    activate_material_valuation(&mut f.client, &f.realm, "other-item-v1").unwrap();
    let before = f.snapshot();
    assert!(matches!(
        f.quote(0, "missing-item", 1),
        Err(EconomyError::MaterialValuationUnavailable)
    ));
    assert_eq!(f.snapshot(), before);
}

#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_quote_retry_conflict_and_current_actor_authorization() {
    let mut f = Fixture::new("quote");
    f.add_actor(1, false);
    let quote = f.quote(0, "original", 3).unwrap();
    assert!(!quote.replayed);
    let before = f.snapshot();
    let duplicate = quote_material_conversion_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        "original",
        10_001,
        3,
        LOCK_MS,
        STATEMENT_MS,
        |tx, _| authorize(tx, 0),
        |_, _, _| panic!("original quote replay must not reconstruct its source"),
    )
    .unwrap();
    assert!(duplicate.replayed);
    assert_eq!(duplicate.receipt, quote.receipt);
    assert!(matches!(
        f.quote(0, "original", 2),
        Err(EconomyError::RequestConflict)
    ));
    assert!(matches!(
        quote_material_conversion_authorized_with_source_with_timeouts(
            &mut f.client,
            &f.realm,
            "original",
            10_002,
            3,
            LOCK_MS,
            STATEMENT_MS,
            |tx, _| authorize(tx, 0),
            |_, _, _| panic!("payload conflict precedes source")
        ),
        Err(EconomyError::RequestConflict)
    ));
    assert!(matches!(
        query(&mut f, 1, &quote.receipt.quote_id),
        Err(EconomyError::CharacterNotFound)
    ));
    assert!(query(&mut f, 0, &quote.receipt.quote_id).unwrap().is_none());
    assert_eq!(f.snapshot(), before);
    f.client
        .execute(
            "UPDATE material_fixture_actors SET allowed=false WHERE actor_id=0",
            &[],
        )
        .unwrap();
    let before = f.snapshot();
    assert!(matches!(
        f.confirm(0, &quote.receipt.quote_id),
        Err(EconomyError::AccountUnavailable)
    ));
    assert!(matches!(
        query(&mut f, 0, &quote.receipt.quote_id),
        Err(EconomyError::AccountUnavailable)
    ));
    assert_eq!(f.snapshot(), before);
}

#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_valuation_version_and_aba_invalidate_only_new_confirmation() {
    let mut f = Fixture::new("aba");
    let quote = f.quote(0, "old-valuation", 3).unwrap().receipt;
    f.set_prices("fixed-v2", 600);
    activate_material_valuation(&mut f.client, &f.realm, "fixed-v1").unwrap();
    let before = f.snapshot();
    assert!(matches!(
        f.confirm(0, &quote.quote_id),
        Err(EconomyError::MaterialQuoteChanged)
    ));
    assert_eq!(f.snapshot(), before);
    assert_eq!(source_calls(&mut f.client), 0);
    let fresh = f.quote(0, "fresh-valuation", 3).unwrap().receipt;
    assert_eq!(fresh.valuation_version, quote.valuation_version);
    assert_eq!(fresh.valuation_digest, quote.valuation_digest);
    assert!(fresh.valuation_revision > quote.valuation_revision);
    f.confirm(0, &fresh.quote_id).unwrap();
}

#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_whole_and_partial_debit_existing_unbound_without_minting_coins() {
    let mut f = Fixture::new("success");
    f.add_actor(1, true);
    let coin_rows = count(&mut f.client, "hunting_economy_conversions");
    for (actor, quantity) in [(0, 10), (1, 3)] {
        let owner = f.owner(actor);
        let wallet = read_wallet(&mut f.client, &f.realm, &owner)
            .unwrap()
            .unwrap();
        let pool = read_realm(&mut f.client, &f.realm)
            .unwrap()
            .pool
            .stock_atoms;
        let quote = f
            .quote(actor, &format!("convert-{actor}"), quantity)
            .unwrap()
            .receipt;
        let result = f.confirm(actor, &quote.quote_id).unwrap();
        assert!(!result.replayed);
        assert_eq!(result.receipt.wallet_after.bound_gold, wallet.bound_gold);
        assert_eq!(
            result.receipt.wallet_after.unbound_gold,
            wallet.unbound_gold - quote.quote.fee_unbound_gold
        );
        assert_eq!(
            result.receipt.pool_after_atoms,
            pool - quote.quote.gold_equivalent
        );
        assert_eq!(
            read_wallet(&mut f.client, &f.realm, &owner)
                .unwrap()
                .unwrap(),
            result.receipt.wallet_after
        );
        assert_eq!(
            read_realm(&mut f.client, &f.realm)
                .unwrap()
                .pool
                .stock_atoms,
            result.receipt.pool_after_atoms
        );
        assert_eq!(result.receipt.source.checkpoint["legacyGold"], json!(1234));
        let items = result.receipt.source.checkpoint["items"]
            .as_array()
            .unwrap();
        let output = items
            .iter()
            .find(|item| item.get("converted").is_some())
            .unwrap();
        assert_eq!(output["quantity"], json!(quantity));
        assert_eq!(
            output["converted"]["operation"],
            result.receipt.operation_key
        );
        assert_eq!(
            output["converted"]["sourceBindingDigest"],
            quote.source_binding_digest
        );
        assert_eq!(
            items
                .iter()
                .map(|item| item["quantity"].as_u64().unwrap())
                .sum::<u64>(),
            10
        );
        if quantity == 10 {
            assert_eq!(items.len(), 1);
            assert_eq!(
                result.receipt.source.output_unique_id,
                quote.input_unique_id
            );
        } else {
            assert_eq!(items.len(), 2);
            assert_ne!(
                result.receipt.source.output_unique_id,
                quote.input_unique_id
            );
            assert_eq!(
                items
                    .iter()
                    .find(|item| item.get("bound").is_some())
                    .unwrap()["quantity"],
                json!(7)
            );
        }
        let saved=f.client.query_one("SELECT checkpoint,saved_at_ms,business_day FROM material_fixture_sources WHERE account_id=$1",&[&owner.account_id]).unwrap();
        assert_eq!(saved.get::<_, Value>(0), result.receipt.source.checkpoint);
        assert_eq!(saved.get::<_, i64>(1), result.receipt.committed_at_ms);
        assert_eq!(saved.get::<_, i64>(2), result.receipt.business_day);
        let burn=f.client.query_one("SELECT layer,source_kind,gold_equivalent FROM hunting_economy_burn_events WHERE realm_id=$1 AND event_id=$2",&[&f.realm,&result.receipt.burn_event_id]).unwrap();
        assert_eq!(burn.get::<_, String>(0), "daily");
        assert_eq!(burn.get::<_, String>(1), "material_conversion_fee");
        assert_eq!(burn.get::<_, i64>(2), quote.quote.fee_unbound_gold as i64);
        let before = f.snapshot();
        assert_eq!(
            query(&mut f, actor, &quote.quote_id).unwrap(),
            Some(result.receipt)
        );
        assert_eq!(f.snapshot(), before);
    }
    assert_eq!(
        count(&mut f.client, "hunting_economy_conversions"),
        coin_rows
    );
    assert_eq!(
        count(&mut f.client, "hunting_economy_material_conversions"),
        2
    );
    assert_eq!(source_calls(&mut f.client), 2);
}

#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_balance_quota_source_cas_capacity_and_bad_proof_fail_atomically() {
    for case in 0..5 {
        let mut f = Fixture::new(&format!("failure{case}"));
        let actor = if case == 0 {
            f.add_actor(1, false);
            1
        } else {
            0
        };
        if case == 1 {
            let stock = read_realm(&mut f.client, &f.realm)
                .unwrap()
                .pool
                .stock_atoms;
            f.set_prices("above-real-stock", stock + 1);
        }
        if case == 3 {
            f.client.execute("UPDATE material_fixture_sources SET capacity=1 WHERE account_id='material-player-0'",&[]).unwrap();
        }
        let quote = f.quote(actor, "failure", 3).unwrap().receipt;
        if case == 2 {
            f.client.execute("UPDATE material_fixture_sources SET revision=revision+1,checkpoint=jsonb_set(checkpoint,'{revision}',to_jsonb(revision+1)) WHERE account_id='material-player-0'",&[]).unwrap();
        }
        let before = f.snapshot();
        let result = if case == 4 {
            confirm_material_conversion_authorized_with_source_with_timeouts(
                &mut f.client,
                &f.realm,
                &quote.quote_id,
                LOCK_MS,
                STATEMENT_MS,
                |tx, _| authorize(tx, actor),
                |tx, clock, quote, key| {
                    let mut changed = apply_source(tx, clock, quote, key)?;
                    // The source UPDATE happened, but partial output cannot
                    // reuse the still-Bound remainder's UID. Roll it all back.
                    changed.output_unique_id = changed.input_unique_id;
                    Ok(changed)
                },
            )
        } else {
            f.confirm(actor, &quote.quote_id)
        };
        let expected = match case {
            0 => EconomyError::InsufficientUnboundGold,
            1 => EconomyError::InsufficientPermits,
            _ => EconomyError::SourceRejected,
        };
        assert_eq!(result.unwrap_err(), expected, "case {case}");
        assert_eq!(
            f.snapshot(),
            before,
            "case {case}: source, wallet, pool, receipts, burn and outbox all roll back"
        );
        assert_eq!(source_calls(&mut f.client), 0);
    }
}

#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_before_commit_fault_rolls_back_source_and_all_economy_writes() {
    let mut f = Fixture::new("rollback");
    let quote = f.quote(0, "rollback", 3).unwrap().receipt;
    let before = f.snapshot();
    let result = confirm_material_conversion_authorized_with_source_with_test_fault(
        &mut f.client,
        &f.realm,
        &quote.quote_id,
        LOCK_MS,
        STATEMENT_MS,
        TestFault::BeforeCommit,
        |tx, _| authorize(tx, 0),
        apply_source,
    );
    assert!(matches!(result, Err(EconomyError::InjectedFailure)));
    assert_eq!(f.snapshot(), before);
    assert!(query(&mut f, 0, &quote.quote_id).unwrap().is_none());
    assert!(!f.confirm(0, &quote.quote_id).unwrap().replayed);
    assert_eq!(source_calls(&mut f.client), 1);
}

#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_lost_commit_ack_query_and_confirm_replay_do_not_charge_or_mutate_twice() {
    let mut f = Fixture::new("unknown");
    let quote = f.quote(0, "unknown", 3).unwrap().receipt;
    let result = confirm_material_conversion_authorized_with_source_with_test_fault(
        &mut f.client,
        &f.realm,
        &quote.quote_id,
        LOCK_MS,
        STATEMENT_MS,
        TestFault::CommitAcknowledgementLost,
        |tx, _| authorize(tx, 0),
        apply_source,
    );
    assert!(matches!(result, Err(EconomyError::OutcomeUnknown)));
    let mut disabled = policy();
    disabled.policy_version = "disabled-after-material".into();
    disabled.enabled = false;
    register_policy(&mut f.client, &f.realm, &disabled).unwrap();
    activate_policy(&mut f.client, &f.realm, &disabled.policy_version, false).unwrap();
    let before = f.snapshot();
    let recovered = query(&mut f, 0, &quote.quote_id).unwrap().unwrap();
    let repeated = confirm_material_conversion_authorized_with_source_with_timeouts(
        &mut f.client,
        &f.realm,
        &quote.quote_id,
        LOCK_MS,
        STATEMENT_MS,
        |tx, _| authorize(tx, 0),
        |_, _, _, _| panic!("committed material must never execute its source again"),
    )
    .unwrap();
    assert!(repeated.replayed);
    assert_eq!(repeated.receipt, recovered);
    assert_eq!(f.snapshot(), before);
    assert_eq!(source_calls(&mut f.client), 1);
    assert_eq!(
        count(&mut f.client, "hunting_material_conversion_results"),
        1
    );
    let same = read_material_conversion_quote_authorized_with_timeouts(
        &mut f.client,
        &f.realm,
        &quote.quote_id,
        LOCK_MS,
        STATEMENT_MS,
        |tx, _| authorize(tx, 0),
    )
    .unwrap()
    .unwrap();
    assert_eq!(same, quote);
}

#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_two_instances_compete_for_the_last_real_permit_stock() {
    let mut f = Fixture::new("last-quota");
    f.add_actor(1, true);
    let stock = read_realm(&mut f.client, &f.realm)
        .unwrap()
        .pool
        .stock_atoms;
    assert!(stock > 0);
    f.set_prices("last-quota-fixed-test-price", stock);
    let quotes = [
        f.quote(0, "race-a", 1).unwrap().receipt,
        f.quote(1, "race-b", 1).unwrap().receipt,
    ];
    let wallet_sum: i64 = f
        .client
        .query_one(
            "SELECT sum(unbound_gold)::bigint FROM hunting_economy_wallets",
            &[],
        )
        .unwrap()
        .get(0);
    let gate = Arc::new(Barrier::new(3));
    let handles = quotes
        .into_iter()
        .enumerate()
        .map(|(actor, quote)| {
            let connection = f.connection.clone();
            let realm = f.realm.clone();
            let gate = gate.clone();
            thread::spawn(move || {
                let mut client = connection
                    .connect(NoTls)
                    .unwrap_or_else(|_| panic!("isolated worker connection failed"));
                gate.wait();
                confirm_on(&mut client, &realm, actor as i32, &quote.quote_id)
            })
        })
        .collect::<Vec<_>>();
    gate.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(EconomyError::InsufficientPermits)))
            .count(),
        1
    );
    assert_eq!(
        read_realm(&mut f.client, &f.realm)
            .unwrap()
            .pool
            .stock_atoms,
        0
    );
    let winner = results.into_iter().find_map(Result::ok).unwrap().receipt;
    let after_sum: i64 = f
        .client
        .query_one(
            "SELECT sum(unbound_gold)::bigint FROM hunting_economy_wallets",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        wallet_sum - after_sum,
        winner.quote.quote.fee_unbound_gold as i64
    );
    assert_eq!(source_calls(&mut f.client), 1);
    assert_eq!(
        count(&mut f.client, "hunting_material_conversion_results"),
        1
    );
    assert_eq!(
        f.client
            .query_one(
                "SELECT sum(bound_gold)::bigint FROM hunting_economy_wallets",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        200_000
    );
}

#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_same_quote_concurrent_confirmations_return_one_immutable_outcome() {
    let mut f = Fixture::new("same-quote");
    let quote = f.quote(0, "same", 3).unwrap().receipt;
    let owner = f.owner(0);
    let wallet = read_wallet(&mut f.client, &f.realm, &owner)
        .unwrap()
        .unwrap();
    let pool = read_realm(&mut f.client, &f.realm)
        .unwrap()
        .pool
        .stock_atoms;
    let gate = Arc::new(Barrier::new(3));
    let handles = (0..2)
        .map(|_| {
            let connection = f.connection.clone();
            let realm = f.realm.clone();
            let id = quote.quote_id.clone();
            let gate = gate.clone();
            thread::spawn(move || {
                let mut client = connection
                    .connect(NoTls)
                    .unwrap_or_else(|_| panic!("isolated worker connection failed"));
                gate.wait();
                confirm_on(&mut client, &realm, 0, &id).unwrap()
            })
        })
        .collect::<Vec<_>>();
    gate.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.replayed).count(), 1);
    assert_eq!(results[0].receipt, results[1].receipt);
    let current = read_wallet(&mut f.client, &f.realm, &owner)
        .unwrap()
        .unwrap();
    assert_eq!(current.bound_gold, wallet.bound_gold);
    assert_eq!(
        current.unbound_gold,
        wallet.unbound_gold - quote.quote.fee_unbound_gold
    );
    assert_eq!(
        read_realm(&mut f.client, &f.realm)
            .unwrap()
            .pool
            .stock_atoms,
        pool - quote.quote.license_atoms_debit
    );
    assert_eq!(source_calls(&mut f.client), 1);
    assert_eq!(
        count(&mut f.client, "hunting_material_conversion_results"),
        1
    );
}

fn sqlstate(error: postgres::Error, expected: &str) {
    assert_eq!(error.code().map(|code| code.code()), Some(expected));
}
#[test]
#[ignore = "requires isolated loopback PostgreSQL and test-support; retains its unique schema"]
fn postgres_material_history_immutability_and_quote_result_ledger_foreign_keys() {
    let mut f = Fixture::new("constraints");
    let quoted = f.quote(0, "committed", 3).unwrap().receipt;
    f.confirm(0, &quoted.quote_id).unwrap();
    let uncommitted = f.quote(0, "other-bound-remainder", 1).unwrap().receipt;
    let before = f.snapshot();
    for table in [
        "hunting_material_valuations",
        "hunting_material_conversion_quotes",
        "hunting_economy_material_conversions",
        "hunting_material_conversion_results",
    ] {
        sqlstate(
            f.client
                .execute(
                    &format!("UPDATE {table} SET realm_id=realm_id WHERE realm_id=$1"),
                    &[&f.realm],
                )
                .unwrap_err(),
            "P0001",
        );
        sqlstate(
            f.client
                .execute(
                    &format!("DELETE FROM {table} WHERE realm_id=$1"),
                    &[&f.realm],
                )
                .unwrap_err(),
            "P0001",
        );
    }
    sqlstate(
        f.client
            .execute(
                "UPDATE hunting_material_active_valuations SET revision=revision WHERE realm_id=$1",
                &[&f.realm],
            )
            .unwrap_err(),
        "P0001",
    );
    let fake = "9".repeat(64);
    sqlstate(f.client.execute("INSERT INTO hunting_material_conversion_quotes SELECT (jsonb_populate_record(NULL::hunting_material_conversion_quotes,to_jsonb(q)||jsonb_build_object('quote_id',$3::text,'valuation_digest','missing'))).* FROM hunting_material_conversion_quotes q WHERE realm_id=$1 AND quote_id=$2",
        &[&f.realm,&uncommitted.quote_id,&fake]).unwrap_err(),"23503");
    // Result -> material ledger is deferred, so failure must be checked at COMMIT.
    let mut tx = f.client.transaction().unwrap();
    tx.execute("INSERT INTO hunting_material_conversion_results(realm_id,quote_id,operation_key,receipt_json,receipt_digest,before_revision,after_revision,committed_at_ms) VALUES($1,$2,$3,'{}','fixture',2,3,0)",
        &[&f.realm,&uncommitted.quote_id,&fake]).unwrap();
    sqlstate(tx.commit().unwrap_err(), "23503");
    // A fully quote-matching ledger row still needs a real operation parent.
    let mut tx = f.client.transaction().unwrap();
    tx.execute("INSERT INTO hunting_economy_material_conversions(realm_id,operation_key,quote_id,account_id,character_index,character_incarnation,item_index,quantity,valuation_version,valuation_digest,gold_equivalent,fee_gold,consumed_atoms,policy_version,business_day,committed_at_ms) SELECT realm_id,$3,quote_id,account_id,character_index,character_incarnation,item_index,quantity,valuation_version,valuation_digest,gold_equivalent,fee_gold,consumed_atoms,policy_version,0,0 FROM hunting_material_conversion_quotes WHERE realm_id=$1 AND quote_id=$2",
        &[&f.realm,&uncommitted.quote_id,&fake]).unwrap();
    sqlstate(tx.commit().unwrap_err(), "23503");
    // Its owner and monetary fields must independently match the immutable quote.
    // Use an EXISTING operation parent, otherwise the missing-operation FK
    // could mask an absent owner/price FK and give this test a false success.
    let existing_parent: String = f
        .client
        .query_one(
            "SELECT operation_key FROM hunting_economy_conversions LIMIT 1",
            &[],
        )
        .unwrap()
        .get(0);
    sqlstate(f.client.execute("INSERT INTO hunting_economy_material_conversions(realm_id,operation_key,quote_id,account_id,character_index,character_incarnation,item_index,quantity,valuation_version,valuation_digest,gold_equivalent,fee_gold,consumed_atoms,policy_version,business_day,committed_at_ms) SELECT realm_id,$3,quote_id,account_id||'-wrong',character_index,character_incarnation,item_index,quantity,valuation_version,valuation_digest,gold_equivalent,fee_gold,consumed_atoms,policy_version,0,0 FROM hunting_material_conversion_quotes WHERE realm_id=$1 AND quote_id=$2",
        &[&f.realm,&uncommitted.quote_id,&existing_parent]).unwrap_err(),"23503");
    sqlstate(f.client.execute("INSERT INTO hunting_economy_material_conversions(realm_id,operation_key,quote_id,account_id,character_index,character_incarnation,item_index,quantity,valuation_version,valuation_digest,gold_equivalent,fee_gold,consumed_atoms,policy_version,business_day,committed_at_ms) SELECT realm_id,$3,quote_id,account_id,character_index,character_incarnation,item_index,quantity,valuation_version,valuation_digest,gold_equivalent,fee_gold+1,consumed_atoms,policy_version,0,0 FROM hunting_material_conversion_quotes WHERE realm_id=$1 AND quote_id=$2",
        &[&f.realm,&uncommitted.quote_id,&existing_parent]).unwrap_err(),"23503");
    assert_eq!(f.snapshot(), before);
}

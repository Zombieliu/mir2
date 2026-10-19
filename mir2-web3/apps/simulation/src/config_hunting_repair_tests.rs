//! Full durable character checkpoint acceptance for the trusted repair bridge.
//! These tests never start PostgreSQL, Docker or any other external service.
use super::*;
use mir2_economy_service as economy;
use postgres::{Client, NoTls};
use serde_json::{json, Value};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const COST: u64 = 30;

fn isolated_repair_database_url() -> String {
    let url = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
        .expect("ignored repair acceptance requires MIR2_ECONOMY_TEST_DATABASE_URL");
    assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
    let parsed: postgres::Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid isolated acceptance configuration"));
    assert!(parsed
        .get_dbname()
        .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
    assert!(!parsed.get_hosts().is_empty() && parsed.get_hosts().iter().all(|host|
        matches!(host,postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()))));
    assert!(parsed
        .get_hostaddrs()
        .iter()
        .all(std::net::IpAddr::is_loopback));
    url
}

fn repaired_checkpoint(before: &CharacterSaveRecord) -> CharacterSaveRecord {
    let mut after = before.clone();
    after.revision += 1;
    let mut item: Value = serde_json::from_str(&after.inventory_items_json[1]).unwrap();
    item["durability_current"] = json!(40);
    after.inventory_items_json[1] = item.to_string();
    after
}

fn stage_repaired(
    account: String,
    before: CharacterSaveRecord,
) -> impl FnOnce(&mut AccountStore) -> Result<(), String> {
    move |store| {
        let after = repaired_checkpoint(&before);
        store
            .accounts
            .get_mut(&account)
            .unwrap()
            .saves
            .insert(after.character.index, after);
        Ok(())
    }
}

fn checkpoint_json(save: &CharacterSaveRecord) -> Value {
    serde_json::to_value(save).unwrap()
}

struct RepairFixture {
    client: Client,
    config: SimulationConfig,
    account: String,
    realm: String,
    incarnation: String,
    before: CharacterSaveRecord,
    url: String,
}
impl RepairFixture {
    fn new(bound_gold: u64) -> Self {
        let base = isolated_repair_database_url();
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
        let schema = format!("hunting_repair_{nonce}");
        let mut bootstrap = Client::connect(&base, NoTls)
            .unwrap_or_else(|_| panic!("isolated acceptance connection failed"));
        bootstrap
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let separator = if base.contains('?') { '&' } else { '?' };
        let url = format!("{base}{separator}options=-csearch_path%3D{schema}");
        let mut client = Client::connect(&url, NoTls)
            .unwrap_or_else(|_| panic!("isolated schema connection failed"));
        assert_eq!(
            client
                .query_one("SELECT current_schema()", &[])
                .unwrap()
                .get::<_, String>(0),
            schema
        );
        crate::apply_migrations(&mut client).unwrap();

        // Each fixture owns a private schema; use an ordinary valid login ID so
        // the real standard-login preflight is exercised without policy bypass.
        let account = "repairfixture".to_owned();
        let mut store = AccountStore::new(SimulationConfig::default().default_character);
        let mut record = store.accounts.remove("demo").unwrap();
        let before = record.saves.get_mut(&0).unwrap();
        before.gold = 42;
        before.credit = 17;
        before.experience = 33;
        before.map_file_name = "3".into();
        before.map_title = "Bichon Province".into();
        before.position = Point { x: 100, y: 200 };
        before.inventory_items_json = vec![
            r#"{"key":"red-potion","name":"Red Potion","icon":1,"slot":0,"unique_id":101,"container":"Bag1","quantity":20,"description":"","durability_current":null,"durability_max":null,"weight":1,"equip_slot":null,"grade":"Common","attack":0,"defence":0,"heal_hp":50,"heal_mp":0}"#.into(),
            r#"{"key":"iron-sword","name":"Iron Sword","icon":2,"slot":1,"unique_id":102,"container":"Bag1","quantity":1,"description":"trusted repair target","durability_current":35,"durability_max":40,"weight":10,"equip_slot":null,"grade":"Rare","attack":7,"defence":0,"heal_hp":0,"heal_mp":0}"#.into(),
        ];
        before.city_currencies.insert("bichon".into(), 9);
        let before = before.clone();
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();

        let realm = format!("repair-realm-{nonce}");
        let policy = serde_json::from_value(json!({"schema_version":1,"policy_version":"repair-fixture-v1","enabled":true,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":1,"denominator":50},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap();
        economy::initialize_realm(&mut client, &realm, &policy).unwrap();
        let config = SimulationConfig::default()
            .with_postgres_account_store(&url)
            .unwrap()
            .with_hunting_economy_realm(&realm)
            .unwrap();
        let incarnation = config
            .bind_hunting_character_incarnation(&account, &before)
            .unwrap()
            .unwrap();
        if bound_gold > 0 {
            economy::credit_bound_with_source(
                &mut client,
                &realm,
                &economy::WalletOwner {
                    account_id: account.clone(),
                    character_index: 0,
                    incarnation: incarnation.clone(),
                },
                "fixture-trusted-production",
                "initial-funding",
                bound_gold,
                |_, _| Ok(()),
            )
            .unwrap();
        }
        Self {
            client,
            config,
            account,
            realm,
            incarnation,
            before,
            url,
        }
    }

    fn quote_id(slot: u8) -> String {
        format!("{slot:064x}")
    }

    fn quote_json(cost: u64) -> Value {
        json!({"schemaVersion":1,"kind":"hunting-repair-quote-v1","version":"hunting-repair-v1",
            "costBoundGold":cost.to_string(),"mode":"normal","npcObjectId":77,
            "targetUniqueId":"102","beforeDurability":35,"afterDurability":40,
            "durabilityMax":40,"trustedFixture":true})
    }

    fn register(&self, slot: u8, cost: u64) {
        let quote = Self::quote_json(cost);
        let digest = repair_quote_digest(&quote).unwrap();
        self.config
            .register_hunting_repair_quote(
                &self.account,
                &self.incarnation,
                &Self::quote_id(slot),
                &digest,
                cost,
                self.before.revision,
                quote,
            )
            .unwrap();
    }

    fn register_with_ttl(&self, slot: u8, cost: u64, ttl_ms: i64) {
        let quote = Self::quote_json(cost);
        let digest = repair_quote_digest(&quote).unwrap();
        self.config
            .register_hunting_repair_quote_with_ttl_for_test(
                &self.account,
                &self.incarnation,
                &Self::quote_id(slot),
                &digest,
                cost,
                self.before.revision,
                quote,
                ttl_ms,
            )
            .unwrap();
    }

    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(self.client.query_one("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0",&[&self.account]).unwrap().get(0)).unwrap()
    }

    fn mirror(&self) -> Value {
        checkpoint_json(
            &self.config.account_store.lock().unwrap().accounts[&self.account].saves[&0],
        )
    }

    fn bound(&mut self) -> Option<u64> {
        economy::read_wallet(
            &mut self.client,
            &self.realm,
            &economy::WalletOwner {
                account_id: self.account.clone(),
                character_index: 0,
                incarnation: self.incarnation.clone(),
            },
        )
        .unwrap()
        .map(|wallet| wallet.bound_gold)
    }

    fn operation_counts(&mut self) -> (i64, i64, i64, i64) {
        let row = self.client.query_one("SELECT
            (SELECT count(*) FROM hunting_economy_operations WHERE realm_id=$1 AND operation_kind IN ('bound_debit_burn','zero_fee_source')),
            (SELECT count(*) FROM hunting_economy_outbox WHERE realm_id=$1 AND event_kind IN ('bound_debit_burn','zero_fee_source')),
            (SELECT count(*) FROM hunting_economy_burn_events WHERE realm_id=$1),
            (SELECT COALESCE(sum(gold_equivalent),0)::bigint FROM hunting_economy_burn_events WHERE realm_id=$1)",&[&self.realm]).unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }

    fn results(&mut self) -> i64 {
        self.client
            .query_one(
                "SELECT count(*) FROM hunting_repair_results WHERE realm_id=$1",
                &[&self.realm],
            )
            .unwrap()
            .get(0)
    }

    fn assert_full_projection(&mut self, expected: &CharacterSaveRecord) {
        assert_eq!(checkpoint_json(&self.durable()), checkpoint_json(expected));
        let raw: Value = self
            .client
            .query_one(
                "SELECT raw_json FROM accounts WHERE account_id=$1",
                &[&self.account],
            )
            .unwrap()
            .get(0);
        let account: AccountRecord = serde_json::from_value(raw).unwrap();
        assert_eq!(
            checkpoint_json(&account.saves[&0]),
            checkpoint_json(expected)
        );
        let row = self.client.query_one("SELECT durability_current,durability_max,quantity FROM character_items WHERE account_id=$1 AND character_index=0 AND unique_id=102",&[&self.account]).unwrap();
        let item: Value = serde_json::from_str(&expected.inventory_items_json[1]).unwrap();
        assert_eq!(
            row.get::<_, i32>("durability_current"),
            item["durability_current"].as_i64().unwrap() as i32
        );
        assert_eq!(row.get::<_, i32>("durability_max"), 40);
        assert_eq!(row.get::<_, i64>("quantity"), 1);
        let row = self.client.query_one("SELECT gold,credit,experience,inventory_count,position_x,position_y FROM character_state WHERE account_id=$1 AND character_index=0",&[&self.account]).unwrap();
        assert_eq!(row.get::<_, i64>("gold"), i64::from(expected.gold));
        assert_eq!(row.get::<_, i64>("credit"), i64::from(expected.credit));
        assert_eq!(row.get::<_, i64>("experience"), expected.experience);
        assert_eq!(row.get::<_, i32>("inventory_count"), 2);
        assert_eq!(row.get::<_, i32>("position_x"), expected.position.x);
        assert_eq!(row.get::<_, i32>("position_y"), expected.position.y);
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_complete_checkpoint_and_payment_commit_atomically() {
    for cost in [COST, 0] {
        let funding = if cost == 0 { 0 } else { 100 };
        let mut db = RepairFixture::new(funding);
        db.register(1, cost);
        let original = checkpoint_json(&db.before);
        let expected = repaired_checkpoint(&db.before);
        assert_eq!(db.mirror(), original);
        assert_eq!(db.operation_counts(), (0, 0, 0, 0));
        let committed = db
            .config
            .publish_hunting_repair_source(
                &db.account,
                &db.incarnation,
                &RepairFixture::quote_id(1),
                db.before.clone(),
                stage_repaired(db.account.clone(), db.before.clone()),
            )
            .unwrap();
        assert!(!committed.receipt.replayed);
        assert_eq!(
            checkpoint_json(&committed.checkpoint),
            checkpoint_json(&expected)
        );
        assert_eq!(committed.current_revision, Some(expected.revision));
        assert_eq!(db.mirror(), checkpoint_json(&expected));
        db.assert_full_projection(&expected);
        let charged = i64::from(cost > 0);
        assert_eq!(db.operation_counts(), (1, 1, charged, cost as i64));
        assert_eq!(db.results(), 1);
        assert_eq!(
            db.bound(),
            if funding == 0 {
                None
            } else {
                Some(funding - cost)
            }
        );
        let rows = db.client.query("SELECT operation_kind,receipt_json FROM hunting_economy_operations WHERE realm_id=$1 AND operation_kind IN ('bound_debit_burn','zero_fee_source')",&[&db.realm]).unwrap();
        assert_eq!(rows.len(), 1);
        let receipt: Value = rows[0].get("receipt_json");
        assert_eq!(
            rows[0].get::<_, String>("operation_kind"),
            if cost == 0 {
                "zero_fee_source"
            } else {
                "bound_debit_burn"
            }
        );
        assert_eq!(receipt["owner"]["incarnation"], db.incarnation);
        assert_eq!(receipt["owner"]["account_id"], db.account);
        if cost == 0 {
            assert!(receipt.get("wallet_after").is_none());
            assert!(receipt.get("burn_event_id").is_none());
        } else {
            let row = db.client.query_one("SELECT layer,gold_equivalent,source_kind,valuation_version FROM hunting_economy_burn_events WHERE realm_id=$1",&[&db.realm]).unwrap();
            assert_eq!(row.get::<_, String>("layer"), "daily");
            assert_eq!(row.get::<_, i64>("gold_equivalent"), cost as i64);
            assert_eq!(
                row.get::<_, String>("valuation_version"),
                economy::BOUND_GOLD_FACE_VALUE_VALUATION_VERSION
            );
        }
        // Query and duplicate Confirm never invoke a replacement source closure.
        for _ in 0..2 {
            let query = db
                .config
                .lookup_hunting_repair_source(
                    &db.account,
                    &db.incarnation,
                    0,
                    &RepairFixture::quote_id(1),
                )
                .unwrap()
                .unwrap();
            assert!(query.receipt.replayed);
            assert_eq!(
                checkpoint_json(&query.checkpoint),
                checkpoint_json(&expected)
            );
            let duplicate = db
                .config
                .publish_hunting_repair_source(
                    &db.account,
                    &db.incarnation,
                    &RepairFixture::quote_id(1),
                    db.before.clone(),
                    |_| panic!("duplicate cannot rerun source"),
                )
                .unwrap();
            assert!(duplicate.receipt.replayed);
            assert_eq!(
                checkpoint_json(&duplicate.checkpoint),
                checkpoint_json(&expected)
            );
        }
        assert_eq!(db.operation_counts(), (1, 1, charged, cost as i64));
        assert_eq!(db.results(), 1);
        db.assert_full_projection(&expected);
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_full_source_rollback_and_insufficient_bound_leave_no_receipt() {
    for cost in [COST, 0] {
        let funding = if cost == 0 { 0 } else { 100 };
        let mut db = RepairFixture::new(funding);
        db.register(1, cost);
        let memory = db.mirror();
        let result = db.config.publish_hunting_repair_source_with_test_fault(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone()),
            1,
        );
        assert!(result.is_err());
        assert_eq!(db.operation_counts(), (0, 0, 0, 0));
        assert_eq!(db.results(), 0);
        assert_eq!(db.mirror(), memory);
        assert_eq!(db.bound(), if funding == 0 { None } else { Some(funding) });
        let before = db.before.clone();
        db.assert_full_projection(&before);
        assert!(db
            .config
            .lookup_hunting_repair_source(
                &db.account,
                &db.incarnation,
                0,
                &RepairFixture::quote_id(1)
            )
            .unwrap()
            .is_none());
        assert!(db.config.ensure_account_store_writable().is_ok());
    }
    let mut poor = RepairFixture::new(COST - 1);
    poor.register(1, COST);
    assert!(poor
        .config
        .publish_hunting_repair_source(
            &poor.account,
            &poor.incarnation,
            &RepairFixture::quote_id(1),
            poor.before.clone(),
            stage_repaired(poor.account.clone(), poor.before.clone())
        )
        .is_err());
    assert_eq!(poor.bound(), Some(COST - 1));
    assert_eq!(poor.operation_counts(), (0, 0, 0, 0));
    assert_eq!(poor.results(), 0);
    assert_eq!(
        checkpoint_json(&poor.durable()),
        checkpoint_json(&poor.before)
    );
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_original_query_never_rewinds_later_full_save() {
    let mut db = RepairFixture::new(100);
    db.register(1, COST);
    let first = db
        .config
        .publish_hunting_repair_source(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone()),
        )
        .unwrap();
    let original_receipt: Value = db.client.query_one("SELECT receipt_json FROM hunting_economy_operations WHERE realm_id=$1 AND operation_kind='bound_debit_burn'",&[&db.realm]).unwrap().get(0);
    let mut later = repaired_checkpoint(&db.before);
    later.revision += 1;
    later.credit += 9;
    later.position = Point { x: 101, y: 201 };
    let mut item: Value = serde_json::from_str(&later.inventory_items_json[1]).unwrap();
    item["durability_current"] = json!(37);
    later.inventory_items_json[1] = item.to_string();
    let later_for_stage = later.clone();
    db.config
        .commit_account_store_transaction(&[db.account.clone()], |store| {
            store
                .accounts
                .get_mut(&db.account)
                .unwrap()
                .saves
                .insert(0, later_for_stage);
            Ok(())
        })
        .unwrap();
    for _ in 0..2 {
        let query = db
            .config
            .lookup_hunting_repair_source(
                &db.account,
                &db.incarnation,
                0,
                &RepairFixture::quote_id(1),
            )
            .unwrap()
            .unwrap();
        assert!(query.receipt.replayed);
        assert_eq!(
            checkpoint_json(&query.checkpoint),
            checkpoint_json(&first.checkpoint)
        );
        assert_eq!(query.current_revision, Some(later.revision));
        assert_eq!(db.mirror(), checkpoint_json(&later));
        db.assert_full_projection(&later);
    }
    let receipt: Value = db.client.query_one("SELECT receipt_json FROM hunting_economy_operations WHERE realm_id=$1 AND operation_kind='bound_debit_burn'",&[&db.realm]).unwrap().get(0);
    assert_eq!(receipt, original_receipt);
    assert_eq!(db.bound(), Some(100 - COST));
    assert_eq!(db.operation_counts(), (1, 1, 1, COST as i64));
    assert_eq!(db.results(), 1);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_unknown_recovers_only_original_quote_and_never_reexecutes() {
    for cost in [COST, 0] {
        for fault in [2, 5] {
            let funding = if cost == 0 { 0 } else { 100 };
            let mut db = RepairFixture::new(funding);
            db.register(1, cost);
            db.register(2, cost);
            let memory = db.mirror();
            let result = db.config.publish_hunting_repair_source_with_test_fault(
                &db.account,
                &db.incarnation,
                &RepairFixture::quote_id(1),
                db.before.clone(),
                stage_repaired(db.account.clone(), db.before.clone()),
                fault,
            );
            assert!(matches!(
                result,
                Err(HuntingRepairSourceError::OutcomeUnknown(_))
            ));
            assert!(db.config.ensure_account_store_writable().is_err());
            assert_eq!(db.mirror(), memory);
            assert!(
                db.config
                    .commit_account_store_transaction(&[db.account.clone()], |_| Ok(()))
                    .is_err(),
                "an uncertain source must block stale ordinary saves"
            );
            let charged = i64::from(cost > 0);
            assert_eq!(db.operation_counts(), (1, 1, charged, cost as i64));
            assert_eq!(db.results(), 1);
            assert_eq!(
                checkpoint_json(&db.durable()),
                checkpoint_json(&repaired_checkpoint(&db.before))
            );
            let another_quote = db.config.lookup_hunting_repair_source(
                &db.account,
                &db.incarnation,
                0,
                &RepairFixture::quote_id(2),
            );
            assert!(
                !matches!(another_quote, Ok(Some(_))),
                "another quote cannot return the original operation receipt"
            );
            assert!(
                db.config.ensure_account_store_writable().is_err(),
                "another quote cannot release the original fence"
            );
            let original = db
                .config
                .lookup_hunting_repair_source(
                    &db.account,
                    &db.incarnation,
                    0,
                    &RepairFixture::quote_id(1),
                )
                .unwrap()
                .unwrap();
            assert!(original.receipt.replayed);
            assert_eq!(
                checkpoint_json(&original.checkpoint),
                checkpoint_json(&repaired_checkpoint(&db.before))
            );
            assert_eq!(db.mirror(), checkpoint_json(&original.checkpoint));
            assert!(db.config.ensure_account_store_writable().is_ok());
            assert_eq!(
                db.bound(),
                if funding == 0 {
                    None
                } else {
                    Some(funding - cost)
                }
            );
            assert_eq!(db.operation_counts(), (1, 1, charged, cost as i64));
            assert_eq!(db.results(), 1);
        }
    }

    // The same recovery protocol can prove that a reported Unknown rolled back.
    let mut absent = RepairFixture::new(100);
    absent.register(1, COST);
    assert!(matches!(
        absent.config.publish_hunting_repair_source_with_test_fault(
            &absent.account,
            &absent.incarnation,
            &RepairFixture::quote_id(1),
            absent.before.clone(),
            stage_repaired(absent.account.clone(), absent.before.clone()),
            4
        ),
        Err(HuntingRepairSourceError::OutcomeUnknown(_))
    ));
    assert!(absent.config.ensure_account_store_writable().is_err());
    assert_eq!(absent.operation_counts(), (0, 0, 0, 0));
    assert_eq!(absent.results(), 0);
    assert_eq!(absent.bound(), Some(100));
    assert!(absent
        .config
        .lookup_hunting_repair_source(
            &absent.account,
            &absent.incarnation,
            0,
            &RepairFixture::quote_id(1)
        )
        .unwrap()
        .is_none());
    assert!(absent.config.ensure_account_store_writable().is_ok());
    let committed = absent
        .config
        .publish_hunting_repair_source(
            &absent.account,
            &absent.incarnation,
            &RepairFixture::quote_id(1),
            absent.before.clone(),
            stage_repaired(absent.account.clone(), absent.before.clone()),
        )
        .unwrap();
    assert!(!committed.receipt.replayed);
    assert_eq!(absent.bound(), Some(100 - COST));
    assert_eq!(absent.operation_counts(), (1, 1, 1, COST as i64));
    assert_eq!(absent.results(), 1);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_known_commit_and_foreign_freeze_preserve_receipt_and_owner() {
    let mut known = RepairFixture::new(100);
    known.register(1, COST);
    let result = known.config.publish_hunting_repair_source_with_test_fault(
        &known.account,
        &known.incarnation,
        &RepairFixture::quote_id(1),
        known.before.clone(),
        stage_repaired(known.account.clone(), known.before.clone()),
        3,
    );
    match result {
        Err(HuntingRepairSourceError::KnownCommitted { commit, .. }) => {
            assert!(!commit.receipt.replayed);
            assert_eq!(
                checkpoint_json(&commit.checkpoint),
                checkpoint_json(&repaired_checkpoint(&known.before))
            );
            assert!(matches!(
                commit.receipt.receipt,
                HuntingRepairReceipt::Charged(_)
            ));
        }
        other => panic!("publication failure must retain a known committed receipt: {other:?}"),
    }
    assert_eq!(known.bound(), Some(100 - COST));
    assert_eq!(known.results(), 1);
    assert!(known.config.ensure_account_store_writable().is_err());
    let query = known
        .config
        .lookup_hunting_repair_source(
            &known.account,
            &known.incarnation,
            0,
            &RepairFixture::quote_id(1),
        )
        .unwrap()
        .unwrap();
    assert!(query.receipt.replayed);
    assert!(known.config.ensure_account_store_writable().is_ok());

    for cost in [COST, 0] {
        let funding = if cost == 0 { 0 } else { 100 };
        let mut db = RepairFixture::new(funding);
        db.register(1, cost);
        let memory = db.mirror();
        assert!(matches!(
            db.config.publish_hunting_repair_source_with_test_fault(
                &db.account,
                &db.incarnation,
                &RepairFixture::quote_id(1),
                db.before.clone(),
                stage_repaired(db.account.clone(), db.before.clone()),
                2
            ),
            Err(HuntingRepairSourceError::OutcomeUnknown(_))
        ));
        db.config
            .freeze_account_store_writes("unrelated subsystem projection failure".into());
        let reason = match &*db.config.account_store_write_state.lock().unwrap() {
            AccountStoreWriteState::Frozen { reason } => reason.clone(),
            AccountStoreWriteState::Writable => panic!("foreign freeze missing"),
        };
        let epoch = db.config.account_store_freeze_epoch.load(Ordering::Acquire);
        let result = db.config.lookup_hunting_repair_source(
            &db.account,
            &db.incarnation,
            0,
            &RepairFixture::quote_id(1),
        );
        // Historical lookup may return the immutable result while keeping the
        // foreign fence; it may also reject publication. Neither may unfreeze it.
        if let Ok(Some(commit)) = result {
            assert!(commit.receipt.replayed);
            assert_eq!(
                checkpoint_json(&commit.checkpoint),
                checkpoint_json(&repaired_checkpoint(&db.before))
            );
        }
        assert!(db.config.ensure_account_store_writable().is_err());
        assert!(
            matches!(&*db.config.account_store_write_state.lock().unwrap(),AccountStoreWriteState::Frozen { reason: current } if current == &reason)
        );
        assert_eq!(
            db.config.account_store_freeze_epoch.load(Ordering::Acquire),
            epoch
        );
        assert_eq!(
            db.mirror(),
            memory,
            "historical results cannot publish through another subsystem's fence"
        );
        let charged = i64::from(cost > 0);
        assert_eq!(db.operation_counts(), (1, 1, charged, cost as i64));
        assert_eq!(db.results(), 1);
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_owner_generation_and_complete_checkpoint_cas_are_enforced() {
    let mut stale = RepairFixture::new(100);
    stale.register(1, COST);
    let separate = SimulationConfig::default()
        .with_postgres_account_store(&stale.url)
        .unwrap();
    separate
        .commit_account_store_transaction(&[stale.account.clone()], |store| {
            let save = store
                .accounts
                .get_mut(&stale.account)
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap();
            save.revision += 1;
            save.credit += 9;
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        stale.config.publish_hunting_repair_source(
            &stale.account,
            &stale.incarnation,
            &RepairFixture::quote_id(1),
            stale.before.clone(),
            stage_repaired(stale.account.clone(), stale.before.clone())
        ),
        Err(HuntingRepairSourceError::Rejected(_))
    ));
    assert_eq!(stale.bound(), Some(100));
    assert_eq!(stale.operation_counts(), (0, 0, 0, 0));
    assert_eq!(stale.results(), 0);
    assert_eq!(stale.durable().credit, stale.before.credit + 9);

    let mut db = RepairFixture::new(100);
    db.register(1, COST);
    let before = checkpoint_json(&db.before);
    assert!(db
        .config
        .publish_hunting_repair_source(
            "another-account",
            &db.incarnation,
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone())
        )
        .is_err());
    assert!(db
        .config
        .publish_hunting_repair_source(
            &db.account,
            "pg-character-v1:retired",
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone())
        )
        .is_err());
    assert!(db
        .config
        .lookup_hunting_repair_source(
            &db.account,
            "pg-character-v1:retired",
            0,
            &RepairFixture::quote_id(1)
        )
        .is_err());
    db.client
        .execute(
            "UPDATE accounts SET is_banned=true,ban_until_ms=NULL WHERE account_id=$1",
            &[&db.account],
        )
        .unwrap();
    assert!(db
        .config
        .publish_hunting_repair_source(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone())
        )
        .is_err());
    db.client
        .execute(
            "UPDATE accounts SET is_banned=false WHERE account_id=$1",
            &[&db.account],
        )
        .unwrap();
    assert_eq!(db.operation_counts(), (0, 0, 0, 0));
    assert_eq!(db.results(), 0);
    assert_eq!(checkpoint_json(&db.durable()), before);

    let committed = db
        .config
        .publish_hunting_repair_source(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone()),
        )
        .unwrap();
    assert!(matches!(
        committed.receipt.receipt,
        HuntingRepairReceipt::Charged(_)
    ));
    db.client
        .execute(
            "UPDATE accounts SET is_banned=true,ban_until_ms=NULL WHERE account_id=$1",
            &[&db.account],
        )
        .unwrap();
    assert!(
        db.config
            .lookup_hunting_repair_source(
                &db.account,
                &db.incarnation,
                0,
                &RepairFixture::quote_id(1)
            )
            .is_err(),
        "authorization must also precede an existing receipt"
    );
    db.client
        .execute(
            "UPDATE accounts SET is_banned=false WHERE account_id=$1",
            &[&db.account],
        )
        .unwrap();
    db.client.execute("UPDATE characters SET hunting_incarnation=gen_random_uuid() WHERE account_id=$1 AND character_index=0",&[&db.account]).unwrap();
    assert!(
        db.config
            .lookup_hunting_repair_source(
                &db.account,
                &db.incarnation,
                0,
                &RepairFixture::quote_id(1)
            )
            .is_err(),
        "retired incarnation cannot recover a previous receipt"
    );
    assert_eq!(db.bound(), Some(100 - COST));
    assert_eq!(db.operation_counts(), (1, 1, 1, COST as i64));
    assert_eq!(db.results(), 1);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_expiry_blocks_fresh_source_and_disabled_policy_keeps_old_result_queryable(
) {
    let mut db = RepairFixture::new(100);
    db.register_with_ttl(2, COST, 1);
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(matches!(
        db.config.publish_hunting_repair_source(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(2),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone())
        ),
        Err(HuntingRepairSourceError::Rejected(_))
    ));
    assert!(db
        .config
        .lookup_hunting_repair_source(&db.account, &db.incarnation, 0, &RepairFixture::quote_id(2))
        .unwrap()
        .is_none());
    assert_eq!(db.operation_counts(), (0, 0, 0, 0));
    assert_eq!(db.results(), 0);
    assert_eq!(db.bound(), Some(100));
    assert_eq!(checkpoint_json(&db.durable()), checkpoint_json(&db.before));

    db.register_with_ttl(1, COST, 500);
    let first = db
        .config
        .publish_hunting_repair_source(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone()),
        )
        .unwrap();
    // This test-only server TTL uses the real DB clock. Historical immutable
    // quote/result rows are never rewritten merely to manufacture expiry.
    std::thread::sleep(std::time::Duration::from_millis(600));
    let expired: bool = db.client.query_one("SELECT expires_at_ms<floor(extract(epoch FROM clock_timestamp())*1000)::bigint FROM hunting_repair_quotes WHERE realm_id=$1 AND quote_id=$2",&[&db.realm,&RepairFixture::quote_id(1)]).unwrap().get(0);
    assert!(expired);
    let original = db
        .config
        .lookup_hunting_repair_source(&db.account, &db.incarnation, 0, &RepairFixture::quote_id(1))
        .unwrap()
        .unwrap();
    assert!(original.receipt.replayed);
    assert_eq!(
        checkpoint_json(&original.checkpoint),
        checkpoint_json(&first.checkpoint)
    );

    let mut disabled_json: Value = db.client.query_one("SELECT policy_json FROM hunting_economy_policies WHERE realm_id=$1 AND policy_version='repair-fixture-v1'",&[&db.realm]).unwrap().get(0);
    disabled_json["policy_version"] = json!("repair-disabled");
    disabled_json["enabled"] = json!(false);
    let disabled = serde_json::from_value(disabled_json).unwrap();
    economy::register_policy(&mut db.client, &db.realm, &disabled).unwrap();
    economy::activate_policy(&mut db.client, &db.realm, "repair-disabled", true).unwrap();
    let old = db
        .config
        .lookup_hunting_repair_source(&db.account, &db.incarnation, 0, &RepairFixture::quote_id(1))
        .unwrap()
        .unwrap();
    assert!(old.receipt.replayed);
    assert_eq!(
        checkpoint_json(&old.checkpoint),
        checkpoint_json(&first.checkpoint)
    );
    assert_eq!(db.bound(), Some(100 - COST));
    assert_eq!(db.operation_counts(), (1, 1, 1, COST as i64));
    assert_eq!(db.results(), 1);
    assert!(
        db.client
            .execute(
                "UPDATE hunting_repair_results SET checkpoint_json='{}'::jsonb WHERE realm_id=$1",
                &[&db.realm]
            )
            .is_err(),
        "complete original source history must be immutable"
    );
    assert!(
        db.client
            .execute(
                "UPDATE hunting_repair_quotes SET cost_bound_gold=0 WHERE realm_id=$1",
                &[&db.realm]
            )
            .is_err(),
        "changing price cannot free an already charged source"
    );
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_immutable_quote_rejects_repricing_digest_and_revision_changes() {
    let mut db = RepairFixture::new(100);
    db.register(1, COST);
    for cost in [0, COST + 1] {
        let changed = RepairFixture::quote_json(cost);
        let hash = repair_quote_digest(&changed).unwrap();
        assert!(
            db.config
                .register_hunting_repair_quote(
                    &db.account,
                    &db.incarnation,
                    &RepairFixture::quote_id(1),
                    &hash,
                    cost,
                    db.before.revision,
                    changed
                )
                .is_err(),
            "a zero/positive reprice cannot obtain a second source slot"
        );
    }
    let original = RepairFixture::quote_json(COST);
    let hash = repair_quote_digest(&original).unwrap();
    assert!(db
        .config
        .register_hunting_repair_quote(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(2),
            "wrong-canonical-digest",
            COST,
            db.before.revision,
            original.clone()
        )
        .is_err());
    assert!(db
        .config
        .register_hunting_repair_quote(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(2),
            &hash,
            COST + 1,
            db.before.revision,
            original.clone()
        )
        .is_err());
    assert!(db
        .config
        .register_hunting_repair_quote(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(2),
            &hash,
            COST,
            db.before.revision + 1,
            original.clone()
        )
        .is_err());
    for malformed in ["short".to_owned(), "A".repeat(64)] {
        assert!(db
            .config
            .register_hunting_repair_quote(
                &db.account,
                &db.incarnation,
                &malformed,
                &hash,
                COST,
                db.before.revision,
                original.clone()
            )
            .is_err());
    }
    assert_eq!(db.operation_counts(), (0, 0, 0, 0));
    assert_eq!(db.results(), 0);
    assert_eq!(db.bound(), Some(100));
    let committed = db
        .config
        .publish_hunting_repair_source(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone()),
        )
        .unwrap();
    match committed.receipt.receipt {
        HuntingRepairReceipt::Charged(receipt) => assert_eq!(receipt.bound_gold_debit, COST),
        HuntingRepairReceipt::Free(_) => panic!("immutable positive quote was repriced to zero"),
    }
    assert_eq!(db.bound(), Some(100 - COST));
    assert_eq!(db.operation_counts(), (1, 1, 1, COST as i64));
    assert_eq!(db.results(), 1);
    let row = db.client.query_one("SELECT before_revision,after_revision,checkpoint_json,cost_bound_gold FROM hunting_repair_results WHERE realm_id=$1",&[&db.realm]).unwrap();
    assert_eq!(
        row.get::<_, i64>("before_revision"),
        db.before.revision as i64
    );
    assert_eq!(
        row.get::<_, i64>("after_revision"),
        (db.before.revision + 1) as i64
    );
    assert_eq!(
        row.get::<_, Value>("checkpoint_json"),
        checkpoint_json(&repaired_checkpoint(&db.before))
    );
    assert_eq!(row.get::<_, i64>("cost_bound_gold"), COST as i64);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_login_refresh_reconciles_lost_session_commits_and_rollbacks() {
    for cost in [COST, 0] {
        for fault in [2, 4] {
            let funding = if cost == 0 { 0 } else { 100 };
            let mut db = RepairFixture::new(funding);
            db.register(1, cost);
            assert!(matches!(
                db.config.publish_hunting_repair_source_with_test_fault(
                    &db.account,
                    &db.incarnation,
                    &RepairFixture::quote_id(1),
                    db.before.clone(),
                    stage_repaired(db.account.clone(), db.before.clone()),
                    fault
                ),
                Err(HuntingRepairSourceError::OutcomeUnknown(_))
            ));
            assert!(db.config.ensure_account_store_writable().is_err());
            // No active Actor or LocalQuote exists in this fresh login preflight.
            // A config clone shares only the server's private recovery fence.
            let fresh_config = db.config.clone();
            assert!(crate::SimulationSession::standard_login_recovery_preflight(
                &fresh_config,
                &db.account,
                &default_account_password()
            )
            .unwrap());
            assert!(fresh_config.ensure_account_store_writable().is_ok());
            assert!(db.config.hunting_repair_fence.lock().unwrap().is_none());
            let committed = i64::from(fault == 2);
            let charged = i64::from(cost > 0) * committed;
            assert_eq!(
                db.operation_counts(),
                (committed, committed, charged, (cost as i64) * committed)
            );
            assert_eq!(db.results(), committed);
            let expected = if fault == 2 {
                repaired_checkpoint(&db.before)
            } else {
                db.before.clone()
            };
            assert_eq!(db.mirror(), checkpoint_json(&expected));
            db.assert_full_projection(&expected);
            assert_eq!(
                db.bound(),
                if funding == 0 {
                    None
                } else {
                    Some(funding - if fault == 2 { cost } else { 0 })
                }
            );
            assert!(fresh_config
                .refresh_account_store_account(&db.account)
                .is_ok());
            assert_eq!(
                db.operation_counts(),
                (committed, committed, charged, (cost as i64) * committed)
            );
        }
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_login_refresh_resolves_old_outcome_without_bypassing_new_ban() {
    let mut db = RepairFixture::new(100);
    db.register(1, COST);
    assert!(matches!(
        db.config.publish_hunting_repair_source_with_test_fault(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone()),
            2
        ),
        Err(HuntingRepairSourceError::OutcomeUnknown(_))
    ));
    // A separate authoritative moderation action updates both canonical account
    // JSON and its indexed ban fields while the original Session is gone.
    let raw: Value = db
        .client
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id=$1",
            &[&db.account],
        )
        .unwrap()
        .get(0);
    let mut banned: AccountRecord = serde_json::from_value(raw).unwrap();
    banned.is_banned = true;
    banned.ban_reason = "moderation after uncertain repair".into();
    banned.ban_until_ms = None;
    db.client.execute("UPDATE accounts SET is_banned=true,ban_until_ms=NULL,raw_json=$2,store_version=store_version+1 WHERE account_id=$1",&[&db.account,&serde_json::to_value(&banned).unwrap()]).unwrap();
    let fresh_config = db.config.clone();
    assert!(
        !crate::SimulationSession::standard_login_recovery_preflight(
            &fresh_config,
            &db.account,
            &default_account_password()
        )
        .unwrap(),
        "normal canonical ban validation still rejects login"
    );
    assert!(
        fresh_config.ensure_account_store_writable().is_ok(),
        "the resolved repair cannot strand a global source freeze behind a ban"
    );
    assert!(fresh_config.account_store.lock().unwrap().accounts[&db.account].is_banned);
    assert!(fresh_config.hunting_repair_fence.lock().unwrap().is_none());
    assert!(
        fresh_config
            .lookup_hunting_repair_source(
                &db.account,
                &db.incarnation,
                0,
                &RepairFixture::quote_id(1)
            )
            .is_err(),
        "the internal refresh is not a public receipt authorization bypass"
    );
    assert_eq!(db.operation_counts(), (1, 1, 1, COST as i64));
    assert_eq!(db.results(), 1);
    assert_eq!(db.bound(), Some(100 - COST));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_login_refresh_handles_retired_or_missing_current_owner() {
    for lifecycle in ["recreated", "deleted-character", "deleted-account"] {
        let mut db = RepairFixture::new(0);
        db.register(1, 0);
        assert!(matches!(
            db.config.publish_hunting_repair_source_with_test_fault(
                &db.account,
                &db.incarnation,
                &RepairFixture::quote_id(1),
                db.before.clone(),
                stage_repaired(db.account.clone(), db.before.clone()),
                2
            ),
            Err(HuntingRepairSourceError::OutcomeUnknown(_))
        ));
        if lifecycle == "recreated" {
            db.client.execute("UPDATE characters SET hunting_incarnation=gen_random_uuid() WHERE account_id=$1 AND character_index=0",&[&db.account]).unwrap();
        } else if lifecycle == "deleted-character" {
            let raw: Value = db
                .client
                .query_one(
                    "SELECT raw_json FROM accounts WHERE account_id=$1",
                    &[&db.account],
                )
                .unwrap()
                .get(0);
            let mut current: AccountRecord = serde_json::from_value(raw).unwrap();
            current.characters.clear();
            current.saves.clear();
            let mut tx = db.client.transaction().unwrap();
            tx.execute(
                "UPDATE accounts SET raw_json=$2,store_version=store_version+1 WHERE account_id=$1",
                &[&db.account, &serde_json::to_value(&current).unwrap()],
            )
            .unwrap();
            tx.execute("DELETE FROM characters WHERE account_id=$1", &[&db.account])
                .unwrap();
            tx.commit().unwrap();
        } else {
            db.client
                .execute("DELETE FROM accounts WHERE account_id=$1", &[&db.account])
                .unwrap();
        }
        let fresh = db.config.clone();
        let result = fresh.refresh_account_store_account(&db.account).unwrap();
        assert!(fresh.ensure_account_store_writable().is_ok());
        assert!(fresh.hunting_repair_fence.lock().unwrap().is_none());
        if lifecycle == "deleted-account" {
            assert_eq!(result, AccountSourceRefreshOutcome::Missing);
            let mirror = fresh.account_store.lock().unwrap();
            assert!(!mirror.accounts.contains_key(&db.account));
            assert!(!mirror.source_account_versions.contains_key(&db.account));
            assert!(!mirror.source_save_versions.contains_key(&db.account));
        } else {
            assert_eq!(result, AccountSourceRefreshOutcome::Refreshed);
            if lifecycle == "deleted-character" {
                let mirror = fresh.account_store.lock().unwrap();
                assert!(mirror.accounts[&db.account].characters.is_empty());
                assert!(mirror.accounts[&db.account].saves.is_empty());
            } else {
                let canonical = db.durable();
                let new_witness = fresh
                    .bind_hunting_character_incarnation(&db.account, &canonical)
                    .unwrap()
                    .unwrap();
                assert_ne!(new_witness, db.incarnation);
                assert!(
                    fresh
                        .lookup_hunting_repair_source(
                            &db.account,
                            &new_witness,
                            0,
                            &RepairFixture::quote_id(1)
                        )
                        .is_err(),
                    "new UUID cannot claim the old owner's receipt"
                );
            }
        }
        assert_eq!(db.operation_counts(), (1, 1, 0, 0));
        assert_eq!(
            db.results(),
            1,
            "immutable original evidence survives the lifecycle change"
        );
        assert_eq!(db.bound(), None);
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_postgres_login_refresh_cannot_claim_foreign_or_rebound_fence() {
    let mut db = RepairFixture::new(100);
    db.register(1, COST);
    assert!(matches!(
        db.config.publish_hunting_repair_source_with_test_fault(
            &db.account,
            &db.incarnation,
            &RepairFixture::quote_id(1),
            db.before.clone(),
            stage_repaired(db.account.clone(), db.before.clone()),
            2
        ),
        Err(HuntingRepairSourceError::OutcomeUnknown(_))
    ));
    let memory = db.mirror();
    db.config
        .reconcile_hunting_repair_before_account_refresh("other-account")
        .unwrap();
    assert!(db.config.ensure_account_store_writable().is_err());
    let mut rebound = db.config.clone();
    rebound.account_store_database_url =
        Some("postgres://fixture@127.0.0.1:1/mir2_hunting_acceptance_wrong_binding".into());
    rebound
        .reconcile_hunting_repair_before_account_refresh(&db.account)
        .unwrap();
    assert!(db.config.ensure_account_store_writable().is_err());
    db.config
        .freeze_account_store_writes("foreign publication failure".into());
    let epoch = db.config.account_store_freeze_epoch.load(Ordering::Acquire);
    let reason = match &*db.config.account_store_write_state.lock().unwrap() {
        AccountStoreWriteState::Frozen { reason } => reason.clone(),
        AccountStoreWriteState::Writable => panic!("foreign freeze missing"),
    };
    db.config
        .reconcile_hunting_repair_before_account_refresh(&db.account)
        .unwrap();
    assert!(db
        .config
        .refresh_account_store_account(&db.account)
        .is_err());
    assert!(db.config.ensure_account_store_writable().is_err());
    assert_eq!(
        db.config.account_store_freeze_epoch.load(Ordering::Acquire),
        epoch
    );
    assert!(
        matches!(&*db.config.account_store_write_state.lock().unwrap(),AccountStoreWriteState::Frozen{reason:current} if current==&reason)
    );
    assert_eq!(db.mirror(), memory);
    assert_eq!(db.operation_counts(), (1, 1, 1, COST as i64));
    assert_eq!(db.results(), 1);
}

//! Isolated native PostgreSQL authority fixtures. No chain, signing, payment,
//! user account or deployed service is accessed. These test the actual triggers.
use mir2_simulation::billing::RechargeOffer;
use mir2_simulation::billing_provenance::monthly_revenue_sources;
use mir2_simulation::sui_usdc_billing::{
    UsdcDomain, UsdcOrderState, UsdcPaymentMode, UsdcRechargeOrder,
    VerifiedNativeUsdcTransferReceipt,
};
use mir2_simulation::{apply_migrations, AccountRecord, SimulationConfig, MIGRATIONS};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};

const AT: u64 = 1_800_000_000_000;
const CHAIN: &str = "4c78adac";
const GENESIS: &str = "69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD";
const COIN: &str = mir2_simulation::sui_usdc_billing::TESTNET_USDC_COIN_TYPE;
fn addr(c: char) -> String {
    format!("0x{}", c.to_string().repeat(64))
}
fn id(c: char) -> String {
    format!("usdc_{}", c.to_string().repeat(64))
}

struct Fixture {
    base: String,
    schema: String,
    scoped: String,
    db: Client,
}
impl Fixture {
    fn legacy() -> Option<Self> {
        if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
            eprintln!("SKIPPED native transfer PG: isolated local opt-in required");
            return None;
        }
        let base =
            std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL").expect("isolated local URI required");
        let parsed: Config = base.parse().unwrap();
        assert!(parsed.get_dbname().unwrap_or("").contains("promotion"));
        assert!(parsed.get_options().is_none());
        assert!(parsed
            .get_hosts()
            .iter()
            .all(|h| matches!(h,postgres::config::Host::Tcp(n) if n=="127.0.0.1"||n=="localhost")));
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("promotion_native_transfer_{}_{nonce}", std::process::id());
        Client::connect(&base, NoTls)
            .unwrap()
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let scoped = format!("{base}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000", if base.contains('?') {"&"} else {"?"});
        let mut db = Client::connect(&scoped, NoTls).unwrap();
        assert_eq!(
            MIGRATIONS.last().unwrap().0,
            "0024_sui_usdc_native_transfer_authority"
        );
        // Model the previous deployed migration runner, including its durable
        // history. Without it, opening SimulationConfig would rerun old SQL.
        let mut migration = db.transaction().unwrap();
        migration.batch_execute("CREATE TABLE schema_migrations(version TEXT PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT now())").unwrap();
        for (version, sql) in &MIGRATIONS[..MIGRATIONS.len() - 1] {
            migration.batch_execute(sql).unwrap();
            migration
                .execute(
                    "INSERT INTO schema_migrations(version) VALUES($1)",
                    &[version],
                )
                .unwrap();
        }
        migration.commit().unwrap();
        Some(Self {
            base,
            schema,
            scoped,
            db,
        })
    }
    fn upgrade(&mut self) {
        apply_migrations(&mut self.db).unwrap();
        assert_eq!(
            self.db
                .query_one("SELECT count(*) FROM schema_migrations", &[])
                .unwrap()
                .get::<_, i64>(0),
            MIGRATIONS.len() as i64
        );
    }
    fn empty_account(&mut self, account: &str) {
        self.db
            .execute(
                "INSERT INTO accounts(account_id,raw_json) VALUES($1,$2)",
                &[&account, &json!({"billing":{"suiUsdcOrders":{}}})],
            )
            .unwrap();
    }
    fn credential(&mut self, account: &str, payer: &str) {
        self.db.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms) VALUES($1,$2,'sui_wallet',$3,1)", &[&format!("fixture-{account}"),&account,&payer]).unwrap();
    }
    fn save(&mut self, account: &str, order: &Value) -> Result<u64, postgres::Error> {
        self.db.execute(
            "UPDATE accounts SET raw_json=$1 WHERE account_id=$2",
            &[&raw(order), &account],
        )
    }
    fn raw(&mut self, account: &str) -> Value {
        self.db
            .query_one(
                "SELECT raw_json FROM accounts WHERE account_id=$1",
                &[&account],
            )
            .unwrap()
            .get(0)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        Client::connect(&self.base, NoTls)
            .unwrap()
            .batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .unwrap();
    }
}
fn raw(order: &Value) -> Value {
    let mut orders = serde_json::Map::new();
    orders.insert(order["id"].as_str().unwrap().to_owned(), order.clone());
    json!({"billing":{"suiUsdcOrders":orders}})
}
fn order(c: char, payer: Option<&str>) -> Value {
    json!({"id":id(c),"requestId":format!("fixture-{c}"),"characterIndex":0,
        "offer":{"id":"fixture","label":"Synthetic SQL fixture","currency":"usd","amountMinor":1000,"credits":1000},
        "domain":{"chainIdentifier":CHAIN,"packageId":addr('1'),"configId":addr('2'),"merchant":addr('3'),"coinType":COIN,"ruleVersion":1},
        "creditOrigin":"test","accountRef":addr('4'),"amountMicro":10_000_000u64,
        "createdAtMs":AT,"expiresAtMs":AT+60_000,"capabilityHash":"c".repeat(64),"capabilityExpiresAtMs":AT+60_000,
        "payer":payer,"state":if payer.is_some(){"payerBound"}else{"prepared"},
        "preparedTx":null,"invoice":null,"paymentHint":null,"receipt":null,"applied":null,"jobLease":null,"reviewReason":null})
}
fn direct(c: char, payer: &str) -> Value {
    let mut o = order(c, Some(payer));
    o["paymentMode"] = json!("direct_transfer");
    o
}
fn paid_direct(mut o: Value, digest: &str) -> Value {
    o["state"] = json!("paidPending");
    o["transferReceipt"] = json!({"chainIdentifier":CHAIN,"genesisDigest":GENESIS,"orderId":o["id"],"accountRef":o["accountRef"],
        "payer":o["payer"],"merchant":o["domain"]["merchant"],"coinType":COIN,"amountMicro":10_000_000u64,
        "frozenHash":addr('f'),"digest":digest,"checkpoint":124,"confirmedAtMs":AT+20,"observedAtMs":AT+21});
    o
}
fn paid_invoice(c: char, payer: &str, digest: &str) -> Value {
    let mut o = order(c, Some(payer));
    o["state"] = json!("paidPending");
    o["preparedTx"] =
        json!({"bytesBase64":"TUFDSw==","signatureBase64":"c2ln","digest":"1".repeat(44)});
    o["invoice"] = json!({"invoiceId":addr(c),"createDigest":"1".repeat(44),"checkpoint":123});
    o["receipt"] = json!({"chainIdentifier":CHAIN,"packageId":addr('1'),"configId":addr('2'),"invoiceId":addr(c),"orderId":o["id"],
        "accountRef":o["accountRef"],"payer":payer,"merchant":addr('3'),"coinType":COIN,"amountMicro":10_000_000u64,
        "frozenHash":addr('f'),"digest":digest,"eventSeq":0,"checkpoint":124,"confirmedAtMs":AT+20});
    o
}

#[test]
fn native_transfer_migration_preserves_legacy_json_and_refuses_ambiguous_digest_backfill() {
    let Some(mut f) = Fixture::legacy() else {
        return;
    };
    f.empty_account("legacy");
    let o = paid_invoice('5', &addr('8'), &"2".repeat(44));
    f.save("legacy", &o).unwrap();
    let before=f.db.query_one("SELECT raw_json,xmin::text,store_version,updated_at::text FROM accounts WHERE account_id='legacy'",&[]).unwrap();
    let before: (Value, String, i64, String) =
        (before.get(0), before.get(1), before.get(2), before.get(3));
    {
        let mut tx = f.db.transaction().unwrap();
        tx.batch_execute("INSERT INTO sui_usdc_payment_receipts SELECT chain_identifier,transaction_digest,event_seq+1,invoice_id||'-collision',account_id||'-collision',order_id||'-collision',receipt_json FROM sui_usdc_payment_receipts").unwrap();
        let error = tx.batch_execute(MIGRATIONS.last().unwrap().1).unwrap_err();
        assert!(error
            .as_db_error()
            .unwrap()
            .message()
            .contains("multiple payment claims"));
        tx.rollback().unwrap();
    }
    f.upgrade();
    f.upgrade();
    let after=f.db.query_one("SELECT raw_json,xmin::text,store_version,updated_at::text FROM accounts WHERE account_id='legacy'",&[]).unwrap();
    let after: (Value, String, i64, String) =
        (after.get(0), after.get(1), after.get(2), after.get(3));
    assert_eq!(
        before, after,
        "migration must not rewrite old account JSON, row version or timestamps"
    );
    assert_eq!(
        f.db.query_one(
            "SELECT receipt_json,payment_mode FROM sui_usdc_transaction_claims",
            &[]
        )
        .unwrap()
        .get::<_, Value>(0),
        o["receipt"]
    );
    f.save("legacy", &o).unwrap();
    assert_eq!(
        f.db.query_one("SELECT count(*) FROM sui_usdc_payment_receipts", &[])
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        f.db.query_one("SELECT active FROM sui_usdc_payer_windows", &[])
            .unwrap()
            .get::<_, bool>(0),
        false
    );
}

#[test]
fn native_transfer_receipts_modes_credentials_progress_and_digest_claims_fail_closed() {
    let Some(mut f) = Fixture::legacy() else {
        return;
    };
    f.upgrade();
    f.empty_account("direct");
    let payer = addr('8');
    let mut o = direct('6', &payer);
    assert!(
        f.save("direct", &o).is_err(),
        "raw SQL cannot bind an unverified payer"
    );
    f.credential("direct", &payer);
    let mut prepared = o.clone();
    prepared["payer"] = Value::Null;
    prepared["state"] = json!("prepared");
    assert!(f.save("direct", &prepared).is_err());
    f.save("direct", &o).unwrap();
    let mut missing_cursor = o.clone();
    missing_cursor["directScanComplete"] = json!(true);
    assert!(f.save("direct", &missing_cursor).is_err());
    o["directScanCursor"] = json!("{\"page\":1}");
    o["directScanComplete"] = json!(false);
    f.save("direct", &o).unwrap();
    let mut erase = o.clone();
    erase.as_object_mut().unwrap().remove("directScanCursor");
    assert!(f.save("direct", &erase).is_err());
    o["directScanCursor"] = json!("{\"page\":2}");
    o["directScanComplete"] = json!(true);
    f.save("direct", &o).unwrap();
    let mut regress = o.clone();
    regress["directScanComplete"] = json!(false);
    assert!(f.save("direct", &regress).is_err());
    for cursor in [json!("[]"), json!("not json"), json!(" ".repeat(4097))] {
        let mut invalid = o.clone();
        invalid["directScanCursor"] = cursor;
        assert!(f.save("direct", &invalid).is_err());
    }
    let paid = paid_direct(o.clone(), &"3".repeat(44));
    for (field, value) in [
        ("genesisDigest", json!("wrong")),
        ("payer", json!(addr('9'))),
        ("amountMicro", json!(9)),
        ("confirmedAtMs", json!(AT + 60_000)),
        ("observedAtMs", json!(AT + 19)),
        ("checkpoint", json!(0)),
        ("futureAuthority", json!(1)),
    ] {
        let mut invalid = paid.clone();
        invalid["transferReceipt"][field] = value;
        assert!(f.save("direct", &invalid).is_err(), "{field}");
    }
    f.save("direct", &paid).unwrap();
    f.save("direct", &paid).unwrap();
    let stable = f.raw("direct");
    for field in [
        "paymentMode",
        "transferReceipt",
        "directScanCursor",
        "directScanComplete",
    ] {
        let mut invalid = paid.clone();
        invalid.as_object_mut().unwrap().remove(field);
        assert!(f.save("direct", &invalid).is_err(), "{field}");
    }
    let mut changed = paid.clone();
    changed["transferReceipt"]["digest"] = json!("4".repeat(44));
    assert!(f.save("direct", &changed).is_err());
    let mut unknown = paid.clone();
    unknown["futureMoneyAuthority"] = json!({"mustKeep":1});
    assert!(f.save("direct", &unknown).is_err());
    assert_eq!(stable, f.raw("direct"));
    f.empty_account("invoice-replay");
    let invoice = paid_invoice('7', &addr('9'), &"3".repeat(44));
    assert!(
        f.save("invoice-replay", &invoice).is_err(),
        "invoice cannot reuse a direct digest"
    );
    let invoice = paid_invoice('7', &addr('9'), &"4".repeat(44));
    f.save("invoice-replay", &invoice).unwrap();
    let mut replay = direct('a', &payer);
    replay["createdAtMs"] = json!(AT + 60_000);
    replay["expiresAtMs"] = json!(AT + 120_000);
    let mut replay = paid_direct(replay, &"4".repeat(44));
    replay["transferReceipt"]["confirmedAtMs"] = json!(AT + 60_020);
    replay["transferReceipt"]["observedAtMs"] = json!(AT + 60_021);
    let mut both = stable.clone();
    both["billing"]["suiUsdcOrders"][id('a')] = replay;
    assert!(
        f.db.execute(
            "UPDATE accounts SET raw_json=$1 WHERE account_id='direct'",
            &[&both]
        )
        .is_err(),
        "direct cannot reuse invoice digest"
    );
    assert_eq!(
        f.db.query_one("SELECT count(*) FROM sui_usdc_transaction_claims", &[])
            .unwrap()
            .get::<_, i64>(0),
        2
    );
    assert_eq!(
        f.db.query_one(
            "SELECT active FROM sui_usdc_payer_windows WHERE account_id='direct'",
            &[]
        )
        .unwrap()
        .get::<_, bool>(0),
        false
    );
}

#[test]
fn native_transfer_clean_prepared_upgrade_and_two_connection_window_fence_are_atomic() {
    let Some(mut f) = Fixture::legacy() else {
        return;
    };
    f.upgrade();
    f.empty_account("direct");
    f.empty_account("invoice");
    let payer = addr('8');
    f.credential("direct", &payer);
    let clean = order('b', None);
    f.save("direct", &clean).unwrap();
    let mut upgraded = clean.clone();
    upgraded["paymentMode"] = json!("direct_transfer");
    upgraded["payer"] = json!(payer);
    upgraded["state"] = json!("payerBound");
    let mut first = Client::connect(&f.scoped, NoTls).unwrap();
    let mut tx = first.transaction().unwrap();
    tx.execute(
        "UPDATE accounts SET raw_json=$1 WHERE account_id='direct'",
        &[&raw(&upgraded)],
    )
    .unwrap();
    let blocker: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    let scoped = f.scoped.clone();
    let invoice = order('c', Some(&payer));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let join = std::thread::spawn(move || {
        let mut writer = Client::connect(&scoped, NoTls).unwrap();
        let writer_pid: i32 = writer
            .query_one("SELECT pg_backend_pid()", &[])
            .unwrap()
            .get(0);
        ready_tx.send(writer_pid).unwrap();
        writer.execute(
            "UPDATE accounts SET raw_json=$1 WHERE account_id='invoice'",
            &[&raw(&invoice)],
        )
    });
    let writer_pid = ready_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("second PostgreSQL connection must be established");
    let mut blocked = false;
    for _ in 0..100 {
        // pg_stat_activity caches a statistics snapshot inside a transaction.
        // This independent autocommit observer checks the known backend's live
        // lock blockers, so connection creation cannot be hidden by that cache.
        blocked =
            f.db.query_one(
                "SELECT $1=ANY(pg_blocking_pids($2))",
                &[&blocker, &writer_pid],
            )
            .unwrap()
            .get(0);
        if blocked || join.is_finished() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    tx.commit().unwrap();
    let second_error = join
        .join()
        .unwrap()
        .expect_err("second unresolved payment window must be rejected");
    let second_error = second_error
        .as_db_error()
        .expect("expected a PostgreSQL authority rejection, not a transport failure");
    assert_eq!(
        second_error.code().code(),
        "23505",
        "unexpected SQLSTATE: {}",
        second_error.message()
    );
    assert!(
        second_error.message().contains("windows overlap"),
        "unexpected SQL rejection: {}",
        second_error.message()
    );
    assert!(
        blocked,
        "independent account writer must first wait on the shared payer lock; returned SQLSTATE {} / {}",
        second_error.code().code(), second_error.message()
    );
    assert_eq!(
        f.db.query_one("SELECT count(*) FROM sui_usdc_payer_windows", &[])
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    let paid = paid_direct(upgraded, &"5".repeat(44));
    f.save("direct", &paid).unwrap();
    let invoice = order('c', Some(&payer));
    f.save("invoice", &invoice).unwrap();
    assert_eq!(
        f.db.query_one(
            "SELECT count(*) FROM sui_usdc_payer_windows WHERE active",
            &[]
        )
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    let mut admission = f.raw("direct");
    admission["billing"]["suiUsdcOrders"][id('a')] =
        paid_direct(direct('a', &payer), &"6".repeat(44));
    let admission_error =
        f.db.execute(
            "UPDATE accounts SET raw_json=$1 WHERE account_id='direct'",
            &[&admission],
        )
        .unwrap_err();
    assert!(
        admission_error
            .as_db_error()
            .unwrap()
            .message()
            .contains("windows overlap"),
        "a receipt in the first bind cannot bypass an unresolved conflicting window"
    );
    assert!(
        f.save("direct", &clean).is_err(),
        "a default-mode older writer cannot revert a converted order"
    );
    assert!(f
        .db
        .execute("DELETE FROM accounts WHERE account_id='direct'", &[])
        .is_err());
    // Only a truly clean legacy Prepared row may change mode. A persisted hint
    // is authority, even though it cannot independently fund credits.
    f.empty_account("hinted");
    let mut hinted = order('d', None);
    hinted["paymentHint"] = json!("6".repeat(44));
    f.save("hinted", &hinted).unwrap();
    f.db.execute(
        "UPDATE identity_credentials SET revoked_at_ms=1 WHERE account_id='direct'",
        &[],
    )
    .unwrap();
    f.credential("hinted", &payer);
    hinted["paymentMode"] = json!("direct_transfer");
    hinted["payer"] = json!(payer);
    hinted["state"] = json!("payerBound");
    assert!(
        f.save("hinted", &hinted).is_err(),
        "hinted legacy rows cannot switch payment protocol"
    );
}

fn simulation(url: &str) -> SimulationConfig {
    SimulationConfig::default()
        .with_postgres_account_store(url.to_owned())
        .unwrap()
        .with_billing_monthly_card_credit_price(Some(600))
        .unwrap()
}
fn typed_receipt(
    order: &UsdcRechargeOrder,
    digest: &str,
    paid: u64,
    observed: u64,
) -> VerifiedNativeUsdcTransferReceipt {
    VerifiedNativeUsdcTransferReceipt {
        chain_identifier: order.domain.chain_identifier.clone(),
        genesis_digest: GENESIS.into(),
        order_id: order.id.clone(),
        account_ref: order.account_ref.clone(),
        payer: order.payer.clone().unwrap(),
        merchant: order.domain.merchant.clone(),
        coin_type: order.domain.coin_type.clone(),
        amount_micro: order.amount_micro,
        frozen_hash: order.frozen_hash().unwrap(),
        digest: digest.into(),
        checkpoint: 124,
        confirmed_at_ms: paid,
        observed_at_ms: observed,
    }
}

#[test]
fn native_transfer_public_simulation_methods_preserve_wallet_authority_fifo_restart_and_provenance()
{
    let Some(mut f) = Fixture::legacy() else {
        return;
    };
    f.upgrade();
    let c = simulation(&f.scoped);
    for (account, name) in [("member", "DirectMember"), ("other", "DirectOther")] {
        let mut ch = c.default_character.clone();
        ch.index = 0;
        ch.name = name.into();
        c.account_store
            .lock()
            .unwrap()
            .accounts
            .insert(account.into(), AccountRecord::new(ch));
        c.save_account_store_account(account).unwrap();
    }
    let payer = addr('8');
    f.credential("member", &payer);
    f.credential("other", &addr('9'));
    let domain = UsdcDomain {
        chain_identifier: CHAIN.into(),
        package_id: addr('1'),
        config_id: addr('2'),
        merchant: addr('3'),
        coin_type: COIN.into(),
        rule_version: 1,
    };
    let offer = RechargeOffer {
        id: "native-transfer-fixture".into(),
        label: "Synthetic fullnode receipt, not payment".into(),
        currency: "usd".into(),
        amount_minor: 1000,
        credits: 1000,
    };
    let cap = "c".repeat(64);
    for (account, credential, wallet) in [
        ("member", "missing", payer.clone()),
        ("member", "fixture-other", payer.clone()),
        ("member", "fixture-member", addr('9')),
        ("other", "fixture-member", payer.clone()),
    ] {
        assert!(c
            .prepare_direct_usdc_recharge_order(
                account,
                0,
                "bad-wallet",
                &offer,
                &domain,
                credential,
                &wallet,
                &cap,
                AT + 60_000,
                AT + 60_000,
                AT + 10
            )
            .is_err());
    }
    f.db.execute(
        "UPDATE identity_credentials SET revoked_at_ms=1 WHERE credential_id='fixture-member'",
        &[],
    )
    .unwrap();
    assert!(c
        .prepare_direct_usdc_recharge_order(
            "member",
            0,
            "revoked-wallet",
            &offer,
            &domain,
            "fixture-member",
            &payer,
            &cap,
            AT + 60_000,
            AT + 60_000,
            AT + 10
        )
        .is_err());
    f.db.execute(
        "UPDATE identity_credentials SET revoked_at_ms=NULL WHERE credential_id='fixture-member'",
        &[],
    )
    .unwrap();
    let o = c
        .prepare_direct_usdc_recharge_order(
            "member",
            0,
            "direct-one",
            &offer,
            &domain,
            "fixture-member",
            &payer,
            &cap,
            AT + 60_000,
            AT + 60_000,
            AT + 10,
        )
        .unwrap();
    assert_eq!(o.payment_mode, UsdcPaymentMode::DirectTransfer);
    assert_eq!(o.state, UsdcOrderState::PayerBound);
    assert!(
        o.invoice.is_none()
            && o.prepared_tx.is_none()
            && o.receipt.is_none()
            && o.transfer_receipt.is_none()
    );
    let retried = c
        .prepare_direct_usdc_recharge_order(
            "member",
            0,
            "direct-one",
            &offer,
            &domain,
            "fixture-member",
            &payer,
            &cap,
            AT + 120_000,
            AT + 120_000,
            AT + 11,
        )
        .unwrap();
    assert_eq!(
        retried, o,
        "retry must preserve original immutable payment window"
    );
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("member", 0, AT + 12)
            .unwrap()
            .granted,
        0
    );
    let receipt = typed_receipt(&o, &"7".repeat(44), AT + 20, AT + 21);
    let mut wrong = receipt.clone();
    wrong.payer = addr('9');
    assert!(c
        .confirm_usdc_native_transfer_payment("member", &o.id, &wrong, AT + 22)
        .is_err());
    for _ in 0..3 {
        c.confirm_usdc_native_transfer_payment("member", &o.id, &receipt, AT + 22)
            .unwrap();
    }
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("member", 0, AT + 23)
            .unwrap()
            .granted,
        1000
    );
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("member", 0, AT + 24)
            .unwrap()
            .granted,
        0
    );
    let restarted = simulation(&f.scoped);
    assert_eq!(
        restarted.recharge_credit_status("member", 0).unwrap(),
        (1000, 0)
    );
    assert_eq!(
        restarted
            .usdc_recharge_order("member", &o.id)
            .unwrap()
            .state,
        UsdcOrderState::Applied
    );
    let mut later = receipt.clone();
    later.observed_at_ms = AT + 25;
    restarted
        .confirm_usdc_native_transfer_payment("member", &o.id, &later, AT + 26)
        .unwrap();
    assert_eq!(
        restarted
            .usdc_recharge_order("member", &o.id)
            .unwrap()
            .transfer_receipt
            .as_ref(),
        Some(&receipt)
    );
    let card = restarted
        .buy_monthly_card_for_character("member", 0, "direct-monthly-one", AT + 30)
        .unwrap();
    restarted
        .activate_monthly_card_for_character("member", 0, card.unique_id, AT + 31)
        .unwrap();
    let restarted_again = simulation(&f.scoped);
    assert_eq!(
        restarted_again.recharge_credit_status("member", 0).unwrap(),
        (400, 0)
    );
    assert_eq!(
        restarted_again
            .buy_monthly_card_for_character("member", 0, "direct-monthly-one", AT + 32)
            .unwrap()
            .unique_id,
        card.unique_id
    );
    let member = restarted_again.account_store.lock().unwrap().accounts["member"].clone();
    let sources = monthly_revenue_sources(&member, "member", false).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].eligible_micro_usd, 6_000_000);
    assert_eq!(sources[0].parts.len(), 1);
    assert!(sources[0].parts[0].source.is_none());
    let evidence = sources[0].parts[0].native_transfer_source.as_ref().unwrap();
    assert_eq!(evidence.network, "sui:testnet");
    assert_eq!(evidence.receipt, receipt);
    let production_sources = monthly_revenue_sources(&member, "member", true).unwrap();
    assert_eq!(production_sources.len(), 1);
    assert_eq!(production_sources[0].eligible_micro_usd, 0);
    assert_eq!(production_sources[0].parts[0].eligible_micro_usd, 0);
    assert_eq!(
        production_sources[0].parts[0].reason_code,
        "payment_environment_mismatch"
    );
    assert_eq!(
        f.db.query_one("SELECT count(*) FROM sui_usdc_transaction_claims", &[])
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        f.db.query_one("SELECT count(*) FROM sui_usdc_invoice_receipts", &[])
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        f.db.query_one("SELECT count(*) FROM sui_usdc_payment_receipts", &[])
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    // Recovery retains the old quote and payment window, even when discovered
    // after expiry. Only a finalized payment inside that window may fund it.
    let old = restarted_again
        .prepare_usdc_recharge_order(
            "member",
            0,
            "old-prepared",
            &offer,
            &domain,
            &cap,
            AT + 120_000,
            AT + 100_000,
            AT + 60_001,
        )
        .unwrap();
    let recovered = restarted_again
        .recover_prepared_usdc_native_transfer(
            "member",
            &old.id,
            "fixture-member",
            &payer,
            &"8".repeat(44),
            AT + 200_000,
        )
        .unwrap();
    assert_eq!(recovered.id, old.id);
    assert_eq!(recovered.created_at_ms, old.created_at_ms);
    assert_eq!(recovered.expires_at_ms, old.expires_at_ms);
    assert_eq!(recovered.offer, old.offer);
    assert_eq!(recovered.domain, old.domain);
    let old_receipt = typed_receipt(&recovered, &"8".repeat(44), AT + 60_020, AT + 200_001);
    restarted_again
        .confirm_usdc_native_transfer_payment("member", &old.id, &old_receipt, AT + 200_002)
        .unwrap();
    assert_eq!(
        restarted_again
            .consume_inactive_pending_recharge_credits("member", 0, AT + 200_003)
            .unwrap()
            .granted,
        1000
    );
    let final_open = simulation(&f.scoped);
    final_open
        .recover_prepared_usdc_native_transfer(
            "member",
            &old.id,
            "fixture-member",
            &payer,
            &"8".repeat(44),
            AT + 200_004,
        )
        .unwrap();
    assert_eq!(
        final_open
            .consume_inactive_pending_recharge_credits("member", 0, AT + 200_005)
            .unwrap()
            .granted,
        0
    );
    assert_eq!(
        final_open.recharge_credit_status("member", 0).unwrap(),
        (1400, 0)
    );
    assert_eq!(
        f.db.query_one("SELECT count(*) FROM sui_usdc_transaction_claims", &[])
            .unwrap()
            .get::<_, i64>(0),
        2
    );
}

//! Native PG authority/FIFO/import verification. Fullnode receipts below are
//! explicitly fixtures; no chain signing, payment or deployed account is used.
use mir2_simulation::billing::RechargeOffer;
use mir2_simulation::billing_provenance::monthly_revenue_sources;
use mir2_simulation::promotion::{PromotionRules, PromotionStore};
use mir2_simulation::sui_usdc_billing::*;
use mir2_simulation::{apply_migrations, AccountRecord, SimulationConfig};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};
const AT: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;
fn addr(c: char) -> String {
    format!("0x{}", c.to_string().repeat(64))
}
fn open(url: &str) -> SimulationConfig {
    SimulationConfig::default()
        .with_postgres_account_store(url.to_owned())
        .unwrap()
        .with_billing_monthly_card_credit_price(Some(600))
        .unwrap()
}
fn control(db: &mut Client, env: &str) {
    let rules = serde_json::to_value(PromotionRules::default()).unwrap();
    db.execute("INSERT INTO promotion_campaigns(environment,campaign_id,terms_version,status,starts_at_ms,ends_at_ms,rules_json,rules_approved,commission_enabled,budget_micro_usd,reserved_micro_usd,member_limit,leader_limit,per_leader_member_limit,per_member_revenue_cap_micro_usd,configured_by,approved_by,created_at_ms) VALUES($1,'sui-season','fixture-terms','active',$2,$3,$4,TRUE,$5,100000000,3000000,10,2,10,30000000,'fixture-op','fixture-reviewer',$2)",&[&env,&(AT as i64),&((AT+90*DAY)as i64),&rules,&(env!="production")]).unwrap();
    db.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,status,accepted_terms_version,applied_at_ms) VALUES($1,'sui-season','leader','sui-leader',$2,0,'Leader','approved','fixture-terms',$3)",&[&env,&format!("sui-{env}"),&(AT as i64)]).unwrap();
    db.execute("INSERT INTO promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id,first_clicked_at_ms,expires_at_ms,source) VALUES($1,'sui-season',$2,'member','sui-leader',$3,$4,'manual_code')",&[&env,&format!("sui-{env}"),&((AT+1)as i64),&((AT+DAY)as i64)]).unwrap();
    db.execute("INSERT INTO promotion_attributions(environment,campaign_id,account_id,leader_id,candidate_id,first_clicked_at_ms,registered_at_ms,confirmed_at_ms,terms_version,source,reserved_micro_usd) VALUES($1,'sui-season','member','sui-leader',$2,$3,$4,$5,'fixture-terms','manual_code',3000000)",&[&env,&format!("sui-{env}"),&((AT+1)as i64),&((AT+2)as i64),&((AT+5)as i64)]).unwrap();
}
#[test]
fn native_sui_receipt_fences_fifo_and_monthly_import_are_replay_safe() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIPPED native Sui PG: explicit isolated opt-in required");
        return;
    }
    let url =
        std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL").expect("private isolated URI required");
    let parsed: Config = url.parse().unwrap();
    assert!(parsed.get_dbname().unwrap_or("").contains("promotion"));
    assert!(parsed.get_options().is_none());
    assert!(parsed
        .get_hosts()
        .iter()
        .all(|h| matches!(h,postgres::config::Host::Tcp(n) if n=="127.0.0.1"||n=="localhost")));
    let mut base = Client::connect(&url, NoTls).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("promotion_sui_{}_{nonce}", std::process::id());
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let scoped=format!("{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",if url.contains('?'){"&"}else{"?"});
    let mut db = Client::connect(&scoped, NoTls).unwrap();
    apply_migrations(&mut db).unwrap();
    apply_migrations(&mut db).unwrap();
    assert_eq!(
        db.query_one("SELECT count(*) FROM schema_migrations", &[])
            .unwrap()
            .get::<_, i64>(0),
        24
    );
    let c = open(&scoped);
    for name in ["leader", "member"] {
        let mut ch = c.default_character.clone();
        ch.index = 0;
        ch.name = if name == "leader" {
            "SuiLeader"
        } else {
            "SuiMember"
        }
        .into();
        let account = AccountRecord::new(ch);
        c.account_store
            .lock()
            .unwrap()
            .accounts
            .insert(name.into(), account);
        c.save_account_store_account(name).unwrap();
    }
    control(&mut db, "test");
    control(&mut db, "production");
    let domain = UsdcDomain {
        chain_identifier: TESTNET_CHAIN_IDENTIFIER.into(),
        package_id: addr('1'),
        config_id: addr('2'),
        merchant: addr('3'),
        coin_type: TESTNET_USDC_COIN_TYPE.into(),
        rule_version: 1,
    };
    let offer = RechargeOffer {
        id: "sui-native-fixture".into(),
        label: "Explicit fixture; not external payment".into(),
        currency: "usd".into(),
        amount_minor: 1000,
        credits: 1000,
    };
    let cap = "c".repeat(64);
    let o = c
        .prepare_usdc_recharge_order(
            "member",
            0,
            "sui-order-one",
            &offer,
            &domain,
            &cap,
            AT + 1_800_000,
            AT + 1_200_000,
            AT + 10,
        )
        .unwrap();
    c.bind_usdc_recharge_payer("member", &o.id, &addr('4'), &cap, AT + 11)
        .unwrap();
    c.claim_usdc_recharge_job("member", &o.id, "native-worker", AT + 12)
        .unwrap()
        .unwrap();
    c.record_usdc_prepared_transaction(
        "member",
        &o.id,
        "native-worker",
        &UsdcPreparedTransaction {
            bytes_base64: "TUFDSw==".into(),
            signature_base64: "c2ln".into(),
            digest: "1".repeat(44),
        },
        AT + 13,
    )
    .unwrap();
    let o = c
        .bind_usdc_recharge_invoice(
            "member",
            &o.id,
            "native-worker",
            &UsdcInvoiceBinding {
                invoice_id: addr('5'),
                create_digest: "1".repeat(44),
                checkpoint: 123,
            },
            AT + 14,
        )
        .unwrap();
    let receipt = VerifiedUsdcReceipt {
        chain_identifier: domain.chain_identifier.clone(),
        package_id: domain.package_id.clone(),
        config_id: domain.config_id.clone(),
        invoice_id: addr('5'),
        order_id: o.id.clone(),
        account_ref: o.account_ref.clone(),
        payer: addr('4'),
        merchant: domain.merchant.clone(),
        coin_type: domain.coin_type.clone(),
        amount_micro: 10_000_000,
        frozen_hash: o.frozen_hash().unwrap(),
        digest: "2".repeat(44),
        event_seq: 0,
        checkpoint: 124,
        confirmed_at_ms: AT + 20,
    };
    c.submit_usdc_payment_hint("member", &o.id, &cap, &receipt.digest, AT + 21)
        .unwrap();
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("member", 0, AT + 22)
            .unwrap()
            .granted,
        0
    );
    for _ in 0..3 {
        c.confirm_usdc_recharge_payment("member", &o.id, &receipt, AT + 23)
            .unwrap();
    }
    assert_eq!(
        db.query_one("SELECT count(*) FROM sui_usdc_payment_receipts", &[])
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        db.query_one("SELECT count(*) FROM sui_usdc_invoice_receipts", &[])
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    c.consume_inactive_pending_recharge_credits("member", 0, AT + 24)
        .unwrap();
    let card = c
        .buy_monthly_card_for_character("member", 0, "normal-sui-monthly", AT + 30)
        .unwrap();
    c.activate_monthly_card_for_character("member", 0, card.unique_id, AT + 31)
        .unwrap();
    let restarted = open(&scoped);
    assert_eq!(
        restarted.recharge_credit_status("member", 0).unwrap(),
        (400, 0)
    );
    let account = restarted.account_store.lock().unwrap().accounts["member"].clone();
    let sources = monthly_revenue_sources(&account, "member", false).unwrap();
    assert_eq!(sources[0].eligible_micro_usd, 6_000_000);
    assert_eq!(
        sources[0].parts[0].source.as_ref().unwrap().receipt,
        receipt
    );
    assert!(account.billing.as_ref().unwrap().orders.is_empty());
    let store = PromotionStore::new(scoped.clone(), "test").unwrap();
    assert_eq!(
        store.reconcile_live_monthly_revenue(AT + 40).unwrap()["allocationsImported"],
        1
    );
    assert_eq!(
        store.reconcile_live_monthly_revenue(AT + 41).unwrap()["allocationsImported"],
        0
    );
    assert_eq!(
        db.query_one(
            "SELECT eligible_micro_usd FROM promotion_revenue_allocations WHERE environment='test'",
            &[]
        )
        .unwrap()
        .get::<_, i64>(0),
        6_000_000
    );
    let observed:Value=db.query_one("SELECT source_json FROM promotion_monthly_source_observations WHERE environment='test'",&[]).unwrap().get(0);
    assert_eq!(
        observed["parts"][0]["source"]["receipt"]["digest"],
        receipt.digest
    );
    assert_eq!(observed["parts"][0]["source"]["network"], "sui:testnet");
    let prod = PromotionStore::new(scoped.clone(), "production").unwrap();
    assert_eq!(
        prod.reconcile_live_monthly_revenue(AT + 42).unwrap()["allocationsImported"],
        0
    );
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM promotion_revenue_allocations WHERE environment='production'",
            &[]
        )
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    let snapshot: Value = db
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id='member'",
            &[],
        )
        .unwrap()
        .get(0);
    let mut forged = snapshot.clone();
    forged["billing"]["suiUsdcOrders"][&o.id]["receipt"]["digest"] = json!("3".repeat(44));
    assert!(db
        .execute(
            "UPDATE accounts SET raw_json=$1 WHERE account_id='member'",
            &[&forged]
        )
        .is_err());
    let after: Value = db
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id='member'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(snapshot, after);
    assert_eq!(
        restarted
            .consume_inactive_pending_recharge_credits("member", 0, AT + 50)
            .unwrap()
            .granted,
        0
    );
    assert_eq!(
        restarted
            .buy_monthly_card_for_character("member", 0, "normal-sui-monthly", AT + 51)
            .unwrap()
            .unique_id,
        card.unique_id
    );
    println!("Native PG Sui fixture verified: immutable invoice/payment registry, hint cannot fund, once-only credit/FIFO, normal monthly 6 testUSD, full chain provenance, replay/restart, production rejection. No external payment or deployed writes.");
    drop(db);
    drop(c);
    drop(restarted);
    drop(store);
    drop(prod);
    base.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}

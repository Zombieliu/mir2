//! Original Windows database compatibility. All amounts are local fixtures.
use mir2_simulation::billing::{RechargeOffer, RechargeOrder, VerifiedRechargePayment};
use mir2_simulation::sui_usdc_billing::*;
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SimulationConfig, MIGRATIONS,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OriginalBillingReader {
    orders: BTreeMap<String, RechargeOrder>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    sui_usdc_orders: BTreeMap<String, OriginalV13UsdcOrderReader>,
}
// Freeze the historical wire shape: reusing today's Order type would let a
// newly added authority field silently weaken this older-writer regression.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OriginalV13UsdcOrderReader {
    id: String,
    request_id: String,
    character_index: i32,
    offer: RechargeOffer,
    domain: UsdcDomain,
    credit_origin: mir2_simulation::procurement_fees::CreditOrigin,
    account_ref: String,
    amount_micro: u64,
    created_at_ms: u64,
    expires_at_ms: u64,
    capability_hash: String,
    capability_expires_at_ms: u64,
    payer: Option<String>,
    state: UsdcOrderState,
    prepared_tx: Option<UsdcPreparedTransaction>,
    invoice: Option<UsdcInvoiceBinding>,
    payment_hint: Option<String>,
    receipt: Option<VerifiedUsdcReceipt>,
    applied: Option<mir2_simulation::billing::RechargeAppliedReceipt>,
    job_lease: Option<UsdcJobLease>,
    review_reason: Option<String>,
}
// Exact original account field set including procurement and metadata.
#[derive(Serialize, Deserialize)]
struct OriginalAccountReader {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub procurement: Option<mir2_simulation::procurement::ProcurementLedger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activities: Option<mir2_simulation::activities::ActivityLedger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub billing: Option<OriginalBillingReader>,
    #[serde(
        rename = "monthlyCard",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub monthly_card: Option<mir2_simulation::monthly_card::MonthlyCardLedger>,
    pub password: String,
    pub storage_size: u16,
    #[serde(default)]
    pub has_expanded_storage: bool,
    #[serde(default)]
    pub expanded_storage_expiry_time_binary_datetime: i64,
    #[serde(default)]
    pub storage_password: String,
    #[serde(default)]
    pub storage_password_last_set_binary_datetime: i64,
    #[serde(default)]
    pub is_banned: bool,
    #[serde(default)]
    pub ban_reason: String,
    #[serde(default)]
    pub ban_until_ms: Option<u64>,
    #[serde(default)]
    pub banned_at_ms: Option<u64>,
    /// GM rank for in-game `@` command access (0 = normal player). Set by admin
    /// tooling / account provisioning; never granted implicitly.
    #[serde(default)]
    pub gm_level: u8,
    /// Crystal `CharacterInfo.LastLogoutDate`, keyed by character index.
    ///
    /// This lives on the account record so the select roster can be produced
    /// without loading a private in-world save. Legacy stores deserialize as
    /// `Never` (zero) until that character next leaves the world.
    #[serde(default)]
    pub character_last_access_binary_datetimes: BTreeMap<i32, i64>,
    pub characters: Vec<CharacterRecord>,
    pub saves: BTreeMap<i32, CharacterSaveRecord>,
}

const AT: u64 = 1_800_000_000_000;
fn addr(c: char) -> String {
    format!("0x{}", c.to_string().repeat(64))
}
fn historical_account() -> AccountRecord {
    let c = SimulationConfig::default()
        .with_billing_monthly_card_credit_price(Some(600))
        .unwrap();
    let offer = RechargeOffer {
        id: "compat-usd-10".into(),
        label: "Local fixture".into(),
        currency: "usd".into(),
        amount_minor: 1000,
        credits: 1000,
    };
    let stripe = c
        .prepare_recharge_order("demo", 0, "compat-stripe", &offer, AT, false)
        .unwrap();
    c.confirm_recharge_payment(
        "demo",
        &stripe.id,
        &VerifiedRechargePayment {
            session_id: "cs_compat".into(),
            payment_intent_id: "pi_compat".into(),
            currency: "usd".into(),
            amount_minor: 1000,
            livemode: false,
        },
        AT + 1,
    )
    .unwrap();
    c.consume_inactive_pending_recharge_credits("demo", 0, AT + 2)
        .unwrap();
    let domain = UsdcDomain {
        chain_identifier: TESTNET_CHAIN_IDENTIFIER.into(),
        package_id: addr('1'),
        config_id: addr('2'),
        merchant: addr('3'),
        coin_type: TESTNET_USDC_COIN_TYPE.into(),
        rule_version: 1,
    };
    let cap = "c".repeat(64);
    let o = c
        .prepare_usdc_recharge_order(
            "demo",
            0,
            "compat-sui",
            &offer,
            &domain,
            &cap,
            AT + 1_800_000,
            AT + 1_200_000,
            AT + 3,
        )
        .unwrap();
    c.bind_usdc_recharge_payer("demo", &o.id, &addr('4'), &cap, AT + 4)
        .unwrap();
    c.claim_usdc_recharge_job("demo", &o.id, "compat-worker", AT + 5)
        .unwrap()
        .unwrap();
    c.record_usdc_prepared_transaction(
        "demo",
        &o.id,
        "compat-worker",
        &UsdcPreparedTransaction {
            bytes_base64: "TUFDSw==".into(),
            signature_base64: "c2ln".into(),
            digest: "1".repeat(44),
        },
        AT + 6,
    )
    .unwrap();
    let o = c
        .bind_usdc_recharge_invoice(
            "demo",
            &o.id,
            "compat-worker",
            &UsdcInvoiceBinding {
                invoice_id: addr('5'),
                create_digest: "1".repeat(44),
                checkpoint: 123,
            },
            AT + 7,
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
        merchant: domain.merchant,
        coin_type: domain.coin_type,
        amount_micro: o.amount_micro,
        frozen_hash: o.frozen_hash().unwrap(),
        digest: "2".repeat(44),
        event_seq: 0,
        checkpoint: 124,
        confirmed_at_ms: AT + 8,
    };
    c.confirm_usdc_recharge_payment("demo", &o.id, &receipt, AT + 9)
        .unwrap();
    c.consume_inactive_pending_recharge_credits("demo", 0, AT + 10)
        .unwrap();
    let monthly = c
        .buy_monthly_card_for_character("demo", 0, "compat-monthly", AT + 11)
        .unwrap();
    c.activate_monthly_card_for_character("demo", 0, monthly.unique_id, AT + 12)
        .unwrap();
    let mut a = c.account_store.lock().unwrap().accounts["demo"].clone();
    // A genuine pre-bridge shape has receipts and balances without provenance.
    a.billing.as_mut().unwrap().provenance = Default::default();
    let mut p = mir2_simulation::procurement::ProcurementLedger::default();
    p.credits.insert(
        "legacy-quota".into(),
        mir2_simulation::procurement::CreditBatch {
            id: "legacy-quota".into(),
            source: mir2_simulation::procurement::CreditSource::Test,
            granted: 10,
            reserved: 2,
            consumed: 3,
            expires_at_ms: AT + 99_999,
            payment_order_id: None,
        },
    );
    p.cursors.insert("testnet".into(), "checkpoint-123".into());
    p.requests
        .insert("legacy-request".into(), "legacy-quota".into());
    p.validate().unwrap();
    a.procurement = Some(p);
    a.activities = Some(Default::default());
    a.password = "$argon2id$mir2-wallet-locked:v1!".into();
    a.storage_size = 120;
    a.has_expanded_storage = true;
    a.expanded_storage_expiry_time_binary_datetime = 123456;
    a.storage_password = "storage-fixture".into();
    a.storage_password_last_set_binary_datetime = 123455;
    a.ban_reason = "retained-old-reason".into();
    a.ban_until_ms = Some(AT - 1);
    a.banned_at_ms = Some(AT - 2);
    a.gm_level = 1;
    a.character_last_access_binary_datetimes.insert(0, 123454);
    a
}
fn assert_legacy_and_current_roundtrip(raw: &Value) {
    let old: OriginalAccountReader = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(old).unwrap(), *raw);
    let current: AccountRecord = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(&current).unwrap(), *raw);
    assert!(current.billing.as_ref().unwrap().provenance.is_empty());
}
#[test]
fn original_account_full_field_roundtrip_preserves_old_orders_and_refuses_unknown_money() {
    let a = historical_account();
    let mut raw = serde_json::to_value(&a).unwrap();
    assert_legacy_and_current_roundtrip(&raw);
    assert_eq!(raw["billing"]["orders"].as_object().unwrap().len(), 1);
    assert_eq!(
        raw["billing"]["suiUsdcOrders"].as_object().unwrap().len(),
        1
    );
    assert!(raw["billing"].get("provenance").is_none());
    let c = SimulationConfig::default();
    c.account_store
        .lock()
        .unwrap()
        .accounts
        .insert("demo".into(), a);
    let order_id = c.account_store.lock().unwrap().accounts["demo"]
        .billing
        .as_ref()
        .unwrap()
        .orders
        .keys()
        .next()
        .unwrap()
        .clone();
    c.confirm_recharge_payment(
        "demo",
        &order_id,
        &VerifiedRechargePayment {
            session_id: "cs_compat".into(),
            payment_intent_id: "pi_compat".into(),
            currency: "usd".into(),
            amount_minor: 1000,
            livemode: false,
        },
        AT + 20,
    )
    .unwrap();
    let observed = c.account_store.lock().unwrap().accounts["demo"].clone();
    let provenance = &observed.billing.as_ref().unwrap().provenance;
    assert!(!provenance.is_empty());
    assert!(provenance.payments.is_empty());
    assert!(provenance
        .wallets
        .values()
        .flat_map(|w| &w.lots)
        .all(|l| l.kind == mir2_simulation::billing_provenance::CreditFundingKind::Unknown));
    let mut upgraded = serde_json::to_value(&observed).unwrap();
    assert!(
        serde_json::from_value::<OriginalAccountReader>(upgraded.clone()).is_err(),
        "original typed readers must upgrade before provenance writes"
    );
    upgraded["billing"]
        .as_object_mut()
        .unwrap()
        .remove("provenance");
    raw.as_object_mut().unwrap().remove("activities");
    upgraded.as_object_mut().unwrap().remove("activities");
    assert_eq!(
        upgraded, raw,
        "all financial, metadata and save fields survive a new writer"
    );
    let mut future = serde_json::to_value(observed).unwrap();
    future["billing"]["futureMoneyAuthority"] = json!({"mustNotDrop":1});
    assert!(serde_json::from_value::<AccountRecord>(future).is_err());
}
#[test]
fn original_v13_usdc_reader_rejects_new_transfer_authority_instead_of_dropping_it() {
    let original = historical_account();
    let raw = serde_json::to_value(&original).unwrap();
    let (_, old_order) = raw["billing"]["suiUsdcOrders"]
        .as_object()
        .unwrap()
        .iter()
        .next()
        .unwrap();
    let legacy: OriginalV13UsdcOrderReader = serde_json::from_value(old_order.clone()).unwrap();
    assert_eq!(serde_json::to_value(legacy).unwrap(), *old_order);
    for (key, value) in [
        ("paymentMode", json!("direct_transfer")),
        ("transferReceipt", json!({"future":true})),
        ("directScanCursor", json!("{}")),
        ("directScanComplete", json!(true)),
    ] {
        let mut future = old_order.clone();
        future[key] = value;
        assert!(
            serde_json::from_value::<OriginalV13UsdcOrderReader>(future).is_err(),
            "{key}"
        );
    }
}
#[test]
fn migration_union_keeps_original_nineteen_and_never_repeats_usdc_backfill() {
    assert_eq!(MIGRATIONS.len(), 28);
    assert!(MIGRATIONS[..19]
        .iter()
        .all(|(id, _)| !id.contains("promotion")));
    assert_eq!(MIGRATIONS[18].0, "0019_sui_usdc_billing_authority");
    assert!(MIGRATIONS[19..26]
        .iter()
        .all(|(id, _)| id.contains("promotion")));
    assert_eq!(MIGRATIONS[26].0, "0024_sui_usdc_native_transfer_authority");
    assert_eq!(MIGRATIONS[27].0, "0025_promotion_attribution_migration");
    assert!(MIGRATIONS
        .iter()
        .all(|(id, _)| *id != "0024_sui_usdc_billing_authority"));
}
#[test]
fn native_postgres_original_migrations_upgrade_and_typed_roundtrip_preserve_every_old_field() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIPPED: isolated native PG opt-in required");
        return;
    }
    let url = std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL").expect("isolated DB URL required");
    let parsed: postgres::Config = url.parse().unwrap();
    let name = parsed.get_dbname().unwrap_or("");
    assert!(name.starts_with("mir2_promotion_") || name.starts_with("promotion_"));
    assert!(parsed.get_options().is_none());
    assert!(
        !parsed.get_hosts().is_empty()
            && parsed.get_hosts().iter().all(
                |h| matches!(h,postgres::config::Host::Tcp(n) if n=="127.0.0.1"||n=="localhost")
            )
    );
    let schema = format!(
        "promotion_compat_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let mut base = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let mut scoped = parsed.clone();
    scoped.options(&format!(
        "-c search_path={schema} -c lock_timeout=5000 -c statement_timeout=15000"
    ));
    let mut db = scoped.connect(postgres::NoTls).unwrap();
    db.batch_execute("CREATE TABLE schema_migrations(version TEXT PRIMARY KEY,applied_at TIMESTAMPTZ NOT NULL DEFAULT now())").unwrap();
    for (id, sql) in &MIGRATIONS[..19] {
        db.batch_execute(sql).unwrap();
        db.execute("INSERT INTO schema_migrations(version)VALUES($1)", &[id])
            .unwrap();
    }
    let raw = serde_json::to_value(historical_account()).unwrap();
    db.execute(
        "INSERT INTO accounts(account_id,raw_json)VALUES('demo',$1)",
        &[&raw],
    )
    .unwrap();
    let fingerprint: String = db
        .query_one(
            "SELECT md5(raw_json::text) FROM accounts WHERE account_id='demo'",
            &[],
        )
        .unwrap()
        .get(0);
    for _ in 0..2 {
        mir2_simulation::apply_migrations(&mut db).unwrap();
    }
    assert_eq!(
        db.query_one("SELECT count(*) FROM schema_migrations", &[])
            .unwrap()
            .get::<_, i64>(0),
        MIGRATIONS.len() as i64
    );
    let preserved: Value = db
        .query_one("SELECT raw_json FROM accounts WHERE account_id='demo'", &[])
        .unwrap()
        .get(0);
    assert_eq!(preserved, raw);
    assert_legacy_and_current_roundtrip(&preserved);
    assert_eq!(
        db.query_one(
            "SELECT md5(raw_json::text) FROM accounts WHERE account_id='demo'",
            &[]
        )
        .unwrap()
        .get::<_, String>(0),
        fingerprint
    );
    let typed: AccountRecord = serde_json::from_value(preserved).unwrap();
    let roundtrip = serde_json::to_value(typed).unwrap();
    db.execute(
        "UPDATE accounts SET raw_json=$1 WHERE account_id='demo'",
        &[&roundtrip],
    )
    .unwrap();
    assert_eq!(
        db.query_one("SELECT raw_json FROM accounts WHERE account_id='demo'", &[])
            .unwrap()
            .get::<_, Value>(0),
        raw
    );
    let mut no_procurement = raw.clone();
    no_procurement
        .as_object_mut()
        .unwrap()
        .remove("procurement");
    assert!(db
        .execute(
            "UPDATE accounts SET raw_json=$1 WHERE account_id='demo'",
            &[&no_procurement]
        )
        .is_err());
    let mut no_sui = raw.clone();
    no_sui["billing"]
        .as_object_mut()
        .unwrap()
        .remove("suiUsdcOrders");
    assert!(db
        .execute(
            "UPDATE accounts SET raw_json=$1 WHERE account_id='demo'",
            &[&no_sui]
        )
        .is_err());
    drop(db);
    assert!(schema.starts_with("promotion_compat_"));
    base.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}

#[test]
fn windows_playtest_draft_keeps_seven_day_test_only_revenue_share_and_payout_off() {
    let v: Value = serde_json::from_str(include_str!(
        "../../../config/promotion-windows-playtest-20261030.test.json"
    ))
    .unwrap();
    let d: mir2_simulation::promotion::CampaignDraft =
        serde_json::from_value(v["draft"].clone()).unwrap();
    d.rules.validate_environment("test").unwrap();
    assert_eq!(d.campaign_id, "numeron-windows-playtest-20261030");
    assert_eq!(d.ends_at_ms - d.starts_at_ms, 7 * 86_400_000);
    assert_eq!(d.starts_at_ms, 1_793_219_738_449);
    assert_eq!(
        (
            d.rules.minimum_level,
            d.rules.active_utc_days,
            d.rules.reserve_bps
        ),
        (1, 1, 0)
    );
    assert!(d.rules.test_wallet_enrollment);
    assert_eq!(d.funding_mode, "revenue_share");
    assert_eq!(d.budget_micro_usd, 0);
    assert_eq!(
        (
            d.member_limit,
            d.leader_limit,
            d.per_leader_member_limit,
            d.per_member_revenue_cap_micro_usd
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(v["gameClient"], "windows");
    assert_eq!(v["environment"], "test");
    assert_eq!(v["paymentNetwork"], "sui:testnet");
    assert_eq!(v["paymentCurrency"], "USDC");
    assert_eq!(v["commissionTiersBps"], json!([1000, 2000, 3000]));
    assert_eq!(v["intendedSwitches"]["payoutEnabled"], false);
}

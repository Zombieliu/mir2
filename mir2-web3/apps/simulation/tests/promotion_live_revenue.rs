//! Native PostgreSQL + real game billing/monthly APIs. Payment provider inputs
//! here are explicitly trusted fixtures, not an external Stripe acceptance.
use mir2_simulation::billing::{RechargeOffer, RechargeOrder, VerifiedRechargePayment};
use mir2_simulation::billing_provenance::{VerifiedPaymentAdverseEvent, VerifiedPaymentRevenue};
use mir2_simulation::promotion::{PromotionRules, PromotionStore};
use mir2_simulation::{apply_migrations, AccountRecord, SimulationConfig};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};
use std::sync::{Arc, Barrier};

const AT: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;

fn open(url: &str) -> SimulationConfig {
    SimulationConfig::default()
        .with_postgres_account_store(url.to_owned())
        .unwrap()
        .with_billing_monthly_card_credit_price(Some(60))
        .unwrap()
}
fn fund(
    c: &SimulationConfig,
    account: &str,
    key: &str,
    credits: u32,
    amount: u64,
    at: u64,
) -> RechargeOrder {
    let offer = RechargeOffer {
        id: format!("offer-{key}"),
        label: "Native PG payment fixture".into(),
        currency: "usd".into(),
        amount_minor: amount,
        credits,
    };
    let o = c
        .prepare_recharge_order(account, 0, key, &offer, at, false)
        .unwrap();
    let p = VerifiedRechargePayment {
        session_id: format!("cs_{key}"),
        payment_intent_id: format!("pi_{key}"),
        amount_minor: amount,
        currency: "usd".into(),
        livemode: false,
    };
    let o = c
        .confirm_recharge_payment(account, &o.id, &p, at + 1)
        .unwrap();
    c.record_verified_payment_revenue(
        account,
        &VerifiedPaymentRevenue {
            provider: "stripe".into(),
            source_event_id: format!("evt_{key}"),
            order_id: o.id.clone(),
            checkout_session_id: p.session_id,
            payment_intent_id: p.payment_intent_id,
            livemode: false,
            currency: "usd".into(),
            gross_amount_minor: amount,
            indirect_tax_minor: Some(0),
            discount_minor: Some(0),
            paid_at_ms: at + 1,
            observed_at_ms: at + 2,
            evidence_ref: format!("explicit-provider-fixture:{key}"),
        },
    )
    .unwrap();
    c.consume_inactive_pending_recharge_credits(account, 0, at + 3)
        .unwrap();
    o
}
fn card(c: &SimulationConfig, account: &str, key: &str, at: u64) -> u64 {
    let item = c
        .buy_monthly_card_for_character(account, 0, key, at)
        .unwrap();
    c.activate_monthly_card_for_character(account, 0, item.unique_id, at + 1)
        .unwrap();
    item.unique_id
}
fn adverse(
    c: &SimulationConfig,
    account: &str,
    o: &RechargeOrder,
    id: &str,
    refund: u64,
    status: &str,
    at: u64,
) {
    c.record_verified_payment_adverse_event(
        account,
        &VerifiedPaymentAdverseEvent {
            provider: "stripe".into(),
            source_event_id: id.into(),
            order_id: o.id.clone(),
            payment_intent_id: o.payment_intent_id.clone().unwrap(),
            livemode: false,
            currency: "usd".into(),
            gross_amount_minor: o.offer.amount_minor,
            cumulative_refunded_minor: refund,
            disputed_minor: if status == "none" {
                0
            } else {
                o.offer.amount_minor
            },
            dispute_status: status.into(),
            occurred_at_ms: at,
            observed_at_ms: at,
            evidence_ref: format!("explicit-adverse-fixture:{id}"),
        },
    )
    .unwrap();
}
fn seed_control(db: &mut Client, env: &str) {
    let rules = serde_json::to_value(PromotionRules::default()).unwrap();
    db.execute("INSERT INTO promotion_campaigns(environment,campaign_id,terms_version,status,starts_at_ms,ends_at_ms,rules_json,rules_approved,commission_enabled,budget_micro_usd,reserved_micro_usd,member_limit,leader_limit,per_leader_member_limit,per_member_revenue_cap_micro_usd,configured_by,approved_by,created_at_ms) VALUES($1,'season','fixture-terms','active',$2,$3,$4,TRUE,$5,100000000,9000000,10,2,10,30000000,'fixture-operator','fixture-reviewer',$2)",&[&env,&(AT as i64),&((AT+90*DAY) as i64),&rules,&(env!="production")]).unwrap();
    db.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,status,accepted_terms_version,applied_at_ms) VALUES($1,'season','leader','leader-1',$2,0,'Leader','approved','fixture-terms',$3)",&[&env,&format!("invite-{env}"),&(AT as i64)]).unwrap();
    for member in ["member", "late", "unknown"] {
        let candidate = format!("candidate-{env}-{member}");
        db.execute("INSERT INTO promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id,first_clicked_at_ms,expires_at_ms,source) VALUES($1,'season',$2,$3,'leader-1',$4,$5,'manual_code')",&[&env,&candidate,&member,&((AT+10) as i64),&((AT+DAY) as i64)]).unwrap();
        db.execute("INSERT INTO promotion_attributions(environment,campaign_id,account_id,leader_id,candidate_id,first_clicked_at_ms,registered_at_ms,confirmed_at_ms,terms_version,source,reserved_micro_usd) VALUES($1,'season',$2,'leader-1',$3,$4,$5,$6,'fixture-terms','manual_code',3000000)",&[&env,&member,&candidate,&((AT+10) as i64),&((AT+20) as i64),&((AT+50) as i64)]).unwrap();
    }
}

#[test]
fn native_credit_fifo_import_refund_hold_and_environment_are_durable() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIPPED native PG: explicit isolated opt-in required");
        return;
    }
    let url =
        std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL").expect("private isolated URI required");
    let parsed: Config = url.parse().expect("private URI invalid");
    assert!(parsed.get_dbname().unwrap_or("").contains("promotion"));
    assert!(parsed.get_options().is_none());
    assert!(parsed
        .get_hosts()
        .iter()
        .all(|h| matches!(h,postgres::config::Host::Tcp(n) if n=="127.0.0.1"||n=="localhost")));
    let mut base = Client::connect(&url, NoTls).expect("isolated PG connection");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("promotion_fifo_{}_{nonce}", std::process::id());
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let scoped=format!("{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",if url.contains('?') {"&"} else {"?"});
    let mut db = Client::connect(&scoped, NoTls).unwrap();
    apply_migrations(&mut db).unwrap();
    apply_migrations(&mut db).unwrap();
    let c = open(&scoped);
    for name in ["leader", "member", "late", "unknown"] {
        let mut character = c.default_character.clone();
        character.name = match name {
            "leader" => "Leader",
            "member" => "Member",
            "late" => "Late",
            _ => "Unknown",
        }
        .into();
        let mut account = AccountRecord::new(character);
        if name == "unknown" {
            account.saves.get_mut(&0).unwrap().credit = 60;
        }
        c.account_store
            .lock()
            .unwrap()
            .accounts
            .insert(name.into(), account);
        c.save_account_store_account(name).unwrap();
    }
    seed_control(&mut db, "test");
    seed_control(&mut db, "production");
    db.execute(
        "UPDATE promotion_campaigns SET input_watermark_ms=$1 WHERE environment='test'",
        &[&((AT + 90 * DAY) as i64)],
    )
    .unwrap();
    db.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test','leader','finance','isolated-fixture',1)",&[]).unwrap();
    let first = fund(&c, "member", "first-native", 30, 300, AT + 100);
    fund(&c, "member", "second-native", 70, 1400, AT + 110);
    let uid = card(&c, "member", "monthly-two-native-lots", AT + 200);
    // Actual payment predates confirmation. The callback input arrives later.
    fund(&c, "late", "early-payment", 60, 600, AT + 30);
    card(&c, "late", "monthly-early-native", AT + 200);
    card(&c, "unknown", "monthly-unknown-native", AT + 200);
    let store = PromotionStore::new(scoped.clone(), "test").unwrap();
    let result = store.reconcile_live_monthly_revenue(AT + 1000).unwrap();
    assert_eq!(result["allocationsImported"], 1);
    assert_eq!(result["activeHolds"], 1);
    let subscription = format!("monthly-item:{uid}");
    let r=db.query_one("SELECT allocation_id,eligible_micro_usd FROM promotion_revenue_allocations WHERE environment='test' AND subscription_id=$1",&[&subscription]).unwrap();
    let allocation: String = r.get(0);
    assert_eq!(r.get::<_, i64>(1), 9_000_000);
    assert_eq!(db.query_one("SELECT count(*) FROM promotion_credit_lot_allocations WHERE environment='test' AND allocation_id=$1",&[&allocation]).unwrap().get::<_,i64>(0),2);
    assert_eq!(db.query_one("SELECT status FROM promotion_attributions WHERE environment='test' AND account_id='late'",&[]).unwrap().get::<_,String>(0),"review");
    let source:Value=db.query_one("SELECT source_json FROM promotion_monthly_source_observations WHERE environment='test' AND account_id='unknown' ORDER BY observed_at_ms DESC LIMIT 1",&[]).unwrap().get(0);
    assert_eq!(source["unknownCredits"], 60);
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let gate = Arc::clone(&barrier);
            let url = scoped.clone();
            std::thread::spawn(move || {
                gate.wait();
                PromotionStore::new(url, "test")
                    .unwrap()
                    .reconcile_live_monthly_revenue(AT + 1001)
                    .unwrap()
            })
        })
        .collect();
    for worker in workers {
        assert!(matches!(
            worker.join().unwrap()["status"].as_str(),
            Some("busy" | "reconciled")
        ));
    }
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM promotion_revenue_allocations WHERE environment='test'",
            &[]
        )
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    // A freshly loaded Source sees the exact immutable paid lots and UID.
    let restarted = open(&scoped);
    let member: AccountRecord = serde_json::from_value(
        db.query_one(
            "SELECT raw_json FROM accounts WHERE account_id='member'",
            &[],
        )
        .unwrap()
        .get(0),
    )
    .unwrap();
    assert_eq!(
        member.billing.unwrap().provenance.monthly_items[&uid]
            .parts
            .len(),
        2
    );
    adverse(
        &restarted,
        "member",
        &first,
        "evt_native_refund_b",
        150,
        "none",
        AT + 1100,
    );
    store.reconcile_live_monthly_revenue(AT + 1200).unwrap();
    store.reconcile_live_monthly_revenue(AT + 1201).unwrap();
    assert_eq!(db.query_one("SELECT sum(reversal_micro_usd)::bigint FROM promotion_revenue_adjustments WHERE environment='test' AND allocation_id=$1",&[&allocation]).unwrap().get::<_,i64>(0),1_500_000);
    adverse(
        &restarted,
        "member",
        &first,
        "evt_native_refund_a",
        50,
        "none",
        AT + 1300,
    );
    store.reconcile_live_monthly_revenue(AT + 1400).unwrap();
    assert_eq!(db.query_one("SELECT sum(reversal_micro_usd)::bigint FROM promotion_revenue_adjustments WHERE environment='test' AND allocation_id=$1",&[&allocation]).unwrap().get::<_,i64>(0),1_500_000);
    adverse(
        &restarted,
        "member",
        &first,
        "evt_native_dispute",
        150,
        "needs_response",
        AT + 1500,
    );
    store.reconcile_live_monthly_revenue(AT + 1600).unwrap();
    assert!(db.query_one("SELECT active FROM promotion_revenue_source_holds WHERE environment='test' AND allocation_id=$1",&[&allocation]).unwrap().get::<_,bool>(0));
    assert_eq!(
        store
            .admin(
                "season",
                "leader",
                json!({"action":"createSettlement","leaderId":"leader-1","evidenceRef":"fixture"}),
                "hold-settlement",
                AT + 110 * DAY
            )
            .unwrap_err(),
        "promotionRevenueReconciliationRequired"
    );
    adverse(
        &restarted,
        "member",
        &first,
        "evt_native_won",
        150,
        "won",
        AT + 1700,
    );
    store.reconcile_live_monthly_revenue(AT + 1800).unwrap();
    assert!(!db.query_one("SELECT active FROM promotion_revenue_source_holds WHERE environment='test' AND allocation_id=$1",&[&allocation]).unwrap().get::<_,bool>(0));
    let production = PromotionStore::new(scoped.clone(), "production").unwrap();
    assert_eq!(
        production
            .reconcile_live_monthly_revenue(AT + 1900)
            .unwrap()["allocationsImported"],
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
    assert!(db
        .execute(
            "UPDATE promotion_monthly_source_observations SET reason_code='forged'",
            &[]
        )
        .is_err());
    println!("native PG FIFO: exact paid lots, actual redeemed UID, 9 USD basis, partial refund 1.50 USD, replay/concurrency, pending/hold, historical unknown, late attribution and environment isolation passed");
}

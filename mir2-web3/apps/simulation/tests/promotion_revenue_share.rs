//! Revenue-sharing mode uses earned income, without reserving prepaid cash.
//! Native PostgreSQL fixtures never enable a real production campaign.
use mir2_simulation::billing::{RechargeOffer, RechargeOrder, VerifiedRechargePayment};
use mir2_simulation::billing_provenance::{VerifiedPaymentAdverseEvent, VerifiedPaymentRevenue};
use mir2_simulation::promotion::{
    calculate_commission, CampaignDraft, CommissionInput, MemberInput, PromotionRules,
    PromotionStore, RevenueInput, TrustedGameFact,
};
use mir2_simulation::{apply_migrations, AccountRecord, SimulationConfig};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};

const AT: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;
const TERMS: &str = "approved-revenue-share-fixture";

#[test]
fn fifty_members_ten_usd_at_first_tier_and_no_reserve_pay_fifty_usd() {
    let members = (0..50)
        .map(|n| MemberInput {
            account_id: format!("member-{n:02}"),
            level_completed_at_ms: Some(AT + 10),
            play_completed_at_ms: Some(AT + 20),
            established_qualification_at_ms: None,
            excluded: false,
            revenue: vec![RevenueInput {
                allocation_id: format!("monthly-{n}"),
                paid_at_ms: AT + 30,
                entitlement_at_ms: AT + 40,
                service_start_ms: AT + 40,
                service_end_ms: AT + 30 * DAY + 40,
                amount_micro_usd: 10_000_000,
                reversed_micro_usd: 0,
                is_card_payment: true,
            }],
        })
        .collect();
    let result = calculate_commission(&CommissionInput {
        environment: "test".into(),
        campaign_id: "revenue-share".into(),
        terms_version: TERMS.into(),
        starts_at_ms: AT,
        ends_at_ms: AT + 90 * DAY,
        maximum_coverage_ms: 90 * DAY,
        per_member_revenue_cap_micro_usd: u64::MAX,
        reserve_bps: 0,
        members,
    })
    .unwrap();
    assert_eq!(result.members.len(), 50);
    assert!(result.members.iter().all(|m| m.rate_bps == 1000));
    assert_eq!(result.eligible_revenue_micro_usd, 500_000_000);
    assert_eq!(result.commission_micro_usd, 50_000_000);
    assert_eq!(result.reserve_micro_usd, 0);
    assert_eq!(result.payable_cents, 5000);
    // This is 50 USD payable. An approved 1:1 USDC quote yields 50 USDC;
    // the calculator itself never submits a transaction or assumes an FX rate.
}

fn command(store: &PromotionStore, actor: &str, value: Value, key: &str, now: u64) -> Value {
    store
        .admin("share-season", actor, value, key, now)
        .unwrap_or_else(|e| panic!("isolated revenue-share {key}: {e}"))
}
fn fund(config: &SimulationConfig, account: &str, at: u64) -> RechargeOrder {
    let offer = RechargeOffer {
        id: "real-cost-fixture".into(),
        label: "Revenue sharing native PG fixture".into(),
        currency: "usd".into(),
        amount_minor: 1000,
        credits: 1000,
    };
    let o = config
        .prepare_recharge_order(
            account,
            0,
            &format!("recharge-{account}"),
            &offer,
            at,
            false,
        )
        .unwrap();
    let p = VerifiedRechargePayment {
        session_id: format!("cs_{account}"),
        payment_intent_id: format!("pi_{account}"),
        amount_minor: 1000,
        currency: "usd".into(),
        livemode: false,
    };
    let o = config
        .confirm_recharge_payment(account, &o.id, &p, at + 1)
        .unwrap();
    config
        .record_verified_payment_revenue(
            account,
            &VerifiedPaymentRevenue {
                provider: "stripe".into(),
                source_event_id: format!("evt_paid_{account}"),
                order_id: o.id.clone(),
                checkout_session_id: p.session_id,
                payment_intent_id: p.payment_intent_id,
                livemode: false,
                currency: "usd".into(),
                gross_amount_minor: 1000,
                indirect_tax_minor: Some(0),
                discount_minor: Some(0),
                paid_at_ms: at + 1,
                observed_at_ms: at + 2,
                evidence_ref: format!("explicit-provider-fixture:{account}"),
            },
        )
        .unwrap();
    config
        .consume_inactive_pending_recharge_credits(account, 0, at + 3)
        .unwrap();
    let unit = config
        .buy_monthly_card_for_character(account, 0, &format!("monthly-{account}"), at + 4)
        .unwrap();
    config
        .activate_monthly_card_for_character(account, 0, unit.unique_id, at + 5)
        .unwrap();
    o
}
fn adverse(
    config: &SimulationConfig,
    order: &RechargeOrder,
    refund: u64,
    status: &str,
    id: &str,
    at: u64,
) {
    config
        .record_verified_payment_adverse_event(
            "memberone",
            &VerifiedPaymentAdverseEvent {
                provider: "stripe".into(),
                source_event_id: id.into(),
                order_id: order.id.clone(),
                payment_intent_id: order.payment_intent_id.clone().unwrap(),
                livemode: false,
                currency: "usd".into(),
                gross_amount_minor: 1000,
                cumulative_refunded_minor: refund,
                disputed_minor: if status == "none" { 0 } else { 1000 },
                dispute_status: status.into(),
                occurred_at_ms: at,
                observed_at_ms: at,
                evidence_ref: format!("explicit-adverse-fixture:{id}"),
            },
        )
        .unwrap();
}

#[test]
fn native_revenue_share_has_no_budget_reservations_and_holds_keep_qualification_time() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIPPED native PG: explicit isolated opt-in required");
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
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("promotion_share_{}_{nonce}", std::process::id());
    let mut base = Client::connect(&url, NoTls).unwrap();
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let scoped=format!("{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",if url.contains('?') {"&"} else {"?"});
    let mut db = Client::connect(&scoped, NoTls).unwrap();
    apply_migrations(&mut db).unwrap();
    apply_migrations(&mut db).unwrap();
    let config = SimulationConfig::default()
        .with_postgres_account_store(scoped.clone())
        .unwrap()
        .with_billing_monthly_card_credit_price(Some(1000))
        .unwrap();
    for (id, name) in [
        ("operator", "Operator"),
        ("finance", "Finance"),
        ("risk", "Risk"),
        ("leader", "Leader"),
        ("memberone", "Memberone"),
        ("membertwo", "Membertwo"),
    ] {
        let mut character = config.default_character.clone();
        character.name = name.into();
        character.level = 30;
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .insert(id.into(), AccountRecord::new(character));
        config.save_account_store_account(id).unwrap();
        db.execute("UPDATE accounts SET created_at=to_timestamp($2::bigint::double precision/1000.0) WHERE account_id=$1",&[&id,&((AT+1000) as i64)]).unwrap();
    }
    for (actor, role) in [
        ("operator", "operations"),
        ("finance", "finance"),
        ("risk", "risk"),
    ] {
        db.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,$2,'isolated-fixture',1)",&[&actor,&role]).unwrap();
    }
    let store = PromotionStore::new(scoped.clone(), "test").unwrap();
    let mut rules = PromotionRules::default();
    rules.reserve_bps = 0;
    let draft = CampaignDraft {
        campaign_id: "share-season".into(),
        terms_version: TERMS.into(),
        funding_mode: "revenue_share".into(),
        starts_at_ms: AT,
        ends_at_ms: AT + 90 * DAY,
        rules,
        budget_micro_usd: 0,
        member_limit: 0,
        leader_limit: 0,
        per_leader_member_limit: 0,
        per_member_revenue_cap_micro_usd: 0,
    };
    command(
        &store,
        "operator",
        json!({"action":"configureDraft","draft":draft,"evidenceRef":"user-authorized-revenue-share-fixture"}),
        "draft-share",
        AT + 1,
    );
    command(
        &store,
        "finance",
        json!({"action":"approveRules","evidenceRef":"independent-test-review"}),
        "approve-share",
        AT + 2,
    );
    let leader = store
        .apply(
            "share-season",
            "leader",
            0,
            TERMS,
            "leader-share-apply",
            AT + 1200,
        )
        .unwrap()["leaderId"]
        .as_str()
        .unwrap()
        .to_owned();
    command(
        &store,
        "risk",
        json!({"action":"reviewLeader","leaderId":leader,"decision":"approved","reason":"isolated-fixture","evidenceRef":"fixture"}),
        "leader-share-review",
        AT + 1300,
    );
    command(
        &store,
        "operator",
        json!({"action":"setRecruitment","enabled":true,"evidenceRef":"fixture"}),
        "open-share",
        AT + 1400,
    );
    command(
        &store,
        "finance",
        json!({"action":"setCommission","enabled":true,"evidenceRef":"fixture"}),
        "commission-share",
        AT + 1401,
    );
    let code = store.me("share-season", "leader", AT + 1500).unwrap()["leader"]["inviteCode"]
        .as_str()
        .unwrap()
        .to_owned();
    for member in ["memberone", "membertwo"] {
        let candidate = store
            .create_candidate(
                "share-season",
                &code,
                member,
                "manual_code",
                &format!("candidate-{member}"),
                AT + 2000,
            )
            .unwrap()["candidateId"]
            .as_str()
            .unwrap()
            .to_owned();
        let result = store
            .confirm(
                "share-season",
                member,
                &candidate,
                TERMS,
                &format!("confirm-{member}"),
                AT + 2500,
            )
            .unwrap();
        assert_eq!(result["reservedMicroUsd"], "0");
    }
    let first = fund(&config, "memberone", AT + 5000);
    fund(&config, "membertwo", AT + 6000);
    for member in ["memberone", "membertwo"] {
        store
            .record_game_fact(
                &TrustedGameFact {
                    source: "level_commit".into(),
                    kind: "levelCommitted".into(),
                    source_event_id: format!("level-{member}"),
                    account_id: member.into(),
                    character_index: 0,
                    occurred_at_ms: AT + 7000,
                    level: Some(30),
                    evidence_ref: "explicit-committed-level-fixture".into(),
                },
                AT + 3 * DAY,
            )
            .unwrap();
        for day in 0..3 {
            store
                .record_game_fact(
                    &TrustedGameFact {
                        source: "combat_commit".into(),
                        kind: "combatKill".into(),
                        source_event_id: format!("combat-{member}-{day}"),
                        account_id: member.into(),
                        character_index: 0,
                        occurred_at_ms: AT + 8000 + day * DAY,
                        level: None,
                        evidence_ref: "explicit-committed-combat-fixture".into(),
                    },
                    AT + 3 * DAY,
                )
                .unwrap();
        }
    }
    store.reconcile_live_monthly_revenue(AT + 3 * DAY).unwrap();
    let work = store.process_pending_work(100, AT + 3 * DAY).unwrap();
    assert_eq!(work["failed"], 0);
    let row=db.query_one("SELECT budget_micro_usd,reserved_micro_usd,funding_mode FROM promotion_campaigns WHERE environment='test' AND campaign_id='share-season'",&[]).unwrap();
    assert_eq!(row.get::<_, i64>(0), 0);
    assert_eq!(row.get::<_, i64>(1), 0);
    assert_eq!(row.get::<_, String>(2), "revenue_share");
    assert_eq!(db.query_one("SELECT count(*),sum(reserved_micro_usd)::bigint FROM promotion_attributions WHERE environment='test' AND campaign_id='share-season'",&[]).unwrap().get::<_,i64>(0),2);
    let initial = store.me("share-season", "leader", AT + 3 * DAY).unwrap();
    assert_eq!(initial["dashboard"]["qualified"], 2);
    assert_eq!(
        initial["dashboard"]["estimatedCommissionMicroUsd"],
        "2000000"
    );
    let qualified:i64=db.query_one("SELECT qualified_at_ms FROM promotion_attributions WHERE environment='test' AND campaign_id='share-season' AND account_id='memberone'",&[]).unwrap().get(0);
    adverse(
        &config,
        &first,
        0,
        "needs_response",
        "evt_share_dispute",
        AT + 4 * DAY,
    );
    store.reconcile_live_monthly_revenue(AT + 4 * DAY).unwrap();
    assert_eq!(
        store.process_pending_work(100, AT + 4 * DAY).unwrap()["failed"],
        0
    );
    let during=db.query_one("SELECT status,qualified_at_ms FROM promotion_attributions WHERE environment='test' AND campaign_id='share-season' AND account_id='memberone'",&[]).unwrap();
    assert_eq!(during.get::<_, String>(0), "qualified");
    assert_eq!(during.get::<_, i64>(1), qualified);
    adverse(&config, &first, 0, "won", "evt_share_won", AT + 5 * DAY);
    store.reconcile_live_monthly_revenue(AT + 5 * DAY).unwrap();
    store.process_pending_work(100, AT + 5 * DAY).unwrap();
    assert_eq!(db.query_one("SELECT qualified_at_ms FROM promotion_attributions WHERE environment='test' AND campaign_id='share-season' AND account_id='memberone'",&[]).unwrap().get::<_,i64>(0),qualified);
    adverse(
        &config,
        &first,
        1000,
        "none",
        "evt_share_full_refund",
        AT + 6 * DAY,
    );
    store.reconcile_live_monthly_revenue(AT + 6 * DAY).unwrap();
    assert_eq!(
        store.process_pending_work(100, AT + 6 * DAY).unwrap()["failed"],
        0
    );
    let refunded=db.query_one("SELECT status,qualified_at_ms FROM promotion_attributions WHERE environment='test' AND campaign_id='share-season' AND account_id='memberone'",&[]).unwrap();
    assert_eq!(refunded.get::<_, String>(0), "bound");
    assert!(refunded.get::<_, Option<i64>>(1).is_none());
    assert_eq!(
        store.me("share-season", "leader", AT + 6 * DAY).unwrap()["dashboard"]
            ["estimatedCommissionMicroUsd"],
        "1000000"
    );
    println!("native PG revenue share: zero budget/caps/reservations, two actual paid monthly units, 2 USD commission, recoverable hold rank, full refund and zero budget worker passed");
}

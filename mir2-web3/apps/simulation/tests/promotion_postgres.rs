//! Explicit opt-in, native PostgreSQL integration. Fixtures are marked test data;
//! this never connects to Stripe, a game test server, a wallet signer or Docker.
use std::collections::BTreeMap;
use std::sync::{Arc, Barrier};

use mir2_protocol::{MirClass, MirGender};
use mir2_simulation::billing::{
    BillingLedger, RechargeAppliedReceipt, RechargeOffer, RechargeOrder, RechargeOrderState,
};
use mir2_simulation::monthly_card::{
    MonthlyCardItemDelivery, MonthlyCardItemReceipt, MonthlyCardLedger,
};
use mir2_simulation::promotion::{
    CampaignDraft, PromotionRules, PromotionStore, SandboxMonthlyRevenue, TrustedGameFact,
};
use mir2_simulation::sui_usdc_billing::{
    UsdcOrderState, UsdcPaymentMode, UsdcRechargeOrder, VerifiedNativeUsdcTransferReceipt,
    VerifiedUsdcReceipt, TESTNET_GENESIS_DIGEST, TESTNET_USDC_COIN_TYPE,
};
use mir2_simulation::{
    apply_migrations, with_account_store_postgres_client, AccountRecord, CharacterRecord,
};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const START: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;
const END: u64 = START + 92 * DAY;
const TERMS: &str = "promotion-fixture-v1-draft";

fn record(name: &str, level: u16) -> AccountRecord {
    let character = CharacterRecord {
        index: 1,
        name: name.into(),
        level,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    };
    serde_json::from_value(json!({"characters":[character],"saves":{}})).unwrap()
}
fn insert_account(client: &mut Client, id: &str, level: u16, registered: u64) {
    let raw = serde_json::to_value(record(id, level)).unwrap();
    client.execute("INSERT INTO accounts(account_id,raw_json,created_at) VALUES($1,$2,to_timestamp($3::bigint::double precision/1000.0))",&[&id,&raw,&(registered as i64)]).unwrap();
    client.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES($1,1,$1,'Warrior','Male',$2,$3)",&[&id,&i32::from(level),&raw["characters"][0]]).unwrap();
}
fn grant(client: &mut Client, account: &str, role: &str) {
    client.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,$2,'isolated_fixture',1)",&[&account,&role]).unwrap();
}
fn command(
    store: &PromotionStore,
    campaign: &str,
    actor: &str,
    action: Value,
    key: &str,
    now: u64,
) -> Value {
    store
        .admin(campaign, actor, action, key, now)
        .unwrap_or_else(|e| panic!("fixture command {key}: {e}"))
}
fn configure(store: &PromotionStore, id: &str, budget: u64, members: u64) -> String {
    let draft = CampaignDraft {
        funding_mode:"budget_capped".into(),
        campaign_id: id.into(),
        terms_version: TERMS.into(),
        starts_at_ms: START,
        ends_at_ms: END,
        rules: PromotionRules::default(),
        budget_micro_usd: budget,
        member_limit: members,
        leader_limit: 5,
        per_leader_member_limit: members,
        per_member_revenue_cap_micro_usd: 30_000_000,
    };
    command(
        store,
        id,
        "operator",
        json!({"action":"configureDraft","draft":draft,"evidenceRef":"fixture-only"}),
        &format!("{id}-draft"),
        START + 1,
    );
    assert_eq!(
        store.public_campaign(id).unwrap()["recruitmentEnabled"],
        false
    );
    assert!(store.public_campaign(id).unwrap()["rules"].is_null());
    assert_eq!(
        store
            .admin(
                id,
                "operator",
                json!({"action":"approveRules","evidenceRef":"self"}),
                "self-approve",
                START + 1
            )
            .unwrap_err(),
        "promotionPermissionDenied"
    );
    command(
        store,
        id,
        "finance1",
        json!({"action":"approveRules","evidenceRef":"fixture-review"}),
        &format!("{id}-approve"),
        START + 1,
    );
    let leader = store
        .apply(id, "leader", 1, TERMS, &format!("{id}-apply"), START + 2)
        .unwrap();
    let leader_id = leader["leaderId"].as_str().unwrap().to_owned();
    command(
        store,
        id,
        "risk",
        json!({"action":"reviewLeader","leaderId":leader_id,"decision":"approved","reason":"fixture only","evidenceRef":"fixture-review"}),
        &format!("{id}-leader"),
        START + 3,
    );
    command(
        store,
        id,
        "operator",
        json!({"action":"setRecruitment","enabled":true,"evidenceRef":"isolated-test-only"}),
        &format!("{id}-open"),
        START + 3,
    );
    command(
        store,
        id,
        "finance1",
        json!({"action":"setCommission","enabled":true,"evidenceRef":"isolated-test-only"}),
        &format!("{id}-commission"),
        START + 3,
    );
    leader_id
}
fn invite_code(store: &PromotionStore, campaign: &str) -> String {
    store.me(campaign, "leader", START + 10).unwrap()["leader"]["inviteCode"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn bind(store: &PromotionStore, campaign: &str, member: &str, code: &str) -> Value {
    let click = store
        .record_invitation_click(campaign, code, START + 900)
        .unwrap();
    let candidate = store
        .candidate_from_click(
            campaign,
            member,
            click["clickId"].as_str().unwrap(),
            &format!("{campaign}-candidate-{member}"),
            START + 1200,
        )
        .unwrap();
    store
        .confirm(
            campaign,
            member,
            candidate["candidateId"].as_str().unwrap(),
            TERMS,
            &format!("{campaign}-confirm-{member}"),
            START + 1500,
        )
        .unwrap()
}
fn seed_paid_monthly(client: &mut Client, account: &str) -> SandboxMonthlyRevenue {
    let mut state = record(account, 30);
    let monthly_uid = if account == "member" {
        123_456
    } else {
        234_567
    };
    let order_id = format!("rch_{}", "a".repeat(64));
    let order = RechargeOrder {
        id: order_id.clone(),
        request_id: "recharge-fixture".into(),
        character_index: 1,
        offer: RechargeOffer {
            id: "sandbox-usd-10".into(),
            label: "Fixture only".into(),
            currency: "usd".into(),
            amount_minor: 1000,
            credits: 1000,
        },
        created_at_ms: START + 2000,
        livemode: false,
        checkout_session_id: Some("cs_fixture".into()),
        payment_intent_id: Some("pi_fixture".into()),
        state: RechargeOrderState::Applied,
        paid_at_ms: Some(START + 3000),
        applied: Some(RechargeAppliedReceipt {
            character_index: 1,
            credits: 1000,
            credit_before: 0,
            credit_after: 1000,
            revision: 1,
            at_ms: START + 3001,
        }),
        review_events: BTreeMap::new(),
    };
    state.billing = Some(BillingLedger {
        orders: BTreeMap::from([(order_id.clone(), order)]),
        ..Default::default()
    });
    let receipt = MonthlyCardItemReceipt {
        account_id: account.into(),
        character_index: 1,
        request_id: "monthly-fixture".into(),
        ordinal: 0,
        purchase_quantity: 1,
        unique_id: monthly_uid,
        credit_price: 1000,
        delivery: MonthlyCardItemDelivery::Bag,
        issued_at_ms: START + 4000,
        redeemed_at_ms: Some(START + 5 * DAY),
        credited_until_ms: Some(START + 35 * DAY),
    };
    state.monthly_card = Some(MonthlyCardLedger {
        expires_at_ms: START + 35 * DAY,
        codes: BTreeMap::new(),
        item_receipts: BTreeMap::from([("fixture-monthly".into(), receipt)]),
    });
    client
        .execute(
            "UPDATE accounts SET raw_json=$2 WHERE account_id=$1",
            &[&account, &serde_json::to_value(state).unwrap()],
        )
        .unwrap();
    SandboxMonthlyRevenue {
        allocation_id: format!("allocation-{account}"),
        campaign_id: "season".into(),
        account_id: account.into(),
        recharge_order_id: order_id,
        monthly_item_uid: monthly_uid,
        paid_credits: 1000,
        eligible_micro_usd: 10_000_000,
        original_currency: "usd".into(),
        original_amount_minor: 1000,
        service_start_ms: START + 5 * DAY,
        service_end_ms: START + 35 * DAY,
        fx_snapshot_ref: "usd:1:fixture".into(),
        evidence_ref: "native-pg-fixture-not-stripe-sandbox".into(),
        sandbox: true,
    }
}

fn seed_later_monthly(
    client: &mut Client,
    original: &SandboxMonthlyRevenue,
) -> SandboxMonthlyRevenue {
    let row = client
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id=$1",
            &[&original.account_id],
        )
        .unwrap();
    let mut account: AccountRecord = serde_json::from_value(row.get(0)).unwrap();
    let billing = account.billing.as_mut().unwrap();
    let mut order = billing.orders[&original.recharge_order_id].clone();
    let order_id = format!("rch_{}", "b".repeat(64));
    order.id = order_id.clone();
    order.request_id = "later-recharge-fixture".into();
    order.created_at_ms = START + 34 * DAY;
    order.paid_at_ms = Some(START + 34 * DAY + 1000);
    order.checkout_session_id = Some("cs_later_fixture".into());
    order.payment_intent_id = Some("pi_later_fixture".into());
    order.applied.as_mut().unwrap().at_ms = START + 34 * DAY + 1001;
    billing.orders.insert(order_id.clone(), order);
    let monthly = account.monthly_card.as_mut().unwrap();
    let mut receipt = monthly.item_receipts.values().next().unwrap().clone();
    receipt.request_id = "later-monthly-fixture".into();
    receipt.unique_id = original.monthly_item_uid + 1;
    receipt.issued_at_ms = START + 34 * DAY + 2000;
    receipt.redeemed_at_ms = Some(START + 35 * DAY);
    receipt.credited_until_ms = Some(START + 65 * DAY);
    monthly.expires_at_ms = START + 65 * DAY;
    monthly
        .item_receipts
        .insert("later-monthly-fixture".into(), receipt);
    client
        .execute(
            "UPDATE accounts SET raw_json=$2 WHERE account_id=$1",
            &[
                &original.account_id,
                &serde_json::to_value(account).unwrap(),
            ],
        )
        .unwrap();
    let mut event = original.clone();
    event.allocation_id = format!("late-{}", original.allocation_id);
    event.recharge_order_id = order_id;
    event.monthly_item_uid += 1;
    event.service_start_ms = START + 35 * DAY;
    event.service_end_ms = START + 65 * DAY;
    event
}

#[test]
fn isolated_native_postgres_promotion_invariants() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIPPED promotion PG integration: set MIR2_RUN_PROMOTION_PG_TESTS=1 and a dedicated local URI");
        return;
    }
    let url = std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL")
        .expect("explicit isolated native PG URI required");
    let config: Config = url.parse().expect("valid local URI");
    assert!(
        config.get_dbname().unwrap_or("").contains("promotion"),
        "dedicated promotion database required"
    );
    assert!(config.get_hosts().iter().all(|h|matches!(h,postgres::config::Host::Tcp(name) if name=="127.0.0.1"||name=="localhost")),"local native PG only");
    assert!(
        config.get_options().is_none(),
        "fixture owns its isolated search_path"
    );
    let mut base = Client::connect(&url, NoTls).expect("isolated connection");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("promotion_qa_{}_{}", std::process::id(), nonce);
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let separator = if url.contains('?') { '&' } else { '?' };
    let scoped=format!("{url}{separator}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000");
    let mut client = Client::connect(&scoped, NoTls).expect("isolated scoped connection");
    apply_migrations(&mut client).unwrap();
    apply_migrations(&mut client).unwrap();
    for (id, level) in [
        ("operator", 1),
        ("finance1", 1),
        ("finance2", 1),
        ("risk", 1),
        ("payer", 1),
        ("reviewer", 1),
        ("leader", 30),
        ("otherleader", 30),
        ("member", 30),
        ("oldmember", 30),
    ] {
        insert_account(
            &mut client,
            id,
            level,
            if id == "oldmember" {
                START - 1
            } else {
                START + 1000
            },
        );
    }
    for (id, role) in [
        ("operator", "operations"),
        ("finance1", "finance"),
        ("finance2", "finance"),
        ("risk", "risk"),
        ("payer", "payoutSubmit"),
        ("reviewer", "payoutReview"),
    ] {
        grant(&mut client, id, role);
    }
    let store = PromotionStore::new(scoped.clone(), "test").unwrap();
    let pooled_schema = with_account_store_postgres_client(&scoped, |client| {
        client
            .query_one("SELECT current_schema()", &[])
            .map(|r| r.get::<_, String>(0))
            .map_err(|_| "fixture pool query failed".into())
    })
    .unwrap();
    assert_eq!(pooled_schema, schema);
    let leader = configure(&store, "season", 10_000_000, 10);
    let code = invite_code(&store, "season");
    assert_eq!(
        store
            .invite_preview_at("season", &code, START + 1100)
            .unwrap()["rewardedPlacesAvailable"],
        true
    );
    assert_eq!(
        store
            .apply(
                "season",
                "member",
                99,
                TERMS,
                "steal-character",
                START + 1001
            )
            .unwrap_err(),
        "promotionCharacterMissing"
    );
    assert_eq!(
        store
            .admin_view("season", "member", START + 1001)
            .unwrap_err(),
        "promotionPermissionDenied"
    );
    assert_eq!(
        store
            .create_candidate(
                "season",
                &code,
                "leader",
                "manual_code",
                "self",
                START + 1200
            )
            .unwrap_err(),
        "promotionSelfReferral"
    );
    let old = store
        .create_candidate(
            "season",
            &code,
            "oldmember",
            "manual_code",
            "old",
            START + 1200,
        )
        .unwrap();
    assert_eq!(
        store
            .confirm(
                "season",
                "oldmember",
                old["candidateId"].as_str().unwrap(),
                TERMS,
                "old-confirm",
                START + 1300
            )
            .unwrap_err(),
        "promotionExistingAccountExcluded"
    );
    let bound = bind(&store, "season", "member", &code);
    assert_eq!(bound["status"], "bound");
    assert_eq!(
        store
            .invite_preview_at("season", &code, START + 2000)
            .unwrap()["rewardedPlacesAvailable"],
        false
    );
    assert_eq!(
        store
            .invite_preview_at("season", &code, START + 2000)
            .unwrap()["capacityReason"],
        "budget_full"
    );
    assert_eq!(
        store.invite_preview_at("season", &code, END).unwrap()["capacityReason"],
        "campaign_ended"
    );
    assert_eq!(
        store
            .members("season", "otherleader", 0, 20, START + 3000)
            .unwrap_err(),
        "promotionLeaderMissing"
    );
    let before = store.me("season", "leader", START + 3000).unwrap();
    assert_eq!(before["dashboard"]["estimatedCommissionMicroUsd"], "0");
    let event = seed_paid_monthly(&mut client, "member");
    let mut shortened = event.clone();
    shortened.allocation_id = "shortened-service".into();
    shortened.service_start_ms = shortened.service_end_ms - DAY;
    assert_eq!(
        store
            .ingest_sandbox_monthly_revenue(&shortened, START + 6 * DAY)
            .unwrap_err(),
        "promotionSubscriptionEvidenceMissing"
    );
    assert_eq!(
        store
            .ingest_reversal(
                &event.allocation_id,
                "refund-before-allocation",
                1_000_000,
                "fixture-early-refund",
                START + 7000,
                START + 6 * DAY
            )
            .unwrap()["status"],
        "pending_source"
    );
    assert!(store
        .ingest_sandbox_monthly_revenue(&event, START + 6 * DAY)
        .unwrap()["accepted"]
        .as_bool()
        .unwrap());
    assert_eq!(
        store
            .ingest_sandbox_monthly_revenue(&event, START + 6 * DAY)
            .unwrap()["replayed"],
        true
    );
    let mut bad = event.clone();
    bad.eligible_micro_usd += 1;
    assert_eq!(
        store
            .ingest_sandbox_monthly_revenue(&bad, START + 6 * DAY)
            .unwrap_err(),
        "promotionSourceEventConflict"
    );
    let level = TrustedGameFact {
        source: "level_commit".into(),
        kind: "levelCommitted".into(),
        source_event_id: "level-1".into(),
        account_id: "member".into(),
        character_index: 1,
        occurred_at_ms: START + 2000,
        level: Some(30),
        evidence_ref: "committed-save-fixture".into(),
    };
    store.record_game_fact(&level, START + 6 * DAY).unwrap();
    assert_eq!(
        store.record_game_fact(&level, START + 6 * DAY).unwrap()["replayed"],
        true
    );
    for day in 0..3 {
        let event = TrustedGameFact {
            source: "combat_commit".into(),
            kind: "combatKill".into(),
            source_event_id: format!("kill-{day}"),
            account_id: "member".into(),
            character_index: 1,
            occurred_at_ms: START + 3000 + day * DAY,
            level: None,
            evidence_ref: "durable-combat-fixture".into(),
        };
        store.record_game_fact(&event, START + 6 * DAY).unwrap();
    }
    let dashboard = store.me("season", "leader", START + 6 * DAY).unwrap();
    assert_eq!(dashboard["dashboard"]["qualified"], 1);
    assert_eq!(
        dashboard["dashboard"]["estimatedCommissionMicroUsd"],
        "900000"
    );
    let member_qualification = store.me("season", "member", START + 6 * DAY).unwrap();
    assert_eq!(
        member_qualification["attribution"]["qualifiedAtMs"],
        START + 5 * DAY
    );
    assert_eq!(
        store.public_campaign("season").unwrap()["dataMode"],
        "sandbox"
    );
    let private = store
        .members("season", "leader", 0, 20, START + 6 * DAY)
        .unwrap();
    assert!(private["items"][0]["memberRef"]
        .as_str()
        .unwrap()
        .starts_with("member-"));
    assert!(!private.to_string().contains("\"accountId\""));
    let mut invalid = level.clone();
    invalid.kind = "heartbeat".into();
    assert_eq!(
        store
            .record_game_fact(&invalid, START + 6 * DAY)
            .unwrap_err(),
        "promotionSourceEventInvalid"
    );
    command(
        &store,
        "season",
        "finance1",
        json!({"action":"reconcile","evidenceRef":"all-fixture-inputs"}),
        "reconcile",
        END + 1,
    );
    assert_eq!(store.admin("season","finance1",json!({"action":"createSettlement","leaderId":leader,"evidenceRef":"missing-watermark"}),"seal-too-early",END+1).unwrap_err(),"promotionReconciliationIncomplete");
    command(
        &store,
        "season",
        "finance1",
        json!({"action":"setInputWatermark","watermarkMs":END,"evidenceRef":"all-fixture-inputs-complete"}),
        "watermark",
        END + 1,
    );
    let settlement = command(
        &store,
        "season",
        "finance1",
        json!({"action":"createSettlement","leaderId":leader,"evidenceRef":"fixture-close"}),
        "settle",
        END + 1,
    );
    let settlement_id = settlement["settlementId"].as_str().unwrap();
    let sha = settlement["snapshotSha256"].as_str().unwrap();
    assert_eq!(store.admin("season","finance1",json!({"action":"approveSettlement","settlementId":settlement_id,"snapshotSha256":sha,"evidenceRef":"self"}),"self-settle",END+2).unwrap_err(),"promotionDistinctReviewerRequired");
    command(
        &store,
        "season",
        "finance2",
        json!({"action":"approveSettlement","settlementId":settlement_id,"snapshotSha256":sha,"evidenceRef":"second-person"}),
        "approve-settle",
        END + 2,
    );
    assert_eq!(
        store
            .settlement_export("season", "leader", settlement_id, END + 2)
            .unwrap_err(),
        "promotionPermissionDenied"
    );
    let exported = store
        .settlement_export("season", "finance2", settlement_id, END + 2)
        .unwrap();
    let canonical = exported["snapshotCanonicalJson"].as_str().unwrap();
    let digest = Sha256::digest(canonical.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(digest, exported["snapshotSha256"].as_str().unwrap());
    assert!(exported.get("snapshot").is_none());
    if let Ok(directory) = std::env::var("MIR2_PROMOTION_TEST_EVIDENCE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        assert!(
            directory.is_absolute(),
            "explicit evidence directory must be absolute"
        );
        std::fs::create_dir_all(&directory).unwrap();
        let mut sample = exported.clone();
        sample["fixtureData"] = json!(true);
        sample["dataMode"] = json!("sandbox");
        sample["fixtureDescription"] =
            json!("Synthetic native PG seed; not a real Stripe sandbox checkout or payable debt");
        std::fs::write(
            directory.join("promotion-settlement-fixture.json"),
            serde_json::to_vec_pretty(&sample).unwrap(),
        )
        .unwrap();
    }
    let late_allocation = seed_later_monthly(&mut client, &event);
    assert_eq!(
        store
            .ingest_sandbox_monthly_revenue(&late_allocation, END + 2)
            .unwrap()["accepted"],
        true
    );
    assert_eq!(
        store.me("season", "leader", END + 2).unwrap()["dashboard"]["estimatedCommissionMicroUsd"],
        "900000"
    );
    let wallet = format!("0x{}", "1".repeat(64));
    let other_wallet = format!("0x{}", "2".repeat(64));
    client.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms) VALUES('leader-wallet','leader','sui_wallet',$1,1),('other-wallet','otherleader','sui_wallet',$2,1)",&[&wallet,&other_wallet]).unwrap();
    assert_eq!(
        store
            .verify_payout_address(
                "leader",
                "other-wallet",
                "sui:testnet",
                "steal-wallet",
                END + 2
            )
            .unwrap_err(),
        "promotionWalletProofRequired"
    );
    store
        .verify_payout_address(
            "leader",
            "leader-wallet",
            "sui:testnet",
            "own-wallet",
            END + 2,
        )
        .unwrap();
    assert_eq!(store.admin("season","payer",json!({"action":"createPayoutDraft","settlementId":settlement_id,"network":"sui:testnet","coinType":"fixture-usdc-type-not-approved","evidenceRef":"fixture-draft"}),"cooldown-payout",END+3).unwrap_err(),"promotionWalletCooldown");
    store
        .ingest_reversal(
            &event.allocation_id,
            "refund-1",
            5_000_000,
            "fixture-refund",
            END + 3,
            END + 3,
        )
        .unwrap();
    assert_eq!(
        store
            .ingest_reversal(
                &event.allocation_id,
                "refund-1",
                5_000_000,
                "fixture-refund",
                END + 3,
                END + 3
            )
            .unwrap()["replayed"],
        true
    );
    assert_eq!(
        store.me("season", "leader", END + 4).unwrap()["dashboard"]["estimatedCommissionMicroUsd"],
        "400000"
    );
    let payout = command(
        &store,
        "season",
        "payer",
        json!({"action":"createPayoutDraft","settlementId":settlement_id,"network":"sui:testnet","coinType":"fixture-usdc-type-not-approved","evidenceRef":"fixture-draft"}),
        "draft-payout",
        END + 4 * DAY,
    );
    assert_eq!(payout["payoutEnabled"], false);
    assert_eq!(payout["amountMicroUsd"], "310000");
    assert_eq!(store.admin("season","reviewer",json!({"action":"approvePayout","payoutId":payout["payoutId"],"evidenceRef":"second-person-fixture"}),"payout-review",END+4*DAY).unwrap_err(),"promotionPayoutIntentRequired");
    assert_eq!(
        store
            .admin(
                "season",
                "payer",
                json!({"action":"submitPayout","payoutId":"nonexistent"}),
                "cannot-pay",
                END + 4
            )
            .unwrap_err(),
        "promotionPayoutSignedEnvelopeRequired"
    );
    let audit_count = client
        .query_one("SELECT count(*) FROM promotion_audit_log", &[])
        .unwrap()
        .get::<_, i64>(0);
    assert!(audit_count > 20);
    assert!(client
        .execute("UPDATE promotion_audit_log SET actor_id='forged'", &[])
        .is_err());
    assert!(client
        .execute(
            "UPDATE promotion_audit_log SET actor_id='forged' WHERE FALSE",
            &[]
        )
        .is_err());
    assert!(client
        .batch_execute("TRUNCATE promotion_audit_log")
        .is_err());
    // A second season proves concurrent callers cannot overreserve two places.
    configure(&store, "race", 18_000_000, 2);
    let race_code = invite_code(&store, "race");
    let mut candidates = Vec::new();
    for n in 0..8 {
        let id = format!("racer-{n}");
        insert_account(&mut client, &id, 1, START + 1000);
        let candidate = store
            .create_candidate(
                "race",
                &race_code,
                &id,
                "manual_code",
                &format!("race-cand-{n}"),
                START + 1200,
            )
            .unwrap();
        candidates.push((id, candidate["candidateId"].as_str().unwrap().to_owned()));
    }
    let barrier = Arc::new(Barrier::new(candidates.len()));
    let threads = candidates
        .into_iter()
        .enumerate()
        .map(|(n, (id, candidate))| {
            let store = store.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.confirm(
                    "race",
                    &id,
                    &candidate,
                    TERMS,
                    &format!("race-confirm-{n}"),
                    START + 1500,
                )
            })
        })
        .collect::<Vec<_>>();
    let results = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 2);
    assert!(results
        .iter()
        .filter_map(|r| r.as_ref().err())
        .all(|e| e == "promotionBudgetFull"));
    let row=client.query_one("SELECT reserved_micro_usd,(SELECT count(*) FROM promotion_attributions WHERE environment='test' AND campaign_id='race') FROM promotion_campaigns WHERE environment='test' AND campaign_id='race'",&[]).unwrap();
    assert_eq!(row.get::<_, i64>(0), 18_000_000);
    assert_eq!(row.get::<_, i64>(1), 2);
    // A full refund of the first card does not postpone an already-established
    // member while a later renewal still has eligible income.
    insert_account(&mut client, "stable-member", 30, START + 1000);
    configure(&store, "stable", 9_000_000, 1);
    let stable_code = invite_code(&store, "stable");
    bind(&store, "stable", "stable-member", &stable_code);
    let mut stable_first = seed_paid_monthly(&mut client, "stable-member");
    stable_first.campaign_id = "stable".into();
    store
        .ingest_sandbox_monthly_revenue(&stable_first, START + 6 * DAY)
        .unwrap();
    let mut stable_level = level.clone();
    stable_level.account_id = "stable-member".into();
    stable_level.source_event_id = "stable-level".into();
    store
        .record_game_fact(&stable_level, START + 6 * DAY)
        .unwrap();
    for day in 0..3 {
        store
            .record_game_fact(
                &TrustedGameFact {
                    source: "combat_commit".into(),
                    kind: "combatKill".into(),
                    source_event_id: format!("stable-kill-{day}"),
                    account_id: "stable-member".into(),
                    character_index: 1,
                    occurred_at_ms: START + 3000 + day * DAY,
                    level: None,
                    evidence_ref: "stable-durable-fixture".into(),
                },
                START + 6 * DAY,
            )
            .unwrap();
    }
    let stable_later = seed_later_monthly(&mut client, &stable_first);
    store
        .ingest_sandbox_monthly_revenue(&stable_later, START + 40 * DAY)
        .unwrap();
    store
        .ingest_reversal(
            &stable_first.allocation_id,
            "stable-first-refund",
            10_000_000,
            "stable-first-full-refund",
            START + 41 * DAY,
            START + 41 * DAY,
        )
        .unwrap();
    let stable = store
        .me("stable", "stable-member", START + 41 * DAY)
        .unwrap();
    assert_eq!(stable["attribution"]["qualifiedAtMs"], START + 5 * DAY);
    assert_eq!(stable["attribution"]["status"], "qualified");
    store
        .ingest_reversal(
            &stable_later.allocation_id,
            "stable-last-refund",
            10_000_000,
            "stable-all-income-refunded",
            START + 42 * DAY,
            START + 42 * DAY,
        )
        .unwrap();
    assert!(store
        .me("stable", "stable-member", START + 42 * DAY)
        .unwrap()["attribution"]["qualifiedAtMs"]
        .is_null());

    // The consumer is bounded and transactional, projects immutable source
    // state once per campaign and explicitly leaves external delivery blocked.
    let work_now = END + 5 * DAY;
    let drained = store.process_pending_work(100, work_now).unwrap();
    assert!(drained["completed"].as_u64().unwrap() > 0);
    assert_eq!(drained["externalDispatchImplemented"], false);
    let run_count = client
        .query_one("SELECT count(*) FROM promotion_commission_runs", &[])
        .unwrap()
        .get::<_, i64>(0);
    let insert_work = |client: &mut Client, label: &str, at: u64, kind: &str| -> i64 {
        client.query_one("INSERT INTO promotion_outbox(environment,event_type,resource_id,event_json,created_at_ms) VALUES('test',$1,$2,$3,$4) RETURNING outbox_id",&[&kind,&label,&json!({"campaignId":"race"}),&(at as i64)]).unwrap().get(0)
    };
    // A corrupted reservation cache fails without committing a projection;
    // after an explicit operator repair the durable failed item can retry.
    client.execute("UPDATE promotion_campaigns SET reserved_micro_usd=0 WHERE environment='test' AND campaign_id='race'",&[]).unwrap();
    let failed_id = insert_work(
        &mut client,
        "worker-budget-retry",
        work_now + 1,
        "admin.command",
    );
    let failed = store.process_pending_work(1, work_now + 1).unwrap();
    assert_eq!(failed["failed"], 1);
    let failed_row = client
        .query_one(
            "SELECT status,attempts,last_error_code FROM promotion_outbox WHERE outbox_id=$1",
            &[&failed_id],
        )
        .unwrap();
    assert_eq!(failed_row.get::<_, String>(0), "failed");
    assert_eq!(failed_row.get::<_, i32>(1), 1);
    assert_eq!(
        failed_row.get::<_, String>(2),
        "promotionBudgetReservationMismatch"
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM promotion_commission_runs", &[])
            .unwrap()
            .get::<_, i64>(0),
        run_count
    );
    client.execute("UPDATE promotion_campaigns SET reserved_micro_usd=18000000 WHERE environment='test' AND campaign_id='race'",&[]).unwrap();
    store.process_pending_work(100, work_now + 2000).unwrap();
    let recovered = client
        .query_one(
            "SELECT status,attempts,last_error_code FROM promotion_outbox WHERE outbox_id=$1",
            &[&failed_id],
        )
        .unwrap();
    assert_eq!(recovered.get::<_, String>(0), "completed");
    assert_eq!(recovered.get::<_, i32>(1), 2);
    assert!(recovered.get::<_, Option<String>>(2).is_none());
    // Two workers compete for the same due queue. SKIP LOCKED claims each row
    // once; repeated batches do not append an identical input-hash run.
    let mut concurrent_ids = Vec::new();
    for n in 0..20 {
        concurrent_ids.push(insert_work(
            &mut client,
            &format!("worker-race-{n}"),
            work_now + 2001,
            "admin.command",
        ));
    }
    let work_barrier = Arc::new(Barrier::new(2));
    let workers = (0..2)
        .map(|_| {
            let store = store.clone();
            let barrier = Arc::clone(&work_barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.process_pending_work(5, work_now + 2001).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let reports = workers
        .into_iter()
        .map(|w| w.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        reports
            .iter()
            .map(|r| r["claimed"].as_u64().unwrap())
            .sum::<u64>(),
        10
    );
    assert!(reports.iter().all(|r| r["failed"] == 0));
    store.process_pending_work(100, work_now + 2002).unwrap();
    let exactly_once=client.query_one("SELECT count(*),min(attempts),max(attempts),count(*) FILTER(WHERE status='completed') FROM promotion_outbox WHERE outbox_id=ANY($1)",&[&concurrent_ids]).unwrap();
    assert_eq!(exactly_once.get::<_, i64>(0), 20);
    assert_eq!(exactly_once.get::<_, i32>(1), 1);
    assert_eq!(exactly_once.get::<_, i32>(2), 1);
    assert_eq!(exactly_once.get::<_, i64>(3), 20);
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM promotion_commission_runs", &[])
            .unwrap()
            .get::<_, i64>(0),
        run_count
    );
    // An uncommitted claim is rolled back to pending, including its attempt
    // increment, just as on a worker crash before commit.
    let rollback_id = insert_work(
        &mut client,
        "worker-rollback",
        work_now + 2100,
        "admin.command",
    );
    {
        let mut claim = client.transaction().unwrap();
        claim
            .query_one(
                "SELECT outbox_id FROM promotion_outbox WHERE outbox_id=$1 FOR UPDATE SKIP LOCKED",
                &[&rollback_id],
            )
            .unwrap();
        claim.execute("UPDATE promotion_outbox SET status='processing',attempts=attempts+1 WHERE outbox_id=$1",&[&rollback_id]).unwrap();
        claim.rollback().unwrap();
    }
    let rolled = client
        .query_one(
            "SELECT status,attempts FROM promotion_outbox WHERE outbox_id=$1",
            &[&rollback_id],
        )
        .unwrap();
    assert_eq!(rolled.get::<_, String>(0), "pending");
    assert_eq!(rolled.get::<_, i32>(1), 0);
    store.process_pending_work(1, work_now + 2100).unwrap();
    let external_id = insert_work(
        &mut client,
        "worker-external-payment-sync",
        work_now + 2200,
        "payment.sync.requested",
    );
    store.process_pending_work(1, work_now + 2200).unwrap();
    let external = client
        .query_one(
            "SELECT status,last_error_code,result_json FROM promotion_outbox WHERE outbox_id=$1",
            &[&external_id],
        )
        .unwrap();
    assert_eq!(external.get::<_, String>(0), "failed");
    assert_eq!(
        external.get::<_, String>(1),
        "promotionWorkExternalAdapterRequired"
    );
    assert_eq!(
        external.get::<_, Value>(2)["externalDispatchCompleted"],
        false
    );
    let health = store
        .admin_view("race", "finance1", work_now + 2200)
        .unwrap();
    assert_eq!(
        health["queueHealth"]["internalProjectionWorkerImplemented"],
        true
    );
    assert_eq!(health["queueHealth"]["externalDispatchImplemented"], false);
    assert!(health["queueHealth"]["failed"].as_u64().unwrap() >= 1);
    // The isolated schema is retained for inspection; no production/shared data
    // is deleted, no transaction is signed and no real payable debt is created.
    eprintln!("promotion native PG fixture passed; isolated schema {schema}; actual Stripe/phone/testnet transfer not exercised");
}

fn seed_attribution_usdc_fixture(
    client: &mut Client,
    account: &str,
    marker: char,
    native: bool,
    paid: bool,
) {
    // Begin at the trusted already-verified receipt boundary. This fixture
    // exercises the real typed ledger and SQL authority guards, not a mocked
    // chain RPC or a claim that a synthetic transaction occurred on Testnet.
    let payer = format!("0x{}", marker.to_string().repeat(64));
    let account_ref = format!(
        "0x{}",
        Sha256::digest(account.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let mut order: UsdcRechargeOrder = serde_json::from_value(json!({
        "id":format!("usdc_{}",marker.to_string().repeat(64)),
        "requestId":format!("paid-gate-{account}"),"characterIndex":1,
        "offer":{"id":"test-usdc-1","label":"Synthetic paid-gate fixture","currency":"usd","amountMinor":100,"credits":100},
        "domain":{"chainIdentifier":"4c78adac","packageId":format!("0x{}","1".repeat(64)),
            "configId":format!("0x{}","2".repeat(64)),"merchant":format!("0x{}","3".repeat(64)),
            "coinType":TESTNET_USDC_COIN_TYPE,"ruleVersion":1},
        "creditOrigin":"test","accountRef":account_ref,
        "amountMicro":1_000_000,"createdAtMs":START+2000,"expiresAtMs":START+1_200_000,
        "capabilityHash":"c".repeat(64),"capabilityExpiresAtMs":START+1_800_000,
        "payer":payer,"state":"payerBound","preparedTx":null,"invoice":null,
        "paymentHint":null,"receipt":null,"applied":null,"jobLease":null,"reviewReason":null
    }))
    .unwrap();
    if native {
        order.payment_mode = UsdcPaymentMode::DirectTransfer;
        client.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms) VALUES($1,$2,'sui_wallet',$3,$4)",
            &[&format!("paid-gate-{account}"),&account,&payer,&((START+1000) as i64)]).unwrap();
    } else {
        order.state = UsdcOrderState::InvoiceBound;
        order.prepared_tx = Some(
            serde_json::from_value(json!({
                "bytesBase64":"AQ==","signatureBase64":"AQ==","digest":marker.to_string().repeat(44)
            }))
            .unwrap(),
        );
        order.invoice = Some(
            serde_json::from_value(json!({
                "invoiceId":format!("0x{}",marker.to_string().repeat(64)),
                "createDigest":marker.to_string().repeat(44),"checkpoint":100
            }))
            .unwrap(),
        );
    }
    if paid {
        order.state = UsdcOrderState::PaidPending;
        if native {
            order.transfer_receipt = Some(VerifiedNativeUsdcTransferReceipt {
                chain_identifier: order.domain.chain_identifier.clone(),
                genesis_digest: TESTNET_GENESIS_DIGEST.into(),
                order_id: order.id.clone(),
                account_ref: order.account_ref.clone(),
                payer: payer.clone(),
                merchant: order.domain.merchant.clone(),
                coin_type: order.domain.coin_type.clone(),
                amount_micro: order.amount_micro,
                frozen_hash: order.frozen_hash().unwrap(),
                digest: format!("{marker}{}", "Z".repeat(43)),
                checkpoint: 101,
                confirmed_at_ms: START + 3000,
                observed_at_ms: START + 3100,
            });
            assert!(order.receipt.is_none());
        } else {
            order.receipt = Some(VerifiedUsdcReceipt {
                chain_identifier: order.domain.chain_identifier.clone(),
                package_id: order.domain.package_id.clone(),
                config_id: order.domain.config_id.clone(),
                invoice_id: order.invoice.as_ref().unwrap().invoice_id.clone(),
                order_id: order.id.clone(),
                account_ref: order.account_ref.clone(),
                payer: payer.clone(),
                merchant: order.domain.merchant.clone(),
                coin_type: order.domain.coin_type.clone(),
                amount_micro: order.amount_micro,
                frozen_hash: order.frozen_hash().unwrap(),
                digest: format!("{marker}{}", "Z".repeat(43)),
                event_seq: 0,
                checkpoint: 101,
                confirmed_at_ms: START + 3000,
            });
            assert!(order.transfer_receipt.is_none());
        }
    }
    order.validate().unwrap();
    assert_eq!(order.has_payment_receipt(), paid);
    let raw: Value = client
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id=$1",
            &[&account],
        )
        .unwrap()
        .get(0);
    let mut saved: AccountRecord = serde_json::from_value(raw).unwrap();
    saved.billing = Some(BillingLedger {
        sui_usdc_orders: BTreeMap::from([(order.id.clone(), order)]),
        ..Default::default()
    });
    client
        .execute(
            "UPDATE accounts SET raw_json=$2 WHERE account_id=$1",
            &[&account, &serde_json::to_value(saved).unwrap()],
        )
        .unwrap();
}

#[test]
fn native_postgres_attribution_rejects_both_paid_usdc_modes_and_preserves_unpaid_admission() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!(
            "SKIPPED paid-attribution PG integration: explicit isolated native PG opt-in required"
        );
        return;
    }
    let url =
        std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL").expect("isolated native PG URI required");
    let config: Config = url.parse().unwrap();
    assert!(config.get_dbname().unwrap_or("").contains("promotion"));
    assert!(config.get_hosts().iter().all(
        |h| matches!(h, postgres::config::Host::Tcp(host) if host=="127.0.0.1"||host=="localhost")
    ));
    assert!(config.get_options().is_none());
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("promotion_paid_gate_{}_{}", std::process::id(), nonce);
    let mut base = Client::connect(&url, NoTls).unwrap();
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let scoped = format!("{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000", if url.contains('?') {'&'} else {'?'});
    let mut client = Client::connect(&scoped, NoTls).unwrap();
    apply_migrations(&mut client).unwrap();
    apply_migrations(&mut client).unwrap();
    for account in [
        "operator",
        "finance1",
        "risk",
        "leader",
        "native-paid",
        "invoice-paid",
        "stripe-paid",
        "native-unpaid",
        "invoice-unpaid",
    ] {
        insert_account(&mut client, account, 30, START + 1000);
    }
    for (account, role) in [
        ("operator", "operations"),
        ("finance1", "finance"),
        ("risk", "risk"),
    ] {
        grant(&mut client, account, role);
    }
    for (account, marker, native, paid) in [
        ("native-paid", 'a', true, true),
        ("invoice-paid", 'b', false, true),
        ("native-unpaid", 'c', true, false),
        ("invoice-unpaid", 'd', false, false),
    ] {
        seed_attribution_usdc_fixture(&mut client, account, marker, native, paid);
    }
    seed_paid_monthly(&mut client, "stripe-paid");
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM sui_usdc_transaction_claims", &[])
            .unwrap()
            .get::<_, i64>(0),
        2
    );
    let store = PromotionStore::new(scoped, "test").unwrap();
    configure(&store, "paid-gate", 1_000_000_000, 10);
    let code = invite_code(&store, "paid-gate");
    let mut expected_reserve = 0u64;
    for (account, paid) in [
        ("native-paid", true),
        ("invoice-paid", true),
        ("stripe-paid", true),
        ("native-unpaid", false),
        ("invoice-unpaid", false),
    ] {
        let candidate = store
            .create_candidate(
                "paid-gate",
                &code,
                account,
                "manual_code",
                &format!("candidate-{account}"),
                START + 4000,
            )
            .unwrap();
        let before: Value = client
            .query_one(
                "SELECT raw_json FROM accounts WHERE account_id=$1",
                &[&account],
            )
            .unwrap()
            .get(0);
        let result = store.confirm(
            "paid-gate",
            account,
            candidate["candidateId"].as_str().unwrap(),
            TERMS,
            &format!("confirm-{account}"),
            START + 4500,
        );
        if paid {
            assert_eq!(result.unwrap_err(), "promotionAlreadyPaid", "{account}");
        } else {
            let bound = result.unwrap();
            assert_eq!(bound["status"], "bound", "{account}");
            expected_reserve += bound["reservedMicroUsd"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();
        }
        assert_eq!(client.query_one("SELECT count(*) FROM promotion_attributions WHERE environment='test' AND campaign_id='paid-gate' AND account_id=$1", &[&account]).unwrap().get::<_,i64>(0), if paid {0} else {1});
        assert_eq!(client.query_one("SELECT reserved_micro_usd FROM promotion_campaigns WHERE environment='test' AND campaign_id='paid-gate'", &[]).unwrap().get::<_,i64>(0) as u64, expected_reserve);
        let after: Value = client
            .query_one(
                "SELECT raw_json FROM accounts WHERE account_id=$1",
                &[&account],
            )
            .unwrap()
            .get(0);
        assert_eq!(
            after, before,
            "attribution must not rewrite {account}'s billing or other account fields"
        );
    }
    assert!(expected_reserve > 0);
    eprintln!("paid-attribution native PG fixture passed in isolated schema {schema}; no chain RPC or payment performed");
}

//! Explicit fixtures only: no game, provider, signer or chain is contacted.
//! These tests do not certify human review or a real revenue acceptance run.
use mir2_simulation::{apply_migrations, promotion::*};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn input() -> CommissionInput {
    CommissionInput {
        environment: "test".into(),
        campaign_id: "micro-fixture".into(),
        terms_version: "fixture".into(),
        starts_at_ms: 1,
        ends_at_ms: 600_001,
        maximum_coverage_ms: 600_000,
        per_member_revenue_cap_micro_usd: 10_000_000,
        reserve_bps: 0,
        members: vec![MemberInput {
            account_id: "member".into(),
            level_completed_at_ms: Some(2),
            play_completed_at_ms: Some(3),
            established_qualification_at_ms: None,
            excluded: false,
            revenue: vec![RevenueInput {
                allocation_id: "fixture-monthly".into(),
                paid_at_ms: 4,
                entitlement_at_ms: 4,
                service_start_ms: 1,
                service_end_ms: 1 + 30 * 86_400_000,
                amount_micro_usd: 10_000_000,
                reversed_micro_usd: 0,
                is_card_payment: true,
            }],
        }],
    }
}
#[test]
fn real_thirty_day_proration_can_pay_micro_usd_without_a_minimum_grant() {
    let mut i = input();
    let legacy = calculate_commission(&i).unwrap();
    let micro = calculate_test_usdc_micro_commission(&i).unwrap();
    assert_eq!(micro.eligible_revenue_micro_usd, 2314);
    assert_eq!(micro.commission_micro_usd, 231);
    assert_eq!(micro.payable_micro_usd, Some(231));
    assert_eq!(legacy.payable_cents, 0);
    assert_eq!(legacy.payable_micro_usd, None);
    assert_eq!(micro.members, legacy.members);
    assert!(serde_json::to_value(&legacy)
        .unwrap()
        .get("payableMicroUsd")
        .is_none());
    i.reserve_bps = 1000;
    let reserved = calculate_test_usdc_micro_commission(&i).unwrap();
    assert_eq!(reserved.reserve_micro_usd, 24);
    assert_eq!(reserved.payable_micro_usd, Some(207));
    i.members[0].revenue[0].amount_micro_usd = 1;
    assert_eq!(
        calculate_test_usdc_micro_commission(&i)
            .unwrap()
            .payable_micro_usd,
        Some(0)
    );
    i.environment = "production".into();
    assert_eq!(
        calculate_test_usdc_micro_commission(&i).unwrap_err(),
        "promotionEnvironmentMismatch"
    );
    i.environment = "development".into();
    assert!(calculate_test_usdc_micro_commission(&i).is_err());
}
#[test]
fn default_rules_bytes_and_non_test_gates_remain_compatible() {
    let mut rules = PromotionRules::default();
    assert_eq!(
        serde_json::to_value(&rules).unwrap(),
        json!({"minimumLevel":30,"activeUtcDays":3,
        "candidateTtlMs":604800000,"confirmationWindowMs":86400000,"maximumCoverageMs":7948800000u64,
        "reserveBps":1000,"walletCooldownMs":259200000})
    );
    rules.test_usdc_micro_payouts = true;
    assert!(rules.validate_environment("test").is_ok());
    assert!(rules.validate_environment("production").is_err());
    assert!(rules.validate_environment("development").is_err());
    rules.test_first_payout_address_no_cooldown = true;
    assert!(rules.validate_environment("test").is_ok());
    rules.test_usdc_micro_payouts = false;
    assert!(rules.validate().is_err());
}

fn command(
    s: &PromotionStore,
    campaign: &str,
    actor: &str,
    body: Value,
    key: &str,
    now: u64,
) -> Result<Value, String> {
    s.admin(campaign, actor, body, key, now)
}
fn fixture(
    client: &mut Client,
    s: &PromotionStore,
    name: &str,
    first: bool,
) -> PromotionPayoutIntent {
    let mut data = input();
    data.campaign_id = format!("micro-{name}");
    let c = &data.campaign_id;
    let mut rules = PromotionRules::default();
    rules.test_usdc_micro_payouts = true;
    rules.test_first_payout_address_no_cooldown = first;
    rules.minimum_level = 1;
    rules.active_utc_days = 1;
    rules.reserve_bps = 0;
    let draft = CampaignDraft {
        campaign_id: c.clone(),
        funding_mode: "revenue_share".into(),
        terms_version: "fixture".into(),
        starts_at_ms: data.starts_at_ms,
        ends_at_ms: data.ends_at_ms,
        rules,
        budget_micro_usd: 0,
        member_limit: 0,
        leader_limit: 0,
        per_leader_member_limit: 0,
        per_member_revenue_cap_micro_usd: 0,
    };
    command(
        s,
        c,
        "ops",
        json!({"action":"configureDraft","draft":draft,"evidenceRef":"synthetic-test"}),
        &format!("configure-{name}"),
        10,
    )
    .unwrap();
    command(
        s,
        c,
        "reviewer",
        json!({"action":"approveRules","evidenceRef":"synthetic-not-human-review"}),
        &format!("rules-{name}"),
        11,
    )
    .unwrap();
    let view = s.admin_view(c, "ops", 12).unwrap();
    let manifest:PromotionLaunchManifest=serde_json::from_value(json!({"configurationSha256":view["launchConfigurationSha256"],
        "network":"sui:testnet","chainIdentifier":TESTNET_CHAIN_IDENTIFIER,"coinType":TESTNET_USDC_COIN_TYPE,"coinDecimals":6,
        "fxSnapshotRef":"test-usdc-usd-1to1","minimumPayoutMicroUsd":"1","gasFeePolicy":"treasury",
        "identityTaxPolicyRef":"explicit-test-fixture","regions":["US"],"termsUrl":"https://fixture.invalid/terms",
        "mobileSignatureEvidence":"","providerPaymentEvidence":"","onchainPayoutDigest":""})).unwrap();
    let mut bad = manifest.clone();
    bad.coin_type = "0x2::sui::SUI".into();
    bad.coin_decimals = 9;
    assert_eq!(
        s.propose_launch(c, "ops", &bad, "fixture-wrong-coin", 12)
            .unwrap_err(),
        "promotionPayoutManifestMismatch"
    );
    let proposal = s
        .propose_launch(c, "ops", &manifest, "synthetic-test-only", 12)
        .unwrap();
    s.approve_launch(
        c,
        "reviewer",
        proposal["manifestSha256"].as_str().unwrap(),
        "synthetic-test-only",
        13,
    )
    .unwrap();
    command(
        s,
        c,
        "reviewer",
        json!({"action":"setPayout","enabled":true,"evidenceRef":"synthetic-test"}),
        &format!("enable-{name}"),
        14,
    )
    .unwrap();
    let leader = format!("leader-{name}");
    let settle = format!("settlement-{name}");
    client.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,status,accepted_terms_version,applied_at_ms) VALUES('test',$1,'leader',$2,$2,1,'fixture','approved','fixture',1)",&[&c,&leader]).unwrap();
    let result = calculate_test_usdc_micro_commission(&data).unwrap();
    let snapshot = json!({"input":data,"result":result,"inputWatermarkMs":600001,"evidenceRef":"synthetic-no-real-income"});
    let hash = Sha256::digest(serde_json::to_vec(&snapshot).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    client.execute("INSERT INTO promotion_settlements(environment,campaign_id,settlement_id,leader_id,status,algorithm_version,snapshot_json,snapshot_sha256,total_micro_usd,payable_cents,reserve_micro_usd,created_by,reviewed_by,created_at_ms,reviewed_at_ms) VALUES('test',$1,$2,$3,'approved',$4,$5,$6,231,0,0,'ops','reviewer',1,2)",&[&c,&settle,&leader,&ALGORITHM_VERSION,&snapshot,&hash]).unwrap();
    let address=client.query_one("SELECT version,wallet_address FROM promotion_payout_addresses WHERE environment='test' AND account_id='leader' ORDER BY version DESC LIMIT 1",&[]).unwrap();
    serde_json::from_value(json!({"version":"numeron-sui-payout-v1","environment":"test","campaignId":c,
        "payoutId":"placeholder","settlementId":settle,"settlementSha256":hash,"network":"sui:testnet",
        "chainIdentifier":TESTNET_CHAIN_IDENTIFIER,"coinType":TESTNET_USDC_COIN_TYPE,"coinDecimals":6,
        "amountMicroUsd":"231","coinAmountAtomic":"231","quoteNumerator":"1","quoteDenominator":"1",
        "fxSnapshotRef":"test-usdc-usd-1to1","destination":address.get::<_,String>(1),"addressVersion":address.get::<_,i64>(0).to_string(),
        "sender":format!("0x{}","1".repeat(64)),"gasBudgetAtomic":"10000000","gasPriceAtomic":"1000","gasSource":"coinObjects",
        "gasValidDuring":null,"gasPayment":[{"objectId":format!("0x{}","3".repeat(64)),"version":"1","digest":"11111111111111111111111111111111"}],
        "payoutCoin":{"objectId":format!("0x{}","4".repeat(64)),"version":"1","digest":"11111111111111111111111111111111"},"expiresAtEpoch":"999999"})).unwrap()
}
fn draft(s: &PromotionStore, i: &PromotionPayoutIntent, key: &str) -> Result<Value, String> {
    command(
        s,
        &i.campaign_id,
        "ops",
        json!({"action":"createPayoutDraft","settlementId":i.settlement_id,"network":i.network,"coinType":i.coin_type,"evidenceRef":"synthetic-fixture"}),
        key,
        20,
    )
}
fn frozen(j: &PromotionPayoutJournal, i: &PromotionPayoutIntent) {
    j.freeze_intent("ops", i, "synthetic-fixture", false, 21)
        .unwrap();
    j.approve_intent(
        "reviewer",
        &i.payout_id,
        &i.sha256().unwrap(),
        "synthetic-fixture",
        22,
    )
    .unwrap();
}
fn signed(i: &PromotionPayoutIntent, digit: &str) -> PromotionSignedPayout {
    PromotionSignedPayout {
        intent_sha256: i.sha256().unwrap(),
        transaction_bytes: "c3ludGhldGlj".into(),
        digest: digit.repeat(44),
        signatures: vec!["fixture-not-a-chain-signature".into()],
    }
}

#[test]
fn native_postgres_micro_draft_journal_refund_cooldown_and_public_receipt() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIPPED: explicit isolated native PG required");
        return;
    }
    let url = std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL").unwrap();
    let cfg: Config = url.parse().unwrap();
    assert!(cfg.get_dbname().unwrap_or("").contains("promotion"));
    assert!(cfg.get_options().is_none());
    assert!(cfg
        .get_hosts()
        .iter()
        .all(|h| matches!(h,postgres::config::Host::Tcp(n) if n=="127.0.0.1"||n=="localhost")));
    let mut base = Client::connect(&url, NoTls).unwrap();
    let schema = format!(
        "promotion_micro_qa_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let sep = if url.contains('?') { '&' } else { '?' };
    let scoped=format!("{url}{sep}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000");
    let mut client = Client::connect(&scoped, NoTls).unwrap();
    apply_migrations(&mut client).unwrap();
    for a in ["ops", "reviewer", "leader"] {
        client.execute("INSERT INTO accounts(account_id,raw_json) VALUES($1,'{\"characters\":[],\"saves\":{}}')",&[&a]).unwrap();
    }
    for (a, r) in [
        ("ops", "operations"),
        ("ops", "finance"),
        ("ops", "payoutSubmit"),
        ("reviewer", "finance"),
        ("reviewer", "payoutReview"),
    ] {
        client.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,$2,'synthetic-fixture',1)",&[&a,&r]).unwrap();
    }
    let address = format!("0x{}", "2".repeat(64));
    client.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms) VALUES('wallet','leader','sui_wallet',$1,1)",&[&address]).unwrap();
    client.execute("INSERT INTO promotion_payout_addresses(environment,account_id,version,network,wallet_address,verified_credential_id,verified_at_ms,cooldown_until_ms) VALUES('test','leader',1,'sui:testnet',$1,'wallet',1,259200001)",&[&address]).unwrap();
    let s = PromotionStore::new(&scoped, "test").unwrap();
    let j = PromotionPayoutJournal::new(&scoped, "test").unwrap();
    let cold = fixture(&mut client, &s, "cold", false);
    assert_eq!(
        draft(&s, &cold, "cold-draft").unwrap_err(),
        "promotionWalletCooldown"
    );
    let mut i = fixture(&mut client, &s, "paid", true);
    assert_eq!(
        command(
            &s,
            &i.campaign_id,
            "reviewer",
            json!({"action":"createSettlement","leaderId":"leader-paid","evidenceRef":"fixture"}),
            "too-early",
            20
        )
        .unwrap_err(),
        "promotionReconciliationIncomplete"
    );
    let d = draft(&s, &i, "micro-draft").unwrap();
    assert_eq!(d["amountMicroUsd"], "231");
    i.payout_id = d["payoutId"].as_str().unwrap().into();
    assert_eq!(client.query_one("SELECT cooldown_until_ms FROM promotion_payout_addresses WHERE account_id='leader' AND version=1",&[]).unwrap().get::<_,i64>(0),259200001);
    let mut wrong = i.clone();
    wrong.quote_numerator = "2".into();
    wrong.coin_amount_atomic = "462".into();
    assert_eq!(
        j.freeze_intent("ops", &wrong, "fixture", false, 21)
            .unwrap_err(),
        "promotionPayoutQuoteMismatch"
    );
    frozen(&j, &i);
    let signed_tx = signed(&i, "5");
    assert_eq!(
        j.reserve_signed_attempt("ops", &i, &signed_tx, false, 23)
            .unwrap()["maySubmit"],
        true
    );
    j.mark_submitted("ops", &i.payout_id, &signed_tx.digest, 24)
        .unwrap();
    j.record_outcome("ops",&i.payout_id,&json!({"status":"confirmed","digest":signed_tx.digest,"intentSha256":i.sha256().unwrap(),
        "destination":i.destination,"coinType":i.coin_type,"coinAmountAtomic":"231","gasUsedAtomic":"1","checkpoint":"fixture-checkpoint"}),25).unwrap();
    let view = s.settlements(&i.campaign_id, "leader").unwrap();
    assert_eq!(view["items"][0]["payableMicroUsd"], "231");
    assert_eq!(view["items"][0]["payoutPrecision"], 6);
    assert_eq!(view["items"][0]["paidMicroUsd"], "231");
    assert_eq!(view["items"][0]["txDigest"], signed_tx.digest);
    assert_eq!(view["items"][0]["checkpoint"], "fixture-checkpoint");
    assert!(view.to_string().find("transactionBytes").is_none());
    let mut r = fixture(&mut client, &s, "refund", true);
    let d = draft(&s, &r, "refund-draft").unwrap();
    r.payout_id = d["payoutId"].as_str().unwrap().into();
    frozen(&j, &r);
    let signed_refund = signed(&r, "6");
    j.reserve_signed_attempt("ops", &r, &signed_refund, false, 23)
        .unwrap();
    client.execute("INSERT INTO promotion_commission_entries(environment,entry_id,campaign_id,leader_id,account_id,kind,amount_micro_usd,source_adjustment_id,settlement_id,occurred_at_ms) VALUES('test','fixture-reversal',$1,'leader-refund','leader','locked_reversal',-1,'fixture-reversal',$2,24)",&[&r.campaign_id,&r.settlement_id]).unwrap();
    assert_eq!(
        j.mark_submitted("ops", &r.payout_id, &signed_refund.digest, 25)
            .unwrap_err(),
        "promotionPayoutBalanceChanged"
    );
    assert_eq!(
        j.reserve_signed_attempt("ops", &r, &signed_refund, false, 26)
            .unwrap()["maySubmit"],
        false
    );
    // Later wallet versions never inherit the first-address exception.
    client.execute("INSERT INTO promotion_payout_addresses(environment,account_id,version,network,wallet_address,verified_credential_id,verified_at_ms,cooldown_until_ms) VALUES('test','leader',2,'sui:testnet',$1,'wallet',1,259200001)",&[&address]).unwrap();
    let changed = fixture(&mut client, &s, "changed", true);
    assert_eq!(
        draft(&s, &changed, "changed-draft").unwrap_err(),
        "promotionWalletCooldown"
    );
    client
        .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}

//! Isolated fixtures exercise production *code paths*, never certify human
//! acceptance. No live campaign, chain or provider is contacted by this test.
use mir2_simulation::{
    apply_migrations,
    promotion::{
        CampaignDraft, PromotionLaunchManifest, PromotionPayoutIntent, PromotionPayoutJournal,
        PromotionRules, PromotionStore,
    },
};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};

const NOW: u64 = 1_800_000_001_000;
fn command(store: &PromotionStore, who: &str, value: Value, key: &str) -> Result<Value, String> {
    store.admin("launch-fixture", who, value, key, NOW)
}
fn payout(client: &mut Client, name: &str, coin: &str) -> PromotionPayoutIntent {
    let leader = format!("leader-{name}");
    let settlement = format!("settlement-{name}");
    let payout = format!("payout-{name}");
    client.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,status,accepted_terms_version,applied_at_ms) VALUES('production','launch-fixture',$1,$2,$2,1,'fixture','approved','fixture',1)",&[&name,&leader]).unwrap();
    let hash = "a".repeat(64);
    client.execute("INSERT INTO promotion_settlements(environment,campaign_id,settlement_id,leader_id,status,algorithm_version,snapshot_json,snapshot_sha256,total_micro_usd,payable_cents,reserve_micro_usd,created_by,reviewed_by,created_at_ms,reviewed_at_ms) VALUES('production','launch-fixture',$1,$2,'approved','fixture','{}',$3,10000,1,0,'finance1','finance2',1,2)",&[&settlement,&leader,&hash]).unwrap();
    let destination = format!(
        "0x{}",
        (match name {
            "badcoin" => "4",
            "belowminimum" => "5",
            _ => "2",
        })
        .repeat(64)
    );
    let credential = format!("{name}-wallet");
    client.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms) VALUES($1,$2,'sui_wallet',$3,1)",&[&credential,&name,&destination]).unwrap();
    client.execute("INSERT INTO promotion_payout_addresses(environment,account_id,version,network,wallet_address,verified_credential_id,verified_at_ms,cooldown_until_ms) VALUES('production',$1,1,'sui:mainnet',$2,$3,1,1)",&[&name,&destination,&credential]).unwrap();
    client.execute("INSERT INTO promotion_payouts(environment,payout_id,settlement_id,network,coin_type,destination,address_version,amount_micro_usd,created_by,created_at_ms) VALUES('production',$1,$2,'sui:mainnet',$3,$4,1,10000,'payer',1)",&[&payout,&settlement,&coin,&destination]).unwrap();
    let mut value = json!({"version":"numeron-sui-payout-v1","environment":"production","campaignId":"launch-fixture","payoutId":payout,"settlementId":settlement,"settlementSha256":hash,"network":"sui:mainnet","chainIdentifier":"11111111111111111111111111111111","coinType":coin,"coinDecimals":9,"amountMicroUsd":"10000","coinAmountAtomic":"10000000","quoteNumerator":"1000","quoteDenominator":"1","fxSnapshotRef":"synthetic-quote-not-market-price","destination":destination,"addressVersion":"1","sender":format!("0x{}","1".repeat(64)),"gasBudgetAtomic":"10000000","gasPriceAtomic":"1000","gasSource":"coinObjects","gasValidDuring":null,"gasPayment":[{"objectId":format!("0x{}","3".repeat(64)),"version":"1","digest":"11111111111111111111111111111111"}],"payoutCoin":null,"expiresAtEpoch":"999999"});
    if !coin.ends_with("::sui::SUI") {
        value["payoutCoin"] = json!({"objectId":format!("0x{}","4".repeat(64)),"version":"1","digest":"11111111111111111111111111111111"});
    }
    serde_json::from_value(value).unwrap()
}
#[test]
fn isolated_production_launch_approval_and_exact_coin_gate() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIPPED: explicit isolated native PG required");
        return;
    }
    let url = std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL").unwrap();
    let config: Config = url.parse().unwrap();
    assert!(config.get_dbname().unwrap_or("").contains("promotion"));
    assert!(config.get_options().is_none());
    assert!(config
        .get_hosts()
        .iter()
        .all(|h| matches!(h,postgres::config::Host::Tcp(n) if n=="127.0.0.1"||n=="localhost")));
    let mut base = Client::connect(&url, NoTls).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("promotion_launch_fixture_{}_{nonce}", std::process::id());
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let sep = if url.contains('?') { '&' } else { '?' };
    let scoped=format!("{url}{sep}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000");
    let mut client = Client::connect(&scoped, NoTls).unwrap();
    apply_migrations(&mut client).unwrap();
    for account in [
        "operator",
        "finance1",
        "finance2",
        "payer",
        "goodcoin",
        "badcoin",
        "belowminimum",
    ] {
        client.execute("INSERT INTO accounts(account_id,raw_json) VALUES($1,'{\"characters\":[],\"saves\":{}}')",&[&account]).unwrap();
    }
    for (a, r) in [
        ("operator", "operations"),
        ("operator", "finance"),
        ("finance1", "finance"),
        ("finance2", "finance"),
        ("payer", "payoutSubmit"),
    ] {
        client.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('production',$1,$2,'isolated-test-fixture',1)",&[&a,&r]).unwrap();
    }
    let store = PromotionStore::new(scoped.clone(), "production").unwrap();
    let draft = CampaignDraft {
        campaign_id: "launch-fixture".into(),
        funding_mode: "budget_capped".into(),
        terms_version: "fixture".into(),
        starts_at_ms: NOW - 1000,
        ends_at_ms: NOW + 90 * 86_400_000,
        rules: PromotionRules::default(),
        budget_micro_usd: 1_000_000_000,
        member_limit: 10,
        leader_limit: 5,
        per_leader_member_limit: 5,
        per_member_revenue_cap_micro_usd: 30_000_000,
    };
    command(
        &store,
        "operator",
        json!({"action":"configureDraft","draft":draft,"evidenceRef":"synthetic-test-only"}),
        "draft",
    )
    .unwrap();
    command(
        &store,
        "finance1",
        json!({"action":"approveRules","evidenceRef":"synthetic-test-only"}),
        "rules",
    )
    .unwrap();
    for (action, who) in [
        ("setRecruitment", "operator"),
        ("setCommission", "finance1"),
        ("setPayout", "finance1"),
    ] {
        assert!(command(
            &store,
            who,
            json!({"action":action,"enabled":true,"evidenceRef":"synthetic-test-only"}),
            &format!("before-{action}")
        )
        .is_err());
    }
    assert!(client.batch_execute("UPDATE promotion_campaigns SET recruitment_enabled=true,status='active',commission_enabled=true,payout_enabled=true WHERE campaign_id='launch-fixture'").is_err());
    let view = store.admin_view("launch-fixture", "operator", NOW).unwrap();
    let manifest:PromotionLaunchManifest=serde_json::from_value(json!({"configurationSha256":view["launchConfigurationSha256"],"network":"sui:mainnet","chainIdentifier":"11111111111111111111111111111111","coinType":"0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI","coinDecimals":9,"fxSnapshotRef":"synthetic-quote-not-market-price","minimumPayoutMicroUsd":"10000","gasFeePolicy":"treasury","identityTaxPolicyRef":"synthetic-identity-tax-policy-not-human-acceptance","regions":["US"],"termsUrl":"https://fixture.invalid/terms","mobileSignatureEvidence":"synthetic-fixture-not-a-phone-signature","providerPaymentEvidence":"synthetic-fixture-not-a-provider-payment","onchainPayoutDigest":"22222222222222222222222222222222"})).unwrap();
    let mut changed = manifest.clone();
    changed.configuration_sha256 = "b".repeat(64);
    assert_eq!(
        store
            .propose_launch("launch-fixture", "operator", &changed, "fixture", NOW)
            .unwrap_err(),
        "promotionLaunchConfigurationChanged"
    );
    let proposal = store
        .propose_launch(
            "launch-fixture",
            "operator",
            &manifest,
            "fixture-not-human-acceptance",
            NOW,
        )
        .unwrap();
    let sha = proposal["manifestSha256"].as_str().unwrap();
    assert_eq!(
        store
            .approve_launch("launch-fixture", "operator", sha, "fixture", NOW)
            .unwrap_err(),
        "promotionDistinctReviewerRequired"
    );
    assert_eq!(
        store
            .approve_launch(
                "launch-fixture",
                "finance2",
                &"b".repeat(64),
                "fixture",
                NOW
            )
            .unwrap_err(),
        "promotionLaunchConfigurationChanged"
    );
    client.batch_execute("UPDATE promotion_campaigns SET budget_micro_usd=1000000001 WHERE campaign_id='launch-fixture'").unwrap();
    assert_eq!(
        store
            .approve_launch("launch-fixture", "finance2", sha, "fixture", NOW)
            .unwrap_err(),
        "promotionLaunchConfigurationChanged"
    );
    client.batch_execute("UPDATE promotion_campaigns SET budget_micro_usd=1000000000 WHERE campaign_id='launch-fixture'").unwrap();
    store
        .approve_launch(
            "launch-fixture",
            "finance2",
            sha,
            "fixture-not-human-acceptance",
            NOW,
        )
        .unwrap();
    assert!(client
        .batch_execute("UPDATE promotion_production_approvals SET manifest_json='{}'")
        .is_err());
    assert!(client
        .batch_execute("DELETE FROM promotion_production_approvals")
        .is_err());
    assert!(client
        .batch_execute("TRUNCATE promotion_production_approvals")
        .is_err());
    let sui = "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI";
    let good = payout(&mut client, "goodcoin", sui);
    let journal = PromotionPayoutJournal::new(scoped, "production").unwrap();
    assert_eq!(
        journal
            .freeze_intent("payer", &good, "fixture", true, NOW)
            .unwrap_err(),
        "promotionPayoutExecutionDisabled"
    );
    for (action, who) in [
        ("setRecruitment", "operator"),
        ("setCommission", "finance1"),
        ("setPayout", "finance1"),
    ] {
        command(
            &store,
            who,
            json!({"action":action,"enabled":true,"evidenceRef":"synthetic-test-only"}),
            &format!("after-{action}"),
        )
        .unwrap();
    }
    let public = store.public_campaign("launch-fixture").unwrap();
    for key in ["recruitmentEnabled", "commissionEnabled", "payoutEnabled"] {
        assert_eq!(public[key], true);
    }
    assert_eq!(
        journal
            .freeze_intent("payer", &good, "fixture", false, NOW)
            .unwrap_err(),
        "promotionPayoutMainnetApprovalRequired"
    );
    journal
        .freeze_intent("payer", &good, "synthetic-test-only", true, NOW)
        .unwrap();
    let bad = payout(
        &mut client,
        "badcoin",
        "0x0000000000000000000000000000000000000000000000000000000000000009::fake::FAKE",
    );
    assert_eq!(
        journal
            .freeze_intent("payer", &bad, "fixture", true, NOW)
            .unwrap_err(),
        "promotionPayoutManifestMismatch"
    );
    let mut below = payout(&mut client, "belowminimum", sui);
    below.amount_micro_usd = "9999".into();
    below.coin_amount_atomic = "9999000".into();
    client
        .execute(
            "UPDATE promotion_payouts SET amount_micro_usd=9999 WHERE payout_id=$1",
            &[&below.payout_id],
        )
        .unwrap();
    assert_eq!(
        journal
            .freeze_intent("payer", &below, "fixture", true, NOW)
            .unwrap_err(),
        "promotionPayoutBelowMinimum"
    );
    if let Ok(dir) = std::env::var("MIR2_PROMOTION_TEST_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("launch-postgres-result.json"),serde_json::to_vec_pretty(&json!({"passed":true,"schema":schema,"syntheticFixturesOnly":true,"humanProductionAcceptance":false,"phoneSignatureAccepted":false,"chainPaymentAccepted":false,"checks":["unapprovedSwitchesRejected","directSqlGateRejected","independentReviewRequired","configurationAndManifestHashesBound","manifestAppendOnly","reviewedFixtureSwitchesEnabled","payoutFlagAndDeploymentApprovalRequired","unapprovedCoinRejected"]})).unwrap()).unwrap();
    }
}

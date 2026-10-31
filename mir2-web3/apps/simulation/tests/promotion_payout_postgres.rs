//! Real isolated PostgreSQL journal tests; transaction bytes here are synthetic.
//! Cryptographic transaction validation is tested by test-promotion-payout.mjs.
//! This test never calls a chain, faucet, payment service or Docker.
use mir2_simulation::{
    apply_migrations,
    promotion::{
        PromotionLaunchManifest, PromotionPayoutIntent, PromotionPayoutJournal,
        PromotionSignedPayout, PromotionStore,
    },
};
use postgres::{Client, Config, NoTls};
use serde_json::json;
use std::sync::{Arc, Barrier};

fn fixture(client: &mut Client, store: &PromotionStore, name: &str) -> PromotionPayoutIntent {
    let campaign = format!("campaign-{name}");
    let leader = format!("leader-{name}");
    let settlement = format!("settlement-{name}");
    let payout = format!("payout-{name}");
    client.execute("INSERT INTO promotion_campaigns(environment,campaign_id,terms_version,starts_at_ms,ends_at_ms,rules_json,rules_approved,configured_by,approved_by,budget_micro_usd,member_limit,leader_limit,per_leader_member_limit,per_member_revenue_cap_micro_usd,created_at_ms) VALUES('test',$1,'fixture',1,2,'{}',true,'payer','reviewer',1000000000,10,5,5,30000000,1)",&[&campaign]).unwrap();
    client.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,status,accepted_terms_version,applied_at_ms) VALUES('test',$1,'leader',$2,$2,1,'test','approved','fixture',1)",&[&campaign,&leader]).unwrap();
    let hash = "a".repeat(64);
    client.execute("INSERT INTO promotion_settlements(environment,campaign_id,settlement_id,leader_id,status,algorithm_version,snapshot_json,snapshot_sha256,total_micro_usd,payable_cents,reserve_micro_usd,created_by,reviewed_by,created_at_ms,reviewed_at_ms) VALUES('test',$1,$2,$3,'approved','fixture','{}',$4,10000,1,0,'finance1','finance2',1,2)",&[&campaign,&settlement,&leader,&hash]).unwrap();
    let latest=client.query_one("SELECT version,wallet_address FROM promotion_payout_addresses WHERE environment='test' AND account_id='leader' ORDER BY version DESC LIMIT 1",&[]).unwrap();
    let address_version: i64 = latest.get(0);
    let destination: String = latest.get(1);
    let coin = "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI";
    let view = store.admin_view(&campaign, "payer", 10).unwrap();
    let manifest:PromotionLaunchManifest=serde_json::from_value(json!({"configurationSha256":view["launchConfigurationSha256"],"network":"sui:testnet","chainIdentifier":"69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD","coinType":coin,"coinDecimals":9,"fxSnapshotRef":"fixture-not-market-price","minimumPayoutMicroUsd":"1","gasFeePolicy":"treasury","identityTaxPolicyRef":"synthetic-config-policy-no-human-acceptance","regions":["US"],"termsUrl":"https://fixture.invalid/terms","mobileSignatureEvidence":"","providerPaymentEvidence":"","onchainPayoutDigest":""})).unwrap();
    let proposal = store
        .propose_launch(
            &campaign,
            "payer",
            &manifest,
            "synthetic-testnet-config-not-chain-acceptance",
            10,
        )
        .unwrap();
    store
        .approve_launch(
            &campaign,
            "reviewer",
            proposal["manifestSha256"].as_str().unwrap(),
            "synthetic-testnet-config-review",
            11,
        )
        .unwrap();
    store
        .admin(
            &campaign,
            "reviewer",
            json!({"action":"setPayout","enabled":true,"evidenceRef":"synthetic-testnet-config"}),
            &format!("payout-enable-{name}"),
            12,
        )
        .unwrap();
    client.execute("INSERT INTO promotion_payouts(environment,payout_id,settlement_id,network,coin_type,destination,address_version,amount_micro_usd,created_by,created_at_ms) VALUES('test',$1,$2,'sui:testnet',$3,$4,$5,10000,'payer',1)",&[&payout,&settlement,&coin,&destination,&address_version]).unwrap();
    serde_json::from_value(json!({"version":"numeron-sui-payout-v1","environment":"test","campaignId":campaign,"payoutId":payout,"settlementId":settlement,"settlementSha256":hash,"network":"sui:testnet","chainIdentifier":"69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD","coinType":coin,"coinDecimals":9,"amountMicroUsd":"10000","coinAmountAtomic":"10000000","quoteNumerator":"1000","quoteDenominator":"1","fxSnapshotRef":"fixture-not-market-price","destination":destination,"addressVersion":address_version.to_string(),"sender":format!("0x{}","1".repeat(64)),"gasBudgetAtomic":"10000000","gasPriceAtomic":"1000","gasSource":"coinObjects","gasValidDuring":null,"gasPayment":[{"objectId":format!("0x{}","3".repeat(64)),"version":"1","digest":"11111111111111111111111111111111"}],"payoutCoin":null,"expiresAtEpoch":"999999"})).unwrap()
}
fn signed(intent: &PromotionPayoutIntent, digit: &str) -> PromotionSignedPayout {
    PromotionSignedPayout {
        intent_sha256: intent.sha256().unwrap(),
        transaction_bytes: "dGVzdA==".into(),
        digest: digit.repeat(44),
        signatures: vec!["fixture-worker-verified-by-separate-sdk-tests".into()],
    }
}
fn frozen(journal: &PromotionPayoutJournal, intent: &PromotionPayoutIntent) {
    journal
        .freeze_intent("payer", intent, "fixture", false, 10)
        .unwrap();
    journal
        .approve_intent(
            "reviewer",
            &intent.payout_id,
            &intent.sha256().unwrap(),
            "fixture",
            11,
        )
        .unwrap();
}
fn wait_for_blocked_connection(tx: &mut postgres::Transaction<'_>) {
    let pid: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    for _ in 0..50 {
        let count: i64 = tx
            .query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid))",
                &[&pid],
            )
            .unwrap()
            .get(0);
        if count > 0 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("expected second connection waiting on payee advisory lock");
}
#[test]
fn isolated_native_postgres_durable_payout_journal() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIPPED: explicit isolated promotion PG required");
        return;
    }
    let url = std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL").expect("isolated local URI");
    let config: Config = url.parse().unwrap();
    assert!(config.get_dbname().unwrap_or("").contains("promotion"));
    assert!(config
        .get_hosts()
        .iter()
        .all(|h| matches!(h,postgres::config::Host::Tcp(n) if n=="127.0.0.1"||n=="localhost")));
    assert!(config.get_options().is_none());
    let mut base = Client::connect(&url, NoTls).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("promotion_payout_qa_{}_{nonce}", std::process::id());
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let sep = if url.contains('?') { '&' } else { '?' };
    let scoped=format!("{url}{sep}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000");
    let mut client = Client::connect(&scoped, NoTls).unwrap();
    apply_migrations(&mut client).unwrap();
    apply_migrations(&mut client).unwrap();
    for account in [
        "payer", "reviewer", "leader", "intruder", "worker", "member",
    ] {
        client.execute("INSERT INTO accounts(account_id,raw_json) VALUES($1,'{\"characters\":[],\"saves\":{}}')",&[&account]).unwrap();
    }
    for (a, r) in [
        ("payer", "payoutSubmit"),
        ("payer", "operations"),
        ("worker", "payoutSubmit"),
        ("payer", "payoutReview"),
        ("reviewer", "payoutReview"),
        ("reviewer", "finance"),
    ] {
        client.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,$2,'fixture',1)",&[&a,&r]).unwrap();
    }
    let destination = format!("0x{}", "2".repeat(64));
    client.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms) VALUES('fixture','leader','sui_wallet',$1,1)",&[&destination]).unwrap();
    client.execute("INSERT INTO promotion_payout_addresses(environment,account_id,version,network,wallet_address,verified_credential_id,verified_at_ms,cooldown_until_ms) VALUES('test','leader',1,'sui:testnet',$1,'fixture',1,1)",&[&destination]).unwrap();
    let store = PromotionStore::new(scoped.clone(), "test").unwrap();
    let journal = PromotionPayoutJournal::new(scoped.clone(), "test").unwrap();
    let intent = fixture(&mut client, &store, "normal");
    client
        .execute(
            "UPDATE promotion_campaigns SET payout_enabled=false WHERE campaign_id=$1",
            &[&intent.campaign_id],
        )
        .unwrap();
    assert_eq!(
        journal
            .freeze_intent("payer", &intent, "fixture", false, 10)
            .unwrap_err(),
        "promotionPayoutExecutionDisabled"
    );
    client
        .execute(
            "UPDATE promotion_campaigns SET payout_enabled=true WHERE campaign_id=$1",
            &[&intent.campaign_id],
        )
        .unwrap();
    assert!(journal
        .freeze_intent("intruder", &intent, "fixture", false, 10)
        .is_err());
    client
        .batch_execute(
            "UPDATE identity_credentials SET revoked_at_ms=2 WHERE credential_id='fixture'",
        )
        .unwrap();
    assert_eq!(
        journal
            .freeze_intent("payer", &intent, "fixture", false, 10)
            .unwrap_err(),
        "promotionWalletProofRequired"
    );
    client
        .batch_execute(
            "UPDATE identity_credentials SET revoked_at_ms=NULL WHERE credential_id='fixture'",
        )
        .unwrap();
    journal
        .freeze_intent("payer", &intent, "fixture", false, 10)
        .unwrap();
    assert_eq!(
        journal
            .approve_intent(
                "payer",
                &intent.payout_id,
                &intent.sha256().unwrap(),
                "fixture",
                11
            )
            .unwrap_err(),
        "promotionDistinctReviewerRequired"
    );
    assert_eq!(
        journal
            .approve_intent(
                "reviewer",
                &intent.payout_id,
                &"b".repeat(64),
                "fixture",
                11
            )
            .unwrap_err(),
        "promotionPayoutIntentChanged"
    );
    let signed_tx = signed(&intent, "2");
    assert!(journal
        .reserve_signed_attempt("payer", &intent, &signed_tx, false, 12)
        .is_err());
    journal
        .approve_intent(
            "reviewer",
            &intent.payout_id,
            &intent.sha256().unwrap(),
            "fixture",
            11,
        )
        .unwrap();
    assert!(client
        .execute(
            "UPDATE promotion_payout_intents SET intent_json='{}' WHERE payout_id=$1",
            &[&intent.payout_id]
        )
        .is_err());
    assert!(client
        .execute(
            "UPDATE promotion_payouts SET destination=$2 WHERE payout_id=$1",
            &[&intent.payout_id, &format!("0x{}", "4".repeat(64))]
        )
        .is_err());
    let barrier = Arc::new(Barrier::new(8));
    let joins: Vec<_> = (0..8)
        .map(|_| {
            let j = journal.clone();
            let i = intent.clone();
            let s = signed_tx.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                j.reserve_signed_attempt("worker", &i, &s, false, 12)
                    .unwrap()
            })
        })
        .collect();
    let results: Vec<_> = joins.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|v| v["maySubmit"] == true).count(), 1);
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM promotion_payout_attempts", &[])
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert!(client
        .execute(
            "UPDATE promotion_payout_attempts SET transaction_bytes_base64='other'",
            &[]
        )
        .is_err());
    client
        .batch_execute(
            "UPDATE identity_credentials SET revoked_at_ms=2 WHERE credential_id='fixture'",
        )
        .unwrap();
    assert_eq!(
        journal
            .mark_submitted("worker", &intent.payout_id, &signed_tx.digest, 13)
            .unwrap_err(),
        "promotionWalletProofRequired"
    );
    client
        .batch_execute(
            "UPDATE identity_credentials SET revoked_at_ms=NULL WHERE credential_id='fixture'",
        )
        .unwrap();
    journal
        .mark_submitted("worker", &intent.payout_id, &signed_tx.digest, 13)
        .unwrap();
    assert!(journal
        .mark_submitted("worker", &intent.payout_id, &signed_tx.digest, 13)
        .is_err());
    journal.record_outcome("payer",&intent.payout_id,&json!({"digest":signed_tx.digest,"status":"unknown","code":"promotionPayoutSubmissionUnknown"}),14).unwrap();
    client
        .execute(
            "UPDATE promotion_campaigns SET payout_enabled=false WHERE campaign_id=$1",
            &[&intent.campaign_id],
        )
        .unwrap();
    client
        .batch_execute(
            "UPDATE identity_credentials SET revoked_at_ms=2 WHERE credential_id='fixture'",
        )
        .unwrap();
    let replay = journal
        .reserve_signed_attempt("payer", &intent, &signed_tx, false, 15)
        .unwrap();
    assert_eq!(replay["maySubmit"], false);
    assert_eq!(replay["status"], "unknown");
    let mut changed = signed_tx.clone();
    changed.digest = "3".repeat(44);
    assert_eq!(
        journal
            .reserve_signed_attempt("payer", &intent, &changed, false, 15)
            .unwrap_err(),
        "promotionPayoutAttemptConflict"
    );
    assert!(journal
        .record_outcome(
            "payer",
            &intent.payout_id,
            &json!({"digest":signed_tx.digest,"status":"confirmed"}),
            16
        )
        .is_err());
    let outcome = json!({"digest":signed_tx.digest,"status":"confirmed","intentSha256":signed_tx.intent_sha256,"destination":intent.destination,"coinType":intent.coin_type,"coinAmountAtomic":intent.coin_amount_atomic,"gasUsedAtomic":"250"});
    journal
        .record_outcome("payer", &intent.payout_id, &outcome, 17)
        .unwrap();
    assert_eq!(
        journal
            .record_outcome(
                "payer",
                &intent.payout_id,
                &json!({"digest":signed_tx.digest,"status":"unknown"}),
                18
            )
            .unwrap()["status"],
        "confirmed"
    );
    assert!(client
        .execute(
            "UPDATE promotion_payouts SET status='unknown' WHERE payout_id=$1",
            &[&intent.payout_id]
        )
        .is_err());
    assert!(client
        .execute("DELETE FROM promotion_payout_chain_observations", &[])
        .is_err());
    assert!(client
        .batch_execute("TRUNCATE promotion_payout_chain_observations")
        .is_err());
    client
        .batch_execute(
            "UPDATE identity_credentials SET revoked_at_ms=NULL WHERE credential_id='fixture'",
        )
        .unwrap();
    let refund = fixture(&mut client, &store, "refund");
    frozen(&journal, &refund);
    client.execute("INSERT INTO promotion_commission_entries(environment,entry_id,campaign_id,leader_id,account_id,kind,amount_micro_usd,source_adjustment_id,settlement_id,occurred_at_ms) VALUES('test','refund-entry',$1,'leader-refund','leader','locked_reversal',-1000,'fixture-refund',$2,20)",&[&refund.campaign_id,&refund.settlement_id]).unwrap();
    assert_eq!(
        journal
            .reserve_signed_attempt("payer", &refund, &signed(&refund, "4"), false, 21)
            .unwrap_err(),
        "promotionPayoutBalanceChanged"
    );
    let held = fixture(&mut client, &store, "held");
    client.execute("INSERT INTO promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id,first_clicked_at_ms,expires_at_ms,source) VALUES('test',$1,'held-candidate','member','leader-held',1,2,'manual_code')",&[&held.campaign_id]).unwrap();
    client.execute("INSERT INTO promotion_attributions(environment,campaign_id,account_id,leader_id,candidate_id,first_clicked_at_ms,registered_at_ms,confirmed_at_ms,terms_version,source,reserved_micro_usd) VALUES('test',$1,'member','leader-held','held-candidate',1,1,1,'fixture','manual_code',0)",&[&held.campaign_id]).unwrap();
    client.execute("INSERT INTO promotion_revenue_source_holds(environment,campaign_id,account_id,subscription_id,active,reason_code,source_payload_sha256,updated_at_ms) VALUES('test',$1,'member','held-subscription',true,'fixture',$2,1)",&[&held.campaign_id,&"a".repeat(64)]).unwrap();
    assert_eq!(
        journal
            .freeze_intent("payer", &held, "fixture", false, 21)
            .unwrap_err(),
        "promotionPayoutRiskReviewRequired"
    );
    client
        .execute(
            "UPDATE promotion_revenue_source_holds SET active=false WHERE campaign_id=$1",
            &[&held.campaign_id],
        )
        .unwrap();
    journal
        .freeze_intent("payer", &held, "fixture", false, 21)
        .unwrap();
    client
        .execute(
            "UPDATE promotion_revenue_source_holds SET active=true WHERE campaign_id=$1",
            &[&held.campaign_id],
        )
        .unwrap();
    assert_eq!(
        journal
            .approve_intent(
                "reviewer",
                &held.payout_id,
                &held.sha256().unwrap(),
                "fixture",
                21
            )
            .unwrap_err(),
        "promotionPayoutRiskReviewRequired"
    );
    client
        .execute(
            "UPDATE promotion_revenue_source_holds SET active=false WHERE campaign_id=$1",
            &[&held.campaign_id],
        )
        .unwrap();
    journal
        .approve_intent(
            "reviewer",
            &held.payout_id,
            &held.sha256().unwrap(),
            "fixture",
            21,
        )
        .unwrap();
    client
        .execute(
            "UPDATE promotion_revenue_source_holds SET active=true WHERE campaign_id=$1",
            &[&held.campaign_id],
        )
        .unwrap();
    assert_eq!(
        journal
            .reserve_signed_attempt("worker", &held, &signed(&held, "6"), false, 22)
            .unwrap_err(),
        "promotionPayoutRiskReviewRequired"
    );
    client
        .execute(
            "UPDATE promotion_revenue_source_holds SET active=false WHERE campaign_id=$1",
            &[&held.campaign_id],
        )
        .unwrap();
    journal
        .reserve_signed_attempt("worker", &held, &signed(&held, "6"), false, 22)
        .unwrap();
    client
        .execute(
            "UPDATE promotion_revenue_source_holds SET active=true WHERE campaign_id=$1",
            &[&held.campaign_id],
        )
        .unwrap();
    assert_eq!(
        journal
            .mark_submitted("worker", &held.payout_id, &signed(&held, "6").digest, 23)
            .unwrap_err(),
        "promotionPayoutRiskReviewRequired"
    );
    let risk = fixture(&mut client, &store, "risk");
    frozen(&journal, &risk);
    client
        .execute(
            "UPDATE promotion_leaders SET status='suspended' WHERE campaign_id=$1",
            &[&risk.campaign_id],
        )
        .unwrap();
    assert_eq!(
        journal
            .reserve_signed_attempt("worker", &risk, &signed(&risk, "7"), false, 23)
            .unwrap_err(),
        "promotionPayoutRiskReviewRequired"
    );
    let late = fixture(&mut client, &store, "late-refund");
    frozen(&journal, &late);
    journal
        .reserve_signed_attempt("worker", &late, &signed(&late, "8"), false, 24)
        .unwrap();
    client.execute("INSERT INTO promotion_commission_entries(environment,entry_id,campaign_id,leader_id,account_id,kind,amount_micro_usd,source_adjustment_id,settlement_id,occurred_at_ms) VALUES('test','late-refund-entry',$1,'leader-late-refund','leader','locked_reversal',-1000,'fixture-late-refund',$2,25)",&[&late.campaign_id,&late.settlement_id]).unwrap();
    assert_eq!(
        journal
            .mark_submitted("worker", &late.payout_id, &signed(&late, "8").digest, 26)
            .unwrap_err(),
        "promotionPayoutBalanceChanged"
    );
    let member_ban = fixture(&mut client, &store, "member-ban");
    client.execute("INSERT INTO promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id,first_clicked_at_ms,expires_at_ms,source) VALUES('test',$1,'ban-candidate','member','leader-member-ban',1,2,'manual_code')",&[&member_ban.campaign_id]).unwrap();
    client.execute("INSERT INTO promotion_attributions(environment,campaign_id,account_id,leader_id,candidate_id,first_clicked_at_ms,registered_at_ms,confirmed_at_ms,terms_version,source,reserved_micro_usd) VALUES('test',$1,'member','leader-member-ban','ban-candidate',1,1,1,'fixture','manual_code',0)",&[&member_ban.campaign_id]).unwrap();
    frozen(&journal, &member_ban);
    client
        .batch_execute("UPDATE accounts SET is_banned=true WHERE account_id='member'")
        .unwrap();
    assert_eq!(
        journal
            .reserve_signed_attempt("worker", &member_ban, &signed(&member_ban, "9"), false, 26)
            .unwrap_err(),
        "promotionRiskReviewRequired"
    );
    client
        .batch_execute("UPDATE accounts SET is_banned=false WHERE account_id='member'")
        .unwrap();
    journal
        .reserve_signed_attempt("worker", &member_ban, &signed(&member_ban, "9"), false, 26)
        .unwrap();
    client
        .batch_execute("UPDATE accounts SET is_banned=true WHERE account_id='member'")
        .unwrap();
    assert_eq!(
        journal
            .mark_submitted(
                "worker",
                &member_ban.payout_id,
                &signed(&member_ban, "9").digest,
                27
            )
            .unwrap_err(),
        "promotionRiskReviewRequired"
    );
    client
        .batch_execute("UPDATE accounts SET is_banned=false WHERE account_id='member'")
        .unwrap();
    let addr = fixture(&mut client, &store, "address");
    frozen(&journal, &addr);
    client.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms) VALUES('fixture2','leader','sui_wallet',$1,1)",&[&format!("0x{}","5".repeat(64))]).unwrap();
    client.execute("INSERT INTO promotion_payout_addresses(environment,account_id,version,network,wallet_address,verified_credential_id,verified_at_ms,cooldown_until_ms) VALUES('test','leader',2,'sui:testnet',$1,'fixture2',20,20)",&[&format!("0x{}","5".repeat(64))]).unwrap();
    assert_eq!(
        journal
            .reserve_signed_attempt("payer", &addr, &signed(&addr, "5"), false, 21)
            .unwrap_err(),
        "promotionPayoutAddressChanged"
    );
    // Address transaction wins the shared payee lock. Reservation must wait,
    // then observe the changed address/cancelled draft instead of caching it.
    let race = fixture(&mut client, &store, "race-address-first");
    frozen(&journal, &race);
    let mut tx = client.transaction().unwrap();
    let payee_key = "promotion-payee:test:leader";
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        &[&payee_key],
    )
    .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let jb = journal.clone();
    let ib = race.clone();
    let bb = barrier.clone();
    let waiter = std::thread::spawn(move || {
        bb.wait();
        jb.reserve_signed_attempt("worker", &ib, &signed(&ib, "A"), false, 30)
    });
    barrier.wait();
    wait_for_blocked_connection(&mut tx);
    assert!(!waiter.is_finished());
    let next = format!("0x{}", "6".repeat(64));
    tx.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms) VALUES('fixture3','leader','sui_wallet',$1,1)",&[&next]).unwrap();
    tx.execute("INSERT INTO promotion_payout_addresses(environment,account_id,version,network,wallet_address,verified_credential_id,verified_at_ms,cooldown_until_ms) VALUES('test','leader',3,'sui:testnet',$1,'fixture3',30,30)",&[&next]).unwrap();
    tx.execute(
        "UPDATE promotion_payouts SET status='cancelled' WHERE payout_id=$1",
        &[&race.payout_id],
    )
    .unwrap();
    tx.commit().unwrap();
    let race_error = waiter.join().unwrap().unwrap_err();
    assert!(matches!(
        race_error.as_str(),
        "promotionPayoutStateInvalid" | "promotionPayoutAddressChanged"
    ));
    // With an already reserved/unknown attempt, the address side must reject;
    // the sender waits for the same lock, then may use only the original bytes.
    let race_send = fixture(&mut client, &store, "race-send");
    frozen(&journal, &race_send);
    journal
        .reserve_signed_attempt("worker", &race_send, &signed(&race_send, "B"), false, 31)
        .unwrap();
    let mut tx = client.transaction().unwrap();
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        &[&payee_key],
    )
    .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let jb = journal.clone();
    let ib = race_send.clone();
    let bb = barrier.clone();
    let waiter = std::thread::spawn(move || {
        bb.wait();
        jb.mark_submitted("worker", &ib.payout_id, &signed(&ib, "B").digest, 32)
    });
    barrier.wait();
    wait_for_blocked_connection(&mut tx);
    assert!(!waiter.is_finished());
    assert_eq!(
        tx.query_one(
            "SELECT status FROM promotion_payouts WHERE payout_id=$1",
            &[&race_send.payout_id]
        )
        .unwrap()
        .get::<_, String>(0),
        "unknown"
    );
    tx.rollback().unwrap();
    waiter.join().unwrap().unwrap();
    let snapshot_ban = fixture(&mut client, &store, "snapshot-ban");
    client.execute("UPDATE promotion_settlements SET snapshot_json='{\"input\":{\"members\":[{\"accountId\":\"intruder\"}]}}' WHERE settlement_id=$1",&[&snapshot_ban.settlement_id]).unwrap();
    frozen(&journal, &snapshot_ban);
    client
        .batch_execute("UPDATE accounts SET is_banned=true WHERE account_id='intruder'")
        .unwrap();
    assert_eq!(
        journal
            .reserve_signed_attempt(
                "worker",
                &snapshot_ban,
                &signed(&snapshot_ban, "C"),
                false,
                33
            )
            .unwrap_err(),
        "promotionRiskReviewRequired"
    );
    let concurrent_ban = fixture(&mut client, &store, "concurrent-ban");
    frozen(&journal, &concurrent_ban);
    let mut tx = client.transaction().unwrap();
    tx.execute(
        "UPDATE accounts SET is_banned=true WHERE account_id='leader'",
        &[],
    )
    .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let jb = journal.clone();
    let ib = concurrent_ban.clone();
    let bb = barrier.clone();
    let waiter = std::thread::spawn(move || {
        bb.wait();
        jb.reserve_signed_attempt("worker", &ib, &signed(&ib, "D"), false, 34)
    });
    barrier.wait();
    wait_for_blocked_connection(&mut tx);
    assert!(!waiter.is_finished());
    tx.commit().unwrap();
    assert_eq!(
        waiter.join().unwrap().unwrap_err(),
        "promotionRiskReviewRequired"
    );
    client
        .batch_execute("UPDATE accounts SET is_banned=false WHERE account_id='leader'")
        .unwrap();
    if let Ok(dir) = std::env::var("MIR2_PROMOTION_TEST_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("payout-postgres-result.json"),serde_json::to_vec_pretty(&json!({"passed":true,"schema":schema,"attempts":5,"concurrentReservations":8,"firstSendClaims":1,"phoneSignatureAccepted":false,"chainPaymentAccepted":false,"syntheticTransactionBytes":true,"checks":["independentRoleAndShaApproval","immutableIntentAndSignedBytes","reserveBeforeSubmit","concurrentSingleClaim","unknownDigestReplay","finalStateNoRegression","appendOnlyObservations","refundRecheck","destinationVersionRecheck","activeMemberHoldAtFreezeReviewReserveAndSubmit","suspendedLeaderRejected","independentServiceWorkerRole","refundBetweenReservationAndSendRejected","allEnvironmentPayoutSwitch","emptyEvidenceTestnetConfigIsNotAcceptance","revokedCredentialBlocksNewSendButAllowsQuery","attributedAndSnapshotMemberBan","twoConnectionPayeeLockAddressFirst","twoConnectionPayeeLockUnknownAddressRejected","twoConnectionAccountBanBeforeSend"]})).unwrap()).unwrap();
    }
}

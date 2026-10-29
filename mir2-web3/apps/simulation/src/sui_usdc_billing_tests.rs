use super::*;
use crate::billing::{RechargeOrderState, VerifiedRechargePayment};
use crate::{AccountStoreRepository, AccountStoreTransactionFault, FileAccountStoreRepository};

const NOW: u64 = 1_900_000_000_000;
fn addr(c: char) -> String {
    format!("0x{}", c.to_string().repeat(64))
}
fn cap() -> String {
    "c".repeat(64)
}
fn offer() -> RechargeOffer {
    RechargeOffer {
        id: "existing-usd-10".into(),
        label: "Existing server offer".into(),
        currency: "usd".into(),
        amount_minor: 1000,
        credits: 1000,
    }
}
fn domain() -> UsdcDomain {
    UsdcDomain {
        chain_identifier: TESTNET_CHAIN_IDENTIFIER.into(),
        package_id: addr('1'),
        config_id: addr('2'),
        merchant: addr('3'),
        coin_type: TESTNET_USDC_COIN_TYPE.into(),
        rule_version: 1,
    }
}
fn prepared(config: &SimulationConfig) -> UsdcRechargeOrder {
    config
        .prepare_usdc_recharge_order(
            "demo",
            config.default_character.index,
            "request-one",
            &offer(),
            &domain(),
            &cap(),
            NOW + 1_800_000,
            NOW + 1_200_000,
            NOW,
        )
        .unwrap()
}
fn invoice(config: &SimulationConfig) -> UsdcRechargeOrder {
    let o = prepared(config);
    config
        .bind_usdc_recharge_payer("demo", &o.id, &addr('4'), &cap(), NOW + 1)
        .unwrap();
    config
        .claim_usdc_recharge_job("demo", &o.id, "worker-one", NOW + 2)
        .unwrap()
        .unwrap();
    config
        .record_usdc_prepared_transaction(
            "demo",
            &o.id,
            "worker-one",
            &UsdcPreparedTransaction {
                bytes_base64: "TUFDSw==".into(),
                signature_base64: "c2ln".into(),
                digest: "1".repeat(44),
            },
            NOW + 3,
        )
        .unwrap();
    config
        .bind_usdc_recharge_invoice(
            "demo",
            &o.id,
            "worker-one",
            &UsdcInvoiceBinding {
                invoice_id: addr('5'),
                create_digest: "1".repeat(44),
                checkpoint: 123,
            },
            NOW + 4,
        )
        .unwrap()
}
fn payment(o: &UsdcRechargeOrder) -> VerifiedUsdcReceipt {
    VerifiedUsdcReceipt {
        chain_identifier: o.domain.chain_identifier.clone(),
        package_id: o.domain.package_id.clone(),
        config_id: o.domain.config_id.clone(),
        invoice_id: o.invoice.as_ref().unwrap().invoice_id.clone(),
        order_id: o.id.clone(),
        account_ref: o.account_ref.clone(),
        payer: o.payer.clone().unwrap(),
        merchant: o.domain.merchant.clone(),
        coin_type: o.domain.coin_type.clone(),
        amount_micro: o.amount_micro,
        frozen_hash: o.frozen_hash().unwrap(),
        digest: "2".repeat(44),
        event_seq: 0,
        checkpoint: 124,
        confirmed_at_ms: NOW + 10,
    }
}

#[test]
fn usdc_existing_usd_offer_has_exact_six_decimal_price_and_test_provenance() {
    let c = SimulationConfig::default();
    let o = prepared(&c);
    assert_eq!(o.amount_micro, 10_000_000);
    assert_eq!(o.offer, offer());
    assert_eq!(o.credit_origin, UsdcCreditOrigin::Test);
    let replay = c
        .prepare_usdc_recharge_order(
            "demo",
            c.default_character.index,
            "request-one",
            &offer(),
            &domain(),
            &cap(),
            NOW + 1_800_100,
            NOW + 1_200_100,
            NOW + 100,
        )
        .unwrap();
    assert_eq!(o, replay);
    let mut changed = offer();
    changed.credits += 1;
    assert!(c
        .prepare_usdc_recharge_order(
            "demo",
            c.default_character.index,
            "request-one",
            &changed,
            &domain(),
            &cap(),
            NOW + 1000,
            NOW + 1000,
            NOW + 100
        )
        .is_err());
}
#[test]
fn usdc_rejects_mainnet_wrapped_coin_and_price_overflow() {
    let c = SimulationConfig::default();
    for change in 0..4 {
        let mut d = domain();
        let mut f = offer();
        match change {
            0 => d.chain_identifier = "mainnet".into(),
            1 => d.coin_type = "0x2::sui::SUI".into(),
            2 => f.currency = "eur".into(),
            _ => f.amount_minor = u64::MAX,
        }
        assert!(c
            .prepare_usdc_recharge_order(
                "demo",
                c.default_character.index,
                "rejected",
                &f,
                &d,
                &cap(),
                NOW + 1000,
                NOW + 1000,
                NOW
            )
            .is_err());
    }
    assert!(c
        .usdc_recharge_orders("demo", c.default_character.index)
        .unwrap()
        .is_empty());
}
#[test]
fn usdc_capability_is_scoped_expiring_and_payer_is_immutable() {
    let c = SimulationConfig::default();
    let o = prepared(&c);
    assert!(c
        .bind_usdc_recharge_payer("demo", &o.id, &addr('4'), &"d".repeat(64), NOW + 1)
        .is_err());
    assert!(c
        .bind_usdc_recharge_payer(
            "demo",
            &o.id,
            &addr('4'),
            &cap(),
            o.capability_expires_at_ms
        )
        .is_err());
    c.bind_usdc_recharge_payer("demo", &o.id, &addr('4'), &cap(), NOW + 1)
        .unwrap();
    assert!(c
        .bind_usdc_recharge_payer("demo", &o.id, &addr('6'), &cap(), NOW + 2)
        .is_err());
    assert!(c
        .bind_usdc_recharge_payer("demo", &o.id, &addr('4'), &cap(), NOW + 2)
        .is_ok());
}
#[test]
fn usdc_lease_takeover_keeps_exact_prepared_bytes_and_rejects_old_worker() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    assert!(c
        .claim_usdc_recharge_job("demo", &o.id, "worker-two", NOW + 10)
        .unwrap()
        .is_none());
    let takeover = c
        .claim_usdc_recharge_job("demo", &o.id, "worker-two", NOW + JOB_LEASE_MS + 3)
        .unwrap()
        .unwrap();
    assert_eq!(takeover.prepared_tx, o.prepared_tx);
    assert!(c
        .record_usdc_prepared_transaction(
            "demo",
            &o.id,
            "worker-one",
            o.prepared_tx.as_ref().unwrap(),
            NOW + JOB_LEASE_MS + 4
        )
        .is_err());
    let mut wrong = o.prepared_tx.clone().unwrap();
    wrong.digest = "3".repeat(44);
    assert!(c
        .record_usdc_prepared_transaction(
            "demo",
            &o.id,
            "worker-two",
            &wrong,
            NOW + JOB_LEASE_MS + 4
        )
        .is_err());
}
#[test]
fn usdc_invoice_requires_persisted_creation_digest_and_final_checkpoint() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    let mut bad = o.invoice.clone().unwrap();
    bad.create_digest = "4".repeat(44);
    assert!(c
        .bind_usdc_recharge_invoice("demo", &o.id, "worker-one", &bad, NOW + 5)
        .is_err());
    assert_eq!(
        c.usdc_recharge_order("demo", &o.id).unwrap().invoice,
        o.invoice
    );
}
#[test]
fn usdc_submitted_digest_is_never_payment_authority() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    let hinted = c
        .submit_usdc_payment_hint("demo", &o.id, &cap(), &"3".repeat(44), NOW + 5)
        .unwrap();
    assert_eq!(hinted.state, UsdcOrderState::InvoiceBound);
    assert!(hinted.receipt.is_none());
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("demo", c.default_character.index, NOW + 20)
            .unwrap()
            .granted,
        0
    );
}
#[test]
fn usdc_payment_every_frozen_binding_and_finality_is_checked() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    let r = payment(&o);
    for change in 0..15 {
        let mut bad = r.clone();
        match change {
            0 => bad.chain_identifier = "mainnet".into(),
            1 => bad.package_id = addr('a'),
            2 => bad.config_id = addr('a'),
            3 => bad.invoice_id = addr('a'),
            4 => bad.order_id = "usdc_wrong".into(),
            5 => bad.account_ref = addr('a'),
            6 => bad.payer = addr('a'),
            7 => bad.merchant = addr('a'),
            8 => bad.coin_type = "0x2::sui::SUI".into(),
            9 => bad.amount_micro += 1,
            10 => bad.frozen_hash = addr('a'),
            11 => bad.checkpoint = 122,
            12 => bad.confirmed_at_ms = NOW - 1,
            13 => bad.confirmed_at_ms = o.expires_at_ms + 1,
            _ => bad.digest = "not-a-digest".into(),
        }
        assert!(
            c.confirm_usdc_recharge_payment("demo", &o.id, &bad, NOW + 2_000_000)
                .is_err(),
            "change {change}"
        );
    }
    assert!(c
        .usdc_recharge_order("demo", &o.id)
        .unwrap()
        .receipt
        .is_none());
}
#[test]
fn usdc_duplicate_callback_parallel_confirmation_and_apply_credit_once() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    let r = payment(&o);
    std::thread::scope(|s| {
        for _ in 0..8 {
            let c = &c;
            let o = &o;
            let r = &r;
            s.spawn(move || {
                c.confirm_usdc_recharge_payment("demo", &o.id, r, NOW + 11)
                    .unwrap()
            });
        }
    });
    assert_eq!(
        c.recharge_credit_status("demo", c.default_character.index)
            .unwrap()
            .1,
        1000
    );
    let before = c
        .recharge_credit_status("demo", c.default_character.index)
        .unwrap()
        .0;
    let applied = c
        .consume_inactive_pending_recharge_credits("demo", c.default_character.index, NOW + 20)
        .unwrap();
    assert_eq!(applied.granted, 1000);
    assert_eq!(applied.credit, before + 1000);
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("demo", c.default_character.index, NOW + 21)
            .unwrap()
            .granted,
        0
    );
    c.confirm_usdc_recharge_payment("demo", &o.id, &r, NOW + 22)
        .unwrap();
    assert_eq!(
        c.usdc_recharge_order("demo", &o.id).unwrap().state,
        UsdcOrderState::Applied
    );
}
#[test]
fn usdc_overflow_retains_paid_receipt_until_wallet_has_room() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    let r = payment(&o);
    c.confirm_usdc_recharge_payment("demo", &o.id, &r, NOW + 11)
        .unwrap();
    c.account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&c.default_character.index)
        .unwrap()
        .credit = u32::MAX;
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("demo", c.default_character.index, NOW + 20)
            .unwrap()
            .granted,
        0
    );
    assert_eq!(
        c.usdc_recharge_order("demo", &o.id).unwrap().state,
        UsdcOrderState::PaidPending
    );
    c.account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&c.default_character.index)
        .unwrap()
        .credit = 0;
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("demo", c.default_character.index, NOW + 21)
            .unwrap()
            .granted,
        1000
    );
}
#[test]
fn usdc_and_stripe_share_one_atomic_wallet_application_without_receipt_relabeling() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    c.confirm_usdc_recharge_payment("demo", &o.id, &payment(&o), NOW + 11)
        .unwrap();
    let stripe = c
        .prepare_recharge_order(
            "demo",
            c.default_character.index,
            "stripe-request",
            &offer(),
            NOW,
            false,
        )
        .unwrap();
    c.confirm_recharge_payment(
        "demo",
        &stripe.id,
        &VerifiedRechargePayment {
            session_id: "cs_adjacent".into(),
            payment_intent_id: "pi_adjacent".into(),
            amount_minor: 1000,
            currency: "usd".into(),
            livemode: false,
        },
        NOW + 11,
    )
    .unwrap();
    let result = c
        .consume_inactive_pending_recharge_credits("demo", c.default_character.index, NOW + 20)
        .unwrap();
    assert_eq!(result.granted, 2000);
    assert_eq!(result.order_ids.len(), 2);
    let stripe = c.recharge_order_for_payment("demo", &stripe.id).unwrap();
    assert_eq!(stripe.state, RechargeOrderState::Applied);
    assert_eq!(
        stripe.applied.as_ref().unwrap().revision,
        c.usdc_recharge_order("demo", &o.id)
            .unwrap()
            .applied
            .unwrap()
            .revision
    );
}
#[test]
fn usdc_receipt_cannot_be_replayed_into_a_different_account_or_order() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    let r = payment(&o);
    let mut a = AccountRecord::new(crate::CharacterRecord {
        index: 99,
        name: "Other".into(),
        ..c.default_character.clone()
    });
    let mut copied = o.clone();
    copied.character_index = 99;
    a.billing = Some(BillingLedger {
        provenance: Default::default(),
        orders: Default::default(),
        sui_usdc_orders: [(copied.id.clone(), copied)].into_iter().collect(),
    });
    c.account_store
        .lock()
        .unwrap()
        .accounts
        .insert("other".into(), a);
    assert!(c
        .confirm_usdc_recharge_payment("other", &o.id, &r, NOW + 11)
        .is_err());
}
fn file_config(label: &str) -> (std::path::PathBuf, SimulationConfig) {
    let p = std::env::temp_dir().join(format!(
        "mir2-usdc-{label}-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let c = SimulationConfig::default().with_account_store_path(p.clone());
    (p, c)
}
#[test]
fn usdc_chain_success_database_failure_can_retry_without_double_credit() {
    let (_path, c) = file_config("known-failure");
    let o = invoice(&c);
    let r = payment(&o);
    c.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(c
        .confirm_usdc_recharge_payment("demo", &o.id, &r, NOW + 11)
        .is_err());
    assert!(c
        .usdc_recharge_order("demo", &o.id)
        .unwrap()
        .receipt
        .is_none());
    c.confirm_usdc_recharge_payment("demo", &o.id, &r, NOW + 12)
        .unwrap();
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("demo", c.default_character.index, NOW + 20)
            .unwrap()
            .granted,
        1000
    );
}
#[test]
fn usdc_unknown_commit_restart_recovers_durable_payment_without_resubmission() {
    let (path, c) = file_config("unknown");
    let o = invoice(&c);
    let r = payment(&o);
    c.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(c
        .confirm_usdc_recharge_payment("demo", &o.id, &r, NOW + 11)
        .is_err());
    let loaded = FileAccountStoreRepository::new(path)
        .load(c.default_character.clone())
        .unwrap();
    let persisted = &loaded.accounts["demo"]
        .billing
        .as_ref()
        .unwrap()
        .sui_usdc_orders[&o.id];
    assert_eq!(persisted.receipt.as_ref(), Some(&r));
    assert_eq!(persisted.state, UsdcOrderState::PaidPending);
    let restarted = SimulationConfig::default();
    *restarted.account_store.lock().unwrap() = loaded;
    restarted
        .confirm_usdc_recharge_payment("demo", &o.id, &r, NOW + 12)
        .unwrap();
    assert_eq!(
        restarted
            .consume_inactive_pending_recharge_credits(
                "demo",
                restarted.default_character.index,
                NOW + 20
            )
            .unwrap()
            .granted,
        1000
    );
    assert_eq!(
        restarted
            .consume_inactive_pending_recharge_credits(
                "demo",
                restarted.default_character.index,
                NOW + 21
            )
            .unwrap()
            .granted,
        0
    );
}
#[test]
fn usdc_schema12_cannot_claim_new_authority_and_legacy_clean_store_upgrades() {
    let c = SimulationConfig::default();
    invoice(&c);
    let mut s = c.account_store.lock().unwrap().clone();
    s.schema_version = 12;
    let (path, _) = file_config("schema");
    s.save_to_path(&path).unwrap();
    assert!(FileAccountStoreRepository::new(path.clone())
        .load(c.default_character.clone())
        .is_err());
    s.accounts.get_mut("demo").unwrap().billing = None;
    s.save_to_path(&path).unwrap();
    assert_eq!(
        FileAccountStoreRepository::new(path)
            .load(c.default_character.clone())
            .unwrap()
            .schema_version,
        13
    );
}
#[test]
fn usdc_expired_capability_does_not_stop_reconciliation_or_delayed_credit() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    let r = payment(&o);
    assert!(c
        .submit_usdc_payment_hint(
            "demo",
            &o.id,
            &cap(),
            &r.digest,
            o.capability_expires_at_ms + 1
        )
        .is_err());
    assert!(c
        .usdc_recharge_reconcile_candidates(100)
        .unwrap()
        .iter()
        .any(|(_, x)| x.id == o.id));
    c.confirm_usdc_recharge_payment("demo", &o.id, &r, o.capability_expires_at_ms + 1)
        .unwrap();
    assert_eq!(
        c.consume_inactive_pending_recharge_credits(
            "demo",
            c.default_character.index,
            o.capability_expires_at_ms + 2
        )
        .unwrap()
        .granted,
        1000
    );
}
#[test]
fn usdc_review_rejects_unknown_and_never_clears_prepared_transaction() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    assert!(c
        .mark_usdc_recharge_review("demo", &o.id, "worker-one", "timeout", NOW + 5)
        .is_err());
    let reviewed = c
        .mark_usdc_recharge_review("demo", &o.id, "worker-one", "invoiceMismatch", NOW + 5)
        .unwrap();
    assert_eq!(reviewed.prepared_tx, o.prepared_tx);
    assert_eq!(reviewed.invoice, o.invoice);
    assert_eq!(reviewed.state, UsdcOrderState::Review);
    assert!(reviewed.receipt.is_none());
}

#[test]
fn usdc_candidates_keep_expired_issued_transactions_but_skip_unissued_and_confirmed_orders() {
    let c = SimulationConfig::default();
    let original = invoice(&c);
    let now = crate::monthly_card::monthly_card_now_ms();
    // Model restoration of an old persisted invoice, with a payment whose
    // confirmed chain timestamp precedes expiry and whose RPC result is late.
    let issued = {
        let mut store = c.account_store.lock().unwrap();
        let ledger = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .billing
            .as_mut()
            .unwrap();
        let issued = ledger.sui_usdc_orders.get_mut(&original.id).unwrap();
        issued.created_at_ms = now - 20_000;
        issued.expires_at_ms = now - 10_000;
        issued.capability_expires_at_ms = now - 5_000;
        issued.job_lease = None;
        let issued = issued.clone();
        let mut never_issued = issued.clone();
        never_issued.id = format!("usdc_{}", "b".repeat(64));
        never_issued.request_id = "expired-never-issued".into();
        never_issued.prepared_tx = None;
        never_issued.invoice = None;
        never_issued.state = UsdcOrderState::PayerBound;
        ledger
            .sui_usdc_orders
            .insert(never_issued.id.clone(), never_issued);
        issued
    };
    let candidates = c.usdc_recharge_reconcile_candidates(100).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].1.id, issued.id);
    let mut receipt = payment(&issued);
    receipt.confirmed_at_ms = issued.created_at_ms + 1;
    c.confirm_usdc_recharge_payment("demo", &issued.id, &receipt, now)
        .unwrap();
    // Confirmed receipts need only ordinary Source settlement, independent of
    // the chain driver or an expired browser capability.
    assert!(c
        .usdc_recharge_reconcile_candidates(100)
        .unwrap()
        .is_empty());
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("demo", c.default_character.index, now)
            .unwrap()
            .granted,
        1000
    );
}

#[test]
fn usdc_reconcile_cursor_reaches_later_orders_and_accounts_beyond_first_page() {
    let c = SimulationConfig::default();
    for (offset, account) in ["alpha", "beta", "zeta"].into_iter().enumerate() {
        let character = crate::CharacterRecord {
            index: 100 + offset as i32,
            name: account.into(),
            ..c.default_character.clone()
        };
        c.account_store
            .lock()
            .unwrap()
            .accounts
            .insert(account.into(), AccountRecord::new(character.clone()));
        for request in ["first", "second", "third"] {
            let order = c
                .prepare_usdc_recharge_order(
                    account,
                    character.index,
                    request,
                    &offer(),
                    &domain(),
                    &cap(),
                    NOW + 1_800_000,
                    NOW + 1_200_000,
                    NOW,
                )
                .unwrap();
            c.bind_usdc_recharge_payer(account, &order.id, &addr('4'), &cap(), NOW + 1)
                .unwrap();
        }
    }
    let full = c.usdc_recharge_reconcile_candidates(100).unwrap();
    assert_eq!(full.len(), 9);
    // Another live worker holding the earliest job must not hide the following
    // records; its action is fenced at claim while pagination still advances.
    c.claim_usdc_recharge_job(&full[0].0, &full[0].1.id, "active-worker", NOW + 2)
        .unwrap()
        .unwrap();
    assert!(c
        .claim_usdc_recharge_job(&full[0].0, &full[0].1.id, "other-worker", NOW + 3)
        .unwrap()
        .is_none());
    let mut after: Option<(String, String)> = None;
    let mut seen = Vec::new();
    for _ in 0..10 {
        let page = c
            .usdc_recharge_reconcile_candidates_after(
                2,
                after.as_ref().map(|(a, o)| (a.as_str(), o.as_str())),
            )
            .unwrap();
        assert!(page.len() <= 2);
        if page.is_empty() {
            break;
        }
        after = page.last().map(|(a, o)| (a.clone(), o.id.clone()));
        seen.extend(page.into_iter().map(|(a, o)| (a, o.id)));
    }
    let expected = full.into_iter().map(|(a, o)| (a, o.id)).collect::<Vec<_>>();
    assert_eq!(seen, expected);
    assert!(c
        .usdc_recharge_reconcile_candidates_after(
            2,
            after.as_ref().map(|(a, o)| (a.as_str(), o.as_str()))
        )
        .unwrap()
        .is_empty());
    assert_eq!(
        c.usdc_recharge_reconcile_candidates_after(2, None).unwrap()[0].0,
        "alpha"
    );
}

fn two_creation_jobs(c: &SimulationConfig) -> Vec<(String, UsdcRechargeOrder)> {
    let mut jobs = Vec::new();
    for (offset, account) in ["first-creator", "second-creator"].into_iter().enumerate() {
        let character = crate::CharacterRecord {
            index: 100 + offset as i32,
            name: account.into(),
            ..c.default_character.clone()
        };
        c.account_store
            .lock()
            .unwrap()
            .accounts
            .insert(account.into(), AccountRecord::new(character.clone()));
        let o = c
            .prepare_usdc_recharge_order(
                account,
                character.index,
                "creation-test",
                &offer(),
                &domain(),
                &cap(),
                NOW + 1_800_000,
                NOW + 1_200_000,
                NOW,
            )
            .unwrap();
        c.bind_usdc_recharge_payer(account, &o.id, &addr('4'), &cap(), NOW + 1)
            .unwrap();
        let leased = c
            .claim_usdc_recharge_job(account, &o.id, account, NOW + 2)
            .unwrap()
            .unwrap();
        jobs.push((account.into(), leased));
    }
    jobs
}
fn creation_tx(marker: char) -> UsdcPreparedTransaction {
    UsdcPreparedTransaction {
        bytes_base64: "TUFDSw==".into(),
        signature_base64: "c2ln".into(),
        digest: marker.to_string().repeat(44),
    }
}
#[test]
fn usdc_only_one_sponsor_creation_persists_and_unknown_or_expired_lease_cannot_release_it() {
    let c = SimulationConfig::default();
    let jobs = two_creation_jobs(&c);
    let results = std::thread::scope(|s| {
        jobs.iter()
            .enumerate()
            .map(|(i, (account, o))| {
                let c = &c;
                s.spawn(move || {
                    c.record_usdc_prepared_transaction(
                        account,
                        &o.id,
                        account,
                        &creation_tx(if i == 0 { '1' } else { '2' }),
                        NOW + 3,
                    )
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| r.as_ref().is_err_and(|e| e == "usdcCreationBusy"))
            .count(),
        1
    );
    let winner = results.iter().position(|r| r.is_ok()).unwrap();
    let loser = 1 - winner;
    let (wa, wo) = &jobs[winner];
    let (la, lo) = &jobs[loser];
    let active = c.usdc_recharge_active_creation(&domain()).unwrap().unwrap();
    assert_eq!((&active.0, &active.1.id), (wa, &wo.id));
    let mut other_domain = domain();
    other_domain.config_id = addr('9');
    assert!(c
        .usdc_recharge_active_creation(&other_domain)
        .unwrap()
        .is_none());
    c.release_usdc_recharge_job(wa, &wo.id, wa, NOW + JOB_LEASE_MS + 3)
        .unwrap();
    c.claim_usdc_recharge_job(la, &lo.id, la, NOW + JOB_LEASE_MS + 4)
        .unwrap()
        .unwrap();
    assert_eq!(
        c.record_usdc_prepared_transaction(
            la,
            &lo.id,
            la,
            &creation_tx('3'),
            NOW + JOB_LEASE_MS + 5
        )
        .unwrap_err(),
        "usdcCreationBusy"
    );
    let persisted = c.usdc_recharge_order(wa, &wo.id).unwrap();
    assert!(creation_in_flight(&persisted));
    assert!(persisted.invoice.is_none());
    assert_eq!(
        persisted.prepared_tx,
        results[winner].as_ref().unwrap().prepared_tx
    );
}
#[test]
fn usdc_confirmed_invoice_or_finalized_creation_failure_releases_sponsor_fence() {
    for final_failed in [false, true] {
        let c = SimulationConfig::default();
        let jobs = two_creation_jobs(&c);
        let (wa, wo) = &jobs[0];
        let (la, lo) = &jobs[1];
        c.record_usdc_prepared_transaction(wa, &wo.id, wa, &creation_tx('1'), NOW + 3)
            .unwrap();
        assert_eq!(
            c.record_usdc_prepared_transaction(la, &lo.id, la, &creation_tx('2'), NOW + 4)
                .unwrap_err(),
            "usdcCreationBusy"
        );
        if final_failed {
            c.mark_usdc_recharge_review(wa, &wo.id, wa, "createFailed", NOW + 5)
                .unwrap();
        } else {
            c.bind_usdc_recharge_invoice(
                wa,
                &wo.id,
                wa,
                &UsdcInvoiceBinding {
                    invoice_id: addr('5'),
                    create_digest: "1".repeat(44),
                    checkpoint: 123,
                },
                NOW + 5,
            )
            .unwrap();
        }
        assert!(!creation_in_flight(
            &c.usdc_recharge_order(wa, &wo.id).unwrap()
        ));
        assert!(c
            .usdc_recharge_active_creation(&domain())
            .unwrap()
            .is_none());
        assert!(c
            .record_usdc_prepared_transaction(la, &lo.id, la, &creation_tx('2'), NOW + 6)
            .is_ok());
    }
}
#[test]
fn usdc_invoice_mismatch_review_does_not_claim_final_creation_failure() {
    let c = SimulationConfig::default();
    let jobs = two_creation_jobs(&c);
    let (wa, wo) = &jobs[0];
    let (la, lo) = &jobs[1];
    c.record_usdc_prepared_transaction(wa, &wo.id, wa, &creation_tx('1'), NOW + 3)
        .unwrap();
    let reviewed = c
        .mark_usdc_recharge_review(wa, &wo.id, wa, "invoiceMismatch", NOW + 4)
        .unwrap();
    assert!(creation_in_flight(&reviewed));
    assert_eq!(
        c.record_usdc_prepared_transaction(la, &lo.id, la, &creation_tx('2'), NOW + 5)
            .unwrap_err(),
        "usdcCreationBusy"
    );
}

// Provider fixtures exercise the real credit/monthly authority. These are not
// chain acceptance and never seed the deployed promotion database.
#[test]
fn usdc_native_funding_fifo_monthly_provenance_is_exact_and_never_production_cash() {
    use crate::billing_provenance::{monthly_revenue_sources, CreditFundingKind};
    let c = SimulationConfig::default()
        .with_billing_monthly_card_credit_price(Some(600))
        .unwrap();
    let index = c.default_character.index;
    let order = invoice(&c);
    let receipt = payment(&order);
    c.confirm_usdc_recharge_payment("demo", &order.id, &receipt, NOW + 11)
        .unwrap();
    assert_eq!(c.recharge_credit_status("demo", index).unwrap(), (0, 1000));
    c.consume_inactive_pending_recharge_credits("demo", index, NOW + 20)
        .unwrap();
    assert_eq!(c.recharge_credit_status("demo", index).unwrap(), (1000, 0));
    let item = c
        .buy_monthly_card_for_character("demo", index, "normal-monthly-purchase", NOW + 30)
        .unwrap();
    assert_eq!(c.recharge_credit_status("demo", index).unwrap(), (400, 0));
    c.activate_monthly_card_for_character("demo", index, item.unique_id, NOW + 31)
        .unwrap();
    let get_account = || c.account_store.lock().unwrap().accounts["demo"].clone();
    let account = get_account();
    let ledger = account.billing.as_ref().unwrap();
    assert!(ledger.orders.is_empty() && ledger.provenance.payments.is_empty());
    assert_eq!(ledger.provenance.wallets[&index].lots.len(), 1);
    assert_eq!(
        ledger.provenance.wallets[&index].lots[0].kind,
        CreditFundingKind::Paid
    );
    assert_eq!(
        ledger.provenance.wallets[&index].lots[0].consumed_credits,
        600
    );
    let sources = monthly_revenue_sources(&account, "demo", false).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].eligible_micro_usd, 6_000_000);
    assert_eq!(sources[0].funding_status, "verified_paid");
    assert_eq!(
        sources[0].parts[0].source.as_ref().unwrap().receipt,
        receipt
    );
    assert_eq!(
        sources[0].parts[0].source.as_ref().unwrap().network,
        "sui:testnet"
    );
    assert_eq!(
        sources[0].parts[0].source.as_ref().unwrap().module,
        "recharge"
    );
    assert_eq!(
        sources[0].parts[0].source.as_ref().unwrap().provider,
        "sui_native_usdc"
    );
    assert_eq!(sources[0].parts[0].original_currency, "testnet-usdc");
    assert_eq!(sources[0].parts[0].source_order_id, order.id);
    let production = monthly_revenue_sources(&account, "demo", true).unwrap();
    assert_eq!(production[0].eligible_micro_usd, 0);
    assert_eq!(
        production[0].parts[0].reason_code,
        "payment_environment_mismatch"
    );
    c.confirm_usdc_recharge_payment("demo", &order.id, &receipt, NOW + 40)
        .unwrap();
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("demo", index, NOW + 41)
            .unwrap()
            .granted,
        0
    );
    assert_eq!(
        c.buy_monthly_card_for_character("demo", index, "normal-monthly-purchase", NOW + 42)
            .unwrap()
            .unique_id,
        item.unique_id
    );
    assert!(
        c.activate_monthly_card_for_character("demo", index, item.unique_id, NOW + 43)
            .unwrap()
            .replayed
    );
    assert_eq!(
        monthly_revenue_sources(&get_account(), "demo", false).unwrap(),
        sources
    );
    assert!(monthly_revenue_sources(&get_account(), "different-owner", false).is_err());
}

#[test]
fn usdc_payment_hint_without_verified_receipt_grants_neither_credit_nor_revenue() {
    let c = SimulationConfig::default();
    let o = invoice(&c);
    c.submit_usdc_payment_hint("demo", &o.id, &cap(), &"3".repeat(44), NOW + 11)
        .unwrap();
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("demo", c.default_character.index, NOW + 12)
            .unwrap()
            .granted,
        0
    );
    let a = c.account_store.lock().unwrap().accounts["demo"].clone();
    assert!(
        crate::billing_provenance::monthly_revenue_sources(&a, "demo", false)
            .unwrap()
            .is_empty()
    );
    assert!(a
        .billing
        .unwrap()
        .provenance
        .wallets
        .values()
        .all(|w| w.lots.is_empty()));
}

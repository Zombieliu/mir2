use super::*;
use crate::billing::{RechargeOffer, VerifiedRechargePayment};
use crate::monthly_card::MONTHLY_CARD_DURATION_MS;

const AT: u64 = 1_800_000_000_000;
fn config(price: u32) -> SimulationConfig {
    SimulationConfig::default()
        .with_billing_monthly_card_credit_price(Some(price))
        .unwrap()
}
fn snapshot(c: &SimulationConfig) -> AccountRecord {
    c.account_store.lock().unwrap().accounts["demo"].clone()
}
fn set_balance(c: &SimulationConfig, amount: u32) {
    c.commit_account_store_transaction(&["demo".into()], |s| {
        s.accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap()
            .credit = amount;
        Ok(())
    })
    .unwrap();
}
fn pay(
    c: &SimulationConfig,
    credits: u32,
    amount: u64,
    tax: Option<u64>,
    currency: &str,
    live: bool,
    key: &str,
) -> RechargeOrder {
    let offer = RechargeOffer {
        id: format!("offer-{key}"),
        label: "test authoritative credit".into(),
        currency: currency.into(),
        amount_minor: amount,
        credits,
    };
    let order = c
        .prepare_recharge_order("demo", 0, key, &offer, AT, live)
        .unwrap();
    let p = VerifiedRechargePayment {
        session_id: format!("cs_{key}"),
        payment_intent_id: format!("pi_{key}"),
        amount_minor: amount,
        currency: currency.into(),
        livemode: live,
    };
    let order = c
        .confirm_recharge_payment("demo", &order.id, &p, AT + 1)
        .unwrap();
    let revenue = VerifiedPaymentRevenue {
        provider: "stripe".into(),
        source_event_id: format!("evt_{key}"),
        order_id: order.id.clone(),
        checkout_session_id: p.session_id,
        payment_intent_id: p.payment_intent_id,
        livemode: live,
        currency: currency.into(),
        gross_amount_minor: amount,
        indirect_tax_minor: tax,
        discount_minor: Some(0),
        paid_at_ms: AT + 1,
        observed_at_ms: AT + 2,
        evidence_ref: format!("stripe:provider-requery:{key}"),
    };
    c.record_verified_payment_revenue("demo", &revenue).unwrap();
    c.consume_inactive_pending_recharge_credits("demo", 0, AT + 3)
        .unwrap();
    order
}
fn card(c: &SimulationConfig, key: &str, at: u64) -> u64 {
    let item = c
        .buy_monthly_card_for_character("demo", 0, key, at)
        .unwrap();
    c.activate_monthly_card_for_character("demo", 0, item.unique_id, at + 1)
        .unwrap();
    item.unique_id
}
fn adverse(
    c: &SimulationConfig,
    order: &RechargeOrder,
    event: &str,
    refund: u64,
    status: &str,
    disputed: u64,
    at: u64,
) {
    c.record_verified_payment_adverse_event(
        "demo",
        &VerifiedPaymentAdverseEvent {
            provider: "stripe".into(),
            source_event_id: event.into(),
            order_id: order.id.clone(),
            payment_intent_id: order.payment_intent_id.clone().unwrap(),
            livemode: order.livemode,
            currency: order.offer.currency.clone(),
            gross_amount_minor: order.offer.amount_minor,
            cumulative_refunded_minor: refund,
            disputed_minor: disputed,
            dispute_status: status.into(),
            occurred_at_ms: at,
            observed_at_ms: at,
            evidence_ref: format!("stripe:adverse:{event}"),
        },
    )
    .unwrap();
}

#[test]
fn historical_credit_is_unknown_and_fifo_only_attributes_actual_paid_slice() {
    let c = config(60);
    set_balance(&c, 50);
    pay(&c, 100, 500, Some(50), "usd", false, "mixed");
    card(&c, "monthly-mixed", AT + 10);
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(
        (s.eligible_micro_usd, s.unknown_credits, s.gift_credits),
        (450_000, 50, 0)
    );
    assert_eq!(s.funding_status, "verified_paid");
    assert_eq!(s.parts[0].credits, 10);
    assert_eq!(
        s.service_end_ms - s.service_start_ms,
        MONTHLY_CARD_DURATION_MS
    );
}

#[test]
fn recharge_alone_is_not_monthly_revenue_and_retry_does_not_mint_new_lots() {
    let c = config(60);
    pay(&c, 100, 500, Some(0), "usd", false, "once");
    c.consume_inactive_pending_recharge_credits("demo", 0, AT + 5)
        .unwrap();
    let a = snapshot(&c);
    assert!(monthly_revenue_sources(&a, "demo", false)
        .unwrap()
        .is_empty());
    let wallet = &a.billing.unwrap().provenance.wallets[&0];
    assert_eq!(wallet.lots.len(), 1);
    assert_eq!(wallet.balance().unwrap(), 100);
}

#[test]
fn ordinary_spending_drains_same_fifo_before_monthly_purchase() {
    let c = config(60);
    pay(&c, 100, 500, Some(50), "usd", false, "other");
    c.commit_account_store_transaction(&["demo".into()], |store| {
        let account = store.accounts.get_mut("demo").unwrap();
        consume_ordinary_credit(account, 0, 100, 40, AT + 4)?;
        account.saves.get_mut(&0).unwrap().credit = 60;
        Ok(())
    })
    .unwrap();
    card(&c, "monthly-after-other", AT + 10);
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.parts[0].credits_offset, 40);
    assert_eq!(s.eligible_micro_usd, 2_700_000);
}

#[test]
fn gameplay_credit_stays_non_cash_even_when_reward_and_purchase_net_to_zero() {
    let c = config(60);
    pay(&c, 60, 600, Some(0), "usd", false, "earned");
    c.commit_account_store_transaction(&["demo".into()], |store| {
        let a = store.accounts.get_mut("demo").unwrap();
        // Source runtime gained 60 and immediately bought an ordinary 60 item.
        consume_ordinary_credit(a, 0, 120, 60, AT + 4)?;
        a.saves.get_mut(&0).unwrap().credit = 60;
        Ok(())
    })
    .unwrap();
    card(&c, "monthly-with-reward", AT + 10);
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.eligible_micro_usd, 0);
    assert_eq!(s.gift_credits, 60);
    assert_eq!(s.funding_status, "excluded");
}

#[test]
fn tax_fx_and_environment_are_proven_instead_of_inferred_from_amount() {
    for (currency, tax, live, query, reason) in [
        ("usd", None, false, false, "payment_tax_unverified"),
        ("eur", Some(0), false, false, "payment_fx_unapproved"),
        ("usd", Some(0), false, true, "payment_environment_mismatch"),
    ] {
        let c = config(60);
        pay(&c, 60, 600, tax, currency, live, "pending");
        card(&c, "monthly-pending", AT + 10);
        let s = monthly_revenue_sources(&snapshot(&c), "demo", query)
            .unwrap()
            .remove(0);
        assert_eq!(s.funding_status, "pending");
        assert_eq!(s.eligible_micro_usd, 0);
        assert_eq!(s.parts[0].reason_code, reason);
    }
}

#[test]
fn duplicate_out_of_order_refunds_use_cumulative_provider_total_at_original_cost() {
    let c = config(60);
    let o = pay(&c, 100, 500, Some(50), "usd", false, "refund");
    card(&c, "monthly-refund", AT + 10);
    adverse(&c, &o, "evt_refund_b", 250, "none", 0, AT + 20);
    adverse(&c, &o, "evt_refund_b", 250, "none", 0, AT + 21);
    adverse(&c, &o, "evt_refund_a", 100, "none", 0, AT + 22);
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.eligible_micro_usd, 2_700_000);
    assert_eq!(s.reversed_micro_usd, 1_350_000);
    assert_eq!(snapshot(&c).saves[&0].credit, 40);
    assert_eq!(
        snapshot(&c)
            .billing
            .unwrap()
            .provenance
            .adverse_events
            .len(),
        2
    );
}

#[test]
fn dispute_open_is_pending_won_is_released_and_lost_is_reversed() {
    let c = config(60);
    let o = pay(&c, 100, 500, Some(0), "usd", false, "dispute");
    card(&c, "monthly-dispute", AT + 10);
    adverse(
        &c,
        &o,
        "evt_dispute_open",
        0,
        "needs_response",
        500,
        AT + 20,
    );
    assert_eq!(
        monthly_revenue_sources(&snapshot(&c), "demo", false).unwrap()[0].funding_status,
        "pending"
    );
    adverse(&c, &o, "evt_dispute_won", 0, "won", 500, AT + 21);
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.funding_status, "verified_paid");
    assert_eq!(s.reversed_micro_usd, 0);
    adverse(&c, &o, "evt_dispute_lost", 0, "lost", 500, AT + 22);
    assert_eq!(
        monthly_revenue_sources(&snapshot(&c), "demo", false).unwrap()[0].reversed_micro_usd,
        3_000_000
    );
}

#[test]
fn gift_monthly_card_never_inherits_recipients_paid_wallet() {
    let c = config(60);
    pay(&c, 100, 500, Some(0), "usd", false, "gift");
    let uid = card(&c, "monthly-gift", AT + 10);
    let mut a = snapshot(&c);
    let p = &mut a
        .billing
        .as_mut()
        .unwrap()
        .provenance
        .monthly_items
        .get_mut(&uid)
        .unwrap();
    p.gift = true;
    p.payer_account_id = "someone_else".into();
    let s = monthly_revenue_sources(&a, "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.funding_status, "excluded");
    assert_eq!(s.eligible_micro_usd, 0);
}

#[test]
fn integer_lot_cost_is_conserved_without_per_purchase_rounding_gain() {
    let amounts: Vec<_> = (0..97)
        .map(|offset| portion(10_001, offset, 1, 97).unwrap())
        .collect();
    assert_eq!(amounts.iter().sum::<u64>(), 10_001);
    assert!(portion(10_001, 96, 2, 97).is_err());
}

#[test]
fn immutable_source_evidence_and_spent_lots_cannot_be_reset() {
    let c = config(60);
    pay(&c, 100, 500, Some(0), "usd", false, "immutable");
    card(&c, "monthly-immutable", AT + 10);
    let err = c
        .commit_account_store_transaction(&["demo".into()], |store| {
            store
                .accounts
                .get_mut("demo")
                .unwrap()
                .billing
                .as_mut()
                .unwrap()
                .provenance
                .payments
                .clear();
            Ok(())
        })
        .unwrap_err();
    assert_eq!(err, "billingProvenanceHistoryChanged");
    let err = c
        .commit_account_store_transaction(&["demo".into()], |store| {
            store
                .accounts
                .get_mut("demo")
                .unwrap()
                .billing
                .as_mut()
                .unwrap()
                .provenance
                .wallets
                .get_mut(&0)
                .unwrap()
                .lots[0]
                .consumed_credits = 0;
            Ok(())
        })
        .unwrap_err();
    assert_eq!(err, "billingProvenanceHistoryChanged");
}

#[test]
fn two_paid_lots_fund_one_card_and_refund_only_its_source_slice() {
    let c = config(60);
    let first = pay(&c, 30, 300, Some(0), "usd", false, "first");
    pay(&c, 70, 1400, Some(0), "usd", false, "second");
    card(&c, "monthly-two-lots", AT + 10);
    adverse(&c, &first, "evt_first_refunded", 300, "none", 0, AT + 20);
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.parts.len(), 2);
    assert_eq!(s.eligible_micro_usd, 9_000_000);
    assert_eq!(s.reversed_micro_usd, 3_000_000);
    assert_eq!(s.parts[1].reversed_micro_usd, 0);
}

#[test]
fn legacy_order_does_not_retroactively_relabel_historical_balance_as_paid() {
    let c = config(60);
    pay(&c, 100, 500, Some(0), "usd", false, "legacy");
    // Model a historical persisted order from before the provenance migration.
    {
        let mut store = c.account_store.lock().unwrap();
        store
            .accounts
            .get_mut("demo")
            .unwrap()
            .billing
            .as_mut()
            .unwrap()
            .provenance = CreditProvenance::default();
    }
    card(&c, "monthly-legacy", AT + 10);
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.eligible_micro_usd, 0);
    assert_eq!(s.unknown_credits, 60);
    assert_eq!(s.funding_status, "pending");
}

#[test]
fn restoring_an_old_game_backup_cannot_rewind_paid_credit_or_monthly_history() {
    let c = config(60);
    let path = std::env::temp_dir().join(format!(
        "promotion-financial-restore-{}-{}.json",
        std::process::id(),
        AT
    ));
    c.backup_account_store(&path).unwrap();
    pay(&c, 100, 500, Some(0), "usd", false, "restore");
    card(&c, "monthly-restore-protection", AT + 10);
    let before = serde_json::to_value(snapshot(&c)).unwrap();
    let result = c.restore_account_store_from_backup(&path);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        result.unwrap_err(),
        "billingFinancialRestoreRequiresReconciliation"
    );
    assert_eq!(serde_json::to_value(snapshot(&c)).unwrap(), before);
}

#[test]
fn overlapping_refund_and_chargeback_hold_unproven_loss_instead_of_double_counting() {
    let c = config(60);
    let o = pay(&c, 100, 500, Some(0), "usd", false, "overlap");
    card(&c, "monthly-overlap-hold", AT + 10);
    adverse(&c, &o, "evt_overlap_refund", 250, "none", 0, AT + 20);
    adverse(&c, &o, "evt_overlap_lost", 250, "lost", 250, AT + 21);
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.funding_status, "pending");
    assert_eq!(
        s.parts[0].reason_code,
        "payment_refund_dispute_reconciliation_required"
    );
    assert_eq!(s.reversed_micro_usd, 1_500_000);
}

#[test]
fn review_marker_holds_revenue_during_callback_crash_before_financial_capture() {
    let c = config(60);
    let o = pay(&c, 100, 500, Some(0), "usd", false, "callback-gap");
    card(&c, "monthly-callback-gap", AT + 10);
    c.mark_recharge_review(
        "demo",
        &o.id,
        "charge.refunded",
        "evt_callback_gap",
        AT + 20,
    )
    .unwrap();
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.funding_status, "pending");
    assert_eq!(s.parts[0].reason_code, "payment_review_evidence_pending");
    adverse(
        &c,
        &o,
        "evt_callback_gap:refund:250:dispute:0:none",
        250,
        "none",
        0,
        AT + 21,
    );
    let s = monthly_revenue_sources(&snapshot(&c), "demo", false)
        .unwrap()
        .remove(0);
    assert_eq!(s.funding_status, "verified_paid");
    assert_eq!(s.reversed_micro_usd, 1_500_000);
}

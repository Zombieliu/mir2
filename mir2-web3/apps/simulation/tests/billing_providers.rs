//! Provider identity, crash recovery, legacy JSON and shared credit invariants.
use mir2_simulation::{
    billing::{
        FiatPaymentProvider as Provider, RechargeOffer, RechargeOrder, RechargeOrderState,
        VerifiedRechargePayment,
    },
    SimulationConfig,
};

const NOW: u64 = 1_800_000_000_000;
fn offer() -> RechargeOffer {
    RechargeOffer {
        id: "credits-100".into(),
        label: "Credits".into(),
        currency: "usd".into(),
        amount_minor: 100,
        credits: 100,
    }
}
fn prepare(config: &SimulationConfig, request: &str, provider: Provider) -> RechargeOrder {
    config
        .prepare_recharge_order_for_provider(
            "demo",
            config.default_character.index,
            request,
            &offer(),
            NOW,
            false,
            provider,
        )
        .unwrap()
}
fn payment(id: &str) -> VerifiedRechargePayment {
    VerifiedRechargePayment {
        session_id: id.into(),
        payment_intent_id: id.into(),
        amount_minor: 100,
        currency: "usd".into(),
        livemode: false,
    }
}

#[test]
fn provider_legacy_json_preserves_stripe_and_rejects_unknown_providers() {
    let config = SimulationConfig::default();
    let order = prepare(&config, "legacy", Provider::Stripe);
    let value = serde_json::to_value(&order).unwrap();
    assert!(value.get("provider").is_none());
    assert!(value.get("providerRequestStartedAtMs").is_none());
    assert_eq!(
        serde_json::from_value::<RechargeOrder>(value.clone()).unwrap(),
        order
    );
    let mut unknown = value;
    unknown["provider"] = serde_json::json!("untrusted-provider");
    assert!(serde_json::from_value::<RechargeOrder>(unknown).is_err());
}

#[test]
fn provider_selection_is_frozen_and_callbacks_cannot_cross_channels() {
    let config = SimulationConfig::default();
    let stripe = prepare(&config, "stripe-intent", Provider::Stripe);
    let waffo = prepare(&config, "waffo-intent", Provider::Waffo);
    assert_eq!(
        config
            .prepare_recharge_order_for_provider(
                "demo",
                config.default_character.index,
                "stripe-intent",
                &offer(),
                NOW,
                false,
                Provider::Waffo
            )
            .unwrap_err(),
        "billingRequestIdReused"
    );
    assert!(config
        .begin_recharge_provider_request("demo", &waffo.id, Provider::Waffo, NOW)
        .unwrap());
    assert_eq!(
        config
            .confirm_recharge_provider_payment(
                "demo",
                &stripe.id,
                Provider::Waffo,
                &payment("acquiring-123"),
                NOW + 1
            )
            .unwrap_err(),
        "billingPaymentBindingMismatch"
    );
    let stripe_payment = VerifiedRechargePayment {
        session_id: "cs_verified".into(),
        payment_intent_id: "pi_verified".into(),
        ..payment("unused")
    };
    assert_eq!(
        config
            .confirm_recharge_payment("demo", &waffo.id, &stripe_payment, NOW + 1)
            .unwrap_err(),
        "billingPaymentBindingMismatch"
    );
    assert_eq!(
        config
            .recharge_credit_status("demo", config.default_character.index)
            .unwrap(),
        (0, 0)
    );
}

#[test]
fn provider_request_claim_survives_restart_and_never_sends_a_second_create() {
    let directory = std::env::temp_dir().join(format!(
        "mir2-provider-recovery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("accounts.json");
    let config = SimulationConfig::default().with_account_store_path(&path);
    let order = prepare(&config, "request-once", Provider::Waffo);
    assert!(config
        .begin_recharge_provider_request("demo", &order.id, Provider::Waffo, NOW)
        .unwrap());
    let restarted = SimulationConfig::default().with_account_store_path(&path);
    assert!(!restarted
        .begin_recharge_provider_request("demo", &order.id, Provider::Waffo, NOW + 1)
        .unwrap());
    assert_eq!(
        restarted
            .recharge_order_for_payment("demo", &order.id)
            .unwrap()
            .provider_request_started_at_ms,
        Some(NOW)
    );
}

#[test]
fn provider_verified_waffo_payment_and_duplicate_delivery_share_existing_ledger() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    let order = prepare(&config, "waffo-paid", Provider::Waffo);
    config
        .begin_recharge_provider_request("demo", &order.id, Provider::Waffo, NOW)
        .unwrap();
    let verified = payment("acquiring-verified-123");
    config
        .bind_recharge_provider_checkout(
            "demo",
            &order.id,
            Provider::Waffo,
            &verified.session_id,
            NOW + 1,
        )
        .unwrap();
    config
        .confirm_recharge_provider_payment("demo", &order.id, Provider::Waffo, &verified, NOW + 2)
        .unwrap();
    let applied = config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 3)
        .unwrap();
    assert_eq!((applied.credit, applied.granted), (100, 100));
    let duplicate = config
        .confirm_recharge_provider_payment("demo", &order.id, Provider::Waffo, &verified, NOW + 4)
        .unwrap();
    assert_eq!(duplicate.state, RechargeOrderState::Applied);
    assert_eq!(
        config
            .consume_inactive_pending_recharge_credits("demo", index, NOW + 5)
            .unwrap()
            .granted,
        0
    );
    let other = prepare(&config, "waffo-another", Provider::Waffo);
    config
        .begin_recharge_provider_request("demo", &other.id, Provider::Waffo, NOW)
        .unwrap();
    assert_eq!(
        config
            .confirm_recharge_provider_payment(
                "demo",
                &other.id,
                Provider::Waffo,
                &verified,
                NOW + 6
            )
            .unwrap_err(),
        "billingProviderBindingAlreadyUsed"
    );
}

#[test]
fn provider_revenue_and_refund_evidence_remain_bound_to_channel() {
    use mir2_simulation::billing_provenance::VerifiedPaymentRevenue;
    let config = SimulationConfig::default();
    let order = prepare(&config, "revenue", Provider::Waffo);
    config
        .begin_recharge_provider_request("demo", &order.id, Provider::Waffo, NOW)
        .unwrap();
    config
        .confirm_recharge_provider_payment(
            "demo",
            &order.id,
            Provider::Waffo,
            &payment("acquiring-revenue"),
            NOW + 1,
        )
        .unwrap();
    let mut revenue = VerifiedPaymentRevenue {
        provider: "stripe".into(),
        source_event_id: "payment-acquiring-revenue".into(),
        order_id: order.id.clone(),
        checkout_session_id: "acquiring-revenue".into(),
        payment_intent_id: "acquiring-revenue".into(),
        livemode: false,
        currency: "usd".into(),
        gross_amount_minor: 100,
        indirect_tax_minor: None,
        discount_minor: None,
        paid_at_ms: NOW + 1,
        observed_at_ms: NOW + 2,
        evidence_ref: "waffo:acquiring-revenue".into(),
    };
    assert_eq!(
        config
            .record_verified_payment_revenue("demo", &revenue)
            .unwrap_err(),
        "billingRevenueEvidenceMismatch"
    );
    revenue.provider = "waffo".into();
    config
        .record_verified_payment_revenue("demo", &revenue)
        .unwrap();
    assert_eq!(
        config
            .mark_recharge_review("demo", &order.id, "refund", "evt_wrong_provider", NOW + 2)
            .unwrap_err(),
        "billingPaymentBindingMismatch"
    );
    config
        .mark_recharge_provider_review(
            "demo",
            &order.id,
            Provider::Waffo,
            "refund",
            "refund-acquiring-revenue",
            NOW + 2,
        )
        .unwrap();
    assert_eq!(
        config
            .consume_inactive_pending_recharge_credits(
                "demo",
                config.default_character.index,
                NOW + 3
            )
            .unwrap()
            .granted,
        0
    );
}

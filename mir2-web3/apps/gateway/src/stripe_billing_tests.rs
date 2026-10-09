//! Actual local HTTP transport, with a prepared Stripe provider fixture.
//! These tests do not call Stripe or establish real-payment acceptance.
use super::*;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, Uri},
    routing::any,
    Json, Router,
};
use mir2_simulation::{
    billing::RechargeOrderState,
    monthly_card::{monthly_card_now_ms, MONTHLY_CARD_DURATION_MS},
    AccountStoreTransactionFault,
};
use std::{collections::BTreeMap, sync::Mutex};

#[derive(Default)]
struct Fixture {
    responses: Mutex<BTreeMap<String, Value>>,
    calls: Mutex<Vec<(String, String, HeaderMap, Vec<u8>)>>,
}
async fn respond(
    State(f): State<Arc<Fixture>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Json<Value> {
    f.calls.lock().unwrap().push((
        method.to_string(),
        uri.path().into(),
        headers,
        body.to_vec(),
    ));
    Json(
        f.responses
            .lock()
            .unwrap()
            .get(uri.path())
            .cloned()
            .unwrap_or(json!({"error":"unknown fixture path"})),
    )
}
struct Mock {
    provider: StripeBilling,
    fixture: Arc<Fixture>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn mock() -> Mock {
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new()
        .fallback(any(respond))
        .with_state(Arc::clone(&fixture));
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Mock {
        provider: StripeBilling {
            key: Arc::new("sk_test_fixture_only_no_real_key".into()),
            webhook_secret: Arc::new("whsec_fixture_only_no_real_secret".into()),
            offers: vec![offer()],
            livemode: false,
            public_base: "https://billing.example.test".into(),
            api_base: format!("http://{addr}/v1"),
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
        },
        fixture,
        task,
    }
}
fn offer() -> RechargeOffer {
    RechargeOffer {
        id: "test-points".into(),
        label: "Test Credits".into(),
        currency: "usd".into(),
        amount_minor: 500,
        credits: 100,
    }
}
fn config() -> SimulationConfig {
    let root = std::env::temp_dir().join(format!(
        "mir2-stripe-http-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&root).unwrap();
    SimulationConfig::default()
        .with_account_store_path(root.join("accounts.json"))
        .with_billing_monthly_card_credit_price(Some(60))
        .unwrap()
}
fn order(config: &SimulationConfig) -> RechargeOrder {
    config
        .prepare_recharge_order(
            "demo",
            config.default_character.index,
            "http-stable-order",
            &offer(),
            monthly_card_now_ms(),
            false,
        )
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stripe_store_policy_requires_durable_wallet_price_and_live_source_of_truth() {
    let mut m = mock().await;
    let memory = SimulationConfig::default();
    assert_eq!(
        m.provider.validate_store(&memory).unwrap_err(),
        "billingDurableStoreRequired"
    );
    let mut durable = config();
    durable.billing_monthly_card_credit_price = None;
    assert_eq!(
        m.provider.validate_store(&durable).unwrap_err(),
        "billingMonthlyCardPriceRequired"
    );
    durable.billing_monthly_card_credit_price = Some(60);
    assert!(m.provider.validate_store(&durable).is_ok());
    m.provider.livemode = true;
    assert_eq!(
        m.provider.validate_store(&durable).unwrap_err(),
        "billingLiveSourceStoreRequired"
    );
    // Check configuration policy without opening a database or contacting Stripe.
    durable.account_store_database_url =
        Some("postgres://billing-policy-fixture.invalid/unused".into());
    durable.account_store_database_mode = mir2_simulation::AccountStoreDatabaseMode::Mirror;
    assert_eq!(
        m.provider.validate_store(&durable).unwrap_err(),
        "billingLiveSourceStoreRequired"
    );
    durable.account_store_database_mode = mir2_simulation::AccountStoreDatabaseMode::SourceOfTruth;
    assert!(m.provider.validate_store(&durable).is_ok());
    assert!(m.fixture.calls.lock().unwrap().is_empty());
}
fn session(order: &RechargeOrder) -> Value {
    json!({"id":"cs_test_fixture","object":"checkout.session","mode":"payment","client_reference_id":order.id,
        "metadata":{"mir2_account_id":"demo","mir2_character_index":order.character_index.to_string(),"mir2_order_id":order.id},
        "amount_total":500,"currency":"usd","livemode":false,"payment_status":"paid","payment_intent":"pi_fixture",
        "url":"https://checkout.stripe.com/c/pay/cs_test_fixture#opaque"})
}
fn event(kind: &str, id: &str, object: Value) -> Vec<u8> {
    serde_json::to_vec(
        &json!({"id":id,"object":"event","api_version":STRIPE_API_VERSION,"livemode":false,
        "account":null,"type":kind,"data":{"object":object}}),
    )
    .unwrap()
}
fn signature(provider: &StripeBilling, body: &[u8], now: u64) -> String {
    let stamp = now / 1000;
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(provider.webhook_secret.as_bytes()).unwrap();
    mac.update(stamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    format!(
        "t={stamp},v1={}",
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
fn set_session(mock: &Mock, value: Value) {
    let mut responses = mock.fixture.responses.lock().unwrap();
    responses.insert("/v1/checkout/sessions".into(), value.clone());
    responses.insert("/v1/checkout/sessions/cs_test_fixture".into(), value);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stripe_http_checkout_binding_failure_retries_same_frozen_order_and_key() {
    let m = mock().await;
    let c = config();
    let o = order(&c);
    set_session(&m, session(&o));
    c.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(m.provider.checkout(&c, "demo", &o).await.is_err());
    assert_eq!(
        c.recharge_orders("demo", o.character_index).unwrap()[0].state,
        RechargeOrderState::Prepared
    );
    let url = m.provider.checkout(&c, "demo", &o).await.unwrap();
    assert!(valid_checkout_url(&url));
    let bound = c
        .recharge_orders("demo", o.character_index)
        .unwrap()
        .remove(0);
    assert_eq!(
        bound.checkout_session_id.as_deref(),
        Some("cs_test_fixture")
    );
    m.provider.checkout(&c, "demo", &bound).await.unwrap();
    let calls = m.fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].0, "POST");
    assert_eq!(calls[1].0, "POST");
    assert_eq!(calls[2].0, "GET");
    assert_eq!(
        calls[0].2.get("idempotency-key"),
        calls[1].2.get("idempotency-key")
    );
    assert_eq!(
        calls[0].2.get("stripe-version").unwrap(),
        STRIPE_API_VERSION
    );
    let params = reqwest::Url::parse(&format!(
        "https://fixture.invalid/?{}",
        String::from_utf8(calls[0].3.clone()).unwrap()
    ))
    .unwrap()
    .query_pairs()
    .map(|(k, v)| (k.into_owned(), v.into_owned()))
    .collect::<BTreeMap<_, _>>();
    assert_eq!(params["line_items[0][price_data][unit_amount]"], "500");
    assert_eq!(params["line_items[0][price_data][currency]"], "usd");
    assert_eq!(params["metadata[mir2_order_id]"], o.id);
    assert_eq!(
        params["payment_intent_data[metadata][mir2_account_id]"],
        "demo"
    );
    assert_eq!(params["payment_method_types[0]"], "card");
    assert_eq!(
        c.recharge_credit_status("demo", o.character_index).unwrap(),
        (0, 0)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stripe_http_duplicate_webhooks_only_prepare_one_credit_application() {
    let m = mock().await;
    let c = config();
    let o = order(&c);
    set_session(&m, session(&o));
    let now = monthly_card_now_ms();
    let body = event(
        "checkout.session.completed",
        "evt_fixture",
        json!({"id":"cs_test_fixture"}),
    );
    let signed = signature(&m.provider, &body, now);
    // Payment callback preceding bind is valid; later binding cannot replace it.
    m.provider.webhook(&c, &signed, &body, now).await.unwrap();
    m.provider.webhook(&c, &signed, &body, now).await.unwrap();
    assert_eq!(
        c.recharge_credit_status("demo", o.character_index).unwrap(),
        (0, 100)
    );
    let applied = c
        .consume_inactive_pending_recharge_credits("demo", o.character_index, now + 1)
        .unwrap();
    assert_eq!((applied.granted, applied.credit), (100, 100));
    m.provider
        .webhook(&c, &signed, &body, now + 2)
        .await
        .unwrap();
    assert_eq!(
        c.consume_inactive_pending_recharge_credits("demo", o.character_index, now + 3)
            .unwrap()
            .granted,
        0
    );
    assert_eq!(
        c.recharge_credit_status("demo", o.character_index).unwrap(),
        (100, 0)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stripe_http_signature_and_frozen_payment_mismatch_cannot_credit_wallet() {
    let m = mock().await;
    let c = config();
    let o = order(&c);
    set_session(&m, session(&o));
    let now = monthly_card_now_ms();
    let body = event(
        "checkout.session.completed",
        "evt_mismatch",
        json!({"id":"cs_test_fixture"}),
    );
    let signed = signature(&m.provider, &body, now);
    assert!(m.provider.webhook(&c, &signed, b"{}", now).await.is_err());
    assert!(m.fixture.calls.lock().unwrap().is_empty());
    for (key, value) in [
        ("amount_total", json!(499)),
        ("currency", json!("brl")),
        ("livemode", json!(true)),
        ("client_reference_id", json!("other-order")),
        (
            "metadata",
            json!({"mir2_account_id":"other","mir2_character_index":o.character_index.to_string(),"mir2_order_id":o.id}),
        ),
    ] {
        let mut bad = session(&o);
        bad[key] = value;
        set_session(&m, bad);
        assert!(
            m.provider.webhook(&c, &signed, &body, now).await.is_err(),
            "{key}"
        );
        assert_eq!(
            c.recharge_credit_status("demo", o.character_index).unwrap(),
            (0, 0)
        );
    }
    // A valid provider response with payment_status=unpaid is not settlement.
    let mut unpaid = session(&o);
    unpaid["payment_status"] = json!("unpaid");
    set_session(&m, unpaid);
    m.provider.webhook(&c, &signed, &body, now).await.unwrap();
    assert_eq!(
        c.recharge_credit_status("demo", o.character_index).unwrap(),
        (0, 0)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stripe_http_refund_after_monthly_card_purchase_preserves_spent_wallet_and_expiry() {
    let m = mock().await;
    let c = config();
    let o = order(&c);
    set_session(&m, session(&o));
    let now = monthly_card_now_ms();
    let body = event(
        "checkout.session.completed",
        "evt_paid_refund",
        json!({"id":"cs_test_fixture"}),
    );
    m.provider
        .webhook(&c, &signature(&m.provider, &body, now), &body, now)
        .await
        .unwrap();
    c.consume_inactive_pending_recharge_credits("demo", o.character_index, now + 1)
        .unwrap();
    let card = c
        .buy_monthly_card_for_character("demo", o.character_index, "spend-for-card", now + 2)
        .unwrap();
    c.activate_monthly_card_for_character("demo", o.character_index, card.unique_id, now + 3)
        .unwrap();
    {
        let mut responses = m.fixture.responses.lock().unwrap();
        responses.insert("/v1/charges/ch_fixture".into(),json!({"object":"charge","id":"ch_fixture","payment_intent":"pi_fixture","livemode":false,"currency":"usd","amount":500}));
        responses.insert("/v1/payment_intents/pi_fixture".into(),json!({"object":"payment_intent","id":"pi_fixture","livemode":false,"amount":500,"currency":"usd","metadata":session(&o)["metadata"]}));
    }
    let adverse = event(
        "charge.refunded",
        "evt_refund_fixture",
        json!({"id":"ch_fixture"}),
    );
    m.provider
        .webhook(
            &c,
            &signature(&m.provider, &adverse, now + 4),
            &adverse,
            now + 4,
        )
        .await
        .unwrap();
    m.provider
        .webhook(
            &c,
            &signature(&m.provider, &adverse, now + 5),
            &adverse,
            now + 5,
        )
        .await
        .unwrap();
    assert_eq!(
        c.recharge_credit_status("demo", o.character_index).unwrap(),
        (40, 0)
    );
    assert_eq!(
        c.refresh_monthly_card_status("demo", now + 5)
            .unwrap()
            .expires_at_ms,
        Some(now + 3 + MONTHLY_CARD_DURATION_MS)
    );
    let reviewed = c
        .recharge_orders("demo", o.character_index)
        .unwrap()
        .remove(0);
    assert_eq!(reviewed.state, RechargeOrderState::Review);
    assert_eq!(reviewed.review_events.len(), 1);
    assert!(reviewed.applied.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stripe_http_wrong_checkout_host_and_old_unbound_order_never_create_payment_again() {
    let m = mock().await;
    let c = config();
    let o = order(&c);
    let mut bad = session(&o);
    bad["url"] = json!("https://attacker.invalid/c/pay/x");
    set_session(&m, bad);
    assert!(m.provider.checkout(&c, "demo", &o).await.is_err());
    assert!(c.recharge_orders("demo", o.character_index).unwrap()[0]
        .checkout_session_id
        .is_none());
    let old = c
        .prepare_recharge_order(
            "demo",
            o.character_index,
            "old-unbound-order",
            &offer(),
            monthly_card_now_ms() - 21 * 60 * 60 * 1000,
            false,
        )
        .unwrap();
    let before = m.fixture.calls.lock().unwrap().len();
    assert_eq!(
        m.provider.checkout(&c, "demo", &old).await.unwrap_err(),
        "billingResultUnconfirmed"
    );
    assert_eq!(m.fixture.calls.lock().unwrap().len(), before);
}

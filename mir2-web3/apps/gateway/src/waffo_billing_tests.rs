//! Local signed HTTP fixtures only; no WAFFO merchant or real payment is used.
use super::*;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use std::{collections::BTreeMap, sync::Mutex};

struct Fixture {
    responses: Mutex<BTreeMap<String, Value>>,
    calls: Mutex<Vec<(String, HeaderMap, Vec<u8>)>>,
    unsigned: Mutex<bool>,
    key: EncodingKey,
}
async fn respond(
    State(f): State<Arc<Fixture>>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    f.calls
        .lock()
        .unwrap()
        .push((uri.path().into(), headers, body.to_vec()));
    let data = f
        .responses
        .lock()
        .unwrap()
        .get(uri.path())
        .cloned()
        .unwrap_or(json!({"code":"E0001"}));
    let raw = data.to_string();
    let signature = if *f.unsigned.lock().unwrap() {
        "invalid".into()
    } else {
        protocol::sign(raw.as_bytes(), &f.key).unwrap()
    };
    (
        StatusCode::OK,
        [
            ("content-type", "application/json"),
            ("x-signature", signature.as_str()),
        ],
        raw,
    )
        .into_response()
}
struct Mock {
    provider: WaffoBilling,
    fixture: Arc<Fixture>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn mock() -> Mock {
    let (signing_key, verifying_key) = protocol::test_keys();
    let fixture = Arc::new(Fixture {
        responses: Mutex::new(BTreeMap::new()),
        calls: Mutex::new(Vec::new()),
        unsigned: Mutex::new(false),
        key: signing_key.clone(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/api/v1/order/create", post(respond))
        .route("/api/v1/order/inquiry", post(respond))
        .with_state(fixture.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Mock {
        provider: WaffoBilling {
            offers: vec![offer()],
            livemode: false,
            merchant_id: "merchant-fixture".into(),
            api_key: Arc::new("api-key-fixture-only".into()),
            signing_key,
            verifying_key,
            public_base: "https://billing.example.test".into(),
            api_base: format!("http://{address}"),
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
        id: "credits-100".into(),
        label: "Test Credits".into(),
        currency: "usd".into(),
        amount_minor: 100,
        credits: 100,
    }
}
fn config() -> SimulationConfig {
    let root = std::env::temp_dir().join(format!(
        "mir2-waffo-http-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&root).unwrap();
    SimulationConfig::default()
        .with_account_store_path(root.join("accounts.json"))
        .with_billing_monthly_card_credit_price(Some(1000))
        .unwrap()
}
fn order(config: &SimulationConfig) -> RechargeOrder {
    config
        .prepare_recharge_order_for_provider(
            "demo",
            config.default_character.index,
            "waffo-http-intent",
            &offer(),
            monthly_card_now_ms() - 1000,
            false,
            PROVIDER,
        )
        .unwrap()
}
fn data(m: &Mock, o: &RechargeOrder, status: &str) -> Value {
    json!({"paymentRequestId":payment_request_id(o).unwrap(),"merchantOrderId":merchant_order_id(o).unwrap(),"acquiringOrderId":"ACQ_fixture_123","orderStatus":status,
        "orderAction":json!({"actionType":"REDIRECT","webUrl":"https://cashier.waffo.com/fixture-key"}).to_string(),
        "merchantInfo":{"merchantId":"merchant-fixture"},"userInfo":{"userId":"demo"},"extendInfo":m.provider.metadata("demo",o),"orderCurrency":"USD","orderAmount":"1.00","orderCompletedAt":requested_at(o.created_at_ms).unwrap()})
}
fn set(m: &Mock, path: &str, data: Value) {
    m.fixture
        .responses
        .lock()
        .unwrap()
        .insert(path.into(), json!({"code":"0","msg":"Success","data":data}));
}
fn event(m: &Mock, kind: &str, result: Value) -> (Vec<u8>, String) {
    let raw = serde_json::to_vec(&json!({"eventType":kind,"result":result})).unwrap();
    let signature = protocol::sign(&raw, &m.fixture.key).unwrap();
    (raw, signature)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waffo_checkout_signs_frozen_quote_and_restart_retry_only_inquires() {
    let m = mock().await;
    let config = config();
    let o = order(&config);
    let d = data(&m, &o, "PAYMENT_PROCESSING");
    set(&m, "/api/v1/order/create", d.clone());
    set(&m, "/api/v1/order/inquiry", d);
    assert_eq!(
        m.provider
            .checkout(&config, "demo", &o)
            .await
            .unwrap()
            .as_deref(),
        Some("https://cashier.waffo.com/fixture-key")
    );
    let restarted = SimulationConfig::default()
        .with_account_store_path(config.account_store_path.as_ref().unwrap())
        .with_billing_monthly_card_credit_price(Some(1000))
        .unwrap();
    assert!(m
        .provider
        .checkout(&restarted, "demo", &o)
        .await
        .unwrap()
        .is_some());
    let calls = m.fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "/api/v1/order/create");
    assert_eq!(calls[1].0, "/api/v1/order/inquiry");
    let (_, headers, raw) = &calls[0];
    assert_eq!(headers["x-api-version"], "1.0.0");
    assert_eq!(headers["x-api-key"], "api-key-fixture-only");
    protocol::verify(
        raw,
        headers["x-signature"].to_str().unwrap(),
        &m.provider.verifying_key,
    )
    .unwrap();
    let request: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(request["orderAmount"], "1.00");
    assert_eq!(request["orderCurrency"], "USD");
    assert_eq!(request["paymentRequestId"].as_str().unwrap().len(), 32);
    assert_eq!(request["merchantOrderId"].as_str().unwrap().len(), 64);
    assert_eq!(request["paymentInfo"]["productName"], "ONE_TIME_PAYMENT");
    assert!(request["paymentInfo"].get("payMethodName").is_none());
    assert_eq!(request["extendInfo"], m.provider.metadata("demo", &o));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waffo_unsigned_or_lost_create_result_recovers_paid_query_without_second_charge() {
    let m = mock().await;
    let config = config();
    let o = order(&config);
    set(
        &m,
        "/api/v1/order/create",
        data(&m, &o, "PAYMENT_PROCESSING"),
    );
    *m.fixture.unsigned.lock().unwrap() = true;
    assert_eq!(
        m.provider.checkout(&config, "demo", &o).await.unwrap_err(),
        "billingProviderSignatureInvalid"
    );
    assert_eq!(
        config
            .recharge_credit_status("demo", o.character_index)
            .unwrap(),
        (0, 0)
    );
    *m.fixture.unsigned.lock().unwrap() = false;
    set(&m, "/api/v1/order/inquiry", data(&m, &o, "PAY_SUCCESS"));
    assert!(m
        .provider
        .checkout(&config, "demo", &o)
        .await
        .unwrap()
        .is_none());
    let applied = config
        .consume_inactive_pending_recharge_credits("demo", o.character_index, monthly_card_now_ms())
        .unwrap();
    assert_eq!((applied.credit, applied.granted), (100, 100));
    assert!(m
        .provider
        .checkout(&config, "demo", &o)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        config
            .consume_inactive_pending_recharge_credits(
                "demo",
                o.character_index,
                monthly_card_now_ms()
            )
            .unwrap()
            .granted,
        0
    );
    assert_eq!(
        m.fixture
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.0 == "/api/v1/order/create")
            .count(),
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waffo_signed_callback_requeries_and_grants_once_while_bad_signature_never_queries() {
    let m = mock().await;
    let config = config();
    let o = order(&config);
    config
        .begin_recharge_provider_request("demo", &o.id, PROVIDER, monthly_card_now_ms())
        .unwrap();
    let paid = data(&m, &o, "PAY_SUCCESS");
    set(&m, "/api/v1/order/inquiry", paid.clone());
    let (raw, signature) = event(&m, "PAYMENT_NOTIFICATION", paid);
    let mut tampered = raw.clone();
    tampered.push(b' ');
    assert!(m
        .provider
        .webhook(&config, &signature, &tampered, monthly_card_now_ms())
        .await
        .is_err());
    assert!(m.fixture.calls.lock().unwrap().is_empty());
    for _ in 0..2 {
        m.provider
            .webhook(&config, &signature, &raw, monthly_card_now_ms())
            .await
            .unwrap();
    }
    assert_eq!(
        config
            .consume_inactive_pending_recharge_credits(
                "demo",
                o.character_index,
                monthly_card_now_ms()
            )
            .unwrap()
            .granted,
        100
    );
    m.provider
        .webhook(&config, &signature, &raw, monthly_card_now_ms())
        .await
        .unwrap();
    assert_eq!(
        config
            .consume_inactive_pending_recharge_credits(
                "demo",
                o.character_index,
                monthly_card_now_ms()
            )
            .unwrap()
            .granted,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waffo_signed_mismatches_and_authorization_only_never_grant() {
    let m = mock().await;
    let config = config();
    let o = order(&config);
    config
        .begin_recharge_provider_request("demo", &o.id, PROVIDER, monthly_card_now_ms())
        .unwrap();
    let paid = data(&m, &o, "PAY_SUCCESS");
    for (field_name, value) in [
        ("orderAmount", json!("1.01")),
        ("orderCurrency", json!("EUR")),
        ("merchantOrderId", json!("wrong")),
        ("paymentRequestId", json!("bad")),
        ("extendInfo", json!("{}")),
    ] {
        let mut invalid = paid.clone();
        invalid[field_name] = value;
        let (raw, signature) = event(&m, "PAYMENT_NOTIFICATION", invalid);
        assert!(
            m.provider
                .webhook(&config, &signature, &raw, monthly_card_now_ms())
                .await
                .is_err(),
            "{field_name}"
        );
    }
    let mut wrong_merchant = paid.clone();
    wrong_merchant["merchantInfo"]["merchantId"] = json!("another-merchant");
    let (raw, signature) = event(&m, "PAYMENT_NOTIFICATION", wrong_merchant);
    assert!(m
        .provider
        .webhook(&config, &signature, &raw, monthly_card_now_ms())
        .await
        .is_err());
    set(
        &m,
        "/api/v1/order/inquiry",
        data(&m, &o, "AUTHED_WAITING_CAPTURE"),
    );
    let (raw, signature) = event(&m, "PAYMENT_NOTIFICATION", paid.clone());
    m.provider
        .webhook(&config, &signature, &raw, monthly_card_now_ms())
        .await
        .unwrap();
    assert_eq!(
        config
            .recharge_credit_status("demo", o.character_index)
            .unwrap(),
        (0, 0)
    );
    let mut future = paid.clone();
    future["orderCompletedAt"] = json!(requested_at(monthly_card_now_ms() + 60_000).unwrap());
    set(&m, "/api/v1/order/inquiry", future);
    assert!(m
        .provider
        .webhook(&config, &signature, &raw, monthly_card_now_ms())
        .await
        .is_err());
    let mut requery_mismatch = paid;
    requery_mismatch["userInfo"]["userId"] = json!("someone-else");
    set(&m, "/api/v1/order/inquiry", requery_mismatch);
    assert!(m
        .provider
        .webhook(&config, &signature, &raw, monthly_card_now_ms())
        .await
        .is_err());
    assert_eq!(
        config
            .recharge_credit_status("demo", o.character_index)
            .unwrap(),
        (0, 0)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waffo_refund_before_payment_is_sticky_and_never_rewinds_a_save() {
    let m = mock().await;
    let config = config();
    let o = order(&config);
    config
        .begin_recharge_provider_request("demo", &o.id, PROVIDER, monthly_card_now_ms())
        .unwrap();
    let paid = data(&m, &o, "PAY_SUCCESS");
    set(&m, "/api/v1/order/inquiry", paid.clone());
    let (raw, signature) = event(
        &m,
        "REFUND_NOTIFICATION",
        json!({"origPaymentRequestId":payment_request_id(&o).unwrap(),"acquiringOrderId":"ACQ_fixture_123","refundRequestId":"refund-fixture","refundStatus":"ORDER_FULLY_REFUNDED","refundAmount":"1.00"}),
    );
    m.provider
        .webhook(&config, &signature, &raw, monthly_card_now_ms())
        .await
        .unwrap();
    let (raw, signature) = event(&m, "PAYMENT_NOTIFICATION", paid);
    m.provider
        .webhook(&config, &signature, &raw, monthly_card_now_ms())
        .await
        .unwrap();
    assert_eq!(
        config
            .recharge_order_for_payment("demo", &o.id)
            .unwrap()
            .state,
        mir2_simulation::billing::RechargeOrderState::Review
    );
    assert_eq!(
        config
            .consume_inactive_pending_recharge_credits(
                "demo",
                o.character_index,
                monthly_card_now_ms()
            )
            .unwrap()
            .granted,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waffo_malicious_signed_checkout_url_cannot_escape_cashier_origin() {
    let m = mock().await;
    let config = config();
    let o = order(&config);
    let mut d = data(&m, &o, "PAYMENT_PROCESSING");
    d["orderAction"] =
        json!(json!({"webUrl":"https://cashier.waffo.com.evil.test/attack"}).to_string());
    set(&m, "/api/v1/order/create", d);
    assert_eq!(
        m.provider.checkout(&config, "demo", &o).await.unwrap_err(),
        "billingCheckoutUrlInvalid"
    );
    assert_eq!(
        config
            .recharge_credit_status("demo", o.character_index)
            .unwrap(),
        (0, 0)
    );
    assert!(config
        .recharge_order_for_payment("demo", &o.id)
        .unwrap()
        .checkout_session_id
        .is_none());
    let ack = b"{\"message\":\"success\"}";
    protocol::verify(
        ack,
        &m.provider.sign_ack(ack).unwrap(),
        &m.provider.verifying_key,
    )
    .unwrap();
}

#[tokio::test]
async fn waffo_live_configuration_requires_durable_source_and_monthly_price() {
    let mut m = mock().await;
    assert!(m
        .provider
        .validate_store(&SimulationConfig::default())
        .is_err());
    let mut config = config();
    assert!(m.provider.validate_store(&config).is_ok());
    m.provider.livemode = true;
    assert_eq!(
        m.provider.validate_store(&config).unwrap_err(),
        "billingLiveSourceStoreRequired"
    );
    config.account_store_database_url = Some("postgres://fixture.invalid/never-opened".into());
    config.account_store_database_mode = mir2_simulation::AccountStoreDatabaseMode::SourceOfTruth;
    assert!(m.provider.validate_store(&config).is_ok());
}

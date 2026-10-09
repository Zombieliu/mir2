//! Local billing boundary tests; no provider configuration or external network is used.
use super::*;
use crate::cache::{GatewaySessionCache, InMemoryGatewaySessionCache};

const OPERATIONS: [&str; 4] = [
    "status",
    "checkout",
    "buyMonthlyCard",
    "activateMonthlyCard",
];

fn request_json(operation: &str) -> Value {
    let mut value = json!({"characterIndex": 7, "requestId": u64::MAX});
    match operation {
        "checkout" => {
            value["orderRequestId"] = json!("native-billing-test-001");
            value["offerId"] = json!("server-owned-offer");
        }
        "buyMonthlyCard" => value["orderRequestId"] = json!("native-billing-test-002"),
        "activateMonthlyCard" => value["itemUniqueId"] = json!(u64::MAX.to_string()),
        "status" => {}
        _ => panic!("unknown test operation"),
    }
    value
}

fn decode_request(operation: &str, value: Value) -> Result<Request, serde_json::Error> {
    match operation {
        "status" => serde_json::from_value(value).map(Request::Status),
        "checkout" => serde_json::from_value(value).map(Request::Checkout),
        "buyMonthlyCard" => serde_json::from_value(value).map(Request::Buy),
        "activateMonthlyCard" => serde_json::from_value(value).map(Request::Activate),
        _ => panic!("unknown test operation"),
    }
}

#[test]
fn strict_requests_accept_server_catalogue_identifiers_and_exact_u64_ids() {
    for operation in OPERATIONS {
        let request = decode_request(operation, request_json(operation)).expect("valid request");
        assert_eq!(request.operation(), operation);
        assert_eq!(request.character(), 7);
        assert_eq!(request.request_id(), u64::MAX);
        if let Request::Activate(activate) = request {
            assert_eq!(activate.item_unique_id, "18446744073709551615");
        }
    }
}

#[test]
fn strict_requests_reject_client_account_and_financial_authority() {
    for operation in OPERATIONS {
        for field in [
            "accountId",
            "account",
            "credits",
            "credit",
            "price",
            "priceId",
            "amountMinor",
            "currency",
            "paid",
            "monthlyCardCreditPrice",
            "metadata",
            "character_index",
        ] {
            let mut value = request_json(operation);
            value[field] = json!("untrusted-client-value");
            assert!(
                decode_request(operation, value).is_err(),
                "{operation} must reject extra {field}"
            );
        }
    }
}

#[test]
fn strict_requests_require_all_fields_and_reject_numeric_item_ids() {
    for operation in OPERATIONS {
        let value = request_json(operation);
        for field in value.as_object().expect("request object").keys() {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                decode_request(operation, missing).is_err(),
                "missing {field}"
            );
        }
        for (field, invalid) in [
            ("characterIndex", json!("7")),
            ("requestId", json!("42")),
            ("requestId", json!(-1)),
            ("requestId", Value::Null),
        ] {
            let mut invalid_request = value.clone();
            invalid_request[field] = invalid;
            assert!(decode_request(operation, invalid_request).is_err());
        }
    }
    for invalid in [
        json!(9_007_199_254_740_993_u64),
        Value::Null,
        json!({"id": 1}),
    ] {
        let mut value = request_json("activateMonthlyCard");
        value["itemUniqueId"] = invalid;
        assert!(decode_request("activateMonthlyCard", value).is_err());
    }
    for operation in ["checkout", "buyMonthlyCard"] {
        let mut value = request_json(operation);
        value["orderRequestId"] = json!(17);
        assert!(decode_request(operation, value).is_err());
    }
}

#[test]
fn strict_checkout_rejects_duplicate_correlation_and_offer_fields() {
    for duplicate in [
        "\"characterIndex\":8",
        "\"requestId\":8",
        "\"orderRequestId\":\"another-order-id\"",
        "\"offerId\":\"another-offer\"",
    ] {
        let body = format!(
            "{{\"characterIndex\":7,\"requestId\":42,\"orderRequestId\":\"native-billing-test-001\",\"offerId\":\"server-offer\",{duplicate}}}"
        );
        assert!(serde_json::from_str::<CheckoutRequest>(&body).is_err());
    }
}

#[test]
fn error_replies_keep_operation_character_and_exact_request_correlation() {
    for operation in OPERATIONS {
        let request = decode_request(operation, request_json(operation)).unwrap();
        assert_eq!(
            error_reply(&request, "billingCharacterMismatch".into()),
            json!({
                "type": "billing",
                "payload": {
                    "operation": operation,
                    "requestId": u64::MAX,
                    "characterIndex": 7,
                    "error": "billingCharacterMismatch"
                }
            })
        );
        let success = reply(
            &request,
            Ok(json!({"status": {"credit": 18}, "replayed": true})),
        );
        assert_eq!(success["payload"]["operation"], operation);
        assert_eq!(success["payload"]["requestId"].as_u64(), Some(u64::MAX));
        assert_eq!(success["payload"]["characterIndex"], 7);
        assert_eq!(success["payload"]["status"]["credit"], 18);
        assert_eq!(success["payload"]["replayed"], true);
    }
}

#[test]
fn public_errors_preserve_definitive_codes_and_hide_unknown_outcomes() {
    for (internal, public) in [
        ("monthlyCardAccountBanned", "billingAccountBanned"),
        ("monthlyCardCharacterMissing", "billingCharacterMissing"),
        ("billingCharacterSaveMissing", "billingCharacterMissing"),
        ("monthlyCardRequestIdInvalid", "billingRequestIdInvalid"),
        ("monthlyCardInsufficientCredit", "billingInsufficientCredit"),
        ("monthlyCardShopUnavailable", "billingMonthlyCardDisabled"),
        ("monthlyCardItemMissing", "billingItemMissing"),
        ("monthlyCardItemInvalid", "billingItemInvalid"),
        (
            "billingAuthenticationRequired",
            "billingAuthenticationRequired",
        ),
        ("billingCharacterMismatch", "billingCharacterMismatch"),
        ("billingOfferMissing", "billingOfferMissing"),
        ("billingUnavailable", "billingUnavailable"),
        ("billingInsufficientCredits", "billingInsufficientCredits"),
        ("billingTooManyRequests", "billingTooManyRequests"),
        ("billingOrderMismatch", "billingOrderMismatch"),
        ("billingCharacterOnline", "billingCharacterOnline"),
    ] {
        assert_eq!(public_error(internal), public);
    }
    let request = decode_request("checkout", request_json("checkout")).unwrap();
    for unknown in [
        "billingProviderResultUnconfirmed",
        "billingResultUnconfirmed",
        "billingCommitOutcomeUnknown",
        "billingServiceUnavailable",
        "Source RPC response lost after commit; internal-only diagnostic",
    ] {
        assert_eq!(public_error(unknown), "billingResultUnconfirmed");
        assert_eq!(
            error_reply(&request, unknown.into())["payload"]["error"],
            "billingResultUnconfirmed"
        );
    }
}

fn issue_identity(
    identity: &IdentityService,
    account: &str,
) -> (
    crate::identity::IdentitySessionGrant,
    VerifiedIdentitySession,
) {
    let grant = identity
        .issue_session(
            account,
            "password",
            account,
            "127.0.0.1",
            "billing-local-test",
        )
        .expect("local identity session should issue");
    let verified = identity
        .verify_session_token(&grant.token)
        .expect("signed local identity");
    (grant, verified)
}

#[test]
fn verified_account_requires_current_authenticated_matching_real_identity() {
    let identity = IdentityService::local_for_tests();
    let (_, verified) = issue_identity(&identity, "billing_alice");
    assert_eq!(
        verified_account(&identity, Some(&verified), true, Some("billing_alice")),
        Ok("billing_alice".into())
    );
    for (cached, authenticated, account) in [
        (Some(&verified), false, Some("billing_alice")),
        (None, true, Some("billing_alice")),
        (Some(&verified), true, None),
        (Some(&verified), true, Some("billing_bob")),
    ] {
        assert_eq!(
            verified_account(&identity, cached, authenticated, account),
            Err("billingAuthenticationRequired".into())
        );
    }
    // Matching caller strings cannot turn Alice's stored session into Bob's identity.
    let mut wrong_account = verified.clone();
    wrong_account.account_id = "billing_bob".into();
    assert_eq!(
        verified_account(&identity, Some(&wrong_account), true, Some("billing_bob")),
        Err("billingAuthenticationRequired".into())
    );
    let unknown_store = IdentityService::local_for_tests();
    assert_eq!(
        verified_account(&unknown_store, Some(&verified), true, Some("billing_alice")),
        Err("billingAuthenticationRequired".into())
    );
}

#[test]
fn verified_account_rejects_expired_cached_snapshot_and_real_revocation() {
    let identity = IdentityService::local_for_tests();
    let (grant, verified) = issue_identity(&identity, "billing_alice");
    // Exercise the cached expiry guard without changing global TTL or waiting for it.
    for expires_at_ms in [0, monthly_card_now_ms()] {
        let mut expired = verified.clone();
        expired.expires_at_ms = expires_at_ms;
        assert_eq!(
            verified_account(&identity, Some(&expired), true, Some("billing_alice")),
            Err("billingAuthenticationRequired".into())
        );
    }
    assert!(identity
        .revoke_session(&verified, &verified.session_id, "local logout")
        .unwrap());
    assert!(identity.verify_session_token(&grant.token).is_err());
    assert_eq!(
        verified_account(&identity, Some(&verified), true, Some("billing_alice")),
        Err("billingAuthenticationRequired".into())
    );
}

#[tokio::test]
async fn http_identity_verifies_real_bearer_and_rechecks_revocation() {
    let identity = Arc::new(IdentityService::local_for_tests());
    let (grant, verified) = issue_identity(&identity, "billing_alice");
    for authorization in [
        None,
        Some("Basic ignored"),
        Some("Bearer malformed"),
        Some("Bearer "),
    ] {
        let mut headers = HeaderMap::new();
        if let Some(value) = authorization {
            headers.insert(AUTHORIZATION, value.parse().unwrap());
        }
        assert_eq!(
            http_identity(&identity, &headers).await,
            Err("billingAuthenticationRequired".into())
        );
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        format!("Bearer {}", grant.token).parse().unwrap(),
    );
    assert_eq!(
        http_identity(&identity, &headers).await,
        Ok(verified.clone())
    );
    assert!(identity
        .revoke_session(&verified, &verified.session_id, "HTTP logout")
        .unwrap());
    assert_eq!(
        http_identity(&identity, &headers).await,
        Err("billingAuthenticationRequired".into())
    );
}

#[test]
fn shared_rate_helper_has_exact_per_account_operation_budget() {
    let cache = InMemoryGatewaySessionCache::default();
    for operation in OPERATIONS {
        let budget = if operation == "status" { 120 } else { 20 };
        for _ in 0..budget {
            assert_eq!(rate_limit(&cache, "billing_alice", operation), Ok(()));
        }
        assert_eq!(
            rate_limit(&cache, "billing_alice", operation),
            Err("billingTooManyRequests".into())
        );
        assert_eq!(rate_limit(&cache, "billing_bob", operation), Ok(()));
    }
}

#[test]
fn inactive_lease_blocks_other_writers_and_releases_only_confirmed_results() {
    let cache = InMemoryGatewaySessionCache::default();
    for result in [
        Ok(json!({"status": {"credit": 5}})),
        Err("billingInsufficientCredit".into()),
        Err("monthlyCardItemMissing".into()),
    ] {
        let lease = inactive_lease(&cache, "billing_alice", 7).expect("inactive character lease");
        assert!(lease.1.starts_with("billing-offline-"));
        assert_eq!(cache.route_lease_count(), 1);
        assert_eq!(
            inactive_lease(&cache, "billing_alice", 7),
            Err("billingCharacterOnline".into())
        );
        assert!(cache
            .acquire_route_lease(&lease.0, "ordinary-socket", 30)
            .is_err());
        release_confirmed_lease(&cache, Some(lease.clone()), &result).unwrap();
        assert_eq!(cache.route_lease_count(), 0);
        cache
            .acquire_route_lease(&lease.0, "ordinary-socket", 30)
            .unwrap();
        cache
            .release_route_lease(&lease.0, "ordinary-socket")
            .unwrap();
    }
}

#[test]
fn inactive_lease_retains_route_fence_when_source_commit_outcome_is_unknown() {
    let cache = InMemoryGatewaySessionCache::default();
    for error in [
        "billingResultUnconfirmed",
        "billingProviderResultUnconfirmed",
        "billingCommitOutcomeUnknown",
        "billingServiceUnavailable",
        "Source RPC transport closed after dispatch",
    ] {
        let lease = inactive_lease(&cache, "billing_alice", 7).unwrap();
        let result: Result<Value, String> = Err(error.into());
        release_confirmed_lease(&cache, Some(lease.clone()), &result).unwrap();
        assert_eq!(cache.route_lease_count(), 1);
        cache
            .release_route_lease(&lease.0, "unrelated-socket")
            .unwrap();
        assert_eq!(
            cache.route_lease_count(),
            1,
            "an unrelated owner cannot release the fence"
        );
        assert!(cache
            .acquire_route_lease(&lease.0, "ordinary-socket", 30)
            .is_err());
        assert_eq!(
            inactive_lease(&cache, "billing_alice", 7),
            Err("billingCharacterOnline".into())
        );
        let other = inactive_lease(&cache, "billing_alice", 8).unwrap();
        cache.release_route_lease(&other.0, &other.1).unwrap();
        cache.release_route_lease(&lease.0, &lease.1).unwrap();
        assert_eq!(cache.route_lease_count(), 0);
    }
    release_confirmed_lease(&cache, None, &Ok(json!({}))).unwrap();
}

#[test]
fn inactive_lease_rejects_existing_online_character_without_removing_it() {
    let cache = InMemoryGatewaySessionCache::default();
    let record: GatewaySessionCacheRecord = serde_json::from_value(json!({
        "key": {"accountId": "billing_alice", "characterIndex": 7},
        "characterName": "BillingScout",
        "gatewaySessionId": "ordinary-live-session",
        "mapFileName": "0",
        "playerObjectId": 1001,
        "playerHp": 18,
        "playerMaxHp": 18,
        "gold": 5,
        "tick": 7
    }))
    .expect("ordinary session cache record");
    cache.put(record.clone());
    assert_eq!(
        inactive_lease(&cache, "billing_alice", 7),
        Err("billingCharacterOnline".into())
    );
    assert_eq!(cache.get(&record.key), Some(record));
    assert_eq!(cache.route_lease_count(), 0);
}

#[test]
fn dropping_socket_fence_invalidates_delayed_generation_even_after_reauthentication() {
    let authenticated = Arc::new(AtomicBool::new(true));
    let generation = Arc::new(AtomicU64::new(41));
    let captured_generation = generation.load(Ordering::Acquire);
    let fence = socket_fence(Arc::clone(&authenticated), Arc::clone(&generation));
    assert!(authenticated.load(Ordering::Acquire));
    drop(fence);
    assert!(!authenticated.load(Ordering::Acquire));
    assert_eq!(generation.load(Ordering::Acquire), 42);
    authenticated.store(true, Ordering::Release);
    assert_ne!(generation.load(Ordering::Acquire), captured_generation);
    drop(socket_fence(
        Arc::clone(&authenticated),
        Arc::clone(&generation),
    ));
    assert_eq!(generation.load(Ordering::Acquire), 43);
    assert!(!authenticated.load(Ordering::Acquire));
}

fn web_state(config: GatewayConfig) -> WebState {
    let disabled_data_dir = std::env::temp_dir().join("mir2-billing-web-tests-disabled");
    WebState {
        config: Arc::new(config),
        deploy_revision: None,
        zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
        chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
        session_cache: Arc::new(InMemoryGatewaySessionCache::default()),
        reconnect_sessions: Arc::new(ReconnectSessionStore::default()),
        capacity: Arc::new(GatewayCapacityState::unlimited()),
        gameplay_event_sink: None,
        identity: Arc::new(IdentityService::local_for_tests()),
        injector: crate::inject::LiveSessionInjector::default(),
        spectator: crate::spectator::SpectatorHub::new(crate::spectator::SpectatorConfig {
            enabled: false,
            recording_enabled: false,
            public_enabled: false,
            public_maps: Vec::new(),
            director_token: None,
            capture_interval_ms: 250,
            public_delay_ms: 30_000,
            max_delay_ms: 120_000,
            ring_frames: 40,
            max_entities: 16,
            replay_limit: 100,
            retention_hours: 1,
            entity_stale_ms: 15_000,
            data_dir: disabled_data_dir.clone(),
        }),
        ai_live: crate::ai_live::AiLiveHub::new(crate::ai_live::AiLiveConfig::disabled_for_tests(
            disabled_data_dir,
        ))
        .expect("disabled local AI hub"),
        channel_identity: crate::ChannelIdentityRegistry::in_memory(),
    }
}

struct LocalHttp {
    url: String,
    client: reqwest::Client,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for LocalHttp {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn local_http(state: WebState) -> LocalHttp {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("isolated HTTP listener");
    let address = listener.local_addr().unwrap();
    let app = router().with_state(state);
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("local billing router should serve");
    });
    LocalHttp {
        url: format!("http://{address}"),
        client: reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        server,
    }
}

async fn assert_private_error(response: reqwest::Response, status: StatusCode, error: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({"error": error})
    );
}

#[tokio::test]
async fn http_router_rejects_unauthenticated_and_revoked_identity_with_no_store() {
    let state = web_state(GatewayConfig::default());
    let (grant, verified) = issue_identity(&state.identity, "billing_alice");
    let http = local_http(state.clone()).await;
    assert!(state
        .identity
        .revoke_session(&verified, &verified.session_id, "route logout")
        .unwrap());
    for token in [None, Some(grant.token.as_str())] {
        let mut status = http.client.get(format!(
            "{}/v1/billing/status?characterIndex=7&requestId=42",
            http.url
        ));
        let mut checkout = http
            .client
            .post(format!("{}/v1/billing/checkout", http.url))
            .json(&request_json("checkout"));
        if let Some(token) = token {
            status = status.bearer_auth(token);
            checkout = checkout.bearer_auth(token);
        }
        assert_private_error(
            status.send().await.unwrap(),
            StatusCode::UNAUTHORIZED,
            "billingAuthenticationRequired",
        )
        .await;
        assert_private_error(
            checkout.send().await.unwrap(),
            StatusCode::UNAUTHORIZED,
            "billingAuthenticationRequired",
        )
        .await;
    }
    assert_eq!(state.session_cache.route_lease_count(), 0);
}

#[tokio::test]
async fn http_router_rejects_body_query_injection_and_duplicate_fields_before_execution() {
    let state = web_state(GatewayConfig::default());
    let account_store = Arc::clone(&state.config.account_store);
    let before = serde_json::to_value(&*account_store.lock().unwrap()).unwrap();
    let http = local_http(state.clone()).await;
    for (field, value) in [
        ("accountId", json!("other-account")),
        ("credits", json!(u32::MAX)),
        ("priceId", json!("client-selected-price")),
        ("amountMinor", json!(1)),
        ("currency", json!("usd")),
    ] {
        let mut body = request_json("checkout");
        body[field] = value;
        let response = http
            .client
            .post(format!("{}/v1/billing/checkout", http.url))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    let duplicate = "{\"characterIndex\":7,\"requestId\":42,\"orderRequestId\":\"native-billing-test-001\",\"offerId\":\"server-offer\",\"characterIndex\":8}";
    let response = http
        .client
        .post(format!("{}/v1/billing/checkout", http.url))
        .header(CONTENT_TYPE, "application/json")
        .body(duplicate)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    for suffix in [
        "&accountId=other-account",
        "&credit=999",
        "&paid=true",
        "&characterIndex=8",
    ] {
        let response = http
            .client
            .get(format!(
                "{}/v1/billing/status?characterIndex=7&requestId=42{suffix}",
                http.url
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert_eq!(
        serde_json::to_value(&*account_store.lock().unwrap()).unwrap(),
        before
    );
    assert_eq!(state.session_cache.route_lease_count(), 0);
}

#[tokio::test]
async fn authenticated_http_status_and_checkout_fail_closed_without_durable_store() {
    let state = web_state(GatewayConfig::default());
    let (grant, _) = issue_identity(&state.identity, "billing_alice");
    let http = local_http(state).await;
    let status = http
        .client
        .get(format!(
            "{}/v1/billing/status?characterIndex=7&requestId=42",
            http.url
        ))
        .bearer_auth(&grant.token)
        .send()
        .await
        .unwrap();
    assert_private_error(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "billingUnavailable",
    )
    .await;
    let checkout = http
        .client
        .post(format!("{}/v1/billing/checkout", http.url))
        .bearer_auth(&grant.token)
        .json(&request_json("checkout"))
        .send()
        .await
        .unwrap();
    assert_private_error(
        checkout,
        StatusCode::SERVICE_UNAVAILABLE,
        "billingUnavailable",
    )
    .await;
}

#[tokio::test]
async fn http_and_socket_helper_share_the_same_account_operation_rate_bucket() {
    let mut config = GatewayConfig::default();
    // A path marks durable configuration; the exhausted rate bucket returns before
    // provider configuration or persistence, so this path is never accessed.
    config.account_store_path =
        Some(std::env::temp_dir().join("mir2-billing-rate-never-accessed.json"));
    let state = web_state(config);
    let (grant, _) = issue_identity(&state.identity, "billing_alice");
    for _ in 0..120 {
        rate_limit(state.session_cache.as_ref(), "billing_alice", "status").unwrap();
    }
    for _ in 0..20 {
        rate_limit(state.session_cache.as_ref(), "billing_alice", "checkout").unwrap();
    }
    let http = local_http(state.clone()).await;
    let status = http
        .client
        .get(format!(
            "{}/v1/billing/status?characterIndex=7&requestId=42",
            http.url
        ))
        .bearer_auth(&grant.token)
        .send()
        .await
        .unwrap();
    assert_private_error(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "billingTooManyRequests",
    )
    .await;
    let checkout = http
        .client
        .post(format!("{}/v1/billing/checkout", http.url))
        .bearer_auth(&grant.token)
        .json(&request_json("checkout"))
        .send()
        .await
        .unwrap();
    assert_private_error(
        checkout,
        StatusCode::TOO_MANY_REQUESTS,
        "billingTooManyRequests",
    )
    .await;
    assert_eq!(
        rate_limit(
            state.session_cache.as_ref(),
            "billing_alice",
            "buyMonthlyCard"
        ),
        Ok(())
    );
    assert_eq!(
        rate_limit(state.session_cache.as_ref(), "billing_bob", "status"),
        Ok(())
    );
}

#[tokio::test]
async fn unsigned_webhook_and_cosmetic_return_never_grant_credit_or_change_accounts() {
    let state = web_state(GatewayConfig::default());
    let account_store = Arc::clone(&state.config.account_store);
    let before = serde_json::to_value(&*account_store.lock().unwrap()).unwrap();
    let http = local_http(state.clone()).await;
    let response = http
        .client
        .post(format!("{}/v1/billing/stripe/webhook", http.url))
        .header(CONTENT_TYPE, "application/json")
        .body("{\"type\":\"checkout.session.completed\",\"paid\":true,\"credits\":4294967295}")
        .send()
        .await
        .unwrap();
    assert_private_error(
        response,
        StatusCode::BAD_REQUEST,
        "billingWebhookSignatureInvalid",
    )
    .await;
    let mut original_body = None;
    for query in [
        "",
        "?paid=true&accountId=demo&characterIndex=0&credits=4294967295",
        "?session_id=cs_test_cosmetic&payment_status=paid&orderId=client-order",
        "?paid=false&cancelled=true",
    ] {
        let response = http
            .client
            .get(format!("{}/v1/billing/return{query}", http.url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );
        assert!(response
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'"));
        let body = response.text().await.unwrap();
        assert!(body.contains("Check status"));
        if let Some(original) = &original_body {
            assert_eq!(
                &body, original,
                "query claims must not change the return page"
            );
        } else {
            original_body = Some(body);
        }
        assert_eq!(
            serde_json::to_value(&*account_store.lock().unwrap()).unwrap(),
            before
        );
    }
    assert_eq!(state.session_cache.route_lease_count(), 0);
    assert!(state.session_cache.list().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_websocket_billing_recovers_expired_entry_and_replays_offline_and_online() {
    use mir2_simulation::monthly_card::{MonthlyCardPolicy, MONTHLY_CARD_DURATION_MS};
    use mir2_simulation::{AccountStoreRepository, FileAccountStoreRepository};
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as WireMessage};

    type Socket = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;
    const ACCOUNT: &str = "billing_ws_fixture";
    const PASSWORD: &str = "billing-ws-pass";
    const INITIAL_CREDIT: u32 = 100;
    const PRICE: u32 = 10;
    const KEY: &str = "billing-ws-test-only-issuance-key-do-not-deploy";

    struct FixtureDirectory(std::path::PathBuf);
    impl Drop for FixtureDirectory {
        fn drop(&mut self) {
            // This directory is created exclusively for this test before this
            // guard exists; no configured or user-owned path can reach it.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn send(socket: &mut Socket, command: Value) {
        socket
            .send(WireMessage::Text(command.to_string().into()))
            .await
            .expect("ordinary browser command should send");
    }

    async fn read_until(
        socket: &mut Socket,
        label: &str,
        predicate: impl Fn(&Value) -> bool,
    ) -> (Value, Vec<Value>) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut observed = Vec::new();
        let mut summaries = std::collections::BTreeMap::<String, usize>::new();
        // Ordinary StartGame streams the full item/quest/shop bootstrap before
        // the next correlated reply. The prior 512-frame fixture budget cut
        // that valid FIFO stream short. Keep the ten-second deadline and a
        // finite budget; retain all financial and packet-order assertions.
        for _ in 0..4096 {
            let frame = tokio::time::timeout_at(deadline, socket.next())
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for {label}: {summaries:?}"))
                .unwrap_or_else(|| panic!("socket closed waiting for {label}"))
                .unwrap_or_else(|_| panic!("socket transport failed waiting for {label}"));
            match frame {
                WireMessage::Text(text) => {
                    let event: Value = serde_json::from_str(text.as_ref())
                        .expect("ordinary gateway frame should be JSON");
                    // Failure diagnostics never print identity tokens, full
                    // world payloads, or a possible Checkout URL.
                    let kind = format!(
                        "{}:{}:{}",
                        event["type"].as_str().unwrap_or("?"),
                        event["packet"].as_str().unwrap_or(""),
                        event["payload"]["requestId"]
                            .as_u64()
                            .map_or(String::new(), |id| id.to_string())
                    );
                    *summaries.entry(kind).or_default() += 1;
                    if event["type"] == "error" {
                        panic!("gateway rejected {label}; frame kinds={summaries:?}");
                    }
                    observed.push(event.clone());
                    if predicate(&event) {
                        return (event, observed);
                    }
                }
                WireMessage::Close(_) => panic!("socket closed waiting for {label}"),
                _ => {}
            }
        }
        panic!("frame budget exhausted waiting for {label}: {summaries:?}");
    }

    async fn billing(socket: &mut Socket, command: Value, operation: &str) -> (Value, Vec<Value>) {
        let request_id = command["requestId"].as_u64().unwrap();
        let character = command["characterIndex"].as_i64().unwrap();
        send(socket, command).await;
        let (event, observed) = read_until(socket, operation, |event| {
            event["type"] == "billing" && event["payload"]["requestId"].as_u64() == Some(request_id)
        })
        .await;
        assert_eq!(event["payload"]["operation"], operation);
        assert_eq!(event["payload"]["characterIndex"].as_i64(), Some(character));
        (event["payload"].clone(), observed)
    }

    fn successful_status(payload: &Value) -> &Value {
        assert!(
            payload.get("error").is_none(),
            "billing operation failed with public code {}",
            payload["error"].as_str().unwrap_or("?")
        );
        assert!(payload["status"].is_object());
        &payload["status"]
    }

    fn packet(event: &Value, name: &str) -> bool {
        event["type"] == "packet" && event["packet"] == name
    }

    fn unique_card(status: &Value) -> String {
        let cards = status["ownedMonthlyCards"].as_array().unwrap();
        assert_eq!(cards.len(), 1);
        let id = cards[0]["uniqueId"]
            .as_str()
            .expect("UID must be a decimal string");
        let numeric = id.parse::<u64>().expect("valid exact UID");
        assert!(numeric > u64::from(u32::MAX));
        assert_eq!(numeric.to_string(), id);
        id.to_owned()
    }

    let directory = std::env::temp_dir().join(format!(
        "mir2-billing-ws-lifecycle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).expect("create exclusive billing fixture directory");
    let fixture = FixtureDirectory(directory);
    let path = fixture.0.join("accounts.json");
    let config = GatewayConfig::default()
        .with_billing_monthly_card_credit_price(Some(PRICE))
        .unwrap()
        .with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap());
    let default_character = config.default_character.clone();
    let character = default_character.index;
    {
        let mut store = config.account_store.lock().unwrap();
        let mut account = store
            .accounts
            .remove("demo")
            .expect("ordinary starter account");
        account.password = PASSWORD.into();
        // Explicit prepared wallet fixture. This is neither an earned-credit
        // test nor a Stripe callback/provider/payment acceptance receipt.
        account.saves.get_mut(&character).unwrap().credit = INITIAL_CREDIT;
        store.accounts.insert(ACCOUNT.into(), account);
    }
    // Ordinary trusted Source issuance/redemption in the past prepares expired
    // account access without editing ledger timestamps or bypassing entry guards.
    let expired_started_at = monthly_card_now_ms() - MONTHLY_CARD_DURATION_MS - 60_000;
    let expired = config
        .issue_monthly_card(ACCOUNT, "billing-ws-expired-fixture", expired_started_at)
        .unwrap();
    config
        .redeem_monthly_card(ACCOUNT, &expired.code, expired_started_at)
        .unwrap();
    assert!(
        !config
            .monthly_card_status(ACCOUNT, monthly_card_now_ms())
            .unwrap()
            .can_enter_game
    );
    let repository = FileAccountStoreRepository::new(&path);
    repository
        .save(&config.account_store.lock().unwrap())
        .unwrap();
    let config = config
        .with_account_store_path(&path)
        // Dedicated deterministic fixture key, separate from issuance. Keep
        // the normal save-recovery MAC requirement active during password login.
        .with_save_recovery_mac_key(std::array::from_fn::<_, 32, _>(|index| {
            (index as u8).wrapping_mul(7)
        }))
        .unwrap();
    assert_eq!(
        config.recharge_credit_status(ACCOUNT, character).unwrap(),
        (INITIAL_CREDIT, 0)
    );
    let state = web_state(config);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/ws", get(ws_upgrade))
        .merge(router())
        .with_state(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("ordinary local billing WebSocket router");
    });
    let mut local = LocalHttp {
        url: format!("http://{address}"),
        client: reqwest::Client::builder().no_proxy().build().unwrap(),
        server,
    };
    let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
    let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
        .ok()
        .and_then(|origins| {
            origins
                .split(',')
                .map(str::trim)
                .find(|s| !s.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| local.url.clone());
    request
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    let (mut socket, upgrade) = tokio_tungstenite::connect_async(request).await.unwrap();
    assert_eq!(upgrade.status(), StatusCode::SWITCHING_PROTOCOLS);

    let (unauthenticated, _) = billing(
        &mut socket,
        json!({
            "type": "billingCheckout", "characterIndex": character, "requestId": 1,
            "orderRequestId": "billing-ws-unauth-checkout", "offerId": "fixture-no-provider-offer"
        }),
        "checkout",
    )
    .await;
    assert_eq!(unauthenticated["error"], "billingAuthenticationRequired");
    send(
        &mut socket,
        json!({"type":"login", "accountId":ACCOUNT, "password":PASSWORD}),
    )
    .await;
    let (_, login_events) = read_until(&mut socket, "ordinary password LoginSuccess", |event| {
        packet(event, "LoginSuccess")
    })
    .await;
    let grant = login_events
        .iter()
        .find(|event| event["type"] == "identitySession")
        .expect("password login must issue a real identity session");
    let verified = state
        .identity
        .verify_session_token(grant["token"].as_str().unwrap())
        .expect("actual returned signed identity");
    assert_eq!(verified.account_id, ACCOUNT);
    let (initial, _) = billing(
        &mut socket,
        json!({"type":"billingStatus", "characterIndex":character, "requestId":2}),
        "status",
    )
    .await;
    let initial = successful_status(&initial);
    assert_eq!(initial["credit"], INITIAL_CREDIT);
    assert_eq!(initial["pendingCredit"], 0);
    assert_eq!(initial["monthlyCardCreditPrice"], PRICE);
    assert_eq!(initial["monthlyCard"]["required"], true);
    assert_eq!(initial["monthlyCard"]["active"], false);
    assert_eq!(initial["monthlyCard"]["canEnterGame"], false);
    assert!(initial["ownedMonthlyCards"].as_array().unwrap().is_empty());

    // An authenticated absent offer must fail before any provider transport.
    let absent_offer = "billing-ws-absent-provider-offer";
    assert!(!initial["offers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|offer| offer["id"] == absent_offer));
    let checkout_enabled = initial["checkoutEnabled"].as_bool().unwrap();
    let (checkout, _) = billing(
        &mut socket,
        json!({
            "type":"billingCheckout", "characterIndex":character, "requestId":3,
            "orderRequestId":"billing-ws-absent-checkout", "offerId":absent_offer
        }),
        "checkout",
    )
    .await;
    assert_eq!(
        checkout["error"],
        if checkout_enabled {
            "billingOfferMissing"
        } else {
            "billingUnavailable"
        }
    );

    send(
        &mut socket,
        json!({"type":"startGame", "characterIndex":character}),
    )
    .await;
    let (denied, _) = read_until(&mut socket, "expired StartGame refusal", |event| {
        packet(event, "StartGame")
    })
    .await;
    assert_eq!(denied["payload"]["result"], 2);
    send(&mut socket, json!({"type":"keepAlive", "time":91001})).await;
    read_until(&mut socket, "entry refusal cleanup barrier", |event| {
        packet(event, "KeepAlive") && event["payload"]["time"] == 91001
    })
    .await;
    assert_eq!(state.capacity.status().current_active_sessions, 0);

    let offline_order = "billing-ws-offline-purchase-001";
    let (bought, _) = billing(&mut socket, json!({
        "type":"billingBuyMonthlyCard", "characterIndex":character, "requestId":4, "orderRequestId":offline_order
    }), "buyMonthlyCard").await;
    let bought = successful_status(&bought);
    let first_uid = unique_card(bought);
    assert_eq!(bought["credit"], INITIAL_CREDIT - PRICE);
    assert_eq!(bought["monthlyCard"]["canEnterGame"], false);
    let (repeated, events) = billing(&mut socket, json!({
        "type":"billingBuyMonthlyCard", "characterIndex":character, "requestId":5, "orderRequestId":offline_order
    }), "buyMonthlyCard").await;
    let repeated = successful_status(&repeated);
    assert_eq!(repeated["credit"], INITIAL_CREDIT - PRICE);
    assert_eq!(unique_card(repeated), first_uid);
    assert!(!events
        .iter()
        .any(|event| packet(event, "LoseCredit") || packet(event, "GainedItem")));
    let (activated, _) = billing(&mut socket, json!({
        "type":"billingActivateMonthlyCard", "characterIndex":character, "requestId":6, "itemUniqueId":first_uid
    }), "activateMonthlyCard").await;
    let activated = successful_status(&activated);
    let first_expiry = activated["monthlyCard"]["expiresAtMs"].as_u64().unwrap();
    assert_eq!(activated["credit"], INITIAL_CREDIT - PRICE);
    assert_eq!(activated["monthlyCard"]["active"], true);
    assert_eq!(activated["monthlyCard"]["canEnterGame"], true);
    assert!(activated["ownedMonthlyCards"]
        .as_array()
        .unwrap()
        .is_empty());
    let (repeated, _) = billing(&mut socket, json!({
        "type":"billingActivateMonthlyCard", "characterIndex":character, "requestId":7, "itemUniqueId":first_uid
    }), "activateMonthlyCard").await;
    assert_eq!(
        successful_status(&repeated)["monthlyCard"]["expiresAtMs"],
        first_expiry
    );

    send(
        &mut socket,
        json!({"type":"startGame", "characterIndex":character}),
    )
    .await;
    let (started, _) = read_until(&mut socket, "renewed ordinary StartGame", |event| {
        packet(event, "StartGame")
    })
    .await;
    assert_eq!(started["payload"]["result"], 4);
    let (online, _) = billing(
        &mut socket,
        json!({"type":"billingStatus", "characterIndex":character, "requestId":8}),
        "status",
    )
    .await;
    let online = successful_status(&online);
    assert_eq!(online["credit"], INITIAL_CREDIT - PRICE);
    assert_eq!(online["monthlyCard"]["expiresAtMs"], first_expiry);
    assert_eq!(state.capacity.status().current_active_sessions, 1);
    let online_order = "billing-ws-online-purchase-001";
    let (bought, events) = billing(&mut socket, json!({
        "type":"billingBuyMonthlyCard", "characterIndex":character, "requestId":9, "orderRequestId":online_order
    }), "buyMonthlyCard").await;
    let bought = successful_status(&bought);
    let second_uid = unique_card(bought);
    assert_ne!(second_uid, first_uid);
    assert_eq!(bought["credit"], INITIAL_CREDIT - 2 * PRICE);
    let debits: Vec<_> = events
        .iter()
        .filter(|event| packet(event, "LoseCredit"))
        .collect();
    assert_eq!(
        debits.len(),
        1,
        "ordinary Source debit must precede correlated billing reply"
    );
    assert_eq!(debits[0]["payload"]["credit"], PRICE);
    let gained: Vec<_> = events
        .iter()
        .filter(|event| packet(event, "GainedItem"))
        .collect();
    assert_eq!(gained.len(), 1);
    assert_eq!(
        gained[0]["payload"]["item"]["unique_id"].as_u64(),
        Some(second_uid.parse::<u64>().unwrap())
    );
    assert_eq!(
        gained[0]["payload"]["item"]["item_index"].as_i64(),
        Some(i64::from(mir2_game_data::BILLING_MONTHLY_CARD_ITEM_INDEX))
    );
    let (repeated, events) = billing(&mut socket, json!({
        "type":"billingBuyMonthlyCard", "characterIndex":character, "requestId":10, "orderRequestId":online_order
    }), "buyMonthlyCard").await;
    let repeated = successful_status(&repeated);
    assert_eq!(repeated["credit"], INITIAL_CREDIT - 2 * PRICE);
    assert_eq!(unique_card(repeated), second_uid);
    assert!(!events
        .iter()
        .any(|event| packet(event, "LoseCredit") || packet(event, "GainedItem")));
    let (activated, events) = billing(&mut socket, json!({
        "type":"billingActivateMonthlyCard", "characterIndex":character, "requestId":11, "itemUniqueId":second_uid
    }), "activateMonthlyCard").await;
    let activated = successful_status(&activated);
    let second_expiry = first_expiry + MONTHLY_CARD_DURATION_MS;
    assert_eq!(activated["monthlyCard"]["expiresAtMs"], second_expiry);
    assert_eq!(activated["credit"], INITIAL_CREDIT - 2 * PRICE);
    assert!(activated["ownedMonthlyCards"]
        .as_array()
        .unwrap()
        .is_empty());
    let use_item = events
        .iter()
        .find(|event| packet(event, "UseItem"))
        .expect("ordinary Source use acknowledgement before billing result");
    assert_eq!(
        use_item["payload"]["uniqueId"].as_u64(),
        Some(second_uid.parse::<u64>().unwrap())
    );
    assert_eq!(use_item["payload"]["success"], true);
    let (repeated, _) = billing(&mut socket, json!({
        "type":"billingActivateMonthlyCard", "characterIndex":character, "requestId":12, "itemUniqueId":second_uid
    }), "activateMonthlyCard").await;
    let repeated = successful_status(&repeated);
    assert_eq!(repeated["monthlyCard"]["expiresAtMs"], second_expiry);
    assert_eq!(repeated["credit"], INITIAL_CREDIT - 2 * PRICE);
    assert!(repeated["ownedMonthlyCards"].as_array().unwrap().is_empty());

    send(&mut socket, json!({"type":"logOut"})).await;
    read_until(&mut socket, "ordinary final LogOutSuccess", |event| {
        packet(event, "LogOutSuccess")
    })
    .await;
    socket.close(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while state.capacity.status().current_ws_connections != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("ordinary socket teardown should finish");
    assert_eq!(state.capacity.status().current_active_sessions, 0);
    assert_eq!(state.session_cache.route_lease_count(), 0);
    let saved = repository.load(default_character).unwrap();
    let account = &saved.accounts[ACCOUNT];
    assert_eq!(account.saves[&character].credit, INITIAL_CREDIT - 2 * PRICE);
    let ledger = account.monthly_card.as_ref().unwrap();
    assert_eq!(ledger.expires_at_ms, second_expiry);
    assert_eq!(ledger.item_receipts.len(), 2);
    assert!(ledger
        .item_receipts
        .values()
        .all(|receipt| receipt.redeemed_at_ms.is_some()));
    assert!(account
        .billing
        .as_ref()
        .is_none_or(|ledger| ledger.orders.is_empty()));
    local.server.abort();
    let _ = (&mut local.server).await;
    drop(local);
    drop(state);
}

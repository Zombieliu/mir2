//! Real local HTTP + File authority. Starting credits are explicitly unknown
//! fixtures, not provider income. No environment keys or chain signing used.
use super::*;
fn fixture() -> (WebState, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "mir2-offline-billing-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = GatewayConfig::default()
        .with_billing_monthly_card_credit_price(Some(600))
        .unwrap();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&config.default_character.index)
        .unwrap()
        .credit = 1000;
    std::fs::write(
        &path,
        serde_json::to_vec(&*config.account_store.lock().unwrap()).unwrap(),
    )
    .unwrap();
    (web_state(config.with_account_store_path(&path)), path)
}
#[tokio::test]
async fn offline_http_monthly_uses_normal_owned_item_and_exact_replay() {
    let (state, path) = fixture();
    let (grant, _) = issue_identity(&state.identity, "demo");
    let http = local_http(state.clone()).await;
    let index = state.config.default_character.index;
    let buy =
        json!({"characterIndex":index,"requestId":1,"orderRequestId":"offline-http-card-one"});
    let response = http
        .client
        .post(format!("{}/v1/billing/monthly-card/buy", http.url))
        .bearer_auth(&grant.token)
        .json(&buy)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let first: Value = response.json().await.unwrap();
    assert_eq!(first["status"]["credit"], 400);
    let uid = first["status"]["ownedMonthlyCards"][0]["uniqueId"]
        .as_str()
        .unwrap()
        .to_owned();
    let second: Value = http
        .client
        .post(format!("{}/v1/billing/monthly-card/buy", http.url))
        .bearer_auth(&grant.token)
        .json(&buy)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["status"]["credit"], 400);
    assert_eq!(second["status"]["ownedMonthlyCards"][0]["uniqueId"], uid);
    let activate = json!({"characterIndex":index,"requestId":2,"itemUniqueId":uid});
    let response = http
        .client
        .post(format!("{}/v1/billing/monthly-card/activate", http.url))
        .bearer_auth(&grant.token)
        .json(&activate)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let active: Value = response.json().await.unwrap();
    assert!(
        active["status"]["monthlyCard"]["expiresAtMs"]
            .as_u64()
            .unwrap()
            > monthly_card_now_ms()
    );
    let again: Value = http
        .client
        .post(format!("{}/v1/billing/monthly-card/activate", http.url))
        .bearer_auth(&grant.token)
        .json(&activate)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        again["status"]["monthlyCard"]["expiresAtMs"],
        active["status"]["monthlyCard"]["expiresAtMs"]
    );
    assert_eq!(again["status"]["credit"], 400);
    let account = state.config.account_store.lock().unwrap().accounts["demo"].clone();
    let sources =
        mir2_simulation::billing_provenance::monthly_revenue_sources(&account, "demo", false)
            .unwrap();
    assert_eq!(sources[0].eligible_micro_usd, 0);
    assert_eq!(sources[0].unknown_credits, 600);
    assert_eq!(state.session_cache.route_lease_count(), 0);
    drop(http);
    drop(state);
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn offline_http_auth_ownership_and_existing_online_owner_are_enforced() {
    let (state, path) = fixture();
    let (grant, verified) = issue_identity(&state.identity, "demo");
    let (other, _) = issue_identity(&state.identity, "other");
    let http = local_http(state.clone()).await;
    let index = state.config.default_character.index;
    let body = json!({"characterIndex":index,"requestId":1});
    assert_private_error(
        http.client
            .post(format!("{}/v1/billing/refresh", http.url))
            .json(&body)
            .send()
            .await
            .unwrap(),
        StatusCode::UNAUTHORIZED,
        "billingAuthenticationRequired",
    )
    .await;
    let other_response = http
        .client
        .post(format!("{}/v1/billing/refresh", http.url))
        .bearer_auth(&other.token)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_ne!(other_response.status(), StatusCode::OK);
    let key = GatewaySessionCacheKey {
        account_id: "demo".into(),
        character_index: index,
    };
    state
        .session_cache
        .acquire_route_lease(&key, "real-source-owner", 300)
        .unwrap();
    assert_private_error(
        http.client
            .post(format!("{}/v1/billing/refresh", http.url))
            .bearer_auth(&grant.token)
            .json(&body)
            .send()
            .await
            .unwrap(),
        StatusCode::CONFLICT,
        "billingCharacterOnline",
    )
    .await;
    state
        .session_cache
        .release_route_lease(&key, "real-source-owner")
        .unwrap();
    state
        .identity
        .revoke_session(&verified, &verified.session_id, "explicit logout")
        .unwrap();
    assert_private_error(
        http.client
            .post(format!("{}/v1/billing/refresh", http.url))
            .bearer_auth(&grant.token)
            .json(&body)
            .send()
            .await
            .unwrap(),
        StatusCode::UNAUTHORIZED,
        "billingAuthenticationRequired",
    )
    .await;
    assert_eq!(
        state.config.recharge_credit_status("demo", index).unwrap(),
        (1000, 0)
    );
    drop(http);
    drop(state);
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn offline_http_never_accepts_browser_financial_authority() {
    let (state, path) = fixture();
    let http = local_http(state.clone()).await;
    for (route, mut body) in [
        ("refresh", json!({"characterIndex":0,"requestId":1})),
        (
            "monthly-card/buy",
            json!({"characterIndex":0,"requestId":1,"orderRequestId":"strict-buy"}),
        ),
        (
            "monthly-card/activate",
            json!({"characterIndex":0,"requestId":1,"itemUniqueId":"1"}),
        ),
    ] {
        body["credits"] = json!(u32::MAX);
        let response = http
            .client
            .post(format!("{}/v1/billing/{route}", http.url))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(
        state.config.recharge_credit_status("demo", 0).unwrap(),
        (1000, 0)
    );
    drop(http);
    drop(state);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn promotion_game_session_requires_auth_and_directs_windows_without_world_mutation() {
    let (state, path) = fixture();
    let (grant, _) = issue_identity(&state.identity, "demo");
    let before = serde_json::to_value(&*state.config.account_store.lock().unwrap()).unwrap();
    let http = local_http(state.clone()).await;
    let url = format!("{}/v1/promotions/game-session", http.url);
    let unauthorized = http
        .client
        .post(&url)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let authorized = http
        .client
        .post(&url)
        .bearer_auth(&grant.token)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(authorized.status(), StatusCode::CONFLICT);
    let body: Value = authorized.json().await.unwrap();
    assert_eq!(body, json!({"error":"promotionWindowsClientRequired"}));
    assert_eq!(
        serde_json::to_value(&*state.config.account_store.lock().unwrap()).unwrap(),
        before
    );
    drop(http);
    drop(state);
    let _ = std::fs::remove_file(path);
}

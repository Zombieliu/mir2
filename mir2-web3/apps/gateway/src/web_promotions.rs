//! The promotion website shares commercial identity and the authoritative game
//! store. No browser amount, account claim, role header, or wallet signer is trusted.
use super::*;
use hmac::{Hmac, KeyInit, Mac};
use mir2_simulation::promotion::{
    PromotionPayoutIntent, PromotionPayoutJournal, PromotionSignedPayout, PromotionStore,
    SandboxMonthlyRevenue, TrustedGameFact,
};
use sha2::Sha256;

pub(super) fn router() -> Router<WebState> {
    Router::new()
        .route("/v1/promotions/auth/exchange", post(wallet_exchange))
        .route("/v1/promotions/auth/password", post(password_session))
        .route("/v1/promotions/game-session", post(game_session))
        .route("/v1/promotions/public", get(public_campaign))
        .route("/v1/promotions/invite", get(invite))
        .route("/v1/promotions/click", post(click))
        .route("/v1/promotions/me", get(me))
        .route("/v1/promotions/members", get(members))
        .route("/v1/promotions/settlements", get(settlements))
        .route(
            "/v1/promotions/payout-address",
            get(payout_settings).post(payout_address),
        )
        .route("/v1/promotions/apply", post(apply))
        .route("/v1/promotions/candidate", post(candidate))
        .route("/v1/promotions/confirm", post(confirm))
        .route("/v1/promotions/appeals", post(appeal))
        .route("/v1/promotions/admin", get(admin_overview).post(admin))
        .route("/v1/promotions/export", get(settlement_export))
        .route("/v1/promotions/source-events", post(source_event))
        .route("/v1/promotions/payout-worker", post(payout_worker))
        .layer(axum::extract::DefaultBodyLimit::max(16_384))
}

fn response(status: StatusCode, value: Value) -> Response {
    (
        status,
        [
            (CACHE_CONTROL, "no-store"),
            (axum::http::header::REFERRER_POLICY, "no-referrer"),
        ],
        Json(value),
    )
        .into_response()
}
fn failure(error: &str) -> Response {
    let status = match error {
        "promotionAuthenticationRequired" => StatusCode::UNAUTHORIZED,
        "promotionPermissionDenied" | "promotionSourceRejected" => StatusCode::FORBIDDEN,
        "promotionRequestInvalid" | "promotionSourceInvalid" => StatusCode::BAD_REQUEST,
        "promotionTooManyRequests" => StatusCode::TOO_MANY_REQUESTS,
        "promotionUnavailable" | "promotionConfigurationInvalid" => StatusCode::SERVICE_UNAVAILABLE,
        value if value.ends_with("Missing") || value.ends_with("NotFound") => StatusCode::NOT_FOUND,
        _ => StatusCode::CONFLICT,
    };
    let public = if error.starts_with("promotion")
        && error.len() <= 100
        && error.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        error
    } else {
        "promotionUnavailable"
    };
    response(
        if public == "promotionUnavailable" {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            status
        },
        json!({"error":public}),
    )
}
fn store(state: &WebState) -> Result<PromotionStore, String> {
    let database = state
        .config
        .account_store_database_url
        .as_ref()
        .ok_or("promotionUnavailable")?;
    let environment =
        std::env::var("MIR2_PROMOTION_ENVIRONMENT").map_err(|_| "promotionUnavailable")?;
    PromotionStore::new(database.clone(), &environment)
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"_:-.".contains(&v))
}
fn idempotency(headers: &HeaderMap) -> Result<String, String> {
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .ok_or("promotionRequestInvalid")?;
    if !valid_id(key) {
        return Err("promotionRequestInvalid".into());
    }
    Ok(key.into())
}
fn rate(state: &WebState, account: &str, operation: &str) -> Result<(), String> {
    billing::rate_limit(state.session_cache.as_ref(), account, operation)
        .map_err(|_| "promotionTooManyRequests".into())
}

async fn game_session(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(_request): Json<super::promotion_game::GameSessionRequest>,
) -> Response {
    let identity = match billing::http_identity(&state.identity, &headers).await {
        Ok(identity) => identity,
        Err(_) => return failure("promotionAuthenticationRequired"),
    };
    match tokio::task::spawn_blocking(move || {
        rate(&state, &identity.account_id, "promotionGameSession")?;
        super::promotion_game::issue(&state, &identity)
    }).await {
        Ok(Ok(value)) => response(StatusCode::OK, value),
        Ok(Err(error)) => failure(&error),
        Err(_) => failure("promotionUnavailable"),
    }
}
#[derive(Clone, Copy)]
enum Access {
    Read,
    Write,
    WalletWrite,
}

async fn execute(
    state: WebState,
    headers: HeaderMap,
    access: Access,
    action: impl FnOnce(PromotionStore, String, String, u64) -> Result<Value, String> + Send + 'static,
) -> Response {
    let write = !matches!(access, Access::Read);
    let identity = match billing::http_identity(&state.identity, &headers).await {
        Ok(value) => value,
        Err(_) => return failure("promotionAuthenticationRequired"),
    };
    let key = if write {
        match idempotency(&headers) {
            Ok(v) => v,
            Err(e) => return failure(&e),
        }
    } else {
        String::new()
    };
    let result = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        rate(
            &state,
            &identity.account_id,
            if write { "promotionWrite" } else { "status" },
        )?;
        if matches!(access, Access::WalletWrite) {
            let credentials = state
                .identity
                .list_credentials(&identity)
                .map_err(|_| "promotionUnavailable")?;
            if !credentials.iter().any(|credential| {
                credential.credential_kind == "sui_wallet" && credential.revoked_at_ms.is_none()
            }) {
                return Err("promotionWalletRequired".into());
            }
        }
        let store = store(&state)?;
        action(store, identity.account_id, key, gateway_now_ms())
    })
    .await;
    match result {
        Ok(Ok(value)) => response(StatusCode::OK, value),
        Ok(Err(error)) => failure(&error),
        Err(_) => failure("promotionUnavailable"),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CampaignQuery {
    campaign_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InviteQuery {
    campaign_id: String,
    code: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MembersQuery {
    campaign_id: String,
    #[serde(default)]
    offset: u32,
    #[serde(default = "page_limit")]
    limit: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportQuery {
    campaign_id: String,
    settlement_id: String,
}
fn page_limit() -> u32 {
    25
}

async fn public_campaign(
    State(state): State<WebState>,
    Query(query): Query<CampaignQuery>,
) -> Response {
    if !valid_id(&query.campaign_id) {
        return failure("promotionRequestInvalid");
    }
    match tokio::task::spawn_blocking(move || store(&state)?.public_campaign(&query.campaign_id))
        .await
    {
        Ok(Ok(v)) => response(StatusCode::OK, v),
        Ok(Err(e)) => failure(&e),
        Err(_) => failure("promotionUnavailable"),
    }
}
async fn invite(State(state): State<WebState>, Query(query): Query<InviteQuery>) -> Response {
    if !valid_id(&query.campaign_id) || !valid_id(&query.code) {
        return failure("promotionRequestInvalid");
    }
    match tokio::task::spawn_blocking(move || {
        store(&state)?.invite_preview(&query.campaign_id, &query.code)
    })
    .await
    {
        Ok(Ok(v)) => response(StatusCode::OK, v),
        Ok(Err(e)) => failure(&e),
        Err(_) => failure("promotionUnavailable"),
    }
}
async fn click(State(state): State<WebState>, Json(query): Json<InviteQuery>) -> Response {
    if !valid_id(&query.campaign_id) || !valid_id(&query.code) {
        return failure("promotionRequestInvalid");
    }
    match tokio::task::spawn_blocking(move || {
        rate(&state, &format!("invite:{}", query.code), "status")?;
        store(&state)?.record_invitation_click(&query.campaign_id, &query.code, gateway_now_ms())
    })
    .await
    {
        Ok(Ok(v)) => response(StatusCode::OK, v),
        Ok(Err(e)) => failure(&e),
        Err(_) => failure("promotionUnavailable"),
    }
}
async fn me(State(s): State<WebState>, h: HeaderMap, Query(q): Query<CampaignQuery>) -> Response {
    execute(s, h, Access::Read, move |s, a, _, now| {
        s.me(&q.campaign_id, &a, now)
    })
    .await
}
async fn members(
    State(s): State<WebState>,
    h: HeaderMap,
    Query(q): Query<MembersQuery>,
) -> Response {
    if q.limit == 0 || q.limit > 100 || q.offset > 100_000 {
        return failure("promotionRequestInvalid");
    }
    execute(s, h, Access::Read, move |s, a, _, now| {
        s.members(&q.campaign_id, &a, q.offset, q.limit, now)
    })
    .await
}
async fn settlements(
    State(s): State<WebState>,
    h: HeaderMap,
    Query(q): Query<CampaignQuery>,
) -> Response {
    execute(s, h, Access::Read, move |s, a, _, _| {
        s.settlements(&q.campaign_id, &a)
    })
    .await
}
async fn payout_settings(State(s): State<WebState>, h: HeaderMap) -> Response {
    execute(s, h, Access::Read, move |s, a, _, _| s.payout_settings(&a)).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Apply {
    campaign_id: String,
    character_index: i32,
    terms_version: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Candidate {
    campaign_id: String,
    code: String,
    source: String,
    #[serde(default)]
    click_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Confirm {
    campaign_id: String,
    candidate_id: String,
    terms_version: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PayoutAddress {
    credential_id: String,
    network: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Appeal {
    campaign_id: String,
    category: String,
    message: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Admin {
    campaign_id: String,
    command: Value,
}
async fn apply(State(s): State<WebState>, h: HeaderMap, Json(q): Json<Apply>) -> Response {
    execute(s, h, Access::WalletWrite, move |s, a, k, n| {
        s.apply(
            &q.campaign_id,
            &a,
            q.character_index,
            &q.terms_version,
            &k,
            n,
        )
    })
    .await
}
async fn candidate(State(s): State<WebState>, h: HeaderMap, Json(q): Json<Candidate>) -> Response {
    execute(s, h, Access::Write, move |s, a, k, n| {
        match (q.source.as_str(), q.click_id) {
            ("invite_link", Some(click_id)) => {
                s.candidate_from_click(&q.campaign_id, &a, &click_id, &k, n)
            }
            ("manual_code", None) => {
                s.create_candidate(&q.campaign_id, &q.code, &a, &q.source, &k, n)
            }
            _ => Err("promotionClickRequired".into()),
        }
    })
    .await
}
async fn confirm(State(s): State<WebState>, h: HeaderMap, Json(q): Json<Confirm>) -> Response {
    execute(s, h, Access::Write, move |s, a, k, n| {
        s.confirm(&q.campaign_id, &a, &q.candidate_id, &q.terms_version, &k, n)
    })
    .await
}
async fn payout_address(
    State(s): State<WebState>,
    h: HeaderMap,
    Json(q): Json<PayoutAddress>,
) -> Response {
    // The payout destination is the wallet used for this authenticated login.
    // A browser cannot select a different linked credential as a payee.
    let identity = match billing::http_identity(&s.identity, &h).await {
        Ok(v) => v,
        Err(_) => return failure("promotionAuthenticationRequired"),
    };
    let service = s.identity.clone();
    let requested = q.credential_id.clone();
    let allowed = tokio::task::spawn_blocking(move || {
        service.list_sessions(&identity).is_ok_and(|sessions| {
            sessions.iter().any(|session| {
                session.current
                    && session.auth_method == "sui_wallet"
                    && session.credential_id.as_deref() == Some(requested.as_str())
            })
        })
    })
    .await
    .unwrap_or(false);
    if !allowed {
        return failure("promotionWalletProofRequired");
    }
    execute(s, h, Access::Write, move |s, a, k, n| {
        s.verify_payout_address(&a, &q.credential_id, &q.network, &k, n)
    })
    .await
}
async fn appeal(State(s): State<WebState>, h: HeaderMap, Json(q): Json<Appeal>) -> Response {
    if q.message.len() > 4000 || q.category.len() > 60 {
        return failure("promotionRequestInvalid");
    }
    execute(s, h, Access::Write, move |s, a, k, n| {
        s.appeal(&q.campaign_id, &a, &q.category, &q.message, &k, n)
    })
    .await
}
async fn admin_overview(
    State(s): State<WebState>,
    h: HeaderMap,
    Query(q): Query<CampaignQuery>,
) -> Response {
    execute(s, h, Access::Read, move |s, a, _, n| {
        s.admin_view(&q.campaign_id, &a, n)
    })
    .await
}
async fn admin(State(s): State<WebState>, h: HeaderMap, Json(q): Json<Admin>) -> Response {
    execute(s, h, Access::Write, move |s, a, k, n| {
        s.admin(&q.campaign_id, &a, q.command, &k, n)
    })
    .await
}
async fn settlement_export(
    State(s): State<WebState>,
    h: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Response {
    execute(s, h, Access::Read, move |s, a, _, n| {
        s.settlement_export(&q.campaign_id, &a, &q.settlement_id, n)
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WalletExchange {
    provider: String,
    credential: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PasswordLogin {
    account_id: String,
    password: String,
}

fn wallet_subject(subject: &str) -> Result<String, String> {
    let address = subject
        .strip_prefix("sui:")
        .ok_or("promotionAuthenticationRequired")?;
    if address.len() != 66
        || !address.starts_with("0x")
        || !address[2..].bytes().all(|v| v.is_ascii_hexdigit())
    {
        return Err("promotionAuthenticationRequired".into());
    }
    Ok(address.to_ascii_lowercase())
}
fn login_accepted(packets: &[ServerPacket]) -> bool {
    packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. }))
}
fn one_use_wallet_proof(proof: &VerifiedChannelSubject, now: u64) -> bool {
    proof
        .token_id
        .as_deref()
        .is_some_and(|jti| !jti.is_empty() && jti.len() <= 256)
        && proof.expires_at_ms.is_some_and(|expiry| expiry > now)
}

fn ensure_wallet_account(config: &GatewayConfig, account_id: &str) -> Result<(), String> {
    use crate::session::save_recovery;
    use mir2_simulation::{PasskeyRecoveryPreflight, SimulationSession};
    match SimulationSession::passkey_login_recovery_preflight(config, account_id)? {
        PasskeyRecoveryPreflight::ExistingEligible => {
            save_recovery::replay_account(config, account_id)?;
        }
        PasskeyRecoveryPreflight::Missing => {
            save_recovery::provision_account_if_recovery_clear(config, account_id, || {
                SimulationSession::provision_passkey_account(config, account_id)
            })?;
        }
        PasskeyRecoveryPreflight::Rejected => return Err("promotionAuthenticationRequired".into()),
    }
    // Check current source policy again after recovery/provisioning. This
    // path never executes PasskeyLogin's native password-healing mutation.
    match SimulationSession::passkey_login_recovery_preflight(config, account_id)? {
        PasskeyRecoveryPreflight::ExistingEligible => Ok(()),
        _ => Err("promotionAuthenticationRequired".into()),
    }
}
async fn wallet_exchange(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(q): Json<WalletExchange>,
) -> Response {
    if q.provider != "suiWallet" || q.credential.len() > 12_000 {
        return failure("promotionAuthenticationRequired");
    }
    let _permit = match state
        .capacity
        .try_acquire_action(GatewayCapacityKind::Login)
    {
        Ok(p) => p,
        Err(_) => return failure("promotionTooManyRequests"),
    };
    let verified =
        match verified_channel_subject(ChannelIdentityProvider::SuiWallet, &q.credential).await {
            Ok(v) => v,
            Err(_) => return failure("promotionAuthenticationRequired"),
        };
    if !one_use_wallet_proof(&verified, gateway_now_ms()) {
        return failure("promotionAuthenticationRequired");
    }
    let address = match wallet_subject(&verified.subject) {
        Ok(v) => v,
        Err(e) => return failure(&e),
    };
    if consume_channel_subject_proof(state.session_cache.as_ref(), &verified).is_err() {
        return failure("promotionAuthenticationRequired");
    }
    let prior = if headers.contains_key(axum::http::header::AUTHORIZATION) {
        match identity_bearer_token(&headers) {
            Ok(v) => Some(v),
            Err(_) => return failure("promotionAuthenticationRequired"),
        }
    } else {
        None
    };
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("promotion-web")
        .chars()
        .take(160)
        .collect::<String>();
    let result = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        store(&state)?;
        rate(&state, &verified.subject, "promotionWalletLogin")?;
        let mut pending_binding = None;
        let canonical = if let Some(token) = prior {
            let own = state
                .identity
                .verify_session_token(&token)
                .map_err(|_| "promotionAuthenticationRequired")?;
            let current = state
                .identity
                .list_sessions(&own)
                .map_err(|_| "promotionUnavailable")?
                .into_iter()
                .find(|s| s.current)
                .ok_or("promotionAuthenticationRequired")?;
            if current.auth_method != "password"
                || gateway_now_ms().saturating_sub(current.issued_at_ms) > 600_000
            {
                return Err("promotionRecentPasswordRequired".into());
            }
            let account = own.account_id.clone();
            pending_binding = Some(token);
            account
        } else {
            match state
                .identity
                .resolve_sui_account("sui_wallet", &address)
                .map_err(|_| "promotionUnavailable")?
            {
                Some(account) => account,
                None => {
                    state
                        .channel_identity
                        .resolve_or_create(ChannelIdentityProvider::SuiWallet, &verified.subject)
                        .map_err(|_| "promotionUnavailable")?
                        .player_id
                }
            }
        };
        // Reuse recovery and authoritative account policy without opening a
        // gameplay session or rewriting an existing password credential.
        ensure_wallet_account(&state.config, &canonical).map_err(|error| {
            if error == "promotionAuthenticationRequired" {
                error
            } else {
                eprintln!("[promotion-auth] authoritative account recovery/provisioning failed");
                "promotionUnavailable".into()
            }
        })?;
        if let Some(token) = pending_binding {
            let own = state
                .identity
                .verify_session_token(&token)
                .map_err(|_| "promotionAuthenticationRequired")?;
            state
                .identity
                .bind_sui_credential(&own, "sui_wallet", &address)
                .map_err(|_| "promotionWalletBindingConflict")?;
        }
        let grant = state
            .identity
            .issue_session(
                &canonical,
                "sui_wallet",
                &address,
                &peer.ip().to_string(),
                &user_agent,
            )
            .map_err(|_| "promotionUnavailable")?;
        let credential = grant
            .session
            .credential_id
            .as_deref()
            .ok_or("promotionWalletProofRequired")?;
        let environment =
            std::env::var("MIR2_PROMOTION_ENVIRONMENT").map_err(|_| "promotionUnavailable")?;
        let network = if environment == "production" {
            "sui:mainnet"
        } else {
            "sui:testnet"
        };
        store(&state)?.verify_payout_address(
            &canonical,
            credential,
            network,
            &format!("login:{}", grant.session.session_id),
            gateway_now_ms(),
        )?;
        serde_json::to_value(grant).map_err(|_| "promotionUnavailable".into())
    })
    .await;
    match result {
        Ok(Ok(v)) => response(StatusCode::OK, v),
        Ok(Err(e)) => failure(&e),
        Err(_) => failure("promotionUnavailable"),
    }
}
async fn password_session(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(q): Json<PasswordLogin>,
) -> Response {
    if !valid_id(&q.account_id) || q.password.is_empty() || q.password.len() > 1024 {
        return failure("promotionAuthenticationRequired");
    }
    let _permit = match state
        .capacity
        .try_acquire_action(GatewayCapacityKind::Login)
    {
        Ok(p) => p,
        Err(_) => return failure("promotionTooManyRequests"),
    };
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("promotion-web")
        .chars()
        .take(160)
        .collect::<String>();
    let result = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        store(&state)?;
        rate(&state, &q.account_id, "promotionPasswordLogin")?;
        let peer_address = peer.ip().to_string();
        let context = AuthSecurityContext {
            account_id: q.account_id.clone(),
            action: AuthSecurityAction::Login,
        };
        enforce_auth_rate_limits(
            state.session_cache.as_ref(),
            &state.identity,
            &peer_address,
            &user_agent,
            &context,
        )
        .map_err(|_| "promotionTooManyRequests")?;
        let mut game =
            GatewaySession::new_with_zone_registry((*state.config).clone(), &state.zone_registry);
        if !login_accepted(
            &game
                .try_handle_packet(ClientPacket::Login {
                    account_id: q.account_id.clone(),
                    password: q.password,
                })
                .map_err(|_| "promotionUnavailable")?,
        ) {
            let _ = state.identity.record_auth_security_event(
                Some(&q.account_id),
                "promotion_password_login",
                "failure",
                "invalid_credentials_or_policy",
                &peer_address,
                &user_agent,
            );
            return Err("promotionAuthenticationRequired".into());
        }
        clear_successful_login_rate_limits(
            state.session_cache.as_ref(),
            &state.identity,
            &peer_address,
            &q.account_id,
        );
        let grant = state
            .identity
            .issue_session(
                &q.account_id,
                "password",
                &q.account_id,
                &peer_address,
                &user_agent,
            )
            .map_err(|_| "promotionUnavailable")?;
        serde_json::to_value(grant).map_err(|_| "promotionUnavailable".into())
    })
    .await;
    match result {
        Ok(Ok(v)) => response(StatusCode::OK, v),
        Ok(Err(e)) => failure(&e),
        Err(_) => failure("promotionUnavailable"),
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum SourceEvent {
    GameFact {
        fact: TrustedGameFact,
    },
    SandboxMonthlyRevenue {
        event: SandboxMonthlyRevenue,
    },
    Reversal {
        allocation_id: String,
        source_event_id: String,
        reversal_micro_usd: u64,
        evidence_ref: String,
        occurred_at_ms: u64,
    },
}
fn verify_source(
    secret: &str,
    timestamp: &str,
    signature: &str,
    body: &[u8],
    now: u64,
) -> Result<(), String> {
    verify_scoped_source(
        secret,
        "/v1/promotions/source-events",
        timestamp,
        signature,
        body,
        now,
    )
}
fn verify_scoped_source(
    secret: &str,
    path: &str,
    timestamp: &str,
    signature: &str,
    body: &[u8],
    now: u64,
) -> Result<(), String> {
    if secret.len() < 32 || body.len() > 16_384 || timestamp.len() > 16 {
        return Err("promotionSourceRejected".into());
    }
    let at = timestamp
        .parse::<u64>()
        .map_err(|_| "promotionSourceRejected")?;
    if at.abs_diff(now) > 60_000
        || signature.len() != 64
        || !signature.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("promotionSourceRejected".into());
    }
    let mut bytes = Vec::with_capacity(32);
    for index in (0..64).step_by(2) {
        bytes.push(
            u8::from_str_radix(&signature[index..index + 2], 16)
                .map_err(|_| "promotionSourceRejected")?,
        );
    }
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret.as_bytes())
        .map_err(|_| "promotionSourceRejected")?;
    mac.update(b"mir2-promotion-source-v1\nPOST\n");
    mac.update(path.as_bytes());
    mac.update(b"\n");
    mac.update(timestamp.as_bytes());
    mac.update(b"\n");
    mac.update(body);
    mac.verify_slice(&bytes)
        .map_err(|_| "promotionSourceRejected".into())
}

#[derive(Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum PayoutWorkerCommand {
    Reserve {
        intent: PromotionPayoutIntent,
        envelope: PromotionSignedPayout,
    },
    MarkSubmitted {
        payout_id: String,
        digest: String,
    },
    RecordOutcome {
        payout_id: String,
        outcome: Value,
    },
}
async fn payout_worker(
    State(state): State<WebState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let secret = match std::env::var("MIR2_PROMOTION_PAYOUT_WORKER_SECRET") {
        Ok(v) => v,
        Err(_) => return failure("promotionSourceRejected"),
    };
    if std::env::var("MIR2_PROMOTION_SOURCE_SECRET").is_ok_and(|v| v == secret) {
        return failure("promotionConfigurationInvalid");
    }
    let timestamp = headers
        .get("x-promotion-source-timestamp")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let signature = headers
        .get("x-promotion-source-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if verify_scoped_source(
        &secret,
        "/v1/promotions/payout-worker",
        timestamp,
        signature,
        &body,
        gateway_now_ms(),
    )
    .is_err()
    {
        return failure("promotionSourceRejected");
    }
    let command = match serde_json::from_slice::<PayoutWorkerCommand>(&body) {
        Ok(v) => v,
        Err(_) => return failure("promotionSourceInvalid"),
    };
    let actor = match std::env::var("MIR2_PROMOTION_PAYOUT_WORKER_ACCOUNT") {
        Ok(v) if valid_id(&v) => v,
        _ => return failure("promotionConfigurationInvalid"),
    };
    let result = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let database = state
            .config
            .account_store_database_url
            .clone()
            .ok_or("promotionUnavailable")?;
        let environment =
            std::env::var("MIR2_PROMOTION_ENVIRONMENT").map_err(|_| "promotionUnavailable")?;
        let journal = PromotionPayoutJournal::new(database, &environment)?;
        let now = gateway_now_ms();
        // Mainnet also requires the campaign's explicit production approval in
        // the journal; this deployment switch alone never authorizes money.
        let mainnet = std::env::var("MIR2_PROMOTION_ALLOW_MAINNET_PAYOUT").as_deref() == Ok("true");
        match command {
            PayoutWorkerCommand::Reserve { intent, envelope } => {
                journal.reserve_signed_attempt(&actor, &intent, &envelope, mainnet, now)
            }
            PayoutWorkerCommand::MarkSubmitted { payout_id, digest } => {
                journal.mark_submitted(&actor, &payout_id, &digest, now)?;
                Ok(json!({"status":"submitted","digest":digest}))
            }
            PayoutWorkerCommand::RecordOutcome { payout_id, outcome } => {
                journal.record_outcome(&actor, &payout_id, &outcome, now)
            }
        }
    })
    .await;
    match result {
        Ok(Ok(v)) => response(StatusCode::OK, v),
        Ok(Err(e)) => failure(&e),
        Err(_) => failure("promotionUnavailable"),
    }
}
async fn source_event(
    State(state): State<WebState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let secret = match std::env::var("MIR2_PROMOTION_SOURCE_SECRET") {
        Ok(v) => v,
        Err(_) => return failure("promotionSourceRejected"),
    };
    let timestamp = headers
        .get("x-promotion-source-timestamp")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let signature = headers
        .get("x-promotion-source-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if verify_source(&secret, timestamp, signature, &body, gateway_now_ms()).is_err() {
        return failure("promotionSourceRejected");
    }
    let event = match serde_json::from_slice::<SourceEvent>(&body) {
        Ok(v) => v,
        Err(_) => return failure("promotionSourceInvalid"),
    };
    let result = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let store = store(&state)?;
        let now = gateway_now_ms();
        match event {
            SourceEvent::GameFact { fact } => {
                if std::env::var("MIR2_PROMOTION_ENVIRONMENT").as_deref() == Ok("production") {
                    store.record_committed_game_fact(&fact, now)
                } else {
                    store.record_game_fact(&fact, now)
                }
            }
            SourceEvent::SandboxMonthlyRevenue { event } => {
                store.ingest_sandbox_monthly_revenue(&event, now)
            }
            SourceEvent::Reversal {
                allocation_id,
                source_event_id,
                reversal_micro_usd,
                evidence_ref,
                occurred_at_ms,
            } => {
                if std::env::var("MIR2_PROMOTION_ENVIRONMENT").as_deref() == Ok("production") {
                    return Err("promotionSourceRejected".into());
                }
                store.ingest_reversal(
                    &allocation_id,
                    &source_event_id,
                    reversal_micro_usd,
                    &evidence_ref,
                    occurred_at_ms,
                    now,
                )
            }
        }
    })
    .await;
    match result {
        Ok(Ok(v)) => response(StatusCode::OK, v),
        Ok(Err(e)) => failure(&e),
        Err(_) => failure("promotionUnavailable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn promotion_requests_never_accept_authority_or_identity_fields() {
        assert!(serde_json::from_value::<Confirm>(json!({"campaignId":"test","candidateId":"c","termsVersion":"draft","accountId":"victim"})).is_err());
        assert!(serde_json::from_value::<PayoutAddress>(
            json!({"credentialId":"own","network":"testnet","amount":100,"address":"attacker"})
        )
        .is_err());
        assert!(serde_json::from_value::<WalletExchange>(
            json!({"provider":"suiWallet","credential":"proof","accountId":"victim"})
        )
        .is_err());
        assert!(wallet_subject("sui:alice").is_err());
        assert!(wallet_subject(&format!("sui:0x{}", "a".repeat(64))).is_ok());
    }
    #[test]
    fn promotion_source_signature_binds_scope_time_and_payload() {
        let secret = "0123456789abcdef0123456789abcdef";
        let body = br#"{"type":"gameFact"}"#;
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(b"mir2-promotion-source-v1\nPOST\n/v1/promotions/source-events\n100000\n");
        mac.update(body);
        let signature = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert!(verify_source(secret, "100000", &signature, body, 100000).is_ok());
        assert!(verify_source(secret, "100000", &signature, b"{}", 100000).is_err());
        assert!(verify_source(secret, "100000", &signature, body, 200001).is_err());
        assert!(verify_source("short", "100000", &signature, body, 100000).is_err());
    }
    #[test]
    fn promotion_wallet_rejects_legacy_missing_or_empty_nonce_even_in_development() {
        let mut proof = VerifiedChannelSubject {
            subject: format!("sui:0x{}", "a".repeat(64)),
            token_id: None,
            expires_at_ms: Some(200),
        };
        assert!(!one_use_wallet_proof(&proof, 100));
        proof.token_id = Some(String::new());
        assert!(!one_use_wallet_proof(&proof, 100));
        proof.token_id = Some("dubhe-connect:one-use".into());
        assert!(one_use_wallet_proof(&proof, 100));
        assert!(!one_use_wallet_proof(&proof, 200));
    }

    #[test]
    fn promotion_wallet_account_check_preserves_existing_password() {
        let config = GatewayConfig::default();
        let mut account = mir2_simulation::AccountRecord::empty();
        account.password = "existing-password-fixture".into();
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .insert("existing-player".into(), account);
        ensure_wallet_account(&config, "existing-player").unwrap();
        assert_eq!(
            config.account_store.lock().unwrap().accounts["existing-player"].password,
            "existing-password-fixture"
        );
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get_mut("existing-player")
            .unwrap()
            .is_banned = true;
        assert_eq!(
            ensure_wallet_account(&config, "existing-player").unwrap_err(),
            "promotionAuthenticationRequired"
        );
        ensure_wallet_account(&config, "new-wallet-player").unwrap();
        let store = config.account_store.lock().unwrap();
        assert!(store.accounts["new-wallet-player"].characters.is_empty());
        assert!(store.accounts["new-wallet-player"].saves.is_empty());
    }
}

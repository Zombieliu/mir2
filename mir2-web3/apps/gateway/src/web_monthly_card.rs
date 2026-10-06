//! Monthly access HTTP routes and authenticated native socket requests.
use super::*;
use mir2_simulation::monthly_card::{monthly_card_now_ms, MonthlyCardStatus};

#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub(super) struct RedeemCode(pub String);
impl std::fmt::Debug for RedeemCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RedeemCode([REDACTED])")
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IssueRequest {
    account_id: String,
    request_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RedeemRequest {
    code: RedeemCode,
}
pub(super) fn router() -> Router<WebState> {
    Router::new()
        .route("/admin/monthly-cards/issue", post(issue))
        .route("/v1/monthly-card", get(status))
        .route("/v1/monthly-card/redeem", post(redeem))
}
pub(super) fn public_error(error: &str) -> &str {
    match error {
        "monthlyCardAuthenticationRequired"
        | "monthlyCardAccountBanned"
        | "monthlyCardCodeInvalid"
        | "monthlyCardCodeExpired"
        | "monthlyCardRequestIdInvalid"
        | "monthlyCardAccountMissing"
        | "monthlyCardIssuanceUnavailable"
        | "monthlyCardIssuanceKeyChanged"
        | "monthlyCardLedgerFull" => error,
        _ => "monthlyCardServiceUnavailable",
    }
}
fn private_json(status: StatusCode, value: Value) -> Response {
    (status, [(CACHE_CONTROL, "no-store")], Json(value)).into_response()
}
fn failure(error: &str) -> Response {
    let status = match error {
        "monthlyCardAuthenticationRequired" => StatusCode::UNAUTHORIZED,
        "monthlyCardAccountBanned" => StatusCode::FORBIDDEN,
        "monthlyCardCodeInvalid" | "monthlyCardCodeExpired" | "monthlyCardRequestIdInvalid" => {
            StatusCode::BAD_REQUEST
        }
        "monthlyCardAccountMissing" => StatusCode::NOT_FOUND,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    private_json(status, json!({"error":public_error(error)}))
}
async fn issue(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<IssueRequest>,
) -> Response {
    // Paid entitlement issuance deliberately has no developer-token fallback.
    if !env::var("MIR2_GATEWAY_OPERATOR_TOKEN")
        .ok()
        .is_some_and(|token| token.len() >= 32)
        || !bearer_token(&headers)
            .is_some_and(|token| crate::auth::verify_operator_token(token).is_ok())
    {
        return private_json(
            StatusCode::UNAUTHORIZED,
            json!({"error":"monthlyCardOperatorRequired"}),
        );
    }
    if state.config.account_store_path.is_none()
        && state.config.account_store_database_url.is_none()
    {
        return failure("monthlyCardIssuanceUnavailable");
    }
    match tokio::task::spawn_blocking(move || {
        state.config.issue_monthly_card(
            &request.account_id,
            &request.request_id,
            monthly_card_now_ms(),
        )
    })
    .await
    {
        Ok(Ok(receipt)) => private_json(StatusCode::OK, json!(receipt)),
        Ok(Err(error)) => failure(&error),
        Err(_) => failure("monthlyCardServiceUnavailable"),
    }
}
async fn status(State(state): State<WebState>, headers: HeaderMap) -> Response {
    let Ok(token) = identity_bearer_token(&headers) else {
        return failure("monthlyCardAuthenticationRequired");
    };
    match tokio::task::spawn_blocking(move || {
        let verified = state
            .identity
            .verify_session_token(&token)
            .map_err(|_| "monthlyCardAuthenticationRequired".to_owned())?;
        state
            .config
            .refresh_monthly_card_status(&verified.account_id, monthly_card_now_ms())
    })
    .await
    {
        Ok(Ok(status)) => private_json(StatusCode::OK, json!(status)),
        Ok(Err(error)) => failure(&error),
        Err(_) => failure("monthlyCardServiceUnavailable"),
    }
}
async fn redeem(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<RedeemRequest>,
) -> Response {
    let Ok(token) = identity_bearer_token(&headers) else {
        return failure("monthlyCardAuthenticationRequired");
    };
    match tokio::task::spawn_blocking(move || {
        let verified = state
            .identity
            .verify_session_token(&token)
            .map_err(|_| "monthlyCardAuthenticationRequired".to_owned())?;
        state.config.redeem_monthly_card(
            &verified.account_id,
            &request.code.0,
            monthly_card_now_ms(),
        )
    })
    .await
    {
        Ok(Ok(receipt)) => private_json(StatusCode::OK, json!(receipt)),
        Ok(Err(error)) => failure(&error),
        Err(_) => failure("monthlyCardServiceUnavailable"),
    }
}
pub(super) fn socket_reply(
    session: &GatewaySession,
    identity: &IdentityService,
    verified: Option<&VerifiedIdentitySession>,
    authenticated: bool,
    account_id: Option<&str>,
    action: &SessionAction,
) -> Value {
    let operation = if matches!(action, SessionAction::RedeemMonthlyCard { .. }) {
        "redeem"
    } else {
        "status"
    };
    let request_id = match action {
        SessionAction::RedeemMonthlyCard { request_id, .. } => Some(*request_id),
        _ => None,
    };
    let result: Result<(MonthlyCardStatus, bool), String> = (|| {
        let account = account_id
            .filter(|_| authenticated)
            .ok_or("monthlyCardAuthenticationRequired")?;
        let verified = verified
            .filter(|v| v.account_id == account && v.expires_at_ms > monthly_card_now_ms())
            .ok_or("monthlyCardAuthenticationRequired")?;
        identity
            .touch_session(verified)
            .map_err(|_| "monthlyCardAuthenticationRequired")?;
        let config = session.monthly_card_config()?;
        match action {
            SessionAction::RedeemMonthlyCard { code, .. } => config
                .redeem_monthly_card(account, &code.0, monthly_card_now_ms())
                .map(|receipt| (receipt.status, receipt.replayed)),
            _ => config
                .refresh_monthly_card_status(account, monthly_card_now_ms())
                .map(|status| (status, false)),
        }
    })();
    match result {
        Ok((status, replayed)) => {
            json!({"type":"monthlyCard","payload":{"operation":operation,"requestId":request_id,"status":status,"replayed":replayed}})
        }
        Err(error) => {
            json!({"type":"monthlyCard","payload":{"operation":operation,"requestId":request_id,"error":public_error(&error)}})
        }
    }
}
pub(super) async fn send_status(
    sender: &SharedWebSocketSender,
    session: &GatewaySession,
    account: &str,
) -> Result<(), String> {
    let status = session
        .monthly_card_config()?
        .monthly_card_status(account, monthly_card_now_ms())?;
    sender
        .lock()
        .await
        .send(Message::Text(
            json!({"type":"monthlyCard","payload":{"operation":"status","status":status}})
                .to_string()
                .into(),
        ))
        .await
        .map_err(|_| "monthlyCardSendFailed".into())
}

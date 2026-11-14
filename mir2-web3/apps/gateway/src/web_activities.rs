//! Account-owned campaign reservations; never write an active character save.
use super::*;
use mir2_simulation::monthly_card::monthly_card_now_ms;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StatusRequest {
    request_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClaimRequest {
    character_index: i32,
    reward_id: String,
    request_id: u64,
}

pub(super) fn router() -> Router<WebState> {
    Router::new()
        .route("/v1/activities/status", get(status))
        .route("/v1/activities/claim", post(claim))
        .layer(axum::extract::DefaultBodyLimit::max(2048))
}

fn private_response(status: StatusCode, value: Value) -> Response {
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

fn error_response(error: &str) -> Response {
    let (status, public) = match error {
        "billingAuthenticationRequired" => {
            (StatusCode::UNAUTHORIZED, "activitiesAuthenticationRequired")
        }
        "billingTooManyRequests" => (StatusCode::TOO_MANY_REQUESTS, "activitiesTooManyRequests"),
        "activityAccountBanned" => (StatusCode::FORBIDDEN, "activitiesAccountBanned"),
        "activityCharacterMissing" | "activityCharacterIdentityMismatch" => {
            (StatusCode::NOT_FOUND, "activitiesCharacterMissing")
        }
        "activityRewardMissing" => (StatusCode::NOT_FOUND, "activitiesRewardMissing"),
        "activityLevelRequired" => (StatusCode::CONFLICT, "activitiesLevelTooLow"),
        "activityDisabled" => (StatusCode::CONFLICT, "activitiesDisabled"),
        "activityPhaseSealed" => (StatusCode::CONFLICT, "activitiesPhaseClosed"),
        "activityClaimRecipientChanged" => {
            (StatusCode::CONFLICT, "activitiesClaimRecipientChanged")
        }
        "activitiesLevelTooLow"
        | "activitiesPhaseClosed"
        | "activitiesNotEligible"
        | "activitiesPhaseMismatch"
        | "activitiesDisabled" => (StatusCode::CONFLICT, error),
        "activitiesRequestInvalid" => (StatusCode::BAD_REQUEST, error),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            "activitiesResultUnconfirmed",
        ),
    };
    private_response(status, json!({"error":public}))
}

fn durable(config: &GatewayConfig) -> Result<(), String> {
    if config.account_store_path.is_none() && config.account_store_database_url.is_none() {
        return Err("activitiesUnavailable".into());
    }
    Ok(())
}

async fn status(
    State(state): State<WebState>,
    headers: HeaderMap,
    Query(request): Query<StatusRequest>,
) -> Response {
    let verified = match billing::http_identity(&state.identity, &headers).await {
        Ok(v) => v,
        Err(e) => return error_response(&e),
    };
    if request.request_id == 0 {
        return error_response("activitiesRequestInvalid");
    }
    let result = tokio::task::spawn_blocking(move || {
        durable(&state.config)?;
        billing::rate_limit(
            state.session_cache.as_ref(),
            &verified.account_id,
            "activitiesStatus",
        )?;
        state.config.activity_status(
            &verified.account_id,
            &state.config.activity_policy,
            monthly_card_now_ms(),
        )
    })
    .await;
    match result {
        Ok(Ok(value)) => private_response(StatusCode::OK, json!(value)),
        Ok(Err(e)) => error_response(&e),
        _ => error_response("activitiesResultUnconfirmed"),
    }
}

async fn claim(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<ClaimRequest>,
) -> Response {
    let verified = match billing::http_identity(&state.identity, &headers).await {
        Ok(v) => v,
        Err(e) => return error_response(&e),
    };
    if request.request_id == 0 || request.character_index < 0 || request.reward_id.len() > 80 {
        return error_response("activitiesRequestInvalid");
    }
    let result = tokio::task::spawn_blocking(move || {
        durable(&state.config)?;
        billing::rate_limit(
            state.session_cache.as_ref(),
            &verified.account_id,
            "activitiesClaim",
        )?;
        state.config.activity_claim(
            &verified.account_id,
            request.character_index,
            &request.reward_id,
            &state.config.activity_policy,
            monthly_card_now_ms(),
        )
    })
    .await;
    match result {
        Ok(Ok(value)) => private_response(StatusCode::OK, json!(value)),
        Ok(Err(e)) => error_response(&e),
        _ => error_response("activitiesResultUnconfirmed"),
    }
}

#[cfg(test)]
#[path = "web_activities_tests.rs"]
mod tests;

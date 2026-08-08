use std::env;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mir2_gateway::{
    Gate14AuthoritativeState, Gate14Command, Gate14CommandEnvelope, Gate14Placement,
    Gate14QuorumClient, Gate14SessionLease, Gate14WorldDirectorAnchor,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug, Clone)]
struct ObservedState {
    state: Gate14AuthoritativeState,
    state_root: String,
    agreeing_validators: Vec<String>,
    responding_validators: usize,
    observed_at_ms: u64,
    last_error: Option<String>,
    redis_available: bool,
}

impl Default for ObservedState {
    fn default() -> Self {
        Self {
            state: Gate14AuthoritativeState::default(),
            state_root: String::new(),
            agreeing_validators: Vec::new(),
            responding_validators: 0,
            observed_at_ms: 0,
            last_error: Some("waiting for Commonware quorum".to_string()),
            redis_available: false,
        }
    }
}

#[derive(Clone)]
struct GatewayState {
    gateway_id: String,
    quorum: Gate14QuorumClient,
    observed: Arc<RwLock<ObservedState>>,
    submit_lock: Arc<Mutex<()>>,
    redis_url: Option<String>,
    control_token: Option<String>,
    started_at_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GatewayStatus {
    role: &'static str,
    gateway_id: String,
    healthy: bool,
    finalized_height: u64,
    state_root: String,
    agreeing_validators: Vec<String>,
    responding_validators: usize,
    placement_count: usize,
    session_lease_count: usize,
    redis_available: bool,
    redis_authoritative: bool,
    observed_at_ms: u64,
    started_at_ms: u64,
    last_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RouteResponse {
    gateway_id: String,
    finalized_height: u64,
    state_root: String,
    placement: Gate14Placement,
    primary_endpoint: String,
    replica_endpoints: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcquireSessionRequest {
    session_id: String,
    account_id: String,
    character_id: String,
    zone_id: String,
    ttl_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcquireSessionResponse {
    accepted: bool,
    finalized_height: u64,
    state_root: String,
    lease: Gate14SessionLease,
    route: RouteResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandResponse {
    accepted: bool,
    command_digest: String,
    finalized_height: u64,
    state_root: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorldDirectorAnchorResponse {
    gateway_id: String,
    finalized_height: u64,
    state_root: String,
    anchor: Gate14WorldDirectorAnchor,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({"accepted": false, "error": self.message})),
        )
            .into_response()
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("gate14 gateway failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let gateway_id = env::var("GATE14_GATEWAY_ID").unwrap_or_else(|_| "gateway-a".to_string());
    let validator_urls = env::var("GATE14_VALIDATOR_URLS")
        .unwrap_or_else(|_| {
            "http://127.0.0.1:19400,http://127.0.0.1:19401,http://127.0.0.1:19402,http://127.0.0.1:19403"
                .to_string()
        })
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let quorum = Gate14QuorumClient::new(validator_urls)?;
    let bind = SocketAddr::from_str(
        &env::var("GATE14_GATEWAY_BIND").unwrap_or_else(|_| "0.0.0.0:9500".to_string()),
    )
    .map_err(|error| format!("invalid GATE14_GATEWAY_BIND: {error}"))?;
    let redis_url = env::var("GATE14_REDIS_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let control_token = env::var("GATE14_CONTROL_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let require_control_token = env_flag("GATE14_REQUIRE_CONTROL_TOKEN")
        || env::var("MIR2_DEPLOYMENT_ENV").is_ok_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "production" | "prod"
            )
        });
    if require_control_token && control_token.is_none() {
        return Err(
            "GATE14_CONTROL_TOKEN is required when Gate 14 control authentication is enabled"
                .to_string(),
        );
    }
    let state = GatewayState {
        gateway_id,
        quorum,
        observed: Arc::new(RwLock::new(ObservedState::default())),
        submit_lock: Arc::new(Mutex::new(())),
        redis_url,
        control_token,
        started_at_ms: now_ms(),
    };
    let watcher = state.clone();
    tokio::spawn(async move {
        loop {
            refresh_observed(&watcher).await;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });
    let app = Router::new()
        .route("/healthz", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/status", get(status))
        .route("/v1/control/commands", post(submit_command))
        .route(
            "/v1/world-director/anchors/{command_id}",
            get(world_director_anchor),
        )
        .route("/v1/routes/{zone_id}", get(route))
        .route("/v1/sessions/acquire", post(acquire_session))
        .route("/v1/sessions/{session_id}", get(session))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|error| format!("bind Gate 14 gateway {bind} failed: {error}"))?;
    axum::serve(listener, app)
        .await
        .map_err(|error| format!("serve Gate 14 gateway failed: {error}"))
}

async fn refresh_observed(state: &GatewayState) {
    match state.quorum.quorum_state().await {
        Ok(snapshot) => {
            let cache_payload = serde_json::to_vec(&snapshot.state).unwrap_or_default();
            let redis_available = match state.redis_url.clone() {
                Some(redis_url) => {
                    let key = format!("obelisk:gate14:gateway:{}", state.gateway_id);
                    tokio::task::spawn_blocking(move || {
                        redis_set(&redis_url, &key, &cache_payload, 30)
                    })
                    .await
                    .is_ok_and(|result| result.is_ok())
                }
                None => false,
            };
            *state.observed.write().await = ObservedState {
                state: snapshot.state,
                state_root: snapshot.state_root,
                agreeing_validators: snapshot.agreeing_validators,
                responding_validators: snapshot.responding_validators,
                observed_at_ms: now_ms(),
                last_error: None,
                redis_available,
            };
        }
        Err(error) => {
            let mut observed = state.observed.write().await;
            observed.last_error = Some(error);
            observed.observed_at_ms = now_ms();
        }
    }
}

async fn health(State(state): State<GatewayState>) -> impl IntoResponse {
    let observed = state.observed.read().await;
    if observed.last_error.is_none() && observed.agreeing_validators.len() >= 3 {
        (StatusCode::OK, "ok\n")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "waiting for quorum\n")
    }
}

async fn status(State(state): State<GatewayState>) -> Json<GatewayStatus> {
    let observed = state.observed.read().await;
    Json(GatewayStatus {
        role: "dynamic-gateway",
        gateway_id: state.gateway_id.clone(),
        healthy: observed.last_error.is_none() && observed.agreeing_validators.len() >= 3,
        finalized_height: observed.state.finalized_height,
        state_root: observed.state_root.clone(),
        agreeing_validators: observed.agreeing_validators.clone(),
        responding_validators: observed.responding_validators,
        placement_count: observed.state.placements.len(),
        session_lease_count: observed.state.session_leases.len(),
        redis_available: observed.redis_available,
        redis_authoritative: false,
        observed_at_ms: observed.observed_at_ms,
        started_at_ms: state.started_at_ms,
        last_error: observed.last_error.clone(),
    })
}

async fn route(
    State(state): State<GatewayState>,
    Path(zone_id): Path<String>,
) -> Result<Json<RouteResponse>, ApiError> {
    let observed = state.observed.read().await;
    Ok(Json(route_from_observed(
        &state.gateway_id,
        &observed,
        &zone_id,
    )?))
}

async fn session(
    State(state): State<GatewayState>,
    Path(session_id): Path<String>,
) -> Result<Json<Gate14SessionLease>, ApiError> {
    let observed = state.observed.read().await;
    observed
        .state
        .session_lease(&session_id, now_ms())
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::bad_request(format!("no live session lease {session_id}")))
}

async fn submit_command(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(command): Json<Gate14CommandEnvelope>,
) -> Result<Json<CommandResponse>, ApiError> {
    require_control_token(&headers, state.control_token.as_deref())?;
    let _guard = state.submit_lock.lock().await;
    let observed_height = state.observed.read().await.state.finalized_height;
    if command.sequence != observed_height.saturating_add(1) {
        return Err(ApiError::bad_request(format!(
            "next command sequence is {}, got {}",
            observed_height.saturating_add(1),
            command.sequence
        )));
    }
    let digest = state
        .quorum
        .submit(&command)
        .await
        .map_err(ApiError::unavailable)?;
    let snapshot = state
        .quorum
        .wait_for_height(command.sequence, Duration::from_secs(15))
        .await
        .map_err(ApiError::unavailable)?;
    refresh_observed(&state).await;
    Ok(Json(CommandResponse {
        accepted: true,
        command_digest: digest,
        finalized_height: snapshot.state.finalized_height,
        state_root: snapshot.state_root,
    }))
}

async fn world_director_anchor(
    State(state): State<GatewayState>,
    Path(command_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<WorldDirectorAnchorResponse>, ApiError> {
    require_control_token(&headers, state.control_token.as_deref())?;
    let observed = state.observed.read().await;
    let anchor = observed
        .state
        .world_director_anchors
        .get(&command_id)
        .cloned()
        .ok_or_else(|| {
            ApiError::not_found(format!("world director anchor not found: {command_id}"))
        })?;
    Ok(Json(WorldDirectorAnchorResponse {
        gateway_id: state.gateway_id.clone(),
        finalized_height: observed.state.finalized_height,
        state_root: observed.state_root.clone(),
        anchor,
    }))
}

fn require_control_token(headers: &HeaderMap, expected: Option<&str>) -> Result<(), ApiError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let supplied = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or_default();
    if !constant_time_equal(supplied.as_bytes(), expected.as_bytes()) {
        return Err(ApiError::unauthorized(
            "valid Gate 14 control bearer token is required",
        ));
    }
    Ok(())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        )
    })
}

async fn acquire_session(
    State(state): State<GatewayState>,
    Json(request): Json<AcquireSessionRequest>,
) -> Result<Json<AcquireSessionResponse>, ApiError> {
    let _guard = state.submit_lock.lock().await;
    let observed = state.observed.read().await.clone();
    let placement = observed
        .state
        .placement(&request.zone_id, now_ms())
        .cloned()
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "Zone {} has no live finalized placement",
                request.zone_id
            ))
        })?;
    let current_fence = observed
        .state
        .session_leases
        .get(&request.session_id)
        .map(|lease| lease.fencing_token)
        .unwrap_or(0);
    let ttl_ms = request.ttl_ms.unwrap_or(15_000).clamp(1_000, 60_000);
    let expires_at_ms = now_ms().saturating_add(ttl_ms).min(placement.expires_at_ms);
    let command = Gate14CommandEnvelope {
        sequence: observed.state.last_sequence.saturating_add(1),
        idempotency_key: format!(
            "session:{}:{}:{}",
            request.session_id,
            current_fence.saturating_add(1),
            state.gateway_id
        ),
        submitted_at_ms: now_ms(),
        command: Gate14Command::GrantSessionLease {
            session_id: request.session_id.clone(),
            account_id: request.account_id,
            character_id: request.character_id,
            gateway_id: state.gateway_id.clone(),
            zone_id: request.zone_id.clone(),
            fencing_token: current_fence.saturating_add(1),
            expires_at_ms,
        },
    };
    state
        .quorum
        .submit(&command)
        .await
        .map_err(ApiError::unavailable)?;
    let snapshot = state
        .quorum
        .wait_for_height(command.sequence, Duration::from_secs(15))
        .await
        .map_err(ApiError::unavailable)?;
    let lease = snapshot
        .state
        .session_leases
        .get(&request.session_id)
        .cloned()
        .ok_or_else(|| ApiError::unavailable("finalized session lease is missing"))?;
    refresh_observed(&state).await;
    let observed = state.observed.read().await;
    let route = route_from_observed(&state.gateway_id, &observed, &request.zone_id)?;
    Ok(Json(AcquireSessionResponse {
        accepted: true,
        finalized_height: snapshot.state.finalized_height,
        state_root: snapshot.state_root,
        lease,
        route,
    }))
}

async fn metrics(State(state): State<GatewayState>) -> String {
    let observed = state.observed.read().await;
    format!(
        "# HELP obelisk_gate14_gateway_finalized_height Finalized height observed by the gateway.\n\
         # TYPE obelisk_gate14_gateway_finalized_height gauge\n\
         obelisk_gate14_gateway_finalized_height{{gateway=\"{}\"}} {}\n\
         # HELP obelisk_gate14_gateway_quorum Validators agreeing on the current state root.\n\
         # TYPE obelisk_gate14_gateway_quorum gauge\n\
         obelisk_gate14_gateway_quorum{{gateway=\"{}\"}} {}\n\
         # HELP obelisk_gate14_gateway_redis_available Non-authoritative Redis cache availability.\n\
         # TYPE obelisk_gate14_gateway_redis_available gauge\n\
         obelisk_gate14_gateway_redis_available{{gateway=\"{}\"}} {}\n",
        metric_label(&state.gateway_id),
        observed.state.finalized_height,
        metric_label(&state.gateway_id),
        observed.agreeing_validators.len(),
        metric_label(&state.gateway_id),
        u8::from(observed.redis_available)
    )
}

fn route_from_observed(
    gateway_id: &str,
    observed: &ObservedState,
    zone_id: &str,
) -> Result<RouteResponse, ApiError> {
    let placement = observed
        .state
        .placement(zone_id, now_ms())
        .cloned()
        .ok_or_else(|| {
            ApiError::bad_request(format!("Zone {zone_id} has no live finalized placement"))
        })?;
    let primary_endpoint = observed
        .state
        .zone_hosts
        .get(&placement.primary_host_id)
        .map(|host| host.endpoint.clone())
        .ok_or_else(|| ApiError::unavailable("finalized primary Zone Host is missing"))?;
    let replica_endpoints = placement
        .replica_host_ids
        .iter()
        .map(|host_id| {
            observed
                .state
                .zone_hosts
                .get(host_id)
                .map(|host| host.endpoint.clone())
                .ok_or_else(|| {
                    ApiError::unavailable(format!(
                        "finalized replica Zone Host {host_id} is missing"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RouteResponse {
        gateway_id: gateway_id.to_string(),
        finalized_height: observed.state.finalized_height,
        state_root: observed.state_root.clone(),
        placement,
        primary_endpoint,
        replica_endpoints,
    })
}

fn redis_set(redis_url: &str, key: &str, value: &[u8], ttl_seconds: u64) -> Result<(), String> {
    let address = redis_address(redis_url)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .map_err(|error| format!("Redis connect failed: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|error| format!("Redis read timeout failed: {error}"))?;
    let ttl = ttl_seconds.to_string();
    let command = resp_command(&[b"SET", key.as_bytes(), value, b"EX", ttl.as_bytes()]);
    stream
        .write_all(&command)
        .map_err(|error| format!("Redis SET write failed: {error}"))?;
    let mut response = [0_u8; 64];
    let read = stream
        .read(&mut response)
        .map_err(|error| format!("Redis SET read failed: {error}"))?;
    if response[..read].starts_with(b"+OK") {
        Ok(())
    } else {
        Err("Redis SET was not acknowledged".to_string())
    }
}

fn redis_address(redis_url: &str) -> Result<SocketAddr, String> {
    let without_scheme = redis_url
        .strip_prefix("redis://")
        .ok_or_else(|| "GATE14_REDIS_URL must start with redis://".to_string())?;
    let authority = without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .rsplit('@')
        .next()
        .unwrap_or(without_scheme);
    let address = if authority.contains(':') {
        authority.to_string()
    } else {
        format!("{authority}:6379")
    };
    address
        .to_socket_addrs()
        .map_err(|error| format!("resolve Redis address failed: {error}"))?
        .next()
        .ok_or_else(|| "Redis address did not resolve".to_string())
}

fn resp_command(parts: &[&[u8]]) -> Vec<u8> {
    let mut output = format!("*{}\r\n", parts.len()).into_bytes();
    for part in parts {
        output.extend_from_slice(format!("${}\r\n", part.len()).as_bytes());
        output.extend_from_slice(part);
        output.extend_from_slice(b"\r\n");
    }
    output
}

fn metric_label(value: &str) -> String {
    value.replace(['\\', '"', '\n', '\r'], "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn control_token_rejects_missing_and_wrong_bearer_values() {
        let expected = Some("director-control-secret");
        assert!(require_control_token(&HeaderMap::new(), expected).is_err());
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer wrong-secret"),
        );
        assert!(require_control_token(&headers, expected).is_err());
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer director-control-secret"),
        );
        assert!(require_control_token(&headers, expected).is_ok());
    }

    #[test]
    fn absent_control_token_keeps_local_development_compatible() {
        assert!(require_control_token(&HeaderMap::new(), None).is_ok());
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

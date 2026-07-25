use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mir2_gateway::{HomeTelemetryStore, SignedHomeNodeTelemetry};
use serde::Deserialize;

const DEFAULT_BIND: &str = "127.0.0.1:18080";

#[derive(Clone)]
struct CollectorState {
    store: Arc<HomeTelemetryStore>,
    operator_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicQuery {
    expected_reports: u32,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("HOME_TELEMETRY_COLLECTOR_FATAL {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let bind = env::var("MIR2_HOME_TELEMETRY_COLLECTOR_BIND")
        .unwrap_or_else(|_| DEFAULT_BIND.to_string())
        .parse::<SocketAddr>()
        .map_err(|error| format!("invalid Home telemetry collector bind: {error}"))?;
    if !bind.ip().is_loopback() && !boolean_env("MIR2_HOME_TELEMETRY_TRUSTED_PROXY", false)? {
        return Err(
            "non-loopback telemetry collector bind requires a trusted TLS/privacy proxy"
                .to_string(),
        );
    }
    let operator_token = operator_token()?;
    if operator_token.as_bytes().len() < 32 {
        return Err("Home telemetry operator token must contain at least 32 bytes".to_string());
    }
    let maximum_age_ms = positive_u64_env("MIR2_HOME_TELEMETRY_MAXIMUM_AGE_MS", 120_000)?;
    let retention_ms = positive_u64_env(
        "MIR2_HOME_TELEMETRY_RETENTION_MS",
        30 * 24 * 60 * 60 * 1_000,
    )?;
    let state = CollectorState {
        store: Arc::new(HomeTelemetryStore::new(maximum_age_ms, retention_ms)?),
        operator_token,
    };
    let router = Router::new()
        .route("/healthz", get(health))
        .route("/v1/telemetry", post(ingest))
        .route("/v1/public", get(public_view))
        .route(
            "/v1/operator/{node_id}",
            get(operator_view).delete(delete_node),
        )
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|error| format!("bind Home telemetry collector {bind}: {error}"))?;
    println!("HOME_TELEMETRY_COLLECTOR_READY http=http://{bind}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|error| format!("serve Home telemetry collector: {error}"))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"ready": true, "rawIpStored": false}))
}

async fn ingest(
    State(state): State<CollectorState>,
    Json(report): Json<SignedHomeNodeTelemetry>,
) -> Response {
    match state.store.ingest(report, now_ms()) {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({"accepted": true})),
        )
            .into_response(),
        Err(error) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"accepted": false, "error": error})),
        )
            .into_response(),
    }
}

async fn public_view(
    State(state): State<CollectorState>,
    Query(query): Query<PublicQuery>,
) -> Response {
    match state.store.public_view(now_ms(), query.expected_reports) {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(error) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

async fn operator_view(
    State(state): State<CollectorState>,
    Path(node_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !bearer_matches(&headers, &state.operator_token) {
        return unauthorized();
    }
    match state.store.operator_view(&node_id, now_ms()) {
        Some(view) => (StatusCode::OK, Json(view)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "node telemetry not found"})),
        )
            .into_response(),
    }
}

async fn delete_node(
    State(state): State<CollectorState>,
    Path(node_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !bearer_matches(&headers, &state.operator_token) {
        return unauthorized();
    }
    match state.store.delete_node(&node_id) {
        Ok(deleted) => (
            StatusCode::OK,
            Json(serde_json::json!({"deleted": deleted})),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": "valid operator bearer token required"})),
    )
        .into_response()
}

fn bearer_matches(headers: &HeaderMap, expected: &str) -> bool {
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    supplied.is_some_and(|value| constant_time_equal(value.as_bytes(), expected.as_bytes()))
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn operator_token() -> Result<String, String> {
    let inline = env::var("MIR2_HOME_TELEMETRY_OPERATOR_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let file = env::var("MIR2_HOME_TELEMETRY_OPERATOR_TOKEN_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (inline, file) {
        (Some(_), Some(_)) => Err(
            "configure only one of MIR2_HOME_TELEMETRY_OPERATOR_TOKEN or MIR2_HOME_TELEMETRY_OPERATOR_TOKEN_FILE"
                .to_string(),
        ),
        (Some(value), None) => Ok(value),
        (None, Some(path)) => std::fs::read_to_string(&path)
            .map(|value| value.trim_end_matches(['\r', '\n']).to_string())
            .map_err(|error| format!("read Home telemetry operator token file {path}: {error}")),
        (None, None) => Err(
            "MIR2_HOME_TELEMETRY_OPERATOR_TOKEN or MIR2_HOME_TELEMETRY_OPERATOR_TOKEN_FILE is required"
                .to_string(),
        ),
    }
}

fn positive_u64_env(name: &str, default: u64) -> Result<u64, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("{name} must be a positive integer"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn boolean_env(name: &str, default: bool) -> Result<bool, String> {
    match env::var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Ok(true),
            "0" | "false" | "no" => Ok(false),
            _ => Err(format!("{name} must be true or false")),
        },
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(format!("read {name}: {error}")),
    }
}

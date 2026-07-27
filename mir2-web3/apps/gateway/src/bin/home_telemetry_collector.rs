use std::collections::BTreeMap;
use std::env;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mir2_gateway::{
    HomeNodeOperatorTelemetryView, HomeTelemetryStore, SignedHomeEnrollmentBundle,
    SignedHomeNodeTelemetry,
};
use serde::{Deserialize, Serialize};

const DEFAULT_BIND: &str = "127.0.0.1:18081";

#[derive(Clone)]
struct CollectorState {
    store: Arc<HomeTelemetryStore>,
    operator_token: String,
    admissions: Arc<RwLock<BTreeMap<String, HomeTelemetryAdmission>>>,
    admission_enforced: bool,
    admissions_file: Option<PathBuf>,
    enrollment_issuer_public_key: Option<String>,
}

#[derive(Debug, Clone)]
struct HomeTelemetryAdmission {
    public_key: String,
    key_generation: u64,
    capacity_certificate_id: String,
    capacity_max_sessions: usize,
    capacity_max_zones: usize,
    assigned_zone_id: String,
    placement_generation: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HomeTelemetryOperatorNode {
    node_id: String,
    assigned_zone_id: String,
    capacity_max_sessions: usize,
    capacity_max_zones: usize,
    placement_generation: u64,
    admission_expires_at_ms: u64,
    telemetry: Option<HomeNodeOperatorTelemetryView>,
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
    let (admissions, admission_enforced, admissions_file, enrollment_issuer_public_key) =
        admissions_from_env(now_ms())?;
    if !bind.ip().is_loopback() && !admission_enforced {
        return Err(
            "non-loopback telemetry collector requires signed Home enrollment admissions"
                .to_string(),
        );
    }
    let state = CollectorState {
        store: Arc::new(HomeTelemetryStore::new(maximum_age_ms, retention_ms)?),
        operator_token,
        admissions: Arc::new(RwLock::new(admissions)),
        admission_enforced,
        admissions_file,
        enrollment_issuer_public_key,
    };
    let router = Router::new()
        .route("/healthz", get(health))
        .route("/v1/telemetry", post(ingest))
        .route("/v1/public", get(public_view))
        .route("/v1/operator", get(operator_nodes))
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

async fn health(State(state): State<CollectorState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ready": true,
        "rawIpStored": false,
        "admissionEnforced": state.admission_enforced,
        "admittedNodes": state.admissions.read().map(|value| value.len()).unwrap_or_default(),
    }))
}

async fn ingest(
    State(state): State<CollectorState>,
    Json(report): Json<SignedHomeNodeTelemetry>,
) -> Response {
    if let Err(error) = refresh_admissions(&state, now_ms()) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"accepted": false, "error": error})),
        )
            .into_response();
    }
    if let Err(error) = validate_admission(&state, &report, now_ms()) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"accepted": false, "error": error})),
        )
            .into_response();
    }
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

fn validate_admission(
    state: &CollectorState,
    report: &SignedHomeNodeTelemetry,
    now_ms: u64,
) -> Result<(), String> {
    if !state.admission_enforced {
        return Ok(());
    }
    let admissions = state
        .admissions
        .read()
        .map_err(|_| "Home telemetry admissions lock poisoned".to_string())?;
    let admission = admissions
        .get(&report.payload.node_id)
        .ok_or_else(|| "Home telemetry node has no signed production admission".to_string())?;
    if admission.public_key != report.payload.public_key
        || admission.key_generation != report.payload.key_generation
        || admission.capacity_certificate_id != report.payload.capacity_certificate_id
        || admission.placement_generation != report.payload.placement_generation
    {
        return Err(
            "Home telemetry identity, certificate, or placement does not match admission"
                .to_string(),
        );
    }
    if now_ms > admission.expires_at_ms
        || report.payload.capacity_certificate_expires_at_ms > admission.expires_at_ms
    {
        return Err("Home telemetry production admission is expired or overstated".to_string());
    }
    Ok(())
}

fn admissions_from_env(
    now_ms: u64,
) -> Result<
    (
        BTreeMap<String, HomeTelemetryAdmission>,
        bool,
        Option<PathBuf>,
        Option<String>,
    ),
    String,
> {
    let path = env::var("MIR2_HOME_TELEMETRY_ADMISSIONS_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let issuer = env::var("MIR2_HOME_TELEMETRY_ENROLLMENT_ISSUER_PUBLIC_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let (Some(path), Some(issuer)) = (path, issuer) else {
        if env::var("MIR2_HOME_TELEMETRY_ADMISSIONS_FILE")
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
            || env::var("MIR2_HOME_TELEMETRY_ENROLLMENT_ISSUER_PUBLIC_KEY")
                .ok()
                .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(
                "Home telemetry admissions file and trusted issuer must be configured together"
                    .to_string(),
            );
        }
        return Ok((BTreeMap::new(), false, None, None));
    };
    let admissions = load_admissions(&path, &issuer, now_ms)?;
    Ok((admissions, true, Some(PathBuf::from(path)), Some(issuer)))
}

fn refresh_admissions(state: &CollectorState, now_ms: u64) -> Result<(), String> {
    let (Some(path), Some(issuer)) = (
        state.admissions_file.as_ref(),
        state.enrollment_issuer_public_key.as_ref(),
    ) else {
        return Ok(());
    };
    let admissions = load_admissions(path, issuer, now_ms)?;
    *state
        .admissions
        .write()
        .map_err(|_| "Home telemetry admissions lock poisoned".to_string())? = admissions;
    Ok(())
}

fn load_admissions(
    path: impl AsRef<FsPath>,
    issuer: &str,
    now_ms: u64,
) -> Result<BTreeMap<String, HomeTelemetryAdmission>, String> {
    let bundles: Vec<SignedHomeEnrollmentBundle> = read_json(path)?;
    let mut admissions = BTreeMap::new();
    for bundle in bundles {
        bundle.verify(
            issuer,
            &bundle.payload.node_id,
            &bundle.payload.public_key,
            now_ms,
        )?;
        if !bundle.relay_ready() {
            return Err(format!(
                "Home telemetry admission {} is not production-ready",
                bundle.payload.enrollment_id
            ));
        }
        let certificate = bundle
            .payload
            .capacity_certificate
            .as_ref()
            .ok_or_else(|| "Home telemetry admission has no capacity certificate".to_string())?;
        let placement = bundle
            .payload
            .placement
            .as_ref()
            .ok_or_else(|| "Home telemetry admission has no placement".to_string())?;
        let admission = HomeTelemetryAdmission {
            public_key: bundle.payload.public_key.clone(),
            key_generation: bundle.payload.key_generation,
            capacity_certificate_id: certificate.certificate_id.clone(),
            capacity_max_sessions: certificate.max_sessions,
            capacity_max_zones: certificate.max_zones,
            assigned_zone_id: placement.zone_id.clone(),
            placement_generation: placement.generation,
            expires_at_ms: bundle
                .payload
                .expires_at_ms
                .min(certificate.expires_at_ms)
                .min(
                    bundle
                        .payload
                        .relay_credential
                        .as_ref()
                        .expect("relay_ready checked")
                        .expires_at_ms,
                ),
        };
        if admissions
            .insert(bundle.payload.node_id.clone(), admission)
            .is_some()
        {
            return Err("Home telemetry admissions contain a duplicate node".to_string());
        }
    }
    Ok(admissions)
}

fn read_json<T: serde::de::DeserializeOwned>(path: impl AsRef<FsPath>) -> Result<T, String> {
    let path = path.as_ref();
    serde_json::from_slice(
        &std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("decode {}: {error}", path.display()))
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

async fn operator_nodes(State(state): State<CollectorState>, headers: HeaderMap) -> Response {
    if !bearer_matches(&headers, &state.operator_token) {
        return unauthorized();
    }
    if let Err(error) = refresh_admissions(&state, now_ms()) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": error})),
        )
            .into_response();
    }
    let now = now_ms();
    let admissions = match state.admissions.read() {
        Ok(admissions) => admissions,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Home telemetry admissions lock poisoned"})),
            )
                .into_response();
        }
    };
    let nodes = admissions
        .iter()
        .map(|(node_id, admission)| HomeTelemetryOperatorNode {
            node_id: node_id.clone(),
            assigned_zone_id: admission.assigned_zone_id.clone(),
            capacity_max_sessions: admission.capacity_max_sessions,
            capacity_max_zones: admission.capacity_max_zones,
            placement_generation: admission.placement_generation,
            admission_expires_at_ms: admission.expires_at_ms,
            telemetry: state.store.operator_view(node_id, now),
        })
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "generatedAtMs": now,
            "nodes": nodes,
        })),
    )
        .into_response()
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

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use mir2_gateway::{
        CapacityChallenge, CapacityChallengeResponse, CapacityWorkload,
        FinalizedGuildNodeRegistration, GuildNodeStatus, HomeAgentWorkMode,
        HomeEnrollmentBundlePayload, HomeEnrollmentRelayConfig, HomeEnrollmentRelayCredential,
        HomeEnrollmentResourcePolicy, HomeNodeTelemetryPayload, HomeTunnelPlacement,
        NodeCapacityCertificate, NodeSigningIdentity, SuiFinalityProof, HOME_TELEMETRY_SCHEMA,
    };

    #[test]
    fn empty_dynamic_admission_set_starts_fail_closed() {
        let path = temporary_json_path();
        publish_json(&path, &Vec::<SignedHomeEnrollmentBundle>::new());
        let admissions = load_admissions(&path, "unused-while-empty", now_ms())
            .expect("empty signed admission set should be a valid fail-closed startup state");
        assert!(admissions.is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn signed_production_admission_allows_only_matching_telemetry() {
        let now = now_ms();
        let node = NodeSigningIdentity::from_seed([71; 32]);
        let issuer = NodeSigningIdentity::from_seed([72; 32]);
        let relay = NodeSigningIdentity::from_seed([73; 32]);
        let certificate = capacity_certificate(&node, &issuer, now);
        let placement = HomeTunnelPlacement::issue(
            "telemetry-placement",
            "relay-telemetry-test",
            "primary",
            node.node_id(),
            1,
            7,
            4,
            91,
            now.saturating_sub(100),
            now.saturating_add(60_000),
            &issuer,
        )
        .unwrap();
        let bundle = SignedHomeEnrollmentBundle::issue(
            HomeEnrollmentBundlePayload {
                schema: String::new(),
                enrollment_id: "telemetry-enrollment".to_string(),
                node_id: node.node_id().to_string(),
                public_key: node.public_key().to_string(),
                key_generation: 1,
                issued_at_ms: now.saturating_sub(100),
                expires_at_ms: now.saturating_add(60_000),
                relay: HomeEnrollmentRelayConfig {
                    relay_id: "relay-telemetry-test".to_string(),
                    address: "127.0.0.1:9443".to_string(),
                    server_name: "relay.test".to_string(),
                    relay_public_key: relay.public_key().to_string(),
                },
                control_issuer_public_key: issuer.public_key().to_string(),
                telemetry_url: "https://telemetry.test/v1/telemetry".to_string(),
                resource_policy: HomeEnrollmentResourcePolicy {
                    max_sessions: 8,
                    max_sessions_per_zone: 4,
                    max_zones: 2,
                    cpu_limit_percent: 60,
                    reserved_memory_bytes: 2 * 1024 * 1024 * 1024,
                },
                allowed_games: vec!["mir2".to_string()],
                allowed_zones: vec!["primary".to_string()],
                capacity_issuer_public_key: issuer.public_key().to_string(),
                capacity_certificate: Some(certificate.clone()),
                placement: Some(placement),
                relay_credential: Some(HomeEnrollmentRelayCredential {
                    ca_certificate_der: URL_SAFE_NO_PAD.encode([1; 32]),
                    certificate_chain_der: vec![URL_SAFE_NO_PAD.encode([2; 32])],
                    issued_at_ms: now.saturating_sub(100),
                    expires_at_ms: now.saturating_add(60_000),
                }),
            },
            &issuer,
        )
        .unwrap();
        let path = temporary_json_path();
        publish_json(&path, &vec![bundle]);
        let admissions = load_admissions(&path, issuer.public_key(), now).unwrap();
        let state = CollectorState {
            store: Arc::new(HomeTelemetryStore::new(120_000, 86_400_000).unwrap()),
            operator_token: "a".repeat(32),
            admissions: Arc::new(RwLock::new(admissions)),
            admission_enforced: true,
            admissions_file: Some(path.clone()),
            enrollment_issuer_public_key: Some(issuer.public_key().to_string()),
        };
        let report =
            SignedHomeNodeTelemetry::sign(telemetry_payload(&node, &certificate, now, 7), &node)
                .unwrap();
        validate_admission(&state, &report, now).unwrap();

        let mismatched =
            SignedHomeNodeTelemetry::sign(telemetry_payload(&node, &certificate, now, 8), &node)
                .unwrap();
        assert!(validate_admission(&state, &mismatched, now).is_err());

        let stranger = NodeSigningIdentity::from_seed([74; 32]);
        let stranger_report = SignedHomeNodeTelemetry::sign(
            telemetry_payload(&stranger, &certificate, now, 7),
            &stranger,
        )
        .unwrap();
        assert!(validate_admission(&state, &stranger_report, now).is_err());
        let _ = std::fs::remove_file(path);
    }

    fn capacity_certificate(
        node: &NodeSigningIdentity,
        issuer: &NodeSigningIdentity,
        now: u64,
    ) -> NodeCapacityCertificate {
        let registration = FinalizedGuildNodeRegistration {
            node_id: node.node_id().to_string(),
            operator_sui_address: format!("0x{}", "11".repeat(32)),
            public_key: node.public_key().to_string(),
            endpoint: "outbound-only".to_string(),
            failure_domain: "telemetry-test".to_string(),
            stake_mist: 1_000_000,
            max_sessions: 8,
            max_zones: 2,
            key_generation: 1,
            status: GuildNodeStatus::Active,
            finality: SuiFinalityProof {
                network: "testnet".to_string(),
                package_id: format!("0x{}", "22".repeat(32)),
                transaction_digest: "telemetry-admission-test".to_string(),
                event_sequence: 0,
                checkpoint: 42,
            },
        };
        let challenge = CapacityChallenge {
            challenge_id: "telemetry-capacity".to_string(),
            node_id: node.node_id().to_string(),
            nonce: URL_SAFE_NO_PAD.encode([3; 32]),
            issued_at_ms: now.saturating_sub(100),
            expires_at_ms: now.saturating_add(60_000),
            workload: CapacityWorkload {
                concurrent_sessions: 8,
                max_sessions_per_zone: 4,
                zone_count: 2,
                command_count: 100,
                maximum_p95_latency_ms: 200,
                minimum_success_bps: 9_900,
            },
        };
        let response =
            CapacityChallengeResponse::sign(challenge, node, 1, 100, 0, 40, "ab".repeat(32), now)
                .unwrap();
        NodeCapacityCertificate::issue(&response, &registration, issuer, now, 60_000, 9).unwrap()
    }

    fn telemetry_payload(
        identity: &NodeSigningIdentity,
        certificate: &NodeCapacityCertificate,
        now: u64,
        placement_generation: u64,
    ) -> HomeNodeTelemetryPayload {
        HomeNodeTelemetryPayload {
            schema: HOME_TELEMETRY_SCHEMA.to_string(),
            node_id: identity.node_id().to_string(),
            public_key: identity.public_key().to_string(),
            key_generation: 1,
            agent_instance_id: "telemetry-agent".to_string(),
            sequence: 1,
            window_started_at_ms: now.saturating_sub(1_000),
            observed_at_ms: now,
            coarse_region: "hk".to_string(),
            provider_code: "isp-test".to_string(),
            relay_rtt_ms: 20,
            packet_loss_bps: 0,
            measured_upstream_kbps: 20_000,
            active_sessions: 1,
            active_zones: 1,
            zone_ids: vec!["primary".to_string()],
            checkpoint_lag_ms: 10,
            cpu_usage_bps: 1_000,
            memory_usage_bps: 2_000,
            work_mode: HomeAgentWorkMode::Serving,
            capacity_certificate_id: certificate.certificate_id.clone(),
            capacity_certificate_expires_at_ms: certificate.expires_at_ms,
            capacity_max_sessions: 8,
            capacity_max_zones: 2,
            finalized_control_height: 91,
            placement_generation,
            game_id: "mir2".to_string(),
            reward_epoch: 1,
            verified_work_units: 1,
            session_milliseconds: 1_000,
            agent_version: "0.1.0".to_string(),
        }
    }

    fn temporary_json_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "mir2-home-telemetry-admissions-{}-{}.json",
            std::process::id(),
            now_ms()
        ))
    }

    fn publish_json(path: &FsPath, value: &impl serde::Serialize) {
        let staging = path.with_extension("json.next");
        std::fs::write(&staging, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        std::fs::rename(staging, path).unwrap();
    }
}

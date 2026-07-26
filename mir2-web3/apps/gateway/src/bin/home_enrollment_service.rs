use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mir2_gateway::{
    node_id_from_public_key, CapacityChallenge, CapacityWorkload, FinalizedGuildNodeRegistration,
    HomeCapacityCertificationRequest, HomeEnrollmentBundlePayload, HomeEnrollmentRelayConfig,
    HomeEnrollmentRelayCredential, HomeEnrollmentRequest, HomeEnrollmentResourcePolicy,
    HomeTunnelPlacement, NodeCapacityCertificate, NodeSigningIdentity, SignedHomeEnrollmentBundle,
    SignedHomeEnrollmentChallenge,
};
use rand::RngCore;
use rcgen::{
    Certificate, CertificateParams, CertificateSigningRequestParams, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose,
};
use rustls_pki_types::{CertificateDer, CertificateSigningRequestDer};
use serde::{Deserialize, Serialize};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::sync::Mutex;

const DEFAULT_BIND: &str = "127.0.0.1:18080";
const DEFAULT_CHALLENGE_TTL_MS: u64 = 60_000;
const DEFAULT_BUNDLE_TTL_MS: u64 = 24 * 60 * 60 * 1_000;
const DEFAULT_CAPACITY_CERTIFICATE_TTL_MS: u64 = 24 * 60 * 60 * 1_000;
const DEFAULT_RELAY_CREDENTIAL_TTL_MS: u64 = 24 * 60 * 60 * 1_000;
const CLOCK_SKEW_ALLOWANCE_MS: u64 = 5_000;
const MAX_CSR_DER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
struct EnrollmentPolicy {
    relay: HomeEnrollmentRelayConfig,
    control_issuer_public_key: String,
    telemetry_url: String,
    resources: HomeEnrollmentResourcePolicy,
    allowed_games: Vec<String>,
    allowed_zones: Vec<String>,
    allowed_node_ids: BTreeSet<String>,
    finalized_registrations: BTreeMap<String, FinalizedGuildNodeRegistration>,
    capacity_workload: CapacityWorkload,
    capacity_certificate_ttl_ms: u64,
    relay_credential_ttl_ms: u64,
    challenge_ttl_ms: u64,
    bundle_ttl_ms: u64,
}

#[derive(Debug, Clone)]
struct ChallengeRecord {
    challenge: SignedHomeEnrollmentChallenge,
    consumed: bool,
}

#[derive(Debug, Clone)]
struct CapacityChallengeRecord {
    enrollment_id: String,
    challenge: CapacityChallenge,
    consumed: bool,
}

struct RelayTlsAuthority {
    ca_certificate_der: Vec<u8>,
    certificate: Certificate,
    key: KeyPair,
}

#[derive(Clone)]
struct AppState {
    issuer: Arc<NodeSigningIdentity>,
    control_issuer: Arc<NodeSigningIdentity>,
    relay_tls_authority: Option<Arc<RelayTlsAuthority>>,
    policy: Arc<EnrollmentPolicy>,
    challenges: Arc<Mutex<HashMap<String, ChallengeRecord>>>,
    capacity_challenges: Arc<Mutex<HashMap<String, CapacityChallengeRecord>>>,
    placements_file: Option<Arc<Mutex<PathBuf>>>,
    admissions_file: Option<Arc<Mutex<PathBuf>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChallengeRequest {
    node_id: String,
    public_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    healthy: bool,
    role: &'static str,
    issuer_public_key: String,
    challenge_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnrollmentResponse {
    accepted: bool,
    bundle: SignedHomeEnrollmentBundle,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CapacityCertificationResponse {
    accepted: bool,
    bundle: SignedHomeEnrollmentBundle,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
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

    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("HOME_ENROLLMENT_FATAL {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let bind = SocketAddr::from_str(
        &env::var("MIR2_HOME_ENROLLMENT_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string()),
    )
    .map_err(|error| format!("invalid MIR2_HOME_ENROLLMENT_BIND: {error}"))?;
    let issuer = Arc::new(signing_identity_from_env()?);
    let control_issuer = Arc::new(control_signing_identity_from_env(&issuer)?);
    let policy = Arc::new(policy_from_env(bind, &issuer, &control_issuer)?);
    let relay_tls_authority = relay_tls_authority_from_env()?.map(Arc::new);
    let state = AppState {
        issuer: Arc::clone(&issuer),
        control_issuer,
        relay_tls_authority,
        policy,
        challenges: Arc::new(Mutex::new(HashMap::new())),
        capacity_challenges: Arc::new(Mutex::new(HashMap::new())),
        placements_file: env::var("MIR2_HOME_ENROLLMENT_PLACEMENTS_FILE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .map(Mutex::new)
            .map(Arc::new),
        admissions_file: env::var("MIR2_HOME_ENROLLMENT_ADMISSIONS_FILE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .map(Mutex::new)
            .map(Arc::new),
    };
    let app = Router::new()
        .route("/healthz", get(health))
        .route("/v1/challenges", post(issue_challenge))
        .route("/v1/enrollments", post(complete_enrollment))
        .route("/v1/capacity/challenges", post(issue_capacity_challenge))
        .route(
            "/v1/capacity/certifications",
            post(complete_capacity_certification),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|error| format!("bind Home enrollment service {bind}: {error}"))?;
    println!(
        "HOME_ENROLLMENT_READY http=http://{bind}/ issuer_public_key={}",
        issuer.public_key()
    );
    axum::serve(listener, app)
        .await
        .map_err(|error| format!("serve Home enrollment service: {error}"))
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        healthy: true,
        role: "home-enrollment-authority",
        issuer_public_key: state.issuer.public_key().to_string(),
        challenge_count: state.challenges.lock().await.len()
            + state.capacity_challenges.lock().await.len(),
    })
}

async fn issue_challenge(
    State(state): State<AppState>,
    Json(request): Json<ChallengeRequest>,
) -> Result<Json<SignedHomeEnrollmentChallenge>, ApiError> {
    if node_id_from_public_key(&request.public_key).map_err(ApiError::bad_request)?
        != request.node_id
    {
        return Err(ApiError::bad_request(
            "Home enrollment Node ID does not match its public key",
        ));
    }
    ensure_allowed(&state.policy, &request.node_id)?;
    let now_ms = now_ms();
    let issued_at_ms = now_ms.saturating_sub(CLOCK_SKEW_ALLOWANCE_MS);
    let challenge_id = random_token(18);
    let challenge = SignedHomeEnrollmentChallenge::issue(
        challenge_id.clone(),
        random_token(32),
        request.node_id,
        request.public_key,
        issued_at_ms,
        state.policy.challenge_ttl_ms,
        &state.issuer,
    )
    .map_err(ApiError::internal)?;
    let mut challenges = state.challenges.lock().await;
    challenges.retain(|_, record| record.challenge.payload.expires_at_ms >= now_ms);
    challenges.insert(
        challenge_id,
        ChallengeRecord {
            challenge: challenge.clone(),
            consumed: false,
        },
    );
    Ok(Json(challenge))
}

async fn complete_enrollment(
    State(state): State<AppState>,
    Json(request): Json<HomeEnrollmentRequest>,
) -> Result<Json<EnrollmentResponse>, ApiError> {
    let now_ms = now_ms();
    let issued_at_ms = now_ms.saturating_sub(CLOCK_SKEW_ALLOWANCE_MS);
    request
        .verify(state.issuer.public_key(), now_ms)
        .map_err(ApiError::bad_request)?;
    ensure_allowed(&state.policy, &request.challenge.payload.node_id)?;
    {
        let mut challenges = state.challenges.lock().await;
        let record = challenges
            .get_mut(&request.challenge.payload.challenge_id)
            .ok_or_else(|| ApiError::bad_request("unknown Home enrollment challenge"))?;
        if record.consumed {
            return Err(ApiError::conflict(
                "Home enrollment challenge was already consumed",
            ));
        }
        if record.challenge != request.challenge {
            return Err(ApiError::bad_request(
                "Home enrollment challenge does not match the issued challenge",
            ));
        }
        record.consumed = true;
    }

    let key_generation = state
        .policy
        .finalized_registrations
        .get(&request.challenge.payload.node_id)
        .map(|registration| registration.key_generation)
        .unwrap_or(1);
    let payload = HomeEnrollmentBundlePayload {
        schema: String::new(),
        enrollment_id: format!("home-{}", random_token(18)),
        node_id: request.challenge.payload.node_id,
        public_key: request.challenge.payload.public_key,
        key_generation,
        issued_at_ms,
        expires_at_ms: issued_at_ms.saturating_add(state.policy.bundle_ttl_ms),
        relay: state.policy.relay.clone(),
        control_issuer_public_key: state.policy.control_issuer_public_key.clone(),
        telemetry_url: state.policy.telemetry_url.clone(),
        resource_policy: state.policy.resources.clone(),
        allowed_games: state.policy.allowed_games.clone(),
        allowed_zones: state.policy.allowed_zones.clone(),
        capacity_issuer_public_key: state.issuer.public_key().to_string(),
        capacity_certificate: None,
        placement: None,
        relay_credential: None,
    };
    let bundle =
        SignedHomeEnrollmentBundle::issue(payload, &state.issuer).map_err(ApiError::internal)?;
    Ok(Json(EnrollmentResponse {
        accepted: true,
        bundle,
    }))
}

async fn issue_capacity_challenge(
    State(state): State<AppState>,
    Json(enrollment): Json<SignedHomeEnrollmentBundle>,
) -> Result<Json<CapacityChallenge>, ApiError> {
    let now_ms = now_ms();
    let issued_at_ms = now_ms.saturating_sub(CLOCK_SKEW_ALLOWANCE_MS);
    enrollment
        .verify(
            state.issuer.public_key(),
            &enrollment.payload.node_id,
            &enrollment.payload.public_key,
            now_ms,
        )
        .map_err(ApiError::bad_request)?;
    ensure_allowed(&state.policy, &enrollment.payload.node_id)?;
    let registration = state
        .policy
        .finalized_registrations
        .get(&enrollment.payload.node_id)
        .ok_or_else(|| {
            ApiError::forbidden(
                "Home node has no active finalized Sui registration for capacity certification",
            )
        })?;
    registration.validate().map_err(ApiError::forbidden)?;
    if registration.public_key != enrollment.payload.public_key
        || registration.key_generation != enrollment.payload.key_generation
    {
        return Err(ApiError::forbidden(
            "Home enrollment identity does not match the finalized Sui registration",
        ));
    }
    if state.relay_tls_authority.is_none() {
        return Err(ApiError::unavailable(
            "Home Relay TLS certificate authority is not configured",
        ));
    }
    let mut workload = state.policy.capacity_workload.clone();
    workload.concurrent_sessions = workload
        .concurrent_sessions
        .min(registration.max_sessions)
        .min(enrollment.payload.resource_policy.max_sessions);
    workload.max_sessions_per_zone = workload
        .max_sessions_per_zone
        .min(registration.max_sessions)
        .min(enrollment.payload.resource_policy.max_sessions_per_zone)
        .min(workload.concurrent_sessions);
    workload.zone_count = workload
        .zone_count
        .min(registration.max_zones)
        .min(enrollment.payload.resource_policy.max_zones);
    workload.validate().map_err(ApiError::internal)?;
    let challenge_id = format!("capacity-{}", random_token(18));
    let challenge = CapacityChallenge {
        challenge_id: challenge_id.clone(),
        node_id: enrollment.payload.node_id.clone(),
        nonce: random_token(32),
        issued_at_ms,
        expires_at_ms: issued_at_ms.saturating_add(state.policy.challenge_ttl_ms),
        workload,
    };
    challenge.validate(now_ms).map_err(ApiError::internal)?;
    let mut challenges = state.capacity_challenges.lock().await;
    challenges.retain(|_, record| record.challenge.expires_at_ms >= now_ms);
    challenges.insert(
        challenge_id,
        CapacityChallengeRecord {
            enrollment_id: enrollment.payload.enrollment_id,
            challenge: challenge.clone(),
            consumed: false,
        },
    );
    Ok(Json(challenge))
}

async fn complete_capacity_certification(
    State(state): State<AppState>,
    Json(request): Json<HomeCapacityCertificationRequest>,
) -> Result<Json<CapacityCertificationResponse>, ApiError> {
    let now_ms = now_ms();
    let issued_at_ms = now_ms.saturating_sub(CLOCK_SKEW_ALLOWANCE_MS);
    request
        .enrollment
        .verify(
            state.issuer.public_key(),
            &request.enrollment.payload.node_id,
            &request.enrollment.payload.public_key,
            now_ms,
        )
        .map_err(ApiError::bad_request)?;
    ensure_allowed(&state.policy, &request.enrollment.payload.node_id)?;
    let registration = state
        .policy
        .finalized_registrations
        .get(&request.enrollment.payload.node_id)
        .ok_or_else(|| {
            ApiError::forbidden(
                "Home node has no active finalized Sui registration for capacity certification",
            )
        })?;
    request
        .response
        .verify(registration, now_ms)
        .map_err(ApiError::forbidden)?;
    let relay_tls_authority = state.relay_tls_authority.as_ref().ok_or_else(|| {
        ApiError::unavailable("Home Relay TLS certificate authority is not configured")
    })?;
    let csr_der = decode_csr(&request.certificate_signing_request_der)?;
    let relay_certificate = issue_relay_client_certificate(
        relay_tls_authority,
        &csr_der,
        &request.enrollment.payload.node_id,
        issued_at_ms,
        state.policy.relay_credential_ttl_ms,
    )?;

    {
        let mut challenges = state.capacity_challenges.lock().await;
        let record = challenges
            .get_mut(&request.response.challenge.challenge_id)
            .ok_or_else(|| ApiError::bad_request("unknown Home capacity challenge"))?;
        if record.consumed {
            return Err(ApiError::conflict(
                "Home capacity challenge was already consumed",
            ));
        }
        if record.enrollment_id != request.enrollment.payload.enrollment_id
            || record.challenge != request.response.challenge
        {
            return Err(ApiError::bad_request(
                "Home capacity response does not match the issued challenge",
            ));
        }
        record.consumed = true;
    }

    let certificate = NodeCapacityCertificate::issue(
        &request.response,
        registration,
        &state.issuer,
        issued_at_ms,
        state.policy.capacity_certificate_ttl_ms,
        registration.finality.checkpoint,
    )
    .map_err(ApiError::forbidden)?;
    let bundle_expires_at_ms = now_ms
        .saturating_add(state.policy.bundle_ttl_ms)
        .min(certificate.expires_at_ms)
        .min(relay_certificate.expires_at_ms);
    let zone_id = request
        .enrollment
        .payload
        .allowed_zones
        .first()
        .cloned()
        .unwrap_or_else(|| "primary".to_string());
    let placement = HomeTunnelPlacement::issue(
        format!("placement-{}", random_token(18)),
        request.enrollment.payload.relay.relay_id.clone(),
        zone_id,
        request.enrollment.payload.node_id.clone(),
        registration.key_generation,
        1,
        certificate.max_sessions_per_zone,
        registration.finality.checkpoint,
        issued_at_ms,
        bundle_expires_at_ms,
        &state.control_issuer,
    )
    .map_err(ApiError::internal)?;
    publish_placement(&state, &placement).await?;
    let mut payload = request.enrollment.payload;
    payload.issued_at_ms = issued_at_ms;
    payload.expires_at_ms = bundle_expires_at_ms;
    payload.capacity_issuer_public_key = state.issuer.public_key().to_string();
    payload.capacity_certificate = Some(certificate);
    payload.placement = Some(placement);
    payload.relay_credential = Some(relay_certificate);
    let bundle =
        SignedHomeEnrollmentBundle::issue(payload, &state.issuer).map_err(ApiError::internal)?;
    publish_admission(&state, &bundle).await?;
    Ok(Json(CapacityCertificationResponse {
        accepted: true,
        bundle,
    }))
}

async fn publish_admission(
    state: &AppState,
    bundle: &SignedHomeEnrollmentBundle,
) -> Result<(), ApiError> {
    let Some(path) = &state.admissions_file else {
        return Ok(());
    };
    let path = path.lock().await;
    let mut bundles = match std::fs::read(path.as_path()) {
        Ok(bytes) => {
            serde_json::from_slice::<Vec<SignedHomeEnrollmentBundle>>(&bytes).map_err(|error| {
                ApiError::internal(format!(
                    "decode Home telemetry admissions {}: {error}",
                    path.display()
                ))
            })?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(ApiError::internal(format!(
                "read Home telemetry admissions {}: {error}",
                path.display()
            )));
        }
    };
    bundles.retain(|existing| existing.payload.node_id != bundle.payload.node_id);
    bundles.push(bundle.clone());
    let bytes = serde_json::to_vec_pretty(&bundles).map_err(|error| {
        ApiError::internal(format!("encode Home telemetry admissions: {error}"))
    })?;
    let parent = path
        .parent()
        .ok_or_else(|| ApiError::internal("Home telemetry admissions path has no parent"))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        ApiError::internal(format!(
            "create Home telemetry admissions directory {}: {error}",
            parent.display()
        ))
    })?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes).map_err(|error| {
        ApiError::internal(format!(
            "write Home telemetry admissions {}: {error}",
            temporary.display()
        ))
    })?;
    std::fs::rename(&temporary, path.as_path()).map_err(|error| {
        ApiError::internal(format!(
            "install Home telemetry admissions {}: {error}",
            path.display()
        ))
    })
}

async fn publish_placement(
    state: &AppState,
    placement: &HomeTunnelPlacement,
) -> Result<(), ApiError> {
    let Some(path) = &state.placements_file else {
        return Ok(());
    };
    let path = path.lock().await;
    let mut placements = match std::fs::read(path.as_path()) {
        Ok(bytes) => {
            serde_json::from_slice::<Vec<HomeTunnelPlacement>>(&bytes).map_err(|error| {
                ApiError::internal(format!(
                    "decode Home Relay placements {}: {error}",
                    path.display()
                ))
            })?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(ApiError::internal(format!(
                "read Home Relay placements {}: {error}",
                path.display()
            )));
        }
    };
    placements.retain(|existing| {
        existing.placement_id != placement.placement_id
            && existing.zone_id != placement.zone_id
            && existing.node_id != placement.node_id
    });
    placements.push(placement.clone());
    let bytes = serde_json::to_vec_pretty(&placements)
        .map_err(|error| ApiError::internal(format!("encode Home Relay placements: {error}")))?;
    let parent = path
        .parent()
        .ok_or_else(|| ApiError::internal("Home Relay placements path has no parent"))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        ApiError::internal(format!(
            "create Home Relay placements directory {}: {error}",
            parent.display()
        ))
    })?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes).map_err(|error| {
        ApiError::internal(format!(
            "write Home Relay placements {}: {error}",
            temporary.display()
        ))
    })?;
    std::fs::rename(&temporary, path.as_path()).map_err(|error| {
        ApiError::internal(format!(
            "install Home Relay placements {}: {error}",
            path.display()
        ))
    })
}

fn ensure_allowed(policy: &EnrollmentPolicy, node_id: &str) -> Result<(), ApiError> {
    if !policy.allowed_node_ids.is_empty() && !policy.allowed_node_ids.contains(node_id) {
        return Err(ApiError::forbidden(
            "Home node is not included in the enrollment allowlist",
        ));
    }
    Ok(())
}

fn signing_identity_from_env() -> Result<NodeSigningIdentity, String> {
    let inline = env::var("MIR2_HOME_ENROLLMENT_SIGNING_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let file = env::var("MIR2_HOME_ENROLLMENT_SIGNING_KEY_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (inline, file) {
        (Some(value), None) => NodeSigningIdentity::from_base64_seed(&value),
        (None, Some(path)) => NodeSigningIdentity::from_file(path),
        (Some(_), Some(_)) => {
            Err("configure only one Home enrollment signing key source".to_string())
        }
        (None, None) => Err(
            "MIR2_HOME_ENROLLMENT_SIGNING_KEY or MIR2_HOME_ENROLLMENT_SIGNING_KEY_FILE is required"
                .to_string(),
        ),
    }
}

fn control_signing_identity_from_env(
    enrollment_issuer: &NodeSigningIdentity,
) -> Result<NodeSigningIdentity, String> {
    let inline = env::var("MIR2_HOME_ENROLLMENT_CONTROL_SIGNING_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let file = env::var("MIR2_HOME_ENROLLMENT_CONTROL_SIGNING_KEY_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (inline, file) {
        (Some(value), None) => NodeSigningIdentity::from_base64_seed(&value),
        (None, Some(path)) => NodeSigningIdentity::from_file(path),
        (Some(_), Some(_)) => Err("configure only one Home control signing key source".to_string()),
        (None, None) => Ok(enrollment_issuer.clone()),
    }
}

fn relay_tls_authority_from_env() -> Result<Option<RelayTlsAuthority>, String> {
    let certificate_path = env::var("MIR2_HOME_ENROLLMENT_TLS_CA_CERTIFICATE_DER")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let key_path = env::var("MIR2_HOME_ENROLLMENT_TLS_CA_KEY_DER")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let (Some(certificate_path), Some(key_path)) = (certificate_path, key_path) else {
        if env::var("MIR2_HOME_ENROLLMENT_TLS_CA_CERTIFICATE_DER")
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
            || env::var("MIR2_HOME_ENROLLMENT_TLS_CA_KEY_DER")
                .ok()
                .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(
                "Home Relay TLS CA certificate and private key must be configured together"
                    .to_string(),
            );
        }
        return Ok(None);
    };
    let ca_certificate_der = std::fs::read(&certificate_path).map_err(|error| {
        format!("read Home Relay TLS CA certificate {certificate_path}: {error}")
    })?;
    let key_der = std::fs::read(&key_path)
        .map_err(|error| format!("read Home Relay TLS CA key {key_path}: {error}"))?;
    let key = KeyPair::try_from(key_der)
        .map_err(|error| format!("decode Home Relay TLS CA PKCS#8 key: {error}"))?;
    let params =
        CertificateParams::from_ca_cert_der(&CertificateDer::from(ca_certificate_der.clone()))
            .map_err(|error| format!("decode Home Relay TLS CA certificate: {error}"))?;
    if !matches!(params.is_ca, IsCa::Ca(_)) {
        return Err("Home Relay TLS issuer certificate is not a CA".to_string());
    }
    let certificate = params
        .self_signed(&key)
        .map_err(|error| format!("validate Home Relay TLS CA key pair: {error}"))?;
    Ok(Some(RelayTlsAuthority {
        ca_certificate_der,
        certificate,
        key,
    }))
}

fn policy_from_env(
    bind: SocketAddr,
    issuer: &NodeSigningIdentity,
    control_issuer: &NodeSigningIdentity,
) -> Result<EnrollmentPolicy, String> {
    let relay_public_key = required_env("MIR2_HOME_ENROLLMENT_RELAY_PUBLIC_KEY")?;
    let control_issuer_public_key = required_env("MIR2_HOME_ENROLLMENT_CONTROL_ISSUER_PUBLIC_KEY")?;
    if control_issuer_public_key != control_issuer.public_key() {
        return Err(
            "configured Home control issuer public key does not match its signing key".to_string(),
        );
    }
    let allowed_node_ids = csv_env("MIR2_HOME_ENROLLMENT_ALLOWED_NODE_IDS")
        .into_iter()
        .collect::<BTreeSet<_>>();
    if !bind.ip().is_loopback() && allowed_node_ids.is_empty() {
        return Err(
            "non-loopback Home enrollment service requires MIR2_HOME_ENROLLMENT_ALLOWED_NODE_IDS"
                .to_string(),
        );
    }
    let finalized_registrations = env::var("MIR2_HOME_ENROLLMENT_REGISTRATIONS_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(read_finalized_registrations)
        .transpose()?
        .unwrap_or_default();
    let resources = HomeEnrollmentResourcePolicy {
        max_sessions: usize_env("MIR2_HOME_ENROLLMENT_MAX_SESSIONS", 32)?,
        max_sessions_per_zone: usize_env("MIR2_HOME_ENROLLMENT_MAX_SESSIONS_PER_ZONE", 16)?,
        max_zones: usize_env("MIR2_HOME_ENROLLMENT_MAX_ZONES", 2)?,
        cpu_limit_percent: u8_env("MIR2_HOME_ENROLLMENT_CPU_LIMIT_PERCENT", 60)?,
        reserved_memory_bytes: u64_env(
            "MIR2_HOME_ENROLLMENT_RESERVED_MEMORY_BYTES",
            2 * 1024 * 1024 * 1024,
        )?,
    };
    let policy = EnrollmentPolicy {
        relay: HomeEnrollmentRelayConfig {
            relay_id: env::var("MIR2_HOME_ENROLLMENT_RELAY_ID")
                .unwrap_or_else(|_| "home-relay-local".to_string()),
            address: env::var("MIR2_HOME_ENROLLMENT_RELAY_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:9443".to_string()),
            server_name: env::var("MIR2_HOME_ENROLLMENT_RELAY_SERVER_NAME")
                .unwrap_or_else(|_| "localhost".to_string()),
            relay_public_key,
        },
        control_issuer_public_key,
        telemetry_url: env::var("MIR2_HOME_ENROLLMENT_TELEMETRY_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:18081/v1/telemetry".to_string()),
        resources: resources.clone(),
        allowed_games: {
            let values = csv_env("MIR2_HOME_ENROLLMENT_ALLOWED_GAMES");
            if values.is_empty() {
                vec!["mir2".to_string()]
            } else {
                values
            }
        },
        allowed_zones: csv_env("MIR2_HOME_ENROLLMENT_ALLOWED_ZONES"),
        allowed_node_ids,
        finalized_registrations,
        capacity_workload: CapacityWorkload {
            concurrent_sessions: usize_env(
                "MIR2_HOME_ENROLLMENT_CAPACITY_CONCURRENT_SESSIONS",
                resources.max_sessions,
            )?,
            max_sessions_per_zone: usize_env(
                "MIR2_HOME_ENROLLMENT_CAPACITY_SESSIONS_PER_ZONE",
                resources.max_sessions_per_zone,
            )?,
            zone_count: usize_env(
                "MIR2_HOME_ENROLLMENT_CAPACITY_ZONE_COUNT",
                resources.max_zones,
            )?,
            command_count: u64_env("MIR2_HOME_ENROLLMENT_CAPACITY_COMMANDS", 2_000)?,
            maximum_p95_latency_ms: u64_env("MIR2_HOME_ENROLLMENT_CAPACITY_MAXIMUM_P95_MS", 100)?,
            minimum_success_bps: u16_env(
                "MIR2_HOME_ENROLLMENT_CAPACITY_MINIMUM_SUCCESS_BPS",
                9_990,
            )?,
        },
        capacity_certificate_ttl_ms: u64_env(
            "MIR2_HOME_ENROLLMENT_CAPACITY_CERTIFICATE_TTL_MS",
            DEFAULT_CAPACITY_CERTIFICATE_TTL_MS,
        )?,
        relay_credential_ttl_ms: u64_env(
            "MIR2_HOME_ENROLLMENT_RELAY_CREDENTIAL_TTL_MS",
            DEFAULT_RELAY_CREDENTIAL_TTL_MS,
        )?,
        challenge_ttl_ms: u64_env(
            "MIR2_HOME_ENROLLMENT_CHALLENGE_TTL_MS",
            DEFAULT_CHALLENGE_TTL_MS,
        )?,
        bundle_ttl_ms: u64_env("MIR2_HOME_ENROLLMENT_BUNDLE_TTL_MS", DEFAULT_BUNDLE_TTL_MS)?,
    };
    let probe_node = NodeSigningIdentity::from_seed([255; 32]);
    SignedHomeEnrollmentBundle::issue(
        HomeEnrollmentBundlePayload {
            schema: String::new(),
            enrollment_id: "configuration-probe".to_string(),
            node_id: probe_node.node_id().to_string(),
            public_key: probe_node.public_key().to_string(),
            key_generation: 1,
            issued_at_ms: 1,
            expires_at_ms: 2,
            relay: policy.relay.clone(),
            control_issuer_public_key: policy.control_issuer_public_key.clone(),
            telemetry_url: policy.telemetry_url.clone(),
            resource_policy: policy.resources.clone(),
            allowed_games: policy.allowed_games.clone(),
            allowed_zones: policy.allowed_zones.clone(),
            capacity_issuer_public_key: issuer.public_key().to_string(),
            capacity_certificate: None,
            placement: None,
            relay_credential: None,
        },
        &probe_node,
    )
    .map_err(|error| format!("invalid Home enrollment policy: {error}"))?;
    Ok(policy)
}

fn decode_csr(value: &str) -> Result<Vec<u8>, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ApiError::bad_request("Home Relay CSR must be URL-safe base64 DER"))?;
    if bytes.is_empty() || bytes.len() > MAX_CSR_DER_BYTES {
        return Err(ApiError::bad_request(
            "Home Relay CSR DER size is outside the accepted bound",
        ));
    }
    Ok(bytes)
}

fn issue_relay_client_certificate(
    authority: &RelayTlsAuthority,
    csr_der: &[u8],
    node_id: &str,
    issued_at_ms: u64,
    ttl_ms: u64,
) -> Result<HomeEnrollmentRelayCredential, ApiError> {
    if ttl_ms == 0 {
        return Err(ApiError::internal(
            "Home Relay credential TTL must be positive",
        ));
    }
    let mut request =
        CertificateSigningRequestParams::from_der(&CertificateSigningRequestDer::from(csr_der))
            .map_err(|error| ApiError::bad_request(format!("invalid Home Relay CSR: {error}")))?;
    let issued_at_seconds = i64::try_from(issued_at_ms / 1_000)
        .map_err(|_| ApiError::internal("Home Relay certificate time overflow"))?;
    let ttl_seconds = i64::try_from(ttl_ms.div_ceil(1_000))
        .map_err(|_| ApiError::internal("Home Relay certificate TTL overflow"))?;
    let not_before = OffsetDateTime::from_unix_timestamp(issued_at_seconds).map_err(|error| {
        ApiError::internal(format!("Home Relay certificate start time: {error}"))
    })? - TimeDuration::minutes(1);
    let not_after = not_before + TimeDuration::seconds(ttl_seconds.saturating_add(60));
    request.params.not_before = not_before;
    request.params.not_after = not_after;
    request.params.is_ca = IsCa::NoCa;
    request.params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    request.params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    request.params.subject_alt_names.clear();
    request.params.distinguished_name = DistinguishedName::new();
    request
        .params
        .distinguished_name
        .push(DnType::CommonName, node_id);
    let certificate = request
        .signed_by(&authority.certificate, &authority.key)
        .map_err(|error| ApiError::bad_request(format!("sign Home Relay CSR: {error}")))?;
    Ok(HomeEnrollmentRelayCredential {
        ca_certificate_der: URL_SAFE_NO_PAD.encode(&authority.ca_certificate_der),
        certificate_chain_der: vec![URL_SAFE_NO_PAD.encode(certificate.der())],
        issued_at_ms,
        expires_at_ms: issued_at_ms.saturating_add(ttl_ms),
    })
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RegistrationFile {
    One(FinalizedGuildNodeRegistration),
    Many(Vec<FinalizedGuildNodeRegistration>),
}

fn read_finalized_registrations(
    path: impl AsRef<Path>,
) -> Result<BTreeMap<String, FinalizedGuildNodeRegistration>, String> {
    let path = path.as_ref();
    let registrations = match read_json::<RegistrationFile>(path)? {
        RegistrationFile::One(value) => vec![value],
        RegistrationFile::Many(values) => values,
    };
    if registrations.is_empty() {
        return Err(format!(
            "Home finalized registration file {} is empty",
            path.display()
        ));
    }
    let mut indexed = BTreeMap::new();
    for registration in registrations {
        registration.validate().map_err(|error| {
            format!(
                "invalid finalized registration in {}: {error}",
                path.display()
            )
        })?;
        if indexed
            .insert(registration.node_id.clone(), registration)
            .is_some()
        {
            return Err(format!(
                "duplicate Home node registration in {}",
                path.display()
            ));
        }
    }
    Ok(indexed)
}

fn read_json<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, String> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("decode {}: {error}", path.display()))
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn csv_env(name: &str) -> Vec<String> {
    env::var(name)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn usize_env(name: &str, default: usize) -> Result<usize, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|error| format!("invalid {name}: {error}"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn u64_env(name: &str, default: u64) -> Result<u64, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|error| format!("invalid {name}: {error}"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn u8_env(name: &str, default: u8) -> Result<u8, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<u8>()
                .map_err(|error| format!("invalid {name}: {error}"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn u16_env(name: &str, default: u16) -> Result<u16, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<u16>()
                .map_err(|error| format!("invalid {name}: {error}"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn random_token(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    rand::thread_rng().fill_bytes(&mut value);
    URL_SAFE_NO_PAD.encode(value)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_gateway::{GuildNodeStatus, SuiFinalityProof};

    fn registration(identity: &NodeSigningIdentity) -> FinalizedGuildNodeRegistration {
        FinalizedGuildNodeRegistration {
            node_id: identity.node_id().to_string(),
            operator_sui_address: format!("0x{}", "11".repeat(32)),
            public_key: identity.public_key().to_string(),
            endpoint: "home-node.test:7020".to_string(),
            failure_domain: "home-test-a".to_string(),
            stake_mist: 2_000_000,
            max_sessions: 32,
            max_zones: 2,
            key_generation: 1,
            status: GuildNodeStatus::Active,
            finality: SuiFinalityProof {
                network: "testnet".to_string(),
                package_id: format!("0x{}", "22".repeat(32)),
                transaction_digest: "home-enrollment-testnet-transaction".to_string(),
                event_sequence: 0,
                checkpoint: 42,
            },
        }
    }

    fn tls_authority() -> RelayTlsAuthority {
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];
        let key = KeyPair::generate().unwrap();
        let certificate = params.self_signed(&key).unwrap();
        RelayTlsAuthority {
            ca_certificate_der: certificate.der().to_vec(),
            certificate,
            key,
        }
    }

    fn state(
        issuer: &NodeSigningIdentity,
        relay: &NodeSigningIdentity,
        node: &NodeSigningIdentity,
    ) -> AppState {
        let registration = registration(node);
        AppState {
            issuer: Arc::new(issuer.clone()),
            control_issuer: Arc::new(issuer.clone()),
            relay_tls_authority: Some(Arc::new(tls_authority())),
            policy: Arc::new(EnrollmentPolicy {
                relay: HomeEnrollmentRelayConfig {
                    relay_id: "relay-test".to_string(),
                    address: "127.0.0.1:9443".to_string(),
                    server_name: "relay.test".to_string(),
                    relay_public_key: relay.public_key().to_string(),
                },
                control_issuer_public_key: issuer.public_key().to_string(),
                telemetry_url: "http://127.0.0.1:18081/v1/telemetry".to_string(),
                resources: HomeEnrollmentResourcePolicy {
                    max_sessions: 32,
                    max_sessions_per_zone: 16,
                    max_zones: 2,
                    cpu_limit_percent: 60,
                    reserved_memory_bytes: 2 * 1024 * 1024 * 1024,
                },
                allowed_games: vec!["mir2".to_string()],
                allowed_zones: vec!["primary".to_string()],
                allowed_node_ids: BTreeSet::from([node.node_id().to_string()]),
                finalized_registrations: BTreeMap::from([(
                    registration.node_id.clone(),
                    registration,
                )]),
                capacity_workload: CapacityWorkload {
                    concurrent_sessions: 32,
                    max_sessions_per_zone: 16,
                    zone_count: 2,
                    command_count: 2_000,
                    maximum_p95_latency_ms: 100,
                    minimum_success_bps: 9_990,
                },
                capacity_certificate_ttl_ms: 60_000,
                relay_credential_ttl_ms: 60_000,
                challenge_ttl_ms: 60_000,
                bundle_ttl_ms: 60_000,
            }),
            challenges: Arc::new(Mutex::new(HashMap::new())),
            capacity_challenges: Arc::new(Mutex::new(HashMap::new())),
            placements_file: None,
            admissions_file: None,
        }
    }

    #[tokio::test]
    async fn enrollment_capacity_csr_and_production_bundle_round_trip() {
        let issuer = NodeSigningIdentity::from_seed([1; 32]);
        let relay = NodeSigningIdentity::from_seed([2; 32]);
        let node = NodeSigningIdentity::from_seed([3; 32]);
        let state = state(&issuer, &relay, &node);
        let challenge = issue_challenge(
            State(state.clone()),
            Json(ChallengeRequest {
                node_id: node.node_id().to_string(),
                public_key: node.public_key().to_string(),
            }),
        )
        .await
        .unwrap()
        .0;
        let enrollment_request =
            HomeEnrollmentRequest::sign(challenge, &node, issuer.public_key(), now_ms()).unwrap();
        let enrollment = complete_enrollment(State(state.clone()), Json(enrollment_request))
            .await
            .unwrap()
            .0
            .bundle;
        assert!(!enrollment.capacity_ready());
        let capacity = issue_capacity_challenge(State(state.clone()), Json(enrollment.clone()))
            .await
            .unwrap()
            .0;
        let response = mir2_gateway::CapacityChallengeResponse::sign(
            capacity.clone(),
            &node,
            1,
            capacity.workload.command_count,
            0,
            1,
            "ab".repeat(32),
            now_ms(),
        )
        .unwrap();
        let client_key = KeyPair::generate().unwrap();
        let mut request_params = CertificateParams::new(Vec::<String>::new()).unwrap();
        request_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        let csr = request_params.serialize_request(&client_key).unwrap();
        let certified = complete_capacity_certification(
            State(state.clone()),
            Json(HomeCapacityCertificationRequest {
                enrollment,
                response,
                certificate_signing_request_der: URL_SAFE_NO_PAD.encode(csr.der().as_ref()),
            }),
        )
        .await
        .unwrap()
        .0
        .bundle;
        certified
            .verify(
                issuer.public_key(),
                node.node_id(),
                node.public_key(),
                now_ms(),
            )
            .unwrap();
        assert!(certified.capacity_ready());
        assert!(certified.relay_ready());
        assert_eq!(
            certified
                .payload
                .capacity_certificate
                .as_ref()
                .unwrap()
                .max_sessions,
            32
        );

        let replay = complete_capacity_certification(
            State(state),
            Json(HomeCapacityCertificationRequest {
                enrollment: certified.clone(),
                response: mir2_gateway::CapacityChallengeResponse::sign(
                    capacity.clone(),
                    &node,
                    1,
                    capacity.workload.command_count,
                    0,
                    1,
                    "ab".repeat(32),
                    now_ms(),
                )
                .unwrap(),
                certificate_signing_request_der: URL_SAFE_NO_PAD.encode(csr.der().as_ref()),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(replay.status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn capacity_challenge_requires_finalized_registration() {
        let issuer = NodeSigningIdentity::from_seed([11; 32]);
        let relay = NodeSigningIdentity::from_seed([12; 32]);
        let node = NodeSigningIdentity::from_seed([13; 32]);
        let mut state = state(&issuer, &relay, &node);
        Arc::make_mut(&mut state.policy)
            .finalized_registrations
            .clear();
        let bundle = SignedHomeEnrollmentBundle::issue(
            HomeEnrollmentBundlePayload {
                schema: String::new(),
                enrollment_id: "home-no-registration".to_string(),
                node_id: node.node_id().to_string(),
                public_key: node.public_key().to_string(),
                key_generation: 1,
                issued_at_ms: now_ms(),
                expires_at_ms: now_ms().saturating_add(60_000),
                relay: state.policy.relay.clone(),
                control_issuer_public_key: issuer.public_key().to_string(),
                telemetry_url: state.policy.telemetry_url.clone(),
                resource_policy: state.policy.resources.clone(),
                allowed_games: vec!["mir2".to_string()],
                allowed_zones: vec!["primary".to_string()],
                capacity_issuer_public_key: issuer.public_key().to_string(),
                capacity_certificate: None,
                placement: None,
                relay_credential: None,
            },
            &issuer,
        )
        .unwrap();
        let error = issue_capacity_challenge(State(state), Json(bundle))
            .await
            .unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
    }
}

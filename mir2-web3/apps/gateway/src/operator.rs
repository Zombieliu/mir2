use std::env;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    node_id_from_public_key, verify_ed25519_signature, CapacityChallenge,
    CapacityChallengeResponse, FinalizedDirectorSubmission, NodeSigningIdentity,
    WorldDirectorRuntimeService, WorldDirectorRuntimeStatus, ZoneHostHealth, ZoneHostServer,
    ZoneHostTelemetrySnapshot, ZoneHostZoneTelemetry, ZoneMapScope,
};

const HEARTBEAT_SCHEMA: &str = "obelisk.zone-host-heartbeat.v3";
const HMAC_SIGNATURE_ALGORITHM: &str = "hmac-sha256";
const ED25519_SIGNATURE_ALGORITHM: &str = "ed25519-zip215";
// An ephemeral loopback port keeps local multi-host tests and developer
// processes isolated. Deployments set an explicit stable address.
const DEFAULT_OPERATOR_ADDR: &str = "127.0.0.1:0";
const MAX_HTTP_REQUEST_BYTES: usize = 2 * 1024 * 1024;

type HmacSha256 = Hmac<Sha256>;

pub fn zone_host_signing_identity_from_env() -> Result<Option<NodeSigningIdentity>, String> {
    let configured_identity = NodeSigningIdentity::from_env()?;
    let keyring_account = env::var("MIR2_ZONE_HOST_KEYRING_ACCOUNT")
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (configured_identity, keyring_account) {
        (Some(_), Some(_)) => Err(
            "configure either Zone Host signing key environment variables or MIR2_ZONE_HOST_KEYRING_ACCOUNT, not both"
                .to_string(),
        ),
        (Some(identity), None) => Ok(Some(identity)),
        (None, Some(account)) => crate::HomeAgentKeyring::new(account)?
            .load_identity()
            .map(Some),
        (None, None) => Ok(None),
    }
}

#[derive(Debug, Clone)]
pub struct ZoneHostOperatorConfig {
    pub address: SocketAddr,
    pub advertised_endpoint: String,
    pub failure_domain: String,
    heartbeat_secret: Option<String>,
    management_token: Option<String>,
    signing_identity: Option<NodeSigningIdentity>,
    key_generation: u64,
    heartbeat_sequence: Arc<AtomicU64>,
    capacity_max_commands: u64,
    capacity_challenge_inflight: Arc<AtomicBool>,
}

impl ZoneHostOperatorConfig {
    pub fn from_env(bound_rpc_address: SocketAddr, expected_host_id: &str) -> Result<Self, String> {
        let address = env::var("MIR2_ZONE_HOST_METRICS_ADDR")
            .unwrap_or_else(|_| DEFAULT_OPERATOR_ADDR.to_string())
            .parse::<SocketAddr>()
            .map_err(|error| {
                format!("MIR2_ZONE_HOST_METRICS_ADDR must be a socket address: {error}")
            })?;
        let advertised_endpoint = env::var("MIR2_ZONE_HOST_ADVERTISE_ADDR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| bound_rpc_address.to_string());
        let failure_domain = env::var("MIR2_ZONE_HOST_FAILURE_DOMAIN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "local".to_string());
        let heartbeat_secret = env::var("MIR2_ZONE_HOST_HEARTBEAT_SECRET")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let management_token = env::var("MIR2_ZONE_HOST_MANAGEMENT_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let signing_identity = zone_host_signing_identity_from_env()?;
        let key_generation = env::var("MIR2_ZONE_HOST_KEY_GENERATION")
            .ok()
            .map(|value| {
                value
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| "MIR2_ZONE_HOST_KEY_GENERATION must be a positive integer")
            })
            .transpose()?
            .unwrap_or(1);
        if signing_identity.is_some() && key_generation == 0 {
            return Err("MIR2_ZONE_HOST_KEY_GENERATION must be positive".to_string());
        }
        if let Some(identity) = signing_identity.as_ref() {
            if identity.node_id() != expected_host_id {
                return Err(format!(
                    "MIR2_ZONE_HOST_ID {expected_host_id} does not match signing key node id {}",
                    identity.node_id()
                ));
            }
        }
        if let Some(secret) = heartbeat_secret.as_deref() {
            if secret.as_bytes().len() < 32 {
                return Err(
                    "MIR2_ZONE_HOST_HEARTBEAT_SECRET must contain at least 32 bytes".to_string(),
                );
            }
        }
        if management_token
            .as_deref()
            .is_some_and(|token| token.as_bytes().len() < 32)
        {
            return Err(
                "MIR2_ZONE_HOST_MANAGEMENT_TOKEN must contain at least 32 bytes".to_string(),
            );
        }
        if !address.ip().is_loopback() && signing_identity.is_none() {
            return Err(
                "MIR2_ZONE_HOST_SIGNING_KEY or MIR2_ZONE_HOST_SIGNING_KEY_FILE is required when Zone Host telemetry binds to a non-loopback address"
                    .to_string(),
            );
        }
        let capacity_max_commands = env::var("MIR2_ZONE_HOST_CAPACITY_MAX_COMMANDS")
            .ok()
            .map(|value| {
                value
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| "MIR2_ZONE_HOST_CAPACITY_MAX_COMMANDS must be a positive integer")
            })
            .transpose()?
            .unwrap_or(10_000);
        if capacity_max_commands == 0 || capacity_max_commands > 1_000_000 {
            return Err(
                "MIR2_ZONE_HOST_CAPACITY_MAX_COMMANDS must be within 1..=1000000".to_string(),
            );
        }
        Ok(Self {
            address,
            advertised_endpoint,
            failure_domain,
            heartbeat_secret,
            management_token,
            signing_identity,
            key_generation,
            heartbeat_sequence: Arc::new(AtomicU64::new(1)),
            capacity_max_commands,
            capacity_challenge_inflight: Arc::new(AtomicBool::new(false)),
        })
    }

    #[cfg(test)]
    fn for_test(secret: Option<&str>) -> Self {
        Self {
            address: DEFAULT_OPERATOR_ADDR.parse().expect("valid test address"),
            advertised_endpoint: "zone-a:7020".to_string(),
            failure_domain: "test-az-a".to_string(),
            heartbeat_secret: secret.map(str::to_string),
            management_token: Some("0123456789abcdef0123456789abcdef".to_string()),
            signing_identity: None,
            key_generation: 0,
            heartbeat_sequence: Arc::new(AtomicU64::new(1)),
            capacity_max_commands: 10_000,
            capacity_challenge_inflight: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    fn for_ed25519_test(seed: [u8; 32]) -> Self {
        let signing_identity = NodeSigningIdentity::from_seed(seed);
        Self {
            address: DEFAULT_OPERATOR_ADDR.parse().expect("valid test address"),
            advertised_endpoint: "zone-a:7020".to_string(),
            failure_domain: "test-az-a".to_string(),
            heartbeat_secret: None,
            management_token: Some("0123456789abcdef0123456789abcdef".to_string()),
            signing_identity: Some(signing_identity),
            key_generation: 3,
            heartbeat_sequence: Arc::new(AtomicU64::new(1)),
            capacity_max_commands: 10_000,
            capacity_challenge_inflight: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneHostHeartbeatPayload {
    pub schema: String,
    pub host_id: String,
    pub public_key: String,
    pub key_generation: u64,
    pub advertised_endpoint: String,
    pub failure_domain: String,
    pub observed_at_ms: u64,
    pub sequence: u64,
    pub process_id: u32,
    pub protocol_version: u16,
    pub session_count: usize,
    pub session_capacity: usize,
    pub session_capacity_per_zone: usize,
    pub busiest_zone_session_count: usize,
    pub zone_count: usize,
    pub zone_capacity: usize,
    #[serde(default)]
    pub zones: Vec<ZoneHostZoneTelemetry>,
    pub active_connections: usize,
    pub draining: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedZoneHostHeartbeat {
    pub payload: ZoneHostHeartbeatPayload,
    pub signature_algorithm: String,
    pub signature: String,
}

impl SignedZoneHostHeartbeat {
    pub fn verify(&self, secret: &[u8]) -> Result<(), String> {
        self.validate_payload()?;
        if self.signature_algorithm != HMAC_SIGNATURE_ALGORITHM {
            return Err("unsupported heartbeat signature algorithm".to_string());
        }
        let bytes = serde_json::to_vec(&self.payload)
            .map_err(|error| format!("heartbeat serialization failed: {error}"))?;
        let signature = URL_SAFE_NO_PAD
            .decode(&self.signature)
            .map_err(|_| "invalid heartbeat signature encoding".to_string())?;
        let mut mac = HmacSha256::new_from_slice(secret)
            .map_err(|_| "invalid heartbeat signing secret".to_string())?;
        mac.update(&bytes);
        mac.verify_slice(&signature)
            .map_err(|_| "invalid heartbeat signature".to_string())
    }

    pub fn verify_ed25519(&self) -> Result<(), String> {
        self.validate_payload()?;
        if self.signature_algorithm != ED25519_SIGNATURE_ALGORITHM {
            return Err("unsupported heartbeat signature algorithm".to_string());
        }
        if self.payload.key_generation == 0 {
            return Err("Ed25519 heartbeat key generation must be positive".to_string());
        }
        let expected_node_id = node_id_from_public_key(&self.payload.public_key)?;
        if expected_node_id != self.payload.host_id {
            return Err(format!(
                "heartbeat node id {} does not match public key identity {expected_node_id}",
                self.payload.host_id
            ));
        }
        let bytes = serde_json::to_vec(&self.payload)
            .map_err(|error| format!("heartbeat serialization failed: {error}"))?;
        verify_ed25519_signature(&self.payload.public_key, &bytes, &self.signature)
    }

    fn validate_payload(&self) -> Result<(), String> {
        if self.payload.schema != HEARTBEAT_SCHEMA {
            return Err("unsupported heartbeat schema".to_string());
        }
        if self.payload.session_capacity == 0
            || self.payload.session_capacity_per_zone == 0
            || self.payload.session_capacity_per_zone > self.payload.session_capacity
            || self.payload.session_count > self.payload.session_capacity
            || self.payload.busiest_zone_session_count > self.payload.session_capacity_per_zone
            || self.payload.busiest_zone_session_count > self.payload.session_count
            || self.payload.zone_capacity == 0
            || self.payload.zone_count > self.payload.zone_capacity
        {
            return Err("heartbeat capacity claims are inconsistent".to_string());
        }
        if self.payload.zones.len() != self.payload.zone_count
            || self
                .payload
                .zones
                .iter()
                .map(|zone| zone.session_count)
                .sum::<usize>()
                != self.payload.session_count
        {
            return Err("heartbeat Zone details do not match aggregate counts".to_string());
        }
        let mut zone_ids = std::collections::BTreeSet::new();
        for zone in &self.payload.zones {
            if zone.zone_id.trim().is_empty()
                || zone.zone_id.len() > 160
                || zone.zone_id.chars().any(char::is_control)
                || !zone_ids.insert(zone.zone_id.as_str())
                || zone.session_count == 0
                || zone.session_count > self.payload.session_capacity_per_zone
            {
                return Err("heartbeat Zone detail is invalid".to_string());
            }
            let maps_are_valid = match zone.map_scope {
                ZoneMapScope::All | ZoneMapScope::Unknown => zone.map_file_names.is_empty(),
                ZoneMapScope::Explicit => {
                    !zone.map_file_names.is_empty()
                        && zone.map_file_names.iter().all(|map| {
                            !map.trim().is_empty()
                                && map.len() <= 160
                                && !map.chars().any(char::is_control)
                        })
                }
            };
            if !maps_are_valid {
                return Err("heartbeat Zone map membership is invalid".to_string());
            }
        }
        Ok(())
    }
}

pub fn serve_zone_host_operator(
    listener: TcpListener,
    server: Arc<ZoneHostServer>,
    config: ZoneHostOperatorConfig,
) -> io::Result<()> {
    serve_zone_host_operator_with_world_director(listener, server, config, None)
}

pub fn serve_zone_host_operator_with_world_director(
    listener: TcpListener,
    server: Arc<ZoneHostServer>,
    config: ZoneHostOperatorConfig,
    world_director: Option<Arc<WorldDirectorRuntimeService>>,
) -> io::Result<()> {
    for stream in listener.incoming() {
        let mut stream = stream?;
        let server = Arc::clone(&server);
        let config = config.clone();
        let world_director = world_director.clone();
        thread::spawn(move || {
            if let Err(error) = handle_operator_request(
                &mut stream,
                server.as_ref(),
                &config,
                world_director.as_deref(),
            ) {
                eprintln!("zone host operator request failed: {error}");
            }
        });
    }
    Ok(())
}

fn handle_operator_request(
    stream: &mut TcpStream,
    server: &ZoneHostServer,
    config: &ZoneHostOperatorConfig,
    world_director: Option<&WorldDirectorRuntimeService>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    let bytes = read_http_request(stream)?;
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed HTTP request"))?;
    let request = String::from_utf8_lossy(&bytes[..header_end]);
    let request_line = request.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();
    if method == "POST" && path == "/v1/capacity-challenge" {
        return handle_capacity_challenge(stream, server, config, &bytes[header_end + 4..]);
    }
    if method == "POST"
        && matches!(
            path,
            "/v1/world-director/finalized" | "/v1/world-director/advance"
        )
    {
        if !management_token_matches(&request, config.management_token.as_deref()) {
            return write_json_error(stream, 401, "valid management bearer token required");
        }
        let Some(world_director) = world_director else {
            return write_json_error(stream, 503, "world director runtime is not configured");
        };
        if path == "/v1/world-director/advance" {
            return match world_director.advance(now_ms()) {
                Ok(receipt) => {
                    let body = serde_json::to_vec(&receipt).map_err(io::Error::other)?;
                    write_response(stream, 200, "application/json", &body)
                }
                Err(error) => write_json_error(stream, 400, &error),
            };
        }
        let submission: FinalizedDirectorSubmission =
            match serde_json::from_slice(&bytes[header_end + 4..]) {
                Ok(submission) => submission,
                Err(error) => {
                    return write_json_error(
                        stream,
                        400,
                        &format!("invalid finalized director submission JSON: {error}"),
                    )
                }
            };
        return match world_director.install_submission(submission, now_ms()) {
            Ok(receipt) => {
                let body = serde_json::to_vec(&receipt).map_err(io::Error::other)?;
                write_response(stream, 200, "application/json", &body)
            }
            Err(error) => write_json_error(stream, 400, &error),
        };
    }
    if method == "POST" && matches!(path, "/v1/drain" | "/v1/resume") {
        if !management_token_matches(&request, config.management_token.as_deref()) {
            return write_response(
                stream,
                401,
                "application/json",
                br#"{"error":"valid management bearer token required"}"#,
            );
        }
        let draining = path == "/v1/drain";
        server.set_draining(draining);
        let body = serde_json::to_vec(&server.health()).map_err(io::Error::other)?;
        return write_response(stream, 200, "application/json", &body);
    }
    if method != "GET" {
        return write_response(
            stream,
            405,
            "text/plain; charset=utf-8",
            b"method not allowed\n",
        );
    }

    match path {
        "/v1/world-director" => {
            if !management_token_matches(&request, config.management_token.as_deref()) {
                return write_json_error(stream, 401, "valid management bearer token required");
            }
            let Some(world_director) = world_director else {
                return write_json_error(stream, 503, "world director runtime is not configured");
            };
            match world_director.status() {
                Ok(status) => {
                    let body = serde_json::to_vec(&status).map_err(io::Error::other)?;
                    write_response(stream, 200, "application/json", &body)
                }
                Err(error) => write_json_error(stream, 503, &error),
            }
        }
        "/healthz" => {
            let body =
                serde_json::to_vec(&server.telemetry_snapshot()).map_err(io::Error::other)?;
            write_response(stream, 200, "application/json", &body)
        }
        "/readyz" => {
            let health = server.health();
            let ready = !health.draining
                && health.session_count < health.session_capacity
                && health.zone_count < health.zone_capacity;
            let status = if ready { 200 } else { 503 };
            let body = serde_json::to_vec(&ReadinessResponse { ready, health })
                .map_err(io::Error::other)?;
            write_response(stream, status, "application/json", &body)
        }
        "/metrics" => {
            let mut body = render_prometheus(&server.telemetry_snapshot());
            if let Some(world_director) = world_director {
                match world_director.status() {
                    Ok(status) => body.push_str(&render_world_director_prometheus(&status)),
                    Err(_) => body.push_str(
                        "# HELP obelisk_world_director_scrape_error World director status scrape failures.\n\
                         # TYPE obelisk_world_director_scrape_error gauge\n\
                         obelisk_world_director_scrape_error 1\n",
                    ),
                }
            }
            write_response(
                stream,
                200,
                "text/plain; version=0.0.4; charset=utf-8",
                body.as_bytes(),
            )
        }
        "/v1/heartbeat" => {
            if config.signing_identity.is_some() || config.heartbeat_secret.is_some() {
                let sequence = config.heartbeat_sequence.fetch_add(1, Ordering::Relaxed);
                let snapshot = server.telemetry_snapshot();
                let heartbeat =
                    sign_heartbeat(snapshot.health, snapshot.zones, config, now_ms(), sequence)
                        .map_err(io::Error::other)?;
                let body = serde_json::to_vec(&heartbeat).map_err(io::Error::other)?;
                write_response(stream, 200, "application/json", &body)
            } else {
                write_response(
                    stream,
                    503,
                    "application/json",
                    br#"{"error":"signed heartbeat is not configured"}"#,
                )
            }
        }
        _ => write_response(stream, 404, "text/plain; charset=utf-8", b"not found\n"),
    }
}

fn render_world_director_prometheus(status: &WorldDirectorRuntimeStatus) -> String {
    format!(
        "# HELP obelisk_world_director_enabled Whether the world director runtime is enabled.\n\
# TYPE obelisk_world_director_enabled gauge\n\
obelisk_world_director_enabled 1\n\
# HELP obelisk_world_director_finalized_height Last imported Commonware control height.\n\
# TYPE obelisk_world_director_finalized_height gauge\n\
obelisk_world_director_finalized_height {}\n\
# HELP obelisk_world_director_installed_commands Installed signed world director commands.\n\
# TYPE obelisk_world_director_installed_commands gauge\n\
obelisk_world_director_installed_commands {}\n\
# HELP obelisk_world_director_applied_actions Idempotently applied world director actions.\n\
# TYPE obelisk_world_director_applied_actions gauge\n\
obelisk_world_director_applied_actions {}\n\
# HELP obelisk_world_director_spawned_monsters_total Monsters spawned by finalized world events.\n\
# TYPE obelisk_world_director_spawned_monsters_total counter\n\
obelisk_world_director_spawned_monsters_total {}\n\
# HELP obelisk_world_director_broadcast_messages_total World-event messages broadcast to Zones.\n\
# TYPE obelisk_world_director_broadcast_messages_total counter\n\
obelisk_world_director_broadcast_messages_total {}\n",
        status.finalized_height,
        status.installed_command_count,
        status.applied_action_count,
        status.spawned_monsters_total,
        status.broadcast_messages_total,
    )
}

fn management_token_matches(request: &str, expected: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    let supplied = request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("authorization")
            .then_some(value.trim())
            .and_then(|value| value.strip_prefix("Bearer "))
    });
    let Some(supplied) = supplied else {
        return false;
    };
    constant_time_bytes_equal(supplied.as_bytes(), expected.as_bytes())
}

fn constant_time_bytes_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn read_http_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut request = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    let mut expected_len = None;
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..count]);
        if request.len() > MAX_HTTP_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "operator HTTP request exceeds limit",
            ));
        }
        if expected_len.is_none() {
            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or_default();
                expected_len = Some(header_end + 4 + content_length);
            }
        }
        if expected_len.is_some_and(|length| request.len() >= length) {
            request.truncate(expected_len.expect("checked above"));
            break;
        }
    }
    Ok(request)
}

fn handle_capacity_challenge(
    stream: &mut TcpStream,
    server: &ZoneHostServer,
    config: &ZoneHostOperatorConfig,
    body: &[u8],
) -> io::Result<()> {
    if config.signing_identity.is_none() {
        return write_response(
            stream,
            503,
            "application/json",
            br#"{"error":"Ed25519 node identity is not configured"}"#,
        );
    }
    if config
        .capacity_challenge_inflight
        .swap(true, Ordering::AcqRel)
    {
        return write_response(
            stream,
            429,
            "application/json",
            br#"{"error":"capacity challenge already running"}"#,
        );
    }
    let _guard = CapacityChallengeGuard(&config.capacity_challenge_inflight);
    let challenge: CapacityChallenge = match serde_json::from_slice(body) {
        Ok(challenge) => challenge,
        Err(error) => {
            return write_json_error(
                stream,
                400,
                &format!("invalid capacity challenge JSON: {error}"),
            )
        }
    };
    let response = match run_capacity_challenge(server, config, challenge) {
        Ok(response) => response,
        Err(error) => return write_json_error(stream, 400, &error),
    };
    let body = serde_json::to_vec(&response).map_err(io::Error::other)?;
    write_response(stream, 200, "application/json", &body)
}

fn run_capacity_challenge(
    server: &ZoneHostServer,
    config: &ZoneHostOperatorConfig,
    challenge: CapacityChallenge,
) -> Result<CapacityChallengeResponse, String> {
    let identity = config
        .signing_identity
        .as_ref()
        .ok_or_else(|| "Ed25519 node identity is not configured".to_string())?;
    let started_at_ms = now_ms();
    challenge.validate(started_at_ms)?;
    let health = server.health();
    if challenge.node_id != health.host_id {
        return Err("capacity challenge targets another node".to_string());
    }
    if challenge.workload.command_count > config.capacity_max_commands {
        return Err("capacity challenge command count exceeds node limit".to_string());
    }
    if challenge.workload.concurrent_sessions > health.session_capacity
        || challenge.workload.max_sessions_per_zone > health.session_capacity_per_zone
        || challenge.workload.zone_count > health.zone_capacity
    {
        return Err("capacity challenge exceeds configured node capacity".to_string());
    }

    let mut transcript = Sha256::new();
    transcript.update(b"obelisk.capacity-remote-transcript.v1\0");
    transcript.update(challenge.challenge_id.as_bytes());
    transcript.update(challenge.nonce.as_bytes());
    let mut latencies_ms = Vec::with_capacity(challenge.workload.command_count as usize);
    for sequence in 0..challenge.workload.command_count {
        let command_started = Instant::now();
        let mut command = Sha256::new();
        command.update(b"obelisk.capacity-remote-command.v1\0");
        command.update(challenge.nonce.as_bytes());
        command.update(sequence.to_be_bytes());
        command.update((sequence % challenge.workload.concurrent_sessions as u64).to_be_bytes());
        command.update((sequence % challenge.workload.zone_count as u64).to_be_bytes());
        transcript.update(command.finalize());
        latencies_ms.push(command_started.elapsed().as_millis().max(1) as u64);
    }
    latencies_ms.sort_unstable();
    let p95_index = latencies_ms
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    let p95_latency_ms = latencies_ms.get(p95_index).copied().unwrap_or(1);
    let observed_at_ms = now_ms();
    CapacityChallengeResponse::sign(
        challenge,
        identity,
        config.key_generation,
        latencies_ms.len() as u64,
        0,
        p95_latency_ms,
        hex_digest(transcript.finalize().as_slice()),
        observed_at_ms,
    )
}

struct CapacityChallengeGuard<'a>(&'a AtomicBool);

impl Drop for CapacityChallengeGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn write_json_error(stream: &mut TcpStream, status: u16, error: &str) -> io::Result<()> {
    let body =
        serde_json::to_vec(&serde_json::json!({ "error": error })).map_err(io::Error::other)?;
    write_response(stream, status, "application/json", &body)
}

fn sign_heartbeat(
    health: ZoneHostHealth,
    zones: Vec<ZoneHostZoneTelemetry>,
    config: &ZoneHostOperatorConfig,
    observed_at_ms: u64,
    sequence: u64,
) -> Result<SignedZoneHostHeartbeat, String> {
    let (public_key, key_generation) = config
        .signing_identity
        .as_ref()
        .map(|identity| (identity.public_key().to_string(), config.key_generation))
        .unwrap_or_default();
    let payload = ZoneHostHeartbeatPayload {
        schema: HEARTBEAT_SCHEMA.to_string(),
        host_id: health.host_id,
        public_key,
        key_generation,
        advertised_endpoint: config.advertised_endpoint.clone(),
        failure_domain: config.failure_domain.clone(),
        observed_at_ms,
        sequence,
        process_id: health.process_id,
        protocol_version: health.protocol_version,
        session_count: health.session_count,
        session_capacity: health.session_capacity,
        session_capacity_per_zone: health.session_capacity_per_zone,
        busiest_zone_session_count: health.busiest_zone_session_count,
        zone_count: health.zone_count,
        zone_capacity: health.zone_capacity,
        zones,
        active_connections: health.active_connections,
        draining: health.draining,
    };
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("heartbeat serialization failed: {error}"))?;
    let (signature_algorithm, signature) = if let Some(identity) = config.signing_identity.as_ref()
    {
        (
            ED25519_SIGNATURE_ALGORITHM.to_string(),
            identity.sign(&bytes),
        )
    } else if let Some(secret) = config.heartbeat_secret.as_deref() {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|_| "invalid heartbeat signing secret".to_string())?;
        mac.update(&bytes);
        (
            HMAC_SIGNATURE_ALGORITHM.to_string(),
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()),
        )
    } else {
        return Err("signed heartbeat is not configured".to_string());
    };
    Ok(SignedZoneHostHeartbeat {
        payload,
        signature_algorithm,
        signature,
    })
}

fn render_prometheus(snapshot: &ZoneHostTelemetrySnapshot) -> String {
    let health = &snapshot.health;
    let host_id = prometheus_label(&health.host_id);
    let mut output = String::new();
    macro_rules! metric {
        ($help:literal, $kind:literal, $name:literal, $value:expr) => {
            output.push_str(concat!("# HELP ", $name, " ", $help, "\n"));
            output.push_str(concat!("# TYPE ", $name, " ", $kind, "\n"));
            output.push_str(&format!(
                concat!($name, "{{host_id=\"{}\"}} {}\n"),
                host_id, $value
            ));
        };
    }
    metric!(
        "Zone Host process readiness.",
        "gauge",
        "obelisk_zone_host_up",
        1
    );
    metric!(
        "Zone Host process uptime in seconds.",
        "gauge",
        "obelisk_zone_host_uptime_seconds",
        snapshot.uptime_seconds
    );
    metric!(
        "Currently hosted sessions.",
        "gauge",
        "obelisk_zone_host_sessions",
        health.session_count
    );
    metric!(
        "Configured session capacity.",
        "gauge",
        "obelisk_zone_host_session_capacity",
        health.session_capacity
    );
    metric!(
        "Configured session capacity per Zone.",
        "gauge",
        "obelisk_zone_host_session_capacity_per_zone",
        health.session_capacity_per_zone
    );
    metric!(
        "Current session count in the busiest Zone.",
        "gauge",
        "obelisk_zone_host_busiest_zone_sessions",
        health.busiest_zone_session_count
    );
    metric!(
        "Currently hosted Zones.",
        "gauge",
        "obelisk_zone_host_zones",
        health.zone_count
    );
    metric!(
        "Configured Zone capacity.",
        "gauge",
        "obelisk_zone_host_zone_capacity",
        health.zone_capacity
    );
    metric!(
        "Currently active Zone RPC connections.",
        "gauge",
        "obelisk_zone_host_active_connections",
        health.active_connections
    );
    metric!(
        "Whether the Zone Host is draining.",
        "gauge",
        "obelisk_zone_host_draining",
        u8::from(health.draining)
    );
    metric!(
        "Accepted Zone RPC connections.",
        "counter",
        "obelisk_zone_host_connections_total",
        snapshot.accepted_connections_total
    );
    metric!(
        "Zone RPC requests received.",
        "counter",
        "obelisk_zone_host_rpc_requests_total",
        snapshot.rpc_requests_total
    );
    metric!(
        "Zone RPC requests that returned an error.",
        "counter",
        "obelisk_zone_host_rpc_errors_total",
        snapshot.rpc_errors_total
    );
    metric!(
        "Nanoseconds spent decoding, dispatching, and encoding Zone RPC requests.",
        "counter",
        "obelisk_zone_host_rpc_service_duration_ns_total",
        snapshot.rpc_service_duration_ns_total
    );
    metric!(
        "Maximum nanoseconds spent decoding, dispatching, and encoding one Zone RPC request.",
        "gauge",
        "obelisk_zone_host_rpc_service_duration_ns_max",
        snapshot.rpc_service_duration_ns_max
    );
    metric!(
        "Encoded Zone RPC response bytes.",
        "counter",
        "obelisk_zone_host_rpc_response_bytes_total",
        snapshot.rpc_response_bytes_total
    );
    metric!(
        "Largest encoded Zone RPC response in bytes.",
        "gauge",
        "obelisk_zone_host_rpc_response_bytes_max",
        snapshot.rpc_response_bytes_max
    );
    metric!(
        "Nanoseconds spent waiting for per-Zone mutation lanes.",
        "counter",
        "obelisk_zone_host_zone_gate_wait_duration_ns_total",
        snapshot.zone_gate_wait_duration_ns_total
    );
    metric!(
        "Maximum nanoseconds spent waiting for a per-Zone mutation lane.",
        "gauge",
        "obelisk_zone_host_zone_gate_wait_duration_ns_max",
        snapshot.zone_gate_wait_duration_ns_max
    );
    metric!(
        "Execute RPC requests received.",
        "counter",
        "obelisk_zone_host_execute_requests_total",
        snapshot.execute_requests_total
    );
    metric!(
        "Nanoseconds spent executing gameplay runtime commands.",
        "counter",
        "obelisk_zone_host_execute_runtime_duration_ns_total",
        snapshot.execute_runtime_duration_ns_total
    );
    metric!(
        "Nanoseconds spent appending accepted gameplay commands to the journal.",
        "counter",
        "obelisk_zone_host_execute_journal_duration_ns_total",
        snapshot.execute_journal_duration_ns_total
    );
    metric!(
        "Current v4 checkpoint journal entries.",
        "gauge",
        "obelisk_zone_host_checkpoint_journal_entries",
        snapshot.checkpoint.journal_entries
    );
    metric!(
        "Successful durable-prefix journal compactions.",
        "counter",
        "obelisk_zone_host_journal_compactions_total",
        snapshot.checkpoint.journal_compactions_total
    );
    metric!(
        "Journal entries removed by durable-prefix compaction.",
        "counter",
        "obelisk_zone_host_journal_compacted_entries_total",
        snapshot.checkpoint.journal_compacted_entries_total
    );
    metric!(
        "Successful v4 checkpoint exports.",
        "counter",
        "obelisk_zone_host_checkpoint_exports_total",
        snapshot.checkpoint.exports_total
    );
    metric!(
        "Bytes produced by successful v4 checkpoint exports.",
        "counter",
        "obelisk_zone_host_checkpoint_export_bytes_total",
        snapshot.checkpoint.export_bytes_total
    );
    metric!(
        "Nanoseconds spent producing successful v4 checkpoint exports.",
        "counter",
        "obelisk_zone_host_checkpoint_export_duration_ns_total",
        snapshot.checkpoint.export_duration_ns_total
    );
    metric!(
        "Bytes in the last successful v4 checkpoint export.",
        "gauge",
        "obelisk_zone_host_checkpoint_export_last_bytes",
        snapshot.checkpoint.export_last_bytes
    );
    metric!(
        "Nanoseconds spent in the last successful v4 checkpoint export.",
        "gauge",
        "obelisk_zone_host_checkpoint_export_last_duration_ns",
        snapshot.checkpoint.export_last_duration_ns
    );
    metric!(
        "Successful v4 checkpoint installs.",
        "counter",
        "obelisk_zone_host_checkpoint_installs_total",
        snapshot.checkpoint.installs_total
    );
    metric!(
        "Bytes consumed by successful v4 checkpoint installs.",
        "counter",
        "obelisk_zone_host_checkpoint_install_bytes_total",
        snapshot.checkpoint.install_bytes_total
    );
    metric!(
        "Nanoseconds spent applying successful v4 checkpoint installs.",
        "counter",
        "obelisk_zone_host_checkpoint_install_duration_ns_total",
        snapshot.checkpoint.install_duration_ns_total
    );
    metric!(
        "Bytes in the last successful v4 checkpoint install.",
        "gauge",
        "obelisk_zone_host_checkpoint_install_last_bytes",
        snapshot.checkpoint.install_last_bytes
    );
    metric!(
        "Nanoseconds spent in the last successful v4 checkpoint install.",
        "gauge",
        "obelisk_zone_host_checkpoint_install_last_duration_ns",
        snapshot.checkpoint.install_last_duration_ns
    );
    metric!(
        "Journal entries replayed by successful v4 checkpoint installs.",
        "counter",
        "obelisk_zone_host_checkpoint_replay_entries_total",
        snapshot.checkpoint.replay_entries_total
    );
    metric!(
        "Journal entries replayed by the last successful v4 checkpoint install.",
        "gauge",
        "obelisk_zone_host_checkpoint_replay_last_entries",
        snapshot.checkpoint.replay_last_entries
    );
    metric!(
        "Standby promotion readiness assessments.",
        "counter",
        "obelisk_zone_host_promotion_assessments_total",
        snapshot.promotion.assessments_total
    );
    metric!(
        "Standby promotion readiness assessments that issued a receipt.",
        "counter",
        "obelisk_zone_host_promotion_ready_assessments_total",
        snapshot.promotion.ready_assessments_total
    );
    metric!(
        "Fenced standby promotion attempts.",
        "counter",
        "obelisk_zone_host_promotion_attempts_total",
        snapshot.promotion.promotion_attempts_total
    );
    metric!(
        "Successful fenced standby promotions.",
        "counter",
        "obelisk_zone_host_promotions_total",
        snapshot.promotion.promotions_total
    );
    metric!(
        "Unix timestamp in milliseconds of the last successful promotion.",
        "gauge",
        "obelisk_zone_host_promotion_last_promoted_at_ms",
        snapshot.promotion.last_promoted_at_ms
    );
    metric!(
        "Number of Zones with an unexpired promotion readiness receipt.",
        "gauge",
        "obelisk_zone_host_promotion_ready_zones",
        snapshot.promotion.ready_zone_ids.len()
    );
    output.push_str(
        "# HELP obelisk_zone_host_build_info Static Zone Host build and protocol information.\n",
    );
    output.push_str("# TYPE obelisk_zone_host_build_info gauge\n");
    output.push_str(&format!(
        "obelisk_zone_host_build_info{{host_id=\"{}\",version=\"{}\",protocol_version=\"{}\"}} 1\n",
        host_id,
        env!("CARGO_PKG_VERSION"),
        health.protocol_version
    ));
    output
}

fn prometheus_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Serialize)]
struct ReadinessResponse {
    ready: bool,
    health: ZoneHostHealth,
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        404 => "Not Found",
        405 => "Method Not Allowed",
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapacityWorkload, FinalizedGuildNodeRegistration, GatewayConfig, GuildNodeStatus,
        InMemoryZoneOwnerLeaseAuthority, SharedInProcessZoneRuntimeFactory, SuiFinalityProof,
        ZoneRpcLimits,
    };

    fn health() -> ZoneHostHealth {
        ZoneHostHealth {
            host_id: "guild-a\"node".to_string(),
            process_id: 7,
            session_count: 3,
            active_connections: 2,
            session_capacity: 64,
            session_capacity_per_zone: 16,
            busiest_zone_session_count: 3,
            zone_count: 1,
            zone_capacity: 8,
            draining: false,
            protocol_version: 6,
        }
    }

    fn zones() -> Vec<ZoneHostZoneTelemetry> {
        vec![ZoneHostZoneTelemetry {
            zone_id: "map:0".to_string(),
            map_scope: ZoneMapScope::Explicit,
            map_file_names: vec!["0".to_string()],
            session_count: 3,
        }]
    }

    #[test]
    fn signed_heartbeat_round_trips_and_rejects_wrong_secret() {
        let secret = b"0123456789abcdef0123456789abcdef";
        let heartbeat = sign_heartbeat(
            health(),
            zones(),
            &ZoneHostOperatorConfig::for_test(Some("0123456789abcdef0123456789abcdef")),
            42,
            7,
        )
        .expect("heartbeat should sign");
        heartbeat.verify(secret).expect("signature should verify");
        assert!(heartbeat
            .verify(b"abcdef0123456789abcdef0123456789")
            .is_err());
        assert_eq!(heartbeat.payload.observed_at_ms, 42);
        assert_eq!(heartbeat.payload.sequence, 7);
        assert_eq!(heartbeat.payload.failure_domain, "test-az-a");
        assert_eq!(heartbeat.payload.schema, HEARTBEAT_SCHEMA);
        let mut missing_zone_details = heartbeat.clone();
        missing_zone_details.payload.zones.clear();
        assert!(missing_zone_details.validate_payload().is_err());
    }

    #[test]
    fn ed25519_heartbeat_binds_node_id_and_key_generation() {
        let config = ZoneHostOperatorConfig::for_ed25519_test([9; 32]);
        let mut health = health();
        health.host_id = config
            .signing_identity
            .as_ref()
            .expect("test identity")
            .node_id()
            .to_string();
        let heartbeat =
            sign_heartbeat(health, zones(), &config, 42, 7).expect("heartbeat should sign");
        heartbeat.verify_ed25519().unwrap();
        assert_eq!(heartbeat.payload.key_generation, 3);
        assert!(!heartbeat.payload.public_key.is_empty());
        let mut tampered = heartbeat.clone();
        tampered.payload.session_count += 1;
        assert!(tampered.verify_ed25519().is_err());
    }

    #[test]
    fn prometheus_output_is_low_cardinality_and_escapes_host_id() {
        let snapshot = ZoneHostTelemetrySnapshot {
            health: health(),
            zones: zones(),
            checkpoint: Default::default(),
            promotion: Default::default(),
            started_at_ms: 1,
            uptime_seconds: 9,
            accepted_connections_total: 10,
            rpc_requests_total: 11,
            rpc_errors_total: 1,
            rpc_service_duration_ns_total: 12,
            rpc_service_duration_ns_max: 7,
            rpc_response_bytes_total: 13,
            rpc_response_bytes_max: 8,
            zone_gate_wait_duration_ns_total: 14,
            zone_gate_wait_duration_ns_max: 9,
            execute_requests_total: 3,
            execute_runtime_duration_ns_total: 15,
            execute_journal_duration_ns_total: 16,
        };
        let output = render_prometheus(&snapshot);
        assert!(output.contains("obelisk_zone_host_sessions{host_id=\"guild-a\\\"node\"} 3"));
        assert!(output.contains("obelisk_zone_host_rpc_requests_total"));
        assert!(output.contains("obelisk_zone_host_rpc_service_duration_ns_total"));
        assert!(output.contains("obelisk_zone_host_session_capacity_per_zone"));
        assert!(output.contains("obelisk_zone_host_busiest_zone_sessions"));
        assert!(output.contains("obelisk_zone_host_checkpoint_journal_entries"));
        assert!(output.contains("obelisk_zone_host_journal_compactions_total"));
        assert!(output.contains("obelisk_zone_host_journal_compacted_entries_total"));
        assert!(output.contains("obelisk_zone_host_checkpoint_replay_entries_total"));
        assert!(output.contains("obelisk_zone_host_promotion_assessments_total"));
        assert!(output.contains("obelisk_zone_host_promotions_total"));
        assert!(!output.contains("session_id"));
        assert!(!output.contains("account_id"));
    }

    #[test]
    fn heartbeat_secret_must_have_adequate_entropy_length() {
        env::set_var("MIR2_ZONE_HOST_HEARTBEAT_SECRET", "short");
        let result =
            ZoneHostOperatorConfig::from_env("127.0.0.1:7020".parse().unwrap(), "local-host");
        env::remove_var("MIR2_ZONE_HOST_HEARTBEAT_SECRET");
        assert!(result.is_err());
    }

    #[test]
    fn management_bearer_token_is_required_and_compared_exactly() {
        let token = "0123456789abcdef0123456789abcdef";
        assert!(management_token_matches(
            &format!("POST /v1/drain HTTP/1.1\r\nAuthorization: Bearer {token}\r\n"),
            Some(token),
        ));
        assert!(!management_token_matches(
            "POST /v1/drain HTTP/1.1\r\n",
            Some(token),
        ));
        assert!(!management_token_matches(
            "POST /v1/drain HTTP/1.1\r\nAuthorization: Bearer wrong\r\n",
            Some(token),
        ));
        assert!(!management_token_matches(
            &format!("POST /v1/drain HTTP/1.1\r\nAuthorization: Bearer {token}\r\n"),
            None,
        ));
    }

    #[test]
    fn management_token_must_have_adequate_entropy_length() {
        env::set_var("MIR2_ZONE_HOST_MANAGEMENT_TOKEN", "short");
        let result =
            ZoneHostOperatorConfig::from_env("127.0.0.1:7020".parse().unwrap(), "local-host");
        env::remove_var("MIR2_ZONE_HOST_MANAGEMENT_TOKEN");
        assert!(result.is_err());
    }

    #[test]
    fn world_director_operator_status_is_authenticated_and_exported_as_metrics() {
        let config = ZoneHostOperatorConfig::for_test(None);
        let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
        let server = Arc::new(ZoneHostServer::with_identity_and_factory(
            "operator-test-host",
            8,
            GatewayConfig::default(),
            Arc::new(InMemoryZoneOwnerLeaseAuthority::new()),
            None,
            ZoneRpcLimits::default(),
            Arc::clone(&factory),
        ));
        let director_identity = NodeSigningIdentity::from_seed([19; 32]);
        let director = Arc::new(
            WorldDirectorRuntimeService::new(
                [
                    "validator-a".to_string(),
                    "validator-b".to_string(),
                    "validator-c".to_string(),
                    "validator-d".to_string(),
                ],
                director_identity.public_key(),
                factory,
                None,
            )
            .unwrap(),
        );

        let unauthorized = operator_request(
            Arc::clone(&server),
            config.clone(),
            Some(Arc::clone(&director)),
            "GET /v1/world-director HTTP/1.1\r\nHost: local\r\n\r\n",
        );
        assert!(unauthorized.starts_with("HTTP/1.1 401"));

        let token = "0123456789abcdef0123456789abcdef";
        let authorized = operator_request(
            Arc::clone(&server),
            config.clone(),
            Some(Arc::clone(&director)),
            &format!(
                "GET /v1/world-director HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer {token}\r\n\r\n"
            ),
        );
        assert!(authorized.starts_with("HTTP/1.1 200"));
        assert!(authorized.contains("\"enabled\":true"));
        assert!(authorized.contains("\"finalizedHeight\":0"));

        let metrics = operator_request(
            server,
            config,
            Some(director),
            "GET /metrics HTTP/1.1\r\nHost: local\r\n\r\n",
        );
        assert!(metrics.starts_with("HTTP/1.1 200"));
        assert!(metrics.contains("obelisk_world_director_enabled 1"));
        assert!(metrics.contains("obelisk_world_director_finalized_height 0"));
    }

    #[test]
    fn public_operator_bind_requires_signed_heartbeats() {
        env::remove_var("MIR2_ZONE_HOST_HEARTBEAT_SECRET");
        env::remove_var("MIR2_ZONE_HOST_SIGNING_KEY");
        env::remove_var("MIR2_ZONE_HOST_SIGNING_KEY_FILE");
        env::set_var("MIR2_ZONE_HOST_METRICS_ADDR", "0.0.0.0:9100");
        let result =
            ZoneHostOperatorConfig::from_env("127.0.0.1:7020".parse().unwrap(), "local-host");
        env::remove_var("MIR2_ZONE_HOST_METRICS_ADDR");
        assert!(result.is_err());
    }

    #[test]
    fn remote_capacity_challenge_is_bounded_and_signed_by_node_identity() {
        let config = ZoneHostOperatorConfig::for_ed25519_test([9; 32]);
        let identity = config.signing_identity.as_ref().unwrap();
        let server = ZoneHostServer::with_identity_and_factory(
            identity.node_id(),
            8,
            GatewayConfig::default(),
            Arc::new(InMemoryZoneOwnerLeaseAuthority::new()),
            None,
            ZoneRpcLimits {
                max_sessions: 64,
                max_sessions_per_zone: 32,
                ..ZoneRpcLimits::default()
            },
            Arc::new(SharedInProcessZoneRuntimeFactory::new()),
        );
        let now = now_ms();
        let challenge = CapacityChallenge {
            challenge_id: "remote-capacity-1".to_string(),
            node_id: identity.node_id().to_string(),
            nonce: URL_SAFE_NO_PAD.encode([7_u8; 32]),
            issued_at_ms: now.saturating_sub(100),
            expires_at_ms: now.saturating_add(10_000),
            workload: CapacityWorkload {
                concurrent_sessions: 4,
                max_sessions_per_zone: 2,
                zone_count: 2,
                command_count: 100,
                maximum_p95_latency_ms: 100,
                minimum_success_bps: 10_000,
            },
        };
        let response = run_capacity_challenge(&server, &config, challenge).unwrap();
        let registration = FinalizedGuildNodeRegistration {
            node_id: identity.node_id().to_string(),
            operator_sui_address: format!("0x{}", "11".repeat(32)),
            public_key: identity.public_key().to_string(),
            endpoint: "node-a:7020".to_string(),
            failure_domain: "test-az-a".to_string(),
            stake_mist: 2_000_000,
            max_sessions: 64,
            max_zones: 8,
            key_generation: 3,
            status: GuildNodeStatus::Active,
            finality: SuiFinalityProof {
                network: "testnet".to_string(),
                package_id: format!("0x{}", "22".repeat(32)),
                transaction_digest: "register-node-a".to_string(),
                event_sequence: 0,
                checkpoint: 42,
            },
        };
        response.verify(&registration, now_ms()).unwrap();
        assert_eq!(response.completed_commands, 100);
        assert_eq!(response.failed_commands, 0);
        assert_eq!(response.challenge.workload.max_sessions_per_zone, 2);

        let mut oversized = response.challenge;
        oversized.workload.command_count = config.capacity_max_commands + 1;
        assert!(run_capacity_challenge(&server, &config, oversized).is_err());

        let mut oversized_per_zone = CapacityChallenge {
            challenge_id: "remote-capacity-per-zone".to_string(),
            node_id: identity.node_id().to_string(),
            nonce: URL_SAFE_NO_PAD.encode([8_u8; 32]),
            issued_at_ms: now.saturating_sub(100),
            expires_at_ms: now.saturating_add(10_000),
            workload: CapacityWorkload {
                concurrent_sessions: 64,
                max_sessions_per_zone: 33,
                zone_count: 2,
                command_count: 100,
                maximum_p95_latency_ms: 100,
                minimum_success_bps: 10_000,
            },
        };
        assert!(run_capacity_challenge(&server, &config, oversized_per_zone.clone()).is_err());
        oversized_per_zone.workload.max_sessions_per_zone = 32;
        assert!(run_capacity_challenge(&server, &config, oversized_per_zone).is_ok());
    }

    fn operator_request(
        server: Arc<ZoneHostServer>,
        config: ZoneHostOperatorConfig,
        world_director: Option<Arc<WorldDirectorRuntimeService>>,
        request: &str,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            handle_operator_request(
                &mut stream,
                server.as_ref(),
                &config,
                world_director.as_deref(),
            )
            .unwrap();
        });
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        worker.join().unwrap();
        response
    }
}

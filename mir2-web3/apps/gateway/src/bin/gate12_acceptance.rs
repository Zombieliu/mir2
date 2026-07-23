use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mir2_gateway::{
    CapacityChallenge, CapacityChallengeResponse, CapacityWorkload, SignedZoneHostHeartbeat,
    ZoneHostTelemetrySnapshot,
};
use mir2_protocol::{
    decode_server_packet, encode_client_packet, ClientPacket, MirDirection, ServerPacket,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;

const DEFAULT_GATEWAY_ADDR: &str = "gateway:7000";
const DEFAULT_PRIMARY_OPERATOR_ADDR: &str = "zone-host-a:9100";
const DEFAULT_STANDBY_OPERATOR_ADDR: &str = "zone-host-b:9100";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate12AcceptanceEvidence {
    gate: &'static str,
    accepted: bool,
    generated_at_ms: u64,
    gateway_address: String,
    primary_host_id: String,
    standby_host_id: String,
    primary_session_count_before_failure: usize,
    standby_session_count_before_failure: usize,
    standby_session_count_after_failure: usize,
    primary_heartbeat_verified: bool,
    standby_heartbeat_verified: bool,
    primary_remote_capacity_verified: bool,
    standby_remote_capacity_verified: bool,
    prometheus_metric_verified: bool,
    post_failure_user_location_observed: bool,
    post_failure_packet_count: usize,
    standby_rpc_requests_before_failure: u64,
    standby_rpc_requests_after_failure: u64,
    failover_observed_ms: u128,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("gate12 acceptance failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let gateway_address =
        env::var("GATE12_GATEWAY_ADDR").unwrap_or_else(|_| DEFAULT_GATEWAY_ADDR.to_string());
    let primary_operator = env::var("GATE12_PRIMARY_OPERATOR_ADDR")
        .unwrap_or_else(|_| DEFAULT_PRIMARY_OPERATOR_ADDR.to_string());
    let standby_operator = env::var("GATE12_STANDBY_OPERATOR_ADDR")
        .unwrap_or_else(|_| DEFAULT_STANDBY_OPERATOR_ADDR.to_string());
    let heartbeat_secret = env::var("MIR2_ZONE_HOST_HEARTBEAT_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let evidence_dir =
        PathBuf::from(env::var("GATE12_EVIDENCE_DIR").unwrap_or_else(|_| "/evidence".to_string()));
    fs::create_dir_all(&evidence_dir)
        .map_err(|error| format!("failed to create evidence directory: {error}"))?;

    let primary_before = wait_for_health(&primary_operator, |_| true, Duration::from_secs(60))?;
    let standby_empty = wait_for_health(
        &standby_operator,
        |snapshot| snapshot.health.session_count == 0,
        Duration::from_secs(60),
    )?;
    let primary_heartbeat = fetch_heartbeat(&primary_operator, heartbeat_secret.as_deref())?;
    let standby_heartbeat = fetch_heartbeat(&standby_operator, heartbeat_secret.as_deref())?;
    let primary_capacity =
        run_capacity_challenge(&primary_operator, &primary_heartbeat.payload.host_id)?;
    let standby_capacity =
        run_capacity_challenge(&standby_operator, &standby_heartbeat.payload.host_id)?;
    let metrics = http_get(&primary_operator, "/metrics")?;
    let metric_verified = metrics.status == 200
        && metrics
            .body
            .contains("obelisk_zone_host_rpc_requests_total")
        && !metrics.body.contains("account_id")
        && !metrics.body.contains("session_id");
    if !metric_verified {
        return Err("primary Prometheus endpoint did not expose the expected safe metrics".into());
    }

    let mut gateway = TcpStream::connect(&gateway_address)
        .map_err(|error| format!("failed to connect gateway {gateway_address}: {error}"))?;
    gateway
        .set_read_timeout(Some(Duration::from_millis(750)))
        .map_err(|error| format!("failed to configure gateway read timeout: {error}"))?;
    let _ = read_available(&mut gateway, Duration::from_secs(2))?;
    send(
        &mut gateway,
        ClientPacket::ClientVersion {
            version_hash: Vec::new(),
        },
    )?;
    let _ = read_available(&mut gateway, Duration::from_secs(2))?;
    send(
        &mut gateway,
        ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        },
    )?;
    let login_packets = read_available(&mut gateway, Duration::from_secs(3))?;
    if !login_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. }))
    {
        return Err(format!(
            "demo login did not succeed; received {} packets",
            login_packets.len()
        ));
    }
    send(&mut gateway, ClientPacket::StartGame { character_index: 0 })?;
    let start_packets = read_available(&mut gateway, Duration::from_secs(5))?;
    if !start_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { .. }))
    {
        return Err(format!(
            "StartGame did not succeed; received {} packets",
            start_packets.len()
        ));
    }

    let primary_ready = wait_for_health(
        &primary_operator,
        |snapshot| snapshot.health.session_count >= 1,
        Duration::from_secs(60),
    )?;
    let standby_ready = wait_for_health(
        &standby_operator,
        |snapshot| snapshot.health.session_count >= 1,
        Duration::from_secs(90),
    )?;
    fs::write(evidence_dir.join("primary-ready"), b"ready\n")
        .map_err(|error| format!("failed to write primary-ready marker: {error}"))?;
    wait_for_file(
        &evidence_dir.join("continue-after-primary-stop"),
        Duration::from_secs(90),
    )?;

    let failover_started = Instant::now();
    send(
        &mut gateway,
        ClientPacket::Walk {
            direction: MirDirection::Right,
        },
    )?;
    send(
        &mut gateway,
        ClientPacket::Chat {
            message: "gate12 failover probe".to_string(),
            linked_items: Vec::new(),
        },
    )?;
    let post_failure_packets = read_available(&mut gateway, Duration::from_secs(10))?;
    let failover_observed_ms = failover_started.elapsed().as_millis();
    let user_location_observed = post_failure_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserLocation { .. }));
    if !user_location_observed {
        return Err(format!(
            "no authoritative UserLocation was received after primary failure; packets={post_failure_packets:?}"
        ));
    }

    let standby_after = wait_for_health(
        &standby_operator,
        |snapshot| {
            snapshot.rpc_requests_total > standby_ready.rpc_requests_total
                && snapshot.health.session_count >= 1
        },
        Duration::from_secs(30),
    )?;
    let evidence = Gate12AcceptanceEvidence {
        gate: "12-distribution-node-telemetry",
        accepted: true,
        generated_at_ms: now_ms(),
        gateway_address,
        primary_host_id: primary_before.health.host_id,
        standby_host_id: standby_empty.health.host_id,
        primary_session_count_before_failure: primary_ready.health.session_count,
        standby_session_count_before_failure: standby_ready.health.session_count,
        standby_session_count_after_failure: standby_after.health.session_count,
        primary_heartbeat_verified: true,
        standby_heartbeat_verified: true,
        primary_remote_capacity_verified: primary_capacity,
        standby_remote_capacity_verified: standby_capacity,
        prometheus_metric_verified: metric_verified,
        post_failure_user_location_observed: user_location_observed,
        post_failure_packet_count: post_failure_packets.len(),
        standby_rpc_requests_before_failure: standby_ready.rpc_requests_total,
        standby_rpc_requests_after_failure: standby_after.rpc_requests_total,
        failover_observed_ms,
    };
    let bytes = serde_json::to_vec_pretty(&evidence)
        .map_err(|error| format!("failed to serialize acceptance evidence: {error}"))?;
    let output = evidence_dir.join("gate12-acceptance.json");
    fs::write(&output, [&bytes[..], b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    println!(
        "{}",
        String::from_utf8(bytes).expect("serialized JSON should be UTF-8")
    );
    Ok(())
}

fn send(stream: &mut TcpStream, packet: ClientPacket) -> Result<(), String> {
    let bytes = encode_client_packet(&packet)
        .map_err(|error| format!("failed to encode {packet:?}: {error}"))?;
    stream
        .write_all(&bytes)
        .map_err(|error| format!("failed to send {packet:?}: {error}"))
}

fn read_available(stream: &mut TcpStream, budget: Duration) -> Result<Vec<ServerPacket>, String> {
    let deadline = Instant::now() + budget;
    let mut packets = Vec::new();
    while Instant::now() < deadline {
        let mut header = [0_u8; 2];
        match stream.read_exact(&mut header) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                if !packets.is_empty() {
                    break;
                }
                continue;
            }
            Err(error) => return Err(format!("gateway frame header read failed: {error}")),
        }
        let frame_len = u16::from_le_bytes(header) as usize;
        if frame_len < 2 {
            return Err(format!("gateway returned invalid frame length {frame_len}"));
        }
        let mut frame = vec![0_u8; frame_len];
        frame[..2].copy_from_slice(&header);
        stream
            .read_exact(&mut frame[2..])
            .map_err(|error| format!("gateway frame body read failed: {error}"))?;
        packets.push(
            decode_server_packet(&frame)
                .map_err(|error| format!("gateway packet decode failed: {error}"))?,
        );
    }
    Ok(packets)
}

fn wait_for_health(
    address: &str,
    predicate: impl Fn(&ZoneHostTelemetrySnapshot) -> bool,
    timeout: Duration,
) -> Result<ZoneHostTelemetrySnapshot, String> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    while Instant::now() < deadline {
        match http_get(address, "/healthz").and_then(|response| {
            if response.status != 200 {
                return Err(format!("health returned HTTP {}", response.status));
            }
            serde_json::from_str::<ZoneHostTelemetrySnapshot>(&response.body)
                .map_err(|error| format!("invalid health JSON: {error}"))
        }) {
            Ok(snapshot) if predicate(&snapshot) => return Ok(snapshot),
            Ok(_) => {}
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err(format!(
        "timed out waiting for health at {address}: {}",
        last_error.unwrap_or_else(|| "predicate not satisfied".to_string())
    ))
}

fn fetch_heartbeat(
    address: &str,
    legacy_secret: Option<&str>,
) -> Result<SignedZoneHostHeartbeat, String> {
    let response = http_get(address, "/v1/heartbeat")?;
    if response.status != 200 {
        return Err(format!(
            "heartbeat at {address} returned HTTP {}: {}",
            response.status, response.body
        ));
    }
    let heartbeat: SignedZoneHostHeartbeat = serde_json::from_str(&response.body)
        .map_err(|error| format!("invalid heartbeat JSON from {address}: {error}"))?;
    match heartbeat.signature_algorithm.as_str() {
        "ed25519-zip215" => heartbeat.verify_ed25519()?,
        "hmac-sha256" => heartbeat.verify(
            legacy_secret
                .ok_or_else(|| "legacy HMAC heartbeat needs a verification secret".to_string())?
                .as_bytes(),
        )?,
        algorithm => {
            return Err(format!(
                "unsupported heartbeat signature algorithm {algorithm}"
            ))
        }
    }
    Ok(heartbeat)
}

fn run_capacity_challenge(address: &str, node_id: &str) -> Result<bool, String> {
    let issued_at_ms = now_ms();
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);
    let challenge = CapacityChallenge {
        challenge_id: format!("gate12-remote-{issued_at_ms}"),
        node_id: node_id.to_string(),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        issued_at_ms,
        expires_at_ms: issued_at_ms.saturating_add(30_000),
        workload: CapacityWorkload {
            concurrent_sessions: 4,
            zone_count: 2,
            command_count: 1_000,
            maximum_p95_latency_ms: 100,
            minimum_success_bps: 10_000,
        },
    };
    let body = serde_json::to_vec(&challenge)
        .map_err(|error| format!("failed to encode capacity challenge: {error}"))?;
    let response = http_post(address, "/v1/capacity-challenge", &body)?;
    if response.status != 200 {
        return Err(format!(
            "capacity challenge at {address} returned HTTP {}: {}",
            response.status, response.body
        ));
    }
    let response: CapacityChallengeResponse = serde_json::from_str(&response.body)
        .map_err(|error| format!("invalid capacity response from {address}: {error}"))?;
    response.verify_node_claim(now_ms())?;
    Ok(true)
}

struct HttpResponse {
    status: u16,
    body: String,
}

fn http_get(address: &str, path: &str) -> Result<HttpResponse, String> {
    let mut stream = TcpStream::connect(address)
        .map_err(|error| format!("failed to connect operator endpoint {address}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("failed to set operator read timeout: {error}"))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| format!("failed to write operator request: {error}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("failed to read operator response: {error}"))?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "operator returned malformed HTTP".to_string())?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "operator returned malformed HTTP status".to_string())?;
    Ok(HttpResponse {
        status,
        body: body.to_string(),
    })
}

fn http_post(address: &str, path: &str, body: &[u8]) -> Result<HttpResponse, String> {
    let mut stream = TcpStream::connect(address)
        .map_err(|error| format!("failed to connect operator endpoint {address}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| format!("failed to set operator read timeout: {error}"))?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .map_err(|error| format!("failed to write operator request: {error}"))?;
    stream
        .write_all(body)
        .map_err(|error| format!("failed to write operator request body: {error}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("failed to read operator response: {error}"))?;
    parse_http_response(response)
}

fn parse_http_response(response: String) -> Result<HttpResponse, String> {
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "operator returned malformed HTTP".to_string())?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "operator returned malformed HTTP status".to_string())?;
    Ok(HttpResponse {
        status,
        body: body.to_string(),
    })
}

fn wait_for_file(path: &Path, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.is_file() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(format!("timed out waiting for {}", path.display()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

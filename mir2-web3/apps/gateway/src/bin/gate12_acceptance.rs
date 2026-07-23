use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_gateway::{SignedZoneHostHeartbeat, ZoneHostTelemetrySnapshot};
use mir2_protocol::{
    decode_server_packet, encode_client_packet, ClientPacket, MirDirection, ServerPacket,
};
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
    let heartbeat_secret = required_env("MIR2_ZONE_HOST_HEARTBEAT_SECRET")?;
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
    let primary_heartbeat = fetch_heartbeat(&primary_operator, heartbeat_secret.as_bytes())?;
    let standby_heartbeat = fetch_heartbeat(&standby_operator, heartbeat_secret.as_bytes())?;
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
        primary_heartbeat_verified: primary_heartbeat,
        standby_heartbeat_verified: standby_heartbeat,
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

fn fetch_heartbeat(address: &str, secret: &[u8]) -> Result<bool, String> {
    let response = http_get(address, "/v1/heartbeat")?;
    if response.status != 200 {
        return Err(format!(
            "heartbeat at {address} returned HTTP {}: {}",
            response.status, response.body
        ));
    }
    let heartbeat: SignedZoneHostHeartbeat = serde_json::from_str(&response.body)
        .map_err(|error| format!("invalid heartbeat JSON from {address}: {error}"))?;
    heartbeat.verify(secret)?;
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

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

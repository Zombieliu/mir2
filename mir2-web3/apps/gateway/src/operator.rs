use std::env;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{ZoneHostHealth, ZoneHostServer, ZoneHostTelemetrySnapshot};

const HEARTBEAT_SCHEMA: &str = "obelisk.zone-host-heartbeat.v1";
const SIGNATURE_ALGORITHM: &str = "hmac-sha256";
// An ephemeral loopback port keeps local multi-host tests and developer
// processes isolated. Deployments set an explicit stable address.
const DEFAULT_OPERATOR_ADDR: &str = "127.0.0.1:0";
const MAX_HTTP_REQUEST_BYTES: usize = 8 * 1024;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct ZoneHostOperatorConfig {
    pub address: SocketAddr,
    pub advertised_endpoint: String,
    pub failure_domain: String,
    heartbeat_secret: Option<String>,
    heartbeat_sequence: Arc<AtomicU64>,
}

impl ZoneHostOperatorConfig {
    pub fn from_env(bound_rpc_address: SocketAddr) -> Result<Self, String> {
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
        if let Some(secret) = heartbeat_secret.as_deref() {
            if secret.as_bytes().len() < 32 {
                return Err(
                    "MIR2_ZONE_HOST_HEARTBEAT_SECRET must contain at least 32 bytes".to_string(),
                );
            }
        }
        if !address.ip().is_loopback() && heartbeat_secret.is_none() {
            return Err(
                "MIR2_ZONE_HOST_HEARTBEAT_SECRET is required when Zone Host telemetry binds to a non-loopback address"
                    .to_string(),
            );
        }
        Ok(Self {
            address,
            advertised_endpoint,
            failure_domain,
            heartbeat_secret,
            heartbeat_sequence: Arc::new(AtomicU64::new(1)),
        })
    }

    #[cfg(test)]
    fn for_test(secret: Option<&str>) -> Self {
        Self {
            address: DEFAULT_OPERATOR_ADDR.parse().expect("valid test address"),
            advertised_endpoint: "zone-a:7020".to_string(),
            failure_domain: "test-az-a".to_string(),
            heartbeat_secret: secret.map(str::to_string),
            heartbeat_sequence: Arc::new(AtomicU64::new(1)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneHostHeartbeatPayload {
    pub schema: String,
    pub host_id: String,
    pub advertised_endpoint: String,
    pub failure_domain: String,
    pub observed_at_ms: u64,
    pub sequence: u64,
    pub process_id: u32,
    pub protocol_version: u16,
    pub session_count: usize,
    pub session_capacity: usize,
    pub zone_count: usize,
    pub zone_capacity: usize,
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
        if self.signature_algorithm != SIGNATURE_ALGORITHM {
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
}

pub fn serve_zone_host_operator(
    listener: TcpListener,
    server: Arc<ZoneHostServer>,
    config: ZoneHostOperatorConfig,
) -> io::Result<()> {
    for stream in listener.incoming() {
        let mut stream = stream?;
        let server = Arc::clone(&server);
        let config = config.clone();
        thread::spawn(move || {
            if let Err(error) = handle_operator_request(&mut stream, server.as_ref(), &config) {
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
) -> io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    let mut bytes = vec![0_u8; MAX_HTTP_REQUEST_BYTES];
    let count = stream.read(&mut bytes)?;
    let request = String::from_utf8_lossy(&bytes[..count]);
    let request_line = request.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();
    if method != "GET" {
        return write_response(
            stream,
            405,
            "text/plain; charset=utf-8",
            b"method not allowed\n",
        );
    }

    match path {
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
            let body = render_prometheus(&server.telemetry_snapshot());
            write_response(
                stream,
                200,
                "text/plain; version=0.0.4; charset=utf-8",
                body.as_bytes(),
            )
        }
        "/v1/heartbeat" => match config.heartbeat_secret.as_deref() {
            Some(secret) => {
                let sequence = config.heartbeat_sequence.fetch_add(1, Ordering::Relaxed);
                let heartbeat = sign_heartbeat(
                    server.health(),
                    config,
                    now_ms(),
                    sequence,
                    secret.as_bytes(),
                )
                .map_err(io::Error::other)?;
                let body = serde_json::to_vec(&heartbeat).map_err(io::Error::other)?;
                write_response(stream, 200, "application/json", &body)
            }
            None => write_response(
                stream,
                503,
                "application/json",
                br#"{"error":"signed heartbeat is not configured"}"#,
            ),
        },
        _ => write_response(stream, 404, "text/plain; charset=utf-8", b"not found\n"),
    }
}

fn sign_heartbeat(
    health: ZoneHostHealth,
    config: &ZoneHostOperatorConfig,
    observed_at_ms: u64,
    sequence: u64,
    secret: &[u8],
) -> Result<SignedZoneHostHeartbeat, String> {
    let payload = ZoneHostHeartbeatPayload {
        schema: HEARTBEAT_SCHEMA.to_string(),
        host_id: health.host_id,
        advertised_endpoint: config.advertised_endpoint.clone(),
        failure_domain: config.failure_domain.clone(),
        observed_at_ms,
        sequence,
        process_id: health.process_id,
        protocol_version: health.protocol_version,
        session_count: health.session_count,
        session_capacity: health.session_capacity,
        zone_count: health.zone_count,
        zone_capacity: health.zone_capacity,
        active_connections: health.active_connections,
        draining: health.draining,
    };
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("heartbeat serialization failed: {error}"))?;
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|_| "invalid heartbeat signing secret".to_string())?;
    mac.update(&bytes);
    let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    Ok(SignedZoneHostHeartbeat {
        payload,
        signature_algorithm: SIGNATURE_ALGORITHM.to_string(),
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

    fn health() -> ZoneHostHealth {
        ZoneHostHealth {
            host_id: "guild-a\"node".to_string(),
            process_id: 7,
            session_count: 3,
            active_connections: 2,
            session_capacity: 64,
            zone_count: 1,
            zone_capacity: 8,
            draining: false,
            protocol_version: 5,
        }
    }

    #[test]
    fn signed_heartbeat_round_trips_and_rejects_wrong_secret() {
        let secret = b"0123456789abcdef0123456789abcdef";
        let heartbeat = sign_heartbeat(
            health(),
            &ZoneHostOperatorConfig::for_test(None),
            42,
            7,
            secret,
        )
        .expect("heartbeat should sign");
        heartbeat.verify(secret).expect("signature should verify");
        assert!(heartbeat
            .verify(b"abcdef0123456789abcdef0123456789")
            .is_err());
        assert_eq!(heartbeat.payload.observed_at_ms, 42);
        assert_eq!(heartbeat.payload.sequence, 7);
        assert_eq!(heartbeat.payload.failure_domain, "test-az-a");
    }

    #[test]
    fn prometheus_output_is_low_cardinality_and_escapes_host_id() {
        let snapshot = ZoneHostTelemetrySnapshot {
            health: health(),
            started_at_ms: 1,
            uptime_seconds: 9,
            accepted_connections_total: 10,
            rpc_requests_total: 11,
            rpc_errors_total: 1,
        };
        let output = render_prometheus(&snapshot);
        assert!(output.contains("obelisk_zone_host_sessions{host_id=\"guild-a\\\"node\"} 3"));
        assert!(output.contains("obelisk_zone_host_rpc_requests_total"));
        assert!(!output.contains("session_id"));
        assert!(!output.contains("account_id"));
    }

    #[test]
    fn heartbeat_secret_must_have_adequate_entropy_length() {
        env::set_var("MIR2_ZONE_HOST_HEARTBEAT_SECRET", "short");
        let result = ZoneHostOperatorConfig::from_env("127.0.0.1:7020".parse().unwrap());
        env::remove_var("MIR2_ZONE_HOST_HEARTBEAT_SECRET");
        assert!(result.is_err());
    }

    #[test]
    fn public_operator_bind_requires_signed_heartbeats() {
        env::remove_var("MIR2_ZONE_HOST_HEARTBEAT_SECRET");
        env::set_var("MIR2_ZONE_HOST_METRICS_ADDR", "0.0.0.0:9100");
        let result = ZoneHostOperatorConfig::from_env("127.0.0.1:7020".parse().unwrap());
        env::remove_var("MIR2_ZONE_HOST_METRICS_ADDR");
        assert!(result.is_err());
    }
}

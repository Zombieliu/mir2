use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_simulation::{WorldCommandKind, WorldCommandOutcome};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::routing::ZoneId;

pub const DEFAULT_GAMEPLAY_EVENT_TOPIC: &str = "gameplay.command.executed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayGameplayEvent {
    pub event_id: String,
    pub event_type: String,
    pub schema_version: u16,
    pub occurred_at_ms: u64,
    pub zone_id: String,
    pub command_kind: String,
    pub account_id: Option<String>,
    pub character_index: Option<i32>,
    pub character_name: Option<String>,
    pub packet_count: usize,
    pub snapshot_tick: u64,
    pub payload_json: String,
}

impl GatewayGameplayEvent {
    pub fn from_world_outcome(
        zone_id: &ZoneId,
        event_sequence: u64,
        outcome: &WorldCommandOutcome,
    ) -> Self {
        let identity = outcome.active_identity.as_ref();
        let command_kind = command_kind_label(&outcome.command_kind);
        let payload = json!({
            "zoneId": zone_id.as_str(),
            "commandKind": command_kind,
            "packetCount": outcome.packet_count,
            "snapshotTick": outcome.snapshot_tick
        });
        Self {
            event_id: format!(
                "{}:{}:{}",
                zone_id.as_str(),
                outcome.snapshot_tick,
                event_sequence
            ),
            event_type: DEFAULT_GAMEPLAY_EVENT_TOPIC.to_string(),
            schema_version: 1,
            occurred_at_ms: current_unix_ms(),
            zone_id: zone_id.as_str().to_string(),
            command_kind,
            account_id: identity.map(|identity| identity.account_id.clone()),
            character_index: identity.map(|identity| identity.character_index),
            character_name: identity.map(|identity| identity.character_name.clone()),
            packet_count: outcome.packet_count,
            snapshot_tick: outcome.snapshot_tick,
            payload_json: payload.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEventSinkStatus {
    pub configured: bool,
    pub sink: String,
    pub topic: Option<String>,
    pub accepted: u64,
    pub published: u64,
    pub failed: u64,
    pub dropped: u64,
    pub last_error: Option<String>,
}

impl GameplayEventSinkStatus {
    pub fn not_configured() -> Self {
        Self {
            configured: false,
            sink: "none".to_string(),
            topic: None,
            accepted: 0,
            published: 0,
            failed: 0,
            dropped: 0,
            last_error: None,
        }
    }
}

pub trait GameplayEventSink: Send + Sync {
    fn publish(&self, event: GatewayGameplayEvent);
    fn status(&self) -> GameplayEventSinkStatus;
}

pub type SharedGameplayEventSink = Arc<dyn GameplayEventSink>;

#[derive(Clone)]
pub(crate) struct GatewayGameplayEventPublisher {
    state: Arc<Mutex<GatewayGameplayEventPublisherState>>,
}

struct GatewayGameplayEventPublisherState {
    zone_id: ZoneId,
    event_sequence: u64,
    sink: SharedGameplayEventSink,
}

impl GatewayGameplayEventPublisher {
    pub(crate) fn new(zone_id: ZoneId, sink: SharedGameplayEventSink) -> Self {
        Self {
            state: Arc::new(Mutex::new(GatewayGameplayEventPublisherState {
                zone_id,
                event_sequence: 0,
                sink,
            })),
        }
    }

    pub(crate) fn publish(&self, outcome: &WorldCommandOutcome) {
        let mut state = self
            .state
            .lock()
            .expect("gameplay event publisher mutex should not be poisoned");
        state.event_sequence = state.event_sequence.saturating_add(1);
        let event =
            GatewayGameplayEvent::from_world_outcome(&state.zone_id, state.event_sequence, outcome);
        // Keep sequence allocation and sink enqueue in the same critical section so
        // reader fast-path and session events cannot be reordered.
        state.sink.publish(event);
    }

    pub(crate) fn rebind_zone(&self, zone_id: ZoneId) {
        self.state
            .lock()
            .expect("gameplay event publisher mutex should not be poisoned")
            .zone_id = zone_id;
    }
}

#[derive(Debug, Default)]
pub struct InMemoryGameplayEventSink {
    events: Mutex<Vec<GatewayGameplayEvent>>,
}

impl InMemoryGameplayEventSink {
    pub fn drain(&self) -> Vec<GatewayGameplayEvent> {
        std::mem::take(
            &mut *self
                .events
                .lock()
                .expect("gameplay event sink mutex should not be poisoned"),
        )
    }

    pub fn list(&self) -> Vec<GatewayGameplayEvent> {
        self.events
            .lock()
            .expect("gameplay event sink mutex should not be poisoned")
            .clone()
    }
}

impl GameplayEventSink for InMemoryGameplayEventSink {
    fn publish(&self, event: GatewayGameplayEvent) {
        self.events
            .lock()
            .expect("gameplay event sink mutex should not be poisoned")
            .push(event);
    }

    fn status(&self) -> GameplayEventSinkStatus {
        let count = self
            .events
            .lock()
            .expect("gameplay event sink mutex should not be poisoned")
            .len() as u64;
        GameplayEventSinkStatus {
            configured: true,
            sink: "in_memory".to_string(),
            topic: Some(DEFAULT_GAMEPLAY_EVENT_TOPIC.to_string()),
            accepted: count,
            published: count,
            failed: 0,
            dropped: 0,
            last_error: None,
        }
    }
}

#[derive(Debug, Default)]
struct RedpandaSinkMetrics {
    accepted: AtomicU64,
    published: AtomicU64,
    failed: AtomicU64,
    dropped: AtomicU64,
    last_error: Mutex<Option<String>>,
}

#[derive(Debug)]
pub struct RedpandaGameplayEventSink {
    redpanda_url: String,
    topic: String,
    sender: mpsc::Sender<GatewayGameplayEvent>,
    metrics: Arc<RedpandaSinkMetrics>,
}

impl RedpandaGameplayEventSink {
    pub fn new(redpanda_url: impl Into<String>, topic: impl Into<String>) -> Self {
        let redpanda_url = redpanda_url.into();
        let topic = topic.into();
        let (sender, receiver) = mpsc::channel::<GatewayGameplayEvent>();
        let metrics = Arc::new(RedpandaSinkMetrics::default());
        let worker_metrics = Arc::clone(&metrics);
        let worker_url = redpanda_url.clone();
        let worker_topic = topic.clone();
        thread::Builder::new()
            .name("mir2-gameplay-redpanda-sink".to_string())
            .spawn(move || {
                while let Ok(event) = receiver.recv() {
                    match publish_redpanda(&worker_url, &worker_topic, &event) {
                        Ok(()) => {
                            worker_metrics.published.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(error) => {
                            worker_metrics.failed.fetch_add(1, Ordering::Relaxed);
                            *worker_metrics
                                .last_error
                                .lock()
                                .expect("redpanda sink metrics mutex should not be poisoned") =
                                Some(error);
                        }
                    }
                }
            })
            .expect("gameplay event Redpanda sink worker should spawn");
        Self {
            redpanda_url,
            topic,
            sender,
            metrics,
        }
    }
}

impl GameplayEventSink for RedpandaGameplayEventSink {
    fn publish(&self, event: GatewayGameplayEvent) {
        self.metrics.accepted.fetch_add(1, Ordering::Relaxed);
        if let Err(error) = self.sender.send(event) {
            self.metrics.dropped.fetch_add(1, Ordering::Relaxed);
            *self
                .metrics
                .last_error
                .lock()
                .expect("redpanda sink metrics mutex should not be poisoned") =
                Some(format!("gameplay event queue closed: {error}"));
        }
    }

    fn status(&self) -> GameplayEventSinkStatus {
        GameplayEventSinkStatus {
            configured: true,
            sink: format!("redpanda:{}", self.redpanda_url),
            topic: Some(self.topic.clone()),
            accepted: self.metrics.accepted.load(Ordering::Relaxed),
            published: self.metrics.published.load(Ordering::Relaxed),
            failed: self.metrics.failed.load(Ordering::Relaxed),
            dropped: self.metrics.dropped.load(Ordering::Relaxed),
            last_error: self
                .metrics
                .last_error
                .lock()
                .expect("redpanda sink metrics mutex should not be poisoned")
                .clone(),
        }
    }
}

#[derive(Debug, Default)]
pub struct LoggingGameplayEventSink {
    accepted: AtomicU64,
}

impl GameplayEventSink for LoggingGameplayEventSink {
    fn publish(&self, event: GatewayGameplayEvent) {
        self.accepted.fetch_add(1, Ordering::Relaxed);
        match serde_json::to_string(&event) {
            Ok(json) => eprintln!("mir2-gateway gameplay event {json}"),
            Err(error) => eprintln!("mir2-gateway gameplay event encode failed: {error}"),
        }
    }

    fn status(&self) -> GameplayEventSinkStatus {
        let accepted = self.accepted.load(Ordering::Relaxed);
        GameplayEventSinkStatus {
            configured: true,
            sink: "stderr".to_string(),
            topic: Some(DEFAULT_GAMEPLAY_EVENT_TOPIC.to_string()),
            accepted,
            published: accepted,
            failed: 0,
            dropped: 0,
            last_error: None,
        }
    }
}

pub fn default_gameplay_event_sink_from_env() -> Option<SharedGameplayEventSink> {
    if let Ok(redpanda_url) = std::env::var("MIR2_GAMEPLAY_EVENT_REDPANDA_URL") {
        if !redpanda_url.trim().is_empty() {
            let topic = std::env::var("MIR2_GAMEPLAY_EVENT_TOPIC")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_GAMEPLAY_EVENT_TOPIC.to_string());
            return Some(Arc::new(RedpandaGameplayEventSink::new(
                redpanda_url,
                topic,
            )));
        }
    }
    if std::env::var("MIR2_GAMEPLAY_EVENT_LOG")
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
    {
        return Some(Arc::new(LoggingGameplayEventSink::default()));
    }
    None
}

pub fn gameplay_event_sink_status(
    sink: Option<&SharedGameplayEventSink>,
) -> GameplayEventSinkStatus {
    sink.map(|sink| sink.status())
        .unwrap_or_else(GameplayEventSinkStatus::not_configured)
}

fn command_kind_label(kind: &WorldCommandKind) -> String {
    match kind {
        WorldCommandKind::ClientPacket(name) => format!("client.{name}"),
        WorldCommandKind::PasskeyLogin => "runtime.passkeyLogin".to_string(),
        WorldCommandKind::MoveTo => "runtime.moveTo".to_string(),
        WorldCommandKind::Attack => "runtime.attack".to_string(),
        WorldCommandKind::Interact => "runtime.interact".to_string(),
        WorldCommandKind::SelectNpcDialog => "runtime.selectNpcDialog".to_string(),
        WorldCommandKind::SubmitNpcInput => "runtime.submitNpcInput".to_string(),
        WorldCommandKind::PickUp => "runtime.pickUp".to_string(),
        WorldCommandKind::UseItem => "runtime.useItem".to_string(),
        WorldCommandKind::DropItem => "runtime.dropItem".to_string(),
        WorldCommandKind::DeleteCharacter => "runtime.deleteCharacter".to_string(),
        WorldCommandKind::CastSkill => "runtime.castSkill".to_string(),
        WorldCommandKind::TransferMap => "runtime.transferMap".to_string(),
        WorldCommandKind::ApplyHandoffTransform => "runtime.applyHandoffTransform".to_string(),
        WorldCommandKind::Stage5Command(action) => format!("stage5.{action}"),
        WorldCommandKind::GrantOnchainOre => "onchain.grantOre".to_string(),
        WorldCommandKind::CreditGoldFromOre => "onchain.creditGold".to_string(),
        WorldCommandKind::ItemRentalRequest => "runtime.itemRentalRequest".to_string(),
        WorldCommandKind::SetLanguage => "runtime.setLanguage".to_string(),
        WorldCommandKind::Tick => "runtime.tick".to_string(),
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn publish_redpanda(
    redpanda_url: &str,
    topic: &str,
    event: &GatewayGameplayEvent,
) -> Result<(), String> {
    validate_topic(topic)?;
    let value = serde_json::to_value(event)
        .map_err(|error| format!("gameplay event payload encode failed: {error}"))?;
    let body = json!({
        "records": [
            {
                "key": event.event_id,
                "value": value
            }
        ]
    });
    let body = serde_json::to_string(&body)
        .map_err(|error| format!("redpanda gameplay payload encode failed: {error}"))?;
    let url = ParsedHttpUrl::parse(redpanda_url)?;
    let path = format!("{}/topics/{topic}", url.base_path.trim_end_matches('/'));
    let response = post_http_json(&url, &path, &body)?;
    if !response.status_code_is_success() {
        return Err(format!(
            "redpanda gameplay publish failed with HTTP {}: {}",
            response.status_code,
            response.body.trim()
        ));
    }
    Ok(())
}

fn validate_topic(topic: &str) -> Result<(), String> {
    if topic.trim().is_empty() || topic.chars().any(char::is_whitespace) {
        return Err(format!("invalid gameplay event topic: {topic:?}"));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHttpUrl {
    host: String,
    port: u16,
    base_path: String,
}

impl ParsedHttpUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let Some(rest) = url.strip_prefix("http://") else {
            return Err(format!(
                "only http:// Redpanda pandaproxy URLs are supported: {url}"
            ));
        };
        let (host_port, base_path) = rest
            .split_once('/')
            .map(|(host_port, path)| (host_port, format!("/{path}")))
            .unwrap_or((rest, String::new()));
        if host_port.trim().is_empty() {
            return Err(format!("invalid Redpanda pandaproxy URL: {url}"));
        }
        let (host, port) = match host_port.rsplit_once(':') {
            Some((host, port)) => {
                let port = port
                    .parse::<u16>()
                    .map_err(|error| format!("invalid Redpanda pandaproxy port: {error}"))?;
                (host.to_string(), port)
            }
            None => (host_port.to_string(), 80),
        };
        Ok(Self {
            host,
            port,
            base_path,
        })
    }

    fn host_header(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

struct HttpResponse {
    status_code: u16,
    body: String,
}

impl HttpResponse {
    fn status_code_is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }
}

fn post_http_json(url: &ParsedHttpUrl, path: &str, body: &str) -> Result<HttpResponse, String> {
    let mut addrs = (url.host.as_str(), url.port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve Redpanda pandaproxy failed: {error}"))?;
    let addr = addrs
        .next()
        .ok_or_else(|| format!("no socket address for Redpanda pandaproxy {}", url.host))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map_err(|error| format!("redpanda pandaproxy connect failed at {addr}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("redpanda read timeout setup failed: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("redpanda write timeout setup failed: {error}"))?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/vnd.kafka.json.v2+json\r\nAccept: application/vnd.kafka.v2+json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        url.host_header(),
        body.len(),
        body
    )
    .map_err(|error| format!("redpanda pandaproxy request write failed: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("redpanda pandaproxy request flush failed: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("redpanda pandaproxy response read failed: {error}"))?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| format!("invalid Redpanda pandaproxy HTTP response: {response:?}"))?;
    let status_code = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| format!("missing Redpanda pandaproxy HTTP status: {head:?}"))?;
    Ok(HttpResponse {
        status_code,
        body: body.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        publish_redpanda, GatewayGameplayEvent, GatewayGameplayEventPublisher,
        InMemoryGameplayEventSink, ParsedHttpUrl, DEFAULT_GAMEPLAY_EVENT_TOPIC,
    };
    use crate::routing::ZoneId;
    use mir2_simulation::{WorldCommandKind, WorldCommandOutcome};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn parsed_http_url_extracts_host_port_and_base_path() {
        assert_eq!(
            ParsedHttpUrl::parse("http://127.0.0.1:8082").expect("parse"),
            ParsedHttpUrl {
                host: "127.0.0.1".into(),
                port: 8082,
                base_path: String::new()
            }
        );
        assert_eq!(
            ParsedHttpUrl::parse("http://redpanda:18082/kafka").expect("parse"),
            ParsedHttpUrl {
                host: "redpanda".into(),
                port: 18082,
                base_path: "/kafka".into()
            }
        );
    }

    #[test]
    fn gameplay_event_json_matches_clickhouse_kafka_projection_columns() {
        let event = GatewayGameplayEvent {
            event_id: "primary:0:1".into(),
            event_type: DEFAULT_GAMEPLAY_EVENT_TOPIC.into(),
            schema_version: 1,
            occurred_at_ms: 123,
            zone_id: "primary".into(),
            command_kind: "client.StartGame".into(),
            account_id: Some("demo".into()),
            character_index: Some(0),
            character_name: Some("Scout".into()),
            packet_count: 3,
            snapshot_tick: 7,
            payload_json: "{}".into(),
        };
        let value = serde_json::to_value(&event).expect("event should serialize");
        let object = value.as_object().expect("event should serialize as object");
        let clickhouse_schema =
            include_str!("../../../infra/clickhouse/initdb/002_gameplay_events.sql");

        let projected_columns = [
            ("eventId", "event_id"),
            ("eventType", "event_type"),
            ("schemaVersion", "schema_version"),
            ("occurredAtMs", "occurred_at_ms"),
            ("zoneId", "zone_id"),
            ("commandKind", "command_kind"),
            ("accountId", "account_id"),
            ("characterIndex", "character_index"),
            ("characterName", "character_name"),
            ("packetCount", "packet_count"),
            ("snapshotTick", "snapshot_tick"),
            ("payloadJson", "payload_json"),
        ];

        for (json_field, table_field) in projected_columns {
            assert!(
                object.contains_key(json_field),
                "Gateway event JSON should contain {json_field}"
            );
            assert!(
                clickhouse_schema.contains(&format!("    {json_field} ")),
                "ClickHouse Kafka schema should consume {json_field}"
            );
            assert!(
                clickhouse_schema.contains(&format!("    {table_field} ")),
                "ClickHouse MergeTree schema should store {table_field}"
            );
            assert!(
                clickhouse_schema.contains(&format!("{json_field} AS {table_field}")),
                "ClickHouse materialized view should project {json_field} into {table_field}"
            );
        }
    }

    #[test]
    fn gameplay_event_publisher_serializes_concurrent_sequences() {
        let sink = Arc::new(InMemoryGameplayEventSink::default());
        let publisher = GatewayGameplayEventPublisher::new(ZoneId::primary(), sink.clone());
        let mut workers = Vec::new();
        for _ in 0..4 {
            let publisher = publisher.clone();
            workers.push(thread::spawn(move || {
                for _ in 0..25 {
                    publisher.publish(&WorldCommandOutcome {
                        command_kind: WorldCommandKind::Tick,
                        packet_count: 0,
                        snapshot_tick: 0,
                        active_identity: None,
                    });
                }
            }));
        }
        for worker in workers {
            worker.join().expect("publisher worker should finish");
        }

        let events = sink.list();
        assert_eq!(events.len(), 100);
        for (index, event) in events.iter().enumerate() {
            assert_eq!(event.event_id, format!("primary:0:{}", index + 1));
        }
    }

    #[test]
    fn redpanda_publish_posts_gameplay_event_records() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("set read timeout");
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 512];
            loop {
                let read = stream.read(&mut buffer).expect("read request");
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                let request = String::from_utf8_lossy(&bytes);
                if let Some((head, body)) = request.split_once("\r\n\r\n") {
                    let content_length = head
                        .lines()
                        .find_map(|line| line.strip_prefix("Content-Length: "))
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or_default();
                    if body.len() >= content_length {
                        break;
                    }
                }
            }
            let request = String::from_utf8_lossy(&bytes).to_string();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                .expect("write response");
            stream.flush().expect("flush response");
            request
        });

        let event = GatewayGameplayEvent {
            event_id: "primary:0:1".into(),
            event_type: "gameplay.command.executed".into(),
            schema_version: 1,
            occurred_at_ms: 123,
            zone_id: "primary".into(),
            command_kind: "client.StartGame".into(),
            account_id: Some("demo".into()),
            character_index: Some(0),
            character_name: Some("Scout".into()),
            packet_count: 3,
            snapshot_tick: 0,
            payload_json: "{}".into(),
        };
        publish_redpanda(
            &format!("http://{}", addr),
            "gameplay.command.executed",
            &event,
        )
        .expect("publish should pass");

        let request = server.join().expect("server thread");
        assert!(request.contains("POST /topics/gameplay.command.executed HTTP/1.1"));
        assert!(request.contains("Content-Type: application/vnd.kafka.json.v2+json"));
        assert!(request.contains(r#""key":"primary:0:1""#));
        assert!(request.contains(r#""eventId":"primary:0:1""#));
        assert!(request.contains(r#""commandKind":"client.StartGame""#));
    }
}

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, TryLockError, Weak};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::{Compression, GzBuilder};
use mir2_protocol::{
    decode_client_packet, decode_server_packet, encode_client_packet, encode_server_packet,
    ClientPacket, Point, ServerPacket,
};
use mir2_simulation::{
    AccountRecord, ActiveSessionIdentity, CharacterSaveRecord, WorldCommand, WorldCommandExecution,
    WorldCommandOutcome, WorldSnapshot,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::routing::{
    HostedZoneOwnerCommandClient, SharedInProcessZoneRuntimeFactory, SharedZoneLiveOutbound,
    SharedZoneLiveOutboundRegistration, SharedZoneLiveOutboundSender, SharedZoneMutationGate,
    SharedZoneOwnerLeaseAuthority, ZoneId, ZoneLiveOutboundRegistration, ZoneOwnerCommandMode,
    ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerRpcTransport, ZoneRuntimeFactory,
};
use crate::GatewayConfig;
use crate::ZonePlacementLease;

pub const ZONE_RPC_PROTOCOL_VERSION: u16 = 7;
pub const DEFAULT_ZONE_RPC_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_ZONE_RPC_MAX_CONNECTIONS: usize = 64;
pub const DEFAULT_ZONE_RPC_MAX_SESSIONS: usize = 4096;
pub const DEFAULT_ZONE_RPC_MAX_SESSIONS_PER_ZONE: usize = 4096;
pub const DEFAULT_ZONE_RPC_MAX_OUTBOUND_MESSAGES: usize = 1024;
pub const DEFAULT_ZONE_RPC_OUTBOUND_POLL_LIMIT: usize = 128;
pub const DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES: usize = 512;
pub const DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES: usize = 1024 * 1024;
pub const DEFAULT_ZONE_BASE_SNAPSHOT_MAX_UNCOMPRESSED_BYTES: usize = 64 * 1024 * 1024;
pub const ZONE_HOST_CHECKPOINT_VERSION: u32 = 4;
pub const ZONE_REPLICATION_HEAD_VERSION: u32 = 5;
pub const ZONE_PROMOTION_READINESS_VERSION: u32 = 1;
pub const DEFAULT_ZONE_PROMOTION_MAX_LAG_MS: u64 = 250;
pub const DEFAULT_ZONE_PROMOTION_RECEIPT_TTL_MS: u64 = 30_000;
const ZONE_HOST_CHECKPOINT_DOMAIN: &[u8] = b"obelisk.mir2.zone-host-checkpoint.v4\0";
const ZONE_REPLICATION_HEAD_DOMAIN: &[u8] = b"obelisk.mir2.zone-replication-head.v5\0";
const ZONE_BASE_SNAPSHOT_DOMAIN: &[u8] = b"obelisk.mir2.zone-base-snapshot.v5\0";
const ZONE_RPC_BINARY_MAGIC: &[u8; 4] = b"MRM1";

static NEXT_RPC_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_REMOTE_OUTBOUND_REGISTRATION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_OUTBOUND_STREAM_ID: AtomicU64 = AtomicU64::new(1);
static SHARED_RPC_POOLS: OnceLock<Mutex<BTreeMap<String, Weak<SharedZoneRpcConnectionPool>>>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZoneRpcCodec {
    Json,
    MessagePack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZoneRpcPriority {
    Control,
    Gameplay,
}

#[derive(Debug)]
struct SharedEndpointConnections {
    slots: Vec<Mutex<Option<TcpStream>>>,
    next_control: AtomicUsize,
    next_gameplay: AtomicUsize,
    control_slots: usize,
}

#[derive(Debug)]
struct SharedZoneRpcConnectionPool {
    endpoints: Vec<SharedEndpointConnections>,
    queue_timeout: Duration,
}

impl ZoneRpcCodec {
    fn from_env() -> Self {
        match std::env::var("MIR2_ZONE_RPC_CODEC") {
            Ok(value)
                if value.trim().eq_ignore_ascii_case("msgpack")
                    || value.trim().eq_ignore_ascii_case("messagepack") =>
            {
                Self::MessagePack
            }
            _ => Self::Json,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZoneRpcLimits {
    pub max_frame_bytes: usize,
    pub max_connections: usize,
    pub max_sessions: usize,
    pub max_sessions_per_zone: usize,
    pub max_outbound_messages: usize,
    pub outbound_poll_limit: usize,
    pub io_timeout: Duration,
}

impl Default for ZoneRpcLimits {
    fn default() -> Self {
        Self {
            max_frame_bytes: DEFAULT_ZONE_RPC_MAX_FRAME_BYTES,
            max_connections: DEFAULT_ZONE_RPC_MAX_CONNECTIONS,
            max_sessions: DEFAULT_ZONE_RPC_MAX_SESSIONS,
            max_sessions_per_zone: DEFAULT_ZONE_RPC_MAX_SESSIONS_PER_ZONE,
            max_outbound_messages: DEFAULT_ZONE_RPC_MAX_OUTBOUND_MESSAGES,
            outbound_poll_limit: DEFAULT_ZONE_RPC_OUTBOUND_POLL_LIMIT,
            io_timeout: Duration::from_secs(5),
        }
    }
}

impl ZoneRpcLimits {
    pub fn from_env() -> Self {
        let defaults = Self::default();
        let max_sessions =
            positive_usize_env("MIR2_ZONE_HOST_MAX_SESSIONS").unwrap_or(defaults.max_sessions);
        let max_sessions_per_zone = positive_usize_env("MIR2_ZONE_HOST_MAX_SESSIONS_PER_ZONE")
            .unwrap_or(max_sessions)
            .min(max_sessions);
        Self {
            max_frame_bytes: positive_usize_env("MIR2_ZONE_RPC_MAX_FRAME_BYTES")
                .unwrap_or(defaults.max_frame_bytes),
            max_connections: positive_usize_env("MIR2_ZONE_HOST_MAX_CONNECTIONS")
                .unwrap_or(defaults.max_connections),
            max_sessions,
            max_sessions_per_zone,
            max_outbound_messages: positive_usize_env("MIR2_ZONE_RPC_MAX_OUTBOUND_MESSAGES")
                .unwrap_or(defaults.max_outbound_messages),
            outbound_poll_limit: positive_usize_env("MIR2_ZONE_RPC_OUTBOUND_POLL_LIMIT")
                .unwrap_or(defaults.outbound_poll_limit),
            io_timeout: Duration::from_millis(
                positive_u64_env("MIR2_ZONE_RPC_TIMEOUT_MS")
                    .unwrap_or(defaults.io_timeout.as_millis() as u64),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneHostHealth {
    pub host_id: String,
    pub process_id: u32,
    pub session_count: usize,
    pub active_connections: usize,
    pub session_capacity: usize,
    pub session_capacity_per_zone: usize,
    pub busiest_zone_session_count: usize,
    pub zone_count: usize,
    pub zone_capacity: usize,
    pub draining: bool,
    pub protocol_version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneHostTelemetrySnapshot {
    pub health: ZoneHostHealth,
    pub zones: Vec<ZoneHostZoneTelemetry>,
    pub checkpoint: ZoneHostCheckpointTelemetry,
    pub promotion: ZoneHostPromotionTelemetry,
    pub started_at_ms: u64,
    pub uptime_seconds: u64,
    pub accepted_connections_total: u64,
    pub rpc_requests_total: u64,
    pub rpc_errors_total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneHostPromotionTelemetry {
    pub assessments_total: u64,
    pub ready_assessments_total: u64,
    pub promotion_attempts_total: u64,
    pub promotions_total: u64,
    pub last_promoted_at_ms: u64,
    pub ready_zone_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneHostCheckpointTelemetry {
    pub journal_entries: u64,
    pub exports_total: u64,
    pub export_bytes_total: u64,
    pub export_duration_ns_total: u64,
    pub export_last_bytes: u64,
    pub export_last_duration_ns: u64,
    pub installs_total: u64,
    pub install_bytes_total: u64,
    pub install_duration_ns_total: u64,
    pub install_last_bytes: u64,
    pub install_last_duration_ns: u64,
    pub replay_entries_total: u64,
    pub replay_last_entries: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneReplicationHead {
    pub version: u32,
    pub zone_id: String,
    pub build_id: String,
    pub mutation_coverage: ZoneReplicationCoverage,
    pub promotion_ready: bool,
    pub base_snapshot_id: Option<String>,
    pub base_sequence: u64,
    pub oldest_available_sequence: u64,
    pub entry_count: u64,
    pub next_sequence: u64,
    pub last_sequence: Option<u64>,
    pub latest_digest: String,
}

/// A short-lived, standby-issued proof that it held an exact, fenced replica
/// image at `assessed_at_ms`. It is intentionally not an ownership grant:
/// promotion still requires a newer lease from the finalized control plane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZonePromotionReadiness {
    pub version: u32,
    pub readiness_id: Option<String>,
    pub zone_id: String,
    pub standby_host_id: String,
    pub active_build_id: String,
    pub standby_build_id: String,
    pub active_next_sequence: u64,
    pub standby_next_sequence: u64,
    pub active_latest_digest: String,
    pub standby_latest_digest: String,
    pub source_observed_at_ms: u64,
    pub assessed_at_ms: u64,
    pub observed_lag_ms: u64,
    pub max_lag_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub session_count: usize,
    pub session_capacity: usize,
    pub zone_count: usize,
    pub zone_capacity: usize,
    pub build_matches: bool,
    pub cursor_matches: bool,
    pub digest_matches: bool,
    pub base_matches: bool,
    pub replica_clock_disabled: bool,
    pub capacity_available: bool,
    pub ready: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZonePromotionReceipt {
    pub version: u32,
    pub readiness_id: String,
    pub zone_id: String,
    pub promoted_host_id: String,
    pub owner_id: String,
    pub generation: u64,
    pub promoted_at_ms: u64,
    pub head: ZoneReplicationHead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneQuiesceReceipt {
    pub version: u32,
    pub zone_id: String,
    pub host_id: String,
    pub owner_id: String,
    pub generation: u64,
    pub quiesced_at_ms: u64,
    pub head: ZoneReplicationHead,
}

#[derive(Debug, Clone)]
struct ZonePromotionReadinessRecord {
    readiness: ZonePromotionReadiness,
    head: ZoneReplicationHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoneReplicationCoverage {
    CommandJournal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneMutationEntry {
    pub sequence: u64,
    pub digest: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneMutationBatch {
    pub version: u32,
    pub zone_id: String,
    pub build_id: String,
    pub mutation_coverage: ZoneReplicationCoverage,
    pub first_sequence: u64,
    pub next_sequence: u64,
    pub previous_digest: String,
    pub latest_digest: String,
    pub entries: Vec<ZoneMutationEntry>,
    pub has_more: bool,
}

impl ZoneMutationBatch {
    pub fn verify(&self) -> Result<(), String> {
        if self.version != ZONE_REPLICATION_HEAD_VERSION {
            return Err(format!(
                "unsupported mutation batch version {}, expected {}",
                self.version, ZONE_REPLICATION_HEAD_VERSION
            ));
        }
        validate_identifier("mutation batch zone id", &self.zone_id)?;
        validate_identifier("mutation batch build id", &self.build_id)?;
        if self.mutation_coverage != ZoneReplicationCoverage::CommandJournal {
            return Err("unsupported mutation coverage".to_string());
        }
        let expected_next = self
            .first_sequence
            .checked_add(saturating_u64(self.entries.len()))
            .ok_or_else(|| "mutation batch sequence overflow".to_string())?;
        if self.next_sequence != expected_next {
            return Err(format!(
                "mutation batch next sequence {} does not match expected {expected_next}",
                self.next_sequence
            ));
        }
        let mut previous_digest = parse_hex_digest(&self.previous_digest)?;
        for (offset, mutation) in self.entries.iter().enumerate() {
            let expected_sequence = self
                .first_sequence
                .checked_add(saturating_u64(offset))
                .ok_or_else(|| "mutation entry sequence overflow".to_string())?;
            if mutation.sequence != expected_sequence {
                return Err(format!(
                    "mutation entry sequence {} does not match expected {expected_sequence}",
                    mutation.sequence
                ));
            }
            let entry: WireHostJournalEntry = serde_json::from_slice(&mutation.payload)
                .map_err(|error| format!("mutation entry decode failed: {error}"))?;
            if entry.zone_id != self.zone_id {
                return Err(format!(
                    "mutation entry Zone {} does not match batch Zone {}",
                    entry.zone_id, self.zone_id
                ));
            }
            if entry.sequence != mutation.sequence {
                return Err(format!(
                    "mutation payload sequence {} does not match envelope sequence {}",
                    entry.sequence, mutation.sequence
                ));
            }
            let expected_digest =
                zone_replication_entry_digest(&previous_digest, mutation.sequence, &entry)
                    .map_err(|error| error.message)?;
            let actual_digest = parse_hex_digest(&mutation.digest)?;
            if !constant_time_bytes_equal(&expected_digest, &actual_digest) {
                return Err(format!(
                    "mutation entry {} digest mismatch",
                    mutation.sequence
                ));
            }
            previous_digest = expected_digest;
        }
        let latest_digest = parse_hex_digest(&self.latest_digest)?;
        if !constant_time_bytes_equal(&previous_digest, &latest_digest) {
            return Err("mutation batch latest digest mismatch".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoneBaseSnapshotCompression {
    Gzip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneBaseSnapshot {
    pub version: u32,
    pub snapshot_id: String,
    pub zone_id: String,
    pub build_id: String,
    pub mutation_coverage: ZoneReplicationCoverage,
    pub apply_ready: bool,
    pub base_sequence: u64,
    pub latest_digest: String,
    pub created_at_ms: u64,
    pub session_count: usize,
    pub compression: ZoneBaseSnapshotCompression,
    pub uncompressed_bytes: usize,
    #[serde(with = "base64_snapshot_payload")]
    pub payload: Vec<u8>,
}

impl ZoneBaseSnapshot {
    fn new(
        zone_id: String,
        build_id: String,
        base_sequence: u64,
        latest_digest: String,
        zone_state_bytes: Vec<u8>,
        sessions: Vec<WireSessionCommitment>,
    ) -> Result<Self, ZoneRpcFault> {
        let session_count = sessions.len();
        let apply_ready = sessions.iter().all(session_image_is_complete);
        let payload = serde_json::to_vec(&WireZoneBaseSnapshotPayload {
            version: ZONE_REPLICATION_HEAD_VERSION,
            zone_id: zone_id.clone(),
            zone_state_bytes,
            sessions,
        })
        .map_err(|error| {
            ZoneRpcFault::new(
                "base_snapshot_encode",
                format!("Zone {zone_id} base snapshot payload encode failed: {error}"),
            )
        })?;
        if payload.len() > DEFAULT_ZONE_BASE_SNAPSHOT_MAX_UNCOMPRESSED_BYTES {
            return Err(ZoneRpcFault::new(
                "base_snapshot_too_large",
                format!(
                    "Zone {zone_id} base snapshot payload {} exceeds {} bytes",
                    payload.len(),
                    DEFAULT_ZONE_BASE_SNAPSHOT_MAX_UNCOMPRESSED_BYTES
                ),
            ));
        }
        let uncompressed_bytes = payload.len();
        let mut encoder: GzEncoder<Vec<u8>> = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::new(6));
        encoder.write_all(&payload).map_err(|error| {
            ZoneRpcFault::new(
                "base_snapshot_encode",
                format!("Zone {zone_id} base snapshot compression failed: {error}"),
            )
        })?;
        let payload = encoder.finish().map_err(|error| {
            ZoneRpcFault::new(
                "base_snapshot_encode",
                format!("Zone {zone_id} base snapshot compression finish failed: {error}"),
            )
        })?;
        let mut snapshot = Self {
            version: ZONE_REPLICATION_HEAD_VERSION,
            snapshot_id: String::new(),
            zone_id,
            build_id,
            mutation_coverage: ZoneReplicationCoverage::CommandJournal,
            apply_ready,
            base_sequence,
            latest_digest,
            created_at_ms: unix_now_ms(),
            session_count,
            compression: ZoneBaseSnapshotCompression::Gzip,
            uncompressed_bytes,
            payload,
        };
        snapshot.snapshot_id = zone_base_snapshot_checksum(&snapshot)?;
        Ok(snapshot)
    }

    pub fn verify(&self) -> Result<(), String> {
        if self.version != ZONE_REPLICATION_HEAD_VERSION {
            return Err(format!(
                "unsupported base snapshot version {}, expected {}",
                self.version, ZONE_REPLICATION_HEAD_VERSION
            ));
        }
        validate_identifier("base snapshot id", &self.snapshot_id)?;
        validate_identifier("base snapshot Zone id", &self.zone_id)?;
        validate_identifier("base snapshot build id", &self.build_id)?;
        if self.mutation_coverage != ZoneReplicationCoverage::CommandJournal {
            return Err("unsupported base snapshot mutation coverage".to_string());
        }
        parse_hex_digest(&self.latest_digest)?;
        if self.uncompressed_bytes > DEFAULT_ZONE_BASE_SNAPSHOT_MAX_UNCOMPRESSED_BYTES {
            return Err(format!(
                "base snapshot declares {} uncompressed bytes, maximum is {}",
                self.uncompressed_bytes, DEFAULT_ZONE_BASE_SNAPSHOT_MAX_UNCOMPRESSED_BYTES
            ));
        }
        let expected_id = zone_base_snapshot_checksum(self)?;
        if !constant_time_bytes_equal(expected_id.as_bytes(), self.snapshot_id.as_bytes()) {
            return Err("base snapshot checksum mismatch".to_string());
        }
        let payload = self.decode_payload()?;
        if payload.version != self.version || payload.zone_id != self.zone_id {
            return Err("base snapshot payload identity mismatch".to_string());
        }
        if payload.sessions.len() != self.session_count {
            return Err(format!(
                "base snapshot contains {} sessions, expected {}",
                payload.sessions.len(),
                self.session_count
            ));
        }
        let mut identities = BTreeSet::new();
        for session in payload.sessions {
            if session.zone_id != self.zone_id {
                return Err(format!(
                    "base snapshot session {} belongs to Zone {}",
                    session.session_id, session.zone_id
                ));
            }
            if !identities.insert(session.session_id.clone()) {
                return Err(format!(
                    "base snapshot contains duplicate session {}",
                    session.session_id
                ));
            }
            validate_session_image(&session)?;
            if self.apply_ready && !session_image_is_complete(&session) {
                return Err(format!(
                    "base snapshot session {} is incomplete but snapshot claims apply readiness",
                    session.session_id
                ));
            }
        }
        if payload.zone_state_bytes.is_empty() {
            return Err("base snapshot contains empty Zone state".to_string());
        }
        Ok(())
    }

    fn decode_payload(&self) -> Result<WireZoneBaseSnapshotPayload, String> {
        let mut decoder = match self.compression {
            ZoneBaseSnapshotCompression::Gzip => GzDecoder::new(self.payload.as_slice()),
        };
        let mut decoded = Vec::with_capacity(self.uncompressed_bytes.min(1024 * 1024));
        decoder
            .by_ref()
            .take((DEFAULT_ZONE_BASE_SNAPSHOT_MAX_UNCOMPRESSED_BYTES as u64).saturating_add(1))
            .read_to_end(&mut decoded)
            .map_err(|error| format!("base snapshot decompression failed: {error}"))?;
        if decoded.len() != self.uncompressed_bytes {
            return Err(format!(
                "base snapshot decompressed to {} bytes, expected {}",
                decoded.len(),
                self.uncompressed_bytes
            ));
        }
        serde_json::from_slice(&decoded)
            .map_err(|error| format!("base snapshot payload decode failed: {error}"))
    }
}

mod base64_snapshot_payload {
    use super::{Engine, BASE64_STANDARD};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64_STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        BASE64_STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoneMapScope {
    All,
    Explicit,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneHostZoneTelemetry {
    pub zone_id: String,
    pub map_scope: ZoneMapScope,
    pub map_file_names: Vec<String>,
    pub session_count: usize,
}

#[derive(Debug, Default)]
struct ZoneMapCatalog {
    maps_by_zone: BTreeMap<String, Vec<String>>,
    all_maps_zone_ids: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct ZoneReplicationCursor {
    base_snapshot_id: Option<String>,
    base_sequence: u64,
    base_digest: [u8; 32],
    next_sequence: u64,
    latest_digest: [u8; 32],
    host_entry_indexes: Vec<usize>,
    entry_digests: Vec<[u8; 32]>,
}

impl Default for ZoneReplicationCursor {
    fn default() -> Self {
        Self {
            base_snapshot_id: None,
            base_sequence: 0,
            base_digest: [0; 32],
            next_sequence: 0,
            latest_digest: [0; 32],
            host_entry_indexes: Vec::new(),
            entry_digests: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone)]
struct ZoneReplicationCatalog {
    cursors: BTreeMap<String, ZoneReplicationCursor>,
}

impl ZoneReplicationCatalog {
    fn append(
        &mut self,
        host_entry_index: usize,
        entry: &WireHostJournalEntry,
    ) -> Result<(), ZoneRpcFault> {
        let cursor = self.cursors.entry(entry.zone_id.clone()).or_default();
        let next_sequence = cursor.next_sequence.checked_add(1).ok_or_else(|| {
            ZoneRpcFault::new(
                "replication_sequence_exhausted",
                format!("Zone {} exhausted its replication sequence", entry.zone_id),
            )
        })?;
        let next_digest =
            zone_replication_entry_digest(&cursor.latest_digest, cursor.next_sequence, entry)?;
        cursor.latest_digest = next_digest;
        cursor.next_sequence = next_sequence;
        cursor.host_entry_indexes.push(host_entry_index);
        cursor.entry_digests.push(next_digest);
        Ok(())
    }

    fn from_entries(entries: &[WireHostJournalEntry]) -> Result<Self, ZoneRpcFault> {
        let mut catalog = Self::default();
        for (host_entry_index, entry) in entries.iter().enumerate() {
            catalog.append(host_entry_index, entry)?;
        }
        Ok(catalog)
    }

    fn from_base(snapshot: &ZoneBaseSnapshot) -> Result<Self, ZoneRpcFault> {
        let base_digest = parse_hex_digest(&snapshot.latest_digest).map_err(|error| {
            ZoneRpcFault::new(
                "base_snapshot_invalid",
                format!(
                    "Zone {} base snapshot digest is invalid: {error}",
                    snapshot.zone_id
                ),
            )
        })?;
        let mut cursors = BTreeMap::new();
        cursors.insert(
            snapshot.zone_id.clone(),
            ZoneReplicationCursor {
                base_snapshot_id: Some(snapshot.snapshot_id.clone()),
                base_sequence: snapshot.base_sequence,
                base_digest,
                next_sequence: snapshot.base_sequence,
                latest_digest: base_digest,
                host_entry_indexes: Vec::new(),
                entry_digests: Vec::new(),
            },
        );
        Ok(Self { cursors })
    }

    fn contains_compacted_history(&self) -> bool {
        self.cursors
            .values()
            .any(|cursor| cursor.base_snapshot_id.is_some() || cursor.base_sequence > 0)
    }

    fn head(&self, zone_id: &str) -> ZoneReplicationHead {
        let cursor = self.cursors.get(zone_id).cloned().unwrap_or_default();
        ZoneReplicationHead {
            version: ZONE_REPLICATION_HEAD_VERSION,
            zone_id: zone_id.to_string(),
            build_id: zone_replication_build_id(),
            mutation_coverage: ZoneReplicationCoverage::CommandJournal,
            promotion_ready: false,
            base_snapshot_id: cursor.base_snapshot_id,
            base_sequence: cursor.base_sequence,
            oldest_available_sequence: cursor.base_sequence,
            entry_count: cursor.next_sequence,
            next_sequence: cursor.next_sequence,
            last_sequence: cursor.next_sequence.checked_sub(1),
            latest_digest: hex_lower_bytes(&cursor.latest_digest),
        }
    }

    fn export_batch(
        &self,
        host_entries: &[WireHostJournalEntry],
        zone_id: &str,
        first_sequence: u64,
        max_entries: usize,
        max_payload_bytes: usize,
    ) -> Result<ZoneMutationBatch, ZoneRpcFault> {
        let empty = ZoneReplicationCursor::default();
        let cursor = self.cursors.get(zone_id).unwrap_or(&empty);
        if first_sequence < cursor.base_sequence {
            return Err(ZoneRpcFault::new(
                "replication_cursor_compacted",
                format!(
                    "Zone {zone_id} cursor {first_sequence} predates oldest available sequence {}",
                    cursor.base_sequence
                ),
            ));
        }
        if first_sequence > cursor.next_sequence {
            return Err(ZoneRpcFault::new(
                "replication_cursor_ahead",
                format!(
                    "Zone {zone_id} cursor {first_sequence} is ahead of next sequence {}",
                    cursor.next_sequence
                ),
            ));
        }
        let first_index = usize::try_from(first_sequence.saturating_sub(cursor.base_sequence))
            .map_err(|_| {
                ZoneRpcFault::new(
                    "replication_cursor_invalid",
                    format!("Zone {zone_id} cursor does not fit this host"),
                )
            })?;
        let previous_digest = if first_index == 0 {
            cursor.base_digest
        } else {
            *cursor.entry_digests.get(first_index - 1).ok_or_else(|| {
                ZoneRpcFault::new(
                    "replication_cursor_compacted",
                    format!("Zone {zone_id} cursor {first_sequence} is no longer available"),
                )
            })?
        };
        let mut entries = Vec::new();
        let mut payload_bytes = 0usize;
        for zone_index in first_index
            ..cursor
                .host_entry_indexes
                .len()
                .min(first_index.saturating_add(max_entries))
        {
            let host_index = cursor.host_entry_indexes[zone_index];
            let host_entry = host_entries.get(host_index).ok_or_else(|| {
                ZoneRpcFault::new(
                    "replication_index_invalid",
                    format!("Zone {zone_id} references missing host journal entry {host_index}"),
                )
            })?;
            let sequence = cursor
                .base_sequence
                .checked_add(u64::try_from(zone_index).map_err(|_| {
                    ZoneRpcFault::new(
                        "replication_sequence_invalid",
                        format!("Zone {zone_id} sequence does not fit the wire format"),
                    )
                })?)
                .ok_or_else(|| {
                    ZoneRpcFault::new(
                        "replication_sequence_exhausted",
                        format!("Zone {zone_id} sequence overflowed"),
                    )
                })?;
            let mut canonical_entry = host_entry.clone();
            canonical_entry.sequence = sequence;
            let payload = serde_json::to_vec(&canonical_entry).map_err(|error| {
                ZoneRpcFault::new(
                    "replication_encode",
                    format!("Zone {zone_id} mutation encode failed: {error}"),
                )
            })?;
            if payload.len() > max_payload_bytes && entries.is_empty() {
                return Err(ZoneRpcFault::new(
                    "replication_entry_too_large",
                    format!(
                        "Zone {zone_id} mutation payload {} exceeds batch byte limit {max_payload_bytes}",
                        payload.len()
                    ),
                ));
            }
            if payload_bytes.saturating_add(payload.len()) > max_payload_bytes {
                break;
            }
            payload_bytes = payload_bytes.saturating_add(payload.len());
            entries.push(ZoneMutationEntry {
                sequence,
                digest: hex_lower_bytes(&cursor.entry_digests[zone_index]),
                payload,
            });
        }
        let next_sequence = first_sequence
            .checked_add(saturating_u64(entries.len()))
            .ok_or_else(|| {
                ZoneRpcFault::new(
                    "replication_sequence_exhausted",
                    format!("Zone {zone_id} batch sequence overflowed"),
                )
            })?;
        let latest_digest = entries
            .last()
            .map(|entry| entry.digest.clone())
            .unwrap_or_else(|| hex_lower_bytes(&previous_digest));
        Ok(ZoneMutationBatch {
            version: ZONE_REPLICATION_HEAD_VERSION,
            zone_id: zone_id.to_string(),
            build_id: zone_replication_build_id(),
            mutation_coverage: ZoneReplicationCoverage::CommandJournal,
            first_sequence,
            next_sequence,
            previous_digest: hex_lower_bytes(&previous_digest),
            latest_digest,
            entries,
            has_more: next_sequence < cursor.next_sequence,
        })
    }
}

#[derive(Debug, Default, Clone)]
struct ZoneHostJournal {
    entries: Vec<WireHostJournalEntry>,
    replication: ZoneReplicationCatalog,
}

impl ZoneHostJournal {
    fn export_batch(
        &self,
        zone_id: &str,
        first_sequence: u64,
        max_entries: usize,
        max_payload_bytes: usize,
    ) -> Result<ZoneMutationBatch, ZoneRpcFault> {
        self.replication.export_batch(
            &self.entries,
            zone_id,
            first_sequence,
            max_entries,
            max_payload_bytes,
        )
    }

    fn install_base(&mut self, snapshot: &ZoneBaseSnapshot) -> Result<(), ZoneRpcFault> {
        self.entries
            .retain(|entry| entry.zone_id != snapshot.zone_id);
        let mut cursors = self
            .replication
            .cursors
            .iter()
            .filter(|(zone_id, _)| zone_id.as_str() != snapshot.zone_id)
            .map(|(zone_id, cursor)| {
                (
                    zone_id.clone(),
                    ZoneReplicationCursor {
                        base_snapshot_id: cursor.base_snapshot_id.clone(),
                        base_sequence: cursor.base_sequence,
                        base_digest: cursor.base_digest,
                        next_sequence: cursor.base_sequence,
                        latest_digest: cursor.base_digest,
                        host_entry_indexes: Vec::new(),
                        entry_digests: Vec::new(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        cursors.extend(ZoneReplicationCatalog::from_base(snapshot)?.cursors);
        let mut replication = ZoneReplicationCatalog { cursors };
        for (host_entry_index, entry) in self.entries.iter().enumerate() {
            replication.append(host_entry_index, entry)?;
        }
        self.replication = replication;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SequencedZoneHostPacket {
    pub sequence: u64,
    pub packet: ServerPacket,
}

#[derive(Debug, Clone)]
pub struct ZoneHostOutboundBatch {
    pub items: Vec<SequencedZoneHostPacket>,
    pub stream_id: String,
    pub reset: bool,
    pub last_issued_sequence: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct ZoneHostCheckpoint {
    bytes: Vec<u8>,
    pub entry_count: usize,
    pub session_count: usize,
    pub zone_count: usize,
    pub zone_state_bytes: usize,
    pub checksum: String,
}

impl ZoneHostCheckpoint {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        let checkpoint: WireZoneHostCheckpoint = serde_json::from_slice(&bytes)
            .map_err(|error| format!("zone host checkpoint decode failed: {error}"))?;
        checkpoint.verify()?;
        Ok(Self {
            entry_count: checkpoint.entries.len(),
            session_count: checkpoint.sessions.len(),
            zone_count: checkpoint.zone_count,
            zone_state_bytes: checkpoint.zone_state_bytes.len(),
            checksum: checkpoint.checksum,
            bytes,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone)]
pub struct TcpZoneOwnerRpcTransport {
    addresses: Arc<Vec<String>>,
    active_endpoint: Arc<AtomicUsize>,
    zone_id: ZoneId,
    session_id: String,
    auth_token: Option<String>,
    limits: ZoneRpcLimits,
    codec: ZoneRpcCodec,
    reuse_connections: bool,
    connections: Arc<Vec<Mutex<Option<TcpStream>>>>,
    shared_connections: Option<Arc<SharedZoneRpcConnectionPool>>,
    outbound_acknowledged: Arc<AtomicU64>,
    outbound_generation: Arc<AtomicU64>,
    outbound_stream_id: Arc<Mutex<Option<String>>>,
}

impl fmt::Debug for TcpZoneOwnerRpcTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TcpZoneOwnerRpcTransport")
            .field("addresses", &self.addresses)
            .field(
                "active_endpoint",
                &self.active_endpoint.load(Ordering::Relaxed),
            )
            .field("zone_id", &self.zone_id)
            .field("session_id", &self.session_id)
            .field("authenticated", &self.auth_token.is_some())
            .field("codec", &self.codec)
            .field("reuse_connections", &self.reuse_connections)
            .field("shared_connection_pool", &self.shared_connections.is_some())
            .field("limits", &self.limits)
            .finish()
    }
}

impl TcpZoneOwnerRpcTransport {
    pub fn new(address: impl Into<String>, zone_id: ZoneId) -> Self {
        Self::with_options(
            address,
            zone_id,
            next_rpc_session_id(),
            std::env::var("MIR2_ZONE_HOST_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            ZoneRpcLimits::from_env(),
        )
    }

    pub fn with_options(
        address: impl Into<String>,
        zone_id: ZoneId,
        session_id: impl Into<String>,
        auth_token: Option<String>,
        limits: ZoneRpcLimits,
    ) -> Self {
        Self::with_endpoints(
            vec![address.into()],
            zone_id,
            session_id,
            auth_token,
            limits,
        )
        .expect("single Zone Host endpoint must be valid")
    }

    pub fn with_endpoints(
        addresses: Vec<String>,
        zone_id: ZoneId,
        session_id: impl Into<String>,
        auth_token: Option<String>,
        limits: ZoneRpcLimits,
    ) -> Result<Self, String> {
        let addresses = addresses
            .into_iter()
            .map(|address| address.trim().to_string())
            .filter(|address| !address.is_empty())
            .collect::<Vec<_>>();
        if addresses.is_empty() {
            return Err("at least one Zone Host endpoint is required".to_string());
        }
        let connections = (0..addresses.len())
            .map(|_| Mutex::new(None))
            .collect::<Vec<_>>();
        let shared_connections = positive_usize_env("MIR2_ZONE_RPC_SHARED_POOL_SIZE")
            .map(|size| shared_rpc_pool(&addresses, size, limits.io_timeout));
        Ok(Self {
            addresses: Arc::new(addresses),
            active_endpoint: Arc::new(AtomicUsize::new(0)),
            zone_id,
            session_id: session_id.into(),
            auth_token,
            limits,
            codec: ZoneRpcCodec::from_env(),
            reuse_connections: false,
            connections: Arc::new(connections),
            shared_connections,
            outbound_acknowledged: Arc::new(AtomicU64::new(0)),
            outbound_generation: Arc::new(AtomicU64::new(0)),
            outbound_stream_id: Arc::new(Mutex::new(None)),
        })
    }

    /// Reuse one framed TCP stream per configured endpoint.
    ///
    /// Calls on the same transport remain serialized so response ordering is
    /// unambiguous. Distinct player/session transports still execute in
    /// parallel over independent connections.
    pub fn with_connection_reuse(mut self) -> Self {
        self.reuse_connections = true;
        self
    }

    /// Use the compact RPC codec. A magic prefix lets the same Zone Host
    /// listener continue serving JSON clients during rolling upgrades.
    pub fn with_binary_codec(mut self) -> Self {
        self.codec = ZoneRpcCodec::MessagePack;
        self
    }

    /// Multiplex logical sessions over a bounded set of long-lived sockets.
    /// Control traffic uses reserved lanes so gameplay saturation cannot block
    /// health, replication, fencing, or promotion.
    pub fn with_shared_connection_pool(mut self, pool_size: usize) -> Self {
        self.shared_connections = Some(shared_rpc_pool(
            self.addresses.as_ref(),
            pool_size,
            self.limits.io_timeout,
        ));
        self.reuse_connections = true;
        self
    }

    pub fn with_placement(
        placement: &ZonePlacementLease,
        session_id: impl Into<String>,
        auth_token: Option<String>,
        limits: ZoneRpcLimits,
    ) -> Result<Self, String> {
        Self::with_endpoints(
            placement.endpoints(),
            placement.zone_id.clone(),
            session_id,
            auth_token,
            limits,
        )
    }

    pub fn from_env(zone_id: ZoneId) -> Option<Self> {
        let addresses = std::env::var("MIR2_ZONE_HOST_ADDRS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|addresses| !addresses.is_empty())
            .or_else(|| {
                std::env::var("MIR2_ZONE_HOST_ADDR")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .map(|address| vec![address.trim().to_string()])
            })?;
        let transport = Self::with_endpoints(
            addresses,
            zone_id,
            next_rpc_session_id(),
            std::env::var("MIR2_ZONE_HOST_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            ZoneRpcLimits::from_env(),
        )
        .ok()?;
        Some(if enabled_env("MIR2_ZONE_RPC_REUSE_CONNECTIONS") {
            transport.with_connection_reuse()
        } else {
            transport
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn health(&self) -> Result<ZoneHostHealth, String> {
        match self.call(ZoneRpcRequest::Health)? {
            ZoneRpcPayload::Health {
                host_id,
                process_id,
                session_count,
                active_connections,
                session_capacity,
                session_capacity_per_zone,
                busiest_zone_session_count,
                zone_count,
                zone_capacity,
                draining,
                protocol_version,
            } => Ok(ZoneHostHealth {
                host_id,
                process_id,
                session_count,
                active_connections,
                session_capacity,
                session_capacity_per_zone,
                busiest_zone_session_count,
                zone_count,
                zone_capacity,
                draining,
                protocol_version,
            }),
            payload => Err(unexpected_payload("health", &payload)),
        }
    }

    pub fn replication_head(&self) -> Result<ZoneReplicationHead, String> {
        match self.call(ZoneRpcRequest::ReplicationHead)? {
            ZoneRpcPayload::ReplicationHead { head } => Ok(head),
            payload => Err(unexpected_payload("replication_head", &payload)),
        }
    }

    pub fn assess_promotion_readiness(
        &self,
        active_head: ZoneReplicationHead,
        source_observed_at_ms: u64,
        max_lag_ms: u64,
    ) -> Result<ZonePromotionReadiness, String> {
        match self.call(ZoneRpcRequest::AssessPromotionReadiness {
            active_head,
            source_observed_at_ms,
            max_lag_ms,
        })? {
            ZoneRpcPayload::PromotionReadiness { readiness } => Ok(readiness),
            payload => Err(unexpected_payload("assess_promotion_readiness", &payload)),
        }
    }

    pub fn promote_replica(
        &self,
        readiness_id: impl Into<String>,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<ZonePromotionReceipt, String> {
        match self.call(ZoneRpcRequest::PromoteReplica {
            readiness_id: readiness_id.into(),
            owner_lease: WireZoneOwnerLease::from(owner_lease),
        })? {
            ZoneRpcPayload::PromotionReceipt { receipt } => Ok(receipt),
            payload => Err(unexpected_payload("promote_replica", &payload)),
        }
    }

    pub fn quiesce_for_promotion(
        &self,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<ZoneQuiesceReceipt, String> {
        match self.call(ZoneRpcRequest::QuiesceForPromotion {
            owner_lease: WireZoneOwnerLease::from(owner_lease),
        })? {
            ZoneRpcPayload::QuiesceReceipt { receipt } => Ok(receipt),
            payload => Err(unexpected_payload("quiesce_for_promotion", &payload)),
        }
    }

    pub fn resume_after_quiesce(&self, owner_lease: &ZoneOwnerLease) -> Result<(), String> {
        match self.call(ZoneRpcRequest::ResumeAfterQuiesce {
            owner_lease: WireZoneOwnerLease::from(owner_lease),
        })? {
            ZoneRpcPayload::Unit => Ok(()),
            payload => Err(unexpected_payload("resume_after_quiesce", &payload)),
        }
    }

    pub fn export_mutation_batch(
        &self,
        first_sequence: u64,
        max_entries: usize,
        max_payload_bytes: usize,
    ) -> Result<ZoneMutationBatch, String> {
        let batch = match self.call(ZoneRpcRequest::ExportMutationBatch {
            first_sequence,
            max_entries,
            max_payload_bytes,
        })? {
            ZoneRpcPayload::MutationBatch { batch } => batch,
            payload => return Err(unexpected_payload("export_mutation_batch", &payload)),
        };
        batch.verify()?;
        if batch.zone_id != self.zone_id.as_str() {
            return Err(format!(
                "mutation batch Zone {} does not match requested Zone {}",
                batch.zone_id,
                self.zone_id.as_str()
            ));
        }
        Ok(batch)
    }

    pub fn export_base_snapshot(&self) -> Result<ZoneBaseSnapshot, String> {
        let snapshot = match self.call(ZoneRpcRequest::ExportBaseSnapshot)? {
            ZoneRpcPayload::BaseSnapshot { snapshot } => snapshot,
            payload => return Err(unexpected_payload("export_base_snapshot", &payload)),
        };
        snapshot.verify()?;
        if snapshot.zone_id != self.zone_id.as_str() {
            return Err(format!(
                "base snapshot Zone {} does not match requested Zone {}",
                snapshot.zone_id,
                self.zone_id.as_str()
            ));
        }
        Ok(snapshot)
    }

    pub fn install_base_snapshot(&self, snapshot: &ZoneBaseSnapshot) -> Result<(), String> {
        snapshot.verify()?;
        if snapshot.zone_id != self.zone_id.as_str() {
            return Err(format!(
                "base snapshot Zone {} does not match requested Zone {}",
                snapshot.zone_id,
                self.zone_id.as_str()
            ));
        }
        match self.call(ZoneRpcRequest::InstallBaseSnapshot {
            snapshot: snapshot.clone(),
        })? {
            ZoneRpcPayload::Unit => {
                self.outbound_acknowledged.store(0, Ordering::Release);
                *self
                    .outbound_stream_id
                    .lock()
                    .map_err(|_| "zone RPC outbound stream mutex poisoned".to_string())? = None;
                Ok(())
            }
            payload => Err(unexpected_payload("install_base_snapshot", &payload)),
        }
    }

    pub fn apply_mutation_batch(&self, batch: &ZoneMutationBatch) -> Result<(), String> {
        batch.verify()?;
        if batch.zone_id != self.zone_id.as_str() {
            return Err(format!(
                "mutation batch Zone {} does not match requested Zone {}",
                batch.zone_id,
                self.zone_id.as_str()
            ));
        }
        match self.call(ZoneRpcRequest::ApplyMutationBatch {
            batch: batch.clone(),
        })? {
            ZoneRpcPayload::Unit => Ok(()),
            payload => Err(unexpected_payload("apply_mutation_batch", &payload)),
        }
    }

    pub fn poll_outbounds(
        &self,
        acknowledged_sequence: u64,
        max_items: usize,
    ) -> Result<ZoneHostOutboundBatch, String> {
        let max_items = max_items.max(1).min(self.limits.outbound_poll_limit);
        let stream_id = self
            .outbound_stream_id
            .lock()
            .map_err(|_| "zone RPC outbound stream mutex poisoned".to_string())?
            .clone();
        match self.call(ZoneRpcRequest::PollOutbounds {
            stream_id,
            acknowledged_sequence,
            max_items,
        })? {
            ZoneRpcPayload::Outbounds {
                items,
                stream_id,
                reset,
                last_issued_sequence,
                has_more,
            } => {
                if reset {
                    self.outbound_acknowledged.store(0, Ordering::Release);
                }
                *self
                    .outbound_stream_id
                    .lock()
                    .map_err(|_| "zone RPC outbound stream mutex poisoned".to_string())? =
                    Some(stream_id.clone());
                let mut decoded = Vec::with_capacity(items.len());
                for item in items {
                    decoded.push(SequencedZoneHostPacket {
                        sequence: item.sequence,
                        packet: decode_server_packet(&item.frame).map_err(|error| {
                            format!("zone RPC outbound packet decode failed: {error}")
                        })?,
                    });
                }
                Ok(ZoneHostOutboundBatch {
                    items: decoded,
                    stream_id,
                    reset,
                    last_issued_sequence,
                    has_more,
                })
            }
            payload => Err(unexpected_payload("poll_outbounds", &payload)),
        }
    }

    pub fn export_host_checkpoint(&self) -> Result<ZoneHostCheckpoint, String> {
        match self.call(ZoneRpcRequest::ExportHostCheckpoint)? {
            ZoneRpcPayload::HostCheckpoint { bytes } => ZoneHostCheckpoint::from_bytes(bytes),
            payload => Err(unexpected_payload("export_host_checkpoint", &payload)),
        }
    }

    pub fn install_host_checkpoint(&self, checkpoint: &ZoneHostCheckpoint) -> Result<(), String> {
        match self.call(ZoneRpcRequest::InstallHostCheckpoint {
            bytes: checkpoint.as_bytes().to_vec(),
        })? {
            ZoneRpcPayload::Unit => {
                self.outbound_acknowledged.store(0, Ordering::Release);
                *self
                    .outbound_stream_id
                    .lock()
                    .map_err(|_| "zone RPC outbound stream mutex poisoned".to_string())? = None;
                Ok(())
            }
            payload => Err(unexpected_payload("install_host_checkpoint", &payload)),
        }
    }

    fn call(&self, request: ZoneRpcRequest) -> Result<ZoneRpcPayload, String> {
        validate_identifier("RPC session id", &self.session_id)?;
        let priority = request.priority();
        let envelope = ZoneRpcEnvelope {
            protocol_version: ZONE_RPC_PROTOCOL_VERSION,
            session_id: self.session_id.clone(),
            zone_id: self.zone_id.as_str().to_string(),
            auth_token: self.auth_token.clone(),
            request,
        };
        let encoded = encode_rpc_envelope(&envelope, self.codec)
            .map_err(|error| format!("zone RPC request encode failed: {error}"))?;
        if encoded.len() > self.limits.max_frame_bytes {
            return Err(format!(
                "zone RPC request frame exceeds {} bytes",
                self.limits.max_frame_bytes
            ));
        }

        let endpoint_count = self.addresses.len();
        let start = self.active_endpoint.load(Ordering::Acquire) % endpoint_count;
        let mut failures = Vec::new();
        for offset in 0..endpoint_count {
            let index = (start + offset) % endpoint_count;
            let address = &self.addresses[index];
            let response = if let Some(pool) = self.shared_connections.as_ref() {
                pool.call(index, address, &encoded, &self.limits, self.codec, priority)
            } else if self.reuse_connections {
                call_reused_endpoint(
                    address,
                    &encoded,
                    &self.limits,
                    &self.connections[index],
                    self.codec,
                )
            } else {
                call_endpoint(address, &encoded, &self.limits, self.codec)
            };
            match response {
                Ok(ZoneRpcResponse::Ok { payload }) => {
                    self.active_endpoint.store(index, Ordering::Release);
                    return Ok(*payload);
                }
                Ok(ZoneRpcResponse::Error { code, message })
                    if matches!(
                        code.as_str(),
                        "host_draining"
                            | "capacity"
                            | "stale_lease"
                            | "zone_quiesced"
                            | "owner_mismatch"
                    ) =>
                {
                    failures.push(format!("{address}: zone RPC {code}: {message}"));
                }
                Ok(ZoneRpcResponse::Error { code, message }) => {
                    return Err(format!("zone RPC {code}: {message}"));
                }
                Err(error) => failures.push(format!("{address}: {error}")),
            }
        }
        Err(format!(
            "zone RPC endpoints unavailable: {}",
            failures.join("; ")
        ))
    }
}

impl ZoneOwnerRpcTransport for TcpZoneOwnerRpcTransport {
    fn on_connect(&self) -> Result<Vec<ServerPacket>, String> {
        match self.call(ZoneRpcRequest::OnConnect)? {
            ZoneRpcPayload::Packets { frames } => decode_server_frames(frames),
            payload => Err(unexpected_payload("on_connect", &payload)),
        }
    }

    fn execute(&self, request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        let (owner_lease, mode, command) = request.into_parts();
        let command_kind = command.kind();
        let request = ZoneRpcRequest::Execute {
            owner_lease: WireZoneOwnerLease::from(&owner_lease),
            mode: WireZoneOwnerCommandMode::from(mode),
            command: WireWorldCommand::from_world(command)?,
        };
        match self.call(request)? {
            ZoneRpcPayload::Execution {
                frames,
                packet_count,
                snapshot_tick,
                active_identity,
            } => {
                let packets = decode_server_frames(frames)?;
                if packet_count != packets.len() {
                    return Err(format!(
                        "zone RPC execution packet count mismatch: declared {packet_count}, decoded {}",
                        packets.len()
                    ));
                }
                Ok(WorldCommandExecution {
                    packets,
                    outcome: WorldCommandOutcome {
                        command_kind,
                        packet_count,
                        snapshot_tick,
                        active_identity,
                    },
                })
            }
            payload => Err(unexpected_payload("execute", &payload)),
        }
    }

    fn world_snapshot(&self) -> Result<WorldSnapshot, String> {
        match self.call(ZoneRpcRequest::WorldSnapshot)? {
            ZoneRpcPayload::WorldSnapshot { snapshot } => Ok(*snapshot),
            payload => Err(unexpected_payload("world_snapshot", &payload)),
        }
    }

    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> {
        match self.call(ZoneRpcRequest::ActiveIdentity)? {
            ZoneRpcPayload::ActiveIdentity { identity } => Ok(identity),
            payload => Err(unexpected_payload("active_identity", &payload)),
        }
    }

    fn save_active_character(&self) -> Result<(), String> {
        match self.call(ZoneRpcRequest::SaveActiveCharacter)? {
            ZoneRpcPayload::Unit => Ok(()),
            payload => Err(unexpected_payload("save_active_character", &payload)),
        }
    }

    fn refresh_active_external_mail(&self) -> Result<bool, String> {
        match self.call(ZoneRpcRequest::RefreshActiveExternalMail)? {
            ZoneRpcPayload::Bool { value } => Ok(value),
            payload => Err(unexpected_payload("refresh_active_external_mail", &payload)),
        }
    }

    fn close_session(&self, owner_lease: &ZoneOwnerLease) -> Result<(), String> {
        match self.call(ZoneRpcRequest::CloseSession {
            owner_lease: WireZoneOwnerLease::from(owner_lease),
        })? {
            ZoneRpcPayload::Unit => Ok(()),
            payload => Err(unexpected_payload("close_session", &payload)),
        }
    }

    fn register_live_outbound(
        &self,
        sender: SharedZoneLiveOutboundSender,
    ) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
        let registration_id = NEXT_REMOTE_OUTBOUND_REGISTRATION_ID
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        let generation = self
            .outbound_generation
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1);
        let stop = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_active = Arc::clone(&active);
        let transport = self.clone();
        let acknowledged = Arc::clone(&self.outbound_acknowledged);
        let current_generation = Arc::clone(&self.outbound_generation);
        let handle = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) && !worker_active.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(1));
            }
            while !worker_stop.load(Ordering::Acquire)
                && current_generation.load(Ordering::Acquire) == generation
            {
                let ack = acknowledged.load(Ordering::Acquire);
                match transport.poll_outbounds(ack, transport.limits.outbound_poll_limit) {
                    Ok(batch) => {
                        if batch.items.is_empty() {
                            thread::sleep(Duration::from_millis(20));
                            continue;
                        }
                        for item in batch.items {
                            if worker_stop.load(Ordering::Acquire)
                                || current_generation.load(Ordering::Acquire) != generation
                            {
                                return;
                            }
                            let sequence = item.sequence;
                            if sender
                                .blocking_send(SharedZoneLiveOutbound::new(
                                    registration_id,
                                    item.packet,
                                ))
                                .is_err()
                            {
                                return;
                            }
                            if current_generation.load(Ordering::Acquire) != generation {
                                return;
                            }
                            acknowledged.store(sequence, Ordering::Release);
                        }
                    }
                    Err(error) => {
                        if error.contains("outbound_gap") {
                            eprintln!("zone RPC live outbound requires snapshot resync: {error}");
                            return;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        });
        Ok(Some(Box::new(RemoteZoneLiveOutboundRegistration {
            registration_id,
            stop,
            active,
            handle: Some(handle),
        })))
    }
}

struct RemoteZoneLiveOutboundRegistration {
    registration_id: u64,
    stop: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl fmt::Debug for RemoteZoneLiveOutboundRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RemoteZoneLiveOutboundRegistration")
            .field("registration_id", &self.registration_id)
            .finish_non_exhaustive()
    }
}

impl ZoneLiveOutboundRegistration for RemoteZoneLiveOutboundRegistration {
    fn registration_id(&self) -> u64 {
        self.registration_id
    }

    fn activate(&self) {
        self.active.store(true, Ordering::Release);
    }
}

impl Drop for RemoteZoneLiveOutboundRegistration {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // A network read may still be inside its bounded timeout. Detach instead
        // of delaying socket teardown; the stop flag terminates the worker on its
        // next boundary.
        self.handle.take();
    }
}

struct ZoneHostSession {
    hosted: Arc<HostedZoneOwnerCommandClient>,
    outbound_sender: SharedZoneLiveOutboundSender,
    outbound_receiver: Mutex<mpsc::Receiver<SharedZoneLiveOutbound>>,
    live_registration: Mutex<Option<SharedZoneLiveOutboundRegistration>>,
    outbox: Mutex<ZoneHostOutbox>,
    max_outbound_messages: usize,
}

impl ZoneHostSession {
    fn new(hosted: Arc<HostedZoneOwnerCommandClient>, max_outbound_messages: usize) -> Self {
        let capacity = max_outbound_messages.max(1);
        let (outbound_sender, outbound_receiver) = mpsc::channel(capacity);
        Self {
            hosted,
            outbound_sender,
            outbound_receiver: Mutex::new(outbound_receiver),
            live_registration: Mutex::new(None),
            outbox: Mutex::new(ZoneHostOutbox::new()),
            max_outbound_messages: capacity,
        }
    }

    fn refresh_live_registration(&self) -> Result<(), ZoneRpcFault> {
        let registration = self
            .hosted
            .register_live_outbound(self.outbound_sender.clone())
            .map_err(classify_runtime_error)?;
        if let Some(registration) = registration {
            *self.live_registration.lock().map_err(|_| {
                ZoneRpcFault::new("internal", "zone host live registration mutex poisoned")
            })? = Some(registration);
        }
        Ok(())
    }

    fn execute_request(
        &self,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, ZoneRpcFault> {
        let execution = self
            .hosted
            .execute_request(request)
            .map_err(classify_runtime_error)?;
        self.refresh_live_registration()?;
        Ok(execution)
    }

    fn execute_replay_request(
        &self,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, ZoneRpcFault> {
        let execution = self
            .hosted
            .execute_replay_request(request)
            .map_err(classify_runtime_error)?;
        self.refresh_live_registration()?;
        Ok(execution)
    }

    fn poll_outbounds(
        &self,
        stream_id: Option<&str>,
        acknowledged_sequence: u64,
        max_items: usize,
    ) -> Result<ZoneRpcPayload, ZoneRpcFault> {
        self.refresh_live_registration()?;
        let mut receiver = self.outbound_receiver.lock().map_err(|_| {
            ZoneRpcFault::new("internal", "zone host outbound receiver mutex poisoned")
        })?;
        let mut outbox = self
            .outbox
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host outbox mutex poisoned"))?;
        while let Ok(outbound) = receiver.try_recv() {
            let frame = encode_server_packet(&outbound.into_packet()).map_err(|error| {
                ZoneRpcFault::new(
                    "packet_encode",
                    format!("live outbound packet encode failed: {error}"),
                )
            })?;
            outbox.push(frame, self.max_outbound_messages);
        }
        let reset = stream_id != Some(outbox.stream_id.as_str());
        let acknowledged_sequence = if reset { 0 } else { acknowledged_sequence };
        outbox.acknowledge(acknowledged_sequence)?;
        let max_items = max_items.max(1);
        let items = outbox
            .messages
            .iter()
            .filter(|item| item.sequence > acknowledged_sequence)
            .take(max_items)
            .cloned()
            .collect::<Vec<_>>();
        let has_more = items
            .last()
            .is_some_and(|item| item.sequence < outbox.last_issued_sequence);
        Ok(ZoneRpcPayload::Outbounds {
            items,
            stream_id: outbox.stream_id.clone(),
            reset,
            last_issued_sequence: outbox.last_issued_sequence,
            has_more,
        })
    }
}

struct ZoneHostOutbox {
    stream_id: String,
    messages: VecDeque<WireSequencedServerFrame>,
    last_issued_sequence: u64,
    dropped_through_sequence: u64,
}

impl ZoneHostOutbox {
    fn new() -> Self {
        Self {
            stream_id: next_outbound_stream_id(),
            messages: VecDeque::new(),
            last_issued_sequence: 0,
            dropped_through_sequence: 0,
        }
    }

    fn push(&mut self, frame: Vec<u8>, capacity: usize) {
        self.last_issued_sequence = self.last_issued_sequence.saturating_add(1).max(1);
        self.messages.push_back(WireSequencedServerFrame {
            sequence: self.last_issued_sequence,
            frame,
        });
        while self.messages.len() > capacity.max(1) {
            if let Some(dropped) = self.messages.pop_front() {
                self.dropped_through_sequence = dropped.sequence;
            }
        }
    }

    fn acknowledge(&mut self, acknowledged_sequence: u64) -> Result<(), ZoneRpcFault> {
        if acknowledged_sequence > self.last_issued_sequence {
            return Err(ZoneRpcFault::new(
                "invalid_ack",
                format!(
                    "acknowledged sequence {acknowledged_sequence} exceeds last issued sequence {}",
                    self.last_issued_sequence
                ),
            ));
        }
        if acknowledged_sequence < self.dropped_through_sequence {
            return Err(ZoneRpcFault::new(
                "outbound_gap",
                format!(
                    "outbound messages through sequence {} were dropped before acknowledgement {acknowledged_sequence}",
                    self.dropped_through_sequence
                ),
            ));
        }
        while self
            .messages
            .front()
            .is_some_and(|item| item.sequence <= acknowledged_sequence)
        {
            self.messages.pop_front();
        }
        Ok(())
    }
}

pub struct ZoneHostServer {
    host_id: String,
    owner_ids: BTreeSet<String>,
    config: Mutex<GatewayConfig>,
    replay_baseline_config: GatewayConfig,
    runtime_factory: Mutex<Arc<SharedInProcessZoneRuntimeFactory>>,
    owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    sessions: Mutex<BTreeMap<(String, String), Arc<ZoneHostSession>>>,
    zone_map_catalog: Mutex<ZoneMapCatalog>,
    operation_gate: Arc<SharedZoneMutationGate>,
    journal: Arc<Mutex<ZoneHostJournal>>,
    promotion_readiness: Mutex<BTreeMap<String, ZonePromotionReadinessRecord>>,
    quiesced_zones: Mutex<BTreeSet<String>>,
    promotion_frozen_zones: Mutex<BTreeSet<String>>,
    auth_token: Option<String>,
    limits: ZoneRpcLimits,
    zone_capacity: usize,
    draining: AtomicBool,
    active_connections: AtomicUsize,
    started_at_ms: u64,
    accepted_connections_total: AtomicU64,
    rpc_requests_total: AtomicU64,
    rpc_errors_total: AtomicU64,
    checkpoint_exports_total: AtomicU64,
    checkpoint_export_bytes_total: AtomicU64,
    checkpoint_export_duration_ns_total: AtomicU64,
    checkpoint_export_last_bytes: AtomicU64,
    checkpoint_export_last_duration_ns: AtomicU64,
    checkpoint_installs_total: AtomicU64,
    checkpoint_install_bytes_total: AtomicU64,
    checkpoint_install_duration_ns_total: AtomicU64,
    checkpoint_install_last_bytes: AtomicU64,
    checkpoint_install_last_duration_ns: AtomicU64,
    checkpoint_replay_entries_total: AtomicU64,
    checkpoint_replay_last_entries: AtomicU64,
    promotion_assessments_total: AtomicU64,
    promotion_ready_assessments_total: AtomicU64,
    promotion_attempts_total: AtomicU64,
    promotions_total: AtomicU64,
    promotion_last_promoted_at_ms: AtomicU64,
}

impl fmt::Debug for ZoneHostServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZoneHostServer")
            .field("host_id", &self.host_id)
            .field("authenticated", &self.auth_token.is_some())
            .field("draining", &self.draining.load(Ordering::Acquire))
            .field("limits", &self.limits)
            .field("session_count", &self.session_count())
            .finish()
    }
}

impl ZoneHostServer {
    pub fn new(
        config: GatewayConfig,
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    ) -> Self {
        Self::with_options(
            config,
            owner_lease_authority,
            std::env::var("MIR2_ZONE_HOST_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            ZoneRpcLimits::from_env(),
        )
    }

    pub fn with_options(
        config: GatewayConfig,
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
        auth_token: Option<String>,
        limits: ZoneRpcLimits,
    ) -> Self {
        Self::with_options_and_factory(
            config,
            owner_lease_authority,
            auth_token,
            limits,
            Arc::new(SharedInProcessZoneRuntimeFactory::new()),
        )
    }

    pub fn with_options_and_factory(
        config: GatewayConfig,
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
        auth_token: Option<String>,
        limits: ZoneRpcLimits,
        runtime_factory: Arc<SharedInProcessZoneRuntimeFactory>,
    ) -> Self {
        let host_id = std::env::var("MIR2_ZONE_HOST_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("zone-host-{}", std::process::id()));
        let zone_capacity = positive_usize_env("MIR2_ZONE_HOST_MAX_ZONES").unwrap_or(128);
        Self::with_identity_and_factory(
            host_id,
            zone_capacity,
            config,
            owner_lease_authority,
            auth_token,
            limits,
            runtime_factory,
        )
    }

    pub fn with_identity_and_factory(
        host_id: impl Into<String>,
        zone_capacity: usize,
        config: GatewayConfig,
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
        auth_token: Option<String>,
        limits: ZoneRpcLimits,
        runtime_factory: Arc<SharedInProcessZoneRuntimeFactory>,
    ) -> Self {
        let host_id = host_id.into();
        let owner_ids = configured_zone_host_owner_ids(&host_id);
        let replay_baseline_config = config
            .fork_with_isolated_account_store()
            .expect("Zone Host replay baseline account store should be available");
        let operation_gate = Arc::new(SharedZoneMutationGate::default());
        let journal = Arc::new(Mutex::new(ZoneHostJournal::default()));
        let tick_journal = Arc::clone(&journal);
        let tick_authority = Arc::clone(&owner_lease_authority);
        let tick_host_id = host_id.clone();
        let tick_owner_ids = owner_ids.clone();
        runtime_factory.configure_mutation_capture(
            Arc::clone(&operation_gate),
            Arc::new(move |zone_id| {
                let lease = tick_authority.owner_lease(zone_id);
                let legacy_in_process_owner =
                    lease.owner_id() == "in-process" || lease.owner_id().starts_with("in-process:");
                if !legacy_in_process_owner && !tick_owner_ids.contains(lease.owner_id()) {
                    return Err(format!(
                        "Zone {zone_id} is fenced to {}, local host {} is not active",
                        lease.owner_id(),
                        tick_host_id
                    ));
                }
                tick_authority.validate_owner_lease(&lease)
            }),
            Arc::new(move |zone_id, now_ms| {
                append_host_journal(
                    tick_journal.as_ref(),
                    WireHostJournalEntry {
                        sequence: 0,
                        session_id: "__zone_tick__".to_string(),
                        zone_id: zone_id.as_str().to_string(),
                        owner_lease: WireZoneOwnerLease::from(&ZoneOwnerLease::in_process(zone_id)),
                        mode: WireZoneOwnerCommandMode::Direct,
                        command: None,
                        closed: false,
                        zone_tick_ms: Some(now_ms),
                    },
                )
                .map_err(|error| error.message)
            }),
        );
        Self {
            host_id,
            owner_ids,
            config: Mutex::new(config),
            replay_baseline_config,
            runtime_factory: Mutex::new(runtime_factory),
            owner_lease_authority,
            sessions: Mutex::new(BTreeMap::new()),
            zone_map_catalog: Mutex::new(ZoneMapCatalog::default()),
            operation_gate,
            journal,
            promotion_readiness: Mutex::new(BTreeMap::new()),
            quiesced_zones: Mutex::new(BTreeSet::new()),
            promotion_frozen_zones: Mutex::new(BTreeSet::new()),
            auth_token,
            limits,
            zone_capacity: zone_capacity.max(1),
            draining: AtomicBool::new(false),
            active_connections: AtomicUsize::new(0),
            started_at_ms: unix_now_ms(),
            accepted_connections_total: AtomicU64::new(0),
            rpc_requests_total: AtomicU64::new(0),
            rpc_errors_total: AtomicU64::new(0),
            checkpoint_exports_total: AtomicU64::new(0),
            checkpoint_export_bytes_total: AtomicU64::new(0),
            checkpoint_export_duration_ns_total: AtomicU64::new(0),
            checkpoint_export_last_bytes: AtomicU64::new(0),
            checkpoint_export_last_duration_ns: AtomicU64::new(0),
            checkpoint_installs_total: AtomicU64::new(0),
            checkpoint_install_bytes_total: AtomicU64::new(0),
            checkpoint_install_duration_ns_total: AtomicU64::new(0),
            checkpoint_install_last_bytes: AtomicU64::new(0),
            checkpoint_install_last_duration_ns: AtomicU64::new(0),
            checkpoint_replay_entries_total: AtomicU64::new(0),
            checkpoint_replay_last_entries: AtomicU64::new(0),
            promotion_assessments_total: AtomicU64::new(0),
            promotion_ready_assessments_total: AtomicU64::new(0),
            promotion_attempts_total: AtomicU64::new(0),
            promotions_total: AtomicU64::new(0),
            promotion_last_promoted_at_ms: AtomicU64::new(0),
        }
    }

    pub fn health(&self) -> ZoneHostHealth {
        ZoneHostHealth {
            host_id: self.host_id.clone(),
            process_id: std::process::id(),
            session_count: self.session_count(),
            active_connections: self.active_connections.load(Ordering::Acquire),
            session_capacity: self.limits.max_sessions,
            session_capacity_per_zone: self.limits.max_sessions_per_zone,
            busiest_zone_session_count: self.busiest_zone_session_count(),
            zone_count: self.zone_count(),
            zone_capacity: self.zone_capacity,
            draining: self.is_draining(),
            protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        }
    }

    pub fn telemetry_snapshot(&self) -> ZoneHostTelemetrySnapshot {
        let now_ms = unix_now_ms();
        let ready_zone_ids = self
            .promotion_readiness
            .lock()
            .map(|records| {
                records
                    .values()
                    .filter(|record| {
                        record.readiness.ready
                            && record
                                .readiness
                                .expires_at_ms
                                .is_some_and(|expires_at_ms| now_ms <= expires_at_ms)
                    })
                    .map(|record| record.readiness.zone_id.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default();
        ZoneHostTelemetrySnapshot {
            health: self.health(),
            zones: self.active_zones(),
            checkpoint: ZoneHostCheckpointTelemetry {
                journal_entries: self
                    .journal
                    .lock()
                    .map(|journal| saturating_u64(journal.entries.len()))
                    .unwrap_or_default(),
                exports_total: self.checkpoint_exports_total.load(Ordering::Acquire),
                export_bytes_total: self.checkpoint_export_bytes_total.load(Ordering::Acquire),
                export_duration_ns_total: self
                    .checkpoint_export_duration_ns_total
                    .load(Ordering::Acquire),
                export_last_bytes: self.checkpoint_export_last_bytes.load(Ordering::Acquire),
                export_last_duration_ns: self
                    .checkpoint_export_last_duration_ns
                    .load(Ordering::Acquire),
                installs_total: self.checkpoint_installs_total.load(Ordering::Acquire),
                install_bytes_total: self.checkpoint_install_bytes_total.load(Ordering::Acquire),
                install_duration_ns_total: self
                    .checkpoint_install_duration_ns_total
                    .load(Ordering::Acquire),
                install_last_bytes: self.checkpoint_install_last_bytes.load(Ordering::Acquire),
                install_last_duration_ns: self
                    .checkpoint_install_last_duration_ns
                    .load(Ordering::Acquire),
                replay_entries_total: self.checkpoint_replay_entries_total.load(Ordering::Acquire),
                replay_last_entries: self.checkpoint_replay_last_entries.load(Ordering::Acquire),
            },
            promotion: ZoneHostPromotionTelemetry {
                assessments_total: self.promotion_assessments_total.load(Ordering::Acquire),
                ready_assessments_total: self
                    .promotion_ready_assessments_total
                    .load(Ordering::Acquire),
                promotion_attempts_total: self.promotion_attempts_total.load(Ordering::Acquire),
                promotions_total: self.promotions_total.load(Ordering::Acquire),
                last_promoted_at_ms: self.promotion_last_promoted_at_ms.load(Ordering::Acquire),
                ready_zone_ids,
            },
            started_at_ms: self.started_at_ms,
            uptime_seconds: unix_now_ms().saturating_sub(self.started_at_ms) / 1_000,
            accepted_connections_total: self.accepted_connections_total.load(Ordering::Acquire),
            rpc_requests_total: self.rpc_requests_total.load(Ordering::Acquire),
            rpc_errors_total: self.rpc_errors_total.load(Ordering::Acquire),
        }
    }

    pub fn replication_head(&self, zone_id: &ZoneId) -> Result<ZoneReplicationHead, String> {
        let mut head = self
            .journal
            .lock()
            .map(|journal| journal.replication.head(zone_id.as_str()))
            .map_err(|_| "zone host journal mutex poisoned".to_string())?;
        let now_ms = unix_now_ms();
        let replica_clock_disabled = self
            .runtime_factory
            .lock()
            .map(|factory| {
                factory.is_zone_replica(zone_id) && !factory.autonomous_ticks_enabled(zone_id)
            })
            .unwrap_or(false);
        head.promotion_ready = replica_clock_disabled
            && self
                .promotion_readiness
                .lock()
                .map(|records| {
                    records.values().any(|record| {
                        record.readiness.zone_id == zone_id.as_str()
                            && record.readiness.ready
                            && record
                                .readiness
                                .expires_at_ms
                                .is_some_and(|expires_at_ms| now_ms <= expires_at_ms)
                            && replication_heads_match(&record.head, &head)
                    })
                })
                .unwrap_or(false);
        Ok(head)
    }

    fn prune_expired_promotion_readiness(&self, now_ms: u64) -> Result<(), ZoneRpcFault> {
        let mut records = self
            .promotion_readiness
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "promotion readiness mutex poisoned"))?;
        records.retain(|_, record| {
            record
                .readiness
                .expires_at_ms
                .is_some_and(|expires_at_ms| now_ms <= expires_at_ms)
        });
        let ready_zones = records
            .values()
            .filter(|record| record.readiness.ready)
            .map(|record| record.readiness.zone_id.as_str())
            .collect::<BTreeSet<_>>();
        self.promotion_frozen_zones
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "promotion freeze mutex poisoned"))?
            .retain(|zone_id| ready_zones.contains(zone_id.as_str()));
        Ok(())
    }

    fn accepts_owner_id(&self, owner_id: &str) -> bool {
        self.owner_ids.contains(owner_id)
    }

    fn quiesce_for_promotion_locked(
        &self,
        zone_id: &ZoneId,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<ZoneQuiesceReceipt, ZoneRpcFault> {
        if owner_lease.zone_id() != zone_id {
            return Err(ZoneRpcFault::new(
                "zone_mismatch",
                "quiesce lease Zone does not match requested Zone",
            ));
        }
        if !self.accepts_owner_id(owner_lease.owner_id()) {
            return Err(ZoneRpcFault::new(
                "quiesce_owner_mismatch",
                format!(
                    "quiesce lease owner {} does not match active host {}",
                    owner_lease.owner_id(),
                    self.host_id
                ),
            ));
        }
        self.owner_lease_authority
            .validate_owner_lease(owner_lease)
            .map_err(|message| ZoneRpcFault::new("quiesce_fence_rejected", message))?;
        self.runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
            .quiesce_active_zone(zone_id)
            .map_err(|message| ZoneRpcFault::new("quiesce_runtime_invalid", message))?;
        self.quiesced_zones
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "quiesced Zone mutex poisoned"))?
            .insert(zone_id.as_str().to_string());
        let head = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
            .replication
            .head(zone_id.as_str());
        Ok(ZoneQuiesceReceipt {
            version: ZONE_PROMOTION_READINESS_VERSION,
            zone_id: zone_id.as_str().to_string(),
            host_id: self.host_id.clone(),
            owner_id: owner_lease.owner_id().to_string(),
            generation: owner_lease.fencing_token(),
            quiesced_at_ms: unix_now_ms(),
            head,
        })
    }

    fn resume_after_quiesce_locked(
        &self,
        zone_id: &ZoneId,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<(), ZoneRpcFault> {
        if owner_lease.zone_id() != zone_id || !self.accepts_owner_id(owner_lease.owner_id()) {
            return Err(ZoneRpcFault::new(
                "resume_owner_mismatch",
                "resume lease does not name this Zone Host",
            ));
        }
        self.owner_lease_authority
            .validate_owner_lease(owner_lease)
            .map_err(|message| ZoneRpcFault::new("resume_fence_rejected", message))?;
        self.runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
            .resume_active_zone(zone_id)
            .map_err(|message| ZoneRpcFault::new("resume_runtime_invalid", message))?;
        self.quiesced_zones
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "quiesced Zone mutex poisoned"))?
            .remove(zone_id.as_str());
        Ok(())
    }

    pub fn assess_promotion_readiness(
        &self,
        zone_id: &ZoneId,
        active_head: &ZoneReplicationHead,
        source_observed_at_ms: u64,
        max_lag_ms: u64,
    ) -> Result<ZonePromotionReadiness, String> {
        let _operation = self
            .operation_gate
            .lock_zone(zone_id)
            .map_err(|_| "zone host operation mutex poisoned".to_string())?;
        self.assess_promotion_readiness_locked(
            zone_id,
            active_head,
            source_observed_at_ms,
            max_lag_ms,
        )
        .map_err(|error| error.message)
    }

    fn assess_promotion_readiness_locked(
        &self,
        zone_id: &ZoneId,
        active_head: &ZoneReplicationHead,
        source_observed_at_ms: u64,
        max_lag_ms: u64,
    ) -> Result<ZonePromotionReadiness, ZoneRpcFault> {
        if active_head.version != ZONE_REPLICATION_HEAD_VERSION {
            return Err(ZoneRpcFault::new(
                "promotion_head_version",
                format!(
                    "active replication head version {} does not match {}",
                    active_head.version, ZONE_REPLICATION_HEAD_VERSION
                ),
            ));
        }
        if active_head.zone_id != zone_id.as_str() {
            return Err(ZoneRpcFault::new(
                "zone_mismatch",
                "active replication head Zone does not match requested Zone",
            ));
        }
        let assessed_at_ms = unix_now_ms();
        if source_observed_at_ms > assessed_at_ms.saturating_add(1_000) {
            return Err(ZoneRpcFault::new(
                "promotion_clock_skew",
                "active replication head observation is in the future",
            ));
        }
        let max_lag_ms = max_lag_ms.max(1).min(5_000);
        let observed_lag_ms = assessed_at_ms.saturating_sub(source_observed_at_ms);
        let standby_head = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
            .replication
            .head(zone_id.as_str());
        let factory = self
            .runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
            .clone();
        let replica_clock_disabled =
            factory.is_zone_replica(zone_id) && !factory.autonomous_ticks_enabled(zone_id);
        let session_count = self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))?
            .keys()
            .filter(|(_, session_zone_id)| session_zone_id == zone_id.as_str())
            .count();
        let zone_count = self.zone_count();
        let build_matches = active_head.build_id == standby_head.build_id
            && active_head.mutation_coverage == standby_head.mutation_coverage;
        let cursor_matches = active_head.next_sequence == standby_head.next_sequence;
        let digest_matches = active_head.latest_digest == standby_head.latest_digest;
        let base_matches = standby_head.base_snapshot_id.is_some()
            && standby_head.base_sequence <= standby_head.next_sequence;
        let capacity_available = !self.is_draining()
            && session_count <= self.limits.max_sessions_per_zone
            && self.session_count() <= self.limits.max_sessions
            && zone_count <= self.zone_capacity;
        let lag_ready = observed_lag_ms <= max_lag_ms;
        let ready = build_matches
            && cursor_matches
            && digest_matches
            && base_matches
            && replica_clock_disabled
            && capacity_available
            && lag_ready;
        let reason = if ready {
            None
        } else if !build_matches {
            Some("build_or_coverage_mismatch".to_string())
        } else if !cursor_matches {
            Some("replica_cursor_behind".to_string())
        } else if !digest_matches {
            Some("replica_digest_mismatch".to_string())
        } else if !base_matches {
            Some("restorable_base_missing".to_string())
        } else if !replica_clock_disabled {
            Some("replica_clock_not_disabled".to_string())
        } else if !capacity_available {
            Some("standby_capacity_unavailable".to_string())
        } else {
            Some("replication_observation_stale".to_string())
        };
        let expires_at_ms =
            ready.then(|| assessed_at_ms.saturating_add(DEFAULT_ZONE_PROMOTION_RECEIPT_TTL_MS));
        let readiness_id = ready.then(|| {
            zone_promotion_readiness_id(
                zone_id,
                &self.host_id,
                &standby_head,
                assessed_at_ms,
                expires_at_ms.unwrap_or_default(),
            )
        });
        let readiness = ZonePromotionReadiness {
            version: ZONE_PROMOTION_READINESS_VERSION,
            readiness_id: readiness_id.clone(),
            zone_id: zone_id.as_str().to_string(),
            standby_host_id: self.host_id.clone(),
            active_build_id: active_head.build_id.clone(),
            standby_build_id: standby_head.build_id.clone(),
            active_next_sequence: active_head.next_sequence,
            standby_next_sequence: standby_head.next_sequence,
            active_latest_digest: active_head.latest_digest.clone(),
            standby_latest_digest: standby_head.latest_digest.clone(),
            source_observed_at_ms,
            assessed_at_ms,
            observed_lag_ms,
            max_lag_ms,
            expires_at_ms,
            session_count,
            session_capacity: self.limits.max_sessions_per_zone,
            zone_count,
            zone_capacity: self.zone_capacity,
            build_matches,
            cursor_matches,
            digest_matches,
            base_matches,
            replica_clock_disabled,
            capacity_available,
            ready,
            reason,
        };
        self.promotion_assessments_total
            .fetch_add(1, Ordering::Relaxed);
        if readiness.ready {
            self.promotion_ready_assessments_total
                .fetch_add(1, Ordering::Relaxed);
        }
        let mut records = self
            .promotion_readiness
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "promotion readiness mutex poisoned"))?;
        records.retain(|_, record| {
            record
                .readiness
                .expires_at_ms
                .is_some_and(|expires_at_ms| assessed_at_ms <= expires_at_ms)
        });
        if let Some(readiness_id) = readiness_id {
            records.insert(
                readiness_id,
                ZonePromotionReadinessRecord {
                    readiness: readiness.clone(),
                    head: standby_head,
                },
            );
            self.promotion_frozen_zones
                .lock()
                .map_err(|_| ZoneRpcFault::new("internal", "promotion freeze mutex poisoned"))?
                .insert(zone_id.as_str().to_string());
        } else {
            records.retain(|_, record| record.readiness.zone_id != zone_id.as_str());
            self.promotion_frozen_zones
                .lock()
                .map_err(|_| ZoneRpcFault::new("internal", "promotion freeze mutex poisoned"))?
                .remove(zone_id.as_str());
        }
        Ok(readiness)
    }

    pub fn promote_replica(
        &self,
        zone_id: &ZoneId,
        readiness_id: &str,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<ZonePromotionReceipt, String> {
        let _operation = self
            .operation_gate
            .lock_zone(zone_id)
            .map_err(|_| "zone host operation mutex poisoned".to_string())?;
        self.promote_replica_locked(zone_id, readiness_id, owner_lease)
            .map_err(|error| error.message)
    }

    fn promote_replica_locked(
        &self,
        zone_id: &ZoneId,
        readiness_id: &str,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<ZonePromotionReceipt, ZoneRpcFault> {
        self.promotion_attempts_total
            .fetch_add(1, Ordering::Relaxed);
        if readiness_id.trim().is_empty() {
            return Err(ZoneRpcFault::new(
                "promotion_receipt_missing",
                "promotion readiness id is required",
            ));
        }
        if owner_lease.zone_id() != zone_id {
            return Err(ZoneRpcFault::new(
                "zone_mismatch",
                "promotion lease Zone does not match requested Zone",
            ));
        }
        if !self.accepts_owner_id(owner_lease.owner_id()) {
            return Err(ZoneRpcFault::new(
                "promotion_owner_mismatch",
                format!(
                    "promotion lease owner {} does not match standby host {}",
                    owner_lease.owner_id(),
                    self.host_id
                ),
            ));
        }
        self.owner_lease_authority
            .validate_owner_lease(owner_lease)
            .map_err(|message| ZoneRpcFault::new("promotion_fence_rejected", message))?;
        let now_ms = unix_now_ms();
        let record = self
            .promotion_readiness
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "promotion readiness mutex poisoned"))?
            .get(readiness_id)
            .cloned()
            .ok_or_else(|| {
                ZoneRpcFault::new(
                    "promotion_receipt_unknown",
                    "promotion readiness receipt is unknown or already consumed",
                )
            })?;
        if record.readiness.zone_id != zone_id.as_str()
            || !record.readiness.ready
            || record
                .readiness
                .expires_at_ms
                .is_none_or(|expires_at_ms| now_ms > expires_at_ms)
        {
            return Err(ZoneRpcFault::new(
                "promotion_receipt_expired",
                "promotion readiness receipt is no longer valid",
            ));
        }
        let current_head = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
            .replication
            .head(zone_id.as_str());
        if !replication_heads_match(&record.head, &current_head) {
            return Err(ZoneRpcFault::new(
                "promotion_replica_changed",
                "standby replication head changed after readiness was assessed",
            ));
        }
        self.runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
            .promote_zone_from_replica(zone_id)
            .map_err(|message| ZoneRpcFault::new("promotion_replica_invalid", message))?;
        self.promotion_readiness
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "promotion readiness mutex poisoned"))?
            .remove(readiness_id);
        self.promotion_frozen_zones
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "promotion freeze mutex poisoned"))?
            .remove(zone_id.as_str());
        self.promotions_total.fetch_add(1, Ordering::Relaxed);
        self.promotion_last_promoted_at_ms
            .store(now_ms, Ordering::Release);
        Ok(ZonePromotionReceipt {
            version: ZONE_PROMOTION_READINESS_VERSION,
            readiness_id: readiness_id.to_string(),
            zone_id: zone_id.as_str().to_string(),
            promoted_host_id: self.host_id.clone(),
            owner_id: owner_lease.owner_id().to_string(),
            generation: owner_lease.fencing_token(),
            promoted_at_ms: now_ms,
            head: current_head,
        })
    }

    pub fn export_mutation_batch(
        &self,
        zone_id: &ZoneId,
        first_sequence: u64,
        max_entries: usize,
        max_payload_bytes: usize,
    ) -> Result<ZoneMutationBatch, String> {
        let max_entries = max_entries
            .max(1)
            .min(DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES);
        let max_payload_bytes = max_payload_bytes
            .max(1)
            .min(DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES)
            .min(self.limits.max_frame_bytes.saturating_div(2).max(1));
        self.journal
            .lock()
            .map_err(|_| "zone host journal mutex poisoned".to_string())?
            .export_batch(
                zone_id.as_str(),
                first_sequence,
                max_entries,
                max_payload_bytes,
            )
            .map_err(|error| error.message)
    }

    pub fn session_count(&self) -> usize {
        self.sessions
            .lock()
            .map(|sessions| sessions.len())
            .unwrap_or(0)
    }

    pub fn configure_zone_map_catalog(
        &self,
        maps_by_zone: BTreeMap<String, Vec<String>>,
        all_maps_zone_ids: BTreeSet<String>,
    ) {
        if let Ok(mut catalog) = self.zone_map_catalog.lock() {
            catalog.maps_by_zone = maps_by_zone;
            catalog.all_maps_zone_ids = all_maps_zone_ids;
        }
    }

    pub fn active_zones(&self) -> Vec<ZoneHostZoneTelemetry> {
        let counts = self
            .sessions
            .lock()
            .map(|sessions| {
                let mut counts = BTreeMap::<String, usize>::new();
                for (_, zone_id) in sessions.keys() {
                    *counts.entry(zone_id.clone()).or_default() += 1;
                }
                counts
            })
            .unwrap_or_default();
        let Ok(catalog) = self.zone_map_catalog.lock() else {
            return counts
                .into_iter()
                .map(|(zone_id, session_count)| unknown_zone_telemetry(zone_id, session_count))
                .collect();
        };
        counts
            .into_iter()
            .map(|(zone_id, session_count)| {
                if catalog.all_maps_zone_ids.contains(&zone_id) {
                    return ZoneHostZoneTelemetry {
                        zone_id,
                        map_scope: ZoneMapScope::All,
                        map_file_names: Vec::new(),
                        session_count,
                    };
                }
                if let Some(map_file_names) = catalog.maps_by_zone.get(&zone_id) {
                    return ZoneHostZoneTelemetry {
                        zone_id,
                        map_scope: ZoneMapScope::Explicit,
                        map_file_names: map_file_names.clone(),
                        session_count,
                    };
                }
                if let Some(map_file_name) = dynamic_map_file_name(&zone_id) {
                    return ZoneHostZoneTelemetry {
                        zone_id,
                        map_scope: ZoneMapScope::Explicit,
                        map_file_names: vec![map_file_name],
                        session_count,
                    };
                }
                unknown_zone_telemetry(zone_id, session_count)
            })
            .collect()
    }

    pub fn zone_count(&self) -> usize {
        self.sessions
            .lock()
            .map(|sessions| {
                sessions
                    .keys()
                    .map(|(_, zone_id)| zone_id.as_str())
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
            })
            .unwrap_or(0)
    }

    pub fn busiest_zone_session_count(&self) -> usize {
        self.sessions
            .lock()
            .map(|sessions| {
                let mut counts = BTreeMap::<&str, usize>::new();
                for (_, zone_id) in sessions.keys() {
                    *counts.entry(zone_id.as_str()).or_default() += 1;
                }
                counts.into_values().max().unwrap_or(0)
            })
            .unwrap_or(0)
    }

    pub fn set_draining(&self, draining: bool) {
        self.draining.store(draining, Ordering::Release);
    }

    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::Acquire)
    }

    pub fn serve(self: Arc<Self>, listener: TcpListener) -> io::Result<()> {
        for stream in listener.incoming() {
            let stream = stream?;
            self.spawn_connection(stream);
        }
        Ok(())
    }

    pub fn serve_until(
        self: Arc<Self>,
        listener: TcpListener,
        stop: Arc<AtomicBool>,
    ) -> io::Result<()> {
        listener.set_nonblocking(true)?;
        while !stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => self.spawn_connection(stream),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn spawn_connection(self: &Arc<Self>, stream: TcpStream) {
        if self.active_connections.fetch_add(1, Ordering::AcqRel) >= self.limits.max_connections {
            self.active_connections.fetch_sub(1, Ordering::AcqRel);
            return;
        }
        self.accepted_connections_total
            .fetch_add(1, Ordering::Relaxed);
        let server = Arc::clone(self);
        thread::spawn(move || {
            let _guard = ActiveConnectionGuard(&server.active_connections);
            if let Err(error) = server.handle_connection(stream) {
                eprintln!("zone host connection error: {error}");
            }
        });
    }

    fn handle_connection(&self, mut stream: TcpStream) -> io::Result<()> {
        // `serve_until` uses a non-blocking listener so it can observe the stop
        // flag. Some platforms propagate that mode to accepted sockets; the
        // framed request handler is deliberately blocking with bounded timeouts.
        stream.set_nonblocking(false)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(self.limits.io_timeout))?;
        stream.set_write_timeout(Some(self.limits.io_timeout))?;
        loop {
            let bytes = match read_frame_allowing_idle(&mut stream, self.limits.max_frame_bytes) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => continue,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::UnexpectedEof
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::BrokenPipe
                    ) =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            self.rpc_requests_total.fetch_add(1, Ordering::Relaxed);
            let codec = detect_rpc_codec(&bytes);
            let response = match decode_rpc_envelope(&bytes, codec) {
                Ok(envelope) => match self.handle_envelope(envelope) {
                    Ok(payload) => ZoneRpcResponse::Ok {
                        payload: Box::new(payload),
                    },
                    Err(error) => error.into_response(),
                },
                Err(error) => ZoneRpcResponse::Error {
                    code: "invalid_request".to_string(),
                    message: format!("invalid JSON request: {error}"),
                },
            };
            if matches!(response, ZoneRpcResponse::Error { .. }) {
                self.rpc_errors_total.fetch_add(1, Ordering::Relaxed);
            }
            let bytes = encode_rpc_response(&response, codec).map_err(io::Error::other)?;
            write_frame(&mut stream, &bytes, self.limits.max_frame_bytes)?;
        }
    }

    fn handle_envelope(&self, envelope: ZoneRpcEnvelope) -> Result<ZoneRpcPayload, ZoneRpcFault> {
        if envelope.protocol_version != ZONE_RPC_PROTOCOL_VERSION {
            return Err(ZoneRpcFault::new(
                "unsupported_version",
                format!(
                    "expected protocol version {}, got {}",
                    ZONE_RPC_PROTOCOL_VERSION, envelope.protocol_version
                ),
            ));
        }
        if !tokens_equal(self.auth_token.as_deref(), envelope.auth_token.as_deref()) {
            return Err(ZoneRpcFault::new("unauthorized", "invalid zone host token"));
        }
        validate_identifier("RPC session id", &envelope.session_id)
            .map_err(|message| ZoneRpcFault::new("invalid_request", message))?;
        validate_identifier("zone id", &envelope.zone_id)
            .map_err(|message| ZoneRpcFault::new("invalid_request", message))?;

        if matches!(envelope.request, ZoneRpcRequest::Health) {
            let health = self.health();
            return Ok(ZoneRpcPayload::Health {
                host_id: health.host_id,
                process_id: health.process_id,
                session_count: health.session_count,
                active_connections: health.active_connections,
                session_capacity: health.session_capacity,
                session_capacity_per_zone: health.session_capacity_per_zone,
                busiest_zone_session_count: health.busiest_zone_session_count,
                zone_count: health.zone_count,
                zone_capacity: health.zone_capacity,
                draining: health.draining,
                protocol_version: health.protocol_version,
            });
        }

        let request = envelope.request;
        let zone_id = ZoneId::new(&envelope.zone_id);
        let _operation = if matches!(
            &request,
            ZoneRpcRequest::ExportHostCheckpoint | ZoneRpcRequest::InstallHostCheckpoint { .. }
        ) {
            self.operation_gate.lock()
        } else {
            self.operation_gate.lock_zone(&zone_id)
        }
        .map_err(|_| ZoneRpcFault::new("internal", "zone host operation mutex poisoned"))?;
        self.prune_expired_promotion_readiness(unix_now_ms())?;
        if self
            .quiesced_zones
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "quiesced Zone mutex poisoned"))?
            .contains(&envelope.zone_id)
            || self
                .promotion_frozen_zones
                .lock()
                .map_err(|_| ZoneRpcFault::new("internal", "promotion freeze mutex poisoned"))?
                .contains(&envelope.zone_id)
        {
            if !zone_rpc_request_requires_active_mutation(&request) {
                // Read-only and replication administration remains available
                // while the promotion barrier is held.
            } else {
                return Err(ZoneRpcFault::new(
                    "zone_quiesced",
                    format!(
                        "Zone {} is quiesced for a safe promotion and rejects new mutations",
                        envelope.zone_id
                    ),
                ));
            }
        }
        match request {
            ZoneRpcRequest::ReplicationHead => {
                return self
                    .replication_head(&ZoneId::new(&envelope.zone_id))
                    .map(|head| ZoneRpcPayload::ReplicationHead { head })
                    .map_err(|message| ZoneRpcFault::new("internal", message));
            }
            ZoneRpcRequest::ExportMutationBatch {
                first_sequence,
                max_entries,
                max_payload_bytes,
            } => {
                let max_entries = max_entries
                    .max(1)
                    .min(DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES);
                let max_payload_bytes = max_payload_bytes
                    .max(1)
                    .min(DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES)
                    .min(self.limits.max_frame_bytes.saturating_div(2).max(1));
                return self
                    .journal
                    .lock()
                    .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
                    .export_batch(
                        &envelope.zone_id,
                        first_sequence,
                        max_entries,
                        max_payload_bytes,
                    )
                    .map(|batch| ZoneRpcPayload::MutationBatch { batch });
            }
            ZoneRpcRequest::ExportBaseSnapshot => {
                return self.export_base_snapshot(&ZoneId::new(envelope.zone_id));
            }
            ZoneRpcRequest::InstallBaseSnapshot { snapshot } => {
                self.install_base_snapshot(&envelope.zone_id, &snapshot)?;
                return Ok(ZoneRpcPayload::Unit);
            }
            ZoneRpcRequest::ApplyMutationBatch { batch } => {
                self.apply_mutation_batch(&envelope.zone_id, &batch)?;
                return Ok(ZoneRpcPayload::Unit);
            }
            ZoneRpcRequest::AssessPromotionReadiness {
                active_head,
                source_observed_at_ms,
                max_lag_ms,
            } => {
                let readiness = self.assess_promotion_readiness_locked(
                    &ZoneId::new(&envelope.zone_id),
                    &active_head,
                    source_observed_at_ms,
                    max_lag_ms,
                )?;
                return Ok(ZoneRpcPayload::PromotionReadiness { readiness });
            }
            ZoneRpcRequest::PromoteReplica {
                readiness_id,
                owner_lease,
            } => {
                let owner_lease = owner_lease.into_lease()?;
                let receipt = self.promote_replica_locked(
                    &ZoneId::new(&envelope.zone_id),
                    &readiness_id,
                    &owner_lease,
                )?;
                return Ok(ZoneRpcPayload::PromotionReceipt { receipt });
            }
            ZoneRpcRequest::QuiesceForPromotion { owner_lease } => {
                let owner_lease = owner_lease.into_lease()?;
                let receipt = self
                    .quiesce_for_promotion_locked(&ZoneId::new(&envelope.zone_id), &owner_lease)?;
                return Ok(ZoneRpcPayload::QuiesceReceipt { receipt });
            }
            ZoneRpcRequest::ResumeAfterQuiesce { owner_lease } => {
                let owner_lease = owner_lease.into_lease()?;
                self.resume_after_quiesce_locked(&ZoneId::new(&envelope.zone_id), &owner_lease)?;
                return Ok(ZoneRpcPayload::Unit);
            }
            ZoneRpcRequest::ExportHostCheckpoint => return self.export_host_checkpoint(),
            ZoneRpcRequest::InstallHostCheckpoint { bytes } => {
                self.install_host_checkpoint(&bytes)?;
                return Ok(ZoneRpcPayload::Unit);
            }
            ZoneRpcRequest::CloseSession { owner_lease } => {
                return self.close_hosted_session(
                    &envelope.session_id,
                    &envelope.zone_id,
                    owner_lease,
                );
            }
            request => {
                let session = self.hosted_session(&envelope.session_id, &envelope.zone_id)?;
                return self.handle_session_request(
                    &envelope.session_id,
                    &envelope.zone_id,
                    session,
                    request,
                );
            }
        }
    }

    fn handle_session_request(
        &self,
        session_id: &str,
        zone_id: &str,
        session: Arc<ZoneHostSession>,
        request: ZoneRpcRequest,
    ) -> Result<ZoneRpcPayload, ZoneRpcFault> {
        match request {
            ZoneRpcRequest::Health => unreachable!(),
            ZoneRpcRequest::ReplicationHead
            | ZoneRpcRequest::ExportMutationBatch { .. }
            | ZoneRpcRequest::ExportBaseSnapshot
            | ZoneRpcRequest::InstallBaseSnapshot { .. }
            | ZoneRpcRequest::ApplyMutationBatch { .. }
            | ZoneRpcRequest::AssessPromotionReadiness { .. }
            | ZoneRpcRequest::PromoteReplica { .. }
            | ZoneRpcRequest::QuiesceForPromotion { .. }
            | ZoneRpcRequest::ResumeAfterQuiesce { .. }
            | ZoneRpcRequest::ExportHostCheckpoint
            | ZoneRpcRequest::InstallHostCheckpoint { .. } => {
                unreachable!()
            }
            ZoneRpcRequest::CloseSession { .. } => unreachable!(),
            ZoneRpcRequest::OnConnect => Ok(ZoneRpcPayload::Packets {
                frames: encode_server_frames(session.hosted.on_connect()?)?,
            }),
            ZoneRpcRequest::Execute {
                owner_lease,
                mode,
                command,
            } => {
                if owner_lease.zone_id != zone_id {
                    return Err(ZoneRpcFault::new(
                        "zone_mismatch",
                        "lease zone id does not match envelope zone id",
                    ));
                }
                let source_sequence = self
                    .journal
                    .lock()
                    .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
                    .replication
                    .head(zone_id)
                    .next_sequence;
                let journal_entry = WireHostJournalEntry {
                    sequence: 0,
                    session_id: session_id.to_string(),
                    zone_id: zone_id.to_string(),
                    owner_lease: owner_lease.clone(),
                    mode: mode.clone(),
                    command: Some(command.clone()),
                    closed: false,
                    zone_tick_ms: None,
                };
                let request = match mode.into_mode() {
                    ZoneOwnerCommandMode::Direct => ZoneOwnerCommandRequest::direct(
                        owner_lease.into_lease()?,
                        command.into_world()?,
                    ),
                    ZoneOwnerCommandMode::ProductionPlayer { authenticated } => {
                        ZoneOwnerCommandRequest::production_player(
                            owner_lease.into_lease()?,
                            authenticated,
                            command.into_world()?,
                        )
                    }
                }
                .with_source_sequence(source_sequence);
                let execution = session.execute_request(request)?;
                self.append_journal(journal_entry)?;
                Ok(ZoneRpcPayload::Execution {
                    frames: encode_server_frames(execution.packets)?,
                    packet_count: execution.outcome.packet_count,
                    snapshot_tick: execution.outcome.snapshot_tick,
                    active_identity: execution.outcome.active_identity,
                })
            }
            ZoneRpcRequest::PollOutbounds {
                stream_id,
                acknowledged_sequence,
                max_items,
            } => session.poll_outbounds(stream_id.as_deref(), acknowledged_sequence, max_items),
            ZoneRpcRequest::WorldSnapshot => Ok(ZoneRpcPayload::WorldSnapshot {
                snapshot: Box::new(
                    session
                        .hosted
                        .world_snapshot()
                        .map_err(classify_runtime_error)?,
                ),
            }),
            ZoneRpcRequest::ActiveIdentity => Ok(ZoneRpcPayload::ActiveIdentity {
                identity: session
                    .hosted
                    .active_identity()
                    .map_err(classify_runtime_error)?,
            }),
            ZoneRpcRequest::SaveActiveCharacter => {
                ZoneOwnerRpcTransport::save_active_character(session.hosted.as_ref())
                    .map_err(classify_runtime_error)?;
                Ok(ZoneRpcPayload::Unit)
            }
            ZoneRpcRequest::RefreshActiveExternalMail => Ok(ZoneRpcPayload::Bool {
                value: ZoneOwnerRpcTransport::refresh_active_external_mail(session.hosted.as_ref())
                    .map_err(classify_runtime_error)?,
            }),
        }
    }

    fn close_hosted_session(
        &self,
        session_id: &str,
        zone_id: &str,
        owner_lease: WireZoneOwnerLease,
    ) -> Result<ZoneRpcPayload, ZoneRpcFault> {
        if owner_lease.zone_id != zone_id {
            return Err(ZoneRpcFault::new(
                "zone_mismatch",
                "lease zone id does not match envelope zone id",
            ));
        }
        let lease = owner_lease.clone().into_lease()?;
        self.owner_lease_authority
            .validate_owner_lease(&lease)
            .map_err(classify_runtime_error)?;
        let removed = self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))?
            .remove(&(session_id.to_string(), zone_id.to_string()));
        if removed.is_some() {
            self.append_journal(WireHostJournalEntry {
                sequence: 0,
                session_id: session_id.to_string(),
                zone_id: zone_id.to_string(),
                owner_lease,
                mode: WireZoneOwnerCommandMode::Direct,
                command: None,
                closed: true,
                zone_tick_ms: None,
            })?;
        }
        Ok(ZoneRpcPayload::Unit)
    }

    fn hosted_session(
        &self,
        session_id: &str,
        zone_id: &str,
    ) -> Result<Arc<ZoneHostSession>, ZoneRpcFault> {
        let key = (session_id.to_string(), zone_id.to_string());
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))?;
        if let Some(hosted) = sessions.get(&key) {
            return Ok(Arc::clone(hosted));
        }
        if self.is_draining() {
            return Err(ZoneRpcFault::new(
                "host_draining",
                format!("zone host {} is draining", self.host_id),
            ));
        }
        let zone_already_present = sessions.keys().any(|(_, existing)| existing == zone_id);
        let current_zone_count = sessions
            .keys()
            .map(|(_, existing)| existing)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        if !zone_already_present && current_zone_count >= self.zone_capacity {
            return Err(ZoneRpcFault::new(
                "capacity",
                format!("zone host Zone capacity {} reached", self.zone_capacity),
            ));
        }
        if sessions.len() >= self.limits.max_sessions {
            return Err(ZoneRpcFault::new(
                "capacity",
                format!(
                    "zone host session capacity {} reached",
                    self.limits.max_sessions
                ),
            ));
        }
        let zone_session_count = sessions
            .keys()
            .filter(|(_, existing)| existing == zone_id)
            .count();
        if zone_session_count >= self.limits.max_sessions_per_zone {
            return Err(ZoneRpcFault::new(
                "capacity",
                format!(
                    "Zone {zone_id} session capacity {} reached",
                    self.limits.max_sessions_per_zone
                ),
            ));
        }
        let factory = self
            .runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
            .clone();
        let session = self.create_hosted_session(&factory, zone_id);
        sessions.insert(key, Arc::clone(&session));
        Ok(session)
    }

    fn create_hosted_session(
        &self,
        factory: &Arc<SharedInProcessZoneRuntimeFactory>,
        zone_id: &str,
    ) -> Arc<ZoneHostSession> {
        let zone_id = ZoneId::new(zone_id);
        let config = self
            .config
            .lock()
            .expect("Zone Host config mutex should not be poisoned")
            .clone();
        let runtime = factory.create_runtime(config, &zone_id);
        let hosted = Arc::new(HostedZoneOwnerCommandClient::with_owner_lease_authority(
            runtime,
            Arc::clone(&self.owner_lease_authority),
        ));
        let session = Arc::new(ZoneHostSession::new(
            hosted,
            self.limits.max_outbound_messages,
        ));
        session
    }

    fn create_replay_session(
        &self,
        factory: &Arc<SharedInProcessZoneRuntimeFactory>,
        config: &GatewayConfig,
        zone_id: &str,
    ) -> Arc<ZoneHostSession> {
        let zone_id = ZoneId::new(zone_id);
        let runtime = factory.create_runtime(config.clone(), &zone_id);
        // Journal entries were accepted under historical fencing tokens. Replay
        // verifies their checkpoint checksum and sequence, but must not compare
        // those historical leases with today's finalized owner.
        let hosted = Arc::new(HostedZoneOwnerCommandClient::new(runtime));
        Arc::new(ZoneHostSession::new(
            hosted,
            self.limits.max_outbound_messages,
        ))
    }

    fn append_journal(&self, entry: WireHostJournalEntry) -> Result<(), ZoneRpcFault> {
        append_host_journal(self.journal.as_ref(), entry)
    }

    fn export_host_checkpoint(&self) -> Result<ZoneRpcPayload, ZoneRpcFault> {
        let started = Instant::now();
        let journal = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?;
        if journal.replication.contains_compacted_history() {
            return Err(ZoneRpcFault::new(
                "checkpoint_history_compacted",
                "v4 host checkpoint export is unavailable after a v5 base snapshot installation",
            ));
        }
        let entries = journal.entries.clone();
        drop(journal);
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))?;
        let mut commitments = Vec::with_capacity(sessions.len());
        for ((session_id, zone_id), session) in sessions.iter() {
            let snapshot = session
                .hosted
                .world_snapshot()
                .map_err(classify_runtime_error)?;
            let durable_snapshot = durable_session_snapshot(snapshot);
            let active_character_bytes = session
                .hosted
                .active_character_checkpoint()
                .map_err(classify_runtime_error)?
                .map(|checkpoint| serde_json::to_vec(&checkpoint))
                .transpose()
                .map_err(|error| {
                    ZoneRpcFault::new(
                        "checkpoint_encode",
                        format!("active character checkpoint encode failed: {error}"),
                    )
                })?;
            commitments.push(WireSessionCommitment {
                session_id: session_id.clone(),
                zone_id: zone_id.clone(),
                snapshot_digest: snapshot_digest(&durable_snapshot)?,
                durable_snapshot: Box::new(durable_snapshot),
                active_character_bytes,
                active_identity: session
                    .hosted
                    .active_identity()
                    .map_err(classify_runtime_error)?,
            });
        }
        let factory = self
            .runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
            .clone();
        let zone_state_bytes = factory.checkpoint_bytes().map_err(classify_runtime_error)?;
        let checkpoint = WireZoneHostCheckpoint::new(
            entries,
            commitments,
            factory.active_zone_count(),
            zone_state_bytes,
        )?;
        let bytes = serde_json::to_vec(&checkpoint).map_err(|error| {
            ZoneRpcFault::new(
                "checkpoint_encode",
                format!("zone host checkpoint encode failed: {error}"),
            )
        })?;
        if bytes.len() > self.limits.max_frame_bytes {
            return Err(ZoneRpcFault::new(
                "checkpoint_too_large",
                format!(
                    "zone host checkpoint exceeds {} bytes",
                    self.limits.max_frame_bytes
                ),
            ));
        }
        let byte_count = saturating_u64(bytes.len());
        let duration_ns = saturating_duration_ns(started.elapsed());
        self.checkpoint_exports_total
            .fetch_add(1, Ordering::Relaxed);
        self.checkpoint_export_bytes_total
            .fetch_add(byte_count, Ordering::Relaxed);
        self.checkpoint_export_duration_ns_total
            .fetch_add(duration_ns, Ordering::Relaxed);
        self.checkpoint_export_last_bytes
            .store(byte_count, Ordering::Release);
        self.checkpoint_export_last_duration_ns
            .store(duration_ns, Ordering::Release);
        Ok(ZoneRpcPayload::HostCheckpoint { bytes })
    }

    fn export_base_snapshot(&self, zone_id: &ZoneId) -> Result<ZoneRpcPayload, ZoneRpcFault> {
        let head = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
            .replication
            .head(zone_id.as_str());
        let selected_sessions = self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))?
            .iter()
            .filter(|((_, session_zone_id), _)| session_zone_id == zone_id.as_str())
            .map(|(key, session)| (key.clone(), Arc::clone(session)))
            .collect::<BTreeMap<_, _>>();
        let sessions = session_commitments(&selected_sessions)?;
        let factory = self
            .runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
            .clone();
        let zone_state_bytes = factory
            .zone_checkpoint_bytes(zone_id)
            .map_err(classify_runtime_error)?;
        let snapshot = ZoneBaseSnapshot::new(
            zone_id.as_str().to_string(),
            head.build_id,
            head.next_sequence,
            head.latest_digest,
            zone_state_bytes,
            sessions,
        )?;
        snapshot.verify().map_err(|error| {
            ZoneRpcFault::new(
                "base_snapshot_invalid",
                format!("generated Zone base snapshot failed verification: {error}"),
            )
        })?;
        let wire_bytes = serde_json::to_vec(&snapshot).map_err(|error| {
            ZoneRpcFault::new(
                "base_snapshot_encode",
                format!("Zone base snapshot wire encode failed: {error}"),
            )
        })?;
        if wire_bytes.len().saturating_add(1024) > self.limits.max_frame_bytes {
            return Err(ZoneRpcFault::new(
                "base_snapshot_too_large",
                format!(
                    "Zone base snapshot wire payload {} exceeds RPC frame limit {}",
                    wire_bytes.len(),
                    self.limits.max_frame_bytes
                ),
            ));
        }
        Ok(ZoneRpcPayload::BaseSnapshot { snapshot })
    }

    fn install_base_snapshot(
        &self,
        requested_zone_id: &str,
        snapshot: &ZoneBaseSnapshot,
    ) -> Result<(), ZoneRpcFault> {
        snapshot.verify().map_err(|error| {
            ZoneRpcFault::new(
                "base_snapshot_invalid",
                format!("Zone base snapshot verification failed: {error}"),
            )
        })?;
        if snapshot.zone_id != requested_zone_id {
            return Err(ZoneRpcFault::new(
                "zone_mismatch",
                format!(
                    "base snapshot Zone {} does not match requested Zone {requested_zone_id}",
                    snapshot.zone_id
                ),
            ));
        }
        if snapshot.build_id != zone_replication_build_id() {
            return Err(ZoneRpcFault::new(
                "base_snapshot_build_mismatch",
                format!(
                    "base snapshot build {} does not match host build {}",
                    snapshot.build_id,
                    zone_replication_build_id()
                ),
            ));
        }
        if !snapshot.apply_ready {
            return Err(ZoneRpcFault::new(
                "base_snapshot_incomplete",
                "base snapshot does not contain a complete restorable Session image",
            ));
        }
        let payload = snapshot.decode_payload().map_err(|error| {
            ZoneRpcFault::new(
                "base_snapshot_decode",
                format!("Zone base snapshot payload decode failed: {error}"),
            )
        })?;

        // First reconstruct against an isolated account store. This validates
        // every bootstrap, private character image, shared Zone image, and
        // commitment without mutating the live host.
        let validation_config = self
            .config
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host config mutex poisoned"))?
            .fork_for_replica_apply()
            .map_err(classify_runtime_error)?;
        seed_base_snapshot_accounts(&validation_config, &payload.sessions)?;
        let validation_factory = Arc::new(
            self.runtime_factory
                .lock()
                .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
                .fresh_replica(),
        );
        self.reconstruct_base_sessions(&validation_factory, &validation_config, &payload)?;

        // Build the publishable image a second time with an independent,
        // persistence-disabled replica account store. Replayed mutations must
        // not duplicate the active host's file/PostgreSQL writes.
        let live_config = self
            .config
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host config mutex poisoned"))?
            .fork_for_replica_apply()
            .map_err(classify_runtime_error)?;
        seed_base_snapshot_accounts(&live_config, &payload.sessions)?;
        let staged = (|| {
            let live_factory = Arc::new(
                self.runtime_factory
                    .lock()
                    .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
                    .fresh_replica(),
            );
            let live_sessions =
                self.reconstruct_base_sessions(&live_factory, &live_config, &payload)?;
            let mut staged_journal = self
                .journal
                .lock()
                .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
                .clone();
            staged_journal.install_base(snapshot)?;
            Ok::<_, ZoneRpcFault>((live_factory, live_sessions, staged_journal))
        })();
        let (live_factory, live_sessions, staged_journal) = match staged {
            Ok(staged) => staged,
            Err(error) => return Err(error),
        };

        let zone_id = ZoneId::new(&snapshot.zone_id);
        let target_factory = self
            .runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?;
        target_factory.mark_zone_as_replica(&zone_id);
        let published = match target_factory
            .adopt_zone_resources_from(&live_factory, &zone_id)
            .map_err(classify_runtime_error)
        {
            Ok(published) => published,
            Err(error) => return Err(error),
        };
        drop(target_factory);
        if !published && !payload.sessions.is_empty() {
            return Err(ZoneRpcFault::new(
                "base_snapshot_zone_state",
                format!(
                    "base snapshot reconstructed Sessions for Zone {} without shared Zone resources",
                    snapshot.zone_id
                ),
            ));
        }

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))?;
        sessions.retain(|(_, zone_id), _| zone_id != &snapshot.zone_id);
        sessions.extend(live_sessions);
        drop(sessions);
        *self
            .config
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host config mutex poisoned"))? =
            live_config;
        *self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))? =
            staged_journal;
        Ok(())
    }

    fn reconstruct_base_sessions(
        &self,
        factory: &Arc<SharedInProcessZoneRuntimeFactory>,
        config: &GatewayConfig,
        payload: &WireZoneBaseSnapshotPayload,
    ) -> Result<BTreeMap<(String, String), Arc<ZoneHostSession>>, ZoneRpcFault> {
        let mut sessions = BTreeMap::new();
        for commitment in &payload.sessions {
            let key = (commitment.session_id.clone(), commitment.zone_id.clone());
            let session = self.create_replay_session(factory, config, commitment.zone_id.as_str());
            if let (Some(identity), Some(active_character_bytes)) = (
                commitment.active_identity.as_ref(),
                commitment.active_character_bytes.as_ref(),
            ) {
                let zone_id = ZoneId::new(&commitment.zone_id);
                let lease = ZoneOwnerLease::in_process(&zone_id);
                session.execute_request(ZoneOwnerCommandRequest::direct(
                    lease.clone(),
                    WorldCommand::PasskeyLogin {
                        account_id: identity.account_id.clone(),
                    },
                ))?;
                session.execute_request(ZoneOwnerCommandRequest::direct(
                    lease,
                    WorldCommand::ClientPacket(ClientPacket::StartGame {
                        character_index: identity.character_index,
                    }),
                ))?;
                let actual_identity = session
                    .hosted
                    .active_identity()
                    .map_err(classify_runtime_error)?;
                if actual_identity.as_ref() != Some(identity) {
                    return Err(ZoneRpcFault::new(
                        "base_snapshot_identity",
                        format!(
                            "base snapshot bootstrap identity mismatch for session {}: expected={identity:?}, actual={actual_identity:?}",
                            commitment.session_id
                        ),
                    ));
                }
                let active_character: CharacterSaveRecord =
                    serde_json::from_slice(active_character_bytes).map_err(|error| {
                        ZoneRpcFault::new(
                            "base_snapshot_decode",
                            format!(
                                "session {} active character decode failed: {error}",
                                commitment.session_id
                            ),
                        )
                    })?;
                session
                    .hosted
                    .restore_active_character_checkpoint(&active_character)
                    .map_err(classify_runtime_error)?;
            }
            sessions.insert(key, session);
        }
        factory
            .install_checkpoint_bytes(&payload.zone_state_bytes)
            .map_err(classify_runtime_error)?;
        for session in sessions.values() {
            session
                .hosted
                .refresh_replica_zone_binding()
                .map_err(classify_runtime_error)?;
        }
        let actual = session_commitments(&sessions)?;
        if actual != payload.sessions {
            return Err(ZoneRpcFault::new(
                "base_snapshot_commitment",
                format!(
                    "Zone base snapshot Session image mismatch: {}",
                    commitment_mismatch_details(&payload.sessions, &actual)
                ),
            ));
        }

        let mut live_sessions = BTreeMap::new();
        for (key, replay_session) in sessions {
            let runtime = replay_session
                .hosted
                .take_runtime_for_handoff()
                .map_err(classify_runtime_error)?;
            let hosted = Arc::new(
                HostedZoneOwnerCommandClient::from_handoff_with_owner_lease_authority(
                    runtime,
                    Arc::clone(&self.owner_lease_authority),
                ),
            );
            live_sessions.insert(
                key,
                Arc::new(ZoneHostSession::new(
                    hosted,
                    self.limits.max_outbound_messages,
                )),
            );
        }
        Ok(live_sessions)
    }

    fn apply_mutation_batch(
        &self,
        requested_zone_id: &str,
        batch: &ZoneMutationBatch,
    ) -> Result<(), ZoneRpcFault> {
        batch.verify().map_err(|error| {
            ZoneRpcFault::new(
                "mutation_batch_invalid",
                format!("Zone mutation batch verification failed: {error}"),
            )
        })?;
        if batch.zone_id != requested_zone_id {
            return Err(ZoneRpcFault::new(
                "zone_mismatch",
                format!(
                    "mutation batch Zone {} does not match requested Zone {requested_zone_id}",
                    batch.zone_id
                ),
            ));
        }
        if batch.build_id != zone_replication_build_id() {
            return Err(ZoneRpcFault::new(
                "mutation_build_mismatch",
                format!(
                    "mutation batch build {} does not match host build {}",
                    batch.build_id,
                    zone_replication_build_id()
                ),
            ));
        }
        let current = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
            .replication
            .head(requested_zone_id);
        if batch.first_sequence != current.next_sequence
            || batch.previous_digest != current.latest_digest
        {
            return Err(ZoneRpcFault::new(
                "mutation_cursor_mismatch",
                format!(
                    "mutation batch starts at {}/{}, standby Head is {}/{}",
                    batch.first_sequence,
                    batch.previous_digest,
                    current.next_sequence,
                    current.latest_digest
                ),
            ));
        }

        for mutation in &batch.entries {
            let entry: WireHostJournalEntry =
                serde_json::from_slice(&mutation.payload).map_err(|error| {
                    ZoneRpcFault::new(
                        "mutation_decode",
                        format!(
                            "Zone mutation {} payload decode failed: {error}",
                            mutation.sequence
                        ),
                    )
                })?;
            let key = (entry.session_id.clone(), entry.zone_id.clone());
            if let Some(now_ms) = entry.zone_tick_ms {
                if entry.command.is_some() || entry.closed {
                    return Err(ZoneRpcFault::new(
                        "mutation_decode",
                        "Zone cadence tick cannot contain a Session command or close marker",
                    ));
                }
                self.runtime_factory
                    .lock()
                    .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
                    .apply_replicated_zone_tick(&ZoneId::new(&entry.zone_id), now_ms)
                    .map_err(classify_runtime_error)?;
            } else if entry.closed {
                self.sessions
                    .lock()
                    .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))?
                    .remove(&key);
            } else {
                let session = {
                    let mut sessions = self.sessions.lock().map_err(|_| {
                        ZoneRpcFault::new("internal", "zone host session mutex poisoned")
                    })?;
                    if let Some(session) = sessions.get(&key) {
                        Arc::clone(session)
                    } else {
                        let factory = self
                            .runtime_factory
                            .lock()
                            .map_err(|_| {
                                ZoneRpcFault::new("internal", "zone host factory mutex poisoned")
                            })?
                            .clone();
                        let session = self.create_hosted_session(&factory, &entry.zone_id);
                        sessions.insert(key, Arc::clone(&session));
                        session
                    }
                };
                session.execute_replay_request(
                    entry
                        .clone()
                        .into_request()?
                        .with_source_sequence(mutation.sequence),
                )?;
            }
            self.append_journal(entry)?;
            let applied = self
                .journal
                .lock()
                .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
                .replication
                .head(requested_zone_id);
            if applied.latest_digest != mutation.digest
                || applied.next_sequence != mutation.sequence.saturating_add(1)
            {
                return Err(ZoneRpcFault::new(
                    "mutation_apply_diverged",
                    format!(
                        "applied mutation {} produced Head {}/{}, expected {}/{}",
                        mutation.sequence,
                        applied.next_sequence,
                        applied.latest_digest,
                        mutation.sequence.saturating_add(1),
                        mutation.digest
                    ),
                ));
            }
        }
        let applied = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
            .replication
            .head(requested_zone_id);
        if applied.next_sequence != batch.next_sequence
            || applied.latest_digest != batch.latest_digest
        {
            return Err(ZoneRpcFault::new(
                "mutation_apply_diverged",
                format!(
                    "applied batch produced Head {}/{}, expected {}/{}",
                    applied.next_sequence,
                    applied.latest_digest,
                    batch.next_sequence,
                    batch.latest_digest
                ),
            ));
        }
        Ok(())
    }

    fn install_host_checkpoint(&self, bytes: &[u8]) -> Result<(), ZoneRpcFault> {
        let started = Instant::now();
        let checkpoint: WireZoneHostCheckpoint =
            serde_json::from_slice(bytes).map_err(|error| {
                ZoneRpcFault::new(
                    "checkpoint_decode",
                    format!("zone host checkpoint decode failed: {error}"),
                )
            })?;
        checkpoint.verify().map_err(|error| {
            ZoneRpcFault::new(
                "checkpoint_invalid",
                format!("zone host checkpoint verification failed: {error}"),
            )
        })?;

        let replay_entries = saturating_u64(checkpoint.entries.len());
        let replication_catalog = ZoneReplicationCatalog::from_entries(&checkpoint.entries)?;
        let factory = Arc::new(
            self.runtime_factory
                .lock()
                .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
                .fresh_replica(),
        );
        let replay_config = self
            .replay_baseline_config
            .fork_for_replica_apply()
            .map_err(classify_runtime_error)?;
        let mut sessions = BTreeMap::<(String, String), Arc<ZoneHostSession>>::new();
        for (expected_sequence, entry) in checkpoint.entries.iter().enumerate() {
            if entry.sequence != expected_sequence as u64 {
                return Err(ZoneRpcFault::new(
                    "checkpoint_sequence",
                    format!(
                        "zone host checkpoint expected journal sequence {expected_sequence}, got {}",
                        entry.sequence
                    ),
                ));
            }
            if entry.owner_lease.zone_id != entry.zone_id {
                return Err(ZoneRpcFault::new(
                    "checkpoint_zone_mismatch",
                    "zone host checkpoint lease does not match journal zone",
                ));
            }
            if let Some(now_ms) = entry.zone_tick_ms {
                if entry.command.is_some() || entry.closed {
                    return Err(ZoneRpcFault::new(
                        "checkpoint_command",
                        "Zone cadence tick cannot contain a Session command or close marker",
                    ));
                }
                factory
                    .apply_replicated_zone_tick(&ZoneId::new(&entry.zone_id), now_ms)
                    .map_err(classify_runtime_error)?;
                continue;
            }
            let key = (entry.session_id.clone(), entry.zone_id.clone());
            if entry.closed {
                if entry.command.is_some() {
                    return Err(ZoneRpcFault::new(
                        "checkpoint_command",
                        "journal close entry unexpectedly contains a command",
                    ));
                }
                sessions.remove(&key);
                continue;
            }
            let session = sessions
                .entry(key)
                .or_insert_with(|| {
                    self.create_replay_session(&factory, &replay_config, &entry.zone_id)
                })
                .clone();
            let request = entry.clone().into_request()?;
            session.execute_request(request)?;
        }

        let installed_zone_count = factory
            .install_checkpoint_bytes(&checkpoint.zone_state_bytes)
            .map_err(classify_runtime_error)?;
        if installed_zone_count != checkpoint.zone_count {
            return Err(ZoneRpcFault::new(
                "checkpoint_zone_count",
                format!(
                    "zone host checkpoint restored {installed_zone_count} Zones, expected {}",
                    checkpoint.zone_count
                ),
            ));
        }

        for commitment in &checkpoint.sessions {
            let Some(active_character_bytes) = commitment.active_character_bytes.as_ref() else {
                continue;
            };
            let key = (commitment.session_id.clone(), commitment.zone_id.clone());
            let session = sessions.get(&key).ok_or_else(|| {
                ZoneRpcFault::new(
                    "checkpoint_session",
                    format!(
                        "active character checkpoint has no replayed session {}/{}",
                        commitment.session_id, commitment.zone_id
                    ),
                )
            })?;
            let active_character: CharacterSaveRecord =
                serde_json::from_slice(active_character_bytes).map_err(|error| {
                    ZoneRpcFault::new(
                        "checkpoint_decode",
                        format!("active character checkpoint decode failed: {error}"),
                    )
                })?;
            session
                .hosted
                .restore_active_character_checkpoint(&active_character)
                .map_err(classify_runtime_error)?;
        }

        let actual = session_commitments(&sessions)?;
        if actual != checkpoint.sessions {
            return Err(ZoneRpcFault::new(
                "checkpoint_commitment",
                format!(
                    "zone host checkpoint replay commitment mismatch: {}",
                    commitment_mismatch_details(&checkpoint.sessions, &actual)
                ),
            ));
        }

        let mut live_sessions = BTreeMap::new();
        for (key, replay_session) in sessions {
            let runtime = replay_session
                .hosted
                .take_runtime_for_handoff()
                .map_err(classify_runtime_error)?;
            let hosted = Arc::new(
                HostedZoneOwnerCommandClient::from_handoff_with_owner_lease_authority(
                    runtime,
                    Arc::clone(&self.owner_lease_authority),
                ),
            );
            live_sessions.insert(
                key,
                Arc::new(ZoneHostSession::new(
                    hosted,
                    self.limits.max_outbound_messages,
                )),
            );
        }

        *self
            .runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))? =
            factory;
        *self
            .config
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host config mutex poisoned"))? =
            replay_config;
        *self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))? =
            live_sessions;
        *self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))? =
            ZoneHostJournal {
                entries: checkpoint.entries,
                replication: replication_catalog,
            };
        let byte_count = saturating_u64(bytes.len());
        let duration_ns = saturating_duration_ns(started.elapsed());
        self.checkpoint_installs_total
            .fetch_add(1, Ordering::Relaxed);
        self.checkpoint_install_bytes_total
            .fetch_add(byte_count, Ordering::Relaxed);
        self.checkpoint_install_duration_ns_total
            .fetch_add(duration_ns, Ordering::Relaxed);
        self.checkpoint_install_last_bytes
            .store(byte_count, Ordering::Release);
        self.checkpoint_install_last_duration_ns
            .store(duration_ns, Ordering::Release);
        self.checkpoint_replay_entries_total
            .fetch_add(replay_entries, Ordering::Relaxed);
        self.checkpoint_replay_last_entries
            .store(replay_entries, Ordering::Release);
        Ok(())
    }
}

fn append_host_journal(
    journal: &Mutex<ZoneHostJournal>,
    mut entry: WireHostJournalEntry,
) -> Result<(), ZoneRpcFault> {
    let mut journal = journal
        .lock()
        .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?;
    entry.sequence = journal
        .entries
        .last()
        .map(|entry| entry.sequence.saturating_add(1))
        .unwrap_or(0);
    let host_entry_index = journal.entries.len();
    journal.replication.append(host_entry_index, &entry)?;
    journal.entries.push(entry);
    Ok(())
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn saturating_duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn zone_replication_build_id() -> String {
    if let Ok(build_id) = std::env::var("MIR2_ZONE_HOST_BUILD_ID") {
        let build_id = build_id.trim();
        if !build_id.is_empty() && build_id.len() <= 128 && !build_id.chars().any(char::is_control)
        {
            return build_id.to_string();
        }
    }
    option_env!("GIT_COMMIT_SHA")
        .filter(|build_id| !build_id.trim().is_empty() && build_id.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| format!("mir2-gateway/{}", env!("CARGO_PKG_VERSION")))
}

fn zone_replication_entry_digest(
    previous_digest: &[u8; 32],
    zone_sequence: u64,
    entry: &WireHostJournalEntry,
) -> Result<[u8; 32], ZoneRpcFault> {
    let mut canonical_entry = entry.clone();
    canonical_entry.sequence = zone_sequence;
    let bytes = serde_json::to_vec(&canonical_entry).map_err(|error| {
        ZoneRpcFault::new(
            "replication_encode",
            format!("zone replication entry encode failed: {error}"),
        )
    })?;
    let mut hasher = Sha256::new();
    hasher.update(ZONE_REPLICATION_HEAD_DOMAIN);
    hasher.update(entry.zone_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(previous_digest);
    hasher.update(zone_sequence.to_be_bytes());
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

struct ActiveConnectionGuard<'a>(&'a AtomicUsize);

impl Drop for ActiveConnectionGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZoneRpcEnvelope {
    protocol_version: u16,
    session_id: String,
    zone_id: String,
    auth_token: Option<String>,
    request: ZoneRpcRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", content = "arguments", rename_all = "camelCase")]
enum ZoneRpcRequest {
    Health,
    ReplicationHead,
    ExportMutationBatch {
        first_sequence: u64,
        max_entries: usize,
        max_payload_bytes: usize,
    },
    ExportBaseSnapshot,
    InstallBaseSnapshot {
        snapshot: ZoneBaseSnapshot,
    },
    ApplyMutationBatch {
        batch: ZoneMutationBatch,
    },
    AssessPromotionReadiness {
        active_head: ZoneReplicationHead,
        source_observed_at_ms: u64,
        max_lag_ms: u64,
    },
    PromoteReplica {
        readiness_id: String,
        owner_lease: WireZoneOwnerLease,
    },
    QuiesceForPromotion {
        owner_lease: WireZoneOwnerLease,
    },
    ResumeAfterQuiesce {
        owner_lease: WireZoneOwnerLease,
    },
    OnConnect,
    Execute {
        owner_lease: WireZoneOwnerLease,
        mode: WireZoneOwnerCommandMode,
        command: WireWorldCommand,
    },
    PollOutbounds {
        stream_id: Option<String>,
        acknowledged_sequence: u64,
        max_items: usize,
    },
    WorldSnapshot,
    ActiveIdentity,
    SaveActiveCharacter,
    RefreshActiveExternalMail,
    CloseSession {
        owner_lease: WireZoneOwnerLease,
    },
    ExportHostCheckpoint,
    InstallHostCheckpoint {
        bytes: Vec<u8>,
    },
}

impl ZoneRpcRequest {
    fn priority(&self) -> ZoneRpcPriority {
        match self {
            Self::Health
            | Self::ReplicationHead
            | Self::ExportMutationBatch { .. }
            | Self::ExportBaseSnapshot
            | Self::InstallBaseSnapshot { .. }
            | Self::ApplyMutationBatch { .. }
            | Self::AssessPromotionReadiness { .. }
            | Self::PromoteReplica { .. }
            | Self::QuiesceForPromotion { .. }
            | Self::ResumeAfterQuiesce { .. }
            | Self::ExportHostCheckpoint
            | Self::InstallHostCheckpoint { .. } => ZoneRpcPriority::Control,
            Self::OnConnect
            | Self::Execute { .. }
            | Self::PollOutbounds { .. }
            | Self::WorldSnapshot
            | Self::ActiveIdentity
            | Self::SaveActiveCharacter
            | Self::RefreshActiveExternalMail
            | Self::CloseSession { .. } => ZoneRpcPriority::Gameplay,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum ZoneRpcResponse {
    Ok { payload: Box<ZoneRpcPayload> },
    Error { code: String, message: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ZoneRpcPayload {
    Health {
        host_id: String,
        process_id: u32,
        session_count: usize,
        active_connections: usize,
        session_capacity: usize,
        session_capacity_per_zone: usize,
        busiest_zone_session_count: usize,
        zone_count: usize,
        zone_capacity: usize,
        draining: bool,
        protocol_version: u16,
    },
    ReplicationHead {
        head: ZoneReplicationHead,
    },
    MutationBatch {
        batch: ZoneMutationBatch,
    },
    BaseSnapshot {
        snapshot: ZoneBaseSnapshot,
    },
    PromotionReadiness {
        readiness: ZonePromotionReadiness,
    },
    PromotionReceipt {
        receipt: ZonePromotionReceipt,
    },
    QuiesceReceipt {
        receipt: ZoneQuiesceReceipt,
    },
    Packets {
        frames: Vec<Vec<u8>>,
    },
    Execution {
        frames: Vec<Vec<u8>>,
        packet_count: usize,
        snapshot_tick: u64,
        active_identity: Option<ActiveSessionIdentity>,
    },
    Outbounds {
        items: Vec<WireSequencedServerFrame>,
        stream_id: String,
        reset: bool,
        last_issued_sequence: u64,
        has_more: bool,
    },
    WorldSnapshot {
        snapshot: Box<WorldSnapshot>,
    },
    ActiveIdentity {
        identity: Option<ActiveSessionIdentity>,
    },
    Unit,
    Bool {
        value: bool,
    },
    HostCheckpoint {
        bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireSequencedServerFrame {
    sequence: u64,
    frame: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireZoneOwnerLease {
    zone_id: String,
    owner_id: String,
    fencing_token: u64,
}

impl From<&ZoneOwnerLease> for WireZoneOwnerLease {
    fn from(lease: &ZoneOwnerLease) -> Self {
        Self {
            zone_id: lease.zone_id().as_str().to_string(),
            owner_id: lease.owner_id().to_string(),
            fencing_token: lease.fencing_token(),
        }
    }
}

impl WireZoneOwnerLease {
    fn into_lease(self) -> Result<ZoneOwnerLease, ZoneRpcFault> {
        validate_identifier("lease zone id", &self.zone_id)
            .map_err(|message| ZoneRpcFault::new("invalid_request", message))?;
        validate_identifier("lease owner id", &self.owner_id)
            .map_err(|message| ZoneRpcFault::new("invalid_request", message))?;
        if self.fencing_token == 0 {
            return Err(ZoneRpcFault::new(
                "invalid_request",
                "lease fencing token must be positive",
            ));
        }
        Ok(ZoneOwnerLease::new(
            ZoneId::new(self.zone_id),
            self.owner_id,
            self.fencing_token,
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
enum WireZoneOwnerCommandMode {
    Direct,
    ProductionPlayer { authenticated: bool },
}

impl From<ZoneOwnerCommandMode> for WireZoneOwnerCommandMode {
    fn from(mode: ZoneOwnerCommandMode) -> Self {
        match mode {
            ZoneOwnerCommandMode::Direct => Self::Direct,
            ZoneOwnerCommandMode::ProductionPlayer { authenticated } => {
                Self::ProductionPlayer { authenticated }
            }
        }
    }
}

impl WireZoneOwnerCommandMode {
    fn into_mode(self) -> ZoneOwnerCommandMode {
        match self {
            Self::Direct => ZoneOwnerCommandMode::Direct,
            Self::ProductionPlayer { authenticated } => {
                ZoneOwnerCommandMode::ProductionPlayer { authenticated }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", content = "arguments", rename_all = "camelCase")]
enum WireWorldCommand {
    ClientPacket {
        frame: Vec<u8>,
    },
    PasskeyLogin {
        account_id: String,
    },
    MoveTo {
        position: Point,
        running: bool,
    },
    Attack {
        object_id: u32,
    },
    Interact {
        object_id: u32,
    },
    SelectNpcDialog {
        target: String,
    },
    SubmitNpcInput {
        value: String,
    },
    PickUp {
        object_id: u32,
    },
    UseItem {
        key: String,
    },
    DropItem {
        key: String,
    },
    DeleteCharacter {
        character_index: i32,
    },
    CastSkill {
        key: String,
    },
    TransferMap {
        key: String,
    },
    ApplyHandoffTransform {
        position: Point,
        direction: mir2_protocol::MirDirection,
        #[serde(default)]
        hp: Option<i32>,
        #[serde(default)]
        mp: Option<i32>,
    },
    Stage5Command {
        action: String,
        args: Vec<String>,
    },
    GrantOnchainOre {
        account: String,
        ore_kind: String,
        amount: u64,
        mine_id: u64,
        stones_left: u32,
        idempotency_key: String,
    },
    CreditGoldFromOre {
        account: String,
        gold: u32,
        idempotency_key: String,
    },
    ItemRentalRequest {
        partner_name: String,
        renting: bool,
    },
    SetLanguage {
        language: String,
    },
    Tick,
}

impl WireWorldCommand {
    fn from_world(command: WorldCommand) -> Result<Self, String> {
        Ok(match command {
            WorldCommand::ClientPacket(packet) => Self::ClientPacket {
                frame: encode_client_packet(&packet)
                    .map_err(|error| format!("client packet encode failed: {error}"))?,
            },
            WorldCommand::PasskeyLogin { account_id } => Self::PasskeyLogin { account_id },
            WorldCommand::MoveTo { position, running } => Self::MoveTo { position, running },
            WorldCommand::Attack { object_id } => Self::Attack { object_id },
            WorldCommand::Interact { object_id } => Self::Interact { object_id },
            WorldCommand::SelectNpcDialog { target } => Self::SelectNpcDialog { target },
            WorldCommand::SubmitNpcInput { value } => Self::SubmitNpcInput { value },
            WorldCommand::PickUp { object_id } => Self::PickUp { object_id },
            WorldCommand::UseItem { key } => Self::UseItem { key },
            WorldCommand::DropItem { key } => Self::DropItem { key },
            WorldCommand::DeleteCharacter { character_index } => {
                Self::DeleteCharacter { character_index }
            }
            WorldCommand::CastSkill { key } => Self::CastSkill { key },
            WorldCommand::TransferMap { key } => Self::TransferMap { key },
            WorldCommand::ApplyHandoffTransform {
                position,
                direction,
                hp,
                mp,
            } => Self::ApplyHandoffTransform {
                position,
                direction,
                hp,
                mp,
            },
            WorldCommand::Stage5Command { action, args } => Self::Stage5Command { action, args },
            WorldCommand::GrantOnchainOre {
                account,
                ore_kind,
                amount,
                mine_id,
                stones_left,
                idempotency_key,
            } => Self::GrantOnchainOre {
                account,
                ore_kind,
                amount,
                mine_id,
                stones_left,
                idempotency_key,
            },
            WorldCommand::CreditGoldFromOre {
                account,
                gold,
                idempotency_key,
            } => Self::CreditGoldFromOre {
                account,
                gold,
                idempotency_key,
            },
            WorldCommand::ItemRentalRequest {
                partner_name,
                renting,
            } => Self::ItemRentalRequest {
                partner_name,
                renting,
            },
            WorldCommand::SetLanguage { language } => Self::SetLanguage { language },
            WorldCommand::Tick => Self::Tick,
        })
    }

    fn into_world(self) -> Result<WorldCommand, ZoneRpcFault> {
        Ok(match self {
            Self::ClientPacket { frame } => {
                WorldCommand::ClientPacket(decode_client_packet(&frame).map_err(|error| {
                    ZoneRpcFault::new(
                        "invalid_command",
                        format!("client packet decode failed: {error}"),
                    )
                })?)
            }
            Self::PasskeyLogin { account_id } => WorldCommand::PasskeyLogin { account_id },
            Self::MoveTo { position, running } => WorldCommand::MoveTo { position, running },
            Self::Attack { object_id } => WorldCommand::Attack { object_id },
            Self::Interact { object_id } => WorldCommand::Interact { object_id },
            Self::SelectNpcDialog { target } => WorldCommand::SelectNpcDialog { target },
            Self::SubmitNpcInput { value } => WorldCommand::SubmitNpcInput { value },
            Self::PickUp { object_id } => WorldCommand::PickUp { object_id },
            Self::UseItem { key } => WorldCommand::UseItem { key },
            Self::DropItem { key } => WorldCommand::DropItem { key },
            Self::DeleteCharacter { character_index } => {
                WorldCommand::DeleteCharacter { character_index }
            }
            Self::CastSkill { key } => WorldCommand::CastSkill { key },
            Self::TransferMap { key } => WorldCommand::TransferMap { key },
            Self::ApplyHandoffTransform {
                position,
                direction,
                hp,
                mp,
            } => WorldCommand::ApplyHandoffTransform {
                position,
                direction,
                hp,
                mp,
            },
            Self::Stage5Command { action, args } => WorldCommand::Stage5Command { action, args },
            Self::GrantOnchainOre {
                account,
                ore_kind,
                amount,
                mine_id,
                stones_left,
                idempotency_key,
            } => WorldCommand::GrantOnchainOre {
                account,
                ore_kind,
                amount,
                mine_id,
                stones_left,
                idempotency_key,
            },
            Self::CreditGoldFromOre {
                account,
                gold,
                idempotency_key,
            } => WorldCommand::CreditGoldFromOre {
                account,
                gold,
                idempotency_key,
            },
            Self::ItemRentalRequest {
                partner_name,
                renting,
            } => WorldCommand::ItemRentalRequest {
                partner_name,
                renting,
            },
            Self::SetLanguage { language } => WorldCommand::SetLanguage { language },
            Self::Tick => WorldCommand::Tick,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireHostJournalEntry {
    sequence: u64,
    session_id: String,
    zone_id: String,
    owner_lease: WireZoneOwnerLease,
    mode: WireZoneOwnerCommandMode,
    command: Option<WireWorldCommand>,
    #[serde(default)]
    closed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zone_tick_ms: Option<u64>,
}

impl WireHostJournalEntry {
    fn into_request(self) -> Result<ZoneOwnerCommandRequest, ZoneRpcFault> {
        if self.zone_tick_ms.is_some() {
            return Err(ZoneRpcFault::new(
                "checkpoint_command",
                "Zone cadence tick is not a Session command",
            ));
        }
        let source_sequence = self.sequence;
        let lease = self.owner_lease.into_lease()?;
        let command = self.command.ok_or_else(|| {
            ZoneRpcFault::new("checkpoint_command", "journal execute entry has no command")
        })?;
        let command = command.into_world()?;
        Ok(match self.mode.into_mode() {
            ZoneOwnerCommandMode::Direct => ZoneOwnerCommandRequest::direct(lease, command),
            ZoneOwnerCommandMode::ProductionPlayer { authenticated } => {
                ZoneOwnerCommandRequest::production_player(lease, authenticated, command)
            }
        }
        .with_source_sequence(source_sequence))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireSessionCommitment {
    session_id: String,
    zone_id: String,
    snapshot_digest: String,
    durable_snapshot: Box<WorldSnapshot>,
    active_character_bytes: Option<Vec<u8>>,
    active_identity: Option<ActiveSessionIdentity>,
}

fn session_image_is_complete(session: &WireSessionCommitment) -> bool {
    matches!(
        (&session.active_identity, &session.active_character_bytes),
        (None, None) | (Some(_), Some(_))
    )
}

fn validate_session_image(session: &WireSessionCommitment) -> Result<(), String> {
    validate_identifier("base snapshot session id", &session.session_id)?;
    validate_identifier("base snapshot session Zone id", &session.zone_id)?;
    parse_hex_digest(&session.snapshot_digest)?;
    let actual_digest =
        snapshot_digest(session.durable_snapshot.as_ref()).map_err(|error| error.message)?;
    if !constant_time_bytes_equal(actual_digest.as_bytes(), session.snapshot_digest.as_bytes()) {
        return Err(format!(
            "base snapshot session {} durable snapshot digest mismatch",
            session.session_id
        ));
    }
    let (Some(identity), Some(bytes)) = (&session.active_identity, &session.active_character_bytes)
    else {
        return Ok(());
    };
    validate_identifier("base snapshot account id", &identity.account_id)?;
    validate_identifier(
        "base snapshot active character name",
        &identity.character_name,
    )?;
    let save: CharacterSaveRecord = serde_json::from_slice(bytes).map_err(|error| {
        format!(
            "base snapshot session {} active character decode failed: {error}",
            session.session_id
        )
    })?;
    if identity.character_index != save.character.index
        || identity.character_name != save.character.name
    {
        return Err(format!(
            "base snapshot session {} active character identity mismatch: identity={}/{}, save={}/{}",
            session.session_id,
            identity.character_index,
            identity.character_name,
            save.character.index,
            save.character.name
        ));
    }
    Ok(())
}

fn seed_base_snapshot_accounts(
    config: &GatewayConfig,
    sessions: &[WireSessionCommitment],
) -> Result<(), ZoneRpcFault> {
    let mut decoded = Vec::new();
    let mut seen = BTreeMap::<(String, i32), Vec<u8>>::new();
    for session in sessions {
        let (Some(identity), Some(bytes)) =
            (&session.active_identity, &session.active_character_bytes)
        else {
            continue;
        };
        if let Some(previous) = seen.insert(
            (identity.account_id.clone(), identity.character_index),
            bytes.clone(),
        ) {
            if previous != *bytes {
                return Err(ZoneRpcFault::new(
                    "base_snapshot_account_conflict",
                    format!(
                        "base snapshot contains conflicting images for account {} character {}",
                        identity.account_id, identity.character_index
                    ),
                ));
            }
        }
        let save: CharacterSaveRecord = serde_json::from_slice(bytes).map_err(|error| {
            ZoneRpcFault::new(
                "base_snapshot_decode",
                format!(
                    "session {} active character decode failed: {error}",
                    session.session_id
                ),
            )
        })?;
        decoded.push((identity.clone(), save));
    }

    let mut store = config
        .account_store
        .lock()
        .map_err(|_| ZoneRpcFault::new("internal", "account store mutex poisoned"))?;
    for (identity, save) in decoded {
        let account = store
            .accounts
            .entry(identity.account_id)
            .or_insert_with(AccountRecord::empty);
        if let Some(character) = account
            .characters
            .iter_mut()
            .find(|character| character.index == save.character.index)
        {
            *character = save.character.clone();
        } else {
            account.characters.push(save.character.clone());
            account.characters.sort_by_key(|character| character.index);
        }
        account.saves.insert(save.character.index, save.clone());
        store.next_character_index = store
            .next_character_index
            .max(save.character.index.saturating_add(1));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireZoneBaseSnapshotPayload {
    version: u32,
    zone_id: String,
    zone_state_bytes: Vec<u8>,
    sessions: Vec<WireSessionCommitment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireZoneHostCheckpoint {
    version: u32,
    entries: Vec<WireHostJournalEntry>,
    sessions: Vec<WireSessionCommitment>,
    zone_count: usize,
    zone_state_bytes: Vec<u8>,
    checksum: String,
}

impl WireZoneHostCheckpoint {
    fn new(
        entries: Vec<WireHostJournalEntry>,
        sessions: Vec<WireSessionCommitment>,
        zone_count: usize,
        zone_state_bytes: Vec<u8>,
    ) -> Result<Self, ZoneRpcFault> {
        let checksum = zone_host_checkpoint_checksum(
            ZONE_HOST_CHECKPOINT_VERSION,
            &entries,
            &sessions,
            zone_count,
            &zone_state_bytes,
        )?;
        Ok(Self {
            version: ZONE_HOST_CHECKPOINT_VERSION,
            entries,
            sessions,
            zone_count,
            zone_state_bytes,
            checksum,
        })
    }

    fn verify(&self) -> Result<(), String> {
        if self.version != ZONE_HOST_CHECKPOINT_VERSION {
            return Err(format!(
                "unsupported version {}, expected {}",
                self.version, ZONE_HOST_CHECKPOINT_VERSION
            ));
        }
        let expected = zone_host_checkpoint_checksum_bytes(
            self.version,
            &self.entries,
            &self.sessions,
            self.zone_count,
            &self.zone_state_bytes,
        )?;
        if !constant_time_bytes_equal(expected.as_bytes(), self.checksum.as_bytes()) {
            return Err("checksum mismatch".to_string());
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ZoneRpcFault {
    code: &'static str,
    message: String,
}

impl ZoneRpcFault {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn into_response(self) -> ZoneRpcResponse {
        ZoneRpcResponse::Error {
            code: self.code.to_string(),
            message: self.message,
        }
    }
}

impl From<String> for ZoneRpcFault {
    fn from(message: String) -> Self {
        Self::new("runtime", message)
    }
}

fn classify_runtime_error(message: String) -> ZoneRpcFault {
    if message.contains("stale zone owner lease") {
        ZoneRpcFault::new("stale_lease", message)
    } else {
        ZoneRpcFault::new("runtime", message)
    }
}

fn encode_server_frames(packets: Vec<ServerPacket>) -> Result<Vec<Vec<u8>>, ZoneRpcFault> {
    packets
        .iter()
        .map(|packet| {
            encode_server_packet(packet).map_err(|error| {
                ZoneRpcFault::new(
                    "packet_encode",
                    format!("server packet encode failed: {error}"),
                )
            })
        })
        .collect()
}

fn decode_server_frames(frames: Vec<Vec<u8>>) -> Result<Vec<ServerPacket>, String> {
    frames
        .iter()
        .map(|frame| {
            decode_server_packet(frame)
                .map_err(|error| format!("server packet decode failed: {error}"))
        })
        .collect()
}

fn connect_with_timeout(address: &str, timeout: Duration) -> Result<TcpStream, String> {
    let addresses = address
        .to_socket_addrs()
        .map_err(|error| format!("zone RPC resolve {address} failed: {error}"))?;
    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(stream) => {
                stream
                    .set_nodelay(true)
                    .map_err(|error| format!("set TCP_NODELAY failed: {error}"))?;
                return Ok(stream);
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(format!(
        "zone RPC connect {address} failed: {}",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "address resolved to no endpoints".to_string())
    ))
}

fn shared_rpc_pool(
    addresses: &[String],
    requested_size: usize,
    io_timeout: Duration,
) -> Arc<SharedZoneRpcConnectionPool> {
    let size = requested_size.clamp(2, 1_024);
    let queue_timeout_ms = positive_u64_env("MIR2_ZONE_RPC_QUEUE_TIMEOUT_MS")
        .unwrap_or(500)
        .clamp(1, 30_000);
    let queue_timeout = Duration::from_millis(queue_timeout_ms).min(io_timeout);
    let key = format!(
        "{}|size={size}|queue_ms={queue_timeout_ms}",
        addresses.join(",")
    );
    let registry = SHARED_RPC_POOLS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut registry = registry
        .lock()
        .expect("shared Zone RPC pool registry mutex poisoned");
    if let Some(pool) = registry.get(&key).and_then(Weak::upgrade) {
        return pool;
    }
    registry.retain(|_, pool| pool.strong_count() > 0);
    let control_slots = (size / 16).clamp(1, size - 1);
    let pool = Arc::new(SharedZoneRpcConnectionPool {
        endpoints: addresses
            .iter()
            .map(|_| SharedEndpointConnections {
                slots: (0..size).map(|_| Mutex::new(None)).collect(),
                next_control: AtomicUsize::new(0),
                next_gameplay: AtomicUsize::new(0),
                control_slots,
            })
            .collect(),
        queue_timeout,
    });
    registry.insert(key, Arc::downgrade(&pool));
    pool
}

impl SharedZoneRpcConnectionPool {
    fn call(
        &self,
        endpoint_index: usize,
        address: &str,
        encoded: &[u8],
        limits: &ZoneRpcLimits,
        codec: ZoneRpcCodec,
        priority: ZoneRpcPriority,
    ) -> Result<ZoneRpcResponse, String> {
        let endpoint = self
            .endpoints
            .get(endpoint_index)
            .ok_or_else(|| format!("shared Zone RPC endpoint index {endpoint_index} is invalid"))?;
        let (start, length, sequence) = match priority {
            ZoneRpcPriority::Control => (
                0,
                endpoint.control_slots,
                endpoint.next_control.fetch_add(1, Ordering::Relaxed),
            ),
            ZoneRpcPriority::Gameplay => (
                endpoint.control_slots,
                endpoint.slots.len() - endpoint.control_slots,
                endpoint.next_gameplay.fetch_add(1, Ordering::Relaxed),
            ),
        };
        let deadline = Instant::now() + self.queue_timeout;
        loop {
            for offset in 0..length {
                let index = start + ((sequence + offset) % length);
                match endpoint.slots[index].try_lock() {
                    Ok(connection) => {
                        return call_locked_reused_endpoint(
                            address, encoded, limits, codec, connection,
                        );
                    }
                    Err(TryLockError::WouldBlock) => {}
                    Err(TryLockError::Poisoned(_)) => {
                        return Err(format!(
                            "zone RPC shared connection {endpoint_index}/{index} mutex poisoned"
                        ));
                    }
                }
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "zone RPC backpressure: {:?} queue exceeded {}ms",
                    priority,
                    self.queue_timeout.as_millis()
                ));
            }
            // A bounded micro-sleep avoids turning queue pressure into a
            // CPU-burning spin loop while remaining far below the gameplay
            // latency budget.
            thread::sleep(Duration::from_micros(50));
        }
    }
}

fn call_endpoint(
    address: &str,
    encoded: &[u8],
    limits: &ZoneRpcLimits,
    codec: ZoneRpcCodec,
) -> Result<ZoneRpcResponse, String> {
    let mut stream = connect_with_timeout(address, limits.io_timeout)?;
    stream
        .set_read_timeout(Some(limits.io_timeout))
        .map_err(|error| format!("set read timeout failed: {error}"))?;
    stream
        .set_write_timeout(Some(limits.io_timeout))
        .map_err(|error| format!("set write timeout failed: {error}"))?;
    write_frame(&mut stream, encoded, limits.max_frame_bytes)
        .map_err(|error| format!("write failed: {error}"))?;
    let response = read_frame(&mut stream, limits.max_frame_bytes)
        .map_err(|error| format!("read failed: {error}"))?;
    decode_rpc_response(&response, codec)
        .map_err(|error| format!("response decode failed: {error}"))
}

fn call_reused_endpoint(
    address: &str,
    encoded: &[u8],
    limits: &ZoneRpcLimits,
    connection: &Mutex<Option<TcpStream>>,
    codec: ZoneRpcCodec,
) -> Result<ZoneRpcResponse, String> {
    let connection = connection
        .lock()
        .map_err(|_| "zone RPC connection mutex poisoned".to_string())?;
    call_locked_reused_endpoint(address, encoded, limits, codec, connection)
}

fn call_locked_reused_endpoint(
    address: &str,
    encoded: &[u8],
    limits: &ZoneRpcLimits,
    codec: ZoneRpcCodec,
    mut connection: MutexGuard<'_, Option<TcpStream>>,
) -> Result<ZoneRpcResponse, String> {
    if connection.is_none() {
        let stream = connect_with_timeout(address, limits.io_timeout)?;
        stream
            .set_read_timeout(Some(limits.io_timeout))
            .map_err(|error| format!("set read timeout failed: {error}"))?;
        stream
            .set_write_timeout(Some(limits.io_timeout))
            .map_err(|error| format!("set write timeout failed: {error}"))?;
        *connection = Some(stream);
    }
    let result = (|| {
        let stream = connection
            .as_mut()
            .ok_or_else(|| "zone RPC reusable connection missing".to_string())?;
        write_frame(stream, encoded, limits.max_frame_bytes)
            .map_err(|error| format!("write failed: {error}"))?;
        let response = read_frame(stream, limits.max_frame_bytes)
            .map_err(|error| format!("read failed: {error}"))?;
        decode_rpc_response(&response, codec)
            .map_err(|error| format!("response decode failed: {error}"))
    })();
    if result.is_err() {
        *connection = None;
    }
    result
}

fn read_frame_allowing_idle(
    reader: &mut TcpStream,
    max_frame_bytes: usize,
) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0_u8; 4];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            // An idle persistent client is healthy. Only the fixed-size frame
            // header is allowed to time out this way; once a header arrives,
            // the body must complete within the configured I/O deadline.
            return Ok(None);
        }
        Err(error) => return Err(error),
    }
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > max_frame_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid zone RPC frame length {length}"),
        ));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

fn read_frame(reader: &mut impl Read, max_frame_bytes: usize) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    reader.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > max_frame_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid zone RPC frame length {length}"),
        ));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn write_frame(writer: &mut impl Write, bytes: &[u8], max_frame_bytes: usize) -> io::Result<()> {
    if bytes.is_empty() || bytes.len() > max_frame_bytes || bytes.len() > u32::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid zone RPC frame length {}", bytes.len()),
        ));
    }
    writer.write_all(&(bytes.len() as u32).to_be_bytes())?;
    writer.write_all(bytes)?;
    writer.flush()
}

fn detect_rpc_codec(bytes: &[u8]) -> ZoneRpcCodec {
    if bytes.starts_with(ZONE_RPC_BINARY_MAGIC) {
        ZoneRpcCodec::MessagePack
    } else {
        ZoneRpcCodec::Json
    }
}

fn encode_rpc_envelope(envelope: &ZoneRpcEnvelope, codec: ZoneRpcCodec) -> Result<Vec<u8>, String> {
    encode_rpc_value(envelope, codec)
}

fn decode_rpc_envelope(bytes: &[u8], codec: ZoneRpcCodec) -> Result<ZoneRpcEnvelope, String> {
    decode_rpc_value(bytes, codec)
}

fn encode_rpc_response(response: &ZoneRpcResponse, codec: ZoneRpcCodec) -> Result<Vec<u8>, String> {
    encode_rpc_value(response, codec)
}

fn decode_rpc_response(bytes: &[u8], codec: ZoneRpcCodec) -> Result<ZoneRpcResponse, String> {
    decode_rpc_value(bytes, codec)
}

fn encode_rpc_value<T: Serialize>(value: &T, codec: ZoneRpcCodec) -> Result<Vec<u8>, String> {
    match codec {
        ZoneRpcCodec::Json => serde_json::to_vec(value).map_err(|error| error.to_string()),
        ZoneRpcCodec::MessagePack => {
            let payload = rmp_serde::to_vec_named(value).map_err(|error| error.to_string())?;
            let mut bytes = Vec::with_capacity(ZONE_RPC_BINARY_MAGIC.len() + payload.len());
            bytes.extend_from_slice(ZONE_RPC_BINARY_MAGIC);
            bytes.extend_from_slice(&payload);
            Ok(bytes)
        }
    }
}

fn decode_rpc_value<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    codec: ZoneRpcCodec,
) -> Result<T, String> {
    match codec {
        ZoneRpcCodec::Json => serde_json::from_slice(bytes).map_err(|error| error.to_string()),
        ZoneRpcCodec::MessagePack => {
            let payload = bytes
                .strip_prefix(ZONE_RPC_BINARY_MAGIC)
                .ok_or_else(|| "binary RPC frame is missing its magic prefix".to_string())?;
            rmp_serde::from_slice(payload).map_err(|error| error.to_string())
        }
    }
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.len() > 160 {
        return Err(format!("{label} exceeds 160 bytes"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{label} contains control characters"));
    }
    Ok(())
}

fn tokens_equal(expected: Option<&str>, provided: Option<&str>) -> bool {
    match (expected, provided) {
        (None, None) => true,
        (Some(expected), Some(provided)) => {
            let expected = expected.as_bytes();
            let provided = provided.as_bytes();
            let mut difference = expected.len() ^ provided.len();
            for index in 0..expected.len().max(provided.len()) {
                difference |= usize::from(
                    expected.get(index).copied().unwrap_or_default()
                        ^ provided.get(index).copied().unwrap_or_default(),
                );
            }
            difference == 0
        }
        _ => false,
    }
}

fn next_rpc_session_id() -> String {
    let sequence = NEXT_RPC_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("gateway-{}-{now}-{sequence}", std::process::id())
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn next_outbound_stream_id() -> String {
    let sequence = NEXT_OUTBOUND_STREAM_ID.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("outbound-{}-{now}-{sequence}", std::process::id())
}

fn session_commitments(
    sessions: &BTreeMap<(String, String), Arc<ZoneHostSession>>,
) -> Result<Vec<WireSessionCommitment>, ZoneRpcFault> {
    let mut commitments = Vec::with_capacity(sessions.len());
    for ((session_id, zone_id), session) in sessions {
        let snapshot = session
            .hosted
            .world_snapshot()
            .map_err(classify_runtime_error)?;
        let durable_snapshot = durable_session_snapshot(snapshot);
        let active_character_bytes = session
            .hosted
            .active_character_checkpoint()
            .map_err(classify_runtime_error)?
            .map(|checkpoint| serde_json::to_vec(&checkpoint))
            .transpose()
            .map_err(|error| {
                ZoneRpcFault::new(
                    "checkpoint_encode",
                    format!("active character checkpoint encode failed: {error}"),
                )
            })?;
        commitments.push(WireSessionCommitment {
            session_id: session_id.clone(),
            zone_id: zone_id.clone(),
            snapshot_digest: snapshot_digest(&durable_snapshot)?,
            durable_snapshot: Box::new(durable_snapshot),
            active_character_bytes,
            active_identity: session
                .hosted
                .active_identity()
                .map_err(classify_runtime_error)?,
        });
    }
    Ok(commitments)
}

fn snapshot_digest(durable_snapshot: &WorldSnapshot) -> Result<String, ZoneRpcFault> {
    let bytes = serde_json::to_vec(durable_snapshot).map_err(|error| {
        ZoneRpcFault::new(
            "checkpoint_encode",
            format!("zone host snapshot encode failed: {error}"),
        )
    })?;
    let mut hasher = Sha256::new();
    hasher.update(ZONE_HOST_CHECKPOINT_DOMAIN);
    hasher.update(b"snapshot\0");
    hasher.update(bytes);
    Ok(hex_lower_bytes(&hasher.finalize()))
}

fn commitment_mismatch_details(
    expected: &[WireSessionCommitment],
    actual: &[WireSessionCommitment],
) -> String {
    if expected.len() != actual.len() {
        return format!(
            "session count differs: expected={}, actual={}",
            expected.len(),
            actual.len()
        );
    }
    for (expected, actual) in expected.iter().zip(actual) {
        if expected == actual {
            continue;
        }
        let expected_snapshot =
            serde_json::to_value(expected.durable_snapshot.as_ref()).unwrap_or_default();
        let actual_snapshot =
            serde_json::to_value(actual.durable_snapshot.as_ref()).unwrap_or_default();
        let differing_fields = match (expected_snapshot.as_object(), actual_snapshot.as_object()) {
            (Some(expected), Some(actual)) => expected
                .keys()
                .chain(actual.keys())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter(|key| expected.get(*key) != actual.get(*key))
                .map(|key| {
                    format!(
                        "{key}: expected={}, actual={}",
                        expected.get(key).unwrap_or(&serde_json::Value::Null),
                        actual.get(key).unwrap_or(&serde_json::Value::Null)
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
            _ => "durable snapshot JSON shape differs".to_string(),
        };
        return format!(
            "session={}/{}, expected_digest={}, actual_digest={}, identity_expected={:?}, identity_actual={:?}, fields=[{}]",
            expected.session_id,
            expected.zone_id,
            expected.snapshot_digest,
            actual.snapshot_digest,
            expected.active_identity,
            actual.active_identity,
            differing_fields
        );
    }
    "commitment ordering differs".to_string()
}

fn durable_session_snapshot(mut snapshot: WorldSnapshot) -> WorldSnapshot {
    snapshot.tick = 0;
    snapshot.map_title = None;
    snapshot.light_setting = 0;
    snapshot.player_object_id = None;
    // The complete Zone image owns live player vitals. Do not commit them a
    // second time from a session snapshot taken on the other side of an
    // autonomous Zone tick; private persisted vitals are independently
    // committed by `active_character_bytes`.
    snapshot.player_hp = None;
    snapshot.player_max_hp = None;
    snapshot.player_mp = None;
    snapshot.player_max_mp = None;
    snapshot
        .entities
        .retain(|entity| entity.kind == mir2_simulation::WorldEntityKind::SelfPlayer);
    for entity in &mut snapshot.entities {
        entity.object_id = 0;
        // Checkpoint v4 makes the shared Zone state authoritative for the
        // player's transform. The per-session commitment must not duplicate
        // that transform: journal replay can allocate a different shared
        // presence before the exact Zone image is installed.
        entity.x = 0;
        entity.y = 0;
        entity.direction = mir2_protocol::MirDirection::Up;
        entity.hp = None;
        entity.max_hp = None;
        entity.dead = false;
    }
    // Shared map actors, player transforms, and ground drops are committed by
    // the separate Zone state image. Keep this digest scoped to the private
    // durable player/session projection.
    snapshot.ground_drops.clear();
    snapshot
}

fn zone_base_snapshot_checksum(snapshot: &ZoneBaseSnapshot) -> Result<String, String> {
    let payload = serde_json::to_vec(&(
        snapshot.version,
        &snapshot.zone_id,
        &snapshot.build_id,
        snapshot.mutation_coverage,
        snapshot.apply_ready,
        snapshot.base_sequence,
        &snapshot.latest_digest,
        snapshot.created_at_ms,
        snapshot.session_count,
        snapshot.compression,
        snapshot.uncompressed_bytes,
        &snapshot.payload,
    ))
    .map_err(|error| format!("failed to encode base snapshot checksum payload: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(ZONE_BASE_SNAPSHOT_DOMAIN);
    hasher.update(payload);
    Ok(hex_lower_bytes(&hasher.finalize()))
}

fn zone_host_checkpoint_checksum(
    version: u32,
    entries: &[WireHostJournalEntry],
    sessions: &[WireSessionCommitment],
    zone_count: usize,
    zone_state_bytes: &[u8],
) -> Result<String, ZoneRpcFault> {
    zone_host_checkpoint_checksum_bytes(version, entries, sessions, zone_count, zone_state_bytes)
        .map_err(|error| {
            ZoneRpcFault::new(
                "checkpoint_encode",
                format!("zone host checkpoint checksum failed: {error}"),
            )
        })
}

fn zone_host_checkpoint_checksum_bytes(
    version: u32,
    entries: &[WireHostJournalEntry],
    sessions: &[WireSessionCommitment],
    zone_count: usize,
    zone_state_bytes: &[u8],
) -> Result<String, String> {
    let payload =
        serde_json::to_vec(&(version, entries, sessions, zone_count, zone_state_bytes))
            .map_err(|error| format!("failed to encode checkpoint checksum payload: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(ZONE_HOST_CHECKPOINT_DOMAIN);
    hasher.update(payload);
    Ok(hex_lower_bytes(&hasher.finalize()))
}

fn constant_time_bytes_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

fn hex_lower_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn replication_heads_match(left: &ZoneReplicationHead, right: &ZoneReplicationHead) -> bool {
    left.version == right.version
        && left.zone_id == right.zone_id
        && left.build_id == right.build_id
        && left.mutation_coverage == right.mutation_coverage
        && left.base_snapshot_id == right.base_snapshot_id
        && left.base_sequence == right.base_sequence
        && left.next_sequence == right.next_sequence
        && left.latest_digest == right.latest_digest
}

fn zone_promotion_readiness_id(
    zone_id: &ZoneId,
    standby_host_id: &str,
    head: &ZoneReplicationHead,
    assessed_at_ms: u64,
    expires_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"obelisk.mir2.zone-promotion-readiness.v1\0");
    hasher.update(zone_id.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(standby_host_id.as_bytes());
    hasher.update([0]);
    hasher.update(head.build_id.as_bytes());
    hasher.update([0]);
    hasher.update(head.next_sequence.to_le_bytes());
    hasher.update(head.latest_digest.as_bytes());
    hasher.update(assessed_at_ms.to_le_bytes());
    hasher.update(expires_at_ms.to_le_bytes());
    hex_lower_bytes(&hasher.finalize())
}

fn parse_hex_digest(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err(format!(
            "replication digest must contain 64 lowercase hexadecimal characters, got {}",
            value.len()
        ));
    }
    let mut digest = [0u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = lowercase_hex_nibble(pair[0])
            .ok_or_else(|| "replication digest contains a non-lowercase-hex byte".to_string())?;
        let low = lowercase_hex_nibble(pair[1])
            .ok_or_else(|| "replication digest contains a non-lowercase-hex byte".to_string())?;
        digest[index] = (high << 4) | low;
    }
    Ok(digest)
}

fn lowercase_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn configured_zone_host_owner_ids(host_id: &str) -> BTreeSet<String> {
    let mut owner_ids = BTreeSet::from([host_id.to_string()]);
    let configured = std::env::var("MIR2_ZONE_HOST_OWNER_ALIASES")
        .ok()
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        });
    let gate15_legacy = std::env::var("MIR2_GATE15_LOCAL_ZONE_HOST_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .into_iter();
    for owner_id in configured.chain(gate15_legacy) {
        if validate_identifier("Zone Host owner alias", &owner_id).is_ok() {
            owner_ids.insert(owner_id);
        }
    }
    owner_ids
}

fn positive_usize_env(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn positive_u64_env(name: &str) -> Option<u64> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
}

fn enabled_env(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn unknown_zone_telemetry(zone_id: String, session_count: usize) -> ZoneHostZoneTelemetry {
    ZoneHostZoneTelemetry {
        zone_id,
        map_scope: ZoneMapScope::Unknown,
        map_file_names: Vec::new(),
        session_count,
    }
}

fn dynamic_map_file_name(zone_id: &str) -> Option<String> {
    let raw = zone_id
        .strip_prefix("map:")
        .or_else(|| zone_id.strip_prefix("mir2/map/"))?;
    let map_file_name = raw
        .split_once("/shard/")
        .map(|(map, _)| map)
        .or_else(|| raw.split_once(":shard:").map(|(map, _)| map))
        .unwrap_or(raw)
        .trim();
    (!map_file_name.is_empty()).then(|| map_file_name.to_string())
}

fn unexpected_payload(operation: &str, payload: &ZoneRpcPayload) -> String {
    format!("zone RPC {operation} returned unexpected payload {payload:?}")
}

fn zone_rpc_request_requires_active_mutation(request: &ZoneRpcRequest) -> bool {
    matches!(
        request,
        ZoneRpcRequest::OnConnect
            | ZoneRpcRequest::Execute { .. }
            | ZoneRpcRequest::SaveActiveCharacter
            | ZoneRpcRequest::RefreshActiveExternalMail
            | ZoneRpcRequest::CloseSession { .. }
    )
}

pub fn validate_zone_host_bind(address: SocketAddr, auth_token: Option<&str>) -> io::Result<()> {
    if !address.ip().is_loopback() && auth_token.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "MIR2_ZONE_HOST_TOKEN is required for a non-loopback zone host bind",
        ));
    }
    Ok(())
}

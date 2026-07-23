use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_protocol::{
    decode_client_packet, decode_server_packet, encode_client_packet, encode_server_packet, Point,
    ServerPacket,
};
use mir2_simulation::{
    ActiveSessionIdentity, WorldCommand, WorldCommandExecution, WorldCommandOutcome, WorldSnapshot,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::routing::{
    HostedZoneOwnerCommandClient, SharedInProcessZoneRuntimeFactory, SharedZoneLiveOutbound,
    SharedZoneLiveOutboundRegistration, SharedZoneLiveOutboundSender,
    SharedZoneOwnerLeaseAuthority, ZoneId, ZoneLiveOutboundRegistration, ZoneOwnerCommandMode,
    ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerRpcTransport, ZoneRuntimeFactory,
};
use crate::GatewayConfig;
use crate::ZonePlacementLease;

pub const ZONE_RPC_PROTOCOL_VERSION: u16 = 5;
pub const DEFAULT_ZONE_RPC_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_ZONE_RPC_MAX_CONNECTIONS: usize = 64;
pub const DEFAULT_ZONE_RPC_MAX_SESSIONS: usize = 4096;
pub const DEFAULT_ZONE_RPC_MAX_OUTBOUND_MESSAGES: usize = 1024;
pub const DEFAULT_ZONE_RPC_OUTBOUND_POLL_LIMIT: usize = 128;
const ZONE_HOST_CHECKPOINT_VERSION: u32 = 3;
const ZONE_HOST_CHECKPOINT_DOMAIN: &[u8] = b"obelisk.mir2.zone-host-checkpoint.v3\0";

static NEXT_RPC_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_REMOTE_OUTBOUND_REGISTRATION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_OUTBOUND_STREAM_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct ZoneRpcLimits {
    pub max_frame_bytes: usize,
    pub max_connections: usize,
    pub max_sessions: usize,
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
            max_outbound_messages: DEFAULT_ZONE_RPC_MAX_OUTBOUND_MESSAGES,
            outbound_poll_limit: DEFAULT_ZONE_RPC_OUTBOUND_POLL_LIMIT,
            io_timeout: Duration::from_secs(5),
        }
    }
}

impl ZoneRpcLimits {
    pub fn from_env() -> Self {
        let defaults = Self::default();
        Self {
            max_frame_bytes: positive_usize_env("MIR2_ZONE_RPC_MAX_FRAME_BYTES")
                .unwrap_or(defaults.max_frame_bytes),
            max_connections: positive_usize_env("MIR2_ZONE_HOST_MAX_CONNECTIONS")
                .unwrap_or(defaults.max_connections),
            max_sessions: positive_usize_env("MIR2_ZONE_HOST_MAX_SESSIONS")
                .unwrap_or(defaults.max_sessions),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneHostHealth {
    pub host_id: String,
    pub process_id: u32,
    pub session_count: usize,
    pub active_connections: usize,
    pub session_capacity: usize,
    pub zone_count: usize,
    pub zone_capacity: usize,
    pub draining: bool,
    pub protocol_version: u16,
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
        Ok(Self {
            addresses: Arc::new(addresses),
            active_endpoint: Arc::new(AtomicUsize::new(0)),
            zone_id,
            session_id: session_id.into(),
            auth_token,
            limits,
            outbound_acknowledged: Arc::new(AtomicU64::new(0)),
            outbound_generation: Arc::new(AtomicU64::new(0)),
            outbound_stream_id: Arc::new(Mutex::new(None)),
        })
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
        Self::with_endpoints(
            addresses,
            zone_id,
            next_rpc_session_id(),
            std::env::var("MIR2_ZONE_HOST_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            ZoneRpcLimits::from_env(),
        )
        .ok()
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
                zone_count,
                zone_capacity,
                draining,
                protocol_version,
            }),
            payload => Err(unexpected_payload("health", &payload)),
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
        let envelope = ZoneRpcEnvelope {
            protocol_version: ZONE_RPC_PROTOCOL_VERSION,
            session_id: self.session_id.clone(),
            zone_id: self.zone_id.as_str().to_string(),
            auth_token: self.auth_token.clone(),
            request,
        };
        let encoded = serde_json::to_vec(&envelope)
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
            match call_endpoint(address, &encoded, &self.limits) {
                Ok(ZoneRpcResponse::Ok { payload }) => {
                    self.active_endpoint.store(index, Ordering::Release);
                    return Ok(*payload);
                }
                Ok(ZoneRpcResponse::Error { code, message })
                    if code == "host_draining" || code == "capacity" =>
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
    config: GatewayConfig,
    runtime_factory: Mutex<Arc<SharedInProcessZoneRuntimeFactory>>,
    owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    sessions: Mutex<BTreeMap<(String, String), Arc<ZoneHostSession>>>,
    operation_gate: Mutex<()>,
    journal: Mutex<Vec<WireHostJournalEntry>>,
    auth_token: Option<String>,
    limits: ZoneRpcLimits,
    zone_capacity: usize,
    draining: AtomicBool,
    active_connections: AtomicUsize,
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
        Self {
            host_id: host_id.into(),
            config,
            runtime_factory: Mutex::new(runtime_factory),
            owner_lease_authority,
            sessions: Mutex::new(BTreeMap::new()),
            operation_gate: Mutex::new(()),
            journal: Mutex::new(Vec::new()),
            auth_token,
            limits,
            zone_capacity: zone_capacity.max(1),
            draining: AtomicBool::new(false),
            active_connections: AtomicUsize::new(0),
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions
            .lock()
            .map(|sessions| sessions.len())
            .unwrap_or(0)
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
        // flag.  Some platforms propagate that mode to accepted sockets; the
        // framed request handler is deliberately blocking with bounded timeouts.
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(self.limits.io_timeout))?;
        stream.set_write_timeout(Some(self.limits.io_timeout))?;
        let bytes = read_frame(&mut stream, self.limits.max_frame_bytes)?;
        let response = match serde_json::from_slice::<ZoneRpcEnvelope>(&bytes) {
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
        let bytes = serde_json::to_vec(&response).map_err(io::Error::other)?;
        write_frame(&mut stream, &bytes, self.limits.max_frame_bytes)
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
            return Ok(ZoneRpcPayload::Health {
                host_id: self.host_id.clone(),
                process_id: std::process::id(),
                session_count: self.session_count(),
                active_connections: self.active_connections.load(Ordering::Acquire),
                session_capacity: self.limits.max_sessions,
                zone_count: self.zone_count(),
                zone_capacity: self.zone_capacity,
                draining: self.is_draining(),
                protocol_version: ZONE_RPC_PROTOCOL_VERSION,
            });
        }

        let _operation = self
            .operation_gate
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host operation mutex poisoned"))?;
        let request = envelope.request;
        match request {
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
            ZoneRpcRequest::ExportHostCheckpoint | ZoneRpcRequest::InstallHostCheckpoint { .. } => {
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
                let journal_entry = WireHostJournalEntry {
                    sequence: 0,
                    session_id: session_id.to_string(),
                    zone_id: zone_id.to_string(),
                    owner_lease: owner_lease.clone(),
                    mode: mode.clone(),
                    command: Some(command.clone()),
                    closed: false,
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
                };
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
        let runtime = factory.create_runtime(self.config.clone(), &zone_id);
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

    fn append_journal(&self, mut entry: WireHostJournalEntry) -> Result<(), ZoneRpcFault> {
        let mut journal = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?;
        entry.sequence = journal
            .last()
            .map(|entry| entry.sequence.saturating_add(1))
            .unwrap_or(0);
        journal.push(entry);
        Ok(())
    }

    fn export_host_checkpoint(&self) -> Result<ZoneRpcPayload, ZoneRpcFault> {
        let entries = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))?
            .clone();
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
            commitments.push(WireSessionCommitment {
                session_id: session_id.clone(),
                zone_id: zone_id.clone(),
                snapshot_digest: snapshot_digest(&snapshot)?,
                active_identity: session
                    .hosted
                    .active_identity()
                    .map_err(classify_runtime_error)?,
            });
        }
        let checkpoint = WireZoneHostCheckpoint::new(entries, commitments)?;
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
        Ok(ZoneRpcPayload::HostCheckpoint { bytes })
    }

    fn install_host_checkpoint(&self, bytes: &[u8]) -> Result<(), ZoneRpcFault> {
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

        let factory = Arc::new(
            self.runtime_factory
                .lock()
                .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))?
                .fresh(),
        );
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
                .or_insert_with(|| self.create_hosted_session(&factory, &entry.zone_id))
                .clone();
            let request = entry.clone().into_request()?;
            session.execute_request(request)?;
        }

        let actual = session_commitments(&sessions)?;
        if actual != checkpoint.sessions {
            return Err(ZoneRpcFault::new(
                "checkpoint_commitment",
                "zone host checkpoint replay commitment mismatch",
            ));
        }

        *self
            .runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host factory mutex poisoned"))? =
            factory;
        *self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))? =
            sessions;
        *self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host journal mutex poisoned"))? =
            checkpoint.entries;
        Ok(())
    }
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
        zone_count: usize,
        zone_capacity: usize,
        draining: bool,
        protocol_version: u16,
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
}

impl WireHostJournalEntry {
    fn into_request(self) -> Result<ZoneOwnerCommandRequest, ZoneRpcFault> {
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
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireSessionCommitment {
    session_id: String,
    zone_id: String,
    snapshot_digest: String,
    active_identity: Option<ActiveSessionIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireZoneHostCheckpoint {
    version: u32,
    entries: Vec<WireHostJournalEntry>,
    sessions: Vec<WireSessionCommitment>,
    checksum: String,
}

impl WireZoneHostCheckpoint {
    fn new(
        entries: Vec<WireHostJournalEntry>,
        sessions: Vec<WireSessionCommitment>,
    ) -> Result<Self, ZoneRpcFault> {
        let checksum =
            zone_host_checkpoint_checksum(ZONE_HOST_CHECKPOINT_VERSION, &entries, &sessions)?;
        Ok(Self {
            version: ZONE_HOST_CHECKPOINT_VERSION,
            entries,
            sessions,
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
        let expected =
            zone_host_checkpoint_checksum_bytes(self.version, &self.entries, &self.sessions)?;
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
            Ok(stream) => return Ok(stream),
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

fn call_endpoint(
    address: &str,
    encoded: &[u8],
    limits: &ZoneRpcLimits,
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
    serde_json::from_slice(&response).map_err(|error| format!("response decode failed: {error}"))
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
        commitments.push(WireSessionCommitment {
            session_id: session_id.clone(),
            zone_id: zone_id.clone(),
            snapshot_digest: snapshot_digest(&snapshot)?,
            active_identity: session
                .hosted
                .active_identity()
                .map_err(classify_runtime_error)?,
        });
    }
    Ok(commitments)
}

fn snapshot_digest(snapshot: &WorldSnapshot) -> Result<String, ZoneRpcFault> {
    let durable = durable_session_snapshot(snapshot.clone());
    let bytes = serde_json::to_vec(&durable).map_err(|error| {
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

fn durable_session_snapshot(mut snapshot: WorldSnapshot) -> WorldSnapshot {
    snapshot.tick = 0;
    snapshot.map_title = None;
    snapshot.player_object_id = None;
    snapshot
        .entities
        .retain(|entity| entity.kind == mir2_simulation::WorldEntityKind::SelfPlayer);
    let player_hp = snapshot.player_hp;
    let player_max_hp = snapshot.player_max_hp;
    for entity in &mut snapshot.entities {
        entity.object_id = 0;
        entity.hp = player_hp;
        entity.max_hp = player_max_hp;
    }
    // Shared map actors and ground drops advance on the Zone cadence and are
    // not reconstructable from the per-session command journal. Version 3
    // therefore commits only the durable player/session projection. A future
    // map-state checkpoint must serialize the Zone state machine separately.
    snapshot.ground_drops.clear();
    snapshot
}

fn zone_host_checkpoint_checksum(
    version: u32,
    entries: &[WireHostJournalEntry],
    sessions: &[WireSessionCommitment],
) -> Result<String, ZoneRpcFault> {
    zone_host_checkpoint_checksum_bytes(version, entries, sessions).map_err(|error| {
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
) -> Result<String, String> {
    let payload = serde_json::to_vec(&(version, entries, sessions))
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

fn unexpected_payload(operation: &str, payload: &ZoneRpcPayload) -> String {
    format!("zone RPC {operation} returned unexpected payload {payload:?}")
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

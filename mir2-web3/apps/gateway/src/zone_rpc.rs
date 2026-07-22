use std::collections::BTreeMap;
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

use crate::routing::{
    HostedZoneOwnerCommandClient, SharedInProcessZoneRuntimeFactory, SharedZoneOwnerLeaseAuthority,
    ZoneId, ZoneOwnerCommandMode, ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerRpcTransport,
    ZoneRuntimeFactory,
};
use crate::GatewayConfig;

pub const ZONE_RPC_PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_ZONE_RPC_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_ZONE_RPC_MAX_CONNECTIONS: usize = 64;
pub const DEFAULT_ZONE_RPC_MAX_SESSIONS: usize = 4096;

static NEXT_RPC_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct ZoneRpcLimits {
    pub max_frame_bytes: usize,
    pub max_connections: usize,
    pub max_sessions: usize,
    pub io_timeout: Duration,
}

impl Default for ZoneRpcLimits {
    fn default() -> Self {
        Self {
            max_frame_bytes: DEFAULT_ZONE_RPC_MAX_FRAME_BYTES,
            max_connections: DEFAULT_ZONE_RPC_MAX_CONNECTIONS,
            max_sessions: DEFAULT_ZONE_RPC_MAX_SESSIONS,
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
            io_timeout: Duration::from_millis(
                positive_u64_env("MIR2_ZONE_RPC_TIMEOUT_MS")
                    .unwrap_or(defaults.io_timeout.as_millis() as u64),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneHostHealth {
    pub process_id: u32,
    pub session_count: usize,
    pub protocol_version: u16,
}

#[derive(Clone)]
pub struct TcpZoneOwnerRpcTransport {
    address: String,
    zone_id: ZoneId,
    session_id: String,
    auth_token: Option<String>,
    limits: ZoneRpcLimits,
}

impl fmt::Debug for TcpZoneOwnerRpcTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TcpZoneOwnerRpcTransport")
            .field("address", &self.address)
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
        Self {
            address: address.into(),
            zone_id,
            session_id: session_id.into(),
            auth_token,
            limits,
        }
    }

    pub fn from_env(zone_id: ZoneId) -> Option<Self> {
        std::env::var("MIR2_ZONE_HOST_ADDR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|address| Self::new(address.trim(), zone_id))
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn health(&self) -> Result<ZoneHostHealth, String> {
        match self.call(ZoneRpcRequest::Health)? {
            ZoneRpcPayload::Health {
                process_id,
                session_count,
                protocol_version,
            } => Ok(ZoneHostHealth {
                process_id,
                session_count,
                protocol_version,
            }),
            payload => Err(unexpected_payload("health", &payload)),
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

        let mut stream = connect_with_timeout(&self.address, self.limits.io_timeout)?;
        stream
            .set_read_timeout(Some(self.limits.io_timeout))
            .map_err(|error| format!("zone RPC set read timeout failed: {error}"))?;
        stream
            .set_write_timeout(Some(self.limits.io_timeout))
            .map_err(|error| format!("zone RPC set write timeout failed: {error}"))?;
        write_frame(&mut stream, &encoded, self.limits.max_frame_bytes)
            .map_err(|error| format!("zone RPC write failed: {error}"))?;
        let response = read_frame(&mut stream, self.limits.max_frame_bytes)
            .map_err(|error| format!("zone RPC read failed: {error}"))?;
        let response: ZoneRpcResponse = serde_json::from_slice(&response)
            .map_err(|error| format!("zone RPC response decode failed: {error}"))?;
        match response {
            ZoneRpcResponse::Ok { payload } => Ok(*payload),
            ZoneRpcResponse::Error { code, message } => Err(format!("zone RPC {code}: {message}")),
        }
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
}

pub struct ZoneHostServer {
    config: GatewayConfig,
    runtime_factory: Arc<SharedInProcessZoneRuntimeFactory>,
    owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    sessions: Mutex<BTreeMap<(String, String), Arc<HostedZoneOwnerCommandClient>>>,
    auth_token: Option<String>,
    limits: ZoneRpcLimits,
    active_connections: AtomicUsize,
}

impl fmt::Debug for ZoneHostServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZoneHostServer")
            .field("authenticated", &self.auth_token.is_some())
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
        Self {
            config,
            runtime_factory: Arc::new(SharedInProcessZoneRuntimeFactory::new()),
            owner_lease_authority,
            sessions: Mutex::new(BTreeMap::new()),
            auth_token,
            limits,
            active_connections: AtomicUsize::new(0),
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions
            .lock()
            .map(|sessions| sessions.len())
            .unwrap_or(0)
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
                process_id: std::process::id(),
                session_count: self.session_count(),
                protocol_version: ZONE_RPC_PROTOCOL_VERSION,
            });
        }

        let hosted = self.hosted_session(&envelope.session_id, &envelope.zone_id)?;
        match envelope.request {
            ZoneRpcRequest::Health => unreachable!(),
            ZoneRpcRequest::OnConnect => Ok(ZoneRpcPayload::Packets {
                frames: encode_server_frames(hosted.on_connect()?)?,
            }),
            ZoneRpcRequest::Execute {
                owner_lease,
                mode,
                command,
            } => {
                if owner_lease.zone_id != envelope.zone_id {
                    return Err(ZoneRpcFault::new(
                        "zone_mismatch",
                        "lease zone id does not match envelope zone id",
                    ));
                }
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
                let execution = hosted
                    .execute_request(request)
                    .map_err(classify_runtime_error)?;
                Ok(ZoneRpcPayload::Execution {
                    frames: encode_server_frames(execution.packets)?,
                    packet_count: execution.outcome.packet_count,
                    snapshot_tick: execution.outcome.snapshot_tick,
                    active_identity: execution.outcome.active_identity,
                })
            }
            ZoneRpcRequest::WorldSnapshot => Ok(ZoneRpcPayload::WorldSnapshot {
                snapshot: Box::new(hosted.world_snapshot().map_err(classify_runtime_error)?),
            }),
            ZoneRpcRequest::ActiveIdentity => Ok(ZoneRpcPayload::ActiveIdentity {
                identity: hosted.active_identity().map_err(classify_runtime_error)?,
            }),
            ZoneRpcRequest::SaveActiveCharacter => {
                ZoneOwnerRpcTransport::save_active_character(hosted.as_ref())
                    .map_err(classify_runtime_error)?;
                Ok(ZoneRpcPayload::Unit)
            }
            ZoneRpcRequest::RefreshActiveExternalMail => Ok(ZoneRpcPayload::Bool {
                value: ZoneOwnerRpcTransport::refresh_active_external_mail(hosted.as_ref())
                    .map_err(classify_runtime_error)?,
            }),
        }
    }

    fn hosted_session(
        &self,
        session_id: &str,
        zone_id: &str,
    ) -> Result<Arc<HostedZoneOwnerCommandClient>, ZoneRpcFault> {
        let key = (session_id.to_string(), zone_id.to_string());
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "zone host session mutex poisoned"))?;
        if let Some(hosted) = sessions.get(&key) {
            return Ok(Arc::clone(hosted));
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
        let zone_id = ZoneId::new(zone_id);
        let runtime = self
            .runtime_factory
            .create_runtime(self.config.clone(), &zone_id);
        let hosted = Arc::new(HostedZoneOwnerCommandClient::with_owner_lease_authority(
            runtime,
            Arc::clone(&self.owner_lease_authority),
        ));
        sessions.insert(key, Arc::clone(&hosted));
        Ok(hosted)
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", content = "arguments", rename_all = "camelCase")]
enum ZoneRpcRequest {
    Health,
    OnConnect,
    Execute {
        owner_lease: WireZoneOwnerLease,
        mode: WireZoneOwnerCommandMode,
        command: WireWorldCommand,
    },
    WorldSnapshot,
    ActiveIdentity,
    SaveActiveCharacter,
    RefreshActiveExternalMail,
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
        process_id: u32,
        session_count: usize,
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
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

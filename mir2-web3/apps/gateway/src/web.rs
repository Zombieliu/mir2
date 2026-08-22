use std::collections::HashMap;
use std::env;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path as AxumPath, Query, State};
use axum::http::header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use mir2_game_data::crystal_item_by_index;
use mir2_protocol::{
    crystal_stat_label, packet_payload_hex, server_packet_display_name,
    server_packet_raw_display_name, ClientAuction, ClientFriend, ClientIntelligentCreature,
    ClientPacket, ClientQuestInfo, MirDirection, MirGridType, Point, QuestItemReward, ServerPacket,
    UserItem, UserItemStat,
};
use mir2_simulation::{
    deliver_stage5_system_mail, GameShopPurchaseFailure, GameShopPurchaseOutcome,
    NativeGameShopPurchaseRequest, Stage5MailDelivery, Stage5MailTargetKind, WorldCommand,
    NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{
    mpsc, Mutex as AsyncMutex, OwnedSemaphorePermit, RwLock as AsyncRwLock, Semaphore,
};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::ai_live::{AiLiveHub, AiLiveMode};
use crate::auth::{
    issue_gateway_identity_token_for_subject, verify_channel_guest_proof,
    verify_gateway_identity_token, verify_operator_token, verify_passkey_gateway_token,
    verify_sui_login_proof,
};
use crate::browser_commands::{
    default_drop_count, default_market_max_shape, parse_class, parse_direction, parse_gender,
    parse_grid, parse_move_mode, parse_spell, parse_spell_name,
};
use crate::cache::{
    gateway_session_cache_from_env, gateway_session_cache_status,
    refresh_session_cache_with_route_lease, remove_owned_session_cache, session_cache_key,
    session_cache_record, GatewaySessionCacheKey, GatewaySessionCacheRecord,
    GatewaySessionCacheStatus, GatewaySessionTraceEvent, SharedGatewaySessionCache,
};
use crate::channel_identity::{
    verify_crazygames_token, ChannelIdentityProvider, ChannelIdentityRegistry,
    ChannelIdentityRegistryStatus, PlayerIdentityAccount,
};
use crate::events::{
    default_gameplay_event_sink_from_env, gameplay_event_sink_status, GameplayEventSinkStatus,
    SharedGameplayEventSink,
};
use crate::identity::{IdentityService, VerifiedIdentitySession};
use crate::resume::{
    IssuedResumeCredential, ResumeAuthRevision, ResumeBinding, ResumeConnectionNonce,
    ResumeCredential, ResumeCredentialRegistry, ResumeFamilyId, ResumeIssueContext,
    NATIVE_RESUME_PROTOCOL, RESUME_CREDENTIAL_ROTATION_MS,
};
use crate::routing::{SharedZoneLiveOutbound, ZoneLiveOutboundRegistration};
use crate::session::{catch_gateway_panic, GatewayZoneMovementIngress};
use crate::spectator::{SpectatorFrame, SpectatorHub};
use crate::tcp::chat_broadcast::{
    recv_optional_chat, ChatBroadcastHub, ChatPresence, ChatProtocol,
};
use crate::{GatewayConfig, GatewaySession, ZoneRegistry, ZoneTopology};

type WebSocketSender = futures_util::stream::SplitSink<WebSocket, Message>;
type SharedWebSocketSender = Arc<AsyncMutex<WebSocketSender>>;
type WebSocketReceiver = futures_util::stream::SplitStream<WebSocket>;
type SharedZoneMovementIngressSlot = Arc<RwLock<Option<GatewayZoneMovementIngress>>>;
type SharedSerialExecutionGate = Arc<AsyncRwLock<()>>;
const LIVE_ZONE_OUTBOUND_CAPACITY: usize = 256;
const SOCKET_INPUT_CAPACITY: usize = 256;
const WEBSOCKET_MAX_FRAME_BYTES: usize = 64 * 1024;
const WEBSOCKET_MAX_MESSAGE_BYTES: usize = WEBSOCKET_MAX_FRAME_BYTES;
const SOCKET_INPUT_MAX_BUFFERED_BYTES: usize = WEBSOCKET_MAX_FRAME_BYTES * SOCKET_INPUT_CAPACITY;
const DEFAULT_PRODUCTION_MAX_WS_CONNECTIONS: usize = 2_048;
const DEFAULT_PRODUCTION_MAX_ACTIVE_SESSIONS: usize = 512;
const DEFAULT_PRODUCTION_MAX_RECONNECT_LEASES: usize = 512;
const AUTH_REVISION_BLOCKED: u64 = u64::MAX;
const NATIVE_GAME_SHOP_RECEIPT_PROTOCOL: &str = "nativeGameShopReceiptV1";

enum ParsedSocketInput {
    Action(SessionAction),
    ClientCapabilities(Vec<String>),
    ResumeSession(ResumeCredential),
    ResumeRejected,
    ProtocolError(String),
}

struct PendingSerialAction {
    pending_count: Arc<AtomicUsize>,
}

impl PendingSerialAction {
    fn new(pending_count: Arc<AtomicUsize>) -> Self {
        pending_count.fetch_add(1, Ordering::AcqRel);
        Self { pending_count }
    }
}

impl Drop for PendingSerialAction {
    fn drop(&mut self) {
        self.pending_count.fetch_sub(1, Ordering::AcqRel);
    }
}

struct QueuedSocketInput {
    input: Option<ParsedSocketInput>,
    _pending: PendingSerialAction,
    _buffered_bytes: OwnedSemaphorePermit,
}

impl QueuedSocketInput {
    fn new(
        input: ParsedSocketInput,
        pending_count: Arc<AtomicUsize>,
        buffered_bytes: OwnedSemaphorePermit,
    ) -> Self {
        Self {
            input: Some(input),
            _pending: PendingSerialAction::new(pending_count),
            _buffered_bytes: buffered_bytes,
        }
    }

    fn take_input(&mut self) -> ParsedSocketInput {
        self.input
            .take()
            .expect("queued socket input should only be consumed once")
    }
}

enum SocketInbound {
    Queued(QueuedSocketInput),
    Closed,
    ReadError(String),
}

struct SocketReaderTask {
    handle: JoinHandle<()>,
}

impl Drop for SocketReaderTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

struct ZoneOutboundSenderTask {
    handle: JoinHandle<()>,
}

impl Drop for ZoneOutboundSenderTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

#[derive(Clone)]
struct WebState {
    config: Arc<GatewayConfig>,
    deploy_revision: Option<String>,
    zone_registry: Arc<ZoneRegistry>,
    chat_hub: ChatBroadcastHub,
    session_cache: SharedGatewaySessionCache,
    reconnect_sessions: Arc<ReconnectSessionStore>,
    capacity: Arc<GatewayCapacityState>,
    gameplay_event_sink: Option<SharedGameplayEventSink>,
    identity: Arc<IdentityService>,
    /// On-chain command injection registry (M4, WF-5) — routes Relayer commands to live sessions.
    injector: crate::inject::LiveSessionInjector,
    spectator: SpectatorHub,
    ai_live: AiLiveHub,
    channel_identity: ChannelIdentityRegistry,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpectatorAccessQuery {
    map: Option<String>,
    target: Option<String>,
    delay_ms: Option<u64>,
    token: Option<String>,
    mode: Option<String>,
    replay_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum SpectatorControl {
    Follow { target: Option<String> },
    Map { map: String },
    Director { enabled: bool },
    Camera { x: i32, y: i32 },
    CameraClear,
    ReplayPlay,
    ReplayPause,
    ReplaySeek { captured_at_ms: u64 },
    ReplaySpeed { speed: f64 },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiLiveControlRequest {
    action: String,
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiDistributionControlRequest {
    channel: String,
    action: String,
    token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpectatorDirectoryResponse {
    source: String,
    generated_at_ms: u64,
    public_delay_ms: u64,
    max_delay_ms: u64,
    matches: Vec<crate::spectator::SpectatorMatch>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpectatorRecordingsResponse {
    source: String,
    generated_at_ms: u64,
    recordings: Vec<crate::spectator::SpectatorRecording>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpectatorReplayResponse {
    source: String,
    generated_at_ms: u64,
    recording_id: String,
    frames: Vec<SpectatorFrame>,
}

#[derive(Debug)]
struct GatewayCapacityState {
    max_ws_connections: Option<usize>,
    max_active_sessions: Option<usize>,
    max_reconnect_leases: Option<usize>,
    max_login_in_flight: Option<usize>,
    max_new_character_in_flight: Option<usize>,
    max_start_game_in_flight: Option<usize>,
    current_ws_connections: AtomicUsize,
    current_active_sessions: AtomicUsize,
    current_reconnect_leases: AtomicUsize,
    current_login_in_flight: AtomicUsize,
    current_new_character_in_flight: AtomicUsize,
    current_start_game_in_flight: AtomicUsize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct GatewayCapacityStatus {
    max_ws_connections: Option<usize>,
    max_active_sessions: Option<usize>,
    max_reconnect_leases: Option<usize>,
    max_login_in_flight: Option<usize>,
    max_new_character_in_flight: Option<usize>,
    max_start_game_in_flight: Option<usize>,
    current_ws_connections: usize,
    current_active_sessions: usize,
    current_reconnect_leases: usize,
    current_login_in_flight: usize,
    current_new_character_in_flight: usize,
    current_start_game_in_flight: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatewayCapacityKind {
    WebSocketConnection,
    ActiveSession,
    ReconnectLease,
    Login,
    NewCharacter,
    StartGame,
}

#[derive(Debug)]
struct GatewayCapacityPermit {
    state: Arc<GatewayCapacityState>,
    kind: GatewayCapacityKind,
}

impl GatewayCapacityState {
    fn from_env() -> Self {
        let production_like = gateway_prod_like_env();
        Self {
            max_ws_connections: positive_usize_env("MIR2_GATEWAY_MAX_WS_CONNECTIONS")
                .or_else(|| production_like.then_some(DEFAULT_PRODUCTION_MAX_WS_CONNECTIONS)),
            max_active_sessions: positive_usize_env("MIR2_GATEWAY_MAX_ACTIVE_SESSIONS")
                .or_else(|| production_like.then_some(DEFAULT_PRODUCTION_MAX_ACTIVE_SESSIONS)),
            max_reconnect_leases: positive_usize_env("MIR2_GATEWAY_MAX_RECONNECT_LEASES")
                .or_else(|| production_like.then_some(DEFAULT_PRODUCTION_MAX_RECONNECT_LEASES)),
            max_login_in_flight: positive_usize_env("MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT")
                .or_else(|| production_like.then_some(8)),
            max_new_character_in_flight: positive_usize_env(
                "MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT",
            )
            .or_else(|| production_like.then_some(4)),
            max_start_game_in_flight: positive_usize_env("MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT")
                .or_else(|| production_like.then_some(4)),
            current_ws_connections: AtomicUsize::new(0),
            current_active_sessions: AtomicUsize::new(0),
            current_reconnect_leases: AtomicUsize::new(0),
            current_login_in_flight: AtomicUsize::new(0),
            current_new_character_in_flight: AtomicUsize::new(0),
            current_start_game_in_flight: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    fn unlimited() -> Self {
        Self {
            max_ws_connections: None,
            max_active_sessions: None,
            max_reconnect_leases: None,
            max_login_in_flight: None,
            max_new_character_in_flight: None,
            max_start_game_in_flight: None,
            current_ws_connections: AtomicUsize::new(0),
            current_active_sessions: AtomicUsize::new(0),
            current_reconnect_leases: AtomicUsize::new(0),
            current_login_in_flight: AtomicUsize::new(0),
            current_new_character_in_flight: AtomicUsize::new(0),
            current_start_game_in_flight: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    fn with_limits(
        max_ws_connections: Option<usize>,
        max_active_sessions: Option<usize>,
        max_reconnect_leases: Option<usize>,
    ) -> Self {
        Self {
            max_ws_connections,
            max_active_sessions,
            max_reconnect_leases,
            max_login_in_flight: None,
            max_new_character_in_flight: None,
            max_start_game_in_flight: None,
            current_ws_connections: AtomicUsize::new(0),
            current_active_sessions: AtomicUsize::new(0),
            current_reconnect_leases: AtomicUsize::new(0),
            current_login_in_flight: AtomicUsize::new(0),
            current_new_character_in_flight: AtomicUsize::new(0),
            current_start_game_in_flight: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    fn with_action_limits(
        max_login_in_flight: Option<usize>,
        max_new_character_in_flight: Option<usize>,
        max_start_game_in_flight: Option<usize>,
    ) -> Self {
        Self {
            max_ws_connections: None,
            max_active_sessions: None,
            max_reconnect_leases: None,
            max_login_in_flight,
            max_new_character_in_flight,
            max_start_game_in_flight,
            current_ws_connections: AtomicUsize::new(0),
            current_active_sessions: AtomicUsize::new(0),
            current_reconnect_leases: AtomicUsize::new(0),
            current_login_in_flight: AtomicUsize::new(0),
            current_new_character_in_flight: AtomicUsize::new(0),
            current_start_game_in_flight: AtomicUsize::new(0),
        }
    }

    fn status(&self) -> GatewayCapacityStatus {
        GatewayCapacityStatus {
            max_ws_connections: self.max_ws_connections,
            max_active_sessions: self.max_active_sessions,
            max_reconnect_leases: self.max_reconnect_leases,
            max_login_in_flight: self.max_login_in_flight,
            max_new_character_in_flight: self.max_new_character_in_flight,
            max_start_game_in_flight: self.max_start_game_in_flight,
            current_ws_connections: self.current_ws_connections.load(Ordering::Acquire),
            current_active_sessions: self.current_active_sessions.load(Ordering::Acquire),
            current_reconnect_leases: self.current_reconnect_leases.load(Ordering::Acquire),
            current_login_in_flight: self.current_login_in_flight.load(Ordering::Acquire),
            current_new_character_in_flight: self
                .current_new_character_in_flight
                .load(Ordering::Acquire),
            current_start_game_in_flight: self.current_start_game_in_flight.load(Ordering::Acquire),
        }
    }

    fn try_acquire_ws_connection(self: &Arc<Self>) -> Result<GatewayCapacityPermit, String> {
        self.try_acquire(GatewayCapacityKind::WebSocketConnection)
    }

    fn try_acquire_active_session(self: &Arc<Self>) -> Result<GatewayCapacityPermit, String> {
        self.try_acquire(GatewayCapacityKind::ActiveSession)
    }

    fn try_acquire_reconnect_lease(self: &Arc<Self>) -> Result<GatewayCapacityPermit, String> {
        self.try_acquire(GatewayCapacityKind::ReconnectLease)
    }

    fn try_acquire_action(
        self: &Arc<Self>,
        kind: GatewayCapacityKind,
    ) -> Result<GatewayCapacityPermit, String> {
        debug_assert!(matches!(
            kind,
            GatewayCapacityKind::Login
                | GatewayCapacityKind::NewCharacter
                | GatewayCapacityKind::StartGame
        ));
        self.try_acquire(kind)
    }

    async fn acquire_action_with_wait(
        self: &Arc<Self>,
        kind: GatewayCapacityKind,
        maximum_wait: Duration,
    ) -> Result<GatewayCapacityPermit, String> {
        let started_at = Instant::now();
        loop {
            match self.try_acquire_action(kind) {
                Ok(permit) => return Ok(permit),
                Err(error) if maximum_wait.is_zero() => return Err(error),
                Err(error) => {
                    let Some(remaining) = maximum_wait.checked_sub(started_at.elapsed()) else {
                        return Err(format!(
                            "{error}; queue wait timed out after {}ms",
                            maximum_wait.as_millis()
                        ));
                    };
                    tokio::time::sleep(remaining.min(Duration::from_millis(100))).await;
                }
            }
        }
    }

    fn try_acquire(
        self: &Arc<Self>,
        kind: GatewayCapacityKind,
    ) -> Result<GatewayCapacityPermit, String> {
        let counter = self.counter(kind);
        let previous = counter.fetch_add(1, Ordering::AcqRel);
        if let Some(limit) = self.limit(kind) {
            if previous >= limit {
                counter.fetch_sub(1, Ordering::AcqRel);
                return Err(format!(
                    "gateway {} capacity reached (limit {limit})",
                    kind.label()
                ));
            }
        }
        Ok(GatewayCapacityPermit {
            state: Arc::clone(self),
            kind,
        })
    }

    fn counter(&self, kind: GatewayCapacityKind) -> &AtomicUsize {
        match kind {
            GatewayCapacityKind::WebSocketConnection => &self.current_ws_connections,
            GatewayCapacityKind::ActiveSession => &self.current_active_sessions,
            GatewayCapacityKind::ReconnectLease => &self.current_reconnect_leases,
            GatewayCapacityKind::Login => &self.current_login_in_flight,
            GatewayCapacityKind::NewCharacter => &self.current_new_character_in_flight,
            GatewayCapacityKind::StartGame => &self.current_start_game_in_flight,
        }
    }

    fn limit(&self, kind: GatewayCapacityKind) -> Option<usize> {
        match kind {
            GatewayCapacityKind::WebSocketConnection => self.max_ws_connections,
            GatewayCapacityKind::ActiveSession => self.max_active_sessions,
            GatewayCapacityKind::ReconnectLease => self.max_reconnect_leases,
            GatewayCapacityKind::Login => self.max_login_in_flight,
            GatewayCapacityKind::NewCharacter => self.max_new_character_in_flight,
            GatewayCapacityKind::StartGame => self.max_start_game_in_flight,
        }
    }
}

impl GatewayCapacityKind {
    fn label(self) -> &'static str {
        match self {
            GatewayCapacityKind::WebSocketConnection => "WebSocket connection",
            GatewayCapacityKind::ActiveSession => "active session",
            GatewayCapacityKind::ReconnectLease => "reconnect lease",
            GatewayCapacityKind::Login => "login in-flight",
            GatewayCapacityKind::NewCharacter => "new-character in-flight",
            GatewayCapacityKind::StartGame => "StartGame in-flight",
        }
    }
}

impl Drop for GatewayCapacityPermit {
    fn drop(&mut self) {
        let previous = self.state.counter(self.kind).fetch_sub(1, Ordering::AcqRel);
        debug_assert!(
            previous > 0,
            "gateway capacity permit dropped with no matching acquisition"
        );
    }
}

#[derive(Debug, Default)]
struct ReconnectSessionStore {
    state: Mutex<ReconnectSessionState>,
}

#[derive(Debug, Default)]
struct ReconnectSessionState {
    sessions: HashMap<GatewaySessionCacheKey, ReconnectSessionLease>,
    credentials: ResumeCredentialRegistry,
    account_auth_revisions: HashMap<String, u64>,
    identity_session_auth_revisions: HashMap<String, u64>,
    account_revocations_in_progress: HashMap<String, usize>,
    identity_session_revocations_in_progress: HashMap<String, usize>,
}

#[derive(Debug)]
struct ReconnectSessionLease {
    session: GatewaySession,
    active_session_permit: Option<GatewayCapacityPermit>,
    _reconnect_lease_permit: GatewayCapacityPermit,
    resume_family_id: Option<ResumeFamilyId>,
    expires_at: Instant,
}

#[derive(Debug)]
struct ReconnectSessionRestore {
    session: GatewaySession,
    active_session_permit: Option<GatewayCapacityPermit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconnectSessionCommitError {
    ForeignReservation,
    LeaseExpired,
    CredentialUnavailable,
    AuthorizationRevisionChanged,
}

/// Reversible, single-owner lease reservation for native resume preparation.
/// The credential family remains live until commit; all other drops restore
/// the exact lease and its capacity permits through RAII.
struct ReconnectSessionReservation<'a> {
    store: &'a ReconnectSessionStore,
    key: GatewaySessionCacheKey,
    lease: Option<ReconnectSessionLease>,
    binding: ResumeBinding,
}

/// RAII marker for an identity mutation. Entering the fence advances the
/// affected authorization revision before the durable identity/cache mutation
/// starts. Resume issuance, reservation, and commit all fail closed while the
/// marker is live, and old credentials remain stale after it is dropped.
struct IdentityRevocationFence<'a> {
    store: &'a ReconnectSessionStore,
    account_id: Option<String>,
    identity_session_ids: Vec<String>,
}

impl ReconnectSessionReservation<'_> {
    fn session(&self) -> &GatewaySession {
        &self
            .lease
            .as_ref()
            .expect("an uncommitted reconnect reservation must retain its lease")
            .session
    }

    fn discard_and_revoke(mut self) {
        self.store.revoke_resume_family(&self.binding.family_id);
        // Taking the lease makes Drop a no-op. Dropping it here releases both
        // active-session and reconnect-lease permits immediately instead of
        // retaining an unusable lease until the reconnect TTL expires.
        drop(self.lease.take());
    }

    #[cfg(test)]
    fn rollback(self) {
        drop(self);
    }
}

impl Drop for ReconnectSessionReservation<'_> {
    fn drop(&mut self) {
        let Some(lease) = self.lease.take() else {
            return;
        };
        self.store
            .rollback_reservation(&self.key, &self.binding, lease);
    }
}

impl Drop for IdentityRevocationFence<'_> {
    fn drop(&mut self) {
        let mut state = self
            .store
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        if let Some(account_id) = self.account_id.as_deref() {
            ReconnectSessionStore::decrement_in_progress(
                &mut state.account_revocations_in_progress,
                account_id,
            );
        }
        for session_id in &self.identity_session_ids {
            ReconnectSessionStore::decrement_in_progress(
                &mut state.identity_session_revocations_in_progress,
                session_id,
            );
        }
    }
}

struct NativeResumeConnectionState {
    opted_in: bool,
    resume_allowed: bool,
    connection_nonce: ResumeConnectionNonce,
    family_id: Option<ResumeFamilyId>,
    minimum_generation: u64,
    last_issued_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeGameShopRequest {
    request_id: String,
    server_idempotency_key: String,
    g_index: i32,
    quantity: u8,
    price_type: i32,
}

#[derive(Debug, Default)]
struct NativeGameShopConnectionState {
    opted_in: bool,
    pending: Option<NativeGameShopRequest>,
}

#[derive(Debug, PartialEq)]
enum NativeGameShopPostExecution {
    SendReceipt(Value),
    CloseUnknown { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeGameShopPreExecutionReceiptDisposition {
    Continue,
    CloseUnknown,
}

#[derive(Debug, PartialEq)]
struct NativeGameShopHandlerDispatch {
    normal_packets: Vec<ServerPacket>,
    post_execution: NativeGameShopPostExecution,
}

impl NativeGameShopConnectionState {
    fn reserve(&mut self, request: NativeGameShopRequest) -> Result<(), Value> {
        if self.pending.is_some() {
            return Err(native_game_shop_failure_event(
                &request,
                "requestInFlight",
                None,
            ));
        }
        self.pending = Some(request);
        Ok(())
    }

    fn clear_exact(&mut self, request: &NativeGameShopRequest) -> bool {
        if self.pending.as_ref() == Some(request) {
            self.pending = None;
            true
        } else {
            false
        }
    }
}

fn finish_native_game_shop_pre_execution_receipt(
    state: &mut NativeGameShopConnectionState,
    request: &NativeGameShopRequest,
    send_succeeded: bool,
) -> NativeGameShopPreExecutionReceiptDisposition {
    if !send_succeeded || !state.clear_exact(request) {
        return NativeGameShopPreExecutionReceiptDisposition::CloseUnknown;
    }
    NativeGameShopPreExecutionReceiptDisposition::Continue
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeClientCapabilities {
    native_resume_v1: bool,
    native_game_shop_receipt_v1: bool,
}

impl NativeResumeConnectionState {
    fn new() -> Self {
        Self {
            opted_in: false,
            resume_allowed: true,
            connection_nonce: ResumeConnectionNonce::generate(),
            family_id: None,
            minimum_generation: 1,
            last_issued_at_ms: None,
        }
    }

    fn disable_and_revoke(&mut self, store: &ReconnectSessionStore) {
        self.resume_allowed = false;
        if let Some(family_id) = self.family_id.take() {
            store.revoke_resume_family(&family_id);
        }
        self.last_issued_at_ms = None;
    }

    fn reset_for_authenticated_login(&mut self) {
        self.resume_allowed = true;
        self.family_id = None;
        self.minimum_generation = 1;
        self.last_issued_at_ms = None;
    }

    fn should_rotate(&self, now_ms: u64, force: bool) -> bool {
        self.opted_in
            && self.resume_allowed
            && (force
                || self.last_issued_at_ms.is_none_or(|issued_at_ms| {
                    now_ms.saturating_sub(issued_at_ms) >= RESUME_CREDENTIAL_ROTATION_MS
                }))
    }
}

#[derive(Debug, Clone, Copy)]
struct GatewaySaveQueueConfig {
    debounce: Duration,
    checkpoint: Duration,
    queue_limit: usize,
}

#[derive(Debug)]
struct WebSessionSaveQueue {
    config: GatewaySaveQueueConfig,
    dirty: bool,
    queued_requests: usize,
    last_saved_at: Instant,
}

#[derive(Debug, Clone, Copy)]
struct GatewayRouteRefreshConfig {
    interval: Duration,
}

#[derive(Debug)]
struct WebSessionRouteRefresh {
    config: GatewayRouteRefreshConfig,
    last_refreshed_at: Option<Instant>,
}

type SharedBackgroundRouteRefreshRecord = Arc<Mutex<Option<(GatewaySessionCacheRecord, String)>>>;

struct BackgroundRouteRefreshTask {
    handle: JoinHandle<()>,
}

impl GatewaySaveQueueConfig {
    fn from_env() -> Self {
        Self {
            debounce: Duration::from_millis(
                env::var("MIR2_GATEWAY_SAVE_DEBOUNCE_MS")
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .unwrap_or(1_500)
                    .max(1),
            ),
            checkpoint: Duration::from_secs(
                env::var("MIR2_GATEWAY_SAVE_CHECKPOINT_SECONDS")
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .unwrap_or(15)
                    .max(1),
            ),
            queue_limit: positive_usize_env("MIR2_GATEWAY_SAVE_QUEUE_LIMIT").unwrap_or(64),
        }
    }

    #[cfg(test)]
    fn new(debounce: Duration, checkpoint: Duration, queue_limit: usize) -> Self {
        Self {
            debounce,
            checkpoint,
            queue_limit: queue_limit.max(1),
        }
    }
}

impl WebSessionSaveQueue {
    fn new(config: GatewaySaveQueueConfig) -> Self {
        Self {
            config,
            dirty: false,
            queued_requests: 0,
            last_saved_at: Instant::now(),
        }
    }

    fn request_save(
        &mut self,
        now: Instant,
        save: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        self.dirty = true;
        self.queued_requests = self.queued_requests.saturating_add(1);
        if self.queued_requests >= self.config.queue_limit
            || now.duration_since(self.last_saved_at) >= self.config.debounce
        {
            return self.flush_now(now, save);
        }
        Ok(())
    }

    fn checkpoint(
        &mut self,
        now: Instant,
        save: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        if self.dirty && now.duration_since(self.last_saved_at) >= self.config.checkpoint {
            return self.flush_now(now, save);
        }
        Ok(())
    }

    fn flush_now(
        &mut self,
        now: Instant,
        save: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        save()?;
        self.dirty = false;
        self.queued_requests = 0;
        self.last_saved_at = now;
        Ok(())
    }

    fn force_save_now(
        &mut self,
        now: Instant,
        save: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        save()?;
        self.dirty = false;
        self.queued_requests = 0;
        self.last_saved_at = now;
        Ok(())
    }

    #[cfg(test)]
    fn has_pending_save(&self) -> bool {
        self.dirty
    }
}

impl GatewayRouteRefreshConfig {
    fn from_env() -> Self {
        Self {
            interval: Duration::from_millis(
                env::var("MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS")
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .unwrap_or(5_000)
                    .clamp(250, 30_000),
            ),
        }
    }

    #[cfg(test)]
    fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl WebSessionRouteRefresh {
    fn new(config: GatewayRouteRefreshConfig) -> Self {
        Self {
            config,
            last_refreshed_at: None,
        }
    }

    fn claim_refresh_due(&mut self, session: &GatewaySession, now: Instant, force: bool) -> bool {
        if session.active_identity().is_none() {
            return false;
        }
        if !force
            && self
                .last_refreshed_at
                .is_some_and(|last| now.duration_since(last) < self.config.interval)
        {
            return false;
        }
        self.last_refreshed_at = Some(now);
        true
    }

    fn maybe_refresh(
        &mut self,
        session_cache: &dyn crate::cache::GatewaySessionCache,
        session: &GatewaySession,
        now: Instant,
        force: bool,
    ) -> Result<bool, String> {
        if !self.claim_refresh_due(session, now, force) {
            return Ok(false);
        }
        refresh_session_cache_with_route_lease(session_cache, session, route_lease_ttl_seconds())?;
        Ok(true)
    }
}

impl Drop for BackgroundRouteRefreshTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl ReconnectSessionStore {
    fn auth_revision_locked(
        state: &ReconnectSessionState,
        account_id: &str,
        identity_session_id: &str,
    ) -> ResumeAuthRevision {
        ResumeAuthRevision {
            account: state
                .account_auth_revisions
                .get(account_id)
                .copied()
                .unwrap_or_default(),
            identity_session: state
                .identity_session_auth_revisions
                .get(identity_session_id)
                .copied()
                .unwrap_or_default(),
        }
    }

    fn auth_revocation_in_progress_locked(
        state: &ReconnectSessionState,
        account_id: &str,
        identity_session_id: &str,
    ) -> bool {
        state
            .account_revocations_in_progress
            .get(account_id)
            .copied()
            .unwrap_or_default()
            > 0
            || state
                .identity_session_revocations_in_progress
                .get(identity_session_id)
                .copied()
                .unwrap_or_default()
                > 0
    }

    fn auth_revision_blocked_locked(
        state: &ReconnectSessionState,
        account_id: &str,
        identity_session_id: &str,
    ) -> bool {
        state.account_auth_revisions.get(account_id).copied() == Some(AUTH_REVISION_BLOCKED)
            || state
                .identity_session_auth_revisions
                .get(identity_session_id)
                .copied()
                == Some(AUTH_REVISION_BLOCKED)
    }

    fn advance_revision(revisions: &mut HashMap<String, u64>, key: &str) {
        let revision = revisions.entry(key.to_string()).or_default();
        *revision = revision.checked_add(1).unwrap_or(AUTH_REVISION_BLOCKED);
    }

    fn increment_in_progress(in_progress: &mut HashMap<String, usize>, key: &str) {
        let count = in_progress.entry(key.to_string()).or_default();
        *count = count.saturating_add(1);
    }

    fn decrement_in_progress(in_progress: &mut HashMap<String, usize>, key: &str) {
        let Some(count) = in_progress.get_mut(key) else {
            return;
        };
        *count = count.saturating_sub(1);
        if *count == 0 {
            in_progress.remove(key);
        }
    }

    fn begin_account_identity_revocation(&self, account_id: &str) -> IdentityRevocationFence<'_> {
        let account_id = account_id.to_string();
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::advance_revision(&mut state.account_auth_revisions, &account_id);
        Self::increment_in_progress(&mut state.account_revocations_in_progress, &account_id);
        drop(state);
        IdentityRevocationFence {
            store: self,
            account_id: Some(account_id),
            identity_session_ids: Vec::new(),
        }
    }

    fn begin_identity_session_revocations(
        &self,
        identity_session_ids: impl IntoIterator<Item = String>,
    ) -> IdentityRevocationFence<'_> {
        let mut identity_session_ids = identity_session_ids.into_iter().collect::<Vec<_>>();
        identity_session_ids.sort_unstable();
        identity_session_ids.dedup();
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        for session_id in &identity_session_ids {
            Self::advance_revision(&mut state.identity_session_auth_revisions, session_id);
            Self::increment_in_progress(
                &mut state.identity_session_revocations_in_progress,
                session_id,
            );
        }
        drop(state);
        IdentityRevocationFence {
            store: self,
            account_id: None,
            identity_session_ids,
        }
    }

    fn begin_identity_session_revocation(
        &self,
        identity_session_id: &str,
    ) -> IdentityRevocationFence<'_> {
        self.begin_identity_session_revocations([identity_session_id.to_string()])
    }

    fn store(
        &self,
        key: GatewaySessionCacheKey,
        session: GatewaySession,
        active_session_permit: Option<GatewayCapacityPermit>,
        reconnect_lease_permit: GatewayCapacityPermit,
        resume_family_id: Option<ResumeFamilyId>,
        ttl: Duration,
    ) {
        let expires_at = Instant::now() + ttl.max(Duration::from_millis(1));
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
        let replaced = state.sessions.insert(
            key,
            ReconnectSessionLease {
                session,
                active_session_permit,
                _reconnect_lease_permit: reconnect_lease_permit,
                resume_family_id,
                expires_at,
            },
        );
        if let Some(family_id) = replaced.and_then(|lease| lease.resume_family_id) {
            state.credentials.revoke_family(&family_id);
        }
    }

    fn take(&self, key: &GatewaySessionCacheKey) -> Option<ReconnectSessionRestore> {
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
        let lease = state.sessions.remove(key)?;
        if lease.expires_at <= Instant::now() {
            return None;
        }
        if let Some(family_id) = lease.resume_family_id.as_ref() {
            state.credentials.revoke_family(family_id);
        }
        Some(ReconnectSessionRestore {
            session: lease.session,
            active_session_permit: lease.active_session_permit,
        })
    }

    fn issue_resume_credential(
        &self,
        current_family: Option<&ResumeFamilyId>,
        context: ResumeIssueContext<'_>,
        now_ms: u64,
        minimum_generation: u64,
        identity_is_active: impl FnOnce() -> bool,
    ) -> Option<IssuedResumeCredential> {
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
        if Self::auth_revocation_in_progress_locked(
            &state,
            context.account_id,
            context.identity_session_id,
        ) || Self::auth_revision_blocked_locked(
            &state,
            context.account_id,
            context.identity_session_id,
        ) || !identity_is_active()
        {
            return None;
        }
        let auth_revision =
            Self::auth_revision_locked(&state, context.account_id, context.identity_session_id);
        Some(state.credentials.issue(
            current_family,
            context,
            now_ms,
            minimum_generation,
            auth_revision,
        ))
    }

    fn resume_binding(&self, credential: &ResumeCredential, now_ms: u64) -> Option<ResumeBinding> {
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
        state
            .credentials
            .binding_for_credential(credential.as_str(), now_ms)
    }

    fn reserve_by_credential<'a>(
        &'a self,
        credential: &ResumeCredential,
        expected: &ResumeBinding,
        now_ms: u64,
    ) -> Option<ReconnectSessionReservation<'a>> {
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
        let key = GatewaySessionCacheKey {
            account_id: expected.account_id.clone(),
            character_index: expected.character_index,
        };
        let lease = state.sessions.get(&key)?;
        let active_identity = lease.session.active_identity()?;
        if lease.expires_at <= Instant::now()
            || lease.session.session_id() != expected.gateway_session_id
            || active_identity.account_id != expected.account_id
            || active_identity.character_index != expected.character_index
            || lease.resume_family_id.as_ref() != Some(&expected.family_id)
        {
            return None;
        }
        let binding = state
            .credentials
            .binding_for_credential(credential.as_str(), now_ms)?;
        if &binding != expected
            || binding.auth_revision
                != Self::auth_revision_locked(
                    &state,
                    &binding.account_id,
                    &binding.identity_session_id,
                )
            || Self::auth_revocation_in_progress_locked(
                &state,
                &binding.account_id,
                &binding.identity_session_id,
            )
            || Self::auth_revision_blocked_locked(
                &state,
                &binding.account_id,
                &binding.identity_session_id,
            )
        {
            return None;
        }
        let lease = state
            .sessions
            .remove(&key)
            .expect("validated reconnect lease must remain present under the same mutex");
        Some(ReconnectSessionReservation {
            store: self,
            key,
            lease: Some(lease),
            binding,
        })
    }

    fn commit_resume(
        &self,
        mut reservation: ReconnectSessionReservation<'_>,
        credential: &ResumeCredential,
        now_ms: u64,
    ) -> Result<(ReconnectSessionRestore, ResumeBinding), ReconnectSessionCommitError> {
        if !std::ptr::eq(self, reservation.store) {
            reservation.discard_and_revoke();
            return Err(ReconnectSessionCommitError::ForeignReservation);
        }
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
        // The reservation lives outside the store while route and Zone
        // preparation run. Its lease can therefore expire while this mutex is
        // contended; re-check only after acquiring the commit mutex.
        let Some(lease) = reservation.lease.as_ref() else {
            state
                .credentials
                .revoke_family(&reservation.binding.family_id);
            return Err(ReconnectSessionCommitError::CredentialUnavailable);
        };
        if lease.expires_at <= Instant::now() {
            state
                .credentials
                .revoke_family(&reservation.binding.family_id);
            let expired_lease = reservation.lease.take();
            drop(state);
            drop(expired_lease);
            return Err(ReconnectSessionCommitError::LeaseExpired);
        }
        if reservation.binding.auth_revision
            != Self::auth_revision_locked(
                &state,
                &reservation.binding.account_id,
                &reservation.binding.identity_session_id,
            )
            || Self::auth_revocation_in_progress_locked(
                &state,
                &reservation.binding.account_id,
                &reservation.binding.identity_session_id,
            )
            || Self::auth_revision_blocked_locked(
                &state,
                &reservation.binding.account_id,
                &reservation.binding.identity_session_id,
            )
        {
            state
                .credentials
                .revoke_family(&reservation.binding.family_id);
            let stale_lease = reservation.lease.take();
            drop(state);
            drop(stale_lease);
            return Err(ReconnectSessionCommitError::AuthorizationRevisionChanged);
        }
        let Some(binding) =
            state
                .credentials
                .consume_matching(credential.as_str(), &reservation.binding, now_ms)
        else {
            state
                .credentials
                .revoke_family(&reservation.binding.family_id);
            let unavailable_lease = reservation.lease.take();
            drop(state);
            drop(unavailable_lease);
            return Err(ReconnectSessionCommitError::CredentialUnavailable);
        };
        let lease = reservation
            .lease
            .take()
            .expect("validated reconnect reservation must retain its exact lease");
        Ok((
            ReconnectSessionRestore {
                session: lease.session,
                active_session_permit: lease.active_session_permit,
            },
            binding,
        ))
    }

    fn rollback_reservation(
        &self,
        key: &GatewaySessionCacheKey,
        binding: &ResumeBinding,
        lease: ReconnectSessionLease,
    ) {
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
        let auth_revision_is_current = binding.auth_revision
            == Self::auth_revision_locked(
                &state,
                &binding.account_id,
                &binding.identity_session_id,
            );
        let credential_is_live = state
            .credentials
            .contains_binding(binding, gateway_unix_ms());
        if lease.expires_at <= Instant::now()
            || state.sessions.contains_key(key)
            || !auth_revision_is_current
            || Self::auth_revocation_in_progress_locked(
                &state,
                &binding.account_id,
                &binding.identity_session_id,
            )
            || Self::auth_revision_blocked_locked(
                &state,
                &binding.account_id,
                &binding.identity_session_id,
            )
            || !credential_is_live
        {
            state.credentials.revoke_family(&binding.family_id);
            return;
        }
        state.sessions.insert(key.clone(), lease);
    }

    #[cfg(test)]
    fn take_by_credential(
        &self,
        credential: &ResumeCredential,
        expected: &ResumeBinding,
        now_ms: u64,
    ) -> Option<(ReconnectSessionRestore, ResumeBinding)> {
        let reservation = self.reserve_by_credential(credential, expected, now_ms)?;
        self.commit_resume(reservation, credential, now_ms).ok()
    }

    fn revoke_resume_family(&self, family_id: &ResumeFamilyId) {
        self.state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned")
            .credentials
            .revoke_family(family_id);
    }

    fn purge_expired(&self) {
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
        state.sessions.len()
    }

    #[cfg(test)]
    fn capacity_state_for_binding(
        &self,
        binding: &ResumeBinding,
    ) -> Option<Arc<GatewayCapacityState>> {
        let mut state = self
            .state
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut state);
        state
            .sessions
            .get(&GatewaySessionCacheKey {
                account_id: binding.account_id.clone(),
                character_index: binding.character_index,
            })
            .map(|lease| Arc::clone(&lease._reconnect_lease_permit.state))
    }

    fn purge_expired_locked(state: &mut ReconnectSessionState) {
        let now = Instant::now();
        let expired_families = state
            .sessions
            .values()
            .filter(|lease| lease.expires_at <= now)
            .filter_map(|lease| lease.resume_family_id.clone())
            .collect::<Vec<_>>();
        state.sessions.retain(|_, lease| lease.expires_at > now);
        for family_id in expired_families {
            state.credentials.revoke_family(&family_id);
        }
        state.credentials.purge_expired(gateway_unix_ms());
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum BrowserCommand {
    ClientVersion,
    ClientCapabilities {
        capabilities: Vec<String>,
    },
    ResumeSession(NativeResumeRequest),
    Disconnect,
    TownRevive,
    Login {
        #[serde(alias = "accountId")]
        account_id: String,
        password: String,
    },
    PasskeyLogin {
        #[serde(alias = "accountId")]
        account_id: String,
        token: String,
    },
    NewAccount {
        #[serde(alias = "accountId")]
        account_id: String,
        password: String,
        #[serde(alias = "birthDateBinary", default)]
        birth_date_binary: i64,
        #[serde(alias = "userName", default)]
        user_name: String,
        #[serde(alias = "secretQuestion", default)]
        secret_question: String,
        #[serde(alias = "secretAnswer", default)]
        secret_answer: String,
        #[serde(alias = "emailAddress", default)]
        email_address: String,
    },
    ChangePassword {
        #[serde(alias = "accountId")]
        account_id: String,
        #[serde(alias = "currentPassword")]
        current_password: String,
        #[serde(alias = "newPassword")]
        new_password: String,
    },
    UnlockStorage {
        password: String,
    },
    SetStoragePassword {
        #[serde(alias = "currentPassword")]
        current_password: String,
        #[serde(alias = "newPassword")]
        new_password: String,
    },
    RemoveStoragePassword {
        #[serde(alias = "currentPassword")]
        current_password: String,
    },
    NewCharacter {
        name: String,
        gender: String,
        class: String,
    },
    NewHero {
        name: String,
        gender: String,
        class: String,
    },
    StartGame {
        #[serde(alias = "characterIndex")]
        character_index: i32,
    },
    Turn {
        direction: String,
    },
    Walk {
        direction: String,
    },
    Run {
        direction: String,
    },
    Chat {
        message: String,
    },
    KeepAlive {
        #[serde(alias = "time")]
        time: i64,
    },
    MoveTo {
        x: i32,
        y: i32,
        mode: Option<String>,
    },
    Attack {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    AttackDirection {
        direction: String,
        #[serde(default)]
        spell: Option<u8>,
    },
    RangeAttack {
        direction: String,
        x: i32,
        y: i32,
        #[serde(alias = "targetId")]
        target_id: u32,
        #[serde(alias = "targetX")]
        target_x: i32,
        #[serde(alias = "targetY")]
        target_y: i32,
    },
    Harvest {
        direction: String,
    },
    Interact {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    SelectNpcDialog {
        target: String,
    },
    SubmitNpcInput {
        value: String,
    },
    PickUp {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    PickUpTile,
    UseItem {
        #[serde(default)]
        key: Option<String>,
        #[serde(alias = "uniqueId", default)]
        unique_id: Option<u64>,
        #[serde(default)]
        slot: Option<u8>,
        #[serde(default)]
        grid: Option<String>,
    },
    MoveItem {
        grid: String,
        from: i32,
        to: i32,
    },
    MergeItem {
        #[serde(alias = "gridFrom")]
        grid_from: String,
        #[serde(alias = "gridTo")]
        grid_to: String,
        #[serde(alias = "idFrom")]
        id_from: u64,
        #[serde(alias = "idTo")]
        id_to: u64,
    },
    EquipItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        to: i32,
    },
    RemoveItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        to: i32,
    },
    RemoveSlotItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        #[serde(alias = "gridTo")]
        grid_to: String,
        #[serde(alias = "fromUniqueId")]
        from_unique_id: u64,
        to: i32,
    },
    SplitItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        count: u16,
    },
    StoreItem {
        from: i32,
        to: i32,
    },
    TakeBackItem {
        from: i32,
        to: i32,
    },
    StoreItemV2 {
        #[serde(rename = "requestId")]
        request_id: String,
        from: i32,
        to: i32,
    },
    TakeBackItemV2 {
        #[serde(rename = "requestId")]
        request_id: String,
        from: i32,
        to: i32,
    },
    TakeBackHeroItem {
        from: i32,
        to: i32,
    },
    TransferHeroItem {
        from: i32,
        to: i32,
    },
    DropItem {
        key: String,
        #[serde(alias = "uniqueId", default)]
        unique_id: Option<u64>,
        #[serde(default = "default_drop_count")]
        count: u16,
        #[serde(alias = "heroInventory", default)]
        hero_inventory: bool,
    },
    DeleteItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        #[serde(default = "default_drop_count")]
        count: u16,
        #[serde(alias = "heroInventory", default)]
        hero_inventory: bool,
    },
    DropGold {
        amount: u32,
    },
    RequestMapInfo {
        #[serde(alias = "mapIndex")]
        map_index: i32,
    },
    SearchMap {
        text: String,
    },
    TeleportToNpc {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    RequestItemInfo {
        #[serde(alias = "itemIndex")]
        item_index: i32,
    },
    SellItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        #[serde(default = "default_drop_count")]
        count: u16,
    },
    BuyItem {
        #[serde(alias = "itemIndex")]
        item_index: u64,
        #[serde(default = "default_drop_count")]
        count: u16,
        #[serde(alias = "panelType", default)]
        panel_type: u8,
    },
    GameShopBuy {
        #[serde(alias = "requestId", default)]
        request_id: Option<String>,
        #[serde(alias = "gIndex")]
        g_index: i32,
        quantity: u8,
        #[serde(alias = "priceType")]
        price_type: i32,
    },
    RepairItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
    },
    SpecialRepairItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
    },
    DeleteCharacter {
        #[serde(alias = "characterIndex")]
        character_index: i32,
    },
    MagicKey {
        spell: String,
        key: u8,
        #[serde(alias = "oldKey", default)]
        old_key: u8,
    },
    Magic {
        #[serde(alias = "objectId", default)]
        object_id: u32,
        spell: String,
        direction: String,
        #[serde(alias = "targetId", default)]
        target_id: u32,
        #[serde(default)]
        x: i32,
        #[serde(default)]
        y: i32,
        #[serde(alias = "spellTargetLock", default)]
        spell_target_lock: bool,
    },
    SpellToggle {
        spell: String,
        #[serde(alias = "toggleState", default)]
        toggle_state: Option<i8>,
        #[serde(alias = "canUse", default)]
        can_use: Option<bool>,
    },
    SetHeroBehaviour {
        behaviour: u8,
    },
    ChangeHero {
        #[serde(alias = "listIndex")]
        list_index: i32,
    },
    // Crystal `C.SetAutoPotValue` (Shared/ClientPackets.cs:1221). Sets the
    // hero auto-potion HP/MP trigger percentage. `stat` is the Crystal `Stat`
    // byte (0 = HP, otherwise MP), matching MirConnection.SetAutoPotValue.
    SetAutoPotValue {
        stat: u8,
        value: u32,
    },
    // Crystal `C.SetAutoPotItem` (Shared/ClientPackets.cs:1240). Selects which
    // inventory item the hero auto-pots from. `grid` is the Crystal
    // `MirGridType` (HeroHpItem = 23, HeroMpItem = 24).
    SetAutoPotItem {
        grid: String,
        #[serde(alias = "itemIndex")]
        item_index: i32,
    },
    // Crystal `C.ChangeAMode` (Shared/ClientPackets.cs:756). Cycles the
    // player's attack mode (Peace/Group/Guild/etc.); server echoes
    // `S.ChangeAMode` (MirConnection.ChangeAMode, MirConnection.cs:1432).
    ChangeAMode {
        mode: u8,
    },
    // Crystal `C.ChangePMode` (Shared/ClientPackets.cs:771). Cycles the
    // player's pet command mode; server echoes `S.ChangePMode`
    // (MirConnection.ChangePMode, MirConnection.cs:1440).
    ChangePMode {
        mode: u8,
    },
    // Crystal `C.Opendoor` (Shared/ClientPackets.cs:2524). Opens a map door /
    // conquest siege gate by index; server runs Map.OpenDoor (auto-close after
    // 5s) and toggles conquest gate bookkeeping (PlayerObject @GATES, Gate.cs).
    OpenDoor {
        #[serde(alias = "doorIndex")]
        door_index: u8,
    },
    ConsignItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        price: u32,
        #[serde(alias = "marketType", default)]
        market_type: u8,
    },
    MarketSearch {
        #[serde(alias = "matchText")]
        match_text: String,
        #[serde(alias = "itemType", default)]
        item_type: u8,
        #[serde(alias = "userMode", default)]
        user_mode: bool,
        #[serde(alias = "minShape", default)]
        min_shape: i16,
        #[serde(alias = "maxShape", default = "default_market_max_shape")]
        max_shape: i16,
        #[serde(alias = "marketType", default)]
        market_type: u8,
    },
    MarketRefresh,
    MarketPage {
        page: i32,
    },
    MarketBuy {
        #[serde(alias = "auctionId")]
        auction_id: u64,
        #[serde(alias = "bidPrice", default)]
        bid_price: u32,
    },
    MarketGetBack {
        mode: u8,
        #[serde(alias = "auctionId")]
        auction_id: u64,
    },
    MarketSellNow {
        #[serde(alias = "auctionId")]
        auction_id: u64,
    },
    MarriageRequest,
    MarriageReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    ChangeMarriage,
    DivorceRequest,
    DivorceReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    AddMentor {
        name: String,
    },
    MentorReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    AllowMentor,
    CancelMentor,
    DepositTradeItem {
        from: i32,
        to: i32,
    },
    RetrieveTradeItem {
        from: i32,
        to: i32,
    },
    TradeRequest,
    TradeReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    TradeGold {
        amount: u32,
    },
    TradeConfirm {
        locked: bool,
    },
    TradeCancel,
    FishingCast {
        #[serde(alias = "castOut")]
        cast_out: bool,
    },
    FishingChangeAutocast {
        #[serde(alias = "autoCast")]
        auto_cast: bool,
    },
    SendMail {
        name: String,
        message: String,
        gold: u32,
        #[serde(alias = "itemsIdx")]
        items_idx: [u64; 5],
        stamped: bool,
    },
    ReadMail {
        #[serde(alias = "mailId")]
        mail_id: u64,
    },
    CollectParcel {
        #[serde(alias = "mailId")]
        mail_id: u64,
    },
    DeleteMail {
        #[serde(alias = "mailId")]
        mail_id: u64,
    },
    LockMail {
        #[serde(alias = "mailId")]
        mail_id: u64,
        lock: bool,
    },
    MailLockedItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        locked: bool,
    },
    MailCost {
        gold: u32,
        #[serde(alias = "itemsIdx")]
        items_idx: [u64; 5],
        stamped: bool,
    },
    UpdateIntelligentCreature {
        creature: ClientIntelligentCreature,
        #[serde(alias = "summonMe")]
        summon_me: bool,
        #[serde(alias = "unsummonMe")]
        unsummon_me: bool,
        #[serde(alias = "releaseMe")]
        release_me: bool,
    },
    IntelligentCreaturePickup {
        #[serde(alias = "mouseMode")]
        mouse_mode: bool,
        location: Point,
    },
    RequestIntelligentCreatureUpdates {
        update: bool,
    },
    AddFriend {
        name: String,
        blocked: bool,
    },
    RemoveFriend {
        #[serde(alias = "characterIndex")]
        character_index: i32,
    },
    RefreshFriends,
    AddMemo {
        #[serde(alias = "characterIndex")]
        character_index: i32,
        memo: String,
    },
    GetRanking {
        #[serde(alias = "rankType")]
        rank_type: u8,
        #[serde(alias = "rankIndex", default)]
        rank_index: i32,
        #[serde(alias = "onlineOnly", default)]
        online_only: bool,
    },
    GetRentedItems,
    ItemRentalRequest,
    ItemRentalFee {
        amount: u32,
    },
    ItemRentalPeriod {
        days: u32,
    },
    DepositRentalItem {
        from: i32,
        to: i32,
    },
    RetrieveRentalItem {
        from: i32,
        to: i32,
    },
    CancelItemRental,
    ItemRentalLockFee,
    ItemRentalLockItem,
    ConfirmItemRental,
    AcceptQuest {
        #[serde(alias = "npcIndex", default)]
        npc_index: u32,
        #[serde(alias = "questIndex")]
        quest_index: i32,
    },
    FinishQuest {
        #[serde(alias = "questIndex")]
        quest_index: i32,
        #[serde(alias = "selectedItemIndex", default = "default_selected_item_index")]
        selected_item_index: i32,
    },
    AbandonQuest {
        #[serde(alias = "questIndex")]
        quest_index: i32,
    },
    ShareQuest {
        #[serde(alias = "questIndex")]
        quest_index: i32,
    },
    SwitchGroup {
        #[serde(alias = "allowGroup")]
        allow_group: bool,
    },
    AddMember {
        name: String,
    },
    DelMember {
        name: String,
    },
    GroupInvite {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    EditGuildMember {
        #[serde(alias = "changeType")]
        change_type: u8,
        #[serde(alias = "rankIndex", default)]
        rank_index: u8,
        #[serde(default)]
        name: String,
        #[serde(alias = "rankName", default)]
        rank_name: String,
    },
    EditGuildNotice {
        #[serde(default)]
        notice: Vec<String>,
    },
    GuildInvite {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    GuildNameReturn {
        name: String,
    },
    RequestGuildInfo {
        #[serde(alias = "infoType", default)]
        info_type: u8,
    },
    GuildStorageGoldChange {
        #[serde(alias = "changeType")]
        change_type: u8,
        amount: u32,
    },
    GuildStorageItemChange {
        #[serde(alias = "changeType")]
        change_type: u8,
        from: i32,
        to: i32,
    },
    CastSkill {
        key: String,
    },
    TransferMap {
        key: String,
    },
    Stage5Command {
        action: String,
        #[serde(default)]
        args: Vec<String>,
    },
    QaControl {
        token: String,
        action: QaControlAction,
    },
    SetLanguage {
        language: String,
    },
    Tick,
    LogOut,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeResumeRequest {
    credential: ResumeCredential,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum QaControlAction {
    TransferMap {
        key: String,
    },
    Stage5Command {
        action: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Chat {
        message: String,
    },
    Tick,
}

#[derive(Debug, Clone)]
enum SessionAction {
    Packet(ClientPacket),
    GameShopBuy {
        request_id: Option<String>,
        g_index: i32,
        quantity: u8,
        price_type: i32,
    },
    PasskeyLogin {
        account_id: String,
        proof_account_id: String,
        token: String,
    },
    MoveTo {
        x: i32,
        y: i32,
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
    QaControl {
        token: String,
        action: QaControlAction,
    },
    SetLanguage {
        language: String,
    },
    Tick,
}

/// Default `selected_item_index` for `FinishQuest`: `-1` means "no reward
/// choice", which the simulation maps to `None` (see `stage5_finish_quest_packet`).
fn default_selected_item_index() -> i32 {
    -1
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    http: &'static str,
    ws: &'static str,
    tcp_stub: &'static str,
    gate15: Option<crate::gate15::Gate15Health>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<String>,
    session_cache: GatewaySessionCacheStatus,
    capacity: GatewayCapacityStatus,
    gameplay_events: GameplayEventSinkStatus,
    spectator: crate::spectator::SpectatorMetrics,
    ai_live: crate::ai_live::AiLiveMetrics,
    channel_identity: ChannelIdentityRegistryStatus,
    identity_backend: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityOverviewResponse {
    account_id: String,
    current_session_id: String,
    sessions: Vec<crate::identity::IdentitySessionView>,
    credentials: Vec<crate::identity::IdentityCredentialView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityRevokeSessionRequest {
    session_id: String,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityRevokeCredentialRequest {
    credential_id: String,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityBindSuiCredentialRequest {
    address: String,
    proof_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityRecoverRequest {
    account_id: String,
    recovery_code: String,
    new_password: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityMutationReceipt {
    accepted: bool,
    affected: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityRecoveryCodesResponse {
    recovery_codes: Vec<String>,
    warning: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminIdentityQuery {
    account_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminIdentityRevokeRequest {
    account_id: String,
    session_id: Option<String>,
    reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminIdentityResponse {
    source: &'static str,
    account_id: String,
    sessions: Vec<crate::identity::IdentitySessionView>,
    credentials: Vec<crate::identity::IdentityCredentialView>,
    audit_events: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminSystemMailRequest {
    target_kind: Stage5MailTargetKind,
    target_id: String,
    from: String,
    subject: String,
    body: String,
    #[serde(default)]
    gold: u32,
    #[serde(default)]
    items: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminKickPlayerRequest {
    character_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminKickPlayerReceipt {
    character_id: String,
    removed: bool,
    account_id: Option<String>,
    character_index: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminSessionsResponse {
    source: String,
    sessions: Vec<GatewaySessionCacheRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminSessionTraceQuery {
    account_id: String,
    character_index: i32,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminSessionTraceResponse {
    source: String,
    generated_at_ms: u64,
    status: String,
    current: Option<GatewaySessionCacheRecord>,
    events: Vec<GatewaySessionTraceEvent>,
    commonware: Option<crate::gate15::Gate15PlayerLease>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminControlRequest {
    action: String,
    target: Option<String>,
    operator_id: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminControlReceipt {
    action: String,
    target: Option<String>,
    accepted: bool,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChannelSessionExchangeRequest {
    provider: String,
    credential: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelSessionExchangeResponse {
    account_id: String,
    player_id: String,
    provider: String,
    token: String,
    expires_at: u64,
    created: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChannelIdentityLinkRequest {
    account_id: String,
    session_token: String,
    provider: String,
    credential: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelIdentityLinkResponse {
    account_id: String,
    linked_provider: String,
    identity_count: usize,
}

struct VerifiedChannelSubject {
    subject: String,
    token_id: Option<String>,
    expires_at_ms: Option<u64>,
}

pub async fn run_web_gateway(
    addr: &str,
    config: GatewayConfig,
    chat_hub: ChatBroadcastHub,
) -> io::Result<()> {
    // Activated Crystal world: host every map full-size in the shared zone so
    // players roam all of Bichon (and reach every transfer), not just the
    // starter slice. Empty maps stay dormant regardless.
    if config.monster_spawn_source == mir2_simulation::MonsterSpawnSource::CrystalWorld {
        mir2_simulation::set_crystal_full_world_zone_collision(true);
    }
    let topology = ZoneTopology::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let spectator = SpectatorHub::from_env();
    let ai_live = AiLiveHub::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    ai_live.spawn(spectator.clone());
    let channel_identity =
        run_blocking_component_initializer("channel identity", ChannelIdentityRegistry::from_env)
            .await?;
    let identity_database_url = config.account_store_database_url.clone();
    let identity = run_blocking_component_initializer("commercial identity", move || {
        IdentityService::from_env(identity_database_url)
    })
    .await?;
    let state = WebState {
        config: Arc::new(config),
        deploy_revision: deploy_revision_from_env(),
        zone_registry: Arc::new(
            topology
                .zone_registry(crate::zone_lease::default_zone_owner_lease_authority_from_env()),
        ),
        chat_hub,
        session_cache: gateway_session_cache_from_env()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
        reconnect_sessions: Arc::new(ReconnectSessionStore::default()),
        capacity: Arc::new(GatewayCapacityState::from_env()),
        gameplay_event_sink: default_gameplay_event_sink_from_env(),
        identity: Arc::new(identity),
        injector: crate::inject::LiveSessionInjector::default(),
        spectator,
        ai_live,
        channel_identity,
    };

    let app = Router::new()
        .route("/", get(manual_ui))
        .route("/health", get(health))
        .route("/admin/system-mail", post(admin_system_mail))
        .route("/admin/sessions", get(admin_sessions))
        .route("/admin/session-trace", get(admin_session_trace))
        .route(
            "/admin/channel-identities/{player_id}",
            get(admin_channel_identity),
        )
        .route("/admin/identity", get(admin_identity_overview))
        .route("/admin/identity/revoke", post(admin_identity_revoke))
        .route("/admin/kick-player", post(admin_kick_player))
        .route("/admin/control", post(admin_control))
        .route("/v1/identity/me", get(identity_overview))
        .route(
            "/v1/identity/sessions/revoke",
            post(identity_revoke_session),
        )
        .route(
            "/v1/identity/sessions/revoke-others",
            post(identity_revoke_other_sessions),
        )
        .route(
            "/v1/identity/recovery-codes/rotate",
            post(identity_rotate_recovery_codes),
        )
        .route(
            "/v1/identity/credentials/revoke",
            post(identity_revoke_credential),
        )
        .route(
            "/v1/identity/credentials/bind-sui",
            post(identity_bind_sui_credential),
        )
        .route("/v1/identity/recover", post(identity_recover_account))
        .route("/onchain/inject", post(onchain_inject))
        .route(
            "/v1/channels/session/exchange",
            post(channel_session_exchange),
        )
        .route("/v1/channels/identity/link", post(channel_identity_link))
        .route("/spectator/matches", get(spectator_matches))
        .route("/spectator/recordings", get(spectator_recordings))
        .route("/spectator/replay", get(spectator_replay))
        .route("/spectator/metrics", get(spectator_metrics))
        .route("/spectator/ws", get(spectator_ws_upgrade))
        .route("/ai-live/status", get(ai_live_status))
        .route("/ai-live/metrics", get(ai_live_metrics))
        .route("/ai-live/metrics/prometheus", get(ai_live_prometheus))
        .route("/ai-live/control", post(ai_live_control))
        .route(
            "/ai-live/distribution",
            get(ai_distribution_status).post(ai_distribution_control),
        )
        .route(
            "/ai-live/distribution/heartbeat",
            post(ai_distribution_heartbeat),
        )
        .route("/ai-live/audio/{clip}", get(ai_live_audio))
        .route("/ws", get(ws_upgrade))
        .with_state(state);

    let listener = TcpListener::bind(addr).await?;
    eprintln!("mir2-gateway web listening on http://{addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
}

async fn run_blocking_component_initializer<T>(
    component: &'static str,
    initializer: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> io::Result<T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(initializer)
        .await
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("{component} initialization task failed: {error}"),
            )
        })?
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

async fn manual_ui() -> Html<&'static str> {
    Html(include_str!("../static/manual.html"))
}

fn deploy_revision_from_env() -> Option<String> {
    env::var("MIR2_DEPLOY_REVISION")
        .ok()
        .map(|revision| revision.trim().to_owned())
        .filter(|revision| !revision.is_empty())
}

async fn health(State(state): State<WebState>) -> Json<HealthResponse> {
    state.reconnect_sessions.purge_expired();
    let session_cache = Arc::clone(&state.session_cache);
    let session_cache_status =
        tokio::task::spawn_blocking(move || gateway_session_cache_status(session_cache.as_ref()))
            .await
            .unwrap_or_else(|error| GatewaySessionCacheStatus {
                configured: true,
                backend: "unknown".to_string(),
                ttl_seconds: None,
                record_count: 0,
                stale_record_count: 0,
                route_lease_count: 0,
                healthy: false,
                last_error: Some(format!("session-cache health task failed: {error}")),
            });
    let channel_identity = state.channel_identity.clone();
    let channel_identity_status = tokio::task::spawn_blocking(move || channel_identity.status())
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(ChannelIdentityRegistryStatus {
            backend: "error".to_string(),
            durable: false,
            account_count: 0,
            identity_count: 0,
        });
    let channel_identity_healthy = channel_identity_status.backend != "error";
    Json(HealthResponse {
        ok: channel_identity_healthy,
        http: "ready",
        ws: "ready",
        tcp_stub: "ready",
        gate15: crate::gate15::health(),
        revision: state.deploy_revision.clone(),
        session_cache: session_cache_status,
        capacity: state.capacity.status(),
        gameplay_events: gameplay_event_sink_status(state.gameplay_event_sink.as_ref()),
        spectator: state.spectator.metrics(),
        ai_live: state.ai_live.metrics(),
        channel_identity: channel_identity_status,
        identity_backend: state.identity.backend_label(),
    })
}

async fn channel_session_exchange(
    State(state): State<WebState>,
    Json(request): Json<ChannelSessionExchangeRequest>,
) -> Result<Json<ChannelSessionExchangeResponse>, (StatusCode, Json<AdminErrorResponse>)> {
    let _login_permit = state
        .capacity
        .try_acquire_action(GatewayCapacityKind::Login)
        .map_err(|error| channel_exchange_error(StatusCode::TOO_MANY_REQUESTS, error))?;
    let provider = ChannelIdentityProvider::parse(&request.provider)
        .map_err(|error| channel_exchange_error(StatusCode::BAD_REQUEST, error))?;
    let verified = verified_channel_subject(provider, &request.credential).await?;
    consume_channel_subject_proof(state.session_cache.as_ref(), &verified)?;
    let subject = verified.subject;
    let credential_subject = subject.clone();

    let registry = state.channel_identity.clone();
    let (account, created) = tokio::task::spawn_blocking(move || {
        registry.resolve_or_create_with_outcome(provider, &subject)
    })
    .await
    .map_err(|error| {
        channel_exchange_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("channel identity task failed: {error}"),
        )
    })?
    .map_err(|error| channel_exchange_error(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    let expires_at = gateway_now_ms().saturating_add(channel_session_token_ttl_ms());
    let token = issue_gateway_identity_token_for_subject(
        &account.player_id,
        provider.as_str(),
        Some(&credential_subject),
        expires_at,
    )
    .map_err(|error| channel_exchange_error(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    Ok(Json(ChannelSessionExchangeResponse {
        account_id: account.player_id.clone(),
        player_id: account.player_id,
        provider: provider.as_str().to_string(),
        token,
        expires_at,
        created,
    }))
}

async fn channel_identity_link(
    State(state): State<WebState>,
    Json(request): Json<ChannelIdentityLinkRequest>,
) -> Result<Json<ChannelIdentityLinkResponse>, (StatusCode, Json<AdminErrorResponse>)> {
    let _login_permit = state
        .capacity
        .try_acquire_action(GatewayCapacityKind::Login)
        .map_err(|error| channel_exchange_error(StatusCode::TOO_MANY_REQUESTS, error))?;
    verify_gateway_identity_token(&request.account_id, &request.session_token)
        .map_err(|error| channel_exchange_error(StatusCode::UNAUTHORIZED, error))?;
    let provider = ChannelIdentityProvider::parse(&request.provider)
        .map_err(|error| channel_exchange_error(StatusCode::BAD_REQUEST, error))?;
    if !provider.is_primary_capable() {
        return Err(channel_exchange_error(
            StatusCode::BAD_REQUEST,
            "guest channel identities cannot be linked as ownership credentials".to_string(),
        ));
    }
    let verified = verified_channel_subject(provider, &request.credential).await?;
    consume_channel_subject_proof(state.session_cache.as_ref(), &verified)?;
    let subject = verified.subject;
    let registry = state.channel_identity.clone();
    let player_id = request.account_id.clone();
    let account =
        tokio::task::spawn_blocking(move || registry.link_identity(&player_id, provider, &subject))
            .await
            .map_err(|error| {
                channel_exchange_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("channel identity task failed: {error}"),
                )
            })?
            .map_err(|error| {
                let status = if error.contains("another player") {
                    StatusCode::CONFLICT
                } else if error.contains("not found") {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };
                channel_exchange_error(status, error)
            })?;
    Ok(Json(ChannelIdentityLinkResponse {
        account_id: account.player_id,
        linked_provider: provider.as_str().to_string(),
        identity_count: account.identities.len(),
    }))
}

async fn admin_channel_identity(
    State(state): State<WebState>,
    AxumPath(player_id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<PlayerIdentityAccount>, (StatusCode, Json<AdminErrorResponse>)> {
    let token = bearer_token(&headers).ok_or_else(|| {
        channel_exchange_error(
            StatusCode::UNAUTHORIZED,
            "missing operator bearer token".to_string(),
        )
    })?;
    verify_operator_token(token)
        .map_err(|error| channel_exchange_error(StatusCode::UNAUTHORIZED, error))?;
    let registry = state.channel_identity.clone();
    let account = tokio::task::spawn_blocking(move || registry.account(&player_id))
        .await
        .map_err(|error| {
            channel_exchange_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("channel identity task failed: {error}"),
            )
        })?
        .map_err(|error| channel_exchange_error(StatusCode::BAD_REQUEST, error))?
        .ok_or_else(|| {
            channel_exchange_error(
                StatusCode::NOT_FOUND,
                "Obelisk player identity was not found".to_string(),
            )
        })?;
    Ok(Json(account))
}

async fn verified_channel_subject(
    provider: ChannelIdentityProvider,
    credential: &str,
) -> Result<VerifiedChannelSubject, (StatusCode, Json<AdminErrorResponse>)> {
    if credential.is_empty() || credential.len() > 64 * 1024 {
        return Err(channel_exchange_error(
            StatusCode::BAD_REQUEST,
            "channel credential size is invalid".to_string(),
        ));
    }
    match provider {
        ChannelIdentityProvider::SuiPasskey | ChannelIdentityProvider::SuiWallet => {
            let proof = verify_sui_login_proof(credential)
                .map_err(|error| channel_exchange_error(StatusCode::UNAUTHORIZED, error))?;
            if proof.provider != provider.as_str() {
                return Err(channel_exchange_error(
                    StatusCode::UNAUTHORIZED,
                    "Sui login proof provider mismatch".to_string(),
                ));
            }
            Ok(VerifiedChannelSubject {
                subject: proof.subject,
                token_id: proof.token_id,
                expires_at_ms: Some(proof.expires_at_ms),
            })
        }
        ChannelIdentityProvider::CrazyGames => Ok(VerifiedChannelSubject {
            subject: verify_crazygames_token(credential)
                .await
                .map_err(|error| channel_exchange_error(StatusCode::UNAUTHORIZED, error))?
                .user_id,
            token_id: None,
            expires_at_ms: None,
        }),
        ChannelIdentityProvider::Itch
        | ChannelIdentityProvider::DirectGuest
        | ChannelIdentityProvider::CrazyGamesGuest => {
            let proof = verify_channel_guest_proof(credential)
                .map_err(|error| channel_exchange_error(StatusCode::UNAUTHORIZED, error))?;
            if proof.provider != provider.as_str() {
                return Err(channel_exchange_error(
                    StatusCode::UNAUTHORIZED,
                    "channel guest proof provider mismatch".to_string(),
                ));
            }
            Ok(VerifiedChannelSubject {
                subject: proof.subject,
                token_id: None,
                expires_at_ms: Some(proof.exp_ms),
            })
        }
    }
}

fn consume_channel_subject_proof(
    session_cache: &dyn crate::GatewaySessionCache,
    proof: &VerifiedChannelSubject,
) -> Result<(), (StatusCode, Json<AdminErrorResponse>)> {
    let Some(token_id) = proof.token_id.as_deref() else {
        return Ok(());
    };
    let ttl_seconds = proof
        .expires_at_ms
        .unwrap_or_else(gateway_now_ms)
        .saturating_sub(gateway_now_ms())
        .saturating_add(999)
        / 1_000;
    match session_cache
        .consume_auth_token(token_id, ttl_seconds.max(1))
        .map_err(|error| channel_exchange_error(StatusCode::SERVICE_UNAVAILABLE, error))?
    {
        true => Ok(()),
        false => Err(channel_exchange_error(
            StatusCode::UNAUTHORIZED,
            "channel identity proof was already used".to_string(),
        )),
    }
}

fn channel_exchange_error(
    status: StatusCode,
    error: String,
) -> (StatusCode, Json<AdminErrorResponse>) {
    (status, Json(AdminErrorResponse { error }))
}

fn channel_session_token_ttl_ms() -> u64 {
    env::var("MIR2_CHANNEL_SESSION_TOKEN_TTL_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(3_600)
        .clamp(60, 43_200)
        .saturating_mul(1_000)
}

fn gateway_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn ai_live_status(State(state): State<WebState>) -> Json<crate::ai_live::AiLiveStatus> {
    Json(state.ai_live.status())
}

async fn ai_distribution_status(
    State(state): State<WebState>,
) -> Json<crate::ai_distribution::AiDistributionStatus> {
    Json(state.ai_live.distribution_status())
}

async fn ai_live_metrics(State(state): State<WebState>) -> Json<crate::ai_live::AiLiveMetrics> {
    Json(state.ai_live.metrics())
}

async fn ai_live_prometheus(State(state): State<WebState>) -> Response {
    let metrics = state.ai_live.metrics();
    let mode = match metrics.mode {
        AiLiveMode::Live => 2,
        AiLiveMode::Shadow => 1,
        AiLiveMode::Paused => 0,
    };
    let body = format!(
        concat!(
            "# HELP mir2_ai_live_mode AI live mode: paused=0 shadow=1 live=2.\n",
            "# TYPE mir2_ai_live_mode gauge\n",
            "mir2_ai_live_mode {mode}\n",
            "# HELP mir2_ai_live_processed_frames_total Sanitized spectator frames inspected.\n",
            "# TYPE mir2_ai_live_processed_frames_total counter\n",
            "mir2_ai_live_processed_frames_total {processed}\n",
            "# HELP mir2_ai_live_generated_segments_total Broadcast segments generated.\n",
            "# TYPE mir2_ai_live_generated_segments_total counter\n",
            "mir2_ai_live_generated_segments_total {segments}\n",
            "# HELP mir2_ai_live_model_failures_total Model failures handled by fallback.\n",
            "# TYPE mir2_ai_live_model_failures_total counter\n",
            "mir2_ai_live_model_failures_total {model_failures}\n",
            "# HELP mir2_ai_live_tts_failures_total TTS failures handled without blocking broadcast.\n",
            "# TYPE mir2_ai_live_tts_failures_total counter\n",
            "mir2_ai_live_tts_failures_total {tts_failures}\n",
            "# HELP mir2_ai_distribution_delivered_total Successful channel deliveries.\n",
            "# TYPE mir2_ai_distribution_delivered_total counter\n",
            "mir2_ai_distribution_delivered_total {distribution_delivered}\n",
            "# HELP mir2_ai_distribution_failures_total Failed channel delivery attempts.\n",
            "# TYPE mir2_ai_distribution_failures_total counter\n",
            "mir2_ai_distribution_failures_total {distribution_failures}\n",
            "# HELP mir2_ai_distribution_queue Pending channel-neutral delivery jobs.\n",
            "# TYPE mir2_ai_distribution_queue gauge\n",
            "mir2_ai_distribution_queue {distribution_queue}\n",
            "# HELP mir2_ai_distribution_dead_letters_total Exhausted channel deliveries.\n",
            "# TYPE mir2_ai_distribution_dead_letters_total counter\n",
            "mir2_ai_distribution_dead_letters_total {distribution_dead_letters}\n",
            "# HELP mir2_ai_live_discord_queue Pending Discord highlight deliveries.\n",
            "# TYPE mir2_ai_live_discord_queue gauge\n",
            "mir2_ai_live_discord_queue {discord_queue}\n",
            "# HELP mir2_ai_live_discord_dead_letters_total Exhausted Discord deliveries.\n",
            "# TYPE mir2_ai_live_discord_dead_letters_total counter\n",
            "mir2_ai_live_discord_dead_letters_total {discord_dead_letters}\n"
        ),
        mode = mode,
        processed = metrics.processed_frames_total,
        segments = metrics.generated_segments_total,
        model_failures = metrics.model_failure_total,
        tts_failures = metrics.tts_failure_total,
        distribution_delivered = metrics.distribution_success_total,
        distribution_failures = metrics.distribution_failure_total,
        distribution_queue = metrics.queued_distribution_deliveries,
        distribution_dead_letters = metrics.distribution_dead_letters_total,
        discord_queue = metrics.queued_discord_deliveries,
        discord_dead_letters = metrics.discord_dead_letters_total,
    );
    (
        StatusCode::OK,
        [(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn ai_distribution_control(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<AiDistributionControlRequest>,
) -> Result<
    Json<crate::ai_distribution::AiDistributionStatus>,
    (StatusCode, Json<AdminErrorResponse>),
> {
    let channel = crate::ai_distribution::AiDistributionChannel::parse(&request.channel)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(AdminErrorResponse {
                    error: "unsupported AI distribution channel".to_string(),
                }),
            )
        })?;
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);
    let token = request.token.as_deref().or(bearer);
    let result = match request.action.trim().to_ascii_lowercase().as_str() {
        "enable" | "start" => state.ai_live.set_distribution_channel(token, channel, true),
        "disable" | "pause" | "stop" => state
            .ai_live
            .set_distribution_channel(token, channel, false),
        "retry" => state.ai_live.retry_distribution_channel(token, channel),
        _ => Err("action must be enable, disable, or retry".to_string()),
    };
    result.map(Json).map_err(|error| {
        let status = if error.contains("token") {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::BAD_REQUEST
        };
        (status, Json(AdminErrorResponse { error }))
    })
}

async fn ai_distribution_heartbeat(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<crate::ai_distribution::AiRuntimeHeartbeatRequest>,
) -> Result<
    Json<crate::ai_distribution::AiDistributionStatus>,
    (StatusCode, Json<AdminErrorResponse>),
> {
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);
    state
        .ai_live
        .record_distribution_runtime_heartbeat(bearer, request)
        .map(Json)
        .map_err(|error| {
            let status = if error.contains("token") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(AdminErrorResponse { error }))
        })
}

async fn ai_live_control(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<AiLiveControlRequest>,
) -> Result<Json<crate::ai_live::AiLiveStatus>, (StatusCode, Json<AdminErrorResponse>)> {
    let mode = match request.action.trim().to_ascii_lowercase().as_str() {
        "live" | "start" => AiLiveMode::Live,
        "shadow" => AiLiveMode::Shadow,
        "pause" | "paused" | "stop" => AiLiveMode::Paused,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(AdminErrorResponse {
                    error: "action must be live, shadow, or pause".to_string(),
                }),
            ));
        }
    };
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);
    state
        .ai_live
        .set_mode(request.token.as_deref().or(bearer), mode)
        .map(Json)
        .map_err(|error| (StatusCode::FORBIDDEN, Json(AdminErrorResponse { error })))
}

async fn ai_live_audio(
    State(state): State<WebState>,
    AxumPath(clip): AxumPath<String>,
) -> Response {
    let path = match state.ai_live.audio_path(&clip) {
        Ok(path) => path,
        Err(error) => {
            return (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })).into_response();
        }
    };
    let result = tokio::task::spawn_blocking(move || std::fs::read(path)).await;
    match result {
        Ok(Ok(bytes)) => (
            StatusCode::OK,
            [
                (CONTENT_TYPE, "audio/mpeg"),
                (CACHE_CONTROL, "public, max-age=86400, immutable"),
            ],
            bytes,
        )
            .into_response(),
        Ok(Err(error)) if error.kind() == io::ErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            Json(AdminErrorResponse {
                error: "AI live audio clip not found".to_string(),
            }),
        )
            .into_response(),
        Ok(Err(error)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AdminErrorResponse {
                error: format!("read AI live audio clip failed: {error}"),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AdminErrorResponse {
                error: format!("AI live audio task failed: {error}"),
            }),
        )
            .into_response(),
    }
}

async fn identity_overview(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> Result<Json<IdentityOverviewResponse>, (StatusCode, Json<AdminErrorResponse>)> {
    let identity = Arc::clone(&state.identity);
    let token = identity_bearer_token(&headers)?;
    tokio::task::spawn_blocking(move || {
        let verified = identity
            .verify_session_token(&token)
            .map_err(identity_unauthorized)?;
        let sessions = identity
            .list_sessions(&verified)
            .map_err(identity_unavailable)?;
        let credentials = identity
            .list_credentials(&verified)
            .map_err(identity_unavailable)?;
        Ok(Json(IdentityOverviewResponse {
            account_id: verified.account_id,
            current_session_id: verified.session_id,
            sessions,
            credentials,
        }))
    })
    .await
    .map_err(|error| identity_unavailable(format!("identity task failed: {error}")))?
}

async fn identity_revoke_session(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<IdentityRevokeSessionRequest>,
) -> Result<Json<IdentityMutationReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let identity = Arc::clone(&state.identity);
    let session_cache = Arc::clone(&state.session_cache);
    let reconnect_sessions = Arc::clone(&state.reconnect_sessions);
    let token = identity_bearer_token(&headers)?;
    tokio::task::spawn_blocking(move || {
        let verified = identity
            .verify_session_token(&token)
            .map_err(identity_unauthorized)?;
        let _revocation_fence =
            reconnect_sessions.begin_identity_session_revocation(&request.session_id);
        let changed = identity
            .revoke_session(&verified, &request.session_id, &request.reason)
            .map_err(identity_unavailable)?;
        if changed {
            session_cache
                .revoke_identity_session(&request.session_id, 30 * 24 * 60 * 60)
                .map_err(identity_unavailable)?;
        }
        Ok(Json(IdentityMutationReceipt {
            accepted: changed,
            affected: u64::from(changed),
        }))
    })
    .await
    .map_err(|error| identity_unavailable(format!("identity task failed: {error}")))?
}

async fn identity_revoke_other_sessions(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> Result<Json<IdentityMutationReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let identity = Arc::clone(&state.identity);
    let session_cache = Arc::clone(&state.session_cache);
    let reconnect_sessions = Arc::clone(&state.reconnect_sessions);
    let token = identity_bearer_token(&headers)?;
    tokio::task::spawn_blocking(move || {
        let verified = identity
            .verify_session_token(&token)
            .map_err(identity_unauthorized)?;
        let target_sessions = identity
            .list_sessions(&verified)
            .map_err(identity_unavailable)?
            .into_iter()
            .filter(|session| {
                session.session_id != verified.session_id && session.revoked_at_ms.is_none()
            })
            .map(|session| session.session_id)
            .collect::<Vec<_>>();
        let _revocation_fence =
            reconnect_sessions.begin_identity_session_revocations(target_sessions.clone());
        let affected = identity
            .revoke_all_other_sessions(&verified)
            .map_err(identity_unavailable)?;
        for session_id in target_sessions {
            session_cache
                .revoke_identity_session(&session_id, 30 * 24 * 60 * 60)
                .map_err(identity_unavailable)?;
        }
        Ok(Json(IdentityMutationReceipt {
            accepted: true,
            affected,
        }))
    })
    .await
    .map_err(|error| identity_unavailable(format!("identity task failed: {error}")))?
}

async fn identity_rotate_recovery_codes(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> Result<Json<IdentityRecoveryCodesResponse>, (StatusCode, Json<AdminErrorResponse>)> {
    let identity = Arc::clone(&state.identity);
    let token = identity_bearer_token(&headers)?;
    tokio::task::spawn_blocking(move || {
        let verified = identity
            .verify_session_token(&token)
            .map_err(identity_unauthorized)?;
        let recovery_codes = identity
            .generate_recovery_codes(&verified)
            .map_err(identity_bad_request)?;
        Ok(Json(IdentityRecoveryCodesResponse {
            recovery_codes,
            warning: "These recovery codes are shown once. Store them offline.",
        }))
    })
    .await
    .map_err(|error| identity_unavailable(format!("identity task failed: {error}")))?
}

async fn identity_revoke_credential(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<IdentityRevokeCredentialRequest>,
) -> Result<Json<IdentityMutationReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let identity = Arc::clone(&state.identity);
    let session_cache = Arc::clone(&state.session_cache);
    let reconnect_sessions = Arc::clone(&state.reconnect_sessions);
    let token = identity_bearer_token(&headers)?;
    tokio::task::spawn_blocking(move || {
        let verified = identity
            .verify_session_token(&token)
            .map_err(identity_unauthorized)?;
        let target_sessions = identity
            .list_sessions(&verified)
            .map_err(identity_unavailable)?
            .into_iter()
            .filter(|session| {
                session.credential_id.as_deref() == Some(request.credential_id.as_str())
            })
            .map(|session| session.session_id)
            .collect::<Vec<_>>();
        let _revocation_fence =
            reconnect_sessions.begin_identity_session_revocations(target_sessions.clone());
        let changed = identity
            .revoke_credential(&verified, &request.credential_id, &request.reason)
            .map_err(identity_bad_request)?;
        if changed {
            for session_id in target_sessions {
                session_cache
                    .revoke_identity_session(&session_id, 30 * 24 * 60 * 60)
                    .map_err(identity_unavailable)?;
            }
        }
        Ok(Json(IdentityMutationReceipt {
            accepted: changed,
            affected: u64::from(changed),
        }))
    })
    .await
    .map_err(|error| identity_unavailable(format!("identity task failed: {error}")))?
}

async fn identity_bind_sui_credential(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<IdentityBindSuiCredentialRequest>,
) -> Result<Json<IdentityMutationReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let token = identity_bearer_token(&headers)?;
    let proof_account_id = format!("sui:{}", request.address.trim());
    let proof = verify_passkey_gateway_token(&proof_account_id, &request.proof_token)
        .map_err(identity_unauthorized)?;
    let ttl_seconds = proof
        .expires_at_ms
        .saturating_sub(gateway_unix_ms())
        .saturating_add(999)
        / 1_000;
    match state
        .session_cache
        .consume_auth_token(&proof.token_id, ttl_seconds.max(1))
        .map_err(identity_unavailable)?
    {
        true => {}
        false => {
            return Err(identity_unauthorized(
                "Sui credential proof was already used",
            ));
        }
    }
    let identity = Arc::clone(&state.identity);
    tokio::task::spawn_blocking(move || {
        let verified = identity
            .verify_session_token(&token)
            .map_err(identity_unauthorized)?;
        identity
            .bind_sui_credential(&verified, proof.auth_method, request.address.trim())
            .map_err(identity_bad_request)?;
        Ok(Json(IdentityMutationReceipt {
            accepted: true,
            affected: 1,
        }))
    })
    .await
    .map_err(|error| identity_unavailable(format!("identity task failed: {error}")))?
}

async fn identity_recover_account(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<IdentityRecoverRequest>,
) -> Result<Json<IdentityMutationReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let peer_address = trusted_client_address(&headers, peer);
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let context = AuthSecurityContext {
        account_id: request.account_id.clone(),
        action: AuthSecurityAction::Recovery,
    };
    enforce_auth_rate_limits(
        state.session_cache.as_ref(),
        &state.identity,
        &peer_address,
        &user_agent,
        &context,
    )
    .map_err(|_| identity_unauthorized("account recovery could not be completed"))?;
    mir2_simulation::validate_commercial_identity_credentials(
        &request.account_id,
        &request.new_password,
    )
    .map_err(identity_bad_request)?;
    let identity = Arc::clone(&state.identity);
    let session_cache = Arc::clone(&state.session_cache);
    let reconnect_sessions = Arc::clone(&state.reconnect_sessions);
    let config = Arc::clone(&state.config);
    tokio::task::spawn_blocking(move || {
        if !identity
            .consume_recovery_code(&request.account_id, &request.recovery_code)
            .map_err(identity_unavailable)?
        {
            return Err(identity_unauthorized(
                "account recovery could not be completed",
            ));
        }
        let target_sessions = identity
            .list_account_session_ids(&request.account_id)
            .map_err(identity_unavailable)?;
        let _revocation_fence =
            reconnect_sessions.begin_account_identity_revocation(&request.account_id);
        mir2_simulation::reset_account_password_after_recovery(
            config.as_ref(),
            &request.account_id,
            &request.new_password,
        )
        .map_err(identity_unavailable)?;
        identity
            .revoke_all_account_sessions(&request.account_id, "password_recovered")
            .map_err(identity_unavailable)?;
        for session_id in target_sessions {
            session_cache
                .revoke_identity_session(&session_id, 30 * 24 * 60 * 60)
                .map_err(identity_unavailable)?;
        }
        Ok(Json(IdentityMutationReceipt {
            accepted: true,
            affected: 1,
        }))
    })
    .await
    .map_err(|error| identity_unavailable(format!("identity task failed: {error}")))?
}

fn identity_bearer_token(
    headers: &HeaderMap,
) -> Result<String, (StatusCode, Json<AdminErrorResponse>)> {
    bearer_token(headers)
        .map(str::trim)
        .filter(|token| !token.is_empty() && token.len() <= 4096)
        .map(str::to_string)
        .ok_or_else(|| identity_unauthorized("missing identity bearer token"))
}

fn identity_unauthorized(error: impl Into<String>) -> (StatusCode, Json<AdminErrorResponse>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(AdminErrorResponse {
            error: error.into(),
        }),
    )
}

fn identity_bad_request(error: impl Into<String>) -> (StatusCode, Json<AdminErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(AdminErrorResponse {
            error: error.into(),
        }),
    )
}

fn identity_unavailable(error: impl Into<String>) -> (StatusCode, Json<AdminErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(AdminErrorResponse {
            error: error.into(),
        }),
    )
}

async fn admin_system_mail(
    State(state): State<WebState>,
    Json(request): Json<AdminSystemMailRequest>,
) -> Result<Json<mir2_simulation::Stage5MailDeliveryReceipt>, (StatusCode, Json<AdminErrorResponse>)>
{
    let receipt = deliver_stage5_system_mail(
        &state.config,
        Stage5MailDelivery {
            target_kind: request.target_kind,
            target_id: request.target_id,
            from: request.from,
            subject: request.subject,
            body: request.body,
            gold: request.gold,
            items: request.items,
        },
    )
    .map_err(|error| (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })))?;
    Ok(Json(receipt))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OnchainInjectReceipt {
    accepted: bool,
    connected: bool,
    packet_count: usize,
    idempotency_key: String,
}

/// Extract a `Bearer <token>` from the Authorization header.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// M4 WF-5: the trusted Relayer POSTs chain-confirmed commands here. Operator-token
/// authenticated; routed to the target account's LIVE session via the injector. Idempotency
/// place #3 lives in the sim (duplicate `idempotency_key` is a no-op there).
async fn onchain_inject(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(command): Json<crate::inject::OnchainInjectCommand>,
) -> Result<Json<OnchainInjectReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let token = bearer_token(&headers).ok_or((
        StatusCode::UNAUTHORIZED,
        Json(AdminErrorResponse {
            error: "missing operator bearer token".to_string(),
        }),
    ))?;
    crate::auth::verify_operator_token(token)
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(AdminErrorResponse { error })))?;

    let idempotency_key = command.idempotency_key().to_string();
    let world_command = command
        .to_world_command()
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })))?;
    // Render-only (MineDepleted) has no sim WorldCommand — accept + ignore (200) so the
    // relayer's retry loop stops.
    let Some(world_command) = world_command else {
        return Ok(Json(OnchainInjectReceipt {
            accepted: true,
            connected: false,
            packet_count: 0,
            idempotency_key,
        }));
    };
    let Some(account) = command.account().map(str::to_string) else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AdminErrorResponse {
                error: "command has no target account".to_string(),
            }),
        ));
    };
    match state.injector.dispatch(&account, world_command).await {
        Ok(outcome) => Ok(Json(OnchainInjectReceipt {
            accepted: true,
            connected: true,
            packet_count: outcome.packet_count,
            idempotency_key,
        })),
        // Player offline: accept (200) so the relayer stops retrying. NOTE: the grant is NOT
        // persisted for offline players in M4 — offline delivery (persist/mail) is M6.
        Err(crate::inject::InjectionError::NotConnected) => Ok(Json(OnchainInjectReceipt {
            accepted: true,
            connected: false,
            packet_count: 0,
            idempotency_key,
        })),
        Err(crate::inject::InjectionError::SessionGone) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AdminErrorResponse {
                error: "target session became unavailable".to_string(),
            }),
        )),
    }
}

async fn admin_kick_player(
    State(state): State<WebState>,
    Json(request): Json<AdminKickPlayerRequest>,
) -> Result<Json<AdminKickPlayerReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    if request.character_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AdminErrorResponse {
                error: "character_id is required".to_string(),
            }),
        ));
    }
    let removed = state
        .session_cache
        .remove_character(request.character_id.trim());
    Ok(Json(AdminKickPlayerReceipt {
        character_id: request.character_id,
        removed: removed.is_some(),
        account_id: removed.as_ref().map(|record| record.key.account_id.clone()),
        character_index: removed.as_ref().map(|record| record.key.character_index),
    }))
}

async fn admin_sessions(State(state): State<WebState>) -> Json<AdminSessionsResponse> {
    let mut sessions = state.session_cache.list();
    sessions.sort_by(|left, right| {
        left.character_name
            .cmp(&right.character_name)
            .then_with(|| left.key.account_id.cmp(&right.key.account_id))
            .then(left.key.character_index.cmp(&right.key.character_index))
    });
    Json(AdminSessionsResponse {
        source: "gateway_session_cache".to_string(),
        sessions,
    })
}

async fn admin_session_trace(
    State(state): State<WebState>,
    headers: HeaderMap,
    Query(query): Query<AdminSessionTraceQuery>,
) -> Result<Json<AdminSessionTraceResponse>, (StatusCode, Json<AdminErrorResponse>)> {
    require_gateway_admin_trace_token(&headers)?;
    if query.account_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AdminErrorResponse {
                error: "accountId is required".to_string(),
            }),
        ));
    }
    let key = GatewaySessionCacheKey {
        account_id: query.account_id.trim().to_string(),
        character_index: query.character_index,
    };
    let current = state.session_cache.get(&key);
    let events = state
        .session_cache
        .trace_events(&key, query.limit.unwrap_or(64).clamp(1, 128));
    let commonware = current
        .as_ref()
        .and_then(|record| record.zone_id.as_deref())
        .map(|zone_id| {
            crate::gate15::inspect_player_session(
                &key.account_id,
                key.character_index,
                &crate::ZoneId::new(zone_id),
            )
        })
        .transpose()
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AdminErrorResponse { error }),
            )
        })?
        .flatten();
    let now_ms = gateway_unix_ms();
    let status = match current.as_ref() {
        Some(record)
            if record
                .route_lease_expires_at_ms
                .is_some_and(|expires| expires > now_ms) =>
        {
            "online"
        }
        Some(_) => "stale",
        None if events.is_empty() => "not_found",
        None => "offline",
    };
    let reason = match status {
        "not_found" => Some("no current session or retained placement history".to_string()),
        "offline" => Some("session is offline; retained history is available".to_string()),
        "stale" => Some("session cache exists but its route lease is not live".to_string()),
        _ => None,
    };
    Ok(Json(AdminSessionTraceResponse {
        source: "gateway_session_trace".to_string(),
        generated_at_ms: now_ms,
        status: status.to_string(),
        current,
        events,
        commonware,
        reason,
    }))
}

async fn admin_identity_overview(
    State(state): State<WebState>,
    headers: HeaderMap,
    Query(query): Query<AdminIdentityQuery>,
) -> Result<Json<AdminIdentityResponse>, (StatusCode, Json<AdminErrorResponse>)> {
    require_gateway_admin_trace_token(&headers)?;
    let account_id = query.account_id.trim().to_string();
    if account_id.is_empty() || account_id.len() > 160 {
        return Err(identity_bad_request("accountId is required"));
    }
    let identity = Arc::clone(&state.identity);
    tokio::task::spawn_blocking(move || {
        let (sessions, credentials, audit_events) = identity
            .operator_account_security(&account_id)
            .map_err(identity_unavailable)?;
        Ok(Json(AdminIdentityResponse {
            source: "commercial_identity",
            account_id,
            sessions,
            credentials,
            audit_events,
        }))
    })
    .await
    .map_err(|error| identity_unavailable(format!("identity task failed: {error}")))?
}

async fn admin_identity_revoke(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<AdminIdentityRevokeRequest>,
) -> Result<Json<IdentityMutationReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    require_gateway_admin_trace_token(&headers)?;
    let account_id = request.account_id.trim().to_string();
    let reason = request.reason.trim().to_string();
    if account_id.is_empty() || account_id.len() > 160 || reason.len() < 4 || reason.len() > 160 {
        return Err(identity_bad_request(
            "accountId and a reason of 4-160 characters are required",
        ));
    }
    let identity = Arc::clone(&state.identity);
    let session_cache = Arc::clone(&state.session_cache);
    let reconnect_sessions = Arc::clone(&state.reconnect_sessions);
    tokio::task::spawn_blocking(move || {
        let (affected, targets) = if let Some(session_id) = request
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let _revocation_fence =
                reconnect_sessions.begin_identity_session_revocation(session_id);
            let operator = VerifiedIdentitySession {
                account_id: account_id.clone(),
                session_id: String::new(),
                expires_at_ms: u64::MAX,
            };
            let changed = identity
                .revoke_session(&operator, session_id, &reason)
                .map_err(identity_unavailable)?;
            (
                u64::from(changed),
                if changed {
                    vec![session_id.to_string()]
                } else {
                    Vec::new()
                },
            )
        } else {
            let targets = identity
                .list_account_session_ids(&account_id)
                .map_err(identity_unavailable)?;
            let _revocation_fence =
                reconnect_sessions.begin_account_identity_revocation(&account_id);
            let affected = identity
                .revoke_all_account_sessions(&account_id, &reason)
                .map_err(identity_unavailable)?;
            (affected, targets)
        };
        for session_id in targets {
            session_cache
                .revoke_identity_session(&session_id, 30 * 24 * 60 * 60)
                .map_err(identity_unavailable)?;
        }
        Ok(Json(IdentityMutationReceipt {
            accepted: true,
            affected,
        }))
    })
    .await
    .map_err(|error| identity_unavailable(format!("identity task failed: {error}")))?
}

fn require_gateway_admin_trace_token(
    headers: &HeaderMap,
) -> Result<(), (StatusCode, Json<AdminErrorResponse>)> {
    let expected = env::var("MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if gateway_prod_like_env() && expected.as_ref().is_some_and(|value| value.len() < 32) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AdminErrorResponse {
                error: "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN must contain at least 32 characters in production"
                    .to_string(),
            }),
        ));
    }
    let provided = bearer_token(headers).map(str::trim);
    match (expected.as_deref(), provided) {
        (Some(expected), Some(provided))
            if constant_time_text_eq(expected.as_bytes(), provided.as_bytes()) =>
        {
            Ok(())
        }
        (Some(_), _) => Err((
            StatusCode::UNAUTHORIZED,
            Json(AdminErrorResponse {
                error: "invalid gateway admin operator token".to_string(),
            }),
        )),
        (None, _) if gateway_prod_like_env() => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AdminErrorResponse {
                error: "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN is required in production".to_string(),
            }),
        )),
        (None, _) => Ok(()),
    }
}

fn constant_time_text_eq(left: &[u8], right: &[u8]) -> bool {
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

fn gateway_prod_like_env() -> bool {
    ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"]
        .into_iter()
        .filter_map(|name| env::var(name).ok())
        .any(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "production" | "prod" | "staging"
            )
        })
}

fn bounded_websocket_upgrade(ws: WebSocketUpgrade) -> WebSocketUpgrade {
    ws.max_message_size(WEBSOCKET_MAX_MESSAGE_BYTES)
        .max_frame_size(WEBSOCKET_MAX_FRAME_BYTES)
}

fn validate_websocket_origin(headers: &HeaderMap) -> Result<(), String> {
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let configured = env::var("MIR2_ALLOWED_WEB_ORIGINS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if configured.is_empty() {
        return if gateway_prod_like_env() {
            Err("MIR2_ALLOWED_WEB_ORIGINS is required in production".to_string())
        } else {
            Ok(())
        };
    }
    let Some(origin) = origin else {
        return Err("WebSocket Origin header is required".to_string());
    };
    if configured.iter().any(|allowed| allowed == origin) {
        Ok(())
    } else {
        Err("WebSocket origin is not allowed".to_string())
    }
}

fn trusted_client_address(headers: &HeaderMap, peer: SocketAddr) -> String {
    if env::var("MIR2_TRUST_CF_CONNECTING_IP")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
    {
        if let Some(address) = headers
            .get("cf-connecting-ip")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<IpAddr>().ok())
        {
            return address.to_string();
        }
    }
    peer.ip().to_string()
}

fn gateway_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

async fn admin_control(
    Json(request): Json<AdminControlRequest>,
) -> Result<Json<AdminControlReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let action = canonical_admin_control_action(&request.action)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })))?;
    let message = match action.as_str() {
        "status" => "gateway is ready".to_string(),
        "reload_npcs" => {
            "NPC reload request accepted; generated manifests are active for new sessions"
                .to_string()
        }
        "reload_drops" => {
            "drop reload request accepted; generated manifests are active for new sessions"
                .to_string()
        }
        "reload_line_message" => "line-message reload request accepted".to_string(),
        "clear_blocked_ips" => "blocked IP cache cleared".to_string(),
        "start" => "gateway process is already running".to_string(),
        "stop" | "reboot" | "close" => {
            return Err((
                StatusCode::CONFLICT,
                Json(AdminErrorResponse {
                    error: format!(
                        "{action} is recorded by Admin API but not executed by the in-process dev gateway"
                    ),
                }),
            ));
        }
        _ => unreachable!("canonical action should be validated"),
    };
    Ok(Json(AdminControlReceipt {
        action,
        target: request.target,
        accepted: true,
        message: match (request.operator_id, request.reason) {
            (Some(operator_id), Some(reason)) if !reason.trim().is_empty() => {
                format!("{message}; operator={operator_id}; reason={reason}")
            }
            (Some(operator_id), _) => format!("{message}; operator={operator_id}"),
            _ => message,
        },
    }))
}

fn canonical_admin_control_action(action: &str) -> Result<String, String> {
    let action = action
        .trim()
        .replace('-', "_")
        .replace(' ', "_")
        .to_ascii_lowercase();
    let canonical = match action.as_str() {
        "status" | "health" => "status",
        "reload_npcs" | "reload_npc" | "reloadnpc" => "reload_npcs",
        "reload_drops" | "reload_drop" | "reloaddrops" => "reload_drops",
        "reload_line_message" | "reload_line_messages" | "reloadlinemessage" => {
            "reload_line_message"
        }
        "clear_blocked_ips" | "clear_blocked_ip" | "clearblockedips" => "clear_blocked_ips",
        "start" | "start_server" => "start",
        "stop" | "stop_server" => "stop",
        "reboot" | "restart" => "reboot",
        "close" | "shutdown" => "close",
        "" => return Err("action is required".to_string()),
        other => return Err(format!("unsupported admin control action: {other}")),
    };
    Ok(canonical.to_string())
}

async fn spectator_matches(
    State(state): State<WebState>,
    Query(query): Query<SpectatorAccessQuery>,
) -> Result<Json<SpectatorDirectoryResponse>, (StatusCode, Json<AdminErrorResponse>)> {
    let authorization = state
        .spectator
        .config()
        .authorize(query.map.as_deref(), query.delay_ms, query.token.as_deref())
        .map_err(spectator_access_error)?;
    Ok(Json(SpectatorDirectoryResponse {
        source: "gateway-spectator".to_string(),
        generated_at_ms: gateway_unix_ms(),
        public_delay_ms: authorization.delay_ms,
        max_delay_ms: state.spectator.config().max_delay_ms,
        matches: state.spectator.matches(authorization.director),
    }))
}

async fn spectator_recordings(
    State(state): State<WebState>,
    Query(query): Query<SpectatorAccessQuery>,
) -> Result<Json<SpectatorRecordingsResponse>, (StatusCode, Json<AdminErrorResponse>)> {
    let authorization = state
        .spectator
        .config()
        .authorize(None, query.delay_ms, query.token.as_deref())
        .map_err(spectator_access_error)?;
    Ok(Json(SpectatorRecordingsResponse {
        source: "gateway-spectator".to_string(),
        generated_at_ms: gateway_unix_ms(),
        recordings: state.spectator.recordings(authorization.director),
    }))
}

async fn spectator_replay(
    State(state): State<WebState>,
    Query(query): Query<SpectatorAccessQuery>,
) -> Result<Json<SpectatorReplayResponse>, (StatusCode, Json<AdminErrorResponse>)> {
    let recording_id = query
        .replay_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(AdminErrorResponse {
                    error: "replayId is required".to_string(),
                }),
            )
        })?;
    let authorization = state
        .spectator
        .config()
        .authorize(None, query.delay_ms, query.token.as_deref())
        .map_err(spectator_access_error)?;
    let frames = state
        .spectator
        .load_replay(
            recording_id,
            authorization.director,
            gateway_unix_ms().saturating_sub(authorization.delay_ms),
        )
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })))?;
    Ok(Json(SpectatorReplayResponse {
        source: "gateway-spectator".to_string(),
        generated_at_ms: gateway_unix_ms(),
        recording_id: recording_id.to_string(),
        frames,
    }))
}

async fn spectator_metrics(
    State(state): State<WebState>,
    Query(query): Query<SpectatorAccessQuery>,
) -> Result<Json<crate::spectator::SpectatorMetrics>, (StatusCode, Json<AdminErrorResponse>)> {
    state
        .spectator
        .config()
        .authorize(None, query.delay_ms, query.token.as_deref())
        .map_err(spectator_access_error)?;
    Ok(Json(state.spectator.metrics()))
}

fn spectator_access_error(error: String) -> (StatusCode, Json<AdminErrorResponse>) {
    (StatusCode::FORBIDDEN, Json(AdminErrorResponse { error }))
}

async fn spectator_ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<WebState>,
    Query(query): Query<SpectatorAccessQuery>,
) -> Response {
    let ws_connection_permit = match state.capacity.try_acquire_ws_connection() {
        Ok(permit) => permit,
        Err(error) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AdminErrorResponse { error }),
            )
                .into_response();
        }
    };
    let authorization = match state.spectator.config().authorize(
        query.map.as_deref(),
        query.delay_ms,
        query.token.as_deref(),
    ) {
        Ok(authorization) => authorization,
        Err(error) => return spectator_access_error(error).into_response(),
    };
    bounded_websocket_upgrade(ws).on_upgrade(move |socket| {
        handle_spectator_socket(
            socket,
            state.spectator,
            state.ai_live,
            query,
            authorization,
            ws_connection_permit,
        )
    })
}

async fn handle_spectator_socket(
    socket: WebSocket,
    hub: SpectatorHub,
    ai_live: AiLiveHub,
    query: SpectatorAccessQuery,
    authorization: crate::spectator::SpectatorAuthorization,
    _ws_connection_permit: GatewayCapacityPermit,
) {
    let _viewer_guard = hub.viewer_connected();
    let (mut sender, mut receiver) = socket.split();
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut map = query
        .map
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| hub.latest_map(authorization.director))
        .unwrap_or_else(|| "0".to_string());
    let mut target = query
        .target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut director = authorization.director
        && query
            .mode
            .as_deref()
            .is_some_and(|mode| mode.eq_ignore_ascii_case("director"));
    let mut camera: Option<(i32, i32)> = None;
    let mut last_sequence = 0u64;
    let replay_frames = match query.replay_id.as_deref() {
        Some(recording_id) => hub
            .load_replay(
                recording_id,
                authorization.director,
                gateway_unix_ms().saturating_sub(authorization.delay_ms),
            )
            .unwrap_or_default(),
        None => Vec::new(),
    };
    let mut replay_index = 0usize;
    let mut replay_playing = !replay_frames.is_empty();
    let mut replay_speed = 1.0f64;
    let mut replay_due = Instant::now();
    eprintln!(
        "spectator audit event=connect map={} director={} delay_ms={} replay={}",
        map,
        authorization.director,
        authorization.delay_ms,
        query.replay_id.as_deref().unwrap_or("-")
    );

    let initial_status_frame = replay_frames.first().cloned().or_else(|| {
        hub.frame_at(
            &map,
            gateway_unix_ms().saturating_sub(authorization.delay_ms),
            0,
        )
    });
    if send_spectator_status(
        &mut sender,
        &hub,
        &ai_live,
        &map,
        target.as_deref(),
        authorization,
        director,
        camera,
        replay_frames.as_slice(),
        replay_index,
        replay_playing,
        replay_speed,
        initial_status_frame.as_ref(),
    )
    .await
    .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            inbound = receiver.next() => {
                let Some(inbound) = inbound else { return; };
                let message = match inbound {
                    Ok(Message::Text(text)) => text,
                    Ok(Message::Close(_)) => return,
                    Ok(_) => continue,
                    Err(_) => return,
                };
                let control = match serde_json::from_str::<SpectatorControl>(&message) {
                    Ok(control) => control,
                    Err(error) => {
                        let _ = sender.send(Message::Text(json!({
                            "type": "error",
                            "message": format!("invalid spectator control: {error}")
                        }).to_string().into())).await;
                        continue;
                    }
                };
                eprintln!(
                    "spectator audit event=control map={} director={} control={control:?}",
                    map, authorization.director
                );
                match control {
                    SpectatorControl::Follow { target: next_target } => {
                        target = next_target
                            .map(|value| value.trim().to_string())
                            .filter(|value| !value.is_empty());
                        camera = None;
                        director = false;
                    }
                    SpectatorControl::Map { map: next_map } => {
                        let next_map = next_map.trim();
                        if !authorization.director && !hub.config().is_public_map(next_map) {
                            let _ = sender.send(Message::Text(json!({
                                "type": "error",
                                "message": format!("map {next_map} is not public for spectators")
                            }).to_string().into())).await;
                            continue;
                        }
                        map = next_map.to_string();
                        last_sequence = 0;
                        target = None;
                        camera = None;
                    }
                    SpectatorControl::Director { enabled } => {
                        if authorization.director {
                            director = enabled;
                            camera = None;
                        }
                    }
                    SpectatorControl::Camera { x, y } => {
                        if authorization.director {
                            camera = Some((x, y));
                            director = false;
                        }
                    }
                    SpectatorControl::CameraClear => camera = None,
                    SpectatorControl::ReplayPlay => replay_playing = true,
                    SpectatorControl::ReplayPause => replay_playing = false,
                    SpectatorControl::ReplaySeek { captured_at_ms } => {
                        replay_index = replay_frames
                            .iter()
                            .position(|frame| frame.captured_at_ms >= captured_at_ms)
                            .unwrap_or_else(|| replay_frames.len().saturating_sub(1));
                        replay_due = Instant::now();
                    }
                    SpectatorControl::ReplaySpeed { speed } => {
                        replay_speed = speed.clamp(0.25, 8.0);
                    }
                }
                let status_frame = if replay_frames.is_empty() {
                    hub.frame_at(
                        &map,
                        gateway_unix_ms().saturating_sub(authorization.delay_ms),
                        0,
                    )
                } else {
                    replay_frames
                        .get(replay_index.min(replay_frames.len().saturating_sub(1)))
                        .cloned()
                };
                if send_spectator_status(
                    &mut sender,
                    &hub,
                    &ai_live,
                    &map,
                    target.as_deref(),
                    authorization,
                    director,
                    camera,
                    replay_frames.as_slice(),
                    replay_index,
                    replay_playing,
                    replay_speed,
                    status_frame.as_ref(),
                ).await.is_err() {
                    return;
                }
            }
            _ = tick.tick() => {
                let frame = if replay_frames.is_empty() {
                    hub.frame_at(
                        &map,
                        gateway_unix_ms().saturating_sub(authorization.delay_ms),
                        last_sequence,
                    )
                } else if replay_playing && Instant::now() >= replay_due {
                    let current = replay_frames.get(replay_index).cloned();
                    if let Some(current) = current.as_ref() {
                        let next_delta_ms = replay_frames
                            .get(replay_index.saturating_add(1))
                            .map(|next| next.captured_at_ms.saturating_sub(current.captured_at_ms))
                            .unwrap_or(250)
                            .clamp(25, 5_000);
                        replay_due = Instant::now()
                            + Duration::from_millis(
                                ((next_delta_ms as f64) / replay_speed).max(10.0) as u64,
                            );
                        replay_index = (replay_index + 1).min(replay_frames.len().saturating_sub(1));
                        if replay_index + 1 >= replay_frames.len() {
                            replay_playing = false;
                        }
                    }
                    current
                } else {
                    None
                };
                let Some(frame) = frame else { continue; };
                last_sequence = frame.sequence;
                let ai_target = if director {
                    ai_live
                        .status()
                        .latest_segment
                        .filter(|segment| segment.map_file_name == frame.map_file_name)
                        .and_then(|segment| segment.target)
                } else {
                    None
                };
                let world = frame.world_for_view(
                    target.as_deref().or(ai_target.as_deref()),
                    director,
                    camera,
                );
                if sender.send(Message::Text(json!({
                    "type": "worldSnapshot",
                    "payload": world
                }).to_string().into())).await.is_err() {
                    return;
                }
                if send_spectator_status(
                    &mut sender,
                    &hub,
                    &ai_live,
                    &map,
                    target.as_deref(),
                    authorization,
                    director,
                    camera,
                    replay_frames.as_slice(),
                    replay_index,
                    replay_playing,
                    replay_speed,
                    Some(&frame),
                ).await.is_err() {
                    return;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn send_spectator_status(
    sender: &mut WebSocketSender,
    hub: &SpectatorHub,
    ai_live: &AiLiveHub,
    map: &str,
    target: Option<&str>,
    authorization: crate::spectator::SpectatorAuthorization,
    director: bool,
    camera: Option<(i32, i32)>,
    replay_frames: &[SpectatorFrame],
    replay_index: usize,
    replay_playing: bool,
    replay_speed: f64,
    frame: Option<&SpectatorFrame>,
) -> Result<(), axum::Error> {
    let replay_cursor = replay_index.min(replay_frames.len().saturating_sub(1));
    sender
        .send(Message::Text(
            json!({
                "type": "spectatorStatus",
                "payload": {
                    "readOnly": true,
                    "directorAuthorized": authorization.director,
                    "director": director,
                    "delayMs": authorization.delay_ms,
                    "map": map,
                    "target": target,
                    "camera": camera.map(|(x, y)| json!({"x": x, "y": y})),
                    "matches": hub.matches(authorization.director),
                    "targets": frame.map(SpectatorFrame::targets).unwrap_or_default(),
                    "events": frame.map(|frame| frame.events.clone()).unwrap_or_default(),
                    "recordingId": frame.map(|frame| frame.recording_id.clone()),
                    "sequence": frame.map(|frame| frame.sequence),
                    "capturedAtMs": frame.map(|frame| frame.captured_at_ms),
                    "replay": {
                        "active": !replay_frames.is_empty(),
                        "playing": replay_playing,
                        "speed": replay_speed,
                        "startAtMs": replay_frames.first().map(|frame| frame.captured_at_ms),
                        "endAtMs": replay_frames.last().map(|frame| frame.captured_at_ms),
                        "currentAtMs": replay_frames.get(replay_cursor).map(|frame| frame.captured_at_ms)
                    }
                }
            })
            .to_string()
            .into(),
        ))
        .await?;
    sender
        .send(Message::Text(
            json!({
                "type": "aiLiveStatus",
                "payload": ai_live.status()
            })
            .to_string()
            .into(),
        ))
        .await
}

async fn ws_upgrade(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<WebState>,
) -> Response {
    if let Err(error) = validate_websocket_origin(&headers) {
        return (StatusCode::FORBIDDEN, Json(AdminErrorResponse { error })).into_response();
    }
    let ws_connection_permit = match state.capacity.try_acquire_ws_connection() {
        Ok(permit) => permit,
        Err(error) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AdminErrorResponse { error }),
            )
                .into_response();
        }
    };
    let peer_address = trusted_client_address(&headers, peer);
    let tcp_peer_ip = peer.ip();
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .chars()
        .filter(|ch| !ch.is_control())
        .take(160)
        .collect::<String>();
    bounded_websocket_upgrade(ws).on_upgrade(move |socket| {
        handle_socket(
            socket,
            state,
            ws_connection_permit,
            tcp_peer_ip,
            peer_address,
            user_agent,
        )
    })
}

async fn handle_socket(
    socket: WebSocket,
    state: WebState,
    _ws_connection_permit: GatewayCapacityPermit,
    tcp_peer_ip: IpAddr,
    peer_address: String,
    user_agent: String,
) {
    let realm_info = realm_info_event(state.config.as_ref());
    let mut session = new_gateway_session_for_web(&state);
    let mut active_session_permit: Option<GatewayCapacityPermit> = None;
    let mut save_queue = WebSessionSaveQueue::new(GatewaySaveQueueConfig::from_env());
    let mut route_refresh = WebSessionRouteRefresh::new(GatewayRouteRefreshConfig::from_env());
    let mut native_resume = NativeResumeConnectionState::new();
    handle_socket_inner(
        socket,
        &mut session,
        Arc::clone(&state.session_cache),
        Arc::clone(&state.reconnect_sessions),
        Arc::clone(&state.capacity),
        Arc::clone(&state.identity),
        &mut active_session_permit,
        &mut save_queue,
        &mut route_refresh,
        &mut native_resume,
        state.injector.clone(),
        realm_info,
        state.chat_hub.clone(),
        state.spectator.clone(),
        state.ai_live.clone(),
        tcp_peer_ip,
        peer_address,
        user_agent,
    )
    .await;
    let _ = tokio::task::block_in_place(|| {
        catch_gateway_panic("web refresh_active_external_mail", || {
            session.refresh_active_external_mail()
        })
    });
    let _ = save_queue.force_save_now(Instant::now(), || {
        tokio::task::block_in_place(|| {
            catch_gateway_panic("web save_active_character", || {
                session.save_active_character()
            })
        })
    });
    if !native_resume.resume_allowed {
        if let Some(family_id) = native_resume.family_id.take() {
            state.reconnect_sessions.revoke_resume_family(&family_id);
        }
        let _ = remove_owned_session_cache(state.session_cache.as_ref(), &session);
        return;
    }
    if let Some(key) = session_cache_key(&session) {
        let grace_seconds = reconnect_grace_ttl_seconds();
        if let Err(error) = refresh_session_cache_with_route_lease(
            state.session_cache.as_ref(),
            &session,
            grace_seconds,
        ) {
            eprintln!("web reconnect route lease refresh skipped: {error}");
        }
        if active_session_permit.is_none() {
            match state.capacity.try_acquire_active_session() {
                Ok(permit) => active_session_permit = Some(permit),
                Err(error) => {
                    eprintln!("web reconnect grace skipped: {error}");
                    let _ = remove_owned_session_cache(state.session_cache.as_ref(), &session);
                    return;
                }
            }
        }
        state.reconnect_sessions.purge_expired();
        let reconnect_lease_permit = match state.capacity.try_acquire_reconnect_lease() {
            Ok(permit) => permit,
            Err(error) => {
                eprintln!("web reconnect grace skipped: {error}");
                let _ = remove_owned_session_cache(state.session_cache.as_ref(), &session);
                return;
            }
        };
        let log_account_id = key.account_id.clone();
        let log_character_index = key.character_index;
        state.reconnect_sessions.store(
            key,
            session,
            active_session_permit.take(),
            reconnect_lease_permit,
            native_resume
                .opted_in
                .then_some(native_resume.family_id)
                .flatten(),
            Duration::from_secs(grace_seconds),
        );
        schedule_reconnect_session_purge(
            Arc::clone(&state.reconnect_sessions),
            Duration::from_secs(grace_seconds),
        );
        eprintln!(
            "web reconnect grace retained session for {log_account_id}/{log_character_index} for {grace_seconds}s"
        );
        return;
    }
    let _ = remove_owned_session_cache(state.session_cache.as_ref(), &session);
}

fn zone_movement_packet_for_action(action: &SessionAction) -> Option<ClientPacket> {
    match action {
        SessionAction::Packet(
            packet @ (ClientPacket::Walk { .. }
            | ClientPacket::Run { .. }
            | ClientPacket::Turn { .. }),
        ) => Some(packet.clone()),
        _ => None,
    }
}

async fn try_handle_zone_movement_from_reader(
    action: &SessionAction,
    sender: &SharedWebSocketSender,
    movement_ingress: &SharedZoneMovementIngressSlot,
    serial_execution_gate: &SharedSerialExecutionGate,
    authenticated: bool,
    pending_serial_actions: usize,
) -> Result<bool, String> {
    if !authenticated || pending_serial_actions != 0 {
        return Ok(false);
    }
    let Some(packet) = zone_movement_packet_for_action(action) else {
        return Ok(false);
    };
    let Ok(_execution_guard) = serial_execution_gate.try_read() else {
        return Ok(false);
    };
    let ingress = movement_ingress
        .read()
        .map_err(|_| "zone movement ingress slot was poisoned".to_string())?
        .clone();
    let Some(ingress) = ingress else {
        return Ok(false);
    };
    let move_log = move_log_for_action(action);
    let execution = tokio::task::spawn_blocking(move || ingress.try_execute(packet))
        .await
        .map_err(|error| format!("zone movement ingress task failed: {error}"))??;
    let Some(execution) = execution else {
        return Ok(false);
    };
    let responses = execution.packets;
    log_move_action(move_log, &responses);
    for packet in responses {
        send_server_packet(sender, &packet)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(true)
}

fn spawn_zone_outbound_sender(
    mut receiver: mpsc::Receiver<SharedZoneLiveOutbound>,
    sender: SharedWebSocketSender,
    serial_execution_gate: SharedSerialExecutionGate,
    active_registration_id: Arc<AtomicU64>,
) -> ZoneOutboundSenderTask {
    let handle = tokio::spawn(async move {
        while let Some(outbound) = receiver.recv().await {
            let registration_id = outbound.registration_id();
            if active_registration_id.load(Ordering::Acquire) != registration_id {
                continue;
            }
            let _serial_execution = serial_execution_gate.read().await;
            if active_registration_id.load(Ordering::Acquire) != registration_id {
                continue;
            }
            if send_server_packet(&sender, &outbound.into_packet())
                .await
                .is_err()
            {
                return;
            }
        }
    });
    ZoneOutboundSenderTask { handle }
}

fn register_zone_live_outbound(
    session: &GatewaySession,
    sender: &mpsc::Sender<SharedZoneLiveOutbound>,
    active_registration_id: &AtomicU64,
) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
    active_registration_id.store(0, Ordering::Release);
    let registration = prepare_zone_live_outbound(session, sender)?;
    activate_zone_live_outbound(registration.as_deref(), active_registration_id);
    Ok(registration)
}

fn prepare_zone_live_outbound(
    session: &GatewaySession,
    sender: &mpsc::Sender<SharedZoneLiveOutbound>,
) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
    session.register_zone_live_outbound(sender.clone())
}

fn activate_zone_live_outbound(
    registration: Option<&dyn ZoneLiveOutboundRegistration>,
    active_registration_id: &AtomicU64,
) {
    active_registration_id.store(
        registration
            .map(|registration| registration.registration_id())
            .unwrap_or(0),
        Ordering::Release,
    );
    if let Some(registration) = registration {
        registration.activate();
    }
}

fn invalid_browser_command_input(message: &str, error: &serde_json::Error) -> ParsedSocketInput {
    let recognized_resume = serde_json::from_str::<Value>(message)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .as_deref()
        == Some("resumeSession");
    if recognized_resume {
        ParsedSocketInput::ResumeRejected
    } else {
        ParsedSocketInput::ProtocolError(format!("invalid command: {error}"))
    }
}

fn spawn_socket_reader(
    mut receiver: WebSocketReceiver,
    sender: SharedWebSocketSender,
    movement_ingress: SharedZoneMovementIngressSlot,
    serial_execution_gate: SharedSerialExecutionGate,
    authenticated: Arc<AtomicBool>,
) -> (
    mpsc::Receiver<SocketInbound>,
    SocketReaderTask,
    Arc<AtomicUsize>,
) {
    debug_assert_eq!(
        SOCKET_INPUT_MAX_BUFFERED_BYTES,
        WEBSOCKET_MAX_FRAME_BYTES * SOCKET_INPUT_CAPACITY
    );
    let (input_tx, input_rx) = mpsc::channel(SOCKET_INPUT_CAPACITY);
    let buffered_bytes = Arc::new(Semaphore::new(SOCKET_INPUT_MAX_BUFFERED_BYTES));
    let pending_count = Arc::new(AtomicUsize::new(0));
    let reader_pending_count = Arc::clone(&pending_count);
    let reader_buffered_bytes = Arc::clone(&buffered_bytes);
    let handle = tokio::spawn(async move {
        loop {
            let message = match receiver.next().await {
                Some(Ok(Message::Text(text))) => text,
                Some(Ok(Message::Close(_))) | None => {
                    let _ = input_tx.send(SocketInbound::Closed).await;
                    return;
                }
                Some(Ok(_)) => continue,
                Some(Err(error)) => {
                    let _ = input_tx
                        .send(SocketInbound::ReadError(error.to_string()))
                        .await;
                    return;
                }
            };
            if message.len() > WEBSOCKET_MAX_MESSAGE_BYTES {
                return;
            }
            let message_permit = match Arc::clone(&reader_buffered_bytes)
                .acquire_many_owned(message.len().max(1) as u32)
                .await
            {
                Ok(permit) => permit,
                Err(_) => return,
            };

            let command = match serde_json::from_str::<BrowserCommand>(&message) {
                Ok(command) => command,
                Err(error) => {
                    if input_tx
                        .send(SocketInbound::Queued(QueuedSocketInput::new(
                            invalid_browser_command_input(&message, &error),
                            Arc::clone(&reader_pending_count),
                            message_permit,
                        )))
                        .await
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
            };
            let command = match command {
                BrowserCommand::ClientCapabilities { capabilities } => {
                    if input_tx
                        .send(SocketInbound::Queued(QueuedSocketInput::new(
                            ParsedSocketInput::ClientCapabilities(capabilities),
                            Arc::clone(&reader_pending_count),
                            message_permit,
                        )))
                        .await
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
                BrowserCommand::ResumeSession(request) => {
                    if input_tx
                        .send(SocketInbound::Queued(QueuedSocketInput::new(
                            ParsedSocketInput::ResumeSession(request.credential),
                            Arc::clone(&reader_pending_count),
                            message_permit,
                        )))
                        .await
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
                command => command,
            };
            let action = match browser_command_to_action(command) {
                Ok(action) => action,
                Err(error) => {
                    if input_tx
                        .send(SocketInbound::Queued(QueuedSocketInput::new(
                            ParsedSocketInput::ProtocolError(error),
                            Arc::clone(&reader_pending_count),
                            message_permit,
                        )))
                        .await
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
            };
            if let Some(time) = keep_alive_time_for_action(&action) {
                if send_server_packet(&sender, &ServerPacket::KeepAlive { time })
                    .await
                    .is_err()
                {
                    return;
                }
                continue;
            }
            match try_handle_zone_movement_from_reader(
                &action,
                &sender,
                &movement_ingress,
                &serial_execution_gate,
                authenticated.load(Ordering::Acquire),
                reader_pending_count.load(Ordering::Acquire),
            )
            .await
            {
                Ok(true) => continue,
                Ok(false) => {}
                Err(error) => {
                    if input_tx
                        .send(SocketInbound::Queued(QueuedSocketInput::new(
                            ParsedSocketInput::ProtocolError(error),
                            Arc::clone(&reader_pending_count),
                            message_permit,
                        )))
                        .await
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
            }
            if input_tx
                .send(SocketInbound::Queued(QueuedSocketInput::new(
                    ParsedSocketInput::Action(action),
                    Arc::clone(&reader_pending_count),
                    message_permit,
                )))
                .await
                .is_err()
            {
                return;
            }
        }
    });
    (input_rx, SocketReaderTask { handle }, pending_count)
}

fn new_gateway_session_for_web(state: &WebState) -> GatewaySession {
    let mut session = match &state.gameplay_event_sink {
        Some(sink) => GatewaySession::new_with_zone_registry_and_event_sink(
            (*state.config).clone(),
            &state.zone_registry,
            Arc::clone(sink),
        ),
        None => {
            GatewaySession::new_with_zone_registry((*state.config).clone(), &state.zone_registry)
        }
    };
    session.configure_zone_owner_heartbeat(gateway_zone_owner_heartbeat_interval_ms(), 0);
    session
}

// `_injection_registration` is an RAII Drop-guard; reassigning it (to re-register on
// re-login or unregister on logout) drops the prior guard intentionally.
#[allow(unused_assignments)]
async fn handle_socket_inner(
    socket: WebSocket,
    session: &mut GatewaySession,
    session_cache: SharedGatewaySessionCache,
    reconnect_sessions: Arc<ReconnectSessionStore>,
    capacity: Arc<GatewayCapacityState>,
    identity: Arc<IdentityService>,
    active_session_permit: &mut Option<GatewayCapacityPermit>,
    save_queue: &mut WebSessionSaveQueue,
    route_refresh: &mut WebSessionRouteRefresh,
    native_resume: &mut NativeResumeConnectionState,
    injector: crate::inject::LiveSessionInjector,
    realm_info: Value,
    chat_hub: ChatBroadcastHub,
    spectator: SpectatorHub,
    ai_live: AiLiveHub,
    tcp_peer_ip: IpAddr,
    peer_address: String,
    user_agent: String,
) {
    let (sender, receiver) = socket.split();
    let sender = Arc::new(AsyncMutex::new(sender));
    if sender
        .lock()
        .await
        .send(Message::Text(realm_info.to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    let mut runtime_tick = tokio::time::interval(gateway_runtime_tick_interval());
    runtime_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut spectator_publish_tick = tokio::time::interval(Duration::from_millis(
        spectator.config().capture_interval_ms,
    ));
    spectator_publish_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut ai_live_tick = tokio::time::interval(Duration::from_secs(1));
    ai_live_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_ai_live_segment_id: Option<String> = None;
    let mut runtime_tick_deferred_until = Instant::now();
    // The unsafe local-development escape hatch is authorized only from the
    // actual TCP peer. Forwarded/derived client addresses remain useful for
    // logging and rate limits but are never a privilege signal.
    let enforce_player_command_safety = production_player_command_safety_enabled(tcp_peer_ip);
    let mut authenticated = false;
    let mut authenticated_account_id: Option<String> = None;
    let mut active_identity_session: Option<VerifiedIdentitySession> = None;
    let mut native_game_shop = NativeGameShopConnectionState::default();
    let mut first_post_resume_identity_check_pending = false;
    let mut last_identity_revocation_check = Instant::now();
    let mut last_identity_database_check = Instant::now();
    // M4 WF-5: this socket task registers an injection channel keyed by its account so the
    // /onchain/inject HTTP handler can hand chain-confirmed commands to it. The RAII guard
    // unregisters on every exit path (the loop has many early returns).
    let (inject_tx, mut inject_rx) =
        tokio::sync::mpsc::unbounded_channel::<crate::inject::InjectionMessage>();
    let mut _injection_registration: Option<crate::inject::InjectionRegistration> = None;
    let background_route_refresh_record: SharedBackgroundRouteRefreshRecord =
        Arc::new(Mutex::new(None));
    let _background_route_refresh = spawn_background_route_lease_refresh(
        Arc::clone(&session_cache),
        route_refresh.config,
        Arc::clone(&background_route_refresh_record),
    );

    let connect_packets = match catch_gateway_panic("web on_connect", || {
        tokio::task::block_in_place(|| session.on_connect())
    }) {
        Ok(packets) => packets,
        Err(error) => {
            let _ = send_error_message(&sender, &error).await;
            return;
        }
    };

    for packet in connect_packets {
        if send_server_packet(&sender, &packet).await.is_err() {
            return;
        }
    }
    if let Err(error) = tokio::task::block_in_place(|| refresh_external_session_state(session)) {
        let _ = send_error_message(&sender, &error).await;
        return;
    }
    if send_world_snapshot(&sender, &session).await.is_err() {
        return;
    }
    let initial_movement_ingress = session.zone_movement_ingress();
    let movement_ingress = Arc::new(RwLock::new(initial_movement_ingress));
    let serial_execution_gate = Arc::new(AsyncRwLock::new(()));
    let socket_authenticated = Arc::new(AtomicBool::new(authenticated));
    let (zone_outbound_tx, zone_outbound_rx) = mpsc::channel(LIVE_ZONE_OUTBOUND_CAPACITY);
    let active_zone_outbound_registration_id = Arc::new(AtomicU64::new(0));
    let _zone_outbound_sender_task = spawn_zone_outbound_sender(
        zone_outbound_rx,
        Arc::clone(&sender),
        Arc::clone(&serial_execution_gate),
        Arc::clone(&active_zone_outbound_registration_id),
    );
    let mut _zone_live_outbound_registration: Option<Box<dyn ZoneLiveOutboundRegistration>> = None;
    let mut chat_presence: Option<ChatPresence> = None;
    let (mut socket_inputs, _socket_reader_task, pending_socket_actions) = spawn_socket_reader(
        receiver,
        Arc::clone(&sender),
        Arc::clone(&movement_ingress),
        Arc::clone(&serial_execution_gate),
        Arc::clone(&socket_authenticated),
    );

    loop {
        tokio::select! {
            biased;

            socket_input = socket_inputs.recv() => {
                let mut queued_input = match socket_input {
                    Some(SocketInbound::Queued(input)) => input,
                    Some(SocketInbound::Closed) | None => return,
                    Some(SocketInbound::ReadError(error)) => {
                        eprintln!("web receive error: {error}");
                        return;
                    }
                };
                let _serial_execution = serial_execution_gate.write().await;
                if let Err(error) = catch_gateway_panic("web zone owner command heartbeat", || {
                    tokio::task::block_in_place(|| session.renew_zone_owner_lease_if_due())
                })
                .and_then(|result| result.map(|_| ()))
                {
                    let _ = send_error_message(&sender, &error).await;
                    continue;
                }
                let parsed_input = queued_input.take_input();
                let mut action = match parsed_input {
                    ParsedSocketInput::ClientCapabilities(capabilities) => {
                        match validate_native_client_capabilities(&capabilities) {
                            Ok(capabilities) => {
                                if native_resume.opted_in && !capabilities.native_resume_v1 {
                                    if let Some(family_id) = native_resume.family_id.take() {
                                        reconnect_sessions.revoke_resume_family(&family_id);
                                    }
                                    native_resume.last_issued_at_ms = None;
                                }
                                native_resume.opted_in = capabilities.native_resume_v1;
                                native_game_shop.opted_in =
                                    capabilities.native_game_shop_receipt_v1;
                                if !native_game_shop.opted_in {
                                    native_game_shop.pending = None;
                                }
                            }
                            Err(error) => {
                                if send_error_message(&sender, &error).await.is_err() {
                                    return;
                                }
                            }
                        }
                        continue;
                    }
                    ParsedSocketInput::ResumeSession(credential) => {
                        let eligible = native_resume.opted_in
                            && native_resume.resume_allowed
                            && !authenticated
                            && active_session_permit.is_none()
                            && tokio::task::block_in_place(|| session.active_identity()).is_none();
                        let prepared = if eligible {
                            tokio::task::block_in_place(|| {
                                validate_and_prepare_native_resume(
                                    reconnect_sessions.as_ref(),
                                    session_cache.as_ref(),
                                    &identity,
                                    &credential,
                                    gateway_unix_ms(),
                                    |reserved_session| {
                                        route_refresh
                                            .maybe_refresh(
                                                session_cache.as_ref(),
                                                reserved_session,
                                                Instant::now(),
                                                true,
                                            )
                                            .map(|_| ())
                                    },
                                    |reserved_session| {
                                        prepare_zone_live_outbound(
                                            reserved_session,
                                            &zone_outbound_tx,
                                        )
                                    },
                                )
                            })
                        } else {
                            Err(NativeResumePrepareError::Unavailable)
                        };
                        let (reservation, verified, prepared_zone_registration) = match prepared {
                            Ok(prepared) => prepared,
                            Err(error) => {
                                if !matches!(error, NativeResumePrepareError::Unavailable) {
                                    eprintln!("native resume preparation failed: {error:?}");
                                }
                                if send_resume_rejected(&sender).await.is_err() {
                                    return;
                                }
                                continue;
                            }
                        };
                        let committed = tokio::task::block_in_place(|| {
                            revalidate_and_commit_prepared_native_resume(
                                reconnect_sessions.as_ref(),
                                session_cache.as_ref(),
                                &identity,
                                reservation,
                                &credential,
                                verified,
                                prepared_zone_registration,
                                gateway_unix_ms(),
                            )
                        });
                        let Ok((restored, binding, verified, prepared_zone_registration)) =
                            committed
                        else {
                            if send_resume_rejected(&sender).await.is_err() {
                                return;
                            }
                            continue;
                        };

                        *session = restored.session;
                        *active_session_permit = restored.active_session_permit;
                        authenticated = true;
                        authenticated_account_id = Some(binding.account_id.clone());
                        active_identity_session = Some(verified);
                        // Keep the reader's movement fast path closed until the
                        // first post-resume action passes a fresh identity
                        // check in this serial execution loop.
                        first_post_resume_identity_check_pending = true;
                        socket_authenticated.store(false, Ordering::Release);
                        native_resume.resume_allowed = true;
                        native_resume.family_id = None;
                        native_resume.minimum_generation = binding.generation.saturating_add(1);
                        native_resume.last_issued_at_ms = None;
                        _injection_registration = Some(crate::inject::InjectionRegistration::new(
                            injector.clone(),
                            &binding.account_id,
                            inject_tx.clone(),
                        ));
                        let next_movement_ingress = session.zone_movement_ingress();
                        *movement_ingress
                            .write()
                            .expect("zone movement ingress slot should not be poisoned") =
                            next_movement_ingress;
                        activate_zone_live_outbound(
                            prepared_zone_registration.as_deref(),
                            active_zone_outbound_registration_id.as_ref(),
                        );
                        _zone_live_outbound_registration = prepared_zone_registration;
                        chat_presence = Some(chat_hub.register(ChatProtocol::WebSocket));
                        update_background_route_refresh_record(
                            &background_route_refresh_record,
                            session,
                        );
                        let resumed_generation = binding.generation.saturating_add(1);
                        if send_session_resumed(
                            &sender,
                            binding.character_index,
                            resumed_generation,
                        )
                        .await
                        .is_err()
                            || send_world_snapshot(&sender, session).await.is_err()
                        {
                            return;
                        }
                        if maybe_issue_native_resume_credential(
                            &sender,
                            reconnect_sessions.as_ref(),
                            session_cache.as_ref(),
                            &identity,
                            native_resume,
                            session,
                            authenticated_account_id.as_deref(),
                            active_identity_session.as_ref(),
                            true,
                        )
                        .await
                        .is_err()
                        {
                            return;
                        }
                        continue;
                    }
                    ParsedSocketInput::ResumeRejected => {
                        if send_resume_rejected(&sender).await.is_err() {
                            return;
                        }
                        continue;
                    }
                    ParsedSocketInput::Action(action) => action,
                    ParsedSocketInput::ProtocolError(error) => {
                        if send_error_message(&sender, &error).await.is_err() {
                            return;
                        }
                        continue;
                    }
                };
                let first_post_resume_identity_valid = tokio::task::block_in_place(|| {
                    enforce_first_post_resume_action_identity(
                        &mut first_post_resume_identity_check_pending,
                        reconnect_sessions.as_ref(),
                        native_resume,
                        session_cache.as_ref(),
                        &identity,
                        session,
                        authenticated_account_id.as_deref(),
                        active_identity_session.as_ref(),
                        gateway_unix_ms(),
                    )
                });
                if !first_post_resume_identity_valid {
                    socket_authenticated.store(false, Ordering::Release);
                    let _ = send_resume_rejected(&sender).await;
                    return;
                }
                if !first_post_resume_identity_check_pending {
                    socket_authenticated.store(authenticated, Ordering::Release);
                }
                let native_game_shop_request = if native_game_shop.opted_in {
                    match native_game_shop_request_from_action(&action) {
                        Some(Ok(request)) => {
                            if let Err(receipt) = native_game_shop.reserve(request.clone()) {
                                if send_native_game_shop_receipt(&sender, &receipt).await.is_err() {
                                    return;
                                }
                                continue;
                            }
                            let in_game = tokio::task::block_in_place(|| {
                                session.active_identity().is_some()
                            });
                            let typed_outcome_supported = authenticated
                                && in_game
                                && tokio::task::block_in_place(|| {
                                    session.supports_typed_game_shop_purchase_outcome()
                                });
                            if let Some(receipt) = native_game_shop_pre_execution_failure(
                                &request,
                                authenticated,
                                in_game,
                                typed_outcome_supported,
                            ) {
                                let send_succeeded =
                                    send_native_game_shop_receipt(&sender, &receipt).await.is_ok();
                                if finish_native_game_shop_pre_execution_receipt(
                                    &mut native_game_shop,
                                    &request,
                                    send_succeeded,
                                ) == NativeGameShopPreExecutionReceiptDisposition::CloseUnknown
                                {
                                    return;
                                }
                                continue;
                            }
                            Some(request)
                        }
                        Some(Err(error)) => {
                            if send_error_message(&sender, &error).await.is_err() {
                                return;
                            }
                            continue;
                        }
                        None => None,
                    }
                } else {
                    None
                };
                let starts_game = matches!(
                    &action,
                    SessionAction::Packet(ClientPacket::StartGame { .. })
                );
                let leaves_world = is_explicit_session_leave_action(&action);
                if leaves_world {
                    native_resume.disable_and_revoke(reconnect_sessions.as_ref());
                }

                let should_send_snapshot_by_action = should_send_world_snapshot_for_action(&action);
                let low_latency_action = is_low_latency_action(&action);
                let should_queue_save_by_action = should_queue_save_for_action(&action);
                let runtime_tick_defer_duration = runtime_tick_defer_duration_for_action(&action);
                let mut login_account_id = login_account_id_for_action(&action).map(str::to_string);
                let mut login_identity_context = login_identity_context_for_action(&action);
                if let Some(context) = auth_security_context_for_action(&action) {
                    if let Err(error) = enforce_auth_rate_limits(
                        session_cache.as_ref(),
                        &identity,
                        &peer_address,
                        &user_agent,
                        &context,
                    ) {
                        let _ = send_error_message(&sender, &error).await;
                        continue;
                    }
                }
                let keep_alive_time = keep_alive_time_for_action(&action);
                if let Some(time) = keep_alive_time {
                    if send_server_packet(&sender, &ServerPacket::KeepAlive { time })
                        .await
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
                let start_game_character_index = start_game_character_index_for_action(&action);
                if let (true, Some(account_id), Some(character_index)) = (
                    authenticated,
                    authenticated_account_id.as_deref(),
                    start_game_character_index,
                ) {
                    let key = GatewaySessionCacheKey {
                        account_id: account_id.to_string(),
                        character_index,
                    };
                    if let Some(restored) = reconnect_sessions.take(&key) {
                        let restored_session_id = restored.session.session_id().to_string();
                        *session = restored.session;
                        *active_session_permit = restored.active_session_permit;
                        eprintln!(
                            "web reconnect grace restored session {restored_session_id} for {}/{}",
                            key.account_id, key.character_index
                        );
                    }
                }
                let pending_start_game_route_lease = match tokio::task::block_in_place(|| {
                    try_acquire_start_game_route_lease(
                        session_cache.as_ref(),
                        session,
                        authenticated,
                        authenticated_account_id.as_deref(),
                        start_game_character_index,
                    )
                }) {
                    Ok(key) => key,
                    Err(error) => {
                        let _ = send_error_message(&sender, &error).await;
                        continue;
                    }
                };
                if let Some(key) = pending_start_game_route_lease.as_ref() {
                    let zone_id = session.zone_id().clone();
                    match crate::gate15::acquire_player_session(
                        &key.account_id,
                        key.character_index,
                        &zone_id,
                    )
                    .await
                    {
                        Ok(Some(grant)) => {
                            eprintln!(
                                "Gate 15 Web StartGame finalized {}/{} on {} generation {} at height {}",
                                key.account_id,
                                key.character_index,
                                grant.lease.zone_id,
                                grant.placement.generation,
                                grant.finalized_height
                            );
                        }
                        Ok(None) => {}
                        Err(error) => {
                            release_pending_start_game_route_lease(
                                session_cache.as_ref(),
                                session,
                                Some(key),
                            );
                            let _ = send_error_message(
                                &sender,
                                &format!("Commonware session lease unavailable: {error}"),
                            )
                            .await;
                            continue;
                        }
                    }
                }
                if let SessionAction::PasskeyLogin {
                    account_id,
                    proof_account_id,
                    token,
                } = &mut action
                {
                    let verified = match verify_passkey_gateway_token(proof_account_id, token) {
                        Ok(verified) => verified,
                        Err(error) => {
                            let _ = send_error_message(&sender, &error).await;
                            continue;
                        }
                    };
                    if let Some(context) = login_identity_context.as_mut() {
                        context.auth_method = verified.auth_method;
                        context.credential_subject = verified
                            .credential_subject
                            .clone()
                            .unwrap_or_else(|| {
                                proof_account_id
                                    .strip_prefix("sui:")
                                    .unwrap_or(proof_account_id)
                                    .to_string()
                            });
                        let resolved_account = match tokio::task::block_in_place(|| {
                            identity.resolve_sui_account(
                                verified.auth_method,
                                &context.credential_subject,
                            )
                        }) {
                            Ok(Some(resolved)) => resolved,
                            Ok(None) => proof_account_id.clone(),
                            Err(error) => {
                                let _ = send_error_message(
                                    &sender,
                                    &format!("identity credential lookup unavailable: {error}"),
                                )
                                .await;
                                continue;
                            }
                        };
                        *account_id = resolved_account.clone();
                        context.account_id = resolved_account.clone();
                        login_account_id = Some(resolved_account);
                    }
                    let ttl_seconds = verified
                        .expires_at_ms
                        .saturating_sub(gateway_unix_ms())
                        .saturating_add(999)
                        / 1_000;
                    match session_cache
                        .consume_auth_token(&verified.token_id, ttl_seconds.max(1))
                    {
                        Ok(true) => {}
                        Ok(false) => {
                            let _ = send_error_message(
                                &sender,
                                "passkey login token was already consumed",
                            )
                            .await;
                            continue;
                        }
                        Err(error) => {
                            let _ = send_error_message(
                                &sender,
                                &format!("passkey replay protection unavailable: {error}"),
                            )
                            .await;
                            continue;
                        }
                    }
                }
                let action_capacity_permit = match inflight_capacity_kind_for_action(&action) {
                    Some(kind) => match capacity
                        .acquire_action_with_wait(kind, gateway_action_queue_wait(kind))
                        .await
                    {
                        Ok(permit) => Some(permit),
                        Err(error) => {
                            release_pending_start_game_route_lease(
                                session_cache.as_ref(),
                                session,
                                pending_start_game_route_lease.as_ref(),
                            );
                            let _ = send_error_message(&sender, &error).await;
                            continue;
                        }
                    },
                    None => None,
                };
                let mut pending_active_session_permit = None;
                if start_game_character_index.is_some()
                    && tokio::task::block_in_place(|| session.active_identity()).is_none()
                    && active_session_permit.is_none()
                {
                    match capacity.try_acquire_active_session() {
                        Ok(permit) => pending_active_session_permit = Some(permit),
                        Err(error) => {
                            release_pending_start_game_route_lease(
                                session_cache.as_ref(),
                                session,
                                pending_start_game_route_lease.as_ref(),
                            );
                            let _ = send_error_message(&sender, &error).await;
                            continue;
                        }
                    }
                }
                let execution_result = match catch_gateway_panic("web session action", || {
                    tokio::task::block_in_place(|| {
                        if let Some(request) = native_game_shop_request.as_ref() {
                            execute_native_game_shop_handler_seam(session, request).map(|dispatch| {
                                (dispatch.normal_packets, Some(dispatch.post_execution))
                            })
                        } else {
                            execute_session_action(
                                session,
                                action,
                                authenticated,
                                enforce_player_command_safety,
                            )
                            .map(|packets| (packets, None))
                        }
                    })
                }) {
                    Ok(Ok(execution)) => execution,
                    Ok(Err(error)) => {
                        release_pending_start_game_route_lease(
                            session_cache.as_ref(),
                            session,
                            pending_start_game_route_lease.as_ref(),
                        );
                        if native_game_shop_request.is_some() {
                            eprintln!(
                                "native GameShop execution failed with unknown commit state; closing socket: {error}"
                            );
                            return;
                        }
                        let _ = send_error_message(&sender, &error).await;
                        continue;
                    }
                    Err(error) => {
                        release_pending_start_game_route_lease(
                            session_cache.as_ref(),
                            session,
                            pending_start_game_route_lease.as_ref(),
                        );
                        if native_game_shop_request.is_some() {
                            eprintln!(
                                "native GameShop execution panicked with unknown commit state; closing socket without receipt: {error}"
                            );
                            return;
                        }
                        let _ = send_error_message(&sender, &error).await;
                        return;
                    }
                };
                let (responses, native_game_shop_post_execution) = execution_result;
                let native_game_shop_receipt = match native_game_shop_post_execution {
                    Some(post_execution) => match post_execution {
                        NativeGameShopPostExecution::SendReceipt(receipt) => Some(receipt),
                        NativeGameShopPostExecution::CloseUnknown { reason } => {
                            eprintln!(
                                "native GameShop post-execution state is unknown; closing without receipt: {reason}"
                            );
                            return;
                        }
                    },
                    None => None,
                };
                let map_changed = responses_require_resume_rotation(&responses);
                if let Err(error) = finalize_gate15_identities_for_responses(
                    authenticated_account_id.as_deref(),
                    login_account_id.as_deref(),
                    &responses,
                )
                .await
                {
                    if native_game_shop_request.is_some() {
                        eprintln!(
                            "native GameShop post-execution identity finalization failed; closing without receipt: {error}"
                        );
                        return;
                    }
                    let _ = send_error_message(
                        &sender,
                        &format!("Commonware identity finalization unavailable: {error}"),
                    )
                    .await;
                    continue;
                }
                let active_identity =
                    tokio::task::block_in_place(|| session.active_identity());
                if leaves_world || active_identity.is_none() {
                    chat_presence = None;
                }
                drop(action_capacity_permit);
                tokio::task::block_in_place(|| {
                    release_unclaimed_start_game_route_lease(
                        session_cache.as_ref(),
                        session,
                        pending_start_game_route_lease.as_ref(),
                    )
                });
                if pending_active_session_permit.is_some() && active_identity.is_some() {
                    *active_session_permit = pending_active_session_permit.take();
                }
                let force_route_refresh = start_game_character_index.is_some();
                let next_authenticated = update_authenticated_state(authenticated, &responses);
                if !authenticated && !next_authenticated {
                    if let Some(context) = login_identity_context.as_ref() {
                        let _ = tokio::task::block_in_place(|| {
                            identity.record_auth_security_event(
                                Some(&context.account_id),
                                "login_attempt",
                                "failure",
                                "invalid_credentials_or_policy",
                                &peer_address,
                                &user_agent,
                            )
                        });
                    }
                }
                if !authenticated && next_authenticated {
                    native_resume.reset_for_authenticated_login();
                    let Some(context) = login_identity_context.as_ref() else {
                        let _ = send_error_message(
                            &sender,
                            "identity session cannot be issued for this login",
                        )
                        .await;
                        return;
                    };
                    clear_successful_login_rate_limits(
                        session_cache.as_ref(),
                        &identity,
                        &peer_address,
                        &context.account_id,
                    );
                    let grant = match tokio::task::block_in_place(|| {
                        identity.issue_session(
                            &context.account_id,
                            context.auth_method,
                            &context.credential_subject,
                            &peer_address,
                            &user_agent,
                        )
                    }) {
                        Ok(grant) => grant,
                        Err(error) => {
                            let _ = send_error_message(
                                &sender,
                                &format!("identity session unavailable: {error}"),
                            )
                            .await;
                            return;
                        }
                    };
                    active_identity_session = match tokio::task::block_in_place(|| {
                        identity.verify_session_token(&grant.token)
                    }) {
                        Ok(verified) => Some(verified),
                        Err(error) => {
                            let _ = send_error_message(
                                &sender,
                                &format!("identity session unavailable: {error}"),
                            )
                            .await;
                            return;
                        }
                    };
                    if send_identity_session_grant(&sender, &grant).await.is_err() {
                        return;
                    }
                }
                if next_authenticated {
                    if let Some(account_id) = login_account_id {
                        _injection_registration = Some(crate::inject::InjectionRegistration::new(
                            injector.clone(),
                            &account_id,
                            inject_tx.clone(),
                        ));
                        authenticated_account_id = Some(account_id);
                    }
                } else if authenticated {
                    if let Some(verified) = active_identity_session.take() {
                        let _ = tokio::task::block_in_place(|| {
                            let _revocation_fence = reconnect_sessions
                                .begin_identity_session_revocation(&verified.session_id);
                            identity.revoke_session(&verified, &verified.session_id, "player_logout")
                        });
                    }
                    authenticated_account_id = None;
                    _injection_registration = None;
                }
                authenticated = next_authenticated;
                socket_authenticated.store(authenticated, Ordering::Release);
                let next_movement_ingress = session.zone_movement_ingress();
                *movement_ingress
                    .write()
                    .expect("zone movement ingress slot should not be poisoned") =
                    next_movement_ingress.clone();
                let next_zone_live_outbound_registration = if authenticated {
                    match tokio::task::block_in_place(|| {
                        register_zone_live_outbound(
                            session,
                            &zone_outbound_tx,
                            active_zone_outbound_registration_id.as_ref(),
                        )
                    }) {
                        Ok(registration) => registration,
                        Err(error) => {
                            if native_game_shop_request.is_some() {
                                eprintln!(
                                    "native GameShop post-execution Zone registration failed; closing without receipt: {error}"
                                );
                                return;
                            }
                            let _ = send_error_message(&sender, &error).await;
                            return;
                        }
                    }
                } else {
                    active_zone_outbound_registration_id.store(0, Ordering::Release);
                    None
                };
                _zone_live_outbound_registration = next_zone_live_outbound_registration;

                if let Err(error) = flush_session_updates(
                    &sender,
                    session,
                    session_cache.as_ref(),
                    save_queue,
                    route_refresh,
                    responses,
                    should_send_snapshot_by_action,
                    low_latency_action,
                    should_queue_save_by_action,
                    force_route_refresh,
                )
                .await
                {
                    if native_game_shop_request.is_some() {
                        eprintln!(
                            "native GameShop post-execution flush failed; closing without receipt: {error}"
                        );
                        return;
                    }
                    let _ = send_error_message(&sender, &error).await;
                    return;
                }
                if let (Some(request), Some(receipt)) = (
                    native_game_shop_request.as_ref(),
                    native_game_shop_receipt.as_ref(),
                ) {
                    if send_native_game_shop_receipt(&sender, receipt).await.is_err()
                        || !native_game_shop.clear_exact(request)
                    {
                        return;
                    }
                }
                if (starts_game || map_changed)
                    && maybe_issue_native_resume_credential(
                        &sender,
                        reconnect_sessions.as_ref(),
                        session_cache.as_ref(),
                        &identity,
                        native_resume,
                        session,
                        authenticated_account_id.as_deref(),
                        active_identity_session.as_ref(),
                        true,
                    )
                    .await
                    .is_err()
                {
                    return;
                }
                if starts_game
                    && chat_presence.is_none()
                    && active_identity.is_some()
                    && session.zone_movement_ingress().is_some()
                {
                    chat_presence = Some(chat_hub.register(ChatProtocol::WebSocket));
                }
                if force_route_refresh || !low_latency_action {
                    update_background_route_refresh_record(
                        &background_route_refresh_record,
                        session,
                    );
                }
                if let Some(duration) = runtime_tick_defer_duration {
                    runtime_tick_deferred_until = Instant::now() + duration;
                    runtime_tick.reset_after(duration);
                }
            }
            broadcast = recv_optional_chat(&mut chat_presence) => {
                match broadcast {
                    Ok(packet) => {
                        if send_server_packet(&sender, &packet).await.is_err() {
                            return;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        chat_presence = None;
                    }
                }
            }
            maybe_injection = inject_rx.recv() => {
                let Some(crate::inject::InjectionMessage { command, reply }) = maybe_injection else {
                    continue;
                };
                let _serial_execution = serial_execution_gate.write().await;
                let _serial_action =
                    PendingSerialAction::new(Arc::clone(&pending_socket_actions));
                // Apply the chain-confirmed command authoritatively (Direct mode), then push
                // the resulting packets to this player's socket.
                let outcome = catch_gateway_panic("web onchain injection", || {
                    tokio::task::block_in_place(|| session.execute_with_outcome(command))
                });
                let responses = match outcome {
                    Ok(Ok(execution)) => execution.packets,
                    Ok(Err(error)) => {
                        eprintln!("web onchain injection rejected: {error}");
                        let _ = reply.send(crate::inject::InjectionOutcome { packet_count: 0 });
                        continue;
                    }
                    Err(error) => {
                        let _ = send_error_message(&sender, &error).await;
                        return;
                    }
                };
                let map_changed = responses_require_resume_rotation(&responses);
                let packet_count = responses.len();
                if let Err(error) = flush_session_updates(
                    &sender,
                    session,
                    session_cache.as_ref(),
                    save_queue,
                    route_refresh,
                    responses,
                    true,
                    false,
                    true,
                    false,
                )
                .await
                {
                    let _ = send_error_message(&sender, &error).await;
                    return;
                }
                if map_changed
                    && maybe_issue_native_resume_credential(
                        &sender,
                        reconnect_sessions.as_ref(),
                        session_cache.as_ref(),
                        &identity,
                        native_resume,
                        session,
                        authenticated_account_id.as_deref(),
                        active_identity_session.as_ref(),
                        true,
                    )
                    .await
                    .is_err()
                {
                    return;
                }
                let _ = reply.send(crate::inject::InjectionOutcome { packet_count });
            }
            _ = spectator_publish_tick.tick() => {
                if tokio::task::block_in_place(|| session.active_identity()).is_none() {
                    continue;
                }
                let snapshot = tokio::task::block_in_place(|| session.world_snapshot());
                if let Err(error) = spectator.publish(&snapshot) {
                    eprintln!("spectator frame publish skipped: {error}");
                }
            }
            _ = ai_live_tick.tick(), if authenticated => {
                let status = ai_live.status();
                let game_overlay_ready = status.distribution.channels.iter().any(|channel| {
                    channel.channel
                        == crate::ai_distribution::AiDistributionChannel::GameOverlay
                        && channel.enabled
                        && channel.state == "ready"
                });
                let latest_segment_id = status
                    .latest_segment
                    .as_ref()
                    .map(|segment| segment.segment_id.clone());
                let latest_segment_is_fresh = status.latest_segment.as_ref().is_some_and(|segment| {
                    gateway_unix_ms().saturating_sub(segment.created_at_ms) <= 60_000
                });
                if status.mode == AiLiveMode::Live
                    && game_overlay_ready
                    && latest_segment_is_fresh
                    && latest_segment_id.is_some()
                    && latest_segment_id != last_ai_live_segment_id
                {
                    if sender
                        .lock()
                        .await
                        .send(Message::Text(
                            json!({
                                "type": "aiLiveStatus",
                                "payload": status
                            })
                            .to_string()
                            .into(),
                        ))
                        .await
                        .is_err()
                    {
                        return;
                    }
                    last_ai_live_segment_id = latest_segment_id;
                }
            }
            _ = runtime_tick.tick() => {
                let now = Instant::now();
                if authenticated
                    && now.duration_since(last_identity_revocation_check) >= Duration::from_secs(5)
                {
                    last_identity_revocation_check = now;
                    let Some(verified) = active_identity_session.as_ref() else {
                        let _ = send_error_message(&sender, "identity session is missing").await;
                        return;
                    };
                    if verified.expires_at_ms <= gateway_unix_ms() {
                        let _ = send_error_message(&sender, "identity session expired").await;
                        return;
                    }
                    match session_cache.identity_session_is_revoked(&verified.session_id) {
                        Ok(false) => {}
                        Ok(true) => {
                            let _ = send_error_message(&sender, "identity session was revoked").await;
                            return;
                        }
                        Err(error) => {
                            let _ = send_error_message(
                                &sender,
                                &format!("identity revocation check unavailable: {error}"),
                            )
                            .await;
                            return;
                        }
                    }
                    if now.duration_since(last_identity_database_check)
                        >= Duration::from_secs(60)
                    {
                        last_identity_database_check = now;
                        let database_check = tokio::task::block_in_place(|| {
                            identity.touch_session(verified)
                        });
                        if let Err(error) = database_check {
                            let _ = send_error_message(
                                &sender,
                                &format!("identity session is no longer active: {error}"),
                            )
                            .await;
                            return;
                        }
                    }
                }
                if maybe_issue_native_resume_credential(
                    &sender,
                    reconnect_sessions.as_ref(),
                    session_cache.as_ref(),
                    &identity,
                    native_resume,
                    session,
                    authenticated_account_id.as_deref(),
                    active_identity_session.as_ref(),
                    false,
                )
                .await
                .is_err()
                {
                    return;
                }
                if let Err(error) = catch_gateway_panic("web zone owner heartbeat", || {
                    tokio::task::block_in_place(|| session.renew_zone_owner_lease_if_due())
                }).and_then(|result| result.map(|_| ())) {
                    let _ = send_error_message(&sender, &error).await;
                    return;
                }
                if now < runtime_tick_deferred_until {
                    continue;
                }
                let responses = match catch_gateway_panic("web session tick", || {
                    tokio::task::block_in_place(|| {
                        session
                            .execute_with_outcome(WorldCommand::Tick)
                            .map(|execution| execution.packets)
                    })
                })
                .and_then(|result| result) {
                    Ok(responses) => responses,
                    Err(error) => {
                        if crate::gate15::health().is_some() {
                            eprintln!(
                                "Gate 15 transient Zone tick failure; keeping player socket for placement recovery: {error}"
                            );
                            continue;
                        }
                        let _ = send_error_message(&sender, &error).await;
                        return;
                    }
                };
                let map_changed = responses_require_resume_rotation(&responses);
                if responses.is_empty() {
                    if let Err(error) = save_queue.checkpoint(now, || {
                        tokio::task::block_in_place(|| {
                            catch_gateway_panic("web save_active_character", || {
                                session.save_active_character()
                            })
                        })
                    }) {
                        let _ = send_error_message(&sender, &error).await;
                        return;
                    }
                    continue;
                }
                if let Err(error) = flush_session_updates(
                    &sender,
                    session,
                    session_cache.as_ref(),
                    save_queue,
                    route_refresh,
                    responses,
                    false,
                    true,
                    false,
                    false,
                )
                .await
                {
                    let _ = send_error_message(&sender, &error).await;
                    return;
                }
                if map_changed
                    && maybe_issue_native_resume_credential(
                        &sender,
                        reconnect_sessions.as_ref(),
                        session_cache.as_ref(),
                        &identity,
                        native_resume,
                        session,
                        authenticated_account_id.as_deref(),
                        active_identity_session.as_ref(),
                        true,
                    )
                    .await
                    .is_err()
                {
                    return;
                }
                runtime_tick_deferred_until =
                    Instant::now() + gateway_runtime_tick_input_wake_grace();
                runtime_tick.reset_after(gateway_runtime_tick_input_wake_grace());
            }
        }
    }
}

fn spawn_background_route_lease_refresh(
    session_cache: SharedGatewaySessionCache,
    config: GatewayRouteRefreshConfig,
    record: SharedBackgroundRouteRefreshRecord,
) -> BackgroundRouteRefreshTask {
    let handle = tokio::spawn(async move {
        let mut tick = tokio::time::interval(config.interval);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let Some((record, owner)) = record
                .lock()
                .expect("background route refresh record mutex should not be poisoned")
                .clone()
            else {
                continue;
            };
            let session_cache = Arc::clone(&session_cache);
            let result = tokio::task::spawn_blocking(move || {
                session_cache.refresh_owned_route_lease_record(
                    record,
                    &owner,
                    route_lease_ttl_seconds(),
                )
            })
            .await;
            match result {
                Ok(Ok(true)) | Ok(Ok(false)) => {}
                Ok(Err(error)) => eprintln!("web background route lease refresh skipped: {error}"),
                Err(error) => eprintln!("web background route lease refresh task failed: {error}"),
            }
        }
    });
    BackgroundRouteRefreshTask { handle }
}

fn update_background_route_refresh_record(
    record: &SharedBackgroundRouteRefreshRecord,
    session: &GatewaySession,
) {
    let next =
        session_cache_record(session).map(|record| (record, session.session_id().to_string()));
    *record
        .lock()
        .expect("background route refresh record mutex should not be poisoned") = next;
}

fn schedule_reconnect_session_purge(
    reconnect_sessions: Arc<ReconnectSessionStore>,
    grace: Duration,
) {
    tokio::spawn(async move {
        tokio::time::sleep(grace.max(Duration::from_millis(1)) + Duration::from_millis(50)).await;
        reconnect_sessions.purge_expired();
    });
}

const DEFAULT_GATEWAY_RUNTIME_TICK_MS: u64 = 300;
const DEFAULT_GATEWAY_RUNTIME_TICK_INPUT_WAKE_MS: u64 = 75;
const DEFAULT_GATEWAY_RUNTIME_TICK_BOOTSTRAP_GRACE_MS: u64 = 15_000;
const DEFAULT_GATEWAY_ZONE_OWNER_HEARTBEAT_MS: u64 = 10_000;

async fn flush_session_updates(
    sender: &SharedWebSocketSender,
    session: &mut GatewaySession,
    session_cache: &dyn crate::cache::GatewaySessionCache,
    save_queue: &mut WebSessionSaveQueue,
    route_refresh: &mut WebSessionRouteRefresh,
    responses: Vec<ServerPacket>,
    should_send_snapshot_by_action: bool,
    low_latency_action: bool,
    should_queue_save_by_action: bool,
    force_route_refresh: bool,
) -> Result<(), String> {
    let response_requires_snapshot = responses_require_world_snapshot(&responses);

    for response in responses {
        send_server_packet(sender, &response)
            .await
            .map_err(|error| error.to_string())?;
    }

    if low_latency_action {
        return Ok(());
    }

    let external_state_changed =
        tokio::task::block_in_place(|| refresh_external_session_state(session))?;
    let should_send_snapshot =
        should_send_snapshot_by_action || response_requires_snapshot || external_state_changed;

    if should_send_snapshot {
        send_world_snapshot(sender, session).await?;
    }

    if !low_latency_action
        && should_queue_save_by_action
        && tokio::task::block_in_place(|| session.active_identity()).is_some()
    {
        save_queue.request_save(Instant::now(), || {
            tokio::task::block_in_place(|| {
                catch_gateway_panic("web save_active_character", || {
                    session.save_active_character()
                })
            })
        })?;
    } else {
        save_queue.checkpoint(Instant::now(), || {
            tokio::task::block_in_place(|| {
                catch_gateway_panic("web save_active_character", || {
                    session.save_active_character()
                })
            })
        })?;
    }

    if let Err(error) = tokio::task::block_in_place(|| {
        route_refresh.maybe_refresh(session_cache, session, Instant::now(), force_route_refresh)
    }) {
        eprintln!("web session route lease refresh skipped: {error}");
    }

    Ok(())
}

fn route_lease_ttl_seconds() -> u64 {
    std::env::var("MIR2_GATEWAY_ROUTE_LEASE_TTL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or_else(|| {
            std::env::var("MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
        })
        .unwrap_or(30)
        .max(1)
}

fn reconnect_grace_ttl_seconds() -> u64 {
    std::env::var("MIR2_GATEWAY_RECONNECT_GRACE_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(15)
        .clamp(1, 120)
}

fn gateway_runtime_tick_interval() -> Duration {
    duration_from_millis_env(
        "MIR2_GATEWAY_RUNTIME_TICK_MS",
        DEFAULT_GATEWAY_RUNTIME_TICK_MS,
        100,
        5_000,
    )
}

fn gateway_runtime_tick_bootstrap_grace() -> Duration {
    duration_from_millis_env(
        "MIR2_GATEWAY_RUNTIME_TICK_BOOTSTRAP_GRACE_MS",
        DEFAULT_GATEWAY_RUNTIME_TICK_BOOTSTRAP_GRACE_MS,
        0,
        30_000,
    )
}

fn gateway_runtime_tick_input_wake_grace() -> Duration {
    duration_from_millis_env(
        "MIR2_GATEWAY_RUNTIME_TICK_INPUT_WAKE_MS",
        DEFAULT_GATEWAY_RUNTIME_TICK_INPUT_WAKE_MS,
        0,
        500,
    )
}

fn gateway_zone_owner_heartbeat_interval_ms() -> u64 {
    duration_from_millis_env(
        "MIR2_GATEWAY_ZONE_OWNER_HEARTBEAT_MS",
        DEFAULT_GATEWAY_ZONE_OWNER_HEARTBEAT_MS,
        100,
        60_000,
    )
    .as_millis()
    .min(u128::from(u64::MAX)) as u64
}

fn gateway_action_queue_wait(kind: GatewayCapacityKind) -> Duration {
    let (name, production_default_ms) = match kind {
        GatewayCapacityKind::Login => ("MIR2_GATEWAY_LOGIN_QUEUE_WAIT_MS", 30_000),
        GatewayCapacityKind::NewCharacter => ("MIR2_GATEWAY_NEW_CHARACTER_QUEUE_WAIT_MS", 30_000),
        GatewayCapacityKind::StartGame => ("MIR2_GATEWAY_START_GAME_QUEUE_WAIT_MS", 300_000),
        GatewayCapacityKind::WebSocketConnection
        | GatewayCapacityKind::ActiveSession
        | GatewayCapacityKind::ReconnectLease => return Duration::ZERO,
    };
    duration_from_millis_env(
        name,
        if gateway_prod_like_env() {
            production_default_ms
        } else {
            0
        },
        0,
        600_000,
    )
}

fn duration_from_millis_env(name: &str, default_ms: u64, min_ms: u64, max_ms: u64) -> Duration {
    Duration::from_millis(
        std::env::var(name)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(default_ms)
            .clamp(min_ms, max_ms),
    )
}

fn runtime_tick_defer_duration_for_action(action: &SessionAction) -> Option<Duration> {
    match action {
        SessionAction::Packet(ClientPacket::StartGame { .. }) => {
            Some(gateway_runtime_tick_bootstrap_grace())
        }
        // Active input should wake the runtime tick loop, otherwise a queued
        // Crystal movement retry can inherit StartGame's bootstrap grace. Keep
        // a tiny batching window so follow-up input wins races against heavy
        // world ticks on the same WebSocket task.
        SessionAction::MoveTo { .. }
        | SessionAction::Packet(
            ClientPacket::Walk { .. } | ClientPacket::Run { .. } | ClientPacket::Turn { .. },
        ) => Some(gateway_runtime_tick_input_wake_grace()),
        _ => None,
    }
}

fn positive_usize_env(name: &str) -> Option<usize> {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn refresh_external_session_state(session: &mut GatewaySession) -> Result<bool, String> {
    catch_gateway_panic("web refresh_active_external_mail", || {
        session.refresh_active_external_mail()
    })
}

fn login_account_id_for_action(action: &SessionAction) -> Option<&str> {
    match action {
        SessionAction::Packet(ClientPacket::Login { account_id, .. }) => Some(account_id),
        SessionAction::PasskeyLogin { account_id, .. } => Some(account_id),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct LoginIdentityContext {
    account_id: String,
    auth_method: &'static str,
    credential_subject: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthSecurityAction {
    Login,
    Registration,
    PasswordChange,
    Recovery,
}

#[derive(Debug, Clone)]
struct AuthSecurityContext {
    account_id: String,
    action: AuthSecurityAction,
}

fn login_identity_context_for_action(action: &SessionAction) -> Option<LoginIdentityContext> {
    match action {
        SessionAction::Packet(ClientPacket::Login { account_id, .. }) => {
            Some(LoginIdentityContext {
                account_id: account_id.clone(),
                auth_method: "password",
                credential_subject: account_id.clone(),
            })
        }
        SessionAction::PasskeyLogin { account_id, .. } => Some(LoginIdentityContext {
            account_id: account_id.clone(),
            auth_method: "sui_passkey",
            credential_subject: account_id
                .strip_prefix("sui:")
                .unwrap_or(account_id)
                .to_string(),
        }),
        _ => None,
    }
}

fn auth_security_context_for_action(action: &SessionAction) -> Option<AuthSecurityContext> {
    match action {
        SessionAction::Packet(ClientPacket::Login { account_id, .. })
        | SessionAction::PasskeyLogin { account_id, .. } => Some(AuthSecurityContext {
            account_id: account_id.clone(),
            action: AuthSecurityAction::Login,
        }),
        SessionAction::Packet(ClientPacket::NewAccount { account_id, .. }) => {
            Some(AuthSecurityContext {
                account_id: account_id.clone(),
                action: AuthSecurityAction::Registration,
            })
        }
        SessionAction::Packet(ClientPacket::ChangePassword { account_id, .. }) => {
            Some(AuthSecurityContext {
                account_id: account_id.clone(),
                action: AuthSecurityAction::PasswordChange,
            })
        }
        _ => None,
    }
}

fn enforce_auth_rate_limits(
    cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    peer_address: &str,
    user_agent: &str,
    context: &AuthSecurityContext,
) -> Result<(), String> {
    let peer = identity.peer_fingerprint(peer_address)?;
    let device = identity.peer_fingerprint(&format!("device:{}", user_agent.trim()))?;
    let account = context.account_id.trim().to_ascii_lowercase();
    if account.is_empty() || account.len() > 160 {
        return Err("invalid authentication request".to_string());
    }
    let policies: Vec<(String, u64, u64)> = match context.action {
        AuthSecurityAction::Login => vec![
            (format!("login:pair:{peer}:{account}"), 8, 15 * 60),
            (format!("login:peer:{peer}"), 30, 15 * 60),
            (format!("login:device:{device}"), 30, 15 * 60),
            (format!("login:account:{account}"), 50, 15 * 60),
        ],
        AuthSecurityAction::Registration => vec![
            (format!("register:peer:{peer}"), 5, 60 * 60),
            (format!("register:device:{device}"), 5, 60 * 60),
            (format!("register:account:{account}"), 3, 60 * 60),
        ],
        AuthSecurityAction::PasswordChange => vec![
            (format!("password-change:pair:{peer}:{account}"), 5, 60 * 60),
            (format!("password-change:device:{device}"), 10, 60 * 60),
            (format!("password-change:account:{account}"), 20, 60 * 60),
        ],
        AuthSecurityAction::Recovery => vec![
            (format!("recovery:pair:{peer}:{account}"), 5, 60 * 60),
            (format!("recovery:peer:{peer}"), 15, 60 * 60),
            (format!("recovery:device:{device}"), 10, 60 * 60),
            (format!("recovery:account:{account}"), 20, 60 * 60),
        ],
    };
    for (scope, limit, window_seconds) in policies {
        let (attempts, ttl_ms) = cache.record_auth_attempt(&scope, window_seconds)?;
        if attempts > limit {
            let backoff = 1u64
                .checked_shl((attempts - limit).min(8) as u32)
                .unwrap_or(300)
                .clamp(2, 300);
            let retry_after = backoff.max(ttl_ms.saturating_add(999) / 1_000).min(3_600);
            let _ = identity.record_auth_security_event(
                Some(&context.account_id),
                "authentication_rate_limited",
                "blocked",
                "rate_limit_exceeded",
                peer_address,
                user_agent,
            );
            return Err(format!(
                "too many authentication attempts; retry after {retry_after} seconds"
            ));
        }
    }
    Ok(())
}

fn clear_successful_login_rate_limits(
    cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    peer_address: &str,
    account_id: &str,
) {
    let Ok(peer) = identity.peer_fingerprint(peer_address) else {
        return;
    };
    let account = account_id.trim().to_ascii_lowercase();
    for scope in [
        format!("login:pair:{peer}:{account}"),
        format!("login:account:{account}"),
    ] {
        if let Err(error) = cache.clear_auth_attempt(&scope) {
            eprintln!("failed to clear successful authentication counter: {error}");
        }
    }
}

fn start_game_character_index_for_action(action: &SessionAction) -> Option<i32> {
    match action {
        SessionAction::Packet(ClientPacket::StartGame { character_index }) => {
            Some(*character_index)
        }
        _ => None,
    }
}

fn is_explicit_session_leave_action(action: &SessionAction) -> bool {
    matches!(
        action,
        SessionAction::Packet(ClientPacket::Disconnect | ClientPacket::LogOut)
    )
}

fn keep_alive_time_for_action(action: &SessionAction) -> Option<i64> {
    match action {
        SessionAction::Packet(ClientPacket::KeepAlive { time }) => Some(*time),
        _ => None,
    }
}

fn try_acquire_start_game_route_lease(
    session_cache: &dyn crate::cache::GatewaySessionCache,
    session: &GatewaySession,
    authenticated: bool,
    authenticated_account_id: Option<&str>,
    character_index: Option<i32>,
) -> Result<Option<GatewaySessionCacheKey>, String> {
    let Some(character_index) = character_index else {
        return Ok(None);
    };
    if !authenticated || session.active_identity().is_some() {
        return Ok(None);
    }
    let account_id = authenticated_account_id
        .ok_or_else(|| "authenticated account is required before StartGame".to_string())?;
    let key = GatewaySessionCacheKey {
        account_id: account_id.to_string(),
        character_index,
    };
    match session_cache.acquire_route_lease(&key, session.session_id(), route_lease_ttl_seconds()) {
        Ok(_) => Ok(Some(key)),
        Err(error) => {
            eprintln!(
                "web StartGame route lease rejected for {}/{}: {error}",
                key.account_id, key.character_index
            );
            Err("character is already online or route lease is unavailable".to_string())
        }
    }
}

fn release_pending_start_game_route_lease(
    session_cache: &dyn crate::cache::GatewaySessionCache,
    session: &GatewaySession,
    key: Option<&GatewaySessionCacheKey>,
) {
    if let Some(key) = key {
        if let Err(error) = session_cache.release_route_lease(key, session.session_id()) {
            eprintln!(
                "web StartGame route lease release skipped for {}/{}: {error}",
                key.account_id, key.character_index
            );
        }
    }
}

fn release_unclaimed_start_game_route_lease(
    session_cache: &dyn crate::cache::GatewaySessionCache,
    session: &GatewaySession,
    key: Option<&GatewaySessionCacheKey>,
) {
    let Some(key) = key else {
        return;
    };
    let claimed = session.active_identity().is_some_and(|identity| {
        identity.account_id == key.account_id && identity.character_index == key.character_index
    });
    if !claimed {
        release_pending_start_game_route_lease(session_cache, session, Some(key));
    }
}

fn inflight_capacity_kind_for_action(action: &SessionAction) -> Option<GatewayCapacityKind> {
    match action {
        SessionAction::Packet(ClientPacket::Login { .. }) | SessionAction::PasskeyLogin { .. } => {
            Some(GatewayCapacityKind::Login)
        }
        SessionAction::Packet(ClientPacket::NewCharacter { .. }) => {
            Some(GatewayCapacityKind::NewCharacter)
        }
        SessionAction::Packet(ClientPacket::StartGame { .. }) => {
            Some(GatewayCapacityKind::StartGame)
        }
        _ => None,
    }
}

fn should_queue_save_for_action(action: &SessionAction) -> bool {
    !matches!(
        action,
        SessionAction::Packet(
            ClientPacket::ClientVersion { .. }
                | ClientPacket::Login { .. }
                | ClientPacket::KeepAlive { .. }
                | ClientPacket::Turn { .. }
                | ClientPacket::Walk { .. }
                | ClientPacket::Run { .. }
                | ClientPacket::Chat { .. }
        ) | SessionAction::PasskeyLogin { .. }
            | SessionAction::Tick
    )
}

fn execute_session_action(
    session: &mut GatewaySession,
    action: SessionAction,
    authenticated: bool,
    enforce_player_command_safety: bool,
) -> Result<Vec<ServerPacket>, String> {
    let move_log = move_log_for_action(&action);
    if matches!(
        &action,
        SessionAction::Packet(ClientPacket::SendMail { .. })
    ) {
        if !authenticated {
            return Err("authenticated account is required to send mail".to_string());
        }
        if session.active_identity().is_none() {
            return Err("an active in-game character is required to send mail".to_string());
        }
    }
    if !authenticated
        && matches!(
            &action,
            SessionAction::GameShopBuy { .. }
                | SessionAction::Packet(ClientPacket::GameShopBuy { .. })
        )
    {
        return Err("authenticated account is required for game shop purchases".to_string());
    }
    if let SessionAction::QaControl { token, action } = action {
        if enforce_player_command_safety {
            return Err(
                "QA control requires the explicit local dev/test unsafe opt-out".to_string(),
            );
        }
        return execute_qa_control_action(session, &token, action);
    }
    if enforce_player_command_safety {
        return execute_production_session_action(session, action, authenticated, move_log);
    }
    match action {
        SessionAction::Packet(packet) => {
            let responses = session.handle_packet(packet);
            log_move_action(move_log, &responses);
            Ok(responses)
        }
        SessionAction::GameShopBuy {
            g_index,
            quantity,
            price_type,
            ..
        } => Ok(session.handle_packet(ClientPacket::GameShopBuy {
            g_index,
            quantity,
            price_type,
        })),
        SessionAction::PasskeyLogin {
            account_id,
            proof_account_id,
            token,
        } => {
            verify_passkey_gateway_token(&proof_account_id, &token)?;
            Ok(session.passkey_login(&account_id))
        }
        SessionAction::MoveTo { x, y, running } => {
            let responses = session.move_to(x, y, running);
            log_move_action(move_log, &responses);
            Ok(responses)
        }
        SessionAction::Attack { object_id } => Ok(session.attack(object_id)),
        SessionAction::Interact { object_id } => Ok(session.interact(object_id)),
        SessionAction::SelectNpcDialog { target } => Ok(session.select_npc_dialog_target(&target)),
        SessionAction::SubmitNpcInput { value } => Ok(session.submit_npc_input(&value)),
        SessionAction::PickUp { object_id } => Ok(session.pick_up(object_id)),
        SessionAction::UseItem { key } => Ok(session.use_item(&key)),
        SessionAction::DropItem { key } => Ok(session.drop_item(&key)),
        SessionAction::CastSkill { key } => Ok(session.cast_skill(&key)),
        SessionAction::TransferMap { key } => Ok(session.transfer_map(&key)),
        SessionAction::Stage5Command { action, args } => Ok(session.stage5_command(&action, args)),
        SessionAction::QaControl { token, action } => {
            execute_qa_control_action(session, &token, action)
        }
        SessionAction::SetLanguage { language } => session.set_language(&language).map(|_| vec![]),
        SessionAction::Tick => Ok(session.tick()),
    }
}

fn execute_production_session_action(
    session: &mut GatewaySession,
    action: SessionAction,
    authenticated: bool,
    move_log: Option<String>,
) -> Result<Vec<ServerPacket>, String> {
    if !authenticated
        && matches!(
            &action,
            SessionAction::GameShopBuy { .. }
                | SessionAction::Packet(ClientPacket::GameShopBuy { .. })
        )
    {
        return Err("authenticated account is required for game shop purchases".to_string());
    }
    let zone_owner_lease = session.zone_owner_lease().clone();
    let execution = match action {
        SessionAction::Packet(packet) => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::ClientPacket(packet),
            )?,
        SessionAction::GameShopBuy {
            g_index,
            quantity,
            price_type,
            ..
        } => session.execute_production_player_command_with_zone_owner_lease(
            &zone_owner_lease,
            authenticated,
            WorldCommand::ClientPacket(ClientPacket::GameShopBuy {
                g_index,
                quantity,
                price_type,
            }),
        )?,
        SessionAction::PasskeyLogin {
            account_id,
            proof_account_id,
            token,
        } => {
            verify_passkey_gateway_token(&proof_account_id, &token)?;
            return Ok(session.passkey_login(&account_id));
        }
        SessionAction::MoveTo { x, y, running } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::MoveTo {
                    position: Point { x, y },
                    running,
                },
            )?,
        SessionAction::Attack { object_id } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::Attack { object_id },
            )?,
        SessionAction::Interact { object_id } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::Interact { object_id },
            )?,
        SessionAction::SelectNpcDialog { target } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::SelectNpcDialog { target },
            )?,
        SessionAction::SubmitNpcInput { value } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::SubmitNpcInput { value },
            )?,
        SessionAction::PickUp { object_id } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::PickUp { object_id },
            )?,
        SessionAction::UseItem { key } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::UseItem { key },
            )?,
        SessionAction::DropItem { key } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::DropItem { key },
            )?,
        SessionAction::CastSkill { key } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::CastSkill { key },
            )?,
        SessionAction::TransferMap { key } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::TransferMap { key },
            )?,
        SessionAction::Stage5Command { action, args } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::Stage5Command { action, args },
            )?,
        SessionAction::QaControl { .. } => {
            return Err("QA control is not allowed on the production player path".to_string());
        }
        SessionAction::SetLanguage { language } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::SetLanguage { language },
            )?,
        SessionAction::Tick => session.execute_production_player_command_with_zone_owner_lease(
            &zone_owner_lease,
            authenticated,
            WorldCommand::Tick,
        )?,
    };
    log_move_action(move_log, &execution.packets);
    Ok(execution.packets)
}

fn execute_qa_control_action(
    session: &mut GatewaySession,
    token: &str,
    action: QaControlAction,
) -> Result<Vec<ServerPacket>, String> {
    verify_qa_control_token(token)?;
    match action {
        QaControlAction::TransferMap { key } => Ok(session.transfer_map(&key)),
        QaControlAction::Stage5Command { action, args } => {
            Ok(session.stage5_command(&action, args))
        }
        QaControlAction::Chat { message } => Ok(session.handle_packet(ClientPacket::Chat {
            message,
            linked_items: Vec::new(),
        })),
        QaControlAction::Tick => Ok(session.tick()),
    }
}

fn verify_qa_control_token(provided: &str) -> Result<(), String> {
    let expected = env::var("MIR2_GATEWAY_QA_CONTROL_TOKEN")
        .map_err(|_| "QA control is disabled; set MIR2_GATEWAY_QA_CONTROL_TOKEN".to_string())?;
    if expected.trim().is_empty() {
        return Err("QA control is disabled; MIR2_GATEWAY_QA_CONTROL_TOKEN is empty".to_string());
    }
    if constant_time_eq(expected.as_bytes(), provided.as_bytes()) {
        Ok(())
    } else {
        Err("invalid QA control token".to_string())
    }
}

fn constant_time_eq(expected: &[u8], provided: &[u8]) -> bool {
    if expected.len() != provided.len() {
        return false;
    }
    expected
        .iter()
        .zip(provided.iter())
        .fold(0_u8, |diff, (left, right)| diff | (left ^ right))
        == 0
}

fn update_authenticated_state(current: bool, responses: &[ServerPacket]) -> bool {
    if responses
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. }))
    {
        return true;
    }
    if responses.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::Login { .. }
                | ServerPacket::LoginBanned { .. }
                | ServerPacket::ReturnToLogin
        )
    }) {
        return false;
    }
    current
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Gate15IdentityBatch {
    account_id: String,
    characters: Vec<(i32, String)>,
}

fn gate15_identity_batch_for_responses(
    authenticated_account_id: Option<&str>,
    login_account_id: Option<&str>,
    responses: &[ServerPacket],
) -> Result<Option<Gate15IdentityBatch>, String> {
    let has_identity_update = responses.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::LoginSuccess { .. } | ServerPacket::NewCharacterSuccess { .. }
        )
    });
    if !has_identity_update {
        return Ok(None);
    }
    let account_id = login_account_id
        .or(authenticated_account_id)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "authenticated account is required for Commonware identity finalization".to_string()
        })?;
    let mut characters = Vec::new();
    for packet in responses {
        match packet {
            ServerPacket::LoginSuccess {
                characters: login_characters,
            } => {
                characters.extend(
                    login_characters
                        .iter()
                        .map(|character| (character.index, character.name.clone())),
                );
            }
            ServerPacket::NewCharacterSuccess { char_info } => {
                characters.push((char_info.index, char_info.name.clone()));
            }
            _ => {}
        }
    }
    characters.sort_by_key(|(index, _)| *index);
    characters.dedup_by(|left, right| left.0 == right.0);
    Ok(Some(Gate15IdentityBatch {
        account_id: account_id.to_string(),
        characters,
    }))
}

async fn finalize_gate15_identities_for_responses(
    authenticated_account_id: Option<&str>,
    login_account_id: Option<&str>,
    responses: &[ServerPacket],
) -> Result<(), String> {
    let Some(batch) =
        gate15_identity_batch_for_responses(authenticated_account_id, login_account_id, responses)?
    else {
        return Ok(());
    };
    if let Some(finalized_height) =
        crate::gate15::finalize_player_identities(&batch.account_id, &batch.characters).await?
    {
        eprintln!(
            "Gate 15 finalized account {} and {} character identities at height {}",
            batch.account_id,
            batch.characters.len(),
            finalized_height
        );
    }
    Ok(())
}

const UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT: &str =
    "MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS";

fn production_player_command_safety_enabled(tcp_peer_ip: IpAddr) -> bool {
    // Player-command safety is the default in every environment. The sole
    // escape hatch is deliberately limited to an explicitly labelled dev/test
    // process reached from a loopback peer; production and staging always win.
    if gateway_prod_like_env() {
        return true;
    }
    let dev_or_test = ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"]
        .into_iter()
        .filter_map(|name| env::var(name).ok())
        .any(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "development" | "dev" | "test" | "testing"
            )
        });
    let loopback_peer = tcp_peer_ip.is_loopback();
    !(env_flag_enabled(UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT) && dev_or_test && loopback_peer)
}

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn move_log_for_action(action: &SessionAction) -> Option<String> {
    if !move_logging_enabled() {
        return None;
    }

    match action {
        SessionAction::MoveTo { x, y, running } => Some(format!(
            "MoveTo target=({x},{y}) mode={}",
            if *running { "run" } else { "walk" }
        )),
        SessionAction::Packet(ClientPacket::Walk { direction }) => {
            Some(format!("Walk direction={direction:?}"))
        }
        SessionAction::Packet(ClientPacket::Run { direction }) => {
            Some(format!("Run direction={direction:?}"))
        }
        SessionAction::Packet(ClientPacket::Turn { direction }) => {
            Some(format!("Turn direction={direction:?}"))
        }
        _ => None,
    }
}

fn move_logging_enabled() -> bool {
    env_flag_enabled("MIR2_GATEWAY_MOVE_LOG")
}

fn log_move_action(action: Option<String>, responses: &[ServerPacket]) {
    let Some(action) = action else {
        return;
    };

    let movement = responses.iter().find_map(|packet| match packet {
        ServerPacket::UserLocation { location } => Some(format!(
            "UserLocation=({}, {}) {:?}",
            location.position.x, location.position.y, location.direction
        )),
        ServerPacket::ObjectWalk { movement } => Some(format!(
            "ObjectWalk=({}, {}) {:?}",
            movement.position.x, movement.position.y, movement.direction
        )),
        ServerPacket::ObjectRun { movement } => Some(format!(
            "ObjectRun=({}, {}) {:?}",
            movement.position.x, movement.position.y, movement.direction
        )),
        _ => None,
    });

    eprintln!(
        "mir2-gateway movement {action} -> {} packets={} ",
        movement.unwrap_or_else(|| "no movement packet".to_string()),
        responses.len()
    );
}

const MIN_BIG_MAP_SEARCH_CHARS: usize = 3;
const MAX_BIG_MAP_SEARCH_CHARS: usize = 64;

fn normalize_big_map_search_text(text: &str) -> Result<String, String> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let char_count = normalized.chars().count();
    if char_count < MIN_BIG_MAP_SEARCH_CHARS {
        return Err(format!(
            "searchMap text must contain at least {MIN_BIG_MAP_SEARCH_CHARS} characters"
        ));
    }
    if char_count > MAX_BIG_MAP_SEARCH_CHARS {
        return Err(format!(
            "searchMap text must not exceed {MAX_BIG_MAP_SEARCH_CHARS} characters"
        ));
    }
    Ok(normalized)
}

fn browser_command_to_action(command: BrowserCommand) -> Result<SessionAction, String> {
    match command {
        BrowserCommand::ClientVersion => Ok(SessionAction::Packet(ClientPacket::ClientVersion {
            version_hash: Vec::new(),
        })),
        BrowserCommand::ClientCapabilities { .. } | BrowserCommand::ResumeSession(_) => Err(
            "native resume control commands must be handled before gameplay dispatch".to_string(),
        ),
        BrowserCommand::Disconnect => Ok(SessionAction::Packet(ClientPacket::Disconnect)),
        BrowserCommand::TownRevive => Ok(SessionAction::Packet(ClientPacket::TownRevive)),
        BrowserCommand::Login {
            account_id,
            password,
        } => Ok(SessionAction::Packet(ClientPacket::Login {
            account_id,
            password,
        })),
        BrowserCommand::PasskeyLogin { account_id, token } => Ok(SessionAction::PasskeyLogin {
            proof_account_id: account_id.clone(),
            account_id,
            token,
        }),
        BrowserCommand::NewAccount {
            account_id,
            password,
            birth_date_binary,
            user_name,
            secret_question,
            secret_answer,
            email_address,
        } => Ok(SessionAction::Packet(ClientPacket::NewAccount {
            account_id,
            password,
            birth_date_binary,
            user_name,
            secret_question,
            secret_answer,
            email_address,
        })),
        BrowserCommand::ChangePassword {
            account_id,
            current_password,
            new_password,
        } => Ok(SessionAction::Packet(ClientPacket::ChangePassword {
            account_id,
            current_password,
            new_password,
        })),
        BrowserCommand::UnlockStorage { password } => {
            Ok(SessionAction::Packet(ClientPacket::UnlockStorage {
                password,
            }))
        }
        BrowserCommand::SetStoragePassword {
            current_password,
            new_password,
        } => Ok(SessionAction::Packet(ClientPacket::SetStoragePassword {
            current_password,
            new_password,
        })),
        BrowserCommand::RemoveStoragePassword { current_password } => {
            Ok(SessionAction::Packet(ClientPacket::RemoveStoragePassword {
                current_password,
            }))
        }
        BrowserCommand::NewCharacter {
            name,
            gender,
            class,
        } => Ok(SessionAction::Packet(ClientPacket::NewCharacter {
            name,
            gender: parse_gender(&gender)?,
            class: parse_class(&class)?,
        })),
        BrowserCommand::NewHero {
            name,
            gender,
            class,
        } => Ok(SessionAction::Packet(ClientPacket::NewHero {
            name,
            gender: parse_gender(&gender)?,
            class: parse_class(&class)?,
        })),
        BrowserCommand::StartGame { character_index } => {
            Ok(SessionAction::Packet(ClientPacket::StartGame {
                character_index,
            }))
        }
        BrowserCommand::Turn { direction } => Ok(SessionAction::Packet(ClientPacket::Turn {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Walk { direction } => Ok(SessionAction::Packet(ClientPacket::Walk {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Run { direction } => Ok(SessionAction::Packet(ClientPacket::Run {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Chat { message } => Ok(SessionAction::Packet(ClientPacket::Chat {
            message,
            linked_items: Vec::new(),
        })),
        BrowserCommand::KeepAlive { time } => {
            Ok(SessionAction::Packet(ClientPacket::KeepAlive { time }))
        }
        BrowserCommand::MoveTo { x, y, mode } => Ok(SessionAction::MoveTo {
            x,
            y,
            running: parse_move_mode(mode.as_deref())?,
        }),
        BrowserCommand::Attack { object_id } => Ok(SessionAction::Attack { object_id }),
        BrowserCommand::AttackDirection { direction, spell } => {
            Ok(SessionAction::Packet(ClientPacket::Attack {
                direction: parse_direction(&direction)?,
                spell: parse_spell(spell.unwrap_or(0))?,
            }))
        }
        BrowserCommand::RangeAttack {
            direction,
            x,
            y,
            target_id,
            target_x,
            target_y,
        } => Ok(SessionAction::Packet(ClientPacket::RangeAttack {
            direction: parse_direction(&direction)?,
            location: Point { x, y },
            target_id,
            target_location: Point {
                x: target_x,
                y: target_y,
            },
        })),
        BrowserCommand::Harvest { direction } => Ok(SessionAction::Packet(ClientPacket::Harvest {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Interact { object_id } => Ok(SessionAction::Interact { object_id }),
        BrowserCommand::SelectNpcDialog { target } => Ok(SessionAction::SelectNpcDialog { target }),
        BrowserCommand::SubmitNpcInput { value } => Ok(SessionAction::SubmitNpcInput { value }),
        BrowserCommand::PickUp { object_id } => Ok(SessionAction::PickUp { object_id }),
        BrowserCommand::PickUpTile => Ok(SessionAction::Packet(ClientPacket::PickUp)),
        BrowserCommand::UseItem {
            key,
            unique_id,
            slot,
            grid,
        } => {
            if let Some(unique_id) = unique_id {
                Ok(SessionAction::Packet(ClientPacket::UseItem {
                    unique_id,
                    grid: parse_grid(grid.as_deref().unwrap_or("inventory"))?,
                }))
            } else if let Some(slot) = slot {
                Ok(SessionAction::Packet(ClientPacket::UseItem {
                    unique_id: u64::from(slot),
                    grid: parse_grid(grid.as_deref().unwrap_or("inventory"))?,
                }))
            } else if let Some(key) = key {
                Ok(SessionAction::UseItem { key })
            } else {
                Err("useItem requires key, uniqueId, or slot".to_string())
            }
        }
        BrowserCommand::MoveItem { grid, from, to } => {
            Ok(SessionAction::Packet(ClientPacket::MoveItem {
                grid: parse_grid(&grid)?,
                from,
                to,
            }))
        }
        BrowserCommand::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
        } => Ok(SessionAction::Packet(ClientPacket::MergeItem {
            grid_from: parse_grid(&grid_from)?,
            grid_to: parse_grid(&grid_to)?,
            id_from,
            id_to,
        })),
        BrowserCommand::EquipItem {
            unique_id,
            grid,
            to,
        } => Ok(SessionAction::Packet(ClientPacket::EquipItem {
            grid: parse_grid(&grid)?,
            unique_id,
            to,
        })),
        BrowserCommand::RemoveItem {
            unique_id,
            grid,
            to,
        } => Ok(SessionAction::Packet(ClientPacket::RemoveItem {
            grid: parse_grid(&grid)?,
            unique_id,
            to,
        })),
        BrowserCommand::RemoveSlotItem {
            unique_id,
            grid,
            grid_to,
            from_unique_id,
            to,
        } => Ok(SessionAction::Packet(ClientPacket::RemoveSlotItem {
            grid: parse_grid(&grid)?,
            grid_to: parse_grid(&grid_to)?,
            unique_id,
            to,
            from_unique_id,
        })),
        BrowserCommand::SplitItem {
            unique_id,
            grid,
            count,
        } => Ok(SessionAction::Packet(ClientPacket::SplitItem {
            grid: parse_grid(&grid)?,
            unique_id,
            count,
        })),
        BrowserCommand::StoreItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::StoreItem { from, to }))
        }
        BrowserCommand::TakeBackItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::TakeBackItem {
                from,
                to,
            }))
        }
        BrowserCommand::StoreItemV2 {
            request_id,
            from,
            to,
        } => {
            if mir2_protocol::is_valid_request_id(&request_id) {
                Ok(SessionAction::Packet(ClientPacket::StoreItemV2 {
                    request_id,
                    from,
                    to,
                }))
            } else {
                Err("invalid storage requestId".to_string())
            }
        }
        BrowserCommand::TakeBackItemV2 {
            request_id,
            from,
            to,
        } => {
            if mir2_protocol::is_valid_request_id(&request_id) {
                Ok(SessionAction::Packet(ClientPacket::TakeBackItemV2 {
                    request_id,
                    from,
                    to,
                }))
            } else {
                Err("invalid storage requestId".to_string())
            }
        }
        BrowserCommand::TakeBackHeroItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::TakeBackHeroItem {
                from,
                to,
            }))
        }
        BrowserCommand::TransferHeroItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::TransferHeroItem {
                from,
                to,
            }))
        }
        BrowserCommand::DropItem {
            key,
            unique_id,
            count,
            hero_inventory,
        } => {
            if let Some(unique_id) = unique_id {
                Ok(SessionAction::Packet(ClientPacket::DropItem {
                    unique_id,
                    count,
                    hero_inventory,
                }))
            } else {
                Ok(SessionAction::DropItem { key })
            }
        }
        BrowserCommand::DeleteItem {
            unique_id,
            count,
            hero_inventory,
        } => Ok(SessionAction::Packet(ClientPacket::DeleteItem {
            unique_id,
            count,
            hero_inventory,
        })),
        BrowserCommand::DropGold { amount } => {
            Ok(SessionAction::Packet(ClientPacket::DropGold { amount }))
        }
        BrowserCommand::RequestMapInfo { map_index } => {
            if map_index <= 0 {
                return Err("requestMapInfo requires a positive mapIndex".to_string());
            }
            Ok(SessionAction::Packet(ClientPacket::RequestMapInfo {
                map_index,
            }))
        }
        BrowserCommand::SearchMap { text } => {
            let text = normalize_big_map_search_text(&text)?;
            Ok(SessionAction::Packet(ClientPacket::SearchMap { text }))
        }
        BrowserCommand::TeleportToNpc { object_id } => {
            if object_id == 0 {
                return Err("teleportToNpc requires a non-zero objectId".to_string());
            }
            Ok(SessionAction::Packet(ClientPacket::TeleportToNpc {
                object_id,
            }))
        }
        BrowserCommand::RequestItemInfo { item_index } => {
            Ok(SessionAction::Packet(ClientPacket::RequestItemInfo {
                item_index,
            }))
        }
        BrowserCommand::SellItem { unique_id, count } => {
            Ok(SessionAction::Packet(ClientPacket::SellItem {
                unique_id,
                count,
            }))
        }
        BrowserCommand::BuyItem {
            item_index,
            count,
            panel_type,
        } => Ok(SessionAction::Packet(ClientPacket::BuyItem {
            item_index,
            count,
            panel_type,
        })),
        BrowserCommand::GameShopBuy {
            request_id,
            g_index,
            quantity,
            price_type,
        } => Ok(SessionAction::GameShopBuy {
            request_id,
            g_index,
            quantity,
            price_type,
        }),
        BrowserCommand::RepairItem { unique_id } => {
            Ok(SessionAction::Packet(ClientPacket::RepairItem {
                unique_id,
            }))
        }
        BrowserCommand::SpecialRepairItem { unique_id } => {
            Ok(SessionAction::Packet(ClientPacket::SRepairItem {
                unique_id,
            }))
        }
        BrowserCommand::DeleteCharacter { character_index } => {
            Ok(SessionAction::Packet(ClientPacket::DeleteCharacter {
                character_index,
            }))
        }
        BrowserCommand::MagicKey {
            spell,
            key,
            old_key,
        } => Ok(SessionAction::Packet(ClientPacket::MagicKey {
            spell: parse_spell_name(&spell)?,
            key,
            old_key,
        })),
        BrowserCommand::Magic {
            object_id,
            spell,
            direction,
            target_id,
            x,
            y,
            spell_target_lock,
        } => Ok(SessionAction::Packet(ClientPacket::Magic {
            object_id,
            spell: parse_spell_name(&spell)?,
            direction: parse_direction(&direction)?,
            target_id,
            location: Point { x, y },
            spell_target_lock,
        })),
        BrowserCommand::SpellToggle {
            spell,
            toggle_state,
            can_use,
        } => Ok(SessionAction::Packet(ClientPacket::SpellToggle {
            spell: parse_spell_name(&spell)?,
            toggle_state: toggle_state.unwrap_or_else(|| match can_use {
                Some(true) => 1,
                Some(false) => 0,
                None => -1,
            }),
        })),
        BrowserCommand::SetHeroBehaviour { behaviour } => {
            Ok(SessionAction::Packet(ClientPacket::SetHeroBehaviour {
                behaviour,
            }))
        }
        BrowserCommand::ChangeHero { list_index } => {
            Ok(SessionAction::Packet(ClientPacket::ChangeHero {
                list_index,
            }))
        }
        BrowserCommand::SetAutoPotValue { stat, value } => {
            Ok(SessionAction::Packet(ClientPacket::SetAutoPotValue {
                stat,
                value,
            }))
        }
        BrowserCommand::SetAutoPotItem { grid, item_index } => {
            Ok(SessionAction::Packet(ClientPacket::SetAutoPotItem {
                grid: parse_hero_autopot_grid(&grid)?,
                item_index,
            }))
        }
        BrowserCommand::ChangeAMode { mode } => {
            Ok(SessionAction::Packet(ClientPacket::ChangeAMode { mode }))
        }
        BrowserCommand::ChangePMode { mode } => {
            Ok(SessionAction::Packet(ClientPacket::ChangePMode { mode }))
        }
        BrowserCommand::OpenDoor { door_index } => {
            Ok(SessionAction::Packet(ClientPacket::OpenDoor { door_index }))
        }
        BrowserCommand::ConsignItem {
            unique_id,
            price,
            market_type,
        } => Ok(SessionAction::Packet(ClientPacket::ConsignItem {
            unique_id,
            price,
            market_type,
        })),
        BrowserCommand::MarketSearch {
            match_text,
            item_type,
            user_mode,
            min_shape,
            max_shape,
            market_type,
        } => Ok(SessionAction::Packet(ClientPacket::MarketSearch {
            match_text,
            item_type,
            user_mode,
            min_shape,
            max_shape,
            market_type,
        })),
        BrowserCommand::MarketRefresh => Ok(SessionAction::Packet(ClientPacket::MarketRefresh)),
        BrowserCommand::MarketPage { page } => {
            Ok(SessionAction::Packet(ClientPacket::MarketPage { page }))
        }
        BrowserCommand::MarketBuy {
            auction_id,
            bid_price,
        } => Ok(SessionAction::Packet(ClientPacket::MarketBuy {
            auction_id,
            bid_price,
        })),
        BrowserCommand::MarketGetBack { mode, auction_id } => {
            Ok(SessionAction::Packet(ClientPacket::MarketGetBack {
                mode,
                auction_id,
            }))
        }
        BrowserCommand::MarketSellNow { auction_id } => {
            Ok(SessionAction::Packet(ClientPacket::MarketSellNow {
                auction_id,
            }))
        }
        BrowserCommand::MarriageRequest => Ok(SessionAction::Packet(ClientPacket::MarriageRequest)),
        BrowserCommand::MarriageReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::MarriageReply {
                accept_invite,
            }))
        }
        BrowserCommand::ChangeMarriage => Ok(SessionAction::Packet(ClientPacket::ChangeMarriage)),
        BrowserCommand::DivorceRequest => Ok(SessionAction::Packet(ClientPacket::DivorceRequest)),
        BrowserCommand::DivorceReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::DivorceReply {
                accept_invite,
            }))
        }
        BrowserCommand::AddMentor { name } => {
            Ok(SessionAction::Packet(ClientPacket::AddMentor { name }))
        }
        BrowserCommand::MentorReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::MentorReply {
                accept_invite,
            }))
        }
        BrowserCommand::AllowMentor => Ok(SessionAction::Packet(ClientPacket::AllowMentor)),
        BrowserCommand::CancelMentor => Ok(SessionAction::Packet(ClientPacket::CancelMentor)),
        BrowserCommand::DepositTradeItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::DepositTradeItem {
                from,
                to,
            }))
        }
        BrowserCommand::RetrieveTradeItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::RetrieveTradeItem {
                from,
                to,
            }))
        }
        BrowserCommand::TradeRequest => Ok(SessionAction::Packet(ClientPacket::TradeRequest)),
        BrowserCommand::TradeReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::TradeReply {
                accept_invite,
            }))
        }
        BrowserCommand::TradeGold { amount } => {
            Ok(SessionAction::Packet(ClientPacket::TradeGold { amount }))
        }
        BrowserCommand::TradeConfirm { locked } => {
            Ok(SessionAction::Packet(ClientPacket::TradeConfirm { locked }))
        }
        BrowserCommand::TradeCancel => Ok(SessionAction::Packet(ClientPacket::TradeCancel)),
        BrowserCommand::FishingCast { cast_out } => {
            Ok(SessionAction::Packet(ClientPacket::FishingCast {
                cast_out,
            }))
        }
        BrowserCommand::FishingChangeAutocast { auto_cast } => {
            Ok(SessionAction::Packet(ClientPacket::FishingChangeAutocast {
                auto_cast,
            }))
        }
        BrowserCommand::SendMail {
            name,
            message,
            gold,
            items_idx,
            stamped,
        } => Ok(SessionAction::Packet(ClientPacket::SendMail {
            name,
            message,
            gold,
            items_idx,
            stamped,
        })),
        BrowserCommand::ReadMail { mail_id } => {
            Ok(SessionAction::Packet(ClientPacket::ReadMail { mail_id }))
        }
        BrowserCommand::CollectParcel { mail_id } => {
            Ok(SessionAction::Packet(ClientPacket::CollectParcel {
                mail_id,
            }))
        }
        BrowserCommand::DeleteMail { mail_id } => {
            Ok(SessionAction::Packet(ClientPacket::DeleteMail { mail_id }))
        }
        BrowserCommand::LockMail { mail_id, lock } => {
            Ok(SessionAction::Packet(ClientPacket::LockMail {
                mail_id,
                lock,
            }))
        }
        BrowserCommand::MailLockedItem { unique_id, locked } => {
            Ok(SessionAction::Packet(ClientPacket::MailLockedItem {
                unique_id,
                locked,
            }))
        }
        BrowserCommand::MailCost {
            gold,
            items_idx,
            stamped,
        } => Ok(SessionAction::Packet(ClientPacket::MailCost {
            gold,
            items_idx,
            stamped,
        })),
        BrowserCommand::UpdateIntelligentCreature {
            creature,
            summon_me,
            unsummon_me,
            release_me,
        } => Ok(SessionAction::Packet(
            ClientPacket::UpdateIntelligentCreature {
                creature,
                summon_me,
                unsummon_me,
                release_me,
            },
        )),
        BrowserCommand::IntelligentCreaturePickup {
            mouse_mode,
            location,
        } => Ok(SessionAction::Packet(
            ClientPacket::IntelligentCreaturePickup {
                mouse_mode,
                location,
            },
        )),
        BrowserCommand::RequestIntelligentCreatureUpdates { update } => Ok(SessionAction::Packet(
            ClientPacket::RequestIntelligentCreatureUpdates { update },
        )),
        BrowserCommand::AddFriend { name, blocked } => {
            Ok(SessionAction::Packet(ClientPacket::AddFriend {
                name,
                blocked,
            }))
        }
        BrowserCommand::RemoveFriend { character_index } => {
            Ok(SessionAction::Packet(ClientPacket::RemoveFriend {
                character_index,
            }))
        }
        BrowserCommand::RefreshFriends => Ok(SessionAction::Packet(ClientPacket::RefreshFriends)),
        BrowserCommand::AddMemo {
            character_index,
            memo,
        } => Ok(SessionAction::Packet(ClientPacket::AddMemo {
            character_index,
            memo,
        })),
        BrowserCommand::GetRanking {
            rank_type,
            rank_index,
            online_only,
        } => Ok(SessionAction::Packet(ClientPacket::GetRanking {
            rank_type,
            rank_index,
            online_only,
        })),
        BrowserCommand::GetRentedItems => Ok(SessionAction::Packet(ClientPacket::GetRentedItems)),
        BrowserCommand::ItemRentalRequest => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalRequest))
        }
        BrowserCommand::ItemRentalFee { amount } => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalFee {
                amount,
            }))
        }
        BrowserCommand::ItemRentalPeriod { days } => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalPeriod {
                days,
            }))
        }
        BrowserCommand::DepositRentalItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::DepositRentalItem {
                from,
                to,
            }))
        }
        BrowserCommand::RetrieveRentalItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::RetrieveRentalItem {
                from,
                to,
            }))
        }
        BrowserCommand::CancelItemRental => {
            Ok(SessionAction::Packet(ClientPacket::CancelItemRental))
        }
        BrowserCommand::ItemRentalLockFee => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalLockFee))
        }
        BrowserCommand::ItemRentalLockItem => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalLockItem))
        }
        BrowserCommand::ConfirmItemRental => {
            Ok(SessionAction::Packet(ClientPacket::ConfirmItemRental))
        }
        BrowserCommand::AcceptQuest {
            npc_index,
            quest_index,
        } => Ok(SessionAction::Packet(ClientPacket::AcceptQuest {
            npc_index,
            quest_index,
        })),
        BrowserCommand::FinishQuest {
            quest_index,
            selected_item_index,
        } => Ok(SessionAction::Packet(ClientPacket::FinishQuest {
            quest_index,
            selected_item_index,
        })),
        BrowserCommand::AbandonQuest { quest_index } => {
            Ok(SessionAction::Packet(ClientPacket::AbandonQuest {
                quest_index,
            }))
        }
        BrowserCommand::ShareQuest { quest_index } => {
            Ok(SessionAction::Packet(ClientPacket::ShareQuest {
                quest_index,
            }))
        }
        BrowserCommand::SwitchGroup { allow_group } => {
            Ok(SessionAction::Packet(ClientPacket::SwitchGroup {
                allow_group,
            }))
        }
        BrowserCommand::AddMember { name } => {
            Ok(SessionAction::Packet(ClientPacket::AddMember { name }))
        }
        BrowserCommand::DelMember { name } => {
            Ok(SessionAction::Packet(ClientPacket::DelMember { name }))
        }
        BrowserCommand::GroupInvite { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::GroupInvite {
                accept_invite,
            }))
        }
        BrowserCommand::EditGuildMember {
            change_type,
            rank_index,
            name,
            rank_name,
        } => Ok(SessionAction::Packet(ClientPacket::EditGuildMember {
            change_type,
            rank_index,
            name,
            rank_name,
        })),
        BrowserCommand::EditGuildNotice { notice } => {
            Ok(SessionAction::Packet(ClientPacket::EditGuildNotice {
                notice,
            }))
        }
        BrowserCommand::GuildInvite { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::GuildInvite {
                accept_invite,
            }))
        }
        BrowserCommand::GuildNameReturn { name } => {
            Ok(SessionAction::Packet(ClientPacket::GuildNameReturn {
                name,
            }))
        }
        BrowserCommand::RequestGuildInfo { info_type } => {
            Ok(SessionAction::Packet(ClientPacket::RequestGuildInfo {
                info_type,
            }))
        }
        BrowserCommand::GuildStorageGoldChange {
            change_type,
            amount,
        } => Ok(SessionAction::Packet(
            ClientPacket::GuildStorageGoldChange {
                change_type,
                amount,
            },
        )),
        BrowserCommand::GuildStorageItemChange {
            change_type,
            from,
            to,
        } => Ok(SessionAction::Packet(
            ClientPacket::GuildStorageItemChange {
                change_type,
                from,
                to,
            },
        )),
        BrowserCommand::CastSkill { key } => Ok(SessionAction::CastSkill { key }),
        BrowserCommand::TransferMap { key } => Ok(SessionAction::TransferMap { key }),
        BrowserCommand::Stage5Command { action, args } => {
            Ok(SessionAction::Stage5Command { action, args })
        }
        BrowserCommand::QaControl { token, action } => {
            Ok(SessionAction::QaControl { token, action })
        }
        BrowserCommand::SetLanguage { language } => Ok(SessionAction::SetLanguage { language }),
        BrowserCommand::Tick => Ok(SessionAction::Tick),
        BrowserCommand::LogOut => Ok(SessionAction::Packet(ClientPacket::LogOut)),
    }
}

/// Decode the `MirGridType` for `C.SetAutoPotItem`. Crystal only ever sends
/// `HeroHPItem`/`HeroMPItem` for this packet (HeroDialogs auto-pot slots), so we
/// accept those two grids by name or by raw enum byte. `parse_grid` in
/// `browser_commands.rs` does not cover the hero auto-pot grids, hence this
/// dedicated decoder (mirrors `MirGridType` HeroHPItem = 23 / HeroMPItem = 24).
fn parse_hero_autopot_grid(value: &str) -> Result<MirGridType, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "herohpitem" | "hero_hp_item" | "hp" | "23" => Ok(MirGridType::HeroHpItem),
        "herompitem" | "hero_mp_item" | "mp" | "24" => Ok(MirGridType::HeroMpItem),
        other => Err(format!("unsupported hero auto-pot grid: {other}")),
    }
}

fn should_send_world_snapshot_for_action(action: &SessionAction) -> bool {
    !matches!(
        action,
        SessionAction::MoveTo { .. }
            | SessionAction::Packet(ClientPacket::Turn { .. })
            | SessionAction::Packet(ClientPacket::Walk { .. })
            | SessionAction::Packet(ClientPacket::Run { .. })
            | SessionAction::Packet(ClientPacket::Chat { .. })
            | SessionAction::Packet(ClientPacket::KeepAlive { .. })
            | SessionAction::Tick
    )
}

fn is_low_latency_action(action: &SessionAction) -> bool {
    matches!(
        action,
        SessionAction::Packet(
            ClientPacket::Turn { .. }
                | ClientPacket::Walk { .. }
                | ClientPacket::Run { .. }
                | ClientPacket::Chat { .. }
                | ClientPacket::KeepAlive { .. }
        ) | SessionAction::Tick
    )
}

fn responses_require_world_snapshot(responses: &[ServerPacket]) -> bool {
    responses.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::StartGame { .. }
                | ServerPacket::MapInformation { .. }
                | ServerPacket::UserInformation { .. }
                | ServerPacket::UserLocation { .. }
                | ServerPacket::ObjectHealth { .. }
                | ServerPacket::ObjectHide { .. }
                | ServerPacket::ObjectShow { .. }
        )
    })
}

fn responses_require_resume_rotation(responses: &[ServerPacket]) -> bool {
    responses
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MapChanged { .. }))
}

async fn send_world_snapshot(
    sender: &SharedWebSocketSender,
    session: &GatewaySession,
) -> Result<(), String> {
    let snapshot = catch_gateway_panic("web world_snapshot", || {
        tokio::task::block_in_place(|| session.world_snapshot())
    })?;
    sender
        .lock()
        .await
        .send(Message::Text(
            json!({
                "type": "worldSnapshot",
                "payload": snapshot
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|error| error.to_string())
}

fn realm_info_event(config: &GatewayConfig) -> Value {
    let profile = config.content_profile.as_ref();
    let profile_id = profile
        .map(|profile| profile.profile_id.as_str())
        .unwrap_or("crystal_full");
    let realm_id = env::var("MIR2_REALM_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| profile_id.to_string());
    json!({
        "type": "realmInfo",
        "payload": {
            "schema": "mir2-realm-handshake/1",
            "realmId": realm_id,
            "profileId": profile_id,
            "profileVersion": profile.map(|profile| profile.version).unwrap_or(0),
            "acceptanceLevel": profile.map(|profile| profile.acceptance_level),
            "source": profile.map(|profile| profile.source.as_str()),
            "ratePolicy": profile.map(|profile| &profile.rate_policy),
            "bundleHash": profile.map(|profile| profile.bundle_hash.as_str()),
            "bundleBuiltAt": profile.map(|profile| profile.bundle_built_at.as_str()),
            "sourceData": profile.map(|profile| json!({
                "crystalDatabaseVersion": profile.crystal_database_version,
                "crystalDatabaseCustomVersion": profile.crystal_database_custom_version,
            })),
        }
    })
}

async fn send_server_packet(
    sender: &SharedWebSocketSender,
    packet: &ServerPacket,
) -> Result<(), axum::Error> {
    sender
        .lock()
        .await
        .send(Message::Text(
            server_packet_to_event(packet).to_string().into(),
        ))
        .await
}

async fn send_native_game_shop_receipt(
    sender: &SharedWebSocketSender,
    receipt: &Value,
) -> Result<(), axum::Error> {
    sender
        .lock()
        .await
        .send(Message::Text(receipt.to_string().into()))
        .await
}

fn validate_native_game_shop_request_id(request_id: &str) -> bool {
    !request_id.is_empty()
        && request_id.len() <= 64
        && request_id.is_ascii()
        && request_id.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
}

fn native_game_shop_request_from_action(
    action: &SessionAction,
) -> Option<Result<NativeGameShopRequest, String>> {
    let SessionAction::GameShopBuy {
        request_id,
        g_index,
        quantity,
        price_type,
    } = action
    else {
        return None;
    };
    Some(match request_id.as_deref() {
        Some(request_id) if validate_native_game_shop_request_id(request_id) => {
            let mut idempotency_bytes = [0_u8; 32];
            OsRng.fill_bytes(&mut idempotency_bytes);
            Ok(NativeGameShopRequest {
                request_id: request_id.to_string(),
                server_idempotency_key: URL_SAFE_NO_PAD.encode(idempotency_bytes),
                g_index: *g_index,
                quantity: *quantity,
                price_type: *price_type,
            })
        }
        _ => Err("native GameShop purchase requires a valid requestId".to_string()),
    })
}

fn native_game_shop_failure_event(
    request: &NativeGameShopRequest,
    code: &str,
    new_stock_level: Option<i32>,
) -> Value {
    let mut event = json!({
        "type": "gameShopReceipt",
        "protocol": NATIVE_GAME_SHOP_RECEIPT_PROTOCOL,
        "requestId": request.request_id,
        "success": false,
        "gIndex": request.g_index,
        "quantity": request.quantity,
        "priceType": request.price_type,
        "code": code,
    });
    if code == "stockUnavailable" {
        if let Some(new_stock_level) = new_stock_level {
            event["newStockLevel"] = json!(new_stock_level);
        }
    }
    event
}

fn native_game_shop_pre_execution_failure(
    request: &NativeGameShopRequest,
    authenticated: bool,
    in_game: bool,
    typed_outcome_supported: bool,
) -> Option<Value> {
    if !authenticated || !in_game {
        return Some(native_game_shop_failure_event(request, "notInGame", None));
    }
    if !typed_outcome_supported {
        return Some(native_game_shop_failure_event(
            request,
            "commitFailed",
            None,
        ));
    }
    None
}

fn native_game_shop_failure_code(failure: GameShopPurchaseFailure) -> &'static str {
    match failure {
        GameShopPurchaseFailure::NotInGame => "notInGame",
        GameShopPurchaseFailure::InvalidPriceType => "invalidRequest",
        GameShopPurchaseFailure::InvalidQuantity => "invalidQuantity",
        GameShopPurchaseFailure::UnknownProduct => "unknownProduct",
        GameShopPurchaseFailure::ClassUnavailable => "classUnavailable",
        GameShopPurchaseFailure::PaymentUnavailable => "paymentUnavailable",
        GameShopPurchaseFailure::StockUnavailable => "stockUnavailable",
        GameShopPurchaseFailure::InsufficientCurrency => "insufficientCurrency",
        GameShopPurchaseFailure::MailFull => "mailFull",
        GameShopPurchaseFailure::CommitFailed => "commitFailed",
    }
}

fn native_game_shop_receipt_event(
    request: &NativeGameShopRequest,
    outcome: &GameShopPurchaseOutcome,
) -> Result<Value, String> {
    if outcome.g_index != request.g_index
        || outcome.quantity != request.quantity
        || outcome.price_type != request.price_type
    {
        return Err("typed GameShop outcome does not match the pending request".to_string());
    }
    if outcome.success {
        let Some(mail_id) = outcome.mail_id else {
            return Err("successful typed GameShop outcome is missing mailId".to_string());
        };
        if outcome.failure.is_some() {
            return Err("successful typed GameShop outcome contains a failure code".to_string());
        }
        let mut event = json!({
            "type": "gameShopReceipt",
            "protocol": NATIVE_GAME_SHOP_RECEIPT_PROTOCOL,
            "requestId": request.request_id,
            "success": true,
            "gIndex": request.g_index,
            "quantity": request.quantity,
            "priceType": request.price_type,
            "mailId": mail_id,
        });
        if let Some(new_stock_level) = outcome.new_stock_level {
            event["newStockLevel"] = json!(new_stock_level);
        }
        return Ok(event);
    }
    let Some(failure) = outcome.failure else {
        return Err("failed typed GameShop outcome is missing failure".to_string());
    };
    if outcome.mail_id.is_some() {
        return Err("failed typed GameShop outcome contains mailId".to_string());
    }
    if outcome.new_stock_level.is_some() && failure != GameShopPurchaseFailure::StockUnavailable {
        return Err("only stockUnavailable may carry newStockLevel".to_string());
    }
    if failure == GameShopPurchaseFailure::CommitFailed {
        return Err(
            "post-execution GameShop commit state is unknown; commitFailed receipt is forbidden"
                .to_string(),
        );
    }
    Ok(native_game_shop_failure_event(
        request,
        native_game_shop_failure_code(failure),
        outcome.new_stock_level,
    ))
}

fn native_game_shop_post_execution(
    request: &NativeGameShopRequest,
    outcome: Option<&GameShopPurchaseOutcome>,
) -> NativeGameShopPostExecution {
    let Some(outcome) = outcome else {
        return NativeGameShopPostExecution::CloseUnknown {
            reason: "runtime returned no typed outcome after GameShop execution".to_string(),
        };
    };
    match native_game_shop_receipt_event(request, outcome) {
        Ok(receipt) => NativeGameShopPostExecution::SendReceipt(receipt),
        Err(reason) => NativeGameShopPostExecution::CloseUnknown { reason },
    }
}

fn execute_native_game_shop_handler_seam(
    session: &mut GatewaySession,
    request: &NativeGameShopRequest,
) -> Result<NativeGameShopHandlerDispatch, String> {
    let identity = session
        .active_identity()
        .ok_or_else(|| "native GameShop purchase has no active identity".to_string())?;
    let execution = session
        .execute_production_player_command_requiring_typed_game_shop_purchase_outcome(
            true,
            WorldCommand::NativeGameShopPurchase(NativeGameShopPurchaseRequest {
                protocol_version: NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
                server_idempotency_key: request.server_idempotency_key.clone(),
                gateway_session_id: session.session_id().to_string(),
                account_id: identity.account_id,
                character_index: identity.character_index,
                client_request_id: request.request_id.clone(),
                g_index: request.g_index,
                quantity: request.quantity,
                price_type: request.price_type,
            }),
        )?;
    Ok(NativeGameShopHandlerDispatch {
        post_execution: native_game_shop_post_execution(
            request,
            execution.game_shop_purchase_outcome.as_ref(),
        ),
        normal_packets: execution.packets,
    })
}

async fn send_error_message(
    sender: &SharedWebSocketSender,
    message: &str,
) -> Result<(), axum::Error> {
    sender
        .lock()
        .await
        .send(Message::Text(
            json!({
                "type": "error",
                "message": message
            })
            .to_string()
            .into(),
        ))
        .await
}

async fn send_resume_rejected(sender: &SharedWebSocketSender) -> Result<(), axum::Error> {
    sender
        .lock()
        .await
        .send(Message::Text(resume_rejected_event().to_string().into()))
        .await
}

fn resume_rejected_event() -> Value {
    json!({
        "type": "resumeRejected",
        "code": "unavailable",
    })
}

async fn send_resume_credential(
    sender: &SharedWebSocketSender,
    issued: &IssuedResumeCredential,
) -> Result<(), axum::Error> {
    sender
        .lock()
        .await
        .send(Message::Text(
            resume_credential_event(issued).to_string().into(),
        ))
        .await
}

fn resume_credential_event(issued: &IssuedResumeCredential) -> Value {
    json!({
        "type": "resumeCredential",
        "protocol": NATIVE_RESUME_PROTOCOL,
        "credential": issued.credential.as_str(),
        "expiresAtMs": issued.binding.expires_at_ms,
        "generation": issued.binding.generation,
    })
}

async fn send_session_resumed(
    sender: &SharedWebSocketSender,
    character_index: i32,
    generation: u64,
) -> Result<(), axum::Error> {
    sender
        .lock()
        .await
        .send(Message::Text(
            session_resumed_event(character_index, generation)
                .to_string()
                .into(),
        ))
        .await
}

fn session_resumed_event(character_index: i32, generation: u64) -> Value {
    json!({
        "type": "sessionResumed",
        "protocol": NATIVE_RESUME_PROTOCOL,
        "characterIndex": character_index,
        "generation": generation,
    })
}

fn validate_native_client_capabilities(
    capabilities: &[String],
) -> Result<NativeClientCapabilities, String> {
    const MAX_CAPABILITIES: usize = 16;
    const MAX_CAPABILITY_CHARS: usize = 64;
    if capabilities.len() > MAX_CAPABILITIES
        || capabilities.iter().any(|capability| {
            capability.is_empty()
                || capability.len() > MAX_CAPABILITY_CHARS
                || !capability.is_ascii()
                || capability
                    .bytes()
                    .any(|byte| !(0x20..=0x7e).contains(&byte))
        })
    {
        return Err("invalid client capabilities".to_string());
    }
    Ok(NativeClientCapabilities {
        native_resume_v1: capabilities
            .iter()
            .any(|capability| capability == NATIVE_RESUME_PROTOCOL),
        native_game_shop_receipt_v1: capabilities
            .iter()
            .any(|capability| capability == NATIVE_GAME_SHOP_RECEIPT_PROTOCOL),
    })
}

#[derive(Debug, PartialEq, Eq)]
enum NativeResumePrepareError {
    Unavailable,
    Route(String),
    Zone(String),
}

#[derive(Debug, PartialEq, Eq)]
enum NativeResumeCommitError {
    IdentityUnavailable,
    Unavailable,
}

fn native_resume_identity_is_active(
    session_cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    verified: &VerifiedIdentitySession,
    now_ms: u64,
) -> bool {
    verified.expires_at_ms > now_ms
        && matches!(
            session_cache.identity_session_is_revoked(&verified.session_id),
            Ok(false)
        )
        && identity
            .list_sessions(verified)
            .ok()
            .is_some_and(|sessions| {
                sessions.iter().any(|session| {
                    session.session_id == verified.session_id
                        && session.account_id == verified.account_id
                        && session.expires_at_ms == verified.expires_at_ms
                        && session.revoked_at_ms.is_none()
                        && session.expires_at_ms > now_ms
                })
            })
}

fn validate_and_prepare_native_resume<'a, PreparedZone>(
    reconnect_sessions: &'a ReconnectSessionStore,
    session_cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    credential: &ResumeCredential,
    now_ms: u64,
    prepare_route: impl FnOnce(&GatewaySession) -> Result<(), String>,
    prepare_zone: impl FnOnce(&GatewaySession) -> Result<PreparedZone, String>,
) -> Result<
    (
        ReconnectSessionReservation<'a>,
        VerifiedIdentitySession,
        PreparedZone,
    ),
    NativeResumePrepareError,
> {
    let binding = reconnect_sessions
        .resume_binding(credential, now_ms)
        .ok_or(NativeResumePrepareError::Unavailable)?;
    let verified = VerifiedIdentitySession {
        account_id: binding.account_id.clone(),
        session_id: binding.identity_session_id.clone(),
        expires_at_ms: binding.identity_expires_at_ms,
    };
    if !native_resume_identity_is_active(session_cache, identity, &verified, now_ms) {
        return Err(NativeResumePrepareError::Unavailable);
    }
    let reservation = reconnect_sessions
        .reserve_by_credential(credential, &binding, now_ms)
        .ok_or(NativeResumePrepareError::Unavailable)?;
    prepare_route(reservation.session()).map_err(NativeResumePrepareError::Route)?;
    let prepared_zone =
        prepare_zone(reservation.session()).map_err(NativeResumePrepareError::Zone)?;
    Ok((reservation, verified, prepared_zone))
}

fn revalidate_and_commit_prepared_native_resume<PreparedZone>(
    reconnect_sessions: &ReconnectSessionStore,
    session_cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    reservation: ReconnectSessionReservation<'_>,
    credential: &ResumeCredential,
    verified: VerifiedIdentitySession,
    prepared_zone: PreparedZone,
    now_ms: u64,
) -> Result<
    (
        ReconnectSessionRestore,
        ResumeBinding,
        VerifiedIdentitySession,
        PreparedZone,
    ),
    NativeResumeCommitError,
> {
    revalidate_and_commit_prepared_native_resume_with_fence_hook(
        reconnect_sessions,
        session_cache,
        identity,
        reservation,
        credential,
        verified,
        prepared_zone,
        now_ms,
        || {},
    )
}

fn revalidate_and_commit_prepared_native_resume_with_fence_hook<PreparedZone>(
    reconnect_sessions: &ReconnectSessionStore,
    session_cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    reservation: ReconnectSessionReservation<'_>,
    credential: &ResumeCredential,
    verified: VerifiedIdentitySession,
    prepared_zone: PreparedZone,
    now_ms: u64,
    before_commit: impl FnOnce(),
) -> Result<
    (
        ReconnectSessionRestore,
        ResumeBinding,
        VerifiedIdentitySession,
        PreparedZone,
    ),
    NativeResumeCommitError,
> {
    revalidate_and_commit_prepared_native_resume_with_hooks(
        reconnect_sessions,
        session_cache,
        identity,
        reservation,
        credential,
        verified,
        prepared_zone,
        now_ms,
        before_commit,
        || {},
    )
}

fn revalidate_and_commit_prepared_native_resume_with_hooks<PreparedZone>(
    reconnect_sessions: &ReconnectSessionStore,
    session_cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    reservation: ReconnectSessionReservation<'_>,
    credential: &ResumeCredential,
    verified: VerifiedIdentitySession,
    prepared_zone: PreparedZone,
    now_ms: u64,
    before_commit: impl FnOnce(),
    after_local_commit: impl FnOnce(),
) -> Result<
    (
        ReconnectSessionRestore,
        ResumeBinding,
        VerifiedIdentitySession,
        PreparedZone,
    ),
    NativeResumeCommitError,
> {
    if !native_resume_identity_is_active(session_cache, identity, &verified, now_ms) {
        // Identity revocation is terminal for this credential family. The
        // prepared Zone value drops normally, while the reservation explicitly
        // discards its unusable lease and releases both capacity permits.
        reservation.discard_and_revoke();
        return Err(NativeResumeCommitError::IdentityUnavailable);
    }
    // Test hook models the exact TOCTOU window which previously existed after
    // identity revalidation and before credential consumption. Production uses
    // a no-op; the auth revision check and consume still occur under the same
    // reconnect-store mutex.
    before_commit();
    let (restore, binding) = match reconnect_sessions.commit_resume(reservation, credential, now_ms)
    {
        Ok(committed) => committed,
        Err(ReconnectSessionCommitError::AuthorizationRevisionChanged) => {
            return Err(NativeResumeCommitError::IdentityUnavailable);
        }
        Err(
            ReconnectSessionCommitError::ForeignReservation
            | ReconnectSessionCommitError::LeaseExpired
            | ReconnectSessionCommitError::CredentialUnavailable,
        ) => return Err(NativeResumeCommitError::Unavailable),
    };
    after_local_commit();
    // The local revision fence cannot observe a revocation committed through a
    // different Gateway. Re-read the shared Redis/PostgreSQL-backed authority
    // after consuming the local credential but before returning any restorable
    // state to the socket loop. On failure, dropping these values releases the
    // restored session and both permits; the consumed family is never rolled
    // back and the prepared Zone registration is never activated.
    if !native_resume_identity_is_active(
        session_cache,
        identity,
        &verified,
        gateway_unix_ms().max(now_ms),
    ) {
        drop(prepared_zone);
        drop(restore);
        return Err(NativeResumeCommitError::IdentityUnavailable);
    }
    Ok((restore, binding, verified, prepared_zone))
}

fn enforce_first_post_resume_action_identity(
    pending: &mut bool,
    reconnect_sessions: &ReconnectSessionStore,
    native_resume: &mut NativeResumeConnectionState,
    session_cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    session: &GatewaySession,
    authenticated_account_id: Option<&str>,
    verified: Option<&VerifiedIdentitySession>,
    now_ms: u64,
) -> bool {
    if !*pending {
        return true;
    }
    let active = session.active_identity();
    let valid = active
        .zip(authenticated_account_id)
        .zip(verified)
        .is_some_and(|((active, account_id), verified)| {
            active.account_id == account_id
                && verified.account_id == account_id
                && native_resume_identity_is_active(session_cache, identity, verified, now_ms)
        });
    if valid {
        *pending = false;
        return true;
    }
    native_resume.disable_and_revoke(reconnect_sessions);
    false
}

#[cfg(test)]
fn validate_and_commit_native_resume_for_test(
    reconnect_sessions: &ReconnectSessionStore,
    session_cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    credential: &ResumeCredential,
    now_ms: u64,
) -> Option<(
    ReconnectSessionRestore,
    ResumeBinding,
    VerifiedIdentitySession,
)> {
    let (reservation, verified, ()) = validate_and_prepare_native_resume(
        reconnect_sessions,
        session_cache,
        identity,
        credential,
        now_ms,
        |_| Ok(()),
        |_| Ok(()),
    )
    .ok()?;
    let (restore, binding, verified, ()) = revalidate_and_commit_prepared_native_resume(
        reconnect_sessions,
        session_cache,
        identity,
        reservation,
        credential,
        verified,
        (),
        now_ms,
    )
    .ok()?;
    Some((restore, binding, verified))
}

async fn maybe_issue_native_resume_credential(
    sender: &SharedWebSocketSender,
    reconnect_sessions: &ReconnectSessionStore,
    session_cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    native_resume: &mut NativeResumeConnectionState,
    session: &GatewaySession,
    authenticated_account_id: Option<&str>,
    active_identity_session: Option<&VerifiedIdentitySession>,
    force: bool,
) -> Result<bool, axum::Error> {
    let now_ms = gateway_unix_ms();
    if !native_resume.should_rotate(now_ms, force) {
        return Ok(false);
    }
    let Some(authenticated_account_id) = authenticated_account_id else {
        return Ok(false);
    };
    let Some(active_identity) = session.active_identity() else {
        return Ok(false);
    };
    let Some(verified) = active_identity_session else {
        return Ok(false);
    };
    if active_identity.account_id != authenticated_account_id
        || verified.account_id != authenticated_account_id
        || verified.expires_at_ms <= now_ms
    {
        return Ok(false);
    }
    let gateway_session_id = session.session_id().to_string();
    let Some(issued) = reconnect_sessions.issue_resume_credential(
        native_resume.family_id.as_ref(),
        ResumeIssueContext {
            account_id: authenticated_account_id,
            character_index: active_identity.character_index,
            gateway_session_id: &gateway_session_id,
            identity_session_id: &verified.session_id,
            identity_expires_at_ms: verified.expires_at_ms,
            source_connection_nonce: &native_resume.connection_nonce,
        },
        now_ms,
        native_resume.minimum_generation,
        || native_resume_identity_is_active(session_cache, identity, verified, now_ms),
    ) else {
        return Ok(false);
    };
    native_resume.family_id = Some(issued.binding.family_id.clone());
    native_resume.minimum_generation = issued.binding.generation.saturating_add(1);
    native_resume.last_issued_at_ms = Some(now_ms);
    send_resume_credential(sender, &issued).await?;
    Ok(true)
}

async fn send_identity_session_grant(
    sender: &SharedWebSocketSender,
    grant: &crate::identity::IdentitySessionGrant,
) -> Result<(), axum::Error> {
    sender
        .lock()
        .await
        .send(Message::Text(
            json!({
                "type": "identitySession",
                "token": grant.token,
                "session": grant.session,
            })
            .to_string()
            .into(),
        ))
        .await
}

/// Renders a buff stat as a superset object: keeps Crystal's raw `stat`+`value`
/// (backward compatible) and adds `label` (Crystal `Stat` enum name) for the
/// browser buff window's `{label, value}` contract.
fn buff_stat_json(stat: &UserItemStat) -> Value {
    json!({
        "stat": stat.stat,
        "label": crystal_stat_label(stat.stat),
        "value": stat.value,
    })
}

/// Renders a non-blocked friend for the browser `friends` list. `level`/`mapName`
/// are intentionally absent (not carried by Crystal's `ClientFriend` nor known to
/// the simulation); an empty memo is omitted.
fn friend_entry_json(friend: &ClientFriend) -> Value {
    let mut entry = serde_json::Map::new();
    entry.insert("name".into(), json!(friend.name));
    entry.insert("online".into(), json!(friend.online));
    if !friend.memo.is_empty() {
        entry.insert("memo".into(), json!(friend.memo));
    }
    Value::Object(entry)
}

/// Renders a blocked entry for the browser `blocked` list (`{name, memo?}`).
fn blocked_entry_json(friend: &ClientFriend) -> Value {
    let mut entry = serde_json::Map::new();
    entry.insert("name".into(), json!(friend.name));
    if !friend.memo.is_empty() {
        entry.insert("memo".into(), json!(friend.memo));
    }
    Value::Object(entry)
}

/// Crystal `RequiredType.Level` (`Crystal/Shared/Enums.cs:1087-1089`). When an
/// item's `RequiredType` is `Level` the `RequiredAmount` is the level gate the
/// client renders (`Crystal/Client/MirScenes/Dialogs/NPCDialogs.cs:895-897`).
const CRYSTAL_REQUIRED_TYPE_LEVEL: u8 = 0;

/// Resolves a wire `UserItem` (which only carries `item_index`) into the
/// browser-facing `{ name, count?, grade? }` shape used by the trade / market /
/// quest contracts. The name + grade come from the Crystal item template
/// (`Crystal` mirrors `UserItem.Info.Name` / `UserItem.Info.Grade`), looked up
/// by `item_index`. When the index is unknown the name falls back to
/// `Item #<index>` so the window still renders a row.
fn user_item_summary_json(item: &UserItem) -> Value {
    let template = crystal_item_by_index(item.item_index);
    let name = template
        .as_ref()
        .map(|template| template.name.clone())
        .unwrap_or_else(|| format!("Item #{}", item.item_index));
    let mut entry = serde_json::Map::new();
    entry.insert("name".into(), json!(name));
    entry.insert("count".into(), json!(item.count));
    if let Some(template) = template.as_ref() {
        // Crystal `ItemGrade` (0 = Common). Only emit a meaningful grade.
        if template.grade != 0 {
            entry.insert("grade".into(), json!(template.grade));
        }
    }
    Value::Object(entry)
}

/// Resolves an NPC goods wire item into the browser shop contract while
/// retaining every raw `UserItem` field for protocol diagnostics. Crystal's
/// wire payload only carries `item_index`; the native client joins that against
/// its item database before drawing the shop. The web client needs the same
/// name/icon/unit-price join at the gateway boundary.
fn npc_goods_item_json(item: &UserItem, rate: f32) -> Value {
    let mut entry = serde_json::to_value(item)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    entry.insert("id".into(), json!(item.unique_id));
    entry.insert("uniqueId".into(), json!(item.unique_id));
    entry.insert("itemIndex".into(), json!(item.item_index));
    entry.insert("count".into(), json!(item.count));

    if let Some(template) = crystal_item_by_index(item.item_index) {
        entry.insert("name".into(), json!(template.name));
        entry.insert("icon".into(), json!(template.image));
        entry.insert(
            "price".into(),
            json!(((template.price as f32) * rate).floor() as u32),
        );
        entry.insert("grade".into(), json!(template.grade));
        if let Some(description) = template.tooltip.filter(|value| !value.trim().is_empty()) {
            entry.insert("description".into(), json!(description));
        }
    } else {
        entry.insert("name".into(), json!(format!("Item #{}", item.item_index)));
        entry.insert("icon".into(), json!(0));
        entry.insert("price".into(), json!(0));
    }
    Value::Object(entry)
}

fn npc_goods_list_json(list: &[UserItem], rate: f32) -> Value {
    Value::Array(
        list.iter()
            .map(|item| npc_goods_item_json(item, rate))
            .collect(),
    )
}

/// Maps a `Vec<Option<UserItem>>` trade payload into the contract item array,
/// skipping the empty trade slots (Crystal `TradeItem` sends a fixed-length
/// array with `null` holes — see `Crystal/Shared/ServerPackets.cs:1944-1972`).
fn trade_items_summary_json(items: &[Option<UserItem>]) -> Value {
    Value::Array(
        items
            .iter()
            .filter_map(|slot| slot.as_ref())
            .map(user_item_summary_json)
            .collect(),
    )
}

/// Resolves the optional `level` field for an auction/market listing: Crystal
/// only treats `RequiredAmount` as a level when `RequiredType == Level`
/// (`Crystal/Client/MirScenes/Dialogs/NPCDialogs.cs:895`).
fn crystal_item_level_requirement(item_index: i32) -> Option<u32> {
    let template = crystal_item_by_index(item_index)?;
    if template.required_type == CRYSTAL_REQUIRED_TYPE_LEVEL && template.required_amount > 0 {
        Some(u32::from(template.required_amount))
    } else {
        None
    }
}

/// Crystal `MarketItemType.Auction` (`Crystal/Shared/Enums.cs`). A `ClientAuction`
/// whose `ItemType` is `Auction` is shown as a live auction; `Consign`/`GameShop`
/// are fixed-price. Used to fill the contract `auction` flag.
const CRYSTAL_MARKET_ITEM_TYPE_AUCTION: u8 = 2;

/// Serializes a protocol `ClientAuction` into the browser-facing market listing
/// contract: `{ id, itemName, seller?, price, type?, level?, expiry?, auction?,
/// sold? }`. Mirrors `Crystal/Server/MirDatabase/AuctionInfo.cs:104-115`
/// (`CreateClientAuction`) — the wire struct collapses `CurrentBid`/`Expired`/
/// `Sold` into the `Seller` label + `Price`, so for own-listings the `Seller`
/// already encodes the status string ("Sold" / "Expired" / "For Sale" / ...).
/// `expiry` is the consignment `DateTime.ToBinary()` value Crystal ships
/// (`ClientAuction.ConsignmentDate`, `Crystal/Shared/Data/ClientData.cs:213`).
fn client_auction_listing_json(auction: &ClientAuction) -> Value {
    let item_index = auction.item.item_index;
    let item_name = crystal_item_by_index(item_index)
        .map(|template| template.name)
        .unwrap_or_else(|| format!("Item #{item_index}"));
    let mut entry = serde_json::Map::new();
    entry.insert("id".into(), json!(auction.auction_id));
    entry.insert("itemName".into(), json!(item_name));
    if !auction.seller.is_empty() {
        entry.insert("seller".into(), json!(auction.seller));
    }
    entry.insert("price".into(), json!(auction.price));
    entry.insert("type".into(), json!(auction.item_type));
    if let Some(level) = crystal_item_level_requirement(item_index) {
        entry.insert("level".into(), json!(level));
    }
    if auction.consignment_date_binary_datetime != 0 {
        entry.insert(
            "expiry".into(),
            json!(auction.consignment_date_binary_datetime),
        );
    }
    entry.insert(
        "auction".into(),
        json!(auction.item_type == CRYSTAL_MARKET_ITEM_TYPE_AUCTION),
    );
    Value::Object(entry)
}

/// Maps a list of protocol `ClientAuction`s into the contract listing array.
fn client_auction_listings_json(listings: &[ClientAuction]) -> Value {
    Value::Array(listings.iter().map(client_auction_listing_json).collect())
}

/// Parses a Crystal quest task line into a contract `objective`
/// (`{ text, current?, required? }`). The simulation formats task lines as e.g.
/// `"Kill ChickyBoo 2/5"` / `"Collect Wool 0/3"`
/// (`apps/simulation/src/runtime/quests.rs:318-356`), so the trailing
/// `current/required` fraction is parsed out when present; otherwise just the
/// text is kept.
fn quest_objective_json(line: &str) -> Value {
    let mut entry = serde_json::Map::new();
    entry.insert("text".into(), json!(line));
    if let Some((current, required)) = parse_progress_fraction(line) {
        entry.insert("current".into(), json!(current));
        entry.insert("required".into(), json!(required));
        entry.insert("done".into(), json!(current >= required));
    }
    Value::Object(entry)
}

/// Aggregates a fully structured Crystal task list for browser clients. Some
/// quests use prose-only task lines, so totals are only emitted when every
/// non-empty line carries an explicit progress fraction.
fn quest_progress_totals(task_list: &[String]) -> Option<(u32, u32)> {
    let mut current = 0_u32;
    let mut required = 0_u32;
    let mut saw_progress = false;
    for line in task_list.iter().filter(|line| !line.trim().is_empty()) {
        let (task_current, task_required) = parse_progress_fraction(line)?;
        current = current.saturating_add(task_current.min(task_required));
        required = required.saturating_add(task_required);
        saw_progress = true;
    }
    saw_progress.then_some((current, required))
}

/// Extracts the last `current/required` integer fraction from a task line, if
/// one is present (e.g. `"Kill ChickyBoo 2/5"` -> `(2, 5)`).
fn parse_progress_fraction(line: &str) -> Option<(u32, u32)> {
    let token = line.split_whitespace().rev().find(|token| {
        token
            .split_once('/')
            .is_some_and(|(left, right)| !left.is_empty() && !right.is_empty())
    })?;
    let (left, right) = token.split_once('/')?;
    let current: u32 = left.parse().ok()?;
    let required: u32 = right.parse().ok()?;
    Some((current, required))
}

/// Serializes the reward bundle of a `ClientQuestInfo` into the contract
/// `rewards` object (`{ gold?, experience?, credit?, items?, selectItems? }`). Mirrors the
/// Crystal `ClientQuestInfo` reward fields
/// (`Crystal/Shared/Data/ClientData.cs:380-384`). Fixed and selectable rewards
/// stay separate because the Crystal client requires an explicit choice before
/// turn-in. Each item also carries its original icon and item index.
/// Returns `None` when there is nothing to reward so the field can be omitted.
fn quest_rewards_json(info: &ClientQuestInfo) -> Option<Value> {
    let mut entry = serde_json::Map::new();
    if info.reward_gold != 0 {
        entry.insert("gold".into(), json!(info.reward_gold));
    }
    if info.reward_exp != 0 {
        entry.insert("experience".into(), json!(info.reward_exp));
    }
    if info.reward_credit != 0 {
        entry.insert("credit".into(), json!(info.reward_credit));
    }
    let items = quest_reward_items_json(info.rewards_fixed_item.iter(), false);
    if let Value::Array(ref array) = items {
        if !array.is_empty() {
            entry.insert("items".into(), items);
        }
    }
    let select_items = quest_reward_items_json(info.rewards_select_item.iter(), true);
    if let Value::Array(ref array) = select_items {
        if !array.is_empty() {
            entry.insert("selectItems".into(), select_items);
        }
    }
    if entry.is_empty() {
        None
    } else {
        Some(Value::Object(entry))
    }
}

/// Maps quest reward items into the browser contract using fields already
/// carried by `ItemInfo` on the Crystal wire payload.
fn quest_reward_items_json<'a>(
    rewards: impl Iterator<Item = &'a QuestItemReward>,
    selectable: bool,
) -> Value {
    Value::Array(
        rewards
            .enumerate()
            .map(|(selection_index, reward)| {
                let mut item = serde_json::Map::new();
                item.insert("name".into(), json!(reward.item.name));
                item.insert("icon".into(), json!(reward.item.image));
                item.insert("itemIndex".into(), json!(reward.item.index));
                if reward.count != 1 {
                    item.insert("count".into(), json!(reward.count));
                }
                if selectable {
                    item.insert("selectable".into(), json!(true));
                    item.insert("selectionIndex".into(), json!(selection_index));
                }
                Value::Object(item)
            })
            .collect(),
    )
}

/// Formats a quest time limit (Crystal `ClientQuestInfo.TimeLimitInSeconds`,
/// `Crystal/Shared/Data/ClientData.cs:378`) into a human `mm:ss` / `Hh Mm`
/// string for the contract `timeLimit` field. Returns `None` when the quest has
/// no time limit (0 seconds), so the field is omitted.
fn quest_time_limit_label(time_limit_in_seconds: i32) -> Option<String> {
    if time_limit_in_seconds <= 0 {
        return None;
    }
    let total = time_limit_in_seconds as u32;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        Some(format!("{hours}h {minutes:02}m"))
    } else {
        Some(format!("{minutes:02}:{seconds:02}"))
    }
}

fn server_packet_to_event(packet: &ServerPacket) -> Value {
    match packet {
        ServerPacket::Raw { packet_id, payload } => {
            let packet_name = server_packet_raw_display_name(*packet_id);
            json!({
                "type": "packet",
                "packet": packet_name,
                "payload": raw_payload_detail(
                    &packet_name,
                    *packet_id as i16,
                    payload
                )
            })
        }
        ServerPacket::TimeOfDay { lights } => json!({
            "type": "packet",
            "packet": "TimeOfDay",
            "payload": { "lights": lights }
        }),
        ServerPacket::ChangeAMode { mode } => json!({
            "type": "packet",
            "packet": "ChangeAMode",
            "payload": { "mode": mode }
        }),
        ServerPacket::ChangePMode { mode } => json!({
            "type": "packet",
            "packet": "ChangePMode",
            "payload": { "mode": mode }
        }),
        ServerPacket::BaseStatsInfo { stats } => json!({
            "type": "packet",
            "packet": "BaseStatsInfo",
            "payload": { "stats": stats }
        }),
        ServerPacket::HeroBaseStatsInfo { stats } => json!({
            "type": "packet",
            "packet": "HeroBaseStatsInfo",
            "payload": { "stats": stats }
        }),
        ServerPacket::HeroInformation { info } => json!({
            "type": "packet",
            "packet": "HeroInformation",
            "payload": { "info": info }
        }),
        // Crystal `GetMarket` enqueues `S.NPCMarket` with the matched
        // `ClientAuction`s, the page count, and `UserMode`
        // (`Crystal/Server/MirObjects/PlayerObject.cs:8419`). `auctions` resolves
        // each `ClientAuction` to the browser market-listing contract
        // (`{ id, itemName, seller?, price, type?, level?, expiry?, auction?,
        // sold? }`); the raw `listings` array is kept for backward compatibility.
        // `highestBid` / `sold` are omitted because the wire `ClientAuction`
        // collapses `CurrentBid`/`Sold` into `Price` + the `Seller` status label
        // (`Crystal/Server/MirDatabase/AuctionInfo.cs:104-115`), so they are not
        // separately recoverable here.
        ServerPacket::NPCMarket {
            listings,
            pages,
            user_mode,
        } => json!({
            "type": "packet",
            "packet": "NPCMarket",
            "payload": {
                "listings": listings,
                "auctions": client_auction_listings_json(listings),
                "pages": pages,
                "userMode": user_mode
            }
        }),
        ServerPacket::NPCMarketPage { listings } => json!({
            "type": "packet",
            "packet": "NPCMarketPage",
            "payload": {
                "listings": listings,
                "auctions": client_auction_listings_json(listings)
            }
        }),
        ServerPacket::Connected => json!({
            "type": "packet",
            "packet": "Connected",
            "payload": {}
        }),
        ServerPacket::ClientVersion { result } => json!({
            "type": "packet",
            "packet": "ClientVersion",
            "payload": { "result": result }
        }),
        ServerPacket::Disconnect { reason } => json!({
            "type": "packet",
            "packet": "Disconnect",
            "payload": { "reason": reason }
        }),
        ServerPacket::KeepAlive { time } => json!({
            "type": "packet",
            "packet": "KeepAlive",
            "payload": { "time": time }
        }),
        ServerPacket::NewAccount { result } => json!({
            "type": "packet",
            "packet": "NewAccount",
            "payload": { "result": result }
        }),
        ServerPacket::ChangePassword { result } => json!({
            "type": "packet",
            "packet": "ChangePassword",
            "payload": { "result": result }
        }),
        ServerPacket::ChangePasswordBanned {
            reason,
            expiry_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "ChangePasswordBanned",
            "payload": {
                "reason": reason,
                "expiryBinaryDatetime": expiry_binary_datetime
            }
        }),
        ServerPacket::Login { result } => json!({
            "type": "packet",
            "packet": "Login",
            "payload": { "result": result }
        }),
        ServerPacket::LoginBanned {
            reason,
            expiry_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "LoginBanned",
            "payload": {
                "reason": reason,
                "expiryBinaryDatetime": expiry_binary_datetime
            }
        }),
        ServerPacket::LoginSuccess { characters } => json!({
            "type": "packet",
            "packet": "LoginSuccess",
            "payload": {
                "characters": characters.iter().map(|character| {
                    json!({
                        "index": character.index,
                        "name": character.name,
                        "level": character.level,
                        "class": format!("{:?}", character.class),
                        "gender": format!("{:?}", character.gender)
                    })
                }).collect::<Vec<_>>()
            }
        }),
        ServerPacket::NewCharacter { result } => json!({
            "type": "packet",
            "packet": "NewCharacter",
            "payload": { "result": result }
        }),
        ServerPacket::NewHero { result } => json!({
            "type": "packet",
            "packet": "NewHero",
            "payload": { "result": result }
        }),
        ServerPacket::NewCharacterSuccess { char_info } => json!({
            "type": "packet",
            "packet": "NewCharacterSuccess",
            "payload": {
                "character": {
                    "index": char_info.index,
                    "name": char_info.name,
                    "level": char_info.level,
                    "class": format!("{:?}", char_info.class),
                    "gender": format!("{:?}", char_info.gender)
                }
            }
        }),
        ServerPacket::DeleteCharacter { result } => json!({
            "type": "packet",
            "packet": "DeleteCharacter",
            "payload": { "result": result }
        }),
        ServerPacket::DeleteCharacterSuccess { character_index } => json!({
            "type": "packet",
            "packet": "DeleteCharacterSuccess",
            "payload": { "characterIndex": character_index }
        }),
        ServerPacket::StartGame { result, resolution } => json!({
            "type": "packet",
            "packet": "StartGame",
            "payload": {
                "result": result,
                "resolution": resolution
            }
        }),
        ServerPacket::StartGameBanned {
            reason,
            expiry_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "StartGameBanned",
            "payload": {
                "reason": reason,
                "expiryBinaryDatetime": expiry_binary_datetime
            }
        }),
        ServerPacket::StartGameDelay { milliseconds } => json!({
            "type": "packet",
            "packet": "StartGameDelay",
            "payload": { "milliseconds": milliseconds }
        }),
        ServerPacket::MapInformation { info } => json!({
            "type": "packet",
            "packet": "MapInformation",
            "payload": {
                "mapIndex": info.map_index,
                "fileName": info.file_name,
                "title": info.title,
                "miniMapIndex": info.mini_map,
                "bigMapIndex": info.big_map,
                "lights": info.lights,
                "mapDarkLight": info.map_dark_light,
                "weatherParticles": info.weather_particles,
                // Crystal SoundList music id for this map (0 = silent); the Web client loops it as
                // background music, matching MapControl.LoadMap -> SoundManager.PlayMusic(Music).
                "music": info.music,
                "spawnFlags": {
                    "lightning": info.has_lightning(),
                    "fire": info.has_fire()
                }
            }
        }),
        ServerPacket::MapChanged {
            map_index,
            file_name,
            title,
            mini_map,
            big_map,
            lights,
            location,
            direction,
            map_dark_light,
            music,
            weather,
        } => json!({
            "type": "packet",
            "packet": "MapChanged",
            "payload": {
                "mapIndex": map_index,
                "fileName": file_name,
                "title": title,
                "miniMap": mini_map,
                "bigMap": big_map,
                "lights": lights,
                "location": location,
                "direction": format!("{direction:?}"),
                "mapDarkLight": map_dark_light,
                "music": music,
                "weatherParticles": weather
            }
        }),
        ServerPacket::UserInformation { info } => json!({
            "type": "packet",
            "packet": "UserInformation",
            "payload": {
                "objectId": info.object_id,
                "realId": info.real_id,
                "name": info.name,
                "guildName": info.guild_name,
                "guildRank": info.guild_rank,
                "nameColourArgb": info.name_colour_argb,
                "class": format!("{:?}", info.class),
                "gender": format!("{:?}", info.gender),
                "level": info.level,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "hair": info.hair,
                "hp": info.hp,
                "mp": info.mp,
                "maxHp": info.max_hp,
                "maxMp": info.max_mp,
                "experience": info.experience,
                "maxExperience": info.max_experience,
                "levelEffects": info.level_effects,
                "hasHero": info.has_hero,
                "heroBehaviour": info.hero_behaviour,
                "inventorySectionPresent": info.inventory_section_present,
                "equipmentSectionPresent": info.equipment_section_present,
                "questInventorySectionPresent": info.quest_inventory_section_present,
                "gold": info.gold,
                "credit": info.credit,
                "hasExpandedStorage": info.has_expanded_storage,
                "hasStoragePassword": info.has_storage_password,
                "requireStoragePassword": info.require_storage_password,
                "storagePasswordLastSetBinaryDatetime": info.storage_password_last_set_binary_datetime,
                "expandedStorageExpiryTimeBinaryDatetime": info.expanded_storage_expiry_time_binary_datetime,
                "magicCount": info.magic_count,
                "intelligentCreatureCount": info.intelligent_creature_count,
                "summonedCreatureType": info.summoned_creature_type,
                "creatureSummoned": info.creature_summoned,
                "allowObserve": info.allow_observe,
                "observer": info.observer
            }
        }),
        ServerPacket::UserLocation { location } => json!({
            "type": "packet",
            "packet": "UserLocation",
            "payload": {
                "x": location.position.x,
                "y": location.position.y,
                "direction": format!("{:?}", location.direction)
            }
        }),
        ServerPacket::ObjectPlayer { info } => json!({
            "type": "packet",
            "packet": "ObjectPlayer",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "guildName": info.guild_name,
                "guildRankName": info.guild_rank_name,
                "nameColourArgb": info.name_colour_argb,
                "class": format!("{:?}", info.class),
                "gender": format!("{:?}", info.gender),
                "level": info.level,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "hair": info.hair,
                "light": info.light,
                "weapon": info.weapon,
                "weaponEffect": info.weapon_effect,
                "armour": info.armour,
                "poison": info.poison,
                "dead": info.dead,
                "hidden": info.hidden,
                "effect": info.effect,
                "wingEffect": info.wing_effect,
                "extra": info.extra,
                "mountType": info.mount_type,
                "ridingMount": info.riding_mount,
                "fishing": info.fishing,
                "transformType": info.transform_type,
                "elementOrbEffect": info.element_orb_effect,
                "elementOrbLevel": info.element_orb_level,
                "elementOrbMax": info.element_orb_max,
                "buffs": info.buffs,
                "levelEffects": info.level_effects
            }
        }),
        ServerPacket::ObjectHero { info, owner_name } => json!({
            "type": "packet",
            "packet": "ObjectHero",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "ownerName": owner_name,
                "class": format!("{:?}", info.class),
                "gender": format!("{:?}", info.gender),
                "level": info.level,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "buffs": info.buffs,
                "levelEffects": info.level_effects
            }
        }),
        ServerPacket::ObjectRemove { object_id } => json!({
            "type": "packet",
            "packet": "ObjectRemove",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectTeleportOut {
            object_id,
            effect_type,
        } => json!({
            "type": "packet",
            "packet": "ObjectTeleportOut",
            "payload": {
                "objectId": object_id,
                "effectType": effect_type
            }
        }),
        ServerPacket::ObjectTeleportIn {
            object_id,
            effect_type,
        } => json!({
            "type": "packet",
            "packet": "ObjectTeleportIn",
            "payload": {
                "objectId": object_id,
                "effectType": effect_type
            }
        }),
        ServerPacket::TeleportIn => json!({
            "type": "packet",
            "packet": "TeleportIn",
            "payload": {}
        }),
        ServerPacket::NPCGoods {
            list,
            rate,
            panel_type,
            hide_added_stats,
        } => json!({
            "type": "packet",
            "packet": "NPCGoods",
            "payload": {
                "list": npc_goods_list_json(list, *rate),
                "rate": rate,
                "panelType": panel_type,
                "hideAddedStats": hide_added_stats
            }
        }),
        ServerPacket::NPCSell => json!({
            "type": "packet",
            "packet": "NPCSell",
            "payload": {}
        }),
        ServerPacket::NPCRepair { rate } => json!({
            "type": "packet",
            "packet": "NPCRepair",
            "payload": {
                "rate": rate
            }
        }),
        ServerPacket::NPCSRepair { rate } => json!({
            "type": "packet",
            "packet": "NPCSRepair",
            "payload": {
                "rate": rate
            }
        }),
        ServerPacket::NPCRefine { rate, refining } => json!({
            "type": "packet",
            "packet": "NPCRefine",
            "payload": {
                "rate": rate,
                "refining": refining
            }
        }),
        ServerPacket::NPCCheckRefine => json!({
            "type": "packet",
            "packet": "NPCCheckRefine",
            "payload": {}
        }),
        ServerPacket::NPCCollectRefine { success } => json!({
            "type": "packet",
            "packet": "NPCCollectRefine",
            "payload": {
                "success": success
            }
        }),
        ServerPacket::NPCReplaceWedRing { rate } => json!({
            "type": "packet",
            "packet": "NPCReplaceWedRing",
            "payload": {
                "rate": rate
            }
        }),
        ServerPacket::NPCStorage => json!({
            "type": "packet",
            "packet": "NPCStorage",
            "payload": {}
        }),
        ServerPacket::UserStorage { storage } => json!({
            "type": "packet",
            "packet": "UserStorage",
            "payload": {
                "storage": storage
            }
        }),
        ServerPacket::CombineItem {
            grid,
            id_from,
            id_to,
            success,
            destroy,
        } => json!({
            "type": "packet",
            "packet": "CombineItem",
            "payload": {
                "grid": grid,
                "idFrom": id_from,
                "idTo": id_to,
                "success": success,
                "destroy": destroy
            }
        }),
        ServerPacket::ItemUpgraded { item } => json!({
            "type": "packet",
            "packet": "ItemUpgraded",
            "payload": {
                "item": item
            }
        }),
        ServerPacket::ItemRepaired {
            unique_id,
            max_dura,
            current_dura,
        } => json!({
            "type": "packet",
            "packet": "ItemRepaired",
            "payload": {
                "uniqueId": unique_id,
                "maxDura": max_dura,
                "currentDura": current_dura
            }
        }),
        ServerPacket::ItemSlotSizeChanged {
            unique_id,
            slot_size,
        } => json!({
            "type": "packet",
            "packet": "ItemSlotSizeChanged",
            "payload": {
                "uniqueId": unique_id,
                "slotSize": slot_size
            }
        }),
        ServerPacket::ItemSealChanged {
            unique_id,
            expiry_date_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "ItemSealChanged",
            "payload": {
                "uniqueId": unique_id,
                "expiryDateBinaryDatetime": expiry_date_binary_datetime
            }
        }),
        ServerPacket::NewMagic { magic, hero } => json!({
            "type": "packet",
            "packet": "NewMagic",
            "payload": {
                "magic": magic,
                "hero": hero
            }
        }),
        ServerPacket::RemoveMagic { place_id } => json!({
            "type": "packet",
            "packet": "RemoveMagic",
            "payload": {
                "placeId": place_id
            }
        }),
        ServerPacket::MagicLeveled {
            object_id,
            spell,
            level,
            experience,
        } => json!({
            "type": "packet",
            "packet": "MagicLeveled",
            "payload": {
                "objectId": object_id,
                "spell": format!("{spell:?}"),
                "level": level,
                "experience": experience
            }
        }),
        ServerPacket::Magic {
            spell,
            target_id,
            target,
            cast,
            level,
            secondary_target_ids,
        } => json!({
            "type": "packet",
            "packet": "Magic",
            "payload": {
                "spell": format!("{spell:?}"),
                "targetId": target_id,
                "target": { "x": target.x, "y": target.y },
                "cast": cast,
                "level": level,
                "secondaryTargetIds": secondary_target_ids
            }
        }),
        ServerPacket::MagicDelay {
            object_id,
            spell,
            delay,
        } => json!({
            "type": "packet",
            "packet": "MagicDelay",
            "payload": {
                "objectId": object_id,
                "spell": format!("{spell:?}"),
                "delay": delay
            }
        }),
        ServerPacket::MagicCast { spell } => json!({
            "type": "packet",
            "packet": "MagicCast",
            "payload": {
                "spell": format!("{spell:?}")
            }
        }),
        ServerPacket::ObjectMagic {
            object_id,
            location,
            direction,
            spell,
            target_id,
            target,
            cast,
            level,
            self_broadcast,
            secondary_target_ids,
        } => json!({
            "type": "packet",
            "packet": "ObjectMagic",
            "payload": {
                "objectId": object_id,
                "location": { "x": location.x, "y": location.y },
                "direction": format!("{direction:?}"),
                "spell": format!("{spell:?}"),
                "targetId": target_id,
                "target": { "x": target.x, "y": target.y },
                "cast": cast,
                "level": level,
                "selfBroadcast": self_broadcast,
                "secondaryTargetIds": secondary_target_ids
            }
        }),
        ServerPacket::SpellToggle {
            object_id,
            spell,
            can_use,
        } => json!({
            "type": "packet",
            "packet": "SpellToggle",
            "payload": {
                "objectId": object_id,
                "spell": format!("{spell:?}"),
                "canUse": can_use
            }
        }),
        ServerPacket::SwitchGroup { allow_group } => json!({
            "type": "packet",
            "packet": "SwitchGroup",
            "payload": {
                "allowGroup": allow_group
            }
        }),
        ServerPacket::DeleteGroup => json!({
            "type": "packet",
            "packet": "DeleteGroup",
            "payload": {}
        }),
        ServerPacket::DeleteMember { name } => json!({
            "type": "packet",
            "packet": "DeleteMember",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::GroupInvite { name } => json!({
            "type": "packet",
            "packet": "GroupInvite",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::AddMember { name } => json!({
            "type": "packet",
            "packet": "AddMember",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::GroupMembersMap {
            player_name,
            player_map,
        } => json!({
            "type": "packet",
            "packet": "GroupMembersMap",
            "payload": {
                "playerName": player_name,
                "playerMap": player_map
            }
        }),
        // Custom roster snapshot (beyond Crystal): the full enriched party list so
        // the browser group window can render member level/class/HP/online in one
        // update. `members[0]` is the group leader (Crystal `GroupMembers[0]`,
        // e.g. PlayerObject.cs:2497); `leaderName` echoes it. Per-member fields the
        // simulation can't know (e.g. a remote player's HP) are omitted by the
        // `GroupMember` serde `skip_serializing_if` so consumers see only real data.
        ServerPacket::GroupMemberInfo {
            members,
            leader_name,
        } => json!({
            "type": "packet",
            "packet": "GroupMemberInfo",
            "payload": {
                "members": members,
                "leaderName": leader_name
            }
        }),
        ServerPacket::SendMemberLocation {
            member_name,
            member_location,
        } => json!({
            "type": "packet",
            "packet": "SendMemberLocation",
            "payload": {
                "memberName": member_name,
                "memberLocation": {
                    "x": member_location.x,
                    "y": member_location.y
                }
            }
        }),
        ServerPacket::SellItem {
            unique_id,
            count,
            success,
        } => json!({
            "type": "packet",
            "packet": "SellItem",
            "payload": {
                "uniqueId": unique_id,
                "count": count,
                "success": success
            }
        }),
        ServerPacket::RepairItem { unique_id } => json!({
            "type": "packet",
            "packet": "RepairItem",
            "payload": {
                "uniqueId": unique_id
            }
        }),
        ServerPacket::CraftItem { success } => json!({
            "type": "packet",
            "packet": "CraftItem",
            "payload": {
                "success": success
            }
        }),
        ServerPacket::ObjectTurn { movement } => json!({
            "type": "packet",
            "packet": "ObjectTurn",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectWalk { movement } => json!({
            "type": "packet",
            "packet": "ObjectWalk",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectRun { movement } => json!({
            "type": "packet",
            "packet": "ObjectRun",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectBackStep { movement, distance } => {
            let mut payload = movement_json(
                movement.object_id,
                movement.position.x,
                movement.position.y,
                movement.direction,
            );
            payload.insert("distance", json!(distance));
            json!({
                "type": "packet",
                "packet": "ObjectBackStep",
                "payload": payload
            })
        }
        ServerPacket::ObjectSitDown { movement, sitting } => {
            let mut payload = movement_json(
                movement.object_id,
                movement.position.x,
                movement.position.y,
                movement.direction,
            );
            payload.insert("sitting", json!(sitting));
            json!({
                "type": "packet",
                "packet": "ObjectSitDown",
                "payload": payload
            })
        }
        ServerPacket::ObjectHarvest { movement } => json!({
            "type": "packet",
            "packet": "ObjectHarvest",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectHarvested { movement } => json!({
            "type": "packet",
            "packet": "ObjectHarvested",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::Chat { message, chat_type } => json!({
            "type": "packet",
            "packet": "Chat",
            "payload": {
                "message": message,
                "chatType": format!("{:?}", chat_type)
            }
        }),
        ServerPacket::ObjectChat {
            object_id,
            text,
            chat_type,
        } => json!({
            "type": "packet",
            "packet": "ObjectChat",
            "payload": {
                "objectId": object_id,
                "text": text,
                "chatType": format!("{:?}", chat_type)
            }
        }),
        ServerPacket::NewItemInfo { info } => json!({
            "type": "packet",
            "packet": "NewItemInfo",
            "payload": {
                "info": info
            }
        }),
        ServerPacket::NewHeroInfo {
            info,
            storage_index,
        } => json!({
            "type": "packet",
            "packet": "NewHeroInfo",
            "payload": {
                "info": info,
                "storageIndex": storage_index
            }
        }),
        ServerPacket::MoveItem {
            grid,
            from,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "MoveItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::EquipItem {
            grid,
            unique_id,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "EquipItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "uniqueId": unique_id,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
            success,
        } => json!({
            "type": "packet",
            "packet": "MergeItem",
            "payload": {
                "gridFrom": format!("{:?}", grid_from),
                "gridTo": format!("{:?}", grid_to),
                "idFrom": id_from,
                "idTo": id_to,
                "success": success
            }
        }),
        ServerPacket::RemoveItem {
            grid,
            unique_id,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "RemoveItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "uniqueId": unique_id,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RemoveSlotItem {
            grid,
            grid_to,
            unique_id,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "RemoveSlotItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "gridTo": format!("{:?}", grid_to),
                "uniqueId": unique_id,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::DepositRefineItem { from, to, success } => json!({
            "type": "packet",
            "packet": "DepositRefineItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RetrieveRefineItem { from, to, success } => json!({
            "type": "packet",
            "packet": "RetrieveRefineItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RefineCancel => json!({
            "type": "packet",
            "packet": "RefineCancel",
            "payload": {}
        }),
        ServerPacket::RefineItem { unique_id } => json!({
            "type": "packet",
            "packet": "RefineItem",
            "payload": {
                "uniqueId": unique_id
            }
        }),
        ServerPacket::TakeBackItem { from, to, success } => json!({
            "type": "packet",
            "packet": "TakeBackItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::TakeBackItemV2 {
            request_id,
            from,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "TakeBackItemV2",
            "payload": {
                "requestId": request_id,
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::StoreItem { from, to, success } => json!({
            "type": "packet",
            "packet": "StoreItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::StoreItemV2 {
            request_id,
            from,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "StoreItemV2",
            "payload": {
                "requestId": request_id,
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::TakeBackHeroItem { from, to, success } => json!({
            "type": "packet",
            "packet": "TakeBackHeroItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::TransferHeroItem { from, to, success } => json!({
            "type": "packet",
            "packet": "TransferHeroItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::SplitItem { item, grid } => json!({
            "type": "packet",
            "packet": "SplitItem",
            "payload": {
                "item": item,
                "grid": format!("{:?}", grid)
            }
        }),
        ServerPacket::SplitItem1 {
            grid,
            unique_id,
            count,
            success,
        } => json!({
            "type": "packet",
            "packet": "SplitItem1",
            "payload": {
                "grid": format!("{:?}", grid),
                "uniqueId": unique_id,
                "count": count,
                "success": success
            }
        }),
        ServerPacket::UseItem {
            unique_id,
            success,
            grid,
        } => json!({
            "type": "packet",
            "packet": "UseItem",
            "payload": {
                "uniqueId": unique_id,
                "success": success,
                "grid": format!("{:?}", grid)
            }
        }),
        ServerPacket::DropItem {
            unique_id,
            count,
            hero_inventory,
            success,
        } => json!({
            "type": "packet",
            "packet": "DropItem",
            "payload": {
                "uniqueId": unique_id,
                "count": count,
                "heroInventory": hero_inventory,
                "success": success
            }
        }),
        ServerPacket::DeleteItem { unique_id, count } => json!({
            "type": "packet",
            "packet": "DeleteItem",
            "payload": {
                "uniqueId": unique_id,
                "count": count
            }
        }),
        ServerPacket::ObjectItem { info } => json!({
            "type": "packet",
            "packet": "ObjectItem",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "image": info.image,
                "grade": info.grade
            }
        }),
        ServerPacket::ObjectGold { info } => json!({
            "type": "packet",
            "packet": "ObjectGold",
            "payload": {
                "objectId": info.object_id,
                "gold": info.gold,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                }
            }
        }),
        ServerPacket::GainedItem { item } => json!({
            "type": "packet",
            "packet": "GainedItem",
            "payload": {
                "item": item
            }
        }),
        ServerPacket::GainedGold { gold } => json!({
            "type": "packet",
            "packet": "GainedGold",
            "payload": {
                "gold": gold
            }
        }),
        ServerPacket::LoseGold { gold } => json!({
            "type": "packet",
            "packet": "LoseGold",
            "payload": {
                "gold": gold
            }
        }),
        ServerPacket::GainedCredit { credit } => json!({
            "type": "packet",
            "packet": "GainedCredit",
            "payload": {
                "credit": credit
            }
        }),
        ServerPacket::LoseCredit { credit } => json!({
            "type": "packet",
            "packet": "LoseCredit",
            "payload": {
                "credit": credit
            }
        }),
        ServerPacket::NewMonsterInfo { info } => json!({
            "type": "packet",
            "packet": "NewMonsterInfo",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "image": info.image,
                "direction": format!("{:?}", info.direction),
                "effect": info.effect,
                "ai": info.ai,
                "light": info.light,
                "dead": info.dead,
                "skeleton": info.skeleton,
                "poison": info.poison,
                "hidden": info.hidden,
                "shockTime": info.shock_time,
                "bindingShotCenter": info.binding_shot_center,
                "extra": info.extra,
                "extraByte": info.extra_byte,
                "masterObjectId": info.master_object_id,
                "rarity": info.rarity,
                "buffs": info.buffs
            }
        }),
        ServerPacket::ObjectMonster { info } => json!({
            "type": "packet",
            "packet": "ObjectMonster",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "image": info.image,
                "direction": format!("{:?}", info.direction),
                "effect": info.effect,
                "ai": info.ai,
                "light": info.light,
                "dead": info.dead,
                "skeleton": info.skeleton,
                "poison": info.poison,
                "hidden": info.hidden,
                "shockTime": info.shock_time,
                "bindingShotCenter": info.binding_shot_center,
                "extra": info.extra,
                "extraByte": info.extra_byte,
                "masterObjectId": info.master_object_id,
                "rarity": info.rarity,
                "buffs": info.buffs
            }
        }),
        ServerPacket::ObjectAttack { info } => json!({
            "type": "packet",
            "packet": "ObjectAttack",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "spell": info.spell,
                "level": info.level,
                "attackType": info.attack_type
            }
        }),
        ServerPacket::Struck { info } => json!({
            "type": "packet",
            "packet": "Struck",
            "payload": {
                "attackerId": info.attacker_id
            }
        }),
        ServerPacket::ObjectStruck { info } => json!({
            "type": "packet",
            "packet": "ObjectStruck",
            "payload": {
                "objectId": info.object_id,
                "attackerId": info.attacker_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction)
            }
        }),
        ServerPacket::DuraChanged {
            unique_id,
            current_dura,
        } => json!({
            "type": "packet",
            "packet": "DuraChanged",
            "payload": {
                "uniqueId": unique_id,
                "currentDura": current_dura
            }
        }),
        ServerPacket::HeroHealthChanged { hp, mp } => json!({
            "type": "packet",
            "packet": "HeroHealthChanged",
            "payload": {
                "hp": hp,
                "mp": mp
            }
        }),
        ServerPacket::NewNpcInfo { info } => json!({
            "type": "packet",
            "packet": "NewNpcInfo",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "image": info.image,
                "colourArgb": info.colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "questIds": info.quest_ids
            }
        }),
        ServerPacket::ObjectNpc { info } => json!({
            "type": "packet",
            "packet": "ObjectNpc",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "image": info.image,
                "colourArgb": info.colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "questIds": info.quest_ids
            }
        }),
        ServerPacket::NPCResponse { page } => json!({
            "type": "packet",
            "packet": "NPCResponse",
            "payload": { "page": page }
        }),
        ServerPacket::ObjectDied { info } => json!({
            "type": "packet",
            "packet": "ObjectDied",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "kind": info.kind
            }
        }),
        // Self-death signal (Crystal `S.Death`): the client marks the player dead
        // and surfaces the revive-in-town prompt. Distinct from `ObjectDied` (others).
        ServerPacket::Death {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "Death",
            "payload": {
                "location": {
                    "x": location.x,
                    "y": location.y
                },
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::GainHeroExperience { amount } => json!({
            "type": "packet",
            "packet": "GainHeroExperience",
            "payload": {
                "amount": amount
            }
        }),
        ServerPacket::HeroLevelChanged {
            level,
            experience,
            max_experience,
        } => json!({
            "type": "packet",
            "packet": "HeroLevelChanged",
            "payload": {
                "level": level,
                "experience": experience,
                "maxExperience": max_experience
            }
        }),
        ServerPacket::ObjectHide { object_id } => json!({
            "type": "packet",
            "packet": "ObjectHide",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectShow { object_id } => json!({
            "type": "packet",
            "packet": "ObjectShow",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectRevived { info } => json!({
            "type": "packet",
            "packet": "ObjectRevived",
            "payload": {
                "objectId": info.object_id,
                "effect": info.effect
            }
        }),
        // Self-revive reply to `TownRevive` (Crystal `S.Revived`): clears the dead
        // state and dismisses the revive prompt; the player has respawned in town.
        ServerPacket::Revived => json!({
            "type": "packet",
            "packet": "Revived",
            "payload": {}
        }),
        ServerPacket::ObjectEffect { info } => json!({
            "type": "packet",
            "packet": "ObjectEffect",
            "payload": {
                "objectId": info.object_id,
                "effect": info.effect,
                "effectType": info.effect_type,
                "delayTime": info.delay_time,
                "time": info.time
            }
        }),
        ServerPacket::ObjectHealth { info } => json!({
            "type": "packet",
            "packet": "ObjectHealth",
            "payload": {
                "objectId": info.object_id,
                "percent": info.percent,
                "expire": info.expire
            }
        }),
        ServerPacket::ObjectMana { info } => json!({
            "type": "packet",
            "packet": "ObjectMana",
            "payload": {
                "objectId": info.object_id,
                "percent": info.percent
            }
        }),
        ServerPacket::ObjectProjectile {
            spell,
            source_id,
            destination_id,
        } => json!({
            "type": "packet",
            "packet": "ObjectProjectile",
            "payload": {
                "spell": format!("{:?}", spell),
                "sourceId": source_id,
                "destinationId": destination_id
            }
        }),
        ServerPacket::RangeAttack {
            target_id,
            target,
            spell,
        } => json!({
            "type": "packet",
            "packet": "RangeAttack",
            "payload": {
                "targetId": target_id,
                "target": {"x": target.x, "y": target.y},
                "spell": format!("{:?}", spell)
            }
        }),
        ServerPacket::Pushed {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "Pushed",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectPushed {
            object_id,
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "ObjectPushed",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::MapEffect {
            location,
            effect,
            value,
        } => json!({
            "type": "packet",
            "packet": "MapEffect",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "effect": effect,
                "value": value
            }
        }),
        ServerPacket::AllowObserve { allow } => json!({
            "type": "packet",
            "packet": "AllowObserve",
            "payload": {
                "allow": allow
            }
        }),
        // Crystal `S.AddBuff` ships a `ClientBuff` (Type, Caster, Visible, ObjectID,
        // ExpireTime, Infinite, Paused, Stats, Values — Crystal/Shared/Data/
        // ClientData.cs:575). The browser contract additionally wants `type`,
        // `remainingMs`, and `stats` rendered as `{label, value}` (Crystal labels a
        // buff stat with `Stat.ToString()`, BuffDialog.cs:351). In the simulation
        // `ClientBuff.expire_time` is already milliseconds-from-now (it is built as
        // `(expires_at_tick - tick) * 1000`, runtime/buffs.rs:168), so it maps
        // straight to `remainingMs`. `caster` and `name` are omitted: the
        // simulation's buff packet does not carry them (Crystal's `Caster` is a
        // client-only field that is never serialised on the wire, ClientData.cs:595).
        // Existing keys are kept for backward compatibility.
        ServerPacket::AddBuff { buff } => json!({
            "type": "packet",
            "packet": "AddBuff",
            "payload": {
                "buffType": buff.buff_type,
                "type": buff.buff_type,
                "visible": buff.visible,
                "objectId": buff.object_id,
                "expireTime": buff.expire_time,
                "remainingMs": buff.expire_time,
                "infinite": buff.infinite,
                "paused": buff.paused,
                // Superset objects: keep `stat`+`value` (backward compatible) and add
                // `label` (Crystal `Stat` enum name) for the browser buff window.
                "stats": buff
                    .stats
                    .iter()
                    .map(buff_stat_json)
                    .collect::<Vec<_>>(),
                "values": buff.values
            }
        }),
        ServerPacket::RemoveBuff {
            buff_type,
            object_id,
        } => json!({
            "type": "packet",
            "packet": "RemoveBuff",
            "payload": {
                "buffType": buff_type,
                "objectId": object_id
            }
        }),
        ServerPacket::PauseBuff {
            buff_type,
            object_id,
            paused,
        } => json!({
            "type": "packet",
            "packet": "PauseBuff",
            "payload": {
                "buffType": buff_type,
                "objectId": object_id,
                "paused": paused
            }
        }),
        ServerPacket::ObjectHidden { object_id, hidden } => json!({
            "type": "packet",
            "packet": "ObjectHidden",
            "payload": {
                "objectId": object_id,
                "hidden": hidden
            }
        }),
        ServerPacket::ObjectRangeAttack { info } => json!({
            "type": "packet",
            "packet": "ObjectRangeAttack",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "targetId": info.target_id,
                "target": {
                    "x": info.target.x,
                    "y": info.target.y
                },
                "attackType": info.attack_type,
                "spell": info.spell,
                "level": info.level
            }
        }),
        ServerPacket::UserDash {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserDash",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectDash {
            object_id,
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "ObjectDash",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::UserDashFail {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserDashFail",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectDashFail {
            object_id,
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "ObjectDashFail",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::UserDashAttack {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserDashAttack",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectDashAttack {
            object_id,
            location,
            direction,
            distance,
        } => json!({
            "type": "packet",
            "packet": "ObjectDashAttack",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction),
                "distance": distance
            }
        }),
        ServerPacket::UserAttackMove {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserAttackMove",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::RefreshItem { item } => json!({
            "type": "packet",
            "packet": "RefreshItem",
            "payload": {
                "item": item
            }
        }),
        ServerPacket::ObjectSpell { info } => json!({
            "type": "packet",
            "packet": "ObjectSpell",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "spell": info.spell as u8,
                "direction": format!("{:?}", info.direction),
                "param": info.param
            }
        }),
        ServerPacket::ConsignItem { unique_id, success } => json!({
            "type": "packet",
            "packet": "ConsignItem",
            "payload": {
                "uniqueId": unique_id,
                "success": success
            }
        }),
        ServerPacket::MarketFail { reason } => json!({
            "type": "packet",
            "packet": "MarketFail",
            "payload": {
                "reason": reason
            }
        }),
        ServerPacket::MarketSuccess { message } => json!({
            "type": "packet",
            "packet": "MarketSuccess",
            "payload": {
                "message": message
            }
        }),
        ServerPacket::HeroCreateRequest { can_create_class } => json!({
            "type": "packet",
            "packet": "HeroCreateRequest",
            "payload": {
                "canCreateClass": can_create_class
            }
        }),
        ServerPacket::UpdateHeroSpawnState { state } => json!({
            "type": "packet",
            "packet": "UpdateHeroSpawnState",
            "payload": {
                "state": state
            }
        }),
        ServerPacket::UnlockHeroAutoPot => json!({
            "type": "packet",
            "packet": "UnlockHeroAutoPot",
            "payload": {}
        }),
        ServerPacket::SetHeroBehaviour { behaviour } => json!({
            "type": "packet",
            "packet": "SetHeroBehaviour",
            "payload": {
                "behaviour": behaviour
            }
        }),
        ServerPacket::ManageHeroes {
            maximum_count,
            current_hero,
            heroes,
        } => json!({
            "type": "packet",
            "packet": "ManageHeroes",
            "payload": {
                "maximumCount": maximum_count,
                "currentHero": current_hero,
                "heroes": heroes
            }
        }),
        ServerPacket::ChangeHero { from_index } => json!({
            "type": "packet",
            "packet": "ChangeHero",
            "payload": {
                "fromIndex": from_index
            }
        }),
        ServerPacket::DefaultNPC { object_id } => json!({
            "type": "packet",
            "packet": "DefaultNPC",
            "payload": { "objectId": object_id }
        }),
        ServerPacket::NPCUpdate { npc_id } => json!({
            "type": "packet",
            "packet": "NPCUpdate",
            "payload": { "npcId": npc_id }
        }),
        ServerPacket::MarriageRequest { name } => json!({
            "type": "packet",
            "packet": "MarriageRequest",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::DivorceRequest { name } => json!({
            "type": "packet",
            "packet": "DivorceRequest",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::MentorRequest { name, level } => json!({
            "type": "packet",
            "packet": "MentorRequest",
            "payload": {
                "name": name,
                "level": level
            }
        }),
        ServerPacket::DepositTradeItem { from, to, success } => json!({
            "type": "packet",
            "packet": "DepositTradeItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RetrieveTradeItem { from, to, success } => json!({
            "type": "packet",
            "packet": "RetrieveTradeItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        // Crystal sends `TradeRequest`/`TradeAccept` to the *partner* carrying the
        // other player's name (`Crystal/Server/MirObjects/PlayerObject.cs:10705,
        // 10741-10742`), which the client stores as the guest/partner name
        // (`Crystal/Client/MirScenes/GameScene.cs:6314`). `partnerName` is the
        // contract key; `name` is kept for backward compatibility.
        ServerPacket::TradeRequest { name } => json!({
            "type": "packet",
            "packet": "TradeRequest",
            "payload": {
                "name": name,
                "partnerName": name
            }
        }),
        ServerPacket::TradeAccept { name } => json!({
            "type": "packet",
            "packet": "TradeAccept",
            "payload": {
                "name": name,
                "partnerName": name
            }
        }),
        // `TradeGold` is enqueued to the partner with the *partner's* running gold
        // offer (`Crystal/Server/MirObjects/PlayerObject.cs:10759`); the client
        // stores it as `GuestTradeDialog.GuestGold`
        // (`Crystal/Client/MirScenes/GameScene.cs:6319`). `partnerGold` is the
        // contract key; `amount` is kept for backward compatibility.
        ServerPacket::TradeGold { amount } => json!({
            "type": "packet",
            "packet": "TradeGold",
            "payload": {
                "amount": amount,
                "partnerGold": amount
            }
        }),
        // `TradeItem` is enqueued to the partner with the *partner's* offered items
        // (`Crystal/Server/MirObjects/PlayerObject.cs:10776`); the client stores it
        // as `GuestTradeDialog.GuestItems`
        // (`Crystal/Client/MirScenes/GameScene.cs:6325`). `partnerItems` resolves
        // each `UserItem.item_index` to `{ name, count?, grade? }`; the raw
        // `tradeItems` array is kept for backward compatibility. `myItems` /
        // `myGold` / `myLocked` / `partnerLocked` are intentionally omitted: the
        // Crystal trade protocol only pushes the partner's side to each client
        // (own offer + lock state are tracked client-side), so the simulation
        // cannot know them here.
        ServerPacket::TradeItem { trade_items } => json!({
            "type": "packet",
            "packet": "TradeItem",
            "payload": {
                "tradeItems": trade_items,
                "partnerItems": trade_items_summary_json(trade_items)
            }
        }),
        ServerPacket::TradeConfirm => json!({
            "type": "packet",
            "packet": "TradeConfirm",
            "payload": {}
        }),
        ServerPacket::TradeCancel { unlock } => json!({
            "type": "packet",
            "packet": "TradeCancel",
            "payload": {
                "unlock": unlock
            }
        }),
        ServerPacket::MountUpdate {
            object_id,
            mount_type,
            riding_mount,
        } => json!({
            "type": "packet",
            "packet": "MountUpdate",
            "payload": {
                "objectId": object_id,
                "mountType": mount_type,
                "ridingMount": riding_mount
            }
        }),
        ServerPacket::FishingUpdate {
            object_id,
            fishing,
            progress_percent,
            chance_percent,
            fishing_point,
            found_fish,
        } => json!({
            "type": "packet",
            "packet": "FishingUpdate",
            "payload": {
                "objectId": object_id,
                "fishing": fishing,
                "progressPercent": progress_percent,
                "chancePercent": chance_percent,
                "fishingPoint": {
                    "x": fishing_point.x,
                    "y": fishing_point.y
                },
                "foundFish": found_fish
            }
        }),
        ServerPacket::RemoveDelayedExplosion { object_id } => json!({
            "type": "packet",
            "packet": "RemoveDelayedExplosion",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectDeco {
            object_id,
            location,
            image,
        } => json!({
            "type": "packet",
            "packet": "ObjectDeco",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "image": image
            }
        }),
        ServerPacket::ObjectSneaking {
            object_id,
            sneaking_active,
        } => json!({
            "type": "packet",
            "packet": "ObjectSneaking",
            "payload": {
                "objectId": object_id,
                "sneakingActive": sneaking_active
            }
        }),
        ServerPacket::ObjectLevelEffects {
            object_id,
            level_effects,
        } => json!({
            "type": "packet",
            "packet": "ObjectLevelEffects",
            "payload": {
                "objectId": object_id,
                "levelEffects": level_effects
            }
        }),
        ServerPacket::SetBindingShot {
            object_id,
            enabled,
            value,
        } => json!({
            "type": "packet",
            "packet": "SetBindingShot",
            "payload": {
                "objectId": object_id,
                "enabled": enabled,
                "value": value
            }
        }),
        ServerPacket::SendOutputMessage {
            message,
            output_type,
        } => json!({
            "type": "packet",
            "packet": "SendOutputMessage",
            "payload": {
                "message": message,
                "outputType": output_type
            }
        }),
        ServerPacket::OpenDoor { door_index, close } => json!({
            "type": "packet",
            "packet": "Opendoor",
            "payload": {
                "doorIndex": door_index,
                "close": close
            }
        }),
        ServerPacket::OpenBrowser { url } => json!({
            "type": "packet",
            "packet": "OpenBrowser",
            "payload": {
                "url": url
            }
        }),
        ServerPacket::PlaySound { sound } => json!({
            "type": "packet",
            "packet": "PlaySound",
            "payload": {
                "sound": sound
            }
        }),
        ServerPacket::SetTimer {
            key,
            timer_type,
            seconds,
        } => json!({
            "type": "packet",
            "packet": "SetTimer",
            "payload": {
                "key": key,
                "timerType": timer_type,
                "seconds": seconds
            }
        }),
        ServerPacket::ExpireTimer { key } => json!({
            "type": "packet",
            "packet": "ExpireTimer",
            "payload": {
                "key": key
            }
        }),
        ServerPacket::Roll {
            roll_type,
            page,
            result,
            auto_roll,
        } => json!({
            "type": "packet",
            "packet": "Roll",
            "payload": {
                "rollType": roll_type,
                "page": page,
                "result": result,
                "autoRoll": auto_roll
            }
        }),
        ServerPacket::SetCompass { location } => json!({
            "type": "packet",
            "packet": "SetCompass",
            "payload": {
                "location": {
                    "x": location.x,
                    "y": location.y
                }
            }
        }),
        ServerPacket::NPCAwakening => json!({
            "type": "packet",
            "packet": "NPCAwakening",
            "payload": {}
        }),
        ServerPacket::NPCDisassemble => json!({
            "type": "packet",
            "packet": "NPCDisassemble",
            "payload": {}
        }),
        ServerPacket::NPCDowngrade => json!({
            "type": "packet",
            "packet": "NPCDowngrade",
            "payload": {}
        }),
        ServerPacket::NPCReset => json!({
            "type": "packet",
            "packet": "NPCReset",
            "payload": {}
        }),
        ServerPacket::AwakeningLockedItem { unique_id, locked } => json!({
            "type": "packet",
            "packet": "AwakeningLockedItem",
            "payload": {
                "uniqueId": unique_id,
                "locked": locked
            }
        }),
        ServerPacket::Awakening { result, remove_id } => json!({
            "type": "packet",
            "packet": "Awakening",
            "payload": {
                "result": result,
                "removeId": remove_id
            }
        }),
        ServerPacket::ReceiveMail { mail } => json!({
            "type": "packet",
            "packet": "ReceiveMail",
            "payload": {
                "mail": mail
            }
        }),
        ServerPacket::MailLockedItem { unique_id, locked } => json!({
            "type": "packet",
            "packet": "MailLockedItem",
            "payload": {
                "uniqueId": unique_id,
                "locked": locked
            }
        }),
        ServerPacket::MailSendRequest => json!({
            "type": "packet",
            "packet": "MailSendRequest",
            "payload": {}
        }),
        ServerPacket::MailSent { result } => json!({
            "type": "packet",
            "packet": "MailSent",
            "payload": {
                "result": result
            }
        }),
        ServerPacket::ParcelCollected { result } => json!({
            "type": "packet",
            "packet": "ParcelCollected",
            "payload": {
                "result": result
            }
        }),
        ServerPacket::MailCost { cost } => json!({
            "type": "packet",
            "packet": "MailCost",
            "payload": {
                "cost": cost
            }
        }),
        // Crystal `S.FriendUpdate` carries one flat `List<ClientFriend>` where each
        // entry has Name/Memo/Blocked/Online (Crystal/Shared/Data/ClientData.cs:122,
        // populated by CharacterInfo.CreateClientFriend, CharacterInfo.cs:759). The
        // browser contract wants the roster pre-split into a non-blocked `friends`
        // list and a `blocked` list, with friend objects carrying name/online/memo.
        // `level`/`mapName` are omitted: neither Crystal's `ClientFriend` nor the
        // simulation's social state tracks them, so they are genuinely unknown here.
        ServerPacket::FriendUpdate { friends } => json!({
            "type": "packet",
            "packet": "FriendUpdate",
            "payload": {
                "friends": friends
                    .iter()
                    .filter(|friend| !friend.blocked)
                    .map(friend_entry_json)
                    .collect::<Vec<_>>(),
                "blocked": friends
                    .iter()
                    .filter(|friend| friend.blocked)
                    .map(blocked_entry_json)
                    .collect::<Vec<_>>()
            }
        }),
        ServerPacket::LoverUpdate {
            name,
            date_binary_datetime,
            map_name,
            married_days,
        } => json!({
            "type": "packet",
            "packet": "LoverUpdate",
            "payload": {
                "name": name,
                "dateBinaryDatetime": date_binary_datetime,
                "mapName": map_name,
                "marriedDays": married_days
            }
        }),
        ServerPacket::MentorUpdate {
            name,
            level,
            online,
            mentee_exp,
        } => json!({
            "type": "packet",
            "packet": "MentorUpdate",
            "payload": {
                "name": name,
                "level": level,
                "online": online,
                "menteeExp": mentee_exp
            }
        }),
        ServerPacket::GuildBuffList {
            remove,
            active_buffs,
            guild_buffs,
        } => json!({
            "type": "packet",
            "packet": "GuildBuffList",
            "payload": {
                "remove": remove,
                "activeBuffs": active_buffs,
                "guildBuffs": guild_buffs
            }
        }),
        ServerPacket::GameShopInfo { item, stock_level } => json!({
            "type": "packet",
            "packet": "GameShopInfo",
            "payload": {
                "item": item,
                "stockLevel": stock_level
            }
        }),
        ServerPacket::NewIntelligentCreature { creature } => json!({
            "type": "packet",
            "packet": "NewIntelligentCreature",
            "payload": {
                "creature": creature
            }
        }),
        ServerPacket::UpdateIntelligentCreatureList {
            creature_list,
            creature_summoned,
            summoned_creature_type,
            pearl_count,
        } => json!({
            "type": "packet",
            "packet": "UpdateIntelligentCreatureList",
            "payload": {
                "creatureList": creature_list,
                "creatureSummoned": creature_summoned,
                "summonedCreatureType": summoned_creature_type,
                "pearlCount": pearl_count
            }
        }),
        ServerPacket::IntelligentCreatureEnableRename => json!({
            "type": "packet",
            "packet": "IntelligentCreatureEnableRename",
            "payload": {}
        }),
        ServerPacket::IntelligentCreaturePickup { object_id } => json!({
            "type": "packet",
            "packet": "IntelligentCreaturePickup",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::GetRentedItems { rented_items } => json!({
            "type": "packet",
            "packet": "GetRentedItems",
            "payload": {
                "rentedItems": rented_items
            }
        }),
        ServerPacket::ItemRentalRequest { name, renting } => json!({
            "type": "packet",
            "packet": "ItemRentalRequest",
            "payload": {
                "name": name,
                "renting": renting
            }
        }),
        ServerPacket::ItemRentalFee { amount } => json!({
            "type": "packet",
            "packet": "ItemRentalFee",
            "payload": {
                "amount": amount
            }
        }),
        ServerPacket::ItemRentalPeriod { days } => json!({
            "type": "packet",
            "packet": "ItemRentalPeriod",
            "payload": {
                "days": days
            }
        }),
        ServerPacket::DepositRentalItem { from, to, success } => json!({
            "type": "packet",
            "packet": "DepositRentalItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RetrieveRentalItem { from, to, success } => json!({
            "type": "packet",
            "packet": "RetrieveRentalItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::UpdateRentalItem { loan_item } => json!({
            "type": "packet",
            "packet": "UpdateRentalItem",
            "payload": {
                "loanItem": loan_item
            }
        }),
        ServerPacket::CancelItemRental => json!({
            "type": "packet",
            "packet": "CancelItemRental",
            "payload": {}
        }),
        ServerPacket::ItemRentalLock {
            success,
            gold_locked,
            item_locked,
        } => json!({
            "type": "packet",
            "packet": "ItemRentalLock",
            "payload": {
                "success": success,
                "goldLocked": gold_locked,
                "itemLocked": item_locked
            }
        }),
        ServerPacket::ItemRentalPartnerLock {
            gold_locked,
            item_locked,
        } => json!({
            "type": "packet",
            "packet": "ItemRentalPartnerLock",
            "payload": {
                "goldLocked": gold_locked,
                "itemLocked": item_locked
            }
        }),
        ServerPacket::CanConfirmItemRental => json!({
            "type": "packet",
            "packet": "CanConfirmItemRental",
            "payload": {}
        }),
        ServerPacket::ConfirmItemRental => json!({
            "type": "packet",
            "packet": "ConfirmItemRental",
            "payload": {}
        }),
        // Crystal `ChangeQuest` carries the dynamic `ClientQuestProgress`
        // (`Id` + `TaskList` + `Taken`/`Completed`/`New`) plus the `QuestState`
        // enum (Add=0/Update=1/Remove=2) and `TrackQuest`
        // (`Crystal/Shared/ServerPackets.cs:5214-5237`,
        // `Crystal/Shared/Data/ClientData.cs:524-573`). The static name /
        // description / rewards / npc / timeLimit live in `NewQuestInfo`, so here
        // the contract adds only the dynamic fields: `id`, `state` (the
        // `QuestState` byte), `objectives` (parsed from the `TaskList` progress
        // lines), and `descriptionLines` (the raw task lines for a fallback view).
        ServerPacket::ChangeQuest {
            quest_id,
            task_list,
            taken,
            completed,
            new,
            quest_state,
            track_quest,
        } => {
            let progress = quest_progress_totals(task_list);
            json!({
                "type": "packet",
                "packet": "ChangeQuest",
                "payload": {
                    "questId": quest_id,
                    "id": quest_id,
                    "taskList": task_list,
                    "state": quest_state,
                    "descriptionLines": task_list,
                    "objectives": Value::Array(
                        task_list.iter().map(|line| quest_objective_json(line)).collect()
                    ),
                    "current": progress.map(|(current, _)| current),
                    "required": progress.map(|(_, required)| required),
                    "taken": taken,
                    "completed": completed,
                    "new": new,
                    "questState": quest_state,
                    "trackQuest": track_quest
                }
            })
        }
        ServerPacket::CompleteQuest { completed_quests } => json!({
            "type": "packet",
            "packet": "CompleteQuest",
            "payload": {
                "completedQuests": completed_quests
            }
        }),
        ServerPacket::ShareQuest {
            quest_index,
            sharer_name,
        } => json!({
            "type": "packet",
            "packet": "ShareQuest",
            "payload": {
                "questIndex": quest_index,
                "sharerName": sharer_name
            }
        }),
        // Crystal `NewQuestInfo` carries the static `ClientQuestInfo`
        // (`Crystal/Shared/ServerPackets.cs:5285-5302`,
        // `Crystal/Shared/Data/ClientData.cs:360-440`). The raw `info` object is
        // kept for backward compatibility; the contract-shaped quest fields are
        // hoisted alongside it: `id`, `name`, `descriptionLines` (the quest
        // `Description`), `objectives` (the `TaskDescription` lines), `rewards`
        // (gold/exp/credit/items) and `timeLimit`. `npc` (a *name*) is omitted:
        // `ClientQuestInfo` only carries `NPCIndex` (a numeric index the Crystal
        // client resolves against its own NPC list), so the simulation cannot
        // supply an NPC name from this packet alone.
        ServerPacket::NewQuestInfo { info } => {
            let mut payload = serde_json::Map::new();
            payload.insert("info".into(), json!(info));
            payload.insert("id".into(), json!(info.index));
            payload.insert("name".into(), json!(info.name));
            if !info.group.is_empty() {
                payload.insert("group".into(), json!(info.group));
            }
            if !info.description.is_empty() {
                payload.insert("descriptionLines".into(), json!(info.description));
            }
            if !info.task_description.is_empty() {
                payload.insert(
                    "objectives".into(),
                    Value::Array(
                        info.task_description
                            .iter()
                            .map(|line| quest_objective_json(line))
                            .collect(),
                    ),
                );
            }
            if let Some(rewards) = quest_rewards_json(info) {
                payload.insert("rewards".into(), rewards);
            }
            if let Some(time_limit) = quest_time_limit_label(info.time_limit_in_seconds) {
                payload.insert("timeLimit".into(), json!(time_limit));
            }
            json!({
                "type": "packet",
                "packet": "NewQuestInfo",
                "payload": Value::Object(payload)
            })
        }
        ServerPacket::NewRecipeInfo { info } => json!({
            "type": "packet",
            "packet": "NewRecipeInfo",
            "payload": { "info": info }
        }),
        ServerPacket::ResizeInventory { size } => json!({
            "type": "packet",
            "packet": "ResizeInventory",
            "payload": {
                "size": size
            }
        }),
        ServerPacket::ResizeStorage {
            size,
            has_expanded_storage,
            expiry_time_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "ResizeStorage",
            "payload": {
                "size": size,
                "hasExpandedStorage": has_expanded_storage,
                "expiryTimeBinaryDatetime": expiry_time_binary_datetime
            }
        }),
        ServerPacket::StorageUnlockResult {
            result,
            has_password,
        } => json!({
            "type": "packet",
            "packet": "StorageUnlockResult",
            "payload": {
                "result": result,
                "hasPassword": has_password
            }
        }),
        ServerPacket::StoragePasswordResult {
            result,
            removing,
            has_password,
            last_set_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "StoragePasswordResult",
            "payload": {
                "result": result,
                "removing": removing,
                "hasPassword": has_password,
                "lastSetBinaryDatetime": last_set_binary_datetime
            }
        }),
        ServerPacket::LogOutSuccess { characters } => json!({
            "type": "packet",
            "packet": "LogOutSuccess",
            "payload": {
                "characters": characters.iter().map(|character| {
                    json!({
                        "index": character.index,
                        "name": character.name,
                        "level": character.level,
                        "class": format!("{:?}", character.class),
                        "gender": format!("{:?}", character.gender)
                    })
                }).collect::<Vec<_>>()
            }
        }),
        ServerPacket::LogOutFailed => json!({
            "type": "packet",
            "packet": "LogOutFailed",
            "payload": {}
        }),
        other => {
            let (packet_name, payload) = typed_packet_event_detail(other);
            json!({
                "type": "packet",
                "packet": packet_name,
                "payload": payload
            })
        }
    }
}

fn typed_packet_event_detail(packet: &ServerPacket) -> (String, Value) {
    let encoded = match serde_json::to_value(packet) {
        Ok(value) => value,
        Err(error) => {
            return (
                server_packet_display_name(packet),
                json!({
                    "typed": true,
                    "summary": format!("{:?}", packet),
                    "serializationError": error.to_string()
                }),
            );
        }
    };

    match encoded {
        Value::Object(variants) => {
            let Some((packet_name, payload)) = variants.into_iter().next() else {
                return (server_packet_display_name(packet), json!({ "typed": true }));
            };
            let payload = match payload {
                Value::Object(mut payload) => {
                    payload.insert("typed".to_string(), Value::Bool(true));
                    Value::Object(payload)
                }
                Value::Null => json!({ "typed": true }),
                value => json!({
                "typed": true,
                "value": value
                }),
            };
            (packet_name, payload)
        }
        Value::String(packet_name) => (packet_name, json!({ "typed": true })),
        Value::Null => (server_packet_display_name(packet), json!({ "typed": true })),
        value => (
            server_packet_display_name(packet),
            json!({
                "typed": true,
                "value": value
            }),
        ),
    }
}

fn raw_payload_detail(packet_name: &str, packet_id: i16, payload: &[u8]) -> Value {
    json!({
        "packetName": packet_name,
        "packetId": packet_id,
        "payloadLength": payload.len(),
        "payloadHex": packet_payload_hex(payload),
        "rawPayloadLength": payload.len()
    })
}

fn movement_json(
    object_id: u32,
    x: i32,
    y: i32,
    direction: MirDirection,
) -> HashMap<&'static str, Value> {
    HashMap::from([
        ("objectId", json!(object_id)),
        ("x", json!(x)),
        ("y", json!(y)),
        ("direction", json!(format!("{direction:?}"))),
    ])
}

#[cfg(test)]
mod tests {
    use super::{
        realm_info_event, responses_require_world_snapshot, should_send_world_snapshot_for_action,
        BrowserCommand, QaControlAction, SessionAction,
    };
    use crate::cache::GatewaySessionCache;
    use axum::extract::State;
    use axum::Json;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    use futures_util::{SinkExt, StreamExt};
    use mir2_game_data::{crystal_quest_packet_manifest, crystal_quest_packet_payloads};
    use mir2_protocol::{
        decode_server_packet, encode_frame, ClientAuction, ClientBuff, ClientFriend,
        ClientHeroInformation, ClientIntelligentCreature, ClientMail, ClientMapInfo, ClientPacket,
        ClientQuestInfo, GroupMember, IntelligentCreatureItemFilter, IntelligentCreatureRules,
        MapInformation, MirClass, MirDirection, MirGender, MirGridType, ObjectManaInfo, Point,
        RankCharacterInfo, SelectInfo, ServerPacket, ServerPacketId, Spell, UserItem, UserItemStat,
        UserLocation,
    };
    use mir2_simulation::{
        AccountStore, SimulationConfig, Stage5MailMessage, Stage5MailTargetKind, Stage5SystemsState,
    };
    use serde_json::{json, Value};
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message as ClientWebSocketMessage;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    type TestWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

    async fn send_test_websocket_json(socket: &mut TestWebSocket, value: Value) {
        socket
            .send(ClientWebSocketMessage::Text(value.to_string().into()))
            .await
            .expect("test WebSocket command should send");
    }

    async fn read_test_websocket_until(
        socket: &mut TestWebSocket,
        label: &str,
        predicate: impl Fn(&Value) -> bool,
    ) -> (Value, Vec<Value>) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut observed = Vec::new();
        loop {
            let message = tokio::time::timeout_at(deadline, socket.next())
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for {label}; observed={observed:?}"))
                .unwrap_or_else(|| panic!("WebSocket closed waiting for {label}"))
                .unwrap_or_else(|error| panic!("WebSocket failed waiting for {label}: {error}"));
            match message {
                ClientWebSocketMessage::Text(text) => {
                    let event: Value = serde_json::from_str(text.as_ref())
                        .unwrap_or_else(|error| panic!("invalid JSON frame for {label}: {error}"));
                    observed.push(event.clone());
                    if predicate(&event) {
                        return (event, observed);
                    }
                }
                ClientWebSocketMessage::Close(frame) => {
                    panic!("WebSocket closed waiting for {label}: {frame:?}")
                }
                _ => {}
            }
        }
    }

    fn test_packet(event: &Value, packet: &str) -> bool {
        event["type"] == "packet" && event["packet"] == packet
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blocking_component_initializer_supports_sync_clients_with_own_runtime() {
        let value = super::run_blocking_component_initializer("test sync client", || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?;
            Ok(runtime.block_on(async { 17_u8 }))
        })
        .await
        .expect("blocking initialization must not nest runtimes on a Tokio worker");

        assert_eq!(value, 17);
    }

    #[test]
    fn realm_info_exposes_platinum_176_release_identity() {
        let config = SimulationConfig::default().with_platinum_176_profile();
        let event = realm_info_event(&config);

        assert_eq!(event["type"], "realmInfo");
        assert_eq!(event["payload"]["schema"], "mir2-realm-handshake/1");
        assert_eq!(event["payload"]["profileId"], "platinum_176");
        assert_eq!(event["payload"]["profileVersion"], 25);
        assert_eq!(event["payload"]["acceptanceLevel"], 50);
        assert_eq!(
            event["payload"]["ratePolicy"]["monsterExperienceTiers"][0]["multiplier"],
            2
        );
        assert_eq!(
            event["payload"]["ratePolicy"]["monsterExperienceTiers"][2]["multiplier"],
            4
        );
        assert_eq!(
            event["payload"]["bundleHash"].as_str().map(str::len),
            Some(64)
        );
    }

    #[test]
    fn world_snapshot_skill_contract_serializes_authoritative_mp_cost() {
        let skill = mir2_simulation::SkillSnapshot {
            key: "fireball".to_string(),
            name: "FireBall".to_string(),
            description: "Authoritative Crystal skill.".to_string(),
            spell: Some("FireBall".to_string()),
            cast_kind: "target".to_string(),
            offensive: true,
            mp_cost: Some(7),
            level: 1,
            experience: 0,
            hotkey: 1,
            delay_ms: 500,
            cast_time_ms: 0,
            cooldown_remaining_ticks: 0,
        };

        let frame = json!({
            "type": "worldSnapshot",
            "payload": {
                "knownSkills": [skill]
            }
        });
        assert_eq!(frame["payload"]["knownSkills"][0]["mpCost"], 7);
        assert!(frame["payload"]["knownSkills"][0].get("mp_cost").is_none());
    }

    #[test]
    fn channel_identity_proofs_are_consumed_once() {
        let cache = crate::InMemoryGatewaySessionCache::default();
        let proof = super::VerifiedChannelSubject {
            subject: "sui:0xonce".to_string(),
            token_id: Some("channel-proof-once".to_string()),
            expires_at_ms: Some(super::gateway_now_ms().saturating_add(60_000)),
        };
        assert!(super::consume_channel_subject_proof(&cache, &proof).is_ok());
        let replay = super::consume_channel_subject_proof(&cache, &proof)
            .expect_err("the same channel proof must not be accepted twice");
        assert_eq!(replay.0, axum::http::StatusCode::UNAUTHORIZED);
    }

    fn sample_user_item(unique_id: u64, count: u16) -> UserItem {
        UserItem {
            unique_id,
            item_index: 321,
            current_dura: 1000,
            max_dura: 1000,
            count,
            soul_bound_id: -1,
            identified: true,
            cursed: false,
            slots: Vec::new(),
            gem_count: 1,
            added_stats: vec![UserItemStat { stat: 5, value: 1 }],
            awake_type: 0,
            awake_values: Vec::new(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        }
    }

    fn with_env_vars<T>(variables: &[(&str, Option<&str>)], action: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test env mutex should not be poisoned");
        let previous = variables
            .iter()
            .map(|(name, _)| (*name, std::env::var(name).ok()))
            .collect::<Vec<_>>();
        for (name, value) in variables {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        let result = action();
        for (name, value) in previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        result
    }

    fn with_env_var<T>(name: &str, value: Option<&str>, action: impl FnOnce() -> T) -> T {
        with_env_vars(&[(name, value)], action)
    }

    fn sample_intelligent_creature(slot_index: i32) -> ClientIntelligentCreature {
        ClientIntelligentCreature {
            pet_type: 1,
            icon: 44,
            custom_name: "Buddy".to_string(),
            fullness: 50,
            slot_index,
            expire_binary_datetime: 638000000000000000,
            blackstone_time: 12_000,
            pet_mode: 1,
            creature_rules: IntelligentCreatureRules {
                minimal_fullness: 1,
                mouse_pickup_enabled: true,
                mouse_pickup_range: 6,
                auto_pickup_enabled: false,
                auto_pickup_range: 0,
                semi_auto_pickup_enabled: true,
                semi_auto_pickup_range: 4,
                can_produce_blackstone: true,
            },
            filter: IntelligentCreatureItemFilter {
                pet_pickup_all: false,
                pet_pickup_gold: true,
                pet_pickup_weapons: false,
                pet_pickup_armours: false,
                pet_pickup_helmets: false,
                pet_pickup_boots: false,
                pet_pickup_belts: false,
                pet_pickup_accessories: false,
                pet_pickup_others: true,
            },
            pickup_grade: 2,
            maintain_food_time: 24_000,
        }
    }

    fn sample_hero_info(index: i32) -> ClientHeroInformation {
        ClientHeroInformation {
            index,
            name: format!("Hero{index}"),
            level: 12,
            class: MirClass::Taoist,
            gender: MirGender::Female,
        }
    }

    fn demo_game_session() -> crate::GatewaySession {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());
        let login_packets = session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        assert!(login_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let start_packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(start_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
        session
    }

    fn issue_test_identity() -> (
        crate::identity::IdentityService,
        crate::identity::VerifiedIdentitySession,
    ) {
        let identity = crate::identity::IdentityService::local_for_tests();
        let grant = identity
            .issue_session(
                "demo",
                "password",
                "demo",
                "127.0.0.1",
                "native-resume-test",
            )
            .expect("test identity should be issued");
        let verified = identity
            .verify_session_token(&grant.token)
            .expect("test identity should verify");
        (identity, verified)
    }

    fn store_native_resume_fixture(
        store: &super::ReconnectSessionStore,
        verified: &crate::identity::VerifiedIdentitySession,
        gateway_session_id_override: Option<&str>,
        key_override: Option<crate::GatewaySessionCacheKey>,
        ttl: Duration,
    ) -> (
        crate::resume::ResumeCredential,
        crate::resume::ResumeBinding,
    ) {
        let session = demo_game_session();
        let active_identity = session
            .active_identity()
            .expect("test game session should have an active identity");
        let key = key_override.unwrap_or_else(|| crate::GatewaySessionCacheKey {
            account_id: active_identity.account_id.clone(),
            character_index: active_identity.character_index,
        });
        let actual_gateway_session_id = session.session_id().to_string();
        let gateway_session_id =
            gateway_session_id_override.unwrap_or(actual_gateway_session_id.as_str());
        let nonce = crate::resume::ResumeConnectionNonce::generate();
        let now_ms = super::gateway_unix_ms();
        let issued = store
            .issue_resume_credential(
                None,
                crate::resume::ResumeIssueContext {
                    account_id: &active_identity.account_id,
                    character_index: active_identity.character_index,
                    gateway_session_id,
                    identity_session_id: &verified.session_id,
                    identity_expires_at_ms: verified.expires_at_ms,
                    source_connection_nonce: &nonce,
                },
                now_ms,
                1,
                || true,
            )
            .expect("test identity should be active while issuing the fixture credential");
        let binding = issued.binding.clone();
        let family_id = binding.family_id.clone();
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        let active_session_permit = capacity
            .try_acquire_active_session()
            .expect("test active session should fit capacity");
        let reconnect_lease_permit = capacity
            .try_acquire_reconnect_lease()
            .expect("test reconnect lease should fit capacity");
        store.store(
            key,
            session,
            Some(active_session_permit),
            reconnect_lease_permit,
            Some(family_id),
            ttl,
        );
        (issued.credential, binding)
    }

    #[tokio::test]
    async fn admin_system_mail_endpoint_writes_live_account_store() {
        let path = std::env::temp_dir().join(format!(
            "mir2-gateway-admin-mail-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        let default_character = config.default_character.clone();
        let state = super::WebState {
            config: Arc::new(config),
            deploy_revision: None,
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::new(super::ReconnectSessionStore::default()),
            capacity: Arc::new(super::GatewayCapacityState::unlimited()),
            gameplay_event_sink: None,
            identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator: crate::spectator::SpectatorHub::from_env(),
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(
                    std::env::temp_dir().join("mir2-ai-live-web-mail-test"),
                ),
            )
            .expect("test AI live hub"),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        };

        let Json(receipt) = super::admin_system_mail(
            State(state),
            Json(super::AdminSystemMailRequest {
                target_kind: Stage5MailTargetKind::Character,
                target_id: "Scout".into(),
                from: "GM System".into(),
                subject: "Endpoint smoke".into(),
                body: "Delivered through the live gateway admin endpoint.".into(),
                gold: 5000,
                items: vec!["red-potion".into()],
            }),
        )
        .await
        .expect("mail delivery should succeed");

        assert_eq!(receipt.delivered_count, 1);
        assert_eq!(receipt.mail_ids, vec![1]);

        let store = AccountStore::load_or_new(&path, default_character);
        let save = store
            .accounts
            .get("demo")
            .and_then(|account| account.saves.get(&0))
            .expect("demo character save should exist");
        let systems: Stage5SystemsState = serde_json::from_str(
            save.stage5_systems_json
                .as_deref()
                .expect("stage5 systems should be persisted"),
        )
        .expect("stage5 systems should decode");
        assert_eq!(systems.mail[0].subject, "Endpoint smoke");
        assert_eq!(systems.mail[0].gold, 5000);
        assert_eq!(systems.mail[0].items, vec!["red-potion"]);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn health_reports_cache_and_gameplay_event_boundaries() {
        let event_sink = Arc::new(crate::InMemoryGameplayEventSink::default());
        let shared_event_sink: crate::SharedGameplayEventSink = event_sink;
        let state = super::WebState {
            config: Arc::new(crate::GatewayConfig::default()),
            deploy_revision: None,
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::new(super::ReconnectSessionStore::default()),
            capacity: Arc::new(super::GatewayCapacityState::unlimited()),
            gameplay_event_sink: Some(shared_event_sink),
            identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator: crate::spectator::SpectatorHub::from_env(),
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(
                    std::env::temp_dir().join("mir2-ai-live-web-health-test"),
                ),
            )
            .expect("test AI live hub"),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        };

        let Json(response) = super::health(State(state)).await;

        assert!(response.ok);
        assert_eq!(response.revision, None);
        assert!(
            serde_json::to_value(&response)
                .expect("health response should serialize")
                .get("revision")
                .is_none(),
            "local health responses should remain compatible when no revision is configured"
        );
        assert_eq!(response.session_cache.backend, "in_memory");
        assert!(response.session_cache.healthy);
        assert_eq!(
            response.capacity,
            super::GatewayCapacityStatus {
                max_ws_connections: None,
                max_active_sessions: None,
                max_reconnect_leases: None,
                max_login_in_flight: None,
                max_new_character_in_flight: None,
                max_start_game_in_flight: None,
                current_ws_connections: 0,
                current_active_sessions: 0,
                current_reconnect_leases: 0,
                current_login_in_flight: 0,
                current_new_character_in_flight: 0,
                current_start_game_in_flight: 0,
            }
        );
        assert!(response.gameplay_events.configured);
        assert_eq!(
            response.gameplay_events.topic.as_deref(),
            Some(crate::events::DEFAULT_GAMEPLAY_EVENT_TOPIC)
        );
    }

    #[tokio::test]
    async fn admin_sessions_and_control_endpoints_are_queryable() {
        let cache = Arc::new(crate::InMemoryGatewaySessionCache::default());
        crate::GatewaySessionCache::put(
            cache.as_ref(),
            crate::GatewaySessionCacheRecord {
                key: crate::GatewaySessionCacheKey {
                    account_id: "demo".into(),
                    character_index: 0,
                },
                character_name: "Scout".into(),
                gateway_session_id: Some("gateway-test-1".into()),
                gateway_id: Some("gateway-test".into()),
                gateway_endpoint: Some("http://gateway.test".into()),
                relay_id: Some("relay-test".into()),
                relay_endpoint: Some("relay.test:443".into()),
                service_node_id: Some("owner-crystal".into()),
                node_kind: Some("official".into()),
                zone_id: Some("crystal".into()),
                zone_owner_id: Some("owner-crystal".into()),
                zone_owner_fencing_token: Some(1),
                map_file_name: Some("0".into()),
                player_object_id: Some(1001),
                player_hp: Some(18),
                player_max_hp: Some(18),
                gold: 50,
                tick: 7,
                updated_at_ms: 1_000,
                route_lease_owner: None,
                route_lease_expires_at_ms: None,
                handoff_generation: 0,
            },
        );
        let state = super::WebState {
            config: Arc::new(crate::GatewayConfig::default()),
            deploy_revision: None,
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: cache,
            reconnect_sessions: Arc::new(super::ReconnectSessionStore::default()),
            capacity: Arc::new(super::GatewayCapacityState::unlimited()),
            gameplay_event_sink: None,
            identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator: crate::spectator::SpectatorHub::from_env(),
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(
                    std::env::temp_dir().join("mir2-ai-live-web-admin-test"),
                ),
            )
            .expect("test AI live hub"),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        };

        let Json(sessions) = super::admin_sessions(State(state.clone())).await;
        assert_eq!(sessions.source, "gateway_session_cache");
        assert_eq!(sessions.sessions.len(), 1);
        assert_eq!(sessions.sessions[0].character_name, "Scout");

        let Json(trace) = super::admin_session_trace(
            State(state),
            axum::http::HeaderMap::new(),
            axum::extract::Query(super::AdminSessionTraceQuery {
                account_id: "demo".into(),
                character_index: 0,
                limit: Some(16),
            }),
        )
        .await
        .expect("session trace should be queryable in local development");
        assert_eq!(trace.source, "gateway_session_trace");
        assert_eq!(trace.status, "stale");
        assert_eq!(
            trace
                .current
                .as_ref()
                .and_then(|record| record.service_node_id.as_deref()),
            Some("owner-crystal")
        );
        assert!(trace
            .events
            .iter()
            .any(|event| event.event_type == "placement_assigned"));

        let Json(receipt) = super::admin_control(Json(super::AdminControlRequest {
            action: "reload-npcs".into(),
            target: Some("world".into()),
            operator_id: Some("op-1".into()),
            reason: Some("endpoint smoke".into()),
        }))
        .await
        .expect("reload should be accepted");
        assert_eq!(receipt.action, "reload_npcs");
        assert!(receipt.accepted);
        assert!(receipt.message.contains("operator=op-1"));

        let stop_error = super::admin_control(Json(super::AdminControlRequest {
            action: "stop".into(),
            target: None,
            operator_id: None,
            reason: None,
        }))
        .await
        .expect_err("dev gateway should not execute stop");
        assert_eq!(stop_error.0, axum::http::StatusCode::CONFLICT);
    }

    #[test]
    fn admin_session_trace_requires_configured_gateway_operator_token() {
        with_env_var(
            "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN",
            Some("gateway-trace-test-secret"),
            || {
                let missing =
                    super::require_gateway_admin_trace_token(&axum::http::HeaderMap::new())
                        .expect_err("missing token should be rejected");
                assert_eq!(missing.0, axum::http::StatusCode::UNAUTHORIZED);

                let mut headers = axum::http::HeaderMap::new();
                headers.insert(
                    axum::http::header::AUTHORIZATION,
                    "Bearer gateway-trace-test-secret"
                        .parse()
                        .expect("valid authorization header"),
                );
                super::require_gateway_admin_trace_token(&headers)
                    .expect("matching token should be accepted");
            },
        );
    }

    #[test]
    fn production_admin_operator_token_rejects_short_secrets() {
        with_env_var("MIR2_RUNTIME_ENV", Some("production"), || {
            let previous = std::env::var("MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN").ok();
            std::env::set_var("MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN", "too-short");
            let rejected = super::require_gateway_admin_trace_token(&axum::http::HeaderMap::new())
                .expect_err("production must fail closed for a short operator token");
            assert_eq!(rejected.0, axum::http::StatusCode::SERVICE_UNAVAILABLE);
            assert!(rejected.1 .0.error.contains("at least 32 characters"));
            match previous {
                Some(value) => std::env::set_var("MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN", value),
                None => std::env::remove_var("MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN"),
            }
        });
    }

    #[test]
    fn login_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"login","accountId":"demo","password":"demo"}"#,
        )
        .expect("login command should deserialize");

        match command {
            BrowserCommand::Login {
                account_id,
                password,
            } => {
                assert_eq!(account_id, "demo");
                assert_eq!(password, "demo");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn disconnect_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(r#"{"type":"disconnect"}"#)
            .expect("disconnect command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("disconnect command should map to a session action");

        assert!(matches!(
            action,
            SessionAction::Packet(ClientPacket::Disconnect)
        ));
    }

    #[test]
    fn keep_alive_command_maps_to_protocol_packet_without_snapshot() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"keepAlive","time":12345}"#)
                .expect("keep alive command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("keep alive command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::KeepAlive { time }) => assert_eq!(time, 12345),
            _ => panic!("unexpected action"),
        }

        assert!(!should_send_world_snapshot_for_action(
            &SessionAction::Packet(ClientPacket::KeepAlive { time: 12345 },)
        ));
        assert!(!super::should_queue_save_for_action(
            &SessionAction::Packet(ClientPacket::KeepAlive { time: 12345 },)
        ));
        assert!(
            super::inflight_capacity_kind_for_action(&SessionAction::Packet(
                ClientPacket::KeepAlive { time: 12345 },
            ))
            .is_none()
        );
        assert!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::KeepAlive { time: 12345 },
            ))
            .is_none()
        );
    }

    #[test]
    fn new_account_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"newAccount","accountId":"fresh","password":"demo","birthDateBinary":42,"userName":"Fresh User","secretQuestion":"Q","secretAnswer":"A","emailAddress":"fresh@example.test"}"#,
        )
        .expect("new account command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("new account command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::NewAccount {
                account_id,
                password,
                birth_date_binary,
                user_name,
                secret_question,
                secret_answer,
                email_address,
            }) => {
                assert_eq!(account_id, "fresh");
                assert_eq!(password, "demo");
                assert_eq!(birth_date_binary, 42);
                assert_eq!(user_name, "Fresh User");
                assert_eq!(secret_question, "Q");
                assert_eq!(secret_answer, "A");
                assert_eq!(email_address, "fresh@example.test");
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn change_password_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"changePassword","accountId":"demo","currentPassword":"old","newPassword":"new"}"#,
        )
        .expect("change password command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("change password command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::ChangePassword {
                account_id,
                current_password,
                new_password,
            }) => {
                assert_eq!(account_id, "demo");
                assert_eq!(current_password, "old");
                assert_eq!(new_password, "new");
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn storage_password_commands_map_to_protocol_packets() {
        let unlock = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"unlockStorage","password":"vault"}"#,
        )
        .expect("unlock storage command should deserialize");
        let set = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setStoragePassword","currentPassword":"vault","newPassword":"new-vault"}"#,
        )
        .expect("set storage password command should deserialize");
        let remove = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"removeStoragePassword","currentPassword":"new-vault"}"#,
        )
        .expect("remove storage password command should deserialize");

        match super::browser_command_to_action(unlock).expect("unlock should map") {
            SessionAction::Packet(ClientPacket::UnlockStorage { password }) => {
                assert_eq!(password, "vault")
            }
            _ => panic!("unexpected action"),
        }
        match super::browser_command_to_action(set).expect("set should map") {
            SessionAction::Packet(ClientPacket::SetStoragePassword {
                current_password,
                new_password,
            }) => {
                assert_eq!(current_password, "vault");
                assert_eq!(new_password, "new-vault");
            }
            _ => panic!("unexpected action"),
        }
        match super::browser_command_to_action(remove).expect("remove should map") {
            SessionAction::Packet(ClientPacket::RemoveStoragePassword { current_password }) => {
                assert_eq!(current_password, "new-vault")
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn storage_password_server_events_expose_crystal_payload_fields() {
        let unlock = super::server_packet_to_event(&ServerPacket::StorageUnlockResult {
            result: 0,
            has_password: true,
        });
        assert_eq!(unlock["packet"], "StorageUnlockResult");
        assert_eq!(unlock["payload"]["result"], 0);
        assert_eq!(unlock["payload"]["hasPassword"], true);

        let password = super::server_packet_to_event(&ServerPacket::StoragePasswordResult {
            result: 4,
            removing: true,
            has_password: false,
            last_set_binary_datetime: 638000000000000000,
        });
        assert_eq!(password["packet"], "StoragePasswordResult");
        assert_eq!(password["payload"]["result"], 4);
        assert_eq!(password["payload"]["removing"], true);
        assert_eq!(password["payload"]["hasPassword"], false);
        assert_eq!(
            password["payload"]["lastSetBinaryDatetime"],
            638000000000000000_i64
        );

        let user_storage = super::server_packet_to_event(&ServerPacket::UserStorage {
            storage: Some(vec![Some(sample_user_item(90, 2)), None]),
        });
        assert_eq!(user_storage["packet"], "UserStorage");
        assert_eq!(user_storage["payload"]["storage"][0]["unique_id"], 90);
        assert_eq!(user_storage["payload"]["storage"][0]["count"], 2);
        assert!(user_storage["payload"]["storage"][1].is_null());
    }

    #[test]
    fn raw_server_events_expose_copyable_payload_fields() {
        let raw = super::server_packet_to_event(&ServerPacket::Raw {
            packet_id: ServerPacketId::TimeOfDay,
            payload: vec![0x00, 0x11, 0x22, 0xaa],
        });
        assert_eq!(raw["packet"], "TimeOfDay");
        assert_eq!(
            raw["payload"]["packetId"],
            json!(ServerPacketId::TimeOfDay as i16)
        );
        assert_eq!(raw["payload"]["packetName"], "TimeOfDay");
        assert_eq!(raw["payload"]["payloadLength"], 4);
        assert_eq!(raw["payload"]["payloadHex"], "001122aa");
        assert_eq!(raw["payload"]["rawPayloadLength"], 4);
    }

    #[test]
    fn map_environment_packets_use_browser_facing_fields() {
        let map = super::server_packet_to_event(&ServerPacket::MapInformation {
            info: MapInformation {
                map_index: 7,
                file_name: "DogYoHyun".into(),
                title: "DogYoHyun".into(),
                mini_map: 3,
                big_map: 4,
                lights: 0,
                flags: 3,
                map_dark_light: 2,
                music: 11,
                weather_particles: 3,
            },
        });
        assert_eq!(map["payload"]["lights"], 0);
        assert_eq!(map["payload"]["mapDarkLight"], 2);
        assert_eq!(map["payload"]["weatherParticles"], 3);

        let changed = super::server_packet_to_event(&ServerPacket::MapChanged {
            map_index: 8,
            file_name: "D1801".into(),
            title: "PenalCavern".into(),
            mini_map: 0,
            big_map: 0,
            lights: 4,
            location: Point { x: 12, y: 34 },
            direction: MirDirection::Down,
            map_dark_light: 1,
            music: 0,
            weather: 64,
        });
        assert_eq!(changed["payload"]["lights"], 4);
        assert_eq!(changed["payload"]["mapDarkLight"], 1);
        assert_eq!(changed["payload"]["weatherParticles"], 64);
        assert_eq!(changed["payload"]["direction"], "Down");
    }

    #[test]
    fn newly_typed_server_events_expose_structured_payload_fields() {
        let map = super::server_packet_to_event(&ServerPacket::NewMapInfo {
            map_index: 77,
            info: ClientMapInfo {
                title: "CastleGi-Ryoong".into(),
                width: 120,
                height: 220,
                big_map: 121,
                movements: vec![],
                npcs: vec![],
            },
        });
        assert_eq!(map["packet"], "NewMapInfo");
        assert_eq!(map["payload"]["typed"], true);
        assert_eq!(map["payload"]["mapIndex"], 77);
        assert_eq!(map["payload"]["info"]["title"], "CastleGi-Ryoong");
        assert_eq!(map["payload"]["info"]["bigMap"], 121);
        assert!(!map["payload"].as_object().unwrap().contains_key("summary"));

        let rankings = super::server_packet_to_event(&ServerPacket::Rankings {
            rank_type: 2,
            my_rank: 5,
            listing_details: vec![RankCharacterInfo {
                player_id: 9001,
                name: "RankedHero".into(),
                level: 45,
                class: MirClass::Taoist,
            }],
            listings: vec![123_456],
            count: 1,
        });
        assert_eq!(rankings["packet"], "Rankings");
        assert_eq!(rankings["payload"]["typed"], true);
        assert_eq!(rankings["payload"]["rankType"], 2);
        assert_eq!(rankings["payload"]["myRank"], 5);
        assert_eq!(
            rankings["payload"]["listingDetails"][0]["name"],
            "RankedHero"
        );
        assert_eq!(rankings["payload"]["listings"][0], 123_456);

        let unit = super::server_packet_to_event(&ServerPacket::ReturnToLogin);
        assert_eq!(unit["packet"], "ReturnToLogin");
        assert_eq!(unit["payload"], json!({ "typed": true }));
    }

    #[test]
    fn resize_storage_server_event_exposes_crystal_payload_fields() {
        let resize = super::server_packet_to_event(&ServerPacket::ResizeStorage {
            size: 160,
            has_expanded_storage: true,
            expiry_time_binary_datetime: 638000000000000000,
        });
        assert_eq!(resize["packet"], "ResizeStorage");
        assert_eq!(resize["payload"]["size"], 160);
        assert_eq!(resize["payload"]["hasExpandedStorage"], true);
        assert_eq!(
            resize["payload"]["expiryTimeBinaryDatetime"],
            638000000000000000_i64
        );
    }

    #[test]
    fn credit_delta_server_events_expose_crystal_payload_fields() {
        let gained = super::server_packet_to_event(&ServerPacket::GainedCredit { credit: 45 });
        assert_eq!(gained["packet"], "GainedCredit");
        assert_eq!(gained["payload"]["credit"], 45);

        let lost = super::server_packet_to_event(&ServerPacket::LoseCredit { credit: 12 });
        assert_eq!(lost["packet"], "LoseCredit");
        assert_eq!(lost["payload"]["credit"], 12);
    }

    #[test]
    fn item_slot_and_seal_server_events_expose_crystal_payload_fields() {
        let slot = super::server_packet_to_event(&ServerPacket::ItemSlotSizeChanged {
            unique_id: 42,
            slot_size: 3,
        });
        assert_eq!(slot["packet"], "ItemSlotSizeChanged");
        assert_eq!(slot["payload"]["uniqueId"], 42);
        assert_eq!(slot["payload"]["slotSize"], 3);

        let seal = super::server_packet_to_event(&ServerPacket::ItemSealChanged {
            unique_id: 43,
            expiry_date_binary_datetime: 638000000000000000,
        });
        assert_eq!(seal["packet"], "ItemSealChanged");
        assert_eq!(seal["payload"]["uniqueId"], 43);
        assert_eq!(
            seal["payload"]["expiryDateBinaryDatetime"],
            638000000000000000_i64
        );

        let upgraded = super::server_packet_to_event(&ServerPacket::ItemUpgraded {
            item: sample_user_item(44, 1),
        });
        assert_eq!(upgraded["packet"], "ItemUpgraded");
        assert_eq!(upgraded["payload"]["item"]["unique_id"], 44);
    }

    #[test]
    fn combine_item_server_event_exposes_crystal_payload_fields() {
        let packet = super::server_packet_to_event(&ServerPacket::CombineItem {
            grid: MirGridType::Inventory,
            id_from: 31,
            id_to: 32,
            success: true,
            destroy: false,
        });
        assert_eq!(packet["packet"], "CombineItem");
        assert_eq!(packet["payload"]["grid"], "Inventory");
        assert_eq!(packet["payload"]["idFrom"], 31);
        assert_eq!(packet["payload"]["idTo"], 32);
        assert_eq!(packet["payload"]["success"], true);
        assert_eq!(packet["payload"]["destroy"], false);
    }

    #[test]
    fn npc_service_server_events_expose_crystal_payload_fields() {
        let mut hp_drug = sample_user_item(43_122_689, 1);
        hp_drug.item_index = 658;
        let goods = super::server_packet_to_event(&ServerPacket::NPCGoods {
            list: vec![hp_drug],
            rate: 1.25,
            panel_type: 3,
            hide_added_stats: true,
        });
        assert_eq!(goods["packet"], "NPCGoods");
        assert_eq!(goods["payload"]["rate"], 1.25);
        assert_eq!(goods["payload"]["panelType"], 3);
        assert_eq!(goods["payload"]["hideAddedStats"], true);
        assert_eq!(goods["payload"]["list"][0]["id"], 43_122_689_u64);
        assert_eq!(goods["payload"]["list"][0]["itemIndex"], 658);
        assert_eq!(goods["payload"]["list"][0]["name"], "(HP)DrugSmall");
        assert_eq!(goods["payload"]["list"][0]["icon"], 398);
        assert_eq!(goods["payload"]["list"][0]["price"], 50);
        assert_eq!(goods["payload"]["list"][0]["item_index"], 658);

        let repair = super::server_packet_to_event(&ServerPacket::NPCRepair { rate: 1.5 });
        assert_eq!(repair["packet"], "NPCRepair");
        assert_eq!(repair["payload"]["rate"], 1.5);

        let refine = super::server_packet_to_event(&ServerPacket::NPCRefine {
            rate: 2.5,
            refining: true,
        });
        assert_eq!(refine["packet"], "NPCRefine");
        assert_eq!(refine["payload"]["rate"], 2.5);
        assert_eq!(refine["payload"]["refining"], true);

        let craft = super::server_packet_to_event(&ServerPacket::CraftItem { success: false });
        assert_eq!(craft["packet"], "CraftItem");
        assert_eq!(craft["payload"]["success"], false);
    }

    #[test]
    fn start_game_command_accepts_camel_case_fields() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"startGame","characterIndex":0}"#)
                .expect("start game command should deserialize");

        match command {
            BrowserCommand::StartGame { character_index } => {
                assert_eq!(character_index, 0);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn pickup_command_accepts_camel_case_fields() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"pickUp","objectId":5000}"#)
                .expect("pickup command should deserialize");

        match command {
            BrowserCommand::PickUp { object_id } => {
                assert_eq!(object_id, 5000);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn use_item_command_accepts_key_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"useItem","key":"red-potion"}"#)
                .expect("use item command should deserialize");

        match command {
            BrowserCommand::UseItem { key, .. } => assert_eq!(key.as_deref(), Some("red-potion")),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn use_item_with_unique_id_maps_to_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"useItem","uniqueId":1024,"grid":"equipment"}"#,
        )
        .expect("use item command should deserialize");
        match super::browser_command_to_action(command)
            .expect("use item command should map to a session action")
        {
            SessionAction::Packet(ClientPacket::UseItem { unique_id, grid }) => {
                assert_eq!(unique_id, 1024);
                assert_eq!(grid, MirGridType::Equipment);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn drop_item_command_accepts_key_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"dropItem","key":"red-potion"}"#)
                .expect("drop item command should deserialize");

        match command {
            BrowserCommand::DropItem { key, .. } => assert_eq!(key, "red-potion"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn delete_character_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"deleteCharacter","characterIndex":1}"#,
        )
        .expect("delete character command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("delete character command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::DeleteCharacter { character_index }) => {
                assert_eq!(character_index, 1)
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn equip_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"equipItem","uniqueId":3,"grid":"inventory","to":2}"#,
        )
        .expect("equip item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("equip item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::EquipItem {
                unique_id,
                grid,
                to,
            }) => {
                assert_eq!(unique_id, 3);
                assert_eq!(grid, mir2_protocol::MirGridType::Inventory);
                assert_eq!(to, 2);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn split_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"splitItem","uniqueId":0,"grid":"inventory","count":2}"#,
        )
        .expect("split item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("split item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::SplitItem {
                unique_id,
                grid,
                count,
            }) => {
                assert_eq!(unique_id, 0);
                assert_eq!(grid, mir2_protocol::MirGridType::Inventory);
                assert_eq!(count, 2);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn remove_slot_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"removeSlotItem","uniqueId":2,"grid":"equipment","gridTo":"inventory","to":5,"fromUniqueId":9}"#,
        )
        .expect("remove slot item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("remove slot item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::RemoveSlotItem {
                unique_id,
                grid,
                grid_to,
                to,
                from_unique_id,
            }) => {
                assert_eq!(unique_id, 2);
                assert_eq!(grid, mir2_protocol::MirGridType::Equipment);
                assert_eq!(grid_to, mir2_protocol::MirGridType::Inventory);
                assert_eq!(to, 5);
                assert_eq!(from_unique_id, 9);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn merge_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"mergeItem","gridFrom":"inventory","gridTo":"inventory","idFrom":1,"idTo":4}"#,
        )
        .expect("merge item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("merge item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
            }) => {
                assert_eq!(grid_from, mir2_protocol::MirGridType::Inventory);
                assert_eq!(grid_to, mir2_protocol::MirGridType::Inventory);
                assert_eq!(id_from, 1);
                assert_eq!(id_to, 4);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn drop_gold_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(r#"{"type":"dropGold","amount":88}"#)
            .expect("drop gold command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("drop gold command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::DropGold { amount }) => assert_eq!(amount, 88),
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn big_map_browser_commands_map_exactly_to_protocol_packets() {
        let request =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"requestMapInfo","mapIndex":34}"#)
                .expect("requestMapInfo should deserialize");
        assert!(matches!(
            super::browser_command_to_action(request),
            Ok(SessionAction::Packet(ClientPacket::RequestMapInfo {
                map_index: 34
            }))
        ));

        let search = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"searchMap","text":"  Natural   Cave  "}"#,
        )
        .expect("searchMap should deserialize");
        assert!(matches!(
            super::browser_command_to_action(search),
            Ok(SessionAction::Packet(ClientPacket::SearchMap { text }))
                if text == "Natural Cave"
        ));

        let teleport =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"teleportToNpc","objectId":77}"#)
                .expect("teleportToNpc should deserialize");
        assert!(matches!(
            super::browser_command_to_action(teleport),
            Ok(SessionAction::Packet(ClientPacket::TeleportToNpc {
                object_id: 77
            }))
        ));
    }

    #[test]
    fn big_map_browser_commands_reject_invalid_or_malformed_inputs() {
        assert!(serde_json::from_str::<BrowserCommand>(r#"{"type":"requestMapInfo"}"#).is_err());
        assert!(serde_json::from_str::<BrowserCommand>(
            r#"{"type":"teleportToNpc","objectId":"not-a-number"}"#,
        )
        .is_err());

        let invalid_map =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"requestMapInfo","mapIndex":0}"#)
                .unwrap();
        assert!(super::browser_command_to_action(invalid_map).is_err());

        for text in [String::new(), "ab".to_string(), "x".repeat(65)] {
            let search = BrowserCommand::SearchMap { text };
            assert!(super::browser_command_to_action(search).is_err());
        }

        let invalid_teleport = BrowserCommand::TeleportToNpc { object_id: 0 };
        assert!(super::browser_command_to_action(invalid_teleport).is_err());
    }

    #[test]
    fn request_item_info_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"requestItemInfo","itemIndex":658}"#)
                .expect("request item info command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("request item info command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::RequestItemInfo { item_index }) => {
                assert_eq!(item_index, 658);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn delete_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"deleteItem","uniqueId":2,"count":3,"heroInventory":false}"#,
        )
        .expect("delete item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("delete item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::DeleteItem {
                unique_id,
                count,
                hero_inventory,
            }) => {
                assert_eq!(unique_id, 2);
                assert_eq!(count, 3);
                assert!(!hero_inventory);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn sell_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"sellItem","uniqueId":2,"count":1}"#)
                .expect("sell item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("sell item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::SellItem { unique_id, count }) => {
                assert_eq!(unique_id, 2);
                assert_eq!(count, 1);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn buy_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"buyItem","itemIndex":43122688,"count":5,"panelType":0}"#,
        )
        .expect("buy item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("buy item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::BuyItem {
                item_index,
                count,
                panel_type,
            }) => {
                assert_eq!(item_index, 43_122_688);
                assert_eq!(count, 5);
                assert_eq!(panel_type, 0);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn game_shop_buy_command_maps_exactly_to_dedicated_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"gameShopBuy","gIndex":31,"quantity":2,"priceType":1}"#,
        )
        .expect("gameShopBuy should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("gameShopBuy should map to a session action");

        assert!(matches!(
            action,
            SessionAction::GameShopBuy {
                request_id: None,
                g_index: 31,
                quantity: 2,
                price_type: 1,
            }
        ));
        assert!(serde_json::from_str::<BrowserCommand>(
            r#"{"type":"gameShopBuy","gIndex":31,"priceType":1}"#
        )
        .is_err());
    }

    #[test]
    fn native_game_shop_capability_and_request_id_are_independent_and_strict() {
        let both = super::validate_native_client_capabilities(&[
            crate::resume::NATIVE_RESUME_PROTOCOL.to_string(),
            super::NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_string(),
        ])
        .expect("both native capabilities should validate");
        assert!(both.native_resume_v1);
        assert!(both.native_game_shop_receipt_v1);

        let shop_only = super::validate_native_client_capabilities(&[
            super::NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_string(),
        ])
        .expect("GameShop capability should opt in independently");
        assert!(!shop_only.native_resume_v1);
        assert!(shop_only.native_game_shop_receipt_v1);
        assert!(super::validate_native_client_capabilities(&["bad\u{7f}".to_string()]).is_err());

        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"gameShopBuy","requestId":"gs-0001","gIndex":31,"quantity":2,"priceType":1}"#,
        )
        .expect("native buy should deserialize");
        let action = super::browser_command_to_action(command).expect("native buy should map");
        let request = super::native_game_shop_request_from_action(&action)
            .expect("GameShop action should be recognized")
            .expect("requestId should validate");
        assert_eq!(request.request_id, "gs-0001");
        assert_eq!(
            (request.g_index, request.quantity, request.price_type),
            (31, 2, 1)
        );
    }

    #[test]
    fn native_game_shop_pending_rejects_second_request_without_replacing_first() {
        let first = super::NativeGameShopRequest {
            request_id: "gs-first".to_string(),
            server_idempotency_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            g_index: 31,
            quantity: 1,
            price_type: 1,
        };
        let second = super::NativeGameShopRequest {
            request_id: "gs-second".to_string(),
            server_idempotency_key: "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB".to_string(),
            g_index: 32,
            quantity: 2,
            price_type: 0,
        };
        let mut state = super::NativeGameShopConnectionState {
            opted_in: true,
            pending: None,
        };
        state
            .reserve(first.clone())
            .expect("first request should reserve");
        let receipt = state
            .reserve(second.clone())
            .expect_err("second request must fail before Simulation");
        assert_eq!(receipt["requestId"], second.request_id);
        assert_eq!(receipt["code"], "requestInFlight");
        assert_eq!(state.pending.as_ref(), Some(&first));
        assert!(state.clear_exact(&first));
        assert!(state.pending.is_none());
    }

    #[test]
    fn native_game_shop_receipt_is_exact_and_maps_stable_failure_codes() {
        let request = super::NativeGameShopRequest {
            request_id: "gs-exact".to_string(),
            server_idempotency_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            g_index: 31,
            quantity: 1,
            price_type: 1,
        };
        let success = mir2_simulation::GameShopPurchaseOutcome {
            success: true,
            g_index: 31,
            quantity: 1,
            price_type: 1,
            new_stock_level: None,
            mail_id: Some(77),
            failure: None,
        };
        let receipt = super::native_game_shop_receipt_event(&request, &success)
            .expect("exact success should serialize");
        assert_eq!(receipt["success"], true);
        assert_eq!(receipt["mailId"], 77);
        assert!(receipt.get("code").is_none());

        let wrong = mir2_simulation::GameShopPurchaseOutcome {
            g_index: 32,
            ..success.clone()
        };
        assert!(super::native_game_shop_receipt_event(&request, &wrong).is_err());
        let invalid_success = mir2_simulation::GameShopPurchaseOutcome {
            mail_id: None,
            ..success.clone()
        };
        let invalid_failure_stock = mir2_simulation::GameShopPurchaseOutcome {
            success: false,
            new_stock_level: Some(7),
            mail_id: None,
            failure: Some(mir2_simulation::GameShopPurchaseFailure::InsufficientCurrency),
            ..success.clone()
        };
        let ambiguous_commit = mir2_simulation::GameShopPurchaseOutcome {
            success: false,
            new_stock_level: None,
            mail_id: None,
            failure: Some(mir2_simulation::GameShopPurchaseFailure::CommitFailed),
            ..success.clone()
        };
        let unknown_actions = [
            super::native_game_shop_post_execution(&request, Some(&wrong)),
            super::native_game_shop_post_execution(&request, None),
            super::native_game_shop_post_execution(&request, Some(&invalid_success)),
            super::native_game_shop_post_execution(&request, Some(&invalid_failure_stock)),
            super::native_game_shop_post_execution(&request, Some(&ambiguous_commit)),
        ];
        assert!(unknown_actions.iter().all(|action| matches!(
            action,
            super::NativeGameShopPostExecution::CloseUnknown { .. }
        )));
        assert_eq!(
            unknown_actions
                .iter()
                .filter(|action| matches!(
                    action,
                    super::NativeGameShopPostExecution::SendReceipt(_)
                ))
                .count(),
            0,
            "post-execution mismatch/None/invalid must emit zero receipt"
        );
        assert!(matches!(
            super::native_game_shop_post_execution(&request, Some(&success)),
            super::NativeGameShopPostExecution::SendReceipt(_)
        ));

        for (failure, code) in [
            (
                mir2_simulation::GameShopPurchaseFailure::NotInGame,
                "notInGame",
            ),
            (
                mir2_simulation::GameShopPurchaseFailure::InvalidPriceType,
                "invalidRequest",
            ),
            (
                mir2_simulation::GameShopPurchaseFailure::InvalidQuantity,
                "invalidQuantity",
            ),
            (
                mir2_simulation::GameShopPurchaseFailure::UnknownProduct,
                "unknownProduct",
            ),
            (
                mir2_simulation::GameShopPurchaseFailure::ClassUnavailable,
                "classUnavailable",
            ),
            (
                mir2_simulation::GameShopPurchaseFailure::PaymentUnavailable,
                "paymentUnavailable",
            ),
            (
                mir2_simulation::GameShopPurchaseFailure::StockUnavailable,
                "stockUnavailable",
            ),
            (
                mir2_simulation::GameShopPurchaseFailure::InsufficientCurrency,
                "insufficientCurrency",
            ),
            (
                mir2_simulation::GameShopPurchaseFailure::MailFull,
                "mailFull",
            ),
            (
                mir2_simulation::GameShopPurchaseFailure::CommitFailed,
                "commitFailed",
            ),
        ] {
            assert_eq!(super::native_game_shop_failure_code(failure), code);
        }
    }

    #[test]
    fn native_game_shop_pre_execution_failures_are_definite_and_exact() {
        let request = super::NativeGameShopRequest {
            request_id: "gs-preflight".to_string(),
            server_idempotency_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            g_index: 31,
            quantity: 1,
            price_type: 1,
        };
        for (authenticated, in_game) in [(false, false), (false, true), (true, false)] {
            let receipt = super::native_game_shop_pre_execution_failure(
                &request,
                authenticated,
                in_game,
                false,
            )
            .expect("unauthenticated/not-in-game must fail before execution");
            assert_eq!(receipt["success"], false);
            assert_eq!(receipt["code"], "notInGame");
            assert_eq!(receipt["requestId"], request.request_id);
        }
        let unsupported =
            super::native_game_shop_pre_execution_failure(&request, true, true, false)
                .expect("unsupported typed execution must fail before purchase");
        assert_eq!(unsupported["code"], "commitFailed");
        assert!(
            super::native_game_shop_pre_execution_failure(&request, true, true, true).is_none()
        );
    }

    #[test]
    fn native_game_shop_pre_execution_send_failure_keeps_pending_unknown() {
        let request = super::NativeGameShopRequest {
            request_id: "gs-preflight-send-failure".to_string(),
            server_idempotency_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            g_index: 31,
            quantity: 1,
            price_type: 1,
        };
        let mut state = super::NativeGameShopConnectionState {
            opted_in: true,
            pending: None,
        };
        state.reserve(request.clone()).unwrap();

        assert_eq!(
            super::finish_native_game_shop_pre_execution_receipt(&mut state, &request, false,),
            super::NativeGameShopPreExecutionReceiptDisposition::CloseUnknown
        );
        assert_eq!(state.pending.as_ref(), Some(&request));
        let replay = state
            .reserve(request.clone())
            .expect_err("failed receipt delivery must not enable automatic replay");
        assert_eq!(replay["code"], "requestInFlight");

        assert_eq!(
            super::finish_native_game_shop_pre_execution_receipt(&mut state, &request, true,),
            super::NativeGameShopPreExecutionReceiptDisposition::Continue
        );
        assert!(state.pending.is_none());
    }

    fn started_native_game_shop_failure_session(
        account_id: &str,
        character_name: &str,
        gold: u32,
        credit: u32,
        stage5_systems: Option<Stage5SystemsState>,
    ) -> crate::GatewaySession {
        let config = SimulationConfig::default();
        let account_store = Arc::clone(&config.account_store);
        let mut session = crate::GatewaySession::new(config);
        let password = "native-shop-failure-password";

        let create = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::NewAccount {
                account_id: account_id.to_string(),
                password: password.to_string(),
                birth_date_binary: 0,
                user_name: "Native Shop Failure".to_string(),
                secret_question: "q".to_string(),
                secret_answer: "a".to_string(),
                email_address: "native-shop-failure@example.test".to_string(),
            }),
            false,
            true,
        )
        .expect("failure fixture NewAccount");
        assert!(create
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })));
        super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::Login {
                account_id: account_id.to_string(),
                password: password.to_string(),
            }),
            false,
            true,
        )
        .expect("failure fixture Login");
        let create_character = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::NewCharacter {
                name: character_name.to_string(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            }),
            true,
            true,
        )
        .expect("failure fixture NewCharacter");
        let character_index = create_character
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("failure fixture character index");
        {
            let mut store = account_store.lock().unwrap();
            let save = store
                .accounts
                .get_mut(account_id)
                .unwrap()
                .saves
                .get_mut(&character_index)
                .unwrap();
            save.gold = gold;
            save.credit = credit;
            save.stage5_systems_json =
                stage5_systems.map(|systems| serde_json::to_string(&systems).unwrap());
        }
        let start = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::StartGame { character_index }),
            true,
            true,
        )
        .expect("failure fixture StartGame");
        assert!(start
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
        session
    }

    fn assert_native_game_shop_failure_without_mutation(
        session: &mut crate::GatewaySession,
        request: super::NativeGameShopRequest,
        expected_code: &str,
    ) {
        let before = session.world_snapshot();
        let dispatch = super::execute_native_game_shop_handler_seam(session, &request)
            .expect("typed failure must return an authoritative outcome");
        let receipt = match dispatch.post_execution {
            super::NativeGameShopPostExecution::SendReceipt(receipt) => receipt,
            super::NativeGameShopPostExecution::CloseUnknown { reason } => {
                panic!("deterministic failure unexpectedly became unknown: {reason}")
            }
        };
        assert_eq!(receipt["success"], false);
        assert_eq!(receipt["code"], expected_code);
        assert_eq!(receipt["requestId"], request.request_id);
        assert!(receipt.get("mailId").is_none());
        assert!(!dispatch.normal_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::LoseGold { .. } | ServerPacket::LoseCredit { .. }
        )));
        let after = session.world_snapshot();
        assert_eq!(after.gold, before.gold);
        assert_eq!(after.credit, before.credit);
        assert_eq!(after.inventory_items, before.inventory_items);
        assert_eq!(
            after
                .stage5_systems
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .collect::<Vec<_>>(),
            before
                .stage5_systems
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .collect::<Vec<_>>(),
            "a durable hidden outcome ledger is allowed, but player-visible mail is not"
        );
        assert_eq!(
            after.stage5_systems.game_shop_individual_purchases,
            before.stage5_systems.game_shop_individual_purchases
        );
    }

    #[test]
    fn native_game_shop_handler_failures_are_exact_and_atomic() {
        let mut invalid = started_native_game_shop_failure_session(
            "native_shop_invalid",
            "ShopInvalid",
            1_000_000,
            10_000,
            None,
        );
        for (key_seed, request_id, g_index, quantity, price_type, code) in [
            (1, "gs-invalid-quantity", 31, 0, 1, "invalidQuantity"),
            (2, "gs-invalid-price", 31, 1, 77, "invalidRequest"),
            (3, "gs-invalid-product", i32::MAX, 1, 1, "unknownProduct"),
        ] {
            assert_native_game_shop_failure_without_mutation(
                &mut invalid,
                super::NativeGameShopRequest {
                    request_id: request_id.to_string(),
                    server_idempotency_key: URL_SAFE_NO_PAD.encode([key_seed; 32]),
                    g_index,
                    quantity,
                    price_type,
                },
                code,
            );
        }

        let mut insufficient =
            started_native_game_shop_failure_session("native_shop_poor", "ShopPoor", 0, 0, None);
        assert_native_game_shop_failure_without_mutation(
            &mut insufficient,
            super::NativeGameShopRequest {
                request_id: "gs-insufficient".to_string(),
                server_idempotency_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
                g_index: 31,
                quantity: 1,
                price_type: 1,
            },
            "insufficientCurrency",
        );

        let mut full_systems = Stage5SystemsState::default();
        full_systems.mail = (1..=100)
            .map(|id| Stage5MailMessage {
                id,
                delivery_nonce: format!("native-shop-full-{id}"),
                from: "System".to_string(),
                to: "ShopFull".to_string(),
                subject: format!("mail-{id}"),
                body: String::new(),
                gold: 0,
                items: Vec::new(),
                item_states_json: Vec::new(),
                opened: false,
                locked: false,
                claimed: false,
                deleted: false,
            })
            .collect();
        let mut mail_full = started_native_game_shop_failure_session(
            "native_shop_full",
            "ShopFull",
            1_000_000,
            10_000,
            Some(full_systems),
        );
        assert_native_game_shop_failure_without_mutation(
            &mut mail_full,
            super::NativeGameShopRequest {
                request_id: "gs-mail-full".to_string(),
                server_idempotency_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
                g_index: 31,
                quantity: 1,
                price_type: 1,
            },
            "mailFull",
        );
    }

    #[test]
    fn non_opted_in_game_shop_keeps_the_ordinary_packet_path_without_receipt() {
        let mut session = started_native_game_shop_failure_session(
            "ordinary_shop_web",
            "OrdinaryShop",
            200_000,
            0,
            None,
        );
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"gameShopBuy","gIndex":31,"quantity":1,"priceType":1}"#,
        )
        .expect("legacy Web GameShop command should remain valid without requestId");
        let action = super::browser_command_to_action(command).unwrap();
        assert!(matches!(
            super::native_game_shop_request_from_action(&action),
            Some(Err(_))
        ));
        let packets = super::execute_session_action(&mut session, action, true, true)
            .expect("non-opt-in GameShop must use the ordinary production path");
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 165_000 })));
        assert!(packets
            .iter()
            .map(super::server_packet_to_event)
            .all(|event| event["type"] != "gameShopReceipt"));
        assert_eq!(session.world_snapshot().gold, 35_000);
    }

    #[test]
    fn native_game_shop_ordinary_buy_mail_collect_and_reload_is_exactly_once() {
        let account_id = "receipt_account";
        let password = "receipt_password";
        let character_name = "ReceiptHero";
        let config = SimulationConfig::default();
        let account_store = Arc::clone(&config.account_store);
        let mut session = crate::GatewaySession::new(config.clone());

        let create = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::NewAccount {
                account_id: account_id.to_string(),
                password: password.to_string(),
                birth_date_binary: 0,
                user_name: "Receipt Test".to_string(),
                secret_question: "q".to_string(),
                secret_answer: "a".to_string(),
                email_address: "receipt@example.test".to_string(),
            }),
            false,
            true,
        )
        .expect("ordinary NewAccount should execute");
        assert!(create
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })));

        let login = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::Login {
                account_id: account_id.to_string(),
                password: password.to_string(),
            }),
            false,
            true,
        )
        .expect("ordinary Login should execute");
        assert!(login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));

        let create_character = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::NewCharacter {
                name: character_name.to_string(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            }),
            true,
            true,
        )
        .expect("ordinary NewCharacter should execute");
        let character_index = create_character
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("new character should return its durable index");

        {
            let mut store = account_store
                .lock()
                .expect("account store should not be poisoned");
            store
                .accounts
                .get_mut(account_id)
                .expect("new account should exist")
                .saves
                .get_mut(&character_index)
                .expect("new character save should exist")
                .gold = 200_000;
        }
        let start = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::StartGame { character_index }),
            true,
            true,
        )
        .expect("ordinary StartGame should execute");
        assert!(start
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
        assert!(session.supports_typed_game_shop_purchase_outcome());

        let capabilities = super::validate_native_client_capabilities(&[
            super::NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_string(),
        ])
        .expect("native capability should validate");
        let mut native_state = super::NativeGameShopConnectionState {
            opted_in: capabilities.native_game_shop_receipt_v1,
            pending: None,
        };
        let browser_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"gameShopBuy","requestId":"gs-e2e-0001","gIndex":31,"quantity":1,"priceType":1}"#,
        )
        .expect("ordinary native BrowserCommand should deserialize");
        let action = super::browser_command_to_action(browser_command)
            .expect("ordinary native BrowserCommand should map");
        let request = super::native_game_shop_request_from_action(&action)
            .expect("GameShop action")
            .expect("valid requestId");
        native_state
            .reserve(request.clone())
            .expect("one in-flight request should reserve");
        let dispatch = super::execute_native_game_shop_handler_seam(&mut session, &request)
            .expect("ordinary authenticated GameShopBuy should execute once");
        let receipt = match dispatch.post_execution {
            super::NativeGameShopPostExecution::SendReceipt(receipt) => receipt,
            super::NativeGameShopPostExecution::CloseUnknown { reason } => {
                panic!("typed purchase unexpectedly became unknown: {reason}")
            }
        };
        assert_eq!(receipt["success"], true);
        assert_eq!(receipt["requestId"], request.request_id);
        assert_eq!(receipt["gIndex"], 31);
        assert_eq!(receipt["quantity"], 1);
        assert_eq!(receipt["priceType"], 1);
        let mail_id = receipt["mailId"].as_u64().expect("receipt mailId");
        assert!(dispatch
            .normal_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 165_000 })));
        let mut ordered_wire_events = dispatch
            .normal_packets
            .iter()
            .map(super::server_packet_to_event)
            .collect::<Vec<_>>();
        assert!(ordered_wire_events
            .iter()
            .all(|event| event["type"] != "gameShopReceipt"));
        ordered_wire_events.push(receipt.clone());
        assert_eq!(
            ordered_wire_events
                .last()
                .and_then(|event| event["type"].as_str()),
            Some("gameShopReceipt")
        );
        assert!(native_state.clear_exact(&request));
        assert_eq!(session.world_snapshot().gold, 35_000);

        native_state
            .reserve(request.clone())
            .expect("an internal retry reuses the exact server request");
        let duplicate_dispatch =
            super::execute_native_game_shop_handler_seam(&mut session, &request)
                .expect("durable duplicate should return its original typed outcome");
        let duplicate_receipt = match duplicate_dispatch.post_execution {
            super::NativeGameShopPostExecution::SendReceipt(receipt) => receipt,
            super::NativeGameShopPostExecution::CloseUnknown { reason } => {
                panic!("durable duplicate unexpectedly became unknown: {reason}")
            }
        };
        assert_eq!(duplicate_receipt, receipt);
        assert!(!duplicate_dispatch.normal_packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::LoseGold { .. } | ServerPacket::LoseCredit { .. }
            )
        }));
        assert!(native_state.clear_exact(&request));
        assert_eq!(session.world_snapshot().gold, 35_000);
        assert_eq!(
            session
                .world_snapshot()
                .stage5_systems
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .count(),
            1,
            "duplicate server key must not create a second purchase mail"
        );

        let (expected_item_key, attachment_unique_id, expected_quantity) = {
            let store = account_store
                .lock()
                .expect("account store should not be poisoned");
            let save = &store.accounts[account_id].saves[&character_index];
            assert_eq!(save.gold, 35_000);
            let systems: Stage5SystemsState = serde_json::from_str(
                save.stage5_systems_json
                    .as_deref()
                    .expect("purchase should persist mailbox"),
            )
            .expect("mailbox should decode");
            let mail = systems
                .mail
                .iter()
                .find(|mail| u64::from(mail.id) == mail_id)
                .expect("receipt mailId must address the committed mail");
            assert!(!mail.claimed);
            assert_eq!(mail.item_states_json.len(), 1);
            let item: serde_json::Value = serde_json::from_str(&mail.item_states_json[0])
                .expect("mail attachment state should decode");
            (
                item["key"].as_str().expect("attachment key").to_string(),
                item["unique_id"].as_u64().expect("attachment unique_id"),
                item["quantity"].as_u64().expect("attachment quantity") as u32,
            )
        };
        assert_eq!(expected_item_key, "crystal-item-1288");
        assert_eq!(expected_quantity, 5);

        let collect = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::CollectParcel { mail_id }),
            true,
            true,
        )
        .expect("ordinary CollectParcel should execute");
        assert!(!collect.is_empty());
        let snapshot = session.world_snapshot();
        let delivered_items = snapshot
            .inventory_items
            .iter()
            .filter(|item| item.key == expected_item_key && item.quantity == expected_quantity)
            .collect::<Vec<_>>();
        assert_eq!(delivered_items.len(), 1);
        let delivered_unique_id = delivered_items[0].unique_id;
        assert_ne!(
            delivered_unique_id, attachment_unique_id,
            "fresh GameShop grants must not preserve attachment-provided identities"
        );
        {
            let store = account_store
                .lock()
                .expect("account store should not be poisoned");
            let save = &store.accounts[account_id].saves[&character_index];
            let systems: Stage5SystemsState =
                serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
            assert!(
                systems
                    .mail
                    .iter()
                    .find(|mail| u64::from(mail.id) == mail_id)
                    .expect("mail should remain addressable")
                    .claimed
            );
        }
        drop(session);

        let mut reloaded = crate::GatewaySession::new(config);
        reloaded.handle_packet(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: password.to_string(),
        });
        reloaded.handle_packet(ClientPacket::StartGame { character_index });
        let before_retry = reloaded.world_snapshot();
        let retry = reloaded.handle_packet(ClientPacket::CollectParcel { mail_id });
        let after_retry = reloaded.world_snapshot();
        assert_eq!(before_retry.inventory_items, after_retry.inventory_items);
        assert_eq!(
            after_retry
                .inventory_items
                .iter()
                .filter(|item| item.unique_id == delivered_unique_id)
                .count(),
            1
        );
        assert!(!retry.iter().any(|packet| matches!(
            packet,
            ServerPacket::ReceiveMail { mail, .. }
                if mail.iter().any(|entry| entry.mail_id == mail_id && !entry.collected)
        )));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_authenticated_websocket_game_shop_receipt_mail_collect_is_e2e() {
        let config = SimulationConfig::default();
        let account_store = Arc::clone(&config.account_store);
        let ai_data_dir = std::env::temp_dir().join(format!(
            "mir2-native-game-shop-ws-e2e-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let state = super::WebState {
            config: Arc::new(config),
            deploy_revision: None,
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::new(super::ReconnectSessionStore::default()),
            capacity: Arc::new(super::GatewayCapacityState::unlimited()),
            gameplay_event_sink: None,
            identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator: crate::spectator::SpectatorHub::from_env(),
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(ai_data_dir),
            )
            .expect("test AI live hub"),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        };
        let app = axum::Router::new()
            .route("/ws", axum::routing::get(super::ws_upgrade))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind isolated WebSocket gateway");
        let address = listener.local_addr().expect("isolated listener address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .expect("isolated WebSocket gateway should serve");
        });

        let mut request = format!("ws://{address}/ws")
            .into_client_request()
            .expect("valid isolated WebSocket URL");
        let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
            .ok()
            .and_then(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .find(|entry| !entry.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("http://{address}"));
        request.headers_mut().insert(
            "origin",
            origin.parse().expect("valid test WebSocket Origin"),
        );
        let (mut socket, response) = tokio_tungstenite::connect_async(request)
            .await
            .expect("real Axum WebSocket upgrade should succeed");
        assert_eq!(response.status(), 101);

        send_test_websocket_json(
            &mut socket,
            json!({
                "type": "clientCapabilities",
                "capabilities": [super::NATIVE_GAME_SHOP_RECEIPT_PROTOCOL]
            }),
        )
        .await;
        send_test_websocket_json(
            &mut socket,
            json!({
                "type": "newAccount",
                "accountId": "native_ws_shop_e2e",
                "password": "native-ws-shop-pass",
                "birthDateBinary": 0,
                "userName": "Native WS Shop",
                "secretQuestion": "q",
                "secretAnswer": "a",
                "emailAddress": "native-ws-shop@example.test"
            }),
        )
        .await;
        let (new_account, _) =
            read_test_websocket_until(&mut socket, "NewAccount success", |event| {
                test_packet(event, "NewAccount")
            })
            .await;
        assert_eq!(new_account["payload"]["result"], 8);

        send_test_websocket_json(
            &mut socket,
            json!({
                "type": "login",
                "accountId": "native_ws_shop_e2e",
                "password": "native-ws-shop-pass"
            }),
        )
        .await;
        let (login, login_events) =
            read_test_websocket_until(&mut socket, "authenticated LoginSuccess", |event| {
                test_packet(event, "LoginSuccess")
            })
            .await;
        assert!(login["payload"]["characters"].is_array());
        assert!(login_events
            .iter()
            .any(|event| event["type"] == "identitySession"));

        send_test_websocket_json(
            &mut socket,
            json!({
                "type": "newCharacter",
                "name": "WsShopHero",
                "gender": "Male",
                "class": "Warrior"
            }),
        )
        .await;
        let (new_character, _) =
            read_test_websocket_until(&mut socket, "NewCharacterSuccess", |event| {
                test_packet(event, "NewCharacterSuccess")
            })
            .await;
        let character_index = new_character["payload"]["character"]["index"]
            .as_i64()
            .expect("new character index") as i32;
        {
            let mut store = account_store
                .lock()
                .expect("test account store should not be poisoned");
            let save = store
                .accounts
                .get_mut("native_ws_shop_e2e")
                .expect("created account")
                .saves
                .get_mut(&character_index)
                .expect("created character save");
            save.gold = 200_000;
        }

        send_test_websocket_json(
            &mut socket,
            json!({"type": "startGame", "characterIndex": character_index}),
        )
        .await;
        let (start_game, _) =
            read_test_websocket_until(&mut socket, "StartGame result 4", |event| {
                test_packet(event, "StartGame")
            })
            .await;
        assert_eq!(start_game["payload"]["result"], 4);

        send_test_websocket_json(
            &mut socket,
            json!({
                "type": "gameShopBuy",
                "requestId": "native-ws-gs-e2e-0001",
                "gIndex": 31,
                "quantity": 1,
                "priceType": 1
            }),
        )
        .await;
        let (receipt, purchase_events) =
            read_test_websocket_until(&mut socket, "native GameShop receipt", |event| {
                event["type"] == "gameShopReceipt"
            })
            .await;
        assert_eq!(
            receipt["protocol"],
            super::NATIVE_GAME_SHOP_RECEIPT_PROTOCOL
        );
        assert_eq!(receipt["requestId"], "native-ws-gs-e2e-0001");
        assert_eq!(receipt["success"], true);
        assert_eq!(receipt["gIndex"], 31);
        assert_eq!(receipt["quantity"], 1);
        assert_eq!(receipt["priceType"], 1);
        let mail_id = receipt["mailId"].as_u64().expect("receipt mailId");
        assert!(purchase_events
            .iter()
            .any(|event| test_packet(event, "LoseGold")));
        assert!(purchase_events
            .iter()
            .any(|event| test_packet(event, "ReceiveMail")));
        assert_eq!(purchase_events.last(), Some(&receipt));

        let (expected_key, expected_quantity, attachment_unique_id) = {
            let store = account_store
                .lock()
                .expect("test account store should not be poisoned");
            let save = &store.accounts["native_ws_shop_e2e"].saves[&character_index];
            let systems: Stage5SystemsState = serde_json::from_str(
                save.stage5_systems_json
                    .as_deref()
                    .expect("purchase mailbox should persist"),
            )
            .expect("purchase mailbox should decode");
            let mail = systems
                .mail
                .iter()
                .find(|mail| u64::from(mail.id) == mail_id)
                .expect("receipt must name the durable Gameshop mail");
            assert_eq!(mail.from, "Gameshop");
            assert!(!mail.claimed);
            assert_eq!(mail.item_states_json.len(), 1);
            let item: Value =
                serde_json::from_str(&mail.item_states_json[0]).expect("exact attachment JSON");
            (
                item["key"].as_str().expect("attachment key").to_string(),
                item["quantity"].as_u64().expect("attachment quantity") as u32,
                item["unique_id"].as_u64().expect("attachment unique id"),
            )
        };

        send_test_websocket_json(
            &mut socket,
            json!({"type": "collectParcel", "mailId": mail_id}),
        )
        .await;
        let (parcel, claim_events) =
            read_test_websocket_until(&mut socket, "successful ParcelCollected", |event| {
                test_packet(event, "ParcelCollected")
            })
            .await;
        assert_eq!(parcel["payload"]["result"], 1);
        assert_eq!(
            claim_events
                .iter()
                .filter(|event| test_packet(event, "GainedItem"))
                .count(),
            1
        );
        {
            let store = account_store
                .lock()
                .expect("test account store should not be poisoned");
            let save = &store.accounts["native_ws_shop_e2e"].saves[&character_index];
            let systems: Stage5SystemsState =
                serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
            assert!(
                systems
                    .mail
                    .iter()
                    .find(|mail| u64::from(mail.id) == mail_id)
                    .expect("claimed mail should remain addressable")
                    .claimed
            );
            let delivered = save
                .inventory_items_json
                .iter()
                .map(|item| {
                    serde_json::from_str::<Value>(item).expect("persisted inventory item JSON")
                })
                .filter(|item| item["key"] == expected_key && item["quantity"] == expected_quantity)
                .collect::<Vec<_>>();
            assert_eq!(delivered.len(), 1);
            assert_ne!(
                delivered[0]["unique_id"]
                    .as_u64()
                    .expect("delivered unique id"),
                attachment_unique_id
            );
        }

        send_test_websocket_json(
            &mut socket,
            json!({"type": "collectParcel", "mailId": mail_id}),
        )
        .await;
        let (duplicate_parcel, duplicate_claim_events) = read_test_websocket_until(
            &mut socket,
            "duplicate ParcelCollected rejection",
            |event| test_packet(event, "ParcelCollected"),
        )
        .await;
        assert_eq!(duplicate_parcel["payload"]["result"], -1);
        assert!(!duplicate_claim_events
            .iter()
            .any(|event| test_packet(event, "GainedItem")));

        socket
            .close(None)
            .await
            .expect("test WebSocket should close cleanly");
        server.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_resume_handler_reconnects_once_rotates_credential_and_replays_no_commands() {
        let ai_data_dir = std::env::temp_dir().join(format!(
            "mir2-native-resume-ws-e2e-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let reconnect_sessions = Arc::new(super::ReconnectSessionStore::default());
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            Some(2),
            Some(1),
            Some(1),
        ));
        let state = super::WebState {
            config: Arc::new(SimulationConfig::default()),
            deploy_revision: None,
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::clone(&reconnect_sessions),
            capacity: Arc::clone(&capacity),
            gameplay_event_sink: None,
            identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator: crate::spectator::SpectatorHub::from_env(),
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(ai_data_dir),
            )
            .expect("test AI live hub"),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        };
        let app = axum::Router::new()
            .route("/ws", axum::routing::get(super::ws_upgrade))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind isolated native-resume WebSocket gateway");
        let address = listener
            .local_addr()
            .expect("isolated native-resume listener address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .expect("isolated native-resume WebSocket gateway should serve");
        });

        let connect = |address| async move {
            let mut request = format!("ws://{address}/ws")
                .into_client_request()
                .expect("valid isolated WebSocket URL");
            let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
                .ok()
                .and_then(|value| {
                    value
                        .split(',')
                        .map(str::trim)
                        .find(|entry| !entry.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| format!("http://{address}"));
            request.headers_mut().insert(
                "origin",
                origin.parse().expect("valid test WebSocket Origin"),
            );
            let (socket, response) = tokio_tungstenite::connect_async(request)
                .await
                .expect("real Axum WebSocket upgrade should succeed");
            assert_eq!(response.status(), 101);
            socket
        };

        let mut first_socket = connect(address).await;
        send_test_websocket_json(
            &mut first_socket,
            json!({
                "type": "clientCapabilities",
                "capabilities": [super::NATIVE_RESUME_PROTOCOL]
            }),
        )
        .await;
        send_test_websocket_json(
            &mut first_socket,
            json!({
                "type": "newAccount",
                "accountId": "native_resume_ws_e2e",
                "password": "native-resume-ws-pass",
                "birthDateBinary": 0,
                "userName": "Native Resume WS",
                "secretQuestion": "q",
                "secretAnswer": "a",
                "emailAddress": "native-resume-ws@example.test"
            }),
        )
        .await;
        let (new_account, _) =
            read_test_websocket_until(&mut first_socket, "NewAccount success", |event| {
                test_packet(event, "NewAccount")
            })
            .await;
        assert_eq!(new_account["payload"]["result"], 8);

        send_test_websocket_json(
            &mut first_socket,
            json!({
                "type": "login",
                "accountId": "native_resume_ws_e2e",
                "password": "native-resume-ws-pass"
            }),
        )
        .await;
        let (_, login_events) =
            read_test_websocket_until(&mut first_socket, "authenticated LoginSuccess", |event| {
                test_packet(event, "LoginSuccess")
            })
            .await;
        assert!(login_events
            .iter()
            .any(|event| event["type"] == "identitySession"));

        send_test_websocket_json(
            &mut first_socket,
            json!({
                "type": "newCharacter",
                "name": "WsResumeHero",
                "gender": "Male",
                "class": "Warrior"
            }),
        )
        .await;
        let (new_character, _) =
            read_test_websocket_until(&mut first_socket, "NewCharacterSuccess", |event| {
                test_packet(event, "NewCharacterSuccess")
            })
            .await;
        let character_index = new_character["payload"]["character"]["index"]
            .as_i64()
            .expect("new character index") as i32;

        send_test_websocket_json(
            &mut first_socket,
            json!({"type": "startGame", "characterIndex": character_index}),
        )
        .await;
        let (first_credential_event, first_start_events) = read_test_websocket_until(
            &mut first_socket,
            "first native resume credential",
            |event| event["type"] == "resumeCredential",
        )
        .await;
        assert!(first_start_events
            .iter()
            .any(|event| test_packet(event, "StartGame") && event["payload"]["result"] == 4));
        let first_credential = first_credential_event["credential"]
            .as_str()
            .expect("first resume credential")
            .to_string();
        let first_resume_credential =
            serde_json::from_value::<crate::resume::ResumeCredential>(json!(first_credential))
                .expect("issued credential must satisfy the wire contract");
        let first_generation = first_credential_event["generation"]
            .as_u64()
            .expect("first resume credential generation");

        // This is deliberately sent immediately before the transport close.  The server may
        // either process it on the first connection or observe the close first, but it must
        // never reappear on the resumed connection as an inherited queued input.
        const PENDING_CHAT_MARKER: &str = "native-resume-no-command-replay";
        send_test_websocket_json(
            &mut first_socket,
            json!({"type": "chat", "message": PENDING_CHAT_MARKER}),
        )
        .await;
        first_socket
            .close(None)
            .await
            .expect("first native-resume socket should close cleanly");

        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let status = capacity.status();
                if reconnect_sessions.len() == 1
                    && reconnect_sessions
                        .resume_binding(&first_resume_credential, super::gateway_unix_ms())
                        .is_some()
                    && status.current_ws_connections == 0
                    && status.current_active_sessions == 1
                    && status.current_reconnect_leases == 1
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("first socket close should retain exactly one reconnect lease");

        let mut resumed_socket = connect(address).await;
        send_test_websocket_json(
            &mut resumed_socket,
            json!({
                "type": "clientCapabilities",
                "capabilities": [super::NATIVE_RESUME_PROTOCOL]
            }),
        )
        .await;
        send_test_websocket_json(
            &mut resumed_socket,
            json!({"type": "resumeSession", "credential": first_credential.clone()}),
        )
        .await;
        let (rotated_credential_event, resumed_events) = read_test_websocket_until(
            &mut resumed_socket,
            "rotated native resume credential",
            |event| event["type"] == "resumeCredential",
        )
        .await;
        let resumed_at = resumed_events
            .iter()
            .position(|event| event["type"] == "sessionResumed")
            .expect("resume handler must acknowledge the accepted credential");
        let authoritative_snapshot_at = resumed_events
            .iter()
            .enumerate()
            .skip(resumed_at.saturating_add(1))
            .find_map(|(index, event)| (event["type"] == "worldSnapshot").then_some(index))
            .expect("accepted resume must emit an authoritative post-resume snapshot");
        assert!(
            resumed_at < authoritative_snapshot_at,
            "sessionResumed must precede the authoritative post-resume bootstrap"
        );
        assert!(
            !resumed_events
                .iter()
                .skip(resumed_at.saturating_add(1))
                .any(|event| {
                    test_packet(event, "ObjectChat")
                        && event["payload"]["text"] == PENDING_CHAT_MARKER
                }),
            "a command raced with the closed connection must not replay after resume"
        );
        let rotated_credential = rotated_credential_event["credential"]
            .as_str()
            .expect("rotated resume credential")
            .to_string();
        assert_ne!(rotated_credential, first_credential);
        assert_eq!(
            rotated_credential_event["generation"].as_u64(),
            Some(first_generation.saturating_add(1)),
            "resume must rotate the credential generation exactly once"
        );
        assert_eq!(reconnect_sessions.len(), 0);
        let status = capacity.status();
        assert_eq!(status.current_active_sessions, 1);
        assert_eq!(status.current_reconnect_leases, 0);

        let mut replay_socket = connect(address).await;
        send_test_websocket_json(
            &mut replay_socket,
            json!({
                "type": "clientCapabilities",
                "capabilities": [super::NATIVE_RESUME_PROTOCOL]
            }),
        )
        .await;
        send_test_websocket_json(
            &mut replay_socket,
            json!({"type": "resumeSession", "credential": first_credential.clone()}),
        )
        .await;
        let (replay_rejected, _) = read_test_websocket_until(
            &mut replay_socket,
            "replayed credential rejection",
            |event| event["type"] == "resumeRejected",
        )
        .await;
        assert_eq!(replay_rejected, super::resume_rejected_event());
        replay_socket
            .close(None)
            .await
            .expect("replay socket should close cleanly");

        send_test_websocket_json(&mut resumed_socket, json!({"type": "disconnect"})).await;
        let (_, _) = read_test_websocket_until(
            &mut resumed_socket,
            "explicit Disconnect response",
            |event| test_packet(event, "Disconnect"),
        )
        .await;
        resumed_socket
            .close(None)
            .await
            .expect("resumed socket should close cleanly after Disconnect");

        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let status = capacity.status();
                if reconnect_sessions.len() == 0
                    && status.current_ws_connections == 0
                    && status.current_active_sessions == 0
                    && status.current_reconnect_leases == 0
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("explicit Disconnect must release active-session and reconnect-lease capacity");

        server.abort();
    }

    #[test]
    fn repair_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"repairItem","uniqueId":5}"#)
                .expect("repair item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("repair item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::RepairItem { unique_id }) => {
                assert_eq!(unique_id, 5);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn special_repair_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"specialRepairItem","uniqueId":6}"#)
                .expect("special repair item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("special repair item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::SRepairItem { unique_id }) => {
                assert_eq!(unique_id, 6);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn store_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"storeItem","from":2,"to":5}"#)
                .expect("store item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("store item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::StoreItem { from, to }) => {
                assert_eq!(from, 2);
                assert_eq!(to, 5);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn take_back_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"takeBackItem","from":5,"to":3}"#)
                .expect("take back item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("take back item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::TakeBackItem { from, to }) => {
                assert_eq!(from, 5);
                assert_eq!(to, 3);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn storage_request_ids_opt_into_v2_without_breaking_legacy_commands() {
        let store = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"storeItemV2","requestId":"st-0000000000000001","from":2,"to":5}"#,
        )
        .expect("v2 store item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(store).expect("v2 store item command should map"),
            SessionAction::Packet(ClientPacket::StoreItemV2 {
                request_id,
                from: 2,
                to: 5,
            }) if request_id == "st-0000000000000001"
        ));

        let take_back = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"takeBackItemV2","requestId":"st-0000000000000002","from":5,"to":3}"#,
        )
        .expect("v2 take back command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(take_back)
                .expect("v2 take back command should map"),
            SessionAction::Packet(ClientPacket::TakeBackItemV2 {
                request_id,
                from: 5,
                to: 3,
            }) if request_id == "st-0000000000000002"
        ));

        let invalid = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"storeItemV2","requestId":"bad\nline","from":2,"to":5}"#,
        )
        .expect("JSON shape is valid before request id validation");
        assert!(super::browser_command_to_action(invalid).is_err());
    }

    #[test]
    fn frozen_legacy_gateway_schema_rejects_storage_v2_before_execution() {
        #[allow(dead_code)]
        #[derive(serde::Deserialize)]
        #[serde(tag = "type", rename_all = "camelCase")]
        enum LegacyStorageCommand {
            StoreItem { from: i32, to: i32 },
            TakeBackItem { from: i32, to: i32 },
        }

        let executions = std::cell::Cell::new(0_u32);
        let decoded = serde_json::from_str::<LegacyStorageCommand>(
            r#"{"type":"storeItemV2","requestId":"st-0000000000000001","from":2,"to":5}"#,
        );
        if decoded.is_ok() {
            executions.set(executions.get() + 1);
        }
        assert!(decoded.is_err());
        assert_eq!(executions.get(), 0);
    }

    #[test]
    fn storage_v2_server_packets_echo_the_exact_request_id() {
        let event = super::server_packet_to_event(&ServerPacket::StoreItemV2 {
            request_id: "st-0000000000000007".to_string(),
            from: 2,
            to: 5,
            success: false,
        });
        assert_eq!(event["packet"], "StoreItemV2");
        assert_eq!(event["payload"]["requestId"], "st-0000000000000007");
        assert_eq!(event["payload"]["success"], false);

        let event = super::server_packet_to_event(&ServerPacket::TakeBackItemV2 {
            request_id: "st-0000000000000008".to_string(),
            from: 5,
            to: 3,
            success: true,
        });
        assert_eq!(event["packet"], "TakeBackItemV2");
        assert_eq!(event["payload"]["requestId"], "st-0000000000000008");
        assert_eq!(event["payload"]["success"], true);
    }

    #[test]
    fn magic_commands_map_to_crystal_protocol_packets() {
        let key_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"magicKey","spell":"Fury","key":5,"oldKey":0}"#,
        )
        .expect("magic key command should deserialize");
        let action = super::browser_command_to_action(key_command)
            .expect("magic key command should map to a session action");
        match action {
            SessionAction::Packet(ClientPacket::MagicKey {
                spell,
                key,
                old_key,
            }) => {
                assert_eq!(spell, Spell::Fury);
                assert_eq!(key, 5);
                assert_eq!(old_key, 0);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let magic_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"magic","objectId":1001,"spell":"Healing","direction":"downLeft","targetId":1001,"x":333,"y":267,"spellTargetLock":true}"#,
        )
        .expect("magic command should deserialize");
        let action = super::browser_command_to_action(magic_command)
            .expect("magic command should map to a session action");
        match action {
            SessionAction::Packet(ClientPacket::Magic {
                object_id,
                spell,
                direction,
                target_id,
                location,
                spell_target_lock,
            }) => {
                assert_eq!(object_id, 1_001);
                assert_eq!(spell, Spell::Healing);
                assert_eq!(direction, MirDirection::DownLeft);
                assert_eq!(target_id, 1_001);
                assert_eq!(location, Point { x: 333, y: 267 });
                assert!(spell_target_lock);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let toggle_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"spellToggle","spell":"Slaying","canUse":true}"#,
        )
        .expect("spell toggle command should deserialize");
        let action = super::browser_command_to_action(toggle_command)
            .expect("spell toggle command should map to a session action");
        match action {
            SessionAction::Packet(ClientPacket::SpellToggle {
                spell,
                toggle_state,
            }) => {
                assert_eq!(spell, Spell::Slaying);
                assert_eq!(toggle_state, 1);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn magic_packets_are_exposed_as_browser_events() {
        let magic = super::server_packet_to_event(&ServerPacket::Magic {
            spell: Spell::Fury,
            target_id: 0,
            target: Point { x: 333, y: 267 },
            cast: true,
            level: 3,
            secondary_target_ids: vec![2],
        });
        assert_eq!(magic["packet"], "Magic");
        assert_eq!(magic["payload"]["spell"], "Fury");
        assert_eq!(magic["payload"]["secondaryTargetIds"][0], 2);

        let object_magic = super::server_packet_to_event(&ServerPacket::ObjectMagic {
            object_id: 1_001,
            location: Point { x: 332, y: 267 },
            direction: MirDirection::Right,
            spell: Spell::Fury,
            target_id: 0,
            target: Point { x: 333, y: 267 },
            cast: true,
            level: 3,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        });
        assert_eq!(object_magic["packet"], "ObjectMagic");
        assert_eq!(object_magic["payload"]["objectId"], 1_001);
        assert_eq!(object_magic["payload"]["direction"], "Right");

        let toggle = super::server_packet_to_event(&ServerPacket::SpellToggle {
            object_id: 1_001,
            spell: Spell::Slaying,
            can_use: true,
        });
        assert_eq!(toggle["packet"], "SpellToggle");
        assert_eq!(toggle["payload"]["canUse"], true);

        let mana = super::server_packet_to_event(&ServerPacket::ObjectMana {
            info: ObjectManaInfo {
                object_id: 1_001,
                percent: 88,
            },
        });
        assert_eq!(mana["packet"], "ObjectMana");
        assert_eq!(mana["payload"]["percent"], 88);

        let add_buff = super::server_packet_to_event(&ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 5,
                visible: true,
                object_id: 1_001,
                expire_time: 60_000,
                infinite: false,
                paused: false,
                stats: vec![UserItemStat { stat: 14, value: 4 }],
                values: Vec::new(),
            },
        });
        assert_eq!(add_buff["packet"], "AddBuff");
        assert_eq!(add_buff["payload"]["buffType"], 5);
        assert_eq!(add_buff["payload"]["stats"][0]["stat"], 14);
        // Browser buff contract: numeric `type`, `remainingMs` (= expire_time, which
        // the sim already stores as ms-from-now), and `{label, value}` stats.
        assert_eq!(add_buff["payload"]["type"], 5);
        assert_eq!(add_buff["payload"]["remainingMs"], 60_000);
        assert_eq!(add_buff["payload"]["stats"][0]["label"], "AttackSpeed");
        assert_eq!(add_buff["payload"]["stats"][0]["value"], 4);

        let remove_buff = super::server_packet_to_event(&ServerPacket::RemoveBuff {
            buff_type: 5,
            object_id: 1_001,
        });
        assert_eq!(remove_buff["packet"], "RemoveBuff");
        assert_eq!(remove_buff["payload"]["objectId"], 1_001);
    }

    #[test]
    fn simple_bootstrap_social_packets_are_exposed_as_browser_events() {
        let time = super::server_packet_to_event(&ServerPacket::TimeOfDay { lights: 4 });
        assert_eq!(time["packet"], "TimeOfDay");
        assert_eq!(time["payload"]["lights"], 4);

        let attack = super::server_packet_to_event(&ServerPacket::ChangeAMode { mode: 2 });
        assert_eq!(attack["packet"], "ChangeAMode");
        assert_eq!(attack["payload"]["mode"], 2);

        let npc_response = super::server_packet_to_event(&ServerPacket::NPCResponse {
            page: vec!["@main".to_string(), "Welcome".to_string()],
        });
        assert_eq!(npc_response["packet"], "NPCResponse");
        assert_eq!(npc_response["payload"]["page"][1], "Welcome");

        let default_npc =
            super::server_packet_to_event(&ServerPacket::DefaultNPC { object_id: 1_001 });
        assert_eq!(default_npc["packet"], "DefaultNPC");
        assert_eq!(default_npc["payload"]["objectId"], 1_001);

        let npc_update = super::server_packet_to_event(&ServerPacket::NPCUpdate { npc_id: 1_002 });
        assert_eq!(npc_update["packet"], "NPCUpdate");
        assert_eq!(npc_update["payload"]["npcId"], 1_002);

        let lover = super::server_packet_to_event(&ServerPacket::LoverUpdate {
            name: "Partner".to_string(),
            date_binary_datetime: 42,
            map_name: "Bichon".to_string(),
            married_days: 7,
        });
        assert_eq!(lover["packet"], "LoverUpdate");
        assert_eq!(lover["payload"]["mapName"], "Bichon");

        let mentor = super::server_packet_to_event(&ServerPacket::MentorUpdate {
            name: "Mentor".to_string(),
            level: 45,
            online: true,
            mentee_exp: 1_234,
        });
        assert_eq!(mentor["packet"], "MentorUpdate");
        assert_eq!(mentor["payload"]["online"], true);
        assert_eq!(mentor["payload"]["menteeExp"], 1_234);
    }

    #[test]
    fn group_utility_packets_are_exposed_as_browser_events() {
        let switch =
            super::server_packet_to_event(&ServerPacket::SwitchGroup { allow_group: true });
        assert_eq!(switch["packet"], "SwitchGroup");
        assert_eq!(switch["payload"]["allowGroup"], true);

        let add = super::server_packet_to_event(&ServerPacket::AddMember {
            name: "Scout".to_string(),
        });
        assert_eq!(add["packet"], "AddMember");
        assert_eq!(add["payload"]["name"], "Scout");

        let member_map = super::server_packet_to_event(&ServerPacket::GroupMembersMap {
            player_name: "Scout".to_string(),
            player_map: "Bichon Province".to_string(),
        });
        assert_eq!(member_map["packet"], "GroupMembersMap");
        assert_eq!(member_map["payload"]["playerMap"], "Bichon Province");

        // Enriched roster: members widen to objects (level/class/hp/maxHp/online),
        // unknown fields are omitted, and `leaderName` mirrors members[0].
        let roster = super::server_packet_to_event(&ServerPacket::GroupMemberInfo {
            members: vec![
                GroupMember {
                    name: "Scout".to_string(),
                    level: Some(33),
                    class: Some(MirClass::Archer as u8),
                    hp: Some(210),
                    max_hp: Some(260),
                    online: Some(true),
                },
                GroupMember {
                    name: "Faraway".to_string(),
                    level: Some(40),
                    class: Some(MirClass::Wizard as u8),
                    hp: None,
                    max_hp: None,
                    online: Some(true),
                },
            ],
            leader_name: "Scout".to_string(),
        });
        assert_eq!(roster["packet"], "GroupMemberInfo");
        assert_eq!(roster["payload"]["leaderName"], "Scout");
        assert_eq!(roster["payload"]["members"][0]["name"], "Scout");
        assert_eq!(roster["payload"]["members"][0]["level"], 33);
        assert_eq!(roster["payload"]["members"][0]["class"], 4);
        assert_eq!(roster["payload"]["members"][0]["hp"], 210);
        assert_eq!(roster["payload"]["members"][0]["maxHp"], 260);
        assert_eq!(roster["payload"]["members"][0]["online"], true);
        // A member with unknown HP omits hp/maxHp entirely.
        assert!(roster["payload"]["members"][1].get("hp").is_none());
        assert!(roster["payload"]["members"][1].get("maxHp").is_none());
        assert_eq!(roster["payload"]["members"][1]["class"], 1);

        let location = super::server_packet_to_event(&ServerPacket::SendMemberLocation {
            member_name: "Scout".to_string(),
            member_location: Point { x: 330, y: 270 },
        });
        assert_eq!(location["packet"], "SendMemberLocation");
        assert_eq!(location["payload"]["memberLocation"]["x"], 330);

        let door = super::server_packet_to_event(&ServerPacket::OpenDoor {
            door_index: 4,
            close: false,
        });
        assert_eq!(door["packet"], "Opendoor");
        assert_eq!(door["payload"]["doorIndex"], 4);

        let timer = super::server_packet_to_event(&ServerPacket::SetTimer {
            key: "quest".to_string(),
            timer_type: 1,
            seconds: 60,
        });
        assert_eq!(timer["packet"], "SetTimer");
        assert_eq!(timer["payload"]["seconds"], 60);

        let compass = super::server_packet_to_event(&ServerPacket::SetCompass {
            location: Point { x: 331, y: 271 },
        });
        assert_eq!(compass["packet"], "SetCompass");
        assert_eq!(compass["payload"]["location"]["y"], 271);
    }

    #[test]
    fn quest_packets_are_exposed_as_browser_events() {
        let change = super::server_packet_to_event(&ServerPacket::ChangeQuest {
            quest_id: 1001,
            task_list: vec!["Collect Wasp Stinger 0/1".to_string()],
            taken: true,
            completed: false,
            new: true,
            quest_state: 0,
            track_quest: true,
        });
        assert_eq!(change["packet"], "ChangeQuest");
        assert_eq!(change["payload"]["questId"], 1001);
        assert_eq!(change["payload"]["taskList"][0], "Collect Wasp Stinger 0/1");
        // Contract: dynamic quest fields are hoisted from the progress payload.
        assert_eq!(change["payload"]["id"], 1001);
        assert_eq!(change["payload"]["state"], 0);
        assert_eq!(
            change["payload"]["descriptionLines"][0],
            "Collect Wasp Stinger 0/1"
        );
        // The "0/1" progress fraction is parsed into current/required.
        let objective = &change["payload"]["objectives"][0];
        assert_eq!(objective["text"], "Collect Wasp Stinger 0/1");
        assert_eq!(objective["current"], 0);
        assert_eq!(objective["required"], 1);
        assert_eq!(objective["done"], false);
        assert_eq!(change["payload"]["current"], 0);
        assert_eq!(change["payload"]["required"], 1);

        let multi_task = super::server_packet_to_event(&ServerPacket::ChangeQuest {
            quest_id: 1002,
            task_list: vec![
                "Kill Deer 10/10".to_string(),
                "Kill Scarecrow 9/10".to_string(),
            ],
            taken: true,
            completed: false,
            new: false,
            quest_state: 1,
            track_quest: false,
        });
        assert_eq!(multi_task["payload"]["current"], 19);
        assert_eq!(multi_task["payload"]["required"], 20);
        assert_eq!(multi_task["payload"]["objectives"][0]["done"], true);
        assert_eq!(multi_task["payload"]["objectives"][1]["done"], false);

        let complete = super::server_packet_to_event(&ServerPacket::CompleteQuest {
            completed_quests: vec![1001],
        });
        assert_eq!(complete["packet"], "CompleteQuest");
        assert_eq!(complete["payload"]["completedQuests"][0], 1001);

        let share = super::server_packet_to_event(&ServerPacket::ShareQuest {
            quest_index: 1001,
            sharer_name: "Scout".to_string(),
        });
        assert_eq!(share["packet"], "ShareQuest");
        assert_eq!(share["payload"]["sharerName"], "Scout");
    }

    #[test]
    fn new_quest_info_exposes_contract_quest_fields() {
        let reward_item = |index: i32, name: &str, image: u16| mir2_protocol::ItemInfo {
            index,
            name: name.to_string(),
            item_type: 0,
            grade: 0,
            required_type: 0,
            required_class: 0,
            required_gender: 0,
            item_set: 0,
            shape: 1,
            weight: 1,
            light: 0,
            required_amount: 0,
            image,
            durability: 1_000,
            stack_size: 1,
            price: 10,
            start_item: false,
            effect: 0,
            need_identify: false,
            show_group_pickup: false,
            class_based: false,
            level_based: false,
            can_mine: false,
            global_drop_notify: false,
            bind: 0,
            unique: 0,
            random_stats_id: 0,
            can_fast_run: false,
            can_awakening: false,
            slots: 0,
            stats: Vec::new(),
            tooltip: None,
        };
        let info = super::server_packet_to_event(&ServerPacket::NewQuestInfo {
            info: ClientQuestInfo {
                index: 1001,
                npc_index: 1_001,
                name: "Field Wasp".to_string(),
                group: "Starter".to_string(),
                description: vec!["Help the town guard.".to_string()],
                task_description: vec!["Defeat 3 Wasps 0/3".to_string()],
                return_description: vec!["Return to the guard.".to_string()],
                completion_description: vec!["Good work.".to_string()],
                min_level_needed: 1,
                max_level_needed: 0,
                quest_needed: 0,
                class_needed: 31,
                quest_type: 0,
                time_limit_in_seconds: 90,
                reward_gold: 500,
                reward_exp: 1_200,
                reward_credit: 0,
                rewards_fixed_item: vec![mir2_protocol::QuestItemReward {
                    item: reward_item(658, "(HP)DrugSmall", 532),
                    count: 2,
                }],
                rewards_select_item: vec![
                    mir2_protocol::QuestItemReward {
                        item: reward_item(1_172, "SharpDagger", 1_169),
                        count: 1,
                    },
                    mir2_protocol::QuestItemReward {
                        item: reward_item(1_173, "ToughHoaSword", 1_170),
                        count: 1,
                    },
                ],
                finish_npc_index: 1_001,
            },
        });
        assert_eq!(info["packet"], "NewQuestInfo");
        // Raw info retained for backward compatibility.
        assert_eq!(info["payload"]["info"]["index"], 1001);
        // Contract-shaped quest fields hoisted alongside it.
        assert_eq!(info["payload"]["id"], 1001);
        assert_eq!(info["payload"]["name"], "Field Wasp");
        assert_eq!(info["payload"]["group"], "Starter");
        assert_eq!(
            info["payload"]["descriptionLines"][0],
            "Help the town guard."
        );
        assert_eq!(
            info["payload"]["objectives"][0]["text"],
            "Defeat 3 Wasps 0/3"
        );
        assert_eq!(info["payload"]["objectives"][0]["current"], 0);
        assert_eq!(info["payload"]["objectives"][0]["required"], 3);
        assert_eq!(info["payload"]["rewards"]["gold"], 500);
        assert_eq!(info["payload"]["rewards"]["experience"], 1_200);
        assert_eq!(
            info["payload"]["rewards"]["items"][0]["name"],
            "(HP)DrugSmall"
        );
        assert_eq!(info["payload"]["rewards"]["items"][0]["count"], 2);
        assert_eq!(info["payload"]["rewards"]["items"][0]["icon"], 532);
        assert_eq!(info["payload"]["rewards"]["items"][0]["itemIndex"], 658);
        assert_eq!(
            info["payload"]["rewards"]["selectItems"][0]["name"],
            "SharpDagger"
        );
        assert_eq!(
            info["payload"]["rewards"]["selectItems"][0]["selectionIndex"],
            0
        );
        assert_eq!(
            info["payload"]["rewards"]["selectItems"][1]["selectionIndex"],
            1
        );
        assert_eq!(
            info["payload"]["rewards"]["selectItems"][0]["selectable"],
            true
        );
        // No credit reward -> field omitted.
        assert!(info["payload"]["rewards"].get("credit").is_none());
        // 90 seconds -> "01:30".
        assert_eq!(info["payload"]["timeLimit"], "01:30");
        // npc (a name) is omitted because only NPCIndex is known here.
        assert!(info["payload"].get("npc").is_none());
    }

    #[test]
    fn q23_quest_info_event_exposes_all_selectable_rewards() {
        let manifest = crystal_quest_packet_manifest();
        let payloads = crystal_quest_packet_payloads();
        let (template, payload) = manifest
            .quests
            .iter()
            .zip(payloads.iter())
            .find(|(template, _)| template.index == 23)
            .expect("Crystal q23 packet");
        let frame =
            encode_frame(ServerPacketId::NewQuestInfo as i16, payload).expect("q23 frame encodes");
        let packet = decode_server_packet(&frame).expect("q23 frame decodes");
        let event = super::server_packet_to_event(&packet);

        assert_eq!(template.name, "!Attack Oma");
        assert_eq!(
            event["payload"]["rewards"]["selectItems"]
                .as_array()
                .expect("selectable rewards")
                .len(),
            3
        );
        assert_eq!(
            event["payload"]["rewards"]["selectItems"][0]["name"],
            "BronzeShortSword"
        );
        assert_eq!(
            event["payload"]["rewards"]["selectItems"][0]["selectionIndex"],
            0
        );
    }

    #[test]
    fn refine_packets_are_exposed_as_browser_events() {
        let deposit = super::server_packet_to_event(&ServerPacket::DepositRefineItem {
            from: 4,
            to: 0,
            success: true,
        });
        assert_eq!(deposit["packet"], "DepositRefineItem");
        assert_eq!(deposit["payload"]["success"], true);

        let retrieve = super::server_packet_to_event(&ServerPacket::RetrieveRefineItem {
            from: 0,
            to: 4,
            success: true,
        });
        assert_eq!(retrieve["packet"], "RetrieveRefineItem");
        assert_eq!(retrieve["payload"]["to"], 4);

        let cancel = super::server_packet_to_event(&ServerPacket::RefineCancel);
        assert_eq!(cancel["packet"], "RefineCancel");

        let refine = super::server_packet_to_event(&ServerPacket::RefineItem { unique_id: 4 });
        assert_eq!(refine["packet"], "RefineItem");
        assert_eq!(refine["payload"]["uniqueId"], 4);
    }

    #[test]
    fn hero_commands_map_to_crystal_protocol_packets() {
        let new_hero = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"newHero","name":"Aide","gender":"female","class":"taoist"}"#,
        )
        .expect("new hero command should deserialize");
        match super::browser_command_to_action(new_hero).expect("new hero maps") {
            SessionAction::Packet(ClientPacket::NewHero {
                name,
                gender,
                class,
            }) => {
                assert_eq!(name, "Aide");
                assert_eq!(gender, MirGender::Female);
                assert_eq!(class, MirClass::Taoist);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let take_back = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"takeBackHeroItem","from":3,"to":1}"#,
        )
        .expect("take back hero item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(take_back).expect("take back maps"),
            SessionAction::Packet(ClientPacket::TakeBackHeroItem { from: 3, to: 1 })
        ));

        let transfer = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"transferHeroItem","from":1,"to":3}"#,
        )
        .expect("transfer hero item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(transfer).expect("transfer maps"),
            SessionAction::Packet(ClientPacket::TransferHeroItem { from: 1, to: 3 })
        ));

        let behaviour =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"setHeroBehaviour","behaviour":2}"#)
                .expect("set hero behaviour command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(behaviour).expect("behaviour maps"),
            SessionAction::Packet(ClientPacket::SetHeroBehaviour { behaviour: 2 })
        ));

        let change =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"changeHero","listIndex":1}"#)
                .expect("change hero command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(change).expect("change maps"),
            SessionAction::Packet(ClientPacket::ChangeHero { list_index: 1 })
        ));
    }

    // Newly-wired player->server actions: hero auto-pot config (Crystal
    // C.SetAutoPotValue / C.SetAutoPotItem), attack/pet mode toggles
    // (C.ChangeAMode / C.ChangePMode) and the door / conquest gate open
    // (C.Opendoor). Each previously had a simulation handler but no browser
    // command, so the frontend could not reach them.
    #[test]
    fn hero_and_world_control_commands_map_to_crystal_protocol_packets() {
        let auto_pot_value = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setAutoPotValue","stat":0,"value":80}"#,
        )
        .expect("set auto pot value command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(auto_pot_value).expect("auto pot value maps"),
            SessionAction::Packet(ClientPacket::SetAutoPotValue { stat: 0, value: 80 })
        ));

        let auto_pot_item = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setAutoPotItem","grid":"heroHpItem","itemIndex":0}"#,
        )
        .expect("set auto pot item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(auto_pot_item).expect("auto pot item maps"),
            SessionAction::Packet(ClientPacket::SetAutoPotItem {
                grid: MirGridType::HeroHpItem,
                item_index: 0,
            })
        ));

        let auto_pot_item_mp = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setAutoPotItem","grid":"heroMpItem","itemIndex":12}"#,
        )
        .expect("set auto pot item (mp) command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(auto_pot_item_mp).expect("auto pot item mp maps"),
            SessionAction::Packet(ClientPacket::SetAutoPotItem {
                grid: MirGridType::HeroMpItem,
                item_index: 12,
            })
        ));

        // Unknown grids are rejected (only hero auto-pot grids are valid here).
        let bad_grid = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setAutoPotItem","grid":"inventory","itemIndex":0}"#,
        )
        .expect("command should deserialize");
        assert!(super::browser_command_to_action(bad_grid).is_err());

        let a_mode = serde_json::from_str::<BrowserCommand>(r#"{"type":"changeAMode","mode":4}"#)
            .expect("change a mode command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(a_mode).expect("a mode maps"),
            SessionAction::Packet(ClientPacket::ChangeAMode { mode: 4 })
        ));

        let p_mode = serde_json::from_str::<BrowserCommand>(r#"{"type":"changePMode","mode":1}"#)
            .expect("change p mode command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(p_mode).expect("p mode maps"),
            SessionAction::Packet(ClientPacket::ChangePMode { mode: 1 })
        ));

        let open_door =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"openDoor","doorIndex":3}"#)
                .expect("open door command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(open_door).expect("open door maps"),
            SessionAction::Packet(ClientPacket::OpenDoor { door_index: 3 })
        ));
    }

    #[test]
    fn hero_packets_are_exposed_as_browser_events() {
        let new_hero = super::server_packet_to_event(&ServerPacket::NewHero { result: 0 });
        assert_eq!(new_hero["packet"], "NewHero");
        assert_eq!(new_hero["payload"]["result"], 0);

        let hero_info = super::server_packet_to_event(&ServerPacket::NewHeroInfo {
            info: sample_hero_info(0),
            storage_index: -1,
        });
        assert_eq!(hero_info["packet"], "NewHeroInfo");
        assert_eq!(hero_info["payload"]["info"]["name"], "Hero0");
        assert_eq!(hero_info["payload"]["storageIndex"], -1);

        let take_back = super::server_packet_to_event(&ServerPacket::TakeBackHeroItem {
            from: 3,
            to: 1,
            success: false,
        });
        assert_eq!(take_back["packet"], "TakeBackHeroItem");
        assert_eq!(take_back["payload"]["success"], false);

        let create = super::server_packet_to_event(&ServerPacket::HeroCreateRequest {
            can_create_class: vec![true, true, true, false, false],
        });
        assert_eq!(create["packet"], "HeroCreateRequest");
        assert_eq!(create["payload"]["canCreateClass"][0], true);

        let manage = super::server_packet_to_event(&ServerPacket::ManageHeroes {
            maximum_count: 3,
            current_hero: Some(sample_hero_info(0)),
            heroes: Some(vec![Some(sample_hero_info(1)), None]),
        });
        assert_eq!(manage["packet"], "ManageHeroes");
        assert_eq!(manage["payload"]["maximumCount"], 3);
        assert_eq!(manage["payload"]["currentHero"]["name"], "Hero0");

        let behaviour =
            super::server_packet_to_event(&ServerPacket::SetHeroBehaviour { behaviour: 2 });
        assert_eq!(behaviour["packet"], "SetHeroBehaviour");
        assert_eq!(behaviour["payload"]["behaviour"], 2);

        let change = super::server_packet_to_event(&ServerPacket::ChangeHero { from_index: 1 });
        assert_eq!(change["packet"], "ChangeHero");
        assert_eq!(change["payload"]["fromIndex"], 1);

        let spawn_state =
            super::server_packet_to_event(&ServerPacket::UpdateHeroSpawnState { state: 3 });
        assert_eq!(spawn_state["packet"], "UpdateHeroSpawnState");
        assert_eq!(spawn_state["payload"]["state"], 3);

        let gain =
            super::server_packet_to_event(&ServerPacket::GainHeroExperience { amount: 1_500 });
        assert_eq!(gain["packet"], "GainHeroExperience");
        assert_eq!(gain["payload"]["amount"], 1_500);

        let level = super::server_packet_to_event(&ServerPacket::HeroLevelChanged {
            level: 42,
            experience: 12_345,
            max_experience: 20_000,
        });
        assert_eq!(level["packet"], "HeroLevelChanged");
        assert_eq!(level["payload"]["level"], 42);
        assert_eq!(level["payload"]["experience"], 12_345);
        assert_eq!(level["payload"]["maxExperience"], 20_000);
    }

    #[test]
    fn market_relationship_commands_map_to_crystal_protocol_packets() {
        let consign = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"consignItem","uniqueId":77,"price":1500,"marketType":0}"#,
        )
        .expect("consign command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(consign).expect("consign maps"),
            SessionAction::Packet(ClientPacket::ConsignItem {
                unique_id: 77,
                price: 1_500,
                market_type: 0
            })
        ));

        let search = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"marketSearch","matchText":"blade","itemType":5,"userMode":true,"minShape":1,"maxShape":99,"marketType":0}"#,
        )
        .expect("market search command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(search).expect("search maps"),
            SessionAction::Packet(ClientPacket::MarketSearch {
                ref match_text,
                item_type: 5,
                user_mode: true,
                min_shape: 1,
                max_shape: 99,
                market_type: 0
            }) if match_text == "blade"
        ));

        let buy = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"marketBuy","auctionId":88,"bidPrice":2000}"#,
        )
        .expect("market buy command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(buy).expect("buy maps"),
            SessionAction::Packet(ClientPacket::MarketBuy {
                auction_id: 88,
                bid_price: 2_000
            })
        ));

        let marriage = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"marriageReply","acceptInvite":true}"#,
        )
        .expect("marriage reply command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(marriage).expect("marriage maps"),
            SessionAction::Packet(ClientPacket::MarriageReply {
                accept_invite: true
            })
        ));

        let mentor =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"addMentor","name":"Master"}"#)
                .expect("add mentor command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(mentor).expect("mentor maps"),
            SessionAction::Packet(ClientPacket::AddMentor { ref name }) if name == "Master"
        ));
    }

    #[test]
    fn market_relationship_packets_are_exposed_as_browser_events() {
        let consign = super::server_packet_to_event(&ServerPacket::ConsignItem {
            unique_id: 77,
            success: false,
        });
        assert_eq!(consign["packet"], "ConsignItem");
        assert_eq!(consign["payload"]["success"], false);

        let fail = super::server_packet_to_event(&ServerPacket::MarketFail { reason: 1 });
        assert_eq!(fail["packet"], "MarketFail");
        assert_eq!(fail["payload"]["reason"], 1);

        let success = super::server_packet_to_event(&ServerPacket::MarketSuccess {
            message: "Listed".to_string(),
        });
        assert_eq!(success["packet"], "MarketSuccess");
        assert_eq!(success["payload"]["message"], "Listed");

        let marriage = super::server_packet_to_event(&ServerPacket::MarriageRequest {
            name: "Partner".to_string(),
        });
        assert_eq!(marriage["packet"], "MarriageRequest");
        assert_eq!(marriage["payload"]["name"], "Partner");

        let mentor = super::server_packet_to_event(&ServerPacket::MentorRequest {
            name: "Master".to_string(),
            level: 45,
        });
        assert_eq!(mentor["packet"], "MentorRequest");
        assert_eq!(mentor["payload"]["level"], 45);
    }

    #[test]
    fn fishing_commands_map_to_crystal_protocol_packets() {
        let cast_command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"fishingCast","castOut":true}"#)
                .expect("fishing cast command should deserialize");
        let action = super::browser_command_to_action(cast_command)
            .expect("fishing cast command should map to a session action");
        assert!(matches!(
            action,
            SessionAction::Packet(ClientPacket::FishingCast { cast_out: true })
        ));

        let autocast_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"fishingChangeAutocast","autoCast":true}"#,
        )
        .expect("fishing autocast command should deserialize");
        let action = super::browser_command_to_action(autocast_command)
            .expect("fishing autocast command should map to a session action");
        assert!(matches!(
            action,
            SessionAction::Packet(ClientPacket::FishingChangeAutocast { auto_cast: true })
        ));
    }

    #[test]
    fn trade_commands_map_to_crystal_protocol_packets() {
        let deposit = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"depositTradeItem","from":4,"to":0}"#,
        )
        .expect("deposit trade command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(deposit).expect("deposit trade maps"),
            SessionAction::Packet(ClientPacket::DepositTradeItem { from: 4, to: 0 })
        ));

        let reply =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeReply","acceptInvite":true}"#)
                .expect("trade reply command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(reply).expect("trade reply maps"),
            SessionAction::Packet(ClientPacket::TradeReply {
                accept_invite: true
            })
        ));

        let gold = serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeGold","amount":100}"#)
            .expect("trade gold command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(gold).expect("trade gold maps"),
            SessionAction::Packet(ClientPacket::TradeGold { amount: 100 })
        ));

        let confirm =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeConfirm","locked":true}"#)
                .expect("trade confirm command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(confirm).expect("trade confirm maps"),
            SessionAction::Packet(ClientPacket::TradeConfirm { locked: true })
        ));

        let cancel = serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeCancel"}"#)
            .expect("trade cancel command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(cancel).expect("trade cancel maps"),
            SessionAction::Packet(ClientPacket::TradeCancel)
        ));
    }

    #[test]
    fn trade_packets_are_exposed_as_browser_events() {
        let request = super::server_packet_to_event(&ServerPacket::TradeRequest {
            name: "Scout".to_string(),
        });
        assert_eq!(request["packet"], "TradeRequest");
        assert_eq!(request["payload"]["name"], "Scout");
        // Contract: partner name is hoisted onto `partnerName`.
        assert_eq!(request["payload"]["partnerName"], "Scout");

        let accept = super::server_packet_to_event(&ServerPacket::TradeAccept {
            name: "Scout".to_string(),
        });
        assert_eq!(accept["packet"], "TradeAccept");
        assert_eq!(accept["payload"]["partnerName"], "Scout");

        let gold = super::server_packet_to_event(&ServerPacket::TradeGold { amount: 100 });
        assert_eq!(gold["packet"], "TradeGold");
        assert_eq!(gold["payload"]["amount"], 100);
        // Contract: gold is the partner's running offer.
        assert_eq!(gold["payload"]["partnerGold"], 100);

        // Contract: `partnerItems` resolves item_index -> { name, count, grade }.
        // `sample_user_item` uses item_index 321 = "MediumArmour(M)" (grade 2).
        let trade_item = super::server_packet_to_event(&ServerPacket::TradeItem {
            trade_items: vec![
                Some(sample_user_item(7, 3)),
                None,
                Some(sample_user_item(8, 1)),
            ],
        });
        assert_eq!(trade_item["packet"], "TradeItem");
        // Raw array retained for backward compatibility.
        assert!(trade_item["payload"]["tradeItems"].is_array());
        let partner_items = &trade_item["payload"]["partnerItems"];
        assert!(partner_items.is_array());
        // The empty slot is dropped; only the two real items remain.
        assert_eq!(partner_items.as_array().unwrap().len(), 2);
        assert_eq!(partner_items[0]["name"], "MediumArmour(M)");
        assert_eq!(partner_items[0]["count"], 3);
        assert_eq!(partner_items[0]["grade"], 2);

        let cancel = super::server_packet_to_event(&ServerPacket::TradeCancel { unlock: true });
        assert_eq!(cancel["packet"], "TradeCancel");
        assert_eq!(cancel["payload"]["unlock"], true);

        let confirm = super::server_packet_to_event(&ServerPacket::TradeConfirm);
        assert_eq!(confirm["packet"], "TradeConfirm");
        assert!(confirm["payload"].is_object());
    }

    #[test]
    fn market_packets_expose_contract_listing_fields() {
        // Build a ClientAuction whose item is index 321 = "MediumArmour(M)"
        // (grade 2, RequiredType.Level, RequiredAmount 16).
        let auction = ClientAuction {
            auction_id: 42,
            item: sample_user_item(99, 1),
            seller: "Merchant".to_string(),
            price: 1500,
            consignment_date_binary_datetime: 638_000_000_000_000_000,
            // MarketItemType.Auction = 2.
            item_type: 2,
        };
        let market = super::server_packet_to_event(&ServerPacket::NPCMarket {
            listings: vec![auction.clone()],
            pages: 3,
            user_mode: true,
        });
        assert_eq!(market["packet"], "NPCMarket");
        // Raw listings + page metadata retained for backward compatibility.
        assert!(market["payload"]["listings"].is_array());
        assert_eq!(market["payload"]["pages"], 3);
        assert_eq!(market["payload"]["userMode"], true);
        // Contract-shaped listing array.
        let listing = &market["payload"]["auctions"][0];
        assert_eq!(listing["id"], 42);
        assert_eq!(listing["itemName"], "MediumArmour(M)");
        assert_eq!(listing["seller"], "Merchant");
        assert_eq!(listing["price"], 1500);
        assert_eq!(listing["type"], 2);
        assert_eq!(listing["level"], 16);
        assert_eq!(listing["auction"], true);
        assert_eq!(listing["expiry"], 638_000_000_000_000_000i64);

        // The page variant carries the same enriched listing array.
        let page = super::server_packet_to_event(&ServerPacket::NPCMarketPage {
            listings: vec![auction],
        });
        assert_eq!(page["packet"], "NPCMarketPage");
        assert_eq!(
            page["payload"]["auctions"][0]["itemName"],
            "MediumArmour(M)"
        );
        // A non-auction (consign) listing reports auction=false.
        let consign = super::server_packet_to_event(&ServerPacket::NPCMarketPage {
            listings: vec![ClientAuction {
                auction_id: 7,
                item: sample_user_item(1, 1),
                seller: "For Sale".to_string(),
                price: 50,
                consignment_date_binary_datetime: 0,
                item_type: 1,
            }],
        });
        assert_eq!(consign["payload"]["auctions"][0]["auction"], false);
        // A zero consignment date is omitted (Crystal DateTime.MinValue).
        assert!(consign["payload"]["auctions"][0].get("expiry").is_none());
    }

    #[test]
    fn mail_friend_commands_map_to_crystal_protocol_packets() {
        let send = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"sendMail","name":"Blade","message":"Delivery","gold":2500,"itemsIdx":[7,8,0,0,0],"stamped":true}"#,
        )
        .expect("send mail command should deserialize");
        match super::browser_command_to_action(send).expect("send mail maps") {
            SessionAction::Packet(ClientPacket::SendMail {
                name,
                message,
                gold,
                items_idx,
                stamped,
            }) => {
                assert_eq!(name, "Blade");
                assert_eq!(message, "Delivery");
                assert_eq!(gold, 2_500);
                assert_eq!(items_idx, [7, 8, 0, 0, 0]);
                assert!(stamped);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let lock = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"mailLockedItem","uniqueId":7,"locked":true}"#,
        )
        .expect("mail locked item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(lock).expect("mail lock maps"),
            SessionAction::Packet(ClientPacket::MailLockedItem {
                unique_id: 7,
                locked: true
            })
        ));

        let cost = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"mailCost","gold":2500,"itemsIdx":[7,8,0,0,0],"stamped":false}"#,
        )
        .expect("mail cost command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(cost).expect("mail cost maps"),
            SessionAction::Packet(ClientPacket::MailCost {
                gold: 2_500,
                items_idx: [7, 8, 0, 0, 0],
                stamped: false
            })
        ));

        let friend = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"addFriend","name":"Blade","blocked":false}"#,
        )
        .expect("add friend command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(friend).expect("add friend maps"),
            SessionAction::Packet(ClientPacket::AddFriend {
                ref name,
                blocked: false
            }) if name == "Blade"
        ));

        let memo = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"addMemo","characterIndex":42,"memo":"party lead"}"#,
        )
        .expect("add memo command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(memo).expect("add memo maps"),
            SessionAction::Packet(ClientPacket::AddMemo {
                character_index: 42,
                ref memo
            }) if memo == "party lead"
        ));

        let ranking = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"getRanking","rankType":3,"rankIndex":20,"onlineOnly":true}"#,
        )
        .expect("get ranking command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(ranking).expect("ranking maps"),
            SessionAction::Packet(ClientPacket::GetRanking {
                rank_type: 3,
                rank_index: 20,
                online_only: true,
            })
        ));
    }

    #[test]
    fn mail_friend_packets_are_exposed_as_browser_events() {
        let receive = super::server_packet_to_event(&ServerPacket::ReceiveMail {
            mail: vec![ClientMail {
                mail_id: 11,
                sender_name: "GM".to_string(),
                message: "Welcome".to_string(),
                opened: false,
                locked: true,
                can_reply: false,
                collected: false,
                date_sent_binary_datetime: 638000000000000000,
                gold: 2_500,
                items: vec![sample_user_item(7, 1)],
            }],
        });
        assert_eq!(receive["packet"], "ReceiveMail");
        assert_eq!(receive["payload"]["mail"][0]["senderName"], "GM");
        assert_eq!(receive["payload"]["mail"][0]["items"][0]["unique_id"], 7);

        let sent = super::server_packet_to_event(&ServerPacket::MailSent { result: -1 });
        assert_eq!(sent["packet"], "MailSent");
        assert_eq!(sent["payload"]["result"], -1);

        let cost = super::server_packet_to_event(&ServerPacket::MailCost { cost: 200 });
        assert_eq!(cost["packet"], "MailCost");
        assert_eq!(cost["payload"]["cost"], 200);

        let friends = super::server_packet_to_event(&ServerPacket::FriendUpdate {
            friends: vec![
                ClientFriend {
                    index: 42,
                    name: "Blade".to_string(),
                    memo: "party lead".to_string(),
                    blocked: false,
                    online: true,
                },
                ClientFriend {
                    index: 43,
                    name: "Griefer".to_string(),
                    memo: "ignored".to_string(),
                    blocked: true,
                    online: false,
                },
            ],
        });
        assert_eq!(friends["packet"], "FriendUpdate");
        // Roster is split: non-blocked entries in `friends`, blocked in `blocked`.
        assert_eq!(friends["payload"]["friends"][0]["name"], "Blade");
        assert_eq!(friends["payload"]["friends"][0]["online"], true);
        assert_eq!(friends["payload"]["friends"][0]["memo"], "party lead");
        assert_eq!(friends["payload"]["friends"].as_array().unwrap().len(), 1);
        assert_eq!(friends["payload"]["blocked"][0]["name"], "Griefer");
        assert_eq!(friends["payload"]["blocked"][0]["memo"], "ignored");
        assert_eq!(friends["payload"]["blocked"].as_array().unwrap().len(), 1);
        // Friends with an empty memo omit the field entirely.
        let no_memo = super::server_packet_to_event(&ServerPacket::FriendUpdate {
            friends: vec![ClientFriend {
                index: 1,
                name: "Plain".to_string(),
                memo: String::new(),
                blocked: false,
                online: true,
            }],
        });
        assert!(no_memo["payload"]["friends"][0].get("memo").is_none());
    }

    #[test]
    fn intelligent_creature_commands_map_to_crystal_protocol_packets() {
        let request = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"requestIntelligentCreatureUpdates","update":true}"#,
        )
        .expect("request intelligent creature command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(request).expect("request maps"),
            SessionAction::Packet(ClientPacket::RequestIntelligentCreatureUpdates { update: true })
        ));

        let pickup = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"intelligentCreaturePickup","mouseMode":true,"location":{"x":330,"y":270}}"#,
        )
        .expect("intelligent creature pickup command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(pickup).expect("pickup maps"),
            SessionAction::Packet(ClientPacket::IntelligentCreaturePickup {
                mouse_mode: true,
                location: Point { x: 330, y: 270 }
            })
        ));

        let update = serde_json::from_value::<BrowserCommand>(serde_json::json!({
            "type": "updateIntelligentCreature",
            "creature": sample_intelligent_creature(0),
            "summonMe": true,
            "unsummonMe": false,
            "releaseMe": false
        }))
        .expect("update intelligent creature command should deserialize");
        match super::browser_command_to_action(update).expect("update maps") {
            SessionAction::Packet(ClientPacket::UpdateIntelligentCreature {
                creature,
                summon_me,
                unsummon_me,
                release_me,
            }) => {
                assert_eq!(creature.custom_name, "Buddy");
                assert!(summon_me);
                assert!(!unsummon_me);
                assert!(!release_me);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn intelligent_creature_packets_are_exposed_as_browser_events() {
        let update = super::server_packet_to_event(&ServerPacket::UpdateIntelligentCreatureList {
            creature_list: vec![sample_intelligent_creature(0)],
            creature_summoned: true,
            summoned_creature_type: 1,
            pearl_count: 3,
        });
        assert_eq!(update["packet"], "UpdateIntelligentCreatureList");
        assert_eq!(update["payload"]["creatureList"][0]["customName"], "Buddy");
        assert_eq!(update["payload"]["creatureSummoned"], true);
        assert_eq!(update["payload"]["pearlCount"], 3);

        let rename = super::server_packet_to_event(&ServerPacket::IntelligentCreatureEnableRename);
        assert_eq!(rename["packet"], "IntelligentCreatureEnableRename");

        let pickup = super::server_packet_to_event(&ServerPacket::IntelligentCreaturePickup {
            object_id: 1_001,
        });
        assert_eq!(pickup["packet"], "IntelligentCreaturePickup");
        assert_eq!(pickup["payload"]["objectId"], 1_001);
    }

    #[test]
    fn mount_and_fishing_packets_are_exposed_as_browser_events() {
        let mount = super::server_packet_to_event(&ServerPacket::MountUpdate {
            object_id: 1_001,
            mount_type: 12,
            riding_mount: true,
        });
        assert_eq!(mount["packet"], "MountUpdate");
        assert_eq!(mount["payload"]["mountType"], 12);
        assert_eq!(mount["payload"]["ridingMount"], true);

        let fishing = super::server_packet_to_event(&ServerPacket::FishingUpdate {
            object_id: 1_001,
            fishing: true,
            progress_percent: 33,
            chance_percent: 12,
            fishing_point: Point { x: 331, y: 270 },
            found_fish: false,
        });
        assert_eq!(fishing["packet"], "FishingUpdate");
        assert_eq!(fishing["payload"]["progressPercent"], 33);
        assert_eq!(fishing["payload"]["fishingPoint"]["x"], 331);
    }

    #[test]
    fn cast_skill_command_accepts_key_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"castSkill","key":"battle-focus"}"#)
                .expect("cast skill command should deserialize");

        match command {
            BrowserCommand::CastSkill { key } => assert_eq!(key, "battle-focus"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn transfer_map_command_accepts_key_field() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"transferMap","key":"starter-east-field-gate"}"#,
        )
        .expect("transfer map command should deserialize");

        match command {
            BrowserCommand::TransferMap { key } => assert_eq!(key, "starter-east-field-gate"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn stage5_command_accepts_action_and_args() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"stage5Command","action":"guild.create","args":["Bichon"]}"#,
        )
        .expect("stage5 command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("stage5 command should map to a session action");

        match action {
            SessionAction::Stage5Command { action, args } => {
                assert_eq!(action, "guild.create");
                assert_eq!(args, vec!["Bichon"]);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn stage5_damage_equipment_browser_command_returns_dura_changed_event() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let before_current_dura = session
            .world_snapshot()
            .equipment_items
            .iter()
            .find(|item| item.slot == mir2_simulation::EquipmentSlot::Weapon)
            .expect("starter weapon should be equipped")
            .durability_current;
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"stage5Command","action":"qa.damageEquipment","args":["weapon","2500"]}"#,
        )
        .expect("stage5 damage equipment command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("stage5 damage equipment command should map to a session action");

        let responses = super::execute_session_action(&mut session, action, false, false)
            .expect("stage5 damage equipment command should execute");
        let damage_packet = responses
            .iter()
            .find(|packet| matches!(packet, ServerPacket::DuraChanged { .. }))
            .expect("stage5 damage equipment should emit DuraChanged");
        let event = super::server_packet_to_event(damage_packet);

        assert_eq!(event["packet"], "DuraChanged");
        assert_eq!(event["payload"]["uniqueId"], 0);
        assert_eq!(
            event["payload"]["currentDura"].as_u64(),
            Some(u64::from(before_current_dura.saturating_sub(2_500)))
        );
    }

    #[test]
    fn production_web_path_rejects_unauthenticated_start_game() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());

        let error = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::StartGame { character_index: 0 }),
            false,
            true,
        )
        .expect_err("production web path should reject unauthenticated StartGame");

        assert!(error.contains("authenticated account is required"));
    }

    #[test]
    fn socket_serial_guard_blocks_zone_bypass_until_action_finishes() {
        let pending = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let bytes = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        {
            let mut queued = super::QueuedSocketInput::new(
                super::ParsedSocketInput::Action(SessionAction::Tick),
                std::sync::Arc::clone(&pending),
                bytes
                    .clone()
                    .try_acquire_owned()
                    .expect("test input byte permit should be available"),
            );
            assert_eq!(pending.load(std::sync::atomic::Ordering::Acquire), 1);
            assert_eq!(bytes.available_permits(), 0);
            assert!(matches!(
                queued.take_input(),
                super::ParsedSocketInput::Action(SessionAction::Tick)
            ));
            assert_eq!(pending.load(std::sync::atomic::Ordering::Acquire), 1);
        }
        assert_eq!(pending.load(std::sync::atomic::Ordering::Acquire), 0);
        assert_eq!(bytes.available_permits(), 1);
    }

    #[test]
    fn zone_movement_reader_bypass_only_accepts_walk_run_and_turn_packets() {
        for action in [
            SessionAction::Packet(ClientPacket::Walk {
                direction: MirDirection::Right,
            }),
            SessionAction::Packet(ClientPacket::Run {
                direction: MirDirection::Right,
            }),
            SessionAction::Packet(ClientPacket::Turn {
                direction: MirDirection::Right,
            }),
        ] {
            assert!(super::zone_movement_packet_for_action(&action).is_some());
        }
        assert!(
            super::zone_movement_packet_for_action(&SessionAction::MoveTo {
                x: 330,
                y: 270,
                running: false,
            })
            .is_none()
        );
        assert!(super::zone_movement_packet_for_action(&SessionAction::Tick).is_none());
    }

    #[test]
    fn runtime_tick_defers_after_bootstrap_but_not_player_movement() {
        assert!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::StartGame { character_index: 0 },
            ))
            .is_some()
        );
        assert_eq!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::Walk {
                    direction: MirDirection::Right,
                },
            )),
            Some(std::time::Duration::from_millis(75))
        );
        assert_eq!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::Run {
                    direction: MirDirection::Right,
                },
            )),
            Some(std::time::Duration::from_millis(75))
        );
        assert_eq!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::Turn {
                    direction: MirDirection::Right,
                },
            )),
            Some(std::time::Duration::from_millis(75))
        );
        assert!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::Chat {
                    message: "hello".to_string(),
                    linked_items: Vec::new(),
                },
            ))
            .is_none()
        );
    }

    #[test]
    fn successful_identity_packets_build_gate15_write_through_batches() {
        let login_batch = super::gate15_identity_batch_for_responses(
            None,
            Some("fresh"),
            &[ServerPacket::LoginSuccess {
                characters: Vec::new(),
            }],
        )
        .expect("login identity batch should parse")
        .expect("login success should finalize the account");
        assert_eq!(login_batch.account_id, "fresh");
        assert!(login_batch.characters.is_empty());

        let character_batch = super::gate15_identity_batch_for_responses(
            Some("fresh"),
            None,
            &[ServerPacket::NewCharacterSuccess {
                char_info: SelectInfo {
                    index: 1878,
                    name: "AlphaHero".to_string(),
                    level: 1,
                    class: MirClass::Warrior,
                    gender: MirGender::Male,
                    last_access_binary_datetime: 0,
                },
            }],
        )
        .expect("character identity batch should parse")
        .expect("character success should finalize the character");
        assert_eq!(character_batch.account_id, "fresh");
        assert_eq!(
            character_batch.characters,
            vec![(1878, "AlphaHero".to_string())]
        );
    }

    #[test]
    fn production_web_path_allows_start_game_after_login_success() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());

        let login_packets = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }),
            false,
            true,
        )
        .expect("login should execute on production web path");
        let authenticated = super::update_authenticated_state(false, &login_packets);
        assert!(authenticated);

        let start_packets = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::StartGame { character_index: 0 }),
            authenticated,
            true,
        )
        .expect("authenticated StartGame should execute");

        assert!(start_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
    }

    #[test]
    fn capacity_state_rejects_websocket_connections_over_limit() {
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            Some(1),
            None,
            None,
        ));
        let permit = capacity
            .try_acquire_ws_connection()
            .expect("first websocket should fit capacity");

        let error = capacity
            .try_acquire_ws_connection()
            .expect_err("second websocket should be rejected");
        assert!(error.contains("WebSocket connection capacity reached"));
        assert_eq!(capacity.status().current_ws_connections, 1);

        drop(permit);
        assert_eq!(capacity.status().current_ws_connections, 0);
    }

    #[test]
    fn capacity_state_tracks_active_sessions_and_reconnect_leases() {
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        let active = capacity
            .try_acquire_active_session()
            .expect("first active session should fit capacity");
        let reconnect = capacity
            .try_acquire_reconnect_lease()
            .expect("first reconnect lease should fit capacity");

        assert!(capacity.try_acquire_active_session().is_err());
        assert!(capacity.try_acquire_reconnect_lease().is_err());
        assert_eq!(capacity.status().current_active_sessions, 1);
        assert_eq!(capacity.status().current_reconnect_leases, 1);

        drop(active);
        drop(reconnect);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
    }

    #[test]
    fn production_capacity_defaults_are_finite_and_environment_overrides_remain_explicit() {
        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", Some("production")),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                ("MIR2_GATEWAY_MAX_WS_CONNECTIONS", None),
                ("MIR2_GATEWAY_MAX_ACTIVE_SESSIONS", None),
                ("MIR2_GATEWAY_MAX_RECONNECT_LEASES", None),
            ],
            || {
                let status = super::GatewayCapacityState::from_env().status();
                assert_eq!(
                    status.max_ws_connections,
                    Some(super::DEFAULT_PRODUCTION_MAX_WS_CONNECTIONS)
                );
                assert_eq!(
                    status.max_active_sessions,
                    Some(super::DEFAULT_PRODUCTION_MAX_ACTIVE_SESSIONS)
                );
                assert_eq!(
                    status.max_reconnect_leases,
                    Some(super::DEFAULT_PRODUCTION_MAX_RECONNECT_LEASES)
                );
            },
        );

        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", Some("staging")),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                ("MIR2_GATEWAY_MAX_WS_CONNECTIONS", Some("111")),
                ("MIR2_GATEWAY_MAX_ACTIVE_SESSIONS", Some("77")),
                ("MIR2_GATEWAY_MAX_RECONNECT_LEASES", Some("55")),
            ],
            || {
                let status = super::GatewayCapacityState::from_env().status();
                assert_eq!(status.max_ws_connections, Some(111));
                assert_eq!(status.max_active_sessions, Some(77));
                assert_eq!(status.max_reconnect_leases, Some(55));
            },
        );
    }

    #[test]
    fn development_capacity_policy_is_unlimited_unless_explicitly_configured() {
        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", Some("development")),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                ("MIR2_GATEWAY_MAX_WS_CONNECTIONS", None),
                ("MIR2_GATEWAY_MAX_ACTIVE_SESSIONS", None),
                ("MIR2_GATEWAY_MAX_RECONNECT_LEASES", None),
            ],
            || {
                let status = super::GatewayCapacityState::from_env().status();
                assert_eq!(status.max_ws_connections, None);
                assert_eq!(status.max_active_sessions, None);
                assert_eq!(status.max_reconnect_leases, None);
            },
        );
    }

    #[test]
    fn websocket_frame_message_and_input_queue_memory_bounds_are_constant_and_finite() {
        assert_eq!(super::WEBSOCKET_MAX_FRAME_BYTES, 64 * 1024);
        assert_eq!(
            super::WEBSOCKET_MAX_MESSAGE_BYTES,
            super::WEBSOCKET_MAX_FRAME_BYTES
        );
        assert_eq!(super::SOCKET_INPUT_CAPACITY, 256);
        assert_eq!(
            super::SOCKET_INPUT_MAX_BUFFERED_BYTES,
            super::WEBSOCKET_MAX_FRAME_BYTES * super::SOCKET_INPUT_CAPACITY
        );
        assert_eq!(super::SOCKET_INPUT_MAX_BUFFERED_BYTES, 16 * 1024 * 1024);

        let budget = Arc::new(tokio::sync::Semaphore::new(
            super::SOCKET_INPUT_MAX_BUFFERED_BYTES,
        ));
        let full = Arc::clone(&budget)
            .try_acquire_many_owned(super::SOCKET_INPUT_MAX_BUFFERED_BYTES as u32)
            .expect("the queue can account for its exact declared byte budget");
        assert!(Arc::clone(&budget).try_acquire_owned().is_err());
        drop(full);
        let one_frame = Arc::clone(&budget)
            .try_acquire_many_owned(super::WEBSOCKET_MAX_FRAME_BYTES as u32)
            .expect("dropping queued input must release its byte permit");
        assert_eq!(
            budget.available_permits(),
            super::SOCKET_INPUT_MAX_BUFFERED_BYTES - super::WEBSOCKET_MAX_FRAME_BYTES
        );
        drop(one_frame);
        assert_eq!(
            budget.available_permits(),
            super::SOCKET_INPUT_MAX_BUFFERED_BYTES
        );
    }

    #[test]
    fn capacity_state_tracks_account_command_inflight_limits() {
        let capacity = Arc::new(super::GatewayCapacityState::with_action_limits(
            Some(1),
            Some(1),
            Some(1),
        ));
        let login = capacity
            .try_acquire_action(super::GatewayCapacityKind::Login)
            .expect("first login should fit capacity");
        let new_character = capacity
            .try_acquire_action(super::GatewayCapacityKind::NewCharacter)
            .expect("first new-character should fit capacity");
        let start_game = capacity
            .try_acquire_action(super::GatewayCapacityKind::StartGame)
            .expect("first StartGame should fit capacity");

        assert!(capacity
            .try_acquire_action(super::GatewayCapacityKind::Login)
            .expect_err("second login should be rejected")
            .contains("login in-flight capacity reached"));
        assert!(capacity
            .try_acquire_action(super::GatewayCapacityKind::NewCharacter)
            .expect_err("second new-character should be rejected")
            .contains("new-character in-flight capacity reached"));
        assert!(capacity
            .try_acquire_action(super::GatewayCapacityKind::StartGame)
            .expect_err("second StartGame should be rejected")
            .contains("StartGame in-flight capacity reached"));
        assert_eq!(capacity.status().current_login_in_flight, 1);
        assert_eq!(capacity.status().current_new_character_in_flight, 1);
        assert_eq!(capacity.status().current_start_game_in_flight, 1);

        drop(login);
        drop(new_character);
        drop(start_game);
        assert_eq!(capacity.status().current_login_in_flight, 0);
        assert_eq!(capacity.status().current_new_character_in_flight, 0);
        assert_eq!(capacity.status().current_start_game_in_flight, 0);
    }

    #[tokio::test]
    async fn action_capacity_waits_for_a_released_slot() {
        let capacity = Arc::new(super::GatewayCapacityState::with_action_limits(
            None,
            None,
            Some(1),
        ));
        let first = capacity
            .try_acquire_action(super::GatewayCapacityKind::StartGame)
            .expect("first StartGame should fit capacity");
        let waiting_capacity = Arc::clone(&capacity);
        let waiting = tokio::spawn(async move {
            waiting_capacity
                .acquire_action_with_wait(
                    super::GatewayCapacityKind::StartGame,
                    std::time::Duration::from_millis(500),
                )
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(!waiting.is_finished());
        drop(first);
        let second = waiting
            .await
            .expect("capacity waiter should not panic")
            .expect("capacity waiter should acquire the released slot");
        assert_eq!(capacity.status().current_start_game_in_flight, 1);
        drop(second);
        assert_eq!(capacity.status().current_start_game_in_flight, 0);
    }

    #[test]
    fn web_session_save_queue_debounces_until_due_or_limit() {
        let start = std::time::Instant::now();
        let mut saves = 0;
        let mut queue = super::WebSessionSaveQueue::new(super::GatewaySaveQueueConfig::new(
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(30),
            3,
        ));

        queue
            .request_save(start, || {
                saves += 1;
                Ok(())
            })
            .expect("first save request should queue");
        queue
            .request_save(start + std::time::Duration::from_secs(1), || {
                saves += 1;
                Ok(())
            })
            .expect("second save request should still debounce");
        assert_eq!(saves, 0);
        assert!(queue.has_pending_save());

        queue
            .request_save(start + std::time::Duration::from_secs(2), || {
                saves += 1;
                Ok(())
            })
            .expect("queue limit should flush");
        assert_eq!(saves, 1);
        assert!(!queue.has_pending_save());
    }

    #[test]
    fn web_session_save_queue_flushes_on_checkpoint_and_close() {
        let start = std::time::Instant::now();
        let mut saves = 0;
        let mut queue = super::WebSessionSaveQueue::new(super::GatewaySaveQueueConfig::new(
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(20),
            8,
        ));

        queue
            .request_save(start, || {
                saves += 1;
                Ok(())
            })
            .expect("save request should queue");
        queue
            .checkpoint(start + std::time::Duration::from_secs(10), || {
                saves += 1;
                Ok(())
            })
            .expect("early checkpoint should not flush");
        assert_eq!(saves, 0);

        queue
            .checkpoint(start + std::time::Duration::from_secs(21), || {
                saves += 1;
                Ok(())
            })
            .expect("due checkpoint should flush");
        assert_eq!(saves, 1);

        queue
            .request_save(start + std::time::Duration::from_secs(21), || {
                saves += 1;
                Ok(())
            })
            .expect("save request should queue again");
        queue
            .flush_now(start + std::time::Duration::from_secs(22), || {
                saves += 1;
                Ok(())
            })
            .expect("close flush should persist dirty state");
        assert_eq!(saves, 2);
        assert!(!queue.has_pending_save());

        queue
            .force_save_now(start + std::time::Duration::from_secs(23), || {
                saves += 1;
                Ok(())
            })
            .expect("forced close save should run even without dirty state");
        assert_eq!(saves, 3);
    }

    #[test]
    fn web_session_route_refresh_throttles_low_latency_updates() {
        let start = Instant::now();
        let cache = crate::InMemoryGatewaySessionCache::default();
        let session = demo_game_session();
        let mut refresh = super::WebSessionRouteRefresh::new(
            super::GatewayRouteRefreshConfig::new(Duration::from_secs(5)),
        );

        assert!(refresh
            .maybe_refresh(&cache, &session, start, false)
            .expect("initial active session refresh should succeed"));
        assert_eq!(crate::GatewaySessionCache::route_lease_count(&cache), 1);
        assert!(!refresh
            .maybe_refresh(&cache, &session, start + Duration::from_secs(1), false)
            .expect("early refresh should be skipped"));
        assert!(refresh
            .maybe_refresh(&cache, &session, start + Duration::from_secs(1), true)
            .expect("forced refresh should bypass throttle"));
        assert!(!refresh
            .maybe_refresh(&cache, &session, start + Duration::from_secs(3), false)
            .expect("throttle should use the latest forced refresh time"));
        assert!(refresh
            .maybe_refresh(&cache, &session, start + Duration::from_secs(7), false)
            .expect("refresh should run again after the interval"));
    }

    #[test]
    fn reconnect_session_store_restores_active_session_before_grace_expires() {
        let store = super::ReconnectSessionStore::default();
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        let session = demo_game_session();
        let session_id = session.session_id().to_string();
        let key = super::session_cache_key(&session)
            .expect("active demo game session should have a cache key");
        let active_session_permit = capacity
            .try_acquire_active_session()
            .expect("test active session should fit capacity");
        let reconnect_lease_permit = capacity
            .try_acquire_reconnect_lease()
            .expect("test reconnect lease should fit capacity");

        store.store(
            key.clone(),
            session,
            Some(active_session_permit),
            reconnect_lease_permit,
            None,
            std::time::Duration::from_secs(30),
        );
        assert_eq!(store.len(), 1);
        assert_eq!(capacity.status().current_active_sessions, 1);
        assert_eq!(capacity.status().current_reconnect_leases, 1);

        let restored = store
            .take(&key)
            .expect("stored reconnect session should be restored within grace");
        assert_eq!(restored.session.session_id(), session_id);
        assert_eq!(
            restored
                .session
                .active_identity()
                .expect("restored session should remain active")
                .account_id,
            "demo"
        );
        assert!(restored.active_session_permit.is_some());
        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);

        drop(restored);
        assert_eq!(capacity.status().current_active_sessions, 0);
    }

    #[test]
    fn reconnect_session_store_discards_expired_sessions() {
        let store = super::ReconnectSessionStore::default();
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        let session = demo_game_session();
        let key = super::session_cache_key(&session)
            .expect("active demo game session should have a cache key");
        let active_session_permit = capacity
            .try_acquire_active_session()
            .expect("test active session should fit capacity");
        let reconnect_lease_permit = capacity
            .try_acquire_reconnect_lease()
            .expect("test reconnect lease should fit capacity");
        {
            let mut state = store
                .state
                .lock()
                .expect("test reconnect store mutex should not be poisoned");
            state.sessions.insert(
                key.clone(),
                super::ReconnectSessionLease {
                    session,
                    active_session_permit: Some(active_session_permit),
                    _reconnect_lease_permit: reconnect_lease_permit,
                    resume_family_id: None,
                    expires_at: std::time::Instant::now() - std::time::Duration::from_secs(1),
                },
            );
        }
        assert_eq!(capacity.status().current_active_sessions, 1);
        assert_eq!(capacity.status().current_reconnect_leases, 1);

        assert!(store.take(&key).is_none());
        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
    }

    #[tokio::test]
    async fn reconnect_session_store_scheduled_purge_discards_expired_sessions() {
        let store = Arc::new(super::ReconnectSessionStore::default());
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        let session = demo_game_session();
        let key = super::session_cache_key(&session)
            .expect("active demo game session should have a cache key");
        let active_session_permit = capacity
            .try_acquire_active_session()
            .expect("test active session should fit capacity");
        let reconnect_lease_permit = capacity
            .try_acquire_reconnect_lease()
            .expect("test reconnect lease should fit capacity");

        store.store(
            key,
            session,
            Some(active_session_permit),
            reconnect_lease_permit,
            None,
            std::time::Duration::from_millis(10),
        );
        super::schedule_reconnect_session_purge(
            Arc::clone(&store),
            std::time::Duration::from_millis(10),
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
    }

    #[test]
    fn native_resume_json_is_explicit_strict_and_secret_redacted() {
        let capabilities = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"clientCapabilities","capabilities":["nativeResumeV1"]}"#,
        )
        .expect("native capabilities should deserialize");
        assert!(matches!(
            capabilities,
            BrowserCommand::ClientCapabilities { capabilities }
                if capabilities == ["nativeResumeV1"]
        ));

        let secret = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let resume = serde_json::from_str::<BrowserCommand>(&format!(
            r#"{{"type":"resumeSession","credential":"{secret}"}}"#
        ))
        .expect("credential-only resume request should deserialize");
        assert!(!format!("{resume:?}").contains(secret));
        assert!(matches!(resume, BrowserCommand::ResumeSession(_)));
        for forbidden in [
            format!(r#"{{"type":"resumeSession","credential":"{secret}","accountId":"demo"}}"#),
            format!(r#"{{"type":"resumeSession","credential":"{secret}","characterIndex":0}}"#),
        ] {
            let error = serde_json::from_str::<BrowserCommand>(&forbidden).expect_err(
                "resume must derive account and character exclusively from the credential",
            );
            assert!(matches!(
                super::invalid_browser_command_input(&forbidden, &error),
                super::ParsedSocketInput::ResumeRejected
            ));
        }

        for malformed in [
            "A".repeat(42),
            "A".repeat(44),
            format!("{}=", "A".repeat(42)),
            format!("{}+", "A".repeat(42)),
            "A".repeat(super::WEBSOCKET_MAX_MESSAGE_BYTES),
        ] {
            let input = format!(r#"{{"type":"resumeSession","credential":"{malformed}"}}"#);
            let error = serde_json::from_str::<BrowserCommand>(&input)
                .expect_err("malformed credentials must fail during command construction");
            assert!(matches!(
                super::invalid_browser_command_input(&input, &error),
                super::ParsedSocketInput::ResumeRejected
            ));
        }
    }

    #[test]
    fn native_resume_control_never_maps_to_simulation_auth_or_unsafe_actions() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"resumeSession","credential":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#,
        )
        .expect("credential-only resume request should deserialize");
        assert!(super::browser_command_to_action(command).is_err());
    }

    #[test]
    fn native_resume_server_event_contracts_are_exact_and_rejection_is_uniform() {
        let store = super::ReconnectSessionStore::default();
        let nonce = crate::resume::ResumeConnectionNonce::generate();
        let issued = store
            .issue_resume_credential(
                None,
                crate::resume::ResumeIssueContext {
                    account_id: "demo",
                    character_index: 0,
                    gateway_session_id: "gateway-session",
                    identity_session_id: "identity-session",
                    identity_expires_at_ms: super::gateway_unix_ms() + 60_000,
                    source_connection_nonce: &nonce,
                },
                super::gateway_unix_ms(),
                4,
                || true,
            )
            .expect("test identity should be active while issuing the credential");
        let credential = super::resume_credential_event(&issued);
        assert_eq!(credential["type"], "resumeCredential");
        assert_eq!(credential["protocol"], "nativeResumeV1");
        assert_eq!(credential["generation"], 4);
        assert_eq!(
            credential["credential"].as_str(),
            Some(issued.credential.as_str())
        );
        assert!(credential.get("accountId").is_none());
        assert!(credential.get("characterIndex").is_none());

        let resumed = super::session_resumed_event(3, 5);
        assert_eq!(resumed["type"], "sessionResumed");
        assert_eq!(resumed["protocol"], "nativeResumeV1");
        assert_eq!(resumed["characterIndex"], 3);
        assert_eq!(resumed["generation"], 5);

        assert_eq!(
            super::resume_rejected_event(),
            json!({"type":"resumeRejected","code":"unavailable"})
        );
    }

    #[test]
    fn non_opted_in_web_never_reaches_resume_issuance_and_map_change_forces_rotation() {
        let now_ms = super::gateway_unix_ms();
        let mut state = super::NativeResumeConnectionState::new();
        assert!(!state.should_rotate(now_ms, true));
        state.opted_in = true;
        state.last_issued_at_ms = Some(now_ms);
        assert!(!state.should_rotate(now_ms, false));
        let map_changed = ServerPacket::MapChanged {
            map_index: 8,
            file_name: "D1801".into(),
            title: "PenalCavern".into(),
            mini_map: 0,
            big_map: 0,
            lights: 4,
            location: Point { x: 12, y: 34 },
            direction: MirDirection::Down,
            map_dark_light: 1,
            music: 0,
            weather: 64,
        };
        assert!(super::responses_require_resume_rotation(&[map_changed]));
        assert!(state.should_rotate(now_ms, true));
        assert!(!super::responses_require_resume_rotation(&[
            ServerPacket::KeepAlive { time: 1 }
        ]));
    }

    #[test]
    fn native_resume_atomically_restores_bound_session_and_replay_fails() {
        let store = super::ReconnectSessionStore::default();
        let (identity, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
        let session_cache = crate::InMemoryGatewaySessionCache::default();
        let (restored, consumed, resumed_identity) =
            super::validate_and_commit_native_resume_for_test(
                &store,
                &session_cache,
                &identity,
                &credential,
                super::gateway_unix_ms(),
            )
            .expect("active bound credential should restore without Login or PasskeyLogin");
        assert_eq!(consumed, binding);
        assert_eq!(restored.session.session_id(), binding.gateway_session_id);
        assert_eq!(resumed_identity.account_id, "demo");
        assert_eq!(resumed_identity.session_id, binding.identity_session_id);
        assert!(restored.active_session_permit.is_some());
        assert_eq!(store.len(), 0);
        assert!(
            super::validate_and_commit_native_resume_for_test(
                &store,
                &session_cache,
                &identity,
                &credential,
                super::gateway_unix_ms(),
            )
            .is_none(),
            "the same credential must not replay"
        );
    }

    #[test]
    fn native_resume_route_failure_rolls_back_and_exact_token_retries_without_orphan_permits() {
        let store = super::ReconnectSessionStore::default();
        let (identity, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
        let capacity = store
            .capacity_state_for_binding(&binding)
            .expect("fixture lease should expose capacity state");
        let cache = crate::InMemoryGatewaySessionCache::default();
        let last_seen_before = identity
            .list_sessions(&verified)
            .expect("test identity list should be readable")
            .into_iter()
            .find(|session| session.session_id == verified.session_id)
            .expect("issued identity session should be listed")
            .last_seen_at_ms;

        let failure = match super::validate_and_prepare_native_resume(
            &store,
            &cache,
            &identity,
            &credential,
            super::gateway_unix_ms(),
            |_| Err("injected route renew failure".to_string()),
            |_| Ok::<(), String>(()),
        ) {
            Err(error) => error,
            Ok(_) => panic!("injected route failure must reject the first attempt"),
        };
        assert!(matches!(failure, super::NativeResumePrepareError::Route(_)));
        assert_eq!(store.len(), 1);
        assert!(store
            .resume_binding(&credential, super::gateway_unix_ms())
            .is_some());
        assert_eq!(capacity.status().current_active_sessions, 1);
        assert_eq!(capacity.status().current_reconnect_leases, 1);
        let last_seen_after = identity
            .list_sessions(&verified)
            .expect("test identity list should remain readable")
            .into_iter()
            .find(|session| session.session_id == verified.session_id)
            .expect("issued identity session should remain listed")
            .last_seen_at_ms;
        assert_eq!(
            last_seen_after, last_seen_before,
            "resume validation must be read-only before commit"
        );

        let (reservation, _, ()) = super::validate_and_prepare_native_resume(
            &store,
            &cache,
            &identity,
            &credential,
            super::gateway_unix_ms(),
            |_| Ok(()),
            |_| Ok(()),
        )
        .expect("exact token must remain retryable");
        let (restored, consumed) = store
            .commit_resume(reservation, &credential, super::gateway_unix_ms())
            .expect("second attempt should commit");
        assert_eq!(consumed, binding);
        assert_eq!(store.len(), 0);
        assert!(store
            .resume_binding(&credential, super::gateway_unix_ms())
            .is_none());
        assert_eq!(capacity.status().current_reconnect_leases, 0);
        assert_eq!(capacity.status().current_active_sessions, 1);
        drop(restored);
        assert_eq!(capacity.status().current_active_sessions, 0);
    }

    #[test]
    fn native_resume_zone_failure_rolls_back_and_second_attempt_commits() {
        let store = super::ReconnectSessionStore::default();
        let (identity, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
        let capacity = store
            .capacity_state_for_binding(&binding)
            .expect("fixture lease should expose capacity state");
        let cache = crate::InMemoryGatewaySessionCache::default();

        let failure = match super::validate_and_prepare_native_resume(
            &store,
            &cache,
            &identity,
            &credential,
            super::gateway_unix_ms(),
            |_| Ok(()),
            |_| Err::<(), String>("injected Zone registration failure".to_string()),
        ) {
            Err(error) => error,
            Ok(_) => panic!("injected Zone failure must reject the first attempt"),
        };
        assert!(matches!(failure, super::NativeResumePrepareError::Zone(_)));
        assert_eq!(store.len(), 1);
        assert!(store
            .resume_binding(&credential, super::gateway_unix_ms())
            .is_some());
        assert_eq!(capacity.status().current_active_sessions, 1);
        assert_eq!(capacity.status().current_reconnect_leases, 1);

        let (reservation, _, ()) = super::validate_and_prepare_native_resume(
            &store,
            &cache,
            &identity,
            &credential,
            super::gateway_unix_ms(),
            |_| Ok(()),
            |_| Ok(()),
        )
        .expect("exact token must remain retryable");
        let (restored, consumed) = store
            .commit_resume(reservation, &credential, super::gateway_unix_ms())
            .expect("second attempt should commit");
        assert_eq!(consumed, binding);
        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
        drop(restored);
        assert_eq!(capacity.status().current_active_sessions, 0);
    }

    #[test]
    fn native_resume_identity_revoked_during_prepare_cannot_commit_or_retry_and_drops_resources() {
        struct PreparedZoneDrop(Arc<std::sync::atomic::AtomicBool>);

        impl Drop for PreparedZoneDrop {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::Release);
            }
        }

        let store = super::ReconnectSessionStore::default();
        let (identity, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
        let capacity = store
            .capacity_state_for_binding(&binding)
            .expect("fixture lease should expose capacity state");
        let cache = crate::InMemoryGatewaySessionCache::default();
        let prepared_dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let prepared_drop_signal = Arc::clone(&prepared_dropped);

        let (reservation, prepared_identity, prepared_zone) =
            super::validate_and_prepare_native_resume(
                &store,
                &cache,
                &identity,
                &credential,
                super::gateway_unix_ms(),
                |_| Ok(()),
                |_| {
                    identity
                        .revoke_session(
                            &verified,
                            &verified.session_id,
                            "revoked during native resume preparation",
                        )
                        .map_err(|error| error.to_string())?;
                    Ok(PreparedZoneDrop(prepared_drop_signal))
                },
            )
            .expect("the initial read-only identity check should precede injected revocation");

        let commit = super::revalidate_and_commit_prepared_native_resume(
            &store,
            &cache,
            &identity,
            reservation,
            &credential,
            prepared_identity,
            prepared_zone,
            super::gateway_unix_ms(),
        );
        assert!(matches!(
            commit,
            Err(super::NativeResumeCommitError::IdentityUnavailable)
        ));
        assert!(prepared_dropped.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(
            store.len(),
            0,
            "terminal identity failure must not retain a dead reconnect lease"
        );
        assert!(
            store
                .resume_binding(&credential, super::gateway_unix_ms())
                .is_none(),
            "revocation during preparation must permanently revoke this credential family"
        );
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
        assert!(matches!(
            super::validate_and_prepare_native_resume(
                &store,
                &cache,
                &identity,
                &credential,
                super::gateway_unix_ms(),
                |_| Ok(()),
                |_| Ok::<(), String>(()),
            ),
            Err(super::NativeResumePrepareError::Unavailable)
        ));
    }

    #[test]
    fn native_resume_revocation_after_revalidate_before_commit_is_linearized_and_never_activates() {
        struct PreparedZoneDrop(Arc<std::sync::atomic::AtomicBool>);

        impl Drop for PreparedZoneDrop {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::Release);
            }
        }

        let store = super::ReconnectSessionStore::default();
        let (identity, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(120));
        let capacity = store
            .capacity_state_for_binding(&binding)
            .expect("fixture lease should expose capacity state");
        let cache = crate::InMemoryGatewaySessionCache::default();
        let prepared_dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (reservation, prepared_identity, prepared_zone) =
            super::validate_and_prepare_native_resume(
                &store,
                &cache,
                &identity,
                &credential,
                super::gateway_unix_ms(),
                |_| Ok(()),
                |_| Ok(PreparedZoneDrop(Arc::clone(&prepared_dropped))),
            )
            .expect("fixture should reach the post-revalidation commit window");
        let success_sent = std::sync::atomic::AtomicBool::new(false);
        let revalidated = Arc::new(std::sync::Barrier::new(2));
        let revocation_finished = Arc::new(std::sync::Barrier::new(2));
        let commit = std::thread::scope(|scope| {
            let revoker_revalidated = Arc::clone(&revalidated);
            let revoker_finished = Arc::clone(&revocation_finished);
            let revoker_store = &store;
            let revoker_identity = &identity;
            let revoker_cache = &cache;
            let revoker_verified = verified.clone();
            let revoker = scope.spawn(move || {
                revoker_revalidated.wait();
                let _revocation_fence =
                    revoker_store.begin_identity_session_revocation(&revoker_verified.session_id);
                revoker_identity
                    .revoke_session(
                        &revoker_verified,
                        &revoker_verified.session_id,
                        "revoked in the native resume commit window",
                    )
                    .expect("test identity revocation should succeed");
                revoker_cache
                    .revoke_identity_session(&revoker_verified.session_id, 60)
                    .expect("test cache revocation should succeed");
                drop(_revocation_fence);
                revoker_finished.wait();
            });
            let commit = super::revalidate_and_commit_prepared_native_resume_with_fence_hook(
                &store,
                &cache,
                &identity,
                reservation,
                &credential,
                prepared_identity,
                prepared_zone,
                super::gateway_unix_ms(),
                || {
                    // This hook runs after the function's final identity read
                    // and immediately before the store commit. A second thread
                    // completes revocation in that exact deterministic window.
                    revalidated.wait();
                    revocation_finished.wait();
                },
            );
            revoker.join().expect("revoker thread should complete");
            commit
        });
        if commit.is_ok() {
            success_sent.store(true, std::sync::atomic::Ordering::Release);
        }

        assert!(matches!(
            commit,
            Err(super::NativeResumeCommitError::IdentityUnavailable)
        ));
        assert!(prepared_dropped.load(std::sync::atomic::Ordering::Acquire));
        assert!(!success_sent.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(store.len(), 0, "the stale reservation must not restore");
        assert!(store
            .resume_binding(&credential, super::gateway_unix_ms())
            .is_none());
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
    }

    #[test]
    fn native_resume_remote_revocation_after_precheck_is_caught_by_post_commit_shared_revalidate() {
        struct PreparedZoneProbe {
            dropped: Arc<std::sync::atomic::AtomicBool>,
        }

        impl Drop for PreparedZoneProbe {
            fn drop(&mut self) {
                self.dropped
                    .store(true, std::sync::atomic::Ordering::Release);
            }
        }

        let store = super::ReconnectSessionStore::default();
        let (identity, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(120));
        let capacity = store
            .capacity_state_for_binding(&binding)
            .expect("fixture lease should expose capacity state");
        let cache = crate::InMemoryGatewaySessionCache::default();
        let prepared_dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let zone_activated = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let local_commit_completed = std::sync::atomic::AtomicBool::new(false);
        let success_sent = std::sync::atomic::AtomicBool::new(false);
        let (reservation, prepared_identity, prepared_zone) =
            super::validate_and_prepare_native_resume(
                &store,
                &cache,
                &identity,
                &credential,
                super::gateway_unix_ms(),
                |_| Ok(()),
                |_| {
                    Ok(PreparedZoneProbe {
                        dropped: Arc::clone(&prepared_dropped),
                    })
                },
            )
            .expect("fixture should pass the initial shared identity precheck");

        let commit = super::revalidate_and_commit_prepared_native_resume_with_hooks(
            &store,
            &cache,
            &identity,
            reservation,
            &credential,
            prepared_identity,
            prepared_zone,
            super::gateway_unix_ms(),
            || {
                // Simulate Gateway B writing only the shared authorities. Do
                // not touch this Gateway's local auth revision fence.
                cache
                    .revoke_identity_session(&verified.session_id, 60)
                    .expect("remote cache revocation should be visible");
                identity
                    .revoke_session(
                        &verified,
                        &verified.session_id,
                        "remote gateway revocation in resume commit window",
                    )
                    .expect("remote identity revocation should succeed");
            },
            || {
                local_commit_completed.store(true, std::sync::atomic::Ordering::Release);
            },
        );
        if commit.is_ok() {
            zone_activated.store(true, std::sync::atomic::Ordering::Release);
            success_sent.store(true, std::sync::atomic::Ordering::Release);
        }

        assert!(matches!(
            commit,
            Err(super::NativeResumeCommitError::IdentityUnavailable)
        ));
        assert!(local_commit_completed.load(std::sync::atomic::Ordering::Acquire));
        assert!(prepared_dropped.load(std::sync::atomic::Ordering::Acquire));
        assert!(!zone_activated.load(std::sync::atomic::Ordering::Acquire));
        assert!(!success_sent.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
        assert!(store
            .resume_binding(&credential, binding.issued_at_ms)
            .is_none());
        let state = store
            .state
            .lock()
            .expect("test reconnect store should not be poisoned");
        assert_eq!(
            super::ReconnectSessionStore::auth_revision_locked(
                &state,
                &binding.account_id,
                &binding.identity_session_id,
            ),
            binding.auth_revision,
            "the remote-revocation test must not use the local revision fence"
        );
        assert_eq!(
            state
                .credentials
                .family_generation_count(&binding.family_id),
            0
        );
    }

    #[test]
    fn native_resume_auth_revision_overflow_blocks_issue_reserve_and_commit() {
        let store = super::ReconnectSessionStore::default();
        let (_, verified) = issue_test_identity();
        store
            .state
            .lock()
            .expect("test reconnect store should not be poisoned")
            .identity_session_auth_revisions
            .insert(verified.session_id.clone(), u64::MAX - 1);
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(120));
        assert_eq!(binding.auth_revision.identity_session, u64::MAX - 1);
        let capacity = store
            .capacity_state_for_binding(&binding)
            .expect("fixture lease should expose capacity state");
        let reservation = store
            .reserve_by_credential(&credential, &binding, binding.issued_at_ms)
            .expect("MAX-1 revision should still permit one reservation");

        drop(store.begin_identity_session_revocation(&verified.session_id));
        assert_eq!(
            store
                .state
                .lock()
                .expect("test reconnect store should not be poisoned")
                .identity_session_auth_revisions
                .get(&verified.session_id),
            Some(&super::AUTH_REVISION_BLOCKED)
        );
        assert!(matches!(
            store.commit_resume(reservation, &credential, binding.issued_at_ms),
            Err(super::ReconnectSessionCommitError::AuthorizationRevisionChanged)
        ));
        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);

        let nonce = crate::resume::ResumeConnectionNonce::generate();
        assert!(store
            .issue_resume_credential(
                None,
                crate::resume::ResumeIssueContext {
                    account_id: &binding.account_id,
                    character_index: binding.character_index,
                    gateway_session_id: &binding.gateway_session_id,
                    identity_session_id: &binding.identity_session_id,
                    identity_expires_at_ms: binding.identity_expires_at_ms,
                    source_connection_nonce: &nonce,
                },
                binding.issued_at_ms,
                binding.generation.saturating_add(1),
                || true,
            )
            .is_none());
        assert!(store
            .reserve_by_credential(&credential, &binding, binding.issued_at_ms)
            .is_none());
    }

    #[test]
    fn native_resume_first_post_resume_action_rechecks_identity_before_execution() {
        let store = super::ReconnectSessionStore::default();
        let (identity, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
        let cache = crate::InMemoryGatewaySessionCache::default();
        let session = demo_game_session();
        let mut native_resume = super::NativeResumeConnectionState::new();
        native_resume.opted_in = true;
        native_resume.family_id = Some(binding.family_id);
        let mut first_check_pending = true;
        let mut action_executed = false;

        identity
            .revoke_session(
                &verified,
                &verified.session_id,
                "revoked before first post-resume action",
            )
            .expect("test identity revocation should succeed");
        if super::enforce_first_post_resume_action_identity(
            &mut first_check_pending,
            &store,
            &mut native_resume,
            &cache,
            &identity,
            &session,
            Some("demo"),
            Some(&verified),
            super::gateway_unix_ms(),
        ) {
            action_executed = true;
        }

        assert!(!action_executed, "the player action must not execute");
        assert!(first_check_pending);
        assert!(!native_resume.resume_allowed);
        assert!(native_resume.family_id.is_none());
        assert!(store
            .resume_binding(&credential, super::gateway_unix_ms())
            .is_none());
    }

    #[test]
    fn native_resume_commit_rechecks_lease_expiry_under_the_store_mutex() {
        let store = super::ReconnectSessionStore::default();
        let (_, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
        let capacity = store
            .capacity_state_for_binding(&binding)
            .expect("fixture lease should expose capacity state");
        let mut reservation = store
            .reserve_by_credential(&credential, &binding, super::gateway_unix_ms())
            .expect("fixture should reserve before expiry");
        reservation
            .lease
            .as_mut()
            .expect("reservation should retain its lease")
            .expires_at = Instant::now() - Duration::from_millis(1);

        assert!(store
            .commit_resume(reservation, &credential, super::gateway_unix_ms())
            .is_err());
        assert_eq!(store.len(), 0);
        assert!(store
            .resume_binding(&credential, super::gateway_unix_ms())
            .is_none());
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
    }

    #[test]
    fn native_resume_expired_credential_with_live_lease_is_terminal_and_releases_capacity() {
        let store = super::ReconnectSessionStore::default();
        let (_, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(120));
        let capacity = store
            .capacity_state_for_binding(&binding)
            .expect("fixture lease should expose capacity state");
        let reservation = store
            .reserve_by_credential(&credential, &binding, binding.issued_at_ms)
            .expect("fixture should reserve while its credential is live");

        let commit = store.commit_resume(reservation, &credential, binding.expires_at_ms);

        assert!(matches!(
            commit,
            Err(super::ReconnectSessionCommitError::CredentialUnavailable)
        ));
        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
        assert!(store
            .resume_binding(&credential, binding.issued_at_ms)
            .is_none());
        assert_eq!(
            store
                .state
                .lock()
                .expect("test reconnect store should not be poisoned")
                .credentials
                .family_generation_count(&binding.family_id),
            0,
            "terminal commit failure must revoke the entire credential family"
        );
        assert!(matches!(
            super::validate_and_prepare_native_resume(
                &store,
                &crate::InMemoryGatewaySessionCache::default(),
                &crate::identity::IdentityService::local_for_tests(),
                &credential,
                binding.issued_at_ms,
                |_| Ok(()),
                |_| Ok::<(), String>(()),
            ),
            Err(super::NativeResumePrepareError::Unavailable)
        ));
    }

    #[test]
    fn native_resume_reservation_drop_and_explicit_rollback_restore_exact_lease() {
        let store = super::ReconnectSessionStore::default();
        let (_, verified) = issue_test_identity();
        let (credential, binding) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));

        let reservation = store
            .reserve_by_credential(&credential, &binding, super::gateway_unix_ms())
            .expect("fixture should reserve");
        assert_eq!(store.len(), 0);
        reservation.rollback();
        assert_eq!(store.len(), 1);

        let reservation = store
            .reserve_by_credential(&credential, &binding, super::gateway_unix_ms())
            .expect("rolled back fixture should reserve again");
        drop(reservation);
        assert_eq!(store.len(), 1);
        assert!(store
            .resume_binding(&credential, super::gateway_unix_ms())
            .is_some());
    }

    #[test]
    fn native_resume_rejects_revoked_identity_without_consuming_the_lease() {
        let store = super::ReconnectSessionStore::default();
        let (identity, verified) = issue_test_identity();
        let (credential, _) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
        identity
            .revoke_session(&verified, &verified.session_id, "resume security test")
            .expect("test identity revocation should succeed");
        let session_cache = crate::InMemoryGatewaySessionCache::default();
        assert!(super::validate_and_commit_native_resume_for_test(
            &store,
            &session_cache,
            &identity,
            &credential,
            super::gateway_unix_ms(),
        )
        .is_none());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn native_resume_rejects_cache_revocation_without_consuming_the_lease() {
        let store = super::ReconnectSessionStore::default();
        let (identity, verified) = issue_test_identity();
        let (credential, _) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
        let session_cache = crate::InMemoryGatewaySessionCache::default();
        session_cache
            .revoke_identity_session(&verified.session_id, 60)
            .expect("test revocation cache write should succeed");
        assert!(super::validate_and_commit_native_resume_for_test(
            &store,
            &session_cache,
            &identity,
            &credential,
            super::gateway_unix_ms(),
        )
        .is_none());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn native_resume_rejects_expired_identity_binding() {
        let store = super::ReconnectSessionStore::default();
        let (_, mut verified) = issue_test_identity();
        verified.expires_at_ms = super::gateway_unix_ms().saturating_sub(1);
        let (credential, _) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
        let identity = crate::identity::IdentityService::local_for_tests();
        let session_cache = crate::InMemoryGatewaySessionCache::default();
        assert!(super::validate_and_commit_native_resume_for_test(
            &store,
            &session_cache,
            &identity,
            &credential,
            super::gateway_unix_ms(),
        )
        .is_none());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn native_resume_rejects_wrong_gateway_session_and_character_bindings() {
        let (_, verified) = issue_test_identity();
        let wrong_session_store = super::ReconnectSessionStore::default();
        let (wrong_session_credential, wrong_session_binding) = store_native_resume_fixture(
            &wrong_session_store,
            &verified,
            Some("different-gateway-session"),
            None,
            Duration::from_secs(30),
        );
        assert!(wrong_session_store
            .take_by_credential(
                &wrong_session_credential,
                &wrong_session_binding,
                super::gateway_unix_ms(),
            )
            .is_none());
        assert_eq!(wrong_session_store.len(), 1);

        let wrong_character_store = super::ReconnectSessionStore::default();
        let (wrong_character_credential, wrong_character_binding) = store_native_resume_fixture(
            &wrong_character_store,
            &verified,
            None,
            Some(crate::GatewaySessionCacheKey {
                account_id: "demo".to_string(),
                character_index: 1,
            }),
            Duration::from_secs(30),
        );
        assert!(wrong_character_store
            .take_by_credential(
                &wrong_character_credential,
                &wrong_character_binding,
                super::gateway_unix_ms(),
            )
            .is_none());
        assert_eq!(wrong_character_store.len(), 1);
    }

    #[test]
    fn native_resume_credential_and_lease_are_consumed_once_under_one_mutex() {
        let store = Arc::new(super::ReconnectSessionStore::default());
        let (_, verified) = issue_test_identity();
        let (credential, binding) = store_native_resume_fixture(
            store.as_ref(),
            &verified,
            None,
            None,
            Duration::from_secs(30),
        );
        let secret = credential.as_str().to_string();
        let successes = (0..8)
            .map(|_| {
                let store = Arc::clone(&store);
                let binding = binding.clone();
                let secret = secret.clone();
                std::thread::spawn(move || {
                    let credential: crate::resume::ResumeCredential =
                        serde_json::from_value(json!(secret)).unwrap();
                    store
                        .take_by_credential(&credential, &binding, super::gateway_unix_ms())
                        .is_some()
                })
            })
            .map(|worker| worker.join().unwrap() as usize)
            .sum::<usize>();
        assert_eq!(successes, 1);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn explicit_logout_and_disconnect_revoke_native_resume_family_before_execution() {
        for packet in [ClientPacket::LogOut, ClientPacket::Disconnect] {
            let store = super::ReconnectSessionStore::default();
            let (_, verified) = issue_test_identity();
            let (credential, binding) =
                store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
            let action = SessionAction::Packet(packet);
            assert!(super::is_explicit_session_leave_action(&action));
            let mut state = super::NativeResumeConnectionState::new();
            state.opted_in = true;
            state.family_id = Some(binding.family_id);
            state.disable_and_revoke(&store);
            assert!(!state.resume_allowed);
            assert!(store
                .resume_binding(&credential, super::gateway_unix_ms())
                .is_none());
        }
    }

    #[test]
    fn reconnect_grace_expiry_revokes_native_resume_credentials() {
        let store = super::ReconnectSessionStore::default();
        let (_, verified) = issue_test_identity();
        let (credential, _) =
            store_native_resume_fixture(&store, &verified, None, None, Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        store.purge_expired();
        assert_eq!(store.len(), 0);
        assert!(store
            .resume_binding(&credential, super::gateway_unix_ms())
            .is_none());
    }

    #[test]
    fn reconnect_resume_key_helpers_match_login_and_start_game_actions() {
        let login = SessionAction::Packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        assert_eq!(super::login_account_id_for_action(&login), Some("demo"));

        let passkey = SessionAction::PasskeyLogin {
            account_id: "wallet-demo".to_string(),
            proof_account_id: "wallet-demo".to_string(),
            token: "token".to_string(),
        };
        assert_eq!(
            super::login_account_id_for_action(&passkey),
            Some("wallet-demo")
        );

        let start = SessionAction::Packet(ClientPacket::StartGame { character_index: 2 });
        assert_eq!(
            super::start_game_character_index_for_action(&start),
            Some(2)
        );
    }

    #[test]
    fn start_game_route_lease_blocks_duplicate_online_character_before_world_entry() {
        let cache = crate::InMemoryGatewaySessionCache::default();
        let first = crate::GatewaySession::new(SimulationConfig::default());
        let second = crate::GatewaySession::new(SimulationConfig::default());

        let first_key =
            super::try_acquire_start_game_route_lease(&cache, &first, true, Some("demo"), Some(0))
                .expect("first StartGame route lease should succeed")
                .expect("StartGame should acquire a pending route lease");
        let second_error =
            super::try_acquire_start_game_route_lease(&cache, &second, true, Some("demo"), Some(0))
                .expect_err("second StartGame route lease should be rejected");

        assert!(second_error.contains("already online"));
        assert_eq!(first_key.account_id, "demo");
        assert_eq!(crate::GatewaySessionCache::route_lease_count(&cache), 1);

        super::release_pending_start_game_route_lease(&cache, &first, Some(&first_key));
        assert_eq!(crate::GatewaySessionCache::route_lease_count(&cache), 0);
        assert!(super::try_acquire_start_game_route_lease(
            &cache,
            &second,
            true,
            Some("demo"),
            Some(0),
        )
        .expect("second StartGame route lease should succeed after release")
        .is_some());
    }

    #[test]
    fn start_game_route_lease_releases_when_start_game_does_not_claim_identity() {
        let cache = crate::InMemoryGatewaySessionCache::default();
        let session = crate::GatewaySession::new(SimulationConfig::default());
        let key = super::try_acquire_start_game_route_lease(
            &cache,
            &session,
            true,
            Some("demo"),
            Some(0),
        )
        .expect("pending route lease should succeed")
        .expect("StartGame should acquire a pending route lease");

        super::release_unclaimed_start_game_route_lease(&cache, &session, Some(&key));

        assert_eq!(crate::GatewaySessionCache::route_lease_count(&cache), 0);
    }

    #[test]
    fn player_command_safety_defaults_closed_when_environment_is_unconfigured() {
        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", None),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                (super::UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT, None),
            ],
            || {
                let enforce = super::production_player_command_safety_enabled(
                    "127.0.0.1".parse().expect("loopback IP"),
                );
                assert!(
                    enforce,
                    "missing environment configuration must fail closed"
                );
                let mut session = crate::GatewaySession::new(SimulationConfig::default());

                let unauthenticated_game_shop = super::execute_session_action(
                    &mut session,
                    SessionAction::Packet(ClientPacket::GameShopBuy {
                        g_index: 31,
                        quantity: 1,
                        price_type: 0,
                    }),
                    false,
                    enforce,
                )
                .expect_err("identity must still be required with no environment variables");
                assert!(unauthenticated_game_shop.contains("authenticated account"));

                for action in [
                    SessionAction::MoveTo {
                        x: 330,
                        y: 270,
                        running: false,
                    },
                    SessionAction::Stage5Command {
                        action: "qa.giveItem".to_string(),
                        args: vec!["red-potion".to_string()],
                    },
                    SessionAction::TransferMap {
                        key: "crystal:0:330:270".to_string(),
                    },
                ] {
                    assert!(
                        super::execute_session_action(&mut session, action, true, enforce).is_err(),
                        "debug and generic commands must remain closed by default"
                    );
                }
            },
        );
    }

    #[test]
    fn unsafe_player_command_opt_out_requires_explicit_dev_or_test_loopback() {
        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", Some("development")),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                (super::UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT, Some("1")),
            ],
            || {
                assert!(
                    !super::production_player_command_safety_enabled(
                        "127.0.0.1".parse().expect("loopback IP"),
                    ),
                    "explicit local development opt-out should enable direct test commands"
                );
                assert!(
                    super::production_player_command_safety_enabled(
                        "198.51.100.25".parse().expect("remote IP"),
                    ),
                    "the same opt-out must not apply to a non-loopback peer"
                );

                let mut session = crate::GatewaySession::new(SimulationConfig::default());
                session.handle_packet(ClientPacket::StartGame { character_index: 0 });
                let inventory_len = session.world_snapshot().inventory_items.len();
                super::execute_session_action(
                    &mut session,
                    SessionAction::Stage5Command {
                        action: "qa.giveItem".to_string(),
                        args: vec!["red-potion".to_string()],
                    },
                    true,
                    false,
                )
                .expect("the explicit loopback development opt-out should retain direct QA use");
                assert_eq!(
                    session.world_snapshot().inventory_items.len(),
                    inventory_len + 1
                );
            },
        );
    }

    #[test]
    fn forwarded_loopback_header_cannot_authorize_remote_unsafe_commands() {
        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", Some("development")),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                (super::UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT, Some("1")),
                ("MIR2_TRUST_CF_CONNECTING_IP", Some("true")),
            ],
            || {
                let mut headers = axum::http::HeaderMap::new();
                headers.insert(
                    "cf-connecting-ip",
                    axum::http::HeaderValue::from_static("127.0.0.1"),
                );
                let remote_peer: std::net::SocketAddr =
                    "198.51.100.25:43210".parse().expect("remote socket");
                assert_eq!(
                    super::trusted_client_address(&headers, remote_peer),
                    "127.0.0.1",
                    "the forwarded address may still be used for logs/rate limits"
                );
                assert!(
                    super::production_player_command_safety_enabled(remote_peer.ip()),
                    "a forged forwarded loopback address must not authorize unsafe commands"
                );

                let loopback_peer: std::net::SocketAddr =
                    "127.0.0.1:43210".parse().expect("loopback socket");
                assert!(
                    !super::production_player_command_safety_enabled(loopback_peer.ip()),
                    "the real loopback TCP peer may use the explicit dev/test opt-out"
                );
            },
        );
    }

    #[test]
    fn production_and_staging_cannot_disable_player_command_safety() {
        for environment in ["production", "staging"] {
            with_env_vars(
                &[
                    ("MIR2_RUNTIME_ENV", Some(environment)),
                    ("MIR2_DEPLOYMENT_ENV", Some("development")),
                    ("MIR2_ENV", None),
                    (super::UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT, Some("true")),
                ],
                || {
                    let enforce = super::production_player_command_safety_enabled(
                        "127.0.0.1".parse().expect("loopback IP"),
                    );
                    assert!(
                        enforce,
                        "{environment} must override the unsafe local opt-out"
                    );
                    let mut session = crate::GatewaySession::new(SimulationConfig::default());
                    let error = super::execute_session_action(
                        &mut session,
                        SessionAction::Stage5Command {
                            action: "qa.giveItem".to_string(),
                            args: vec!["red-potion".to_string()],
                        },
                        true,
                        enforce,
                    )
                    .expect_err(
                        "generic Stage5 must remain closed in production-like environments",
                    );
                    assert!(error.contains("Stage5Command"));
                },
            );
        }
    }

    #[test]
    fn production_web_path_rejects_debug_runtime_commands() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());

        let move_error = super::execute_session_action(
            &mut session,
            SessionAction::MoveTo {
                x: 330,
                y: 270,
                running: false,
            },
            true,
            true,
        )
        .expect_err("production web path should reject MoveTo");
        assert!(move_error.contains("debug MoveTo"));

        let stage5_error = super::execute_session_action(
            &mut session,
            SessionAction::Stage5Command {
                action: "qa.giveItem".to_string(),
                args: vec!["red-potion".to_string()],
            },
            true,
            true,
        )
        .expect_err("production web path should reject Stage5Command");
        assert!(stage5_error.contains("Stage5Command"));

        for key in [
            "crystal:0:330:270",
            "crystal:0:330",
            "crystal:.map:330:270",
            "crystal:0:330:270:",
            "crystal:0:330:270:extra",
        ] {
            let transfer_error = super::execute_session_action(
                &mut session,
                SessionAction::TransferMap {
                    key: key.to_string(),
                },
                true,
                true,
            )
            .expect_err("production web path should reject the debug crystal namespace");
            assert!(transfer_error.contains("debug crystal transfer"));
        }
    }

    #[test]
    fn local_debug_transfer_rejects_extra_segments_without_relocation() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let before = session.world_snapshot();
        let before_position = before
            .player_object_id
            .and_then(|object_id| {
                before
                    .entities
                    .iter()
                    .find(|entity| entity.object_id == object_id)
            })
            .map(|entity| (entity.x, entity.y));

        let packets = super::execute_session_action(
            &mut session,
            SessionAction::TransferMap {
                key: "crystal:0:330:270:extra".to_string(),
            },
            true,
            false,
        )
        .expect("malformed local debug transfer should be rejected as a protocol result");

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Chat { .. })));
        let after = session.world_snapshot();
        assert_eq!(after.map_file_name, before.map_file_name);
        let after_position = after
            .player_object_id
            .and_then(|object_id| {
                after
                    .entities
                    .iter()
                    .find(|entity| entity.object_id == object_id)
            })
            .map(|entity| (entity.x, entity.y));
        assert_eq!(after_position, before_position);
    }

    #[test]
    fn production_game_shop_requires_authentication_and_generic_stage5_stays_closed() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());
        let dedicated = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"gameShopBuy","gIndex":31,"quantity":2,"priceType":0}"#,
        )
        .expect("dedicated game-shop command should deserialize");
        let dedicated = super::browser_command_to_action(dedicated)
            .expect("dedicated game-shop command should map");
        let unauthenticated_error =
            super::execute_session_action(&mut session, dedicated, false, true)
                .expect_err("unauthenticated game-shop purchase must be rejected");
        assert!(unauthenticated_error.contains("authenticated account"));

        let generic = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"stage5Command","action":"gameShop.buyCredit","args":["31","2"]}"#,
        )
        .expect("legacy Stage5 JSON remains compatible");
        let generic = super::browser_command_to_action(generic)
            .expect("legacy Stage5 JSON should retain its existing mapping");
        let generic_error = super::execute_session_action(&mut session, generic, true, true)
            .expect_err("ordinary production clients cannot execute generic Stage5");
        assert!(generic_error.contains("Stage5Command"));
    }

    #[test]
    fn browser_send_mail_requires_authenticated_active_character_before_simulation() {
        fn send_mail_action() -> SessionAction {
            let command = serde_json::from_str::<BrowserCommand>(
                r#"{"type":"sendMail","name":"Scout","message":"boundary","gold":0,"itemsIdx":[0,0,0,0,0],"stamped":false}"#,
            )
            .expect("sendMail command should deserialize");
            super::browser_command_to_action(command).expect("sendMail command should map")
        }

        let config = SimulationConfig::default();
        let account_store = Arc::clone(&config.account_store);
        let mut session = crate::GatewaySession::new(config);
        let before_world = session.world_snapshot();
        let before_store = serde_json::to_string(
            &*account_store
                .lock()
                .expect("account store lock before anonymous send"),
        )
        .expect("account store serializes");
        let error = super::execute_session_action(&mut session, send_mail_action(), false, true)
            .expect_err("anonymous sendMail must be rejected at the gateway boundary");
        assert!(error.contains("authenticated account"));
        assert_eq!(session.world_snapshot(), before_world);
        assert_eq!(
            serde_json::to_string(
                &*account_store
                    .lock()
                    .expect("account store lock after anonymous send")
            )
            .expect("account store serializes"),
            before_store
        );

        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let before_world = session.world_snapshot();
        let before_store = serde_json::to_string(
            &*account_store
                .lock()
                .expect("account store lock before pre-StartGame send"),
        )
        .expect("account store serializes");
        let error = super::execute_session_action(&mut session, send_mail_action(), true, true)
            .expect_err("authenticated pre-StartGame sendMail must be rejected");
        assert!(error.contains("active in-game character"));
        assert_eq!(session.world_snapshot(), before_world);
        assert_eq!(
            serde_json::to_string(
                &*account_store
                    .lock()
                    .expect("account store lock after pre-StartGame send")
            )
            .expect("account store serializes"),
            before_store
        );

        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let packets = super::execute_session_action(&mut session, send_mail_action(), true, true)
            .expect("authenticated active character may send valid mail");
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::MailSent { result: 1 })));
    }

    #[test]
    fn qa_control_requires_configured_token() {
        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", Some("development")),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                (super::UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT, Some("1")),
                ("MIR2_GATEWAY_QA_CONTROL_TOKEN", None),
            ],
            || {
                let mut session = crate::GatewaySession::new(SimulationConfig::default());
                let enforce = super::production_player_command_safety_enabled(
                    "127.0.0.1".parse().expect("loopback IP"),
                );
                assert!(!enforce);
                let error = super::execute_session_action(
                    &mut session,
                    SessionAction::QaControl {
                        token: "missing".to_string(),
                        action: QaControlAction::TransferMap {
                            key: "crystal:0:330:270".to_string(),
                        },
                    },
                    true,
                    enforce,
                )
                .expect_err("QA control should fail closed when no token is configured");

                assert!(error.contains("QA control is disabled"));
            },
        );
    }

    #[test]
    fn qa_control_token_works_only_with_real_loopback_dev_opt_out() {
        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", Some("development")),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                (super::UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT, Some("1")),
                ("MIR2_GATEWAY_QA_CONTROL_TOKEN", Some("qa-secret")),
            ],
            || {
                let mut session = crate::GatewaySession::new(SimulationConfig::default());
                session.handle_packet(ClientPacket::StartGame { character_index: 0 });
                let enforce = super::production_player_command_safety_enabled(
                    "127.0.0.1".parse().expect("loopback IP"),
                );
                assert!(!enforce);

                let normal_transfer_error = super::execute_session_action(
                    &mut session,
                    SessionAction::TransferMap {
                        key: "crystal:0:330:270".to_string(),
                    },
                    true,
                    true,
                )
                .expect_err("normal production path should still reject debug transfer");
                assert!(normal_transfer_error.contains("debug crystal transfer"));

                let packets = super::execute_session_action(
                    &mut session,
                    SessionAction::QaControl {
                        token: "qa-secret".to_string(),
                        action: QaControlAction::TransferMap {
                            key: "crystal:0:330:270".to_string(),
                        },
                    },
                    true,
                    enforce,
                )
                .expect("authorized local-development QA control should execute");

                assert!(packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::MapInformation { .. } | ServerPacket::UserLocation { .. }
                )));
            },
        );
    }

    #[test]
    fn qa_control_is_rejected_when_player_command_safety_is_enabled() {
        for environment in ["production", "staging"] {
            with_env_vars(
                &[
                    ("MIR2_RUNTIME_ENV", Some(environment)),
                    ("MIR2_DEPLOYMENT_ENV", Some("development")),
                    ("MIR2_ENV", None),
                    (super::UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT, Some("1")),
                    ("MIR2_GATEWAY_QA_CONTROL_TOKEN", Some("qa-secret")),
                ],
                || {
                    let enforce = super::production_player_command_safety_enabled(
                        "127.0.0.1".parse().expect("loopback IP"),
                    );
                    assert!(enforce);
                    let mut session = crate::GatewaySession::new(SimulationConfig::default());
                    let error = super::execute_session_action(
                        &mut session,
                        SessionAction::QaControl {
                            token: "qa-secret".to_string(),
                            action: QaControlAction::Tick,
                        },
                        true,
                        enforce,
                    )
                    .expect_err("production-like player WebSockets must reject QA control");
                    assert!(error.contains("local dev/test unsafe opt-out"));
                },
            );
        }
    }

    #[test]
    fn qa_control_apply_native_state_updates_live_session_snapshot() {
        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", Some("development")),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                (super::UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT, Some("1")),
                ("MIR2_GATEWAY_QA_CONTROL_TOKEN", Some("qa-secret")),
            ],
            || {
                let mut session = demo_game_session();
                let enforce = super::production_player_command_safety_enabled(
                    "127.0.0.1".parse().expect("loopback IP"),
                );
                assert!(!enforce);

                let belt_item = json!({
                    "key": "crystal-item-658",
                    "name": "(HP)DrugSmall",
                    "icon": 398,
                    "slot": 0,
                    "unique_id": 59,
                    "container": "belt",
                    "quantity": 1,
                    "description": "Crystal native account item: (HP)DrugSmall.",
                    "durability_current": null,
                    "durability_max": null,
                    "weight": 1,
                    "equip_slot": null,
                    "grade": "none",
                    "added_attack": 0,
                    "added_defence": 0,
                    "added_stats": [],
                    "socketed": [],
                    "cursed": false,
                    "socket_slots": 0,
                    "gem_count": 0,
                    "identified": true,
                    "soul_bound_id": null,
                    "sealed_expiry_time_binary_datetime": 0,
                    "sealed_next_time_binary_datetime": 0,
                    "rental_binding_flags": 0,
                    "rental_owner_name": "",
                    "rental_expiry_binary_datetime": 0,
                    "rental_locked": false,
                    "attack": 0,
                    "defence": 0,
                    "heal_hp": 30,
                    "heal_mp": 0
                })
                .to_string();
                let payload = json!({
                    "character": {
                        "name": "Scout",
                        "level": 6,
                        "class": "Warrior",
                        "gender": "Male"
                    },
                    "mapFileName": "0",
                    "mapTitle": "BichonProvince",
                    "position": { "x": 335, "y": 262 },
                    "direction": "UpRight",
                    "hp": 51,
                    "maxHp": 51,
                    "mp": 32,
                    "maxMp": 32,
                    "experience": 435,
                    "maxExperience": 900,
                    "gold": 3457,
                    "credit": 0,
                    "inventoryItemsJson": [],
                    "beltItemsJson": [belt_item],
                    "storageItemsJson": [],
                    "equipmentItemsJson": []
                })
                .to_string();

                let packets = super::execute_session_action(
                    &mut session,
                    SessionAction::QaControl {
                        token: "qa-secret".to_string(),
                        action: QaControlAction::Stage5Command {
                            action: "qa.applyNativeState".to_string(),
                            args: vec![payload],
                        },
                    },
                    true,
                    enforce,
                )
                .expect("authorized local-development QA native state apply should execute");

                assert!(packets
                    .iter()
                    .any(|packet| matches!(packet, ServerPacket::UserInformation { .. })));
                let snapshot = session.world_snapshot();
                assert_eq!(snapshot.map_file_name.as_deref(), Some("0"));
                assert_eq!(snapshot.map_title.as_deref(), Some("BichonProvince"));
                assert_eq!(snapshot.player_hp, Some(51));
                assert!(snapshot.player_max_hp.is_some_and(|max_hp| max_hp >= 51));
                assert_eq!(snapshot.player_mp, Some(32));
                assert!(snapshot.player_max_mp.is_some_and(|max_mp| max_mp >= 32));
                assert_eq!(snapshot.player_experience, 435);
                assert_eq!(snapshot.player_max_experience, 900);
                assert_eq!(snapshot.gold, 3457);
                assert_eq!(snapshot.current_weight, 1);
                assert_eq!(snapshot.max_weight, 62);
                assert!((1..=4).contains(&snapshot.light_setting));
                let snapshot_json =
                    serde_json::to_value(&snapshot).expect("snapshot should serialize to JSON");
                assert!(snapshot_json["lightSetting"]
                    .as_u64()
                    .is_some_and(|lights| (1..=4).contains(&lights)));
                assert_eq!(snapshot.belt_items.len(), 1);
                assert_eq!(snapshot.belt_items[0].key, "crystal-item-658");
                assert_eq!(snapshot.equipment_items.len(), 0);
            },
        );
    }

    #[test]
    fn qa_control_rejects_wrong_token() {
        with_env_vars(
            &[
                ("MIR2_RUNTIME_ENV", Some("development")),
                ("MIR2_DEPLOYMENT_ENV", None),
                ("MIR2_ENV", None),
                (super::UNSAFE_LOCAL_PLAYER_COMMANDS_OPT_OUT, Some("1")),
                ("MIR2_GATEWAY_QA_CONTROL_TOKEN", Some("qa-secret")),
            ],
            || {
                let mut session = crate::GatewaySession::new(SimulationConfig::default());
                let enforce = super::production_player_command_safety_enabled(
                    "127.0.0.1".parse().expect("loopback IP"),
                );
                assert!(!enforce);
                let error = super::execute_session_action(
                    &mut session,
                    SessionAction::QaControl {
                        token: "not-secret".to_string(),
                        action: QaControlAction::Tick,
                    },
                    true,
                    enforce,
                )
                .expect_err("wrong QA control token should be rejected");

                assert!(error.contains("invalid QA control token"));
            },
        );
    }

    #[test]
    fn stage5_damage_equipment_still_works_after_storage_password_flow() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.handle_packet(ClientPacket::EquipItem {
            unique_id: 4,
            grid: MirGridType::Inventory,
            to: 0,
        });
        session.transfer_map("crystal:0:317:260");
        session.stage5_command("qa.openStorage", Vec::new());
        session.handle_packet(ClientPacket::SetStoragePassword {
            current_password: String::new(),
            new_password: "Safe123".to_string(),
        });
        session.handle_packet(ClientPacket::UnlockStorage {
            password: "Safe123".to_string(),
        });
        session.handle_packet(ClientPacket::SetStoragePassword {
            current_password: "Safe123".to_string(),
            new_password: "Vault123".to_string(),
        });
        session.handle_packet(ClientPacket::RemoveStoragePassword {
            current_password: "Vault123".to_string(),
        });

        let responses =
            session.stage5_command("qa.damageEquipment", vec!["weapon".into(), "2500".into()]);
        let snapshot = session.world_snapshot();
        let weapon = snapshot
            .equipment_items
            .iter()
            .find(|item| item.slot == mir2_simulation::EquipmentSlot::Weapon)
            .expect("weapon should remain equipped");

        assert_eq!(weapon.name, "Dagger");
        assert_eq!(weapon.durability_current, 0);
        assert!(responses.iter().any(|packet| matches!(
            packet,
            ServerPacket::DuraChanged {
                unique_id: 0,
                current_dura: 0
            }
        )));
    }

    #[test]
    fn select_npc_dialog_command_accepts_target_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"selectNpcDialog","target":"@Buy"}"#)
                .expect("select npc dialog command should deserialize");

        match command {
            BrowserCommand::SelectNpcDialog { target } => assert_eq!(target, "@Buy"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn move_actions_skip_snapshot_without_visibility_changes() {
        let action = SessionAction::MoveTo {
            x: 10,
            y: 20,
            running: false,
        };

        assert!(!should_send_world_snapshot_for_action(&action));
    }

    #[test]
    fn movement_packets_skip_snapshot_by_default() {
        let action = SessionAction::Packet(ClientPacket::Walk {
            direction: mir2_protocol::MirDirection::Up,
        });

        assert!(!should_send_world_snapshot_for_action(&action));
    }

    #[test]
    fn visibility_packets_force_snapshot() {
        let responses = vec![ServerPacket::ObjectShow { object_id: 42 }];

        assert!(responses_require_world_snapshot(&responses));
    }

    #[test]
    fn bootstrap_state_packets_force_snapshot() {
        let responses = vec![
            ServerPacket::StartGame {
                result: 4,
                resolution: 1920,
            },
            ServerPacket::UserLocation {
                location: UserLocation {
                    position: Point { x: 335, y: 262 },
                    direction: MirDirection::UpRight,
                },
            },
        ];

        assert!(responses_require_world_snapshot(&responses));
    }
}

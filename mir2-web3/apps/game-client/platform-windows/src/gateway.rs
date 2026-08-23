//! Gateway WebSocket client for the native host.
//!
//! Reuses the exact JSON `BrowserCommand` wire protocol the Web client speaks
//! (see `apps/gateway/src/web.rs` `BrowserCommand` and the 5-layer data flow in
//! `docs/client/protocol-cross-layer.md`). The server remains authoritative;
//! this client only authenticates, starts a game, and forwards the world
//! snapshot into the shared runtime.
//!
//! Candidate slice: visible login/character flow, StartGame bootstrap, movement,
//! combat, NPC/quest/item intents, and authoritative world/read-model forwarding.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::{SinkExt, StreamExt};
use mir2_client_bevy::game_shop::{GameShopReceipt, GameShopRequest};
use mir2_client_bevy::native_shell::{CharacterSummary, NativeGatewayEvent as ShellGatewayEvent};
use mir2_client_bevy::pending_operations::InventoryOperationAck;
use mir2_client_bevy::skill_model::MAX_LEARNED_SKILLS;
use mir2_client_bevy::social::SocialModel;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use crate::gameplay_bridge::{NativeGameplayAdapter, NativeGameplaySnapshot};
use crate::map_parser::lighting::{NativeLightAssets, NativeLightingBridge, NativeLightingMotion};
use crate::native_protocol::{
    parse_inbound_event, InboundEvent, NativeOutboundCommand, PacketEvent,
};
use crate::session_config::NativeReconnectConfig;

/// The gateway WebSocket endpoint for the local development gateway.
pub const LOCAL_GATEWAY_WS_URL: &str = "ws://127.0.0.1:7110/ws";
const NATIVE_RESUME_PROTOCOL: &str = "nativeResumeV1";
const NATIVE_GAME_SHOP_RECEIPT_PROTOCOL: &str = "nativeGameShopReceiptV1";
const MAX_CREDENTIAL_LENGTH: usize = 43;
const MAX_COMMANDS_PER_POLL: usize = 256;
const MAX_GATEWAY_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// There is no packet-authoritative presentation interpolation stream yet.
/// Keep the fallback explicit and stationary rather than inventing wall-clock
/// movement a second time in the network gateway.
fn native_lighting_default_motion() -> NativeLightingMotion {
    NativeLightingMotion {
        camera_offset_x: 0.0,
        camera_offset_y: 0.0,
        entity_offsets: HashMap::new(),
    }
}

/// Connection-owner correlation for the sole native GameShop transaction.
///
/// `pending` is installed only after the WebSocket write succeeds. `reserved`
/// protects the exact receipt after it has entered the runtime's one-element
/// reserve; later duplicate/wrong receipts cannot overwrite that authoritative
/// result while the Bevy frame is still draining it.
#[derive(Debug, Default)]
struct GameShopReceiptGate {
    pending: Option<GameShopRequest>,
    reserved: Option<GameShopReceipt>,
}

/// Cross the terminal account/session boundary without losing an exact
/// GameShop result already accepted into the runtime reserve. Pending without
/// a receipt remains ambiguous and therefore uses the ordinary DataReset.
fn terminate_session_with_game_shop_boundary<F, P>(
    gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
    mut push_preserving_reset: P,
) -> bool
where
    F: FnMut() -> bool,
    P: FnMut(GameShopReceipt) -> bool,
{
    if let Some(receipt) = gate.reserved.clone() {
        if !push_preserving_reset(receipt) {
            return false;
        }
        gate.clear_terminal();
        return true;
    }
    gate.clear_terminal();
    push_data_reset()
}

impl GameShopReceiptGate {
    fn record_successful_send(&mut self, request: GameShopRequest) -> bool {
        if !request.is_valid() || self.pending.is_some() {
            return false;
        }
        // A new UI request can only be produced after the previous exact
        // receipt cleared shared pending state. It is therefore safe to retire
        // the prior reserve correlation at this point.
        self.reserved = None;
        self.pending = Some(request);
        true
    }

    fn clear_terminal(&mut self) {
        self.pending = None;
        self.reserved = None;
    }
}

/// End only a purchase that has crossed the socket write boundary and is still
/// waiting for its exact receipt. A receipt already accepted into `reserved`
/// is authoritative and must survive later transport/protocol failures.
fn terminate_written_game_shop_unknown<F>(
    gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
) -> bool
where
    F: FnMut() -> bool,
{
    if gate.pending.is_none() {
        return false;
    }
    gate.clear_terminal();
    let _ = push_data_reset();
    true
}

fn process_connected_text_frame<T, A, F>(
    text: &str,
    gate: &mut GameShopReceiptGate,
    apply: A,
    push_data_reset: F,
) -> Result<T, String>
where
    A: FnOnce(&str, &mut GameShopReceiptGate) -> Result<T, String>,
    F: FnMut() -> bool,
{
    if text.len() > MAX_GATEWAY_FRAME_BYTES {
        let _ = terminate_written_game_shop_unknown(gate, push_data_reset);
        return Err(format!(
            "gateway text frame exceeds {MAX_GATEWAY_FRAME_BYTES} bytes"
        ));
    }
    match apply(text, gate) {
        Ok(value) => Ok(value),
        Err(error) => {
            let _ = terminate_written_game_shop_unknown(gate, push_data_reset);
            Err(error)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnectedSocketEnd {
    Disconnected,
    ReadError(String),
}

fn finish_connected_socket<F>(
    end: ConnectedSocketEnd,
    gate: &mut GameShopReceiptGate,
    push_data_reset: F,
) -> ConnectedExit
where
    F: FnMut() -> bool,
{
    let _ = terminate_written_game_shop_unknown(gate, push_data_reset);
    match end {
        ConnectedSocketEnd::Disconnected => ConnectedExit::Disconnected(None),
        ConnectedSocketEnd::ReadError(error) => {
            ConnectedExit::Disconnected(Some(format!("gateway read error: {error}")))
        }
    }
}

/// Player intent commands the native host forwards to the gateway.
///
/// Mirrors the Web client's `BrowserCommand` movement surface. The server
/// remains authoritative; these are requests, not state changes.
#[derive(Debug, Clone)]
pub enum PlayerIntent {
    Walk { direction: String },
    Run { direction: String },
    Turn { direction: String },
}

impl PlayerIntent {
    fn to_json(&self) -> Value {
        match self {
            Self::Walk { direction } => json!({ "type": "walk", "direction": direction }),
            Self::Run { direction } => json!({ "type": "run", "direction": direction }),
            Self::Turn { direction } => json!({ "type": "turn", "direction": direction }),
        }
    }
}

/// Commands crossing from the Bevy main thread to the async Gateway owner.
/// `Wire` contains a production BrowserCommand payload. Lifecycle signals are
/// local and are never serialized.
#[derive(Debug, Clone)]
pub enum GatewayCommand {
    Connect,
    Wire(NativeOutboundCommand),
    Player(PlayerIntent),
    Shutdown,
}

/// Non-blocking producer handle for the sole WebSocket writer. Production uses
/// a bounded normal queue; lifecycle commands use a small priority lane so a
/// full movement queue cannot lose logout or shutdown.
#[derive(Clone)]
pub struct GatewayCommandSender {
    inner: Arc<GatewayCommandSenderInner>,
}

enum GatewayCommandSenderInner {
    Bounded {
        sender: std::sync::mpsc::SyncSender<GatewayCommand>,
        priority: Arc<Mutex<VecDeque<GatewayCommand>>>,
        transaction: Arc<Mutex<Option<GatewayCommand>>>,
    },
    Test(std::sync::mpsc::Sender<GatewayCommand>),
}

pub struct GatewayCommandReceiver {
    receiver: std::sync::mpsc::Receiver<GatewayCommand>,
    priority: Option<Arc<Mutex<VecDeque<GatewayCommand>>>>,
    transaction: Option<Arc<Mutex<Option<GatewayCommand>>>>,
}

/// Create the production command pair. No producer call blocks the UI thread.
pub fn command_channel(capacity: usize) -> (GatewayCommandSender, GatewayCommandReceiver) {
    let (sender, receiver) = std::sync::mpsc::sync_channel(capacity.max(8));
    let priority = Arc::new(Mutex::new(VecDeque::with_capacity(3)));
    let transaction = Arc::new(Mutex::new(None));
    (
        GatewayCommandSender {
            inner: Arc::new(GatewayCommandSenderInner::Bounded {
                sender,
                priority: priority.clone(),
                transaction: transaction.clone(),
            }),
        },
        GatewayCommandReceiver {
            receiver,
            priority: Some(priority),
            transaction: Some(transaction),
        },
    )
}

impl From<std::sync::mpsc::Sender<GatewayCommand>> for GatewayCommandSender {
    fn from(sender: std::sync::mpsc::Sender<GatewayCommand>) -> Self {
        Self {
            inner: Arc::new(GatewayCommandSenderInner::Test(sender)),
        }
    }
}

impl GatewayCommandSender {
    pub fn send(&self, command: GatewayCommand) -> Result<(), ()> {
        match self.inner.as_ref() {
            GatewayCommandSenderInner::Test(sender) => sender.send(command).map_err(|_| ()),
            GatewayCommandSenderInner::Bounded {
                sender,
                priority,
                transaction,
            } => {
                if is_priority_command(&command) {
                    let mut queue = priority
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if !queue
                        .iter()
                        .any(|queued| same_priority_kind(queued, &command))
                    {
                        queue.push_back(command);
                    }
                    Ok(())
                } else if is_correlated_transaction(&command) {
                    let mut slot = transaction
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if slot.is_some() {
                        return Err(());
                    }
                    *slot = Some(command);
                    Ok(())
                } else {
                    match sender.try_send(command) {
                        Ok(()) | Err(std::sync::mpsc::TrySendError::Full(_)) => Ok(()),
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => Err(()),
                    }
                }
            }
        }
    }
}

impl GatewayCommandReceiver {
    fn try_recv(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError> {
        if let Some(priority) = &self.priority {
            if let Some(command) = priority
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
            {
                return Ok(command);
            }
        }
        if let Some(transaction) = &self.transaction {
            if let Some(command) = transaction
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take()
            {
                return Ok(command);
            }
        }
        self.receiver.try_recv()
    }
}

fn is_priority_command(command: &GatewayCommand) -> bool {
    matches!(
        command,
        GatewayCommand::Shutdown
            | GatewayCommand::Wire(NativeOutboundCommand::LogOut)
            | GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
    )
}

fn is_game_shop_transaction(command: &GatewayCommand) -> bool {
    matches!(
        command,
        GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy { .. })
    )
}

fn is_storage_transaction(command: &GatewayCommand) -> bool {
    matches!(
        command,
        GatewayCommand::Wire(
            NativeOutboundCommand::StoreItem { .. } | NativeOutboundCommand::TakeBackItem { .. }
        )
    )
}

fn is_correlated_transaction(command: &GatewayCommand) -> bool {
    is_game_shop_transaction(command) || is_storage_transaction(command)
}

fn game_shop_request_from_command(command: &GatewayCommand) -> Option<GameShopRequest> {
    let GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
        request_id,
        g_index,
        quantity,
        price_type,
    }) = command
    else {
        return None;
    };
    GameShopRequest::new(request_id.clone(), *g_index, *quantity, *price_type)
}

/// Resolve a correlated mutation that was removed from its bounded lane but
/// cannot reach `socket.send`. A reset is the fail-closed "commit unknown"
/// transition for both GameShop and Storage V2; neither command is replayed.
fn discard_correlated_before_socket_write<F>(
    command: &GatewayCommand,
    gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
) -> bool
where
    F: FnMut() -> bool,
{
    if !is_correlated_transaction(command) {
        return false;
    }
    if is_game_shop_transaction(command) {
        gate.clear_terminal();
    }
    let _ = push_data_reset();
    true
}

/// Deliver one authoritative Storage patch. If the ordinary critical FIFO is
/// saturated, replace the queued session models with the non-evictable reset
/// barrier. This deliberately reports the storage commit as unknown instead
/// of losing the exact receipt and leaving the UI pending forever.
fn push_storage_patch_or_reset<P, R>(json: String, mut push_patch: P, mut push_reset: R) -> bool
where
    P: FnMut(String) -> bool,
    R: FnMut() -> bool,
{
    if push_patch(json) {
        true
    } else {
        push_reset()
    }
}

fn same_priority_kind(left: &GatewayCommand, right: &GatewayCommand) -> bool {
    matches!(
        (left, right),
        (GatewayCommand::Shutdown, GatewayCommand::Shutdown)
            | (
                GatewayCommand::Wire(NativeOutboundCommand::LogOut),
                GatewayCommand::Wire(NativeOutboundCommand::LogOut)
            )
            | (
                GatewayCommand::Wire(NativeOutboundCommand::Disconnect),
                GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
            )
    )
}

pub trait CommandSource {
    fn try_command(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError>;
}

impl CommandSource for GatewayCommandReceiver {
    fn try_command(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError> {
        self.try_recv()
    }
}

impl CommandSource for std::sync::mpsc::Receiver<GatewayCommand> {
    fn try_command(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError> {
        self.try_recv()
    }
}

/// Gateway WS messages decoded for the native host.
#[derive(Debug, Clone, Deserialize)]
struct GatewayEnvelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    packet: Option<String>,
    #[serde(default)]
    payload: Option<Value>,
}

/// Per-WebSocket lighting producer. It owns the lifecycle generation and the
/// last successfully submitted snapshot, so a full native-ingest queue never
/// advances local dirty state or suppresses the required retry.
struct NativeLightingPublisher {
    bridge: NativeLightingBridge,
    assets: NativeLightAssets,
    /// This deliberately stays empty until real map frame/presentation data is
    /// exposed by the renderer. Empty means Crystal's static-frame offset of
    /// (0, 0), not a guessed animation position.
    map_frame_offsets: HashMap<(i32, i32), (i32, i32)>,
    last_pushed_json: Option<String>,
}

impl NativeLightingPublisher {
    fn for_connection(generation: u64) -> Self {
        let assets = crate::assets::asset_root()
            .map(|root| NativeLightAssets::from_asset_root(&root))
            .unwrap_or_default();
        let mut bridge = NativeLightingBridge::default();
        bridge.set_generation(generation);
        Self {
            bridge,
            assets,
            map_frame_offsets: HashMap::new(),
            last_pushed_json: None,
        }
    }

    fn reset_scene(&mut self) {
        self.bridge.reset_scene();
        self.map_frame_offsets.clear();
        self.last_pushed_json = None;
    }

    fn reset_session(&mut self) {
        self.bridge.reset_session();
        self.map_frame_offsets.clear();
        self.last_pushed_json = None;
    }

    fn push_clear_state(&mut self) {
        let state = self.bridge.build_render_state(
            &Value::Null,
            None,
            &self.map_frame_offsets,
            &native_lighting_default_motion(),
            &self.assets,
        );
        self.publish(state);
    }

    fn observe_envelope(&mut self, event: &GatewayEnvelope) {
        self.observe_envelope_with(event, |json| {
            mir2_bevy_runtime::native_ingest::push_native_lighting_render_state(json)
        });
    }

    fn observe_envelope_with(
        &mut self,
        event: &GatewayEnvelope,
        mut push: impl FnMut(String) -> bool,
    ) {
        match event.kind.as_str() {
            "worldSnapshot" => {
                let payload = event.payload.as_ref().unwrap_or(&Value::Null);
                self.bridge.observe_world_snapshot(payload);
                let map = payload
                    .get("mapFileName")
                    .and_then(Value::as_str)
                    .and_then(crate::map_parser::load_map);
                self.map_frame_offsets = map
                    .as_ref()
                    .map(crate::map_parser::native_map_light_frame_offsets)
                    .unwrap_or_default();
                let state = self.bridge.build_render_state(
                    payload,
                    map.as_ref(),
                    &self.map_frame_offsets,
                    &native_lighting_default_motion(),
                    &self.assets,
                );
                self.publish_with(state, &mut push);
            }
            "packet" => {
                let Some(packet) = event.packet.as_deref() else {
                    return;
                };
                let payload = event.payload.as_ref().unwrap_or(&Value::Null);
                match packet {
                    "MapChanged" => self.reset_scene(),
                    "LogOutSuccess" | "ReturnToLogin" | "Disconnect" => self.reset_session(),
                    _ => {}
                }
                self.bridge.observe_packet(packet, payload);
                if matches!(
                    packet,
                    "MapChanged" | "LogOutSuccess" | "ReturnToLogin" | "Disconnect"
                ) {
                    // A reset must be visible immediately rather than waiting
                    // for the next snapshot (which can belong to a new map).
                    let state = self.bridge.build_render_state(
                        &Value::Null,
                        None,
                        &self.map_frame_offsets,
                        &native_lighting_default_motion(),
                        &self.assets,
                    );
                    self.publish_with(state, &mut push);
                }
            }
            _ => {}
        }
    }

    fn publish(&mut self, state: Value) {
        let Ok(json) = serde_json::to_string(&state) else {
            return;
        };
        if self.last_pushed_json.as_deref() == Some(json.as_str()) {
            return;
        }
        // Only a successful enqueue commits the producer's dirty state. On
        // backpressure the identical authoritative state is retried on the
        // next matching packet/snapshot.
        if mir2_bevy_runtime::native_ingest::push_native_lighting_render_state(json.clone()) {
            self.last_pushed_json = Some(json);
        }
    }

    fn publish_with(&mut self, state: Value, mut push: impl FnMut(String) -> bool) {
        let json = serde_json::to_string(&state).expect("lighting state serializes");
        if self.last_pushed_json.as_deref() != Some(json.as_str()) && push(json.clone()) {
            self.last_pushed_json = Some(json);
        }
    }
}

/// Outbound login command, matching the Web client's `{type:"login",…}`.
#[derive(Default)]
struct GatewaySessionContext {
    account_id: Option<String>,
    character_index: Option<i32>,
}

#[derive(Default)]
struct NativeResumeClientState {
    credential: Option<String>,
    expires_at_ms: Option<u64>,
    generation: Option<u64>,
    character_index: Option<i32>,
    reconnect_started_at: Option<Instant>,
    retry_attempt: u32,
}

impl std::fmt::Debug for NativeResumeClientState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeResumeClientState")
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_at_ms", &self.expires_at_ms)
            .field("generation", &self.generation)
            .field("character_index", &self.character_index)
            .field("reconnect_started_at", &self.reconnect_started_at)
            .field("retry_attempt", &self.retry_attempt)
            .finish()
    }
}

impl NativeResumeClientState {
    fn has_live_credential(&self) -> bool {
        self.credential
            .as_ref()
            .is_some_and(|credential| !credential.is_empty())
            && self
                .expires_at_ms
                .is_none_or(|expires_at_ms| expires_at_ms > gateway_unix_ms())
    }

    fn record_credential(
        &mut self,
        credential: &str,
        expires_at_ms: Option<u64>,
        generation: Option<u64>,
    ) {
        let credential = credential.trim();
        let Some(generation) = generation else {
            return;
        };
        if !valid_resume_credential(credential) {
            return;
        }
        if expires_at_ms.is_none()
            || expires_at_ms.is_some_and(|expires_at_ms| expires_at_ms <= gateway_unix_ms())
        {
            return;
        }
        if self.generation.is_some_and(|current| generation < current) {
            return;
        }
        if self.generation.is_some_and(|current| current == generation)
            && self.credential.as_deref() == Some(credential)
        {
            return;
        }
        self.credential = Some(credential.to_owned());
        self.expires_at_ms = expires_at_ms;
        self.generation = Some(generation);
        self.retry_attempt = 0;
        self.reconnect_started_at = None;
    }

    fn clear(&mut self) {
        self.credential = None;
        self.expires_at_ms = None;
        self.generation = None;
        self.character_index = None;
        self.reconnect_started_at = None;
        self.retry_attempt = 0;
    }

    fn begin_reconnect(&mut self) {
        if self.reconnect_started_at.is_none() {
            self.reconnect_started_at = Some(Instant::now());
            self.retry_attempt = 0;
        }
    }

    fn reconnect_expired(&self, config: NativeReconnectConfig) -> bool {
        self.reconnect_deadline(config)
            .is_some_and(|deadline| Instant::now() >= deadline)
            || !self.has_live_credential()
    }

    /// The reconnect budget belongs to the entire resumable lifecycle, not
    /// just the retry loop.  In particular, it remains in force while a TCP
    /// handshake, capability/resume write, or post-`sessionResumed` snapshot
    /// is outstanding.
    fn reconnect_deadline(&self, config: NativeReconnectConfig) -> Option<Instant> {
        self.reconnect_started_at
            .and_then(|started| started.checked_add(config.resume_deadline))
    }

    fn resume_credential(&self) -> Option<&str> {
        self.has_live_credential()
            .then(|| self.credential.as_deref())
            .flatten()
    }

    fn accept_resumed_generation(&mut self, generation: Option<u64>) -> bool {
        let Some(generation) = generation else {
            return false;
        };
        if !self.generation.is_some_and(|current| generation > current) {
            return false;
        }
        self.generation = Some(generation);
        true
    }
}

fn valid_resume_credential(credential: &str) -> bool {
    credential.len() == MAX_CREDENTIAL_LENGTH
        && credential.bytes().all(|byte| {
            byte.is_ascii_uppercase()
                || byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || byte == b'-'
                || byte == b'_'
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionPhase {
    Normal,
    AwaitingResume,
    Resumed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnectedExit {
    Shutdown,
    Disconnected(Option<String>),
    ResumeRejected,
    ResumeDeadlineExpired,
}

fn gateway_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct WalletState {
    gold: u32,
    credit: u32,
}

/// Packet-first authoritative HUD cursor for the native client.
///
/// Gateway snapshots may be partial during bootstrap, reconnect, and map
/// transitions. A missing JSON field means "no update"; it must not erase the
/// complete `UserInformation` values that were already delivered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NativeUiPlayerCursor {
    hp: Option<i32>,
    max_hp: Option<i32>,
    mp: Option<i32>,
    max_mp: Option<i32>,
    gold: Option<u32>,
    credit: Option<u32>,
    level: Option<u32>,
    experience: Option<i64>,
    max_experience: Option<i64>,
    current_weight: Option<u16>,
    max_weight: Option<u16>,
    name: Option<String>,
    class_name: Option<String>,
    map_name: Option<String>,
}

impl NativeUiPlayerCursor {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn observe_world_snapshot(&mut self, payload: &Value) {
        if let Some(value) = value_i32(payload.get("playerHp")) {
            self.hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("playerMaxHp")) {
            self.max_hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("playerMp")) {
            self.mp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("playerMaxMp")) {
            self.max_mp = Some(value);
        }
        if let Some(value) = value_u32(payload.get("gold")) {
            self.gold = Some(value);
        }
        if let Some(value) = value_u32(payload.get("credit")) {
            self.credit = Some(value);
        }
        if let Some(value) = value_i64(payload.get("playerExperience")) {
            self.experience = Some(value);
        }
        if let Some(value) = value_i64(payload.get("playerMaxExperience")) {
            self.max_experience = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("currentWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.current_weight = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("maxWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.max_weight = Some(value);
        }
        if let Some(value) = value_string(payload.get("mapTitle")) {
            self.map_name = Some(value);
        }

        let player_object_id = value_u32(payload.get("playerObjectId"));
        let self_player = payload
            .get("entities")
            .and_then(Value::as_array)
            .and_then(|entities| {
                entities.iter().find(|entity| {
                    entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                        || player_object_id.is_some_and(|object_id| {
                            value_u32(entity.get("objectId")) == Some(object_id)
                        })
                })
            });
        if let Some(value) = value_u32(self_player.and_then(|entity| entity.get("level"))) {
            self.level = Some(value);
        }
        if let Some(value) = value_string(self_player.and_then(|entity| entity.get("name"))) {
            self.name = Some(value);
        }
        if let Some(value) = value_string(
            self_player.and_then(|entity| entity.get("class").or_else(|| entity.get("className"))),
        ) {
            self.class_name = Some(value);
        }
    }

    fn observe_user_information(&mut self, payload: &Value) {
        if let Some(value) = value_i32(payload.get("hp").or_else(|| payload.get("playerHp"))) {
            self.hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("maxHp").or_else(|| payload.get("playerMaxHp")))
        {
            self.max_hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("mp").or_else(|| payload.get("playerMp"))) {
            self.mp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("maxMp").or_else(|| payload.get("playerMaxMp")))
        {
            self.max_mp = Some(value);
        }
        if let Some(value) = value_u32(payload.get("gold")) {
            self.gold = Some(value);
        }
        if let Some(value) = value_u32(payload.get("credit")) {
            self.credit = Some(value);
        }
        if let Some(value) = value_u32(payload.get("level")) {
            self.level = Some(value);
        }
        if let Some(value) = value_i64(
            payload
                .get("experience")
                .or_else(|| payload.get("playerExperience")),
        ) {
            self.experience = Some(value);
        }
        if let Some(value) = value_i64(
            payload
                .get("maxExperience")
                .or_else(|| payload.get("playerMaxExperience")),
        ) {
            self.max_experience = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("currentWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.current_weight = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("maxWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.max_weight = Some(value);
        }
        if let Some(value) = value_string(payload.get("name")) {
            self.name = Some(value);
        }
        if let Some(value) = value_string(payload.get("class").or_else(|| payload.get("className")))
        {
            self.class_name = Some(value);
        }
        self.observe_map_identity(payload);
    }

    fn observe_map_identity(&mut self, payload: &Value) {
        if let Some(value) = value_string(
            payload
                .get("title")
                .or_else(|| payload.get("mapTitle"))
                .or_else(|| payload.get("mapName")),
        ) {
            self.map_name = Some(value);
        }
    }

    fn to_read_model_json(&self) -> Value {
        json!({
            "player": {
                "hp": self.hp.unwrap_or_default(),
                "maxHp": self.max_hp.unwrap_or_default(),
                "mp": self.mp.unwrap_or_default(),
                "maxMp": self.max_mp.unwrap_or_default(),
                "gold": self.gold.unwrap_or_default(),
                "credit": self.credit.unwrap_or_default(),
                "level": self.level.unwrap_or_default(),
                "experience": self.experience.unwrap_or_default(),
                "maxExperience": self.max_experience.unwrap_or_default(),
                "currentWeight": self.current_weight.unwrap_or_default(),
                "maxWeight": self.max_weight.unwrap_or_default(),
                "name": self.name,
                "className": self.class_name,
                "mapName": self.map_name,
            }
        })
    }
}

/// Acknowledgement captured before Crystal's follow-up `ReceiveMail` packet.
/// The protocol has no request id, so collect is correlated to the sole
/// native claim that was actually written to this WebSocket connection.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingMailOperationFeedback {
    kind: &'static str,
    success: bool,
    mail_id: Option<u64>,
}

const MAX_PENDING_MAIL_FEEDBACK: usize = 1;

const MAX_SKILL_PACKET_PATCHES: usize = MAX_LEARNED_SKILLS;

#[derive(Debug, Clone, Default)]
struct SkillPacketPatch {
    identity: String,
    base_snapshot_tick: u64,
    /// Tick-less deltas may affect only one bounded snapshot serial. This
    /// prevents an event without an ordering tick from living forever.
    zero_tick_expires_at_snapshot_serial: Option<u64>,
    cooldown_remaining_ticks: Option<u32>,
    level: Option<u8>,
    experience: Option<u16>,
    can_use: Option<bool>,
    mp_cost: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
struct PlayerVitalsPatch {
    base_snapshot_tick: u64,
    zero_tick_expires_at_snapshot_serial: Option<u64>,
    mp: Option<i32>,
    max_mp: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct SkillRemovalPatch {
    hotkey: u8,
    base_snapshot_tick: u64,
    zero_tick_expires_at_snapshot_serial: Option<u64>,
}

/// Packet-first cursor for personal skill/vital deltas. The gateway does not
/// expose a server sequence on these browser events, so the last snapshot
/// tick at packet arrival is used as a bounded stale-snapshot fence: snapshots
/// at or before that tick cannot overwrite the packet delta; a later snapshot
/// is accepted as the new authority and retires the patch.
#[derive(Debug, Default)]
struct SkillPacketCursor {
    snapshot_serial: u64,
    patches: Vec<SkillPacketPatch>,
    removals: Vec<SkillRemovalPatch>,
    vitals: Option<PlayerVitalsPatch>,
    player_object_id: Option<u32>,
}

impl SkillPacketCursor {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn observe_snapshot(&mut self, payload: &mut Value) {
        self.snapshot_serial = self.snapshot_serial.saturating_add(1);
        self.player_object_id = value_u32(payload.get("playerObjectId"));
        let snapshot_tick = world_payload_tick_from(Some(payload));
        let snapshot_serial = self.snapshot_serial;
        self.patches
            .retain(|patch| Self::patch_is_active(patch, snapshot_tick, snapshot_serial));
        self.removals
            .retain(|patch| Self::removal_is_active(patch, snapshot_tick, snapshot_serial));
        if self
            .vitals
            .is_some_and(|patch| !Self::vitals_is_active(&patch, snapshot_tick, snapshot_serial))
        {
            self.vitals = None;
        }
        self.apply_active_patches(payload, snapshot_tick);
        // A tick-less patch is valid for the snapshot that is being observed,
        // then retires even if future snapshots keep reporting tick=0.
        self.patches
            .retain(|patch| patch.zero_tick_expires_at_snapshot_serial != Some(snapshot_serial));
        self.removals
            .retain(|patch| patch.zero_tick_expires_at_snapshot_serial != Some(snapshot_serial));
        if self.vitals.is_some_and(|patch| {
            patch.zero_tick_expires_at_snapshot_serial == Some(snapshot_serial)
        }) {
            self.vitals = None;
        }
    }

    fn apply_active_patches(&self, payload: &mut Value, snapshot_tick: u64) {
        if let Some(skills) = skill_array_mut(payload) {
            skills.truncate(MAX_LEARNED_SKILLS);

            for skill in skills.iter_mut() {
                if self.removals.iter().any(|removal| {
                    skill_hotkey(skill) == Some(removal.hotkey)
                        && Self::removal_is_active(removal, snapshot_tick, self.snapshot_serial)
                }) {
                    continue;
                }
                for patch in self.patches.iter().filter(|patch| {
                    Self::patch_is_active(patch, snapshot_tick, self.snapshot_serial)
                }) {
                    if skill_matches_identity(skill, &patch.identity) {
                        apply_skill_patch(skill, patch);
                    }
                }
            }

            skills.retain(|skill| {
                !self.removals.iter().any(|removal| {
                    skill_hotkey(skill) == Some(removal.hotkey)
                        && Self::removal_is_active(removal, snapshot_tick, self.snapshot_serial)
                })
            });
        }

        if let Some(vitals) = self.vitals {
            if Self::vitals_is_active(&vitals, snapshot_tick, self.snapshot_serial) {
                if let Some(mp) = vitals.mp {
                    payload["playerMp"] = json!(mp);
                }
                if let Some(max_mp) = vitals.max_mp {
                    payload["playerMaxMp"] = json!(max_mp);
                }
            }
        }
    }

    fn patch_is_active(patch: &SkillPacketPatch, snapshot_tick: u64, snapshot_serial: u64) -> bool {
        patch
            .zero_tick_expires_at_snapshot_serial
            .map(|expires| snapshot_serial <= expires)
            .unwrap_or(snapshot_tick == 0 || snapshot_tick <= patch.base_snapshot_tick)
    }

    fn removal_is_active(
        patch: &SkillRemovalPatch,
        snapshot_tick: u64,
        snapshot_serial: u64,
    ) -> bool {
        patch
            .zero_tick_expires_at_snapshot_serial
            .map(|expires| snapshot_serial <= expires)
            .unwrap_or(snapshot_tick == 0 || snapshot_tick <= patch.base_snapshot_tick)
    }

    fn vitals_is_active(
        patch: &PlayerVitalsPatch,
        snapshot_tick: u64,
        snapshot_serial: u64,
    ) -> bool {
        patch
            .zero_tick_expires_at_snapshot_serial
            .map(|expires| snapshot_serial <= expires)
            .unwrap_or(snapshot_tick == 0 || snapshot_tick <= patch.base_snapshot_tick)
    }

    fn apply_packet(&mut self, packet: &str, payload: &Value, base_snapshot_tick: u64) -> bool {
        match packet {
            "UserInformation" => {
                let Some(object_id) = value_u32(payload.get("objectId")) else {
                    return false;
                };
                self.player_object_id = Some(object_id);
                self.vitals = Some(PlayerVitalsPatch {
                    base_snapshot_tick,
                    zero_tick_expires_at_snapshot_serial: zero_tick_expiry_serial(
                        self.snapshot_serial,
                        base_snapshot_tick,
                    ),
                    mp: value_i32(payload.get("mp").or_else(|| payload.get("playerMp"))),
                    max_mp: value_i32(payload.get("maxMp").or_else(|| payload.get("playerMaxMp"))),
                });
                if self
                    .vitals
                    .is_some_and(|patch| patch.mp.is_none() && patch.max_mp.is_none())
                {
                    self.vitals = None;
                    return false;
                }
                true
            }
            "MagicDelay" => {
                if !self.packet_targets_player(payload) {
                    return false;
                }
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                let Some(delay) = value_u32(payload.get("delay")) else {
                    return false;
                };
                self.upsert_patch(identity, base_snapshot_tick, |patch| {
                    patch.cooldown_remaining_ticks = Some(delay);
                    if let Some(mp_cost) =
                        value_u32(payload.get("mpCost").or_else(|| payload.get("mp_cost")))
                    {
                        patch.mp_cost = Some(mp_cost);
                    }
                });
                true
            }
            "MagicLeveled" => {
                if !self.packet_targets_player(payload) {
                    return false;
                }
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                let Some(level) =
                    value_u32(payload.get("level")).and_then(|value| u8::try_from(value).ok())
                else {
                    return false;
                };
                self.upsert_patch(identity, base_snapshot_tick, |patch| {
                    patch.level = Some(level);
                    patch.experience = value_u32(payload.get("experience"))
                        .and_then(|value| u16::try_from(value).ok());
                });
                true
            }
            "SpellToggle" => {
                if !self.packet_targets_player(payload) {
                    return false;
                }
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                let Some(can_use) = payload.get("canUse").and_then(Value::as_bool) else {
                    return false;
                };
                self.upsert_patch(identity, base_snapshot_tick, |patch| {
                    patch.can_use = Some(can_use);
                });
                true
            }
            "RemoveMagic" => {
                let Some(hotkey) = value_u32(payload.get("placeId"))
                    .and_then(|value| (1..=8).contains(&value).then_some(value as u8))
                else {
                    return false;
                };
                if self.removals.len() >= MAX_SKILL_PACKET_PATCHES {
                    self.removals.remove(0);
                }
                self.removals.push(SkillRemovalPatch {
                    hotkey,
                    base_snapshot_tick,
                    zero_tick_expires_at_snapshot_serial: zero_tick_expiry_serial(
                        self.snapshot_serial,
                        base_snapshot_tick,
                    ),
                });
                true
            }
            _ => false,
        }
    }

    fn packet_targets_player(&self, payload: &Value) -> bool {
        // The authoritative SpellToggle packet always carries an object id.
        // Treat a missing/malformed id as an invalid packet instead of letting
        // it mutate the local player's skill state by default.
        let Some(object_id) = value_u32(payload.get("objectId")) else {
            return false;
        };
        object_id != 0
            && self
                .player_object_id
                .map(|player_id| player_id != 0 && player_id == object_id)
                .unwrap_or(false)
    }

    fn upsert_patch(
        &mut self,
        identity: String,
        base_snapshot_tick: u64,
        update: impl FnOnce(&mut SkillPacketPatch),
    ) {
        let zero_tick_expires_at_snapshot_serial =
            zero_tick_expiry_serial(self.snapshot_serial, base_snapshot_tick);
        if let Some(patch) = self
            .patches
            .iter_mut()
            .find(|patch| patch.identity == identity)
        {
            patch.base_snapshot_tick = base_snapshot_tick;
            patch.zero_tick_expires_at_snapshot_serial = zero_tick_expires_at_snapshot_serial;
            update(patch);
            return;
        }
        if self.patches.len() >= MAX_SKILL_PACKET_PATCHES {
            self.patches.remove(0);
        }
        let mut patch = SkillPacketPatch {
            identity,
            base_snapshot_tick,
            zero_tick_expires_at_snapshot_serial,
            ..Default::default()
        };
        update(&mut patch);
        self.patches.push(patch);
    }
}

fn zero_tick_expiry_serial(snapshot_serial: u64, base_snapshot_tick: u64) -> Option<u64> {
    (base_snapshot_tick == 0).then(|| snapshot_serial.saturating_add(1))
}

fn world_payload_tick_from(payload: Option<&Value>) -> u64 {
    payload
        .and_then(|payload| {
            value_u64(payload.get("tick"))
                .or_else(|| value_u64(payload.get("snapshotTick")))
                .or_else(|| value_u64(payload.get("snapshot_tick")))
        })
        .unwrap_or(0)
}

fn skill_array_mut(payload: &mut Value) -> Option<&mut Vec<Value>> {
    if payload.get("knownSkills").is_some() {
        return payload.get_mut("knownSkills").and_then(Value::as_array_mut);
    }
    if payload.get("known_skills").is_some() {
        return payload
            .get_mut("known_skills")
            .and_then(Value::as_array_mut);
    }
    payload.get_mut("skills").and_then(Value::as_array_mut)
}

fn packet_spell_identity(payload: &Value) -> Option<String> {
    let spell = payload.get("spell").and_then(Value::as_str)?.trim();
    (!spell.is_empty()).then(|| spell.to_ascii_lowercase())
}

fn skill_hotkey(skill: &Value) -> Option<u8> {
    value_u32(skill.get("hotkey").or_else(|| skill.get("key")))
        .and_then(|value| u8::try_from(value).ok())
}

fn skill_matches_identity(skill: &Value, identity: &str) -> bool {
    // Packet spell ids may only patch a snapshot's authoritative spell field.
    // Display names and local keys are not protocol identifiers.
    skill
        .get("spell")
        .and_then(Value::as_str)
        .map(|value| value.trim().eq_ignore_ascii_case(identity))
        .unwrap_or(false)
}

fn apply_skill_patch(skill: &mut Value, patch: &SkillPacketPatch) {
    if let Some(cooldown) = patch.cooldown_remaining_ticks {
        skill["cooldownRemainingTicks"] = json!(cooldown);
    }
    if let Some(level) = patch.level {
        skill["level"] = json!(level);
    }
    if let Some(experience) = patch.experience {
        skill["experience"] = json!(experience);
    }
    if let Some(can_use) = patch.can_use {
        skill["canUse"] = json!(can_use);
    }
    if let Some(mp_cost) = patch.mp_cost {
        skill["mpCost"] = json!(mp_cost);
    }
}

/// Connect to the gateway, accept validated native UI/gameplay commands, and
/// forward authoritative packets/snapshots into the shared runtime. A native
/// connection that already received a resume credential retries inside the
/// bounded reconnect window; a normal Web-compatible connection still waits
/// for the visible Retry action after it fails.
pub async fn run_gateway_client<R: CommandSource + Send>(
    base_url: &str,
    commands: R,
    shell_events: std::sync::mpsc::Sender<ShellGatewayEvent>,
    gameplay_events: std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    reconnect_config: NativeReconnectConfig,
) -> Result<(), String> {
    run_gateway_client_with_world_ingest(
        base_url,
        commands,
        shell_events,
        gameplay_events,
        reconnect_config,
        mir2_bevy_runtime::native_ingest::push_native_world_state,
    )
    .await
}

async fn run_gateway_client_with_world_ingest<R, F>(
    base_url: &str,
    mut commands: R,
    shell_events: std::sync::mpsc::Sender<ShellGatewayEvent>,
    gameplay_events: std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    reconnect_config: NativeReconnectConfig,
    mut push_world_state: F,
) -> Result<(), String>
where
    R: CommandSource + Send,
    F: FnMut(String) -> bool,
{
    let mut should_connect = true;
    let mut generation = 0_u64;
    let mut resume_state = NativeResumeClientState::default();
    let mut game_shop_receipt_gate = GameShopReceiptGate::default();
    let mut retry_delay = None;
    loop {
        if let Some(delay) = retry_delay.take() {
            match wait_for_retry_or_leave_until(
                &mut commands,
                delay,
                reconnect_config.command_batch_limit,
                &mut game_shop_receipt_gate,
                resume_state
                    .reconnect_deadline(reconnect_config)
                    .map(tokio::time::Instant::from_std),
            )
            .await?
            {
                RetryWait::Elapsed => {}
                RetryWait::Connect => {}
                RetryWait::Leave => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::ResumeCancelled,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                        reason: Some("native reconnect cancelled".to_owned()),
                    });
                    should_connect = false;
                    continue;
                }
                RetryWait::Shutdown => return Ok(()),
                RetryWait::Deadline => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::RetryExhausted,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                        reason: Some("gateway reconnect deadline expired".to_owned()),
                    });
                    should_connect = false;
                    continue;
                }
            }
        }
        if !should_connect {
            should_connect = wait_for_connect_request(
                &mut commands,
                reconnect_config.command_batch_limit,
                &mut game_shop_receipt_gate,
            )
            .await?;
            if !should_connect {
                return Ok(());
            }
        }

        let attempting_resume = resume_state.reconnect_started_at.is_some()
            && resume_state.resume_credential().is_some();
        if attempting_resume {
            if resume_state.retry_attempt >= u32::from(reconnect_config.max_attempts)
                || resume_state.reconnect_expired(reconnect_config)
            {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::RetryExhausted,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                    reason: Some("gateway reconnect deadline expired".to_owned()),
                });
                should_connect = false;
                continue;
            }
            resume_state.retry_attempt = resume_state.retry_attempt.saturating_add(1);
        }

        let mut socket = match connect_gateway_with_resume_controls(
            base_url,
            &mut commands,
            attempting_resume,
            resume_state
                .reconnect_deadline(reconnect_config)
                .map(tokio::time::Instant::from_std),
            reconnect_config.command_batch_limit,
            &mut game_shop_receipt_gate,
        )
        .await
        {
            ResumeLifecycle::Complete(socket) => socket,
            ResumeLifecycle::Cancel => {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::ResumeCancelled,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                    reason: Some("native reconnect cancelled".to_owned()),
                });
                should_connect = false;
                continue;
            }
            ResumeLifecycle::Shutdown => return Ok(()),
            ResumeLifecycle::Deadline => {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::RetryExhausted,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                    reason: Some("gateway reconnect deadline expired".to_owned()),
                });
                should_connect = false;
                continue;
            }
            ResumeLifecycle::Failed(error) => {
                match resume_handshake_failure_transition(
                    attempting_resume,
                    &resume_state,
                    reconnect_config,
                ) {
                    ResumeHandshakeFailure::Retry => {
                        retry_delay = Some(retry_delay_for(
                            reconnect_config,
                            resume_state.retry_attempt,
                            generation,
                        ));
                    }
                    ResumeHandshakeFailure::TerminalDataReset => {
                        let _ = apply_outer_terminal_transition(
                            OuterTerminalTransition::RetryExhausted,
                            &mut resume_state,
                            &mut game_shop_receipt_gate,
                        );
                        let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                            reason: Some("gateway reconnect unavailable".to_owned()),
                        });
                        should_connect = false;
                    }
                    ResumeHandshakeFailure::InitialDataReset => {
                        let _ = apply_outer_terminal_transition(
                            OuterTerminalTransition::NoCredentialDisconnect,
                            &mut resume_state,
                            &mut game_shop_receipt_gate,
                        );
                        let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                            reason: Some(format!("gateway connect failed: {error}")),
                        });
                        should_connect = false;
                    }
                }
                continue;
            }
        };

        generation = generation.wrapping_add(1);
        eprintln!("[gateway-client] connected generation={generation} resume={attempting_resume}");
        let mut phase = if attempting_resume {
            ConnectionPhase::AwaitingResume
        } else {
            ConnectionPhase::Normal
        };
        let mut resume_scene_reset_sent = false;
        let handshake_result = send_resume_handshake_with_resume_controls(
            &mut socket,
            attempting_resume
                .then(|| resume_state.resume_credential())
                .flatten(),
            &mut commands,
            attempting_resume,
            resume_state
                .reconnect_deadline(reconnect_config)
                .map(tokio::time::Instant::from_std),
            reconnect_config.command_batch_limit,
            &mut game_shop_receipt_gate,
        )
        .await;
        if !matches!(handshake_result, ResumeLifecycle::Complete(())) {
            let error = match handshake_result {
                ResumeLifecycle::Cancel => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::ResumeCancelled,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                        reason: Some("native reconnect cancelled".to_owned()),
                    });
                    should_connect = false;
                    continue;
                }
                ResumeLifecycle::Shutdown => return Ok(()),
                ResumeLifecycle::Deadline => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::RetryExhausted,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                        reason: Some("gateway reconnect deadline expired".to_owned()),
                    });
                    should_connect = false;
                    continue;
                }
                ResumeLifecycle::Complete(()) => unreachable!("matched above"),
                ResumeLifecycle::Failed(error) => error,
            };
            match resume_handshake_failure_transition(
                attempting_resume,
                &resume_state,
                reconnect_config,
            ) {
                ResumeHandshakeFailure::Retry => {
                    retry_delay = Some(retry_delay_for(
                        reconnect_config,
                        resume_state.retry_attempt,
                        generation,
                    ));
                }
                ResumeHandshakeFailure::TerminalDataReset => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::RetryExhausted,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                        reason: Some("gateway reconnect handshake unavailable".to_owned()),
                    });
                    should_connect = false;
                }
                ResumeHandshakeFailure::InitialDataReset => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::NoCredentialDisconnect,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                        reason: Some(format!("gateway handshake failed: {error}")),
                    });
                    should_connect = false;
                }
            }
            continue;
        }
        if !attempting_resume {
            let _ = shell_events.send(ShellGatewayEvent::Connected);
        }
        let exit = run_connected_gateway(
            socket,
            &mut commands,
            &shell_events,
            &gameplay_events,
            generation,
            reconnect_config,
            &mut resume_state,
            &mut phase,
            &mut resume_scene_reset_sent,
            &mut game_shop_receipt_gate,
            &mut push_world_state,
        )
        .await;
        match exit {
            Ok(ConnectedExit::Shutdown) => return Ok(()),
            Ok(ConnectedExit::ResumeRejected) => {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::ResumeRejected,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                    reason: Some("session resume unavailable".to_owned()),
                });
                should_connect = false;
            }
            Ok(ConnectedExit::ResumeDeadlineExpired) => {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::RetryExhausted,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                    reason: Some("gateway reconnect deadline expired".to_owned()),
                });
                should_connect = false;
            }
            Ok(ConnectedExit::Disconnected(reason)) => {
                if resume_state.has_live_credential() {
                    resume_state.begin_reconnect();
                    if resume_state.reconnect_expired(reconnect_config)
                        || reconnect_config.max_attempts == 0
                    {
                        let _ = apply_outer_terminal_transition(
                            OuterTerminalTransition::RetryExhausted,
                            &mut resume_state,
                            &mut game_shop_receipt_gate,
                        );
                        let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                            reason: Some("gateway reconnect unavailable".to_owned()),
                        });
                        should_connect = false;
                    } else {
                        retry_delay = Some(retry_delay_for(reconnect_config, 0, generation));
                    }
                } else {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::NoCredentialDisconnect,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                        reason: reason.or_else(|| Some("connection closed".to_owned())),
                    });
                    should_connect = false;
                }
            }
            Err(reason) => {
                if resume_state.has_live_credential() {
                    resume_state.begin_reconnect();
                    retry_delay = Some(retry_delay_for(reconnect_config, 0, generation));
                } else {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::NoCredentialDisconnect,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send(ShellGatewayEvent::Disconnect {
                        reason: Some(reason),
                    });
                    should_connect = false;
                }
            }
        }
    }
}

async fn wait_for_connect_request<R: CommandSource>(
    commands: &mut R,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> Result<bool, String> {
    wait_for_connect_request_with_reset(
        commands,
        batch_limit,
        game_shop_receipt_gate,
        mir2_bevy_runtime::native_ingest::push_native_data_reset,
    )
    .await
}

async fn wait_for_connect_request_with_reset<R, F>(
    commands: &mut R,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
) -> Result<bool, String>
where
    R: CommandSource,
    F: FnMut() -> bool,
{
    let mut poll = tokio::time::interval(Duration::from_millis(25));
    loop {
        poll.tick().await;
        let batch = drain_command_batch(commands, batch_limit);
        // Scan transactions before honoring Connect/Leave from the same
        // batch; drain_command_batch deliberately appends reserved lanes and
        // a control command may otherwise return first.
        for command in &batch {
            let _ = discard_correlated_before_socket_write(
                command,
                game_shop_receipt_gate,
                &mut push_data_reset,
            );
        }
        for command in batch {
            if is_game_shop_transaction(&command) {
                continue;
            }
            match command {
                GatewayCommand::Connect => return Ok(true),
                GatewayCommand::Shutdown => return Ok(false),
                GatewayCommand::Wire(NativeOutboundCommand::LogOut)
                | GatewayCommand::Wire(NativeOutboundCommand::Disconnect) => return Ok(false),
                GatewayCommand::Wire(_) | GatewayCommand::Player(_) => {}
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetryWait {
    Elapsed,
    Connect,
    Leave,
    Shutdown,
    Deadline,
}

async fn wait_for_retry_or_leave<R: CommandSource>(
    commands: &mut R,
    delay: Duration,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> Result<RetryWait, String> {
    wait_for_retry_or_leave_until(commands, delay, batch_limit, game_shop_receipt_gate, None).await
}

async fn wait_for_retry_or_leave_until<R: CommandSource>(
    commands: &mut R,
    delay: Duration,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    resume_deadline: Option<tokio::time::Instant>,
) -> Result<RetryWait, String> {
    wait_for_retry_or_leave_with_reset(
        commands,
        delay,
        batch_limit,
        game_shop_receipt_gate,
        resume_deadline,
        mir2_bevy_runtime::native_ingest::push_native_data_reset,
    )
    .await
}

async fn wait_for_retry_or_leave_with_reset<R, F>(
    commands: &mut R,
    delay: Duration,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    resume_deadline: Option<tokio::time::Instant>,
    mut push_data_reset: F,
) -> Result<RetryWait, String>
where
    R: CommandSource,
    F: FnMut() -> bool,
{
    let deadline = tokio::time::sleep(delay);
    tokio::pin!(deadline);
    let resume_timeout = async {
        if let Some(deadline) = resume_deadline {
            tokio::time::sleep_until(deadline).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(resume_timeout);
    let mut poll = tokio::time::interval(Duration::from_millis(25));
    loop {
        tokio::select! {
            _ = &mut resume_timeout => return Ok(RetryWait::Deadline),
            _ = &mut deadline => return Ok(RetryWait::Elapsed),
            _ = poll.tick() => {
                let batch = drain_command_batch(commands, batch_limit);
                for command in &batch {
                    let _ = discard_correlated_before_socket_write(
                        command,
                        game_shop_receipt_gate,
                        &mut push_data_reset,
                    );
                }
                for command in batch {
                    if is_game_shop_transaction(&command) {
                        continue;
                    }
                    match command {
                        GatewayCommand::Shutdown => return Ok(RetryWait::Shutdown),
                        GatewayCommand::Connect => return Ok(RetryWait::Connect),
                        GatewayCommand::Wire(NativeOutboundCommand::LogOut)
                        | GatewayCommand::Wire(NativeOutboundCommand::Disconnect) => {
                            return Ok(RetryWait::Leave)
                        }
                        GatewayCommand::Wire(_) | GatewayCommand::Player(_) => {}
                    }
                }
            }
        }
    }
}

fn drain_command_batch<R: CommandSource>(
    commands: &mut R,
    batch_limit: usize,
) -> Vec<GatewayCommand> {
    let limit = batch_limit.clamp(1, MAX_COMMANDS_PER_POLL);
    let mut batch = Vec::with_capacity(limit);
    let mut latest_player = None;
    let mut leave = None;
    let mut transaction = None;
    while let Ok(command) = commands.try_command() {
        match command {
            GatewayCommand::Shutdown => return vec![GatewayCommand::Shutdown],
            GatewayCommand::Connect => {
                if batch.len() < limit {
                    batch.push(GatewayCommand::Connect);
                }
            }
            GatewayCommand::Player(intent) => latest_player = Some(GatewayCommand::Player(intent)),
            GatewayCommand::Wire(NativeOutboundCommand::LogOut)
            | GatewayCommand::Wire(NativeOutboundCommand::Disconnect) => {
                leave = Some(command);
            }
            other if is_correlated_transaction(&other) => {
                transaction = Some(other);
                // A bounded receiver takes the reserved transaction slot
                // atomically. Stop this drain immediately so a second
                // transaction concurrently inserted after that take remains
                // in the slot for the next poll instead of being accepted and
                // silently discarded in this batch.
                break;
            }
            other if batch.len() < limit => batch.push(other),
            _ => {}
        }
    }
    let reserved = usize::from(transaction.is_some())
        .saturating_add(usize::from(leave.is_some()))
        .saturating_add(usize::from(latest_player.is_some()));
    while batch.len().saturating_add(reserved) > limit && !batch.is_empty() {
        batch.pop();
    }
    if let Some(transaction) = transaction {
        batch.push(transaction);
    }
    if let Some(leave) = leave {
        if batch.len() == limit && reserved <= limit {
            batch.pop();
        }
        batch.push(leave);
    }
    if let Some(player) = latest_player {
        if batch.len() == limit && reserved <= limit {
            batch.pop();
        }
        batch.push(player);
    }
    batch
}

fn retry_delay_for(config: NativeReconnectConfig, attempt: u32, generation: u64) -> Duration {
    let exponent = attempt.min(6);
    let base_ms = config
        .initial_backoff
        .as_millis()
        .saturating_mul(1u128 << exponent)
        .min(config.max_backoff.as_millis()) as u64;
    let mut state = generation
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(attempt));
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    let span = u64::from(config.jitter_percent).saturating_mul(2);
    let offset_percent = if span == 0 {
        0
    } else {
        (state % (span + 1)) as i64 - i64::from(config.jitter_percent)
    };
    let adjusted = (i128::from(base_ms) * i128::from(100 + offset_percent) / 100)
        .max(1)
        .min(i128::from(config.max_backoff.as_millis() as u64)) as u64;
    Duration::from_millis(adjusted)
}

type GatewaySocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Every operation inside a native resume attempt is governed by the same
/// absolute deadline and command fence.  A regular first connection retains
/// its existing behavior; only a reconnect with a live credential enters this
/// lifecycle.
#[derive(Debug)]
enum ResumeLifecycle<T> {
    Complete(T),
    Cancel,
    Shutdown,
    Deadline,
    Failed(String),
}

fn drain_resume_lifecycle_commands<R: CommandSource>(
    commands: &mut R,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> ResumeLifecycle<()> {
    drain_resume_lifecycle_commands_with_reset(
        commands,
        batch_limit,
        game_shop_receipt_gate,
        mir2_bevy_runtime::native_ingest::push_native_data_reset,
    )
}

fn drain_resume_lifecycle_commands_with_reset<R, F>(
    commands: &mut R,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
) -> ResumeLifecycle<()>
where
    R: CommandSource,
    F: FnMut() -> bool,
{
    for command in drain_command_batch(commands, batch_limit) {
        if discard_correlated_before_socket_write(
            &command,
            game_shop_receipt_gate,
            &mut push_data_reset,
        ) {
            continue;
        }
        match awaiting_resume_command_action(&command) {
            AwaitingResumeCommandAction::Shutdown => return ResumeLifecycle::Shutdown,
            AwaitingResumeCommandAction::Cancel => return ResumeLifecycle::Cancel,
            // All ordinary/gameplay commands are deliberately consumed here.
            // There is no replay queue across a reconnect boundary.
            AwaitingResumeCommandAction::Ignore => {}
        }
    }
    ResumeLifecycle::Complete(())
}

async fn connect_gateway_with_resume_controls<R: CommandSource>(
    base_url: &str,
    commands: &mut R,
    attempting_resume: bool,
    resume_deadline: Option<tokio::time::Instant>,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> ResumeLifecycle<GatewaySocket> {
    if !attempting_resume {
        return tokio_tungstenite::connect_async(base_url)
            .await
            .map(|(socket, _)| ResumeLifecycle::Complete(socket))
            .unwrap_or_else(|error| ResumeLifecycle::Failed(error.to_string()));
    }

    let connect = tokio_tungstenite::connect_async(base_url);
    tokio::pin!(connect);
    let resume_timeout = async {
        if let Some(deadline) = resume_deadline {
            tokio::time::sleep_until(deadline).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(resume_timeout);
    let mut poll = tokio::time::interval(Duration::from_millis(8));
    loop {
        tokio::select! {
            biased;
            _ = &mut resume_timeout => return ResumeLifecycle::Deadline,
            _ = poll.tick() => match drain_resume_lifecycle_commands(
                commands,
                batch_limit,
                game_shop_receipt_gate,
            ) {
                ResumeLifecycle::Complete(()) => {}
                ResumeLifecycle::Cancel => return ResumeLifecycle::Cancel,
                ResumeLifecycle::Shutdown => return ResumeLifecycle::Shutdown,
                _ => unreachable!("command drain only returns terminal controls"),
            },
            connected = &mut connect => return match connected {
                Ok((socket, _)) => ResumeLifecycle::Complete(socket),
                Err(error) => ResumeLifecycle::Failed(error.to_string()),
            },
        }
    }
}

async fn send_resume_frame_with_controls<R: CommandSource>(
    socket: &mut GatewaySocket,
    payload: Value,
    commands: &mut R,
    resume_deadline: Option<tokio::time::Instant>,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> ResumeLifecycle<()> {
    let send = socket.send(Message::Text(payload.to_string().into()));
    tokio::pin!(send);
    let resume_timeout = async {
        if let Some(deadline) = resume_deadline {
            tokio::time::sleep_until(deadline).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(resume_timeout);
    let mut poll = tokio::time::interval(Duration::from_millis(8));
    loop {
        tokio::select! {
            biased;
            _ = &mut resume_timeout => return ResumeLifecycle::Deadline,
            _ = poll.tick() => match drain_resume_lifecycle_commands(
                commands,
                batch_limit,
                game_shop_receipt_gate,
            ) {
                ResumeLifecycle::Complete(()) => {}
                ResumeLifecycle::Cancel => return ResumeLifecycle::Cancel,
                ResumeLifecycle::Shutdown => return ResumeLifecycle::Shutdown,
                _ => unreachable!("command drain only returns terminal controls"),
            },
            sent = &mut send => return sent
                .map(|_| ResumeLifecycle::Complete(()))
                .unwrap_or_else(|error| ResumeLifecycle::Failed(format!(
                    "gateway resume send failed: {error}"
                ))),
        }
    }
}

async fn send_resume_handshake(
    socket: &mut GatewaySocket,
    credential: Option<&str>,
) -> Result<(), String> {
    let capability = NativeOutboundCommand::ClientCapabilities {
        capabilities: vec![
            NATIVE_RESUME_PROTOCOL.to_owned(),
            NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_owned(),
        ],
    }
    .to_wire_json();
    socket
        .send(Message::Text(capability.to_string().into()))
        .await
        .map_err(|error| format!("gateway capability send failed: {error}"))?;
    if let Some(credential) = credential {
        if credential.len() > MAX_CREDENTIAL_LENGTH {
            return Err("native resume credential rejected".to_owned());
        }
        let payload = NativeOutboundCommand::ResumeSession {
            credential: credential.to_owned(),
        }
        .to_wire_json();
        socket
            .send(Message::Text(payload.to_string().into()))
            .await
            .map_err(|error| format!("gateway resume send failed: {error}"))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumeHandshakeFailure {
    Retry,
    TerminalDataReset,
    InitialDataReset,
}

/// Decide what the outer connection loop must do after either the socket
/// connect or the capability/resume handshake fails. This is deliberately a
/// pure transition: a transient resume failure preserves the read models and
/// gets another bounded attempt; only exhaustion crosses the session boundary.
fn resume_handshake_failure_transition(
    attempting_resume: bool,
    resume_state: &NativeResumeClientState,
    reconnect_config: NativeReconnectConfig,
) -> ResumeHandshakeFailure {
    if !attempting_resume {
        return ResumeHandshakeFailure::InitialDataReset;
    }
    if resume_state.retry_attempt >= u32::from(reconnect_config.max_attempts)
        || resume_state.reconnect_expired(reconnect_config)
    {
        ResumeHandshakeFailure::TerminalDataReset
    } else {
        ResumeHandshakeFailure::Retry
    }
}

async fn send_resume_handshake_with_resume_controls<R: CommandSource>(
    socket: &mut GatewaySocket,
    credential: Option<&str>,
    commands: &mut R,
    attempting_resume: bool,
    resume_deadline: Option<tokio::time::Instant>,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> ResumeLifecycle<()> {
    if !attempting_resume {
        return send_resume_handshake(socket, credential)
            .await
            .map(|()| ResumeLifecycle::Complete(()))
            .unwrap_or_else(ResumeLifecycle::Failed);
    }
    let capability = NativeOutboundCommand::ClientCapabilities {
        capabilities: vec![
            NATIVE_RESUME_PROTOCOL.to_owned(),
            NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_owned(),
        ],
    }
    .to_wire_json();
    match send_resume_frame_with_controls(
        socket,
        capability,
        commands,
        resume_deadline,
        batch_limit,
        game_shop_receipt_gate,
    )
    .await
    {
        ResumeLifecycle::Complete(()) => {}
        ResumeLifecycle::Cancel => return ResumeLifecycle::Cancel,
        ResumeLifecycle::Shutdown => return ResumeLifecycle::Shutdown,
        ResumeLifecycle::Deadline => return ResumeLifecycle::Deadline,
        ResumeLifecycle::Failed(error) => return ResumeLifecycle::Failed(error),
    }
    let Some(credential) = credential else {
        return ResumeLifecycle::Failed("native resume credential missing".to_owned());
    };
    if credential.len() > MAX_CREDENTIAL_LENGTH {
        return ResumeLifecycle::Failed("native resume credential rejected".to_owned());
    }
    send_resume_frame_with_controls(
        socket,
        NativeOutboundCommand::ResumeSession {
            credential: credential.to_owned(),
        }
        .to_wire_json(),
        commands,
        resume_deadline,
        batch_limit,
        game_shop_receipt_gate,
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AwaitingResumeCommandAction {
    Ignore,
    Cancel,
    Shutdown,
}

fn awaiting_resume_command_action(command: &GatewayCommand) -> AwaitingResumeCommandAction {
    match command {
        GatewayCommand::Shutdown => AwaitingResumeCommandAction::Shutdown,
        GatewayCommand::Wire(NativeOutboundCommand::LogOut)
        | GatewayCommand::Wire(NativeOutboundCommand::Disconnect) => {
            AwaitingResumeCommandAction::Cancel
        }
        GatewayCommand::Connect | GatewayCommand::Wire(_) | GatewayCommand::Player(_) => {
            AwaitingResumeCommandAction::Ignore
        }
    }
}

fn record_resume_credential_if_allowed(
    phase: ConnectionPhase,
    resume_state: &mut NativeResumeClientState,
    credential: &str,
    expires_at_ms: Option<u64>,
    generation: Option<u64>,
) {
    // A fresh socket can replay the server's credential before the resume
    // decision arrives. It must not replace the credential or reset the
    // deadline/attempt budget for the in-flight reconnect operation.
    if phase == ConnectionPhase::Normal {
        resume_state.record_credential(credential, expires_at_ms, generation);
    }
}

async fn run_connected_gateway<R, F>(
    mut socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    commands: &mut R,
    shell_events: &std::sync::mpsc::Sender<ShellGatewayEvent>,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    generation: u64,
    reconnect_config: NativeReconnectConfig,
    resume_state: &mut NativeResumeClientState,
    phase: &mut ConnectionPhase,
    resume_scene_reset_sent: &mut bool,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    push_world_state: &mut F,
) -> Result<ConnectedExit, String>
where
    R: CommandSource,
    F: FnMut(String) -> bool,
{
    let resume_deadline = resume_state
        .reconnect_deadline(reconnect_config)
        .map(tokio::time::Instant::from_std);
    let resume_timeout = async {
        if let Some(deadline) = resume_deadline {
            tokio::time::sleep_until(deadline).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(resume_timeout);
    let mut context = GatewaySessionContext::default();
    let mut gameplay_adapter = NativeGameplayAdapter::default();
    gameplay_adapter.set_generation(generation);
    // Every WebSocket generation owns an isolated lighting lifecycle. A
    // reconnect must never retain the previous map's darkness or emitters.
    let mut lighting_publisher = NativeLightingPublisher::for_connection(generation);
    lighting_publisher.push_clear_state();
    let mut last_world_payload: Option<Value> = None;
    let mut last_wallet: Option<WalletState> = None;
    let mut ui_cursor = NativeUiPlayerCursor::default();
    let mut skill_cursor = SkillPacketCursor::default();
    let mut social_cursor = SocialModel::default();
    let mut in_flight_claim_mail_id: Option<u64> = None;
    let mut send_mail_in_flight = false;
    let mut pending_mail_feedback = VecDeque::new();
    let mut keepalive = tokio::time::interval(Duration::from_secs(15));
    let mut input_poll = tokio::time::interval(Duration::from_millis(8));
    let mut snapshot_log_counter: u32 = 0;
    // The visible shell must not enter InGame until this socket has delivered
    // a world snapshot that the render runtime actually accepted.
    let mut connection_bootstrap_sent = false;
    loop {
        tokio::select! {
            // This remains armed through `sessionResumed` and is disabled only
            // after the first authoritative worldSnapshot has been applied.
            _ = &mut resume_timeout, if *phase != ConnectionPhase::Normal => {
                // Never await a peer-dependent close in a terminal recovery
                // branch. Dropping the socket guarantees the outer loop can
                // deliver its one DataReset/Shell disconnect even if the peer
                // never drains its write side.
                return Ok(ConnectedExit::ResumeDeadlineExpired);
            }
            // A recovery socket must not enter an unbounded write while its
            // deadline/cancel fence is active. The first interval tick is
            // immediate, so gate keepalive until the authoritative snapshot
            // has completed the resume lifecycle.
            _ = keepalive.tick(), if *phase == ConnectionPhase::Normal => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as i64)
                    .unwrap_or(0);
                let keepalive = json!({ "type": "keepAlive", "time": now_ms });
                if let Err(error) = socket
                    .send(Message::Text(keepalive.to_string().into()))
                    .await
                {
                    let _ = terminate_written_game_shop_unknown(
                        game_shop_receipt_gate,
                        mir2_bevy_runtime::native_ingest::push_native_data_reset,
                    );
                    return Err(format!("gateway keepalive failed: {error}"));
                }
            }
            _ = input_poll.tick() => {
                for command in drain_command_batch(commands, reconnect_config.command_batch_limit) {
                    if *phase != ConnectionPhase::Normal {
                        if discard_correlated_before_socket_write(
                            &command,
                            game_shop_receipt_gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        ) {
                            continue;
                        }
                        match awaiting_resume_command_action(&command) {
                            AwaitingResumeCommandAction::Shutdown => {
                                reset_native_data_models();
                                return Ok(ConnectedExit::Shutdown);
                            }
                            AwaitingResumeCommandAction::Cancel => {
                                resume_state.clear();
                                return Ok(ConnectedExit::Disconnected(Some(
                                    "native reconnect cancelled".to_owned(),
                                )));
                            }
                            AwaitingResumeCommandAction::Ignore => continue,
                        }
                    }
                    let explicit_leave = matches!(
                        &command,
                        GatewayCommand::Wire(NativeOutboundCommand::LogOut)
                            | GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
                    );
                    let claim_mail_id = match &command {
                        GatewayCommand::Wire(NativeOutboundCommand::CollectParcel { mail_id }) => {
                            Some(*mail_id)
                        }
                        _ => None,
                    };
                    let is_send_mail = matches!(
                        &command,
                        GatewayCommand::Wire(NativeOutboundCommand::SendMail { .. })
                    );
                    // Crystal's mail commands have no request id. Keep one
                    // in-flight command per operation class even if callers
                    // enqueue different mail ids/drafts in the same frame.
                    if !mail_command_allowed(
                        &command,
                        in_flight_claim_mail_id,
                        send_mail_in_flight,
                        !pending_mail_feedback.is_empty(),
                    ) {
                        continue;
                    }
                    let game_shop_request = game_shop_request_from_command(&command);
                    if game_shop_request.is_some() && game_shop_receipt_gate.pending.is_some() {
                        // The UI and command transaction lanes both enforce a
                        // single purchase. Treat any violation at the sole
                        // writer as an ambiguous terminal operation instead of
                        // sending a second purchase or leaving local pending.
                        game_shop_receipt_gate.clear_terminal();
                        let _ = mir2_bevy_runtime::native_ingest::push_native_data_reset();
                        continue;
                    }
                    let trace_player_command = matches!(&command, GatewayCommand::Player(_));
                    let payload = match command {
                        GatewayCommand::Connect => continue,
                        GatewayCommand::Shutdown => {
                            reset_native_data_models();
                            return Ok(ConnectedExit::Shutdown);
                        }
                        GatewayCommand::Player(intent) => intent.to_json(),
                        GatewayCommand::Wire(command) => {
                            update_session_context(&mut context, &command);
                            command.to_wire_json()
                        }
                    };
                    if trace_player_command
                        && std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some()
                    {
                        eprintln!("[gateway-client] sending player command {payload}");
                    }
                    if explicit_leave {
                        resume_state.clear();
                        // Do not leave stale darkness visible while the
                        // server processes logout/disconnect. The eventual
                        // packet reset is idempotent and will retry if this
                        // enqueue was backpressured.
                        lighting_publisher.reset_session();
                        lighting_publisher.push_clear_state();
                    }
                    if let Err(error) = socket
                        .send(Message::Text(payload.to_string().into()))
                        .await
                    {
                        let terminated_written = terminate_written_game_shop_unknown(
                            game_shop_receipt_gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        );
                        if game_shop_request.is_some() && !terminated_written {
                            game_shop_receipt_gate.clear_terminal();
                            let _ = mir2_bevy_runtime::native_ingest::push_native_data_reset();
                        }
                        return Err(format!("gateway command send failed: {error}"));
                    }
                    if let Some(request) = game_shop_request {
                        if !game_shop_receipt_gate.record_successful_send(request) {
                            game_shop_receipt_gate.clear_terminal();
                            let _ = mir2_bevy_runtime::native_ingest::push_native_data_reset();
                        }
                    }
                    if let Some(mail_id) = claim_mail_id {
                        in_flight_claim_mail_id = Some(mail_id);
                    }
                    if is_send_mail {
                        send_mail_in_flight = true;
                    }
                }
            }
            message = socket.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        let disposition = process_connected_text_frame(
                            &text,
                            game_shop_receipt_gate,
                            |text, gate| {
                                let disposition = handle_gateway_text_for_connection(
                                    text,
                                    &mut snapshot_log_counter,
                                    &context,
                                    shell_events,
                                    &mut gameplay_adapter,
                                    gameplay_events,
                                    &mut last_world_payload,
                                    &mut last_wallet,
                                    &mut ui_cursor,
                                    &mut in_flight_claim_mail_id,
                                    &mut send_mail_in_flight,
                                    &mut pending_mail_feedback,
                                    &mut skill_cursor,
                                    &mut social_cursor,
                                    phase,
                                    resume_state,
                                    resume_scene_reset_sent,
                                    &mut connection_bootstrap_sent,
                                    gate,
                                    push_world_state,
                                )?;
                                // Consume exactly the same authoritative
                                // envelope that drove the gameplay bridge.
                                // Quarantined pre-resume frames are forbidden
                                // from leaking into the new render generation.
                                if disposition == InboundDisposition::Applied {
                                    if let Ok(envelope) = serde_json::from_str::<GatewayEnvelope>(text) {
                                        lighting_publisher.observe_envelope(&envelope);
                                    }
                                }
                                Ok(disposition)
                            },
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        )?;
                        match disposition {
                            InboundDisposition::ResumeRejected => {
                                return Ok(ConnectedExit::ResumeRejected);
                            }
                            InboundDisposition::Applied | InboundDisposition::Quarantined => {}
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        return Ok(finish_connected_socket(
                            ConnectedSocketEnd::Disconnected,
                            game_shop_receipt_gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        ));
                    }
                    Some(Ok(Message::Binary(bytes))) if bytes.len() > MAX_GATEWAY_FRAME_BYTES => {
                        let _ = terminate_written_game_shop_unknown(
                            game_shop_receipt_gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        );
                        return Err(format!(
                            "gateway binary frame exceeds {MAX_GATEWAY_FRAME_BYTES} bytes"
                        ));
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => {
                        return Ok(finish_connected_socket(
                            ConnectedSocketEnd::ReadError(error.to_string()),
                            game_shop_receipt_gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        ));
                    }
                }
            }
        }
    }
}

fn update_session_context(context: &mut GatewaySessionContext, command: &NativeOutboundCommand) {
    match command {
        NativeOutboundCommand::Login { account_id, .. }
        | NativeOutboundCommand::NewAccount { account_id, .. } => {
            context.account_id = Some(account_id.clone());
            context.character_index = None;
        }
        NativeOutboundCommand::StartGame { character_index } => {
            context.character_index = Some(*character_index);
        }
        NativeOutboundCommand::LogOut | NativeOutboundCommand::Disconnect => {
            context.character_index = None;
        }
        _ => {}
    }
}

fn mail_command_allowed(
    command: &GatewayCommand,
    in_flight_claim_mail_id: Option<u64>,
    send_mail_in_flight: bool,
    feedback_waiting_for_receive_mail: bool,
) -> bool {
    let is_claim = matches!(
        command,
        GatewayCommand::Wire(NativeOutboundCommand::CollectParcel { .. })
    );
    let is_send = matches!(
        command,
        GatewayCommand::Wire(NativeOutboundCommand::SendMail { .. })
    );
    let mail_operation_in_flight = in_flight_claim_mail_id.is_some()
        || send_mail_in_flight
        || feedback_waiting_for_receive_mail;
    !(is_claim || is_send) || !mail_operation_in_flight
}

fn reset_native_data_models() {
    let _ = mir2_bevy_runtime::native_ingest::push_native_data_reset();
}

fn reset_native_scene() {
    let _ = mir2_bevy_runtime::native_ingest::push_native_scene_reset();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconnectResetPolicy {
    Preserve,
    Scene,
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OuterTerminalTransition {
    ResumeRejected,
    NoCredentialDisconnect,
    RetryExhausted,
    ResumeCancelled,
}

/// Shared outer-loop terminal seam. Every production transition that ends a
/// resumable/native session passes through this function before notifying the
/// shell. The transition label is intentionally explicit for deterministic
/// state-machine coverage even though all terminal variants share reset
/// mechanics.
fn apply_outer_terminal_transition_with<F, P>(
    _transition: OuterTerminalTransition,
    resume_state: &mut NativeResumeClientState,
    gate: &mut GameShopReceiptGate,
    push_data_reset: F,
    push_preserving_reset: P,
) -> bool
where
    F: FnMut() -> bool,
    P: FnMut(GameShopReceipt) -> bool,
{
    resume_state.clear();
    terminate_session_with_game_shop_boundary(gate, push_data_reset, push_preserving_reset)
}

fn apply_outer_terminal_transition(
    transition: OuterTerminalTransition,
    resume_state: &mut NativeResumeClientState,
    gate: &mut GameShopReceiptGate,
) -> bool {
    apply_outer_terminal_transition_with(
        transition,
        resume_state,
        gate,
        mir2_bevy_runtime::native_ingest::push_native_data_reset,
        mir2_bevy_runtime::native_ingest::push_native_data_reset_preserving_exact_game_shop_receipt,
    )
}

fn transport_loss_reset_policy(has_live_credential: bool) -> ReconnectResetPolicy {
    if has_live_credential {
        ReconnectResetPolicy::Preserve
    } else {
        ReconnectResetPolicy::Data
    }
}

fn session_resumed_reset_policy() -> ReconnectResetPolicy {
    ReconnectResetPolicy::Scene
}

fn terminal_failure_reset_policy() -> ReconnectResetPolicy {
    ReconnectResetPolicy::Data
}

fn apply_reconnect_reset_policy(policy: ReconnectResetPolicy) {
    match policy {
        ReconnectResetPolicy::Preserve => {}
        ReconnectResetPolicy::Scene => reset_native_scene(),
        ReconnectResetPolicy::Data => reset_native_data_models(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeResetScope {
    /// Drop the stale scene payload; personal/session models stay authoritative.
    Scene,
    /// Account/character boundary: clear every native read model and pending key.
    Session,
}

fn packet_native_reset_scope(packet: &str) -> Option<NativeResetScope> {
    match packet {
        "MapChanged" => Some(NativeResetScope::Scene),
        "LogOutSuccess" | "ReturnToLogin" | "Disconnect" => Some(NativeResetScope::Session),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InboundDisposition {
    Applied,
    Quarantined,
    ResumeRejected,
}

/// Apply the resume control plane before the ordinary packet/snapshot path.
/// A fresh socket may deliver realm/on_connect/worldSnapshot before it has
/// read our capability and resume request. Those messages are deliberately
/// discarded while AwaitingResume; the post-resume snapshot is the only scene
/// source for a resumed native session.
fn handle_gateway_text_for_connection<F>(
    text: &str,
    snapshot_log_counter: &mut u32,
    context: &GatewaySessionContext,
    shell_events: &std::sync::mpsc::Sender<ShellGatewayEvent>,
    gameplay_adapter: &mut NativeGameplayAdapter,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    last_world_payload: &mut Option<Value>,
    last_wallet: &mut Option<WalletState>,
    ui_cursor: &mut NativeUiPlayerCursor,
    in_flight_claim_mail_id: &mut Option<u64>,
    send_mail_in_flight: &mut bool,
    pending_mail_feedback: &mut VecDeque<PendingMailOperationFeedback>,
    skill_cursor: &mut SkillPacketCursor,
    social_cursor: &mut SocialModel,
    phase: &mut ConnectionPhase,
    resume_state: &mut NativeResumeClientState,
    resume_scene_reset_sent: &mut bool,
    connection_bootstrap_sent: &mut bool,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    push_world_state: &mut F,
) -> Result<InboundDisposition, String>
where
    F: FnMut(String) -> bool,
{
    let parsed = parse_inbound_event(text).map_err(|error| error.to_string())?;
    match parsed {
        InboundEvent::ResumeCredential(event) => {
            record_resume_credential_if_allowed(
                *phase,
                resume_state,
                &event.credential,
                event.expires_at_ms,
                event.generation,
            );
            return Ok(InboundDisposition::Applied);
        }
        InboundEvent::SessionResumed(event) if *phase == ConnectionPhase::AwaitingResume => {
            if !resume_state.accept_resumed_generation(event.generation) {
                return Ok(InboundDisposition::ResumeRejected);
            }
            resume_state.character_index = event.character_index.or(resume_state.character_index);
            if !*resume_scene_reset_sent {
                apply_reconnect_reset_policy(session_resumed_reset_policy());
                *resume_scene_reset_sent = true;
            }
            *phase = ConnectionPhase::Resumed;
            return Ok(InboundDisposition::Applied);
        }
        InboundEvent::ResumeRejected(_) if *phase == ConnectionPhase::AwaitingResume => {
            return Ok(InboundDisposition::ResumeRejected);
        }
        InboundEvent::GameShopReceipt(receipt) if *phase == ConnectionPhase::AwaitingResume => {
            // A server may flush the already-committed transaction receipt
            // before the resume control packet. Unlike snapshots, this event
            // has an exact request four-tuple retained across the transient
            // reconnect, so it is safe to correlate now. Dropping it here
            // would leave the purchase pending forever without any replay.
            let accepted = correlate_and_deliver_game_shop_receipt(
                game_shop_receipt_gate,
                &receipt,
                mir2_bevy_runtime::native_ingest::push_native_game_shop_receipt,
                mir2_bevy_runtime::native_ingest::push_native_data_reset,
            )?;
            return Ok(if accepted {
                InboundDisposition::Applied
            } else {
                InboundDisposition::Quarantined
            });
        }
        _ if *phase == ConnectionPhase::AwaitingResume => {
            return Ok(InboundDisposition::Quarantined);
        }
        InboundEvent::SessionResumed(_) | InboundEvent::ResumeRejected(_) => {
            return Ok(InboundDisposition::Applied);
        }
        _ => {}
    }

    if let InboundEvent::GameShopReceipt(receipt) = &parsed {
        let accepted = correlate_and_deliver_game_shop_receipt(
            game_shop_receipt_gate,
            receipt,
            mir2_bevy_runtime::native_ingest::push_native_game_shop_receipt,
            mir2_bevy_runtime::native_ingest::push_native_data_reset,
        )?;
        return Ok(if accepted {
            InboundDisposition::Applied
        } else {
            InboundDisposition::Quarantined
        });
    }

    let is_world_snapshot = text_kind(text).as_deref() == Some("worldSnapshot");
    let snapshot_ingest = handle_gateway_text_with_world_ingest(
        text,
        snapshot_log_counter,
        context,
        shell_events,
        gameplay_adapter,
        gameplay_events,
        last_world_payload,
        last_wallet,
        ui_cursor,
        in_flight_claim_mail_id,
        send_mail_in_flight,
        pending_mail_feedback,
        skill_cursor,
        social_cursor,
        push_world_state,
    )?;

    if is_world_snapshot && snapshot_ingest == WorldSnapshotIngestOutcome::Applied {
        let value: Value = serde_json::from_str(text)
            .map_err(|error| format!("invalid gateway payload: {error}"))?;
        let payload = value.get("payload").unwrap_or(&Value::Null);
        if !*connection_bootstrap_sent {
            if let Some(character) = resumed_character_from_snapshot(
                payload,
                resume_state.character_index.or(context.character_index),
            ) {
                let _ = shell_events.send(ShellGatewayEvent::PlayerBootstrapped { character });
                *connection_bootstrap_sent = true;
            }
        }
        if *phase == ConnectionPhase::Resumed {
            *phase = ConnectionPhase::Normal;
            resume_state.reconnect_started_at = None;
            resume_state.retry_attempt = 0;
        }
    }
    if is_world_snapshot && snapshot_ingest == WorldSnapshotIngestOutcome::Backpressured {
        return Ok(InboundDisposition::Quarantined);
    }
    Ok(InboundDisposition::Applied)
}

fn text_kind(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text)
        .ok()?
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn resumed_character_from_snapshot(
    payload: &Value,
    index: Option<i32>,
) -> Option<CharacterSummary> {
    let index = index?;
    let entity = payload
        .get("entities")
        .and_then(Value::as_array)
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))
        });
    Some(CharacterSummary::new(
        index,
        entity
            .and_then(|entity| entity.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("Resumed character"),
        entity
            .and_then(|entity| entity.get("level"))
            .and_then(Value::as_u64)
            .and_then(|level| u16::try_from(level).ok())
            .unwrap_or(1),
        entity
            .and_then(|entity| entity.get("class"))
            .and_then(Value::as_str)
            .unwrap_or("Unknown"),
        entity
            .and_then(|entity| entity.get("gender"))
            .and_then(Value::as_str)
            .unwrap_or("Unknown"),
    ))
}

/// Whether a worldSnapshot was accepted by the native runtime's bounded ingest
/// queue. Resume completion is only legal after `Applied`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorldSnapshotIngestOutcome {
    NotSnapshot,
    Applied,
    Backpressured,
}

/// Handle one inbound gateway text message. Returns an error to abort the loop.
fn handle_gateway_text(
    text: &str,
    snapshot_log_counter: &mut u32,
    context: &GatewaySessionContext,
    shell_events: &std::sync::mpsc::Sender<ShellGatewayEvent>,
    gameplay_adapter: &mut NativeGameplayAdapter,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    last_world_payload: &mut Option<Value>,
    last_wallet: &mut Option<WalletState>,
    ui_cursor: &mut NativeUiPlayerCursor,
    in_flight_claim_mail_id: &mut Option<u64>,
    send_mail_in_flight: &mut bool,
    pending_mail_feedback: &mut VecDeque<PendingMailOperationFeedback>,
    skill_cursor: &mut SkillPacketCursor,
    social_cursor: &mut SocialModel,
) -> Result<WorldSnapshotIngestOutcome, String> {
    handle_gateway_text_with_world_ingest(
        text,
        snapshot_log_counter,
        context,
        shell_events,
        gameplay_adapter,
        gameplay_events,
        last_world_payload,
        last_wallet,
        ui_cursor,
        in_flight_claim_mail_id,
        send_mail_in_flight,
        pending_mail_feedback,
        skill_cursor,
        social_cursor,
        mir2_bevy_runtime::native_ingest::push_native_world_state,
    )
}

fn handle_gateway_text_with_world_ingest<F>(
    text: &str,
    snapshot_log_counter: &mut u32,
    context: &GatewaySessionContext,
    shell_events: &std::sync::mpsc::Sender<ShellGatewayEvent>,
    gameplay_adapter: &mut NativeGameplayAdapter,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    last_world_payload: &mut Option<Value>,
    last_wallet: &mut Option<WalletState>,
    ui_cursor: &mut NativeUiPlayerCursor,
    in_flight_claim_mail_id: &mut Option<u64>,
    send_mail_in_flight: &mut bool,
    pending_mail_feedback: &mut VecDeque<PendingMailOperationFeedback>,
    skill_cursor: &mut SkillPacketCursor,
    social_cursor: &mut SocialModel,
    mut push_world_state: F,
) -> Result<WorldSnapshotIngestOutcome, String>
where
    F: FnMut(String) -> bool,
{
    let event: GatewayEnvelope =
        serde_json::from_str(text).map_err(|error| format!("invalid gateway payload: {error}"))?;
    let parsed = parse_inbound_event(text).map_err(|error| error.to_string())?;
    dispatch_shell_event(&parsed, context, shell_events);
    let packet_updates_world = if let InboundEvent::Packet(packet) = &parsed {
        gameplay_adapter.observe_packet(packet)
    } else {
        false
    };
    if let InboundEvent::Packet(packet) = &parsed {
        if packet_updates_big_map(packet) {
            // Static Big Map packets often arrive between periodic world
            // snapshots. Forward a map-only snapshot immediately so the
            // native resource cannot keep a stale NPC/object-id cache.
            let _ = gameplay_events.send(gameplay_adapter.big_map_snapshot());
        }
    }
    if let Some(scope) = event.packet.as_deref().and_then(packet_native_reset_scope) {
        if scope == NativeResetScope::Session {
            reset_native_data_models();
        } else {
            reset_native_scene();
        }
        // The packet-first gameplay adapter clears Zone/effect state for
        // MapChanged. The explicit SceneReset now clears the Bevy retained
        // world/map/entity/effect registry in the same frame. Never re-emit
        // the previous map's personal snapshot.
        *last_world_payload = None;
        if scope == NativeResetScope::Session {
            ui_cursor.reset();
            *last_wallet = None;
            *in_flight_claim_mail_id = None;
            *send_mail_in_flight = false;
            pending_mail_feedback.clear();
            skill_cursor.reset();
            social_cursor.clear_session();
        } else {
            social_cursor.clear_scene();
        }
    }

    let skill_packet_updates_world = if event.kind == "packet" {
        event
            .packet
            .as_deref()
            .zip(event.payload.as_ref())
            .is_some_and(|(packet, payload)| {
                skill_cursor.apply_packet(
                    packet,
                    payload,
                    world_payload_tick_from(last_world_payload.as_ref()),
                )
            })
    } else {
        false
    };

    if let InboundEvent::Packet(PacketEvent::Other { packet, payload }) = &parsed {
        if social_cursor.apply_packet(packet, payload) {
            let json = serde_json::to_string(social_cursor).map_err(|error| error.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_social_model(json);
        }
    }

    match event.kind.as_str() {
        "worldSnapshot" => {
            let payload = event
                .payload
                .ok_or_else(|| "worldSnapshot missing payload".to_owned())?;
            let mut payload = payload;
            gameplay_adapter.apply_authoritative_overlay(&mut payload);
            gameplay_adapter.observe_world_snapshot(&payload);
            skill_cursor.observe_snapshot(&mut payload);
            let _ = gameplay_events.send(gameplay_adapter.snapshot(&payload));
            let runtime_snapshot = transform_world_snapshot(&payload);
            let json = serde_json::to_string(&runtime_snapshot).map_err(|e| e.to_string())?;
            let world_ingest = if push_world_state(json) {
                // Periodic (~1.2 s game tick) snapshot; only log the first few
                // so the native console stays readable while the map renders.
                *snapshot_log_counter += 1;
                if *snapshot_log_counter <= 3 {
                    eprintln!(
                        "[gateway-client] forwarded world snapshot #{}",
                        *snapshot_log_counter
                    );
                }
                WorldSnapshotIngestOutcome::Applied
            } else {
                eprintln!("[gateway-client] runtime not ready; dropping snapshot");
                WorldSnapshotIngestOutcome::Backpressured
            };

            // Feed the HUD read model so the shared Bevy UI renders player stats.
            update_wallet_from_snapshot(last_wallet, &payload);
            // A periodic world snapshot may omit a wallet field immediately
            // after a delta packet. Fold the packet-first absolute cursor back
            // into both the current payload and the delta baseline so a stale
            // snapshot cannot overwrite the fresh HUD balance.
            merge_wallet_into_payload(&mut payload, *last_wallet);
            *last_world_payload = Some(payload.clone());
            ui_cursor.observe_world_snapshot(&payload);
            let ui_model = ui_cursor.to_read_model_json();
            let ui_json = serde_json::to_string(&ui_model).map_err(|e| e.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_ui_read_model(ui_json);

            // Feed the shared map model so client-bevy renders terrain tiles.
            let map_model = transform_map_model(&payload);
            let map_json = serde_json::to_string(&map_model).map_err(|e| e.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_map_model(map_json);

            // When a local map pack + atlas are available, render the real map
            // textures via MapRenderState instead of the colored terrain. The
            // gateway payload carries the authoritative map file name.
            let map_file_name = payload
                .get("mapFileName")
                .and_then(Value::as_str)
                .unwrap_or("0");
            if crate::map_parser::has_local_map_atlas() {
                if let Some(map) = crate::map_parser::load_map(map_file_name) {
                    let viewport = crate::map_parser::MapViewport::from_gateway_payload(&payload);
                    if let Some(render_state) =
                        crate::map_parser::build_map_render_state(&map, viewport)
                    {
                        let render_json =
                            serde_json::to_string(&render_state).map_err(|e| e.to_string())?;
                        let _ = mir2_bevy_runtime::native_ingest::push_native_map_render_state(
                            render_json,
                        );
                        eprintln!(
                            "[gateway-client] pushed real map render state for {map_file_name}"
                        );
                    }
                }
            }

            // Feed the shared entity model set so client-bevy renders entities.
            let entity_model = transform_entity_model_set(&payload);
            let entity_json = serde_json::to_string(&entity_model).map_err(|e| e.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_entity_model_set(entity_json);

            // The main-thread Windows entity presentation resource owns real
            // sprite render-state production so its Crystal frame clock keeps
            // advancing between Gateway snapshots.

            // Feed the shared inventory model so client-bevy renders the bag.
            let inventory = transform_inventory_model(&payload);
            let inventory_json = serde_json::to_string(&inventory).map_err(|e| e.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_inventory_model(inventory_json);

            // These models are deliberately independent. NPCGoods populates
            // ShopModel, while the cash catalogue uses GameShopInfo/Stock.
            if let Some(mut mail) = try_transform_mail_model_from_snapshot(&payload) {
                let _ = push_mail_model_with_feedback(&mut mail, pending_mail_feedback)?;
            }
            if let Some(storage) = try_transform_storage_model_from_snapshot(&payload) {
                let storage_json =
                    serde_json::to_string(&storage).map_err(|error| error.to_string())?;
                let _ = mir2_bevy_runtime::native_ingest::push_native_storage_model(storage_json);
            }
            if payload_has_valid_shop_array(&payload) {
                let shop = transform_shop_model_from_snapshot(&payload);
                let shop_json = serde_json::to_string(&shop).map_err(|error| error.to_string())?;
                let _ = mir2_bevy_runtime::native_ingest::push_native_shop_model(shop_json);
            }

            Ok(world_ingest)
        }
        "packet" => {
            let packet = event.packet.as_deref().unwrap_or("?");
            match packet {
                "LoginSuccess" => {
                    eprintln!("[gateway-client] LoginSuccess");
                }
                "StartGame" => {
                    eprintln!("[gateway-client] StartGame ack");
                }
                "MapInformation" => {
                    eprintln!("[gateway-client] packet {packet}");
                    if let Some(payload) = event.payload.as_ref() {
                        ui_cursor.observe_map_identity(payload);
                        let _ = mir2_bevy_runtime::native_ingest::push_native_ui_read_model(
                            ui_cursor.to_read_model_json().to_string(),
                        );
                    }
                }
                "MapChanged" => {
                    eprintln!("[gateway-client] packet {packet}");
                    if let Some(payload) = event.payload.as_ref() {
                        ui_cursor.observe_map_identity(payload);
                        let _ = mir2_bevy_runtime::native_ingest::push_native_ui_read_model(
                            ui_cursor.to_read_model_json().to_string(),
                        );
                    }
                }
                "UserInformation" => {
                    eprintln!("[gateway-client] packet {packet}");
                    if let Some(payload) = event.payload.as_ref() {
                        update_wallet_from_snapshot(last_wallet, payload);
                        ui_cursor.observe_user_information(payload);
                        let ui_model = ui_cursor.to_read_model_json();
                        let ui_json =
                            serde_json::to_string(&ui_model).map_err(|error| error.to_string())?;
                        let _ =
                            mir2_bevy_runtime::native_ingest::push_native_ui_read_model(ui_json);
                        merge_wallet_into_world(last_world_payload, *last_wallet);
                    }
                }
                "UserLocation" => {
                    if std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some() {
                        eprintln!(
                            "[gateway-client] packet UserLocation payload={}",
                            event.payload.as_ref().unwrap_or(&Value::Null)
                        );
                    } else {
                        eprintln!("[gateway-client] packet UserLocation");
                    }
                }
                "Chat" | "ObjectChat" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(chat) = transform_chat_line(packet, payload) {
                            let _ = mir2_bevy_runtime::native_ingest::push_native_chat_line(
                                serde_json::to_string(&chat).map_err(|e| e.to_string())?,
                            );
                        }
                    }
                }
                "NPCGoods" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(shop) = try_transform_shop_model_from_packet(payload) {
                            let shop_json =
                                serde_json::to_string(&shop).map_err(|error| error.to_string())?;
                            let _ =
                                mir2_bevy_runtime::native_ingest::push_native_shop_model(shop_json);
                            let signal = npc_shop_service_from_packet(packet, payload)
                                .ok_or_else(|| "invalid NPCGoods service signal".to_owned())?;
                            push_native_npc_shop_service(signal)?;
                        }
                    }
                }
                "NPCSell" => {
                    let signal = npc_shop_service_from_packet(packet, &Value::Null)
                        .ok_or_else(|| "invalid NPCSell service signal".to_owned())?;
                    push_native_npc_shop_service(signal)?;
                }
                "NPCRepair" | "NPCSRepair" => {
                    let Some(signal) = event
                        .payload
                        .as_ref()
                        .and_then(|payload| npc_shop_service_from_packet(packet, payload))
                    else {
                        eprintln!("[gateway-client] ignored malformed {packet} service rate");
                        return Ok(WorldSnapshotIngestOutcome::NotSnapshot);
                    };
                    push_native_npc_shop_service(signal)?;
                }
                "DropItem" | "MoveItem" | "MergeItem" | "SplitItem1" | "SellItem" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(ack) = transform_inventory_operation_ack(packet, payload) {
                            if let Ok(json) = serde_json::to_string(&ack) {
                                let _ = mir2_bevy_runtime::native_ingest::push_native_inventory_operation_ack(json);
                            }
                        } else {
                            eprintln!(
                                "[gateway-client] ignored malformed {packet} acknowledgement"
                            );
                        }
                    }
                }
                "GameShopInfo" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(item) = transform_game_shop_info_from_packet(payload) {
                            let json = serde_json::to_string(&item).map_err(|e| e.to_string())?;
                            let _ =
                                mir2_bevy_runtime::native_ingest::push_native_game_shop_info(json);
                        }
                    }
                }
                "GameShopStock" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(stock) = transform_game_shop_stock_from_packet(payload) {
                            let json = serde_json::to_string(&stock).map_err(|e| e.to_string())?;
                            let _ =
                                mir2_bevy_runtime::native_ingest::push_native_game_shop_stock(json);
                        }
                    }
                }
                "GainedCredit" | "LoseCredit" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(value) = apply_wallet_delta(
                            last_wallet,
                            last_world_payload,
                            "credit",
                            wallet_value(payload, "credit"),
                            packet == "GainedCredit",
                        ) {
                            let _ = mir2_bevy_runtime::native_ingest::push_native_wallet_patch(
                                json!({"credit": value}).to_string(),
                            );
                        }
                    }
                }
                "GainedGold" | "LoseGold" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(value) = apply_wallet_delta(
                            last_wallet,
                            last_world_payload,
                            "gold",
                            wallet_value(payload, "gold"),
                            packet == "GainedGold",
                        ) {
                            let _ = mir2_bevy_runtime::native_ingest::push_native_wallet_patch(
                                json!({"gold": value}).to_string(),
                            );
                        }
                    }
                }
                "ReceiveMail" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(mut model) = try_transform_mail_model_from_packet(payload) {
                            let _ =
                                push_mail_model_with_feedback(&mut model, pending_mail_feedback)?;
                        }
                    }
                }
                "MailSent" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(feedback) = mail_operation_feedback(packet, payload, None) {
                            *send_mail_in_flight = false;
                            let _ = enqueue_mail_feedback(pending_mail_feedback, feedback);
                        }
                    }
                }
                "ParcelCollected" => {
                    if let Some(payload) = event.payload.as_ref() {
                        let claim_mail_id = *in_flight_claim_mail_id;
                        if let Some(feedback) =
                            mail_operation_feedback(packet, payload, claim_mail_id)
                        {
                            *in_flight_claim_mail_id = None;
                            let _ = enqueue_mail_feedback(pending_mail_feedback, feedback);
                        }
                    }
                }
                "UserStorage" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(items) = transform_storage_items_from_packet(payload) {
                            let json =
                                serde_json::to_string(&items).map_err(|error| error.to_string())?;
                            let _ =
                                mir2_bevy_runtime::native_ingest::push_native_storage_items(json);
                        }
                    }
                }
                "StoreItem"
                | "StoreItemV2"
                | "TakeBackItem"
                | "TakeBackItemV2"
                | "StorageUnlockResult"
                | "StoragePasswordResult"
                | "ResizeStorage" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(patch) = transform_storage_patch_from_packet(packet, payload) {
                            let json =
                                serde_json::to_string(&patch).map_err(|error| error.to_string())?;
                            let _ = push_storage_patch_or_reset(
                                json,
                                mir2_bevy_runtime::native_ingest::push_native_storage_patch,
                                mir2_bevy_runtime::native_ingest::push_native_data_reset,
                            );
                        }
                    }
                }
                "SplitItem" => {
                    // This success payload contains the new item but not the
                    // source unique id/count. Keep the exact Split pending key
                    // until SplitItem1, a provable model change, or DataReset.
                }
                _ => {
                    // Other packets are folded into the periodic worldSnapshot;
                    // not logged to keep the native console readable.
                }
            }
            if packet_updates_world || skill_packet_updates_world {
                if let Some(base_payload) = last_world_payload.as_ref() {
                    let mut payload = base_payload.clone();
                    gameplay_adapter.apply_authoritative_overlay(&mut payload);
                    forward_packet_first_world(
                        &payload,
                        gameplay_adapter,
                        gameplay_events,
                        pending_mail_feedback,
                    )?;
                }
            }
            Ok(WorldSnapshotIngestOutcome::NotSnapshot)
        }
        "error" => {
            // Command-level errors are visible shell feedback. They do not
            // necessarily close the authenticated WebSocket.
            Ok(WorldSnapshotIngestOutcome::NotSnapshot)
        }
        _ => Ok(WorldSnapshotIngestOutcome::NotSnapshot),
    }
}

fn correlate_and_deliver_game_shop_receipt(
    gate: &mut GameShopReceiptGate,
    receipt: &GameShopReceipt,
    push_receipt: impl FnOnce(String) -> bool,
    push_terminal_reset: impl FnOnce() -> bool,
) -> Result<bool, String> {
    if gate.reserved.is_some() {
        // The first exact receipt is already protected by the runtime reserve.
        // Quarantine every later receipt before semantic validation: invalid,
        // wrong and duplicate frames cannot clear or overwrite it.
        return Ok(false);
    }

    if !receipt.is_valid() {
        gate.clear_terminal();
        let _ = push_terminal_reset();
        return Ok(false);
    }

    let Some(pending) = gate.pending.as_ref() else {
        let _ = push_terminal_reset();
        return Ok(false);
    };
    if !receipt.matches_request(pending) {
        gate.clear_terminal();
        let _ = push_terminal_reset();
        return Ok(false);
    }

    let json = serde_json::to_string(receipt).map_err(|error| error.to_string())?;
    if push_receipt(json) {
        let _ = gate.pending.take();
        gate.reserved = Some(receipt.clone());
        return Ok(true);
    }
    // Receipt backpressure must never masquerade as an acknowledgement. A
    // terminal reset clears the in-flight purchase and surfaces unknown state;
    // it never resends the purchase command.
    gate.clear_terminal();
    let _ = push_terminal_reset();
    Ok(false)
}

fn packet_updates_big_map(packet: &PacketEvent) -> bool {
    matches!(
        packet,
        PacketEvent::NewMapInfo(_)
            | PacketEvent::WorldMapSetup(_)
            | PacketEvent::SearchMapResult(_)
            | PacketEvent::MapInformation(_)
            | PacketEvent::MapChanged(_)
            | PacketEvent::UserLocation(_)
            | PacketEvent::Disconnect(_)
    )
}

/// Re-render the latest personal snapshot after folding packet-authoritative
/// shared-Zone deltas into it. Movement must advance the terrain camera, HUD
/// coordinates and entity projection together; waiting for a personal-session
/// snapshot leaves a successfully moved native player looking frozen in place.
/// Inventory remains on the periodic snapshot path because Zone deltas do not
/// mutate it.
fn forward_packet_first_world(
    payload: &Value,
    gameplay_adapter: &NativeGameplayAdapter,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    pending_mail_feedback: &mut VecDeque<PendingMailOperationFeedback>,
) -> Result<(), String> {
    let _ = gameplay_events.send(gameplay_adapter.snapshot(payload));

    let runtime_snapshot = transform_world_snapshot(payload);
    let runtime_json =
        serde_json::to_string(&runtime_snapshot).map_err(|error| error.to_string())?;
    let _ = mir2_bevy_runtime::native_ingest::push_native_world_state(runtime_json);

    let ui_model = transform_ui_read_model(payload);
    let ui_json = serde_json::to_string(&ui_model).map_err(|error| error.to_string())?;
    let _ = mir2_bevy_runtime::native_ingest::push_native_ui_read_model(ui_json);

    let map_model = transform_map_model(payload);
    let map_json = serde_json::to_string(&map_model).map_err(|error| error.to_string())?;
    let _ = mir2_bevy_runtime::native_ingest::push_native_map_model(map_json);

    let map_file_name = payload
        .get("mapFileName")
        .and_then(Value::as_str)
        .unwrap_or("0");
    if crate::map_parser::has_local_map_atlas() {
        if let Some(map) = crate::map_parser::load_map(map_file_name) {
            let viewport = crate::map_parser::MapViewport::from_gateway_payload(payload);
            if let Some(render_state) = crate::map_parser::build_map_render_state(&map, viewport) {
                let render_json =
                    serde_json::to_string(&render_state).map_err(|error| error.to_string())?;
                let _ = mir2_bevy_runtime::native_ingest::push_native_map_render_state(render_json);
            }
        }
    }

    let entity_model = transform_entity_model_set(payload);
    let entity_json = serde_json::to_string(&entity_model).map_err(|error| error.to_string())?;
    let _ = mir2_bevy_runtime::native_ingest::push_native_entity_model_set(entity_json);

    if let Some(mut mail) = try_transform_mail_model_from_snapshot(payload) {
        let _ = push_mail_model_with_feedback(&mut mail, pending_mail_feedback)?;
    }
    if let Some(storage) = try_transform_storage_model_from_snapshot(payload) {
        let storage_json = serde_json::to_string(&storage).map_err(|error| error.to_string())?;
        let _ = mir2_bevy_runtime::native_ingest::push_native_storage_model(storage_json);
    }
    if payload_has_valid_shop_array(payload) {
        let shop = transform_shop_model_from_snapshot(payload);
        let shop_json = serde_json::to_string(&shop).map_err(|error| error.to_string())?;
        let _ = mir2_bevy_runtime::native_ingest::push_native_shop_model(shop_json);
    }

    Ok(())
}

fn dispatch_shell_event(
    event: &InboundEvent,
    context: &GatewaySessionContext,
    shell_events: &std::sync::mpsc::Sender<ShellGatewayEvent>,
) {
    let shell_event = match event {
        InboundEvent::Packet(PacketEvent::NewAccountResult(result)) => match result.result {
            Some(8) => Some(ShellGatewayEvent::AccountCreated),
            Some(7) => Some(ShellGatewayEvent::AccountCreationFailed {
                message: "account already exists".to_owned(),
            }),
            Some(code) => Some(ShellGatewayEvent::AccountCreationFailed {
                message: format!("account creation failed (result {code})"),
            }),
            None => Some(ShellGatewayEvent::AccountCreationFailed {
                message: "account creation returned no result".to_owned(),
            }),
        },
        InboundEvent::Packet(PacketEvent::LoginSuccess(success)) => {
            let characters = success
                .characters
                .iter()
                .filter_map(|character| {
                    let index = i32::try_from(character.index?).ok()?;
                    let level = u16::try_from(character.level.unwrap_or(1)).unwrap_or(1);
                    Some(CharacterSummary::new(
                        index,
                        character.name.as_deref().unwrap_or("Unnamed"),
                        level,
                        character.class.as_deref().unwrap_or("Unknown"),
                        character.gender.as_deref().unwrap_or("Unknown"),
                    ))
                })
                .collect();
            Some(ShellGatewayEvent::LoginSuccess {
                account: context.account_id.clone().unwrap_or_default(),
                characters,
            })
        }
        InboundEvent::Packet(PacketEvent::LoginFailure(failure)) => {
            let message = failure.reason.clone().unwrap_or_else(|| {
                failure.result.map_or_else(
                    || "login failed".to_owned(),
                    |result| format!("login failed (result {result})"),
                )
            });
            Some(ShellGatewayEvent::LoginFailure { message })
        }
        InboundEvent::Packet(PacketEvent::ChangePasswordResult(result)) => {
            // Preserve the authoritative Crystal result code.  A missing
            // result is an explicit transport failure, never a success.
            Some(ShellGatewayEvent::ChangePasswordResult {
                result: result.result.unwrap_or(-1),
            })
        }
        InboundEvent::Packet(PacketEvent::ChangePasswordBanned(banned)) => {
            let reason = banned
                .reason
                .clone()
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or_else(|| "account is banned".to_owned());
            let expiry = banned.expiry.as_ref().and_then(|value| match value {
                Value::String(value) if !value.is_empty() => Some(value.clone()),
                Value::Null => None,
                value => Some(value.to_string()),
            });
            Some(ShellGatewayEvent::ChangePasswordBanned { reason, expiry })
        }
        InboundEvent::Packet(PacketEvent::NewCharacterSuccess(success)) => {
            match success
                .character
                .as_ref()
                .and_then(character_summary_from_value)
            {
                Some(character) => Some(ShellGatewayEvent::CharacterCreated { character }),
                None => Some(ShellGatewayEvent::OperationFailure {
                    message: "character creation returned an invalid character".to_owned(),
                }),
            }
        }
        InboundEvent::Packet(PacketEvent::DeleteCharacterSuccess(success)) => success
            .character_index
            .map(|character_index| ShellGatewayEvent::CharacterDeleted { character_index })
            .or_else(|| {
                Some(ShellGatewayEvent::OperationFailure {
                    message: "character deletion returned no character index".to_owned(),
                })
            }),
        InboundEvent::Packet(PacketEvent::StartGameAck(ack)) => {
            let accepted = ack.result == Some(4);
            Some(ShellGatewayEvent::StartGameAck {
                accepted,
                reason: (!accepted).then(|| {
                    ack.result.map_or_else(
                        || "start game rejected".to_owned(),
                        |result| format!("start game rejected (result {result})"),
                    )
                }),
            })
        }
        // UserInformation can arrive before the render runtime accepts its
        // opening world snapshot. Entering InGame here would release keyboard
        // input against an empty/stale scene. The connection wrapper emits the
        // bootstrap only after an Applied worldSnapshot.
        InboundEvent::Packet(PacketEvent::UserInformation(_)) => None,
        InboundEvent::Packet(PacketEvent::Other { packet, payload }) => match packet.as_str() {
            "NewCharacter" => Some(ShellGatewayEvent::OperationFailure {
                message: "character creation failed".to_owned(),
            }),
            "DeleteCharacter" => Some(ShellGatewayEvent::OperationFailure {
                message: "character deletion failed".to_owned(),
            }),
            "StartGameBanned" => Some(ShellGatewayEvent::OperationFailure {
                message: payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("start game is banned")
                    .to_owned(),
            }),
            "StartGameDelay" => Some(ShellGatewayEvent::OperationFailure {
                message: "start game is temporarily delayed".to_owned(),
            }),
            "LogOutSuccess" => Some(ShellGatewayEvent::LoggedOut {
                characters: character_summaries_from_payload(payload),
            }),
            "LogOutFailed" => Some(ShellGatewayEvent::OperationFailure {
                message: "log out failed".to_owned(),
            }),
            _ => None,
        },
        InboundEvent::Error(error) => Some(ShellGatewayEvent::OperationFailure {
            message: error
                .message
                .clone()
                .unwrap_or_else(|| "gateway error".to_owned()),
        }),
        _ => None,
    };

    if let Some(event) = shell_event {
        let _ = shell_events.send(event);
    }
}

fn character_summaries_from_payload(payload: &Value) -> Vec<CharacterSummary> {
    payload
        .get("characters")
        .and_then(Value::as_array)
        .map(|characters| {
            characters
                .iter()
                .filter_map(character_summary_from_value)
                .collect()
        })
        .unwrap_or_default()
}

fn character_summary_from_value(value: &Value) -> Option<CharacterSummary> {
    let index = value
        .get("index")
        .and_then(Value::as_i64)
        .and_then(|index| i32::try_from(index).ok())?;
    let level = value
        .get("level")
        .and_then(Value::as_u64)
        .and_then(|level| u16::try_from(level).ok())
        .unwrap_or(1);
    Some(CharacterSummary::new(
        index,
        value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unnamed"),
        level,
        value
            .get("class")
            .and_then(Value::as_str)
            .unwrap_or("Unknown"),
        value
            .get("gender")
            .and_then(Value::as_str)
            .unwrap_or("Unknown"),
    ))
}

/// Transform a gateway `worldSnapshot` payload into the runtime's
/// `WorldSnapshot` JSON shape.
///
/// The gateway serializes `WorldSnapshot` with u32 `objectId`s and a wide field
/// set; the Bevy runtime deserializes a smaller camelCase shape with string
/// object ids and the movement timing the motion table consumes.
fn transform_world_snapshot(payload: &Value) -> Value {
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .map(|entity| {
                    let object_id = entity
                        .get("objectId")
                        .and_then(object_id_string)
                        .unwrap_or_default();
                    let (movement_started_ms, movement_duration_ms) = movement_window(entity);
                    json!({
                        "objectId": object_id,
                        "kind": entity.get("kind").cloned().unwrap_or(json!("monster")),
                        "name": entity.get("name").cloned().unwrap_or(json!("")),
                        "x": entity.get("x").cloned().unwrap_or(json!(0)),
                        "y": entity.get("y").cloned().unwrap_or(json!(0)),
                        "direction": entity.get("direction"),
                        "level": entity.get("level"),
                        "movementStartedMs": movement_started_ms,
                        "movementDurationMs": movement_duration_ms,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mine_nodes = payload
        .get("mineNodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .map(|node| {
                    json!({
                        "x": node.get("x").cloned().unwrap_or(json!(0)),
                        "y": node.get("y").cloned().unwrap_or(json!(0)),
                        "stage": node.get("stage").cloned().unwrap_or(json!(0)),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let self_player = entities
        .iter()
        .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"));

    json!({
        "mapTitle": payload.get("mapTitle"),
        "playerObjectId": payload.get("playerObjectId").and_then(object_id_string),
        "selectedObjectId": payload.get("selectedObjectId").and_then(object_id_string),
        "sceneView": payload.get("sceneView"),
        "terrainPatches": payload.get("terrainPatches").cloned().unwrap_or(Value::Array(vec![])),
        "decorObjects": payload.get("decorObjects").cloned().unwrap_or(Value::Array(vec![])),
        "entities": entities,
        "mineNodes": mine_nodes,
        "playerStats": {
            "hp": value_i32_or(payload.get("playerHp"), 0),
            "maxHp": value_i32_or(payload.get("playerMaxHp"), 0),
            "mp": value_i32_or(payload.get("playerMp"), 0),
            "maxMp": value_i32_or(payload.get("playerMaxMp"), 0),
            "gold": value_u32_or(payload.get("gold"), 0),
            "credit": value_u32_or(payload.get("credit"), 0),
            "level": value_u32_or(self_player.and_then(|entity| entity.get("level")), 0),
            "experience": value_i64_or(payload.get("playerExperience"), 0),
            "maxExperience": value_i64_or(payload.get("playerMaxExperience"), 0),
            "currentWeight": value_u16_or(payload.get("currentWeight"), 0),
            "maxWeight": value_u16_or(payload.get("maxWeight"), 0),
            "name": value_string(self_player.and_then(|entity| entity.get("name"))),
            "className": value_string(
                self_player
                    .and_then(|entity| entity.get("class"))
                    .or_else(|| self_player.and_then(|entity| entity.get("className"))),
            ),
            "mapName": value_string(payload.get("mapTitle")),
        },
    })
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::map::MapModel` JSON shape.
fn transform_map_model(payload: &Value) -> Value {
    let scene_view = payload.get("sceneView");
    let (center_x, center_y) = scene_view
        .and_then(|view| view.get("center"))
        .map(|center| {
            (
                center.get("x").and_then(Value::as_i64).unwrap_or(0) as i32,
                center.get("y").and_then(Value::as_i64).unwrap_or(0) as i32,
            )
        })
        .unwrap_or((0, 0));

    json!({
        "centerX": center_x,
        "centerY": center_y,
        "patches": payload.get("terrainPatches").cloned().unwrap_or(Value::Array(vec![])),
    })
}

/// Convert a gateway JSON value to a string object id (number or string).
fn object_id_string(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(string) => Some(string.clone()),
        _ => None,
    }
}

/// Gateway snapshots are deliberately partial before `StartGame` finishes and
/// therefore serialize several scalar fields as JSON `null`.  The Web host
/// naturally coalesces those values in TypeScript; the native host must do the
/// same before deserializing into Rust's non-optional UI read models.
fn value_i32_or(value: Option<&Value>, fallback: i32) -> i32 {
    value
        .and_then(|value| {
            value
                .as_i64()
                .and_then(|number| i32::try_from(number).ok())
                .or_else(|| value.as_str()?.parse::<i32>().ok())
        })
        .unwrap_or(fallback)
}

fn value_i64_or(value: Option<&Value>, fallback: i64) -> i64 {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or(fallback)
}

fn value_u16_or(value: Option<&Value>, fallback: u16) -> u16 {
    value
        .and_then(|value| {
            value
                .as_u64()
                .and_then(|number| u16::try_from(number).ok())
                .or_else(|| value.as_str()?.parse::<u16>().ok())
        })
        .unwrap_or(fallback)
}

fn value_u32_or(value: Option<&Value>, fallback: u32) -> u32 {
    value_u32(value).unwrap_or(fallback)
}

fn value_u32(value: Option<&Value>) -> Option<u32> {
    value.and_then(|value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<u32>().ok())
    })
}

fn value_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(|value| {
        value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<i32>().ok())
    })
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str()?.parse::<i64>().ok())
    })
}

fn value_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(value_u64_ref)
}

fn value_u64_ref(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse::<u64>().ok())
}

fn transform_inventory_operation_ack(
    packet: &str,
    payload: &Value,
) -> Option<InventoryOperationAck> {
    let success = payload.get("success")?.as_bool()?;
    match packet {
        "DropItem" => Some(InventoryOperationAck::Drop {
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            hero_inventory: payload.get("heroInventory")?.as_bool()?,
            success,
        }),
        "MoveItem" => Some(InventoryOperationAck::Move {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            from: value_i32(payload.get("from"))?,
            to: value_i32(payload.get("to"))?,
            success,
        }),
        "MergeItem" => Some(InventoryOperationAck::Merge {
            grid_from: payload.get("gridFrom")?.as_str()?.to_owned(),
            grid_to: payload.get("gridTo")?.as_str()?.to_owned(),
            id_from: value_u64(payload.get("idFrom"))?,
            id_to: value_u64(payload.get("idTo"))?,
            success,
        }),
        "SplitItem1" => Some(InventoryOperationAck::Split {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            success,
        }),
        "SellItem" => Some(InventoryOperationAck::Sell {
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            success,
        }),
        _ => None,
    }
}

fn transform_game_shop_info_from_packet(payload: &Value) -> Option<Value> {
    let item = payload
        .get("item")
        .filter(|value| value.is_object())
        .unwrap_or(payload);
    let info = item.get("info").filter(|value| value.is_object());
    let game_shop_index = value_i32(item.get("gIndex").or_else(|| item.get("g_index")))?;
    let stock_level = value_i32(
        payload
            .get("stockLevel")
            .or_else(|| payload.get("stock_level"))
            .or_else(|| item.get("stockLevel"))
            .or_else(|| item.get("stock_level")),
    )
    .unwrap_or_else(|| value_i32(item.get("stock")).unwrap_or(0));
    Some(json!({
        "itemIndex": value_i32(item.get("itemIndex").or_else(|| item.get("item_index")))
            .or_else(|| value_i32(info.and_then(|value| value.get("index"))))
            .unwrap_or_default(),
        "gameShopIndex": game_shop_index,
        "itemName": value_string(item.get("itemName"))
            .or_else(|| value_string(info.and_then(|value| value.get("name"))))
            .unwrap_or_else(|| "Item".to_owned()),
        "image": value_u32(item.get("image"))
            .or_else(|| value_u32(info.and_then(|value| value.get("image"))))
            .unwrap_or_default(),
        "itemType": value_u32(item.get("itemType").or_else(|| item.get("item_type")))
            .or_else(|| value_u32(info.and_then(|value| value.get("itemType")).or_else(|| info.and_then(|value| value.get("item_type")))))
            .unwrap_or_default(),
        "goldPrice": value_u32(item.get("goldPrice").or_else(|| item.get("gold_price"))).unwrap_or_default(),
        "creditPrice": value_u32(item.get("creditPrice").or_else(|| item.get("credit_price"))).unwrap_or_default(),
        "count": value_u32(item.get("count")).unwrap_or(1),
        "class": value_string(item.get("class")).unwrap_or_else(|| "All".to_owned()),
        "category": value_string(item.get("category")).unwrap_or_default(),
        "stock": value_i32(item.get("stock")).unwrap_or_default(),
        "stockLevel": stock_level,
        "deal": item.get("deal").and_then(Value::as_bool).unwrap_or(false),
        "topItem": item.get("topItem").or_else(|| item.get("top_item")).and_then(Value::as_bool).unwrap_or(false),
        "dateBinaryDatetime": value_i64(item.get("dateBinaryDatetime").or_else(|| item.get("date_binary_datetime"))).unwrap_or_default(),
        "canBuyCredit": item.get("canBuyCredit").or_else(|| item.get("can_buy_credit")).and_then(Value::as_bool).unwrap_or(false),
        "canBuyGold": item.get("canBuyGold").or_else(|| item.get("can_buy_gold")).and_then(Value::as_bool).unwrap_or(false),
    }))
}

fn transform_game_shop_stock_from_packet(payload: &Value) -> Option<Value> {
    let game_shop_index = value_i32(
        payload
            .get("gIndex")
            .or_else(|| payload.get("g_index"))
            .or_else(|| payload.get("gameShopIndex"))
            .or_else(|| payload.get("game_shop_index"))
            .or_else(|| payload.get("index")),
    )?;
    let stock_level = value_i32(
        payload
            .get("stockLevel")
            .or_else(|| payload.get("stock_level"))
            .or_else(|| payload.get("stock")),
    )?;
    Some(json!({ "gameShopIndex": game_shop_index, "stockLevel": stock_level }))
}

/// `NPCGoods` is the ordinary NPC-only catalogue. It must not be folded into
/// the separately correlated cash GameShop catalogue above. `NPCGoods` and
/// `NPCSell` remain separate packet signals; the shared ShopModel folds that
/// ordered pair into one BUYSELL capability set, while a lone NPCSell stays
/// sell-only.
fn npc_shop_service_from_packet(
    packet: &str,
    payload: &Value,
) -> Option<mir2_client_bevy::shop::NpcShopServiceSignal> {
    use mir2_client_bevy::shop::{NpcShopServiceMode, NpcShopServiceSignal};
    let signal = match packet {
        "NPCGoods" => NpcShopServiceSignal {
            mode: NpcShopServiceMode::Buy,
            repair_rate: None,
        },
        "NPCSell" => NpcShopServiceSignal {
            mode: NpcShopServiceMode::Sell,
            repair_rate: None,
        },
        "NPCRepair" | "NPCSRepair" => NpcShopServiceSignal {
            mode: if packet == "NPCRepair" {
                NpcShopServiceMode::Repair
            } else {
                NpcShopServiceMode::SpecialRepair
            },
            repair_rate: payload
                .get("rate")
                .and_then(Value::as_f64)
                .filter(|rate| rate.is_finite() && *rate >= 0.0)
                .map(|rate| rate as f32),
        },
        _ => return None,
    };
    signal.is_valid().then_some(signal)
}

fn push_native_npc_shop_service(
    signal: mir2_client_bevy::shop::NpcShopServiceSignal,
) -> Result<(), String> {
    if !signal.is_valid() {
        return Err("invalid NPC shop service signal".to_owned());
    }
    let json = serde_json::to_string(&signal).map_err(|error| error.to_string())?;
    let _ = mir2_bevy_runtime::native_ingest::push_native_npc_shop_service(json);
    Ok(())
}

fn transform_shop_model_from_packet(payload: &Value) -> Value {
    let goods: Vec<Value> = payload
        .get("list")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .enumerate()
                .filter_map(|(slot, item)| shop_good_json(item, slot))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "goods": goods,
        "selected_id": Value::Null,
        "selected_bag_slot_for_sell": Value::Null,
        "selected_bag_slot_for_repair": Value::Null,
    })
}

fn transform_shop_model_from_snapshot(payload: &Value) -> Value {
    let list = ["shopGoods", "shop_goods", "npcGoods", "npc_goods"]
        .iter()
        .find_map(|key| payload.get(*key))
        .and_then(Value::as_array);
    let goods: Vec<Value> = list
        .map(|list| {
            list.iter()
                .enumerate()
                .filter_map(|(slot, item)| shop_good_json(item, slot))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "goods": goods,
        "selected_id": Value::Null,
        "selected_bag_slot_for_sell": Value::Null,
        "selected_bag_slot_for_repair": Value::Null,
    })
}

fn shop_good_json(item: &Value, fallback: usize) -> Option<Value> {
    let id = value_u64(
        item.get("uniqueId")
            .or_else(|| item.get("unique_id"))
            .or_else(|| item.get("id"))
            .or_else(|| item.get("itemIndex"))
            .or_else(|| item.get("item_index")),
    )?;
    Some(json!({
        "unique_id": id,
        "name": value_string(item.get("name")).unwrap_or_else(|| format!("Item #{id}")),
        "price": value_u32(item.get("price")).unwrap_or_default(),
        "count": value_u32(item.get("count")).and_then(|value| u16::try_from(value).ok()).unwrap_or(1),
        "stock": value_i32(item.get("stock")).unwrap_or(-1),
        "panel_type": value_u32(item.get("panelType").or_else(|| item.get("panel_type")))
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(u8::try_from(fallback).unwrap_or_default()),
        "icon": value_u32(item.get("icon")).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
        "description": value_string(item.get("description")).unwrap_or_default(),
    }))
}

fn try_transform_shop_model_from_packet(payload: &Value) -> Option<Value> {
    let list = payload.get("list")?.as_array()?;
    if list
        .iter()
        .enumerate()
        .any(|(slot, item)| shop_good_json(item, slot).is_none())
    {
        return None;
    }
    Some(transform_shop_model_from_packet(payload))
}

fn payload_has_valid_shop_array(payload: &Value) -> bool {
    ["shopGoods", "shop_goods", "npcGoods", "npc_goods"]
        .iter()
        .find_map(|key| payload.get(*key))
        .is_some_and(Value::is_array)
}

fn mail_source(payload: &Value) -> Option<&Value> {
    payload
        .get("stage5Systems")
        .and_then(|value| value.get("mail"))
        .or_else(|| {
            payload
                .get("stage5_systems")
                .and_then(|value| value.get("mail"))
        })
        .or_else(|| payload.get("mails"))
        .or_else(|| payload.get("mail"))
        .or_else(|| payload.get("stage5").and_then(|value| value.get("mail")))
}

fn try_transform_mail_model_from_snapshot(payload: &Value) -> Option<Value> {
    mail_model_from_entries(mail_source(payload)?.as_array()?)
}

fn try_transform_mail_model_from_packet(payload: &Value) -> Option<Value> {
    mail_model_from_entries(payload.get("mail")?.as_array()?)
}

fn mail_model_from_entries(entries: &[Value]) -> Option<Value> {
    let mails = entries
        .iter()
        .map(mail_message_json)
        .collect::<Option<Vec<_>>>()?;
    Some(json!({ "mails": mails, "selected_id": Value::Null }))
}

fn mail_message_json(mail: &Value) -> Option<Value> {
    let id = value_u64(mail.get("mailId").or_else(|| mail.get("mail_id")))?;
    let message = mail
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let items = mail
        .get("items")?
        .as_array()?
        .iter()
        .map(mail_attachment_json)
        .collect::<Option<Vec<_>>>()?;
    Some(json!({
        "id": id,
        "sender": mail.get("senderName").or_else(|| mail.get("sender")).and_then(Value::as_str).unwrap_or("System"),
        "subject": message.lines().next().unwrap_or("Mail"),
        "body": message,
        "gold": value_u32(mail.get("gold")).unwrap_or_default(),
        "items": items,
        "claimed": mail.get("collected").or_else(|| mail.get("claimed")).and_then(Value::as_bool).unwrap_or(false),
        "locked": mail.get("locked").and_then(Value::as_bool).unwrap_or(false),
        "read": mail.get("opened").or_else(|| mail.get("read")).and_then(Value::as_bool).unwrap_or(false),
    }))
}

fn mail_attachment_json(item: &Value) -> Option<Value> {
    if let Some(name) = item.as_str().filter(|name| !name.is_empty()) {
        return Some(json!({ "name": name, "count": 1 }));
    }
    let item_index = value_i32(item.get("itemIndex").or_else(|| item.get("item_index")));
    let name = value_string(item.get("name"));
    let key = value_string(item.get("key"));
    if item_index.is_none() && name.is_none() && key.is_none() {
        return None;
    }
    Some(json!({
        "uniqueId": value_u64(item.get("uniqueId").or_else(|| item.get("unique_id"))),
        "itemIndex": item_index,
        "key": key,
        "name": name,
        "count": value_u32(item.get("count")).and_then(|value| u16::try_from(value).ok()).unwrap_or(1),
        "currentDura": value_u32(item.get("currentDura").or_else(|| item.get("current_dura"))).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
        "maxDura": value_u32(item.get("maxDura").or_else(|| item.get("max_dura"))).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
        "soulBoundId": value_i32(item.get("soulBoundId").or_else(|| item.get("soul_bound_id"))).unwrap_or_default(),
        "identified": item.get("identified").and_then(Value::as_bool).unwrap_or(false),
        "cursed": item.get("cursed").and_then(Value::as_bool).unwrap_or(false),
        "gemCount": value_u32(item.get("gemCount").or_else(|| item.get("gem_count"))).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
    }))
}

fn mail_packet_body(payload: &Value) -> &Value {
    payload
        .get("data")
        .filter(|value| value.is_object())
        .unwrap_or(payload)
}

fn mail_operation_feedback(
    packet: &str,
    payload: &Value,
    claim_mail_id: Option<u64>,
) -> Option<PendingMailOperationFeedback> {
    let body = mail_packet_body(payload);
    let result = value_i32(body.get("result")).filter(|result| matches!(result, 1 | -1))?;
    let (kind, mail_id) = match packet {
        "MailSent" => ("send", None),
        // The response has no reliable id: use only the claim that this
        // connection recorded after its WebSocket write completed.
        "ParcelCollected" => ("collect", Some(claim_mail_id?)),
        _ => return None,
    };
    Some(PendingMailOperationFeedback {
        kind,
        success: result == 1,
        mail_id,
    })
}

fn enqueue_mail_feedback(
    pending: &mut VecDeque<PendingMailOperationFeedback>,
    feedback: PendingMailOperationFeedback,
) -> bool {
    if pending.len() >= MAX_PENDING_MAIL_FEEDBACK {
        return false;
    }
    pending.push_back(feedback);
    true
}

fn merge_mail_operation_feedback(
    model: &mut Value,
    pending: &VecDeque<PendingMailOperationFeedback>,
) -> bool {
    let Some(feedback) = pending.front() else {
        return false;
    };
    let Some(mails) = model.get_mut("mails").and_then(Value::as_array_mut) else {
        return false;
    };
    mails.push(json!({
        "id": u64::MAX,
        "sender": "",
        "subject": "",
        "body": "",
        "gold": 0,
        "items": [],
        "claimed": false,
        "locked": true,
        "read": true,
        "operation": { "kind": feedback.kind, "success": feedback.success, "mailId": feedback.mail_id },
    }));
    true
}

fn push_mail_model_with_feedback(
    model: &mut Value,
    pending: &mut VecDeque<PendingMailOperationFeedback>,
) -> Result<bool, String> {
    deliver_mail_model_with_feedback(model, pending, |json| {
        mir2_bevy_runtime::native_ingest::push_native_mail_model(json)
    })
}

fn deliver_mail_model_with_feedback(
    model: &mut Value,
    pending: &mut VecDeque<PendingMailOperationFeedback>,
    deliver: impl FnOnce(String) -> bool,
) -> Result<bool, String> {
    let feedback_attached = merge_mail_operation_feedback(model, pending);
    let json = serde_json::to_string(model).map_err(|error| error.to_string())?;
    let delivered = deliver(json);
    if delivered && feedback_attached {
        pending.pop_front();
    }
    Ok(delivered)
}

fn try_transform_storage_model_from_snapshot(payload: &Value) -> Option<Value> {
    let source = payload
        .get("storageItems")
        .or_else(|| payload.get("storage_items"))?
        .as_array()?;
    let items = storage_items_json(source)?;
    Some(json!({
        "items": items,
        "size": value_u32(payload.get("storageSize").or_else(|| payload.get("storage_size"))).and_then(|value| u16::try_from(value).ok()).unwrap_or(30),
        "has_password": payload.get("hasStoragePassword").or_else(|| payload.get("has_storage_password")).and_then(Value::as_bool).unwrap_or(false),
        "unlocked": payload.get("storageUnlocked").or_else(|| payload.get("storage_unlocked")).and_then(Value::as_bool).unwrap_or(true),
        "has_expanded": payload.get("hasExpandedStorage").or_else(|| payload.get("has_expanded_storage")).and_then(Value::as_bool).unwrap_or(false),
        "expiry": value_i64(payload.get("expiryTimeBinaryDatetime").or_else(|| payload.get("expiry_time_binary_datetime"))).unwrap_or_default(),
        "selected_bag_slot": Value::Null,
        "selected_storage_slot": Value::Null,
        "password_draft": "",
        "new_password_draft": "",
        "confirm_password_draft": "",
    }))
}

fn transform_storage_items_from_packet(payload: &Value) -> Option<Value> {
    Some(json!({ "items": storage_items_json(payload.get("storage")?.as_array()?)? }))
}

fn storage_items_json(entries: &[Value]) -> Option<Vec<Value>> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, item)| !item.is_null())
        .map(|(slot, item)| {
            let unique_id = value_u64(item.get("uniqueId").or_else(|| item.get("unique_id")));
            let item_index = value_i32(item.get("itemIndex").or_else(|| item.get("item_index")));
            if unique_id.is_none() && item_index.is_none() {
                return None;
            }
            let key = unique_id.map(|value| value.to_string()).or_else(|| item_index.map(|value| value.to_string()))?;
            let mut mapped = json!({
                "uniqueId": unique_id,
                "key": key,
                "name": value_string(item.get("name")).unwrap_or_else(|| item_index.map(|value| format!("Item #{value}")).unwrap_or_default()),
                "quantity": value_u32(item.get("count").or_else(|| item.get("quantity"))).unwrap_or(1),
                "slot": normalized_slot(item.get("slot"), u32::try_from(slot).unwrap_or_default()),
                "container": 4,
            });
            extend_item_metadata(&mut mapped, item);
            Some(mapped)
        })
        .collect()
}

fn transform_storage_patch_from_packet(packet: &str, payload: &Value) -> Option<Value> {
    let request_id = payload
        .get("requestId")
        .or_else(|| payload.get("request_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    match packet {
        "StoreItem" | "StoreItemV2" => {
            if packet == "StoreItemV2" && request_id.is_none() {
                return None;
            }
            let request_id = (packet == "StoreItemV2").then_some(request_id).flatten();
            let mut ack = json!({
                "operation": "deposit",
                "from": value_i32(payload.get("from"))?,
                "to": value_i32(payload.get("to"))?,
                "success": payload.get("success").and_then(Value::as_bool)?,
            });
            if let Some(request_id) = request_id {
                ack["requestId"] = json!(request_id);
            }
            Some(json!({ "ack": ack }))
        }
        "TakeBackItem" | "TakeBackItemV2" => {
            if packet == "TakeBackItemV2" && request_id.is_none() {
                return None;
            }
            let request_id = (packet == "TakeBackItemV2").then_some(request_id).flatten();
            let mut ack = json!({
                "operation": "withdraw",
                "from": value_i32(payload.get("from"))?,
                "to": value_i32(payload.get("to"))?,
                "success": payload.get("success").and_then(Value::as_bool)?,
            });
            if let Some(request_id) = request_id {
                ack["requestId"] = json!(request_id);
            }
            Some(json!({ "ack": ack }))
        }
        "StorageUnlockResult" => {
            let result = value_i32(payload.get("result"))?;
            let has_password = payload.get("hasPassword").and_then(Value::as_bool)?;
            let mut patch = json!({
                "has_password": has_password,
                "ack": { "operation": "unlock", "success": result == 0 }
            });
            if result == 0 || !has_password {
                patch["unlocked"] = json!(true);
            }
            Some(patch)
        }
        "StoragePasswordResult" => {
            let result = value_i32(payload.get("result"))?;
            let removing = payload.get("removing").and_then(Value::as_bool)?;
            let has_password = payload.get("hasPassword").and_then(Value::as_bool)?;
            let mut patch = json!({
                "has_password": has_password,
                "expiry": value_i64(payload.get("lastSetBinaryDatetime"))?,
                "ack": {
                    "operation": if removing { "removePassword" } else { "setPassword" },
                    "success": result == 4,
                },
            });
            if result == 4 || !has_password {
                patch["unlocked"] = json!(true);
            }
            Some(patch)
        }
        "ResizeStorage" => Some(json!({
            "size": value_u32(payload.get("size")).and_then(|value| u16::try_from(value).ok())?,
            "has_expanded": payload.get("hasExpandedStorage").and_then(Value::as_bool)?,
            "expiry": value_i64(payload.get("expiryTimeBinaryDatetime"))?,
        })),
        _ => None,
    }
}

fn wallet_value(payload: &Value, field: &str) -> Option<u32> {
    value_u32(payload.get(field))
        .or_else(|| value_u32(payload.get("value")))
        .or_else(|| value_u32(payload.get("amount")))
}

fn transform_skill_model(payload: &Value) -> Value {
    let skills = payload
        .get("knownSkills")
        .or_else(|| payload.get("known_skills"))
        .or_else(|| payload.get("skills"))
        .and_then(Value::as_array)
        .map(|skills| {
            skills
                .iter()
                .take(MAX_LEARNED_SKILLS)
                .enumerate()
                .map(|(idx, skill)| {
                    let id = value_u32(skill.get("id")).unwrap_or(idx as u32);
                    let key = skill.get("key").and_then(Value::as_str).map(str::to_owned);
                    let name = skill
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    let level = value_u32(skill.get("level"))
                        .and_then(|value| u8::try_from(value).ok())
                        .unwrap_or(0);
                    let delay_ms = value_i64(
                        skill
                            .get("delayMs")
                            .or_else(|| skill.get("delay_ms"))
                            .or_else(|| skill.get("cooldownMs")),
                    )
                    .unwrap_or(0);
                    let mut transformed = serde_json::Map::new();
                    transformed.insert("id".to_owned(), json!(id));
                    transformed.insert("name".to_owned(), json!(name));
                    transformed.insert("level".to_owned(), json!(level));
                    transformed.insert("key".to_owned(), json!(key));
                    transformed.insert("cooldown_ms".to_owned(), json!(delay_ms.max(0)));
                    transformed.insert(
                        "spell".to_owned(),
                        optional_non_empty_string_value(skill.get("spell")),
                    );
                    transformed.insert(
                        "castKind".to_owned(),
                        optional_non_empty_string_value(skill.get("castKind")),
                    );
                    transformed.insert(
                        "canUse".to_owned(),
                        skill.get("canUse").cloned().unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "offensive".to_owned(),
                        skill.get("offensive").cloned().unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "hotkey".to_owned(),
                        skill.get("hotkey").cloned().unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "cooldownRemainingTicks".to_owned(),
                        value_u32(
                            skill
                                .get("cooldownRemainingTicks")
                                .or_else(|| skill.get("cooldown_remaining_ticks")),
                        )
                        .map(|value| json!(value))
                        .unwrap_or_else(|| json!(0)),
                    );
                    transformed.insert(
                        "mpCost".to_owned(),
                        value_u32(skill.get("mpCost").or_else(|| skill.get("mp_cost")))
                            .map(|value| json!(value))
                            .unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "castTimeMs".to_owned(),
                        value_i64(
                            skill
                                .get("castTimeMs")
                                .or_else(|| skill.get("cast_time_ms")),
                        )
                        .map(|value| json!(value))
                        .unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "experience".to_owned(),
                        value_u32(skill.get("experience"))
                            .and_then(|value| u16::try_from(value).ok())
                            .map(|value| json!(value))
                            .unwrap_or(Value::Null),
                    );
                    Value::Object(transformed)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({ "skills": skills })
}

fn value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

/// Convert the simulation's named equipment slots to Crystal's stable slot
/// indices. Bag and belt entries already carry numeric slots, while equipment
/// entries intentionally expose names such as `weapon` and `armour`.
fn normalized_slot(value: Option<&Value>, fallback: u32) -> u32 {
    if let Some(slot) = value_u32(value) {
        return slot;
    }

    let Some(name) = value.and_then(Value::as_str) else {
        return fallback;
    };
    match name.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "weapon" => 0,
        "armour" | "armor" => 1,
        "helmet" => 2,
        "torch" => 3,
        "necklace" => 4,
        "bracelet-left" | "braceletl" => 5,
        "bracelet-right" | "braceletr" => 6,
        "ring-left" | "ringl" => 7,
        "ring-right" | "ringr" => 8,
        "amulet" => 9,
        "belt" => 10,
        "boots" => 11,
        "stone" => 12,
        "mount" => 13,
        _ => fallback,
    }
}

/// Transform gateway `Chat` and `ObjectChat` packet payloads into the shared
/// renderer-neutral chat line. Crystal uses `message` for direct/system chat
/// and `text` for object chat, so the packet kind selects the authoritative
/// field instead of accepting an unrelated similarly named property.
fn transform_chat_line(packet: &str, payload: &Value) -> Option<mir2_client_bevy::chat::ChatLine> {
    let text_field = match packet {
        "Chat" => "message",
        "ObjectChat" => "text",
        _ => return None,
    };
    let text = payload
        .get(text_field)
        .and_then(Value::as_str)
        .map(str::to_owned)?;
    let channel = payload
        .get("chatType")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "normal".to_owned());
    Some(mir2_client_bevy::chat::ChatLine { text, channel })
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::inventory::InventoryModel` JSON shape.
///
/// Container mapping: inventory items → 0 (bag), belt items → 1, equipment → 2.
fn transform_inventory_model(payload: &Value) -> Value {
    let gold = value_u32_or(payload.get("gold"), 0);

    let map_items = |items: Option<&Value>, container: u8| -> Vec<Value> {
        items
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let fallback_slot = u32::try_from(index).unwrap_or(0);
                        let unique_id = item
                            .get("uniqueId")
                            .or_else(|| item.get("unique_id"))
                            .and_then(value_u64_ref);
                        let key = value_string(item.get("key"))
                            .or_else(|| value_string(item.get("itemIndex")))
                            .or_else(|| value_string(item.get("item_index")))
                            .or_else(|| unique_id.map(|id| id.to_string()))
                            .unwrap_or_else(|| index.to_string());
                        let mut mapped = json!({
                            "uniqueId": unique_id,
                            "key": key,
                            "name": value_string(item.get("name")).unwrap_or_default(),
                            "quantity": value_u32(item.get("quantity").or_else(|| item.get("count"))).unwrap_or(1),
                            "slot": normalized_slot(item.get("slot"), fallback_slot),
                            "container": container,
                        });
                        extend_item_metadata(&mut mapped, item);
                        mapped
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let mut items = Vec::new();
    items.extend(map_items(payload.get("inventoryItems"), 0));
    items.extend(map_items(payload.get("beltItems"), 1));
    items.extend(map_items(payload.get("equipmentItems"), 2));

    json!({ "gold": gold, "items": items })
}

/// Copy the item fields that the simulation already exposes into the shared
/// native read model.  Keep this schema tolerant of both packet-style snake
/// case and web snapshot camel case: native sessions can receive either while
/// reconnecting or applying a storage patch.
fn extend_item_metadata(mapped: &mut Value, item: &Value) {
    let metadata = [
        ("icon", &["icon"][..]),
        ("description", &["description"][..]),
        (
            "durabilityCurrent",
            &[
                "durabilityCurrent",
                "durability_current",
                "currentDura",
                "current_dura",
            ][..],
        ),
        (
            "durabilityMax",
            &["durabilityMax", "durability_max", "maxDura", "max_dura"][..],
        ),
        ("sellValue", &["sellValue", "sell_value", "price"][..]),
        ("equipSlot", &["equipSlot", "equip_slot"][..]),
        ("grade", &["grade"][..]),
        ("attack", &["attack"][..]),
        ("defence", &["defence", "defense"][..]),
        ("addedAttack", &["addedAttack", "added_attack"][..]),
        (
            "addedDefence",
            &[
                "addedDefence",
                "added_defence",
                "addedDefense",
                "added_defense",
            ][..],
        ),
        ("addedLuck", &["addedLuck", "added_luck"][..]),
        ("shape", &["shape"][..]),
        ("socketSlots", &["socketSlots", "socket_slots"][..]),
    ];
    let Some(target) = mapped.as_object_mut() else {
        return;
    };
    for (target_name, candidates) in metadata {
        if let Some(value) = candidates.iter().find_map(|name| item.get(*name)).cloned() {
            target.insert(target_name.to_owned(), value);
        }
    }
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::entities::EntityModelSet` JSON shape.
fn transform_entity_model_set(payload: &Value) -> Value {
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .map(|entity| {
                    let object_id = entity
                        .get("objectId")
                        .and_then(object_id_string)
                        .unwrap_or_default();
                    json!({
                        "objectId": object_id,
                        "kind": entity.get("kind").cloned().unwrap_or(json!("monster")),
                        "name": entity.get("name").cloned().unwrap_or(json!("")),
                        "x": entity.get("x").cloned().unwrap_or(json!(0)),
                        "y": entity.get("y").cloned().unwrap_or(json!(0)),
                        "level": entity.get("level"),
                        "direction": entity.get("direction"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({ "entities": entities })
}

/// Extract the movement window from a gateway entity, matching the Web
/// client's `movementStartedAt` / `movementUntil` convention.
fn movement_window(entity: &Value) -> (Option<f64>, Option<f64>) {
    let started = entity.get("movementStartedAt").and_then(Value::as_f64);
    let until = entity.get("movementUntil").and_then(Value::as_f64);
    let duration = match (started, until) {
        (Some(start), Some(end)) if end > start => Some(end - start),
        _ => None,
    };
    (started, duration)
}

fn optional_non_empty_string_value(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| json!(value))
        .unwrap_or(Value::Null)
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::read_model::UiReadModel` JSON shape.
fn transform_ui_read_model(payload: &Value) -> Value {
    let self_player = payload
        .get("entities")
        .and_then(Value::as_array)
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))
        });

    json!({
        "player": {
            "hp": value_i32_or(payload.get("playerHp"), 0),
            "maxHp": value_i32_or(payload.get("playerMaxHp"), 0),
            "mp": value_i32_or(payload.get("playerMp"), 0),
            "maxMp": value_i32_or(payload.get("playerMaxMp"), 0),
            "gold": value_u32_or(payload.get("gold"), 0),
            "credit": value_u32_or(payload.get("credit"), 0),
            "level": value_u32_or(self_player.and_then(|entity| entity.get("level")), 0),
            "experience": value_i64_or(payload.get("playerExperience"), 0),
            "maxExperience": value_i64_or(payload.get("playerMaxExperience"), 0),
            "currentWeight": value_u16_or(payload.get("currentWeight"), 0),
            "maxWeight": value_u16_or(payload.get("maxWeight"), 0),
            "name": value_string(self_player.and_then(|entity| entity.get("name"))),
            "className": value_string(
                self_player
                    .and_then(|entity| entity.get("class"))
                    .or_else(|| self_player.and_then(|entity| entity.get("className"))),
            ),
            "mapName": value_string(payload.get("mapTitle")),
        }
    })
}

/// Merge absolute wallet fields carried by a full snapshot or UserInformation
/// packet into the gateway-local packet-first wallet cursor. The cursor only
/// feeds read-model patches; it never claims that a purchase succeeded.
fn update_wallet_from_snapshot(last_wallet: &mut Option<WalletState>, payload: &Value) {
    let mut wallet = last_wallet.unwrap_or_default();
    let mut changed = false;
    if let Some(gold) = value_u32(payload.get("gold")) {
        wallet.gold = gold;
        changed = true;
    }
    if let Some(credit) = value_u32(payload.get("credit")) {
        wallet.credit = credit;
        changed = true;
    }
    if changed {
        *last_wallet = Some(wallet);
    }
}

fn merge_wallet_into_world(last_world_payload: &mut Option<Value>, wallet: Option<WalletState>) {
    let Some(payload) = last_world_payload.as_mut() else {
        return;
    };
    merge_wallet_into_payload(payload, wallet);
}

fn merge_wallet_into_payload(payload: &mut Value, wallet: Option<WalletState>) {
    let Some(wallet) = wallet else {
        return;
    };
    payload["gold"] = json!(wallet.gold);
    payload["credit"] = json!(wallet.credit);
}

/// Crystal's Gained/LoseGold and Gained/LoseCredit packets carry deltas, not
/// the resulting wallet total. Keep the shared read model timely by applying
/// the signed delta to the last authoritative wallet cursor, then fold the
/// absolute value into the latest packet-first world payload as well.
fn apply_wallet_delta(
    last_wallet: &mut Option<WalletState>,
    last_world_payload: &mut Option<Value>,
    field: &str,
    amount: Option<u32>,
    gained: bool,
) -> Option<u32> {
    let amount = amount?;
    if last_wallet.is_none() {
        let mut wallet = WalletState::default();
        if let Some(payload) = last_world_payload.as_ref() {
            wallet.gold = value_u32(payload.get("gold")).unwrap_or_default();
            wallet.credit = value_u32(payload.get("credit")).unwrap_or_default();
        }
        *last_wallet = Some(wallet);
    }
    let wallet = last_wallet.as_mut()?;
    let value = match field {
        "gold" if gained => {
            wallet.gold = wallet.gold.saturating_add(amount);
            wallet.gold
        }
        "gold" => {
            wallet.gold = wallet.gold.saturating_sub(amount);
            wallet.gold
        }
        "credit" if gained => {
            wallet.credit = wallet.credit.saturating_add(amount);
            wallet.credit
        }
        "credit" => {
            wallet.credit = wallet.credit.saturating_sub(amount);
            wallet.credit
        }
        _ => return None,
    };
    merge_wallet_into_world(last_world_payload, *last_wallet);
    Some(value)
}

/// UserInformation is the first packet-first character bootstrap on native
/// transports and already contains wallet/HP/XP values. Forward it directly
/// instead of waiting for the next periodic worldSnapshot.
fn transform_ui_read_model_from_user_information(payload: &Value) -> Value {
    json!({
        "player": {
            "hp": value_i32(payload.get("hp").or_else(|| payload.get("playerHp"))).unwrap_or_default(),
            "maxHp": value_i32(payload.get("maxHp").or_else(|| payload.get("playerMaxHp"))).unwrap_or_default(),
            "mp": value_i32(payload.get("mp").or_else(|| payload.get("playerMp"))).unwrap_or_default(),
            "maxMp": value_i32(payload.get("maxMp").or_else(|| payload.get("playerMaxMp"))).unwrap_or_default(),
            "gold": value_u32(payload.get("gold")).unwrap_or_default(),
            "credit": value_u32(payload.get("credit")).unwrap_or_default(),
            "level": value_u32(payload.get("level")).unwrap_or_default(),
            "experience": value_i64(payload.get("experience").or_else(|| payload.get("playerExperience"))).unwrap_or_default(),
            "maxExperience": value_i64(payload.get("maxExperience").or_else(|| payload.get("playerMaxExperience"))).unwrap_or_default(),
            "currentWeight": value_u32(payload.get("currentWeight")).unwrap_or_default(),
            "maxWeight": value_u32(payload.get("maxWeight")).unwrap_or_default(),
            "name": value_string(payload.get("name")),
            "className": value_string(payload.get("class").or_else(|| payload.get("className"))),
            "mapName": value_string(payload.get("mapTitle").or_else(|| payload.get("mapName")))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio::time::{sleep, timeout};
    use tokio_tungstenite::accept_async;

    #[test]
    fn native_lighting_publisher_retries_backpressure_and_clears_per_generation() {
        let mut bridge = NativeLightingBridge::default();
        bridge.set_generation(7);
        let mut publisher = NativeLightingPublisher {
            bridge,
            assets: NativeLightAssets::complete_fixture(),
            map_frame_offsets: HashMap::new(),
            last_pushed_json: None,
        };
        let state = json!({"enabled": false, "mapLights": [], "entityLights": []});
        let mut attempts = 0;
        publisher.publish_with(state.clone(), |_| {
            attempts += 1;
            false
        });
        assert!(publisher.last_pushed_json.is_none());
        publisher.publish_with(state.clone(), |_| {
            attempts += 1;
            true
        });
        assert_eq!(attempts, 2, "failed enqueue must remain dirty");
        assert!(publisher.last_pushed_json.is_some());
        publisher.publish_with(state, |_| {
            attempts += 1;
            true
        });
        assert_eq!(attempts, 2, "accepted identical state is coalesced");

        let snapshot = GatewayEnvelope {
            kind: "worldSnapshot".to_owned(),
            packet: None,
            payload: Some(json!({
                "mapFileName":"lighting-test-map",
                "lightSetting":4,
                "playerObjectId":1000,
                "sceneView":{"center":{"x":10,"y":20}},
                "entities":[{"objectId":1000,"kind":"selfPlayer","x":10,"y":20}]
            })),
        };
        publisher.last_pushed_json = None;
        let mut snapshot_pushes = 0;
        publisher.observe_envelope_with(&snapshot, |_| {
            snapshot_pushes += 1;
            true
        });
        publisher.observe_envelope_with(&snapshot, |_| {
            snapshot_pushes += 1;
            true
        });
        assert_eq!(
            snapshot_pushes, 1,
            "an unchanged repeated world snapshot must not enqueue lighting twice"
        );

        publisher.bridge.observe_packet(
            "MapInformation",
            &json!({"fileName":"0", "lights":4, "mapDarkLight":2}),
        );
        publisher.reset_scene();
        assert_eq!(
            publisher.bridge.build_render_state(
                &Value::Null,
                None,
                &publisher.map_frame_offsets,
                &native_lighting_default_motion(),
                &publisher.assets,
            )["enabled"],
            json!(false)
        );
        assert!(publisher.last_pushed_json.is_none());
    }

    #[test]
    fn recovered_npc_goods_populates_only_the_independent_npc_shop_model() {
        let model = try_transform_shop_model_from_packet(&json!({
            "list": [{ "uniqueId": 9, "name": "Potion", "price": 50, "count": 20 }],
            "rate": 1.0,
            "panelType": 0,
        }))
        .expect("complete NPCGoods payload");
        let shop = serde_json::from_value::<mir2_client_bevy::shop::ShopModel>(model)
            .expect("ShopModel-compatible NPC catalog");
        assert_eq!(shop.goods.len(), 1);
        assert_eq!(shop.goods[0].unique_id, 9);
        assert_eq!(shop.goods[0].price, 50);
        assert!(try_transform_shop_model_from_packet(&json!({
            "list": [{ "name": "missing identity" }]
        }))
        .is_none());
    }

    #[test]
    fn recovered_receive_mail_feedback_is_bounded_and_waits_for_accepted_delivery() {
        let mut pending = VecDeque::new();
        let feedback = mail_operation_feedback("MailSent", &json!({ "result": 1 }), None)
            .expect("valid mail acknowledgement");
        assert!(enqueue_mail_feedback(&mut pending, feedback));
        assert!(!enqueue_mail_feedback(
            &mut pending,
            mail_operation_feedback("MailSent", &json!({ "result": -1 }), None)
                .expect("second valid acknowledgement"),
        ));

        let mut model = try_transform_mail_model_from_packet(&json!({
            "mail": [{
                "mailId": 77,
                "senderName": "GM",
                "message": "Gift",
                "items": [{ "item_index": 123, "count": 1 }]
            }]
        }))
        .expect("ReceiveMail payload");
        assert!(
            !deliver_mail_model_with_feedback(&mut model, &mut pending, |_| false)
                .expect("backpressured mail model still serializes")
        );
        assert_eq!(pending.len(), 1, "backpressure must retain the ACK");
        assert!(
            deliver_mail_model_with_feedback(&mut model, &mut pending, |_| true)
                .expect("accepted mail model serializes")
        );
        assert!(
            pending.is_empty(),
            "accepted delivery consumes exactly one ACK"
        );
    }

    #[test]
    fn recovered_storage_packets_decode_only_correlatable_items_and_metadata() {
        let items = transform_storage_items_from_packet(&json!({
            "storage": [null, { "unique_id": 55, "item_index": 321, "count": 3, "slot": 1,
                "icon": 24, "description": "Stored", "current_dura": 8, "max_dura": 10,
                "sell_value": 99, "equip_slot": "Boots", "added_attack": 2, "shape": 4 }]
        }))
        .expect("UserStorage payload");
        let storage = serde_json::from_value::<mir2_client_bevy::storage::StorageModel>(items)
            .expect("StorageModel-compatible item refresh");
        assert_eq!(storage.items.len(), 1);
        assert_eq!(storage.items[0].key, "55");
        assert_eq!(storage.items[0].quantity, 3);
        assert_eq!(storage.items[0].icon, 24);
        assert_eq!(storage.items[0].durability_current, Some(8));
        assert_eq!(storage.items[0].durability_max, Some(10));
        assert_eq!(storage.items[0].sell_value, 99);
        assert_eq!(storage.items[0].equip_slot.as_deref(), Some("Boots"));
        assert_eq!(storage.items[0].added_attack, 2);
        assert_eq!(storage.items[0].shape, Some(4));
        assert!(transform_storage_items_from_packet(&json!({
            "storage": [{ "count": 1 }]
        }))
        .is_none());

        assert_eq!(
            transform_storage_patch_from_packet(
                "ResizeStorage",
                &json!({ "size": 42, "hasExpandedStorage": true, "expiryTimeBinaryDatetime": 99 })
            )
            .expect("ResizeStorage metadata")["size"],
            json!(42)
        );

        let deposit = transform_storage_patch_from_packet(
            "StoreItem",
            &json!({ "from": 3, "to": 9, "success": false }),
        )
        .expect("StoreItem acknowledgement");
        let deposit_ack = serde_json::from_value::<
            mir2_client_bevy::pending_operations::StorageOperationAck,
        >(deposit["ack"].clone())
        .expect("typed deposit acknowledgement");
        assert_eq!(
            deposit_ack,
            mir2_client_bevy::pending_operations::StorageOperationAck::Deposit {
                request_id: None,
                from: 3,
                to: 9,
                success: false,
            }
        );
        let legacy_with_untrusted_id = transform_storage_patch_from_packet(
            "StoreItem",
            &json!({
                "requestId": "st-0000000000000042",
                "from": 3,
                "to": 9,
                "success": true
            }),
        )
        .expect("legacy StoreItem acknowledgement");
        assert!(legacy_with_untrusted_id["ack"].get("requestId").is_none());

        let v2_deposit = transform_storage_patch_from_packet(
            "StoreItemV2",
            &json!({
                "requestId": "st-0000000000000042",
                "from": 3,
                "to": 9,
                "success": true
            }),
        )
        .expect("V2 StoreItem acknowledgement");
        assert_eq!(v2_deposit["ack"]["requestId"], json!("st-0000000000000042"));
        assert_eq!(
            serde_json::from_value::<mir2_client_bevy::pending_operations::StorageOperationAck>(
                v2_deposit["ack"].clone()
            )
            .expect("typed V2 deposit acknowledgement"),
            mir2_client_bevy::pending_operations::StorageOperationAck::Deposit {
                request_id: Some("st-0000000000000042".to_owned()),
                from: 3,
                to: 9,
                success: true,
            }
        );
        assert!(transform_storage_patch_from_packet(
            "StoreItemV2",
            &json!({ "from": 3, "to": 9, "success": true }),
        )
        .is_none());

        let password_failure = transform_storage_patch_from_packet(
            "StoragePasswordResult",
            &json!({
                "result": 1,
                "removing": true,
                "hasPassword": true,
                "lastSetBinaryDatetime": 123
            }),
        )
        .expect("password failure acknowledgement");
        assert_eq!(password_failure["ack"]["operation"], "removePassword");
        assert_eq!(password_failure["ack"]["success"], false);

        let resize = transform_storage_patch_from_packet(
            "ResizeStorage",
            &json!({ "size": 42, "hasExpandedStorage": true, "expiryTimeBinaryDatetime": 99 }),
        )
        .expect("ResizeStorage metadata-only patch");
        assert!(
            resize.get("ack").is_none(),
            "a snapshot is not an expand ACK"
        );

        use mir2_client_bevy::shop::NpcShopServiceMode;
        assert_eq!(
            npc_shop_service_from_packet("NPCGoods", &json!({}))
                .unwrap()
                .mode,
            NpcShopServiceMode::Buy
        );
        assert_eq!(
            npc_shop_service_from_packet("NPCSell", &json!({}))
                .unwrap()
                .mode,
            NpcShopServiceMode::Sell
        );
        let repair = npc_shop_service_from_packet("NPCRepair", &json!({ "rate": 1.5 }))
            .expect("repair service");
        assert_eq!(repair.mode, NpcShopServiceMode::Repair);
        assert_eq!(repair.repair_rate, Some(1.5));
        assert_eq!(
            npc_shop_service_from_packet("NPCSRepair", &json!({ "rate": 2.0 }))
                .unwrap()
                .mode,
            NpcShopServiceMode::SpecialRepair
        );
        assert!(npc_shop_service_from_packet("NPCRepair", &json!({})).is_none());
        assert!(npc_shop_service_from_packet("NPCSRepair", &json!({ "rate": -1.0 })).is_none());
    }

    #[test]
    fn npc_goods_then_npc_sell_preserves_buy_and_sell_for_one_service_session() {
        use mir2_client_bevy::shop::{NpcShopServiceMode, ShopModel};

        let mut combined = ShopModel::default();
        for (packet, payload) in [
            ("NPCGoods", json!({ "list": [] })),
            ("NPCSell", Value::Null),
        ] {
            let signal =
                npc_shop_service_from_packet(packet, &payload).expect("valid NPC service packet");
            assert!(combined.apply_service_signal(signal));
        }
        assert_eq!(combined.service_mode, NpcShopServiceMode::Sell);
        assert!(combined.allows_buy(), "NPCGoods capability was lost");
        assert!(combined.allows_sell(), "NPCSell capability was not added");

        let mut sell_only = ShopModel::default();
        let signal = npc_shop_service_from_packet("NPCSell", &Value::Null)
            .expect("standalone NPCSell service packet");
        assert!(sell_only.apply_service_signal(signal));
        assert!(!sell_only.allows_buy());
        assert!(sell_only.allows_sell());

        assert!(combined.apply_service_signal(
            npc_shop_service_from_packet("NPCRepair", &json!({ "rate": 1.25 }))
                .expect("valid repair service packet")
        ));
        assert!(!combined.allows_buy());
        assert!(!combined.allows_sell());
        assert!(combined.allows_repair());
        assert!(!combined.allows_special_repair());
    }

    async fn receive_wire_type(
        socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        expected_type: &str,
    ) -> Value {
        loop {
            let frame = timeout(Duration::from_secs(2), socket.next())
                .await
                .expect("client must send a websocket frame before the test deadline")
                .expect("client websocket must remain open")
                .expect("client websocket frame must be valid");
            let Message::Text(text) = frame else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).expect("client wire payload JSON");
            match value.get("type").and_then(Value::as_str) {
                Some("keepAlive") => continue,
                Some(actual) if actual == expected_type => return value,
                Some(actual) => panic!(
                    "expected client wire type {expected_type:?}, received unexpected {actual:?}"
                ),
                None => panic!("client wire payload omitted type: {value}"),
            }
        }
    }

    async fn assert_no_player_command_while_awaiting_resume(
        socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    ) {
        let deadline = Instant::now() + Duration::from_millis(180);
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let frame = match timeout(remaining.min(Duration::from_millis(25)), socket.next()).await
            {
                Ok(Some(Ok(frame))) => frame,
                Ok(Some(Err(error))) => panic!("client websocket frame must be valid: {error}"),
                Ok(None) => panic!("client websocket closed while resume was pending"),
                Err(_) => continue,
            };
            let Message::Text(text) = frame else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).expect("client wire payload JSON");
            match value.get("type").and_then(Value::as_str) {
                Some("keepAlive") => {}
                Some(actual) => panic!(
                    "client replayed or sent {actual:?} while the resume decision was pending"
                ),
                None => panic!("client wire payload omitted type: {value}"),
            }
        }
    }

    #[test]
    fn player_intents_serialize_to_the_browser_command_protocol() {
        assert_eq!(
            PlayerIntent::Walk {
                direction: "up".into()
            }
            .to_json(),
            json!({ "type": "walk", "direction": "up" })
        );
        assert_eq!(
            PlayerIntent::Run {
                direction: "left".into()
            }
            .to_json(),
            json!({ "type": "run", "direction": "left" })
        );
        assert_eq!(
            PlayerIntent::Turn {
                direction: "down".into()
            }
            .to_json(),
            json!({ "type": "turn", "direction": "down" })
        );
    }

    #[test]
    fn resume_credential_is_strictly_bounded_and_debug_redacted() {
        let mut state = NativeResumeClientState::default();
        let token = "A".repeat(MAX_CREDENTIAL_LENGTH);
        state.record_credential(&token, Some(gateway_unix_ms() + 30_000), Some(1));
        assert_eq!(state.resume_credential(), Some(token.as_str()));
        assert!(!format!("{state:?}").contains(&token));

        let rotated = "B".repeat(MAX_CREDENTIAL_LENGTH);
        state.record_credential(&rotated, Some(gateway_unix_ms() + 30_000), Some(1));
        assert_eq!(state.resume_credential(), Some(rotated.as_str()));
        assert!(state.accept_resumed_generation(Some(2)));
        assert!(!state.accept_resumed_generation(Some(2)));

        let mut malformed = NativeResumeClientState::default();
        malformed.record_credential("short", Some(gateway_unix_ms() + 30_000), Some(1));
        malformed.record_credential(
            &format!("{}!", "A".repeat(MAX_CREDENTIAL_LENGTH - 1)),
            Some(gateway_unix_ms() + 30_000),
            Some(2),
        );
        assert!(malformed.resume_credential().is_none());
    }

    #[test]
    fn reconnect_defaults_are_fourteen_seconds_and_five_attempts() {
        let config = NativeReconnectConfig::default();
        assert_eq!(config.resume_deadline, Duration::from_secs(14));
        assert_eq!(config.max_attempts, 5);
    }

    #[test]
    fn reconnect_reset_policy_preserves_transient_loss_and_separates_terminal_states() {
        assert_eq!(
            transport_loss_reset_policy(true),
            ReconnectResetPolicy::Preserve,
            "live credential transport loss must not emit DataReset or SceneReset"
        );
        assert_eq!(
            session_resumed_reset_policy(),
            ReconnectResetPolicy::Scene,
            "sessionResumed emits exactly one SceneReset before its post-resume snapshot"
        );
        assert_eq!(
            terminal_failure_reset_policy(),
            ReconnectResetPolicy::Data,
            "resume rejection/deadline/attempt exhaustion clears session models"
        );
    }

    #[test]
    fn resume_handshake_failure_transitions_retry_then_terminal_and_initial() {
        let config = NativeReconnectConfig::default();
        let token = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let mut state = NativeResumeClientState::default();
        state.record_credential(&token, Some(gateway_unix_ms() + 30_000), Some(1));
        state.begin_reconnect();
        state.retry_attempt = 1;
        assert_eq!(
            resume_handshake_failure_transition(true, &state, config),
            ResumeHandshakeFailure::Retry
        );

        state.retry_attempt = u32::from(config.max_attempts);
        assert_eq!(
            resume_handshake_failure_transition(true, &state, config),
            ResumeHandshakeFailure::TerminalDataReset
        );
        assert_eq!(
            resume_handshake_failure_transition(false, &state, config),
            ResumeHandshakeFailure::InitialDataReset
        );
    }

    #[test]
    fn awaiting_resume_cancel_is_a_single_outer_data_reset_and_never_reconnects() {
        let cancel = GatewayCommand::Wire(NativeOutboundCommand::Disconnect);
        assert_eq!(
            awaiting_resume_command_action(&cancel),
            AwaitingResumeCommandAction::Cancel
        );
        assert_eq!(
            awaiting_resume_command_action(&GatewayCommand::Wire(NativeOutboundCommand::LogOut)),
            AwaitingResumeCommandAction::Cancel
        );

        let token = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let mut state = NativeResumeClientState::default();
        state.record_credential(&token, Some(gateway_unix_ms() + 30_000), Some(1));
        state.begin_reconnect();
        state.clear();
        // The AwaitingResume branch only clears and returns Disconnected;
        // the outer Disconnected arm is therefore the sole reset owner.
        assert_eq!(
            transport_loss_reset_policy(state.has_live_credential()),
            ReconnectResetPolicy::Data
        );
        assert_eq!(
            awaiting_resume_command_action(&GatewayCommand::Player(PlayerIntent::Turn {
                direction: "up".to_owned(),
            })),
            AwaitingResumeCommandAction::Ignore
        );
    }

    #[test]
    fn resume_credential_during_resume_lifecycle_cannot_refresh_deadline_or_attempt_budget() {
        let original = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let rotated = "B".repeat(MAX_CREDENTIAL_LENGTH);
        let mut state = NativeResumeClientState::default();
        state.record_credential(&original, Some(gateway_unix_ms() + 30_000), Some(1));
        state.begin_reconnect();
        state.retry_attempt = 3;
        let started_at = state.reconnect_started_at;

        record_resume_credential_if_allowed(
            ConnectionPhase::AwaitingResume,
            &mut state,
            &rotated,
            Some(gateway_unix_ms() + 30_000),
            Some(2),
        );

        assert_eq!(state.resume_credential(), Some(original.as_str()));
        assert_eq!(state.generation, Some(1));
        assert_eq!(state.retry_attempt, 3);
        assert_eq!(state.reconnect_started_at, started_at);

        record_resume_credential_if_allowed(
            ConnectionPhase::Resumed,
            &mut state,
            &rotated,
            Some(gateway_unix_ms() + 30_000),
            Some(2),
        );
        assert_eq!(state.resume_credential(), Some(original.as_str()));
        assert_eq!(state.generation, Some(1));
        assert_eq!(state.retry_attempt, 3);
        assert_eq!(state.reconnect_started_at, started_at);
    }

    #[test]
    fn retry_delay_is_jittered_but_bounded_by_config() {
        let config = NativeReconnectConfig {
            resume_deadline: Duration::from_secs(14),
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_millis(500),
            jitter_percent: 20,
            command_batch_limit: 8,
            max_attempts: 5,
        };
        for attempt in 0..8 {
            let delay = retry_delay_for(config, attempt, 7);
            assert!(delay >= Duration::from_millis(1));
            assert!(delay <= config.max_backoff);
        }
    }

    #[test]
    fn command_drain_coalesces_players_and_preserves_explicit_leave() {
        let (sender, receiver) = std::sync::mpsc::channel();
        for _ in 0..1000 {
            sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .expect("send");
        }
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Disconnect))
            .expect("send leave");
        let mut receiver = receiver;
        let batch = drain_command_batch(&mut receiver, 8);
        assert!(batch.len() <= 8);
        assert!(batch.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
        )));
        assert_eq!(
            batch
                .iter()
                .filter(|command| matches!(command, GatewayCommand::Player(_)))
                .count(),
            1
        );
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn production_command_queue_is_bounded_and_priority_leave_survives_full_normal_lane() {
        let (sender, mut receiver) = command_channel(8);
        for _ in 0..256 {
            sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .expect("full normal lane must remain nonblocking");
        }
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Disconnect))
            .expect("priority lane must retain disconnect");

        let batch = drain_command_batch(&mut receiver, 8);
        assert!(batch.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
        )));
        assert!(batch.len() <= 8);
    }

    #[test]
    fn game_shop_transaction_lane_survives_normal_saturation_and_delivers_exactly_once() {
        let (sender, mut receiver) = command_channel(8);
        for _ in 0..256 {
            sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .expect("ordinary saturation remains nonblocking");
        }
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
                request_id: "gs-1".to_owned(),
                g_index: 31,
                quantity: 2,
                price_type: 1,
            }))
            .expect("reserved transaction lane");
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Disconnect))
            .expect("exit priority lane remains independent");

        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(
            batch
                .iter()
                .filter(|command| matches!(
                    command,
                    GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy { request_id, .. })
                        if request_id == "gs-1"
                ))
                .count(),
            1
        );
        assert!(batch.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
        )));
        assert!(!drain_command_batch(&mut receiver, 8)
            .iter()
            .any(is_game_shop_transaction));
    }

    #[test]
    fn storage_transaction_lane_survives_normal_saturation_and_delivers_exactly_once() {
        let (sender, mut receiver) = command_channel(8);
        for _ in 0..256 {
            sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .expect("ordinary saturation remains nonblocking");
        }
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::StoreItem {
                request_id: "st-0000000000000001".to_owned(),
                from: 3,
                to: 9,
            }))
            .expect("reserved correlated transaction lane");

        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(
            batch
                .iter()
                .filter(|command| matches!(
                    command,
                    GatewayCommand::Wire(NativeOutboundCommand::StoreItem { request_id, .. })
                        if request_id == "st-0000000000000001"
                ))
                .count(),
            1
        );
        assert!(!drain_command_batch(&mut receiver, 8)
            .iter()
            .any(is_storage_transaction));
    }

    #[test]
    fn second_correlated_transaction_fails_closed_while_lane_is_occupied() {
        let (sender, mut receiver) = command_channel(8);
        let store = |request_id: &str| {
            GatewayCommand::Wire(NativeOutboundCommand::StoreItem {
                request_id: request_id.to_owned(),
                from: 3,
                to: 9,
            })
        };
        assert!(sender.send(store("st-0000000000000001")).is_ok());
        assert!(sender.send(store("st-0000000000000002")).is_err());
        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(
            batch
                .iter()
                .filter(|command| is_storage_transaction(command))
                .count(),
            1
        );
    }

    #[test]
    fn correlated_transaction_inserted_during_drain_waits_for_next_batch() {
        struct RefillAfterFirst<'a> {
            receiver: &'a mut GatewayCommandReceiver,
            sender: GatewayCommandSender,
            refilled: bool,
        }

        impl CommandSource for RefillAfterFirst<'_> {
            fn try_command(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError> {
                let command = self.receiver.try_recv();
                if !self.refilled
                    && command
                        .as_ref()
                        .is_ok_and(|command| is_correlated_transaction(command))
                {
                    self.refilled = true;
                    self.sender
                        .send(GatewayCommand::Wire(NativeOutboundCommand::TakeBackItem {
                            request_id: "st-0000000000000002".to_owned(),
                            from: 9,
                            to: 3,
                        }))
                        .expect("slot was atomically freed by the first take");
                }
                command
            }
        }

        let (sender, mut receiver) = command_channel(8);
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::StoreItem {
                request_id: "st-0000000000000001".to_owned(),
                from: 3,
                to: 9,
            }))
            .unwrap();
        let mut source = RefillAfterFirst {
            receiver: &mut receiver,
            sender,
            refilled: false,
        };

        let first = drain_command_batch(&mut source, 8);
        assert!(matches!(
            first.as_slice(),
            [GatewayCommand::Wire(NativeOutboundCommand::StoreItem { request_id, .. })]
                if request_id == "st-0000000000000001"
        ));
        drop(source);

        let second = drain_command_batch(&mut receiver, 8);
        assert!(matches!(
            second.as_slice(),
            [GatewayCommand::Wire(NativeOutboundCommand::TakeBackItem { request_id, .. })]
                if request_id == "st-0000000000000002"
        ));
        assert!(drain_command_batch(&mut receiver, 8).is_empty());
    }

    #[test]
    fn second_game_shop_transaction_fails_closed_while_lane_is_occupied() {
        let (sender, mut receiver) = command_channel(8);
        let purchase = |request_id: &str| {
            GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
                request_id: request_id.to_owned(),
                g_index: 31,
                quantity: 1,
                price_type: 1,
            })
        };
        assert!(sender.send(purchase("gs-1")).is_ok());
        assert!(sender.send(purchase("gs-2")).is_err());
        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(
            batch
                .iter()
                .filter(|command| is_game_shop_transaction(command))
                .count(),
            1
        );
        assert!(matches!(
            batch.first(),
            Some(GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
                request_id,
                ..
            })) if request_id == "gs-1"
        ));
    }

    fn purchase_command(request: &GameShopRequest) -> GatewayCommand {
        GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
            request_id: request.request_id.clone(),
            g_index: request.g_index,
            quantity: request.quantity,
            price_type: request.price_type,
        })
    }

    #[tokio::test]
    async fn prewrite_transaction_in_retry_wait_resets_once_and_is_not_replayed() {
        let request = GameShopRequest::new("gs-retry".to_owned(), 31, 1, 1).unwrap();
        let (sender, mut receiver) = command_channel(8);
        sender.send(purchase_command(&request)).unwrap();
        sender.send(GatewayCommand::Connect).unwrap();
        let mut gate = GameShopReceiptGate::default();
        let mut resets = 0;

        let outcome = wait_for_retry_or_leave_with_reset(
            &mut receiver,
            Duration::from_secs(1),
            8,
            &mut gate,
            None,
            || {
                resets += 1;
                true
            },
        )
        .await
        .unwrap();

        assert_eq!(outcome, RetryWait::Connect);
        assert_eq!(resets, 1);
        assert!(drain_command_batch(&mut receiver, 8)
            .iter()
            .all(|command| !is_game_shop_transaction(command)));
    }

    #[tokio::test]
    async fn prewrite_transaction_in_connect_wait_resets_once_and_is_not_replayed() {
        let request = GameShopRequest::new("gs-connect".to_owned(), 31, 1, 1).unwrap();
        let (sender, mut receiver) = command_channel(8);
        sender.send(purchase_command(&request)).unwrap();
        sender.send(GatewayCommand::Connect).unwrap();
        let mut gate = GameShopReceiptGate::default();
        let mut resets = 0;

        assert!(
            wait_for_connect_request_with_reset(&mut receiver, 8, &mut gate, || {
                resets += 1;
                true
            },)
            .await
            .unwrap()
        );

        assert_eq!(resets, 1);
        assert!(drain_command_batch(&mut receiver, 8)
            .iter()
            .all(|command| !is_game_shop_transaction(command)));
    }

    #[tokio::test]
    async fn prewrite_storage_in_retry_and_connect_wait_resets_and_never_replays() {
        let storage = || {
            GatewayCommand::Wire(NativeOutboundCommand::StoreItem {
                request_id: "st-0000000000000010".to_owned(),
                from: 3,
                to: 9,
            })
        };

        let (retry_sender, mut retry_receiver) = command_channel(8);
        retry_sender.send(storage()).unwrap();
        retry_sender.send(GatewayCommand::Connect).unwrap();
        let mut retry_gate = GameShopReceiptGate::default();
        let mut retry_resets = 0;
        let outcome = wait_for_retry_or_leave_with_reset(
            &mut retry_receiver,
            Duration::from_secs(1),
            8,
            &mut retry_gate,
            None,
            || {
                retry_resets += 1;
                true
            },
        )
        .await
        .unwrap();
        assert_eq!(outcome, RetryWait::Connect);
        assert_eq!(retry_resets, 1);
        assert!(drain_command_batch(&mut retry_receiver, 8).is_empty());

        let (connect_sender, mut connect_receiver) = command_channel(8);
        connect_sender.send(storage()).unwrap();
        connect_sender.send(GatewayCommand::Connect).unwrap();
        let mut connect_gate = GameShopReceiptGate::default();
        let mut connect_resets = 0;
        assert!(wait_for_connect_request_with_reset(
            &mut connect_receiver,
            8,
            &mut connect_gate,
            || {
                connect_resets += 1;
                true
            },
        )
        .await
        .unwrap());
        assert_eq!(connect_resets, 1);
        assert!(drain_command_batch(&mut connect_receiver, 8).is_empty());
    }

    #[test]
    fn prewrite_storage_in_resume_and_pre_normal_paths_resets_without_socket_write() {
        let storage = || {
            GatewayCommand::Wire(NativeOutboundCommand::TakeBackItem {
                request_id: "st-0000000000000011".to_owned(),
                from: 9,
                to: 3,
            })
        };

        let (sender, mut receiver) = command_channel(8);
        sender.send(storage()).unwrap();
        let mut gate = GameShopReceiptGate::default();
        let mut resume_resets = 0;
        assert!(matches!(
            drain_resume_lifecycle_commands_with_reset(&mut receiver, 8, &mut gate, || {
                resume_resets += 1;
                true
            },),
            ResumeLifecycle::Complete(())
        ));
        assert_eq!(resume_resets, 1);
        assert!(drain_command_batch(&mut receiver, 8).is_empty());

        let mut pre_normal_resets = 0;
        let mut socket_writes = 0;
        if discard_correlated_before_socket_write(&storage(), &mut gate, || {
            pre_normal_resets += 1;
            true
        }) {
            // This is exactly the branch used by the connected loop while its
            // phase is not Normal.
        } else {
            socket_writes += 1;
        }
        assert_eq!(pre_normal_resets, 1);
        assert_eq!(socket_writes, 0);
    }

    #[test]
    fn saturated_storage_patch_falls_back_to_non_evictable_reset_barrier() {
        let mut delivered = Vec::new();
        let mut resets = 0;
        assert!(push_storage_patch_or_reset(
            "{\"ack\":{\"operation\":\"deposit\"}}".to_owned(),
            |json| {
                delivered.push(json);
                false
            },
            || {
                resets += 1;
                true
            },
        ));
        assert_eq!(delivered.len(), 1);
        assert_eq!(resets, 1);

        assert!(push_storage_patch_or_reset(
            "{}".to_owned(),
            |_| true,
            || {
                resets += 1;
                true
            },
        ));
        assert_eq!(resets, 1, "successful receipt must not reset the session");
    }

    #[test]
    fn prewrite_transaction_frozen_awaiting_resume_marks_all_owners_unknown_once() {
        use mir2_client_bevy::crystal_ui::NativePlayerUiState;
        use mir2_client_bevy::game_shop::GameShopModel;
        use mir2_client_bevy::pending_operations::{PendingOperationKey, PendingOperations};

        let mut player_ui = NativePlayerUiState::default();
        let request = player_ui.core.begin_game_shop_purchase(31, 1, 1).unwrap();
        let mut model = GameShopModel::default();
        assert!(model.reserve_purchase(request.clone()));
        let key = PendingOperationKey::GameShop(request.request_id.clone());
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(key));

        let (sender, mut receiver) = command_channel(8);
        sender.send(purchase_command(&request)).unwrap();
        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(batch.len(), 1);
        let mut resets = 0;
        let mut socket_sends = 0;
        for command in batch {
            if discard_correlated_before_socket_write(
                &command,
                &mut GameShopReceiptGate::default(),
                || {
                    resets += 1;
                    player_ui.core.mark_game_shop_unknown();
                    model.mark_purchase_unknown();
                    pending.clear();
                    true
                },
            ) {
                continue;
            }
            socket_sends += 1;
        }

        assert_eq!(resets, 1);
        assert_eq!(socket_sends, 0);
        assert!(player_ui.core.game_shop_pending.is_none());
        assert!(player_ui.core.game_shop_unknown);
        assert!(model.pending_purchase.is_none());
        assert!(model.purchase_unknown);
        assert!(pending.is_empty());
        assert!(drain_command_batch(&mut receiver, 8).is_empty());
    }

    #[test]
    fn resumable_socket_malformed_receipt_terminates_written_purchase_once() {
        let mut resume = NativeResumeClientState::default();
        resume.record_credential(
            &"A".repeat(MAX_CREDENTIAL_LENGTH),
            Some(gateway_unix_ms() + 30_000),
            Some(1),
        );
        assert!(resume.has_live_credential());

        let request = GameShopRequest::new("gs-malformed".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut resets = 0;
        let result = process_connected_text_frame(
            r#"{"type":"gameShopReceipt","protocol":"nativeGameShopReceiptV1","requestId":"gs-malformed","success":true,"gIndex":31,"quantity":1,"priceType":1"#,
            &mut gate,
            |text, _| {
                parse_inbound_event(text)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            },
            || {
                resets += 1;
                true
            },
        );

        assert!(result.is_err());
        assert_eq!(resets, 1);
        assert!(gate.pending.is_none());
        assert!(gate.reserved.is_none());
    }

    #[test]
    fn resumable_socket_oversize_frame_terminates_before_parse_once() {
        let request = GameShopRequest::new("gs-oversize".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let oversized = "x".repeat(MAX_GATEWAY_FRAME_BYTES + 1);
        let mut resets = 0;
        let mut parsed = false;

        let result = process_connected_text_frame(
            &oversized,
            &mut gate,
            |_, _| {
                parsed = true;
                Ok(())
            },
            || {
                resets += 1;
                true
            },
        );

        assert!(result.is_err());
        assert!(!parsed);
        assert_eq!(resets, 1);
        assert!(gate.pending.is_none());
    }

    #[test]
    fn resumable_socket_read_error_then_disconnect_resets_written_purchase_exactly_once() {
        let request = GameShopRequest::new("gs-read".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut resets = 0;

        let first = finish_connected_socket(
            ConnectedSocketEnd::ReadError("connection reset".to_owned()),
            &mut gate,
            || {
                resets += 1;
                true
            },
        );
        let second = finish_connected_socket(ConnectedSocketEnd::Disconnected, &mut gate, || {
            resets += 1;
            true
        });

        assert!(matches!(first, ConnectedExit::Disconnected(Some(_))));
        assert_eq!(second, ConnectedExit::Disconnected(None));
        assert_eq!(resets, 1);
        assert!(gate.pending.is_none());
    }

    #[test]
    fn exact_receipt_before_protocol_error_is_never_changed_to_unknown() {
        use std::cell::Cell;

        let request = GameShopRequest::new("gs-exact-first".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let resets = Cell::new(0_u32);
        let exact = r#"{"type":"gameShopReceipt","protocol":"nativeGameShopReceiptV1","requestId":"gs-exact-first","success":true,"gIndex":31,"quantity":1,"priceType":1,"mailId":77}"#;

        assert!(process_connected_text_frame(
            exact,
            &mut gate,
            |text, gate| {
                let InboundEvent::GameShopReceipt(receipt) =
                    parse_inbound_event(text).map_err(|error| error.to_string())?
                else {
                    return Err("expected GameShopReceipt".to_owned());
                };
                correlate_and_deliver_game_shop_receipt(
                    gate,
                    &receipt,
                    |_| true,
                    || {
                        resets.set(resets.get() + 1);
                        true
                    },
                )
            },
            || {
                resets.set(resets.get() + 1);
                true
            },
        )
        .unwrap());
        assert!(gate.pending.is_none());
        assert!(gate.reserved.is_some());

        let later_error = process_connected_text_frame(
            "{malformed",
            &mut gate,
            |text, _| {
                parse_inbound_event(text)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            },
            || {
                resets.set(resets.get() + 1);
                true
            },
        );
        assert!(later_error.is_err());
        assert_eq!(resets.get(), 0);
        assert!(gate.reserved.is_some());
    }

    fn successful_game_shop_receipt(request_id: &str, g_index: i32) -> GameShopReceipt {
        GameShopReceipt {
            protocol: NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_owned(),
            request_id: request_id.to_owned(),
            success: true,
            g_index,
            quantity: 1,
            price_type: 1,
            new_stock_level: Some(9),
            mail_id: Some(77),
            code: None,
        }
    }

    #[test]
    fn exact_receipt_then_semantic_invalid_is_quarantined_and_consumed_once_by_full_plugins() {
        use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
        use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
        use mir2_client_bevy::pending_operations::{
            PendingOperationKey, PendingOperations, SessionResetGameShopPreservation,
            SessionResetRevision,
        };

        let (mut app, request) = seeded_terminal_boundary_app();
        let exact = terminal_game_shop_receipt(&request, true);
        let mut invalid = exact.clone();
        invalid.code = Some(mir2_client_bevy::game_shop::GameShopFailureCode::InsufficientCurrency);
        assert!(!invalid.is_valid());

        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request.clone()));
        assert!(correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &exact,
            mir2_bevy_runtime::native_ingest::push_native_game_shop_receipt,
            mir2_bevy_runtime::native_ingest::push_native_data_reset,
        )
        .unwrap());
        assert!(!correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &invalid,
            |_| panic!("reserved receipt must quarantine semantic-invalid payload"),
            || panic!("reserved receipt must not trigger DataReset"),
        )
        .unwrap());
        assert_eq!(gate.reserved.as_ref(), Some(&exact));

        let mut resume = NativeResumeClientState::default();
        assert!(apply_outer_terminal_transition(
            OuterTerminalTransition::NoCredentialDisconnect,
            &mut resume,
            &mut gate,
        ));
        app.world_mut().resource_mut::<NativeShellModel>().screen =
            NativeShellScreen::ConnectionLost;
        app.update();

        let model = app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>();
        assert_eq!(model.last_receipt.as_ref(), Some(&exact));
        assert!(!model.purchase_unknown);
        let ui = app.world().resource::<NativePlayerUiState>();
        assert_eq!(ui.core.game_shop_last_receipt.as_ref(), Some(&exact));
        assert!(!ui.core.game_shop_unknown);
        assert!(!app
            .world()
            .resource::<PendingOperations>()
            .contains(&PendingOperationKey::GameShop(request.request_id)));
        let revision = app.world().resource::<SessionResetRevision>().0;
        assert!(app
            .world()
            .resource::<SessionResetGameShopPreservation>()
            .receipt_for(revision)
            .is_none());

        app.update();
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::game_shop::GameShopModel>()
                .last_receipt
                .as_ref(),
            Some(&exact)
        );
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .core
                .game_shop_last_receipt
                .as_ref(),
            Some(&exact)
        );
    }

    #[test]
    fn exact_receipt_is_reserved_once_and_wrong_flood_cannot_overwrite_it() {
        let request = GameShopRequest::new("gs-exact".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut delivered = Vec::new();
        let mut reset_count = 0;

        assert!(correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &successful_game_shop_receipt("gs-exact", 31),
            |json| {
                delivered.push(json);
                true
            },
            || {
                reset_count += 1;
                true
            },
        )
        .unwrap());

        for index in 0..1_000 {
            assert!(!correlate_and_deliver_game_shop_receipt(
                &mut gate,
                &successful_game_shop_receipt(&format!("gs-wrong-{index}"), 99),
                |json| {
                    delivered.push(json);
                    true
                },
                || {
                    reset_count += 1;
                    true
                },
            )
            .unwrap());
        }

        assert_eq!(delivered.len(), 1);
        assert!(delivered[0].contains("\"requestId\":\"gs-exact\""));
        assert_eq!(
            reset_count, 0,
            "protected exact receipt must remain drainable"
        );
    }

    #[test]
    fn wrong_receipt_for_in_flight_purchase_resets_to_unknown() {
        let request = GameShopRequest::new("gs-exact".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut delivered = false;
        let mut reset = false;

        assert!(!correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &successful_game_shop_receipt("gs-wrong", 31),
            |_| {
                delivered = true;
                true
            },
            || {
                reset = true;
                true
            },
        )
        .unwrap());

        assert!(!delivered);
        assert!(reset);
        assert!(gate.pending.is_none());
        assert!(gate.reserved.is_none());
    }

    #[test]
    fn receipt_reserve_backpressure_is_terminal_unknown_not_acknowledgement() {
        let request = GameShopRequest::new("gs-exact".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut reset = false;

        assert!(!correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &successful_game_shop_receipt("gs-exact", 31),
            |_| false,
            || {
                reset = true;
                true
            },
        )
        .unwrap());

        assert!(reset);
        assert!(gate.pending.is_none());
        assert!(gate.reserved.is_none());
    }

    fn gateway_payload() -> Value {
        json!({
            "tick": 42,
            "mapTitle": "BichonProvince",
            "playerObjectId": 1001,
            "selectedObjectId": null,
            "playerHp": 50,
            "playerMaxHp": 100,
            "playerMp": 25,
            "playerMaxMp": 50,
            "playerExperience": 435,
            "playerMaxExperience": 900,
            "currentWeight": 1,
            "maxWeight": 50,
            "gold": 1234,
            "credit": 45,
            "sceneView": { "center": { "x": 9, "y": 7 }, "width": 19, "height": 15 },
            "terrainPatches": [ { "x": 0, "y": 0, "width": 40, "height": 40, "kind": "grass" } ],
            "decorObjects": [],
            "entities": [
                {
                    "objectId": 1001,
                    "kind": "selfPlayer",
                    "name": "Demo",
                    "x": 9,
                    "y": 7,
                    "direction": "up",
                    "level": 3,
                    "movementStartedAt": 1700000000000_f64,
                    "movementUntil": 1700000000600_f64
                },
                {
                    "objectId": 2001,
                    "kind": "monster",
                    "name": "Wolf",
                    "x": 11,
                    "y": 8,
                    "direction": "down"
                }
            ],
            "mineNodes": [ { "x": 3, "y": 4, "stage": 2 } ]
        })
    }

    #[test]
    fn transform_preserves_core_fields_and_converts_object_ids_to_strings() {
        let transformed = transform_world_snapshot(&gateway_payload());
        assert_eq!(transformed["mapTitle"], json!("BichonProvince"));
        assert_eq!(transformed["playerObjectId"], json!("1001"));
        assert_eq!(transformed["selectedObjectId"], Value::Null);

        let entities = transformed["entities"].as_array().expect("entities array");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0]["objectId"], json!("1001"));
        assert_eq!(entities[0]["kind"], json!("selfPlayer"));
        assert_eq!(entities[0]["x"], json!(9));
        assert_eq!(entities[1]["objectId"], json!("2001"));
        assert_eq!(entities[1]["kind"], json!("monster"));

        let mine_nodes = transformed["mineNodes"].as_array().expect("mine nodes");
        assert_eq!(mine_nodes[0]["stage"], json!(2));
    }

    #[test]
    fn transform_derives_movement_duration_from_until_minus_started() {
        let transformed = transform_world_snapshot(&gateway_payload());
        let entities = transformed["entities"].as_array().expect("entities array");
        assert_eq!(entities[0]["movementStartedMs"], json!(1700000000000_f64));
        assert_eq!(entities[0]["movementDurationMs"], json!(600_f64));
        // No timing metadata on the second entity.
        assert_eq!(entities[1]["movementStartedMs"], Value::Null);
        assert_eq!(entities[1]["movementDurationMs"], Value::Null);
    }

    #[test]
    fn transformed_snapshot_parses_into_the_runtime_world_snapshot_shape() {
        let transformed = transform_world_snapshot(&gateway_payload());
        let json = serde_json::to_string(&transformed).expect("serialize");
        // The runtime deserializes this exact camelCase shape; a parse error
        // here means the transform drifted from the runtime's WorldSnapshot.
        let parsed = serde_json::from_str::<serde_json::Value>(&json).expect("parse");
        assert!(parsed.get("entities").is_some());
    }

    #[test]
    fn transform_extracts_player_stats_for_the_hud() {
        let transformed = transform_world_snapshot(&gateway_payload());
        let stats = &transformed["playerStats"];
        assert_eq!(stats["hp"], json!(50));
        assert_eq!(stats["maxHp"], json!(100));
        assert_eq!(stats["mp"], json!(25));
        assert_eq!(stats["maxMp"], json!(50));
        assert_eq!(stats["gold"], json!(1234));
        assert_eq!(stats["credit"], json!(45));
        assert_eq!(stats["level"], json!(3));
        assert_eq!(stats["name"], json!("Demo"));
        assert_eq!(stats["mapName"], json!("BichonProvince"));
    }

    #[test]
    fn ui_read_model_transform_matches_the_shared_hud_shape() {
        let ui = transform_ui_read_model(&gateway_payload());
        assert_eq!(ui["player"]["hp"], json!(50));
        assert_eq!(ui["player"]["maxHp"], json!(100));
        assert_eq!(ui["player"]["gold"], json!(1234));
        assert_eq!(ui["player"]["credit"], json!(45));
        assert_eq!(ui["player"]["experience"], json!(435));
        assert_eq!(ui["player"]["maxExperience"], json!(900));
        assert_eq!(ui["player"]["currentWeight"], json!(1));
        assert_eq!(ui["player"]["maxWeight"], json!(50));
        assert_eq!(ui["player"]["name"], json!("Demo"));
        assert_eq!(ui["player"]["level"], json!(3));
        // Must deserialize as mir2-client-bevy UiReadModel.
        let model = serde_json::from_str::<mir2_client_bevy::read_model::UiReadModel>(
            &serde_json::to_string(&ui).expect("serialize"),
        )
        .expect("UiReadModel");
        assert_eq!(model.player.hp, 50);
        assert_eq!(model.player.max_hp, 100);
        assert_eq!(model.player.gold, 1234);
        assert_eq!(model.player.credit, 45);
        assert_eq!(model.player.experience, 435);
        assert_eq!(model.player.max_experience, 900);
        assert_eq!(model.player.current_weight, 1);
        assert_eq!(model.player.max_weight, 50);
        assert_eq!(model.player.experience_percent_label(), "48.33%");
        assert_eq!(model.player.available_weight(), 49);
        assert_eq!(model.player.name.as_deref(), Some("Demo"));
        assert!((model.player.normalized_hp() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn null_pre_bootstrap_stats_coalesce_to_typed_zeroes() {
        let payload = json!({
            "mapTitle": null,
            "playerHp": null,
            "playerMaxHp": null,
            "playerMp": null,
            "playerMaxMp": null,
            "playerExperience": null,
            "playerMaxExperience": null,
            "currentWeight": null,
            "maxWeight": null,
            "gold": null,
            "entities": []
        });

        let world = transform_world_snapshot(&payload);
        assert_eq!(world["playerStats"]["hp"], json!(0));
        assert_eq!(world["playerStats"]["gold"], json!(0));
        assert_eq!(world["playerStats"]["mapName"], Value::Null);

        let ui = transform_ui_read_model(&payload);
        let model = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(ui)
            .expect("null gateway scalars must normalize before UiReadModel decode");
        assert_eq!(model.player.hp, 0);
        assert_eq!(model.player.max_hp, 0);
        assert_eq!(model.player.gold, 0);
        assert_eq!(model.player.level, 0);
        assert_eq!(model.player.experience, 0);
        assert_eq!(model.player.max_experience, 0);
        assert_eq!(model.player.current_weight, 0);
        assert_eq!(model.player.max_weight, 0);
        assert_eq!(model.player.map_name, None);
    }

    #[test]
    fn native_ui_cursor_preserves_user_information_across_partial_snapshots() {
        let mut cursor = NativeUiPlayerCursor::default();
        cursor.observe_user_information(&json!({
            "name": "Alice",
            "class": "Wizard",
            "level": 7,
            "hp": 80,
            "maxHp": 100,
            "mp": 20,
            "maxMp": 40,
            "gold": 321,
            "credit": 12,
            "experience": 4,
            "maxExperience": 10,
            "currentWeight": 3,
            "maxWeight": 50,
            "mapTitle": "BichonProvince"
        }));

        cursor.observe_world_snapshot(&json!({
            "mapTitle": null,
            "playerHp": null,
            "playerMaxHp": null,
            "playerMp": null,
            "playerMaxMp": null,
            "entities": []
        }));

        let model = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(
            cursor.to_read_model_json(),
        )
        .expect("cursor read model");
        assert_eq!(model.player.name.as_deref(), Some("Alice"));
        assert_eq!(model.player.class_name.as_deref(), Some("Wizard"));
        assert_eq!(model.player.level, 7);
        assert_eq!((model.player.hp, model.player.max_hp), (80, 100));
        assert_eq!((model.player.mp, model.player.max_mp), (20, 40));
        assert_eq!(model.player.map_name.as_deref(), Some("BichonProvince"));
    }

    #[test]
    fn native_ui_cursor_applies_explicit_updates_and_map_changed_identity() {
        let mut cursor = NativeUiPlayerCursor::default();
        cursor.observe_user_information(&json!({
            "name": "Alice",
            "level": 7,
            "hp": 80,
            "maxHp": 100,
            "mapTitle": "BichonProvince"
        }));
        cursor.observe_world_snapshot(&json!({
            "playerObjectId": 99,
            "playerHp": 0,
            "entities": [{
                "objectId": 99,
                "kind": "player",
                "name": "Alice Renamed",
                "level": 8,
                "className": "Wizard"
            }]
        }));
        cursor.observe_map_identity(&json!({
            "fileName": "1",
            "title": "BorderVillage"
        }));

        let model = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(
            cursor.to_read_model_json(),
        )
        .expect("cursor read model");
        assert_eq!(
            model.player.hp, 0,
            "explicit death HP must remain authoritative"
        );
        assert_eq!(model.player.name.as_deref(), Some("Alice Renamed"));
        assert_eq!(model.player.level, 8);
        assert_eq!(model.player.map_name.as_deref(), Some("BorderVillage"));

        cursor.reset();
        let reset = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(
            cursor.to_read_model_json(),
        )
        .expect("reset cursor read model");
        assert_eq!(reset.player.name, None);
        assert_eq!(reset.player.hp, 0);
        assert_eq!(reset.player.map_name, None);
    }

    #[test]
    fn map_model_transform_matches_the_shared_map_shape() {
        let map = transform_map_model(&gateway_payload());
        assert_eq!(map["centerX"], json!(9));
        assert_eq!(map["centerY"], json!(7));
        let patches = map["patches"].as_array().expect("patches");
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0]["kind"], json!("grass"));
        assert_eq!(patches[0]["width"], json!(40));
        // Must deserialize as mir2-client-bevy MapModel.
        let model = serde_json::from_str::<mir2_client_bevy::map::MapModel>(
            &serde_json::to_string(&map).expect("serialize"),
        )
        .expect("MapModel");
        assert_eq!(model.center_x, 9);
        assert_eq!(model.center_y, 7);
        assert_eq!(model.patches.len(), 1);
    }

    #[test]
    fn entity_model_transform_matches_the_shared_entity_shape() {
        let entities = transform_entity_model_set(&gateway_payload());
        let list = entities["entities"].as_array().expect("entities");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["objectId"], json!("1001"));
        assert_eq!(list[0]["kind"], json!("selfPlayer"));
        assert_eq!(list[0]["x"], json!(9));
        assert_eq!(list[1]["objectId"], json!("2001"));
        assert_eq!(list[1]["kind"], json!("monster"));
        // Must deserialize as mir2-client-bevy EntityModelSet.
        let model = serde_json::from_str::<mir2_client_bevy::entities::EntityModelSet>(
            &serde_json::to_string(&entities).expect("serialize"),
        )
        .expect("EntityModelSet");
        assert_eq!(model.entities.len(), 2);
        assert_eq!(
            model.entities[0].kind,
            mir2_client_bevy::entities::EntityKind::SelfPlayer
        );
        assert_eq!(
            model.entities[1].kind,
            mir2_client_bevy::entities::EntityKind::Monster
        );
    }

    #[test]
    fn inventory_transform_groups_items_by_container() {
        let mut payload = gateway_payload();
        payload["inventoryItems"] = json!([
            { "key": "small-hp-drug", "uniqueId": 42, "name": "Red Potion", "quantity": 5, "slot": 0,
              "icon": 7, "description": "Restores HP", "durabilityCurrent": 4, "durabilityMax": 5,
              "sellValue": 12, "equipSlot": "Weapon", "grade": "Rare", "attack": 3, "defence": 2,
              "addedAttack": 1, "addedDefence": 4, "addedLuck": 2, "shape": 9, "socketSlots": 3 }
        ]);
        payload["beltItems"] = json!([
            { "key": "blue-potion", "uniqueId": 43, "name": "Blue Potion", "quantity": 2, "slot": 0 }
        ]);
        payload["equipmentItems"] = json!([
            { "key": "wooden-sword", "uniqueId": 44, "name": "Wooden Sword", "quantity": 1, "slot": 3 }
        ]);

        let inventory = transform_inventory_model(&payload);
        assert_eq!(inventory["gold"], json!(1234));
        let items = inventory["items"].as_array().expect("items");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["container"], json!(0));
        assert_eq!(items[0]["key"], json!("small-hp-drug"));
        assert_eq!(items[0]["uniqueId"], json!(42));
        assert_eq!(items[1]["container"], json!(1));
        assert_eq!(items[2]["container"], json!(2));
        assert_eq!(items[2]["name"], json!("Wooden Sword"));

        let model = serde_json::from_str::<mir2_client_bevy::inventory::InventoryModel>(
            &serde_json::to_string(&inventory).expect("serialize"),
        )
        .expect("InventoryModel");
        assert_eq!(model.gold, 1234);
        assert_eq!(model.items.len(), 3);
        assert_eq!(model.items[0].key, "small-hp-drug");
        assert_eq!(model.items[0].unique_id, Some(42));
        assert_eq!(model.items[0].icon, 7);
        assert_eq!(model.items[0].description, "Restores HP");
        assert_eq!(model.items[0].durability_current, Some(4));
        assert_eq!(model.items[0].durability_max, Some(5));
        assert_eq!(model.items[0].sell_value, 12);
        assert_eq!(model.items[0].equip_slot.as_deref(), Some("Weapon"));
        assert_eq!(model.items[0].added_defence, 4);
        assert_eq!(model.items[0].shape, Some(9));
        assert_eq!(model.items[0].socket_slots, 3);
    }

    #[test]
    fn real_template_key_keeps_explicit_instance_id_through_ui_intent_and_ack() {
        let payload = json!({
            "gold": 10,
            "inventoryItems": [{
                "key": "small-hp-drug",
                "uniqueId": 42,
                "name": "Small HP Drug",
                "quantity": 3,
                "slot": 0
            }],
            "beltItems": [],
            "equipmentItems": []
        });
        let model = serde_json::from_value::<mir2_client_bevy::inventory::InventoryModel>(
            transform_inventory_model(&payload),
        )
        .expect("real inventory shape");
        let item = &model.items[0];
        assert_eq!(item.key, "small-hp-drug");
        assert_eq!(
            mir2_client_bevy::crystal_ui::overlays::item_unique_id(item),
            Some(42)
        );

        let mut pending = mir2_client_bevy::pending_operations::PendingOperations::default();
        let mut queue =
            mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntentQueue::default();
        assert!(queue.push_pending_intent(
            &mut pending,
            mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntent::DropItem {
                key: item.key.clone(),
                unique_id: item.unique_id.expect("instance id"),
                count: 3,
                hero_inventory: false,
            },
        ));

        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = bevy::prelude::App::new();
        app.insert_resource(mir2_client_bevy::native_shell::NativeShellModel {
            screen: mir2_client_bevy::native_shell::NativeShellScreen::InGame,
            ..Default::default()
        })
        .init_resource::<mir2_client_bevy::quest_ui::QuestUiIntentQueue>()
        .insert_resource(queue)
        .insert_resource(crate::input::GatewayCommands::new(sender))
        .add_systems(
            bevy::prelude::Update,
            crate::gameplay_bridge::forward_quest_ui_intents,
        );
        app.update();
        assert!(matches!(
            receiver.try_recv().expect("drop command"),
            GatewayCommand::Wire(NativeOutboundCommand::DropItem {
                key,
                unique_id: 42,
                count: 3,
                hero_inventory: false,
            }) if key == "small-hp-drug"
        ));

        let ack = transform_inventory_operation_ack(
            "DropItem",
            &json!({
                "uniqueId": 42,
                "count": 3,
                "heroInventory": false,
                "success": true
            }),
        )
        .expect("correlatable ack");
        let mut feedback =
            mir2_client_bevy::pending_operations::InventoryOperationFeedback::default();
        assert_eq!(
            mir2_client_bevy::pending_operations::apply_inventory_operation_ack(
                &mut pending,
                &mut feedback,
                ack,
            ),
            1
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn inventory_transform_normalizes_named_equipment_slots_and_nulls() {
        let payload = json!({
            "gold": null,
            "inventoryItems": [
                { "key": null, "uniqueId": 41, "name": null, "quantity": null, "slot": "2" }
            ],
            "beltItems": [],
            "equipmentItems": [
                { "key": "sword", "name": "WoodenSword", "quantity": 1, "slot": "weapon" },
                { "key": "dress", "name": "BaseDress(M)", "quantity": 1, "slot": "armour" },
                { "key": "mystery", "name": "Mystery", "quantity": 1, "slot": "future-slot" }
            ]
        });

        let inventory = transform_inventory_model(&payload);
        let model =
            serde_json::from_value::<mir2_client_bevy::inventory::InventoryModel>(inventory)
                .expect("named equipment slots must normalize before InventoryModel decode");
        assert_eq!(model.gold, 0);
        assert_eq!(model.items[0].key, "41");
        assert_eq!(model.items[0].unique_id, Some(41));
        assert_eq!(model.items[0].name, "");
        assert_eq!(model.items[0].quantity, 1);
        assert_eq!(model.items[0].slot, 2);
        assert_eq!(model.items[1].slot, 0);
        assert_eq!(model.items[2].slot, 1);
        assert_eq!(model.items[3].slot, 2);
    }

    #[test]
    fn object_chat_line_transform_extracts_text_and_channel() {
        let payload = json!({ "objectId": 1001, "text": "hello world", "chatType": "Normal" });
        let chat = transform_chat_line("ObjectChat", &payload).expect("chat line");
        assert_eq!(chat.text, "hello world");
        assert_eq!(chat.channel, "Normal");

        let missing = json!({ "objectId": 1001 });
        assert!(transform_chat_line("ObjectChat", &missing).is_none());
    }

    #[test]
    fn direct_chat_line_transform_preserves_system_message_and_channel() {
        let payload = json!({
            "message": "server.CannotPickupNotOwner",
            "chatType": "System"
        });
        let chat = transform_chat_line("Chat", &payload).expect("direct chat line");
        assert_eq!(chat.text, "server.CannotPickupNotOwner");
        assert_eq!(chat.channel, "System");

        assert!(transform_chat_line("Chat", &json!({ "text": "wrong field" })).is_none());
        assert!(transform_chat_line("NPCSay", &payload).is_none());
    }

    async fn assert_cancelled_resume_closes_without_replay(
        socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    ) {
        loop {
            let frame = match timeout(Duration::from_secs(2), socket.next())
                .await
                .expect("client must terminate the cancelled resume socket")
            {
                Some(Ok(frame)) => frame,
                // Terminal recovery drops the socket without awaiting a peer
                // Close handshake; EOF or reset are both valid observations.
                Some(Err(_)) | None => return,
            };
            match frame {
                Message::Close(_) => return,
                Message::Ping(payload) => {
                    socket
                        .send(Message::Pong(payload))
                        .await
                        .expect("loopback pong");
                }
                Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => {}
                Message::Text(text) => {
                    let value: Value =
                        serde_json::from_str(text.as_ref()).expect("client wire JSON");
                    if value.get("type").and_then(Value::as_str) != Some("keepAlive") {
                        panic!(
                            "client replayed or restarted a command after explicit resume cancellation: {text}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn actual_server_skill_fixture_keeps_authoritative_spell_and_optional_mp_cost() {
        let payload = json!({
            "tick": 300,
            "playerMp": 41,
            "playerMaxMp": 90,
            "knownSkills": [
                {
                    "key": "mystery",
                    "name": "Localized display only",
                    "castKind": "TARGET"
                },
                {
                    "key": "fireball",
                    "name": "Localized Fireball",
                    "spell": "FireBall",
                    "castKind": "target",
                    "offensive": true,
                    "hotkey": 2,
                    "level": 3,
                    "delayMs": 1200,
                    "castTimeMs": 250,
                    "cooldownRemainingTicks": 0,
                    "mpCost": 7
                },
                {
                    "key": "shield",
                    "name": "Localized Shield",
                    "spell": "Shield",
                    "castKind": "self",
                    "offensive": false,
                    "hotkey": 1,
                    "cooldownRemainingTicks": 2
                }
            ]
        });
        let model = serde_json::from_value::<mir2_client_bevy::skill_model::SkillModel>(
            transform_skill_model(&payload),
        )
        .expect("actual server skill fixture");

        let f1 = model.selection_for_shortcut(1).expect("F1 selected Shield");
        assert_eq!(f1.spell.as_deref(), Some("Shield"));
        assert_eq!(f1.cast_kind.as_deref(), Some("self"));
        assert_eq!(f1.cooldown_remaining_ticks, 2);
        assert_eq!(f1.mp_cost, None);

        let f2 = model
            .selection_for_shortcut(2)
            .expect("F2 selected FireBall");
        assert_eq!(f2.spell.as_deref(), Some("FireBall"));
        assert_eq!(f2.mp_cost, Some(7));
        assert_eq!(f2.cooldown_remaining_ticks, 0);

        // The display-only entry remains learned, but never acquires a
        // protocol spell or an invented MP cost.
        let display_only = model
            .selection_for_shortcut(3)
            .expect("F3 selected display-only skill");
        assert_eq!(display_only.spell, None);
        assert_eq!(display_only.mp_cost, None);
    }

    #[test]
    fn skill_packets_win_over_stale_snapshot_and_fresh_tick_retires_patch() {
        let mut cursor = SkillPacketCursor::default();
        let mut initial = json!({
            "tick": 100,
            "playerObjectId": 1001,
            "playerMp": 60,
            "playerMaxMp": 100,
            "knownSkills": [{
                "key": "fireball",
                "name": "Fireball",
                "spell": "FireBall",
                "castKind": "target",
                "hotkey": 1,
                "cooldownRemainingTicks": 0
            }]
        });
        cursor.observe_snapshot(&mut initial);
        assert!(cursor.apply_packet(
            "MagicDelay",
            &json!({"objectId":1001,"spell":"FireBall","delay":12}),
            100
        ));
        assert!(cursor.apply_packet(
            "UserInformation",
            &json!({"objectId":1001,"mp":40,"maxMp":100}),
            100
        ));

        let mut stale = json!({
            "tick": 99,
            "playerObjectId": 1001,
            "playerMp": 60,
            "playerMaxMp": 100,
            "knownSkills": [{
                "key": "fireball",
                "name": "Fireball",
                "spell": "FireBall",
                "castKind": "target",
                "hotkey": 1,
                "cooldownRemainingTicks": 0
            }]
        });
        cursor.observe_snapshot(&mut stale);
        assert_eq!(stale["playerMp"], json!(40));
        assert_eq!(stale["knownSkills"][0]["cooldownRemainingTicks"], json!(12));

        let mut fresh = json!({
            "tick": 101,
            "playerObjectId": 1001,
            "playerMp": 35,
            "playerMaxMp": 100,
            "knownSkills": [{
                "key": "fireball",
                "name": "Fireball",
                "spell": "FireBall",
                "castKind": "target",
                "hotkey": 1,
                "cooldownRemainingTicks": 9
            }]
        });
        cursor.observe_snapshot(&mut fresh);
        assert_eq!(fresh["playerMp"], json!(35));
        assert_eq!(fresh["knownSkills"][0]["cooldownRemainingTicks"], json!(9));
        assert!(cursor.patches.is_empty());
        assert!(cursor.vitals.is_none());
    }

    #[test]
    fn tickless_patch_removal_and_vitals_apply_to_next_snapshot_then_retire() {
        let mut cursor = SkillPacketCursor::default();
        for cooldown in [0, 1] {
            let mut prior = json!({
                "tick": 0,
                "playerObjectId": 1001,
                "knownSkills": [{
                    "spell": "FireBall",
                    "hotkey": 1,
                    "cooldownRemainingTicks": cooldown
                }]
            });
            cursor.observe_snapshot(&mut prior);
        }
        assert_eq!(cursor.snapshot_serial, 2);
        assert!(cursor.apply_packet(
            "MagicDelay",
            &json!({"objectId":1001,"spell":"FireBall","delay":12}),
            0
        ));
        assert_eq!(
            cursor.patches[0].zero_tick_expires_at_snapshot_serial,
            Some(3)
        );
        let mut next = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "knownSkills": [{
                "spell": "FireBall",
                "hotkey": 1,
                "cooldownRemainingTicks": 3
            }]
        });
        cursor.observe_snapshot(&mut next);
        assert_eq!(next["knownSkills"][0]["cooldownRemainingTicks"], json!(12));
        assert!(cursor.patches.is_empty());
        let mut later = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "knownSkills": [{
                "spell": "FireBall",
                "hotkey": 1,
                "cooldownRemainingTicks": 4
            }]
        });
        cursor.observe_snapshot(&mut later);
        assert_eq!(later["knownSkills"][0]["cooldownRemainingTicks"], json!(4));

        let mut removal_cursor = SkillPacketCursor::default();
        for _ in 0..2 {
            let mut prior = json!({
                "tick": 0,
                "knownSkills": [
                    {"spell":"FireBall","hotkey":1},
                    {"spell":"Lightning","hotkey":2}
                ]
            });
            removal_cursor.observe_snapshot(&mut prior);
        }
        assert!(removal_cursor.apply_packet("RemoveMagic", &json!({"placeId":1}), 0));
        assert_eq!(
            removal_cursor.removals[0].zero_tick_expires_at_snapshot_serial,
            Some(3)
        );
        let mut next = json!({
            "tick": 0,
            "knownSkills": [
                {"spell":"FireBall","hotkey":1},
                {"spell":"Lightning","hotkey":2}
            ]
        });
        removal_cursor.observe_snapshot(&mut next);
        assert_eq!(next["knownSkills"].as_array().unwrap().len(), 1);
        assert_eq!(next["knownSkills"][0]["spell"], json!("Lightning"));
        assert!(removal_cursor.removals.is_empty());
        let mut later = json!({
            "tick": 0,
            "knownSkills": [
                {"spell":"FireBall","hotkey":1},
                {"spell":"Lightning","hotkey":2}
            ]
        });
        removal_cursor.observe_snapshot(&mut later);
        assert_eq!(later["knownSkills"].as_array().unwrap().len(), 2);

        let mut vitals_cursor = SkillPacketCursor::default();
        for mp in [60, 55] {
            let mut prior = json!({
                "tick": 0,
                "playerObjectId": 1001,
                "playerMp": mp,
                "playerMaxMp": 100
            });
            vitals_cursor.observe_snapshot(&mut prior);
        }
        assert!(vitals_cursor.apply_packet(
            "UserInformation",
            &json!({"objectId":1001,"mp":40,"maxMp":100}),
            0
        ));
        assert_eq!(
            vitals_cursor
                .vitals
                .unwrap()
                .zero_tick_expires_at_snapshot_serial,
            Some(3)
        );
        let mut next = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "playerMp": 60,
            "playerMaxMp": 100
        });
        vitals_cursor.observe_snapshot(&mut next);
        assert_eq!(next["playerMp"], json!(40));
        assert!(vitals_cursor.vitals.is_none());
        let mut later = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "playerMp": 55,
            "playerMaxMp": 100
        });
        vitals_cursor.observe_snapshot(&mut later);
        assert_eq!(later["playerMp"], json!(55));
    }

    #[test]
    fn spell_toggle_packet_updates_can_use_without_retyping_or_cross_object_pollution() {
        let mut cursor = SkillPacketCursor::default();
        let mut initial = json!({
            "tick": 100,
            "playerObjectId": 1001,
            "knownSkills": [{
                "id": 8,
                "spell": "FlamingSword",
                "castKind": "toggle",
                "canUse": false,
                "hotkey": 1
            }]
        });
        cursor.observe_snapshot(&mut initial);

        assert!(!cursor.apply_packet(
            "SpellToggle",
            &json!({"objectId":2002,"spell":"FlamingSword","canUse":true}),
            100
        ));
        assert!(cursor.patches.is_empty());

        assert!(cursor.apply_packet(
            "SpellToggle",
            &json!({"objectId":1001,"spell":"FlamingSword","canUse":true}),
            100
        ));
        let mut enabled = initial.clone();
        cursor.observe_snapshot(&mut enabled);
        assert_eq!(enabled["knownSkills"][0]["castKind"], json!("toggle"));
        assert_eq!(enabled["knownSkills"][0]["canUse"], json!(true));
        let enabled_model = serde_json::from_value::<mir2_client_bevy::skill_model::SkillModel>(
            transform_skill_model(&enabled),
        )
        .expect("enabled toggle model");
        let enabled_selection = enabled_model.selection_for_shortcut(1).unwrap();
        assert_eq!(enabled_selection.cast_kind.as_deref(), Some("toggle"));
        assert_eq!(enabled_selection.can_use, Some(true));

        assert!(cursor.apply_packet(
            "SpellToggle",
            &json!({"objectId":1001,"spell":"FlamingSword","canUse":false}),
            100
        ));
        let mut disabled = enabled.clone();
        cursor.observe_snapshot(&mut disabled);
        assert_eq!(disabled["knownSkills"][0]["castKind"], json!("toggle"));
        assert_eq!(disabled["knownSkills"][0]["canUse"], json!(false));
        let disabled_model = serde_json::from_value::<mir2_client_bevy::skill_model::SkillModel>(
            transform_skill_model(&disabled),
        )
        .expect("disabled toggle model");
        let disabled_selection = disabled_model.selection_for_shortcut(1).unwrap();
        assert_eq!(disabled_selection.cast_kind.as_deref(), Some("toggle"));
        assert_eq!(disabled_selection.can_use, Some(false));
    }

    #[test]
    fn personal_skill_deltas_reject_other_or_missing_object_ids() {
        let mut cursor = SkillPacketCursor::default();
        let mut initial = json!({
            "tick": 100,
            "playerObjectId": 1001,
            "knownSkills": [{
                "id": 8,
                "spell": "FireBall",
                "castKind": "target",
                "level": 1,
                "cooldownRemainingTicks": 0,
                "hotkey": 1
            }]
        });
        cursor.observe_snapshot(&mut initial);

        for payload in [
            json!({"objectId":0,"spell":"FireBall","delay":12}),
            json!({"objectId":2002,"spell":"FireBall","delay":12}),
            json!({"spell":"FireBall","delay":12}),
        ] {
            assert!(!cursor.apply_packet("MagicDelay", &payload, 100));
        }
        for payload in [
            json!({"objectId":0,"spell":"FireBall","level":2,"experience":7}),
            json!({"objectId":2002,"spell":"FireBall","level":2,"experience":7}),
            json!({"spell":"FireBall","level":2,"experience":7}),
        ] {
            assert!(!cursor.apply_packet("MagicLeveled", &payload, 100));
        }
        for payload in [
            json!({"objectId":0,"spell":"FireBall","canUse":true}),
            json!({"objectId":2002,"spell":"FireBall","canUse":true}),
            json!({"spell":"FireBall","canUse":true}),
        ] {
            assert!(!cursor.apply_packet("SpellToggle", &payload, 100));
        }
        assert!(cursor.patches.is_empty());

        assert!(cursor.apply_packet(
            "MagicDelay",
            &json!({"objectId":1001,"spell":"FireBall","delay":12}),
            100
        ));
        assert!(cursor.apply_packet(
            "MagicLeveled",
            &json!({"objectId":1001,"spell":"FireBall","level":2,"experience":7}),
            100
        ));
        assert!(cursor.apply_packet(
            "SpellToggle",
            &json!({"objectId":1001,"spell":"FireBall","canUse":true}),
            100
        ));
        let mut patched = initial;
        cursor.observe_snapshot(&mut patched);
        assert_eq!(patched["knownSkills"][0]["cooldownRemainingTicks"], 12);
        assert_eq!(patched["knownSkills"][0]["level"], 2);
        assert_eq!(patched["knownSkills"][0]["experience"], 7);
    }

    type LoopbackSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

    async fn loopback_receive_json(socket: &mut LoopbackSocket) -> Value {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await
                .expect("loopback server frame timeout")
                .expect("loopback client closed before sending the expected frame")
                .expect("loopback websocket read failed");
            match frame {
                Message::Text(text) => {
                    let value: Value =
                        serde_json::from_str(text.as_ref()).expect("valid client JSON");
                    if value.get("type").and_then(Value::as_str) == Some("keepAlive") {
                        continue;
                    }
                    return value;
                }
                Message::Ping(payload) => {
                    socket
                        .send(Message::Pong(payload))
                        .await
                        .expect("loopback pong");
                }
                Message::Close(_) => panic!("loopback client closed unexpectedly"),
                Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }

    fn loopback_world_snapshot(name: &str, x: i32, y: i32, level: u64) -> Value {
        json!({
            "type": "worldSnapshot",
            "payload": {
                "mapFileName": "loopback-test-map",
                "mapTitle": "Loopback Test Map",
                "playerObjectId": 1000,
                "playerHp": 18,
                "playerMaxHp": 20,
                "playerMp": 9,
                "playerMaxMp": 12,
                "gold": 321,
                "credit": 7,
                "sceneView": {"center": {"x": x, "y": y}},
                "terrainPatches": [],
                "entities": [{
                    "objectId": 1000,
                    "kind": "selfPlayer",
                    "name": name,
                    "class": "Wizard",
                    "gender": "Female",
                    "level": level,
                    "x": x,
                    "y": y,
                    "direction": "Down"
                }],
                "inventoryItems": [],
                "beltItems": [],
                "equipmentItems": [],
                "knownSkills": [],
                "stage5Systems": {"mail": []}
            }
        })
    }

    fn loopback_snapshot_self_name(snapshot: &NativeGameplaySnapshot) -> Option<&str> {
        snapshot
            .entity_render_payload
            .as_ref()?
            .get("entities")?
            .as_array()?
            .iter()
            .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))?
            .get("name")?
            .as_str()
    }

    #[tokio::test]
    async fn native_resume_round_trip_preserves_only_post_resume_authority() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind loopback websocket listener");
        let url = format!(
            "ws://{}/ws",
            listener.local_addr().expect("loopback listener address")
        );
        let credential = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let (progress_sender, mut progress_receiver) =
            tokio::sync::mpsc::unbounded_channel::<&'static str>();
        let (allow_stale_sender, allow_stale_receiver) = oneshot::channel();

        let mut server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept initial socket");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("upgrade initial socket");

            let capabilities = loopback_receive_json(&mut socket).await;
            assert_eq!(capabilities["type"], json!("clientCapabilities"));
            assert!(capabilities["capabilities"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value == NATIVE_RESUME_PROTOCOL)));
            socket
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": credential,
                        "expiresAtMs": gateway_unix_ms() + 60_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send initial resume credential");
            progress_sender
                .send("initial-handshake-ready")
                .expect("initial progress receiver");

            let start_game = loop {
                let frame = loopback_receive_json(&mut socket).await;
                if frame["type"] != json!("keepAlive") {
                    break frame;
                }
            };
            assert_eq!(start_game["type"], json!("startGame"));
            assert_eq!(start_game["characterIndex"], json!(3));
            socket
                .send(Message::Text(
                    loopback_world_snapshot("initial-authority", 10, 20, 3)
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send initial authoritative snapshot");
            progress_sender
                .send("initial-snapshot-sent")
                .expect("initial progress receiver");

            let first_player_command = loopback_receive_json(&mut socket).await;
            assert_eq!(first_player_command["type"], json!("walk"));
            assert_eq!(first_player_command["direction"], json!("up"));
            socket
                .send(Message::Close(None))
                .await
                .expect("close initial socket");
            progress_sender
                .send("first-socket-closed")
                .expect("first close progress receiver");

            let (stream, _) = listener.accept().await.expect("accept resume socket");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("upgrade resume socket");
            let capabilities = loopback_receive_json(&mut socket).await;
            assert_eq!(capabilities["type"], json!("clientCapabilities"));
            let resume = loopback_receive_json(&mut socket).await;
            assert_eq!(resume["type"], json!("resumeSession"));
            assert_eq!(resume["credential"], json!(credential));
            progress_sender
                .send("resume-handshake-received")
                .expect("resume progress receiver");
            allow_stale_receiver
                .await
                .expect("test must queue stale input after resume handshake");

            // The client is AwaitingResume here. A player intent queued after
            // the transport loss must be consumed and ignored, never written
            // to the new socket. A short read timeout is the wire-level proof.
            assert_no_player_command_while_awaiting_resume(&mut socket).await;

            socket
                .send(Message::Text(
                    loopback_world_snapshot("stale-pre-resume", 99, 99, 99)
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send quarantined pre-resume snapshot");
            socket
                .send(Message::Text(
                    json!({
                        "type": "sessionResumed",
                        "characterIndex": 3,
                        "generation": 2
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send session resumed");
            socket
                .send(Message::Text(
                    loopback_world_snapshot("post-resume-authority", 30, 40, 8)
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send post-resume authoritative snapshot");
            socket
                .send(Message::Text(
                    json!({
                        "type": "packet",
                        "packet": "ObjectProjectile",
                        "payload": {
                            "objectId": 2001,
                            "spell": "FireBall",
                            "location": {"x": 30, "y": 40},
                            "targetLocation": {"x": 31, "y": 40}
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send post-resume generation event");
            progress_sender
                .send("post-resume-frames-sent")
                .expect("post-resume progress receiver");

            loop {
                let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
                    .await
                    .expect("wait for client shutdown");
                match frame {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        socket.send(Message::Pong(payload)).await.expect("pong");
                    }
                    Some(Ok(_)) => {}
                    // Client terminal branches intentionally drop instead of
                    // awaiting a peer-dependent Close handshake.
                    Some(Err(_)) => break,
                }
            }
        });

        let (command_sender, command_receiver) = std::sync::mpsc::channel();
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = url;
        let mut client_task = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig {
                    resume_deadline: Duration::from_secs(2),
                    initial_backoff: Duration::from_millis(10),
                    max_backoff: Duration::from_millis(20),
                    jitter_percent: 0,
                    command_batch_limit: 16,
                    max_attempts: 3,
                },
                |_| true,
            )
            .await
        });

        let result = tokio::time::timeout(Duration::from_secs(5), async {
            assert_eq!(
                progress_receiver.recv().await,
                Some("initial-handshake-ready")
            );
            command_sender
                .send(GatewayCommand::Wire(NativeOutboundCommand::StartGame {
                    character_index: 3,
                }))
                .expect("send start game");
            assert_eq!(
                progress_receiver.recv().await,
                Some("initial-snapshot-sent")
            );
            command_sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .expect("send first player command");
            assert_eq!(progress_receiver.recv().await, Some("first-socket-closed"));

            assert_eq!(
                progress_receiver.recv().await,
                Some("resume-handshake-received")
            );
            // This command is deliberately queued after the resume request but
            // before sessionResumed. The server's second-socket assertion proves
            // it is consumed locally and never replayed.
            command_sender
                .send(GatewayCommand::Player(PlayerIntent::Run {
                    direction: "right".to_owned(),
                }))
                .expect("send stale player command");
            allow_stale_sender
                .send(())
                .expect("resume server must still await stale-input permission");
            assert_eq!(
                progress_receiver.recv().await,
                Some("post-resume-frames-sent")
            );

            let snapshots = tokio::time::timeout(Duration::from_secs(2), async {
                let mut snapshots = Vec::new();
                while snapshots.len() < 3 {
                    while let Ok(snapshot) = gameplay_receiver.try_recv() {
                        snapshots.push(snapshot);
                    }
                    if snapshots.len() < 3 {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
                snapshots
            })
            .await
            .expect("receive initial, resumed, and packet-first snapshots");

            assert_eq!(
                snapshots
                    .iter()
                    .filter_map(loopback_snapshot_self_name)
                    .collect::<Vec<_>>(),
                vec![
                    "initial-authority",
                    "post-resume-authority",
                    "post-resume-authority"
                ]
            );
            assert!(!snapshots.iter().any(|snapshot| {
                loopback_snapshot_self_name(snapshot) == Some("stale-pre-resume")
            }));
            let effect = snapshots
                .last()
                .and_then(|snapshot| snapshot.effect_events.first())
                .expect("post-resume ObjectProjectile effect");
            assert_eq!(effect.generation, 2);
            assert_eq!(effect.packet, "ObjectProjectile");
            assert_eq!(effect.sequence, 1);

            let bootstraps = shell_receiver
                .try_iter()
                .filter_map(|event| match event {
                    ShellGatewayEvent::PlayerBootstrapped { character } => Some(character),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(bootstraps.len(), 2);
            assert_eq!(bootstraps[0].index, 3);
            assert_eq!(bootstraps[0].name, "initial-authority");
            assert_eq!(bootstraps[1].index, 3);
            assert_eq!(bootstraps[1].name, "post-resume-authority");
            assert_eq!(bootstraps[1].level, 8);
            assert_eq!(bootstraps[1].class_name, "Wizard");

            command_sender
                .send(GatewayCommand::Shutdown)
                .expect("shutdown native transport test client");
            assert_eq!(
                (&mut client_task).await.expect("client task join").unwrap(),
                ()
            );
            (&mut server_task).await.expect("server task join");
        })
        .await;

        if result.is_err() {
            client_task.abort();
            server_task.abort();
            panic!("native resume loopback test timed out");
        }
    }

    #[test]
    fn malformed_skill_packets_and_name_only_matches_fail_closed() {
        let mut cursor = SkillPacketCursor::default();
        assert!(!cursor.apply_packet("MagicDelay", &json!({"spell":"FireBall"}), 0));
        assert!(!cursor.apply_packet(
            "MagicLeveled",
            &json!({"spell":"FireBall","level":"bad"}),
            0
        ));
        assert!(!cursor.apply_packet(
            "SpellToggle",
            &json!({"spell":"FireBall","canUse":"yes"}),
            0
        ));
        assert!(!cursor.apply_packet("SpellToggle", &json!({"spell":"FireBall","canUse":true}), 0));
        assert!(!cursor.apply_packet("RemoveMagic", &json!({"placeId":0}), 0));
        assert!(!cursor.apply_packet("RemoveMagic", &json!({"placeId":9}), 0));
        assert!(!cursor.apply_packet("UserInformation", &json!({}), 0));
        assert!(cursor.patches.is_empty());
        assert!(cursor.removals.is_empty());
        assert!(cursor.vitals.is_none());

        // Personal skill deltas require the authoritative player object id;
        // establish it before testing the name-only snapshot rejection.
        cursor.player_object_id = Some(1001);
        let mut snapshot = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "knownSkills": [{
                "name": "FireBall",
                "key": "fireball",
                "cooldownRemainingTicks": 4
            }]
        });
        assert!(cursor.apply_packet(
            "MagicDelay",
            &json!({"objectId":1001,"spell":"FireBall","delay":12}),
            0
        ));
        cursor.observe_snapshot(&mut snapshot);
        assert_eq!(
            snapshot["knownSkills"][0]["cooldownRemainingTicks"],
            json!(4)
        );
        assert!(cursor.patches.is_empty());
    }

    #[test]
    fn map_change_is_scene_only_while_true_session_boundaries_clear_personal_models() {
        assert_eq!(
            packet_native_reset_scope("MapChanged"),
            Some(NativeResetScope::Scene)
        );
        for packet in ["LogOutSuccess", "ReturnToLogin", "Disconnect"] {
            assert_eq!(
                packet_native_reset_scope(packet),
                Some(NativeResetScope::Session),
                "{packet} must remain a true session reset"
            );
        }
        assert_eq!(packet_native_reset_scope("UserLocation"), None);
    }

    #[test]
    fn typed_big_map_packets_forward_an_immediate_model_only_snapshot() {
        use crate::native_protocol::{MapIdentity, SearchMapResult};

        assert!(packet_updates_big_map(&PacketEvent::MapChanged(
            MapIdentity {
                map_index: 1,
                location: None,
            }
        )));
        assert!(packet_updates_big_map(&PacketEvent::SearchMapResult(
            SearchMapResult {
                map_index: -1,
                npc_index: 0,
            },
        )));
        assert!(!packet_updates_big_map(&PacketEvent::Other {
            packet: "ObjectMonster".into(),
            payload: json!({}),
        }));
    }

    #[test]
    fn inventory_ack_transform_requires_complete_correlatable_fields() {
        assert_eq!(
            transform_inventory_operation_ack(
                "DropItem",
                &json!({
                    "uniqueId": 7001,
                    "count": 3,
                    "heroInventory": false,
                    "success": false
                })
            ),
            Some(InventoryOperationAck::Drop {
                unique_id: 7001,
                count: 3,
                hero_inventory: false,
                success: false,
            })
        );
        assert_eq!(
            transform_inventory_operation_ack(
                "MoveItem",
                &json!({"grid":"Inventory","from":4,"to":9,"success":true})
            ),
            Some(InventoryOperationAck::Move {
                grid: "Inventory".into(),
                from: 4,
                to: 9,
                success: true,
            })
        );
        assert_eq!(
            transform_inventory_operation_ack(
                "MergeItem",
                &json!({
                    "gridFrom":"Inventory",
                    "gridTo":"Inventory",
                    "idFrom":1,
                    "idTo":2,
                    "success":true
                })
            ),
            Some(InventoryOperationAck::Merge {
                grid_from: "Inventory".into(),
                grid_to: "Inventory".into(),
                id_from: 1,
                id_to: 2,
                success: true,
            })
        );
        assert_eq!(
            transform_inventory_operation_ack(
                "SplitItem1",
                &json!({"grid":"Inventory","uniqueId":3,"count":2,"success":true})
            ),
            Some(InventoryOperationAck::Split {
                grid: "Inventory".into(),
                unique_id: 3,
                count: 2,
                success: true,
            })
        );
        assert_eq!(
            transform_inventory_operation_ack(
                "SellItem",
                &json!({"uniqueId":88,"count":2,"success":false})
            ),
            Some(InventoryOperationAck::Sell {
                unique_id: 88,
                count: 2,
                success: false,
            })
        );
        assert!(transform_inventory_operation_ack(
            "SplitItem",
            &json!({"grid":"Inventory","item":{"uniqueId":4}})
        )
        .is_none());
        assert!(transform_inventory_operation_ack(
            "MoveItem",
            &json!({"grid":"Inventory","from":4,"to":9})
        )
        .is_none());
    }

    #[test]
    fn game_shop_packet_transforms_keep_cash_catalog_and_stock_patch_separate() {
        let info = transform_game_shop_info_from_packet(&json!({
            "item": {
                "item_index": 1200,
                "g_index": 42,
                "info": {"index": 1200, "name": "Cash Potion", "item_type": 3, "image": 77},
                "gold_price": 100,
                "credit_price": 5,
                "count": 2,
                "class": "All",
                "category": "Potion",
                "stock": 10,
                "deal": true,
                "top_item": false,
                "date_binary_datetime": 0,
                "can_buy_credit": true,
                "can_buy_gold": true
            },
            "stockLevel": 8
        }))
        .expect("GameShopInfo");
        let entry = serde_json::from_value::<mir2_client_bevy::game_shop::GameShopEntry>(info)
            .expect("cash entry shape");
        assert_eq!(entry.game_shop_index, 42);
        assert_eq!(entry.item_name, "Cash Potion");
        assert_eq!(entry.stock_level, 8);
        assert_eq!(entry.image, 77);

        let stock = transform_game_shop_stock_from_packet(&json!({
            "g_index": 42,
            "stock_level": 3
        }))
        .expect("GameShopStock");
        let patch =
            serde_json::from_value::<mir2_client_bevy::game_shop::GameShopStockPatch>(stock)
                .expect("cash stock patch shape");
        assert_eq!(patch.game_shop_index, 42);
        assert_eq!(patch.stock_level, 3);
    }

    #[test]
    fn wallet_delta_packets_update_absolute_shared_wallet_cursor() {
        let mut wallet = Some(WalletState {
            gold: 100,
            credit: 20,
        });
        let mut world = Some(json!({"gold":100,"credit":20}));
        assert_eq!(
            apply_wallet_delta(&mut wallet, &mut world, "gold", Some(7), true),
            Some(107)
        );
        assert_eq!(
            apply_wallet_delta(&mut wallet, &mut world, "credit", Some(5), false),
            Some(15)
        );
        assert_eq!(world.as_ref().unwrap()["gold"], json!(107));
        assert_eq!(world.as_ref().unwrap()["credit"], json!(15));
    }

    #[test]
    fn wallet_cursor_overlays_stale_user_information_before_hud_transform() {
        let wallet = Some(WalletState {
            gold: 107,
            credit: 15,
        });
        let mut user_information = json!({
            "hp": 50,
            "maxHp": 100,
            "mp": 25,
            "maxMp": 50,
            "gold": 100,
            "credit": 20,
            "class": "Wizard"
        });
        merge_wallet_into_payload(&mut user_information, wallet);
        let hud = transform_ui_read_model_from_user_information(&user_information);
        assert_eq!(hud["player"]["gold"], json!(107));
        assert_eq!(hud["player"]["credit"], json!(15));
    }

    #[test]
    fn user_information_immediately_populates_gold_and_credit_read_model() {
        let model = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(
            transform_ui_read_model_from_user_information(&json!({
                "name":"Alice",
                "class":"Wizard",
                "level":7,
                "hp":80,
                "maxHp":100,
                "mp":20,
                "maxMp":40,
                "gold":321,
                "credit":12,
                "experience":4,
                "maxExperience":10
            })),
        )
        .expect("UserInformation read model");
        assert_eq!(model.player.gold, 321);
        assert_eq!(model.player.credit, 12);
        assert_eq!(model.player.name.as_deref(), Some("Alice"));
    }

    #[test]
    fn shell_dispatch_uses_real_login_roster_but_waits_for_applied_world_snapshot() {
        let context = GatewaySessionContext {
            account_id: Some("player-one".to_owned()),
            character_index: Some(3),
        };
        let (sender, receiver) = std::sync::mpsc::channel();
        let login = parse_inbound_event(
            r#"{"type":"packet","packet":"LoginSuccess","payload":{"characters":[{"index":3,"name":"Alice","level":7,"class":"Warrior","gender":"Female"}]}}"#,
        )
        .expect("login event");

        dispatch_shell_event(&login, &context, &sender);
        match receiver.try_recv().expect("shell login event") {
            ShellGatewayEvent::LoginSuccess {
                account,
                characters,
            } => {
                assert_eq!(account, "player-one");
                assert_eq!(characters.len(), 1);
                assert_eq!(characters[0].index, 3);
                assert_eq!(characters[0].class_name, "Warrior");
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let user = parse_inbound_event(
            r#"{"type":"packet","packet":"UserInformation","payload":{"objectId":99,"name":"Alice","level":7,"class":"Warrior","gender":"Female"}}"#,
        )
        .expect("user event");
        dispatch_shell_event(&user, &context, &sender);
        assert!(
            receiver.try_recv().is_err(),
            "UserInformation must not enter InGame before runtime accepts a world snapshot"
        );
    }

    #[test]
    fn initial_bootstrap_waits_until_world_snapshot_ingest_is_applied() {
        let context = GatewaySessionContext {
            account_id: Some("player-one".to_owned()),
            character_index: Some(3),
        };
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, _gameplay_receiver) = std::sync::mpsc::channel();
        let mut snapshot_log_counter = 0;
        let mut gameplay_adapter = NativeGameplayAdapter::default();
        let mut last_world_payload = None;
        let mut last_wallet = None;
        let mut ui_cursor = NativeUiPlayerCursor::default();
        let mut in_flight_claim_mail_id = None;
        let mut send_mail_in_flight = false;
        let mut pending_mail_feedback = VecDeque::new();
        let mut skill_cursor = SkillPacketCursor::default();
        let mut social_cursor = SocialModel::default();
        let mut phase = ConnectionPhase::Normal;
        let mut resume_state = NativeResumeClientState::default();
        let mut resume_scene_reset_sent = false;
        let mut connection_bootstrap_sent = false;
        let mut game_shop_receipt_gate = GameShopReceiptGate::default();
        let runtime_accepts = std::cell::Cell::new(false);
        let mut push_world_state = |_: String| runtime_accepts.get();
        let snapshot = loopback_world_snapshot("initial-authority", 30, 40, 8).to_string();

        assert_eq!(
            handle_gateway_text_for_connection(
                &snapshot,
                &mut snapshot_log_counter,
                &context,
                &shell_sender,
                &mut gameplay_adapter,
                &gameplay_sender,
                &mut last_world_payload,
                &mut last_wallet,
                &mut ui_cursor,
                &mut in_flight_claim_mail_id,
                &mut send_mail_in_flight,
                &mut pending_mail_feedback,
                &mut skill_cursor,
                &mut social_cursor,
                &mut phase,
                &mut resume_state,
                &mut resume_scene_reset_sent,
                &mut connection_bootstrap_sent,
                &mut game_shop_receipt_gate,
                &mut push_world_state,
            )
            .expect("backpressured opening snapshot"),
            InboundDisposition::Quarantined
        );
        assert!(!connection_bootstrap_sent);
        assert!(shell_receiver.try_recv().is_err());

        runtime_accepts.set(true);
        handle_gateway_text_for_connection(
            &snapshot,
            &mut snapshot_log_counter,
            &context,
            &shell_sender,
            &mut gameplay_adapter,
            &gameplay_sender,
            &mut last_world_payload,
            &mut last_wallet,
            &mut ui_cursor,
            &mut in_flight_claim_mail_id,
            &mut send_mail_in_flight,
            &mut pending_mail_feedback,
            &mut skill_cursor,
            &mut social_cursor,
            &mut phase,
            &mut resume_state,
            &mut resume_scene_reset_sent,
            &mut connection_bootstrap_sent,
            &mut game_shop_receipt_gate,
            &mut push_world_state,
        )
        .expect("accepted opening snapshot");

        assert!(connection_bootstrap_sent);
        match shell_receiver.try_recv().expect("bootstrap after Applied") {
            ShellGatewayEvent::PlayerBootstrapped { character } => {
                assert_eq!(character.index, 3);
                assert_eq!(character.name, "initial-authority");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn shell_dispatch_requires_start_game_result_four() {
        let context = GatewaySessionContext::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        for (result, accepted) in [(4, true), (2, false)] {
            let event = parse_inbound_event(&format!(
                r#"{{"type":"packet","packet":"StartGame","payload":{{"result":{result}}}}}"#
            ))
            .expect("start event");
            dispatch_shell_event(&event, &context, &sender);
            match receiver.try_recv().expect("start ack") {
                ShellGatewayEvent::StartGameAck {
                    accepted: actual, ..
                } => assert_eq!(actual, accepted),
                other => panic!("unexpected event: {other:?}"),
            }
        }
    }

    #[test]
    fn shell_dispatch_maps_change_password_success_and_failure_without_credentials() {
        let context = GatewaySessionContext::default();
        let (sender, receiver) = std::sync::mpsc::channel();

        for result in [6, 2] {
            let event = parse_inbound_event(&format!(
                r#"{{"type":"packet","packet":"ChangePassword","payload":{{"result":{result}}}}}"#
            ))
            .expect("change-password event");
            dispatch_shell_event(&event, &context, &sender);
            assert_eq!(
                receiver.try_recv().expect("shell change-password event"),
                ShellGatewayEvent::ChangePasswordResult { result }
            );
        }

        let missing_result =
            parse_inbound_event(r#"{"type":"packet","packet":"ChangePassword","payload":{}}"#)
                .expect("missing-result change-password event");
        dispatch_shell_event(&missing_result, &context, &sender);
        assert_eq!(
            receiver.try_recv().expect("missing-result shell event"),
            ShellGatewayEvent::ChangePasswordResult { result: -1 }
        );
    }

    #[test]
    fn shell_dispatch_maps_change_password_banned_reason_and_expiry_without_credentials() {
        let context = GatewaySessionContext::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        let event = parse_inbound_event(
            r#"{"type":"packet","packet":"ChangePasswordBanned","payload":{"reason":"manual review","expiryDate":"2030-01-01T00:00:00Z"}}"#,
        )
        .expect("banned change-password event");
        dispatch_shell_event(&event, &context, &sender);
        assert_eq!(
            receiver.try_recv().expect("banned shell event"),
            ShellGatewayEvent::ChangePasswordBanned {
                reason: "manual review".to_owned(),
                expiry: Some("2030-01-01T00:00:00Z".to_owned()),
            }
        );

        let empty = parse_inbound_event(
            r#"{"type":"packet","packet":"ChangePasswordBanned","payload":{}}"#,
        )
        .expect("empty banned change-password event");
        dispatch_shell_event(&empty, &context, &sender);
        assert_eq!(
            receiver.try_recv().expect("default banned shell event"),
            ShellGatewayEvent::ChangePasswordBanned {
                reason: "account is banned".to_owned(),
                expiry: None,
            }
        );
    }

    fn terminal_game_shop_receipt(request: &GameShopRequest, success: bool) -> GameShopReceipt {
        GameShopReceipt {
            protocol: NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_owned(),
            request_id: request.request_id.clone(),
            success,
            g_index: request.g_index,
            quantity: request.quantity,
            price_type: request.price_type,
            new_stock_level: success.then_some(7),
            mail_id: success.then_some(77),
            code: (!success)
                .then_some(mir2_client_bevy::game_shop::GameShopFailureCode::InsufficientCurrency),
        }
    }

    fn seeded_terminal_boundary_app() -> (bevy::prelude::App, GameShopRequest) {
        use bevy::prelude::*;
        use mir2_client_bevy::crystal_ui::overlays::{
            Mir2CrystalOverlayPlugin, NativePlayerUiState,
        };
        use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
        use mir2_client_bevy::pending_operations::{PendingOperationKey, PendingOperations};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<bevy::audio::AudioSource>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Messages<bevy::input::keyboard::KeyboardInput>>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin)
            .add_plugins(Mir2CrystalOverlayPlugin);

        let request = app
            .world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .begin_game_shop_purchase(31, 2, 1)
            .expect("UI reserves purchase");
        assert!(app
            .world_mut()
            .resource_mut::<mir2_client_bevy::game_shop::GameShopModel>()
            .reserve_purchase(request.clone()));
        assert!(app
            .world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(PendingOperationKey::GameShop(request.request_id.clone())));
        app.world_mut()
            .resource_mut::<mir2_client_bevy::inventory::InventoryModel>()
            .gold = 999;
        app.world_mut()
            .resource_mut::<mir2_client_bevy::read_model::UiReadModel>()
            .player
            .gold = 999;
        // Initialize the shell boundary tracker while the character is active.
        app.update();
        (app, request)
    }

    #[test]
    fn outer_terminal_transitions_preserve_and_apply_exact_receipt_to_all_owners() {
        use mir2_client_bevy::crystal_ui::overlays::{
            NativePlayerUiIntentQueue, NativePlayerUiState,
        };
        use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
        use mir2_client_bevy::pending_operations::{
            PendingOperationKey, PendingOperations, SessionResetRevision,
        };

        #[derive(Clone, Copy)]
        enum Scenario {
            ResumeRejected,
            NoCredentialDisconnect,
            ReadError,
            RetryExhausted,
        }
        let scenarios = [
            Scenario::ResumeRejected,
            Scenario::NoCredentialDisconnect,
            Scenario::ReadError,
            Scenario::RetryExhausted,
        ];
        for scenario in scenarios {
            for success in [true, false] {
                let (mut app, request) = seeded_terminal_boundary_app();
                let receipt = terminal_game_shop_receipt(&request, success);
                let mut gate = GameShopReceiptGate::default();
                assert!(gate.record_successful_send(request.clone()));
                assert!(correlate_and_deliver_game_shop_receipt(
                    &mut gate,
                    &receipt,
                    mir2_bevy_runtime::native_ingest::push_native_game_shop_receipt,
                    mir2_bevy_runtime::native_ingest::push_native_data_reset,
                )
                .unwrap());
                assert!(gate.reserved.is_some());

                let transition = match scenario {
                    Scenario::ResumeRejected => {
                        let parse_error = process_connected_text_frame(
                            "{malformed",
                            &mut gate,
                            |text, _| {
                                parse_inbound_event(text)
                                    .map(|_| ())
                                    .map_err(|error| error.to_string())
                            },
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        );
                        assert!(parse_error.is_err());
                        OuterTerminalTransition::ResumeRejected
                    }
                    Scenario::NoCredentialDisconnect => {
                        let _ = finish_connected_socket(
                            ConnectedSocketEnd::Disconnected,
                            &mut gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        );
                        OuterTerminalTransition::NoCredentialDisconnect
                    }
                    Scenario::ReadError => {
                        let _ = finish_connected_socket(
                            ConnectedSocketEnd::ReadError("transport lost".to_owned()),
                            &mut gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        );
                        OuterTerminalTransition::NoCredentialDisconnect
                    }
                    Scenario::RetryExhausted => OuterTerminalTransition::RetryExhausted,
                };
                assert!(
                    gate.reserved.is_some(),
                    "inner error must keep exact receipt"
                );

                let mut resume = NativeResumeClientState::default();
                resume.record_credential(
                    &"A".repeat(MAX_CREDENTIAL_LENGTH),
                    Some(gateway_unix_ms() + 30_000),
                    Some(1),
                );
                assert!(apply_outer_terminal_transition(
                    transition,
                    &mut resume,
                    &mut gate,
                ));
                assert!(!resume.has_live_credential());
                assert!(gate.pending.is_none() && gate.reserved.is_none());

                app.world_mut().resource_mut::<NativeShellModel>().screen =
                    NativeShellScreen::ConnectionLost;
                app.update();

                let model = app
                    .world()
                    .resource::<mir2_client_bevy::game_shop::GameShopModel>();
                assert!(model.pending_purchase.is_none());
                assert_eq!(model.last_receipt.as_ref(), Some(&receipt));
                assert!(!model.purchase_unknown);
                let ui = app.world().resource::<NativePlayerUiState>();
                assert!(ui.core.game_shop_pending.is_none());
                assert_eq!(ui.core.game_shop_last_receipt.as_ref(), Some(&receipt));
                assert!(!ui.core.game_shop_unknown);
                assert!(!app
                    .world()
                    .resource::<PendingOperations>()
                    .contains(&PendingOperationKey::GameShop(request.request_id.clone())));
                assert_eq!(
                    app.world().resource::<SessionResetRevision>().0,
                    1,
                    "shell boundary must not issue a second reset"
                );
                assert_eq!(
                    app.world()
                        .resource::<mir2_client_bevy::inventory::InventoryModel>()
                        .gold,
                    0,
                    "other account/session models must be cleared"
                );
                assert_eq!(
                    app.world()
                        .resource::<mir2_client_bevy::read_model::UiReadModel>()
                        .player
                        .gold,
                    0
                );
                assert!(app
                    .world_mut()
                    .resource_mut::<NativePlayerUiIntentQueue>()
                    .drain_intents()
                    .is_empty());
                assert!(app
                    .world()
                    .resource::<mir2_client_bevy::pending_operations::SessionResetGameShopPreservation>()
                    .receipt_for(app.world().resource::<SessionResetRevision>().0)
                    .is_none());

                // A later ordinary account boundary is not covered by the
                // one-revision exception and clears the old account result.
                assert!(mir2_bevy_runtime::native_ingest::push_native_data_reset());
                app.update();
                assert!(app
                    .world()
                    .resource::<mir2_client_bevy::game_shop::GameShopModel>()
                    .last_receipt
                    .is_none());
                assert!(app
                    .world()
                    .resource::<NativePlayerUiState>()
                    .core
                    .game_shop_last_receipt
                    .is_none());
            }
        }
    }

    #[test]
    fn outer_terminal_pending_without_receipt_still_becomes_unknown() {
        use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
        use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
        use mir2_client_bevy::pending_operations::{PendingOperationKey, PendingOperations};

        let (mut app, request) = seeded_terminal_boundary_app();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request.clone()));
        let mut resume = NativeResumeClientState::default();
        assert!(apply_outer_terminal_transition(
            OuterTerminalTransition::NoCredentialDisconnect,
            &mut resume,
            &mut gate,
        ));
        app.world_mut().resource_mut::<NativeShellModel>().screen =
            NativeShellScreen::ConnectionLost;
        app.update();

        let model = app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>();
        assert!(model.pending_purchase.is_none());
        assert!(model.last_receipt.is_none());
        assert!(model.purchase_unknown);
        let ui = app.world().resource::<NativePlayerUiState>();
        assert!(ui.core.game_shop_pending.is_none());
        assert!(ui.core.game_shop_last_receipt.is_none());
        assert!(ui.core.game_shop_unknown);
        assert!(!app
            .world()
            .resource::<PendingOperations>()
            .contains(&PendingOperationKey::GameShop(request.request_id)));
    }

    #[test]
    fn shell_dispatch_maps_new_account_result_codes() {
        let context = GatewaySessionContext::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        for (result, created) in [(8, true), (7, false)] {
            let event = parse_inbound_event(&format!(
                r#"{{"type":"packet","packet":"NewAccount","payload":{{"result":{result}}}}}"#
            ))
            .expect("new-account event");
            dispatch_shell_event(&event, &context, &sender);
            let actual = receiver.try_recv().expect("shell account event");
            assert_eq!(matches!(actual, ShellGatewayEvent::AccountCreated), created);
        }
    }

    #[tokio::test]
    async fn native_resume_minimal_socket_contract_reconnects_and_accepts_post_resume_snapshot() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let credential = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let (initial_ready_tx, initial_ready_rx) = oneshot::channel();
        let (resume_pending_tx, resume_pending_rx) = oneshot::channel();
        let (allow_resume_tx, allow_resume_rx) = oneshot::channel();
        let (resumed_control_tx, resumed_control_rx) = oneshot::channel();
        let (allow_snapshot_tx, allow_snapshot_rx) = oneshot::channel();
        let (backpressured_snapshot_tx, backpressured_snapshot_rx) = oneshot::channel();
        let (allow_authoritative_snapshot_tx, allow_authoritative_snapshot_rx) = oneshot::channel();
        let (fresh_input_tx, fresh_input_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let server_credential = credential.clone();

        let server = tokio::spawn(async move {
            let (first_tcp, _) = listener.accept().await.expect("first client connection");
            let mut first = accept_async(first_tcp)
                .await
                .expect("first websocket handshake");
            let capabilities = receive_wire_type(&mut first, "clientCapabilities").await;
            assert!(capabilities["capabilities"]
                .as_array()
                .expect("capability array")
                .iter()
                .any(|value| value == NATIVE_RESUME_PROTOCOL));
            first
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": server_credential,
                        "expiresAtMs": gateway_unix_ms() + 30_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send resume credential");
            initial_ready_tx
                .send(())
                .expect("test must wait for the initial credential");
            let first_walk = receive_wire_type(&mut first, "walk").await;
            assert_eq!(first_walk["direction"], json!("up"));
            let _ = first.close(None).await;
            drop(first);

            let (second_tcp, _) = listener
                .accept()
                .await
                .expect("reconnect client connection");
            let mut second = accept_async(second_tcp)
                .await
                .expect("reconnect websocket handshake");
            let capabilities = receive_wire_type(&mut second, "clientCapabilities").await;
            assert!(capabilities["capabilities"]
                .as_array()
                .expect("capability array")
                .iter()
                .any(|value| value == NATIVE_RESUME_PROTOCOL));
            let resume = receive_wire_type(&mut second, "resumeSession").await;
            assert_eq!(resume["credential"], json!(credential));
            resume_pending_tx
                .send(())
                .expect("test must queue the stale command while awaiting resume");
            allow_resume_rx
                .await
                .expect("test must allow the resume decision");
            assert_no_player_command_while_awaiting_resume(&mut second).await;

            second
                .send(Message::Text(
                    json!({
                        "type": "sessionResumed",
                        "characterIndex": 7,
                        "generation": 2
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send session resumed");
            resumed_control_tx
                .send(())
                .expect("test must inject input after sessionResumed");
            allow_snapshot_rx
                .await
                .expect("test must allow the delayed authoritative snapshot");
            assert_no_player_command_while_awaiting_resume(&mut second).await;
            let mut snapshot = gateway_payload();
            snapshot["tick"] = json!(99);
            snapshot["entities"][0]["name"] = json!("ResumedHero");
            snapshot["entities"][0]["x"] = json!(42);
            second
                .send(Message::Text(
                    json!({ "type": "worldSnapshot", "payload": snapshot })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send backpressured post-resume world snapshot");
            backpressured_snapshot_tx
                .send(())
                .expect("test must inject input after failed world ingest");
            allow_authoritative_snapshot_rx
                .await
                .expect("test must allow the next authoritative snapshot");
            assert_no_player_command_while_awaiting_resume(&mut second).await;
            second
                .send(Message::Text(
                    json!({ "type": "worldSnapshot", "payload": snapshot })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send accepted post-resume world snapshot");
            second
                .send(Message::Text(
                    json!({
                        "type": "packet",
                        "packet": "ObjectMagic",
                        "payload": {
                            "objectId": 1001,
                            "spell": "FireBall",
                            "x": 42,
                            "y": 7
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send generation-bound packet");
            let fresh_turn = receive_wire_type(&mut second, "turn").await;
            assert_eq!(fresh_turn["direction"], json!("left"));
            fresh_input_tx
                .send(())
                .expect("test must observe fresh post-snapshot input");
            let _ = finish_rx.await;
            let _ = second.close(None).await;
        });

        let (command_sender, command_receiver) = command_channel(8);
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = format!("ws://{address}");
        let client = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig::default(),
                {
                    let mut snapshot_ingest_count = 0_u8;
                    move |_| {
                        snapshot_ingest_count = snapshot_ingest_count.saturating_add(1);
                        // This minimal fixture has no initial snapshot. The
                        // first resumed snapshot is deliberately backpressured;
                        // only the next resumed snapshot opens the input fence.
                        snapshot_ingest_count != 1
                    }
                },
            )
            .await
        });

        timeout(Duration::from_secs(2), initial_ready_rx)
            .await
            .expect("client must receive the initial credential")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }))
            .expect("initial input must be accepted");
        timeout(Duration::from_secs(2), resume_pending_rx)
            .await
            .expect("client must initiate resume")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Run {
                direction: "right".to_owned(),
            }))
            .expect("stale input must be accepted into the bounded queue");
        sleep(Duration::from_millis(60)).await;
        allow_resume_tx
            .send(())
            .expect("loopback server must still await resume permission");
        timeout(Duration::from_secs(2), resumed_control_rx)
            .await
            .expect("server must enter the sessionResumed-to-snapshot window")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Turn {
                direction: "right".to_owned(),
            }))
            .expect("post-sessionResumed input enters the bounded queue");
        command_sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Chat {
                message: "must-not-cross-resume-fence".to_owned(),
            }))
            .expect("post-sessionResumed business input enters the bounded queue");
        sleep(Duration::from_millis(60)).await;
        allow_snapshot_tx
            .send(())
            .expect("loopback server must still hold the snapshot");
        timeout(Duration::from_secs(2), backpressured_snapshot_rx)
            .await
            .expect("server must send the intentionally backpressured snapshot")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Run {
                direction: "down".to_owned(),
            }))
            .expect("fresh input after a failed ingest enters the bounded queue");
        command_sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Chat {
                message: "must-not-cross-backpressured-snapshot".to_owned(),
            }))
            .expect("fresh business input after failed ingest enters the bounded queue");
        sleep(Duration::from_millis(60)).await;
        allow_authoritative_snapshot_tx
            .send(())
            .expect("server must still hold the accepted authoritative snapshot");

        let mut saw_connected = false;
        let resumed_character = timeout(Duration::from_secs(3), async {
            loop {
                match shell_receiver.try_recv() {
                    Ok(ShellGatewayEvent::Connected) => saw_connected = true,
                    Ok(ShellGatewayEvent::Disconnect { reason }) => {
                        panic!("transient native resume must not emit Disconnect: {reason:?}")
                    }
                    Ok(ShellGatewayEvent::PlayerBootstrapped { character }) => break character,
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("native shell event channel closed before resume completed")
                    }
                }
            }
        })
        .await
        .expect("resume bootstrap must arrive before the test deadline");
        assert!(saw_connected, "only the first connection emits Connected");
        assert_eq!(resumed_character.index, 7);
        assert_eq!(resumed_character.name, "ResumedHero");

        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Turn {
                direction: "left".to_owned(),
            }))
            .expect("fresh post-snapshot input must be accepted");
        timeout(Duration::from_secs(2), fresh_input_rx)
            .await
            .expect("fresh input must cross the post-snapshot wire fence")
            .expect("loopback server must remain running");

        let resumed_effect = timeout(Duration::from_secs(3), async {
            loop {
                match gameplay_receiver.try_recv() {
                    Ok(snapshot) => {
                        if let Some(effect) = snapshot.effect_events.first() {
                            break effect.clone();
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("gameplay event channel closed before generation-bound packet")
                    }
                }
            }
        })
        .await
        .expect("post-resume generation-bound packet must arrive");
        assert_eq!(resumed_effect.generation, 2);
        assert_eq!(resumed_effect.packet, "ObjectMagic");

        command_sender
            .send(GatewayCommand::Shutdown)
            .expect("shutdown must be accepted");
        finish_tx
            .send(())
            .expect("loopback server must still own the resumed socket");
        timeout(Duration::from_secs(3), client)
            .await
            .expect("native client must shut down")
            .expect("native client task must not panic")
            .expect("native client must exit cleanly");
        timeout(Duration::from_secs(3), server)
            .await
            .expect("loopback server must shut down")
            .expect("loopback server task must not panic");
    }

    #[tokio::test]
    async fn native_resume_deadline_covers_waiting_for_resume_result_without_hanging() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let credential = "D".repeat(MAX_CREDENTIAL_LENGTH);
        let (initial_ready_tx, initial_ready_rx) = oneshot::channel();
        let (resume_pending_tx, resume_pending_rx) = oneshot::channel();
        let server_credential = credential.clone();

        let server = tokio::spawn(async move {
            let (first_tcp, _) = listener.accept().await.expect("first client connection");
            let mut first = accept_async(first_tcp)
                .await
                .expect("first websocket handshake");
            let _ = receive_wire_type(&mut first, "clientCapabilities").await;
            first
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": server_credential,
                        "expiresAtMs": gateway_unix_ms() + 30_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send resume credential");
            initial_ready_tx
                .send(())
                .expect("test must release initial input");
            let _ = receive_wire_type(&mut first, "walk").await;
            let _ = first.close(None).await;
            drop(first);

            let (second_tcp, _) = listener
                .accept()
                .await
                .expect("reconnect client connection");
            let mut second = accept_async(second_tcp)
                .await
                .expect("reconnect websocket handshake");
            let _ = receive_wire_type(&mut second, "clientCapabilities").await;
            let resume = receive_wire_type(&mut second, "resumeSession").await;
            assert_eq!(resume["credential"], json!(credential));
            resume_pending_tx
                .send(())
                .expect("test must observe the pending resume");

            // Intentionally neither send nor read anything after the resume
            // request. The peer's receive side remains stalled, proving the
            // client's terminal deadline does not await a Close write before
            // it reaches DataReset/Shell disconnect.
            sleep(Duration::from_millis(360)).await;
            assert!(
                timeout(Duration::from_millis(180), listener.accept())
                    .await
                    .is_err(),
                "deadline must not start a third reconnect attempt"
            );
        });

        let (command_sender, command_receiver) = command_channel(8);
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, _gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = format!("ws://{address}");
        let client = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig {
                    resume_deadline: Duration::from_millis(180),
                    initial_backoff: Duration::from_millis(5),
                    max_backoff: Duration::from_millis(10),
                    jitter_percent: 0,
                    command_batch_limit: 16,
                    max_attempts: 3,
                },
                |_| true,
            )
            .await
        });

        timeout(Duration::from_secs(2), initial_ready_rx)
            .await
            .expect("client must receive the initial credential")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }))
            .expect("initial input must be accepted");
        timeout(Duration::from_secs(2), resume_pending_rx)
            .await
            .expect("client must initiate resume")
            .expect("loopback server must remain running");

        let disconnect_reason = timeout(Duration::from_secs(2), async {
            loop {
                match shell_receiver.try_recv() {
                    Ok(ShellGatewayEvent::Disconnect { reason }) => break reason,
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("native shell event channel closed before deadline terminal state")
                    }
                }
            }
        })
        .await
        .expect("resume deadline must terminate the pending connection");
        assert_eq!(
            disconnect_reason.as_deref(),
            Some("gateway reconnect deadline expired")
        );

        command_sender
            .send(GatewayCommand::Shutdown)
            .expect("shutdown must be accepted after terminal deadline");
        timeout(Duration::from_secs(3), client)
            .await
            .expect("native client must shut down")
            .expect("native client task must not panic")
            .expect("native client must exit cleanly");
        timeout(Duration::from_secs(3), server)
            .await
            .expect("loopback server must shut down")
            .expect("loopback server task must not panic");
    }

    #[tokio::test]
    async fn native_resume_connect_handshake_can_be_cancelled_before_a_websocket_response() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let (accepted_tx, accepted_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.expect("accept stalled TCP socket");
            accepted_tx
                .send(())
                .expect("test must observe the pending websocket handshake");
            let _ = release_rx.await;
        });

        let (command_sender, mut command_receiver) = std::sync::mpsc::channel();
        let client = tokio::spawn(async move {
            let mut gate = GameShopReceiptGate::default();
            connect_gateway_with_resume_controls(
                &format!("ws://{address}"),
                &mut command_receiver,
                true,
                Some(tokio::time::Instant::now() + Duration::from_secs(2)),
                16,
                &mut gate,
            )
            .await
        });

        timeout(Duration::from_secs(2), accepted_rx)
            .await
            .expect("connect_async must be awaiting the server handshake")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::LogOut))
            .expect("logout must cancel a pending resume connection");
        assert!(matches!(
            timeout(Duration::from_millis(500), client)
                .await
                .expect("logout must not wait for websocket handshake completion")
                .expect("connect lifecycle task must not panic"),
            ResumeLifecycle::Cancel
        ));
        release_tx
            .send(())
            .expect("loopback server must still be waiting");
        timeout(Duration::from_secs(2), server)
            .await
            .expect("loopback server must stop")
            .expect("loopback server task must not panic");
    }

    #[tokio::test]
    async fn native_resume_rejected_is_terminal_and_never_replays_queued_input() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let credential = "B".repeat(MAX_CREDENTIAL_LENGTH);
        let (initial_ready_tx, initial_ready_rx) = oneshot::channel();
        let (resume_pending_tx, resume_pending_rx) = oneshot::channel();
        let (allow_rejection_tx, allow_rejection_rx) = oneshot::channel();
        let server_credential = credential.clone();

        let server = tokio::spawn(async move {
            let (first_tcp, _) = listener.accept().await.expect("first client connection");
            let mut first = accept_async(first_tcp)
                .await
                .expect("first websocket handshake");
            let _ = receive_wire_type(&mut first, "clientCapabilities").await;
            first
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": server_credential,
                        "expiresAtMs": gateway_unix_ms() + 30_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send resume credential");
            initial_ready_tx
                .send(())
                .expect("test must send one initial command");
            let _ = receive_wire_type(&mut first, "walk").await;
            let _ = first.close(None).await;
            drop(first);

            let (second_tcp, _) = listener
                .accept()
                .await
                .expect("reconnect client connection");
            let mut second = accept_async(second_tcp)
                .await
                .expect("reconnect websocket handshake");
            let _ = receive_wire_type(&mut second, "clientCapabilities").await;
            let resume = receive_wire_type(&mut second, "resumeSession").await;
            assert_eq!(resume["credential"], json!(credential));
            resume_pending_tx
                .send(())
                .expect("test must queue stale input before rejection");
            allow_rejection_rx
                .await
                .expect("test must allow the terminal response");
            assert_no_player_command_while_awaiting_resume(&mut second).await;
            second
                .send(Message::Text(
                    json!({ "type": "resumeRejected", "code": "unavailable" })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send resume rejection");
        });

        let (command_sender, command_receiver) = command_channel(8);
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, _gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = format!("ws://{address}");
        let client = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig::default(),
                |_| true,
            )
            .await
        });

        timeout(Duration::from_secs(2), initial_ready_rx)
            .await
            .expect("client must receive the initial credential")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }))
            .expect("initial input must be accepted");
        timeout(Duration::from_secs(2), resume_pending_rx)
            .await
            .expect("client must initiate resume")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Run {
                direction: "right".to_owned(),
            }))
            .expect("stale input must be accepted into the bounded queue");
        sleep(Duration::from_millis(60)).await;
        allow_rejection_tx
            .send(())
            .expect("loopback server must still await rejection permission");

        let disconnect_reason = timeout(Duration::from_secs(3), async {
            loop {
                match shell_receiver.try_recv() {
                    Ok(ShellGatewayEvent::Disconnect { reason }) => break reason,
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("native shell event channel closed before terminal rejection")
                    }
                }
            }
        })
        .await
        .expect("resume rejection must reach the shell");
        assert_eq!(
            disconnect_reason.as_deref(),
            Some("session resume unavailable")
        );

        command_sender
            .send(GatewayCommand::Shutdown)
            .expect("shutdown must be accepted");
        timeout(Duration::from_secs(3), client)
            .await
            .expect("native client must shut down")
            .expect("native client task must not panic")
            .expect("native client must exit cleanly");
        timeout(Duration::from_secs(3), server)
            .await
            .expect("loopback server must shut down")
            .expect("loopback server task must not panic");
    }

    #[tokio::test]
    async fn native_resume_logout_cancels_pending_resume_without_retry() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let credential = "C".repeat(MAX_CREDENTIAL_LENGTH);
        let (initial_ready_tx, initial_ready_rx) = oneshot::channel();
        let (resume_pending_tx, resume_pending_rx) = oneshot::channel();
        let server_credential = credential.clone();

        let server = tokio::spawn(async move {
            let (first_tcp, _) = listener.accept().await.expect("first client connection");
            let mut first = accept_async(first_tcp)
                .await
                .expect("first websocket handshake");
            let _ = receive_wire_type(&mut first, "clientCapabilities").await;
            first
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": server_credential,
                        "expiresAtMs": gateway_unix_ms() + 30_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send resume credential");
            initial_ready_tx
                .send(())
                .expect("test must send one initial command");
            let _ = receive_wire_type(&mut first, "walk").await;
            let _ = first.close(None).await;
            drop(first);

            let (second_tcp, _) = listener
                .accept()
                .await
                .expect("reconnect client connection");
            let mut second = accept_async(second_tcp)
                .await
                .expect("reconnect websocket handshake");
            let _ = receive_wire_type(&mut second, "clientCapabilities").await;
            let resume = receive_wire_type(&mut second, "resumeSession").await;
            assert_eq!(resume["credential"], json!(credential));
            resume_pending_tx
                .send(())
                .expect("test must cancel while resume is pending");

            assert_cancelled_resume_closes_without_replay(&mut second).await;
            assert!(
                timeout(Duration::from_millis(180), listener.accept())
                    .await
                    .is_err(),
                "explicit logout must not open a third resume socket"
            );
        });

        let (command_sender, command_receiver) = command_channel(8);
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, _gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = format!("ws://{address}");
        let client = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig::default(),
                {
                    let mut snapshot_ingest_count = 0_u8;
                    move |_| {
                        snapshot_ingest_count = snapshot_ingest_count.saturating_add(1);
                        // Initial snapshot is accepted; the first resumed
                        // snapshot is deliberately backpressured; only the
                        // next resumed snapshot opens the input fence.
                        snapshot_ingest_count != 2
                    }
                },
            )
            .await
        });

        timeout(Duration::from_secs(2), initial_ready_rx)
            .await
            .expect("client must receive the initial credential")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }))
            .expect("initial input must be accepted");
        timeout(Duration::from_secs(2), resume_pending_rx)
            .await
            .expect("client must initiate resume")
            .expect("loopback server must remain running");

        command_sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::LogOut))
            .expect("logout must cancel pending resume");
        let disconnect_reason = timeout(Duration::from_secs(3), async {
            loop {
                match shell_receiver.try_recv() {
                    Ok(ShellGatewayEvent::Disconnect { reason }) => break reason,
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("native shell event channel closed before cancellation")
                    }
                }
            }
        })
        .await
        .expect("logout cancellation must reach the shell");
        assert_eq!(
            disconnect_reason.as_deref(),
            Some("native reconnect cancelled")
        );

        command_sender
            .send(GatewayCommand::Shutdown)
            .expect("shutdown must be accepted after cancellation");
        timeout(Duration::from_secs(3), client)
            .await
            .expect("native client must shut down")
            .expect("native client task must not panic")
            .expect("native client must exit cleanly");
        timeout(Duration::from_secs(3), server)
            .await
            .expect("loopback server must shut down")
            .expect("loopback server task must not panic");
    }
}

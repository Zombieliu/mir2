//! Android's reducer-to-wire boundary.
//!
//! This module owns no websocket and no Android SDK handle. It converts the
//! shared UI reducer's gateway effects into the same JSON objects accepted by
//! the Web/Windows BrowserCommand path, then retains them until an Activity or
//! websocket host explicitly drains the queue.

use std::collections::VecDeque;

use bevy::prelude::Resource;
use mir2_ui_core::action::UiAction;
use mir2_ui_core::effect::{GatewayCommand, UiEffect};
use mir2_ui_core::game_shop::{
    GameShopReceipt, GameShopRequest, NATIVE_GAME_SHOP_RECEIPT_CAPABILITY,
};
use mir2_ui_core::reducer::reduce;
use mir2_ui_core::state::UiState;
use serde_json::{Value, json};

use crate::android_input::{AndroidLifecycle, AndroidNetwork, AndroidShellState};

pub const ANDROID_GATEWAY_QUEUE_CAPACITY: usize = 256;
pub const ANDROID_GATEWAY_INBOUND_CAPACITY: usize = 32;
/// A receipt is a small control message, not an arbitrary Android payload.
/// Keep this comfortably above the shared request/code limits while bounding
/// the JNI handoff before any JSON parser or Bevy system sees it.
pub const ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES: usize = 16 * 1024;
pub const ANDROID_GATEWAY_INBOUND_MAX_BYTES: usize = 128 * 1024;

/// The Android Activity/transport host sends this envelope when it opens its
/// WebSocket. This module intentionally does not own or declare a WebSocket.
pub fn native_game_shop_capabilities_json() -> Value {
    json!({
        "type": "clientCapabilities",
        "capabilities": ["nativeResumeV1", NATIVE_GAME_SHOP_RECEIPT_CAPABILITY]
    })
}

pub fn parse_native_game_shop_receipt(json_text: &str) -> Result<GameShopReceipt, String> {
    let receipt = serde_json::from_str::<GameShopReceipt>(json_text)
        .map_err(|error| format!("invalid gameShopReceipt: {error}"))?;
    receipt
        .is_valid()
        .then_some(receipt)
        .ok_or_else(|| "invalid gameShopReceipt contract".to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AndroidGatewayOutboundKind {
    /// Safe to send as a websocket text frame.
    Wire,
    /// A shared command with no BrowserCommand/server equivalent. It remains
    /// observable to the host, but must not be sent as a fabricated packet.
    LocalOnly { reason: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidGatewayOutbound {
    pub sequence: u64,
    pub kind: AndroidGatewayOutboundKind,
    pub json: String,
}

impl AndroidGatewayOutbound {
    pub fn is_sendable(&self) -> bool {
        matches!(self.kind, AndroidGatewayOutboundKind::Wire)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidGatewayQueueStatus {
    pub capacity: usize,
    pub len: usize,
    pub next_sequence: u64,
    pub overflow_count: u64,
    pub overflowed: bool,
    pub last_overflow_type: Option<String>,
    pub local_only_count: u64,
    pub last_local_only_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AndroidGatewayEnqueueError {
    Full {
        capacity: usize,
        command_type: String,
    },
    RequestInFlight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AndroidGameShopAdapterError {
    ReducerRejected,
    UnexpectedEffects,
    Enqueue(AndroidGatewayEnqueueError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AndroidGatewayInboundMessage {
    GameShopReceipt {
        json: String,
        exact_for_pending: bool,
    },
}

impl AndroidGatewayInboundMessage {
    #[cfg(test)]
    fn game_shop_receipt(json: impl Into<String>) -> Self {
        Self::GameShopReceipt {
            json: json.into(),
            exact_for_pending: false,
        }
    }

    fn byte_len(&self) -> usize {
        match self {
            Self::GameShopReceipt { json, .. } => json.len(),
        }
    }

    fn matches_game_shop_request(&self, request: &GameShopRequest) -> bool {
        match self {
            Self::GameShopReceipt { json, .. } => parse_native_game_shop_receipt(json)
                .is_ok_and(|receipt| receipt.matches_request(request)),
        }
    }

    fn set_exact_for_pending(&mut self, exact: bool) {
        match self {
            Self::GameShopReceipt {
                exact_for_pending, ..
            } => *exact_for_pending = exact,
        }
    }

    fn into_game_shop_receipt(self) -> (String, bool) {
        match self {
            Self::GameShopReceipt {
                json,
                exact_for_pending,
            } => (json, exact_for_pending),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AndroidGatewayInboundEnqueueError {
    Full {
        capacity: usize,
    },
    BytesFull {
        capacity_bytes: usize,
        queued_bytes: usize,
        message_bytes: usize,
    },
    MessageTooLarge {
        max_bytes: usize,
        actual_bytes: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AndroidGatewayInboundStatus {
    pub capacity: usize,
    pub max_message_bytes: usize,
    pub max_bytes: usize,
    pub len: usize,
    pub queued_bytes: usize,
    pub overflow_count: u64,
    pub byte_overflow_count: u64,
    pub oversize_count: u64,
    pub malformed_count: u64,
    pub unmatched_count: u64,
}

/// Bounded handoff owned by the Bevy app. A future JNI/WebSocket transport may
/// enqueue raw receipt text through the public API below; parsing and mutation
/// remain on the single-writer Bevy Update thread.
#[derive(Debug, Resource)]
pub struct AndroidGatewayInboundQueue {
    capacity: usize,
    max_message_bytes: usize,
    max_bytes: usize,
    entries: VecDeque<AndroidGatewayInboundMessage>,
    queued_bytes: usize,
    overflow_count: u64,
    byte_overflow_count: u64,
    oversize_count: u64,
    overflow_pending: bool,
    malformed_count: u64,
    unmatched_count: u64,
    pending_game_shop: Option<GameShopRequest>,
    exact_receipt_reserved: bool,
}

impl Default for AndroidGatewayInboundQueue {
    fn default() -> Self {
        Self::with_capacity(ANDROID_GATEWAY_INBOUND_CAPACITY)
    }
}

impl AndroidGatewayInboundQueue {
    fn with_capacity(capacity: usize) -> Self {
        Self::with_limits(
            capacity,
            ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES,
            ANDROID_GATEWAY_INBOUND_MAX_BYTES,
        )
    }

    fn with_limits(capacity: usize, max_message_bytes: usize, max_bytes: usize) -> Self {
        assert!(
            capacity > 0,
            "Android inbound queue capacity must be non-zero"
        );
        assert!(
            max_message_bytes > 0,
            "Android inbound message limit must be non-zero"
        );
        assert!(
            max_bytes >= max_message_bytes,
            "Android inbound byte budget must fit one message"
        );
        Self {
            capacity,
            max_message_bytes,
            max_bytes,
            entries: VecDeque::with_capacity(capacity),
            queued_bytes: 0,
            overflow_count: 0,
            byte_overflow_count: 0,
            oversize_count: 0,
            overflow_pending: false,
            malformed_count: 0,
            unmatched_count: 0,
            pending_game_shop: None,
            exact_receipt_reserved: false,
        }
    }

    fn enqueue(
        &mut self,
        mut message: AndroidGatewayInboundMessage,
    ) -> Result<(), AndroidGatewayInboundEnqueueError> {
        let message_bytes = message.byte_len();
        if message_bytes > self.max_message_bytes {
            self.oversize_count = self.oversize_count.saturating_add(1);
            return Err(AndroidGatewayInboundEnqueueError::MessageTooLarge {
                max_bytes: self.max_message_bytes,
                actual_bytes: message_bytes,
            });
        }
        if self.queued_bytes.saturating_add(message_bytes) > self.max_bytes {
            self.byte_overflow_count = self.byte_overflow_count.saturating_add(1);
            self.overflow_pending = self.should_mark_pending_unknown();
            return Err(AndroidGatewayInboundEnqueueError::BytesFull {
                capacity_bytes: self.max_bytes,
                queued_bytes: self.queued_bytes,
                message_bytes,
            });
        }
        if self.entries.len() >= self.capacity {
            self.overflow_count = self.overflow_count.saturating_add(1);
            // Never turn a queue-full notification into an unknown result
            // after a valid receipt is already retained. The receipt remains
            // FIFO-owned and will be consumed on the next Bevy update.
            self.overflow_pending = self.should_mark_pending_unknown();
            return Err(AndroidGatewayInboundEnqueueError::Full {
                capacity: self.capacity,
            });
        }
        let reserves_exact_receipt = !self.exact_receipt_reserved
            && self
                .pending_game_shop
                .as_ref()
                .is_some_and(|request| message.matches_game_shop_request(request));
        message.set_exact_for_pending(reserves_exact_receipt);
        self.queued_bytes = self.queued_bytes.saturating_add(message_bytes);
        self.entries.push_back(message);
        if reserves_exact_receipt {
            self.exact_receipt_reserved = true;
        }
        Ok(())
    }

    pub fn status(&self) -> AndroidGatewayInboundStatus {
        AndroidGatewayInboundStatus {
            capacity: self.capacity,
            max_message_bytes: self.max_message_bytes,
            max_bytes: self.max_bytes,
            len: self.entries.len(),
            queued_bytes: self.queued_bytes,
            overflow_count: self.overflow_count,
            byte_overflow_count: self.byte_overflow_count,
            oversize_count: self.oversize_count,
            malformed_count: self.malformed_count,
            unmatched_count: self.unmatched_count,
        }
    }

    fn drain(&mut self) -> Vec<AndroidGatewayInboundMessage> {
        self.queued_bytes = 0;
        self.exact_receipt_reserved = false;
        self.entries.drain(..).collect()
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.entries.clear();
        self.queued_bytes = 0;
        self.exact_receipt_reserved = false;
    }

    fn take_overflow_pending(&mut self) -> bool {
        if self.exact_receipt_reserved {
            self.overflow_pending = false;
            return false;
        }
        std::mem::take(&mut self.overflow_pending)
    }

    fn bind_game_shop_pending(&mut self, pending: Option<GameShopRequest>) {
        if self.pending_game_shop != pending {
            self.pending_game_shop = pending;
            // Receipts received before this exact transaction was bound are
            // quarantined and must never become a reserve retroactively.
            self.exact_receipt_reserved = false;
            self.overflow_pending = false;
        }
    }

    fn clear_game_shop_pending(&mut self) {
        self.pending_game_shop = None;
        self.exact_receipt_reserved = false;
        self.overflow_pending = false;
    }

    fn should_mark_pending_unknown(&self) -> bool {
        self.pending_game_shop.is_some() && !self.exact_receipt_reserved
    }

    fn record_malformed(&mut self) {
        self.malformed_count = self.malformed_count.saturating_add(1);
    }

    fn record_unmatched(&mut self) {
        self.unmatched_count = self.unmatched_count.saturating_add(1);
    }
}

/// Public transport/JNI-host boundary. This does not claim or create a real
/// WebSocket; it only transfers one raw receipt into the installed Bevy queue.
pub fn enqueue_native_game_shop_receipt(
    inbound: &mut AndroidGatewayInboundQueue,
    json_text: impl Into<String>,
) -> Result<(), AndroidGatewayInboundEnqueueError> {
    inbound.enqueue(AndroidGatewayInboundMessage::GameShopReceipt {
        json: json_text.into(),
        exact_for_pending: false,
    })
}

/// Atomically reduce one shared GameShop action and retain its outbound wire
/// command. UiState is committed only after the bounded queue accepts the
/// exact request, so queue pressure cannot leave a phantom pending purchase.
pub fn enqueue_game_shop_purchase(
    ui_state: &mut UiState,
    gateway: &mut AndroidGatewayOutboundQueue,
    inbound: &mut AndroidGatewayInboundQueue,
    g_index: i32,
    quantity: u8,
    price_type: i32,
) -> Result<GameShopRequest, AndroidGameShopAdapterError> {
    let transition = reduce(
        ui_state,
        UiAction::GameShopBuy {
            g_index,
            quantity,
            price_type,
        },
    );
    let request = transition
        .state
        .game_shop_pending
        .clone()
        .ok_or(AndroidGameShopAdapterError::ReducerRejected)?;
    let mut effects = transition.effects.into_iter();
    let Some(UiEffect::GatewayCommand(command @ GatewayCommand::GameShopBuy { .. })) =
        effects.next()
    else {
        return Err(AndroidGameShopAdapterError::UnexpectedEffects);
    };
    if effects.next().is_some() {
        return Err(AndroidGameShopAdapterError::UnexpectedEffects);
    }
    gateway
        .enqueue(command)
        .map_err(AndroidGameShopAdapterError::Enqueue)?;
    inbound.bind_game_shop_pending(Some(request.clone()));
    *ui_state = transition.state;
    Ok(request)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AndroidQueuedReceiptOutcome {
    Applied,
    Unmatched,
    Malformed,
}

/// Queue-drain-only consumer. The internal message carries the delivery
/// qualification frozen by `AndroidGatewayInboundQueue::enqueue`; callers
/// outside this crate cannot construct or pass arbitrary receipt text here.
fn consume_queued_native_game_shop_receipt(
    ui_state: &mut UiState,
    gateway: &mut AndroidGatewayOutboundQueue,
    message: AndroidGatewayInboundMessage,
) -> AndroidQueuedReceiptOutcome {
    let (json, exact_for_pending) = message.into_game_shop_receipt();
    let Ok(receipt) = parse_native_game_shop_receipt(&json) else {
        return AndroidQueuedReceiptOutcome::Malformed;
    };
    if !exact_for_pending {
        return AndroidQueuedReceiptOutcome::Unmatched;
    }
    let ui_matches = ui_state
        .game_shop_pending
        .as_ref()
        .is_some_and(|request| receipt.matches_request(request));
    let queue_matches = gateway
        .game_shop_pending()
        .is_some_and(|request| receipt.matches_request(request));
    if !ui_matches || !queue_matches {
        return AndroidQueuedReceiptOutcome::Unmatched;
    }
    let queue_applied = gateway.apply_game_shop_receipt(&receipt);
    let ui_applied = ui_state.apply_game_shop_receipt(receipt);
    debug_assert_eq!(queue_applied, ui_applied);
    if queue_applied && ui_applied {
        AndroidQueuedReceiptOutcome::Applied
    } else {
        AndroidQueuedReceiptOutcome::Unmatched
    }
}

/// The Bevy system's only inbound-consumption seam. Raw queue entries and
/// their frozen qualification remain private to this module; callers provide
/// only the bounded queue and the two correlation owners.
pub(crate) fn drain_bounded_inbound_into_models(
    inbound: &mut AndroidGatewayInboundQueue,
    ui_state: &mut UiState,
    gateway: &mut AndroidGatewayOutboundQueue,
) {
    inbound.bind_game_shop_pending(gateway.game_shop_pending().cloned());
    if inbound.take_overflow_pending() {
        gateway.mark_game_shop_unknown();
        ui_state.mark_game_shop_unknown();
        inbound.clear_game_shop_pending();
    }

    for message in inbound.drain() {
        match consume_queued_native_game_shop_receipt(ui_state, gateway, message) {
            AndroidQueuedReceiptOutcome::Applied => inbound.clear_game_shop_pending(),
            AndroidQueuedReceiptOutcome::Unmatched => inbound.record_unmatched(),
            AndroidQueuedReceiptOutcome::Malformed => inbound.record_malformed(),
        }
    }
}

/// Owner-level terminal reset seam. It exposes no raw message or eligibility
/// state and only prevents a receipt from qualifying against a closed session.
pub(crate) fn clear_bounded_inbound_transaction(inbound: &mut AndroidGatewayInboundQueue) {
    inbound.clear_game_shop_pending();
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AndroidGatewayQueueCounters {
    pub overflow_count: u64,
    pub local_only_count: u64,
}

#[derive(Debug, Resource)]
pub struct AndroidGatewayOutboundQueue {
    capacity: usize,
    next_sequence: u64,
    entries: VecDeque<AndroidGatewayOutbound>,
    overflow_count: u64,
    last_overflow_type: Option<String>,
    local_only_count: u64,
    last_local_only_type: Option<String>,
    pending_game_shop: Option<GameShopRequest>,
    game_shop_unknown: bool,
}

impl Default for AndroidGatewayOutboundQueue {
    fn default() -> Self {
        Self::with_capacity(ANDROID_GATEWAY_QUEUE_CAPACITY)
    }
}

impl AndroidGatewayOutboundQueue {
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(
            capacity > 0,
            "Android gateway queue capacity must be non-zero"
        );
        Self {
            capacity,
            next_sequence: 1,
            entries: VecDeque::with_capacity(capacity),
            overflow_count: 0,
            last_overflow_type: None,
            local_only_count: 0,
            last_local_only_type: None,
            pending_game_shop: None,
            game_shop_unknown: false,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn counters(&self) -> AndroidGatewayQueueCounters {
        AndroidGatewayQueueCounters {
            overflow_count: self.overflow_count,
            local_only_count: self.local_only_count,
        }
    }

    pub fn status(&self) -> AndroidGatewayQueueStatus {
        AndroidGatewayQueueStatus {
            capacity: self.capacity,
            len: self.entries.len(),
            next_sequence: self.next_sequence,
            overflow_count: self.overflow_count,
            overflowed: self.overflow_count != 0,
            last_overflow_type: self.last_overflow_type.clone(),
            local_only_count: self.local_only_count,
            last_local_only_type: self.last_local_only_type.clone(),
        }
    }

    /// Convert and retain one reducer command. A full queue rejects the new
    /// command and records the rejection; existing FIFO entries are untouched.
    pub fn enqueue(&mut self, command: GatewayCommand) -> Result<(), AndroidGatewayEnqueueError> {
        if let GatewayCommand::GameShopBuy {
            request_id,
            g_index,
            quantity,
            price_type,
        } = &command
        {
            if self.pending_game_shop.is_some() {
                return Err(AndroidGatewayEnqueueError::RequestInFlight);
            }
            let Some(request) =
                GameShopRequest::new(request_id.clone(), *g_index, *quantity, *price_type)
            else {
                return Err(AndroidGatewayEnqueueError::RequestInFlight);
            };
            // Reserved below only after queue capacity is confirmed.
            let _ = request;
        }
        let command_type = command_type(&command);
        if self.entries.len() >= self.capacity {
            self.overflow_count = self.overflow_count.saturating_add(1);
            self.last_overflow_type = Some(command_type.clone());
            return Err(AndroidGatewayEnqueueError::Full {
                capacity: self.capacity,
                command_type,
            });
        }

        if let GatewayCommand::GameShopBuy {
            request_id,
            g_index,
            quantity,
            price_type,
        } = &command
        {
            self.pending_game_shop =
                GameShopRequest::new(request_id.clone(), *g_index, *quantity, *price_type);
            self.game_shop_unknown = false;
        }

        let (kind, value) = to_wire_value(&command);
        if let AndroidGatewayOutboundKind::LocalOnly { .. } = kind {
            self.local_only_count = self.local_only_count.saturating_add(1);
            self.last_local_only_type = Some(command_type);
        }
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.entries.push_back(AndroidGatewayOutbound {
            sequence,
            kind,
            json: serde_json::to_string(&value).expect("wire JSON values are serializable"),
        });
        Ok(())
    }

    fn apply_game_shop_receipt(&mut self, receipt: &GameShopReceipt) -> bool {
        let Some(request) = self.pending_game_shop.as_ref() else {
            return false;
        };
        if !receipt.is_valid() || !receipt.matches_request(request) {
            return false;
        }
        let request_id = request.request_id.clone();
        self.pending_game_shop = None;
        self.game_shop_unknown = false;
        self.entries
            .retain(|entry| !outbound_matches_game_shop_request(entry, &request_id));
        true
    }

    pub(crate) fn mark_game_shop_unknown(&mut self) {
        if self.pending_game_shop.take().is_some() {
            self.game_shop_unknown = true;
        }
        self.entries.retain(|entry| !outbound_is_game_shop(entry));
    }

    pub(crate) fn mark_terminal_reset(&mut self) {
        self.mark_game_shop_unknown();
        self.entries.clear();
    }

    #[cfg(test)]
    pub(crate) fn game_shop_unknown(&self) -> bool {
        self.game_shop_unknown
    }

    pub(crate) fn game_shop_pending(&self) -> Option<&GameShopRequest> {
        self.pending_game_shop.as_ref()
    }

    /// Drain only when the host is in the foreground and has a usable network.
    /// Background/unavailable states leave every entry in place.
    pub fn drain_ready(
        &mut self,
        shell: &AndroidShellState,
        max_entries: usize,
    ) -> Vec<AndroidGatewayOutbound> {
        if !can_send(shell) || max_entries == 0 {
            return Vec::new();
        }
        let count = max_entries.min(self.entries.len());
        self.entries.drain(..count).collect()
    }

    /// Remove all retained entries after the host has deliberately handled a
    /// session teardown. Overflow/local-only counters remain as diagnostics.
    pub fn clear_entries(&mut self) {
        self.entries.clear();
    }
}

fn outbound_is_game_shop(entry: &AndroidGatewayOutbound) -> bool {
    serde_json::from_str::<Value>(&entry.json)
        .ok()
        .and_then(|value| value.get("type").and_then(Value::as_str).map(str::to_owned))
        .as_deref()
        == Some("gameShopBuy")
}

fn outbound_matches_game_shop_request(entry: &AndroidGatewayOutbound, request_id: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(&entry.json) else {
        return false;
    };
    value.get("type").and_then(Value::as_str) == Some("gameShopBuy")
        && value.get("requestId").and_then(Value::as_str) == Some(request_id)
}

pub fn can_send(shell: &AndroidShellState) -> bool {
    shell.lifecycle == AndroidLifecycle::Foreground && shell.network == AndroidNetwork::Available
}

fn command_type(command: &GatewayCommand) -> String {
    match command {
        GatewayCommand::Login { .. } => "login",
        GatewayCommand::RegisterAccount { .. } => "newAccount",
        GatewayCommand::StartGame { .. } => "startGame",
        GatewayCommand::GameShopBuy { .. } => "gameShopBuy",
        GatewayCommand::SendMail { .. } => "sendMail",
        GatewayCommand::CreateCharacter { .. } => "newCharacter",
        GatewayCommand::DeleteCharacter { .. } => "deleteCharacter",
        GatewayCommand::Logout => "logOut",
        GatewayCommand::RetryConnection => "retryConnection",
        GatewayCommand::InteractNpc { .. } => "interact",
        GatewayCommand::SelectNpcDialog { .. } => "selectNpcDialog",
        GatewayCommand::AcceptQuest { .. } => "acceptQuest",
        GatewayCommand::FinishQuest { .. } => "finishQuest",
        GatewayCommand::UseItem { .. } => "useItem",
        GatewayCommand::EquipItem { .. } => "equipItem",
        GatewayCommand::UnequipItem { .. } => "removeItem",
        GatewayCommand::DropItem { .. } => "dropItem",
        GatewayCommand::MoveItem { .. } => "moveItem",
        GatewayCommand::MergeItem { .. } => "mergeItem",
        GatewayCommand::SplitItem { .. } => "splitItem",
        GatewayCommand::AbandonQuest { .. } => "abandonQuest",
        GatewayCommand::SetChatChannel { .. } => "setChatChannel",
        GatewayCommand::AttackTarget { .. } => "attack",
        GatewayCommand::PickUp { .. } => "pickUp",
        GatewayCommand::SendChat { .. } => "chat",
        GatewayCommand::TownRevive => "townRevive",
        GatewayCommand::GroupSwitch { .. } => "switchGroup",
        GatewayCommand::GroupAddMember { .. } => "addMember",
        GatewayCommand::GroupRemoveMember { .. } => "delMember",
        GatewayCommand::GroupInvite { .. } => "groupInvite",
        GatewayCommand::GuildRequestInfo { .. } => "requestGuildInfo",
        GatewayCommand::GuildEditMember { .. } => "editGuildMember",
        GatewayCommand::GuildEditNotice { .. } => "editGuildNotice",
        GatewayCommand::GuildInvite { .. } => "guildInvite",
        GatewayCommand::TradeRequest => "tradeRequest",
        GatewayCommand::TradeReply { .. } => "tradeReply",
        GatewayCommand::TradeGold { .. } => "tradeGold",
        GatewayCommand::TradeDepositItem { .. } => "depositTradeItem",
        GatewayCommand::TradeRetrieveItem { .. } => "retrieveTradeItem",
        GatewayCommand::TradeConfirm { .. } => "tradeConfirm",
        GatewayCommand::TradeCancel => "tradeCancel",
    }
    .to_owned()
}

fn to_wire_value(command: &GatewayCommand) -> (AndroidGatewayOutboundKind, Value) {
    match command {
        GatewayCommand::Login { account, password } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"login","accountId":account,"password":password}),
        ),
        GatewayCommand::RegisterAccount { account, password } => (
            AndroidGatewayOutboundKind::Wire,
            json!({
                "type":"newAccount",
                "accountId":account,
                "password":password,
                "birthDateBinary":0,
                "userName":account,
                "secretQuestion":"",
                "secretAnswer":"",
                "emailAddress":""
            }),
        ),
        GatewayCommand::StartGame { index } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"startGame","characterIndex":index}),
        ),
        GatewayCommand::GameShopBuy {
            request_id,
            g_index,
            quantity,
            price_type,
        } => (
            AndroidGatewayOutboundKind::Wire,
            json!({
                "type":"gameShopBuy",
                "requestId":request_id,
                "gIndex":g_index,
                "quantity":quantity,
                "priceType":price_type
            }),
        ),
        GatewayCommand::SendMail {
            recipient,
            message,
            gold,
            attachment_unique_ids,
        } => {
            if attachment_unique_ids.len() > 5 {
                return (
                    AndroidGatewayOutboundKind::LocalOnly {
                        reason: "sendMail supports at most five authoritative attachment IDs",
                    },
                    json!({"type":"sendMailRejected","reason":"tooManyAttachments"}),
                );
            }
            let mut items_idx = [0_u64; 5];
            for (slot, unique_id) in items_idx.iter_mut().zip(attachment_unique_ids) {
                *slot = *unique_id;
            }
            (
                AndroidGatewayOutboundKind::Wire,
                json!({
                    "type":"sendMail",
                    "name":recipient,
                    "message":message,
                    "gold":gold,
                    "itemsIdx":items_idx,
                    "stamped":false
                }),
            )
        }
        GatewayCommand::CreateCharacter {
            name,
            class,
            gender,
        } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"newCharacter","name":name,"gender":gender,"class":class}),
        ),
        GatewayCommand::DeleteCharacter { index } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"deleteCharacter","characterIndex":index}),
        ),
        GatewayCommand::Logout => (AndroidGatewayOutboundKind::Wire, json!({"type":"logOut"})),
        GatewayCommand::RetryConnection => (
            AndroidGatewayOutboundKind::LocalOnly {
                reason: "the Android transport host must reopen the connection",
            },
            json!({"type":"retryConnection"}),
        ),
        GatewayCommand::InteractNpc { object_id } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"interact","objectId":object_id}),
        ),
        GatewayCommand::SelectNpcDialog { target } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"selectNpcDialog","target":target}),
        ),
        GatewayCommand::AcceptQuest {
            npc_index,
            quest_index,
        } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"acceptQuest","npcIndex":npc_index,"questIndex":quest_index}),
        ),
        GatewayCommand::FinishQuest {
            quest_index,
            selected_item_index,
        } => (
            AndroidGatewayOutboundKind::Wire,
            json!({
                "type":"finishQuest",
                "questIndex":quest_index,
                "selectedItemIndex":selected_item_index
            }),
        ),
        GatewayCommand::UseItem { unique_id } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"useItem","uniqueId":unique_id,"grid":"inventory"}),
        ),
        GatewayCommand::EquipItem { unique_id, to } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"equipItem","uniqueId":unique_id,"grid":"inventory","to":to}),
        ),
        GatewayCommand::UnequipItem { unique_id } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"removeItem","uniqueId":unique_id,"grid":"equipment","to":-1}),
        ),
        GatewayCommand::DropItem {
            key,
            unique_id,
            count,
            hero_inventory,
        } => (
            AndroidGatewayOutboundKind::Wire,
            json!({
                "type":"dropItem",
                "key":key,
                "uniqueId":unique_id,
                "count":count,
                "heroInventory":hero_inventory
            }),
        ),
        GatewayCommand::MoveItem { grid, from, to } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"moveItem","grid":grid,"from":from,"to":to}),
        ),
        GatewayCommand::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
        } => (
            AndroidGatewayOutboundKind::Wire,
            json!({
                "type":"mergeItem",
                "gridFrom":grid_from,
                "gridTo":grid_to,
                "idFrom":id_from,
                "idTo":id_to
            }),
        ),
        GatewayCommand::SplitItem {
            unique_id,
            grid,
            count,
        } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"splitItem","uniqueId":unique_id,"grid":grid,"count":count}),
        ),
        GatewayCommand::AbandonQuest { quest_index } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"abandonQuest","questIndex":quest_index}),
        ),
        GatewayCommand::SetChatChannel { channel } => (
            AndroidGatewayOutboundKind::LocalOnly {
                reason: "BrowserCommand has no setChatChannel variant",
            },
            json!({"type":"setChatChannel","channel":channel}),
        ),
        GatewayCommand::AttackTarget { object_id } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"attack","objectId":object_id}),
        ),
        GatewayCommand::PickUp { object_id } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"pickUp","objectId":object_id}),
        ),
        GatewayCommand::SendChat { message } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"chat","message":message}),
        ),
        GatewayCommand::TownRevive => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"townRevive"}),
        ),
        GatewayCommand::GroupSwitch { allow_group } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"switchGroup","allowGroup":allow_group}),
        ),
        GatewayCommand::GroupAddMember { name } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"addMember","name":name}),
        ),
        GatewayCommand::GroupRemoveMember { name } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"delMember","name":name}),
        ),
        GatewayCommand::GroupInvite { accept_invite } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"groupInvite","acceptInvite":accept_invite}),
        ),
        GatewayCommand::GuildRequestInfo { info_type } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"requestGuildInfo","infoType":info_type}),
        ),
        GatewayCommand::GuildEditMember {
            change_type,
            rank_index,
            name,
            rank_name,
        } => (
            AndroidGatewayOutboundKind::Wire,
            json!({
                "type":"editGuildMember",
                "changeType":change_type,
                "rankIndex":rank_index,
                "name":name,
                "rankName":rank_name
            }),
        ),
        GatewayCommand::GuildEditNotice { notice } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"editGuildNotice","notice":notice}),
        ),
        GatewayCommand::GuildInvite { accept_invite } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"guildInvite","acceptInvite":accept_invite}),
        ),
        GatewayCommand::TradeRequest => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"tradeRequest"}),
        ),
        GatewayCommand::TradeReply { accept_invite } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"tradeReply","acceptInvite":accept_invite}),
        ),
        GatewayCommand::TradeGold { amount } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"tradeGold","amount":amount}),
        ),
        GatewayCommand::TradeDepositItem { from, to } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"depositTradeItem","from":from,"to":to}),
        ),
        GatewayCommand::TradeRetrieveItem { from, to } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"retrieveTradeItem","from":from,"to":to}),
        ),
        GatewayCommand::TradeConfirm { locked } => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"tradeConfirm","locked":locked}),
        ),
        GatewayCommand::TradeCancel => (
            AndroidGatewayOutboundKind::Wire,
            json!({"type":"tradeCancel"}),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::android_input::{AndroidLifecycle, AndroidNetwork};

    fn shell(lifecycle: AndroidLifecycle, network: AndroidNetwork) -> AndroidShellState {
        AndroidShellState {
            lifecycle,
            network,
            ..Default::default()
        }
    }

    fn valid_receipt_json(request_id: &str) -> String {
        format!(
            r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"{request_id}","success":false,"gIndex":31,"quantity":1,"priceType":1,"code":"insufficientCurrency"}}"#
        )
    }

    fn game_shop_request(request_id: &str) -> GameShopRequest {
        GameShopRequest::new(request_id.into(), 31, 1, 1).unwrap()
    }

    fn one(command: GatewayCommand, expected: Value) {
        let mut queue = AndroidGatewayOutboundQueue::with_capacity(4);
        queue.enqueue(command).expect("enqueue");
        let entry = queue
            .drain_ready(
                &shell(AndroidLifecycle::Foreground, AndroidNetwork::Available),
                1,
            )
            .pop()
            .expect("one entry");
        assert!(entry.is_sendable());
        assert_eq!(
            serde_json::from_str::<Value>(&entry.json).unwrap(),
            expected
        );
    }

    #[test]
    fn every_wire_gateway_command_has_exact_browser_shape() {
        one(
            GatewayCommand::Login {
                account: "a".into(),
                password: "p".into(),
            },
            json!({"type":"login","accountId":"a","password":"p"}),
        );
        one(
            GatewayCommand::RegisterAccount {
                account: "a".into(),
                password: "p".into(),
            },
            json!({"type":"newAccount","accountId":"a","password":"p","birthDateBinary":0,"userName":"a","secretQuestion":"","secretAnswer":"","emailAddress":""}),
        );
        one(
            GatewayCommand::StartGame { index: 2 },
            json!({"type":"startGame","characterIndex":2}),
        );
        one(
            GatewayCommand::GameShopBuy {
                request_id: "gs-0000000000000001".into(),
                g_index: 105,
                quantity: 3,
                price_type: 0,
            },
            json!({"type":"gameShopBuy","requestId":"gs-0000000000000001","gIndex":105,"quantity":3,"priceType":0}),
        );
        one(
            GatewayCommand::SendMail {
                recipient: "Rita".into(),
                message: "hello".into(),
                gold: 25,
                attachment_unique_ids: vec![11, 12],
            },
            json!({"type":"sendMail","name":"Rita","message":"hello","gold":25,"itemsIdx":[11,12,0,0,0],"stamped":false}),
        );
        one(
            GatewayCommand::CreateCharacter {
                name: "n".into(),
                class: "Wizard".into(),
                gender: "Male".into(),
            },
            json!({"type":"newCharacter","name":"n","class":"Wizard","gender":"Male"}),
        );
        one(
            GatewayCommand::DeleteCharacter { index: 3 },
            json!({"type":"deleteCharacter","characterIndex":3}),
        );
        one(GatewayCommand::Logout, json!({"type":"logOut"}));
        one(
            GatewayCommand::InteractNpc { object_id: 4 },
            json!({"type":"interact","objectId":4}),
        );
        one(
            GatewayCommand::SelectNpcDialog {
                target: "@Buy".into(),
            },
            json!({"type":"selectNpcDialog","target":"@Buy"}),
        );
        one(
            GatewayCommand::AcceptQuest {
                npc_index: 5,
                quest_index: 6,
            },
            json!({"type":"acceptQuest","npcIndex":5,"questIndex":6}),
        );
        one(
            GatewayCommand::FinishQuest {
                quest_index: 6,
                selected_item_index: -1,
            },
            json!({"type":"finishQuest","questIndex":6,"selectedItemIndex":-1}),
        );
        one(
            GatewayCommand::UseItem { unique_id: 7 },
            json!({"type":"useItem","uniqueId":7,"grid":"inventory"}),
        );
        one(
            GatewayCommand::EquipItem {
                unique_id: 8,
                to: 2,
            },
            json!({"type":"equipItem","uniqueId":8,"grid":"inventory","to":2}),
        );
        one(
            GatewayCommand::UnequipItem { unique_id: 9 },
            json!({"type":"removeItem","uniqueId":9,"grid":"equipment","to":-1}),
        );
        one(
            GatewayCommand::DropItem {
                key: "potion".into(),
                unique_id: 10,
                count: 2,
                hero_inventory: false,
            },
            json!({"type":"dropItem","key":"potion","uniqueId":10,"count":2,"heroInventory":false}),
        );
        one(
            GatewayCommand::MoveItem {
                grid: "inventory".into(),
                from: 1,
                to: 2,
            },
            json!({"type":"moveItem","grid":"inventory","from":1,"to":2}),
        );
        one(
            GatewayCommand::MergeItem {
                grid_from: "inventory".into(),
                grid_to: "inventory".into(),
                id_from: 11,
                id_to: 12,
            },
            json!({"type":"mergeItem","gridFrom":"inventory","gridTo":"inventory","idFrom":11,"idTo":12}),
        );
        one(
            GatewayCommand::SplitItem {
                unique_id: 13,
                grid: "inventory".into(),
                count: 3,
            },
            json!({"type":"splitItem","uniqueId":13,"grid":"inventory","count":3}),
        );
        one(
            GatewayCommand::AbandonQuest { quest_index: 14 },
            json!({"type":"abandonQuest","questIndex":14}),
        );
        one(
            GatewayCommand::AttackTarget { object_id: 15 },
            json!({"type":"attack","objectId":15}),
        );
        one(
            GatewayCommand::PickUp { object_id: 16 },
            json!({"type":"pickUp","objectId":16}),
        );
        one(
            GatewayCommand::SendChat {
                message: "hi".into(),
            },
            json!({"type":"chat","message":"hi"}),
        );
        one(GatewayCommand::TownRevive, json!({"type":"townRevive"}));
        one(
            GatewayCommand::GroupSwitch { allow_group: true },
            json!({"type":"switchGroup","allowGroup":true}),
        );
        one(
            GatewayCommand::GroupAddMember { name: "A".into() },
            json!({"type":"addMember","name":"A"}),
        );
        one(
            GatewayCommand::GroupRemoveMember { name: "B".into() },
            json!({"type":"delMember","name":"B"}),
        );
        one(
            GatewayCommand::GroupInvite {
                accept_invite: true,
            },
            json!({"type":"groupInvite","acceptInvite":true}),
        );
        one(
            GatewayCommand::GuildRequestInfo { info_type: 2 },
            json!({"type":"requestGuildInfo","infoType":2}),
        );
        one(
            GatewayCommand::GuildEditMember {
                change_type: 1,
                rank_index: 3,
                name: "C".into(),
                rank_name: "Officer".into(),
            },
            json!({"type":"editGuildMember","changeType":1,"rankIndex":3,"name":"C","rankName":"Officer"}),
        );
        one(
            GatewayCommand::GuildEditNotice {
                notice: vec!["one".into(), "two".into()],
            },
            json!({"type":"editGuildNotice","notice":["one","two"]}),
        );
        one(
            GatewayCommand::GuildInvite {
                accept_invite: false,
            },
            json!({"type":"guildInvite","acceptInvite":false}),
        );
        one(GatewayCommand::TradeRequest, json!({"type":"tradeRequest"}));
        one(
            GatewayCommand::TradeReply {
                accept_invite: true,
            },
            json!({"type":"tradeReply","acceptInvite":true}),
        );
        one(
            GatewayCommand::TradeGold { amount: 99 },
            json!({"type":"tradeGold","amount":99}),
        );
        one(
            GatewayCommand::TradeDepositItem { from: 2, to: 0 },
            json!({"type":"depositTradeItem","from":2,"to":0}),
        );
        one(
            GatewayCommand::TradeRetrieveItem { from: 0, to: 3 },
            json!({"type":"retrieveTradeItem","from":0,"to":3}),
        );
        one(
            GatewayCommand::TradeConfirm { locked: true },
            json!({"type":"tradeConfirm","locked":true}),
        );
        one(GatewayCommand::TradeCancel, json!({"type":"tradeCancel"}));
    }

    #[test]
    fn native_capability_and_receipt_helpers_use_shared_contract() {
        assert_eq!(
            native_game_shop_capabilities_json(),
            json!({
                "type": "clientCapabilities",
                "capabilities": ["nativeResumeV1", "nativeGameShopReceiptV1"]
            })
        );
        let receipt = parse_native_game_shop_receipt(
            r#"{"protocol":"nativeGameShopReceiptV1","requestId":"gs-0000000000000001","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency"}"#,
        )
        .unwrap();
        let mut queue = AndroidGatewayOutboundQueue::default();
        queue
            .enqueue(GatewayCommand::GameShopBuy {
                request_id: "gs-0000000000000001".into(),
                g_index: 31,
                quantity: 2,
                price_type: 1,
            })
            .unwrap();
        assert!(!queue.apply_game_shop_receipt(&GameShopReceipt {
            request_id: "gs-other".into(),
            ..receipt.clone()
        }));
        assert!(queue.apply_game_shop_receipt(&receipt));
    }

    #[test]
    fn inbound_budget_reports_limits_and_releases_bytes_on_drain_and_clear() {
        let mut queue = AndroidGatewayInboundQueue::with_limits(4, 32, 64);
        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt("1234"))
            .unwrap();
        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt("56789"))
            .unwrap();
        assert_eq!(queue.status().queued_bytes, 9);
        assert_eq!(queue.status().max_message_bytes, 32);
        assert_eq!(queue.status().max_bytes, 64);

        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(queue.status().len, 0);
        assert_eq!(queue.status().queued_bytes, 0);

        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt("again"))
            .unwrap();
        queue.clear();
        assert_eq!(queue.status().len, 0);
        assert_eq!(queue.status().queued_bytes, 0);
    }

    #[test]
    fn inbound_rejects_single_message_and_total_byte_overflow_without_eviction() {
        let mut queue = AndroidGatewayInboundQueue::with_limits(4, 16, 16);
        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt("keep"))
            .unwrap();
        let too_large = queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt(
                "12345678901234567",
            ))
            .unwrap_err();
        assert_eq!(
            too_large,
            AndroidGatewayInboundEnqueueError::MessageTooLarge {
                max_bytes: 16,
                actual_bytes: 17,
            }
        );
        let bytes_full = queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt(
                "1234567890123456",
            ))
            .unwrap_err();
        assert_eq!(
            bytes_full,
            AndroidGatewayInboundEnqueueError::BytesFull {
                capacity_bytes: 16,
                queued_bytes: 4,
                message_bytes: 16,
            }
        );
        assert_eq!(queue.status().len, 1);
        assert_eq!(queue.status().queued_bytes, 4);
        assert_eq!(queue.status().oversize_count, 1);
        assert_eq!(queue.status().byte_overflow_count, 1);
    }

    #[test]
    fn inbound_count_flood_cannot_bypass_byte_budget() {
        let mut queue = AndroidGatewayInboundQueue::with_limits(32, 16, 20);
        for _ in 0..4 {
            queue
                .enqueue(AndroidGatewayInboundMessage::game_shop_receipt("12345"))
                .unwrap();
        }
        assert_eq!(queue.status().len, 4);
        assert_eq!(queue.status().queued_bytes, 20);
        let error = queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt("x"))
            .unwrap_err();
        assert!(matches!(
            error,
            AndroidGatewayInboundEnqueueError::BytesFull { .. }
        ));
        assert_eq!(queue.status().len, 4);
        assert_eq!(queue.status().queued_bytes, 20);
    }

    #[test]
    fn exact_receipt_is_retained_when_malformed_or_semantic_invalid_flood_is_rejected() {
        let exact = valid_receipt_json("gs-exact");
        let mut queue =
            AndroidGatewayInboundQueue::with_limits(2, exact.len() + 8, exact.len() + 8);
        queue.bind_game_shop_pending(Some(game_shop_request("gs-exact")));
        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt(
                exact.clone(),
            ))
            .unwrap();
        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt("not-json"))
            .unwrap();

        let error = queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt("overflow"))
            .unwrap_err();
        assert!(matches!(
            error,
            AndroidGatewayInboundEnqueueError::Full { .. }
                | AndroidGatewayInboundEnqueueError::BytesFull { .. }
        ));
        assert!(!queue.take_overflow_pending());
        let drained = queue.drain();
        let (drained_exact, exact_for_pending) = drained[0].clone().into_game_shop_receipt();
        assert_eq!(drained_exact, exact);
        assert!(exact_for_pending);
        assert_eq!(queue.status().queued_bytes, 0);
    }

    #[test]
    fn invalid_receipt_does_not_overwrite_exact_receipt_reserve() {
        let exact = valid_receipt_json("gs-exact");
        let invalid = r#"{"protocol":"nativeGameShopReceiptV1","requestId":"gs-exact","success":true,"gIndex":31,"quantity":1,"priceType":1,"code":"commitFailed","mailId":99}"#;
        let mut queue = AndroidGatewayInboundQueue::with_limits(4, 512, 1024);
        queue.bind_game_shop_pending(Some(game_shop_request("gs-exact")));
        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt(
                exact.clone(),
            ))
            .unwrap();
        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt(invalid))
            .unwrap();
        let messages = queue.drain();
        let (exact_json, exact_for_pending) = messages[0].clone().into_game_shop_receipt();
        let (invalid_json, invalid_for_pending) = messages[1].clone().into_game_shop_receipt();
        assert_eq!(exact_json, exact);
        assert!(exact_for_pending);
        assert_eq!(invalid_json, invalid);
        assert!(!invalid_for_pending);
        assert!(parse_native_game_shop_receipt(&invalid_json).is_err());
    }

    #[test]
    fn valid_wrong_receipt_cannot_reserve_or_suppress_pending_unknown() {
        let mut queue = AndroidGatewayInboundQueue::with_limits(1, 512, 1024);
        queue.bind_game_shop_pending(Some(game_shop_request("gs-a")));
        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt(
                valid_receipt_json("gs-b"),
            ))
            .unwrap();
        assert!(matches!(
            queue.enqueue(AndroidGatewayInboundMessage::game_shop_receipt("flood")),
            Err(AndroidGatewayInboundEnqueueError::Full { .. })
        ));
        assert!(queue.take_overflow_pending());
    }

    #[test]
    fn receipt_received_without_pending_cannot_be_promoted_later() {
        let mut queue = AndroidGatewayInboundQueue::default();
        queue
            .enqueue(AndroidGatewayInboundMessage::game_shop_receipt(
                valid_receipt_json("gs-a"),
            ))
            .unwrap();
        queue.bind_game_shop_pending(Some(game_shop_request("gs-a")));
        let (json, exact_for_pending) = queue.drain().pop().unwrap().into_game_shop_receipt();
        assert_eq!(json, valid_receipt_json("gs-a"));
        assert!(!exact_for_pending);
    }

    #[test]
    fn inbound_message_limit_counts_unicode_utf8_bytes() {
        let at_limit = format!("{}a", "界".repeat((16 * 1024 - 1) / 3));
        assert_eq!(at_limit.len(), ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES);
        let mut queue = AndroidGatewayInboundQueue::default();
        enqueue_native_game_shop_receipt(&mut queue, at_limit).unwrap();
        assert_eq!(
            queue.status().queued_bytes,
            ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES
        );
        queue.clear();

        let over_limit = format!("{}ab", "界".repeat((16 * 1024 - 1) / 3));
        assert_eq!(
            over_limit.len(),
            ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES + 1
        );
        assert_eq!(
            enqueue_native_game_shop_receipt(&mut queue, over_limit),
            Err(AndroidGatewayInboundEnqueueError::MessageTooLarge {
                max_bytes: ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES,
                actual_bytes: ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES + 1,
            })
        );
        assert_eq!(queue.status().queued_bytes, 0);
    }

    #[test]
    fn public_inbound_push_enforces_default_count_and_total_byte_limits() {
        let mut queue = AndroidGatewayInboundQueue::default();
        for _ in 0..ANDROID_GATEWAY_INBOUND_CAPACITY {
            enqueue_native_game_shop_receipt(&mut queue, "x").unwrap();
        }
        assert_eq!(
            enqueue_native_game_shop_receipt(&mut queue, "count-overflow"),
            Err(AndroidGatewayInboundEnqueueError::Full {
                capacity: ANDROID_GATEWAY_INBOUND_CAPACITY,
            })
        );

        queue.clear();
        let full_message = "x".repeat(ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES);
        let message_count =
            ANDROID_GATEWAY_INBOUND_MAX_BYTES / ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES;
        for _ in 0..message_count {
            enqueue_native_game_shop_receipt(&mut queue, full_message.clone()).unwrap();
        }
        assert_eq!(
            queue.status().queued_bytes,
            ANDROID_GATEWAY_INBOUND_MAX_BYTES
        );
        assert_eq!(
            enqueue_native_game_shop_receipt(&mut queue, "byte-overflow"),
            Err(AndroidGatewayInboundEnqueueError::BytesFull {
                capacity_bytes: ANDROID_GATEWAY_INBOUND_MAX_BYTES,
                queued_bytes: ANDROID_GATEWAY_INBOUND_MAX_BYTES,
                message_bytes: "byte-overflow".len(),
            })
        );
    }

    #[test]
    fn second_game_shop_request_is_rejected_until_exact_receipt() {
        let mut queue = AndroidGatewayOutboundQueue::default();
        queue
            .enqueue(GatewayCommand::GameShopBuy {
                request_id: "gs-1".into(),
                g_index: 1,
                quantity: 1,
                price_type: 0,
            })
            .unwrap();
        assert_eq!(
            queue
                .enqueue(GatewayCommand::GameShopBuy {
                    request_id: "gs-2".into(),
                    g_index: 2,
                    quantity: 1,
                    price_type: 0,
                })
                .unwrap_err(),
            AndroidGatewayEnqueueError::RequestInFlight
        );
    }

    #[test]
    fn set_chat_channel_is_retained_but_never_marked_sendable() {
        let mut queue = AndroidGatewayOutboundQueue::default();
        queue
            .enqueue(GatewayCommand::SetChatChannel {
                channel: "guild".into(),
            })
            .unwrap();
        let entry = queue
            .drain_ready(
                &shell(AndroidLifecycle::Foreground, AndroidNetwork::Available),
                1,
            )
            .pop()
            .unwrap();
        assert!(!entry.is_sendable());
        assert_eq!(
            entry.kind,
            AndroidGatewayOutboundKind::LocalOnly {
                reason: "BrowserCommand has no setChatChannel variant"
            }
        );
        assert_eq!(
            serde_json::from_str::<Value>(&entry.json).unwrap(),
            json!({"type":"setChatChannel","channel":"guild"})
        );
        assert_eq!(queue.status().local_only_count, 1);
    }

    #[test]
    fn oversized_mail_attachment_set_is_never_sent_or_silently_truncated() {
        let mut queue = AndroidGatewayOutboundQueue::default();
        queue
            .enqueue(GatewayCommand::SendMail {
                recipient: "Rita".into(),
                message: "hello".into(),
                gold: 0,
                attachment_unique_ids: vec![1, 2, 3, 4, 5, 6],
            })
            .unwrap();
        let entry = queue
            .drain_ready(
                &shell(AndroidLifecycle::Foreground, AndroidNetwork::Available),
                1,
            )
            .pop()
            .unwrap();
        assert!(!entry.is_sendable());
        assert_eq!(queue.status().local_only_count, 1);
        assert_eq!(
            serde_json::from_str::<Value>(&entry.json).unwrap(),
            json!({"type":"sendMailRejected","reason":"tooManyAttachments"})
        );
    }

    #[test]
    fn retry_connection_is_a_host_action_not_a_fabricated_client_version_packet() {
        let mut queue = AndroidGatewayOutboundQueue::default();
        queue.enqueue(GatewayCommand::RetryConnection).unwrap();
        let entry = queue
            .drain_ready(
                &shell(AndroidLifecycle::Foreground, AndroidNetwork::Available),
                1,
            )
            .pop()
            .unwrap();
        assert!(!entry.is_sendable());
        assert_eq!(
            entry.kind,
            AndroidGatewayOutboundKind::LocalOnly {
                reason: "the Android transport host must reopen the connection"
            }
        );
        assert_eq!(
            serde_json::from_str::<Value>(&entry.json).unwrap(),
            json!({"type":"retryConnection"})
        );
    }

    #[test]
    fn fifo_sequence_and_lifecycle_gate_preserve_commands() {
        let mut queue = AndroidGatewayOutboundQueue::with_capacity(3);
        queue.enqueue(GatewayCommand::TownRevive).unwrap();
        queue
            .enqueue(GatewayCommand::PickUp { object_id: 2 })
            .unwrap();
        assert!(
            queue
                .drain_ready(
                    &shell(AndroidLifecycle::Background, AndroidNetwork::Available),
                    2
                )
                .is_empty()
        );
        assert!(
            queue
                .drain_ready(
                    &shell(AndroidLifecycle::Foreground, AndroidNetwork::Unavailable),
                    2
                )
                .is_empty()
        );
        let entries = queue.drain_ready(
            &shell(AndroidLifecycle::Foreground, AndroidNetwork::Available),
            2,
        );
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            serde_json::from_str::<Value>(&entries[0].json).unwrap(),
            json!({"type":"townRevive"})
        );
        assert_eq!(
            serde_json::from_str::<Value>(&entries[1].json).unwrap(),
            json!({"type":"pickUp","objectId":2})
        );
    }

    #[test]
    fn overflow_is_rejected_and_observable_without_dropping_existing_fifo() {
        let mut queue = AndroidGatewayOutboundQueue::with_capacity(1);
        queue.enqueue(GatewayCommand::TownRevive).unwrap();
        let error = queue
            .enqueue(GatewayCommand::PickUp { object_id: 3 })
            .unwrap_err();
        assert_eq!(
            error,
            AndroidGatewayEnqueueError::Full {
                capacity: 1,
                command_type: "pickUp".into()
            }
        );
        assert_eq!(queue.status().overflow_count, 1);
        assert_eq!(queue.status().last_overflow_type.as_deref(), Some("pickUp"));
        assert_eq!(queue.len(), 1);
        assert_eq!(
            queue.drain_ready(
                &shell(AndroidLifecycle::Foreground, AndroidNetwork::Available),
                1
            )[0]
            .sequence,
            1
        );
    }
}

//! Android's reducer-to-wire boundary.
//!
//! This module owns no websocket and no Android SDK handle. It converts the
//! shared UI reducer's gateway effects into the same JSON objects accepted by
//! the Web/Windows BrowserCommand path, then retains them until an Activity or
//! websocket host explicitly drains the queue.

use std::{collections::VecDeque, fmt};

use bevy::prelude::Resource;
use mir2_ui_core::action::UiAction;
use mir2_ui_core::effect::{GatewayCommand, SecurityRequest, UiEffect};
use mir2_ui_core::game_shop::{
    GameShopReceipt, GameShopRequest, NATIVE_GAME_SHOP_RECEIPT_CAPABILITY,
};
use mir2_ui_core::reducer::reduce;
use mir2_ui_core::state::UiState;
use serde_json::{json, Value};

use crate::android_input::{AndroidLifecycle, AndroidNetwork, AndroidShellState};

pub const ANDROID_GATEWAY_QUEUE_CAPACITY: usize = 256;
pub const ANDROID_GATEWAY_INBOUND_CAPACITY: usize = 32;
/// A receipt is a small control message, not an arbitrary Android payload.
/// Keep this comfortably above the shared request/code limits while bounding
/// the JNI handoff before any JSON parser or Bevy system sees it.
pub const ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES: usize = 16 * 1024;
pub const ANDROID_GATEWAY_INBOUND_MAX_BYTES: usize = 128 * 1024;
/// These limits match the bounded Android change-password form's transport
/// envelope. They are enforced again at the platform boundary because a
/// caller can construct a shared `SecurityRequest` without going through the
/// renderer's text-input limits.
pub const ANDROID_CHANGE_PASSWORD_MAX_ACCOUNT_BYTES: usize = 32;
pub const ANDROID_CHANGE_PASSWORD_MAX_SECRET_BYTES: usize = 128;
pub const ANDROID_CHANGE_PASSWORD_SUCCESS_RESULT: i32 = 6;
const ANDROID_SECURITY_NOTICE_MAX_CHARS: usize = 256;
pub const ANDROID_GUILD_STORAGE_SLOT_COUNT: i32 = 112;
const ANDROID_MAX_PROTOCOL_SLOT: i32 = u8::MAX as i32;

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

/// The two guild-storage BrowserCommand variants currently implemented by
/// the gateway. `requestGuildStorage` is intentionally absent: the server
/// uses `guildStorageItemChange` with changeType 3 and coordinates 0,0 to
/// request the storage list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidGuildStorageCommand {
    GoldChange { change_type: u8, amount: u32 },
    ItemChange { change_type: u8, from: i32, to: i32 },
}

impl AndroidGuildStorageCommand {
    pub fn gold_change(change_type: u8, amount: u32) -> Option<Self> {
        (change_type <= 1 && amount > 0).then_some(Self::GoldChange {
            change_type,
            amount,
        })
    }

    pub fn item_change(change_type: u8, from: i32, to: i32) -> Option<Self> {
        let valid_slot = |slot: i32| (0..ANDROID_GUILD_STORAGE_SLOT_COUNT).contains(&slot);
        let valid_protocol_slot = |slot: i32| (0..=ANDROID_MAX_PROTOCOL_SLOT).contains(&slot);
        let valid = match change_type {
            0 => valid_protocol_slot(from) && valid_slot(to),
            1 => valid_slot(from) && valid_protocol_slot(to),
            2 => valid_slot(from) && valid_slot(to),
            3 => from == 0 && to == 0,
            _ => false,
        };
        valid.then_some(Self::ItemChange {
            change_type,
            from,
            to,
        })
    }

    fn command_type(self) -> &'static str {
        match self {
            Self::GoldChange { .. } => "guildStorageGoldChange",
            Self::ItemChange { .. } => "guildStorageItemChange",
        }
    }

    fn to_wire_value(self) -> Value {
        match self {
            Self::GoldChange {
                change_type,
                amount,
            } => json!({
                "type":"guildStorageGoldChange",
                "changeType":change_type,
                "amount":amount
            }),
            Self::ItemChange {
                change_type,
                from,
                to,
            } => json!({
                "type":"guildStorageItemChange",
                "changeType":change_type,
                "from":from,
                "to":to
            }),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AndroidGatewayOutbound {
    pub sequence: u64,
    pub kind: AndroidGatewayOutboundKind,
    pub json: String,
}

impl fmt::Debug for AndroidGatewayOutbound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::from_str::<Value>(&self.json)
            .map(|mut value| {
                redact_wire_secrets(&mut value);
                serde_json::to_string(&value).unwrap_or_else(|_| "[REDACTED]".to_owned())
            })
            .unwrap_or_else(|_| "[REDACTED]".to_owned());
        formatter
            .debug_struct("AndroidGatewayOutbound")
            .field("sequence", &self.sequence)
            .field("kind", &self.kind)
            .field("json", &json)
            .finish()
    }
}

fn redact_wire_secrets(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if matches!(
                    key.as_str(),
                    "password" | "currentPassword" | "newPassword" | "secretAnswer" | "credential"
                ) {
                    *child = Value::String("[REDACTED]".to_owned());
                } else {
                    redact_wire_secrets(child);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(redact_wire_secrets),
        _ => {}
    }
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
    InvalidGuildStorage {
        command_type: &'static str,
        reason: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AndroidSecurityAdapterError {
    InvalidField {
        field: &'static str,
        reason: &'static str,
    },
    Enqueue(AndroidGatewayEnqueueError),
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
    ChangePasswordResult {
        result: Option<i32>,
        banned: bool,
        message: String,
        wire_bytes: usize,
        exact_for_pending: bool,
    },
}

/// Safe, already-parsed representation of the server's password result. It
/// deliberately contains no request credentials and is the only representation
/// retained by the Android inbound queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidChangePasswordResult {
    pub result: Option<i32>,
    pub banned: bool,
    pub message: String,
}

impl AndroidChangePasswordResult {
    pub fn success(&self) -> bool {
        !self.banned && self.result == Some(ANDROID_CHANGE_PASSWORD_SUCCESS_RESULT)
    }
}

fn bounded_security_notice(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(ANDROID_SECURITY_NOTICE_MAX_CHARS)
        .collect()
}

fn change_password_result_message(
    result: Option<i32>,
    banned: bool,
    reason: Option<&str>,
) -> String {
    if banned {
        let reason = reason.map(bounded_security_notice).unwrap_or_default();
        return if reason.is_empty() {
            "Password change is temporarily blocked.".to_owned()
        } else {
            format!("Password change is temporarily blocked: {reason}")
        };
    }
    match result {
        Some(ANDROID_CHANGE_PASSWORD_SUCCESS_RESULT) => "Password changed successfully.".to_owned(),
        Some(1) => "The account ID is invalid.".to_owned(),
        Some(2) => "The current password is required.".to_owned(),
        Some(3) => "The new password is invalid.".to_owned(),
        Some(4) => "The account was not found.".to_owned(),
        Some(5) => "The current password is incorrect.".to_owned(),
        _ => "Password change failed.".to_owned(),
    }
}

/// Parse the gateway's packet envelope without retaining arbitrary inbound
/// JSON. The Android host may pass either normal packet responses or the
/// authoritative `ChangePasswordBanned` response, but unrelated packets are
/// rejected at this boundary.
pub fn parse_native_change_password_result(
    json_text: &str,
) -> Result<(AndroidChangePasswordResult, usize), String> {
    let wire_bytes = json_text.len();
    let root = serde_json::from_str::<Value>(json_text)
        .map_err(|_| "invalid change-password response JSON".to_owned())?;
    if root.get("type").and_then(Value::as_str) != Some("packet") {
        return Err("change-password response is not a packet envelope".to_owned());
    }
    let packet = root
        .get("packet")
        .and_then(Value::as_str)
        .ok_or_else(|| "change-password response packet is missing".to_owned())?;
    let payload = root
        .get("payload")
        .and_then(Value::as_object)
        .ok_or_else(|| "change-password response payload is missing".to_owned())?;
    let parsed = match packet {
        "ChangePassword" => {
            let result = payload
                .get("result")
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok());
            if result.is_none() {
                return Err("change-password result code is missing or invalid".to_owned());
            }
            AndroidChangePasswordResult {
                result,
                banned: false,
                message: change_password_result_message(result, false, None),
            }
        }
        "ChangePasswordBanned" => AndroidChangePasswordResult {
            result: None,
            banned: true,
            message: change_password_result_message(
                None,
                true,
                payload.get("reason").and_then(Value::as_str),
            ),
        },
        _ => return Err("unrelated packet passed to change-password adapter".to_owned()),
    };
    Ok((parsed, wire_bytes))
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
            Self::ChangePasswordResult { wire_bytes, .. } => *wire_bytes,
        }
    }

    fn matches_game_shop_request(&self, request: &GameShopRequest) -> bool {
        match self {
            Self::GameShopReceipt { json, .. } => parse_native_game_shop_receipt(json)
                .is_ok_and(|receipt| receipt.matches_request(request)),
            Self::ChangePasswordResult { .. } => false,
        }
    }

    fn is_change_password_result(&self) -> bool {
        matches!(self, Self::ChangePasswordResult { .. })
    }

    fn set_exact_for_pending(&mut self, exact: bool) {
        match self {
            Self::GameShopReceipt {
                exact_for_pending, ..
            } => *exact_for_pending = exact,
            Self::ChangePasswordResult {
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
            Self::ChangePasswordResult { .. } => unreachable!("wrong inbound message kind"),
        }
    }

    fn into_change_password_result(self) -> (AndroidChangePasswordResult, bool) {
        match self {
            Self::ChangePasswordResult {
                result,
                banned,
                message,
                exact_for_pending,
                ..
            } => (
                AndroidChangePasswordResult {
                    result,
                    banned,
                    message,
                },
                exact_for_pending,
            ),
            Self::GameShopReceipt { .. } => unreachable!("wrong inbound message kind"),
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
    InvalidChangePasswordResult {
        reason: String,
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
    pending_change_password: bool,
    exact_change_password_reserved: bool,
    change_password_overflow_pending: bool,
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
            pending_change_password: false,
            exact_change_password_reserved: false,
            change_password_overflow_pending: false,
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
            self.change_password_overflow_pending = self.should_mark_change_password_unknown();
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
            self.change_password_overflow_pending = self.should_mark_change_password_unknown();
            return Err(AndroidGatewayInboundEnqueueError::Full {
                capacity: self.capacity,
            });
        }
        let reserves_change_password = !self.exact_change_password_reserved
            && self.pending_change_password
            && message.is_change_password_result();
        let reserves_exact_receipt = !self.exact_receipt_reserved
            && self
                .pending_game_shop
                .as_ref()
                .is_some_and(|request| message.matches_game_shop_request(request));
        message.set_exact_for_pending(reserves_exact_receipt || reserves_change_password);
        self.queued_bytes = self.queued_bytes.saturating_add(message_bytes);
        self.entries.push_back(message);
        if reserves_exact_receipt {
            self.exact_receipt_reserved = true;
        }
        if reserves_change_password {
            self.exact_change_password_reserved = true;
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
        self.exact_change_password_reserved = false;
        self.entries.drain(..).collect()
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.entries.clear();
        self.queued_bytes = 0;
        self.exact_receipt_reserved = false;
        self.exact_change_password_reserved = false;
        self.change_password_overflow_pending = false;
    }

    fn take_overflow_pending(&mut self) -> bool {
        if self.exact_receipt_reserved {
            self.overflow_pending = false;
            return false;
        }
        std::mem::take(&mut self.overflow_pending)
    }

    fn take_change_password_overflow_pending(&mut self) -> bool {
        if self.exact_change_password_reserved {
            self.change_password_overflow_pending = false;
            return false;
        }
        std::mem::take(&mut self.change_password_overflow_pending)
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

    fn bind_change_password_pending(&mut self, pending: bool) {
        if self.pending_change_password != pending {
            self.pending_change_password = pending;
            self.exact_change_password_reserved = false;
            self.change_password_overflow_pending = false;
        }
    }

    fn clear_change_password_pending(&mut self) {
        self.pending_change_password = false;
        self.exact_change_password_reserved = false;
        self.change_password_overflow_pending = false;
    }

    fn should_mark_pending_unknown(&self) -> bool {
        self.pending_game_shop.is_some() && !self.exact_receipt_reserved
    }

    fn should_mark_change_password_unknown(&self) -> bool {
        self.pending_change_password && !self.exact_change_password_reserved
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

/// Public transport/JNI-host boundary for the authoritative password result.
/// Parsing happens before queue insertion, so no raw response (and certainly
/// no credential) is retained by the Android bridge.
pub fn enqueue_native_change_password_result(
    inbound: &mut AndroidGatewayInboundQueue,
    json_text: &str,
) -> Result<(), AndroidGatewayInboundEnqueueError> {
    if json_text.len() > ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES {
        return Err(AndroidGatewayInboundEnqueueError::MessageTooLarge {
            max_bytes: ANDROID_GATEWAY_INBOUND_MAX_MESSAGE_BYTES,
            actual_bytes: json_text.len(),
        });
    }
    let (result, wire_bytes) =
        parse_native_change_password_result(json_text).map_err(|reason| {
            AndroidGatewayInboundEnqueueError::InvalidChangePasswordResult { reason }
        })?;
    inbound.enqueue(AndroidGatewayInboundMessage::ChangePasswordResult {
        result: result.result,
        banned: result.banned,
        message: result.message,
        wire_bytes,
        exact_for_pending: false,
    })
}

fn validate_security_field(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), AndroidSecurityAdapterError> {
    if value.is_empty() {
        return Err(AndroidSecurityAdapterError::InvalidField {
            field,
            reason: "must not be empty",
        });
    }
    if value.len() > max_bytes {
        return Err(AndroidSecurityAdapterError::InvalidField {
            field,
            reason: "exceeds the bounded transport length",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(AndroidSecurityAdapterError::InvalidField {
            field,
            reason: "contains a control character",
        });
    }
    Ok(())
}

/// Convert the shared request-only security effect to the real gateway JSON
/// contract. The queue retains the request only in memory until it is drained;
/// its `Debug` representation redacts all three credential fields and no
/// persistence or logging is performed here.
pub fn enqueue_security_request(
    gateway: &mut AndroidGatewayOutboundQueue,
    inbound: &mut AndroidGatewayInboundQueue,
    request: SecurityRequest,
) -> Result<(), AndroidSecurityAdapterError> {
    gateway.enqueue_security_request(request)?;
    inbound.bind_change_password_pending(true);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AndroidQueuedSecurityOutcome {
    Applied,
    Unmatched,
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

fn consume_queued_native_change_password_result(
    ui_state: &mut UiState,
    gateway: &mut AndroidGatewayOutboundQueue,
    message: AndroidGatewayInboundMessage,
) -> AndroidQueuedSecurityOutcome {
    let (result, exact_for_pending) = message.into_change_password_result();
    if !exact_for_pending
        || !ui_state.security.change_password_pending
        || !gateway.change_password_in_flight()
    {
        return AndroidQueuedSecurityOutcome::Unmatched;
    }
    if !gateway.apply_change_password_result(&result) {
        return AndroidQueuedSecurityOutcome::Unmatched;
    }
    let transition = reduce(
        ui_state,
        UiAction::ChangePasswordResult {
            success: result.success(),
            message: result.message,
        },
    );
    if transition.state.security.change_password_pending {
        return AndroidQueuedSecurityOutcome::Unmatched;
    }
    *ui_state = transition.state;
    AndroidQueuedSecurityOutcome::Applied
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
    inbound.bind_change_password_pending(gateway.change_password_in_flight());
    if inbound.take_overflow_pending() {
        gateway.mark_game_shop_unknown();
        ui_state.mark_game_shop_unknown();
        inbound.clear_game_shop_pending();
    }
    if inbound.take_change_password_overflow_pending() {
        if ui_state.security.change_password_pending && gateway.change_password_in_flight() {
            gateway.mark_change_password_unknown();
            let transition = reduce(
                ui_state,
                UiAction::ChangePasswordResult {
                    success: false,
                    message: "Password change response was not received.".to_owned(),
                },
            );
            *ui_state = transition.state;
        }
        inbound.clear_change_password_pending();
    }

    for message in inbound.drain() {
        if message.is_change_password_result() {
            match consume_queued_native_change_password_result(ui_state, gateway, message) {
                AndroidQueuedSecurityOutcome::Applied => inbound.clear_change_password_pending(),
                AndroidQueuedSecurityOutcome::Unmatched => inbound.record_unmatched(),
            }
        } else {
            match consume_queued_native_game_shop_receipt(ui_state, gateway, message) {
                AndroidQueuedReceiptOutcome::Applied => inbound.clear_game_shop_pending(),
                AndroidQueuedReceiptOutcome::Unmatched => inbound.record_unmatched(),
                AndroidQueuedReceiptOutcome::Malformed => inbound.record_malformed(),
            }
        }
    }
}

/// Owner-level terminal reset seam. It exposes no raw message or eligibility
/// state and only prevents a receipt from qualifying against a closed session.
pub(crate) fn clear_bounded_inbound_transaction(inbound: &mut AndroidGatewayInboundQueue) {
    inbound.clear_game_shop_pending();
    inbound.clear_change_password_pending();
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
    change_password_in_flight: bool,
    change_password_unknown: bool,
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
            change_password_in_flight: false,
            change_password_unknown: false,
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

    /// Enqueue the shared request-only security effect as the gateway's
    /// `changePassword` BrowserCommand shape. A single in-flight request is
    /// retained across transport draining until its authoritative result is
    /// consumed, which prevents double taps from replaying credentials.
    pub fn enqueue_security_request(
        &mut self,
        request: SecurityRequest,
    ) -> Result<(), AndroidSecurityAdapterError> {
        if self.change_password_in_flight {
            return Err(AndroidSecurityAdapterError::Enqueue(
                AndroidGatewayEnqueueError::RequestInFlight,
            ));
        }
        let SecurityRequest::ChangePassword {
            account,
            old_password,
            new_password,
        } = &request;
        validate_security_field(
            "account",
            account,
            ANDROID_CHANGE_PASSWORD_MAX_ACCOUNT_BYTES,
        )?;
        validate_security_field(
            "oldPassword",
            old_password.as_str(),
            ANDROID_CHANGE_PASSWORD_MAX_SECRET_BYTES,
        )?;
        validate_security_field(
            "newPassword",
            new_password.as_str(),
            ANDROID_CHANGE_PASSWORD_MAX_SECRET_BYTES,
        )?;
        let command_type = "changePassword".to_owned();
        if self.entries.len() >= self.capacity {
            self.overflow_count = self.overflow_count.saturating_add(1);
            self.last_overflow_type = Some(command_type.clone());
            return Err(AndroidSecurityAdapterError::Enqueue(
                AndroidGatewayEnqueueError::Full {
                    capacity: self.capacity,
                    command_type,
                },
            ));
        }
        let value = security_request_to_wire(&request);
        self.entries.push_back(AndroidGatewayOutbound {
            sequence: self.next_sequence,
            kind: AndroidGatewayOutboundKind::Wire,
            json: serde_json::to_string(&value).expect("wire JSON values are serializable"),
        });
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.change_password_in_flight = true;
        self.change_password_unknown = false;
        Ok(())
    }

    pub(crate) fn change_password_in_flight(&self) -> bool {
        self.change_password_in_flight
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

    /// Enqueue the exact guild-storage BrowserCommand without requiring a
    /// shared UI enum change. Invalid change types, zero gold amounts, and
    /// out-of-range slots are rejected before touching the bounded FIFO.
    pub fn enqueue_guild_storage_gold_change(
        &mut self,
        change_type: u8,
        amount: u32,
    ) -> Result<(), AndroidGatewayEnqueueError> {
        let Some(command) = AndroidGuildStorageCommand::gold_change(change_type, amount) else {
            return Err(AndroidGatewayEnqueueError::InvalidGuildStorage {
                command_type: "guildStorageGoldChange",
                reason: "changeType must be 0 or 1 and amount must be non-zero",
            });
        };
        self.enqueue_guild_storage_command(command)
    }

    pub fn enqueue_guild_storage_item_change(
        &mut self,
        change_type: u8,
        from: i32,
        to: i32,
    ) -> Result<(), AndroidGatewayEnqueueError> {
        let Some(command) = AndroidGuildStorageCommand::item_change(change_type, from, to) else {
            return Err(AndroidGatewayEnqueueError::InvalidGuildStorage {
                command_type: "guildStorageItemChange",
                reason: "changeType or slot coordinates are outside the server contract",
            });
        };
        self.enqueue_guild_storage_command(command)
    }

    fn enqueue_guild_storage_command(
        &mut self,
        command: AndroidGuildStorageCommand,
    ) -> Result<(), AndroidGatewayEnqueueError> {
        let command_type = command.command_type();
        if self.entries.len() >= self.capacity {
            self.overflow_count = self.overflow_count.saturating_add(1);
            self.last_overflow_type = Some(command_type.to_owned());
            return Err(AndroidGatewayEnqueueError::Full {
                capacity: self.capacity,
                command_type: command_type.to_owned(),
            });
        }
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.entries.push_back(AndroidGatewayOutbound {
            sequence,
            kind: AndroidGatewayOutboundKind::Wire,
            json: serde_json::to_string(&command.to_wire_value())
                .expect("guild storage wire JSON is serializable"),
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

    fn apply_change_password_result(&mut self, _result: &AndroidChangePasswordResult) -> bool {
        if !self.change_password_in_flight {
            return false;
        }
        self.change_password_in_flight = false;
        self.change_password_unknown = false;
        self.entries
            .retain(|entry| !outbound_is_change_password(entry));
        true
    }

    pub(crate) fn mark_game_shop_unknown(&mut self) {
        if self.pending_game_shop.take().is_some() {
            self.game_shop_unknown = true;
        }
        self.entries.retain(|entry| !outbound_is_game_shop(entry));
    }

    pub(crate) fn mark_change_password_unknown(&mut self) {
        if self.change_password_in_flight {
            self.change_password_in_flight = false;
            self.change_password_unknown = true;
        }
        self.entries
            .retain(|entry| !outbound_is_change_password(entry));
    }

    pub(crate) fn mark_terminal_reset(&mut self) {
        self.mark_game_shop_unknown();
        self.mark_change_password_unknown();
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

fn outbound_is_change_password(entry: &AndroidGatewayOutbound) -> bool {
    serde_json::from_str::<Value>(&entry.json)
        .ok()
        .and_then(|value| value.get("type").and_then(Value::as_str).map(str::to_owned))
        .as_deref()
        == Some("changePassword")
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

fn security_request_to_wire(request: &SecurityRequest) -> Value {
    match request {
        SecurityRequest::ChangePassword {
            account,
            old_password,
            new_password,
        } => json!({
            "type": "changePassword",
            "accountId": account,
            "currentPassword": old_password.as_str(),
            "newPassword": new_password.as_str(),
        }),
    }
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

    fn change_password_request() -> SecurityRequest {
        SecurityRequest::ChangePassword {
            account: "demo".to_owned(),
            old_password: mir2_ui_core::effect::SecretText::new("old-secret"),
            new_password: mir2_ui_core::effect::SecretText::new("new-secret"),
        }
    }

    #[test]
    fn change_password_uses_real_gateway_shape_and_redacts_credentials() {
        let mut queue = AndroidGatewayOutboundQueue::default();
        let mut inbound = AndroidGatewayInboundQueue::default();
        enqueue_security_request(&mut queue, &mut inbound, change_password_request()).unwrap();
        let debug = format!("{queue:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("old-secret"));
        assert!(!debug.contains("new-secret"));

        let entry = queue
            .drain_ready(
                &shell(AndroidLifecycle::Foreground, AndroidNetwork::Available),
                1,
            )
            .pop()
            .unwrap();
        assert!(entry.is_sendable());
        assert_eq!(
            serde_json::from_str::<Value>(&entry.json).unwrap(),
            json!({
                "type": "changePassword",
                "accountId": "demo",
                "currentPassword": "old-secret",
                "newPassword": "new-secret"
            })
        );
        assert!(matches!(
            enqueue_security_request(&mut queue, &mut inbound, change_password_request()),
            Err(AndroidSecurityAdapterError::Enqueue(
                AndroidGatewayEnqueueError::RequestInFlight
            ))
        ));
    }

    #[test]
    fn change_password_request_is_bounded_without_echoing_secret_values() {
        let too_long = "x".repeat(ANDROID_CHANGE_PASSWORD_MAX_SECRET_BYTES + 1);
        let request = SecurityRequest::ChangePassword {
            account: "demo".to_owned(),
            old_password: mir2_ui_core::effect::SecretText::new(too_long),
            new_password: mir2_ui_core::effect::SecretText::new("new-secret"),
        };
        let mut queue = AndroidGatewayOutboundQueue::default();
        let mut inbound = AndroidGatewayInboundQueue::default();
        let error = enqueue_security_request(&mut queue, &mut inbound, request).unwrap_err();
        assert_eq!(
            error,
            AndroidSecurityAdapterError::InvalidField {
                field: "oldPassword",
                reason: "exceeds the bounded transport length"
            }
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn authoritative_change_password_result_reduces_shared_state_once() {
        use mir2_ui_core::state::{UiScreen, UiSecurityPanel};

        let mut state = UiState::default();
        state.screen = UiScreen::Login;
        state = reduce(&state, UiAction::ChangePassword).state;
        assert_eq!(state.security.panel, UiSecurityPanel::ChangePassword);
        let transition = reduce(
            &state,
            UiAction::SubmitChangePassword {
                account: "demo".to_owned(),
                old_password: mir2_ui_core::effect::SecretText::new("old-secret"),
                new_password: mir2_ui_core::effect::SecretText::new("new-secret"),
                confirm_password: mir2_ui_core::effect::SecretText::new("new-secret"),
            },
        );
        let request = match transition.effects.into_iter().next() {
            Some(UiEffect::SecurityRequest(request)) => request,
            other => panic!("expected security request, got {other:?}"),
        };
        state = transition.state;

        let mut outbound = AndroidGatewayOutboundQueue::default();
        let mut inbound = AndroidGatewayInboundQueue::default();
        enqueue_security_request(&mut outbound, &mut inbound, request).unwrap();
        enqueue_native_change_password_result(
            &mut inbound,
            r#"{"type":"packet","packet":"ChangePassword","payload":{"result":6}}"#,
        )
        .unwrap();
        drain_bounded_inbound_into_models(&mut inbound, &mut state, &mut outbound);

        assert!(!state.security.change_password_pending);
        assert_eq!(state.security.panel, UiSecurityPanel::None);
        assert!(!outbound.change_password_in_flight());

        // A duplicate authoritative packet cannot close or mutate a new
        // transaction because there is no matching in-flight request.
        enqueue_native_change_password_result(
            &mut inbound,
            r#"{"type":"packet","packet":"ChangePassword","payload":{"result":6}}"#,
        )
        .unwrap();
        drain_bounded_inbound_into_models(&mut inbound, &mut state, &mut outbound);
        assert!(!state.security.change_password_pending);
        assert_eq!(inbound.status().unmatched_count, 1);
    }

    #[test]
    fn change_password_result_parser_preserves_codes_and_bounds_banned_notice() {
        let parsed = parse_native_change_password_result(
            r#"{"type":"packet","packet":"ChangePassword","payload":{"result":5}}"#,
        )
        .unwrap()
        .0;
        assert_eq!(parsed.result, Some(5));
        assert!(!parsed.success());
        assert!(parsed.message.contains("incorrect"));

        let reason = "!".repeat(ANDROID_SECURITY_NOTICE_MAX_CHARS + 10);
        let banned = parse_native_change_password_result(&format!(
            r#"{{"type":"packet","packet":"ChangePasswordBanned","payload":{{"reason":"{reason}"}}}}"#
        ))
        .unwrap()
        .0;
        assert!(banned.banned);
        assert!(!banned.success());
        assert!(banned.message.chars().count() <= ANDROID_SECURITY_NOTICE_MAX_CHARS + 40);

        assert!(parse_native_change_password_result(
            r#"{"type":"packet","packet":"Login","payload":{"result":1}}"#
        )
        .is_err());
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
    fn guild_storage_commands_have_exact_browser_shapes_and_fail_closed() {
        let mut queue = AndroidGatewayOutboundQueue::with_capacity(4);
        queue
            .enqueue_guild_storage_gold_change(0, 250)
            .expect("guild gold command");
        queue
            .enqueue_guild_storage_item_change(2, 4, 7)
            .expect("guild item command");
        let entries = queue.drain_ready(
            &shell(AndroidLifecycle::Foreground, AndroidNetwork::Available),
            4,
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(
            serde_json::from_str::<Value>(&entries[0].json).unwrap(),
            json!({"type":"guildStorageGoldChange","changeType":0,"amount":250})
        );
        assert_eq!(
            serde_json::from_str::<Value>(&entries[1].json).unwrap(),
            json!({"type":"guildStorageItemChange","changeType":2,"from":4,"to":7})
        );

        let mut queue = AndroidGatewayOutboundQueue::with_capacity(2);
        assert_eq!(
            queue.enqueue_guild_storage_gold_change(2, 1),
            Err(AndroidGatewayEnqueueError::InvalidGuildStorage {
                command_type: "guildStorageGoldChange",
                reason: "changeType must be 0 or 1 and amount must be non-zero",
            })
        );
        assert_eq!(
            queue.enqueue_guild_storage_gold_change(0, 0),
            Err(AndroidGatewayEnqueueError::InvalidGuildStorage {
                command_type: "guildStorageGoldChange",
                reason: "changeType must be 0 or 1 and amount must be non-zero",
            })
        );
        assert_eq!(
            queue.enqueue_guild_storage_item_change(0, -1, 0),
            Err(AndroidGatewayEnqueueError::InvalidGuildStorage {
                command_type: "guildStorageItemChange",
                reason: "changeType or slot coordinates are outside the server contract",
            })
        );
        assert_eq!(
            queue.enqueue_guild_storage_item_change(0, 0, 112),
            Err(AndroidGatewayEnqueueError::InvalidGuildStorage {
                command_type: "guildStorageItemChange",
                reason: "changeType or slot coordinates are outside the server contract",
            })
        );
        assert!(queue.enqueue_guild_storage_item_change(3, 0, 0).is_ok());
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
        assert!(queue
            .drain_ready(
                &shell(AndroidLifecycle::Background, AndroidNetwork::Available),
                2
            )
            .is_empty());
        assert!(queue
            .drain_ready(
                &shell(AndroidLifecycle::Foreground, AndroidNetwork::Unavailable),
                2
            )
            .is_empty());
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

//! Native protocol contracts for the Windows gateway client.
//!
//! This module is intentionally transport-only:
//! - exact outbound JSON wire contracts for native-visible flow
//! - tolerant inbound envelope extraction for packet/error wrappers
//! - no Bevy wiring or gameplay side effects.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const INVALID_PACKET_KIND: &str = "invalid";

/// Crystal's guild storage has 112 server-side slots (0..=111). The
/// inventory side of a store/retrieve request is represented by an i32 in
/// BrowserCommand and is narrowed to the protocol's u8 slot domain here.
pub const GUILD_STORAGE_SLOT_COUNT: i32 = 112;
const MAX_PROTOCOL_SLOT: i32 = u8::MAX as i32;

fn parse_default_selected_item_index() -> i32 {
    -1
}

fn valid_guild_storage_slot(slot: i32) -> bool {
    (0..GUILD_STORAGE_SLOT_COUNT).contains(&slot)
}

fn valid_protocol_slot(slot: i32) -> bool {
    (0..=MAX_PROTOCOL_SLOT).contains(&slot)
}

/// Outbound BrowserCommand-compatible payloads used by the Windows visible flow.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NativeOutboundCommand {
    ClientVersion,
    ClientCapabilities {
        capabilities: Vec<String>,
    },
    ResumeSession {
        credential: String,
    },
    Login {
        #[serde(rename = "accountId")]
        account_id: String,
        password: String,
    },
    ChangePassword {
        #[serde(rename = "accountId")]
        account_id: String,
        #[serde(rename = "currentPassword")]
        current_password: String,
        #[serde(rename = "newPassword")]
        new_password: String,
    },
    NewAccount {
        #[serde(rename = "accountId")]
        account_id: String,
        password: String,
        #[serde(rename = "birthDateBinary")]
        birth_date_binary: i64,
        #[serde(rename = "userName")]
        user_name: String,
        #[serde(rename = "secretQuestion")]
        secret_question: String,
        #[serde(rename = "secretAnswer")]
        secret_answer: String,
        #[serde(rename = "emailAddress")]
        email_address: String,
    },
    NewCharacter {
        name: String,
        gender: String,
        class: String,
    },
    DeleteCharacter {
        #[serde(rename = "characterIndex")]
        character_index: i32,
    },
    StartGame {
        #[serde(rename = "characterIndex")]
        character_index: i32,
    },
    Walk {
        direction: String,
    },
    Run {
        direction: String,
    },
    Turn {
        direction: String,
    },
    Attack {
        #[serde(rename = "objectId")]
        object_id: u32,
    },
    /// Directional Crystal melee intent. The server resolves the actual
    /// weapon, damage, cooldown, range, and target interaction.
    AttackDirection {
        direction: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spell: Option<u8>,
    },
    /// Crystal ranged-attack intent. Coordinates and target identity are
    /// copied from the authoritative native read models; the server remains
    /// authoritative for class, equipment, skill, range, and damage rules.
    RangeAttack {
        direction: String,
        x: i32,
        y: i32,
        #[serde(rename = "targetId")]
        target_id: u32,
        #[serde(rename = "targetX")]
        target_x: i32,
        #[serde(rename = "targetY")]
        target_y: i32,
    },
    /// Crystal Alt+world-click harvest intent. The server validates the
    /// corpse, ownership, range, mount state and loot transfer.
    Harvest {
        direction: String,
    },
    PickUp {
        #[serde(rename = "objectId")]
        object_id: u32,
    },
    PickUpTile,
    Interact {
        #[serde(rename = "objectId")]
        object_id: u32,
    },
    SelectNpcDialog {
        target: String,
    },
    AcceptQuest {
        #[serde(rename = "npcIndex")]
        npc_index: u32,
        #[serde(rename = "questIndex")]
        quest_index: i32,
    },
    FinishQuest {
        #[serde(rename = "questIndex")]
        quest_index: i32,
        #[serde(
            rename = "selectedItemIndex",
            default = "parse_default_selected_item_index"
        )]
        selected_item_index: i32,
    },
    AbandonQuest {
        #[serde(rename = "questIndex")]
        quest_index: i32,
    },
    LogOut,
    Disconnect,
    TownRevive,
    UseItem {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        #[serde(rename = "uniqueId", default, skip_serializing_if = "Option::is_none")]
        unique_id: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        grid: Option<String>,
    },
    EquipItem {
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        grid: String,
        to: i32,
    },
    RemoveItem {
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        grid: String,
        to: i32,
    },
    DropItem {
        key: String,
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        count: u16,
        #[serde(rename = "heroInventory")]
        hero_inventory: bool,
    },
    RequestMapInfo {
        #[serde(rename = "mapIndex")]
        map_index: i32,
    },
    SearchMap {
        text: String,
    },
    TeleportToNpc {
        #[serde(rename = "objectId")]
        object_id: u32,
    },
    MoveItem {
        grid: String,
        from: i32,
        to: i32,
    },
    MergeItem {
        #[serde(rename = "gridFrom")]
        grid_from: String,
        #[serde(rename = "gridTo")]
        grid_to: String,
        #[serde(rename = "idFrom")]
        id_from: u64,
        #[serde(rename = "idTo")]
        id_to: u64,
    },
    SplitItem {
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        grid: String,
        count: u16,
    },
    Chat {
        message: String,
    },
    Magic {
        #[serde(rename = "objectId", default)]
        object_id: u32,
        spell: String,
        direction: String,
        #[serde(rename = "targetId", default)]
        target_id: u32,
        #[serde(default)]
        x: i32,
        #[serde(default)]
        y: i32,
        #[serde(rename = "spellTargetLock", default)]
        spell_target_lock: bool,
    },
    SpellToggle {
        spell: String,
        #[serde(rename = "toggleState")]
        toggle_state: i8,
    },
    BuyItem {
        #[serde(rename = "itemIndex")]
        item_index: u64,
        count: u16,
        #[serde(rename = "panelType", default)]
        panel_type: u8,
    },
    GameShopBuy {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "gIndex")]
        g_index: i32,
        quantity: u8,
        #[serde(rename = "priceType")]
        price_type: i32,
    },
    SellItem {
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        count: u16,
    },
    RepairItem {
        #[serde(rename = "uniqueId")]
        unique_id: u64,
    },
    SpecialRepairItem {
        #[serde(rename = "uniqueId")]
        unique_id: u64,
    },
    StoreItem {
        from: i32,
        to: i32,
    },
    TakeBackItem {
        from: i32,
        to: i32,
    },
    UnlockStorage {
        password: String,
    },
    SetStoragePassword {
        #[serde(rename = "currentPassword")]
        current_password: String,
        #[serde(rename = "newPassword")]
        new_password: String,
    },
    RemoveStoragePassword {
        #[serde(rename = "currentPassword")]
        current_password: String,
    },
    ReadMail {
        #[serde(rename = "mailId")]
        mail_id: u64,
    },
    CollectParcel {
        #[serde(rename = "mailId")]
        mail_id: u64,
    },
    DeleteMail {
        #[serde(rename = "mailId")]
        mail_id: u64,
    },
    SendMail {
        name: String,
        message: String,
        gold: u32,
        #[serde(rename = "itemsIdx")]
        items_idx: [u64; 5],
        #[serde(default, skip_deserializing)]
        stamped: bool,
    },
    SwitchGroup {
        #[serde(rename = "allowGroup")]
        allow_group: bool,
    },
    AddMember {
        name: String,
    },
    DelMember {
        name: String,
    },
    GroupInvite {
        #[serde(rename = "acceptInvite")]
        accept_invite: bool,
    },
    RequestGuildInfo {
        #[serde(rename = "infoType")]
        info_type: u8,
    },
    GuildStorageGoldChange {
        #[serde(rename = "changeType")]
        change_type: u8,
        amount: u32,
    },
    GuildStorageItemChange {
        #[serde(rename = "changeType")]
        change_type: u8,
        from: i32,
        to: i32,
    },
    EditGuildMember {
        #[serde(rename = "changeType")]
        change_type: u8,
        #[serde(rename = "rankIndex")]
        rank_index: u8,
        name: String,
        #[serde(rename = "rankName")]
        rank_name: String,
    },
    EditGuildNotice {
        notice: Vec<String>,
    },
    GuildInvite {
        #[serde(rename = "acceptInvite")]
        accept_invite: bool,
    },
    TradeRequest,
    TradeReply {
        #[serde(rename = "acceptInvite")]
        accept_invite: bool,
    },
    TradeGold {
        amount: u32,
    },
    DepositTradeItem {
        from: i32,
        to: i32,
    },
    RetrieveTradeItem {
        from: i32,
        to: i32,
    },
    TradeConfirm {
        locked: bool,
    },
    TradeCancel,
}

impl std::fmt::Debug for NativeOutboundCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Login { account_id, .. } | Self::NewAccount { account_id, .. } => formatter
                .debug_struct(self.command_type())
                .field("account_id", account_id)
                .field("password", &"<redacted>")
                .finish(),
            Self::ChangePassword { account_id, .. } => formatter
                .debug_struct(self.command_type())
                .field("account_id", account_id)
                .field("current_password", &"<redacted>")
                .field("new_password", &"<redacted>")
                .finish(),
            other => formatter
                .debug_struct("NativeOutboundCommand")
                .field("type", &other.command_type())
                .finish(),
        }
    }
}

impl NativeOutboundCommand {
    /// Build a guild gold mutation only for the two server-defined mutation
    /// types. Zero amounts are rejected so a malformed native caller cannot
    /// enqueue a meaningless request; the server remains authoritative for
    /// balance, rank, and safe-zone checks.
    pub fn guild_storage_gold_change(change_type: u8, amount: u32) -> Option<Self> {
        (change_type <= 1 && amount > 0).then_some(Self::GuildStorageGoldChange {
            change_type,
            amount,
        })
    }

    /// Build a guild item mutation using the server's exact change-type
    /// contract: 0 store, 1 retrieve, 2 move, 3 list. The list request uses
    /// the established zero coordinates. There is no separate
    /// `requestGuildStorage` BrowserCommand in this repository.
    pub fn guild_storage_item_change(change_type: u8, from: i32, to: i32) -> Option<Self> {
        let valid = match change_type {
            0 => valid_protocol_slot(from) && valid_guild_storage_slot(to),
            1 => valid_guild_storage_slot(from) && valid_protocol_slot(to),
            2 => valid_guild_storage_slot(from) && valid_guild_storage_slot(to),
            3 => from == 0 && to == 0,
            _ => false,
        };
        valid.then_some(Self::GuildStorageItemChange {
            change_type,
            from,
            to,
        })
    }

    pub fn command_type(&self) -> &'static str {
        match self {
            Self::ClientVersion => "clientVersion",
            Self::ClientCapabilities { .. } => "clientCapabilities",
            Self::ResumeSession { .. } => "resumeSession",
            Self::Login { .. } => "login",
            Self::ChangePassword { .. } => "changePassword",
            Self::NewAccount { .. } => "newAccount",
            Self::NewCharacter { .. } => "newCharacter",
            Self::DeleteCharacter { .. } => "deleteCharacter",
            Self::StartGame { .. } => "startGame",
            Self::Walk { .. } => "walk",
            Self::Run { .. } => "run",
            Self::Turn { .. } => "turn",
            Self::Attack { .. } => "attack",
            Self::AttackDirection { .. } => "attackDirection",
            Self::RangeAttack { .. } => "rangeAttack",
            Self::Harvest { .. } => "harvest",
            Self::PickUp { .. } => "pickUp",
            Self::PickUpTile => "pickUpTile",
            Self::Interact { .. } => "interact",
            Self::SelectNpcDialog { .. } => "selectNpcDialog",
            Self::AcceptQuest { .. } => "acceptQuest",
            Self::FinishQuest { .. } => "finishQuest",
            Self::AbandonQuest { .. } => "abandonQuest",
            Self::TownRevive => "townRevive",
            Self::UseItem { .. } => "useItem",
            Self::EquipItem { .. } => "equipItem",
            Self::RemoveItem { .. } => "removeItem",
            Self::DropItem { .. } => "dropItem",
            Self::RequestMapInfo { .. } => "requestMapInfo",
            Self::SearchMap { .. } => "searchMap",
            Self::TeleportToNpc { .. } => "teleportToNpc",
            Self::MoveItem { .. } => "moveItem",
            Self::MergeItem { .. } => "mergeItem",
            Self::SplitItem { .. } => "splitItem",
            Self::Chat { .. } => "chat",
            Self::LogOut => "logOut",
            Self::Disconnect => "disconnect",
            Self::Magic { .. } => "magic",
            Self::SpellToggle { .. } => "spellToggle",
            Self::BuyItem { .. } => "buyItem",
            Self::GameShopBuy { .. } => "gameShopBuy",
            Self::SellItem { .. } => "sellItem",
            Self::RepairItem { .. } => "repairItem",
            Self::SpecialRepairItem { .. } => "specialRepairItem",
            Self::StoreItem { .. } => "storeItem",
            Self::TakeBackItem { .. } => "takeBackItem",
            Self::UnlockStorage { .. } => "unlockStorage",
            Self::SetStoragePassword { .. } => "setStoragePassword",
            Self::RemoveStoragePassword { .. } => "removeStoragePassword",
            Self::ReadMail { .. } => "readMail",
            Self::CollectParcel { .. } => "collectParcel",
            Self::DeleteMail { .. } => "deleteMail",
            Self::SendMail { .. } => "sendMail",
            Self::SwitchGroup { .. } => "switchGroup",
            Self::AddMember { .. } => "addMember",
            Self::DelMember { .. } => "delMember",
            Self::GroupInvite { .. } => "groupInvite",
            Self::RequestGuildInfo { .. } => "requestGuildInfo",
            Self::GuildStorageGoldChange { .. } => "guildStorageGoldChange",
            Self::GuildStorageItemChange { .. } => "guildStorageItemChange",
            Self::EditGuildMember { .. } => "editGuildMember",
            Self::EditGuildNotice { .. } => "editGuildNotice",
            Self::GuildInvite { .. } => "guildInvite",
            Self::TradeRequest => "tradeRequest",
            Self::TradeReply { .. } => "tradeReply",
            Self::TradeGold { .. } => "tradeGold",
            Self::DepositTradeItem { .. } => "depositTradeItem",
            Self::RetrieveTradeItem { .. } => "retrieveTradeItem",
            Self::TradeConfirm { .. } => "tradeConfirm",
            Self::TradeCancel => "tradeCancel",
        }
    }

    /// Serialize exactly as wire JSON (camelCase tagged command contract).
    pub fn to_wire_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|error| {
            json!({
                "type": INVALID_PACKET_KIND,
                "message": format!("serialization_failed: {error}")
            })
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoginCharacter {
    pub index: Option<i64>,
    pub name: Option<String>,
    pub level: Option<i64>,
    pub class: Option<String>,
    pub gender: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoginSuccess {
    pub characters: Vec<LoginCharacter>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoginFailure {
    pub packet: String,
    pub result: Option<i32>,
    pub reason: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangePasswordResult {
    pub result: Option<i32>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangePasswordBanned {
    pub reason: Option<String>,
    pub expiry: Option<Value>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewAccountResult {
    pub result: Option<i32>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewCharacterSuccess {
    pub character: Option<Value>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeleteCharacterSuccess {
    pub character_index: Option<i32>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StartGameAck {
    pub result: Option<i32>,
    pub resolution: Option<Value>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserInformation {
    pub object_id: Option<i64>,
    pub name: Option<String>,
    pub class_name: Option<String>,
    pub gender: Option<String>,
    pub level: Option<i32>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewQuestInfo {
    pub quest_id: Option<i64>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangeQuest {
    pub quest_id: Option<i64>,
    pub state: Option<i64>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompleteQuest {
    pub completed_quests: Option<Value>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NPCResponse {
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Disconnect {
    pub reason: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErrorEvent {
    pub message: Option<String>,
    pub packet: Option<String>,
    pub payload: Value,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ResumeCredentialEvent {
    pub credential: String,
    pub expires_at_ms: Option<u64>,
    pub generation: Option<u64>,
}

impl std::fmt::Debug for ResumeCredentialEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResumeCredentialEvent")
            .field("credential", &"<redacted>")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("generation", &self.generation)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionResumedEvent {
    pub character_index: Option<i32>,
    pub generation: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeRejectedEvent {
    pub code: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldSnapshot {
    pub payload: Value,
}

/// Packet payloads that feed the native Big Map read model.  These types are
/// intentionally transport-only; the renderer never receives a destination
/// transform or a success acknowledgement from them.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMapInfo {
    pub map_index: i32,
    pub info: mir2_client_bevy::big_map::BigMapInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldMapSetup {
    pub enabled: bool,
    pub icons: Vec<mir2_client_bevy::big_map::BigMapWorldIcon>,
    pub teleport_to_npc_cost: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMapResult {
    pub map_index: i32,
    pub npc_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapIdentity {
    pub map_index: i32,
    pub location: Option<mir2_client_bevy::big_map::BigMapPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserLocation {
    pub location: mir2_client_bevy::big_map::BigMapPoint,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowObserve {
    pub allow: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PacketEvent {
    NewAccountResult(NewAccountResult),
    LoginSuccess(LoginSuccess),
    LoginFailure(LoginFailure),
    ChangePasswordResult(ChangePasswordResult),
    ChangePasswordBanned(ChangePasswordBanned),
    NewCharacterSuccess(NewCharacterSuccess),
    DeleteCharacterSuccess(DeleteCharacterSuccess),
    StartGameAck(StartGameAck),
    UserInformation(UserInformation),
    NPCResponse(NPCResponse),
    NewQuestInfo(NewQuestInfo),
    ChangeQuest(ChangeQuest),
    CompleteQuest(CompleteQuest),
    NewMapInfo(NewMapInfo),
    WorldMapSetup(WorldMapSetup),
    SearchMapResult(SearchMapResult),
    MapInformation(MapIdentity),
    MapChanged(MapIdentity),
    UserLocation(UserLocation),
    AllowObserve(AllowObserve),
    Disconnect(Disconnect),
    Other { packet: String, payload: Value },
    WorldSnapshot(WorldSnapshot),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InboundEvent {
    Packet(PacketEvent),
    Error(ErrorEvent),
    ResumeCredential(ResumeCredentialEvent),
    SessionResumed(SessionResumedEvent),
    ResumeRejected(ResumeRejectedEvent),
    GameShopReceipt(mir2_client_bevy::game_shop::GameShopReceipt),
    Unknown {
        event_type: String,
        payload: Value,
        packet: Option<String>,
    },
}

#[derive(Debug)]
pub enum ParseInboundError {
    InvalidJson(String),
    MissingEnvelopeField { field: &'static str, detail: String },
}

impl std::fmt::Display for ParseInboundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "invalid json: {error}"),
            Self::MissingEnvelopeField { field, detail } => {
                write!(f, "missing field '{field}' in event envelope: {detail}")
            }
        }
    }
}

impl std::error::Error for ParseInboundError {}

/// Parse and classify one raw websocket text frame.
pub fn parse_inbound_event(text: &str) -> Result<InboundEvent, ParseInboundError> {
    let value = serde_json::from_str::<Value>(text)
        .map_err(|error| ParseInboundError::InvalidJson(error.to_string()))?;
    parse_inbound_value(value)
}

/// Parse and classify one JSON envelope value.
pub fn parse_inbound_value(value: Value) -> Result<InboundEvent, ParseInboundError> {
    let event_type = match value.get("type").and_then(Value::as_str) {
        Some(event_type) => event_type.to_owned(),
        None => {
            return Err(ParseInboundError::MissingEnvelopeField {
                field: "type",
                detail: "top-level `type` is required".to_owned(),
            });
        }
    };

    let payload = value.get("payload").cloned().unwrap_or(Value::Null);
    let packet = value
        .get("packet")
        .and_then(Value::as_str)
        .map(str::to_owned);

    match event_type.as_str() {
        "packet" => parse_packet(packet.as_deref(), payload),
        "error" => Ok(InboundEvent::Error(ErrorEvent {
            message: payload
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    payload
                        .get("error")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                }),
            packet: packet.clone(),
            payload,
        })),
        "worldSnapshot" => Ok(InboundEvent::Packet(PacketEvent::WorldSnapshot(
            WorldSnapshot { payload },
        ))),
        "resumeCredential" => Ok(InboundEvent::ResumeCredential(ResumeCredentialEvent {
            credential: value
                .get("credential")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            expires_at_ms: coerce_u64(value.get("expiresAtMs")),
            generation: coerce_u64(value.get("generation")),
        })),
        "sessionResumed" => Ok(InboundEvent::SessionResumed(SessionResumedEvent {
            character_index: coerce_i32(value.get("characterIndex")),
            generation: coerce_u64(value.get("generation")),
        })),
        "resumeRejected" => Ok(InboundEvent::ResumeRejected(ResumeRejectedEvent {
            code: value.get("code").and_then(Value::as_str).map(str::to_owned),
        })),
        "gameShopReceipt" => {
            serde_json::from_value::<mir2_client_bevy::game_shop::GameShopReceipt>(value)
                .map(InboundEvent::GameShopReceipt)
                .map_err(|error| ParseInboundError::MissingEnvelopeField {
                    field: "gameShopReceipt",
                    detail: format!("invalid receipt: {error}"),
                })
        }
        _ => Ok(InboundEvent::Unknown {
            event_type,
            payload,
            packet,
        }),
    }
}

fn parse_packet(packet: Option<&str>, payload: Value) -> Result<InboundEvent, ParseInboundError> {
    let packet = match packet {
        Some(packet) => packet,
        None => {
            return Err(ParseInboundError::MissingEnvelopeField {
                field: "packet",
                detail: "`type: \"packet\" requires packet name".to_owned(),
            });
        }
    };

    match packet {
        "NewAccount" => Ok(InboundEvent::Packet(PacketEvent::NewAccountResult(
            NewAccountResult {
                result: coerce_i32(payload.get("result")),
                payload,
            },
        ))),
        "LoginSuccess" => Ok(InboundEvent::Packet(PacketEvent::LoginSuccess(
            LoginSuccess {
                characters: parse_login_characters(&payload),
                payload,
            },
        ))),
        "Login" | "LoginBanned" => Ok(InboundEvent::Packet(PacketEvent::LoginFailure(
            LoginFailure {
                packet: packet.to_owned(),
                result: coerce_i32(payload.get("result")),
                reason: payload
                    .get("reason")
                    .or_else(|| payload.get("reasonMessage"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                raw: payload,
            },
        ))),
        "ChangePassword" => Ok(InboundEvent::Packet(PacketEvent::ChangePasswordResult(
            ChangePasswordResult {
                result: coerce_i32(payload.get("result")),
                payload,
            },
        ))),
        "ChangePasswordBanned" => Ok(InboundEvent::Packet(PacketEvent::ChangePasswordBanned(
            ChangePasswordBanned {
                reason: payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                expiry: payload
                    .get("expiry")
                    .cloned()
                    .or_else(|| payload.get("expiryDate").cloned()),
                payload,
            },
        ))),
        "NewCharacterSuccess" => Ok(InboundEvent::Packet(PacketEvent::NewCharacterSuccess(
            NewCharacterSuccess {
                character: payload.get("character").cloned().or_else(|| {
                    payload
                        .get("charInfo")
                        .cloned()
                        .or_else(|| payload.get("characterInfo").cloned())
                }),
                payload,
            },
        ))),
        "DeleteCharacterSuccess" => Ok(InboundEvent::Packet(PacketEvent::DeleteCharacterSuccess(
            DeleteCharacterSuccess {
                character_index: coerce_i32(payload.get("characterIndex"))
                    .or_else(|| coerce_i32(payload.get("character")))
                    .or_else(|| coerce_i32(payload.get("index"))),
                payload,
            },
        ))),
        "StartGame" => Ok(InboundEvent::Packet(PacketEvent::StartGameAck(
            StartGameAck {
                result: coerce_i32(payload.get("result")),
                resolution: payload.get("resolution").cloned(),
                payload,
            },
        ))),
        "UserInformation" => Ok(InboundEvent::Packet(PacketEvent::UserInformation(
            UserInformation {
                object_id: coerce_i64(payload.get("objectId"))
                    .or_else(|| coerce_i64(payload.get("object_id"))),
                name: payload
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                class_name: payload
                    .get("class")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                gender: payload
                    .get("gender")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                level: coerce_i32(payload.get("level")),
                payload,
            },
        ))),
        "NPCResponse" => Ok(InboundEvent::Packet(PacketEvent::NPCResponse(
            NPCResponse { payload },
        ))),
        "NewQuestInfo" => Ok(InboundEvent::Packet(PacketEvent::NewQuestInfo(
            NewQuestInfo {
                quest_id: coerce_i64(payload.get("id"))
                    .or_else(|| coerce_i64(payload.get("questId"))),
                payload,
            },
        ))),
        "ChangeQuest" => Ok(InboundEvent::Packet(PacketEvent::ChangeQuest(
            ChangeQuest {
                quest_id: coerce_i64(payload.get("questId"))
                    .or_else(|| coerce_i64(payload.get("quest_id")))
                    .or_else(|| coerce_i64(payload.get("id"))),
                state: coerce_i64(payload.get("state"))
                    .or_else(|| coerce_i64(payload.get("questState"))),
                payload,
            },
        ))),
        "CompleteQuest" => Ok(InboundEvent::Packet(PacketEvent::CompleteQuest(
            CompleteQuest {
                completed_quests: payload
                    .get("completedQuests")
                    .cloned()
                    .or_else(|| payload.get("completed_quests").cloned()),
                payload,
            },
        ))),
        "NewMapInfo" => parse_new_map_info(payload),
        "WorldMapSetup" => parse_world_map_setup(payload),
        "SearchMapResult" => parse_search_map_result(payload),
        "MapInformation" => parse_map_identity(payload, false),
        "MapChanged" => parse_map_identity(payload, true),
        "UserLocation" => parse_user_location(payload),
        "AllowObserve" => {
            let allow = payload
                .get("allow")
                .and_then(Value::as_bool)
                .ok_or_else(|| ParseInboundError::MissingEnvelopeField {
                    field: "AllowObserve.allow",
                    detail: "allow must be a boolean".to_owned(),
                })?;
            Ok(InboundEvent::Packet(PacketEvent::AllowObserve(
                AllowObserve { allow },
            )))
        }
        "Disconnect" => Ok(InboundEvent::Packet(PacketEvent::Disconnect(Disconnect {
            reason: payload
                .get("reason")
                .and_then(Value::as_str)
                .map(str::to_owned),
            raw: payload,
        }))),
        _ => Ok(InboundEvent::Packet(PacketEvent::Other {
            packet: packet.to_owned(),
            payload,
        })),
    }
}

fn parse_new_map_info(payload: Value) -> Result<InboundEvent, ParseInboundError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        map_index: i32,
        info: mir2_client_bevy::big_map::BigMapInfo,
    }

    let decoded = serde_json::from_value::<Payload>(payload.clone()).map_err(|error| {
        ParseInboundError::MissingEnvelopeField {
            field: "NewMapInfo",
            detail: format!("invalid payload: {error}"),
        }
    })?;
    if decoded.map_index <= 0 {
        return Err(ParseInboundError::MissingEnvelopeField {
            field: "NewMapInfo.mapIndex",
            detail: "mapIndex must be positive".to_owned(),
        });
    }
    Ok(InboundEvent::Packet(PacketEvent::NewMapInfo(NewMapInfo {
        map_index: decoded.map_index,
        info: decoded.info,
    })))
}

fn parse_world_map_setup(payload: Value) -> Result<InboundEvent, ParseInboundError> {
    #[derive(Default, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Setup {
        #[serde(default)]
        enabled: bool,
        #[serde(default)]
        icons: Vec<mir2_client_bevy::big_map::BigMapWorldIcon>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        #[serde(default)]
        setup: Setup,
        #[serde(default)]
        teleport_to_npc_cost: i32,
    }

    let decoded = serde_json::from_value::<Payload>(payload).map_err(|error| {
        ParseInboundError::MissingEnvelopeField {
            field: "WorldMapSetup",
            detail: format!("invalid payload: {error}"),
        }
    })?;
    Ok(InboundEvent::Packet(PacketEvent::WorldMapSetup(
        WorldMapSetup {
            enabled: decoded.setup.enabled,
            icons: decoded.setup.icons,
            teleport_to_npc_cost: decoded.teleport_to_npc_cost,
        },
    )))
}

fn parse_search_map_result(payload: Value) -> Result<InboundEvent, ParseInboundError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        map_index: i32,
        #[serde(default)]
        npc_index: u32,
    }
    let decoded = serde_json::from_value::<Payload>(payload).map_err(|error| {
        ParseInboundError::MissingEnvelopeField {
            field: "SearchMapResult",
            detail: format!("invalid payload: {error}"),
        }
    })?;
    if decoded.map_index < -1 {
        return Err(ParseInboundError::MissingEnvelopeField {
            field: "SearchMapResult.mapIndex",
            detail: "mapIndex must be -1 or positive".to_owned(),
        });
    }
    Ok(InboundEvent::Packet(PacketEvent::SearchMapResult(
        SearchMapResult {
            map_index: decoded.map_index,
            npc_index: decoded.npc_index,
        },
    )))
}

fn parse_map_identity(payload: Value, changed: bool) -> Result<InboundEvent, ParseInboundError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        map_index: i32,
        #[serde(default)]
        location: Option<mir2_client_bevy::big_map::BigMapPoint>,
    }
    let decoded = serde_json::from_value::<Payload>(payload).map_err(|error| {
        ParseInboundError::MissingEnvelopeField {
            field: if changed {
                "MapChanged"
            } else {
                "MapInformation"
            },
            detail: format!("invalid payload: {error}"),
        }
    })?;
    if decoded.map_index <= 0 {
        return Err(ParseInboundError::MissingEnvelopeField {
            field: if changed {
                "MapChanged.mapIndex"
            } else {
                "MapInformation.mapIndex"
            },
            detail: "mapIndex must be positive".to_owned(),
        });
    }
    let identity = MapIdentity {
        map_index: decoded.map_index,
        location: decoded.location,
    };
    Ok(InboundEvent::Packet(if changed {
        PacketEvent::MapChanged(identity)
    } else {
        PacketEvent::MapInformation(identity)
    }))
}

fn parse_user_location(payload: Value) -> Result<InboundEvent, ParseInboundError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        x: i32,
        y: i32,
        #[serde(default)]
        direction: Option<String>,
    }
    let decoded = serde_json::from_value::<Payload>(payload).map_err(|error| {
        ParseInboundError::MissingEnvelopeField {
            field: "UserLocation",
            detail: format!("invalid payload: {error}"),
        }
    })?;
    Ok(InboundEvent::Packet(PacketEvent::UserLocation(
        UserLocation {
            location: mir2_client_bevy::big_map::BigMapPoint {
                x: decoded.x,
                y: decoded.y,
            },
            direction: decoded.direction,
        },
    )))
}

fn parse_login_characters(payload: &Value) -> Vec<LoginCharacter> {
    payload
        .get("characters")
        .and_then(Value::as_array)
        .map(|list| list.iter().map(parse_login_character).collect())
        .unwrap_or_default()
}

fn parse_login_character(value: &Value) -> LoginCharacter {
    LoginCharacter {
        index: coerce_i64(value.get("index")).or_else(|| coerce_i64(value.get("characterIndex"))),
        name: value.get("name").and_then(Value::as_str).map(str::to_owned),
        level: coerce_i64(value.get("level")),
        class: value
            .get("class")
            .and_then(Value::as_str)
            .map(str::to_owned),
        gender: value
            .get("gender")
            .and_then(Value::as_str)
            .map(str::to_owned),
        raw: value.clone(),
    }
}

fn coerce_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(num)) => num.as_i64(),
        Some(Value::String(s)) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn coerce_i32(value: Option<&Value>) -> Option<i32> {
    coerce_i64(value).and_then(|v| i32::try_from(v).ok())
}

fn coerce_u64(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(num)) => num.as_u64(),
        Some(Value::String(s)) => s.parse::<u64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_serialized(command: NativeOutboundCommand, expected: Value) {
        assert_eq!(
            serde_json::to_value(command).expect("serialize command"),
            expected
        );
    }

    #[test]
    fn outbound_commands_serialize_with_camel_case_tagged_wire_contract() {
        assert_serialized(
            NativeOutboundCommand::ClientVersion,
            json!({"type":"clientVersion"}),
        );
        assert_serialized(
            NativeOutboundCommand::ClientCapabilities {
                capabilities: vec!["nativeResumeV1".into(), "nativeGameShopReceiptV1".into()],
            },
            json!({"type":"clientCapabilities","capabilities":["nativeResumeV1","nativeGameShopReceiptV1"]}),
        );
        assert_serialized(
            NativeOutboundCommand::ResumeSession {
                credential: "A".repeat(43),
            },
            json!({"type":"resumeSession","credential":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        );
        assert_serialized(
            NativeOutboundCommand::Login {
                account_id: "demo".into(),
                password: "secret".into(),
            },
            json!({"type":"login","accountId":"demo","password":"secret"}),
        );
        assert_serialized(
            NativeOutboundCommand::ChangePassword {
                account_id: "demo".into(),
                current_password: "old-secret".into(),
                new_password: "new-secret".into(),
            },
            json!({
                "type":"changePassword",
                "accountId":"demo",
                "currentPassword":"old-secret",
                "newPassword":"new-secret"
            }),
        );
        assert_serialized(
            NativeOutboundCommand::NewAccount {
                account_id: "fresh".into(),
                password: "secret".into(),
                birth_date_binary: 0,
                user_name: "fresh".into(),
                secret_question: String::new(),
                secret_answer: String::new(),
                email_address: String::new(),
            },
            json!({
                "type":"newAccount",
                "accountId":"fresh",
                "password":"secret",
                "birthDateBinary":0,
                "userName":"fresh",
                "secretQuestion":"",
                "secretAnswer":"",
                "emailAddress":""
            }),
        );
        assert_serialized(
            NativeOutboundCommand::NewCharacter {
                name: "Scion".into(),
                gender: "male".into(),
                class: "warrior".into(),
            },
            json!({"type":"newCharacter","name":"Scion","gender":"male","class":"warrior"}),
        );
        assert_serialized(
            NativeOutboundCommand::DeleteCharacter { character_index: 3 },
            json!({"type":"deleteCharacter","characterIndex":3}),
        );
        assert_serialized(
            NativeOutboundCommand::StartGame { character_index: 1 },
            json!({"type":"startGame","characterIndex":1}),
        );
        assert_serialized(
            NativeOutboundCommand::GameShopBuy {
                request_id: "gs-0000000000000001".into(),
                g_index: 105,
                quantity: 3,
                price_type: 0,
            },
            json!({"type":"gameShopBuy","requestId":"gs-0000000000000001","gIndex":105,"quantity":3,"priceType":0}),
        );
        assert_serialized(
            NativeOutboundCommand::SendMail {
                name: "Receiver".into(),
                message: "Hello".into(),
                gold: 100,
                items_idx: [11, 22, 0, 0, 0],
                stamped: false,
            },
            json!({
                "type":"sendMail",
                "name":"Receiver",
                "message":"Hello",
                "gold":100,
                "itemsIdx":[11,22,0,0,0],
                "stamped":false
            }),
        );
        assert_serialized(
            NativeOutboundCommand::Walk {
                direction: "up".into(),
            },
            json!({"type":"walk","direction":"up"}),
        );
        assert_serialized(
            NativeOutboundCommand::Run {
                direction: "left".into(),
            },
            json!({"type":"run","direction":"left"}),
        );
        assert_serialized(
            NativeOutboundCommand::Turn {
                direction: "down".into(),
            },
            json!({"type":"turn","direction":"down"}),
        );
        assert_serialized(
            NativeOutboundCommand::Attack { object_id: 5000 },
            json!({"type":"attack","objectId":5000}),
        );
        assert_serialized(
            NativeOutboundCommand::AttackDirection {
                direction: "downleft".into(),
                spell: None,
            },
            json!({"type":"attackDirection","direction":"downleft"}),
        );
        assert_serialized(
            NativeOutboundCommand::AttackDirection {
                direction: "right".into(),
                spell: Some(0),
            },
            json!({"type":"attackDirection","direction":"right","spell":0}),
        );
        assert_serialized(
            NativeOutboundCommand::RangeAttack {
                direction: "up".into(),
                x: 100,
                y: 200,
                target_id: 3001,
                target_x: 104,
                target_y: 198,
            },
            json!({
                "type":"rangeAttack",
                "direction":"up",
                "x":100,
                "y":200,
                "targetId":3001,
                "targetX":104,
                "targetY":198
            }),
        );
        assert_serialized(
            NativeOutboundCommand::Harvest {
                direction: "downright".into(),
            },
            json!({"type":"harvest","direction":"downright"}),
        );
        assert_serialized(
            NativeOutboundCommand::PickUp { object_id: 7001 },
            json!({"type":"pickUp","objectId":7001}),
        );
        assert_serialized(
            NativeOutboundCommand::PickUpTile,
            json!({"type":"pickUpTile"}),
        );
        assert_serialized(
            NativeOutboundCommand::Interact { object_id: 3001 },
            json!({"type":"interact","objectId":3001}),
        );
        assert_serialized(
            NativeOutboundCommand::SelectNpcDialog {
                target: "@Buy".into(),
            },
            json!({"type":"selectNpcDialog","target":"@Buy"}),
        );
        assert_serialized(
            NativeOutboundCommand::AcceptQuest {
                npc_index: 2,
                quest_index: 77,
            },
            json!({"type":"acceptQuest","npcIndex":2,"questIndex":77}),
        );
        assert_serialized(
            NativeOutboundCommand::FinishQuest {
                quest_index: 77,
                selected_item_index: 1,
            },
            json!({"type":"finishQuest","questIndex":77,"selectedItemIndex":1}),
        );
        assert_serialized(
            NativeOutboundCommand::AbandonQuest { quest_index: 77 },
            json!({"type":"abandonQuest","questIndex":77}),
        );
        assert_serialized(
            NativeOutboundCommand::Magic {
                object_id: 0,
                spell: "FireBall".into(),
                direction: "down".into(),
                target_id: 2001,
                x: 12,
                y: 10,
                spell_target_lock: true,
            },
            json!({
                "type":"magic",
                "objectId":0,
                "spell":"FireBall",
                "direction":"down",
                "targetId":2001,
                "x":12,
                "y":10,
                "spellTargetLock":true
            }),
        );
        assert_serialized(
            NativeOutboundCommand::SpellToggle {
                spell: "FlamingSword".into(),
                toggle_state: 1,
            },
            json!({
                "type":"spellToggle",
                "spell":"FlamingSword",
                "toggleState":1
            }),
        );
        assert_serialized(NativeOutboundCommand::LogOut, json!({"type":"logOut"}));
        assert_serialized(
            NativeOutboundCommand::Disconnect,
            json!({"type":"disconnect"}),
        );
        assert_serialized(
            NativeOutboundCommand::TownRevive,
            json!({"type":"townRevive"}),
        );
        assert_serialized(
            NativeOutboundCommand::UseItem {
                key: None,
                unique_id: None,
                slot: Some(2),
                grid: Some("belt".into()),
            },
            json!({"type":"useItem","slot":2,"grid":"belt"}),
        );
        assert_serialized(
            NativeOutboundCommand::EquipItem {
                unique_id: 1112,
                grid: "inventory".into(),
                to: 4,
            },
            json!({"type":"equipItem","uniqueId":1112,"grid":"inventory","to":4}),
        );
        assert_serialized(
            NativeOutboundCommand::RemoveItem {
                unique_id: 42,
                grid: "equipment".into(),
                to: -1,
            },
            json!({"type":"removeItem","uniqueId":42,"grid":"equipment","to":-1}),
        );
        assert_serialized(
            NativeOutboundCommand::DropItem {
                key: "small-hp-drug".into(),
                unique_id: 7001,
                count: 3,
                hero_inventory: false,
            },
            json!({
                "type":"dropItem",
                "key":"small-hp-drug",
                "uniqueId":7001,
                "count":3,
                "heroInventory":false
            }),
        );
        assert_serialized(
            NativeOutboundCommand::MoveItem {
                grid: "inventory".into(),
                from: 4,
                to: 9,
            },
            json!({"type":"moveItem","grid":"inventory","from":4,"to":9}),
        );
        assert_serialized(
            NativeOutboundCommand::MergeItem {
                grid_from: "inventory".into(),
                grid_to: "storage".into(),
                id_from: 7001,
                id_to: 8002,
            },
            json!({
                "type":"mergeItem",
                "gridFrom":"inventory",
                "gridTo":"storage",
                "idFrom":7001,
                "idTo":8002
            }),
        );
        assert_serialized(
            NativeOutboundCommand::SplitItem {
                unique_id: 7001,
                grid: "inventory".into(),
                count: 2,
            },
            json!({
                "type":"splitItem",
                "uniqueId":7001,
                "grid":"inventory",
                "count":2
            }),
        );
        assert_serialized(
            NativeOutboundCommand::Chat {
                message: "hello".into(),
            },
            json!({"type":"chat","message":"hello"}),
        );
    }

    #[test]
    fn inbound_game_shop_receipt_uses_shared_contract_and_rejects_bad_shape() {
        let valid = parse_inbound_value(json!({
            "type": "gameShopReceipt",
            "protocol": "nativeGameShopReceiptV1",
            "requestId": "gs-1",
            "success": true,
            "gIndex": 31,
            "quantity": 2,
            "priceType": 1,
            "mailId": 1842,
            "newStockLevel": 3
        }))
        .expect("valid receipt parses");
        assert!(matches!(
            valid,
            InboundEvent::GameShopReceipt(receipt) if receipt.is_valid()
        ));

        let malformed = parse_inbound_value(json!({
            "type": "gameShopReceipt",
            "protocol": "nativeGameShopReceiptV1",
            "requestId": "gs-1",
            "success": true,
            "gIndex": 31,
            "quantity": 2,
            "priceType": 1,
            "mailId": 1842,
            "code": "commitFailed"
        }))
        .expect("shape-valid JSON still parses as typed receipt");
        assert!(matches!(
            malformed,
            InboundEvent::GameShopReceipt(receipt) if !receipt.is_valid()
        ));
    }

    #[test]
    fn allow_observe_packet_is_typed_and_requires_a_boolean() {
        let parsed = parse_inbound_event(
            r#"{"type":"packet","packet":"AllowObserve","payload":{"allow":true}}"#,
        )
        .expect("typed AllowObserve packet");
        assert!(matches!(
            parsed,
            InboundEvent::Packet(PacketEvent::AllowObserve(AllowObserve { allow: true }))
        ));

        assert!(
            parse_inbound_event(
                r#"{"type":"packet","packet":"AllowObserve","payload":{"allow":"true"}}"#,
            )
            .is_err(),
            "a string must not be accepted as authoritative boolean state"
        );
    }

    #[test]
    fn social_commands_use_only_real_typed_wire_shapes() {
        assert_serialized(
            NativeOutboundCommand::SwitchGroup { allow_group: false },
            json!({"type":"switchGroup","allowGroup":false}),
        );
        assert_serialized(
            NativeOutboundCommand::EditGuildMember {
                change_type: 1,
                rank_index: 2,
                name: "Miner".into(),
                rank_name: String::new(),
            },
            json!({"type":"editGuildMember","changeType":1,"rankIndex":2,"name":"Miner","rankName":""}),
        );
        assert_serialized(
            NativeOutboundCommand::GuildStorageGoldChange {
                change_type: 0,
                amount: 250,
            },
            json!({"type":"guildStorageGoldChange","changeType":0,"amount":250}),
        );
        assert_serialized(
            NativeOutboundCommand::GuildStorageItemChange {
                change_type: 2,
                from: 4,
                to: 7,
            },
            json!({"type":"guildStorageItemChange","changeType":2,"from":4,"to":7}),
        );
        assert_serialized(
            NativeOutboundCommand::EditGuildNotice {
                notice: vec!["Notice".into()],
            },
            json!({"type":"editGuildNotice","notice":["Notice"]}),
        );
        assert_serialized(
            NativeOutboundCommand::TradeGold { amount: 100 },
            json!({"type":"tradeGold","amount":100}),
        );
        assert_serialized(
            NativeOutboundCommand::DepositTradeItem { from: 4, to: 0 },
            json!({"type":"depositTradeItem","from":4,"to":0}),
        );
        assert_serialized(
            NativeOutboundCommand::TradeConfirm { locked: true },
            json!({"type":"tradeConfirm","locked":true}),
        );
        assert_serialized(
            NativeOutboundCommand::TradeCancel,
            json!({"type":"tradeCancel"}),
        );
    }

    #[test]
    fn guild_storage_command_builders_fail_closed_at_native_boundary() {
        assert!(NativeOutboundCommand::guild_storage_gold_change(0, 1).is_some());
        assert!(NativeOutboundCommand::guild_storage_gold_change(1, u32::MAX).is_some());
        assert!(NativeOutboundCommand::guild_storage_gold_change(2, 1).is_none());
        assert!(NativeOutboundCommand::guild_storage_gold_change(0, 0).is_none());

        assert!(NativeOutboundCommand::guild_storage_item_change(0, 255, 111).is_some());
        assert!(NativeOutboundCommand::guild_storage_item_change(1, 111, 255).is_some());
        assert!(NativeOutboundCommand::guild_storage_item_change(2, 111, 0).is_some());
        assert!(NativeOutboundCommand::guild_storage_item_change(3, 0, 0).is_some());
        assert!(NativeOutboundCommand::guild_storage_item_change(0, -1, 0).is_none());
        assert!(NativeOutboundCommand::guild_storage_item_change(0, 0, 112).is_none());
        assert!(NativeOutboundCommand::guild_storage_item_change(2, 112, 0).is_none());
        assert!(NativeOutboundCommand::guild_storage_item_change(4, 0, 0).is_none());
    }

    #[test]
    fn big_map_commands_match_gateway_browser_command_wire_contract() {
        assert_serialized(
            NativeOutboundCommand::RequestMapInfo { map_index: 34 },
            json!({"type":"requestMapInfo","mapIndex":34}),
        );
        assert_serialized(
            NativeOutboundCommand::SearchMap {
                text: "Natural Cave".into(),
            },
            json!({"type":"searchMap","text":"Natural Cave"}),
        );
        assert_serialized(
            NativeOutboundCommand::TeleportToNpc { object_id: 77 },
            json!({"type":"teleportToNpc","objectId":77}),
        );
    }

    #[test]
    fn big_map_packets_are_typed_bounded_and_reject_invalid_identity() {
        let new_info = parse_inbound_event(
            r#"{"type":"packet","packet":"NewMapInfo","payload":{"mapIndex":34,"info":{"title":"Natural Cave","width":120,"height":220,"bigMap":9,"movements":[{"destination":35,"title":"Exit","location":{"x":1,"y":2},"icon":3}],"npcs":[{"index":2,"fileName":"NPC/00","name":"Guide","mapIndex":34,"location":{"x":3,"y":4},"objectId":77,"showOnBigMap":true,"canTeleportTo":true}]}}}"#,
        )
        .expect("typed NewMapInfo");
        match new_info {
            InboundEvent::Packet(PacketEvent::NewMapInfo(info)) => {
                assert_eq!(info.map_index, 34);
                assert_eq!(info.info.big_map, 9);
                assert_eq!(info.info.npcs[0].object_id, 77);
            }
            other => panic!("unexpected packet: {other:?}"),
        }

        let setup = parse_inbound_event(
            r#"{"type":"packet","packet":"WorldMapSetup","payload":{"setup":{"enabled":true,"icons":[{"imageIndex":2,"title":"Bichon","mapIndex":34}]},"teleportToNpcCost":3000}}"#,
        )
        .expect("typed WorldMapSetup");
        assert!(matches!(
            setup,
            InboundEvent::Packet(PacketEvent::WorldMapSetup(WorldMapSetup {
                enabled: true,
                teleport_to_npc_cost: 3_000,
                ..
            }))
        ));

        let result = parse_inbound_event(
            r#"{"type":"packet","packet":"SearchMapResult","payload":{"mapIndex":34,"npcIndex":77}}"#,
        )
        .expect("typed SearchMapResult");
        assert!(matches!(
            result,
            InboundEvent::Packet(PacketEvent::SearchMapResult(SearchMapResult {
                map_index: 34,
                npc_index: 77
            }))
        ));

        let map_changed = parse_inbound_event(
            r#"{"type":"packet","packet":"MapChanged","payload":{"mapIndex":34,"location":{"x":40,"y":41}}}"#,
        )
        .expect("typed MapChanged");
        assert!(matches!(
            map_changed,
            InboundEvent::Packet(PacketEvent::MapChanged(MapIdentity {
                map_index: 34,
                location: Some(mir2_client_bevy::big_map::BigMapPoint { x: 40, y: 41 })
            }))
        ));

        let location = parse_inbound_event(
            r#"{"type":"packet","packet":"UserLocation","payload":{"x":41,"y":42,"direction":"Right"}}"#,
        )
        .expect("typed UserLocation");
        assert!(matches!(
            location,
            InboundEvent::Packet(PacketEvent::UserLocation(UserLocation {
                location: mir2_client_bevy::big_map::BigMapPoint { x: 41, y: 42 },
                direction: Some(direction),
            })) if direction == "Right"
        ));

        assert!(parse_inbound_event(
            r#"{"type":"packet","packet":"NewMapInfo","payload":{"mapIndex":0,"info":{}}}"#
        )
        .is_err());
        assert!(parse_inbound_event(
            r#"{"type":"packet","packet":"SearchMapResult","payload":{"mapIndex":-2,"npcIndex":0}}"#
        )
        .is_err());
    }

    #[test]
    fn resume_control_events_parse_without_exposing_credential_in_debug() {
        let credential = "A".repeat(43);
        let event = parse_inbound_event(&format!(
            r#"{{"type":"resumeCredential","credential":"{credential}","expiresAtMs":123,"generation":7}}"#
        ))
        .expect("resume credential");
        match &event {
            InboundEvent::ResumeCredential(value) => {
                assert_eq!(value.credential, credential);
                assert_eq!(value.expires_at_ms, Some(123));
                assert_eq!(value.generation, Some(7));
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(!format!("{event:?}").contains(&credential));
        assert!(matches!(
            parse_inbound_event(r#"{"type":"sessionResumed","characterIndex":2,"generation":8}"#),
            Ok(InboundEvent::SessionResumed(SessionResumedEvent {
                character_index: Some(2),
                generation: Some(8),
            }))
        ));
        assert!(matches!(
            parse_inbound_event(r#"{"type":"resumeRejected","code":"unavailable"}"#),
            Ok(InboundEvent::ResumeRejected(ResumeRejectedEvent {
                code: Some(_),
            }))
        ));
    }

    #[test]
    fn outbound_command_debug_redacts_login_password() {
        let command = NativeOutboundCommand::Login {
            account_id: "player".into(),
            password: "super-secret".into(),
        };
        let debug = format!("{command:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("super-secret"));

        let new_account = NativeOutboundCommand::NewAccount {
            account_id: "fresh".into(),
            password: "registration-secret".into(),
            birth_date_binary: 0,
            user_name: "fresh".into(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        };
        let new_account_debug = format!("{new_account:?}");
        assert!(new_account_debug.contains("<redacted>"));
        assert!(!new_account_debug.contains("registration-secret"));

        let change_password = NativeOutboundCommand::ChangePassword {
            account_id: "player".into(),
            current_password: "old-secret".into(),
            new_password: "new-secret".into(),
        };
        let change_password_debug = format!("{change_password:?}");
        assert!(change_password_debug.contains("<redacted>"));
        assert!(!change_password_debug.contains("old-secret"));
        assert!(!change_password_debug.contains("new-secret"));
    }

    #[test]
    fn inbound_new_account_result_is_classified() {
        let parsed = parse_inbound_event(
            r#"{"type":"packet","packet":"NewAccount","payload":{"result":8}}"#,
        )
        .expect("parse NewAccount result");
        match parsed {
            InboundEvent::Packet(PacketEvent::NewAccountResult(result)) => {
                assert_eq!(result.result, Some(8));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn outbound_finish_quest_keeps_default_selected_item_index_when_omitted_in_json() {
        let command = serde_json::from_str::<NativeOutboundCommand>(
            r#"{"type":"finishQuest","questIndex":13}"#,
        )
        .expect("finishQuest should deserialize");
        match command {
            NativeOutboundCommand::FinishQuest {
                selected_item_index,
                ..
            } => assert_eq!(selected_item_index, -1),
            _ => panic!("wrong command variant"),
        }
    }

    #[test]
    fn inbound_login_success_parses_character_roster() {
        let text = r#"{
            "type":"packet",
            "packet":"LoginSuccess",
            "payload":{
                "characters":[
                    {"index":0,"name":"Scion","level":1,"class":"Warrior","gender":"Male"},
                    {"index":1,"name":"Ranger","level":2,"class":"Archer","gender":"Female"}
                ]
            }
        }"#;
        let parsed = parse_inbound_event(text).expect("parse login success");
        match parsed {
            InboundEvent::Packet(PacketEvent::LoginSuccess(login)) => {
                assert_eq!(login.characters.len(), 2);
                assert_eq!(login.characters[0].index, Some(0));
                assert_eq!(login.characters[0].name.as_deref(), Some("Scion"));
                assert_eq!(login.characters[1].name.as_deref(), Some("Ranger"));
            }
            other => panic!("expected login success, got: {other:?}"),
        }
    }

    #[test]
    fn inbound_login_failure_supports_packet_and_error_shapes() {
        let packet = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"Login",
            "payload":{"result":-1}
        }"#,
        )
        .expect("parse packet login failure");
        match packet {
            InboundEvent::Packet(PacketEvent::LoginFailure(failure)) => {
                assert_eq!(failure.packet, "Login");
                assert_eq!(failure.result, Some(-1));
            }
            _ => panic!("expected packet login failure"),
        }

        let error = parse_inbound_event(
            r#"{
            "type":"error",
            "payload":{"message":"invalid credentials"}
        }"#,
        )
        .expect("parse error");
        match error {
            InboundEvent::Error(err) => {
                assert_eq!(err.message.as_deref(), Some("invalid credentials"));
            }
            _ => panic!("expected error envelope"),
        }
    }

    #[test]
    fn inbound_change_password_results_preserve_authoritative_codes() {
        let result = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"ChangePassword",
            "payload":{"result":5}
        }"#,
        )
        .expect("parse change password result");
        match result {
            InboundEvent::Packet(PacketEvent::ChangePasswordResult(result)) => {
                assert_eq!(result.result, Some(5));
            }
            other => panic!("expected change password result, got: {other:?}"),
        }

        let banned = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"ChangePasswordBanned",
            "payload":{"reason":"manual review","expiryDate":"2030-01-01T00:00:00Z"}
        }"#,
        )
        .expect("parse change password banned result");
        match banned {
            InboundEvent::Packet(PacketEvent::ChangePasswordBanned(banned)) => {
                assert_eq!(banned.reason.as_deref(), Some("manual review"));
                assert_eq!(banned.expiry, Some(json!("2030-01-01T00:00:00Z")));
            }
            other => panic!("expected change password banned result, got: {other:?}"),
        }
    }

    #[test]
    fn inbound_new_character_delete_and_start_game_events_parse() {
        let new_character = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"NewCharacterSuccess",
            "payload":{
                "character":{"index":2,"name":"Scion","level":1,"class":"Warrior","gender":"Male"}
            }
        }"#,
        )
        .expect("parse newCharacter success");
        match new_character {
            InboundEvent::Packet(PacketEvent::NewCharacterSuccess(payload)) => {
                assert_eq!(
                    payload
                        .character
                        .as_ref()
                        .and_then(|value| value.get("name")),
                    Some(&json!("Scion"))
                );
            }
            _ => panic!("expected new char success"),
        }

        let delete = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"DeleteCharacterSuccess",
            "payload":{"characterIndex":2}
        }"#,
        )
        .expect("parse delete success");
        match delete {
            InboundEvent::Packet(PacketEvent::DeleteCharacterSuccess(payload)) => {
                assert_eq!(payload.character_index, Some(2));
            }
            _ => panic!("expected delete character success"),
        }

        let start = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"StartGame",
            "payload":{"result":0,"resolution":"ok"}
        }"#,
        )
        .expect("parse start game");
        match start {
            InboundEvent::Packet(PacketEvent::StartGameAck(start)) => {
                assert_eq!(start.result, Some(0));
                assert_eq!(start.resolution, Some(json!("ok")));
            }
            _ => panic!("expected start game ack"),
        }
    }

    #[test]
    fn inbound_user_info_npc_and_quest_events_parse() {
        let user_information = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"UserInformation",
            "payload":{
                "objectId":1001,
                "name":"Scion",
                "class":"Warrior",
                "gender":"Male",
                "level":15
            }
        }"#,
        )
        .expect("parse user info");
        match user_information {
            InboundEvent::Packet(PacketEvent::UserInformation(info)) => {
                assert_eq!(info.object_id, Some(1001));
                assert_eq!(info.class_name.as_deref(), Some("Warrior"));
                assert_eq!(info.level, Some(15));
            }
            _ => panic!("expected user information"),
        }

        let npc_response = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"NPCResponse",
            "payload":{"page":"quest","npc":"@Quest","lines":["hello"]}
        }"#,
        )
        .expect("parse npc response");
        assert!(matches!(
            npc_response,
            InboundEvent::Packet(PacketEvent::NPCResponse(_))
        ));

        let new_quest = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"NewQuestInfo",
            "payload":{"id":42,"name":"Bugs","descriptionLines":["kill monsters"]}
        }"#,
        )
        .expect("parse new quest");
        match new_quest {
            InboundEvent::Packet(PacketEvent::NewQuestInfo(info)) => {
                assert_eq!(info.quest_id, Some(42));
            }
            _ => panic!("expected new quest info"),
        }

        let change = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"ChangeQuest",
            "payload":{"questId":42,"state":1,"taskList":["A","B"],"taken":true}
        }"#,
        )
        .expect("parse change quest");
        match change {
            InboundEvent::Packet(PacketEvent::ChangeQuest(changed)) => {
                assert_eq!(changed.quest_id, Some(42));
                assert_eq!(changed.state, Some(1));
            }
            _ => panic!("expected change quest"),
        }

        let complete = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"CompleteQuest",
            "payload":{"completedQuests":[1,2,3]}
        }"#,
        )
        .expect("parse complete quest");
        match complete {
            InboundEvent::Packet(PacketEvent::CompleteQuest(done)) => {
                assert_eq!(done.completed_quests, Some(json!([1, 2, 3])));
            }
            _ => panic!("expected complete quest"),
        }
    }

    #[test]
    fn inbound_disconnect_and_unknown_events_parse() {
        let disconnect = parse_inbound_event(
            r#"{
            "type":"packet",
            "packet":"Disconnect",
            "payload":{"reason":"client kicked"}
        }"#,
        )
        .expect("parse disconnect");
        match disconnect {
            InboundEvent::Packet(PacketEvent::Disconnect(event)) => {
                assert_eq!(event.reason.as_deref(), Some("client kicked"));
            }
            _ => panic!("expected disconnect"),
        }

        let unknown =
            parse_inbound_event(r#"{"type":"packet","packet":"UnknownPacket","payload":{"x":1}}"#)
                .expect("parse unknown packet event");
        match unknown {
            InboundEvent::Packet(PacketEvent::Other { packet, .. }) => {
                assert_eq!(packet, "UnknownPacket");
            }
            _ => panic!("expected unknown packet event"),
        }

        let malformed = parse_inbound_event("{not json");
        assert!(malformed.is_err());
    }
}

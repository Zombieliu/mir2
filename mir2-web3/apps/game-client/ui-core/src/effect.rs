//! Effects produced by the reducer. The platform adapter turns them into Gateway commands or window actions.

use serde::{Deserialize, Serialize};

use crate::state::{UiChatSettings, UiOptions, UiWindowMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiEffect {
    GatewayCommand(GatewayCommand),
    ExitApplication,
    /// The renderer/audio adapter must apply these values at runtime. This is
    /// deliberately typed because ui-core cannot own platform audio APIs.
    ApplyAudioSettings {
        music_enabled: bool,
        music_volume: u8,
        sound_enabled: bool,
        sound_volume: u8,
    },
    /// The current Bevy write set does not own the OS window handle. Hosts must
    /// explicitly consume this effect; no fake fullscreen mutation is made.
    ApplyWindowMode {
        mode: UiWindowMode,
    },
    /// Persistence is an adapter seam, not an in-memory-only claim.
    PersistOptions {
        options: UiOptions,
    },
    /// Apply the local chat presentation settings to the renderer.
    ApplyChatSettings {
        settings: UiChatSettings,
    },
    /// Persist the local chat presentation settings in the platform adapter.
    PersistChatSettings {
        settings: UiChatSettings,
    },
    SaveSetting {
        key: String,
        value: String,
    },
    ShowNotice {
        message: String,
        is_error: bool,
    },
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GatewayCommand {
    Login {
        account: String,
        password: String,
    },
    RegisterAccount {
        account: String,
        password: String,
    },
    StartGame {
        index: i32,
    },
    GameShopBuy {
        request_id: String,
        g_index: i32,
        quantity: u8,
        price_type: i32,
    },
    SendMail {
        recipient: String,
        message: String,
        gold: u32,
        attachment_unique_ids: Vec<u64>,
    },
    CreateCharacter {
        name: String,
        class: String,
        gender: String,
    },
    DeleteCharacter {
        index: i32,
    },
    Logout,
    RetryConnection,
    InteractNpc {
        object_id: u32,
    },
    SelectNpcDialog {
        target: String,
    },
    AcceptQuest {
        npc_index: u32,
        quest_index: i32,
    },
    FinishQuest {
        quest_index: i32,
        selected_item_index: i32,
    },
    UseItem {
        unique_id: u64,
    },
    EquipItem {
        unique_id: u64,
        to: i32,
    },
    UnequipItem {
        unique_id: u64,
    },
    DropItem {
        key: String,
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
    },
    MoveItem {
        grid: String,
        from: i32,
        to: i32,
    },
    MergeItem {
        grid_from: String,
        grid_to: String,
        id_from: u64,
        id_to: u64,
    },
    SplitItem {
        unique_id: u64,
        grid: String,
        count: u16,
    },
    AbandonQuest {
        quest_index: i32,
    },
    SetChatChannel {
        channel: String,
    },
    AttackTarget {
        object_id: u32,
    },
    PickUp {
        object_id: u32,
    },
    SendChat {
        message: String,
    },
    TownRevive,
    GroupSwitch { allow_group: bool },
    GroupAddMember { name: String },
    GroupRemoveMember { name: String },
    GroupInvite { accept_invite: bool },
    GuildRequestInfo { info_type: u8 },
    GuildEditMember {
        change_type: u8,
        rank_index: u8,
        name: String,
        rank_name: String,
    },
    GuildEditNotice { notice: Vec<String> },
    GuildInvite { accept_invite: bool },
    TradeRequest,
    TradeReply { accept_invite: bool },
    TradeGold { amount: u32 },
    TradeDepositItem { from: i32, to: i32 },
    TradeRetrieveItem { from: i32, to: i32 },
    TradeConfirm { locked: bool },
    TradeCancel,
}

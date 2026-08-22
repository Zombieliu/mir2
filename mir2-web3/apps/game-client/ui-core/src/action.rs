//! Platform-agnostic UI actions. Every visible button maps to one variant.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiAction {
    // Login / Connection
    Login,
    RegisterAccount,
    ChangePassword,
    SafeKey,
    CancelLogin,
    RetryConnection,
    // Character select / create
    SelectCharacter {
        index: i32,
    },
    StartGame,
    OpenCharacterCreate,
    CreateCharacter {
        name: String,
        class: String,
        gender: String,
    },
    CancelCharacterCreate,
    DeleteCharacter {
        index: i32,
    },
    ConfirmDeleteCharacter,
    CancelDeleteCharacter,
    OpenCredits,
    ExitApplication,
    // In-game HUD primary buttons
    OpenInventory,
    OpenCharacter,
    OpenSkill,
    OpenQuestLog,
    OpenOptions,
    OpenMenu,
    OpenGameShop,
    OpenNpcShop,
    OpenMail,
    OpenBigMap,
    OpenStorage,
    OpenGroup,
    OpenGuild,
    OpenTrade,
    ToggleMinimap,
    OpenMailCompose,
    SetMailRecipient {
        recipient: String,
    },
    SetMailMessage {
        message: String,
    },
    SetMailGold {
        gold: u32,
    },
    AddMailAttachment {
        unique_id: u64,
    },
    RemoveMailAttachment {
        unique_id: u64,
    },
    SubmitMail,
    CancelMailCompose,
    // Options
    SetMusicEnabled {
        enabled: bool,
    },
    SetMusicVolume {
        volume: u8,
    },
    SetSoundEnabled {
        enabled: bool,
    },
    SetSoundVolume {
        volume: u8,
    },
    SetWindowMode {
        mode: crate::state::UiWindowMode,
    },
    ApplyOptions,
    CancelOptions,
    ResetOptionsToDefaults,
    // In-game panels
    ClosePanel,
    CloseAllPanels,
    /// Buy from the server-owned cash shop. The server revalidates every
    /// field; the reducer only rejects malformed local input.
    GameShopBuy {
        g_index: i32,
        quantity: u8,
        price_type: i32,
    },
    // Inventory / Character / Skill
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
    // Quest / NPC
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
    AbandonQuest {
        quest_index: i32,
    },
    // Chat
    FocusChat,
    BlurChat,
    SendChat {
        message: String,
    },
    SetChatChannel {
        channel: String,
    },
    ScrollChatUp,
    ScrollChatDown,
    ResizeChat,
    OpenChatSettings,
    SetChatFilterVisibility {
        channel: crate::state::UiChatChannel,
        visible: bool,
    },
    SetAllChatFilterVisibility {
        visible: bool,
    },
    SetChatTransparency {
        transparent: bool,
    },
    ApplyChatSettings,
    CancelChatSettings,
    ResetChatSettingsToDefaults,
    CloseChatSettings,
    // Combat / World
    AttackTarget {
        object_id: u32,
    },
    PickUp {
        object_id: u32,
    },
    TownRevive,
    // System
    Logout,
    ReturnToCharacterSelect,
}

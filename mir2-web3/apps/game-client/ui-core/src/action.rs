//! Platform-agnostic UI actions. Every visible button maps to one variant.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiAction {
    // Login / Connection
    Login,
    RegisterAccount,
    /// Opens the shared change-password flow. Credentials are supplied only
    /// by `SubmitChangePassword`; they are never staged in `UiState`.
    ChangePassword,
    /// Opens Crystal's local Safe Key flow. Safe Key is a local input aid,
    /// not a fabricated server command.
    SafeKey,
    SubmitChangePassword {
        account: String,
        old_password: crate::effect::SecretText,
        new_password: crate::effect::SecretText,
        confirm_password: crate::effect::SecretText,
    },
    ChangePasswordResult {
        success: bool,
        message: String,
    },
    CancelChangePassword,
    CloseSafeKey,
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
    /// Requests the authoritative guild-storage listing. The Android and
    /// other hosts serialize the resulting typed gateway command; no local
    /// inventory is fabricated.
    RequestGuildStorage,
    /// Deposits (type 0) or withdraws (type 1) guild gold.
    GuildStorageGoldChange {
        change_type: u8,
        amount: u32,
    },
    /// Moves an item between the player's inventory and guild storage, or
    /// requests the storage list with the protocol's type-3 sentinel.
    GuildStorageItemChange {
        change_type: u8,
        from: i32,
        to: i32,
    },
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
    /// Stage one of Crystal OptionDialog's seven persisted local switches.
    SetCrystalOption {
        option: crate::state::UiCrystalOption,
        enabled: bool,
    },
    /// Ask the authoritative server to change observation permission. The
    /// reducer does not optimistically change `observe_allowed`.
    RequestObserve {
        allow: bool,
    },
    /// Host notification sourced from the authoritative AllowObserve packet.
    ObserveAuthoritativeChanged {
        allow: bool,
    },
    /// Opens the separate native-host settings surface. This is deliberately
    /// not reachable through Crystal's HUD OptionDialog.
    OpenPlatformSettings,
    /// Native-only staged extension, intentionally outside Crystal's seven
    /// immediate OptionDialog switches.
    SetPlatformWindowMode {
        mode: crate::state::UiWindowMode,
    },
    ApplyPlatformSettings,
    CancelPlatformSettings,
    ResetPlatformSettingsToDefaults,
    /// Legacy renderer compatibility only. It must not be registered as a
    /// Crystal OptionDialog control; use `SetPlatformWindowMode` instead.
    SetWindowMode {
        mode: crate::state::UiWindowMode,
    },
    /// Legacy renderer compatibility only. Crystal has no Apply button.
    ApplyOptions,
    /// Legacy renderer compatibility only. Crystal Close simply hides.
    CancelOptions,
    /// Legacy renderer compatibility only. Crystal has no Defaults button.
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
    /// Request-only ordinary personal storage operation. The reducer owns
    /// requestId allocation; no Android renderer is implied by this seam.
    StoreItem {
        from: i32,
        to: i32,
    },
    TakeBackItem {
        from: i32,
        to: i32,
    },
    /// Host-delivered authoritative storage receipt. It clears pending state
    /// only when every receipt field matches the current request.
    StorageReceiptReceived {
        receipt: crate::storage::StorageReceipt,
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

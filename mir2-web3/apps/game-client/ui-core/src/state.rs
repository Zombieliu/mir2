//! Platform-agnostic UI state. No Bevy / Windows / Android dependency.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::game_shop::{GameShopReceipt, GameShopRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UiScreen {
    #[default]
    Connecting,
    Login,
    Authenticating,
    CharacterSelect,
    CharacterCreate,
    StartingGame,
    InGame,
    ConnectionLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UiPanel {
    #[default]
    None,
    Inventory,
    Character,
    Skill,
    QuestLog,
    Options,
    /// Native host extension. This is deliberately not Crystal's
    /// `OptionDialog` and has its own staged Apply/Cancel contract.
    PlatformSettings,
    Menu,
    GameShop,
    NpcShop,
    Mail,
    BigMap,
    Storage,
    Group,
    Guild,
    Trade,
    Minimap,
    ChatSettings,
    NpcDialog,
    DeleteConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UiSecurityPanel {
    #[default]
    None,
    ChangePassword,
    SafeKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UiSecurityState {
    pub panel: UiSecurityPanel,
    /// Held until an authoritative result arrives. No credential is stored.
    pub change_password_pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UiWindowMode {
    #[default]
    Windowed,
    Fullscreen,
}

/// Settings that belong to the native host rather than Crystal's
/// `OptionDialog`. Keeping this separate prevents a platform window toggle
/// from changing the semantics of Crystal's immediate local controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UiPlatformSettings {
    pub window_mode: UiWindowMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiOptions {
    /// Crystal `Settings.SkillMode`: `false` selects Ctrl skill bindings and
    /// `true` selects tilde skill bindings.
    #[serde(default)]
    pub skill_mode: bool,
    #[serde(default = "default_true")]
    pub skill_bar: bool,
    #[serde(default = "default_true")]
    pub effect: bool,
    #[serde(default = "default_true")]
    pub drop_view: bool,
    #[serde(default = "default_true")]
    pub name_view: bool,
    #[serde(default = "default_true")]
    pub hp_view: bool,
    #[serde(default)]
    pub new_move: bool,
    pub music_enabled: bool,
    pub music_volume: u8,
    pub sound_enabled: bool,
    pub sound_volume: u8,
    /// Compatibility mirror for older renderers. The canonical native window
    /// setting lives in [`UiPlatformSettings`]; Crystal controls never mutate
    /// this field.
    pub window_mode: UiWindowMode,
}

fn default_true() -> bool {
    true
}

/// The seven persisted local switches owned by Crystal's OptionDialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiCrystalOption {
    SkillMode,
    SkillBar,
    Effect,
    DropView,
    NameView,
    HpView,
    NewMove,
}

/// Runtime/persistence payload for Crystal OptionDialog's exact local fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalClientOptions {
    pub skill_mode: bool,
    pub skill_bar: bool,
    pub effect: bool,
    pub drop_view: bool,
    pub name_view: bool,
    pub hp_view: bool,
    pub new_move: bool,
}

impl Default for CrystalClientOptions {
    fn default() -> Self {
        Self {
            skill_mode: false,
            skill_bar: true,
            effect: true,
            drop_view: true,
            name_view: true,
            hp_view: true,
            new_move: false,
        }
    }
}

pub const MAIL_MAX_ATTACHMENTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MailComposeDraft {
    pub recipient: String,
    pub message: String,
    pub gold: u32,
    pub attachment_unique_ids: Vec<u64>,
}

impl MailComposeDraft {
    pub fn add_attachment(&mut self, unique_id: u64) -> bool {
        if unique_id == 0
            || self.attachment_unique_ids.len() >= MAIL_MAX_ATTACHMENTS
            || self.attachment_unique_ids.contains(&unique_id)
        {
            return false;
        }
        self.attachment_unique_ids.push(unique_id);
        true
    }

    pub fn remove_attachment(&mut self, unique_id: u64) -> bool {
        let before = self.attachment_unique_ids.len();
        self.attachment_unique_ids.retain(|id| *id != unique_id);
        before != self.attachment_unique_ids.len()
    }
}

impl Default for UiOptions {
    fn default() -> Self {
        Self {
            skill_mode: false,
            skill_bar: true,
            effect: true,
            drop_view: true,
            name_view: true,
            hp_view: true,
            new_move: false,
            music_enabled: true,
            music_volume: 80,
            sound_enabled: true,
            sound_volume: 80,
            window_mode: UiWindowMode::Windowed,
        }
    }
}

impl UiOptions {
    pub const MAX_VOLUME: u8 = 100;

    pub fn clamp_volume(volume: u8) -> u8 {
        volume.min(Self::MAX_VOLUME)
    }

    pub fn set_crystal_option(&mut self, option: UiCrystalOption, enabled: bool) {
        match option {
            UiCrystalOption::SkillMode => self.skill_mode = enabled,
            UiCrystalOption::SkillBar => self.skill_bar = enabled,
            UiCrystalOption::Effect => self.effect = enabled,
            UiCrystalOption::DropView => self.drop_view = enabled,
            UiCrystalOption::NameView => self.name_view = enabled,
            UiCrystalOption::HpView => self.hp_view = enabled,
            UiCrystalOption::NewMove => self.new_move = enabled,
        }
    }

    pub fn crystal_option(&self, option: UiCrystalOption) -> bool {
        match option {
            UiCrystalOption::SkillMode => self.skill_mode,
            UiCrystalOption::SkillBar => self.skill_bar,
            UiCrystalOption::Effect => self.effect,
            UiCrystalOption::DropView => self.drop_view,
            UiCrystalOption::NameView => self.name_view,
            UiCrystalOption::HpView => self.hp_view,
            UiCrystalOption::NewMove => self.new_move,
        }
    }

    pub fn crystal(&self) -> CrystalClientOptions {
        CrystalClientOptions {
            skill_mode: self.skill_mode,
            skill_bar: self.skill_bar,
            effect: self.effect,
            drop_view: self.drop_view,
            name_view: self.name_view,
            hp_view: self.hp_view,
            new_move: self.new_move,
        }
    }
}

/// Chat channels supported by the shared native chat filter state.
///
/// Crystal's ChatOptionDialog renders the first eight entries. Trade is kept
/// in the shared state because the native ChatControlBar already exposes it;
/// renderers do not have to show a Trade checkbox in the options dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiChatChannel {
    Normal,
    Whisper,
    Shout,
    System,
    Lover,
    Mentor,
    Group,
    Guild,
    Trade,
}

impl UiChatChannel {
    pub const ALL: [Self; 9] = [
        Self::Normal,
        Self::Whisper,
        Self::Shout,
        Self::System,
        Self::Lover,
        Self::Mentor,
        Self::Group,
        Self::Guild,
        Self::Trade,
    ];

    /// Channels rendered by Crystal's `ChatOptionDialog`. Trade remains a
    /// control-bar filter, but the source dialog has no ninth Trade checkbox.
    pub const SETTINGS: [Self; 8] = [
        Self::Normal,
        Self::Whisper,
        Self::Shout,
        Self::System,
        Self::Lover,
        Self::Mentor,
        Self::Group,
        Self::Guild,
    ];

    pub fn all() -> &'static [Self] {
        &Self::ALL
    }

    pub fn settings() -> &'static [Self] {
        &Self::SETTINGS
    }
}

/// Last applied local chat presentation settings.
///
/// The fields mirror Crystal `Client/Settings.cs` and
/// `Client/MirScenes/Dialogs/ChatOptionDialog.cs`: a boolean hidden filter per
/// supported channel plus the binary transparent-chat setting. There is no
/// unsupported opacity slider or server command in this state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiChatSettings {
    pub filter_normal: bool,
    pub filter_whisper: bool,
    pub filter_shout: bool,
    pub filter_system: bool,
    pub filter_lover: bool,
    pub filter_mentor: bool,
    pub filter_group: bool,
    pub filter_guild: bool,
    pub filter_trade: bool,
    pub transparent: bool,
}

impl Default for UiChatSettings {
    fn default() -> Self {
        Self {
            filter_normal: false,
            filter_whisper: false,
            filter_shout: false,
            filter_system: false,
            filter_lover: false,
            filter_mentor: false,
            filter_group: false,
            filter_guild: false,
            filter_trade: false,
            transparent: false,
        }
    }
}

impl UiChatSettings {
    pub fn is_filter_hidden(self, channel: UiChatChannel) -> bool {
        match channel {
            UiChatChannel::Normal => self.filter_normal,
            UiChatChannel::Whisper => self.filter_whisper,
            UiChatChannel::Shout => self.filter_shout,
            UiChatChannel::System => self.filter_system,
            UiChatChannel::Lover => self.filter_lover,
            UiChatChannel::Mentor => self.filter_mentor,
            UiChatChannel::Group => self.filter_group,
            UiChatChannel::Guild => self.filter_guild,
            UiChatChannel::Trade => self.filter_trade,
        }
    }

    pub fn set_filter_hidden(&mut self, channel: UiChatChannel, hidden: bool) {
        match channel {
            UiChatChannel::Normal => self.filter_normal = hidden,
            UiChatChannel::Whisper => self.filter_whisper = hidden,
            UiChatChannel::Shout => self.filter_shout = hidden,
            UiChatChannel::System => self.filter_system = hidden,
            UiChatChannel::Lover => self.filter_lover = hidden,
            UiChatChannel::Mentor => self.filter_mentor = hidden,
            UiChatChannel::Group => self.filter_group = hidden,
            UiChatChannel::Guild => self.filter_guild = hidden,
            UiChatChannel::Trade => self.filter_trade = hidden,
        }
    }

    pub fn set_filter_visible(&mut self, channel: UiChatChannel, visible: bool) {
        self.set_filter_hidden(channel, !visible);
    }

    pub fn any_filter_hidden(self) -> bool {
        UiChatChannel::all()
            .iter()
            .any(|channel| self.is_filter_hidden(*channel))
    }

    pub fn any_dialog_filter_hidden(self) -> bool {
        UiChatChannel::settings()
            .iter()
            .any(|channel| self.is_filter_hidden(*channel))
    }

    pub fn hidden_filter_count(self) -> usize {
        UiChatChannel::all()
            .iter()
            .filter(|channel| self.is_filter_hidden(**channel))
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Resource)]
pub struct UiState {
    pub screen: UiScreen,
    pub panel: UiPanel,
    pub login_account: String,
    pub login_password: String,
    #[serde(default)]
    pub security: UiSecurityState,
    pub selected_character: Option<i32>,
    pub chat_focused: bool,
    pub minimap_visible: bool,
    /// Last applied settings. Adapters persist these through `UiEffect`.
    pub options: UiOptions,
    /// Crystal OptionDialog is immediate: it has no staged draft.
    pub options_draft: Option<UiOptions>,
    /// Native-only settings are isolated from Crystal's OptionDialog.
    #[serde(default)]
    pub platform_settings: UiPlatformSettings,
    /// The only staged settings draft. It exists solely for the separate
    /// native Platform Settings surface.
    #[serde(skip)]
    pub platform_settings_draft: Option<UiPlatformSettings>,
    /// Last server-authoritative `AllowObserve` value. OptionDialog may only
    /// request a change; it never commits this value optimistically.
    #[serde(default)]
    pub observe_allowed: bool,
    /// Desired value awaiting an authoritative server update. This is
    /// transient session state and is deliberately excluded from persistence.
    #[serde(skip)]
    pub observe_request_pending: Option<bool>,
    /// Last applied local chat settings.
    pub chat_settings: UiChatSettings,
    /// Working copy used only while the Chat Settings panel is open.
    pub chat_settings_draft: Option<UiChatSettings>,
    pub mail_compose: Option<MailComposeDraft>,
    /// Per-session native GameShop correlation state. It is deliberately
    /// shared by Android and reducer-driven hosts so request ids cannot be
    /// reused within one login session.
    #[serde(default = "default_game_shop_next_request_id")]
    pub game_shop_next_request_id: u64,
    pub game_shop_pending: Option<GameShopRequest>,
    pub game_shop_last_receipt: Option<GameShopReceipt>,
    /// Set when a purchase may have committed but its receipt was lost on a
    /// terminal connection reset. The client must refresh mail/wallet and
    /// must never replay the purchase automatically.
    pub game_shop_unknown: bool,
    /// Per-session ordinary personal-storage correlation state. This is a
    /// contract seam only; no renderer is implied by these fields.
    #[serde(default = "default_storage_next_request_id")]
    pub storage_next_request_id: u64,
    pub storage_pending: Option<crate::storage::StorageRequest>,
    pub storage_last_receipt: Option<crate::storage::StorageReceipt>,
    pub storage_unknown: bool,
}

fn default_game_shop_next_request_id() -> u64 {
    1
}

fn default_storage_next_request_id() -> u64 {
    1
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            screen: UiScreen::default(),
            panel: UiPanel::default(),
            login_account: String::new(),
            login_password: String::new(),
            security: UiSecurityState::default(),
            selected_character: None,
            chat_focused: false,
            // Crystal's native HUD starts with the minimap visible.
            minimap_visible: true,
            options: UiOptions::default(),
            options_draft: None,
            platform_settings: UiPlatformSettings::default(),
            platform_settings_draft: None,
            observe_allowed: false,
            observe_request_pending: None,
            chat_settings: UiChatSettings::default(),
            chat_settings_draft: None,
            mail_compose: None,
            game_shop_next_request_id: 1,
            game_shop_pending: None,
            game_shop_last_receipt: None,
            game_shop_unknown: false,
            storage_next_request_id: 1,
            storage_pending: None,
            storage_last_receipt: None,
            storage_unknown: false,
        }
    }
}

impl UiState {
    pub fn begin_game_shop_purchase(
        &mut self,
        g_index: i32,
        quantity: u8,
        price_type: i32,
    ) -> Option<GameShopRequest> {
        if self.game_shop_pending.is_some() || self.game_shop_next_request_id == 0 {
            return None;
        }
        let request_id = crate::game_shop::request_id_for_sequence(self.game_shop_next_request_id);
        let request = GameShopRequest::new(request_id, g_index, quantity, price_type)?;
        self.game_shop_next_request_id =
            crate::game_shop::next_request_sequence(self.game_shop_next_request_id);
        self.game_shop_pending = Some(request.clone());
        self.game_shop_unknown = false;
        Some(request)
    }

    pub fn apply_game_shop_receipt(&mut self, receipt: GameShopReceipt) -> bool {
        if !receipt.is_valid() {
            return false;
        }
        let Some(request) = self.game_shop_pending.as_ref() else {
            return false;
        };
        if !receipt.matches_request(request) {
            return false;
        }
        self.game_shop_last_receipt = Some(receipt);
        self.game_shop_pending = None;
        self.game_shop_unknown = false;
        true
    }

    pub fn mark_game_shop_unknown(&mut self) {
        if self.game_shop_pending.take().is_some() {
            self.game_shop_unknown = true;
        }
    }

    pub fn cancel_game_shop_purchase(&mut self, request_id: &str) -> bool {
        if self
            .game_shop_pending
            .as_ref()
            .is_some_and(|request| request.request_id == request_id)
        {
            self.game_shop_pending = None;
            return true;
        }
        false
    }

    pub fn clear_game_shop_session(&mut self) {
        let had_pending = self.game_shop_pending.take().is_some();
        self.game_shop_next_request_id = 1;
        self.game_shop_last_receipt = None;
        self.game_shop_unknown = had_pending;
    }

    /// Prepare this UI owner to consume an exact receipt across a terminal
    /// session reset. Other UI/session state is reset by the host adapter; this
    /// method retains only the receipt's exact request four-tuple.
    pub fn preserve_exact_game_shop_receipt_boundary(&mut self, receipt: &GameShopReceipt) -> bool {
        if !receipt.is_valid() {
            return false;
        }
        let Some(request) = GameShopRequest::new(
            receipt.request_id.clone(),
            receipt.g_index,
            receipt.quantity,
            receipt.price_type,
        ) else {
            return false;
        };
        self.game_shop_next_request_id = 1;
        self.game_shop_pending = Some(request);
        self.game_shop_last_receipt = None;
        self.game_shop_unknown = false;
        true
    }

    pub fn begin_storage_request(
        &mut self,
        operation: crate::storage::StorageOperation,
        from: i32,
        to: i32,
    ) -> Option<crate::storage::StorageRequest> {
        if self.storage_pending.is_some() || self.storage_next_request_id == 0 {
            return None;
        }
        let request_id = crate::storage::request_id_for_sequence(self.storage_next_request_id);
        let request = crate::storage::StorageRequest::new(request_id, operation, from, to)?;
        self.storage_next_request_id =
            crate::storage::next_request_sequence(self.storage_next_request_id);
        self.storage_pending = Some(request.clone());
        self.storage_unknown = false;
        Some(request)
    }

    pub fn apply_storage_receipt(&mut self, receipt: crate::storage::StorageReceipt) -> bool {
        if !receipt.is_valid() {
            return false;
        }
        let Some(request) = self.storage_pending.as_ref() else {
            return false;
        };
        if !receipt.matches_request(request) {
            return false;
        }
        self.storage_last_receipt = Some(receipt);
        self.storage_pending = None;
        self.storage_unknown = false;
        true
    }

    pub fn mark_storage_unknown(&mut self) {
        if self.storage_pending.take().is_some() {
            self.storage_unknown = true;
        }
    }

    pub fn clear_storage_session(&mut self) {
        let had_pending = self.storage_pending.take().is_some();
        // Keep the process-lifetime sequence monotonic. Reusing an identifier
        // after reconnect would let a delayed receipt match a later request.
        self.storage_last_receipt = None;
        self.storage_unknown = had_pending;
    }

    pub fn is_inventory_open(&self) -> bool {
        self.panel == UiPanel::Inventory
    }
    pub fn is_quest_log_open(&self) -> bool {
        self.panel == UiPanel::QuestLog
    }
    pub fn is_options_open(&self) -> bool {
        self.panel == UiPanel::Options
    }
    pub fn is_chat_settings_open(&self) -> bool {
        self.panel == UiPanel::ChatSettings
    }
    pub fn is_mail_open(&self) -> bool {
        self.panel == UiPanel::Mail
    }
    pub fn is_bigmap_open(&self) -> bool {
        self.panel == UiPanel::BigMap
    }
    pub fn is_shop_open(&self) -> bool {
        self.panel == UiPanel::GameShop
    }
    pub fn is_npc_shop_open(&self) -> bool {
        self.panel == UiPanel::NpcShop
    }
    pub fn is_storage_open(&self) -> bool {
        self.panel == UiPanel::Storage
    }
    pub fn is_group_open(&self) -> bool {
        self.panel == UiPanel::Group
    }
    pub fn is_guild_open(&self) -> bool {
        self.panel == UiPanel::Guild
    }
    pub fn is_trade_open(&self) -> bool {
        self.panel == UiPanel::Trade
    }
    pub fn is_character_open(&self) -> bool {
        self.panel == UiPanel::Character
    }
    pub fn is_skill_open(&self) -> bool {
        self.panel == UiPanel::Skill
    }
    pub fn is_menu_open(&self) -> bool {
        self.panel == UiPanel::Menu
    }

    // Names shared with the native Bevy adapter. These methods intentionally
    // read only this resource; no adapter-owned mirror is allowed.
    pub fn inventory_open(&self) -> bool {
        self.is_inventory_open()
    }

    pub fn equipment_open(&self) -> bool {
        self.panel == UiPanel::Character
    }

    pub fn menu_open(&self) -> bool {
        self.is_menu_open()
    }

    pub fn skill_open(&self) -> bool {
        self.is_skill_open()
    }

    pub fn quest_open(&self) -> bool {
        self.is_quest_log_open()
    }

    pub fn options_open(&self) -> bool {
        self.is_options_open()
    }

    pub fn chat_settings_open(&self) -> bool {
        self.is_chat_settings_open()
    }

    pub fn mail_open(&self) -> bool {
        self.is_mail_open()
    }

    pub fn bigmap_open(&self) -> bool {
        self.is_bigmap_open()
    }

    pub fn shop_open(&self) -> bool {
        self.is_shop_open()
    }

    pub fn npc_shop_open(&self) -> bool {
        self.is_npc_shop_open()
    }

    pub fn storage_open(&self) -> bool {
        self.is_storage_open()
    }

    pub fn minimap_visible(&self) -> bool {
        self.minimap_visible
    }

    pub fn chat_focused(&self) -> bool {
        self.chat_focused
    }

    pub fn blocks_gameplay_keys(&self) -> bool {
        self.chat_focused
    }

    pub fn blocks_world_click(&self) -> bool {
        self.panel != UiPanel::None || self.chat_focused
    }

    pub fn mail_compose_open(&self) -> bool {
        self.mail_compose.is_some()
    }

    pub fn change_password_open(&self) -> bool {
        self.security.panel == UiSecurityPanel::ChangePassword
    }

    pub fn safe_key_open(&self) -> bool {
        self.security.panel == UiSecurityPanel::SafeKey
    }
}
